//! Issue #567 — canonical plan carrier consumption for the S4 consumer (B2c).
//!
//! The producer (searcher-rs scanner, M2 carrier-B) persists the sim-validated
//! plan to Redis as `arbx:validated_plan:{opportunity_id}` (TTL 300s) on
//! SIM_SUCCESS. Until now this consumer RECONSTRUCTED the route on its own:
//! the legacy anvil path builds a single-swap probe from token_in/token_out —
//! which CANNOT represent a cyclic 2-5 hop route (`tx_builder`:
//! `CyclicRouteNotRepresentable` → `not_implemented/
//! strategy_cyclic_route_not_simulatable_in_s4:*`, 712/1000 in production
//! tallies, 0 passed in 24h) — and the B2c path rebuilds a candidate from PG
//! `route_metadata`.
//!
//! This module recovers the producer's OWN `RoundTripContext` by opportunity
//! id and re-simulates the FULL cycle through `execute_multistep_revm`
//! (paper_mode=true, observer-only — same mandate as `sim_runner`). The
//! carrier is the route of record; nothing is re-derived from
//! token_in/token_out.
//!
//! Failure semantics (RULE 00 / R8 fail-honest):
//!   * carrier absent/expired → `Ok(None)` — the caller falls back to the
//!     existing reconstruction path unchanged (strictly additive behavior).
//!   * carrier unparseable    → typed gap `validated_plan_parse_error:*`
//!     (persisted honestly; the opportunity stays non-rejected — same
//!     semantics as the other `counted_gap` returns).
//!   * Redis infra failure    → `Err` (transient; the entry stays in the
//!     group PEL and `recover_stale_pending` redelivers — same contract as
//!     the rest of `simulate_b2c`).

use chrono::Utc;
use prioritization_spine::ValidatedPlan;
use redis::aio::ConnectionManager;
use redis::AsyncCommands;
use shared_rs::contracts::{Opportunity, SimulationResult, SimulatorKind};
use shared_rs::metrics::SIMULATIONS_TOTAL;
use uuid::Uuid;

use crate::consumer::{simulator_for_candidate, B2cCtx};

/// Same key scheme the producer writes (scanner.rs, M2 carrier-B) and
/// relays-client reads back on live admission (`submit_engine`).
pub const CARRIER_KEY_PREFIX: &str = "arbx:validated_plan:";

/// Why a carrier fetch could not produce a plan. Absence is NOT an error —
/// it is the `Ok(None)` fall-through to the reconstruction path.
#[derive(Debug)]
pub enum FetchError {
    /// Redis GET failed — transient infra, PEL retry (never a market verdict).
    Redis(String),
    /// Payload present but not a `ValidatedPlan` — terminal typed gap.
    Parse(String),
}

impl FetchError {
    /// Fail reason as persisted/logged. Prefixes mirror relays-client's
    /// naming (`validated_plan_missing` / `validated_plan_parse_error`).
    pub fn as_fail_reason(&self) -> String {
        match self {
            FetchError::Redis(e) => format!("validated_plan_redis_failed:{e}"),
            FetchError::Parse(e) => format!("validated_plan_parse_error:{e}"),
        }
    }
}

pub fn carrier_key(opportunity_id: Uuid) -> String {
    format!("{CARRIER_KEY_PREFIX}{opportunity_id}")
}

/// Parse a carrier payload (pure — unit-testable without Redis).
pub fn parse_carrier(raw: &str) -> Result<ValidatedPlan, String> {
    serde_json::from_str(raw).map_err(|e| format!("validated_plan_parse_error:{e}"))
}

/// Fetch the producer's validated plan for an opportunity id.
/// `Ok(None)` = carrier absent/expired (normal outside the TTL window or
/// when the producer never reached SIM_SUCCESS).
pub async fn fetch(
    redis: &mut ConnectionManager,
    opportunity_id: Uuid,
) -> Result<Option<ValidatedPlan>, FetchError> {
    let key = carrier_key(opportunity_id);
    let raw: Option<String> = redis
        .get(&key)
        .await
        .map_err(|e| FetchError::Redis(format!("{e}")))?;
    match raw {
        None => Ok(None),
        Some(json) => parse_carrier(&json).map(Some).map_err(FetchError::Parse),
    }
}

/// Re-simulate the producer's validated plan — the FULL multi-hop cycle —
/// through the REVM engine. Mirrors `sim_runner::run_real_simulation`'s
/// config and result mapping, with one #567 difference: the executor, route
/// hash and min-profit come from the CARRIER (the plan of record), not from
/// env defaults, so the replay is byte-comparable with what the producer
/// validated. Block-pinned to the plan's binding block when present
/// (determinism vs the producer's validation), else the opportunity's
/// detection block, else unpinned (`latest`).
pub async fn resimulate(
    b2c: &B2cCtx,
    opp: &Opportunity,
    plan: ValidatedPlan,
) -> Result<SimulationResult, String> {
    if opp.chain_id == 0 {
        return Err(format!(
            "candidate_incomplete:invalid_chain_{}",
            opp.chain_id
        ));
    }
    let chain_id = opp.chain_id;

    // Chain parity: the carrier is keyed by opp.id; a binding from another
    // chain means the producer wrote a mismatched plan — typed gap, never
    // simulate cross-chain state.
    if let Some(binding) = plan.binding.as_ref() {
        if binding.chain_id != chain_id {
            return Ok(crate::consumer::counted_gap(
                opp.id,
                &format!(
                    "validated_plan_chain_mismatch:{}!={}",
                    binding.chain_id, chain_id
                ),
            ));
        }
    }

    // Same fail-closed FLASHLOAN_EXECUTOR guard as the reconstruction path
    // (SIMWIRE-02c P1-3): an env gap is operator config, not a market
    // verdict — transient (PEL), healed when the operator sets the var.
    if let Err(e) = shared_rs::chains::resolve_flashloan_executor_address(chain_id) {
        return Err(format!("flashloan_executor_unresolved: {e}"));
    }

    // Live gas price — transient (PEL retry), same key scheme as step 5 of
    // the reconstruction path.
    let gas_price_wei = crate::read_gas_price(&b2c.gas_redis, chain_id).await?;

    // Block pin: binding block (producer validation block) > opportunity
    // detection block > latest.
    let pinned_block = plan
        .binding
        .as_ref()
        .map(|b| b.block_number)
        .or(opp.block_number);
    let pinned_sim = simulator_for_candidate(&b2c.simulator, pinned_block);

    let exec_config = sim_core::sim_multistep::MultiStepExecutionConfig {
        chain_id,
        executor_address: plan.executor_address,
        route_hash: plan.route_hash,
        min_profit_wei: plan.min_profit_wei,
        gas_price_wei,
        gas_limit_per_step: b2c.env.gas_limit_per_step,
        // paper_mode is MANDATORY true in sim-ctl — the orchestrator refuses
        // to participate in any live-execution path (same comment as
        // sim_runner; this is an observer, not an executor).
        paper_mode: true,
        enable_storage_cheats: true,
        require_trace_hash: true,
        require_positive_net_profit: true,
        max_steps: 8,
        paper_stack: None,
    };

    let ctx = plan.ctx;
    let result = tokio::task::spawn_blocking(move || {
        sim_core::sim_multistep::execute_multistep_revm(&ctx, pinned_sim, &exec_config)
    })
    .await;

    let outcome = match result {
        Ok(o) => o,
        Err(e) => return Err(format!("b2c_spawn_blocking_join:{e}")),
    };

    {
        let passed_str = if outcome.passed { "true" } else { "false" };
        SIMULATIONS_TOTAL
            .with_label_values(&["revm", passed_str])
            .inc();
    }

    // Transient REVM state-fetch infra failures stay in the PEL (same
    // classification as step 8 of the reconstruction path).
    if !outcome.passed {
        if let Some(fr) = outcome.fail_reason.as_deref() {
            if fr.starts_with("multistep_lazy_db_failed")
                || fr.starts_with("b2c_spawn_blocking_join")
            {
                return Err(format!("revm_state_infra: {fr}"));
            }
        }
    }

    // PRICES-FREE by design (R8): net-USD is computed downstream from
    // prices; `simulated_profit_usd` stays None (None = not computed, never
    // fabricated).
    Ok(SimulationResult {
        opportunity_id: opp.id,
        passed: outcome.passed,
        gas_estimate_wei: Some(outcome.gas_used_total.to_string()),
        gas_price_wei: Some(outcome.gas_price_wei.to_string()),
        slippage_pct: None,
        revert_risk_pct: None,
        simulated_profit_usd: None,
        simulator: SimulatorKind::Revm,
        fail_reason: outcome.fail_reason,
        simulated_at: Utc::now(),
        trace_id: Uuid::new_v4(),
    })
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use ethers::types::{Address, U256};
    use prioritization_spine::round_trip_executor::RoundTripContext;

    /// Producer-shaped 3-hop cyclic plan: WETH → USDC → DAI → back to WETH
    /// (token_in == token_out — EXACTLY the topology `tx_builder` refuses
    /// with `CyclicRouteNotRepresentable` and that issue #567 asks to
    /// simulate as a full cycle).
    fn fixture_cyclic_plan() -> ValidatedPlan {
        let weth = Address::from_low_u64_be(0xaaa1);
        let usdc = Address::from_low_u64_be(0xbbb2);
        let dai = Address::from_low_u64_be(0xccc3);
        let router_a = Address::from_low_u64_be(0x1111);
        let router_b = Address::from_low_u64_be(0x2222);
        let ctx = RoundTripContext {
            caller: Address::from_low_u64_be(0x9999),
            token_in: weth,
            token_out: weth, // cyclic: the round trip closes on the entry token
            amount_in: U256::from(1_000_000_000_000_000_000u64),
            forward_router: router_a,
            forward_path: vec![weth, usdc, dai],
            backward_router: router_b,
            backward_path: vec![dai, usdc, weth],
            deadline: U256::from(1_800_000_000u64),
        };
        ValidatedPlan {
            ctx,
            route_hash: [0x56u8; 32],
            min_profit_wei: U256::one(),
            executor_address: Address::from_low_u64_be(0x3333),
            wrapped_calldata: vec![0xde, 0xad],
            binding: None,
        }
    }

    /// HARDENING (Issue #567): the carrier round-trips the FULL multi-hop
    /// cyclic topology the single-swap probe cannot represent. If this test
    /// fails, the S4 consumer has lost the plan-of-record path and cyclic
    /// opportunities regress to `strategy_cyclic_route_not_simulatable_in_s4`.
    #[test]
    fn carrier_round_trip_preserves_full_cycle_context_567() {
        let plan = fixture_cyclic_plan();
        let json = serde_json::to_string(&plan).unwrap();
        let back = parse_carrier(&json).unwrap_or_else(|e| panic!("parse failed: {e}"));
        assert_eq!(back.ctx.token_in, back.ctx.token_out, "cycle must close");
        assert_eq!(back.ctx.forward_path.len(), 3, "3-hop forward leg");
        assert_eq!(back.ctx.backward_path.len(), 3, "3-hop backward leg");
        assert_eq!(back.route_hash, [0x56u8; 32]);
        assert_eq!(back.executor_address, plan.executor_address);
    }

    /// HARDENING: a corrupt carrier is a TYPED GAP, never a silent pass and
    /// never a fabricated plan (RULE 00).
    #[test]
    fn garbage_carrier_is_typed_gap_not_silent_pass() {
        let err = parse_carrier("definitely-not-json").expect_err("must fail");
        assert!(
            err.starts_with("validated_plan_parse_error:"),
            "unexpected reason: {err}"
        );
    }

    /// The carrier key is the SAME key scheme relays-client reads on live
    /// admission — key drift between producer, consumer and broadcast side
    /// would silently break the plan-of-record contract.
    #[test]
    fn carrier_key_matches_producer_scheme() {
        let id = Uuid::nil();
        assert_eq!(
            carrier_key(id),
            "arbx:validated_plan:00000000-0000-0000-0000-000000000000"
        );
        assert_eq!(CARRIER_KEY_PREFIX, "arbx:validated_plan:");
    }
}
