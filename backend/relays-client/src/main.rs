//! relays-client main.
//!
//! Boots:
//!   - Axum HTTP /health /metrics /execute (hot-path invocation).
//!   - Redis Streams consumer on arbx:opps:simulated (if signer+RPC present).
//!
//! Without FLASHBOTS_SIGNER_KEY or RPC_HTTP_1: /execute returns 501,
//! consumer doesn't spawn. paper_mode=true by default; only builds+signs
//! (no submit).

mod bundle_builder;
mod consumer;
mod nonce_manager;
mod persistence;
mod relay_flashbots;
mod signer;
mod submit_engine;
mod tracker;

use axum::{
    extract::State,
    http::StatusCode,
    response::IntoResponse,
    routing::post,
    Json, Router,
};
use ethers::prelude::*;
use shared_rs::{
    config::{AppConfig, require_env},
    contracts::{NotImplementedPayload, Opportunity},
    health::{build_health_router, ServiceInfo},
    killswitch::KillSwitchClient,
    logging::init_tracing,
    metrics::init_metrics,
};
use sqlx::postgres::PgPoolOptions;
use std::{net::SocketAddr, sync::Arc, time::Duration};
use tracing::{info, warn};

use crate::nonce_manager::NonceManager;
use crate::relay_flashbots::FlashbotsClient;
use crate::signer::Signer;
use crate::submit_engine::SubmitEngine;

const SERVICE: &str = "relays-client";
const VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Clone)]
struct AppState {
    engine: Arc<SubmitEngine>,
    has_signer: bool,
    env: String,
}

async fn execute_handler(
    State(st): State<Arc<AppState>>,
    Json(body): Json<serde_json::Value>,
) -> impl IntoResponse {
    if !st.has_signer {
        let payload = NotImplementedPayload::new(
            vec!["FLASHBOTS_SIGNER_KEY", "RPC_HTTP_1"],
            "S5",
            format!("relays-client up but signer not configured (env={})", st.env),
        );
        return (StatusCode::NOT_IMPLEMENTED, Json(serde_json::to_value(payload).unwrap()));
    }
    let opp: Opportunity = match serde_json::from_value(body) {
        Ok(o) => o,
        Err(e) => return (StatusCode::BAD_REQUEST,
                          Json(serde_json::json!({"error":"invalid_body","detail":e.to_string()}))),
    };
    let result = st.engine.execute(&opp).await;
    (StatusCode::OK, Json(serde_json::to_value(result).unwrap()))
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cfg = Arc::new(AppConfig::load()?);
    init_tracing(SERVICE, &cfg.observability.log_level)?;
    init_metrics();

    let redis_url = require_env("REDIS_URL")?;
    let killswitch = KillSwitchClient::connect(&redis_url, cfg.system.kill_switch_enabled_default).await
        .map_err(|e| anyhow::anyhow!("killswitch: {e}"))?;

    // Try to load signer.
    let chain_id = cfg.chains.iter().find(|c| c.enabled).map(|c| c.chain_id).unwrap_or(1);
    let signer = match Signer::from_env(chain_id)? {
        Some(s) => {
            info!(event = "signer.loaded", address = %s.address, chain_id = s.chain_id);
            Some(Arc::new(s))
        }
        None => {
            warn!(event = "signer.missing", "FLASHBOTS_SIGNER_KEY empty/unset — /execute stays 501, consumer idle");
            None
        }
    };

    // Try to connect provider (RPC HTTP).
    let provider: Option<Arc<Provider<Http>>> = match std::env::var("RPC_HTTP_1") {
        Ok(url) if !url.is_empty() => {
            match Provider::<Http>::try_from(url.as_str()) {
                Ok(p) => Some(Arc::new(p)),
                Err(e) => {
                    warn!(event = "provider.invalid_rpc_url", error = %e);
                    None
                }
            }
        }
        _ => None,
    };

    let nonce = provider.clone().map(|p| Arc::new(NonceManager::new(p)));
    let flashbots = {
        // No-hardcode doctrine: the Flashbots relay URL must come from one of
        // the allowed sources — env var, config file, or (once Phase 0.5 R10
        // lands) the DB `relays` catalog. We never fall back to the canonical
        // Flashbots URL in code; silently calling a third-party relay without
        // operator sign-off is a policy violation.
        let endpoint = std::env::var("FLASHBOTS_RELAY_URL")
            .ok()
            .filter(|s| !s.is_empty())
            .or_else(|| cfg.relays.iter()
                .find(|r| r.name == "flashbots" && r.enabled)
                .and_then(|r| r.endpoint.clone())
                .filter(|s| !s.is_empty()));
        match endpoint {
            Some(url) => {
                info!(event = "flashbots.configured", url = %url, "flashbots relay enabled");
                Some(Arc::new(FlashbotsClient::new(
                    url,
                    Duration::from_millis(cfg.execution.flashbots_submit_timeout_ms),
                )))
            }
            None => {
                warn!(
                    event = "flashbots.disabled",
                    reason = "no_endpoint",
                    "flashbots relay disabled: no endpoint in env (FLASHBOTS_RELAY_URL) or config. \
                     Set it in onboarding step 4."
                );
                None
            }
        }
    };

    let engine = Arc::new(SubmitEngine {
        signer: signer.clone(),
        provider: provider.clone(),
        nonce: nonce.clone(),
        flashbots: flashbots.clone(),
        kill_switch: killswitch.clone(),
        cfg: cfg.clone(),
    });

    let state = Arc::new(AppState {
        engine: engine.clone(),
        has_signer: signer.is_some() && provider.is_some(),
        env: cfg.system.env.clone(),
    });

    let port: u16 = std::env::var("RELAYS_PORT").ok().and_then(|v| v.parse().ok()).unwrap_or(3005);
    let exec_router = Router::new()
        .route("/execute", post(execute_handler))
        .with_state(state);
    let app = build_health_router(ServiceInfo::new(SERVICE, VERSION))
        .merge(exec_router);

    info!(
        event = "service.boot",
        service = SERVICE, env = %cfg.system.env, port,
        has_signer = signer.is_some(),
        paper_mode = cfg.execution.paper_mode,
        max_value_eth = cfg.execution.max_value_eth,
        "relays-client listening"
    );

    // Consumer spawned only when both signer and provider are available AND
    // DATABASE_URL is set.
    if signer.is_some() && provider.is_some() {
        if let Ok(db_url) = std::env::var("DATABASE_URL") {
            if !db_url.is_empty() {
                let pool = PgPoolOptions::new().max_connections(4).connect(&db_url).await?;
                let redis_client = redis::Client::open(redis_url.clone())?;
                let redis_conn = redis_client.get_connection_manager().await?;
                let consumer = consumer::Consumer {
                    redis: redis_conn,
                    pool,
                    engine: SubmitEngine {
                        signer, provider, nonce, flashbots,
                        kill_switch: killswitch.clone(),
                        cfg: cfg.clone(),
                    },
                    consumer_name: std::env::var("HOSTNAME").unwrap_or_else(|_| "relay-1".into()),
                };
                tokio::spawn(async move {
                    if let Err(e) = consumer.run().await {
                        tracing::error!(event = "relays_consumer.fatal", error = %e);
                    }
                });
                info!(event = "relays_consumer.spawned");
            }
        }
    }

    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app)
        .with_graceful_shutdown(async { tokio::signal::ctrl_c().await.ok(); })
        .await?;
    Ok(())
}
