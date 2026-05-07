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
mod counters;
mod dedup;
mod patterns;
mod persistence;
mod publisher;
mod scanner;
mod workers;
mod amm_math;
mod reserves;

use shared_rs::{
    config::{require_env, AppConfig},
    health::{build_health_router, ServiceInfo},
    killswitch::KillSwitchClient,
    logging::init_tracing,
    metrics::init_metrics,
    rpc_failover::HttpRpcPool,
    trading_config::TradingConfigClient,
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

    // Trading-config client (Redis-backed, hot-reload <1s) — re-uses the manager
    // above so we don't double the open-fd count per pod. Each scanner per-chain
    // calls `state(chain_id)` per opportunity to honour operator updates without
    // service restart.
    let trading_config = TradingConfigClient::from_manager(redis_conn.clone());

    // Sprint 4 — opt-in v2 simulator dispatch flag.
    // Default = v1 stub in `prioritization-spine` (current production behaviour).
    // When ARBX_USE_SIMULATOR_V2=true the operator opts in to the new REVM-backed
    // simulator. Until simulator-v2 Tasks 4.2 (lazy_db) + 4.3 (revm_runner) land
    // end-to-end, this branch logs a warning and the candidate pipeline keeps
    // using v1 — no production candidate is ever scored against an unimplemented!()
    // path. The flag exists today so dashboards + alerting can verify the
    // configuration plumbing now and the cutover requires zero deploy when 4.3
    // ships.
    let use_simulator_v2 = std::env::var("ARBX_USE_SIMULATOR_V2")
        .map(|v| v.eq_ignore_ascii_case("true"))
        .unwrap_or(false);
    if use_simulator_v2 {
        warn!(
            event = "simulator.v2_requested_but_pending",
            "ARBX_USE_SIMULATOR_V2=true acknowledged; simulator-v2 Task 4.3 not integrated yet, falling through to v1"
        );
    } else {
        info!(event = "simulator.version", version = "v1", "using prioritization-spine stub simulator (default)");
    }

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

    // Init and start Worker Orchestrator. Sub-proyecto-1 wires the real
    // PoolSyncWorker against the primary chain's HTTP RPC + DB pool + Redis.
    // Stub workers (RouteDiscovery / Simulation) were removed.
    let god_protocol_active = true;
    let kernel_bypass_enabled = true;
    let orchestrator = workers::WorkerOrchestrator::new(god_protocol_active, kernel_bypass_enabled);

    // Pick the first enabled chain (defaults to 1 = Ethereum mainnet if config
    // is empty — same fallback used elsewhere). The orchestrator currently
    // syncs pools for ONE chain; multi-chain pool sync is Sprint 2 work.
    let primary_chain: u64 = enabled_chains.first().copied().unwrap_or(1);

    // Resolve the primary HTTP RPC URL using the same `HttpRpcPool::from_env`
    // pattern used by `relays-client` and `recon`. The pool reads
    // `RPC_HTTP_<chain_id>` (CSV `name=url,name=url`) and validates each
    // provider's chain_id at boot. If absent or all entries unhealthy we skip
    // PoolSyncWorker (no fake URL — RULE 00).
    let primary_rpc_http: Option<String> = match HttpRpcPool::from_env(primary_chain).await {
        Ok(Some(pool)) => match pool.pick() {
            Ok(entry) => {
                info!(
                    event = "worker_orchestrator.rpc_selected",
                    provider = %entry.name,
                    chain_id = primary_chain,
                    "primary HTTP RPC selected for pool sync"
                );
                Some(entry.url.clone())
            }
            Err(e) => {
                warn!(
                    event = "worker_orchestrator.rpc_no_healthy",
                    chain_id = primary_chain,
                    error = %e,
                    "no healthy HTTP RPC providers; PoolSyncWorker will not start"
                );
                None
            }
        },
        Ok(None) => {
            warn!(
                event = "worker_orchestrator.rpc_absent",
                chain_id = primary_chain,
                "RPC_HTTP_<chain_id> not set; PoolSyncWorker will not start"
            );
            None
        }
        Err(e) => {
            warn!(
                event = "worker_orchestrator.rpc_invalid",
                chain_id = primary_chain,
                error = %e,
                "RPC_HTTP_<chain_id> value did not parse; PoolSyncWorker will not start"
            );
            None
        }
    };

    // Spawn orchestrator asynchronously. When no HTTP RPC is available we
    // still start the orchestrator (RpcHealthWorker runs) but PoolSyncWorker
    // is gated by Some(db) AND Some(rpc) — done by passing None for db when
    // rpc is missing, so the existing internal gate skips cleanly.
    let db_for_orch = if primary_rpc_http.is_some() { db_pool.clone() } else { None };
    let redis_for_orch = redis_conn.clone();
    // Clone — the original `primary_rpc_http` is reused below to plumb the
    // V3 Provider into the per-chain scanner.
    let primary_rpc_for_orch = primary_rpc_http.clone().unwrap_or_default();
    tokio::spawn(async move {
        orchestrator
            .start_all(primary_chain, primary_rpc_for_orch, db_for_orch, redis_for_orch)
            .await;
    });

    // Price worker — fetches live USD token prices from Alchemy (primary) +
    // Coingecko (fallback) every PRICE_WORKER_INTERVAL_SECS (default 30s) and
    // populates Redis hash `arbx:token_prices:<chain>`. The spine evaluator's
    // CascadePriceOracle reads this snapshot on the hot path, displacing the
    // old operator-toil ConfigPriceOracle as PRIMARY price source.
    //
    // No Alchemy key in env → worker logs warning and skips the Alchemy tier
    // (Coingecko fallback only). No mapping for chain_id → worker exits early.
    // Worker writing failures cascade to ConfigPriceOracle gracefully — see
    // `workers/price_worker.rs` for the full failure-mode matrix.
    let price_period_secs: u64 = std::env::var("PRICE_WORKER_INTERVAL_SECS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(workers::price_worker::DEFAULT_PERIOD_SECS);
    let price_alchemy_key = workers::price_worker::alchemy_key_from_env(primary_chain);
    let price_redis = redis_conn.clone();
    let price_chain = primary_chain;
    tokio::spawn(async move {
        let cfg = workers::price_worker::PriceWorkerConfig::new(
            price_chain,
            price_period_secs,
            price_alchemy_key,
        );
        match workers::price_worker::PriceWorker::new(cfg) {
            Ok(worker) => worker.run(price_redis).await,
            Err(e) => warn!(event = "price_worker.boot_failed", chain_id = price_chain, error = %e),
        }
    });

    // Heartbeat worker — pipeline-state pulse every 60s. Without this, sparse
    // scanner events (sometimes a few per hour) make docker logs look idle even
    // when detection is healthy. The heartbeat emits Redis stream delta + PG
    // insertion rate + profitable-opportunity count, giving operators a steady
    // observability signal independent of mempool tx velocity. See
    // workers/heartbeat_worker.rs for the doctrine.
    let heartbeat_period_secs: u64 = std::env::var("SEARCHER_HEARTBEAT_PERIOD_SECS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(60);
    let heartbeat_redis = redis_conn.clone();
    let heartbeat_db = db_pool.clone();
    let heartbeat_chain = primary_chain;
    tokio::spawn(async move {
        let hb = workers::heartbeat_worker::HeartbeatWorker::new(heartbeat_period_secs, heartbeat_chain);
        hb.run(heartbeat_redis, heartbeat_db).await;
    });

    // Triangular worker — promotes the `triangular` strategy from `scaffold` to
    // `live` by emitting opportunities for hardcoded MVP cycles every block
    // (default 12s tick). Reads V2 reserves from the cache populated by
    // PoolSyncWorker; emits to `arbx:opps:detected` + persists via the standard
    // helpers. Spine evaluator runs downstream with the canonical risk gates.
    //
    // Worker is unconditional (no env gate) — promoting the badge requires the
    // emitter to actually run. Operator can still disable via
    // `trading_config.enabled_strategies` (the spine evaluator drops candidates
    // where `strategy_kind` is absent from that list, surfacing
    // `gate_strategy_disabled` in the heartbeat).
    let triangular_period_secs: u64 = std::env::var("TRIANGULAR_WORKER_INTERVAL_SECS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(workers::triangular_worker::DEFAULT_INTERVAL_SECS);
    let triangular_redis = redis_conn.clone();
    let triangular_db = db_pool.clone();
    let triangular_tc = trading_config.clone();
    let triangular_chain = primary_chain;
    tokio::spawn(async move {
        let tw = workers::triangular_worker::TriangularWorker::new(
            triangular_period_secs,
            triangular_chain,
        );
        tw.run(triangular_redis, triangular_db, triangular_tc).await;
    });

    // Spawn one scanner per chain. The primary chain (used by the orchestrator
    // for V2 pool sync) also gets the resolved HTTP RPC URL so the scanner can
    // build a Provider for V3 QuoterV2 batched calls. Other chains get None
    // and fall through to V2-only enrichment (Sub-proyecto 2 is mainnet-only;
    // multi-chain V3 lands in a future sub-project).
    for chain_id in enabled_chains {
        let ks = killswitch.clone();
        let cfg_c = cfg.clone();
        let redis_c = redis_conn.clone();
        let db_c = db_pool.clone();
        let dedup_c = dedup.clone();
        let tc_c = trading_config.clone();
        let rpc_http = if chain_id == primary_chain {
            primary_rpc_http.clone()
        } else {
            None
        };
        tokio::spawn(async move {
            if let Err(e) = scanner::run_chain(chain_id, cfg_c, ks, redis_c, db_c, dedup_c, tc_c, rpc_http).await {
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
