//! recon — PnL reconciliation + learning loop.
//!
//! S6 additions:
//!   - Consumer on `arbx:opps:executed` → pnl_engine.compute → persist recon_reports.
//!   - Aggregator task (periodic) rolls executions into strategy_scores / relay_scores.
//!   - Anomaly detector flags high revert rate + (optional) trips kill-switch.
//!
//! S1 HTTP endpoints (read-only) are preserved:
//!   - GET /pnl/:opportunity_id — 404 if no execution.
//!   - GET /pnl/summary — real aggregation.

mod aggregator;
mod anomaly;
mod consumer;
mod persistence;
mod pnl_engine;
mod variance;

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
    killswitch::KillSwitchClient,
    logging::init_tracing,
    metrics::init_metrics,
    rpc_failover::{AlloyHttpProvider, HttpRpcPool},
};
use std::{net::SocketAddr, sync::Arc};
use tracing::{info, warn};
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

async fn pnl_one(State(st): State<Arc<AppState>>, Path(id): Path<Uuid>) -> impl IntoResponse {
    use sqlx::Row;
    let row = sqlx::query(
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
    )
    .bind(id)
    .fetch_optional(&st.db)
    .await;

    match row {
        Ok(Some(r)) => {
            let expected: f64 = r.try_get("expected_profit_usd").unwrap_or(0.0);
            let actual: f64 = r.try_get("actual_profit_usd").unwrap_or(0.0);
            let created_at: Option<DateTime<Utc>> = r.try_get("created_at").ok();
            let variance_usd = actual - expected;
            let variance_pct = if expected.abs() > 1e-9 {
                (variance_usd / expected) * 100.0
            } else {
                0.0
            };
            (
                StatusCode::OK,
                Json(
                    serde_json::to_value(PnlRow {
                        opportunity_id: r.try_get::<Uuid, _>("opportunity_id").unwrap_or(id),
                        expected_profit_usd: expected,
                        actual_profit_usd: actual,
                        variance_usd,
                        variance_pct,
                        created_at: created_at.unwrap_or_else(Utc::now),
                    })
                    .unwrap(),
                ),
            )
        }
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({
                "error":"not_found","opportunity_id":id
            })),
        ),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({
                "error":"db_error","detail":e.to_string()
            })),
        ),
    }
}

async fn pnl_summary(
    State(st): State<Arc<AppState>>,
    Query(q): Query<SummaryQuery>,
) -> impl IntoResponse {
    use sqlx::Row;
    let row = sqlx::query(
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
    )
    .bind(q.since)
    .fetch_one(&st.db).await;

    match row {
        Ok(r) => (
            StatusCode::OK,
            Json(
                serde_json::to_value(PnlSummary {
                    sample_count: r.try_get::<i64, _>("sample_count").unwrap_or(0),
                    total_expected_usd: r.try_get::<f64, _>("total_expected_usd").unwrap_or(0.0),
                    total_actual_usd: r.try_get::<f64, _>("total_actual_usd").unwrap_or(0.0),
                    avg_variance_pct: r.try_get::<f64, _>("avg_variance_pct").unwrap_or(0.0),
                    since: q.since,
                })
                .unwrap(),
            ),
        ),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({
                "error":"db_error","detail":e.to_string()
            })),
        ),
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cfg = AppConfig::load()?;
    init_tracing(SERVICE_NAME, &cfg.observability.log_level)?;
    init_metrics();

    let db_url = require_env("DATABASE_URL")?;
    // OMEGA-8/M3 P1-2: timeouts applied (acquire/idle/max_lifetime/min).
    let db = shared_rs::db_pool::options_with_timeouts(&shared_rs::db_pool::PoolConfig::from_env(5))
        .connect(&db_url)
        .await?;
    let state = Arc::new(AppState { db });

    let port: u16 = std::env::var("RECON_PORT")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(3004);

    let pnl_router = Router::new()
        .route("/pnl/:id", get(pnl_one))
        .route("/pnl/summary", get(pnl_summary))
        .with_state(state);
    let app =
        build_health_router(ServiceInfo::new(SERVICE_NAME, SERVICE_VERSION)).merge(pnl_router);

    info!(event = "service.boot", service = SERVICE_NAME, env = %cfg.system.env, port,
          "recon listening — S6: consumer + aggregator active when infra ready");

    // S6: spawn consumer if RPC + REDIS + recon cfg present.
    if let Some(recon_cfg) = cfg.recon.clone() {
        let chain_id = cfg
            .chains
            .iter()
            .find(|c| c.enabled)
            .map(|c| c.chain_id)
            .unwrap_or(1);
        let redis_url = std::env::var("REDIS_URL").unwrap_or_default();
        if redis_url.is_empty() {
            warn!(
                event = "recon.consumer_not_spawned",
                reason = "redis_url_missing"
            );
        } else {
            // RPC failover discipline (G-RPC-1): build a multi-vendor HTTP
            // pool from env. recon's consumer fetches receipts on chain — we
            // pick the primary at boot and let the pool's health loop publish
            // metrics + drift detection + per-provider circuit state.
            let provider_arc: Option<Arc<AlloyHttpProvider>> =
                match HttpRpcPool::from_env(chain_id).await {
                    Ok(Some(pool)) => {
                        let pool = Arc::new(pool);
                        let _health_task = pool.spawn_health_loop();
                        match pool.pick() {
                            Ok(e) => {
                                info!(
                                    event = "rpc_pool.primary_selected",
                                    provider = %e.name, chain_id,
                                    "recon primary RPC selected"
                                );
                                Some(e.provider.clone())
                            }
                            Err(e) => {
                                warn!(event = "rpc_pool.no_healthy", error = %e);
                                None
                            }
                        }
                    }
                    Ok(None) => {
                        warn!(
                        event = "rpc_pool.absent",
                        chain_id,
                        "RPC_HTTP_{chain_id} not set; recon stays read-only (no receipt fetching)"
                    );
                        None
                    }
                    Err(e) => {
                        warn!(event = "rpc_pool.invalid", chain_id, error = %e);
                        None
                    }
                };

            if let Some(provider) = provider_arc {
                // OMEGA-8/M3 P1-2: timeouts applied to all per-task pools.
                let db_for_consumer = shared_rs::db_pool::options_with_timeouts(
                    &shared_rs::db_pool::PoolConfig::from_env(4),
                )
                .connect(&db_url)
                .await?;
                let db_for_aggregator = shared_rs::db_pool::options_with_timeouts(
                    &shared_rs::db_pool::PoolConfig::from_env(2),
                )
                .connect(&db_url)
                .await?;
                let redis_client = redis::Client::open(redis_url.clone())?;
                let redis_conn = redis_client.get_connection_manager().await?;
                let killswitch =
                    KillSwitchClient::connect(&redis_url, cfg.system.kill_switch_enabled_default)
                        .await
                        .map_err(|e| anyhow::anyhow!("killswitch: {e}"))?;

                let consumer = consumer::Consumer {
                    redis: redis_conn,
                    pool: db_for_consumer,
                    provider,
                    cfg: recon_cfg.clone(),
                    consumer_name: std::env::var("HOSTNAME").unwrap_or_else(|_| "recon-1".into()),
                };
                tokio::spawn(async move {
                    if let Err(e) = consumer.run().await {
                        tracing::error!(event = "recon_consumer.fatal", error = %e);
                    }
                });
                info!(event = "recon_consumer.spawned");

                // Open a second connection manager for the aggregator's pub/sub
                // publishing.  The consumer already owns redis_conn; we cannot
                // share a connection manager between pub/sub and regular commands
                // on the same handle (the pub/sub mode takes over the socket).
                let redis_agg: Option<redis::aio::ConnectionManager> =
                    match redis::Client::open(redis_url.clone()) {
                        Ok(c) => match c.get_connection_manager().await {
                            Ok(m) => Some(m),
                            Err(e) => {
                                warn!(event = "aggregator.redis_conn_failed", error = %e,
                                      "aggregator pub/sub disabled — SQL fallback only");
                                None
                            }
                        },
                        Err(e) => {
                            warn!(event = "aggregator.redis_client_failed", error = %e,
                                  "aggregator pub/sub disabled — SQL fallback only");
                            None
                        }
                    };

                let cfg_agg = recon_cfg.clone();
                tokio::spawn(async move {
                    aggregator::run_periodic(db_for_aggregator, cfg_agg, killswitch, redis_agg)
                        .await;
                });
                info!(event = "aggregator.spawned");
            } else {
                warn!(
                    event = "recon.consumer_not_spawned",
                    "RPC pool empty/unhealthy or absent; running read-only"
                );
            }
        }
    } else {
        warn!(
            event = "recon.cfg_absent",
            "[recon] config section missing; running read-only without consumer+aggregator"
        );
    }

    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app)
        .with_graceful_shutdown(async {
            tokio::signal::ctrl_c().await.ok();
        })
        .await?;
    Ok(())
}
