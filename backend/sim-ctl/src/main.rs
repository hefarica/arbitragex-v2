//! sim-ctl — simulation controller.
//!
//! Sprint 1: `/simulate` returns HTTP 501 with a canonical `NotImplementedPayload`.
//! Real Anvil fork + `debug_traceCall` + gas accuracy arrives in Sprint 4.
//!
//! Returning 501 instead of a fake pass/fail is DELIBERATE. Any upstream that
//! interprets a fake pass would flow into execution with no safety.

use axum::{
    extract::State,
    http::StatusCode,
    response::IntoResponse,
    routing::post,
    Json, Router,
};
use shared_rs::{
    config::AppConfig,
    contracts::NotImplementedPayload,
    health::{build_health_router, ServiceInfo},
    logging::init_tracing,
    metrics::init_metrics,
};
use std::{net::SocketAddr, sync::Arc};
use tracing::info;

const SERVICE_NAME: &str = "sim-ctl";
const SERVICE_VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Clone)]
struct AppState {
    env: String,
}

async fn simulate_handler(
    State(_st): State<Arc<AppState>>,
    Json(body): Json<serde_json::Value>,
) -> impl IntoResponse {
    shared_rs::metrics::SIMULATIONS_TOTAL
        .with_label_values(&["not_implemented", "false"])
        .inc();
    let payload = NotImplementedPayload::new(
        vec!["ANVIL_FORK_URL", "RPC_HTTP_<chain_id>"],
        "S4",
        format!(
            "sim-ctl received opportunity but fork-based simulation is not yet wired. Received keys: {}",
            body.as_object()
                .map(|m| m.keys().cloned().collect::<Vec<_>>().join(","))
                .unwrap_or_default()
        ),
    );
    (StatusCode::NOT_IMPLEMENTED, Json(payload))
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cfg = AppConfig::load()?;
    init_tracing(SERVICE_NAME, &cfg.observability.log_level)?;
    init_metrics();

    let port: u16 = std::env::var("SIM_PORT")
        .ok().and_then(|v| v.parse().ok()).unwrap_or(3003);

    let state = Arc::new(AppState { env: cfg.system.env.clone() });

    let app = build_health_router(ServiceInfo::new(SERVICE_NAME, SERVICE_VERSION))
        .route("/simulate", post(simulate_handler))
        .with_state(state);

    info!(event = "service.boot", service = SERVICE_NAME, env = %cfg.system.env, port,
          "sim-ctl listening — /simulate responds 501 (S1 design)");
    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app)
        .with_graceful_shutdown(async { tokio::signal::ctrl_c().await.ok(); })
        .await?;
    Ok(())
}
