//! Persistence for sim-ctl: writes a row per simulation attempt and updates
//! the opportunity state. Transactional so both land or neither does.

use anyhow::{Context, Result};
use shared_rs::contracts::{SimulationResult, SimulatorKind};
use sqlx::postgres::PgPool;
use sqlx::types::BigDecimal;
use std::str::FromStr;

/// Insert one simulation attempt. Returns `true` when a NEW row landed, or
/// `false` when the `(opportunity_id, simulator='revm')` row already existed
/// (SIMWIRE-02c redelivery idempotency: XAUTOCLAIM redelivers an entry whose
/// final XACK failed after persist+XADD succeeded — the caller must skip the
/// downstream XADD so the opportunity is published exactly once).
pub async fn insert_simulation(pool: &PgPool, r: &SimulationResult) -> Result<bool> {
    let sim_str = simulator_str(&r.simulator);
    let gas_est: Option<BigDecimal> = r
        .gas_estimate_wei
        .as_deref()
        .and_then(|s| BigDecimal::from_str(s).ok());
    let gas_price: Option<BigDecimal> = r
        .gas_price_wei
        .as_deref()
        .and_then(|s| BigDecimal::from_str(s).ok());

    let mut tx = pool.begin().await.context("begin tx")?;

    let inserted = sqlx::query(
        r#"
        INSERT INTO simulations (
            opportunity_id, simulator, gas_estimate_wei, gas_price_wei,
            slippage_pct, revert_risk_pct, simulated_profit_usd,
            passed, fail_reason, trace_id, simulated_at
        ) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11)
        ON CONFLICT (opportunity_id) WHERE simulator = 'revm' DO NOTHING
        "#,
    )
    .bind(r.opportunity_id)
    .bind(sim_str)
    .bind(gas_est)
    .bind(gas_price)
    .bind(r.slippage_pct)
    .bind(r.revert_risk_pct)
    .bind(r.simulated_profit_usd)
    .bind(r.passed)
    .bind(r.fail_reason.as_deref())
    .bind(r.trace_id)
    .bind(r.simulated_at)
    .execute(&mut *tx)
    .await
    .context("insert simulation")?;

    if inserted.rows_affected() == 0 {
        // Prior delivery already persisted this revm verdict (and published
        // it — only the final XACK failed). Skip the status update too: the
        // row already carries the state transition.
        tx.commit()
            .await
            .context("commit tx (duplicate redelivery)")?;
        return Ok(false);
    }

    // Advance opportunity state. 'simulated' if passed, else 'rejected' —
    // UNLESS the failure is a sim-capability gap (unsupported strategy/chain,
    // no fork configured). A capability gap means our sim engine can't verify
    // this strategy yet; it is NOT an opportunity-quality rejection. Per
    // EXECUTION_MODES_DOCTRINE §34, the live terminus (relays-client) gates
    // execution independently (default-deny mainnet), so paper/shadow may
    // surface detection without requiring the sim terminus. Keeping the
    // opportunity at its pre-sim status leaves it viable (rejection_reason
    // stays NULL) so the dashboard shows real detections; the simulation row
    // inserted above still records the SIM_SKIP outcome honestly.
    let sim_capability_gap = r
        .fail_reason
        .as_deref()
        .map(is_sim_capability_gap)
        .unwrap_or(false);

    if !r.passed && sim_capability_gap {
        // Commit the simulation row (already inserted) but do NOT flip the
        // opportunity to 'rejected'. The opp stays 'detected'/'validated'.
        tx.commit()
            .await
            .context("commit tx (sim capability gap)")?;
        return Ok(true);
    }

    let next_status = if r.passed { "simulated" } else { "rejected" };
    let reject_reason = if r.passed {
        None
    } else {
        r.fail_reason.clone()
    };
    sqlx::query(
        r#"
        UPDATE opportunities
           SET status = $2,
               rejection_reason = COALESCE($3, rejection_reason),
               updated_at = NOW()
         WHERE id = $1
           AND status IN ('validated','scored','detected')
        "#,
    )
    .bind(r.opportunity_id)
    .bind(next_status)
    .bind(reject_reason)
    .execute(&mut *tx)
    .await
    .context("update opportunity status")?;

    tx.commit().await.context("commit tx")?;
    Ok(true)
}

fn simulator_str(k: &SimulatorKind) -> &'static str {
    match k {
        SimulatorKind::Anvil => "anvil",
        SimulatorKind::Tenderly => "tenderly",
        SimulatorKind::Hardhat => "hardhat",
        SimulatorKind::Revm => "revm",
        SimulatorKind::NotImplemented => "not_implemented",
    }
}

/// Classify a sim `fail_reason` as a *capability gap* (the sim engine cannot
/// run this strategy/chain/fork) rather than a genuine opportunity-quality
/// failure (revert, gas exceeded, etc.). Capability gaps must NOT reject the
/// opportunity — see `insert_simulation`. The strings mirror the
/// `not_implemented` outcomes produced in `sim_engine.rs`.
fn is_sim_capability_gap(fail_reason: &str) -> bool {
    fail_reason.starts_with("strategy_not_simulatable")
        || fail_reason.starts_with("anvil_fork_not_configured")
        || fail_reason.contains("_not_supported_in_s4")
        // SIMWIRE-02 (P1 safety net): typed B2c/stream gaps. Absence of
        // capability must NEVER become an opportunity-quality rejection —
        // otherwise a flipped SIM_BACKEND=revm structurally drains the
        // validated stream into permanent `rejected` rows. Families:
        //   route_encoding_not_available  — legacy RevmBackend empty calldata
        //   route_metadata_not_available  — row lacks route topology
        //   real_sim_unavailable / real_sim_env_missing — B2c env incomplete
        //   candidate_incomplete:*       — S4-02 STRUCTURAL: the row lacks
        //                                   what the encoder needs; retry
        //                                   cannot change the row
        //   b2c_encode_failed:*          — router not in the encoder catalog
        || fail_reason.starts_with("route_encoding_not_available")
        || fail_reason.starts_with("route_metadata_not_available")
        || fail_reason.starts_with("real_sim_unavailable")
        || fail_reason.starts_with("real_sim_env_missing")
        || fail_reason.starts_with("candidate_incomplete")
        || fail_reason.starts_with("b2c_encode_failed")
        // SIMWIRE-02c P1: harness/config failures are NOT market verdicts.
        // These are DETERMINISTIC given the pinned block+calldata (retrying
        // cannot change them), so they persist as typed non-rejecting gap
        // rows instead of either rejecting the opportunity or spinning the
        // PEL forever. The NON-deterministic infra failures (RPC state
        // fetch, runtime join) stay transient in the consumer (PEL).
        //   multistep_flashloan_executor_unresolved — env config gap (A3 path)
        //   multistep_call_infra:*        — REVM harness: transact_infra /
        //                                   evm_db_missing / storage_override
        //   multistep_apply_storage_failed / multistep_empty_calldata
        //   *_halted / *_decode_failed / amounts_out_empty_array — view-call
        //                                   frame/decode faults (harness-level)
        || fail_reason.starts_with("multistep_flashloan_executor_unresolved")
        || fail_reason.starts_with("multistep_call_infra")
        || fail_reason.starts_with("multistep_apply_storage_failed")
        || fail_reason.starts_with("multistep_empty_calldata")
        || fail_reason.contains("balance_read_halted")
        || fail_reason.contains("amounts_out_halted")
        || fail_reason.contains("balance_decode_failed")
        || fail_reason.contains("amounts_out_decode_failed")
        || fail_reason.contains("amounts_out_empty_array")
}

#[cfg(test)]
mod simwire02_classifier_tests {
    use super::is_sim_capability_gap;

    #[test]
    fn legacy_gap_families_stay_gaps() {
        for reason in [
            "strategy_not_simulatable:mev_backrun",
            "anvil_fork_not_configured",
            "strategy_not_supported_in_s4",
        ] {
            assert!(is_sim_capability_gap(reason), "{reason} must stay a gap");
        }
    }

    /// SIMWIRE-02 P1: the structural-drain guard. Every typed B2c/stream
    /// gap must keep the opportunity NON-rejected (status stays
    /// detected/validated, rejection_reason stays NULL).
    #[test]
    fn simwire02_typed_b2c_gaps_are_not_rejections() {
        for reason in [
            "route_encoding_not_available",
            "route_metadata_not_available",
            "real_sim_unavailable: SIM_BACKEND!=revm",
            "real_sim_env_missing: ARBITRAGE_EXECUTOR env var required",
            "candidate_incomplete:token_addresses_empty",
            "candidate_incomplete:missing_decimals_[\"0xabc\"]",
            "candidate_incomplete:amount_in_wei_unparseable",
            "b2c_encode_failed:router_not_in_catalog",
        ] {
            assert!(
                is_sim_capability_gap(reason),
                "{reason} must classify as capability gap, not rejection"
            );
        }
    }

    /// The flip side: genuine opportunity-quality / market verdicts must
    /// still reject — the gap set must not swallow economic truth.
    #[test]
    fn economic_and_market_verdicts_stay_rejections() {
        for reason in [
            "execution_reverted",
            "multistep_call_halt:Revert",
            "multistep_gross_spread_non_positive",
            "stf",
            "gas_floor_breach",
            "net_zero_after_gas",
        ] {
            assert!(
                !is_sim_capability_gap(reason),
                "{reason} must stay a rejection, not a gap"
            );
        }
    }

    /// SIMWIRE-02c P1: deterministic harness/config failures are NOT market
    /// verdicts — they must persist as typed non-rejecting gaps (visible in
    /// the simulations row) instead of rejecting the opportunity.
    #[test]
    fn simwire02c_harness_failures_are_gaps_not_rejections() {
        for reason in [
            "multistep_flashloan_executor_unresolved:Missing { chain_id: 1 }",
            "multistep_call_infra:transact_infra:db commit failed",
            "multistep_call_infra:evm_db_missing",
            "multistep_apply_storage_failed:storage_override_failed",
            "multistep_empty_calldata",
            "multistep_read_balance_failed:balance_of:balance_read_halted",
            "multistep_forward_quote_failed:amounts_out_halted",
            "multistep_read_balance_failed:balance_of:balance_decode_failed",
            "multistep_forward_quote_failed:amounts_out_decode_failed",
            "multistep_forward_quote_failed:amounts_out_empty_array",
        ] {
            assert!(
                is_sim_capability_gap(reason),
                "{reason} must classify as harness gap, not market rejection"
            );
        }
    }

    /// SIMWIRE-02c P1: chain-state verdicts at the pinned block are market
    /// truth — halts of COMMITTED calls and view reverts on the forked state
    /// must keep rejecting (retry at the same block gives the same answer).
    #[test]
    fn simwire02c_chain_state_verdicts_stay_rejections() {
        for reason in [
            "multistep_call_revert:TransferHelper: INSUFFICIENT_OUTPUT",
            "multistep_forward_quote_failed:amounts_out_reverted",
            "multistep_forward_quote_failed:zero_intermediate",
            "multistep_read_balance_failed:balance_of:balance_read_reverted",
        ] {
            assert!(
                !is_sim_capability_gap(reason),
                "{reason} is a chain-state verdict — must stay a rejection"
            );
        }
    }
}
