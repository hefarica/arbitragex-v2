//! Shared admission of real engine/cartridge candidates to simulation and emit.
//!
//! Every amount sent to REVM comes from the emitted decimal U256, never the
//! routing f64. All pool identities, prices and simulation state refer to one
//! block hash. Missing data is an explicit rejection, not an estimated PASS.

use anyhow::{anyhow, bail, ensure, Context};
use bigdecimal::{BigDecimal, ToPrimitive};
use ethers::types::{Address, H256, U256};
use once_cell::sync::Lazy;
use prioritization_spine::round_trip_executor::{RoundTripContext, SimulationOutcome};
use prioritization_spine::route_plan::RoutePlan;
use prioritization_spine::ValidatedPlan;
use serde::Serialize;
use serde_json::{json, Value};
use shared_rs::candidates::RouteMetadata;
use shared_rs::contracts::Opportunity;
use shared_rs::oracle_snapshot::{configured_chain, OracleRpc, Price};
use std::collections::HashSet;
use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{OwnedSemaphorePermit, Semaphore};

/// The permit lives through admission, persistence and publication. A timed-out
/// spawn_blocking retains its own Arc until the RPC/EVM task actually finishes.
static ADMISSION_SLOTS: Lazy<Arc<Semaphore>> = Lazy::new(|| Arc::new(Semaphore::new(4)));
pub type AdmissionPermit = Arc<OwnedSemaphorePermit>;

pub fn try_acquire() -> anyhow::Result<AdmissionPermit> {
    ADMISSION_SLOTS
        .clone()
        .try_acquire_owned()
        .map(Arc::new)
        .map_err(|_| anyhow!("candidate_simulation_capacity"))
}

/// Preserve the original analysis independently of execution eligibility. A
/// missing on-chain role must not erase or rewrite the analysis economics.
pub async fn preserve_analysis(
    opportunity: &Opportunity,
    route: Option<&RouteMetadata>,
    plan: Option<&RoutePlan>,
    redis: &mut redis::aio::ConnectionManager,
) -> anyhow::Result<()> {
    let value = serde_json::to_string(&json!({
        "schema_version":1,"opportunity":opportunity,"route":route,"route_plan":plan,
        "requires_real_execution_validation":true
    }))?;
    ensure!(value.len() <= 131072, "analysis_record_too_large");
    let _: () = redis::cmd("SET")
        .arg(format!("arbx:analysis_candidate:{}", opportunity.id))
        .arg(value)
        .arg("EX")
        .arg(86400)
        .query_async(redis)
        .await?;
    Ok(())
}

#[derive(Debug, Clone, Serialize)]
pub struct ValidatedEconomics {
    pub schema_version: u8,
    pub opportunity_id: uuid::Uuid,
    pub strategy_kind: String,
    pub chain_id: u64,
    pub block_number: u64,
    pub block_hash: H256,
    pub token_in: Address,
    pub token_in_decimals: u8,
    pub principal_wei: U256,
    pub principal_usd: f64,
    pub flash_fee_wei: U256,
    pub gas_used: u64,
    pub gas_price_wei: U256,
    pub gas_cost_usd: f64,
    pub gross_profit_usd: f64,
    pub simulated_net_profit_usd: f64,
    pub admitted_net_profit_usd: f64,
    pub token_price_answer: U256,
    pub token_price_decimals: u8,
    pub token_price_updated_at: u64,
    pub native_price_answer: U256,
    pub native_price_decimals: u8,
    pub native_price_updated_at: u64,
}

pub struct Admission {
    pub opportunity: Opportunity,
    pub plan: ValidatedPlan,
    pub economics: ValidatedEconomics,
}

fn address(text: &str) -> anyhow::Result<Address> {
    let value = Address::from_str(text).map_err(|_| anyhow!("candidate_address_invalid"))?;
    ensure!(value != Address::zero(), "candidate_address_zero");
    Ok(value)
}

/// Construct the two router calls while retaining all 2..7 pool legs. A single
/// contiguous router group is split after the first hop; a second group splits
/// at its boundary. A third group requires a different executor ABI.
pub fn build_context(
    opportunity: &Opportunity,
    route: &RouteMetadata,
    route_plan: &RoutePlan,
    caller: Address,
    deadline: U256,
) -> anyhow::Result<RoundTripContext> {
    let hops = route.pool_addresses.len();
    ensure!((2..=7).contains(&hops), "candidate_hops_unsupported");
    ensure!(
        route.token_addresses.len() == hops + 1 && route.dex_adapters.len() == hops,
        "candidate_topology_lengths"
    );
    ensure!(
        opportunity.chain_id > 0
            && opportunity.chain_id == route_plan.chain_id
            && route_plan.atomic
            && route_plan.legs.len() == hops,
        "candidate_route_plan_mismatch"
    );
    ensure!(
        !opportunity.id.is_nil() && !opportunity.strategy_kind.as_str().trim().is_empty(),
        "candidate_identity_missing"
    );
    if let Some(id) = &opportunity.cartridge_id {
        ensure!(
            id == opportunity.strategy_kind.as_str(),
            "candidate_strategy_identity_mismatch"
        );
    }
    ensure!(
        caller != Address::zero() && !deadline.is_zero(),
        "candidate_context_missing"
    );
    let tokens = route
        .token_addresses
        .iter()
        .map(|s| address(s))
        .collect::<anyhow::Result<Vec<_>>>()?;
    let pools = route
        .pool_addresses
        .iter()
        .map(|s| address(s))
        .collect::<anyhow::Result<Vec<_>>>()?;
    ensure!(
        pools.iter().copied().collect::<HashSet<_>>().len() == hops,
        "candidate_repeated_pool"
    );
    ensure!(
        tokens[0] == address(&opportunity.token_in)? && tokens.last() == tokens.first(),
        "candidate_not_closed"
    );
    ensure!(
        tokens.windows(2).all(|p| p[0] != p[1]),
        "candidate_same_token_hop"
    );
    // Decimal strings only, with no float or u128 intermediary.
    ensure!(
        !opportunity.amount_in_wei.is_empty()
            && opportunity
                .amount_in_wei
                .bytes()
                .all(|b| b.is_ascii_digit()),
        "candidate_principal_invalid"
    );
    let amount_in = U256::from_dec_str(&opportunity.amount_in_wei)
        .map_err(|_| anyhow!("candidate_principal_invalid"))?;
    ensure!(!amount_in.is_zero(), "candidate_principal_zero");
    let routers = route
        .dex_adapters
        .iter()
        .map(|label| {
            let kind = sim_core::sim_encoder::parse_dex_kind(label)
                .map_err(|_| anyhow!("candidate_adapter_unsupported"))?;
            sim_core::sim_encoder::resolve_router_address(opportunity.chain_id, kind)
                .map_err(|_| anyhow!("candidate_router_unavailable"))
        })
        .collect::<anyhow::Result<Vec<_>>>()?;
    for (i, leg) in route_plan.legs.iter().enumerate() {
        ensure!(
            leg.pool_is_active
                && address(&leg.token_in)? == tokens[i]
                && address(&leg.token_out)? == tokens[i + 1]
                && leg.pool_address.as_deref().map(address).transpose()? == Some(pools[i]),
            "candidate_route_plan_mismatch"
        );
        let kind = sim_core::sim_encoder::parse_dex_kind(&leg.protocol_type)
            .map_err(|_| anyhow!("candidate_protocol_unsupported"))?;
        ensure!(
            sim_core::sim_encoder::resolve_router_address(opportunity.chain_id, kind)
                .map_err(|_| anyhow!("candidate_router_unavailable"))?
                == routers[i],
            "candidate_adapter_plan_mismatch"
        );
    }
    let boundaries: Vec<_> = (1..hops)
        .filter(|&i| routers[i] != routers[i - 1])
        .collect();
    ensure!(boundaries.len() <= 1, "candidate_router_groups_unsupported");
    let split = boundaries.first().copied().unwrap_or(1);
    ensure!(tokens[split] != tokens[0], "candidate_pivot_is_input");
    Ok(RoundTripContext {
        caller,
        token_in: tokens[0],
        token_out: tokens[split],
        amount_in,
        forward_router: routers[0],
        forward_path: tokens[..=split].to_vec(),
        backward_router: routers[split],
        backward_path: tokens[split..].to_vec(),
        deadline,
    })
}

fn decode_address(bytes: &[u8]) -> anyhow::Result<Address> {
    ensure!(
        bytes.len() == 32 && bytes[..12].iter().all(|b| *b == 0),
        "candidate_rpc_address_abi"
    );
    let result = Address::from_slice(&bytes[12..]);
    ensure!(result != Address::zero(), "candidate_rpc_address_zero");
    Ok(result)
}

async fn verify_pools(
    rpc: &OracleRpc,
    route: &RouteMetadata,
    plan: &RoutePlan,
    block: H256,
) -> anyhow::Result<()> {
    for (i, leg) in plan.legs.iter().enumerate() {
        let kind = sim_core::sim_encoder::parse_dex_kind(&route.dex_adapters[i])
            .map_err(|_| anyhow!("candidate_adapter_unsupported"))?;
        let router = sim_core::sim_encoder::resolve_router_address(plan.chain_id, kind)
            .map_err(|_| anyhow!("candidate_router_unavailable"))?;
        let factory = decode_address(&rpc.read(router, "0xc45a0155", block).await?)?;
        ensure!(
            factory == address(&leg.factory_address)?,
            "candidate_router_factory_mismatch"
        );
        let mut calldata = ethers::utils::id("getPair(address,address)")[..4].to_vec();
        calldata.extend(ethers::abi::encode(&[
            ethers::abi::Token::Address(address(&leg.token_in)?),
            ethers::abi::Token::Address(address(&leg.token_out)?),
        ]));
        let actual = decode_address(
            &rpc.read(factory, &format!("0x{}", hex::encode(calldata)), block)
                .await?,
        )?;
        ensure!(
            actual == address(&route.pool_addresses[i])?,
            "candidate_router_pool_mismatch"
        );
    }
    Ok(())
}

fn quantity(value: &Value) -> anyhow::Result<U256> {
    let text = value
        .as_str()
        .ok_or_else(|| anyhow!("candidate_rpc_quantity_missing"))?;
    U256::from_str_radix(
        text.strip_prefix("0x")
            .ok_or_else(|| anyhow!("candidate_rpc_quantity_invalid"))?,
        16,
    )
    .map_err(|_| anyhow!("candidate_rpc_quantity_invalid"))
}

fn quantity_u64(value: &Value) -> anyhow::Result<u64> {
    let n = quantity(value)?;
    ensure!(n <= U256::from(u64::MAX), "candidate_rpc_quantity_overflow");
    Ok(n.as_u64())
}

fn decimal(raw: U256, decimals: u8) -> anyhow::Result<BigDecimal> {
    ensure!(decimals <= 77, "candidate_decimals_unsupported");
    Ok(
        BigDecimal::from_str(&raw.to_string()).context("candidate_decimal_invalid")?
            / BigDecimal::from_str(&format!("1{}", "0".repeat(decimals as usize)))
                .context("candidate_decimal_scale_invalid")?,
    )
}

fn usd(raw: U256, decimals: u8, price: &Price) -> anyhow::Result<BigDecimal> {
    ensure!(!price.answer.is_zero(), "candidate_price_nonpositive");
    Ok(decimal(raw, decimals)? * decimal(price.answer, price.decimals)?)
}

fn display(value: &BigDecimal) -> anyhow::Result<f64> {
    value
        .to_f64()
        .filter(|v| v.is_finite())
        .ok_or_else(|| anyhow!("candidate_usd_out_of_range"))
}

/// Prices gas and the post-flash retained gain with exact decimal arithmetic.
/// The spine may account for additional risks/costs: its smaller net is kept.
struct EconomicInputs<'a> {
    principal: U256,
    retained: U256,
    fee: U256,
    decimals: u8,
    gas_used: u64,
    gas_price: U256,
    token_price: &'a Price,
    native_price: &'a Price,
    spine_net: Option<f64>,
}

fn economic_values(inputs: EconomicInputs<'_>) -> anyhow::Result<(f64, f64, f64, f64, f64)> {
    let EconomicInputs {
        principal,
        retained,
        fee,
        decimals,
        gas_used,
        gas_price,
        token_price,
        native_price,
        spine_net,
    } = inputs;
    ensure!(
        gas_used > 0 && !gas_price.is_zero(),
        "candidate_gas_unmeasured"
    );
    let gas = usd(
        gas_price
            .checked_mul(U256::from(gas_used))
            .ok_or_else(|| anyhow!("candidate_gas_overflow"))?,
        18,
        native_price,
    )?;
    let net = usd(retained, decimals, token_price)? - &gas;
    let spine = spine_net
        .filter(|n| n.is_finite() && *n > 0.0)
        .ok_or_else(|| anyhow!("candidate_spine_net_missing"))?;
    let spine = BigDecimal::from_str(&spine.to_string()).context("candidate_spine_net_invalid")?;
    let admitted = net.clone().min(spine);
    ensure!(admitted > 0, "candidate_simulated_net_nonpositive");
    ensure!(
        admitted >= &gas * BigDecimal::from(3),
        "candidate_three_gas_rule"
    );
    let gross = usd(
        retained
            .checked_add(fee)
            .ok_or_else(|| anyhow!("candidate_profit_overflow"))?,
        decimals,
        token_price,
    )?;
    let principal = usd(principal, decimals, token_price)?;
    ensure!(principal > 0, "candidate_principal_usd_nonpositive");
    Ok((
        display(&principal)?,
        display(&gas)?,
        display(&gross)?,
        display(&net)?,
        display(&admitted)?,
    ))
}

pub async fn prepare(
    opportunity: &Opportunity,
    route: &RouteMetadata,
    route_plan: &RoutePlan,
    permit: AdmissionPermit,
) -> anyhow::Result<Admission> {
    tokio::time::timeout(
        Duration::from_secs(20),
        prepare_inner(opportunity, route, route_plan, permit),
    )
    .await
    .map_err(|_| anyhow!("candidate_simulation_timeout"))?
}

#[cfg(not(feature = "v2-simulator"))]
async fn prepare_inner(
    _: &Opportunity,
    _: &RouteMetadata,
    _: &RoutePlan,
    _: AdmissionPermit,
) -> anyhow::Result<Admission> {
    bail!("candidate_simulator_not_compiled")
}

#[cfg(feature = "v2-simulator")]
async fn prepare_inner(
    opportunity: &Opportunity,
    route: &RouteMetadata,
    route_plan: &RoutePlan,
    permit: AdmissionPermit,
) -> anyhow::Result<Admission> {
    // The current accounting model covers ETH L1 gas. L2 data fees require their
    // own measured model before a net USD amount can be admitted.
    ensure!(
        matches!(opportunity.chain_id, 1 | 11155111),
        "candidate_fee_model_unavailable"
    );
    let rpc_url = std::env::var(format!("RPC_HTTP_{}", opportunity.chain_id))
        .map_err(|_| anyhow!("candidate_rpc_missing"))?;
    let rpc = OracleRpc::from_url(&rpc_url)?;
    ensure!(
        quantity_u64(&rpc.call("eth_chainId", json!([])).await?)? == opportunity.chain_id,
        "candidate_rpc_chain_mismatch"
    );
    let requested = opportunity
        .block_number
        .map(|b| format!("0x{b:x}"))
        .unwrap_or_else(|| "latest".into());
    let header = rpc
        .call("eth_getBlockByNumber", json!([requested, false]))
        .await?;
    let block_number = quantity_u64(&header["number"])?;
    let block_timestamp = quantity_u64(&header["timestamp"])?;
    let block_hash = H256::from_str(header["hash"].as_str().unwrap_or(""))
        .map_err(|_| anyhow!("candidate_block_hash_missing"))?;
    ensure!(
        block_hash != H256::zero() && block_timestamp > 0,
        "candidate_block_invalid"
    );
    let caller = address(
        &std::env::var(format!("SIM_CALLER_{}", opportunity.chain_id))
            .map_err(|_| anyhow!("candidate_sim_caller_missing"))?,
    )?;
    let executor = sim_core::sim_encoder::parse_executor_address(opportunity.chain_id)
        .map_err(|_| anyhow!("candidate_executor_missing"))?;
    let deadline = U256::from(
        block_timestamp
            .checked_add(120)
            .ok_or_else(|| anyhow!("candidate_deadline_overflow"))?,
    );
    let ctx = build_context(opportunity, route, route_plan, caller, deadline)?;
    verify_pools(&rpc, route, route_plan, block_hash).await?;
    let decimals = rpc.token_decimals(ctx.token_in, block_hash).await?;
    if let Some(recorded) = route.decimals.get(&format!("{:#x}", ctx.token_in)) {
        ensure!(recorded == decimals, "candidate_token_decimals_changed");
    }
    let feeds = configured_chain(opportunity.chain_id)?;
    let token_feed = feeds
        .assets_usd
        .get(&format!("{:#x}", ctx.token_in))
        .ok_or_else(|| anyhow!("candidate_token_usd_feed_missing"))?;
    let token_price = rpc.price(token_feed, block_hash, block_timestamp).await?;
    let native_price = rpc
        .price(&feeds.native_usd, block_hash, block_timestamp)
        .await?;
    let route_hash = ethers::utils::keccak256(serde_json::to_vec(&(
        opportunity.chain_id,
        opportunity.strategy_kind.as_str(),
        &route.token_addresses,
        &route.pool_addresses,
        &route.dex_adapters,
    ))?);
    let config = sim_core::sim_multistep::MultiStepExecutionConfig {
        chain_id: opportunity.chain_id,
        executor_address: executor,
        route_hash,
        min_profit_wei: U256::one(),
        gas_price_wei: U256::zero(),
        gas_limit_per_step: 16_000_000,
        paper_mode: false,
        enable_storage_cheats: false,
        require_trace_hash: true,
        require_positive_net_profit: true,
        max_steps: 8,
    };
    let simulator = Arc::new(simulator_v2::SimulatorV2::new(rpc_url).with_block(block_number));
    let sim_ctx = ctx.clone();
    let outcome = blocking_with_permit(permit.clone(), move || {
        sim_core::sim_multistep::execute_multistep_revm(&sim_ctx, simulator, &config)
    })
    .await?;
    ensure!(
        outcome.passed,
        "candidate_simulation_failed:{}",
        outcome.fail_reason.as_deref().unwrap_or("no_reason")
    );
    let evidence = outcome
        .evidence
        .as_ref()
        .ok_or_else(|| anyhow!("candidate_simulation_evidence_missing"))?;
    ensure!(
        evidence.chain_id == opportunity.chain_id
            && evidence.block_number == block_number
            && evidence.block_hash == block_hash
            && evidence.block_timestamp == block_timestamp,
        "candidate_simulation_snapshot_changed"
    );
    // The simulator sets this to max(requested floor, measured premium + 1).
    let min_profit_wei = evidence.min_profit_wei;
    let plan = ValidatedPlan::from_simulation(
        ctx.clone(),
        opportunity.id,
        opportunity.strategy_kind.as_str(),
        route_hash,
        min_profit_wei,
        executor,
        &outcome,
    )
    .map_err(|reason| anyhow!(reason))?;
    let (
        principal_usd,
        gas_cost_usd,
        gross_profit_usd,
        simulated_net_profit_usd,
        admitted_net_profit_usd,
    ) = economic_values(EconomicInputs {
        principal: ctx.amount_in,
        retained: outcome.simulated_profit_token_in,
        fee: evidence.flash_fee_wei,
        decimals,
        gas_used: outcome.gas_used_total,
        gas_price: outcome.gas_price_wei,
        token_price: &token_price,
        native_price: &native_price,
        spine_net: opportunity.net_expected_profit_usd,
    })?;
    let economics = ValidatedEconomics {
        schema_version: 1,
        opportunity_id: opportunity.id,
        strategy_kind: opportunity.strategy_kind.as_str().into(),
        chain_id: opportunity.chain_id,
        block_number,
        block_hash,
        token_in: ctx.token_in,
        token_in_decimals: decimals,
        principal_wei: ctx.amount_in,
        principal_usd,
        flash_fee_wei: evidence.flash_fee_wei,
        gas_used: outcome.gas_used_total,
        gas_price_wei: outcome.gas_price_wei,
        gas_cost_usd,
        gross_profit_usd,
        simulated_net_profit_usd,
        admitted_net_profit_usd,
        token_price_answer: token_price.answer,
        token_price_decimals: token_price.decimals,
        token_price_updated_at: token_price.updated_at,
        native_price_answer: native_price.answer,
        native_price_decimals: native_price.decimals,
        native_price_updated_at: native_price.updated_at,
    };
    let mut admitted = opportunity.clone();
    // The legacy pair DTO names the pivot, while RouteMetadata continues to
    // carry the full closed cycle unchanged for topology and exact pool audit.
    admitted.token_out = format!("{:#x}", ctx.token_out);
    admitted.block_number = Some(block_number);
    admitted.expected_profit_usd = Some(gross_profit_usd);
    admitted.net_expected_profit_usd = Some(admitted_net_profit_usd);
    admitted.roi_pct = Some(admitted_net_profit_usd / principal_usd * 100.0);
    admitted.rejection_reason = None;
    Ok(Admission {
        opportunity: admitted,
        plan,
        economics,
    })
}

async fn blocking_with_permit<T: Send + 'static>(
    permit: AdmissionPermit,
    task: impl FnOnce() -> T + Send + 'static,
) -> anyhow::Result<T> {
    tokio::task::spawn_blocking(move || {
        let _permit_until_thread_exit = permit;
        task()
    })
    .await
    .map_err(|_| anyhow!("candidate_simulation_worker_failed"))
}

/// Plan and economic evidence become visible together before the opportunity
/// UUID is published. Redis errors abort accepted publication.
pub async fn persist(
    admission: &Admission,
    redis: &mut redis::aio::ConnectionManager,
) -> anyhow::Result<()> {
    let id = admission.opportunity.id;
    let plan = serde_json::to_string(&admission.plan)?;
    let economics = serde_json::to_string(&admission.economics)?;
    let _: () = redis::pipe()
        .atomic()
        .cmd("SET")
        .arg(format!("arbx:validated_plan:{id}"))
        .arg(plan)
        .arg("EX")
        .arg(60)
        .ignore()
        .cmd("SET")
        .arg(format!("arbx:validated_economics:{id}"))
        .arg(economics)
        .arg("EX")
        .arg(60)
        .ignore()
        .query_async(redis)
        .await
        .context("candidate_validation_persist_failed")?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use prioritization_spine::route_plan::RouteLeg;
    use shared_rs::contracts::StrategyKind;
    use uuid::Uuid;

    fn fixtures(adapters: &[&str]) -> (Opportunity, RouteMetadata, RoutePlan) {
        let tokens: Vec<String> = (1..=adapters.len())
            .chain(std::iter::once(1))
            .map(|n| format!("{:#x}", Address::from_low_u64_be(n as u64)))
            .collect();
        let pools: Vec<String> = (0..adapters.len())
            .map(|n| format!("{:#x}", Address::from_low_u64_be(100 + n as u64)))
            .collect();
        let opp = Opportunity {
            id: Uuid::new_v4(),
            chain_id: 1,
            strategy_kind: StrategyKind::triangular(),
            dex_a: adapters[0].into(),
            dex_b: adapters.last().map(|s| (*s).into()),
            pair_symbol: "A/B".into(),
            token_in: tokens[0].clone(),
            token_out: tokens.last().unwrap().clone(),
            amount_in_wei: "1000".into(),
            expected_profit_usd: Some(10.0),
            net_expected_profit_usd: Some(8.0),
            roi_pct: None,
            risk_score: None,
            block_number: None,
            rejection_reason: None,
            cartridge_id: None,
            detected_at: chrono::Utc::now(),
            trace_id: Uuid::new_v4(),
        };
        let mut route = RouteMetadata::empty();
        route.token_addresses = tokens.clone();
        route.pool_addresses = pools.clone();
        route.dex_adapters = adapters.iter().map(|a| (*a).into()).collect();
        let legs = adapters
            .iter()
            .enumerate()
            .map(|(i, adapter)| RouteLeg {
                dex_id: "fixture".into(),
                dex_name: (*adapter).into(),
                protocol_type: (*adapter).into(),
                factory_address: format!("{:#x}", Address::from_low_u64_be(200)),
                pool_id: None,
                pool_address: Some(pools[i].clone()),
                token_in: tokens[i].clone(),
                token_out: tokens[i + 1].clone(),
                fee_bps: Some(30),
                amount_in: None,
                amount_out: None,
                tvl_usd: None,
                volume_24h_usd: None,
                pool_is_active: true,
            })
            .collect();
        let plan = RoutePlan {
            route_id: None,
            strategy_kind: "triangular".into(),
            chain_id: 1,
            legs,
            atomic: true,
            estimated_slippage_pct: None,
            price_impact_pct: None,
        };
        (opp, route, plan)
    }
    fn build(
        o: &Opportunity,
        r: &RouteMetadata,
        p: &RoutePlan,
    ) -> anyhow::Result<RoundTripContext> {
        build_context(
            o,
            r,
            p,
            Address::from_low_u64_be(999),
            U256::from(1_800_000_000u64),
        )
    }
    #[test]
    fn exact_principal_above_two_to_200_survives() {
        let (mut o, r, p) = fixtures(&["UniswapV2", "SushiSwap"]);
        let amount = (U256::one() << 201) + U256::from(173);
        o.amount_in_wei = amount.to_string();
        assert_eq!(build(&o, &r, &p).unwrap().amount_in, amount);
    }
    #[test]
    fn full_seven_hop_topology_and_pivot_are_preserved() {
        let (o, r, p) = fixtures(&[
            "UniswapV2",
            "UniswapV2",
            "UniswapV2",
            "SushiSwap",
            "SushiSwap",
            "SushiSwap",
            "SushiSwap",
        ]);
        let ctx = build(&o, &r, &p).unwrap();
        assert_eq!(ctx.forward_path.len(), 4);
        assert_eq!(ctx.backward_path.len(), 5);
        assert_eq!(ctx.token_out, address(&r.token_addresses[3]).unwrap());
        assert_eq!(r.token_addresses.last(), r.token_addresses.first());
        assert_ne!(ctx.token_in, ctx.token_out);
    }
    #[test]
    fn repeated_pool_open_path_and_third_router_group_are_rejected() {
        let (o, mut r, mut p) = fixtures(&["UniswapV2", "SushiSwap", "UniswapV2"]);
        assert!(build(&o, &r, &p)
            .unwrap_err()
            .to_string()
            .contains("router_groups"));
        r.pool_addresses[1] = r.pool_addresses[0].clone();
        p.legs[1].pool_address = Some(r.pool_addresses[0].clone());
        assert!(build(&o, &r, &p)
            .unwrap_err()
            .to_string()
            .contains("repeated_pool"));
        let (o, mut r, p) = fixtures(&["UniswapV2", "SushiSwap"]);
        r.token_addresses[2] = format!("{:#x}", Address::from_low_u64_be(600));
        assert!(build(&o, &r, &p)
            .unwrap_err()
            .to_string()
            .contains("not_closed"));
    }
    #[test]
    fn strategy_identity_and_nonatomic_plan_cannot_be_rebound() {
        let (mut o, r, mut p) = fixtures(&["UniswapV2", "SushiSwap"]);
        o.cartridge_id = Some("mev_01_001_dex_dex_arbitrage".into());
        assert!(build(&o, &r, &p)
            .unwrap_err()
            .to_string()
            .contains("strategy_identity"));
        o.strategy_kind = StrategyKind::cartridge(o.cartridge_id.clone().unwrap());
        assert!(build(&o, &r, &p).is_ok());
        p.atomic = false;
        assert!(build(&o, &r, &p).is_err());
    }
    #[test]
    fn economics_uses_postflash_retention_and_the_smaller_spine_net() {
        let price = Price {
            answer: U256::from(100_000_000),
            decimals: 8,
            updated_at: 1,
        };
        let values = economic_values(EconomicInputs {
            principal: U256::from(1000),
            retained: U256::from(10),
            fee: U256::from(2),
            decimals: 0,
            gas_used: 1,
            gas_price: U256::exp10(18),
            token_price: &price,
            native_price: &price,
            spine_net: Some(6.0),
        })
        .unwrap();
        assert_eq!(values, (1000.0, 1.0, 12.0, 9.0, 6.0));
        assert!(economic_values(EconomicInputs {
            principal: U256::from(1000),
            retained: U256::from(3),
            fee: U256::from(2),
            decimals: 0,
            gas_used: 1,
            gas_price: U256::exp10(18),
            token_price: &price,
            native_price: &price,
            spine_net: Some(20.0),
        })
        .unwrap_err()
        .to_string()
        .contains("three_gas"));
    }
    #[tokio::test]
    async fn timeout_does_not_release_still_running_blocking_permit() {
        let semaphore = Arc::new(Semaphore::new(1));
        let permit = Arc::new(semaphore.clone().acquire_owned().await.unwrap());
        let (started_tx, started_rx) = tokio::sync::oneshot::channel();
        let (release_tx, release_rx) = std::sync::mpsc::channel();
        let task = tokio::spawn(blocking_with_permit(permit.clone(), move || {
            let _ = started_tx.send(());
            release_rx.recv().unwrap();
        }));
        started_rx.await.unwrap();
        drop(permit);
        task.abort();
        assert!(semaphore.clone().try_acquire_owned().is_err());
        release_tx.send(()).unwrap();
        let recovered =
            tokio::time::timeout(Duration::from_secs(2), semaphore.acquire_owned()).await;
        assert!(recovered.is_ok());
    }
}
