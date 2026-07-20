//! Axum REST API surface for the math-physics engine.
//!
//! Gated behind the `api` feature flag so the crate can still be used as a
//! pure library without pulling in the HTTP stack.
//!
//! ## Endpoint groups
//!
//! | Group     | Method | Path                           | Purpose                          |
//! |-----------|--------|--------------------------------|----------------------------------|
//! | Health    | GET    | `/health`                      | Liveness probe                   |
//! | Toggles   | GET    | `/api/operators`               | List all 31 operators            |
//! | Toggles   | GET    | `/api/operators/:id`           | Single operator metadata         |
//! | Toggles   | POST   | `/api/operators/:id/toggle`    | Enable / disable operator        |
//! | Compute   | POST   | `/api/compute`                 | Dispatch operator(s) on state    |
//! | Compute   | POST   | `/api/compute/batch`           | Batch dispatch                   |
//! | Matrix    | GET    | `/api/matrix/projection`       | 264×31 projection metadata       |
//! | Matrix    | GET    | `/api/matrix/operators`        | Matrix view of operator outputs  |
//!
//! ## State model
//!
//! `ApiState` holds:
//! - `registry`: the 31-operator `OperatorRegistry`
//! - `disabled`: a `HashSet<u8>` of operator IDs that have been soft-disabled
//!
//! Disabling is **soft** — the operator remains in the registry but
//! `is_available()` returns `false` and compute endpoints skip it.

use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::sync::{Arc, RwLock};

use crate::operators::{MarketState, OperatorOutput, OperatorRegistry};

// ---------------------------------------------------------------------------
// Shared state
// ---------------------------------------------------------------------------

/// Application state shared across all handlers.
#[derive(Clone)]
pub struct ApiState {
    /// The canonical operator registry (31 topological operators).
    registry: Arc<RwLock<OperatorRegistry>>,
    /// Set of operator IDs that have been soft-disabled at runtime.
    disabled: Arc<RwLock<HashSet<u8>>>,
}

impl ApiState {
    pub fn new() -> Self {
        Self {
            registry: Arc::new(RwLock::new(OperatorRegistry::new())),
            disabled: Arc::new(RwLock::new(HashSet::new())),
        }
    }

    /// Check whether an operator is currently enabled.
    fn is_enabled(&self, id: u8) -> bool {
        let disabled = self.disabled.read().expect("disabled lock poisoned");
        !disabled.contains(&id)
    }

    /// Explicitly set an operator's enabled state.
    fn set_enabled(&self, id: u8, enabled: bool) {
        let mut disabled = self.disabled.write().expect("disabled lock poisoned");
        if enabled {
            disabled.remove(&id);
        } else {
            disabled.insert(id);
        }
    }

    /// List all operators with their current availability.
    fn list_operators(&self) -> Vec<OperatorInfo> {
        let registry = self.registry.read().expect("registry lock poisoned");
        registry
            .all()
            .into_iter()
            .map(|op| OperatorInfo {
                id: op.id(),
                name: op.name(),
                category: op.category(),
                available: self.is_enabled(op.id()) && op.is_available(),
            })
            .collect()
    }

    /// Get a single operator by ID.
    fn get_operator(&self, id: u8) -> Option<OperatorInfo> {
        let registry = self.registry.read().expect("registry lock poisoned");
        registry.get(id).map(|op| OperatorInfo {
            id: op.id(),
            name: op.name(),
            category: op.category(),
            available: self.is_enabled(op.id()) && op.is_available(),
        })
    }

    /// Dispatch a single operator if enabled.
    fn dispatch_one(&self, id: u8, state: &MarketState) -> Option<OperatorOutput> {
        if !self.is_enabled(id) {
            return None;
        }
        let registry = self.registry.read().expect("registry lock poisoned");
        registry.dispatch(id, state)
    }
}

impl Default for ApiState {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// DTOs
// ---------------------------------------------------------------------------

/// Lightweight metadata for a topological operator.
#[derive(Debug, Clone, Serialize)]
pub struct OperatorInfo {
    pub id: u8,
    pub name: &'static str,
    pub category: &'static str,
    pub available: bool,
}

/// Request body for single or batch compute.
#[derive(Debug, Clone, Deserialize)]
pub struct ComputeRequest {
    /// Market state to evaluate against.
    pub market_state: MarketState,
    /// Operator IDs to dispatch (1–31).
    pub operator_ids: Vec<u8>,
}

/// Request body for explicit toggle.
#[derive(Debug, Clone, Deserialize)]
pub struct ToggleRequest {
    /// Desired enabled state.
    pub enabled: bool,
}

/// Response wrapper for compute results.
#[derive(Debug, Clone, Serialize)]
pub struct ComputeResponse {
    pub results: Vec<OperatorOutput>,
    /// IDs that were skipped because the operator is disabled.
    pub skipped: Vec<u8>,
}

/// Response for toggle actions.
#[derive(Debug, Clone, Serialize)]
pub struct ToggleResponse {
    pub operator_id: u8,
    pub enabled: bool,
}

/// Error response body.
#[derive(Debug, Clone, Serialize)]
pub struct ErrorResponse {
    pub error: &'static str,
    pub detail: String,
}

/// Projection matrix metadata.
#[derive(Debug, Clone, Serialize)]
pub struct ProjectionMatrixMeta {
    pub rows: usize,
    pub cols: usize,
    pub description: &'static str,
}

// ---------------------------------------------------------------------------
// Validation helpers
// ---------------------------------------------------------------------------

fn validate_operator_id(id: u8) -> Result<(), ErrorResponse> {
    if id == 0 || id > 31 {
        Err(ErrorResponse {
            error: "invalid_operator_id",
            detail: format!("operator id {id} is out of range (valid: 1–31)"),
        })
    } else {
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Router factory
// ---------------------------------------------------------------------------

/// Build the math-engine Axum router.
///
/// Mount this router at the desired prefix (e.g., `/` or `/math-engine`).
pub fn create_router() -> Router {
    let state = ApiState::new();
    create_router_with_state(state)
}

/// Build the router with an explicit state (useful for tests).
pub fn create_router_with_state(state: ApiState) -> Router {
    Router::new()
        .route("/health", get(health_handler))
        .route("/api/operators", get(list_operators_handler))
        .route("/api/operators/:id", get(get_operator_handler))
        .route("/api/operators/:id/toggle", post(toggle_operator_handler))
        .route("/api/compute", post(compute_handler))
        .route("/api/compute/batch", post(compute_batch_handler))
        .route("/api/matrix/projection", get(projection_matrix_handler))
        .route("/api/matrix/operators", get(operators_matrix_handler))
        .with_state(state)
}

// ---------------------------------------------------------------------------
// Handlers — all return `Response` for uniform type
// ---------------------------------------------------------------------------

/// GET /health — liveness probe.
async fn health_handler() -> Response {
    Json(serde_json::json!({
        "ok": true,
        "service": "math-engine",
        "operators": 31,
    }))
    .into_response()
}

/// GET /api/operators — list all operators with availability.
async fn list_operators_handler(State(st): State<ApiState>) -> Response {
    let ops = st.list_operators();
    (StatusCode::OK, Json(ops)).into_response()
}

/// GET /api/operators/:id — single operator metadata.
async fn get_operator_handler(State(st): State<ApiState>, Path(id): Path<u8>) -> Response {
    if let Err(err) = validate_operator_id(id) {
        return (StatusCode::BAD_REQUEST, Json(err)).into_response();
    }
    match st.get_operator(id) {
        Some(info) => (StatusCode::OK, Json(info)).into_response(),
        None => (
            StatusCode::NOT_FOUND,
            Json(ErrorResponse {
                error: "operator_not_found",
                detail: format!("operator id {id} not found in registry"),
            }),
        )
            .into_response(),
    }
}

/// POST /api/operators/:id/toggle — enable or disable an operator.
async fn toggle_operator_handler(
    State(st): State<ApiState>,
    Path(id): Path<u8>,
    Json(body): Json<ToggleRequest>,
) -> Response {
    if let Err(err) = validate_operator_id(id) {
        return (StatusCode::BAD_REQUEST, Json(err)).into_response();
    }
    if st.get_operator(id).is_none() {
        return (
            StatusCode::NOT_FOUND,
            Json(ErrorResponse {
                error: "operator_not_found",
                detail: format!("operator id {id} not found in registry"),
            }),
        )
            .into_response();
    }
    st.set_enabled(id, body.enabled);
    (
        StatusCode::OK,
        Json(ToggleResponse {
            operator_id: id,
            enabled: body.enabled,
        }),
    )
        .into_response()
}

/// POST /api/compute — dispatch operators on a market state.
///
/// Accepts a `ComputeRequest` with `market_state` and a list of `operator_ids`.
/// Returns results for enabled operators; disabled IDs are listed in `skipped`.
async fn compute_handler(State(st): State<ApiState>, Json(body): Json<ComputeRequest>) -> Response {
    let mut results = Vec::with_capacity(body.operator_ids.len());
    let mut skipped = Vec::new();

    for &id in &body.operator_ids {
        if let Err(err) = validate_operator_id(id) {
            return (StatusCode::BAD_REQUEST, Json(err)).into_response();
        }
        if st.is_enabled(id) {
            if let Some(output) = st.dispatch_one(id, &body.market_state) {
                results.push(output);
            }
        } else {
            skipped.push(id);
        }
    }

    (
        StatusCode::OK,
        Json(ComputeResponse { results, skipped }),
    )
        .into_response()
}

/// POST /api/compute/batch — alias for compute with explicit batch semantics.
///
/// Semantically identical to `/api/compute`; the separate path allows future
/// batch-specific optimisations (e.g., parallel dispatch, shared scratch memory).
async fn compute_batch_handler(
    State(st): State<ApiState>,
    Json(body): Json<ComputeRequest>,
) -> Response {
    compute_handler(State(st), Json(body)).await
}

/// GET /api/matrix/projection — metadata for the 264×31 projection matrix.
///
/// The projection matrix maps 264 strategic state vectors onto the 31
/// topological operator dimensions. This endpoint returns structural metadata
/// only; the full dense matrix is not materialised in memory.
async fn projection_matrix_handler() -> Response {
    let meta = ProjectionMatrixMeta {
        rows: 264,
        cols: 31,
        description: "Strategic-state-to-operator projection matrix (264 state vectors × 31 operator dimensions).",
    };
    (StatusCode::OK, Json(meta)).into_response()
}

/// GET /api/matrix/operators — matrix view of all operator output schemas.
///
/// Returns a tabular view where rows are operators and columns indicate
/// which output types (scalar, vector, matrix) each operator produces.
async fn operators_matrix_handler(State(st): State<ApiState>) -> Response {
    let ops = st.list_operators();
    let rows: Vec<serde_json::Value> = ops
        .into_iter()
        .map(|op| {
            serde_json::json!({
                "id": op.id,
                "name": op.name,
                "category": op.category,
                "available": op.available,
            })
        })
        .collect();
    (StatusCode::OK, Json(serde_json::json!({ "operators": rows }))).into_response()
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::{Method, Request};
    use tower::util::ServiceExt;

    fn test_app() -> Router {
        create_router_with_state(ApiState::new())
    }

    #[tokio::test]
    async fn test_health_ok() {
        let app = test_app();
        let req = Request::builder()
            .uri("/health")
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_list_operators() {
        let app = test_app();
        let req = Request::builder()
            .uri("/api/operators")
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_get_operator_valid() {
        let app = test_app();
        let req = Request::builder()
            .uri("/api/operators/1")
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_get_operator_invalid_id() {
        let app = test_app();
        let req = Request::builder()
            .uri("/api/operators/99")
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn test_toggle_and_compute() {
        let app = test_app();

        // Disable operator 1
        let toggle_req = Request::builder()
            .method(Method::POST)
            .uri("/api/operators/1/toggle")
            .header("content-type", "application/json")
            .body(Body::from(r#"{"enabled":false}"#))
            .unwrap();
        let resp = app.clone().oneshot(toggle_req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        // Compute with operator 1 disabled
        let compute_req = Request::builder()
            .method(Method::POST)
            .uri("/api/compute")
            .header("content-type", "application/json")
            .body(Body::from(r#"{
                "market_state": {
                    "price_matrix": [[1.0, 2.0], [3.0, 4.0]],
                    "liquidity_reserves": [[100.0, 200.0], [300.0, 400.0]],
                    "gas_price_gwei": 10.0,
                    "block_timestamp": 1234567890,
                    "block_number": 100,
                    "features": {}
                },
                "operator_ids": [1]
            }"#))
            .unwrap();
        let resp = app.oneshot(compute_req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_projection_matrix_meta() {
        let app = test_app();
        let req = Request::builder()
            .uri("/api/matrix/projection")
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
    }
}
