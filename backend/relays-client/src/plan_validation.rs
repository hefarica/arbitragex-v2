//! Validation of the exact calldata approved by simulation. No reconstruction
//! from labels and no integer-to-float conversion in principal/fee gates.
use alloy::providers::Provider as _;
use ethers::abi::{decode, encode, ParamType, Token};
use ethers::types::{Address, H256, U256};
use prioritization_spine::{
    ValidatedPlan, EXECUTE_ARBITRAGE_FLASH_FUNDED_SELECTOR, REQUEST_FLASH_LOAN_SELECTOR,
};
use shared_rs::{
    contracts::{Opportunity, StrategyKind},
    rpc_failover::AlloyHttpProvider,
};
use std::str::FromStr;

type Check<T> = Result<T, &'static str>;

pub fn supports_strategy(kind: &StrategyKind) -> bool {
    matches!(
        kind.as_str(),
        "dex_arb"
            | "flashloan_arb"
            | "triangular"
            | "mev_01_001_dex_dex_arbitrage"
            | "mev_01_002_cross_pool_arbitrage"
            | "mev_01_008_amm_amm_arbitrage"
            | "mev_01_015_two_leg_arbitrage"
            | "mev_01_016_triangular_arbitrage"
            | "mev_01_017_quadrangular_arbitrage"
            | "mev_01_018_n_leg_cyclic_arbitrage"
            | "mev_01_019_multi_hop_arbitrage"
    )
}
fn uint(t: &Token) -> Check<U256> {
    t.clone().into_uint().ok_or("abi_uint")
}
fn addr(t: &Token) -> Check<Address> {
    t.clone().into_address().ok_or("abi_address")
}
fn bytes(t: &Token) -> Check<Vec<u8>> {
    t.clone().into_bytes().ok_or("abi_bytes")
}
fn array(t: &Token) -> Check<Vec<Token>> {
    t.clone().into_array().ok_or("abi_array")
}
fn canonical(types: &[ParamType], input: &[u8]) -> Check<Vec<Token>> {
    let decoded = decode(types, input).map_err(|_| "abi_decode")?;
    if encode(&decoded) != input {
        return Err("abi_noncanonical_or_trailing_data");
    }
    Ok(decoded)
}

pub fn validate_calldata(plan: &ValidatedPlan) -> Check<()> {
    let ctx = &plan.ctx;
    if plan.executor_address.is_zero()
        || ctx.caller.is_zero()
        || ctx.token_in.is_zero()
        || ctx.token_out.is_zero()
        || ctx.token_in == ctx.token_out
        || ctx.amount_in.is_zero()
        || ctx.forward_router.is_zero()
        || ctx.backward_router.is_zero()
        || plan.min_profit_wei.is_zero()
    {
        return Err("invalid_plan_context");
    }
    let raw = &plan.wrapped_calldata;
    if raw.len() > 32_768 || raw.get(..4) != Some(REQUEST_FLASH_LOAN_SELECTOR.as_slice()) {
        return Err("outer_selector_or_size");
    }
    let outer = canonical(
        &[ParamType::Address, ParamType::Uint(256), ParamType::Bytes],
        &raw[4..],
    )?;
    if addr(&outer[0])? != ctx.token_in || uint(&outer[1])? != ctx.amount_in {
        return Err("outer_principal_mismatch");
    }
    let inner = bytes(&outer[2])?;
    if inner.get(..4) != Some(EXECUTE_ARBITRAGE_FLASH_FUNDED_SELECTOR.as_slice()) {
        return Err("inner_selector");
    }
    let decoded = canonical(
        &[
            ParamType::FixedBytes(32),
            ParamType::Address,
            ParamType::Address,
            ParamType::Uint(256),
            ParamType::Uint(256),
            ParamType::Array(Box::new(ParamType::Address)),
            ParamType::Array(Box::new(ParamType::Bytes)),
        ],
        &inner[4..],
    )?;
    if decoded[0].clone().into_fixed_bytes().as_deref() != Some(plan.route_hash.as_slice())
        || addr(&decoded[1])? != ctx.token_in
        || addr(&decoded[2])? != ctx.token_out
        || uint(&decoded[3])? != ctx.amount_in
        || uint(&decoded[4])? != plan.min_profit_wei
    {
        return Err("inner_context_mismatch");
    }
    let routers = array(&decoded[5])?;
    let payloads = array(&decoded[6])?;
    if routers.len() != 2 || payloads.len() != 2 {
        return Err("two_router_calls_required");
    }
    if addr(&routers[0])? != ctx.forward_router || addr(&routers[1])? != ctx.backward_router {
        return Err("router_mismatch");
    }
    let binding = plan.binding.as_ref().ok_or("simulation_binding_missing")?;
    let paths = [&ctx.forward_path, &ctx.backward_path];
    let endpoints = [(ctx.token_in, ctx.token_out), (ctx.token_out, ctx.token_in)];
    let mut hops = 0usize;
    for i in 0..2 {
        let path = paths[i];
        if path.len() < 2
            || path.len() > 7
            || path.first().copied() != Some(endpoints[i].0)
            || path.last().copied() != Some(endpoints[i].1)
            || path.iter().any(Address::is_zero)
            || path.windows(2).any(|w| w[0] == w[1])
        {
            return Err("invalid_token_path");
        }
        hops += path.len() - 1;
        let leg = bytes(&payloads[i])?;
        if leg.get(..4) != Some([0x38, 0xed, 0x17, 0x39].as_slice()) {
            return Err("unsupported_swap_selector");
        }
        let swap = canonical(
            &[
                ParamType::Uint(256),
                ParamType::Uint(256),
                ParamType::Array(Box::new(ParamType::Address)),
                ParamType::Address,
                ParamType::Uint(256),
            ],
            &leg[4..],
        )?;
        let actual_path: Vec<Address> = array(&swap[2])?.iter().map(addr).collect::<Check<_>>()?;
        if &actual_path != path
            || addr(&swap[3])? != plan.executor_address
            || uint(&swap[4])? != ctx.deadline
        {
            return Err("swap_path_recipient_deadline_mismatch");
        }
        if uint(&swap[0])?.is_zero() || uint(&swap[1])?.is_zero() {
            return Err("unbounded_swap_amount");
        }
        if i == 0 && uint(&swap[0])? != ctx.amount_in {
            return Err("forward_amount_mismatch");
        }
        let (input, minimum) = if i == 0 {
            (ctx.amount_in, binding.forward_quote)
        } else {
            (
                binding.forward_quote,
                prioritization_spine::execute_arbitrage_encoder::minimum_after_slippage(
                    binding.backward_quote,
                    binding.total_slippage_bps,
                )
                .map_err(|_| "invalid_quote_bound")?,
            )
        };
        if uint(&swap[0])? != input || uint(&swap[1])? < minimum {
            return Err("quoted_amount_or_slippage_mismatch");
        }
    }
    if !(2..=7).contains(&hops) {
        return Err("hops_outside_2_7");
    }
    Ok(())
}

pub fn validate_binding(
    opp: &Opportunity,
    plan: &ValidatedPlan,
    signer: Address,
    fle: Address,
    now_ms: i64,
) -> Check<()> {
    if !supports_strategy(&opp.strategy_kind) {
        return Err("unsupported_strategy");
    }
    let b = plan.binding.as_ref().ok_or("simulation_binding_missing")?;
    if b.schema_version != 1
        || b.opportunity_id != opp.id
        || b.chain_id != opp.chain_id
        || b.strategy_kind != opp.strategy_kind.as_str()
        || b.caller != signer
        || plan.ctx.caller != signer
        || b.flash_loan_executor != fle
        || fle.is_zero()
        || b.state_overrides_used
    {
        return Err("simulation_identity_or_overrides_mismatch");
    }
    if b.block_hash.is_zero()
        || b.block_number == 0
        || b.block_timestamp == 0
        || b.simulated_at_ms <= 0
        || b.simulated_at_ms > now_ms
        || now_ms.saturating_sub(b.simulated_at_ms) > 30_000
    {
        return Err("simulation_snapshot_stale");
    }
    if b.calldata_hash != H256(ethers::utils::keccak256(&plan.wrapped_calldata)) {
        return Err("calldata_hash_mismatch");
    }
    if b.gas_used == 0
        || b.gas_limit < b.gas_used
        || b.gas_price_wei.is_zero()
        || b.total_slippage_bps > 50
        || b.retained_profit_wei.is_zero()
        || plan.min_profit_wei <= b.flash_fee_wei
    {
        return Err("simulation_economics_invalid");
    }
    if Address::from_str(&opp.token_in).ok() != Some(plan.ctx.token_in)
        || Address::from_str(&opp.token_out).ok() != Some(plan.ctx.token_out)
        || U256::from_dec_str(&opp.amount_in_wei).ok() != Some(plan.ctx.amount_in)
    {
        return Err("opportunity_principal_mismatch");
    }
    if plan.ctx.deadline <= U256::from((now_ms / 1000).max(0) as u64) {
        return Err("plan_expired");
    }
    validate_calldata(plan)
}

pub fn principal_cap(
    chain: u64,
    token: Address,
    max_eth: f64,
    raw_override: Option<&str>,
) -> Check<U256> {
    if let Some(raw) = raw_override {
        let cap = U256::from_dec_str(raw).map_err(|_| "invalid_raw_principal_cap")?;
        if cap.is_zero() {
            return Err("invalid_raw_principal_cap");
        }
        return Ok(cap);
    }
    let canonical_weth = match chain {
        1 => "0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2",
        11_155_111 => "0xfff9976782d46cc05630d1f6ebab18b2324d6b14",
        _ => return Err("raw_asset_principal_cap_required"),
    };
    if Address::from_str(canonical_weth).ok() != Some(token) {
        return Err("raw_asset_principal_cap_required");
    }
    if !max_eth.is_finite() || max_eth <= 0.0 {
        return Err("invalid_eth_principal_cap");
    }
    ethers::utils::parse_units(format!("{max_eth:.18}"), 18)
        .map(U256::from)
        .map_err(|_| "invalid_eth_principal_cap")
}

async fn pinned_call(
    provider: &AlloyHttpProvider,
    target: Address,
    signature: &str,
    args: &[Token],
    hash: H256,
) -> Check<Vec<u8>> {
    let mut data = ethers::utils::keccak256(signature)[..4].to_vec();
    data.extend(encode(args));
    let result: String = provider
        .raw_request(
            "eth_call".into(),
            serde_json::json!([
              {"to":format!("{target:#x}"),"data":format!("0x{}",hex::encode(data))},
              {"blockHash":format!("{hash:#x}"),"requireCanonical":true}
            ]),
        )
        .await
        .map_err(|_| "pinned_contract_read_failed")?;
    hex::decode(result.strip_prefix("0x").ok_or("contract_read_not_hex")?)
        .map_err(|_| "contract_read_not_hex")
}
async fn read_addr(
    provider: &AlloyHttpProvider,
    target: Address,
    sig: &str,
    hash: H256,
) -> Check<Address> {
    let b = pinned_call(provider, target, sig, &[], hash).await?;
    let t = canonical(&[ParamType::Address], &b)?;
    addr(&t[0])
}
async fn read_uint(
    provider: &AlloyHttpProvider,
    target: Address,
    sig: &str,
    args: &[Token],
    hash: H256,
) -> Check<U256> {
    let b = pinned_call(provider, target, sig, args, hash).await?;
    let t = canonical(&[ParamType::Uint(256)], &b)?;
    uint(&t[0])
}

pub fn fee_math(amount: U256, rate: U256, denominator: U256, ceil: bool) -> Check<U256> {
    if rate > denominator || denominator.is_zero() {
        return Err("invalid_fee_rate");
    }
    let bias = if ceil {
        denominator - U256::one()
    } else {
        denominator / 2
    };
    amount
        .checked_mul(rate)
        .and_then(|n| n.checked_add(bias))
        .map(|n| n / denominator)
        .ok_or("fee_overflow")
}

/// Check the actual FLE wiring at the canonical block. The deployed Aave adapter
/// forwards a different initiator, so only the direct pool path is compatible.
/// The supported adapter branch is Balancer V2 with the same configured Vault.
pub async fn validate_funding(provider: &AlloyHttpProvider, plan: &ValidatedPlan) -> Check<()> {
    let b = plan.binding.as_ref().ok_or("simulation_binding_missing")?;
    let fle = b.flash_loan_executor;
    let hash = b.block_hash;
    if read_addr(provider, fle, "arbitrageExecutor()", hash).await? != plan.executor_address {
        return Err("executor_wiring_mismatch");
    }
    let adapter = read_addr(provider, fle, "flashLoanProvider()", hash).await?;
    let fee = if adapter.is_zero() {
        let pool = read_addr(provider, fle, "aavePool()", hash).await?;
        if pool.is_zero() {
            return Err("aave_pool_missing");
        }
        let rate = read_uint(provider, pool, "FLASHLOAN_PREMIUM_TOTAL()", &[], hash).await?;
        fee_math(plan.ctx.amount_in, rate, U256::from(10_000), false)?
    } else {
        let vault = read_addr(provider, fle, "balancerVault()", hash).await?;
        if vault.is_zero() || read_addr(provider, adapter, "vault()", hash).await? != vault {
            return Err("unsupported_funding_adapter");
        }
        let capacity = read_uint(
            provider,
            plan.ctx.token_in,
            "balanceOf(address)",
            &[Token::Address(vault)],
            hash,
        )
        .await?;
        if capacity < plan.ctx.amount_in {
            return Err("flash_capacity_insufficient");
        }
        let collector = read_addr(provider, vault, "getProtocolFeesCollector()", hash).await?;
        let rate = read_uint(
            provider,
            collector,
            "getFlashLoanFeePercentage()",
            &[],
            hash,
        )
        .await?;
        fee_math(plan.ctx.amount_in, rate, U256::exp10(18), true)?
    };
    if fee != b.flash_fee_wei || plan.min_profit_wei <= fee {
        return Err("flash_fee_changed_or_unprofitable");
    }
    Ok(())
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use prioritization_spine::execute_arbitrage_encoder::build_flash_funded_broadcast_calldata_with_minima;
    use prioritization_spine::{
        round_trip_executor::RoundTripContext, validated_plan::SimulationBinding,
    };
    fn plan() -> ValidatedPlan {
        let ctx = RoundTripContext {
            caller: Address::from_low_u64_be(1),
            token_in: Address::from_low_u64_be(2),
            token_out: Address::from_low_u64_be(3),
            amount_in: (U256::one() << 201) + U256::from(17),
            forward_router: Address::from_low_u64_be(4),
            backward_router: Address::from_low_u64_be(5),
            forward_path: vec![Address::from_low_u64_be(2), Address::from_low_u64_be(3)],
            backward_path: vec![Address::from_low_u64_be(3), Address::from_low_u64_be(2)],
            deadline: U256::from(1800000120u64),
        };
        let bytes = build_flash_funded_broadcast_calldata_with_minima(
            &ctx,
            U256::from(1000),
            U256::from(1000),
            U256::from(1990),
            [7; 32],
            U256::from(10),
            Address::from_low_u64_be(6),
        )
        .unwrap();
        let binding = SimulationBinding {
            schema_version: 1,
            opportunity_id: uuid::Uuid::new_v4(),
            strategy_kind: "dex_arb".into(),
            chain_id: 1,
            block_number: 100,
            block_hash: H256::repeat_byte(8),
            block_timestamp: 1800000000,
            simulated_at_ms: 1800000000000,
            caller: ctx.caller,
            calldata_hash: H256(ethers::utils::keccak256(&bytes)),
            flash_loan_executor: Address::from_low_u64_be(9),
            gas_price_wei: U256::from(1),
            gas_limit: 100000,
            gas_used: 50000,
            flash_fee_wei: U256::from(5),
            retained_profit_wei: U256::from(20),
            forward_quote: U256::from(1000),
            backward_quote: U256::from(2000),
            total_slippage_bps: 50,
            state_overrides_used: false,
        };
        ValidatedPlan {
            ctx,
            route_hash: [7; 32],
            min_profit_wei: U256::from(10),
            executor_address: Address::from_low_u64_be(6),
            wrapped_calldata: bytes,
            binding: Some(binding),
        }
    }
    #[test]
    fn calldata_exact_u256_and_nested_context() {
        let p = plan();
        assert!(validate_calldata(&p).is_ok());
        let mut changed = p.clone();
        changed.ctx.amount_in += U256::one();
        assert_eq!(validate_calldata(&changed), Err("outer_principal_mismatch"));
        changed = p.clone();
        changed.ctx.deadline += U256::one();
        assert_eq!(
            validate_calldata(&changed),
            Err("swap_path_recipient_deadline_mismatch")
        );
        changed = p.clone();
        changed.wrapped_calldata.push(0);
        assert_eq!(
            validate_calldata(&changed),
            Err("abi_noncanonical_or_trailing_data")
        );
        changed = p.clone();
        changed.binding.as_mut().unwrap().forward_quote += U256::one();
        assert_eq!(
            validate_calldata(&changed),
            Err("quoted_amount_or_slippage_mismatch")
        );
        changed = p;
        changed.binding.as_mut().unwrap().backward_quote = U256::from(3000);
        assert_eq!(
            validate_calldata(&changed),
            Err("quoted_amount_or_slippage_mismatch")
        );
    }
    #[test]
    fn exact_asset_caps_never_use_eth_for_other_tokens() {
        let raw = ((U256::one() << 210) + U256::from(19)).to_string();
        assert_eq!(
            principal_cap(1, Address::from_low_u64_be(2), 1.0, Some(&raw))
                .unwrap()
                .to_string(),
            raw
        );
        assert!(principal_cap(1, Address::from_low_u64_be(2), 1.0, None).is_err());
        assert!(principal_cap(1, Address::zero(), 1.0, Some("0")).is_err());
    }
    #[test]
    fn all_analysis_types_need_a_matching_execution_adapter() {
        assert!(supports_strategy(&StrategyKind::triangular()));
        assert!(!supports_strategy(&StrategyKind::liquidation()));
    }
}
