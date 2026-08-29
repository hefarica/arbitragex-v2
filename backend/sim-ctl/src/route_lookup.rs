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

use shared_rs::candidates::RouteMetadata;
use sqlx::{postgres::PgPool, types::Json};
use uuid::Uuid;

/// Everything the A3 path needs to build an `OpportunityCandidate` (S4 PARCH-0):
/// route topology from `route_metadata` + the economic fields from the
/// opportunities row itself. One round-trip.
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

    match row {
        None => Ok(None),
        Some(row) => {
            if row.route_metadata.is_populated() {
                Ok(Some(CandidateInputs {
                    route_metadata: row.route_metadata.0,
                    chain_id: row.chain_id,
                    dex_a: row.dex_a,
                    token_in: row.token_in,
                    token_out: row.token_out,
                    amount_in_wei: row.amount_in_wei,
                    block_number: row.block_number,
                }))
            } else {
                Ok(None)
            }
        }
    }
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
        };
        assert!(rm.is_populated());
    }

    #[test]
    fn test_route_metadata_empty_not_populated() {
        let rm = RouteMetadata::empty();
        assert!(!rm.is_populated());
    }
}
