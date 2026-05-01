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
mod relay_catalog;
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
    rpc_failover::HttpRpcPool,
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

    // RPC failover discipline (G-RPC-1): build a multi-vendor HTTP pool from
    // env `RPC_HTTP_<chain_id>` (CSV `name=url,name=url`). The pool validates
    // each provider's chain_id at boot, exposes Prometheus metrics, drift
    // detection and per-provider circuit breakers via the spawned health loop.
    //
    // For backwards compat with single-vendor onboarding the pool accepts a
    // bare URL (named "primary"). The selected primary feeds the engine; a
    // later pass (Sprint 5, G-PEC-5) will upgrade per-call read sites to
    // `pool.with_retry(...)` for sub-second failover on individual ops.
    let provider: Option<Arc<Provider<Http>>> = match HttpRpcPool::from_env(chain_id).await {
        Ok(Some(pool)) => {
            let pool = Arc::new(pool);
            let _health_task = pool.spawn_health_loop();
            match pool.pick() {
                Ok(entry) => {
                    info!(
                        event = "rpc_pool.primary_selected",
                        provider = %entry.name,
                        chain_id,
                        "relays-client primary RPC selected"
                    );
                    Some(entry.provider.clone())
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
                "RPC_HTTP_{chain_id} not set; relays-client stays in 501 mode"
            );
            None
        }
        Err(e) => {
            warn!(
                event = "rpc_pool.invalid",
                chain_id,
                error = %e,
                "RPC_HTTP_{chain_id} value did not parse; relays-client stays in 501 mode"
            );
            None
        }
    };

    let nonce = provider.clone().map(|p| Arc::new(NonceManager::new(p)));

    // DB pool — required for reading the operator-owned relay catalog (migration
    // 013). If DB is down at boot we still start the service and expose /health
    // so the on-call can see the failure; the catalog is simply empty until DB
    // is back and the service is restarted. No-hardcode doctrine: we never
    // silently fall back to a baked-in relay URL.
    let db_pool_opt: Option<sqlx::postgres::PgPool> = match std::env::var("DATABASE_URL") {
        Ok(url) if !url.is_empty() => {
            match PgPoolOptions::new().max_connections(4).connect(&url).await {
                Ok(pool) => {
                    info!(event = "db.connected", "postgres pool up");
                    Some(pool)
                }
                Err(e) => {
                    warn!(
                        event = "db.connect_failed", error = %e,
                        "continuing without DB — relay catalog will be empty until restart"
                    );
                    None
                }
            }
        }
        _ => {
            warn!(
                event = "db.not_configured",
                "DATABASE_URL not set; relay catalog cannot be loaded, consumer will not spawn"
            );
            None
        }
    };

    // Resolve flashbots endpoint under the no-hardcode doctrine. Order:
    //   1. DB `relays` table (operator-owned catalog, migration 013).
    //   2. FLASHBOTS_RELAY_URL env override (break-glass during onboarding).
    //   3. Nothing → flashbots disabled + warn.
    //
    // cfg.relays (configs/app.toml) is NOT consulted anymore — the TOML block
    // was reduced to a seed-only document in commit 0210d27 and the DB is
    // authoritative from this point on.
    let flashbots = {
        let mut endpoint: Option<String> = None;
        let mut source: &'static str = "unset";

        if let Some(pool) = db_pool_opt.as_ref() {
            match relay_catalog::load_enabled(pool, chain_id as i32).await {
                Ok(catalog) => {
                    if let Some(fb) = relay_catalog::find_flashbots(&catalog, chain_id as i32) {
                        endpoint = Some(fb.endpoint.clone());
                        source = "db";
                    }
                    relay_catalog::warn_if_empty(&catalog, chain_id as i32);
                }
                Err(e) => {
                    warn!(
                        event = "relay_catalog.query_failed",
                        error = %e,
                        "could not load relay catalog from DB — will check env override only"
                    );
                }
            }
        }

        if endpoint.is_none() {
            if let Ok(url) = std::env::var("FLASHBOTS_RELAY_URL") {
                if !url.is_empty() {
                    endpoint = Some(url);
                    source = "env";
                }
            }
        }

        match endpoint {
            Some(url) => {
                info!(event = "flashbots.configured", url = %url, source, "flashbots relay enabled");
                Some(Arc::new(FlashbotsClient::new(
                    url,
                    Duration::from_millis(cfg.execution.flashbots_submit_timeout_ms),
                )))
            }
            None => {
                warn!(
                    event = "flashbots.disabled",
                    reason = "no_endpoint",
                    "flashbots relay disabled: no row in relays table for chain {chain_id} and \
                     no FLASHBOTS_RELAY_URL env override. Populate via POST /admin/relays \
                     (onboarding step 4).",
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

    // Consumer spawns only when signer + provider + DB pool are all present.
    // We reuse the pool opened above for the relay catalog lookup, so we don't
    // double up connections.
    if signer.is_some() && provider.is_some() && db_pool_opt.is_some() {
        let pool = db_pool_opt.clone().unwrap();
        let redis_client = redis::Client::open(redis_url.clone())?;
        let redis_conn = redis_client.get_connection_manager().await?;
        let consumer = consumer::Consumer {
            redis: redis_conn,
            pool,
            engine: SubmitEngine {
                signer: signer.clone(),
                provider: provider.clone(),
                nonce,
                flashbots,
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
    } else {
        info!(
            event = "relays_consumer.skipped",
            has_signer = signer.is_some(),
            has_provider = provider.is_some(),
            has_db = db_pool_opt.is_some(),
            "consumer not spawned — prerequisites missing (service stays up, /execute 501)"
        );
    }

    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app)
        .with_graceful_shutdown(async { tokio::signal::ctrl_c().await.ok(); })
        .await?;
    Ok(())
}
