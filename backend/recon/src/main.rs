//! recon — PnL reconciliation.
//!
//! Sprint 1 behavior (no fabrication):
//!   - `GET /pnl/:opportunity_id` — reads executions; returns 404 if absent.
//!   - `GET /pnl/summary` — aggregates over real rows; if zero rows, returns explicit zeros
//!     with `sample_count: 0` so callers can tell the difference between "no data" and "zero PnL".
//!
//! S6 extends this with: variance analysis, adaptive scoring writers, incident correlators.

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    routing::get,
    Json, Router,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use shared_rs::{
    config::{require_env, AppConfig},
    health::{build_health_router, ServiceInfo},
    logging::init_tracing,
    metrics::init_metrics,
};
use sqlx::postgres::PgPoolOptions;
use std::{net::SocketAddr, sync::Arc};
use tracing::info;
use uuid::Uuid;

const SERVICE_NAME: &str = "recon";
const SERVICE_VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Clone)]
struct AppState {
    db: sqlx::PgPool,
}

#[derive(Serialize)]
struct PnlRow {
    opportunity_id: Uuid,
    expected_profit_usd: f64,
    actual_profit_usd: f64,
    variance_usd: f64,
    variance_pct: f64,
    created_at: DateTime<Utc>,
}

#[derive(Serialize)]
struct PnlSummary {
    sample_count: i64,
    total_expected_usd: f64,
    total_actual_usd: f64,
    avg_variance_pct: f64,
    since: DateTime<Utc>,
}

#[derive(Deserialize)]
struct SummaryQuery {
    #[serde(default = "default_since")]
    since: DateTime<Utc>,
}
fn default_since() -> DateTime<Utc> {
    Utc::now() - chrono::Duration::hours(24)
}

async fn pnl_one(
    State(st): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
) -> impl IntoResponse {
    let row = sqlx::query!(
        r#"
        SELECT o.id AS opportunity_id,
               COALESCE(o.expected_profit_usd, 0)::FLOAT8 AS expected_profit_usd,
               COALESCE(e.actual_profit_usd, 0)::FLOAT8 AS actual_profit_usd,
               e.submitted_at AS created_at
        FROM opportunities o
        LEFT JOIN executions e ON e.opportunity_id = o.id
        WHERE o.id = $1
        ORDER BY e.submitted_at DESC NULLS LAST
        LIMIT 1
        "#,
        id
    ).fetch_optional(&st.db).await;

    match row {
        Ok(Some(r)) => {
            let expected = r.expected_profit_usd.unwrap_or(0.0);
            let actual   = r.actual_profit_usd.unwrap_or(0.0);
            let variance_usd = actual - expected;
            let variance_pct = if expected.abs() > 1e-9 { (variance_usd / expected) * 100.0 } else { 0.0 };
            (StatusCode::OK, Json(serde_json::to_value(PnlRow {
                opportunity_id: r.opportunity_id,
                expected_profit_usd: expected,
                actual_profit_usd: actual,
                variance_usd,
                variance_pct,
                created_at: r.created_at.unwrap_or_else(Utc::now),
            }).unwrap()))
        }
        Ok(None) => (StatusCode::NOT_FOUND, Json(serde_json::json!({
            "error":"not_found","opportunity_id":id
        }))),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({
            "error":"db_error","detail":e.to_string()
        }))),
    }
}

async fn pnl_summary(
    State(st): State<Arc<AppState>>,
    Query(q): Query<SummaryQuery>,
) -> impl IntoResponse {
    let row = sqlx::query!(
        r#"
        SELECT COUNT(*)::INT8 AS sample_count,
               COALESCE(SUM(o.expected_profit_usd), 0)::FLOAT8 AS total_expected_usd,
               COALESCE(SUM(e.actual_profit_usd), 0)::FLOAT8 AS total_actual_usd,
               COALESCE(AVG(
                 CASE WHEN o.expected_profit_usd IS NOT NULL AND ABS(o.expected_profit_usd) > 0
                      THEN ((e.actual_profit_usd - o.expected_profit_usd) / o.expected_profit_usd) * 100.0
                      ELSE NULL END
               ), 0)::FLOAT8 AS avg_variance_pct
        FROM executions e
        JOIN opportunities o ON o.id = e.opportunity_id
        WHERE e.submitted_at >= $1 AND e.status = 'included'
        "#,
        q.since
    ).fetch_one(&st.db).await;

    match row {
        Ok(r) => (StatusCode::OK, Json(serde_json::to_value(PnlSummary {
            sample_count: r.sample_count.unwrap_or(0),
            total_expected_usd: r.total_expected_usd.unwrap_or(0.0),
            total_actual_usd: r.total_actual_usd.unwrap_or(0.0),
            avg_variance_pct: r.avg_variance_pct.unwrap_or(0.0),
            since: q.since,
        }).unwrap())),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({
            "error":"db_error","detail":e.to_string()
        }))),
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cfg = AppConfig::load()?;
    init_tracing(SERVICE_NAME, &cfg.observability.log_level)?;
    init_metrics();

    let db_url = require_env("DATABASE_URL")?;
    let db = PgPoolOptions::new().max_connections(5).connect(&db_url).await?;
    let state = Arc::new(AppState { db });

    let port: u16 = std::env::var("RECON_PORT")
        .ok().and_then(|v| v.parse().ok()).unwrap_or(3004);

    let app = build_health_router(ServiceInfo::new(SERVICE_NAME, SERVICE_VERSION))
        .route("/pnl/:id", get(pnl_one))
        .route("/pnl/summary", get(pnl_summary))
        .with_state(state);

    info!(event = "service.boot", service = SERVICE_NAME, env = %cfg.system.env, port,
          "recon listening — reads DB; 404 when no row (S1)");
    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app)
        .with_graceful_shutdown(async { tokio::signal::ctrl_c().await.ok(); })
        .await?;
    Ok(())
}
