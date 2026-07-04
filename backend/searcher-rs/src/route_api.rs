//! G-SIM-1 PR-B2b Fase 3 (A2) — searcher-rs HTTP API for route metadata.
//!
//! Serves `GET /route/:opp_id` returning the complete `RouteMetadata` for a
//! given opportunity. This is the **A2 enrichment path**: api-server calls
//! this endpoint when `route_source = searcher_api` to obtain route topology
//! that searcher-rs has fresher than the persisted PG column (in-memory cache
//! of recently-detected routes, with PG fallback).
//!
//! ## Why a dedicated endpoint on searcher-rs
//!
//! The scanner discovers route topology in memory BEFORE persisting to PG.
//! For sub-second-fresh opportunities the PG column may not yet be visible
//! (replication lag, batch insert delay). searcher-rs can serve the in-memory
//! copy immediately, making A2 lower-latency than A1 for hot opportunities.
//!
//! ## Current implementation
//!
//! Phase 1 (this commit): PG-backed lookup — reads `opportunities.route_metadata`.
//! This establishes the HTTP contract and the api-server client. The in-memory
//! cache layer lands in Phase 2 once the scanner wires route capture at
//! detection time (dependent on Fase 2 scanner→emitter wiring).
//!
//! R8 fail-honest: returns 404 when the opportunity or route_metadata is
//! absent. Never fabricates topology.

use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    routing::get,
    Json, Router,
};
use serde::{Deserialize, Serialize};
use sqlx::postgres::PgPool;
use uuid::Uuid;

/// Shared state for the route API router.
#[derive(Clone)]
pub struct RouteApiState {
    /// PG pool for `opportunities.route_metadata` lookup (fallback when the
    /// in-memory cache misses — Phase 2 will add the cache layer).
    pub pool: PgPool,
}

/// Response body: the RouteMetadata JSONB contents.
///
/// Mirrors `shared_rs::candidates::RouteMetadata` but kept as a local
/// serde-transparent struct so searcher-rs doesn't need to pull the full
/// candidate contract into the HTTP layer. The api-server reconstructs the
/// full `OpportunityCandidate` by combining this with the `Opportunity` row.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RouteMetadataResponse {
    pub opportunity_id: Uuid,
    /// True when route_metadata is populated (non-empty topology).
    pub populated: bool,
    /// The raw JSONB contents. Empty object `{}` when unpopulated.
    pub route_metadata: serde_json::Value,
}

/// Build the route API axum router. Mount at `/route`.
pub fn route_router(state: RouteApiState) -> Router {
    Router::new()
        .route("/:opp_id", get(route_handler))
        .with_state(state)
}

/// `GET /route/:opp_id` — returns route metadata for the given opportunity.
///
/// - 200 + `{opportunity_id, populated, route_metadata}` on success
/// - 404 when the opportunity doesn't exist
/// - 500 on PG errors (fail-honest: no fabrication)
async fn route_handler(
    State(st): State<RouteApiState>,
    Path(opp_id): Path<Uuid>,
) -> impl IntoResponse {
    match fetch_route_metadata(&st.pool, opp_id).await {
        Ok(Some(resp)) => (StatusCode::OK, Json(serde_json::to_value(resp).unwrap_or_default())),
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({
                "error": "not_found",
                "opportunity_id": opp_id,
                "detail": "opportunity has no route_metadata or does not exist"
            })),
        ),
        Err(e) => {
            tracing::warn!(
                event = "route_api.pg_error",
                opportunity_id = %opp_id,
                error = %e,
                "PG lookup failed"
            );
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({
                    "error": "pg_error",
                    "opportunity_id": opp_id,
                    "detail": e.to_string()
                })),
            )
        }
    }
}

/// Fetch `route_metadata` JSONB for a single opportunity from PG.
///
/// Returns `Ok(None)` when the row doesn't exist or route_metadata is empty.
/// Returns `Ok(Some(response))` when populated.
async fn fetch_route_metadata(
    pool: &PgPool,
    opp_id: Uuid,
) -> Result<Option<RouteMetadataResponse>, sqlx::Error> {
    let row: Option<(serde_json::Value,)> = sqlx::query_as(
        "SELECT route_metadata FROM opportunities WHERE id = $1",
    )
    .bind(opp_id)
    .fetch_optional(pool)
    .await?;

    match row {
        None => Ok(None),
        Some((route_metadata,)) => {
            let populated = is_populated(&route_metadata);
            Ok(Some(RouteMetadataResponse {
                opportunity_id: opp_id,
                populated,
                route_metadata,
            }))
        }
    }
}

/// Heuristic: route_metadata is "populated" when it has at least one entry
/// in pool_addresses / token_addresses / dex_adapters. The JSONB default is
/// `'{}'` which is NOT populated.
fn is_populated(v: &serde_json::Value) -> bool {
    let obj = match v.as_object() {
        Some(o) => o,
        None => return false,
    };
    if obj.is_empty() {
        return false;
    }
    // Any of the three array fields being non-empty counts as populated.
    for key in &["pool_addresses", "token_addresses", "dex_adapters"] {
        if let Some(arr) = obj.get(*key).and_then(|v| v.as_array()) {
            if !arr.is_empty() {
                return true;
            }
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_is_populated_empty_object() {
        assert!(!is_populated(&json!({})));
    }

    #[test]
    fn test_is_populated_with_pools() {
        let v = json!({"pool_addresses": ["0xpool1"], "token_addresses": ["0xa", "0xb"]});
        assert!(is_populated(&v));
    }

    #[test]
    fn test_is_populated_empty_arrays() {
        let v = json!({"pool_addresses": [], "token_addresses": []});
        assert!(!is_populated(&v));
    }

    #[test]
    fn test_is_populated_non_object() {
        assert!(!is_populated(&json!("string")));
        assert!(!is_populated(&json!(42)));
        assert!(!is_populated(&json!(null)));
    }
}
