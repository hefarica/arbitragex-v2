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

    sqlx::query(
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
            $14, 'detected', $15, $16, $17,
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
    .execute(pool)
    .await
    .context("insert opportunity")?;
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
}
