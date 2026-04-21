//! relays-client — private bundle submission.
//!
//! Sprint 1: `/execute` returns HTTP 501 AND checks kill-switch first.
//! Never submits anywhere; never returns a tx hash. That prevents any
//! misconfiguration from producing fake "executed" records.
//!
//! Sprint 5 implements: Flashbots signer, bundle builder, nonce manager,
//! multi-relay routing, retry/replacement strategy.

use axum::{
    extract::State,
    http::StatusCode,
    response::IntoResponse,
    routing::post,
    Json, Router,
};
use shared_rs::{
    config::{require_env, AppConfig},
    contracts::NotImplementedPayload,
    health::{build_health_router, ServiceInfo},
    killswitch::KillSwitchClient,
    logging::init_tracing,
    metrics::init_metrics,
};
use std::{net::SocketAddr, sync::Arc};
use tracing::{info, warn};

const SERVICE_NAME: &str = "relays-client";
const SERVICE_VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Clone)]
struct AppState {
    killswitch: KillSwitchClient,
}

async fn execute_handler(
    State(st): State<Arc<AppState>>,
    Json(_body): Json<serde_json::Value>,
) -> impl IntoResponse {
    if st.killswitch.is_enabled().await {
        warn!(event = "execute.blocked", reason = "kill_switch_enabled");
        let payload = NotImplementedPayload::new(
            vec!["kill_switch:disabled"],
            "S5",
            "kill-switch is ON; refusing to proceed".into(),
        );
        return (StatusCode::SERVICE_UNAVAILABLE, Json(payload));
    }
    shared_rs::metrics::EXECUTIONS_TOTAL
        .with_label_values(&["none", "not_implemented", "0"])
        .inc();
    let payload = NotImplementedPayload::new(
        vec!["FLASHBOTS_SIGNER_KEY", "FLASHBOTS_RELAY_URL"],
        "S5",
        "private execution not yet implemented; NEVER returning a fake tx_hash".into(),
    );
    (StatusCode::NOT_IMPLEMENTED, Json(payload))
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cfg = AppConfig::load()?;
    init_tracing(SERVICE_NAME, &cfg.observability.log_level)?;
    init_metrics();

    let redis_url = require_env("REDIS_URL")?;
    let killswitch = KillSwitchClient::connect(&redis_url, cfg.system.kill_switch_enabled_default).await
        .map_err(|e| anyhow::anyhow!("killswitch connect: {e}"))?;

    let state = Arc::new(AppState { killswitch });

    let port: u16 = std::env::var("RELAYS_PORT")
        .ok().and_then(|v| v.parse().ok()).unwrap_or(3005);

    let app = build_health_router(ServiceInfo::new(SERVICE_NAME, SERVICE_VERSION))
        .route("/execute", post(execute_handler))
        .with_state(state);

    info!(event = "service.boot", service = SERVICE_NAME, env = %cfg.system.env, port,
          "relays-client listening — /execute responds 501 unless kill-switch ON (S1)");
    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app)
        .with_graceful_shutdown(async { tokio::signal::ctrl_c().await.ok(); })
        .await?;
    Ok(())
}
