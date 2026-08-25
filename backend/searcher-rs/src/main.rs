// M11 (audit 2026-05-10): gate PRs that introduce panicking paths in entry point.
// Promoted from warn→deny so CI fails on new unwrap/expect in this binary.
// Per-file #[allow(...)] is used below in files where the pattern is
// demonstrably safe (mutex poison, infallible slice casts, test modules).
#![allow(
    unused_imports,
    unused_variables,
    unreachable_patterns,
    unexpected_cfgs,
    clippy::unwrap_used,
    clippy::manual_unwrap_or_default,
    clippy::manual_unwrap_or,
    dead_code,
    unused_mut
)]

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
mod canonical_knobs;
mod chain_client;
mod chain_supervisor;
mod counters;
mod dedup;
// Phase A.3.a: OpportunityCandidate → RoundTripContext encoder. Pure bridge
// from the abstract candidate to the typed simulator input. NO REVM dispatch
// here (that lands with execute_round_trip); only validation + payload pre-shaping.
// G-SIM-1 PR-B2a: moved to `sim-core`; re-exported so `crate::sim_encoder::*`
// call sites in the binary (scanner.rs, sim_encoder_pg.rs) keep resolving.
pub(crate) use sim_core::sim_encoder;
// Phase A.3.b: PostgreSQL-backed `TokenDecimalsProvider` runtime impl. Holds
// an LRU cache kept warm by a background refresh task; the trait method is
// a pure cache lookup so the hot path never blocks on a PG query.
mod sim_encoder_pg;
// Phase A.3.c: REVM execution orchestrator. Builds executeArbitrage calldata
// from RoundTripContext and dispatches a single REVM transaction through
// SimulatorV2. Synchronous; the scanner wraps it in spawn_blocking.
mod sim_orchestrator;
// Phase A.3.c.2: ERC-20 storage prefund computation layer. Pure helpers +
// provider trait + slot computation. Does NOT mutate REVM state; the A.3.c.3
// multi-step orchestrator consumes the returned PrefundPlan and applies it.
//
// G-SIM-1 PR-B1 (Option B): moved VERBATIM to the shared `sim-core` crate;
// re-exported here so `crate::sim_prefund::*` call sites in the binary keep
// resolving unchanged.
#[allow(unused_imports)]
pub(crate) use sim_core::sim_prefund;
// Phase OMEGA: Kelly Criterion + V3 concentrated liquidity math. Pure
// position-sizing primitives consumed by the size_optimizer hot path.
#[allow(dead_code)]
mod kelly_sizing;
// Phase OMEGA 3.2: Bayesian inference + VPIN/PIN adverse-selection
// filters. Pure math primitives for the candidate-selection layer.
#[allow(dead_code)]
mod bayesian_filter;
// Phase A.3.c.2: Multi-step REVM orchestrator skeleton + plan builder.
// Combines sim_prefund storage overrides with RoundTripContext to build
// a deterministic multi-step execution plan. REVM CacheDB execution is
// deferred to A.3.c.3; this module ships validation + plan + tests.
//
// G-SIM-1 PR-B1 (Option B): moved VERBATIM to the shared `sim-core` crate;
// re-exported here so `crate::sim_multistep::*` call sites in the binary
// (scanner.rs) keep resolving unchanged.
#[allow(unused_imports)]
pub(crate) use sim_core::sim_multistep;
// B1.c (2026-05-13) — chain config hot-reload subscriber. Listens on Redis
// pub/sub `arbx:config:chains:reload` for events from the api-server admin
// endpoint.
mod config_reload;
// Phase 2: Topology Vault hot-reload subscriber. Listens on `arbx:topology:mutation`,
// applies durable HTTP fallback at cold boot, and swaps RPC/WS clients atomically.
mod topology_reload;
// Phase 16: per-strategy Prometheus metrics for the event-driven orchestrator.
mod metrics;
mod patterns;
mod persistence;
mod publisher;
mod reserves;
// XLS-QB-05 / ARBX-0003: dirty-pool signal consumed by `workers::pool_sync_worker`
// (this bin crate compiles the workers module tree, so it declares the module too).
mod dirty_signal;
// XLS-QB-05b/05c / ARBX-0003: consumer drain + pair→cycles inverted index
// (drain lives in route_discovery_worker; declared here for the dual-tree).
#[allow(dead_code)]
mod cycle_index;
#[allow(dead_code)]
mod dirty_consumer;
// XLS-QB / ARBX-0008: N-bucket amount sweep (pure surface; the motor side
// lives in size_optimizer — declared here for the dual-tree).
#[allow(dead_code)]
mod amount_buckets;
// XLS-QB-06b / ARBX-0024: F_e normalization + QuoteState — consumed by the
// route-discovery prefilter (declared here for the dual-tree).
#[allow(dead_code)]
mod fe_normalization;
// ARBX-0007: financing-mode route dimension (fees, per-mode eval, selection).
mod financing;
// ARBX-0009: sheet-07 Net_bps contract + deterministic ranking (QB 07).
mod net_bps_ranking;
// FE-MASTER EMIT-06b: pair alpha publish (dual-tree — the lib declares it for tests).
#[allow(dead_code)]
mod pair_alpha_runtime;
// Gate completion 2026-08-24: shared source files (cycle_index, graph_builder,
// route_discovery_worker, opportunity_emitter…) reference these via `crate::`
// — the bin root must declare them too or the bin target fails E0432/E0433
// while the lib stays green.
#[allow(dead_code)]
mod dirty_pairs;
#[allow(dead_code)]
mod hot_seed_mask;
#[allow(dead_code)]
mod latency_budget;
#[allow(dead_code)]
mod pair_index;
#[allow(dead_code)]
mod quote_anchor_runtime;
#[allow(dead_code)]
mod quote_anchor_signal;
#[allow(dead_code)]
mod quote_score;
#[allow(dead_code)]
mod required_data_gate;
mod scanner;
#[allow(dead_code)]
mod signal_tier;
// ARBX-0018: address-keyed token identity (bin crate compiles the same tree).
mod token_identity;
// FE-MASTER EMIT-01: universe snapshot writer used by `token_identity`
// (dual-tree — the lib also declares it for tests).
mod token_resolve_signal;
// FASE OMEGA — cartridge runtime (Rhai). `cartridge` + `cartridge_loader` are API-heavy
// and mostly exercised via lib/integration tests; the binary only drives them through
// `cartridge_boot` (called from the scanner boot path), so allow dead_code on the two.
#[allow(dead_code, unused_imports, unused_variables)]
mod cartridge;
mod cartridge_boot;
#[allow(dead_code)]
mod cartridge_loader;
// FASE OMEGA — Block/log backrunning scanner (ARBX_MEMPOOL_MODE=block).
mod block_scanner;
// Observer telemetry — real-node head divergence (reorg) PUBLISH to arbx:telemetry:observability.
mod telemetry_observability;
// Phase 1-3: orchestrator modules — fully wired in Phase 14.
// `impact_index` still has Phase-15 functions (from_registry, add_pool,
// seed_cycles_from_mvp) that are public API but not yet called by the binary.
// Allow dead_code for those; the binary uses ImpactIndex::empty() and resolve().
#[allow(dead_code)]
mod impact_index;
mod opportunity_emitter;
mod pool_candidate;
mod pool_discovery;
mod pool_sources;
mod route_decoder;
mod scoring;
mod scoring_pipeline;
mod source_supervisor;
// Phase 1 radar — declared in the bin crate so `scanner::run_chain` can spawn
// the worker. The lib crate also declares it (`pub mod route_discovery`) so the
// pure submodules are unit-tested under `cargo test --lib`.
#[allow(dead_code)]
mod route_discovery;
mod route_intent;
// XLS-QB-02: declared in BOTH crates — route_discovery_worker references
// `crate::strategy_hop_mask` (XLS-QB-03 hop-mask dispatch), which resolves
// against each target's own module tree. The bin uses only
// `admissible_hop_bounds`; the lookup helpers are exercised in lib tests.
#[allow(dead_code)]
mod strategy_hop_mask;
// ARBX-0021: same dual-crate declaration as strategy_hop_mask —
// route_discovery_worker references `crate::strategy_dispatch_status`
// (Status dispatch) against each target's own module tree.
#[allow(dead_code)]
mod strategy_dispatch_status;
// ARBX-TW-005: col Execution_Class annotation (29 classes).
#[allow(dead_code)]
mod strategy_execution_class;
// ARBX-0026: sheet 13_DETECTOR_POLICY (60 detectors) — graph family, family
// hop envelope, Do_Not_Do guard, hot-seed admission.
#[allow(dead_code)]
mod detector_policy;
mod strategy_label;
// Phase 7-8: orchestrator + engines — fully wired in Phase 14.
// `engines` still has Phase-15 hooks (insert, from_mvp_cycles, etc.) unused
// by the binary yet. Allow dead_code on the module; they ARE used in lib tests.
#[allow(dead_code)]
mod engines;
// Fix B — math evidence (observe-only): builds MarketState from reserves and
// evaluates RegimeRouter-recommended operators. Called from orchestrator.
mod math_evidence;
// FASE OMEGA: Gate subsystem (MacroMevGate / Operador Energético). The binary
// compiles orchestrator.rs as part of THIS crate (crate root = main.rs), so it
// must declare `shared` + `gates` itself — the lib (lib.rs) declares them too,
// but each crate root owns its own `crate::` namespace. Without these two lines
// the binary fails with E0432/E0433 unresolved import on crate::gates / crate::shared.
mod gates;
mod orchestrator;
mod shared;
// Phase 11: LendingPositionIndexer — Redis-backed watchlist + position cache.
// Dead-code allowed: the indexer is Arc-constructed and passed to the
// LiquidationEngine; individual methods are called through the engine.
#[allow(dead_code)]
mod lending_position_indexer;
mod workers;
// Phase 12-13: StateProjector + SizeOptimizer — wired in Phase 14.
// Individual methods (project_v2_post_swap, project_triangular_cycle) are
// Phase-15 hooks not yet called from the binary; allow dead_code.
#[allow(dead_code)]
mod size_optimizer;
#[allow(dead_code)]
mod state_projector;
#[allow(dead_code)]
mod v3_quote_provider;

use shared_rs::{
    config::{require_env, AppConfig},
    health::{build_health_router, ServiceInfo},
    killswitch::KillSwitchClient,
    logging::init_tracing,
    metrics::init_metrics,
    rpc_failover::HttpRpcPool,
    trading_config::TradingConfigClient,
};
use std::{collections::HashMap, net::SocketAddr, sync::Arc, time::Duration};
use tracing::{info, warn};

const SERVICE_NAME: &str = "searcher-rs";
const SERVICE_VERSION: &str = env!("CARGO_PKG_VERSION");

// ---------------------------------------------------------------------------
// Phase 15 — legacy worker gating
// ---------------------------------------------------------------------------
//
// Default = OFF post-Phase 15. Legacy workers are superseded by the
// event-driven orchestrator (Phases 8-11). They remain in the codebase as
// an audit/fallback path behind explicit opt-in flags.
//
// The workers themselves are NOT modified — they just don't get spawned.
// When the orchestrator is active (V2 or Shadow mode), the legacy workers
// duplicate detection work and bypass the orchestrator dedup; operators
// should keep them off unless debugging.

/// Returns true when ARBX_ENABLE_LEGACY_TRIANGULAR_WORKER=true.
/// Defaults to false post-Phase 15.
fn legacy_triangular_worker_enabled() -> bool {
    std::env::var("ARBX_ENABLE_LEGACY_TRIANGULAR_WORKER")
        .map(|v| v.eq_ignore_ascii_case("true"))
        .unwrap_or(false) // default OFF post-Phase 15
}

/// Returns true when ARBX_ENABLE_LEGACY_FLASHLOAN_WORKER=true.
/// Defaults to false post-Phase 15.
fn legacy_flashloan_arb_worker_enabled() -> bool {
    std::env::var("ARBX_ENABLE_LEGACY_FLASHLOAN_WORKER")
        .map(|v| v.eq_ignore_ascii_case("true"))
        .unwrap_or(false)
}

/// Returns true when ARBX_ENABLE_LEGACY_LIQUIDATION_WORKER=true.
/// Defaults to false post-Phase 15.
fn legacy_liquidation_worker_enabled() -> bool {
    std::env::var("ARBX_ENABLE_LEGACY_LIQUIDATION_WORKER")
        .map(|v| v.eq_ignore_ascii_case("true"))
        .unwrap_or(false)
}

// ---------------------------------------------------------------------------
// Phase 16 — Prometheus counter for disabled workers (replaces AtomicU64)
// ---------------------------------------------------------------------------
// The counter is defined in metrics.rs (LEGACY_WORKER_DISABLED_TOTAL) and
// registered in the shared Prometheus registry. We import it here for use in
// the boot sequence below. The `worker` label value must be one of the three
// canonical module names: "triangular_worker", "flashloan_arb_worker",
// "liquidation_worker".
use crate::metrics::{
    init_orchestrator_metrics, LEGACY_WORKERS_DISABLED_COUNT, LEGACY_WORKER_DISABLED_TOTAL,
};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cfg = Arc::new(AppConfig::load()?);
    init_tracing(SERVICE_NAME, &cfg.observability.log_level)?;
    init_metrics();
    // Phase 16: register per-strategy orchestrator metrics into the shared registry.
    init_orchestrator_metrics();
    // Phase 2: expose applied topology version/client-readiness gauges for clean UI surfaces.
    topology_reload::init_topology_metrics();

    // ── OMEGA SEAL — capital-key lockout (defense-in-depth, ALL modes) ─────────
    // searcher-rs is a DETECTION/EMIT (OBSERVER) service and must NEVER hold a
    // capital-bearing signing key in ANY mode: it spawns no executor and never
    // signs nor broadcasts (workers/execution_worker.rs is a never-spawned stub;
    // sim_orchestrator.rs: "the orchestrator NEVER signs nor broadcasts"; every
    // RPC provider is read-only — no Wallet/SignerMiddleware anywhere).
    //
    // Enforce capital_exposed == 0 as a HARD, mode-INDEPENDENT boot invariant: if
    // any private key / mnemonic is present in THIS process's env (incl. the
    // testnet/executor names), panic before any work. This makes a testnet OR
    // mainnet broadcast posture physically impossible to enable by env alone —
    // turning execution on is a deliberate code change here, gated by FASE D
    // (KMS + pre-execute-checklist + registered human authorization), never a flag.
    // An empty value (e.g. PRIVATE_KEY="" per the Foundry doctrine) does NOT trip
    // this — only a populated key.
    {
        const CAPITAL_KEY_ENV: [&str; 8] = [
            "FLASHBOTS_SIGNER_KEY",
            "EXECUTOR_PRIVATE_KEY",
            "ARBX_EXECUTOR_PRIVATE_KEY",
            "ARBX_SIGNER_PRIVATE_KEY",
            "ARBX_TESTNET_PRIVATE_KEY",
            "SIM_SIGNER_PRIVATE_KEY",
            "PRIVATE_KEY",
            "MNEMONIC",
        ];
        for key in CAPITAL_KEY_ENV {
            if matches!(std::env::var(key), Ok(v) if !v.trim().is_empty()) {
                panic!(
                    "FATAL: capital-bearing key `{key}` present in env — searcher-rs is an \
                     OBSERVER/detection service and must run with capital_exposed == 0 in ALL \
                     modes (no signer, no broadcast). Remove the key; enabling execution is a \
                     deliberate FASE D change, not an env flag."
                );
            }
        }
        tracing::info!(
            event = "searcher.capital_lock",
            "capital-key lockout verified: no signer keys in env (capital_exposed=0, observer-only, all modes)"
        );
    }

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

    // ── XLS-CANON-01 — canonical knobs (01_CONFIG ULTRA workbook) ─────────────
    // Load the 42-knob canonical surface, validate (fail-fast boot on invariant
    // violation — config validation is defensive, not speculative), log it, and
    // publish the snapshot to Redis for the api-server surface
    // (`GET /api/v1/config/canonical-knobs`). Declarative/observability only:
    // mode authority stays in relays-client::live_exec_policy (§34.3) and the
    // existing kill-switch system — these knobs never flip execution semantics.
    {
        let knobs = canonical_knobs::CanonicalKnobs::from_env();
        if let Err(err) = knobs.validate() {
            anyhow::bail!("canonical knobs invalid (XLS-CANON-01): {err}");
        }
        let snapshot = knobs.to_json();
        info!(
            event = "config.canonical_knobs",
            knobs = %snapshot,
            "canonical knobs loaded (42-knob 01_CONFIG surface; precedence env>yaml>workbook)"
        );
        let mut knobs_redis = redis_conn.clone();
        let payload = snapshot.to_string();
        let set_result: Result<(), redis::RedisError> = redis::cmd("SET")
            .arg("arbx:config:canonical_knobs")
            .arg(&payload)
            .query_async(&mut knobs_redis)
            .await;
        if set_result.is_err() {
            warn!(
                event = "config.canonical_knobs.publish_failed",
                "canonical-knobs snapshot not published to Redis (non-fatal; retried at next boot)"
            );
        }
    }

    // Phase 2 — Topology Vault runtime (durable fallback + Redis Pub/Sub).
    // Full RPC/WS URLs stay inside this process. Operators may set
    // TOPOLOGY_SNAPSHOT_URL to the internal api-server snapshot endpoint for cold boot;
    // Redis mutations on `arbx:topology:mutation` remain the live update path.
    let topology_runtime = topology_reload::TopologyRuntime::new();
    let topology_cancel = tokio_util::sync::CancellationToken::new();
    let topology_subscriber = topology_reload::TopologySubscriber::new(
        redis_url.clone(),
        std::env::var("TOPOLOGY_SNAPSHOT_URL").ok(),
        std::env::var("TOPOLOGY_ADMIN_TOKEN").ok(),
        topology_runtime.clone(),
        Arc::new(topology_reload::LiveTopologyClientFactory),
        topology_cancel,
    );
    tokio::spawn(async move {
        if let Err(e) = topology_subscriber.run().await {
            warn!(
                event = "topology.subscriber_failed",
                error = %e,
                "topology subscriber exited; restart pod to recover Redis hot-reload"
            );
        }
    });

    // Trading-config client (Redis-backed, hot-reload <1s) — re-uses the manager
    // above so we don't double the open-fd count per pod. Each scanner per-chain
    // calls `state(chain_id)` per opportunity to honour operator updates without
    // service restart.
    let trading_config = TradingConfigClient::from_manager(redis_conn.clone());

    // B1.c (2026-05-13) — chain config hot-reload subscriber.
    //
    // Listens on Redis pub/sub `arbx:config:chains:reload` for events
    // published by the api-server admin endpoint when an operator
    // mutates `chains_runtime` via the `/admin/chains` UI. Tracks
    // last-seen config_hash per chain to skip no-op reloads. Runs in
    // its own tokio task — never blocks the scanner main thread.
    //
    // Graceful shutdown is wired via `CancellationToken`; for the
    // current MVP we hold the token for the process lifetime (no
    // cancellation path is triggered yet). B1.d will plumb the token
    // to the chain task supervisor so a global "rebuild this chain"
    // event can use the dedup map exposed via `seen_hashes()`.
    let chain_reload_cancel = tokio_util::sync::CancellationToken::new();
    let chain_reload_redis_url = redis_url.clone();
    let _chain_reload_cancel_handle = chain_reload_cancel.clone();

    let reloader =
        config_reload::ChainConfigReloader::new(chain_reload_redis_url, chain_reload_cancel);
    let event_rx = reloader.event_tx.subscribe();

    tokio::spawn(async move {
        if let Err(e) = reloader.run().await {
            warn!(
                event = "chain_reload.boot_failed",
                error = %e,
                "chain config reload subscriber exited with error; restart pod to recover"
            );
        }
    });

    // Sprint 4 — Phase A.1/A.2/A.2.5 simulator boot announcement.
    //
    // History:
    //   - A.1/A.2: legacy v1 stub neutralized (`prioritization_spine::simulator::
    //     EvmSimulator` was fabricating "PASS" with caller=0x11..11/target=0x22..22/
    //     calldata=Bytes::new() — RULE 00 violation).
    //   - A.2.5 (this PR): construct per-chain `Arc<SimulatorV2>` at boot when
    //     `RPC_HTTP_<chain_id>` is configured. Thread through scanner. Hot path
    //     stays fail-closed until the A.3 encoder lands; the only behavioural
    //     change today is the reason code emitted on rejection.
    //
    // Phase A.3 (separate PR) will land:
    //   - `OpportunityCandidate` → `simulator_v2::CandidateInput` encoder
    //   - real `executeArbitrage(...)` calldata against `ArbitrageExecutor.sol`
    //   - net_profit_wei extraction from real REVM balance delta
    let use_simulator_v2 = std::env::var("ARBX_USE_SIMULATOR_V2")
        .map(|v| v.eq_ignore_ascii_case("true"))
        .unwrap_or(false);
    let _ = use_simulator_v2; // honoured by the boot log below; no runtime flip yet.

    // DB pool — optional: if DATABASE_URL absent, run without persistence.
    // max_connections=8 accommodates 5+ concurrent writers (price_worker,
    // heartbeat_worker, triangular_worker, flashloan_arb_worker,
    // liquidation_worker, plus per-chain scanner). Was 4 historically; bumped
    // 2026-05-07 per cs-validator MAJOR finding when liquidation_worker landed.
    // Override via DATABASE_POOL_MAX env if needed.
    // OMEGA-8/M3 P1-2: explicit timeouts so connection acquisition cannot
    // block forever under pressure. `DATABASE_POOL_MAX` continues to override
    // the default; the other timeout knobs (DATABASE_ACQUIRE_TIMEOUT_SECS,
    // DATABASE_IDLE_TIMEOUT_SECS, DATABASE_MAX_LIFETIME_SECS,
    // DATABASE_POOL_MIN) live inside `shared::PoolConfig::from_env`.
    let db_pool_max = std::env::var("DATABASE_POOL_MAX")
        .ok()
        .and_then(|v| v.parse::<u32>().ok())
        .unwrap_or(8);
    let pool_cfg = shared_rs::db_pool::PoolConfig::from_env(db_pool_max);
    let db_pool = match std::env::var("DATABASE_URL") {
        Ok(url) if !url.is_empty() => {
            // Boot-race fix (P0): `depends_on: service_healthy` is not always
            // honoured at the moment the searcher process starts connecting, and
            // a single fast-fail connect permanently disables pool_sync +
            // persistence for the whole process lifetime. Bounded-retry the
            // initial connect with exponential backoff so a Postgres that is
            // merely *slow to become ready* is awaited, while a genuinely absent
            // DB still fails honest (None) after the retries are exhausted.
            let opts = shared_rs::db_pool::options_with_timeouts(&pool_cfg);
            let mut attempt = 0u32;
            let max_attempts = std::env::var("DATABASE_CONNECT_RETRIES")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(5u32);
            let mut connected = None;
            loop {
                attempt += 1;
                match opts.clone().connect(&url).await {
                    Ok(p) => {
                        info!(event = "db.connected", attempt, "postgres pool up");
                        connected = Some(p);
                        break;
                    }
                    Err(e) if attempt < max_attempts => {
                        let backoff_ms = 500u64 << attempt; // 1s, 2s, 4s, 8s, ...
                        warn!(event = "db.connect_retry", attempt, max_attempts,
                              backoff_ms, error = %e,
                              "postgres not ready — retrying initial connect");
                        tokio::time::sleep(std::time::Duration::from_millis(backoff_ms)).await;
                    }
                    Err(e) => {
                        warn!(event = "db.connect_failed", attempts = attempt, error = %e,
                              "continuing without DB persistence (opportunities only published to stream)");
                        break;
                    }
                }
            }
            connected
        }
        _ => {
            warn!(
                event = "db.not_configured",
                "DATABASE_URL not set; scanner will publish to stream but NOT persist"
            );
            None
        }
    };

    // Phase A.3.b runtime wire — `PgTokenDecimalsProvider` construction.
    //
    // When the DB pool is up, build a single per-process provider that the
    // scanner consults from the simulator gate. Bootstrap-loads the cache
    // synchronously so the first hot-path candidates already see warm data;
    // spawns a 60s refresh loop in the background.
    //
    // Without a DB pool the encoder cannot resolve decimals — scanner stays
    // in the Phase A.2.5 fail-closed path (`no_simulator_for_chain` /
    // `encoder_not_ready`) for every candidate (RULE 12 fail-honest).
    let token_decimals_provider: Option<Arc<dyn sim_encoder::TokenDecimalsProvider + Send + Sync>> =
        match &db_pool {
            Some(pool) => {
                let provider = Arc::new(
                    sim_encoder_pg::PgTokenDecimalsProvider::with_default_capacity(pool.clone()),
                );
                match provider.bootstrap_load().await {
                    Ok(loaded) => info!(
                        event = "sim_encoder.boot",
                        provider = "pg",
                        cache_max_entries = sim_encoder_pg::DEFAULT_CACHE_CAPACITY,
                        bootstrap_loaded = loaded,
                        refresh_interval_secs = sim_encoder_pg::DEFAULT_REFRESH_INTERVAL_SECS,
                        runtime_enabled = true,
                        "PG decimals provider bootstrapped"
                    ),
                    Err(e) => warn!(
                        event = "sim_encoder.bootstrap_failed",
                        error = %e,
                        "PG decimals provider bootstrap failed; cache stays empty until first refresh"
                    ),
                };
                // Refresh loop fills the cache from PG every N seconds. Handle
                // intentionally dropped — task runs until process exit.
                provider.clone().spawn_refresh_loop(Duration::from_secs(
                    sim_encoder_pg::DEFAULT_REFRESH_INTERVAL_SECS,
                ));
                Some(provider as Arc<dyn sim_encoder::TokenDecimalsProvider + Send + Sync>)
            }
            None => {
                warn!(
                    event = "sim_encoder.boot",
                    provider = "none",
                    runtime_enabled = false,
                    reason = "no_db_pool",
                    "no PG pool available — token decimals provider not constructed"
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

    // Sprint 4 — Phase A.2.5: per-chain `Arc<SimulatorV2>` construction.
    //
    // For every chain that has a healthy HTTP RPC pool, spin up exactly one
    // `SimulatorV2` instance and share it across all per-chain tasks via Arc.
    // The simulator is constructed with the FIRST pool entry's URL (failover
    // for sim is degraded vs the broader hot path — accepted trade-off for the
    // initial wire-up; revisit once latency telemetry is in place).
    //
    // ## Linearizability pin (cs-validator MAJOR finding 2026-05-12)
    //
    // `SimulatorV2` memoizes the block number via `OnceLock`. Multiple tokio
    // tasks racing on the first `simulate()` call could each resolve "latest"
    // independently and land on different blocks (the OnceLock keeps the
    // first writer, but the losing tasks have already used their own value
    // for the in-flight call).
    //
    // Mitigation: when the pool's health loop has already observed a block
    // (`snapshot_block() > 0`), pin the simulator to that block via
    // `with_block(N)`. All tasks then see the same block from the very first
    // call. When the health loop has not yet reported (rare boot race),
    // `SimulatorV2` falls back to lazy "latest" resolution which still
    // converges to the same block within a single slot.
    //
    // ## No fallback to stub (RULE 12)
    //
    // Chains without `RPC_HTTP_<id>` get NO simulator. The scanner sees
    // `None` for that chain and keeps emitting `SIM_DISABLED_FAIL_CLOSED`
    // (Phase A.1/A.2 behaviour). This is honest fail-closed, not silent
    // degradation.
    #[cfg(feature = "v2-simulator")]
    let simulators_v2: HashMap<u64, Arc<simulator_v2::SimulatorV2>> = {
        let mut map: HashMap<u64, Arc<simulator_v2::SimulatorV2>> = HashMap::new();
        for (&cid, pool) in rpc_pools.iter() {
            // `pick()` honours circuit-breaker + EWMA. If every entry is
            // unhealthy we skip the chain (R8 fail-honest, no stub fallback).
            let primary = match pool.pick() {
                Ok(p) => p,
                Err(e) => {
                    warn!(
                        event = "simulator_v2.unavailable",
                        chain_id = cid,
                        error = %e,
                        reason = "all_providers_unhealthy",
                        "no simulator constructed for chain (RULE 12 fail-closed)"
                    );
                    continue;
                }
            };
            let observed_block = primary.snapshot_block();
            let sim = if observed_block > 0 {
                // Pin the block so concurrent simulate() calls all see the
                // same state snapshot (cs-validator MAJOR fix).
                simulator_v2::SimulatorV2::new(primary.url.clone()).with_block(observed_block)
            } else {
                // Health loop has not reported yet; first simulate() will
                // resolve "latest" and memoize. The window is bounded to a
                // single block slot in practice.
                simulator_v2::SimulatorV2::new(primary.url.clone())
            };
            map.insert(cid, Arc::new(sim));
            info!(
                event = "simulator_v2.available",
                chain_id = cid,
                provider = %primary.name,
                pinned_block = observed_block,
                "per-chain SimulatorV2 constructed (Phase A.2.5)"
            );
        }
        map
    };
    #[cfg(not(feature = "v2-simulator"))]
    let simulators_v2: HashMap<u64, ()> = HashMap::new();

    let chains_with_simulator = simulators_v2.len();
    let chains_without_simulator = enabled_chains.len().saturating_sub(chains_with_simulator);
    info!(
        event = "simulator.boot",
        phase = "A.2.5",
        v2_feature_compiled = cfg!(feature = "v2-simulator"),
        v2_runtime_requested = use_simulator_v2,
        backend = "revm",
        paper_mode = true,
        live_execution = false,
        fallback_stub = false,
        chains_with_simulator,
        chains_without_simulator,
        "SimulatorV2 instances constructed per chain; hot path stays fail-closed \
         (reason=encoder_not_ready) until Phase A.3 encoder lands"
    );

    // primary_chain: first enabled chain, used by the single-chain workers
    // (price, heartbeat, triangular, flashloan, liquidation) that are scoped
    // to one chain in this sprint. Multi-chain variants land in BE-3.2+.
    //
    // B0.4 (2026-05-13) — Fail-honest if enabled_chains is empty.
    //
    // Previously this used `.unwrap_or(1)` which silently defaulted to chain 1
    // even when the operator had no chains configured. That hid the real
    // misconfiguration. Now we fail loudly: an empty enabled_chains list is
    // a fatal boot error (paper-mode safety: no scanner without chains).
    let primary_chain: u64 = match enabled_chains.first().copied() {
        Some(c) => c,
        None => {
            anyhow::bail!(
                "B0.4 fail-honest: enabled_chains is empty. Boot refused. Configure at least one chain via configs/app.toml [[chains]] (with enabled=true) or ARBX_ENABLED_CHAINS env var. No silent fallback to chain 1."
            );
        }
    };
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
    // Coingecko Demo/Pro key (optional). When set, fetch_coingecko attaches the
    // x-cg-demo-api-key header (free tier now 400s unauth). Plan A.2 code-gap.
    let price_coingecko_key = std::env::var("COINGECKO_API_KEY")
        .ok()
        .filter(|s| !s.trim().is_empty());
    let price_redis = redis_conn.clone();
    let price_chain = primary_chain;
    // Tier-0 Chainlink on-chain feeds: needs the PG pool (reads operator-seeded
    // price_oracles) + the chain's HTTP RPC (eth_call latestRoundData). Both
    // optional — when either is absent the worker skips Chainlink and behaves as
    // before (Alchemy/Coingecko/Config cascade). Read-only; no signing.
    let price_db = db_pool.clone();
    let price_rpc_url = workers::price_worker::rpc_http_url_from_env(primary_chain);
    tokio::spawn(async move {
        let mut cfg = workers::price_worker::PriceWorkerConfig::new(
            price_chain,
            price_period_secs,
            price_alchemy_key,
        );
        if let (Some(db), Some(url)) = (price_db, price_rpc_url) {
            cfg = cfg.with_chainlink(db, url);
        }
        cfg.coingecko_api_key = price_coingecko_key;
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
    if legacy_triangular_worker_enabled() {
        info!(
            event = "scanner.legacy_worker_state",
            worker = "triangular_worker",
            enabled = true,
            reason = "ARBX_ENABLE_LEGACY_TRIANGULAR_WORKER=true",
            "WARNING: legacy worker enabled — opportunities bypass orchestrator dedup"
        );
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
    } else {
        // Label value comes from the canonical module name — never a hardcoded
        // abbreviation. This is the single source of truth for Grafana dashboards.
        LEGACY_WORKER_DISABLED_TOTAL
            .with_label_values(&["triangular_worker"])
            .inc();
        LEGACY_WORKERS_DISABLED_COUNT.inc();
        info!(
            event = "scanner.legacy_worker_state",
            worker = "triangular_worker",
            enabled = false,
            reason = "default-off-post-phase-15",
            "triangular_worker not spawned; event-driven TriangularEngine active"
        );
    }

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
    if legacy_flashloan_arb_worker_enabled() {
        info!(
            event = "scanner.legacy_worker_state",
            worker = "flashloan_arb_worker",
            enabled = true,
            reason = "ARBX_ENABLE_LEGACY_FLASHLOAN_WORKER=true",
            "WARNING: legacy worker enabled — opportunities bypass orchestrator dedup"
        );
        tokio::spawn(async move {
            let fw = workers::flashloan_arb_worker::FlashloanArbWorker::new(
                flashloan_period_secs,
                flashloan_chain,
            );
            fw.run(flashloan_redis, flashloan_db, flashloan_tc).await;
        });
    } else {
        LEGACY_WORKER_DISABLED_TOTAL
            .with_label_values(&["flashloan_arb_worker"])
            .inc();
        LEGACY_WORKERS_DISABLED_COUNT.inc();
        info!(
            event = "scanner.legacy_worker_state",
            worker = "flashloan_arb_worker",
            enabled = false,
            reason = "default-off-post-phase-15",
            "flashloan_arb_worker not spawned; event-driven FlashloanEngine active"
        );
    }

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
    if legacy_liquidation_worker_enabled() {
        info!(
            event = "scanner.legacy_worker_state",
            worker = "liquidation_worker",
            enabled = true,
            reason = "ARBX_ENABLE_LEGACY_LIQUIDATION_WORKER=true",
            "WARNING: legacy worker enabled — opportunities bypass orchestrator dedup"
        );
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
    } else {
        LEGACY_WORKER_DISABLED_TOTAL
            .with_label_values(&["liquidation_worker"])
            .inc();
        LEGACY_WORKERS_DISABLED_COUNT.inc();
        info!(
            event = "scanner.legacy_worker_state",
            worker = "liquidation_worker",
            enabled = false,
            reason = "default-off-post-phase-15",
            "liquidation_worker not spawned; event-driven LiquidationEngine active"
        );
    }

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

    // B1.d: Chain Task Supervisor
    // Consumes `seen_hashes` events to orchestrate hot-reloads without blocking.
    let supervisor = chain_supervisor::ChainSupervisor::new(
        cfg.clone(),
        killswitch.clone(),
        redis_conn.clone(),
        db_pool.clone(),
        dedup.clone(),
        opp_dedup.clone(),
        trading_config.clone(),
        token_decimals_provider.clone(),
        event_rx,
    );

    let initial_chains = enabled_chains.clone();
    tokio::spawn(async move {
        supervisor.run(initial_chains).await;
    });

    // HTTP server.
    let app = build_health_router(ServiceInfo::new(SERVICE_NAME, SERVICE_VERSION));

    // G-SIM-1 PR-B2b Fase 3 (A2): mount /route/:opp_id endpoint when DB pool is
    // available. When DATABASE_URL is absent, the route API stays unmounted
    // (fail-honest: no fabrication, api-server gets a connection refused and
    // falls back to A1/A3).
    let app = match &db_pool {
        Some(pool) => {
            let route_state = searcher_rs::route_api::RouteApiState { pool: pool.clone() };
            let route_router = searcher_rs::route_api::route_router(route_state);
            app.merge(route_router)
        }
        None => {
            warn!(
                event = "route_api.not_mounted",
                "DB pool unavailable; /route/:opp_id endpoint disabled (A2 path inactive)"
            );
            app
        }
    };

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

// ---------------------------------------------------------------------------
// Phase 15 tests
// ---------------------------------------------------------------------------

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    // CI-GATE-RELIABILITY (3rd strike 2026-08-16, #343/#344/#346): cargo test
    // runs these in parallel threads, but the five tests below mutate the SAME
    // process-global env vars — a set_var("true") from one race between the
    // remove_var/assert of another and flipped the default-test red (flaky).
    // Serialize every env-mutating test on one mutex (poison-tolerant: a
    // panicked guard must not brick the rest of the suite).
    static ENV_MUTEX: std::sync::Mutex<()> = std::sync::Mutex::new(());

    // ── main::tests::legacy_workers_disabled_by_default ─────────────────────
    //
    // Verifies that, with no env vars set, all three legacy workers are
    // disabled (env flag returns false).

    #[test]
    fn legacy_workers_disabled_by_default() {
        let _guard = ENV_MUTEX.lock().unwrap_or_else(|p| p.into_inner());
        // Ensure the flags are absent (they may be set in the test environment;
        // we remove them here to test the default).
        std::env::remove_var("ARBX_ENABLE_LEGACY_TRIANGULAR_WORKER");
        std::env::remove_var("ARBX_ENABLE_LEGACY_FLASHLOAN_WORKER");
        std::env::remove_var("ARBX_ENABLE_LEGACY_LIQUIDATION_WORKER");

        assert!(
            !legacy_triangular_worker_enabled(),
            "triangular worker must be disabled by default (Phase 15)"
        );
        assert!(
            !legacy_flashloan_arb_worker_enabled(),
            "flashloan_arb worker must be disabled by default (Phase 15)"
        );
        assert!(
            !legacy_liquidation_worker_enabled(),
            "liquidation worker must be disabled by default (Phase 15)"
        );
    }

    // ── main::tests::legacy_triangular_enabled_by_flag ──────────────────────

    #[test]
    fn legacy_triangular_enabled_by_flag() {
        let _guard = ENV_MUTEX.lock().unwrap_or_else(|p| p.into_inner());
        std::env::set_var("ARBX_ENABLE_LEGACY_TRIANGULAR_WORKER", "true");
        assert!(
            legacy_triangular_worker_enabled(),
            "triangular worker must be enabled when env var = 'true'"
        );
        // Also test case-insensitive "TRUE" variant.
        std::env::set_var("ARBX_ENABLE_LEGACY_TRIANGULAR_WORKER", "TRUE");
        assert!(
            legacy_triangular_worker_enabled(),
            "triangular worker must be enabled for 'TRUE' (case-insensitive)"
        );
        std::env::remove_var("ARBX_ENABLE_LEGACY_TRIANGULAR_WORKER");
    }

    // ── main::tests::legacy_worker_state_logged_at_boot ─────────────────────
    //
    // Verifies the LEGACY_WORKER_DISABLED_TOTAL Prometheus counter logic:
    // each disabled worker increments it by 1 with the correct worker label.
    // Label values MUST match the canonical module names.

    #[test]
    fn legacy_worker_disabled_total_counter_logic() {
        let workers = [
            "triangular_worker",
            "flashloan_arb_worker",
            "liquidation_worker",
        ];

        for worker in &workers {
            let before = LEGACY_WORKER_DISABLED_TOTAL
                .with_label_values(&[worker])
                .get();
            LEGACY_WORKER_DISABLED_TOTAL
                .with_label_values(&[worker])
                .inc();
            let after = LEGACY_WORKER_DISABLED_TOTAL
                .with_label_values(&[worker])
                .get();
            assert_eq!(
                after,
                before + 1,
                "LEGACY_WORKER_DISABLED_TOTAL must increment by 1 for worker={worker}"
            );
        }
    }

    // ── main::tests::legacy_flashloan_enabled_by_flag ───────────────────────

    #[test]
    fn legacy_flashloan_enabled_by_flag() {
        let _guard = ENV_MUTEX.lock().unwrap_or_else(|p| p.into_inner());
        std::env::set_var("ARBX_ENABLE_LEGACY_FLASHLOAN_WORKER", "true");
        assert!(legacy_flashloan_arb_worker_enabled());
        std::env::remove_var("ARBX_ENABLE_LEGACY_FLASHLOAN_WORKER");
    }

    // ── main::tests::legacy_liquidation_enabled_by_flag ─────────────────────

    #[test]
    fn legacy_liquidation_enabled_by_flag() {
        let _guard = ENV_MUTEX.lock().unwrap_or_else(|p| p.into_inner());
        std::env::set_var("ARBX_ENABLE_LEGACY_LIQUIDATION_WORKER", "true");
        assert!(legacy_liquidation_worker_enabled());
        std::env::remove_var("ARBX_ENABLE_LEGACY_LIQUIDATION_WORKER");
    }

    // ── main::tests::false_string_keeps_worker_disabled ─────────────────────

    #[test]
    fn false_string_keeps_worker_disabled() {
        let _guard = ENV_MUTEX.lock().unwrap_or_else(|p| p.into_inner());
        std::env::set_var("ARBX_ENABLE_LEGACY_TRIANGULAR_WORKER", "false");
        assert!(!legacy_triangular_worker_enabled());
        std::env::set_var("ARBX_ENABLE_LEGACY_TRIANGULAR_WORKER", "0");
        assert!(!legacy_triangular_worker_enabled());
        std::env::remove_var("ARBX_ENABLE_LEGACY_TRIANGULAR_WORKER");
    }
}
