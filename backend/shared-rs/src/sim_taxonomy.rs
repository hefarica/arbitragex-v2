//! S4-02 — Simulation failure taxonomy (STRUCTURAL / ECONOMIC / MARKET).
//!
//! Canal B (drift-tracker → paper_trade_runs.actual_* → Stage 2b calibration)
//! must NEVER turn an infrastructure defect into a strategy label (S4-03
//! no-contamination gate). A sim that fails because the fixture is broken
//! (signer without balance, missing fork, missing gas oracle) says nothing
//! about the market — feeding it to calibration as "strategy lost" would
//! poison the priors.
//!
//! Families (runbook S4, accepted 2026-08-29):
//!   STRUCTURAL — fixture/infra defects. Retrying does not fix them (a signer
//!                without tokens is not a transient condition). Terminal for
//!                the row: `calibration_eligible = false`.
//!   ECONOMIC   — the market rejected the trade at the settled block
//!                (negative net, slippage, gas). VALID calibration label:
//!                Y = loss/reject. `calibration_eligible = true`.
//!   MARKET     — the EVM/route rejected it for market-state reasons
//!                (revert, stale state, deadline). Same treatment as ECONOMIC.
//!
//! Fail-closed default: an UNKNOWN reason classifies as STRUCTURAL. Only
//! known-good economic/market tags produce labels; anything unrecognized is
//! suspicious and surfaces in the structural-rate measurement (S4-05) for
//! taxonomy refinement rather than silently leaking into calibration.
//!
//! Matching is case-insensitive substring against the reason tag; producers
//! prefix reasons (`b2c_encode_failed:...`, `gas_price_wei is zero...`), so we
//! match on stable substrings, never on full strings.

use serde::{Deserialize, Serialize};

/// Family of a simulation failure (S4-02 taxonomy).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FailFamily {
    /// Fixture/infra defect — NOT a market label. Ineligible for calibration.
    Structural,
    /// Market rejected economically — VALID label (Y = loss/reject).
    Economic,
    /// EVM/route rejected for market-state reasons — VALID label.
    Market,
}

impl FailFamily {
    pub fn as_str(&self) -> &'static str {
        match self {
            FailFamily::Structural => "structural",
            FailFamily::Economic => "economic",
            FailFamily::Market => "market",
        }
    }
}

/// Substrings identifying STRUCTURAL failures (fixture/infra).
///
/// Sources: the S4 runbook list plus the typed reason tags sim-ctl actually
/// emits today (`real_sim_unavailable`, `b2c_encode_failed`, the gas-oracle
/// family, the A3 lookup family).
const STRUCTURAL_MARKERS: &[&str] = &[
    // Runbook S4-02 list.
    "signer_balance_missing",
    "allowance_missing",
    "transfer_from_failed",
    "fork_unavailable",
    "archive_rpc_failed",
    "gas_oracle_missing",
    "candidate_incomplete",
    "route_encoding_missing",
    "decimals_missing",
    "backend_not_configured",
    // sim-ctl typed error tags (main.rs / sim_runner.rs).
    "real_sim_unavailable",
    "real_sim_env_missing",
    "gas_price_unavailable",
    "gas_price_read_failed",
    "gas_price_wei",
    "b2c_encode_failed",
    "b2c_spawn_blocking_join",
    "route_metadata_not_found",
    "missing_opportunity_id",
    "a3_unavailable",
    "amount_in", // encoder amount validation (NaN/non-positive/sub-wei)
    "missing decimals",
    "missing route",
];

/// Substrings identifying ECONOMIC failures (market rejected the trade).
const ECONOMIC_MARKERS: &[&str] = &[
    "negative_net_profit",
    "non_positive_profit",
    "slippage_too_high",
    "price_impact",
    "insufficient_liquidity",
    "gas_unprofitable",
    "min_profit_gate",
    "min_profit",
];

/// Substrings identifying MARKET/EVM failures.
const MARKET_MARKERS: &[&str] = &[
    "route_revert",
    "state_changed",
    "pool_state_invalid",
    "deadline",
    "token_behavior",
    "stale_state",
];

/// Classify a simulation `fail_reason` into its S4-02 family.
///
/// Matching order: ECONOMIC → MARKET → STRUCTURAL markers, then the
/// fail-closed default. Unknown/unrecognized reasons are STRUCTURAL
/// (fail-closed: they never produce a calibration label).
pub fn classify_fail_reason(reason: &str) -> FailFamily {
    let r = reason.to_lowercase();
    for m in ECONOMIC_MARKERS {
        if r.contains(m) {
            return FailFamily::Economic;
        }
    }
    for m in MARKET_MARKERS {
        if r.contains(m) {
            return FailFamily::Market;
        }
    }
    for m in STRUCTURAL_MARKERS {
        if r.contains(m) {
            return FailFamily::Structural;
        }
    }
    // Unknown reason — STRUCTURAL by default (fail-closed).
    FailFamily::Structural
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn economic_reasons_classify_economic() {
        for r in [
            "negative_net_profit",
            "non_positive_profit",
            "slippage_too_high",
            "price_impact_exceeded",
            "insufficient_liquidity",
            "gas_unprofitable",
            "min_profit_gate",
        ] {
            assert_eq!(classify_fail_reason(r), FailFamily::Economic, "{r}");
        }
    }

    #[test]
    fn market_reasons_classify_market() {
        for r in [
            "route_revert",
            "state_changed",
            "pool_state_invalid",
            "deadline_expired",
            "token_behavior",
            "stale_state",
        ] {
            assert_eq!(classify_fail_reason(r), FailFamily::Market, "{r}");
        }
    }

    #[test]
    fn structural_reasons_classify_structural() {
        for r in [
            "TRANSFER_FROM_FAILED",
            "transfer_from_failed (fixture)",
            "gas_oracle_missing",
            "candidate_incomplete",
            "route_encoding_missing",
            "decimals_missing",
            "backend_not_configured",
            "fork_unavailable",
            "archive_rpc_failed",
            "signer_balance_missing",
            "allowance_missing",
        ] {
            assert_eq!(classify_fail_reason(r), FailFamily::Structural, "{r}");
        }
    }

    #[test]
    fn sim_ctl_typed_tags_classify_structural() {
        // Exact tags the current sim-ctl emits — must never become labels.
        for r in [
            "b2c_encode_failed:MissingDecimals",
            "b2c_spawn_blocking_join:JoinError",
            "gas_price_wei is zero in Redis",
            "gas_price_wei key arbx:gas:1 not in Redis",
        ] {
            assert_eq!(classify_fail_reason(r), FailFamily::Structural, "{r}");
        }
    }

    #[test]
    fn unknown_reason_fails_closed_structural() {
        // Fail-closed: unrecognized tags are structural (ineligible), never labels.
        assert_eq!(
            classify_fail_reason("something_new_we_never_saw"),
            FailFamily::Structural
        );
        assert_eq!(classify_fail_reason(""), FailFamily::Structural);
    }

    #[test]
    fn serde_roundtrip_lowercase() {
        let json = serde_json::to_string(&FailFamily::Economic).unwrap();
        assert_eq!(json, "\"economic\"");
        let back: FailFamily = serde_json::from_str("\"market\"").unwrap();
        assert_eq!(back, FailFamily::Market);
    }

    #[test]
    fn as_str_matches_serde() {
        for f in [
            FailFamily::Structural,
            FailFamily::Economic,
            FailFamily::Market,
        ] {
            let json = serde_json::to_string(&f).unwrap();
            assert_eq!(json, format!("\"{}\"", f.as_str()));
        }
    }
}
