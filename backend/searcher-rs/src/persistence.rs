//! Postgres persistence for opportunities.
//!
//! Uses plain `sqlx::query` (not `query!`) to avoid the offline-data requirement
//! for compilation. Types are hand-bound.

use anyhow::Context;
use shared_rs::candidates::RouteMetadata;
use shared_rs::contracts::{Opportunity, StrategyKind};
use sqlx::{postgres::PgPool, types::BigDecimal};
use std::str::FromStr;

fn strategy_kind_str(k: &StrategyKind) -> &'static str {
    match k {
        StrategyKind::DexArb => "dex_arb",
        StrategyKind::Triangular => "triangular",
        StrategyKind::Backrun => "backrun",
        StrategyKind::Liquidation => "liquidation",
        StrategyKind::FlashloanArb => "flashloan_arb",
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

    // Serialize route_metadata to JSON. Validate first; on failure, log warn
    // and persist '{}' so the row still lands.
    let route_json: serde_json::Value = match route {
        Some(rm) if rm.is_populated() => {
            if let Err(reason) = rm.validate() {
                tracing::warn!(
                    event = "persist.route_metadata_invalid",
                    opportunity_id = %o.id,
                    reason = %reason,
                    "route_metadata validation failed; persisting '{{}}' instead"
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
            route_metadata
        ) VALUES (
            $1, $2, $3, $4, $5, $6,
            $7, $8, $9,
            $10, $11, $12, $13,
            $14, 'detected', $15, $16, $17,
            $18
        )
        ON CONFLICT (id) DO NOTHING
        "#,
    )
    .bind(o.id)
    .bind(o.chain_id as i64)
    .bind(strategy_kind_str(&o.strategy_kind))
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
    .execute(pool)
    .await
    .context("insert opportunity")?;
    Ok(())
}
