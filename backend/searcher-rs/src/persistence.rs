//! Postgres persistence for opportunities.
//!
//! Uses plain `sqlx::query` (not `query!`) to avoid the offline-data requirement
//! for compilation. Types are hand-bound.

use anyhow::Context;
use prioritization_spine::route_plan::RoutePlan;
use shared_rs::candidates::{DecimalsMap, RouteMetadata};
use shared_rs::contracts::Opportunity;
use sqlx::{postgres::PgPool, types::BigDecimal};
use std::str::FromStr;

// CARDS-MIRROR-01 — the opportunities table `status` state-machine.
//
// Sourced from the schema CHECK constraint (migration 003_opportunities.sql:20):
//   status TEXT NOT NULL DEFAULT 'detected' CHECK (status IN (
//     'detected','validated','simulated','scored','executing',
//     'executed','reconciled','rejected','failed'
//   ))
//
// An opportunity is VIABLE (cards-visible) when it carries NO rejection_reason —
// its lifecycle is one of the forward states below. It is REJECTED when a gate
// set a reason, regardless of how far it advanced. The `rejection_reason` field
// is the single source of truth for which class a row belongs to (R8: NULL =
// not rejected, Some(reason) = rejected with the honest reason).
//
// This array is the ONLY place the viable-state set is declared; the
// /opportunities/live LIVE_QUERY and this function both read from it so a new
// schema state only needs adding here (single source of truth, no scattered
// literals). The mirror must hold: a row with rejection_reason IS NULL must
// always land in a status ∈ VIABLE_STATUSES so the LIVE_QUERY surfaces it.
pub const VIABLE_STATUSES: &[&str] = &["detected", "validated", "simulated", "scored"];

/// The terminal non-viable status a row takes when a gate rejected it. Mirrors
/// the paper executor's `REJECTED` classification (executor.ts:250) so the
/// opportunities table and paper_trade_runs agree on the row's class.
pub const REJECTED_STATUS: &str = "rejected";

/// Derive the `opportunities.status` value from `rejection_reason`.
///
/// NULL  → the row's forward lifecycle state (default `detected`; a future
///         enrichment pass can UPDATE it to validated/simulated/scored).
/// Some  → `rejected` — the row is visible in the dashboard with its honest
///         reason, never silently dropped (R8). This replaces the old bug where
///         `status` was hardcoded to `'detected'` for BOTH accepted and
///         rejected rows, producing a detected-row-with-a-reason contradiction
///         that the LIVE_QUERY's `rejection_reason IS NULL` filter excluded —
///         hiding 100% of rejected opportunities and (once the bridge writes
///         accepted rows too) any row the gates hadn't yet promoted.
///
/// Pure + total: covers every `Option<String>` input. No literals inline in
/// the SQL INSERT — this is the single derivation point.
pub fn status_from_rejection_reason(rejection_reason: &Option<String>) -> &'static str {
    match rejection_reason {
        None => VIABLE_STATUSES[0], // 'detected' — forward lifecycle entry point
        Some(_) => REJECTED_STATUS,
    }
}

/// Insert an opportunity WITHOUT route metadata (legacy path).
///
/// `route_metadata` column defaults to `'{}'::jsonb`. Use
/// [`insert_opportunity_with_route`] when the caller has the complete
/// multi-hop topology so sim-ctl can reconstruct an `OpportunityCandidate`
/// via the A1 enrichment path.
pub async fn insert_opportunity(pool: &PgPool, o: &Opportunity) -> anyhow::Result<()> {
    insert_opportunity_with_route(pool, o, None).await
}

/// Insert an opportunity WITH complete route metadata (G-SIM-1 PR-B2b Fase 2 A1).
///
/// `route_metadata` carries the full multi-hop topology (pool_addresses,
/// token_addresses, dex_adapters, decimals) that sim-ctl needs to construct
/// an `OpportunityCandidate` for real REVM simulation. Pass `None` to leave
/// the column at its default (`'{}'::jsonb`) — equivalent to the legacy path.
///
/// Fail-honest: a malformed topology is persisted as `'{}'` with a warn log
/// rather than crashing the hot path (R8: a detection without valid route
/// topology is still a real detection worth persisting).
pub async fn insert_opportunity_with_route(
    pool: &PgPool,
    o: &Opportunity,
    route: Option<&RouteMetadata>,
) -> anyhow::Result<()> {
    let amount_in_wei =
        BigDecimal::from_str(&o.amount_in_wei).context("amount_in_wei to BigDecimal")?;

    // Serialize route_metadata to JSON. Structural check only; on failure,
    // log warn and persist '{}' so the row still lands.
    //
    // ROOT-CAUSE FIX (2026-08-10): the old gate called `rm.validate()`, which
    // REQUIRES every token_address to have a decimals entry. But every builder
    // (build_route_metadata_from_plan AND the per-engine constructors) emits
    // `decimals` EMPTY BY DESIGN — decimals are resolved separately downstream
    // (scanner TokenDecimalsProvider / sim-ctl A1 enrichment), and the
    // documented intent is "persist topology without decimals rather than
    // fabricate them". So `validate()` ALWAYS failed → every route_metadata was
    // silently downgraded to '{}' → the exchange dashboard never saw any
    // multi-leg topology. We now gate on STRUCTURE only (parallel-array lengths
    // consistent), matching the documented design and letting topology persist.
    let route_json: serde_json::Value = match route {
        Some(rm) if rm.is_populated() => {
            let hops = rm.dex_adapters.len();
            // Token path must be consistent (hops+1). Pools may be FEWER than
            // hops — some legs legitimately have no resolved pool/factory at
            // scan time (build_route_metadata_from_plan skips those), and the
            // merge in orchestrator may leave pools shorter than hops. The token
            // path is the load-bearing invariant; pools/dexes are advisory.
            let structurally_ok =
                rm.token_addresses.len() == hops + 1 && rm.pool_addresses.len() <= hops;
            if !structurally_ok {
                tracing::warn!(
                    event = "persist.route_metadata_invalid",
                    opportunity_id = %o.id,
                    reason = %format!(
                        "structural mismatch: token_addresses={} pool_addresses={} dex_adapters={}",
                        rm.token_addresses.len(),
                        rm.pool_addresses.len(),
                        hops
                    ),
                    "route_metadata structural check failed; persisting '{{}}' instead"
                );
                serde_json::json!({})
            } else {
                serde_json::to_value(rm).unwrap_or_else(|_| serde_json::json!({}))
            }
        }
        _ => serde_json::json!({}),
    };

    // CARDS-MIRROR-01: derive `status` from `rejection_reason` via a pure
    // function over the schema's status enum — NO literal inline in the SQL.
    // The old INSERT always wrote status='detected' regardless of whether the
    // opportunity was accepted (emit_accepted) or rejected (emit_rejected). A
    // rejected row landed as status='detected' WITH its rejection_reason
    // populated — a logical contradiction. The /opportunities/live LIVE_QUERY
    // filters `status IN (viable...) AND rejection_reason IS NULL`, so these
    // contradictory rows were excluded → cards showed 0 while paper_trade_runs
    // (which derives status correctly in the paper executor) had 579K rows.
    // `status_from_rejection_reason` is the single source of truth: it reads the
    // `VIABLE_STATUSES` array (sourced from the opportunities table CHECK
    // constraint, migration 003) — if the schema grows a new viable state, the
    // array is the only thing that changes, not the SQL or callers.
    let status = status_from_rejection_reason(&o.rejection_reason);

    let result = sqlx::query(
        r#"
        INSERT INTO opportunities (
            id, chain_id, strategy_kind, dex_a, dex_b, pair_symbol,
            token_in, token_out, amount_in_wei,
            expected_profit_usd, net_expected_profit_usd, roi_pct, risk_score,
            block_number, status, rejection_reason, trace_id, detected_at,
            route_metadata, cartridge_id
        ) VALUES (
            $1, $2, $3, $4, $5, $6,
            $7, $8, $9,
            $10, $11, $12, $13,
            $14, $20, $15, $16, $17,
            $18, $19
        )
        ON CONFLICT (id) DO NOTHING
        "#,
    )
    .bind(o.id)
    .bind(o.chain_id as i64)
    .bind(o.strategy_kind.as_str())
    .bind(&o.dex_a)
    .bind(o.dex_b.as_deref())
    .bind(&o.pair_symbol)
    .bind(&o.token_in)
    .bind(&o.token_out)
    .bind(amount_in_wei)
    .bind(o.expected_profit_usd)
    .bind(o.net_expected_profit_usd)
    .bind(o.roi_pct)
    .bind(o.risk_score)
    .bind(o.block_number.map(|n| n as i64))
    .bind(o.rejection_reason.as_deref())
    .bind(o.trace_id)
    .bind(o.detected_at)
    .bind(route_json)
    .bind(o.cartridge_id.as_deref())
    .bind(status)
    .execute(pool)
    .await
    .context("insert opportunity")?;
    // H2 (FREEZE-01): record the commit for the PIPELINE_SILENCE watchdog
    // (monitoring/alerts.rules.yml). RULE 00: only an INSERT that actually
    // landed a row moves this gauge — never a fabricated tick. The statement
    // is `ON CONFLICT (id) DO NOTHING`, so a committed-but-duplicate
    // redelivery (rows_affected == 0) must NOT tick it: a duplicate-only
    // loop is exactly the "pipeline frozen but looks alive" signature the
    // watchdog exists to catch. This is the single funnel for every
    // opportunities write in this crate, so one hook covers all workers +
    // the orchestrator emitter path.
    if result.rows_affected() > 0 {
        crate::metrics::PIPELINE_LAST_OPPORTUNITY_INSERT_UNIXTIME
            .set(chrono::Utc::now().timestamp());
    }
    Ok(())
}

/// Build a `RouteMetadata` from a scanner `RoutePlan` (G-SIM-1 B2b step 6).
///
/// Extracts the multi-hop topology from the route plan's legs:
/// - `token_addresses`: ordered path [token_in_leg0, token_out_leg0==token_in_leg1, ...]
/// - `pool_addresses`: one per leg (pool_address → factory_address fallback → skip if both empty)
/// - `dex_adapters`: one per leg (dex_name)
/// - `decimals`: EMPTY — the scanner resolves decimals separately via `TokenDecimalsProvider`.
///   The A1 enrichment path will reject with MissingDecimals until a follow-up
///   threads the resolved decimals into this builder. Fail-honest: better to
///   persist topology without decimals than fabricate them.
///
/// Returns `RouteMetadata::empty()` when the plan has no legs (defensive).
pub fn build_route_metadata_from_plan(plan: &RoutePlan) -> RouteMetadata {
    if plan.legs.is_empty() {
        return RouteMetadata::empty();
    }

    let mut pool_addresses = Vec::with_capacity(plan.legs.len());
    let mut dex_adapters = Vec::with_capacity(plan.legs.len());
    let mut token_addresses: Vec<String> = Vec::with_capacity(plan.legs.len() + 1);

    for (i, leg) in plan.legs.iter().enumerate() {
        // First leg seeds token_addresses with token_in; subsequent legs append token_out.
        if i == 0 {
            token_addresses.push(leg.token_in.clone());
        }
        token_addresses.push(leg.token_out.clone());

        // pool_address → factory_address fallback. Both may be empty/None at
        // scan time (calldata decoder doesn't resolve the pool yet). Skip the
        // entry only when BOTH are absent — keeps pool_addresses length aligned
        // with dex_adapters for validate().
        let pool = leg
            .pool_address
            .clone()
            .filter(|s| !s.is_empty())
            .or_else(|| {
                if !leg.factory_address.is_empty() {
                    Some(leg.factory_address.clone())
                } else {
                    None
                }
            })
            .unwrap_or_default();
        pool_addresses.push(pool);
        dex_adapters.push(leg.dex_name.clone());
    }

    RouteMetadata {
        pool_addresses,
        token_addresses,
        dex_adapters,
        // decimals intentionally empty — resolved separately by the scanner's
        // TokenDecimalsProvider in a follow-up. See doc comment above.
        decimals: DecimalsMap::new(),
    }
}

// =============================================================================
// Fidelity tests — build_route_metadata_from_plan token-path preservation
// =============================================================================
// These lock the multi-leg fidelity contract: the builder must extract the
// exact traversal token path [A,B,C,A] from the plan's legs, and `is_populated`
// must reflect whether a usable topology exists. RULE 00: addresses are
// placeholders only (test-local), not fabricated operator/trading data.
#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod fidelity_tests {
    use super::*;
    use prioritization_spine::route_plan::{RouteLeg, RoutePlan};

    fn leg(token_in: &str, token_out: &str, pool: Option<&str>, dex: &str) -> RouteLeg {
        RouteLeg {
            dex_id: dex.to_string(),
            dex_name: dex.to_string(),
            protocol_type: dex.to_string(),
            factory_address: String::new(),
            pool_id: None,
            pool_address: pool.map(str::to_string),
            token_in: token_in.to_string(),
            token_out: token_out.to_string(),
            fee_bps: None,
            amount_in: None,
            amount_out: None,
            tvl_usd: None,
            volume_24h_usd: None,
            pool_is_active: true,
        }
    }

    fn plan(legs: Vec<RouteLeg>) -> RoutePlan {
        RoutePlan {
            route_id: Some("test".into()),
            strategy_kind: "test".into(),
            chain_id: 1,
            legs,
            atomic: true,
            estimated_slippage_pct: None,
            price_impact_pct: None,
        }
    }

    #[test]
    fn build_from_plan_extracts_full_triangular_token_path() {
        let p = plan(vec![
            leg("0xA", "0xB", Some("0xp1"), "uniswap_v2_router"),
            leg("0xB", "0xC", Some("0xp2"), "uniswap_v2_router"),
            leg("0xC", "0xA", Some("0xp3"), "uniswap_v2_router"),
        ]);
        let rm = build_route_metadata_from_plan(&p);
        assert_eq!(rm.token_addresses, vec!["0xA", "0xB", "0xC", "0xA"]);
        assert_eq!(rm.pool_addresses.len(), 3);
        assert_eq!(rm.dex_adapters.len(), 3);
        assert!(rm.is_populated());
    }

    #[test]
    fn build_from_plan_empty_legs_returns_empty() {
        let p = plan(vec![]);
        let rm = build_route_metadata_from_plan(&p);
        assert!(!rm.is_populated());
    }

    #[test]
    fn build_from_plan_two_leg_dex_path() {
        let p = plan(vec![
            leg("0xA", "0xB", Some("0xp1"), "uniswap_v2_router"),
            leg("0xB", "0xA", Some("0xp2"), "sushiswap"),
        ]);
        let rm = build_route_metadata_from_plan(&p);
        assert_eq!(rm.token_addresses, vec!["0xA", "0xB", "0xA"]);
        assert_eq!(rm.dex_adapters, vec!["uniswap_v2_router", "sushiswap"]);
        assert!(rm.is_populated());
    }

    #[test]
    fn build_from_plan_keeps_pool_entry_empty_when_leg_lacks_pool() {
        // A leg with neither pool_address nor factory → pool entry is an empty
        // string, but still pushed (keeps pools aligned to dex_adapters by
        // index). The structural gate (pools <= hops) tolerates this.
        let p = plan(vec![
            leg("0xA", "0xB", None, "uniswap_v2_router"),
            leg("0xB", "0xA", Some("0xp1"), "uniswap_v2_router"),
        ]);
        let rm = build_route_metadata_from_plan(&p);
        assert_eq!(rm.token_addresses, vec!["0xA", "0xB", "0xA"]);
        assert_eq!(rm.pool_addresses.len(), 2);
        assert!(rm.pool_addresses[0].is_empty());
        assert!(!rm.pool_addresses[1].is_empty());
    }

    // HOPS-EMIT-01: the cartridge path (cartridge_boot.rs) now persists the
    // topology from the plan legs on EVERY emit — accepted AND rejected. This
    // locks the contract that makes that safe: the cartridge-shape plan (legs
    // from intent, optional pool/dex hints → "" / "unknown" fallbacks) yields a
    // closed traversal that PASSES the insert_opportunity_with_route structural
    // gate, while the flattened per-leg-pair token_addresses the cartridge
    // `OpportunityCandidate` carries ([A,B,B,C,C,A]) would FAIL it — the reason
    // the plan is the only valid source on that path.
    #[test]
    fn build_from_plan_cartridge_shape_passes_structural_gate() {
        let p = plan(vec![
            leg("0xA", "0xB", Some("0xp1"), "uniswap_v2_router"),
            leg("0xB", "0xC", None, "unknown"),
            leg("0xC", "0xA", Some("0xp3"), "sushiswap"),
        ]);
        let rm = build_route_metadata_from_plan(&p);
        assert_eq!(rm.token_addresses, vec!["0xA", "0xB", "0xC", "0xA"]);
        let hops = rm.dex_adapters.len();
        assert_eq!(hops, 3);
        // Hint-less middle leg → honest empty pool (the cartridge legs carry
        // factory_address empty like every other producer — a zero-address
        // sentinel would persist as a fake pool via the factory fallback).
        assert!(rm.pool_addresses[1].is_empty());
        // insert_opportunity_with_route gate: tokens == hops+1, pools <= hops.
        assert_eq!(rm.token_addresses.len(), hops + 1);
        assert!(rm.pool_addresses.len() <= hops);
        assert!(rm.is_populated());

        // Contrast: the cartridge candidate's flattened per-leg pairs.
        let candidate_flattened = RouteMetadata {
            pool_addresses: vec!["0xp1".to_string(), "0xp3".to_string()],
            token_addresses: vec![
                "0xA".to_string(),
                "0xB".to_string(),
                "0xB".to_string(),
                "0xC".to_string(),
                "0xC".to_string(),
                "0xA".to_string(),
            ],
            dex_adapters: vec!["uniswap_v2_router".to_string(), "sushiswap".to_string()],
            decimals: DecimalsMap::new(),
        };
        assert_ne!(
            candidate_flattened.token_addresses.len(),
            candidate_flattened.dex_adapters.len() + 1,
            "flattened candidate path must FAIL the structural gate — documents why the plan legs are the source"
        );
    }
}
