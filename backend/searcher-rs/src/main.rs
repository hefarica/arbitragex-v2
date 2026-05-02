//! searcher-rs — Sprint 2 entry point.
//!
//! Boot flow:
//!   1. Load config + init tracing + init metrics.
//!   2. Connect Redis (kill-switch + pub/sub).
//!   3. Open DB pool (best effort — if absent, scanner persists nothing but still publishes).
//!   4. Spawn one `scanner::run_chain(..)` per enabled chain in config.
//!   5. Serve `/health` + `/metrics` on SEARCHER_HEALTH_PORT.
//!   6. Await ctrl-c.

mod calldata;
mod chain_client;
mod dedup;
mod patterns;
mod persistence;
mod publisher;
mod scanner;
mod workers;

use shared_rs::{
    config::{require_env, AppConfig},
    health::{build_health_router, ServiceInfo},
    killswitch::KillSwitchClient,
    logging::init_tracing,
    metrics::init_metrics,
};
use sqlx::postgres::PgPoolOptions;
use std::{net::SocketAddr, sync::Arc, time::Duration};
use tracing::{error, info, warn};

const SERVICE_NAME: &str = "searcher-rs";
const SERVICE_VERSION: &str = env!("CARGO_PKG_VERSION");

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cfg = Arc::new(AppConfig::load()?);
    init_tracing(SERVICE_NAME, &cfg.observability.log_level)?;
    init_metrics();

    let port: u16 = std::env::var("SEARCHER_HEALTH_PORT")
        .ok().and_then(|v| v.parse().ok()).unwrap_or(9001);

    let redis_url = require_env("REDIS_URL")?;
    let killswitch = KillSwitchClient::connect(&redis_url, cfg.system.kill_switch_enabled_default).await
        .map_err(|e| anyhow::anyhow!("killswitch connect: {e}"))?;

    // Shared redis connection manager for scanners.
    let redis_client = redis::Client::open(redis_url.clone())?;
    let redis_conn = redis_client.get_connection_manager().await?;

    // DB pool — optional: if DATABASE_URL absent, run without persistence.
    let db_pool = match std::env::var("DATABASE_URL") {
        Ok(url) if !url.is_empty() => match PgPoolOptions::new().max_connections(4).connect(&url).await {
            Ok(p) => {
                info!(event = "db.connected", "postgres pool up");
                Some(p)
            }
            Err(e) => {
                warn!(event = "db.connect_failed", error = %e,
                      "continuing without DB persistence (opportunities only published to stream)");
                None
            }
        },
        _ => {
            warn!(event = "db.not_configured",
                  "DATABASE_URL not set; scanner will publish to stream but NOT persist");
            None
        }
    };

    let dedup = Arc::new(dedup::Dedup::new(
        50_000,
        Duration::from_secs(60),
    ));

    let enabled_chains = cfg.enabled_chains();
    info!(event = "service.boot", service = SERVICE_NAME, version = SERVICE_VERSION,
          enabled_chains = ?enabled_chains,
          "searcher-rs initializing S2 scanners");

    // Init and start Worker Orchestrator with advanced skills (Skill 082, Skill 100)
    let god_protocol_active = true;
    let kernel_bypass_enabled = true;
    let orchestrator = workers::WorkerOrchestrator::new(god_protocol_active, kernel_bypass_enabled);
    
    // Spawn orchestrator asynchronously
    tokio::spawn(async move {
        orchestrator.start_all().await;
    });

    // Spawn one scanner per chain.
    for chain_id in enabled_chains {
        let ks = killswitch.clone();
        let cfg_c = cfg.clone();
        let redis_c = redis_conn.clone();
        let db_c = db_pool.clone();
        let dedup_c = dedup.clone();
        tokio::spawn(async move {
            if let Err(e) = scanner::run_chain(chain_id, cfg_c, ks, redis_c, db_c, dedup_c).await {
                error!(event = "scanner.spawn_failed", chain_id, error = %e);
            }
        });
    }

    // HTTP server.
    let app = build_health_router(ServiceInfo::new(SERVICE_NAME, SERVICE_VERSION));
    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    let listener = tokio::net::TcpListener::bind(addr).await?;
    info!(event = "http.listen", addr = %addr, "searcher-rs health/metrics bound");

    axum::serve(listener, app)
        .with_graceful_shutdown(async { tokio::signal::ctrl_c().await.ok(); })
        .await?;

    Ok(())
}
