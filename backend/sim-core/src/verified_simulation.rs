//! Production evidence producer. Never applies storage or balance overrides.
use crate::sim_multistep::MultiStepExecutionConfig;
use anyhow::{anyhow, ensure, Result};
use ethers::types::{Address, H256, U256};
use prioritization_spine::execute_arbitrage_encoder::{
    build_flash_funded_broadcast_calldata_with_minima, minimum_after_slippage,
};
use prioritization_spine::round_trip_executor::{
    RoundTripContext, SimulationEvidence, SimulationOutcome,
};
use simulator_v2::sequence_runner::{CallOutcome, SequenceCall, SequenceContext};
use std::sync::Arc;

fn addr(a: Address) -> simulator_v2::AlloyAddress {
    simulator_v2::AlloyAddress::from_slice(a.as_bytes())
}
fn amount(v: U256) -> simulator_v2::AlloyU256 {
    let mut b = [0; 32];
    v.to_big_endian(&mut b);
    simulator_v2::AlloyU256::from_be_bytes(b)
}
fn ethers_amount(v: simulator_v2::AlloyU256) -> U256 {
    U256::from_big_endian(&v.to_be_bytes::<32>())
}

pub fn execute(
    ctx: &RoundTripContext,
    simulator: Arc<simulator_v2::SimulatorV2>,
    config: &MultiStepExecutionConfig,
) -> SimulationOutcome {
    match run(ctx, simulator, config) {
        Ok(outcome) => outcome,
        Err(e) => SimulationOutcome::failed(format!("verified_simulation:{e}")),
    }
}

fn run(
    ctx: &RoundTripContext,
    simulator: Arc<simulator_v2::SimulatorV2>,
    c: &MultiStepExecutionConfig,
) -> Result<SimulationOutcome> {
    ensure!(
        !c.paper_mode && !c.enable_storage_cheats,
        "state_overrides_forbidden"
    );
    ensure!(
        !ctx.caller.is_zero()
            && !ctx.amount_in.is_zero()
            && !c.executor_address.is_zero()
            && c.gas_limit_per_step > 21_000
            && c.max_steps >= 3,
        "invalid_execution_config"
    );
    let lazy = simulator_v2::LazyDb::new_verified(&simulator.rpc_url, simulator.pinned_block())?;
    let s = lazy
        .snapshot()
        .ok_or_else(|| anyhow!("snapshot_missing"))?
        .clone();
    ensure!(
        s.chain_id == c.chain_id && ctx.deadline > U256::from(s.timestamp),
        "snapshot_chain_or_deadline"
    );
    let (block, spec) =
        simulator_v2::verified_environment::environment(&s).map_err(|e| anyhow!(e))?;
    let mut gas_limit = c.gas_limit_per_step.min(block.gas_limit);
    if spec >= simulator_v2::verified_environment::osaka_spec() {
        gas_limit = gas_limit.min(16_777_216);
    }
    let gas_price = if c.gas_price_wei.is_zero() {
        lazy.gas_price()?
    } else {
        c.gas_price_wei
    };
    ensure!(
        !gas_price.is_zero()
            && gas_price <= U256::from(u128::MAX)
            && gas_price >= U256::from(block.basefee),
        "gas_price_invalid"
    );
    let fle = shared_rs::chains::resolve_flashloan_executor_address(c.chain_id)?;
    let role = ethers::utils::keccak256("EXECUTOR_ROLE");
    for (contract, grantee, reason) in [
        (
            fle,
            ctx.caller,
            "missing_onchain_role:caller_to_flash_executor",
        ),
        (
            c.executor_address,
            fle,
            "missing_onchain_role:flash_executor_to_arbitrage_executor",
        ),
    ] {
        let granted = lazy.run_rpc_future(shared_rs::flashloan_math::call_word(
            lazy.provider(),
            contract,
            "hasRole(bytes32,address)",
            &[
                ethers::abi::Token::FixedBytes(role.to_vec()),
                ethers::abi::Token::Address(grantee),
            ],
            s.hash,
        ))??;
        ensure!(
            granted == U256::one(),
            "{reason}:contract={contract:#x}:grantee={grantee:#x}"
        );
    }
    let funding =
        lazy.run_rpc_future(shared_rs::flashloan_math::resolve_executor_flash_quote(
            lazy.provider(),
            fle,
            ctx.token_in,
            ctx.amount_in,
            s.hash,
        ))??;
    let actual_executor = lazy.run_rpc_future(shared_rs::flashloan_math::call_address(
        lazy.provider(),
        fle,
        "arbitrageExecutor()",
        &[],
        s.hash,
    ))??;
    ensure!(
        actual_executor == c.executor_address,
        "executor_wiring_mismatch"
    );
    let min_profit = c.min_profit_wei.max(
        funding
            .fee_wei
            .checked_add(U256::one())
            .ok_or_else(|| anyhow!("fee_overflow"))?,
    );
    let mut seq = SequenceContext::new_verified(lazy)?;
    let forward_path: Vec<_> = ctx.forward_path.iter().copied().map(addr).collect();
    let backward_path: Vec<_> = ctx.backward_path.iter().copied().map(addr).collect();
    let forward_quote = ethers_amount(seq.read_amounts_out(
        addr(ctx.forward_router),
        amount(ctx.amount_in),
        &forward_path,
    )?);
    ensure!(!forward_quote.is_zero(), "forward_quote_zero");
    // Consume the entire quoted intermediate. First-leg minOut is exact, so a
    // changed first-leg state reverts; the second leg gets the whole 50bp budget.
    let backward_quote = ethers_amount(seq.read_amounts_out(
        addr(ctx.backward_router),
        amount(forward_quote),
        &backward_path,
    )?);
    let backward_min = minimum_after_slippage(backward_quote, 50)?;
    let calldata = build_flash_funded_broadcast_calldata_with_minima(
        ctx,
        forward_quote,
        forward_quote,
        backward_min,
        c.route_hash,
        min_profit,
        c.executor_address,
    )?;
    let before = seq.read_balance(addr(ctx.token_in), addr(fle), "before")?;
    let execution = seq.call(SequenceCall {
        from: addr(ctx.caller),
        to: addr(fle),
        calldata: calldata.clone(),
        value_wei: 0,
        gas_price_wei: gas_price.as_u128(),
        gas_limit,
        label: "verified_flash",
    })?;
    ensure!(
        matches!(execution, CallOutcome::Success { .. }),
        "flash_execution_failed:{execution:?}"
    );
    let after = seq.read_balance(addr(ctx.token_in), addr(fle), "after")?;
    ensure!(after > before, "retained_profit_nonpositive");
    seq.assert_canonical()?;
    let result = seq.finalize();
    ensure!(
        result.successful_calls == 1 && result.gas_used_total > 0 && result.trace_hash != [0; 32],
        "execution_evidence_missing"
    );
    let simulated_at_ms = i64::try_from(
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_millis(),
    )?;
    Ok(SimulationOutcome {
        passed: true,
        simulated_profit_token_in: ethers_amount(after - before),
        intermediate_amount_out: Some(forward_quote),
        gas_used_total: result.gas_used_total,
        gas_price_wei: gas_price,
        fail_reason: None,
        evidence: Some(SimulationEvidence {
            chain_id: c.chain_id,
            block_number: s.number,
            block_hash: s.hash,
            block_timestamp: s.timestamp,
            simulated_at_ms,
            caller: ctx.caller,
            calldata_hash: H256(ethers::utils::keccak256(&calldata)),
            plan_inputs_hash: prioritization_spine::validated_plan::plan_inputs_hash(
                ctx,
                c.route_hash,
                min_profit,
                c.executor_address,
            )
            .map_err(|e| anyhow!(e))?,
            flash_loan_executor: fle,
            gas_limit,
            flash_fee_wei: funding.fee_wei,
            min_profit_wei: min_profit,
            total_slippage_bps: 50,
            state_overrides_used: false,
            forward_quote,
            backward_quote,
        }),
        wrapped_calldata: Some(calldata),
    })
}
