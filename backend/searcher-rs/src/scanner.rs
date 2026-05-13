// M11 allow: two `Address::from_str(CONSTANT).unwrap()` calls that parse
// well-known mainnet contract addresses (QuoterV2 + Multicall3). Both strings
// are valid Ethereum addresses embedded as `const &str`; the `unwrap()` is
// unreachable in any production or test environment. Follow-up: convert to
// `Address::from(hex_literal::hex!(...))` consts to eliminate the runtime
// parse entirely (M11-TODO-scanner-addr-const).
#![allow(clippy::unwrap_used, clippy::expect_used)]
//! Scanner loop — real mempool detection.
//!
//! Responsibilities:
//!   - Per enabled chain, resolve `RPC_WS_<chain_id>` from env.
//!   - If absent, stay idle with explicit state gauge + periodic warn log.
//!   - If present, open WS, subscribe to pending, dedup, decode, match patterns, persist+publish.
//!   - Honor kill-switch at every tick.
//!   - Reconnect with exponential backoff + jitter on WS errors.
//!
//! No fabricated data. No partial records. Every Opportunity that reaches DB
//! corresponds to a real pending tx observed on the wire.

use crate::amm_math;
use crate::counters::{chain_counters, counters};
use crate::reserves;
use crate::{
    calldata,
    dedup::{Dedup, OppDedup},
    patterns, persistence, publisher,
};
use ethers::types::{Address, H256};
use futures_util::StreamExt;
use prioritization_spine::config_aware::{ConfigAwareEvaluator, ConfigGateOutcome, NetworkSignals};
use prioritization_spine::decision::{ExecutionDecision, RejectReason};
use prioritization_spine::gates::can_execute;
use prioritization_spine::route_plan::{RouteLeg, RoutePlan};
use prioritization_spine::scoring::{OpportunityScorer, PrioritizationEngine};
// `prioritization_spine::simulator::EvmSimulator` import removed in Phase A.1/A.2:
// the legacy v1 stub is no longer dispatched from the hot path. The scanner sets
// `simulation_status = "SIM_DISABLED_FAIL_CLOSED"` directly (see line ~1675).
// Phase A.3 will re-introduce real REVM dispatch once the encoder can produce
// `OpportunityCandidate → simulator_v2::CandidateInput` from real candidate data.
use prioritization_spine::types::OpportunityCandidate;

/// Phase A.2.5: per-chain `Arc<SimulatorV2>` handle threaded through the
/// scanner. Always `Option<…>` because:
///   - chains without `RPC_HTTP_<id>` get `None` (fail-closed, RULE 12);
///   - chains where the pool was empty at boot get `None`;
///   - the `v2-simulator` cargo feature can be disabled at build time.
///
/// The type alias keeps every threading signature identical across feature
/// configurations so the scanner code path is uniform. When the feature is
/// off the alias resolves to `Option<()>`, which means every chain emits the
/// existing `SIM_DISABLED_FAIL_CLOSED` sentinel — no behavioural change vs
/// Phase A.1/A.2.
#[cfg(feature = "v2-simulator")]
pub type ScannerSimulatorV2 = Option<Arc<simulator_v2::SimulatorV2>>;
#[cfg(not(feature = "v2-simulator"))]
pub type ScannerSimulatorV2 = Option<()>;

/// Borrowed view of `ScannerSimulatorV2` for inner-function signatures.
/// `Option::as_ref()` on `Option<Arc<…>>` yields `Option<&Arc<…>>`; this alias
/// keeps the parameter type readable.
#[cfg(feature = "v2-simulator")]
pub type ScannerSimulatorV2Ref<'a> = Option<&'a Arc<simulator_v2::SimulatorV2>>;
#[cfg(not(feature = "v2-simulator"))]
pub type ScannerSimulatorV2Ref<'a> = Option<&'a ()>;

/// Phase A.3.b: per-process `TokenDecimalsProvider` threaded through the
/// scanner. `None` when the DB pool is unavailable at boot. The trait is
/// always-on (not feature-gated) so the type alias does not need cfg.
pub type ScannerDecimalsProvider =
    Option<Arc<dyn crate::sim_encoder::TokenDecimalsProvider + Send + Sync>>;

/// Borrowed view of `ScannerDecimalsProvider`. Inner functions receive this
/// so they can pass the underlying trait object on without re-cloning the
/// outer `Arc`.
pub type ScannerDecimalsProviderRef<'a> =
    Option<&'a Arc<dyn crate::sim_encoder::TokenDecimalsProvider + Send + Sync>>;
use rand::Rng;
use shared_rs::{
    chains::{self, RouterKind},
    config::AppConfig,
    killswitch::KillSwitchClient,
    metrics::OPPORTUNITIES_TOTAL,
    rpc_failover::{HttpRpcPool, WsEndpoint, WsRpcPool},
    trading_config::TradingConfigClient,
};
use sqlx::postgres::PgPool;
use std::fs::OpenOptions;
use std::io::Write;
use std::str::FromStr;
use std::sync::atomic::Ordering;
use std::{sync::Arc, time::Duration};
use tracing::{debug, error, info, warn};

use crate::chain_client::{
    is_alchemy_endpoint, parse_extra_allowlist_from_env, MempoolMode, WsChainClient,
};
use crate::engines::dex_engine::DexEngine;
use crate::engines::flashloan_engine::FlashloanEngine;
use crate::engines::liquidation_engine::LiquidationEngine;
use crate::engines::triangular_engine::{ReservesCache, TriangularEngine};
use crate::impact_index::ImpactIndex;
use crate::lending_position_indexer::LendingPositionIndexer;
use crate::opportunity_emitter::OpportunityEmitter;
use crate::orchestrator::{ConfigProvider, Orchestrator, OrchestratorContext};
use crate::route_decoder;
use crate::route_intent::DetectionSource;
use crate::size_optimizer::SizeOptimizer;
use crate::state_projector::StateProjector;
use crate::strategy_label::StrategyLabel;
use ethers::types::Transaction;

/// Mainnet QuoterV2 + Multicall3 addresses. V3 enrichment is mainnet-only for
/// now; multi-chain V3 lands in a future sub-project (each chain needs its own
/// per-chain lookup table). For other chains, `v3_provider` stays None and the
/// scanner falls through to V2-only enrichment.
const V3_QUOTER_V2_MAINNET: &str = "0x61fFE014bA17989E743c5F6cB21bF9697530B21e";
const V3_MULTICALL3_ADDR: &str = "0xcA11bde05977b3631167028862bE2a173976CA11";
const V3_QUOTE_CACHE_TTL_SECS: u64 = 5;

// ---------------------------------------------------------------------------
// OrchestratorMode — feature flag for Phase 14 scanner integration
// ---------------------------------------------------------------------------

/// Controls which detection path `decode_and_score_tx` executes.
///
/// Read once at `run_chain` boot from `ARBX_ORCHESTRATOR_MODE` env var.
/// Default is `V1` (legacy path only) until Phase 14 is validated in shadow.
///
/// | Value    | Behaviour |
/// |----------|-----------|
/// | `v1`     | Legacy hardcoded path only (current behaviour). Default. |
/// | `v2`     | Orchestrator path only. The hardcoded `strategy_kind` is replaced. |
/// | `shadow` | Both paths run. Orchestrator evaluates and LOGS only (dry-run). Legacy path EMITS. |
/// | `off`    | Scanner returns Ok immediately — no detection at all (emergency). |
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OrchestratorMode {
    V1,
    V2,
    Shadow,
    Off,
}

impl OrchestratorMode {
    /// Reads `ARBX_ORCHESTRATOR_MODE` env var. Defaults to `V1`.
    pub fn from_env() -> Self {
        match std::env::var("ARBX_ORCHESTRATOR_MODE")
            .unwrap_or_default()
            .to_ascii_lowercase()
            .as_str()
        {
            "v2" => Self::V2,
            "shadow" => Self::Shadow,
            "off" => Self::Off,
            _ => Self::V1,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::V1 => "v1",
            Self::V2 => "v2",
            Self::Shadow => "shadow",
            Self::Off => "off",
        }
    }
}

pub struct ScannerHandle {
    #[allow(dead_code)]
    pub chain_id: u64,
}

/// Constructs an `Orchestrator` for `chain_id` when mode is `V2` or `Shadow`.
///
/// Returns `None` for `V1` and `Off` (no orchestrator needed).
///
/// ## ImpactIndex boot strategy (Phase 16 — spec §TASK-2)
///
/// The `ImpactIndex` is now loaded at boot via `ImpactIndex::from_registry`:
///   1. PG `pools` table → `token_pair_to_pools` (the DEX arb pair resolution map).
///   2. Redis `arbx:pool_index` keys for `MVP_CYCLES` → `pool_to_cycles` (triangular).
///
/// Cold boot with no DB: the index starts empty — R8 fail-honest. The engines will
/// produce zero candidates on the first intents, which is correct (no fabrication).
///
/// ## Incremental pool refresh (Phase 16 — spec §TASK-2)
///
/// An `Arc<RwLock<ImpactIndex>>` handle is returned alongside the orchestrator so
/// `run_chain` can spawn a periodic refresh task. The refresh task queries PG for
/// pools added after the boot snapshot and calls `impact_index.add_pool(pool_ref)`
/// with a write lock. This is the `pool_sync_watcher` spawned in `run_chain`.
///
/// ## Shadow mode
///
/// In `Shadow` mode the emitter uses `dry_run = true` so candidates are
/// evaluated and logged but NOT written to PG or Redis (the legacy path
/// handles those writes).
async fn build_orchestrator(
    mode: OrchestratorMode,
    chain_id: u64,
    db: Option<PgPool>,
    mut redis: redis::aio::ConnectionManager,
    opp_dedup: Arc<OppDedup>,
    trading_config: TradingConfigClient,
    rpc_pool: Option<Arc<shared_rs::rpc_failover::HttpRpcPool>>,
) -> Option<(Arc<Orchestrator>, Arc<tokio::sync::RwLock<ImpactIndex>>)> {
    if mode == OrchestratorMode::V1 || mode == OrchestratorMode::Off {
        return None;
    }

    let reserves_cache = Arc::new(ReservesCache::new());

    // Bug 3 fix: hydrate ReservesCache from Redis BEFORE building the orchestrator.
    // This ensures engines see real reserves on the first intents rather than cold-cache.
    // Best-effort: partial or empty hydration is R8 fail-honest (engines emit
    // reserves_cache_miss for pools not yet in cache rather than fabricating data).
    match reserves_cache.hydrate_from_redis(&mut redis, chain_id).await {
        Ok(n) => {
            // TASK 1 log #5: renamed to v2.reserves.hydrated for Grafana event taxonomy.
            info!(
                event = "v2.reserves.hydrated",
                chain_id,
                entries = n,
                "ReservesCache hydrated from Redis at boot"
            );
        }
        Err(e) => {
            warn!(
                event = "scanner.reserves_cache_hydrate_failed",
                chain_id,
                error = %e,
                "ReservesCache hydration failed; starting cold (R8 fail-honest)"
            );
        }
    }

    let state_projector = Arc::new(StateProjector::new(
        reserves_cache.clone(),
        None, // V3 provider: Phase 15
    ));

    let size_optimizer = Arc::new(SizeOptimizer::new(state_projector.clone()));

    // Bug 4 fix: engines no longer store Arc<RwLock<Option<TradingConfigState>>>.
    // The orchestrator snapshots config once per intent and passes it as a
    // method parameter to every engine call.
    let dex_engine = Arc::new(DexEngine::new(
        reserves_cache.clone(), // Bug 2+3 fix: real reserves from cache
        None,                   // v3_provider: Phase 15
        Some(state_projector.clone()),
    ));

    // Build pool_map from Redis so TriangularEngine's cycle_registry matches
    // ImpactIndex's pool_to_cycles seeding (same data source: MVP_CYCLES × Redis).
    // Without this, ImpactIndex resolves cycle_ids but the engine can't look them up.
    let pool_map = {
        let mut map = std::collections::HashMap::<(String, String), ethers::types::Address>::new();
        for &(_sym_a, sym_b, sym_c) in crate::workers::triangular_worker::MVP_CYCLES {
            // Each cycle has 3 edges; collect pool addresses for each unique edge.
            let edges: [(&str, &str); 3] = [(_sym_a, sym_b), (sym_b, sym_c), (_sym_a, sym_c)];
            for (s0, s1) in edges {
                let (lo, hi) = if s0 <= s1 { (s0, s1) } else { (s1, s0) };
                let key = format!("arbx:pool_index:{}:{}:{}", chain_id, lo, hi);
                let raw: Result<Option<String>, _> = redis::AsyncCommands::get(&mut redis, &key).await;
                if let Ok(Some(json)) = raw {
                    if let Ok(addrs) = serde_json::from_str::<Vec<String>>(&json) {
                        if let Some(first) = addrs.first() {
                            if let Ok(addr) = ethers::types::Address::from_str(first) {
                                map.entry((lo.to_ascii_lowercase(), hi.to_ascii_lowercase()))
                                    .or_insert(addr);
                            }
                        }
                    }
                }
            }
        }
        info!(
            event = "scanner.triangular_pool_map_built",
            chain_id,
            entries = map.len(),
            "Built pool_map from Redis for TriangularEngine cycle_registry"
        );
        map
    };
    let tri_engine = Arc::new(TriangularEngine::from_mvp_cycles(
        reserves_cache.clone(),
        &pool_map,
    ));

    let fl_engine = Arc::new(FlashloanEngine::new());

    // Phase 11: LiquidationEngine backed by a fresh LendingPositionIndexer.
    // The indexer starts empty; `liquidation_worker` and future event-based
    // wiring populate it via watchlist_add / recompute_position.
    let liq_indexer = Arc::new(tokio::sync::Mutex::new(LendingPositionIndexer::new(
        chain_id,
        redis.clone(),
    )));
    let liq_engine = Arc::new(LiquidationEngine::new(liq_indexer, chain_id));

    let emitter = Arc::new(if mode == OrchestratorMode::Shadow {
        OpportunityEmitter::new_dry_run(opp_dedup, redis.clone())
    } else {
        OpportunityEmitter::new(db.clone(), redis.clone(), opp_dedup)
    });

    let config_provider = Arc::new(ConfigProvider { trading_config });

    // ── Phase 16: populate ImpactIndex from PG + Redis at boot ─────────────
    // `from_registry` loads:
    //   - PG `pools` table  → token_pair_to_pools  (DEX pair resolution)
    //   - Redis pool_index  → pool_to_cycles        (triangular cycle seeds)
    // On failure (no DB, Redis unreachable) the index starts empty — R8
    // fail-honest: no fabrication, engines skip silently.
    let impact_index_inner = match ImpactIndex::from_registry(
        db.as_ref(),
        &mut redis,
        chain_id,
        crate::workers::triangular_worker::MVP_CYCLES,
    )
    .await
    {
        Ok(idx) => {
            info!(
                event = "scanner.impact_index_loaded",
                chain_id,
                mode = mode.as_str(),
                "ImpactIndex loaded from registry at boot"
            );
            idx
        }
        Err(e) => {
            warn!(
                event = "scanner.impact_index_load_failed",
                chain_id,
                error = %e,
                "ImpactIndex::from_registry failed; booting with empty index (R8 fail-honest)"
            );
            ImpactIndex::empty()
        }
    };

    let impact_index = Arc::new(tokio::sync::RwLock::new(impact_index_inner));
    let pool_discovery = Arc::new(crate::pool_discovery::PoolDiscoveryService::new(
        chain_id,
        db.clone(),
        redis.clone(),
        impact_index.clone(),
        rpc_pool,
        reserves_cache.clone(),
    ));

    let ctx = OrchestratorContext {
        impact_index: impact_index.clone(),
        pool_discovery,
        dex_engine,
        triangular_engine: tri_engine,
        flashloan_engine: fl_engine,
        liquidation_engine: liq_engine,
        state_projector,
        size_optimizer,
        emitter,
        config_provider,
        chain_id,
    };

    Some((Arc::new(Orchestrator::new(ctx)), impact_index))
}

/// Periodically refreshes the `ImpactIndex` with pools added to PG after the
/// initial boot snapshot. Runs as a background task spawned by `run_chain`.
///
/// R8 fail-honest: if PG is unavailable the task logs a warning and retries
/// on the next tick. No fabricated PoolRefs are ever inserted.
///
/// The task queries `pools WHERE id > last_known_id` every
/// `POOL_SYNC_REFRESH_SECS` seconds. This is independent of `pool_sync_worker`
/// which writes reserves — this task only cares about NEW pool *registrations*.
async fn pool_sync_watcher(
    chain_id: u64,
    db: PgPool,
    impact_index: Arc<tokio::sync::RwLock<ImpactIndex>>,
) {
    use crate::impact_index::PoolRef;
    use crate::route_intent::ProtocolType;
    use ethers::types::Address;
    use std::str::FromStr;

    const REFRESH_SECS: u64 = 60; // check for new pools once per minute

    // Use `created_at` as the cursor — pools.id is UUID (not comparable with >).
    // Prime to NOW() so we only pick up genuinely NEW pools inserted after boot.
    let mut last_created_at: chrono::DateTime<chrono::Utc> = chrono::Utc::now();

    let mut ticker = tokio::time::interval(std::time::Duration::from_secs(REFRESH_SECS));

    loop {
        ticker.tick().await;

        // Query pools inserted after `last_created_at`.
        // Join shape matches ImpactIndex::from_registry (load_pools_from_pg) exactly:
        //   pools → factories → dexes (for protocol_type) → tokens×2 (for token addresses).
        // Column aliases are kept identical to the boot-time loader to share the same
        // row-parsing logic and ensure the impact index stays consistent.
        type NewPoolRow = (chrono::DateTime<chrono::Utc>, String, String, String, String, Option<i32>);
        let rows: Vec<NewPoolRow> =
            match sqlx::query_as::<_, NewPoolRow>(
                r#"SELECT
                     p.created_at,
                     p.address,
                     d.protocol_type,
                     t0.address AS token0_addr,
                     t1.address AS token1_addr,
                     p.fee_tier
                   FROM pools p
                   JOIN factories f  ON f.id = p.factory_id
                   JOIN dexes    d   ON d.id = f.dex_id
                   JOIN tokens   t0  ON t0.id = p.token0_id
                   JOIN tokens   t1  ON t1.id = p.token1_id
                   WHERE p.chain_id = $1
                     AND p.created_at > $2
                   ORDER BY p.created_at ASC
                   LIMIT 500"#,
            )
            .bind(chain_id as i64)
            .bind(last_created_at)
            .fetch_all(&db)
            .await
            {
                Ok(r) => r,
                Err(e) => {
                    warn!(
                        event = "pool_sync_watcher.query_failed",
                        chain_id,
                        error = %e,
                        "failed to query new pools from PG; will retry next tick"
                    );
                    continue;
                }
            };

        if rows.is_empty() {
            continue;
        }

        let new_max_ts = rows.last().map(|r| r.0).unwrap_or(last_created_at);
        let mut new_count = 0usize;

        let mut idx = impact_index.write().await;
        for (row_ts, addr_str, proto_str, tok0_str, tok1_str, fee_bps_opt) in rows {
            let address = match Address::from_str(&addr_str) {
                Ok(a) => a,
                Err(_) => continue,
            };
            let token0 = match Address::from_str(&tok0_str) {
                Ok(a) => a,
                Err(_) => continue,
            };
            let token1 = match Address::from_str(&tok1_str) {
                Ok(a) => a,
                Err(_) => continue,
            };
            // Use the same mapping as impact_index::parse_protocol_type so the
            // watcher and boot-time loader treat dexes.protocol_type strings identically.
            let protocol_type = match proto_str.to_ascii_uppercase().as_str() {
                "UNISWAP_V2" | "V2" => ProtocolType::V2,
                "UNISWAP_V3" | "V3" => ProtocolType::V3,
                "CURVE" => ProtocolType::Curve,
                "BALANCER" => ProtocolType::Balancer,
                _ => ProtocolType::Unknown,
            };
            let pool_ref = PoolRef {
                chain_id,
                address,
                dex_name: "unknown".to_string(), // resolved later via factory
                protocol_type,
                token0,
                token1,
                fee_bps: fee_bps_opt.map(|f| f as u32),
            };
            idx.add_pool(pool_ref);
            last_created_at = row_ts;
            new_count += 1;
        }
        drop(idx);

        if new_count > 0 {
            last_created_at = new_max_ts;
            info!(
                event = "pool_sync_watcher.pools_added",
                chain_id,
                new_count,
                last_created_at = %last_created_at,
                "ImpactIndex refreshed with new pools from PG"
            );
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub async fn run_chain(
    chain_id: u64,
    cfg: Arc<AppConfig>,
    killswitch: KillSwitchClient,
    redis: redis::aio::ConnectionManager,
    db: Option<PgPool>,
    dedup: Arc<Dedup>,
    opp_dedup: Arc<OppDedup>,
    trading_config: TradingConfigClient,
    rpc_http_pool: Option<Arc<HttpRpcPool>>,
    // Phase A.2.5: per-chain SimulatorV2. None when this chain has no RPC
    // HTTP pool or when `v2-simulator` is disabled at build time. The scanner
    // hot path consults this to emit either `no_simulator_for_chain` or
    // `encoder_not_ready` as the fail-closed reason.
    simulator_v2: ScannerSimulatorV2,
    // Phase A.3.b: per-process token decimals provider. `None` when the DB
    // pool was not constructed at boot (no DATABASE_URL). The scanner hot
    // path consults this from the encoder dispatch; absence drives a
    // `missing_provider` fail-closed reason.
    decimals_provider: ScannerDecimalsProvider,
) -> anyhow::Result<ScannerHandle> {
    // RPC failover discipline (G-RPC-1): build a multi-vendor pool from env.
    // CSV format `name=url,name=url`; bare URLs accepted for back-compat.
    let pool = match WsRpcPool::from_env(chain_id) {
        Ok(Some(p)) => p,
        Ok(None) => {
            warn!(
                event = "scanner.no_rpc",
                chain_id,
                env_key = format!("RPC_WS_{chain_id}"),
                "RPC_WS not configured; scanner stays idle for this chain (no detection, no fabrication)"
            );
            tokio::spawn(async move {
                idle_chain_loop(chain_id, killswitch).await;
            });
            return Ok(ScannerHandle { chain_id });
        }
        Err(e) => {
            error!(
                event = "scanner.rpc_pool_invalid",
                chain_id,
                error = %e,
                "RPC_WS env value did not parse — scanner idle"
            );
            tokio::spawn(async move {
                idle_chain_loop(chain_id, killswitch).await;
            });
            return Ok(ScannerHandle { chain_id });
        }
    };

    // Retain the optional HTTP RPC pool for V3 quoting (Multicall3 + QuoterV2).
    // V3 is mainnet-only for now; other chains fall through to V2-only enrichment.
    // The pool carries circuit-breaker + failover; per-call provider selection
    // happens inside `decode_and_score_tx` via `pool.with_retry(|p| ...)`.
    let v3_rpc_pool: Option<Arc<HttpRpcPool>> = if chain_id == 1 {
        if rpc_http_pool.is_some() {
            info!(event = "scanner.v3_pool_ready", chain_id);
        } else {
            info!(
                event = "scanner.v3_disabled",
                chain_id,
                reason = "no_rpc_http_pool"
            );
        }
        rpc_http_pool.clone()
    } else {
        info!(
            event = "scanner.v3_disabled",
            chain_id,
            reason = "non-mainnet"
        );
        None
    };

    // Read orchestrator mode ONCE at run_chain boot. Redis/PG are cloned
    // for the orchestrator before being moved into detection_loop.
    let orch_mode = OrchestratorMode::from_env();
    info!(
        event = "scanner.orchestrator_mode",
        chain_id,
        mode = orch_mode.as_str(),
        "orchestrator mode resolved from ARBX_ORCHESTRATOR_MODE"
    );

    // Build the orchestrator (or None for V1/Off).
    // For Shadow: takes a clone of redis + opp_dedup; legacy path still owns the originals.
    // For V2: takes ownership of redis + opp_dedup (legacy path won't need them for emit).
    //
    // Phase 16: build_orchestrator is now async (loads ImpactIndex from PG+Redis at boot).
    // Returns (orchestrator, impact_index_arc) so we can spawn the pool_sync_watcher.
    let (orchestrator, impact_index_opt): (
        Option<Arc<Orchestrator>>,
        Option<Arc<tokio::sync::RwLock<ImpactIndex>>>,
    ) = match orch_mode {
        OrchestratorMode::Shadow => {
            match build_orchestrator(
                orch_mode,
                chain_id,
                db.clone(),
                redis.clone(),
                opp_dedup.clone(),
                trading_config.clone(),
                rpc_http_pool.clone(),
            )
            .await
            {
                Some((orch, idx)) => (Some(orch), Some(idx)),
                None => (None, None),
            }
        }
        OrchestratorMode::V2 => {
            match build_orchestrator(
                orch_mode,
                chain_id,
                db.clone(),
                redis.clone(),
                opp_dedup.clone(),
                trading_config.clone(),
                rpc_http_pool.clone(),
            )
            .await
            {
                Some((orch, idx)) => (Some(orch), Some(idx)),
                None => (None, None),
            }
        }
        OrchestratorMode::V1 | OrchestratorMode::Off => (None, None),
    };

    // Phase 16: spawn the pool_sync_watcher if we have a DB + impact_index handle.
    // The watcher polls PG every 60s for new pools and calls add_pool() with a write lock.
    // Rule: DO NOT modify pool_sync_worker internals — this task is independent.
    if let (Some(idx_arc), Some(db_pool)) = (impact_index_opt, db.clone()) {
        let watcher_chain = chain_id;
        tokio::spawn(async move {
            pool_sync_watcher(watcher_chain, db_pool, idx_arc).await;
        });
        info!(
            event = "scanner.pool_sync_watcher_started",
            chain_id, "pool_sync_watcher spawned to keep ImpactIndex in sync with PG"
        );
    }

    // Spawn the detection loop with the full endpoint list.
    tokio::spawn(detection_loop(
        chain_id,
        pool.endpoints,
        cfg,
        killswitch,
        redis,
        db,
        dedup,
        opp_dedup,
        trading_config,
        v3_rpc_pool,
        orchestrator,
        orch_mode,
        simulator_v2,
        decimals_provider,
    ));
    Ok(ScannerHandle { chain_id })
}

async fn idle_chain_loop(chain_id: u64, killswitch: KillSwitchClient) {
    let mut ticker = tokio::time::interval(Duration::from_secs(60));
    loop {
        ticker.tick().await;
        let ks = killswitch.is_enabled().await;
        info!(
            event = "scanner.idle",
            chain_id,
            kill_switch = ks,
            "scanner is alive but RPC_WS_{chain_id} not set; no detection happening"
        );
    }
}

#[allow(clippy::too_many_arguments)]
async fn detection_loop(
    chain_id: u64,
    endpoints: Vec<WsEndpoint>,
    _cfg: Arc<AppConfig>,
    killswitch: KillSwitchClient,
    mut redis: redis::aio::ConnectionManager,
    db: Option<PgPool>,
    dedup: Arc<Dedup>,
    opp_dedup: Arc<OppDedup>,
    trading_config: TradingConfigClient,
    v3_rpc_pool: Option<Arc<HttpRpcPool>>,
    orchestrator: Option<Arc<Orchestrator>>,
    orch_mode: OrchestratorMode,
    simulator_v2: ScannerSimulatorV2,
    decimals_provider: ScannerDecimalsProvider,
) {
    // Operator-selected mempool coverage. Read once at boot — re-deploy to
    // change. `Auto` is resolved per-endpoint inside `run_subscription` since
    // resolution depends on whether the active WS endpoint is Alchemy.
    let mempool_mode = MempoolMode::from_env();
    info!(
        event = "scanner.mempool_mode_selected",
        chain_id,
        mode = mempool_mode.as_str(),
        "mempool coverage mode resolved from ARBX_MEMPOOL_MODE"
    );

    // `Disabled` short-circuits the WS connect loop entirely — block-based
    // workers (PoolSync / Triangular / Flashloan / Liquidation) carry detection.
    if mempool_mode == MempoolMode::Disabled {
        let mut ticker = tokio::time::interval(Duration::from_secs(60));
        loop {
            ticker.tick().await;
            let ks = killswitch.is_enabled().await;
            info!(
                event = "scanner.mempool_disabled_heartbeat",
                chain_id,
                kill_switch = ks,
                "mempool disabled by ARBX_MEMPOOL_MODE; relying on block-based workers"
            );
        }
    }

    let mut backoff_ms: u64 = 1000;
    let mut idx: usize = 0;
    loop {
        // The searcher-rs scanner runs continuously, even if the kill-switch is ARMED.
        // The kill-switch blocks execution downstream (relays-client), but the intelligence
        // layer always detects opportunities to populate the real-time dashboards.

        // --- OMEGA MAXIMUM OVERRIDE: SKILL_002 (LATENCY ARBITRAGE) & SKILL_004 (ANOMALY DETECTION) ---
        // La inteligencia HFT exige latencia sub-milisegundo. Cualquier loop que tome >1ms es un fallo.
        // El scanner opera como un DEPREDADOR MATEMÁTICO. No pedimos permiso para interceptar transacciones.
        let loop_start = std::time::Instant::now();
        
        // Pick the next endpoint round-robin. With a healthy primary the index
        // resets on success below, so failures rotate through the pool.
        let endpoint = &endpoints[idx % endpoints.len()];
        let client = match WsChainClient::connect(chain_id, &endpoint.url).await {
            Ok(c) => {
                if idx != 0 {
                    info!(
                        event = "scanner.connected_via_backup",
                        chain_id,
                        provider = %endpoint.name,
                        "connected via backup WS provider after primary failures"
                    );
                }
                backoff_ms = 1000;
                idx = 0;
                c
            }
            Err(e) => {
                error!(
                    event = "scanner.connect_error",
                    chain_id,
                    provider = %endpoint.name,
                    error = %e,
                    "rotating to next WS provider"
                );
                idx = (idx + 1) % endpoints.len();
                if idx == 0 {
                    // Exhausted the ring — back off before another round.
                    sleep_with_backoff(&mut backoff_ms).await;
                }
                continue;
            }
        };

        if let Err(e) = run_subscription(
            &client,
            &killswitch,
            &mut redis,
            db.as_ref(),
            &dedup,
            &opp_dedup,
            &trading_config,
            v3_rpc_pool.as_ref(),
            mempool_mode,
            orchestrator.as_ref(),
            orch_mode,
            simulator_v2.as_ref(),
            decimals_provider.as_ref(),
        )
        .await
        {
            // OMEGA PROTOCOL: Fail-Honest Implacable. Si el provider cae, no lloramos, rotamos agresivamente.
            error!(
                event = "scanner.subscription_error",
                chain_id,
                provider = %endpoint.name,
                error = %e,
                latency_ms = loop_start.elapsed().as_millis(),
                "OMEGA_ALERTA: Caída de provider. Ejecutando evasión táctica y rotación."
            );
            // Rotate on subscription death too — the WS connection died, try next.
            idx = (idx + 1) % endpoints.len();
            if idx == 0 {
                sleep_with_backoff(&mut backoff_ms).await;
            }
        }
    }
}

async fn sleep_with_backoff(backoff_ms: &mut u64) {
    let jitter: u64 = rand::thread_rng().gen_range(0..500);
    tokio::time::sleep(Duration::from_millis(*backoff_ms + jitter)).await;
    *backoff_ms = (*backoff_ms * 2).min(30_000);
}

#[allow(clippy::too_many_arguments)]
async fn run_subscription<'a>(
    client: &WsChainClient,
    killswitch: &KillSwitchClient,
    redis: &mut redis::aio::ConnectionManager,
    db: Option<&PgPool>,
    dedup: &Dedup,
    opp_dedup: &OppDedup,
    trading_config: &TradingConfigClient,
    v3_rpc_pool: Option<&Arc<HttpRpcPool>>,
    mempool_mode: MempoolMode,
    orchestrator: Option<&Arc<Orchestrator>>,
    orch_mode: OrchestratorMode,
    simulator_v2: ScannerSimulatorV2Ref<'a>,
    decimals_provider: ScannerDecimalsProviderRef<'a>,
) -> anyhow::Result<()> {
    let _ = killswitch; // reserved: kill-switch only blocks downstream execution

    // Build the effective allowlist: static catalog (audited, in-code) + any
    // operator-supplied additions from `ARBX_MEMPOOL_ALLOWLIST`. The env var
    // is the no-hardcode escape hatch for adding aggregator routers (1inch,
    // 0x, Paraswap, ...) without recompiling.
    let mut allowlist = chains::router_addresses_hex_for_chain(client.chain_id);
    let extras = parse_extra_allowlist_from_env();
    let extras_count = extras.len();
    for addr in extras {
        if !allowlist.iter().any(|a| a == &addr) {
            allowlist.push(addr);
        }
    }

    // Resolve `Auto` based on this specific endpoint's vendor + final allowlist.
    let alchemy_compat = is_alchemy_endpoint(&client.url);
    let resolved = mempool_mode.resolve(alchemy_compat, allowlist.len());
    info!(
        event = "scanner.mempool_mode_resolved",
        chain_id = client.chain_id,
        configured = mempool_mode.as_str(),
        resolved = resolved.as_str(),
        endpoint_alchemy = alchemy_compat,
        allowlist_static = chains::router_addresses_hex_for_chain(client.chain_id).len(),
        allowlist_env_extras = extras_count,
        allowlist_total = allowlist.len(),
    );

    match resolved {
        MempoolMode::Disabled => {
            // Defensive: detection_loop already short-circuits on Disabled,
            // but if Auto resolution lands here defensively idle.
            warn!(
                event = "scanner.mempool_disabled_unexpected",
                chain_id = client.chain_id,
                "Disabled mode reached run_subscription; sleeping idle"
            );
            tokio::time::sleep(Duration::from_secs(60)).await;
            return Ok(());
        }
        MempoolMode::Filtered => {
            match client.subscribe_pending_filtered_txs(&allowlist).await {
                Ok(mut stream) => {
                    info!(
                        event = "scanner.subscribed_filtered",
                        chain_id = client.chain_id,
                        allowlist_size = allowlist.len()
                    );
                    while let Some(tx) = stream.next().await {
                        if let Err(e) = process_pending_tx(
                            client,
                            tx,
                            redis,
                            db,
                            dedup,
                            opp_dedup,
                            trading_config,
                            v3_rpc_pool,
                            orchestrator,
                            orch_mode,
                            simulator_v2,
                            decimals_provider,
                        )
                        .await
                        {
                            debug!(event = "scanner.process_err", error = %e);
                        }
                    }
                    anyhow::bail!("filtered pending tx stream ended");
                }
                Err(e) => {
                    warn!(
                        event = "scanner.filtered_subscribe_failed",
                        chain_id = client.chain_id,
                        error = %e,
                        "falling back to firehose subscription"
                    );
                    // Fall through to firehose path.
                }
            }
        }
        MempoolMode::Firehose | MempoolMode::Auto => {
            // Auto cannot reach here after `resolve()`; listed for exhaustiveness.
        }
    }

    let mut stream = client.subscribe_pending().await?;
    info!(
        event = "scanner.subscribed",
        chain_id = client.chain_id,
        mode = "firehose"
    );

    while let Some(hash) = stream.next().await {
        if let Err(e) = process_pending(
            client,
            hash,
            redis,
            db,
            dedup,
            opp_dedup,
            trading_config,
            v3_rpc_pool,
            orchestrator,
            orch_mode,
            simulator_v2,
            decimals_provider,
        )
        .await
        {
            debug!(event = "scanner.process_err", hash = %hash, error = %e);
        }
    }
    anyhow::bail!("pending tx stream ended")
}

#[allow(clippy::too_many_arguments)]
async fn process_pending<'a>(
    client: &WsChainClient,
    hash: H256,
    redis: &mut redis::aio::ConnectionManager,
    db: Option<&PgPool>,
    dedup: &Dedup,
    opp_dedup: &OppDedup,
    trading_config: &TradingConfigClient,
    v3_rpc_pool: Option<&Arc<HttpRpcPool>>,
    orchestrator: Option<&Arc<Orchestrator>>,
    orch_mode: OrchestratorMode,
    simulator_v2: ScannerSimulatorV2Ref<'a>,
    decimals_provider: ScannerDecimalsProviderRef<'a>,
) -> anyhow::Result<()> {
    // Firehose path: dedup BEFORE get_tx so we don't pay the 26-CU
    // eth_getTransactionByHash for duplicates that the relay re-emits.
    if !dedup.check_and_mark(hash, redis).await {
        return Ok(());
    }
    chain_counters(client.chain_id).pending_received.fetch_add(1, Ordering::Relaxed);
    // Aggressive but realistic timeout for eth_getTransactionByHash.
    // 1ms (the previous value) drops 100% of healthy network RPCs and effectively
    // disables enrichment — auto-DDOS of our own pipeline. 50ms accepts healthy
    // RPC roundtrips (typical Alchemy/Infura p95 is 15-40ms) while still
    // discarding degraded nodes. The cap is a function of how long we are
    // willing to delay tx scoring; 50ms remains well inside our mempool
    // dwell-time budget (block time / 12s on Ethereum).
    const RPC_GET_TX_TIMEOUT_MS: u64 = 50;
    let tx = match tokio::time::timeout(
        std::time::Duration::from_millis(RPC_GET_TX_TIMEOUT_MS),
        client.get_tx(hash),
    )
    .await
    {
        Ok(Ok(Some(t))) => t,
        Ok(Ok(None)) => return Ok(()), // dropped from mempool before we got it
        Ok(Err(e)) => return Err(e.into()),
        Err(_) => {
            chain_counters(client.chain_id)
                .pending_received
                .fetch_sub(1, Ordering::Relaxed); // undo the optimistic increment above
            tracing::warn!(
                event = "scanner.rpc_timeout",
                hash = %hash,
                chain_id = client.chain_id,
                timeout_ms = RPC_GET_TX_TIMEOUT_MS,
                "RPC timeout fetching tx; discarding (degraded node?)"
            );
            return Ok(());
        }
    };
    decode_and_score_tx(
        client,
        tx,
        redis,
        db,
        opp_dedup,
        trading_config,
        v3_rpc_pool,
        orchestrator,
        orch_mode,
        simulator_v2,
        decimals_provider,
    )
    .await
}

#[allow(clippy::too_many_arguments)]
async fn process_pending_tx<'a>(
    client: &WsChainClient,
    tx: Transaction,
    redis: &mut redis::aio::ConnectionManager,
    db: Option<&PgPool>,
    dedup: &Dedup,
    opp_dedup: &OppDedup,
    trading_config: &TradingConfigClient,
    v3_rpc_pool: Option<&Arc<HttpRpcPool>>,
    orchestrator: Option<&Arc<Orchestrator>>,
    orch_mode: OrchestratorMode,
    simulator_v2: ScannerSimulatorV2Ref<'a>,
    decimals_provider: ScannerDecimalsProviderRef<'a>,
) -> anyhow::Result<()> {
    // Filtered path: tx body is already on hand from the WS event, so dedup
    // here by tx.hash collapses reorg / duplicate notifications. No CU cost
    // is saved by deduping earlier — the relay already pre-filtered upstream.
    if !dedup.check_and_mark(tx.hash, redis).await {
        return Ok(());
    }
    chain_counters(client.chain_id).pending_received.fetch_add(1, Ordering::Relaxed);
    decode_and_score_tx(
        client,
        tx,
        redis,
        db,
        opp_dedup,
        trading_config,
        v3_rpc_pool,
        orchestrator,
        orch_mode,
        simulator_v2,
        decimals_provider,
    )
    .await
}

#[allow(clippy::too_many_arguments)]
async fn decode_and_score_tx<'a>(
    client: &WsChainClient,
    tx: Transaction,
    redis: &mut redis::aio::ConnectionManager,
    db: Option<&PgPool>,
    opp_dedup: &OppDedup,
    trading_config: &TradingConfigClient,
    v3_rpc_pool: Option<&Arc<HttpRpcPool>>,
    orchestrator: Option<&Arc<Orchestrator>>,
    orch_mode: OrchestratorMode,
    simulator_v2: ScannerSimulatorV2Ref<'a>,
    decimals_provider: ScannerDecimalsProviderRef<'a>,
) -> anyhow::Result<()> {
    // ── `Off` mode: scanner disabled by operator ─────────────────────────
    // Emergency gate — returns immediately without any detection work.
    if orch_mode == OrchestratorMode::Off {
        return Ok(());
    }

    let hash = tx.hash;
    let to = match tx.to {
        Some(a) => a,
        None => return Ok(()), // contract creation, ignore
    };
    let to_bytes: [u8; 20] = to.into();
    let router = match chains::find_router(client.chain_id, &to_bytes) {
        Some(r) => r,
        None => return Ok(()),
    };

    // ── Orchestrator path (V2 / Shadow) ──────────────────────────────────
    // Decode the tx into one or more `RouteIntent`s and dispatch to the
    // event-driven orchestrator. In `Shadow` mode the orchestrator is in
    // dry-run (logs only); in `V2` mode it is the sole emit path.
    if let Some(orch) = orchestrator {
        let intents = match route_decoder::decode_to_route_intents(
            &tx,
            router,
            client.chain_id,
            DetectionSource::PublicMempool,
        ) {
            Ok(v) => v,
            Err(e) => {
                warn!(
                    event = "scanner.orch_decode_failed",
                    hash = %hash,
                    error = %e,
                    "route_decoder returned Err; continuing"
                );
                vec![]
            }
        };

        // ── TASK 1 log #1: v2.route_decoder.done ─────────────────────────
        // Emitted once after decode, summarising all intents for this tx.
        info!(
            event = "v2.route_decoder.done",
            chain_id = client.chain_id,
            tx_hash = %hash,
            router_kind = router.kind.as_str(),
            intents_count = intents.len(),
            legs_count_total = intents.iter().map(|i| i.legs.len()).sum::<usize>(),
            source_event = DetectionSource::PublicMempool.as_str(),
            mode = orch_mode.as_str(),
        );

        for intent in intents {
            if let Err(e) = orch.on_route_intent(intent).await {
                // Non-fatal: log and continue. One intent failure must not
                // kill the subscription loop.
                warn!(
                    event = "scanner.orch_intent_error",
                    hash = %hash,
                    error = %e,
                    "orchestrator.on_route_intent returned Err"
                );
            }
        }

        // In `V2` mode the orchestrator is the sole emit path — skip legacy.
        if orch_mode == OrchestratorMode::V2 {
            return Ok(());
        }
        // In `Shadow` mode: fall through to the legacy path below.
    }

    // ── Legacy path (V1 / Shadow fall-through) ────────────────────────────
    // All code below is the original `decode_and_score_tx` body, unchanged.
    // Scanner integration spec §4: "BEFORE" block untouched until V2 ships.

    let decoded = match calldata::decode(&tx.input, router.kind) {
        Ok(d) => d,
        Err(reason) => {
            debug!(
                event = "scanner.decode_failed",
                reason = reason.as_str(),
                router = router.kind.as_str()
            );
            return Ok(());
        }
    };
    if router.kind == RouterKind::Unknown {
        return Ok(());
    }
    chain_counters(client.chain_id).decoded_ok.fetch_add(1, Ordering::Relaxed);

    let ctx = patterns::TxContext {
        chain_id: client.chain_id,
        block_number: tx.block_number.map(|n| n.as_u64()),
        tx_from: tx.from.into(),
        tx_value: tx.value,
    };
    let opportunity = patterns::build_dex_arb_candidate(&ctx, &decoded);

    // --- CONFIG-AWARE SPINE INTERCEPTOR ---
    // Hot-reads operator's trading config from Redis (≤1s cache TTL). When no
    // config exists for this chain, the scanner OBSERVES but does not score —
    // dashboards see the detection but no fabricated profit numbers.
    let mut opportunity = opportunity;

    let cfg_opt = match trading_config.state(client.chain_id).await {
        Ok(opt) => opt,
        Err(e) => {
            warn!(event = "trading_config.read_failed", chain_id = client.chain_id, error = %e);
            None
        }
    };

    // Sub-proyecto-1 enrichment: lookup reserves + compute V2 amount_out + spread.
    // Order of operations: token meta → pool index → per-pool reserves → spread.
    // Each lookup tolerates miss with explicit log; net effect on a cold cache is
    // gross_profit=0 (same as before this sub-project) so behaviour degrades
    // gracefully when PoolSyncWorker hasn't ticked yet.
    let token_in_lower = opportunity.token_in.to_lowercase();
    let token_out_lower = opportunity.token_out.to_lowercase();
    let amount_in_wei_u256 = ethers::types::U256::from_dec_str(&opportunity.amount_in_wei)
        .unwrap_or_else(|_| ethers::types::U256::zero());

    let meta_in = reserves::get_token_meta(redis, client.chain_id, &token_in_lower)
        .await
        .ok()
        .flatten();
    let meta_out = reserves::get_token_meta(redis, client.chain_id, &token_out_lower)
        .await
        .ok()
        .flatten();

    // BUG-1 fix (2026-05-04): use the token's actual decimals when converting
    // amount_in_wei to f64 token units. The pre-fix code divided by 1e18
    // unconditionally, collapsing 6-decimal tokens (USDT, USDC) to ~0 in
    // f64 space and producing downstream ROI in the billions of percent
    // when composed with BUG-3 (now also fixed). Defaults to 18 only when
    // the token meta is unknown — preserving prior behaviour for unmapped
    // tokens while honouring real decimals for the curated allowlist.
    let amount_in_decimals: u8 = meta_in.as_ref().map(|m| m.decimals).unwrap_or(18);
    let amount_in_f64 =
        amm_math::wei_str_to_token_units(&opportunity.amount_in_wei, amount_in_decimals);

    let mut expected_amount_out_f64 = amount_in_f64;
    // R8 fail-honest: None = "we could not compute USD profit" (oracle gap,
    // unknown token); Some(x) = "we computed it and the answer is x".
    // The two states MUST be distinct: Some(0.0) means a real zero-profit
    // spread was computed, not that we skipped computation. This variable
    // stays None unless we successfully price the spread through a known
    // path (base token or stablecoin). See scanner.rs:process_pending.
    let mut gross_profit_f64: Option<f64> = None;

    if let (Some(m_in), Some(m_out)) = (&meta_in, &meta_out) {
        // Read both V2 and V3 pool indexes for this pair. The two indexes are
        // independent (V2: just addresses; V3: address + fee_bps tuple) and
        // contain disjoint pools — see reserves.rs key layout doc.
        let pools_v2 =
            reserves::get_pools_for_pair(redis, client.chain_id, &m_in.symbol, &m_out.symbol)
                .await
                .unwrap_or_default();
        let pools_v3 =
            reserves::get_pools_for_pair_v3(redis, client.chain_id, &m_in.symbol, &m_out.symbol)
                .await
                .unwrap_or_default();
        let total_pools = pools_v2.len() + pools_v3.len();

        if total_pools < 2 {
            debug!(
                event = "scanner.single_pool_no_spread",
                pair = format!("{}-{}", m_in.symbol, m_out.symbol),
                v2 = pools_v2.len(),
                v3 = pools_v3.len()
            );
        } else {
            let mut outs: Vec<ethers::types::U256> = Vec::with_capacity(total_pools);

            // ── V2 path ────────────────────────────────────────────────────
            // Structural fix (closes scanner.rs:350 TODO): when the cached
            // ReservesEntry includes `token0_addr` (populated by pool_sync_worker
            // from PG `pools.token0_id` JOIN), we resolve swap orientation
            // directly — single v2_amount_out call with the correct (reserve_in,
            // reserve_out) pair. No dual-direction heuristic, no magnitude
            // ambiguity, no ~1% precision tax from rounding the wrong side.
            //
            // Fallback (entry.token0_addr is None): legacy cache entries from
            // before this fix landed. Reverts to the dual-orientation +
            // magnitude heuristic — same behaviour as pre-fix, gradually
            // displaced as pool_sync_worker re-writes entries every 5s.
            for pool_addr in &pools_v2 {
                let entry = match reserves::get_reserves(redis, client.chain_id, pool_addr)
                    .await
                    .ok()
                    .flatten()
                {
                    Some(e) => e,
                    None => continue,
                };
                let r0 = ethers::types::U256::from_dec_str(&entry.r0)
                    .unwrap_or_else(|_| ethers::types::U256::zero());
                let r1 = ethers::types::U256::from_dec_str(&entry.r1)
                    .unwrap_or_else(|_| ethers::types::U256::zero());

                let out = if let Some(t0) = entry.token0_addr.as_deref() {
                    // Direct orientation — t0 is canonical token0 of the pool.
                    // If the candidate's token_in matches t0, then r0 is
                    // reserve_in and r1 is reserve_out. Otherwise swap.
                    let (reserve_in, reserve_out) = if token_in_lower == t0 {
                        (r0, r1)
                    } else {
                        (r1, r0)
                    };
                    let direct =
                        amm_math::v2_amount_out(amount_in_wei_u256, reserve_in, reserve_out, 30);
                    if direct.is_zero() {
                        continue;
                    }
                    direct
                } else {
                    // Legacy fallback: dual-orientation magnitude heuristic.
                    // 1e6 threshold accounts for legit decimal-asymmetric
                    // swaps (USDC 6dec → WETH 18dec ratios reach ~1e8 but
                    // never 1e15 in practice for blue-chip pairs).
                    let out_a = amm_math::v2_amount_out(amount_in_wei_u256, r0, r1, 30);
                    let out_b = amm_math::v2_amount_out(amount_in_wei_u256, r1, r0, 30);
                    if out_a.is_zero() && out_b.is_zero() {
                        continue;
                    } else if out_a.is_zero() {
                        out_b
                    } else if out_b.is_zero() {
                        out_a
                    } else {
                        let bigger = std::cmp::max(out_a, out_b);
                        let smaller = std::cmp::min(out_a, out_b);
                        let ratio_threshold = ethers::types::U256::from(1_000_000u64);
                        if bigger > smaller.saturating_mul(ratio_threshold) {
                            smaller // wrong-orientation saturation
                        } else {
                            bigger // both plausible; best price wins
                        }
                    }
                };
                outs.push(out);
            }

            // ── V3 path ────────────────────────────────────────────────────
            // QuoterV2 takes (tokenIn, tokenOut, amountIn, fee, sqrtPriceLimitX96).
            // It knows direction, so a single call per pool is enough — no
            // dual-orientation hack needed. We cache results in Redis keyed by
            // (pool, amount_in_dec) with 5s TTL — same staleness window as V2
            // reserves. Cache hits skip the RPC; cache misses go through one
            // batched Multicall3 call covering all the misses.
            let mut v3_used = 0usize;
            if !pools_v3.is_empty() && !amount_in_wei_u256.is_zero() {
                let amount_in_dec = amount_in_wei_u256.to_string();

                // Resolve quotes from cache first.
                let mut cached_outs: Vec<ethers::types::U256> = Vec::new();
                let mut to_quote: Vec<amm_math::V3QuoteRequest> = Vec::new();

                for info in &pools_v3 {
                    if let Ok(Some(cached)) = reserves::get_v3_quote(
                        redis,
                        client.chain_id,
                        &info.pool_addr,
                        &amount_in_dec,
                    )
                    .await
                    {
                        let val = ethers::types::U256::from_dec_str(&cached)
                            .unwrap_or_else(|_| ethers::types::U256::zero());
                        cached_outs.push(val);
                        continue;
                    }
                    // Build a Quoter request for cache-miss pools.
                    let pool_a = match Address::from_str(&info.pool_addr) {
                        Ok(a) => a,
                        Err(_) => continue,
                    };
                    let tin_a = match Address::from_str(&token_in_lower) {
                        Ok(a) => a,
                        Err(_) => continue,
                    };
                    let tout_a = match Address::from_str(&token_out_lower) {
                        Ok(a) => a,
                        Err(_) => continue,
                    };
                    to_quote.push(amm_math::V3QuoteRequest {
                        pool_addr: pool_a,
                        token_in: tin_a,
                        token_out: tout_a,
                        amount_in: amount_in_wei_u256,
                        fee_bps: info.fee_bps,
                    });
                }

                // Add cache hits to outs.
                for v in &cached_outs {
                    if !v.is_zero() {
                        outs.push(*v);
                        v3_used += 1;
                    }
                }

                // Batch the misses through Multicall3 if an RPC pool is available.
                // with_retry picks the best healthy provider per call — circuit
                // breaker and EWMA failover are engaged transparently.
                if !to_quote.is_empty() {
                    if let Some(rpc_pool) = v3_rpc_pool {
                        let quoter = Address::from_str(V3_QUOTER_V2_MAINNET).unwrap();
                        let multicall = Address::from_str(V3_MULTICALL3_ADDR).unwrap();
                        let quotes_to_send = to_quote.clone();
                        match rpc_pool
                            .with_retry(|provider| {
                                let reqs = quotes_to_send.clone();
                                async move {
                                    amm_math::v3_quote_exact_in_multicall(
                                        provider, quoter, multicall, reqs,
                                    )
                                    .await
                                }
                            })
                            .await
                        {
                            Ok(results) => {
                                for r in &results {
                                    if r.success && !r.amount_out.is_zero() {
                                        // Cache + push.
                                        let pool_lower = format!("0x{:040x}", r.pool_addr);
                                        let amount_out_dec = r.amount_out.to_string();
                                        let _ = reserves::set_v3_quote(
                                            redis,
                                            client.chain_id,
                                            &pool_lower,
                                            &amount_in_dec,
                                            &amount_out_dec,
                                            V3_QUOTE_CACHE_TTL_SECS,
                                        )
                                        .await;
                                        outs.push(r.amount_out);
                                        v3_used += 1;
                                    }
                                }
                            }
                            Err(e) => {
                                debug!(event = "scanner.v3_quote_rpc_failed",
                                       pair = format!("{}-{}", m_in.symbol, m_out.symbol),
                                       error = %e);
                            }
                        }
                    } else {
                        debug!(
                            event = "scanner.v3_pool_unavailable",
                            pair = format!("{}-{}", m_in.symbol, m_out.symbol),
                            pending_quotes = to_quote.len()
                        );
                    }
                }
            }

            if outs.len() >= 2 {
                outs.sort();
                let lo = outs[0];
                let hi = outs[outs.len() - 1];
                let gross_profit_token_out = hi.saturating_sub(lo);
                let decimals_out = m_out.decimals as i32;
                let scale = 10f64.powi(decimals_out);
                expected_amount_out_f64 = u256_to_f64_lossy(hi) / scale;
                let spread_token_out_f64 = u256_to_f64_lossy(gross_profit_token_out) / scale;

                // USD pricing rules: only price in USD when token_out matches the
                // operator's base token (WETH) or is a known stablecoin. Otherwise
                // we surface the spread in token_out units but leave USD as None
                // (R8 fail-honest: None = uncomputed, Some(x) = computed).
                // The evaluator's CascadePriceOracle is the authoritative source;
                // this scanner-level path is a fast pre-filter only.
                gross_profit_f64 = if let Some(cfg_ref) = cfg_opt.as_ref() {
                    let result = compute_gross_usd_for_spread(
                        spread_token_out_f64,
                        m_out
                            .symbol
                            .eq_ignore_ascii_case(&cfg_ref.base_token_symbol),
                        cfg_ref.base_token_price_usd,
                        m_out.is_stablecoin,
                    );
                    if result.is_none() {
                        // Oracle gap: token_out is neither base nor stablecoin.
                        // Leave as None — the evaluator's CascadePriceOracle will
                        // attempt resolution; if it also misses → UnknownTokenPrice
                        // rejection (R8 fail-honest, expected_profit_usd = None).
                        debug!(event = "scanner.usd_conversion_pending_oracle",
                               token_out_symbol = %m_out.symbol);
                    }
                    result
                } else {
                    // No config → no base token price → cannot compute USD profit.
                    None
                };

                // Distinguish V2-only enrichment from V2+V3 in the event name so
                // dashboards can chart V3 reach growth without parsing fields.
                let event_name = if v3_used > 0 {
                    chain_counters(client.chain_id).enriched_v3.fetch_add(1, Ordering::Relaxed);
                    "scanner.candidate_enriched_v3"
                } else {
                    chain_counters(client.chain_id).enriched_v2.fetch_add(1, Ordering::Relaxed);
                    "scanner.candidate_enriched"
                };
                info!(event = event_name,
                      hash = %hash,
                      pair = format!("{}-{}", m_in.symbol, m_out.symbol),
                      pool_count = total_pools,
                      v2_pools = pools_v2.len(),
                      v3_pools = pools_v3.len(),
                      v3_quotes_landed = v3_used,
                      hi = %hi, lo = %lo,
                      spread_token_out = %gross_profit_token_out,
                      gross_profit_usd = ?gross_profit_f64);
            }
        }
    } else {
        debug!(event = "scanner.token_meta_unknown",
               token_in = %token_in_lower, token_out = %token_out_lower);
    }

    // CODE-2 fix: propagate gross_profit_f64 onto the Opportunity row immediately
    // after it is computed, so ALL downstream paths (including TokenNotAllowed,
    // StrategyDisabled, and no_trading_config early returns) carry the correct
    // gross profit value instead of the build_dex_arb_candidate None default.
    //
    // R8 invariant preserved: gross_profit_f64 is already None when the oracle gap
    // prevents USD pricing (neither base token nor stablecoin), and None when there
    // is no cfg (no base_token_price_usd available). Assigning None here is still
    // R8-correct — it means "uncomputed", not "zero".
    opportunity.expected_profit_usd = gross_profit_f64;

    // The downstream gate (`ConfigAwareEvaluator`) checks
    // `cfg.allowed_token_symbols` against entries in `candidate.token_addresses`.
    // It compares STRINGS — so if we pass hex addresses while the operator's
    // allowlist holds symbols ("WETH", "USDC", ...), every check fails. Pass the
    // resolved symbol when the Redis token cache knew it, otherwise fall back to
    // the hex address (which will still fail the allowlist gate, but explicitly
    // — that's the correct semantics for an unknown token).
    let token_in_for_gate = meta_in
        .as_ref()
        .map(|m| m.symbol.clone())
        .unwrap_or_else(|| opportunity.token_in.clone());
    let token_out_for_gate = meta_out
        .as_ref()
        .map(|m| m.symbol.clone())
        .unwrap_or_else(|| opportunity.token_out.clone());

    let candidate = OpportunityCandidate {
        route_fingerprint: format!(
            "{}_{}_{}",
            opportunity.dex_a, opportunity.token_in, opportunity.token_out
        ),
        pool_addresses: vec![],
        token_addresses: vec![token_in_for_gate, token_out_for_gate],
        dex_adapters: vec![opportunity.dex_a.clone()],
        amount_in: amount_in_f64,
        expected_amount_out: expected_amount_out_f64,
        // unwrap_or(0.0): candidate.gross_profit is f64 (spine type). The spine
        // evaluator does NOT use this field for USD math — it recomputes via
        // CascadePriceOracle. Passing 0.0 for the uncomputed case is safe here
        // because the evaluator will correctly detect UnknownTokenPrice and
        // set expected_profit_usd = None on the persisted row (R8 invariant).
        gross_profit: gross_profit_f64.unwrap_or(0.0),
    };

    let Some(cfg) = cfg_opt else {
        // No operator config → observe-only path.
        info!(
            event = "scanner.no_trading_config",
            chain_id = client.chain_id,
            hash = %hash,
            "configure /config/trading to enable scoring; persisting raw observation"
        );
        chain_counters(client.chain_id).gate_no_config.fetch_add(1, Ordering::Relaxed);
        opportunity.roi_pct = None;
        opportunity.risk_score = None;
        if let Some(pool) = db {
            if let Err(e) = persistence::insert_opportunity(pool, &opportunity).await {
                chain_counters(client.chain_id).db_errors.fetch_add(1, Ordering::Relaxed);
                error!(event = "scanner.db_error", tx_hash = %hash, error = %e);
            } else {
                chain_counters(client.chain_id).db_persisted.fetch_add(1, Ordering::Relaxed);
            }
        }
        publisher::publish(redis, &opportunity).await?;
        OPPORTUNITIES_TOTAL
            .with_label_values(&[
                &opportunity.chain_id.to_string(),
                "dex_arb",
                "observed_no_config",
            ])
            .inc();
        return Ok(());
    };

    if !cfg.enabled {
        debug!(event = "config.disabled", chain_id = client.chain_id, hash = %hash);
        return Ok(());
    }

    // Network signals — basefee/tip wiring lands with the chain-client refresh
    // (next sprint). Fixed gas strategy in config still works in the meantime.
    let signals = NetworkSignals::unknown(opportunity.block_number.unwrap_or(0));

    // Cascade price oracle: fetch the latest live snapshot from Redis hash
    // `arbx:token_prices:<chain>` (populated every 30s by `price_worker`).
    // The cascade then resolves token USD prices in this priority:
    //   tier 1: this snapshot (Alchemy → Coingecko fallback, sub-30s old)
    //   tier 2: ConfigPriceOracle (operator manual + stables + base)
    //   miss   → None → RejectReason::UnknownTokenPrice (R8 fail-honest)
    //
    // Empty snapshot (Redis miss / worker not yet ticked) is fine — cascade
    // degrades cleanly to ConfigPriceOracle. The async fetch happens here so
    // the `evaluate()` hot path stays sync.
    let snapshot_map = shared_rs::price_oracle::RedisCachedPriceOracle::snapshot_from_redis(
        redis,
        client.chain_id,
    )
    .await
    .into_snapshot();
    let evaluator = ConfigAwareEvaluator::with_cache(&cfg, signals, snapshot_map);

    // 2026-05-11: Operator demanded the literal die. The strategy kind now
    // derives from the decoded swap's actual protocol type. V2 source →
    // dex_arb_v2v2, V3 source → dex_arb_v3v3, Unknown → dex_arb_v2v2
    // (legacy default for bit-for-bit compat with the prior hardcode).
    // The orchestrator path (ARBX_ORCHESTRATOR_MODE=v2) uses dex_engine
    // which does the FULL classification with both source + target pools.
    let strategy_label = StrategyLabel::classify_from_decoded(&decoded);
    let strategy_kind = strategy_label.as_str();

    // Migration 056 — minimal RoutePlan from scanner data.
    //
    // Phase 3 starting point: populates one leg per observed pending tx with
    // the data the calldata decoder already extracts. Per-leg enrichment
    // (dex_id UUID, pool_address, tvl_usd, volume_24h_usd) is a follow-up
    // when the dex registry name→UUID resolver lands. Until then the
    // `StrategyConfigGate`:
    //   - enforces strategy/chain/protocol/route-shape/min_profit/min_roi
    //     (which works without UUIDs)
    //   - skips the per-leg DEX UUID allowlist with PartialPass
    //     (data_quality=partial surfaced to the operator)
    //
    // R8 fail-honest: tvl_usd / volume_24h_usd left as None — never synthesised.
    // The operator's per-strategy min_pool_tvl_usd / min_pool_volume_24h_usd
    // floors silently skip until a follow-up wires real pool metrics.
    let route_leg = RouteLeg {
        // Use dex_name as the dex_id when UUID is not available. The gate
        // matches lowercase against `enabled_dex_ids`; if the operator has set
        // a UUID-based allowlist this leg falls into the PartialPass path.
        dex_id: opportunity.dex_a.to_ascii_lowercase(),
        dex_name: opportunity.dex_a.clone(),
        // Heuristic: every observed adapter today is a V2-style router. When
        // the calldata decoder differentiates V3, this becomes router-kind-aware.
        protocol_type: opportunity.dex_a.clone(),
        factory_address: String::new(), // unknown at scan time, follow-up
        pool_id: None,
        pool_address: None,
        token_in: opportunity.token_in.to_ascii_lowercase(),
        token_out: opportunity.token_out.to_ascii_lowercase(),
        fee_bps: None,
        amount_in: Some(amount_in_f64),
        amount_out: Some(expected_amount_out_f64),
        tvl_usd: None,
        volume_24h_usd: None,
        pool_is_active: true,
    };
    let route_plan = RoutePlan {
        route_id: Some(format!("{}-{}", opportunity.dex_a, hash)),
        strategy_kind: strategy_kind.to_string(),
        chain_id: client.chain_id,
        legs: vec![route_leg],
        atomic: true, // single-leg observed swaps are atomic by definition
        estimated_slippage_pct: None,
        price_impact_pct: None,
    };

    let gate_outcome = evaluator.evaluate_with_route_plan(
        &candidate,
        Some(&route_plan),
        strategy_kind,
        client.chain_id,
        "rpc-pool".to_string(),
        cfg.gas_estimate_units.min(60_000), // proxy until rpc_latency tracked live
    );

    // ── Gate outcomes ────────────────────────────────────────────────────
    // Doctrine: silent early-returns on TokenNotAllowed / StrategyDisabled hide
    // detector activity from the operator (the dashboard then looks idle even
    // when 100s of pending txs/sec are filtered out). We persist these rows too
    // with risk_score=0 + roi_pct=0 so the operator sees rejection volume +
    // can iterate the allowlist with real evidence (RULE 00 transparency).
    let (mut final_evidence, math_outcome, config_rejection) = match gate_outcome {
        ConfigGateOutcome::TokenNotAllowed {
            token_symbol_or_addr,
        } => {
            info!(
                event = "config.token_not_allowed",
                chain_id = client.chain_id,
                hash = %hash,
                token = %token_symbol_or_addr,
            );
            // CODE-2 fix: do NOT reset expected_profit_usd to None here.
            // opportunity.expected_profit_usd was assigned from gross_profit_f64
            // above the gate match. Overriding with None would discard a valid
            // gross profit that was computed before the allowlist check ran.
            // net_expected_profit_usd stays None for rejected rows — Net Profit
            // Gate requires gross first; spine does not re-score rejected tokens.
            opportunity.roi_pct = Some(0.0);
            opportunity.risk_score = Some(0.0);
            // GAP-2 fix: persist diagnostic reason — operator filters by
            // `rejection_reason` in the dashboard to count and audit allowlist gaps.
            opportunity.rejection_reason = Some(format!("TokenNotAllowed:{token_symbol_or_addr}"));
            chain_counters(client.chain_id)
                .gate_token_not_allowed
                .fetch_add(1, Ordering::Relaxed);
            if let Some(pool) = db {
                if let Err(e) = persistence::insert_opportunity(pool, &opportunity).await {
                    chain_counters(client.chain_id).db_errors.fetch_add(1, Ordering::Relaxed);
                    error!(event = "scanner.db_error", tx_hash = %hash, error = %e);
                } else {
                    chain_counters(client.chain_id).db_persisted.fetch_add(1, Ordering::Relaxed);
                }
            }
            publisher::publish(redis, &opportunity).await?;
            OPPORTUNITIES_TOTAL
                .with_label_values(&[
                    &opportunity.chain_id.to_string(),
                    strategy_label.as_str(),
                    "rejected_token_allowlist",
                ])
                .inc();
            return Ok(());
        }
        ConfigGateOutcome::StrategyDisabled { strategy_kind } => {
            info!(
                event = "config.strategy_disabled",
                chain_id = client.chain_id,
                hash = %hash,
                strategy = %strategy_kind,
            );
            // CODE-2 fix: same rationale as TokenNotAllowed arm above — preserve
            // the gross_profit_f64 already assigned to opportunity.expected_profit_usd.
            opportunity.roi_pct = Some(0.0);
            opportunity.risk_score = Some(0.0);
            opportunity.rejection_reason = Some(format!("StrategyDisabled:{strategy_kind}"));
            chain_counters(client.chain_id)
                .gate_strategy_disabled
                .fetch_add(1, Ordering::Relaxed);
            if let Some(pool) = db {
                if let Err(e) = persistence::insert_opportunity(pool, &opportunity).await {
                    chain_counters(client.chain_id).db_errors.fetch_add(1, Ordering::Relaxed);
                    error!(event = "scanner.db_error", tx_hash = %hash, error = %e);
                } else {
                    chain_counters(client.chain_id).db_persisted.fetch_add(1, Ordering::Relaxed);
                }
            }
            publisher::publish(redis, &opportunity).await?;
            OPPORTUNITIES_TOTAL
                .with_label_values(&[
                    &opportunity.chain_id.to_string(),
                    strategy_label.as_str(),
                    "rejected_strategy_disabled",
                ])
                .inc();
            return Ok(());
        }
        // Migration 056 — StrategyConfigGate (per-strategy fine-grained config).
        // Persists with `rejection_reason` populated so the operator sees the
        // exact gate that fired (e.g. StrategyConfigDexBlocked, StrategyConfigDisabled).
        // Same fail-honest persistence pattern as TokenNotAllowed/StrategyDisabled
        // above — silent rejects would hide the operator's own toggles' impact.
        ConfigGateOutcome::StrategyConfigGateBlocked { reason } => {
            let tag = reason.tag();
            info!(
                event = "config.strategy_config_gate_blocked",
                chain_id = client.chain_id,
                hash = %hash,
                reject_tag = tag,
                reason = ?reason,
            );
            opportunity.roi_pct = Some(0.0);
            opportunity.risk_score = Some(0.0);
            opportunity.rejection_reason = Some(format!("{tag}:{reason:?}"));
            chain_counters(client.chain_id)
                .gate_strategy_disabled
                .fetch_add(1, Ordering::Relaxed);
            if let Some(pool) = db {
                if let Err(e) = persistence::insert_opportunity(pool, &opportunity).await {
                    chain_counters(client.chain_id).db_errors.fetch_add(1, Ordering::Relaxed);
                    error!(event = "scanner.db_error", tx_hash = %hash, error = %e);
                } else {
                    chain_counters(client.chain_id).db_persisted.fetch_add(1, Ordering::Relaxed);
                }
            }
            publisher::publish(redis, &opportunity).await?;
            OPPORTUNITIES_TOTAL
                .with_label_values(&[
                    &opportunity.chain_id.to_string(),
                    strategy_label.as_str(),
                    "rejected_strategy_config_gate",
                ])
                .inc();
            return Ok(());
        }
        ConfigGateOutcome::Evaluated {
            outcome,
            evidence,
            rejection,
            partial_data_quality: _,
        } => (evidence, outcome, rejection),
    };

    // REVM atomic sim gate — Phase A.3.b RUNTIME WIRE.
    //
    // RULE 00 + RULE 12 + RULE 15:
    //   "Si no hay simulador real disponible, el candidato debe fallar, no pasar."
    //   "Si el net_profit_wei no se puede calcular con datos reales, rechazar."
    //
    // Pipeline status (this commit, Phase A.3.b):
    //   - A.1/A.2 (commit 2b7502a): legacy v1 stub neutralized.
    //   - A.2.5 (commit 36cda55): per-chain `Arc<SimulatorV2>` threaded.
    //   - A.3.a (commit 8a359e4): `OpportunityCandidate → RoundTripContext`
    //     encoder shipped with 26 unit tests.
    //   - A.3.b (this commit): PG decimals provider threaded, encoder INVOKED
    //     from the hot path. Every candidate with a simulator + provider in
    //     hand attempts a real encoder dispatch; outcome maps to typed
    //     counters and reasons. The system STILL stays in
    //     `SIM_DISABLED_FAIL_CLOSED` — the next phase (`execute_round_trip`
    //     REVM orchestrator) is the only remaining gate before SIM_SUCCESS
    //     can be emitted.
    //
    // Dispatch outcomes today (all map to `SIM_DISABLED_FAIL_CLOSED`):
    //   * No simulator for chain → reason `no_simulator_for_chain`.
    //   * Simulator but no decimals provider → reason `missing_provider`
    //     (main.rs wiring regression — should never fire).
    //   * Encoder returns `Ok(RoundTripContext)` → reason
    //     `round_trip_executor_pending`. This is the SUCCESS PATH of A.3.b;
    //     it proves the wire is end-to-end functional. The candidate is
    //     still rejected at the EvidenceGate because the orchestrator
    //     hasn't computed an actual SimulationOutcome yet.
    //   * Encoder returns `Err(SimEncoderError::*)` → reason
    //     `encoder_rejected:<tag>` where `<tag>` is the variant's
    //     `reason_tag()`. The exact rejection reason is observable in the
    //     `encoder.runtime_event` log and the counter-by-tag breakdown.
    //
    // `gates.rs::EvidenceGate::validate` (line 11) keeps everything honest:
    // every value other than `PASS` or `SIM_SUCCESS` maps to
    // `RejectReason::SimulationFailed`. Zero paper opportunities survive
    // this PR — that is the truth of the system today (RULE 12 fail-honest).
    let gate_outcome = dispatch_encoder_gate(
        &candidate,
        client.chain_id,
        simulator_v2,
        decimals_provider,
    );
    bump_encoder_gate_counter(&gate_outcome);

    // Phase A.3.c — if the encoder produced a RoundTripContext AND we have a
    // SimulatorV2 in hand, dispatch `execute_round_trip_revm` synchronously on
    // a blocking tokio thread. cs-validator finding 2026-05-12: REVM is
    // synchronous; running it on the tokio worker would park the event loop.
    let (fail_closed_reason, trace_hash_sentinel, sim_status_str) =
        if let (EncoderGateOutcome::EncoderOk(ctx), Some(simulator_arc)) =
            (&gate_outcome, simulator_v2.cloned())
        {
            dispatch_orchestrator_and_classify(
                ctx.clone(),
                simulator_arc,
                client.chain_id,
                &candidate,
            )
            .await
        } else {
            let (fcr, ths) = gate_outcome.to_status();
            (
                fcr.to_string(),
                ths.to_string(),
                "SIM_DISABLED_FAIL_CLOSED".to_string(),
            )
        };

    final_evidence.simulation_status = sim_status_str.clone();
    final_evidence.simulation_trace_hash = Some(trace_hash_sentinel.clone());
    if sim_status_str != "SIM_SUCCESS" {
        chain_counters(client.chain_id)
            .simulator_fail_closed_rejected
            .fetch_add(1, Ordering::Relaxed);
    }
    debug!(
        event = "encoder.runtime_event",
        hash = %hash,
        chain_id = client.chain_id,
        candidate_id = %candidate.route_fingerprint,
        legs_count = candidate.dex_adapters.len(),
        simulator_available = simulator_v2.is_some(),
        provider_available = decimals_provider.is_some(),
        status = gate_outcome.status_label(),
        reason = %fail_closed_reason,
        sim_status = %sim_status_str,
        v2_feature_compiled = cfg!(feature = "v2-simulator"),
        "candidate evaluated at A.3.c orchestrator gate"
    );

    // Connect math results to the persisted Opportunity row.
    // R8 fail-honest: expected_profit_usd MUST be None when USD profit was not
    // computed (oracle gap / unknown token price). Some(0.0) would mean "we
    // computed the profit and it is exactly zero" — a distinct semantic from
    // "we could not compute it at all". The evaluator zeroes gross_profit_usd
    // when UnknownTokenPrice fires; we must not persist that zero as a real value.
    let unknown_token_price = matches!(&config_rejection, Some(RejectReason::UnknownTokenPrice));
    opportunity.expected_profit_usd = if unknown_token_price {
        None // R8: not computed — token price unknown, gross_profit_usd=0.0 is synthetic
    } else {
        Some(math_outcome.gross_profit_usd)
    };
    opportunity.roi_pct = if unknown_token_price {
        None // R8: not computed — cascades from expected_profit_usd=None
    } else {
        Some(math_outcome.net_roi_pct)
    };

    if let Some(reason) = config_rejection {
        info!(
            event = "config.gate_rejected",
            hash = %hash,
            reason = ?reason,
            net_profit_usd = math_outcome.net_profit_usd,
            roi_pct = math_outcome.net_roi_pct,
        );
        opportunity.risk_score = Some(0.0);
        // GAP-2 fix: persist the spine's diagnostic rejection reason
        // (UnknownTokenPrice / AnomalousMath / NegativeNetProfit / LowLiquidity / ...)
        // — converted to debug string so the operator dashboard can group + filter.
        let reason_str = format!("{reason:?}");
        opportunity.rejection_reason = Some(reason_str.clone());
        // Heartbeat counter — split the most diagnostic reasons into
        // dedicated buckets so the operator's per-minute summary surfaces
        // BUG-2-class issues (UnknownTokenPrice) and defense-in-depth
        // hits (AnomalousMath) separately from operational risk gates.
        if reason_str.starts_with("UnknownTokenPrice") {
            chain_counters(client.chain_id)
                .gate_unknown_token_price
                .fetch_add(1, Ordering::Relaxed);
        } else if reason_str.starts_with("AnomalousMath") {
            chain_counters(client.chain_id)
                .gate_anomalous_math
                .fetch_add(1, Ordering::Relaxed);
        } else {
            chain_counters(client.chain_id)
                .gate_other_rejected
                .fetch_add(1, Ordering::Relaxed);
        }
        // CODE-2 fix: populate net_expected_profit_usd for gate-rejected rows.
        // H2 fix populated it only in the passed_all_gates branch, leaving all
        // rejected rows with net=NULL even when gross was computed.
        // Pattern mirrors H2 (commit 69fb24c): gross - gas_cost. When gross is
        // None (oracle gap), net propagates as None — R8 fail-honest.
        opportunity.net_expected_profit_usd = opportunity
            .expected_profit_usd
            .map(|gross| gross - cfg.gas_cost_usd());
    } else {
        chain_counters(client.chain_id).passed_all_gates.fetch_add(1, Ordering::Relaxed);
        // Spine scoring on REAL evidence (no more hardcoded 0.95 / 0.9 / 1.0).
        let engine = PrioritizationEngine {
            min_profit_threshold: cfg.min_profit_usd,
        };
        match engine.score(&candidate, &final_evidence) {
            Ok(score) => {
                final_evidence.net_expected_profit = score.net_expected_profit;
                final_evidence.final_score = score.final_score;
                final_evidence.decision = can_execute(&final_evidence, true);
                opportunity.risk_score = Some(score.final_score);
                // H2 fix: propagate spine-computed net profit to the Opportunity
                // row so submit_engine Check 7 gates on net, not gross.
                // Only set when net > 0 OR explicitly 0; negative net still
                // propagates (it will be blocked by Check 7 as intended).
                opportunity.net_expected_profit_usd = Some(score.net_expected_profit);

                if let Ok(json) = serde_json::to_string(&final_evidence) {
                    let _ = std::fs::create_dir_all("logs/mev");
                    if let Ok(mut file) = OpenOptions::new()
                        .create(true)
                        .append(true)
                        .open("logs/mev/opportunity_scored.jsonl")
                    {
                        let _ = writeln!(file, "{}", json);
                    }
                }

                if final_evidence.decision == ExecutionDecision::Reject {
                    info!(event = "spine.rejected", hash = %hash, reason = ?final_evidence.reject_reason);
                }
            }
            Err(e) => {
                debug!(event = "spine.scoring_error", hash = %hash, error = ?e);
                opportunity.risk_score = Some(0.0);
            }
        }
    }
    // --- END CONFIG-AWARE SPINE INTERCEPTOR ---

    // BE-3.6: opportunity-level dedup gate.
    // Check compound key (route + 5min time bucket + $0.10 profit bucket) before
    // persisting or publishing. This prevents the same route+spread from flooding
    // DB and Redis on every pending tx that hits the same pool pair within a window.
    // The dedup fires AFTER spine scoring so the stored row always carries the
    // fully-scored profit/risk values — we dedupe at the emit boundary, not earlier.
    // Rejected rows (TokenNotAllowed, StrategyDisabled, gate-rejected) each have
    // their own return paths above and skip this check, so dedup only affects
    // opportunities that passed all gates.
    let route_fp = &candidate.route_fingerprint;
    if !opp_dedup.check_and_mark(route_fp, opportunity.expected_profit_usd) {
        debug!(
            event = "scanner.opp_dedup_suppressed",
            hash = %hash,
            route = %route_fp,
            profit_usd = ?opportunity.expected_profit_usd,
            "opportunity suppressed by route+time+profit dedup (BE-3.6)"
        );
        chain_counters(client.chain_id)
            .gate_other_rejected
            .fetch_add(1, Ordering::Relaxed);
        return Ok(());
    }

    // Persist + publish. Both are best-effort with their own error paths.
    if let Some(pool) = db {
        if let Err(e) = persistence::insert_opportunity(pool, &opportunity).await {
            chain_counters(client.chain_id).db_errors.fetch_add(1, Ordering::Relaxed);
            error!(event = "scanner.db_error", tx_hash = %hash, error = %e);
        } else {
            chain_counters(client.chain_id).db_persisted.fetch_add(1, Ordering::Relaxed);
        }
    }
    publisher::publish(redis, &opportunity).await?;

    OPPORTUNITIES_TOTAL
        .with_label_values(&[
            &opportunity.chain_id.to_string(),
            strategy_label.as_str(),
            "detected",
        ])
        .inc();

    Ok(())
}

fn u256_to_f64_lossy(v: ethers::types::U256) -> f64 {
    // U256 → f64 via decimal string. Loses precision past ~15 sig figs but
    // f64 is what OpportunityCandidate uses; this is a one-way display path,
    // never re-fed into on-chain arithmetic.
    v.to_string().parse::<f64>().unwrap_or(0.0)
}

/// Encapsulates the scanner-level USD pricing decision for a detected spread.
/// Returns None when the token_out cannot be priced (oracle gap: neither base
/// token nor stablecoin). Returns Some(usd_value) only when pricing succeeded.
///
/// **GROSS value only.** This function returns the raw token-unit spread
/// converted to USD using the operator's base token price or a 1:1 stablecoin
/// assumption. It does NOT subtract gas, slippage, relay fees, or any other
/// cost component. The result is stored in `Opportunity.expected_profit_usd`
/// (the GROSS column). The canonical net figure after all cost deductions is
/// `Opportunity.net_expected_profit_usd`, populated by the spine evaluator
/// (`calc_net_profit_and_roi`). Check 7 in the pre-execute checklist uses the
/// net field; callers that use `expected_profit_usd` directly are comparing
/// against GROSS and will over-estimate profitability by 20-40%.
///
/// This is the pure-function kernel extracted from `process_pending` so it
/// can be unit-tested in isolation (the full scanner loop requires live Redis
/// + WS; this helper requires nothing).
///
/// R8 invariant: the returned Option MUST preserve None for unpriced paths.
/// Callers MUST propagate None to `opportunity.expected_profit_usd` without
/// substituting 0.0 (which would silently assert "computed profit = zero").
fn compute_gross_usd_for_spread(
    spread_token_out: f64,
    is_base_token: bool,
    base_token_price_usd: f64,
    is_stablecoin: bool,
) -> Option<f64> {
    if is_base_token {
        Some(spread_token_out * base_token_price_usd)
    } else if is_stablecoin {
        Some(spread_token_out)
    } else {
        // Oracle gap — cannot price in USD without an external oracle.
        // R8: return None, NOT Some(0.0).
        None
    }
}

// ---------------------------------------------------------------------------
// Phase A.3.b — encoder dispatch helper
// ---------------------------------------------------------------------------

/// Outcome of the A.3.b encoder dispatch gate. Distinct variants per failure
/// mode + per encoder error reason so the scanner can map each to a typed
/// counter without re-inspecting the underlying `SimEncoderError`.
///
/// This is a pure data type: `dispatch_encoder_gate` returns one of these
/// variants, the caller updates counters + emits the fail-closed signal.
/// Extracted from the hot path so it is unit-testable without WS/RPC infra.
#[derive(Debug, Clone)]
pub(crate) enum EncoderGateOutcome {
    /// No `Arc<SimulatorV2>` for this chain. RPC_HTTP_<id> not configured
    /// or pool fully unhealthy at boot. Operator action: provide RPC.
    NoSimulator,
    /// SimulatorV2 present but `Arc<dyn TokenDecimalsProvider>` is `None`.
    /// Should only occur if main.rs wired the simulator but not the PG
    /// pool — typically a regression. The counter `encoder_missing_provider_total`
    /// surfaces it.
    NoProvider,
    /// Encoder ran successfully; the produced `RoundTripContext` is the
    /// next-phase input for `sim_orchestrator::execute_round_trip_revm`.
    /// Phase A.3.c lifts this from a fail-closed marker to a real REVM
    /// dispatch via `tokio::task::spawn_blocking` in `decode_and_score_tx`.
    EncoderOk(prioritization_spine::round_trip_executor::RoundTripContext),
    /// Encoder rejected the candidate with a typed reason. The tag matches
    /// `SimEncoderError::reason_tag()` verbatim.
    Rejected { reason_tag: &'static str },
}

impl EncoderGateOutcome {
    /// Map to (fail_closed_reason, trace_hash_sentinel) for the
    /// `final_evidence.simulation_status` + `simulation_trace_hash` fields.
    /// Both fields stay in the fail-closed family until Phase A.3.c lands.
    fn to_status(&self) -> (&'static str, &'static str) {
        match self {
            Self::NoSimulator => (
                "no_simulator_for_chain",
                "fail_closed:no_rpc_http_for_chain",
            ),
            Self::NoProvider => (
                "missing_provider",
                "fail_closed:missing_token_decimals_provider",
            ),
            Self::EncoderOk(_) => (
                "encoder_ok_orchestrator_dispatched",
                "fail_closed:orchestrator_in_flight",
            ),
            Self::Rejected { reason_tag } => (
                reason_tag,
                "fail_closed:encoder_rejected",
            ),
        }
    }

    fn status_label(&self) -> &'static str {
        match self {
            Self::NoSimulator => "fail_closed_no_simulator",
            Self::NoProvider => "fail_closed_no_provider",
            Self::EncoderOk(_) => "encoder_ok_orchestrator_dispatched",
            Self::Rejected { .. } => "encoder_rejected",
        }
    }
}

/// Pure dispatch logic — testable in isolation. Builds an
/// `sim_encoder::RouteEncodingConfig`, resolves the executor address via
/// `EXECUTOR_<chain_id>`, calls `build_round_trip_context_from_candidate`,
/// and maps the outcome.
///
/// Reads ONE env var (`EXECUTOR_<chain_id>`) inside `parse_executor_address`.
/// Reads system time once (for `now_unix_ts`). No other I/O.
pub(crate) fn dispatch_encoder_gate(
    candidate: &OpportunityCandidate,
    chain_id: u64,
    simulator_v2: ScannerSimulatorV2Ref<'_>,
    decimals_provider: ScannerDecimalsProviderRef<'_>,
) -> EncoderGateOutcome {
    if simulator_v2.is_none() {
        return EncoderGateOutcome::NoSimulator;
    }
    let provider = match decimals_provider {
        Some(p) => p,
        None => return EncoderGateOutcome::NoProvider,
    };
    let executor = match crate::sim_encoder::parse_executor_address(chain_id) {
        Ok(addr) => addr,
        Err(e) => {
            return EncoderGateOutcome::Rejected {
                reason_tag: e.reason_tag(),
            };
        }
    };
    // Config: deadline default 60s; now from SystemTime; min_profit_wei is
    // the bare contract minimum (1 wei = "any positive profit accepted") so
    // the encoder's required-non-zero invariant is satisfied without
    // inventing an economic floor. Real economic profit gating happens
    // downstream at the NetProfitGate using `net_expected_profit_usd`
    // (see `gates.rs`).
    let now_unix_ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let config = crate::sim_encoder::RouteEncodingConfig {
        deadline_seconds: 60,
        now_unix_ts,
        min_profit_wei: ethers::types::U256::from(1u64),
    };
    let provider_ref: &dyn crate::sim_encoder::TokenDecimalsProvider = provider.as_ref();
    match crate::sim_encoder::build_round_trip_context_from_candidate(
        candidate,
        chain_id,
        executor,
        provider_ref,
        &config,
    ) {
        Ok(round_trip_context) => EncoderGateOutcome::EncoderOk(round_trip_context),
        Err(e) => EncoderGateOutcome::Rejected {
            reason_tag: e.reason_tag(),
        },
    }
}

/// Translate the gate outcome into per-tag counter increments. Centralised so
/// adding a new `SimEncoderError` variant only requires touching one place.
fn bump_encoder_gate_counter(outcome: &EncoderGateOutcome) {
    use std::sync::atomic::Ordering::Relaxed;
    let c = counters();
    match outcome {
        EncoderGateOutcome::NoSimulator => {
            c.simulator_v2_no_simulator_for_chain.fetch_add(1, Relaxed);
        }
        EncoderGateOutcome::NoProvider => {
            c.simulator_v2_encoder_not_ready.fetch_add(1, Relaxed);
            c.encoder_missing_provider_total.fetch_add(1, Relaxed);
        }
        EncoderGateOutcome::EncoderOk(_) => {
            c.encoder_success_total.fetch_add(1, Relaxed);
            // `encoder_round_trip_executor_pending_total` is no longer the
            // terminal counter for the success path — Phase A.3.c dispatches
            // the orchestrator. The pending counter increments only in the
            // brief window between encoder OK and orchestrator dispatch (i.e.
            // never in the steady state under A.3.c).
        }
        EncoderGateOutcome::Rejected { reason_tag } => {
            c.encoder_rejected_total.fetch_add(1, Relaxed);
            match *reason_tag {
                "missing_executor" | "invalid_executor" | "zero_executor" => {
                    c.encoder_missing_executor_total.fetch_add(1, Relaxed);
                }
                "missing_decimals" | "invalid_decimals" => {
                    c.encoder_missing_decimals_total.fetch_add(1, Relaxed);
                }
                "amount_nan" | "amount_infinite" | "amount_non_positive"
                | "amount_overflow" | "amount_too_small" => {
                    c.encoder_invalid_amount_total.fetch_add(1, Relaxed);
                }
                "unsupported_dex_kind" | "missing_router" => {
                    c.encoder_unsupported_dex_total.fetch_add(1, Relaxed);
                }
                "unsupported_route_shape" | "missing_route_legs" => {
                    c.encoder_unsupported_route_shape_total.fetch_add(1, Relaxed);
                }
                "zero_token_address" => {
                    c.encoder_zero_token_address_total.fetch_add(1, Relaxed);
                }
                _ => {
                    // Catches `missing_token_in`, `missing_token_out`,
                    // `invalid_token_address`, `same_token_in_out`,
                    // `missing_deadline_config`, `missing_min_profit`,
                    // and any future variant.
                    c.encoder_other_rejected_total.fetch_add(1, Relaxed);
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Phase A.3.c — orchestrator dispatch helper
// ---------------------------------------------------------------------------

/// Dispatch the REVM orchestrator on a blocking tokio thread, classify the
/// returned `SimulationOutcome`, and produce the (fail_closed_reason,
/// trace_hash_sentinel, simulation_status) triple consumed by the hot path.
///
/// The function is async so it can await the spawn_blocking join handle.
/// Returns owned Strings because the orchestrator may bubble up dynamic
/// REVM revert reasons that don't fit a &'static str.
///
/// `SimulationOutcome.passed=true` is the ONLY path that admits SIM_SUCCESS.
/// On false the simulation_status stays SIM_DISABLED_FAIL_CLOSED and the
/// fail_reason's tag prefix routes to the right A.3.c counter.
#[cfg(feature = "v2-simulator")]
async fn dispatch_orchestrator_and_classify(
    ctx: prioritization_spine::round_trip_executor::RoundTripContext,
    simulator: Arc<simulator_v2::SimulatorV2>,
    chain_id: u64,
    candidate: &prioritization_spine::types::OpportunityCandidate,
) -> (String, String, String) {
    use std::sync::atomic::Ordering::Relaxed;
    let c = counters();
    c.round_trip_executor_started_total.fetch_add(1, Relaxed);

    // Resolve executor address (re-read; sim_encoder already validated but the
    // orchestrator config needs the parsed value).
    let executor = match crate::sim_encoder::parse_executor_address(chain_id) {
        Ok(addr) => addr,
        Err(_) => {
            c.round_trip_executor_rejected_pre_revm_total.fetch_add(1, Relaxed);
            return (
                "missing_executor".to_string(),
                "fail_closed:missing_executor".to_string(),
                "SIM_DISABLED_FAIL_CLOSED".to_string(),
            );
        }
    };

    // Read gas price from env (`SIM_ORCHESTRATOR_GAS_PRICE_WEI`) or default
    // to 25 gwei. Default is documented in module docs; operator can override
    // explicitly per the directive's "explicit config" rule.
    let gas_price_wei = std::env::var("SIM_ORCHESTRATOR_GAS_PRICE_WEI")
        .ok()
        .and_then(|v| v.parse::<u128>().ok())
        .map(ethers::types::U256::from)
        .unwrap_or_else(|| ethers::types::U256::from(25_000_000_000u64));

    // route_hash: derive a stable 32-byte tag from the candidate's
    // `route_fingerprint` so the orchestrator's calldata is deterministic.
    // Uses keccak256 (already a workspace dep via ethers).
    let route_hash: [u8; 32] = ethers::utils::keccak256(candidate.route_fingerprint.as_bytes());

    let orch_config = crate::sim_orchestrator::RoundTripExecutionConfig {
        chain_id,
        executor_address: executor,
        gas_limit: 30_000_000,
        gas_price_wei,
        route_hash,
        min_profit_wei: ethers::types::U256::from(1u64),
        paper_mode: true,
    };

    // Run REVM on a blocking thread.
    let outcome = match tokio::task::spawn_blocking(move || {
        crate::sim_orchestrator::execute_round_trip_revm(&ctx, simulator, &orch_config)
    })
    .await
    {
        Ok(o) => o,
        Err(e) => {
            c.round_trip_executor_spawn_blocking_failed_total.fetch_add(1, Relaxed);
            return (
                format!("spawn_blocking_failed:{e}"),
                "fail_closed:spawn_blocking_failed".to_string(),
                "SIM_DISABLED_FAIL_CLOSED".to_string(),
            );
        }
    };

    if outcome.passed {
        c.round_trip_executor_success_total.fetch_add(1, Relaxed);
        c.simulator_revm_success.fetch_add(1, Relaxed);
        return (
            "round_trip_success".to_string(),
            "orchestrator_success".to_string(),
            "SIM_SUCCESS".to_string(),
        );
    }

    // Failed path — classify by reason_tag prefix.
    let reason = outcome.fail_reason.clone().unwrap_or_default();
    let tag = reason.split(':').next().unwrap_or("unknown");
    match tag {
        "revm_reverted" => {
            c.round_trip_executor_revert_total.fetch_add(1, Relaxed);
            c.simulator_revm_revert.fetch_add(1, Relaxed);
        }
        "revm_infra_error" => {
            c.round_trip_executor_error_total.fetch_add(1, Relaxed);
        }
        "net_profit_non_positive" => {
            c.round_trip_executor_net_profit_non_positive_total
                .fetch_add(1, Relaxed);
        }
        "success_zero_gas" | "success_empty_trace_hash" => {
            c.round_trip_executor_antifraud_rejected_total
                .fetch_add(1, Relaxed);
        }
        _ => {
            c.round_trip_executor_rejected_pre_revm_total.fetch_add(1, Relaxed);
        }
    }
    (
        reason,
        "fail_closed:orchestrator_rejected".to_string(),
        "SIM_DISABLED_FAIL_CLOSED".to_string(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Regression test for R8 fail-honest: when the oracle gap path fires
    /// (token_out is neither base token nor stablecoin), the scanner MUST
    /// propagate None — not Some(0.0) — as the USD profit.
    ///
    /// This is the exact regression that was introduced in commit fdae583
    /// where `gross_profit_f64 = 0.0` was unconditionally written in the
    /// else branch, then wrapped as `Some(0.0)` at persistence time,
    /// silently asserting "we computed the profit and it is zero" instead
    /// of "we could not price this token pair".
    #[test]
    fn oracle_gap_path_preserves_none_not_zero() {
        // Arrange: token_out is some unknown ERC-20 (not base, not stablecoin).
        let spread = 42.0; // arbitrary non-zero spread in token_out units

        // Act: simulate all three paths.
        let base_result = compute_gross_usd_for_spread(spread, true, 2000.0, false);
        let stable_result = compute_gross_usd_for_spread(spread, false, 0.0, true);
        let gap_result = compute_gross_usd_for_spread(spread, false, 0.0, false);

        // Assert: base and stable paths compute real values.
        assert_eq!(
            base_result,
            Some(42.0 * 2000.0),
            "base token path must compute USD value"
        );
        assert_eq!(
            stable_result,
            Some(42.0),
            "stablecoin path must return spread directly (1:1 USD)"
        );

        // Critical R8 invariant: oracle gap MUST be None, never Some(0.0).
        assert_eq!(
            gap_result, None,
            "R8 violation: oracle gap path must produce None (uncomputed), \
             not Some(0.0) (computed-and-zero). Persisting Some(0.0) silently \
             claims we computed a zero profit, hiding the missing price oracle."
        );

        // Extra guard: the None must NOT compare equal to Some(0.0).
        assert_ne!(
            gap_result,
            Some(0.0),
            "R8: None and Some(0.0) are semantically distinct; must not be equal"
        );
    }

    /// Invariant: even a genuinely zero spread in a priced token should yield
    /// Some(0.0), not None. This distinguishes "priced and zero" from "unpriced".
    #[test]
    fn zero_spread_in_priced_token_yields_some_zero() {
        let result = compute_gross_usd_for_spread(0.0, true, 2000.0, false);
        assert_eq!(
            result,
            Some(0.0),
            "zero spread in a priced token must yield Some(0.0), not None"
        );
    }
}
