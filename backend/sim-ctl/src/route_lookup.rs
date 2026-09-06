//! G-SIM-1 PR-B2b Fase 4 (A3) — sim-ctl autonomous PG route lookup.
//!
//! When `route_source = "simctl_lookup"`, sim-ctl queries the
//! `opportunities.route_metadata` JSONB column DIRECTLY (no api-server
//! round-trip, no searcher-rs dependency). This is the most autonomous path:
//! sim-ctl is self-sufficient for enrichment.
//!
//! ## Why this path exists
//!
//! A1 (api-server enrich) and A2 (searcher-rs API) both require an upstream
//! service to provide route metadata. A3 lets sim-ctl operate standalone —
//! useful when api-server or searcher-rs are down, or for direct sim-ctl
//! consumers (e.g. the Redis Streams consumer) that don't go through api-server.
//!
//! R8 fail-honest: returns `None` on PG error or empty route_metadata.
//! Never fabricates topology.

use shared_rs::candidates::{DecimalsMap, RouteMetadata};
use sqlx::{postgres::PgPool, types::Json};
use uuid::Uuid;

/// Everything the A3 path needs to build an `OpportunityCandidate` (S4 PARCH-0):
/// route topology from `route_metadata` + the economic fields from the
/// opportunities row itself + decimals resolved downstream.
#[derive(Debug)]
pub struct CandidateInputs {
    pub route_metadata: RouteMetadata,
    pub chain_id: i32,
    pub dex_a: String,
    pub token_in: String,
    pub token_out: String,
    /// Raw wei string (NUMERIC(78,0) cast to text — exceeds i64 in the wild).
    pub amount_in_wei: String,
    pub block_number: Option<i64>,
    /// Decimals for the route's tokens, resolved by [`resolve_route_decimals`]
    /// (tokens table + route_metadata overlay). The searcher persists
    /// `route_metadata.decimals` EMPTY BY DESIGN ("resolved downstream" —
    /// persistence.rs); this is that downstream resolution landing.
    pub resolved_decimals: DecimalsMap,
}

/// Raw DB row shape for `fetch_candidate_inputs` (S4 PARCH-0).
#[derive(sqlx::FromRow)]
struct CandidateInputsRow {
    route_metadata: Json<RouteMetadata>,
    chain_id: i32,
    dex_a: String,
    token_in: String,
    token_out: String,
    amount_in_wei: String,
    block_number: Option<i64>,
}

/// Merge decimals resolved from the `tokens` table with any decimals already
/// persisted in `route_metadata` (the row's own claim wins — forward-compat
/// for the day the searcher persists them). Pure — unit-testable without PG.
///
/// Rows whose `decimals` falls outside `0..=255` (SMALLINT can hold them;
/// `u8` cannot) are skipped honestly rather than truncated into garbage.
pub fn merge_decimals(rm: &DecimalsMap, token_rows: &[(String, i16)]) -> DecimalsMap {
    let mut merged = DecimalsMap::new();
    for (addr, decimals) in token_rows {
        if (0..=255).contains(decimals) {
            merged.insert(addr.clone(), *decimals as u8);
        }
    }
    // Overlay persisted route_metadata decimals last: the row's own claim wins.
    for (addr, decimals) in rm.map.iter() {
        merged.insert(addr.clone(), *decimals);
    }
    merged
}

/// Resolve decimals for the route's tokens from the `tokens` table (raw rows;
/// see [`merge_decimals`] for the precedence merge with route_metadata).
///
/// The searcher persists `route_metadata.decimals` EMPTY BY DESIGN
/// ("resolved downstream" — persistence.rs); the `tokens` table is the
/// reconciled, enriched store (migrations 072/098, ~3.7k rows with decimals
/// on chain 1). R8: no fabrication — a token absent from both sources simply
/// has no entry, and the caller's completeness gate names it in the typed 422.
async fn resolve_route_decimals(
    pool: &PgPool,
    chain_id: i32,
    token_addresses: &[String],
) -> Result<Vec<(String, i16)>, sqlx::Error> {
    let lowered: Vec<String> = token_addresses.iter().map(|a| a.to_lowercase()).collect();
    let token_rows: Vec<(String, i16)> = sqlx::query_as(
        r#"
        SELECT address, decimals
        FROM tokens
        WHERE chain_id = $1 AND lower(address) = ANY($2) AND decimals IS NOT NULL
        "#,
    )
    .bind(chain_id)
    .bind(&lowered)
    .fetch_all(pool)
    .await?;
    Ok(token_rows)
}

/// Fetch route metadata + economic fields for candidate construction (S4 PARCH-0).
///
/// Returns:
/// - `Ok(Some(inputs))` when the row exists and route_metadata is populated
/// - `Ok(None)` when the row doesn't exist OR route_metadata is empty/unpopulated
///   (`{}` or empty arrays — treated as absent, R8: never fabricate topology)
/// - `Err(sqlx::Error)` on PG connectivity issues (caller surfaces 500)
///
/// Economic fields are returned raw; conversion/validation lives in the
/// caller so it can emit typed `candidate_incomplete` responses naming
/// exactly what was missing.
pub async fn fetch_candidate_inputs(
    pool: &PgPool,
    opportunity_id: Uuid,
) -> Result<Option<CandidateInputs>, sqlx::Error> {
    let row: Option<CandidateInputsRow> = sqlx::query_as(
        r#"
        SELECT o.route_metadata, o.chain_id, o.dex_a, o.token_in, o.token_out,
               o.amount_in_wei::text, o.block_number
        FROM opportunities o
        WHERE o.id = $1
        "#,
    )
    .bind(opportunity_id)
    .fetch_optional(pool)
    .await?;

    let row = match row {
        None => return Ok(None),
        Some(row) => {
            if !row.route_metadata.is_populated() {
                return Ok(None);
            }
            row
        }
    };

    // S4-A3 decimals resolution: tokens table, overlaid by any persisted
    // route_metadata decimals (the row's own claim wins).
    let token_rows =
        resolve_route_decimals(pool, row.chain_id, &row.route_metadata.token_addresses).await?;
    let resolved_decimals = merge_decimals(&row.route_metadata.decimals, &token_rows);

    Ok(Some(CandidateInputs {
        route_metadata: row.route_metadata.0,
        chain_id: row.chain_id,
        dex_a: row.dex_a,
        token_in: row.token_in,
        token_out: row.token_out,
        amount_in_wei: row.amount_in_wei,
        block_number: row.block_number,
        resolved_decimals,
    }))
}

#[cfg(test)]
mod tests {
    // NOTE: integration tests for fetch_route_metadata require a live PG with
    // the opportunities table + migration 099 applied. See
    // sim-ctl/tests/route_lookup_integration.rs for the live-PG test (CI runs
    // it under the integration-tests job with DATABASE_URL set).

    use super::*;

    #[test]
    fn test_route_metadata_populated_detection() {
        // Unit-test the is_populated() logic (doesn't need PG).
        let rm = RouteMetadata {
            pool_addresses: vec!["0xpool1".into()],
            token_addresses: vec!["0xtokenIn".into(), "0xtokenOut".into()],
            dex_adapters: vec!["uniswap_v2_router".into()],
            decimals: shared_rs::candidates::DecimalsMap::new(),
            leg_amounts_in: None,
            leg_amounts_out: None,
            leg_zero_for_one: None,
        };
        assert!(rm.is_populated());
    }

    #[test]
    fn test_route_metadata_empty_not_populated() {
        let rm = RouteMetadata::empty();
        assert!(!rm.is_populated());
    }

    #[test]
    fn merge_decimals_uses_tokens_table_when_rm_empty() {
        // The production reality: route_metadata.decimals is empty by design.
        let rm = DecimalsMap::new();
        let rows = vec![
            ("0xC02AAA39b223FE8D0A0e5C4F27eAD9083C756Cc2".to_string(), 18), // WETH checksummed
            ("0x974733a3f37208647577bb925d8ee854c7337e29".to_string(), 9),
        ];
        let merged = merge_decimals(&rm, &rows);
        // Lookup is case-insensitive (DecimalsMap lowercases on insert).
        assert_eq!(
            merged.get("0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2"),
            Some(18)
        );
        assert_eq!(
            merged.get("0x974733a3f37208647577bb925d8ee854c7337e29"),
            Some(9)
        );
    }

    #[test]
    fn merge_decimals_rm_claim_wins_over_tokens_table() {
        // Forward-compat: if the searcher starts persisting decimals, the
        // row's own claim takes precedence over the tokens table.
        let mut rm = DecimalsMap::new();
        rm.insert("0xaaaa".to_string(), 6);
        let rows = vec![("0xAAAA".to_string(), 18)];
        let merged = merge_decimals(&rm, &rows);
        assert_eq!(merged.get("0xaaaa"), Some(6));
    }

    #[test]
    fn merge_decimals_skips_out_of_u8_range() {
        // SMALLINT can hold values u8 cannot — skip honestly, never truncate.
        let rm = DecimalsMap::new();
        let rows = vec![
            ("0xbbbb".to_string(), 300), // > u8::MAX
            ("0xcccc".to_string(), -1),  // negative
            ("0xdddd".to_string(), 0),   // valid boundary
        ];
        let merged = merge_decimals(&rm, &rows);
        assert_eq!(merged.get("0xbbbb"), None);
        assert_eq!(merged.get("0xcccc"), None);
        assert_eq!(merged.get("0xdddd"), Some(0));
    }
}
