// M11 (audit 2026-05-10): gate PRs that introduce panicking paths in entry point.
// Promoted from warn→deny so CI fails on new unwrap/expect in this binary.
// Per-file #[allow(...)] is used below in files where the pattern is
// demonstrably safe (mutex poison, infallible slice casts, test modules).
#![deny(clippy::unwrap_used, clippy::expect_used)]

//! searcher-rs — Sprint 2 entry point.
//!
//! Boot flow:
//!   1. Load config + init tracing + init metrics.
//!   2. Connect Redis (kill-switch + pub/sub).
//!   3. Open DB pool (best effort — if absent, scanner persists nothing but still publishes).
//!   4. Spawn one `scanner::run_chain(..)` per enabled chain in config.
//!   5. Serve `/health` + `/metrics` on SEARCHER_HEALTH_PORT.
//!   6. Await ctrl-c.

mod amm_math;
mod calldata;
mod chain_client;
mod counters;
mod dedup;
mod patterns;
mod persistence;
mod publisher;
mod reserves;
mod scanner;
// Phase 1-3: new orchestrator modules declared here so the binary links them.
// Items are not yet wired into scanner.rs (Phase 12); allow dead_code until
// the integration lands.
#[allow(dead_code)]
mod impact_index;
#[allow(dead_code)]
mod opportunity_emitter;
#[allow(dead_code)]
mod route_decoder;
#[allow(dead_code)]
mod route_intent;
#[allow(dead_code)]
mod strategy_label;
// Phase 7-8: orchestrator skeleton + engines (dex_engine).
// Not yet integrated into scanner.rs (Phase 14).
#[allow(dead_code)]
mod engines;
#[allow(dead_code)]
mod orchestrator;
mod workers;
// Phase 12: StateProjector — virtual post-tx pool state projection.
// Not yet wired into scanner.rs (Phase 14); allow dead_code until integration.
#[allow(dead_code)]
mod state_projector;
// Phase 13: SizeOptimizer — optimal amount_in sizing.
// Not yet wired into scanner.rs (Phase 14); allow dead_code until integration.
#[allow(dead_code)]
mod size_optimizer;

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
use std::{collections::HashMap, net::SocketAddr, sync::Arc, time::Duration};
use tracing::{error, info, warn};

const SERVICE_NAME: &str = "searcher-rs";
const SERVICE_VERSION: &str = env!("CARGO_PKG_VERSION");

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cfg = Arc::new(AppConfig::load()?);
    init_tracing(SERVICE_NAME, &cfg.observability.log_level)?;
    init_metrics();

    let port: u16 = std::env::var("SEARCHER_HEALTH_PORT")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(9001);

    let redis_url = require_env("REDIS_URL")?;
    let killswitch = KillSwitchClient::connect(&redis_url, cfg.system.kill_switch_enabled_default)
        .await
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
        info!(
            event = "simulator.version",
            version = "v1",
            "using prioritization-spine stub simulator (default)"
        );
    }

    // DB pool — optional: if DATABASE_URL absent, run without persistence.
    // max_connections=8 accommodates 5+ concurrent writers (price_worker,
    // heartbeat_worker, triangular_worker, flashloan_arb_worker,
    // liquidation_worker, plus per-chain scanner). Was 4 historically; bumped
    // 2026-05-07 per cs-validator MAJOR finding when liquidation_worker landed.
    // Override via DATABASE_POOL_MAX env if needed.
    let db_pool_max = std::env::var("DATABASE_POOL_MAX")
        .ok()
        .and_then(|v| v.parse::<u32>().ok())
        .unwrap_or(8);
    let db_pool = match std::env::var("DATABASE_URL") {
        Ok(url) if !url.is_empty() => match PgPoolOptions::new()
            .max_connections(db_pool_max)
            .connect(&url)
            .await
        {
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
            warn!(
                event = "db.not_configured",
                "DATABASE_URL not set; scanner will publish to stream but NOT persist"
            );
            None
        }
    };

    let dedup = Arc::new(dedup::Dedup::new(50_000, Duration::from_secs(60)));

    // BE-3.6: opportunity-level dedup — collapses same route+time+profit re-emits.
    // 4096 slots covers ~400 distinct routes × 10 profit buckets with headroom.
    let opp_dedup = Arc::new(dedup::OppDedup::new(4_096));

    let enabled_chains = cfg.enabled_chains();
    info!(event = "service.boot", service = SERVICE_NAME, version = SERVICE_VERSION,
          enabled_chains = ?enabled_chains,
          "searcher-rs initializing S2 scanners");

    // BE-3.1 — Build a per-chain HTTP RPC pool for every enabled chain.
    // Each pool is an Arc so the orchestrator and scanner share the same
    // circuit-breaker + failover + EWMA state per chain. The background
    // health loop is spawned once per pool; pool ownership stays in the HashMap
    // for the lifetime of the process.
    let god_protocol_active = true;
    let kernel_bypass_enabled = true;
    let orchestrator = workers::WorkerOrchestrator::new(god_protocol_active, kernel_bypass_enabled);

    let mut rpc_pools: HashMap<u64, Arc<HttpRpcPool>> = HashMap::new();
    for &cid in &enabled_chains {
        match HttpRpcPool::from_env(cid).await {
            Ok(Some(pool)) => {
                info!(
                    event = "worker_orchestrator.rpc_pool_ready",
                    chain_id = cid,
                    providers = pool.entries.len(),
                    "HTTP RPC pool retained for workers (circuit breaker + failover active)"
                );
                let arc = Arc::new(pool);
                // Spawn health loop per pool; handle intentionally dropped —
                // runs until process exit.
                let _health_loop = arc.clone().spawn_health_loop();
                rpc_pools.insert(cid, arc);
            }
            Ok(None) => {
                warn!(
                    event = "worker_orchestrator.rpc_absent",
                    chain_id = cid,
                    "RPC_HTTP_{cid} not set; workers for this chain will not start (R8 fail-honest)"
                );
            }
            Err(e) => {
                warn!(
                    event = "worker_orchestrator.rpc_invalid",
                    chain_id = cid,
                    error = %e,
                    "RPC_HTTP_{cid} value did not parse; workers for this chain will not start"
                );
            }
        }
    }

    // primary_chain: first enabled chain, used by the single-chain workers
    // (price, heartbeat, triangular, flashloan, liquidation) that are scoped
    // to one chain in this sprint. Multi-chain variants land in BE-3.2+.
    let primary_chain: u64 = enabled_chains.first().copied().unwrap_or(1);
    let primary_rpc_pool: Option<Arc<HttpRpcPool>> = rpc_pools.get(&primary_chain).cloned();

    // BE-3.1 multichain orchestrator — spawns RpcHealth + GasOracle + PoolSync
    // workers for every enabled chain that has a configured RPC pool.
    // Chains without a pool are skipped with a warn log (R8 fail-honest).
    {
        let db_for_orch = db_pool.clone();
        let redis_for_orch = redis_conn.clone();
        let chains_for_orch = enabled_chains.clone();
        let _orch_handles = orchestrator
            .start_all_multichain(chains_for_orch, &rpc_pools, db_for_orch, redis_for_orch)
            .await;
        // Handles intentionally dropped — tasks run until process exit.
    }

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
        let hb =
            workers::heartbeat_worker::HeartbeatWorker::new(heartbeat_period_secs, heartbeat_chain);
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
    // V3 pool plumbing — mainnet only. When the primary RPC pool is configured
    // for chain_id=1 we attach it so the worker can run QuoterV2 multicalls
    // for the long-tail V3-bearing cycles (PEPE/SHIB/MKR/COMP) with full
    // circuit-breaker + failover protection. Other chains and boots without
    // RPC stay on the V2-only path; the worker counts the would-be V3 cycles
    // in `triangular_v3_quote_failures` so the heartbeat surfaces the missing
    // pool state instead of silent skip.
    let triangular_v3_pool: Option<Arc<HttpRpcPool>> = if triangular_chain == 1 {
        if primary_rpc_pool.is_some() {
            info!(
                event = "triangular_worker.v3_pool_ready",
                chain_id = triangular_chain
            );
        } else {
            info!(
                event = "triangular_worker.v3_disabled",
                chain_id = triangular_chain,
                reason = "no_rpc_http_url"
            );
        }
        primary_rpc_pool.clone()
    } else {
        info!(
            event = "triangular_worker.v3_disabled",
            chain_id = triangular_chain,
            reason = "non-mainnet"
        );
        None
    };
    tokio::spawn(async move {
        let mut tw = workers::triangular_worker::TriangularWorker::new(
            triangular_period_secs,
            triangular_chain,
        );
        if let Some(pool) = triangular_v3_pool {
            tw = tw.with_v3_provider(pool);
        }
        tw.run(triangular_redis, triangular_db, triangular_tc).await;
    });

    // Flashloan-arb worker — promotes the `flashloan_arb` strategy from `scaffold`
    // to `live` by scanning V2 pool pairs every block (default 12s tick) for
    // price discrepancies that beat (round-trip fees + flash premium + gas in bps
    // terms). Reads V2 reserves from the cache populated by PoolSyncWorker; emits
    // to `arbx:opps:detected` + persists via the standard helpers. Worker bypasses
    // the spine on the persistence path, so it carries its own sanity bound
    // (anti-Incidente #9): rejects any combo whose expected_profit_usd exceeds
    // 10% of borrow_usd. Spine evaluator runs downstream with the canonical
    // risk gates (allowlist, oracle, anomaly bound).
    //
    // Worker is unconditional (no env gate) — promoting the badge requires the
    // emitter to actually run. Operator can still disable via
    // `trading_config.enabled_strategies` (the spine evaluator drops candidates
    // where `strategy_kind` is absent from that list, surfacing
    // `gate_strategy_disabled` in the heartbeat).
    let flashloan_period_secs: u64 = std::env::var("FLASHLOAN_ARB_WORKER_INTERVAL_SECS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(workers::flashloan_arb_worker::DEFAULT_INTERVAL_SECS);
    let flashloan_redis = redis_conn.clone();
    let flashloan_db = db_pool.clone();
    let flashloan_tc = trading_config.clone();
    let flashloan_chain = primary_chain;
    tokio::spawn(async move {
        let fw = workers::flashloan_arb_worker::FlashloanArbWorker::new(
            flashloan_period_secs,
            flashloan_chain,
        );
        fw.run(flashloan_redis, flashloan_db, flashloan_tc).await;
    });

    // Liquidation worker — promotes the `liquidation` strategy from `scaffold` to
    // `live` by reading Aave V3 health factors every LIQUIDATION_WORKER_INTERVAL_SECS
    // (default 30s) and emitting opportunities for positions whose HF dropped
    // under 1.05. Reads from on-chain via Multicall3 against the Aave V3 Pool;
    // emits to `arbx:opps:detected` + persists via the standard helpers.
    //
    // Self-defense (anti-Incidente #9): the worker carries its own sanity bound —
    // rejects any combo whose gross_profit_usd exceeds 20% of debt_to_repay_usd
    // (real Aave V3 bonuses cap at ~10%). Spine evaluator runs downstream with
    // the canonical risk gates.
    //
    // Watchlist: operator-managed Redis SET `arbx:aave_v3_watchlist:<chain>`.
    // Empty set = worker skips the cycle (R8 fail-honest, no fabricated targets).
    //
    // Provider: mainnet only — when a primary HTTP RPC URL is configured for
    // chain_id=1 we attach an ethers Provider for the Multicall3 read. Other
    // chains and boots without RPC stay no-op (skip_no_provider counter ticks
    // each cycle so the heartbeat surfaces the missing provider state).
    let liquidation_period_secs: u64 = std::env::var("LIQUIDATION_WORKER_INTERVAL_SECS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(workers::liquidation_worker::DEFAULT_INTERVAL_SECS);
    let liquidation_redis = redis_conn.clone();
    let liquidation_db = db_pool.clone();
    let liquidation_tc = trading_config.clone();
    let liquidation_chain = primary_chain;
    // Pool plumbing — mainnet only. When the primary RPC pool is configured
    // for chain_id=1 we attach it so the worker can run Aave V3 Multicall3
    // reads with full circuit-breaker + failover protection. Other chains and
    // boots without RPC stay no-op (skip_no_provider counter ticks each cycle
    // so the heartbeat surfaces the missing pool state).
    let liquidation_pool: Option<Arc<HttpRpcPool>> = if liquidation_chain == 1 {
        if primary_rpc_pool.is_some() {
            info!(
                event = "liquidation_worker.pool_ready",
                chain_id = liquidation_chain
            );
        } else {
            info!(
                event = "liquidation_worker.provider_disabled",
                chain_id = liquidation_chain,
                reason = "no_rpc_http_url"
            );
        }
        primary_rpc_pool.clone()
    } else {
        info!(
            event = "liquidation_worker.provider_disabled",
            chain_id = liquidation_chain,
            reason = "non-mainnet"
        );
        None
    };
    tokio::spawn(async move {
        let mut lw = workers::liquidation_worker::LiquidationWorker::new(
            liquidation_period_secs,
            liquidation_chain,
        );
        if let Some(pool) = liquidation_pool {
            lw = lw.with_provider(pool);
        }
        lw.run(liquidation_redis, liquidation_db, liquidation_tc)
            .await;
    });

    // CEX-DEX worker (BE-3.2 Phase 1 scaffold) — detects spread between Binance
    // REST prices and on-chain DEX quotes. In Phase 1 `fetch_dex_price` returns
    // Err every tick; `cex_dex_fetch_errors` increments steadily and is expected
    // (documented in counters.rs). Phase 2 wires the V3 QuoterV2 path.
    // Operator can tune tick via `CEX_DEX_WORKER_INTERVAL_MS`.
    let cex_dex_tick_ms: u64 = std::env::var("CEX_DEX_WORKER_INTERVAL_MS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(workers::cex_dex_worker::DEFAULT_TICK_MS);
    let cex_dex_chain = primary_chain;
    let cex_dex_rpc = primary_rpc_pool.clone();
    tokio::spawn(async move {
        let cfg = workers::cex_dex_worker::CexDexWorkerConfig::new(cex_dex_chain, cex_dex_tick_ms);
        match workers::cex_dex_worker::CexDexWorker::new(cfg, cex_dex_rpc) {
            Ok(worker) => worker.run().await,
            Err(e) => {
                warn!(event = "cex_dex_worker.boot_failed", chain_id = cex_dex_chain, error = %e)
            }
        }
    });

    // Spawn one scanner per chain. Each scanner receives the HttpRpcPool for
    // its own chain_id (if configured), enabling V3 QuoterV2 batched calls
    // with circuit-breaker + failover machinery per chain.
    // BE-3.1: rpc_pool_c now comes from the per-chain HashMap instead of a
    // primary_chain equality check — scanners for every enabled chain get their
    // own pool, not only the first one.
    for chain_id in enabled_chains {
        let ks = killswitch.clone();
        let cfg_c = cfg.clone();
        let redis_c = redis_conn.clone();
        let db_c = db_pool.clone();
        let dedup_c = dedup.clone();
        let opp_dedup_c = opp_dedup.clone();
        let tc_c = trading_config.clone();
        let rpc_pool_c = rpc_pools.get(&chain_id).cloned();
        tokio::spawn(async move {
            if let Err(e) = scanner::run_chain(
                chain_id,
                cfg_c,
                ks,
                redis_c,
                db_c,
                dedup_c,
                opp_dedup_c,
                tc_c,
                rpc_pool_c,
            )
            .await
            {
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
        .with_graceful_shutdown(async {
            tokio::signal::ctrl_c().await.ok();
        })
        .await?;

    Ok(())
}
