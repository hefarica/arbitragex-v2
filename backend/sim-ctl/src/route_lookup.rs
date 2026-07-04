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

/// Fetch `RouteMetadata` for a single opportunity from PG.
///
/// Returns:
/// - `Ok(Some(route_metadata))` when the row exists and route_metadata is populated
/// - `Ok(None)` when the row doesn't exist OR route_metadata is empty/unpopulated
/// - `Err(sqlx::Error)` on PG connectivity issues (caller surfaces 503/500)
pub async fn fetch_route_metadata(
    pool: &PgPool,
    opportunity_id: Uuid,
) -> Result<Option<RouteMetadata>, sqlx::Error> {
    let row: Option<(Json<RouteMetadata>,)> = sqlx::query_as(
        "SELECT route_metadata FROM opportunities WHERE id = $1",
    )
    .bind(opportunity_id)
    .fetch_optional(pool)
    .await?;

    match row {
        None => Ok(None),
        Some((Json(rm),)) => {
            if rm.is_populated() {
                Ok(Some(rm))
            } else {
                // route_metadata is '{}' or has empty arrays — treat as absent.
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
