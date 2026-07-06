//! `RoutePlan` — structured route metadata for the StrategyConfigGate.
//!
//! Background (anti_reincidencia 2026-05-10 schema-drift incident):
//!   The dashboard showed dex toggles in /strategies that the engine was
//!   silently ignoring. Root cause: `enabled_dex_ids` existed in API + Redis
//!   but not in the Rust struct, AND the worker emitted a flat candidate
//!   without per-leg DEX UUIDs. Fixing only the struct (Phase 1) closed half
//!   the bug; closing the other half required a route contract.
//!
//! Decision B (Incremental migration, see 2026-05-10 design notes):
//!   - `RoutePlan` is OPTIONAL on `OpportunityCandidate` for now.
//!   - When `Some(_)`: gate enforces per-leg DEX/protocol/pool checks.
//!   - When `None` (legacy worker not yet migrated): gate falls back to
//!     `OpportunityCandidate.dex_adapters` (by-name allowlist) + minimal
//!     route shape inference. Operator sees a "data_quality=partial" hint
//!     in the dashboard for these candidates.
//!
//! Workers migrate ONE AT A TIME (dex_arb_v2v2 first, the most active path).
//! Non-migrated workers keep emitting candidates without RoutePlan; the
//! gate's degrade-graceful behaviour preserves their flow.

use serde::{Deserialize, Serialize};

/// One leg of an arbitrage route. Each leg corresponds to a single swap on
/// a specific pool of a specific DEX.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RouteLeg {
    /// Canonical DEX UUID (matches `dexes.id` in PostgreSQL). Lowercase
    /// canonical hex is preferred for case-insensitive matching against
    /// `enabled_dex_ids`.
    pub dex_id: String,
    /// Human-readable DEX adapter name (e.g. "uniswap-v2", "uniswap-v3",
    /// "curve", "balancer"). Used by `default_fee_bps_for_adapter` and as
    /// fallback when `dex_id` is unknown.
    pub dex_name: String,
    /// Protocol type (e.g. "uniswap-v2", "uniswap-v3", "curve",
    /// "balancer"). Drives the per-strategy `enabled_protocol_types` gate
    /// and downstream math selection.
    pub protocol_type: String,
    /// Factory address (lowercase hex). Cross-checked against the chain's
    /// `factories` table when validating that the pool belongs to the
    /// claimed DEX.
    pub factory_address: String,
    /// Pool UUID from `pools` (None when the pool was constructed in-memory
    /// and not yet persisted — gate then skips pool-level checks).
    #[serde(default)]
    pub pool_id: Option<String>,
    /// Pool contract address (lowercase hex). Required for V2/V3/Curve
    /// pools; some virtual routes (e.g. cross-chain bridge legs) may have
    /// `None`.
    #[serde(default)]
    pub pool_address: Option<String>,
    /// Token-in address for this leg (lowercase hex).
    pub token_in: String,
    /// Token-out address for this leg (lowercase hex).
    pub token_out: String,
    /// LP fee in basis points (e.g. 30 for V2 0.30%, 500 for V3 0.05%).
    #[serde(default)]
    pub fee_bps: Option<u32>,
    /// Amount in for this leg (in raw token units, NOT USD). f64 is
    /// sufficient for routing math; precise wei-arithmetic happens
    /// downstream in revm simulation.
    #[serde(default)]
    pub amount_in: Option<f64>,
    /// Amount out for this leg (raw token units).
    #[serde(default)]
    pub amount_out: Option<f64>,
    /// Pool TVL in USD (snapshot at scoring time). `None` = unknown.
    /// Gate uses this for `min_pool_tvl_usd` route-constraint checks.
    /// R8 fail-honest: missing TVL → check skipped, never synthesised.
    #[serde(default)]
    pub tvl_usd: Option<f64>,
    /// Pool 24h trading volume in USD. Same semantics as `tvl_usd`.
    #[serde(default)]
    pub volume_24h_usd: Option<f64>,
    /// True if the pool is currently active (`pools.is_active = true` in
    /// PG). Inactive pools are rejected by the gate even if every other
    /// constraint passes.
    #[serde(default = "default_pool_is_active")]
    pub pool_is_active: bool,
}

fn default_pool_is_active() -> bool {
    true // permissive default: caller didn't specify, assume active
}

/// Full route plan for an opportunity. Carries the ordered legs plus
/// route-level metadata the gate needs.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoutePlan {
    /// Stable identifier (ULID or fingerprint hash) — useful for dedup +
    /// dashboard threading. Optional because legacy paths don't compute it.
    #[serde(default)]
    pub route_id: Option<String>,
    /// Strategy classification (mirrors `OpportunityCandidate`'s strategy
    /// arg passed to `evaluate`). Stored on the plan for downstream
    /// consumers that don't have the call-site context (recon, audit).
    pub strategy_kind: String,
    /// EVM chain id of the route. For cross-chain bridge routes, this is
    /// the SOURCE chain; legs carry their per-leg chain context if needed.
    pub chain_id: u64,
    /// Ordered legs. Length must lie in
    /// `[route_constraints.min_legs, route_constraints.max_legs]` for the
    /// route to pass the shape gate.
    pub legs: Vec<RouteLeg>,
    /// True when the route can be executed in a single atomic transaction
    /// (no cross-chain bridge, no off-chain CEX leg, no waiting period).
    /// `route_constraints.require_atomic` rejects non-atomic routes.
    #[serde(default = "default_atomic")]
    pub atomic: bool,
    /// Estimated worst-case slippage % across all legs (0.5 = 0.5%). Used
    /// when the per-pool reserves snapshot is missing — falls back through
    /// to the chain-level / per-strategy `max_slippage_pct` cap.
    #[serde(default)]
    pub estimated_slippage_pct: Option<f64>,
    /// Estimated price impact % on the largest-impact leg. Same semantics
    /// as `estimated_slippage_pct`.
    #[serde(default)]
    pub price_impact_pct: Option<f64>,
}

fn default_atomic() -> bool {
    true // most routes are atomic; non-atomic must opt-in explicitly
}

impl RoutePlan {
    /// Returns the lowercase set of unique `dex_id` values the route
    /// touches — convenience for dashboard rendering and metric labels.
    pub fn unique_dex_ids(&self) -> Vec<String> {
        let mut ids: Vec<String> = self
            .legs
            .iter()
            .map(|l| l.dex_id.to_ascii_lowercase())
            .collect();
        ids.sort();
        ids.dedup();
        ids
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn leg_v2(dex_id: &str, name: &str) -> RouteLeg {
        RouteLeg {
            dex_id: dex_id.into(),
            dex_name: name.into(),
            protocol_type: "uniswap-v2".into(),
            factory_address: "0x0000000000000000000000000000000000000000".into(),
            pool_id: None,
            pool_address: Some("0xpool".into()),
            token_in: "0xtoken_in".into(),
            token_out: "0xtoken_out".into(),
            fee_bps: Some(30),
            amount_in: Some(1.0),
            amount_out: Some(0.99),
            tvl_usd: Some(1_000_000.0),
            volume_24h_usd: Some(500_000.0),
            pool_is_active: true,
        }
    }

    #[test]
    fn unique_dex_ids_dedup_case_insensitive() {
        let plan = RoutePlan {
            route_id: None,
            strategy_kind: "dex_arb_v2v2".into(),
            chain_id: 1,
            legs: vec![
                leg_v2("AAA-UUID", "uniswap-v2"),
                leg_v2("aaa-uuid", "uniswap-v2"),
                leg_v2("BBB-UUID", "sushi"),
            ],
            atomic: true,
            estimated_slippage_pct: None,
            price_impact_pct: None,
        };
        let ids = plan.unique_dex_ids();
        assert_eq!(ids, vec!["aaa-uuid".to_string(), "bbb-uuid".to_string()]);
    }

    #[test]
    fn deserialise_minimal_route_plan() {
        let json = r#"{
            "strategy_kind": "dex_arb_v2v2",
            "chain_id": 1,
            "legs": [
                {
                    "dex_id": "uuid-A",
                    "dex_name": "uniswap-v2",
                    "protocol_type": "uniswap-v2",
                    "factory_address": "0xfactory",
                    "token_in": "0xa",
                    "token_out": "0xb"
                }
            ]
        }"#;
        let plan: RoutePlan = serde_json::from_str(json).expect("must deserialise");
        assert_eq!(plan.strategy_kind, "dex_arb_v2v2");
        assert_eq!(plan.legs.len(), 1);
        // Default atomic = true when omitted
        assert!(plan.atomic);
        // Default pool_is_active = true when omitted
        assert!(plan.legs[0].pool_is_active);
    }
}
