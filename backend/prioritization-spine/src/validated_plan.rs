//! `ValidatedPlan` — M2 carrier-B record.
//!
//! M2 makes `relays-client` broadcast the SAME `executeArbitrage` transaction
//! that `sim-ctl` validated. To reproduce byte-identical calldata, the broadcast
//! path must encode from the EXACT inputs the sim path encoded from. The encoder
//! [`crate::execute_arbitrage_encoder::build_execute_arbitrage_calldata`] takes
//! `(ctx, route_hash, min_profit_wei, executor_address)` — so carrier-B persists
//! precisely those four inputs keyed by `opp.id` at sim-validate time, and the
//! broadcast path reads them back to call the SAME encoder.
//!
//! This module defines ONLY the serializable carrier type. It does NOT touch the
//! fund path, the broadcast retarget, or any persistence wiring — those land in
//! later M2 increments.

use crate::round_trip_executor::RoundTripContext;
use crate::round_trip_executor::{SimulationEvidence, SimulationOutcome};
use ethers::types::{Address, H256, U256};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Carrier-B record: the exact validated inputs to
/// [`crate::execute_arbitrage_encoder::build_execute_arbitrage_calldata`],
/// persisted by `opp.id` at sim-validate time and read by the broadcast path so
/// it reproduces byte-identical `executeArbitrage` calldata (sim↔exec parity).
///
/// `route_hash` is a fixed 32-byte array. serde's const-generic array impls
/// (available since serde 1.0.123) cover `[u8; 32]` directly, so the plain
/// derive round-trips it with no custom adapter — it serializes as a JSON array
/// of 32 byte values and deserializes back to the identical bytes.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SimulationBinding {
    pub schema_version: u8,
    pub opportunity_id: Uuid,
    pub strategy_kind: String,
    pub chain_id: u64,
    pub block_number: u64,
    pub block_hash: H256,
    pub block_timestamp: u64,
    pub simulated_at_ms: i64,
    pub caller: Address,
    pub calldata_hash: H256,
    pub flash_loan_executor: Address,
    pub gas_price_wei: U256,
    pub gas_limit: u64,
    pub gas_used: u64,
    pub retained_profit_wei: U256,
    pub flash_fee_wei: U256,
    pub forward_quote: U256,
    pub backward_quote: U256,
    pub total_slippage_bps: u16,
    pub state_overrides_used: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidatedPlan {
    pub ctx: RoundTripContext,
    pub route_hash: [u8; 32],
    pub min_profit_wei: U256,
    pub executor_address: Address,
    /// The EXACT sim-validated wrapped-flash calldata
    /// (`build_flash_funded_broadcast_calldata_with_intermediate` output: outer
    /// `requestFlashLoan` 0x5107d61e wrapping inner `executeArbitrageFlashFunded`
    /// 0xdde0bf51). Produced by `sim_multistep::execute_multistep_revm` and filled
    /// from `SimulationOutcome.wrapped_calldata` ONLY on SIM_SUCCESS. The broadcast
    /// path sends these bytes VERBATIM — real byte-parity, not a re-encode. serde
    /// round-trips `Vec<u8>` as a JSON array of byte values.
    pub wrapped_calldata: Vec<u8>,
    /// Missing in older carriers: readable for history, never valid for broadcast.
    #[serde(default)]
    pub binding: Option<SimulationBinding>,
}

/// Hash of all encoder inputs, including the public EOA. This prevents attaching
/// evidence from one simulation to a different route/context at the producer.
pub fn plan_inputs_hash(
    ctx: &RoundTripContext,
    route_hash: [u8; 32],
    min_profit: U256,
    executor: Address,
) -> Result<H256, &'static str> {
    let bytes = serde_json::to_vec(&(ctx, route_hash, min_profit, executor))
        .map_err(|_| "plan_inputs_serialization_failed")?;
    Ok(H256::from(ethers::utils::keccak256(bytes)))
}

impl ValidatedPlan {
    /// Sole production constructor. The simulation supplies state/execution
    /// evidence; the candidate producer attaches identity only after success.
    pub fn from_simulation(
        ctx: RoundTripContext,
        opportunity_id: Uuid,
        strategy_kind: impl Into<String>,
        route_hash: [u8; 32],
        min_profit_wei: U256,
        executor_address: Address,
        outcome: &SimulationOutcome,
    ) -> Result<Self, &'static str> {
        let e: &SimulationEvidence = outcome
            .evidence
            .as_ref()
            .ok_or("simulation_evidence_missing")?;
        let bytes = outcome
            .wrapped_calldata
            .as_ref()
            .filter(|b| !b.is_empty())
            .ok_or("wrapped_calldata_missing")?;
        let strategy_kind = strategy_kind.into();
        if !outcome.passed || opportunity_id.is_nil() || strategy_kind.trim().is_empty() {
            return Err("simulation_or_identity_invalid");
        }
        if e.state_overrides_used
            || e.block_hash == H256::zero()
            || e.chain_id == 0
            || e.block_number == 0
            || e.block_timestamp == 0
            || e.simulated_at_ms <= 0
            || e.caller != ctx.caller
            || e.flash_loan_executor == Address::zero()
            || outcome.gas_used_total == 0
            || e.gas_limit < outcome.gas_used_total
            || outcome.gas_price_wei.is_zero()
            || e.total_slippage_bps > 50
            || min_profit_wei <= e.flash_fee_wei
            || min_profit_wei != e.min_profit_wei
            || e.forward_quote.is_zero()
            || e.backward_quote.is_zero()
            || outcome.simulated_profit_token_in.is_zero()
        {
            return Err("simulation_evidence_invalid");
        }
        if e.plan_inputs_hash
            != plan_inputs_hash(&ctx, route_hash, min_profit_wei, executor_address)?
            || e.calldata_hash != H256::from(ethers::utils::keccak256(bytes))
        {
            return Err("simulation_binding_mismatch");
        }
        Ok(Self {
            ctx,
            route_hash,
            min_profit_wei,
            executor_address,
            wrapped_calldata: bytes.clone(),
            binding: Some(SimulationBinding {
                schema_version: 1,
                opportunity_id,
                strategy_kind,
                chain_id: e.chain_id,
                block_number: e.block_number,
                block_hash: e.block_hash,
                block_timestamp: e.block_timestamp,
                simulated_at_ms: e.simulated_at_ms,
                caller: e.caller,
                calldata_hash: e.calldata_hash,
                flash_loan_executor: e.flash_loan_executor,
                gas_price_wei: outcome.gas_price_wei,
                gas_limit: e.gas_limit,
                gas_used: outcome.gas_used_total,
                flash_fee_wei: e.flash_fee_wei,
                retained_profit_wei: outcome.simulated_profit_token_in,
                total_slippage_bps: e.total_slippage_bps,
                forward_quote: e.forward_quote,
                backward_quote: e.backward_quote,
                state_overrides_used: e.state_overrides_used,
            }),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use shared_rs::chains::{USDC_MAINNET, WETH_MAINNET};
    use std::str::FromStr;

    fn addr(hex: &str) -> Address {
        Address::from_str(hex).expect("valid hex address")
    }

    fn weth() -> Address {
        addr(WETH_MAINNET)
    }
    fn usdc() -> Address {
        addr(USDC_MAINNET)
    }

    fn fixed_plan() -> ValidatedPlan {
        let ctx = RoundTripContext {
            caller: addr("0x1111111111111111111111111111111111111111"),
            token_in: weth(),
            token_out: usdc(),
            amount_in: U256::from(10_u64).pow(U256::from(18)),
            forward_router: addr("0x7a250d5630b4cf539739df2c5dacb4c659f2488d"),
            forward_path: vec![weth(), usdc()],
            backward_router: addr("0x68b3465833fb72A70ecDF485E0e4C7bD8665Fc45"),
            backward_path: vec![usdc(), weth()],
            deadline: U256::from(1_700_000_000u64),
        };
        ValidatedPlan {
            ctx,
            route_hash: [0x11u8; 32],
            min_profit_wei: U256::from(1u64),
            executor_address: addr("0x2222222222222222222222222222222222222222"),
            // Non-trivial, non-uniform bytes (a plausible wrapped-flash calldata
            // prefix: requestFlashLoan 0x5107d61e) so the round-trip test catches
            // truncation / reordering in the byte-array serde path.
            wrapped_calldata: vec![0x51, 0x07, 0xd6, 0x1e, 0xde, 0xad, 0xbe, 0xef],
            binding: None,
        }
    }

    /// Full JSON round trip: serialize → deserialize → compare field-by-field.
    ///
    /// We compare fields explicitly (rather than deriving `PartialEq` on
    /// `RoundTripContext`) to keep `RoundTripContext`'s derive set minimal — the
    /// only derive this increment adds to it is serde, which is purely additive
    /// to the sim path.
    #[test]
    fn validated_plan_json_round_trips() {
        let original = fixed_plan();

        let json = serde_json::to_string(&original).expect("serialize ValidatedPlan");
        let decoded: ValidatedPlan =
            serde_json::from_str(&json).expect("deserialize ValidatedPlan");

        // Load-bearing fields: these are what the broadcast path feeds back into
        // the encoder, so they MUST survive the round trip byte-for-byte.
        assert_eq!(
            decoded.route_hash, original.route_hash,
            "route_hash must survive the JSON round trip"
        );
        assert_eq!(
            decoded.executor_address, original.executor_address,
            "executor_address must survive the JSON round trip"
        );
        assert_eq!(decoded.min_profit_wei, original.min_profit_wei);

        // wrapped_calldata is broadcast VERBATIM, so it MUST survive byte-for-byte.
        assert_eq!(
            decoded.wrapped_calldata, original.wrapped_calldata,
            "wrapped_calldata (broadcast verbatim) must survive the JSON round trip"
        );

        // RoundTripContext (field-by-field — no PartialEq derive on it).
        assert_eq!(decoded.ctx.caller, original.ctx.caller);
        assert_eq!(decoded.ctx.token_in, original.ctx.token_in);
        assert_eq!(decoded.ctx.token_out, original.ctx.token_out);
        assert_eq!(decoded.ctx.amount_in, original.ctx.amount_in);
        assert_eq!(decoded.ctx.forward_router, original.ctx.forward_router);
        assert_eq!(decoded.ctx.forward_path, original.ctx.forward_path);
        assert_eq!(decoded.ctx.backward_router, original.ctx.backward_router);
        assert_eq!(decoded.ctx.backward_path, original.ctx.backward_path);
        assert_eq!(decoded.ctx.deadline, original.ctx.deadline);
    }

    /// Explicit guard that the full 32-byte `route_hash` (not just a prefix)
    /// is preserved — a distinct, non-uniform pattern catches truncation or
    /// element-reordering bugs in any future custom serde adapter.
    #[test]
    fn route_hash_full_32_bytes_preserved() {
        let mut plan = fixed_plan();
        let mut hash = [0u8; 32];
        for (i, b) in hash.iter_mut().enumerate() {
            *b = i as u8;
        }
        plan.route_hash = hash;

        let json = serde_json::to_string(&plan).expect("serialize");
        let decoded: ValidatedPlan = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(decoded.route_hash, hash);
    }

    fn evidence_for(plan: &ValidatedPlan) -> SimulationOutcome {
        let bytes = plan.wrapped_calldata.clone();
        SimulationOutcome {
            passed: true,
            simulated_profit_token_in: U256::from(20),
            intermediate_amount_out: Some(U256::from(100)),
            gas_used_total: 50000,
            gas_price_wei: U256::from(100),
            fail_reason: None,
            wrapped_calldata: Some(bytes.clone()),
            evidence: Some(SimulationEvidence {
                chain_id: 1,
                block_number: 100,
                block_hash: H256::repeat_byte(1),
                block_timestamp: 1700000000,
                simulated_at_ms: 1700000000000,
                caller: plan.ctx.caller,
                calldata_hash: H256(ethers::utils::keccak256(bytes)),
                plan_inputs_hash: plan_inputs_hash(
                    &plan.ctx,
                    plan.route_hash,
                    plan.min_profit_wei,
                    plan.executor_address,
                )
                .unwrap(),
                flash_loan_executor: Address::from_low_u64_be(55),
                gas_limit: 100000,
                flash_fee_wei: U256::zero(),
                min_profit_wei: plan.min_profit_wei,
                total_slippage_bps: 50,
                state_overrides_used: false,
                forward_quote: U256::from(100),
                backward_quote: U256::from(120),
            }),
        }
    }

    #[test]
    fn analysis_requires_real_evidence_to_authorize() {
        let plan = fixed_plan();
        let id = Uuid::new_v4();
        let outcome = evidence_for(&plan);
        let upgrade = |o: &SimulationOutcome| {
            ValidatedPlan::from_simulation(
                plan.ctx.clone(),
                id,
                "dex_arb",
                plan.route_hash,
                plan.min_profit_wei,
                plan.executor_address,
                o,
            )
        };
        assert!(upgrade(&outcome).is_ok());
        let mut paper = outcome.clone();
        paper.evidence.as_mut().unwrap().state_overrides_used = true;
        assert!(upgrade(&paper).is_err());
        paper.evidence = None;
        assert!(upgrade(&paper).is_err());
        // The analysis remains reusable: attach the subsequent real simulation.
        assert!(upgrade(&outcome).is_ok());
    }

    #[test]
    fn execution_evidence_rejects_context_and_calldata_substitution() {
        let plan = fixed_plan();
        let outcome = evidence_for(&plan);
        let mut changed = plan.ctx.clone();
        changed.amount_in += U256::one();
        assert!(ValidatedPlan::from_simulation(
            changed,
            Uuid::new_v4(),
            "dex_arb",
            plan.route_hash,
            plan.min_profit_wei,
            plan.executor_address,
            &outcome
        )
        .is_err());
        let mut altered = outcome.clone();
        altered.wrapped_calldata.as_mut().unwrap().push(0);
        assert!(ValidatedPlan::from_simulation(
            plan.ctx.clone(),
            Uuid::new_v4(),
            "dex_arb",
            plan.route_hash,
            plan.min_profit_wei,
            plan.executor_address,
            &altered
        )
        .is_err());
        let mut zero = outcome;
        zero.evidence.as_mut().unwrap().forward_quote = U256::zero();
        assert!(ValidatedPlan::from_simulation(
            plan.ctx,
            Uuid::new_v4(),
            "dex_arb",
            plan.route_hash,
            plan.min_profit_wei,
            plan.executor_address,
            &zero
        )
        .is_err());
    }
}
