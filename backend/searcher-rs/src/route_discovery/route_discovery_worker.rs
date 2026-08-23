//! RouteDiscoveryWorker — the periodic shadow radar loop.
//!
//! Per tick: read the live pool set from `ImpactIndex` → build the token graph
//! from Redis (`graph_builder`) → enumerate unique cycles (`unique_route_finder`)
//! → annotate each with strategy applicability → PUBLISH telemetry. **No
//! sizing, no profit, no execution, and never a write to `arbx:opps:detected`.**
//!
//! Gated by [`RouteDiscoveryMode`] (`ARBX_ROUTE_DISCOVERY_MODE`, default `Off`):
//! when off, [`spawn_route_discovery`] returns immediately — nothing is spawned.
//!
//! The per-tick work is split into a **pure** [`evaluate_tick`] (graph → events,
//! no Redis/time) and the thin async loop, so the logic unit-tests in memory.

use crate::cartridge::runner::CartridgeRunner;
use crate::cartridge_boot::shadow_evaluate_intent;
use crate::impact_index::ImpactIndex;
use crate::orchestrator::Orchestrator;
use crate::route_discovery::graph_builder::{build_graph, GraphBuildConfig, GraphBuildOutcome};
use crate::route_discovery::route_intent_dispatcher::plan_dispatch;
use crate::route_discovery::strategy_applicability::StrategyApplicabilityEngine;
use crate::route_discovery::telemetry;
use crate::route_discovery::triangular_adapter::{
    cycle_legs, ensure_reserves, gate_dispatch_intents, AdapterOutcome, BackfillBudget,
    RedisRpcBridge, Skipped,
};
use crate::route_discovery::types::{RouteCandidate, RouteKind};
use crate::route_discovery::unique_route_finder::{find_routes, RouteFinderConfig};
use crate::route_discovery::RouteDiscoveryMode;
use crate::route_intent::RouteIntent;
use alloy::providers::Provider as _;
use ethers::types::{Address, H256};
use redis::aio::ConnectionManager;
use serde_json::Value;
use shared_rs::rpc_failover::HttpRpcPool;
use std::path::PathBuf;
use std::str::FromStr;
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tokio::sync::RwLock;
use tokio_util::sync::CancellationToken;
use tracing::{debug, info, warn};

/// Phase-1 algorithm tag stamped onto all telemetry.
pub const ALGORITHM: &str = "dfs_bounded";

const DEFAULT_INTERVAL_MS: u64 = 12_000;
const DEFAULT_MAX_ROUTES_PER_TICK: usize = 500;
const DEFAULT_MAX_TELEMETRY_PER_TICK: usize = 200;
const DEFAULT_MAX_AGE_SECS: u64 = 120;

fn env_usize(key: &str, default: usize) -> usize {
    std::env::var(key)
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(default)
}
fn env_u64(key: &str, default: u64) -> u64 {
    std::env::var(key)
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(default)
}
fn env_u8(key: &str, default: u8) -> u8 {
    std::env::var(key)
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(default)
}

/// Resolve the applicability config path (`ARBX_ROUTE_APPLICABILITY_CONFIG` or
/// the default relative path, matching how cartridges are loaded from `./`).
fn config_path() -> PathBuf {
    std::env::var("ARBX_ROUTE_APPLICABILITY_CONFIG")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("config/strategies/route_applicability.yaml"))
}

/// Parse base-token hex strings to addresses, skipping (with a warn) any that
/// don't parse — R8 fail-honest, never a sentinel.
fn parse_base_tokens(strs: &[String]) -> Vec<Address> {
    let mut out = Vec::new();
    for s in strs {
        match Address::from_str(s.trim()) {
            Ok(a) => out.push(a),
            Err(_) => warn!(event = "route_discovery.bad_base_token", token = %s),
        }
    }
    out
}

/// Resolved worker tunables: env caps override the YAML `discovery:` section.
#[derive(Debug, Clone)]
pub struct WorkerConfig {
    pub interval_ms: u64,
    pub max_telemetry_per_tick: usize,
    pub graph: GraphBuildConfig,
    pub finder: RouteFinderConfig,
}

impl WorkerConfig {
    /// Build from env (caps) layered over the engine's YAML discovery settings.
    pub fn from_env_and_engine(engine: &StrategyApplicabilityEngine) -> Self {
        let disc = &engine.config().discovery;
        let max_routes = env_usize(
            "ARBX_ROUTE_DISCOVERY_MAX_ROUTES_PER_TICK",
            DEFAULT_MAX_ROUTES_PER_TICK,
        );
        let max_telemetry = env_usize(
            "ARBX_ROUTE_DISCOVERY_MAX_TELEMETRY_PER_TICK",
            DEFAULT_MAX_TELEMETRY_PER_TICK,
        );
        // env > yaml > built-in default for the two finder bounds.
        let max_pools_per_pair = env_usize(
            "ARBX_ROUTE_DISCOVERY_MAX_POOLS_PER_PAIR",
            disc.max_pools_per_pair.max(1),
        );
        // XLS-CANON-01: the canonical knob (workbook 01_CONFIG `Max_Hops`) is
        // the TOP explicit tier — `ARBX_KNOB_MAX_HOPS` > legacy env > yaml.
        // Defaults stay the deploy's yaml (no silent hot-path change).
        let max_depth = env_u8(
            "ARBX_KNOB_MAX_HOPS",
            env_u8("ARBX_ROUTE_DISCOVERY_MAX_DEPTH", disc.max_depth.max(2)),
        )
        .clamp(2, 7);
        // XLS-CANON-01 `Min_Hops` (canonical floor 2) — same precedence tier.
        let min_depth = env_u8("ARBX_KNOB_MIN_HOPS", 2).clamp(2, max_depth);
        let interval_ms = env_u64("ARBX_ROUTE_DISCOVERY_INTERVAL_MS", DEFAULT_INTERVAL_MS);
        // XLS-CANON-01 `Max_Freshness_s` — canonical knob > legacy env > default.
        let max_age_secs = env_u64(
            "ARBX_KNOB_MAX_FRESHNESS_S",
            env_u64("ARBX_ROUTE_DISCOVERY_MAX_AGE_SECS", DEFAULT_MAX_AGE_SECS),
        );
        // XLS-QB-03 (workbook step 9 StrategyMask): the canonical
        // `selected_strategy_id` (ARBX_KNOB_SELECTED_STRATEGY_ID > workbook
        // default MEV-01-001) bounds the multi-hop expansion via its 264×u8
        // HopMask BEFORE enumeration. Boot validation of the knobs (which
        // requires the id to be MEV-prefixed) runs elsewhere; here an id
        // unknown to the table honestly skips the pass with telemetry (R8).
        let hop_mask_strategy_id = std::env::var("ARBX_KNOB_SELECTED_STRATEGY_ID")
            .ok()
            .map(|v| v.trim().to_string())
            .filter(|v| !v.is_empty())
            .or_else(|| {
                Some(crate::canonical_knobs::CanonicalKnobs::default().selected_strategy_id)
            });

        // XLS-GRAPH-01 hot-token classification (workbook 03 col P). The hub
        // set defaults to the workbook formula ({WETH, USDC, WBTC, USDT});
        // `ARBX_GRAPH_HOT_TOKENS` overrides per-chain (comma-separated symbols).
        // The prune itself is default-OFF — `ARBX_GRAPH_HOT_TOKEN_ONLY=true`
        // is an explicit operator decision (topology change). Orthogonal to
        // the hop mask above: this gates WHICH pools enter the graph, the
        // mask gates HOW MANY hops expansion may take.
        let hot_tokens = std::env::var("ARBX_GRAPH_HOT_TOKENS")
            .ok()
            .map(|raw| {
                raw.split(',')
                    .map(|s| s.trim().to_uppercase())
                    .filter(|s| !s.is_empty())
                    .collect::<Vec<_>>()
            })
            .filter(|v| !v.is_empty())
            .unwrap_or_default();
        let hot_token_only = std::env::var("ARBX_GRAPH_HOT_TOKEN_ONLY")
            .map(|v| v.eq_ignore_ascii_case("true"))
            .unwrap_or(false);

        Self {
            interval_ms,
            max_telemetry_per_tick: max_telemetry,
            graph: GraphBuildConfig {
                max_age_secs,
                min_liquidity_hint: disc.min_liquidity_hint,
                hot_tokens: if hot_tokens.is_empty() {
                    GraphBuildConfig::default().hot_tokens
                } else {
                    hot_tokens
                },
                hot_token_only,
            },
            finder: RouteFinderConfig {
                min_depth,
                max_depth,
                max_pools_per_pair,
                max_routes_per_tick: max_routes,
                base_tokens: parse_base_tokens(&disc.base_tokens),
                mode: "shadow".to_string(),
                hop_mask_strategy_id,
            },
        }
    }
}

/// Pure per-tick output: annotated routes + telemetry events + tick summary +
/// the intents to shadow-evaluate (executed by the async loop when a cartridge
/// runner is available).
#[derive(Debug, Default)]
pub struct TickOutput {
    pub routes: Vec<RouteCandidate>,
    pub events: Vec<Value>,
    pub tick_summary: Value,
    /// Intents to hand to `shadow_evaluate_intent` (only when dispatch enabled).
    pub dispatch_intents: Vec<RouteIntent>,
    pub routes_found: usize,
    pub routes_dispatched: usize,
    pub telemetry_emitted: usize,
    /// Profitable (Σ log_weight < 0) closed cycles found by the multi_hop_search
    /// negative-cycle pass over the same graph snapshot. Observe-only shadow signal;
    /// bounded by `finder.max_depth` (2..=7) and `finder.max_routes_per_tick`.
    pub multi_hop_cycles_found: usize,
}

/// Pure tick evaluation: run the finder over `outcome.graph`, annotate each
/// candidate with applicability, plan dispatches, and build (capped) telemetry
/// events + the tick summary. No Redis, no clock, no cartridge call —
/// `latency_ms` is injected and the actual `shadow_evaluate_intent` is done by
/// the caller using `dispatch_intents`.
///
/// `dispatch_enabled` is `true` only when a cartridge runner exists; when
/// `false`, no `route_intent.emitted` events and no `dispatch_intents` are
/// produced (R8: the worker then only emits discovery telemetry).
#[allow(clippy::too_many_arguments)]
pub fn evaluate_tick(
    outcome: &GraphBuildOutcome,
    chain_id: u64,
    engine: &StrategyApplicabilityEngine,
    finder: &RouteFinderConfig,
    max_telemetry_per_tick: usize,
    dispatch_enabled: bool,
    latency_ms: u64,
    mode: &str,
) -> TickOutput {
    let found = find_routes(&outcome.graph, chain_id, finder);
    let mut routes = found.routes;

    // Annotate each candidate with strategy applicability.
    for c in routes.iter_mut() {
        let appl = engine.evaluate(c.route_kind);
        c.applicable_strategies = appl.applicable;
        c.rejected_strategies = appl.rejected;
    }

    let mut events: Vec<Value> = Vec::new();
    let mut telemetry_emitted = 0usize;

    for c in routes.iter() {
        if telemetry_emitted >= max_telemetry_per_tick {
            break;
        }
        events.push(telemetry::route_candidate_event(chain_id, ALGORITHM, c));
        events.push(telemetry::strategy_applicability_event(chain_id, c));
        telemetry_emitted += 1;
    }

    // Rejected pools share the same per-tick cap budget (separate counter).
    for (rejected_emitted, r) in outcome.rejected.iter().enumerate() {
        if rejected_emitted >= max_telemetry_per_tick {
            break;
        }
        events.push(telemetry::rejected_event(chain_id, r));
    }

    // Dispatch planning (only when a cartridge runner is available). Capped at
    // max_telemetry_per_tick so neither shadow-eval spawns nor route_intent.
    // emitted events explode under a large graph.
    let mut dispatch_intents: Vec<RouteIntent> = Vec::new();
    let mut routes_dispatched = 0usize;
    if dispatch_enabled {
        let mut dispatch_budget = 0usize;
        'routes: for c in routes.iter() {
            for plan in plan_dispatch(c, engine) {
                if dispatch_budget >= max_telemetry_per_tick {
                    break 'routes;
                }
                events.push(telemetry::route_intent_emitted_event(
                    chain_id,
                    &plan.route_hash,
                    plan.strategy_label.as_str(),
                    mode,
                    plan.dispatch_deferred.as_deref(),
                ));
                if let Some(intent) = plan.intent {
                    dispatch_intents.push(intent);
                    routes_dispatched += 1;
                }
                dispatch_budget += 1;
            }
        }
    }

    // Multi-hop profitable-cycle pass (additive, observe-only). Runs the
    // negative-`log_weight` cycle finder over the SAME graph snapshot, bounded by
    // the already-configured DFS depth (no new explosion surface) and the per-tick
    // route cap. Fail-honest: V3 (None-weight) legs are skipped inside the finder.
    // XLS-CANON-01: Min_Hops/Max_Hops knobs flow through the finder bounds.
    let mh_max_hops = (finder.max_depth as usize).clamp(2, 7);
    let mh_min_hops = (finder.min_depth as usize).clamp(2, mh_max_hops);
    // XLS-QB-03 (workbook step 9 StrategyMask): the selected strategy's
    // HopMask bounds the expansion BEFORE enumeration — O(1) mask test per
    // (strategy, range), the returned span is the mask's admissible EXTENT
    // (observe-only over-approximation, never under-reports). Empty
    // intersection (or unknown MEV_ID) ⇒ honest skip with a reason (R8),
    // never a silently-empty search.
    let (mh_mask_strategy, mh_bounds) = match &finder.hop_mask_strategy_id {
        Some(id) => (
            Some(id.clone()),
            crate::strategy_hop_mask::admissible_hop_bounds(
                id,
                mh_min_hops as u8,
                mh_max_hops as u8,
            ),
        ),
        None => (None, Some((mh_min_hops as u8, mh_max_hops as u8))),
    };
    let mh = match mh_bounds {
        Some((lo, hi)) => crate::route_discovery::multi_hop_search::find_profitable_cycles(
            &outcome.graph,
            lo as usize,
            hi as usize,
            finder.max_routes_per_tick,
        ),
        None => crate::route_discovery::multi_hop_search::MultiHopResult {
            cycles: Vec::new(),
            capped: false,
            dropped_for_cap: 0,
            v3_skipped: 0,
            noise_dropped: 0,
        },
    };
    let multi_hop_cycles_found = mh.cycles.len();

    let mut tick_summary = telemetry::tick_event(
        chain_id,
        ALGORITHM,
        outcome.pools_total,
        outcome.graph.edges.len(),
        outcome.rejected.len(),
        routes.len(),
        routes_dispatched,
        telemetry_emitted,
        found.dropped_for_cap,
        found.capped,
        found.pools_truncated,
        latency_ms,
        mode,
    );
    // Inject the multi-hop signal without changing tick_event's signature.
    tick_summary["multi_hop_profitable_cycles"] = serde_json::json!(multi_hop_cycles_found);
    tick_summary["multi_hop_v3_skipped"] = serde_json::json!(mh.v3_skipped);
    tick_summary["multi_hop_capped"] = serde_json::json!(mh.capped);
    // PR-ROUTE-06: surface the noise-floor prune so it never dies in silence (R8).
    tick_summary["multi_hop_noise_dropped"] = serde_json::json!(mh.noise_dropped);
    // XLS-QB-03: make the StrategyMask hop bounds observable — which strategy
    // gated the pass, the effective span, and an honest skip reason when the
    // mask∩knobs intersection is empty (0 cycles MUST be explainable).
    tick_summary["multi_hop_mask_strategy"] = serde_json::json!(mh_mask_strategy);
    tick_summary["multi_hop_hops_effective"] = serde_json::json!(mh_bounds);
    tick_summary["multi_hop_mask_skip"] = serde_json::json!(mh_bounds.is_none());

    TickOutput {
        routes_found: routes.len(),
        routes,
        events,
        tick_summary,
        dispatch_intents,
        routes_dispatched,
        telemetry_emitted,
        multi_hop_cycles_found,
    }
}

/// The async loop: tick on an interval until `cancel` fires.
#[allow(clippy::too_many_arguments)] // per-tick coordinator wiring
async fn run_loop(
    mut redis: ConnectionManager,
    chain_id: u64,
    rpc_pool: Option<Arc<HttpRpcPool>>,
    impact_index: Arc<RwLock<ImpactIndex>>,
    engine: StrategyApplicabilityEngine,
    cfg: WorkerConfig,
    runner: Option<Arc<CartridgeRunner>>,
    orchestrator: Option<Arc<Orchestrator>>,
    cancel: CancellationToken,
) {
    let dispatch_enabled = runner.is_some();
    // FASE 2 (D-01/F2): per-block backfill allowance for the triangular
    // reserves adapter — created ONCE so the budget persists across ticks.
    let adapter_budget = BackfillBudget::new();
    let mut ticker = tokio::time::interval(Duration::from_millis(cfg.interval_ms.max(1_000)));
    ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    info!(
        event = "route_discovery.started",
        chain_id,
        interval_ms = cfg.interval_ms,
        max_depth = cfg.finder.max_depth,
        max_routes_per_tick = cfg.finder.max_routes_per_tick,
        max_pools_per_pair = cfg.finder.max_pools_per_pair,
        hop_mask_strategy = ?cfg.finder.hop_mask_strategy_id,
        algorithm = ALGORITHM,
        "route discovery worker started (shadow-only)"
    );

    loop {
        tokio::select! {
            _ = cancel.cancelled() => {
                info!(event = "route_discovery.stopped", chain_id);
                break;
            }
            _ = ticker.tick() => {}
        }

        let t0 = Instant::now();
        let pools = { impact_index.read().await.all_pools() };
        let now_ts = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);

        let outcome: GraphBuildOutcome =
            build_graph(&mut redis, chain_id, &pools, now_ts, &cfg.graph).await;
        let latency_ms = t0.elapsed().as_millis() as u64;

        let mut tick = evaluate_tick(
            &outcome,
            chain_id,
            &engine,
            &cfg.finder,
            cfg.max_telemetry_per_tick,
            dispatch_enabled,
            latency_ms,
            "shadow",
        );

        // ── FASE 2 (D-01/F2): triangular reserves adapter — BEFORE dispatch ──
        // The data bridge `needs_triangular_adapter` named: radar-discovered
        // pools mostly lack cached reserves (6/93 coverage today), so every
        // triangular cycle's 3 legs are served from cache when fresh (V2 ≤5
        // blocks behind the chain head / V3 slot0 within its TTL) or
        // backfilled via ONE bounded multicall before any dispatch. Skipped ⇒
        // that route's dispatch is dropped for this tick (honest skip +
        // telemetry — R8/RULE 00, never synthesized). Read-only data plumbing:
        // no emitter, no orchestrator, `arbx:opps:detected` stays untouched.
        // REGLA 0f: this PR merges BEFORE #350 (F1/F3) — the triangular
        // dispatch #350 enables consumes this bridge.
        let mut adapter_cache_hit = 0usize;
        let mut adapter_backfill_ok = 0usize;
        let mut adapter_backfill_fail = 0usize;
        let mut adapter_budget_exhausted = 0usize;
        let mut skipped_hashes: Vec<H256> = Vec::new();
        let mut adapter_events: Vec<Value> = Vec::new();

        // Current chain block for the ≤5-block freshness bound — one bounded
        // RPC read per tick (mirrors pool_sync_worker's tick block fetch). 0 =
        // fetch failed ⇒ freshness unverifiable ⇒ V2 legs read stale and force
        // backfill (fail-honest, never a blind cache hit).
        let current_block = match &rpc_pool {
            Some(rpc) => tokio::time::timeout(
                Duration::from_millis(
                    crate::route_discovery::triangular_adapter::ADAPTER_CALL_TIMEOUT_MS,
                ),
                rpc.with_retry(|p| async move {
                    p.get_block_number()
                        .await
                        .map_err(|e| anyhow::anyhow!("{e}"))
                }),
            )
            .await
            .ok()
            .and_then(|r| r.ok())
            .unwrap_or(0),
            None => 0,
        };

        let bridge = RedisRpcBridge {
            redis: redis.clone(),
            rpc_pool: rpc_pool.clone(),
        };
        for c in tick
            .routes
            .iter()
            .filter(|c| c.route_kind == RouteKind::Triangular)
        {
            let Some(legs) = cycle_legs(c) else {
                warn!(
                    event = "route_discovery.adapter_malformed_route",
                    chain_id,
                    route_hash = %c.route_hash,
                    "ragged candidate vectors — adapter pass skipped (R8)"
                );
                continue;
            };
            match ensure_reserves(
                &bridge,
                &adapter_budget,
                chain_id,
                current_block,
                now_ts,
                &legs,
            )
            .await
            {
                AdapterOutcome::AllFresh => adapter_cache_hit += 1,
                AdapterOutcome::Backfilled(n) => {
                    adapter_backfill_ok += 1;
                    debug!(
                        event = "route_discovery.adapter_backfilled",
                        chain_id,
                        legs_written = n
                    );
                }
                AdapterOutcome::Skipped(reason) => {
                    match reason {
                        Skipped::BudgetExhausted => adapter_budget_exhausted += 1,
                        Skipped::MissingReserves(_) => adapter_backfill_fail += 1,
                    }
                    // The gate must know EVERY skipped route (exact
                    // tx_hash == route_hash match); the per-route EVENT is
                    // capped so a large triangular tick cannot flood the
                    // telemetry channel (R9).
                    if let Ok(h) = H256::from_str(&c.route_hash) {
                        skipped_hashes.push(h);
                    }
                    if adapter_events.len() < cfg.max_telemetry_per_tick {
                        let pool_hex = match reason {
                            Skipped::MissingReserves(pool) => Some(format!("{pool:#x}")),
                            Skipped::BudgetExhausted => None,
                        };
                        adapter_events.push(telemetry::adapter_skipped_event(
                            chain_id,
                            &c.route_hash,
                            reason.as_str(),
                            pool_hex.as_deref(),
                        ));
                    }
                }
            }
        }

        // Gate: drop dispatch for adapter-skipped routes BEFORE any spawn.
        // Triangular plans are still deferred telemetry-only today (PR #350
        // owns building their intents), so nothing is dropped yet — this is
        // the enforcement point that dispatch flows through once #350 lands.
        if !skipped_hashes.is_empty() {
            let (kept, dropped) =
                gate_dispatch_intents(std::mem::take(&mut tick.dispatch_intents), &skipped_hashes);
            tick.dispatch_intents = kept;
            if dropped > 0 {
                tick.routes_dispatched = tick.routes_dispatched.saturating_sub(dropped);
                tick.tick_summary["routes_dispatched"] = serde_json::json!(tick.routes_dispatched);
            }
        }

        // Adapter counters ride the tick summary (same injection pattern as
        // the multi_hop_* fields).
        tick.tick_summary["adapter_cache_hit"] = serde_json::json!(adapter_cache_hit);
        tick.tick_summary["adapter_backfill_ok"] = serde_json::json!(adapter_backfill_ok);
        tick.tick_summary["adapter_backfill_fail"] = serde_json::json!(adapter_backfill_fail);
        tick.tick_summary["adapter_budget_exhausted"] = serde_json::json!(adapter_budget_exhausted);

        for ev in &tick.events {
            telemetry::publish(&mut redis, ev).await;
        }
        for ev in &adapter_events {
            telemetry::publish(&mut redis, ev).await;
        }
        telemetry::publish(&mut redis, &tick.tick_summary).await;

        let routes_found = tick.routes_found;
        let routes_dispatched = tick.routes_dispatched;
        let telemetry_emitted = tick.telemetry_emitted;

        // Execute the planned evaluations. When an ACTIVE orchestrator is wired
        // (cartridge_mode=Active), route closed-cycle candidates DIRECTLY to the
        // canonical cartridge runtime via Orchestrator::spawn_cartridge_eval —
        // each cartridge evaluates the cycle and emits its OWN strategy_kind (its
        // .rhai stem), with no native-engine duplicate. Falls back to
        // shadow_evaluate_intent (observe-only, never writes opps:detected) when
        // no active orchestrator is present.
        if let Some(orch) = &orchestrator {
            for intent in tick.dispatch_intents {
                orch.spawn_cartridge_eval(intent);
            }
        } else if let Some(r) = &runner {
            for intent in tick.dispatch_intents {
                tokio::spawn(shadow_evaluate_intent(r.clone(), intent, chain_id));
            }
        }

        info!(
            event = "route_discovery.tick",
            chain_id,
            pools_total = outcome.pools_total,
            edges_built = outcome.graph.edges.len(),
            edges_rejected = outcome.rejected.len(),
            routes_found,
            routes_dispatched,
            telemetry_emitted,
            latency_ms,
        );
    }
}

/// Spawn the route-discovery worker, gated by `ARBX_ROUTE_DISCOVERY_MODE`.
///
/// `Off` (the default) ⇒ nothing is spawned (zero overhead). `Shadow` requires
/// an `ImpactIndex` (the pool source); when absent (orchestrator off), the
/// worker is skipped with an honest reason rather than fabricating a graph.
pub fn spawn_route_discovery(
    chain_id: u64,
    redis: ConnectionManager,
    rpc_pool: Option<Arc<HttpRpcPool>>,
    impact_index: Option<Arc<RwLock<ImpactIndex>>>,
    runner: Option<Arc<CartridgeRunner>>,
    orchestrator: Option<Arc<Orchestrator>>,
    cancel: CancellationToken,
) {
    let mode = RouteDiscoveryMode::from_env();
    info!(
        event = "route_discovery.mode",
        chain_id,
        mode = mode.as_str(),
        dispatch_enabled = runner.is_some(),
        adapter_backfill_rpc = rpc_pool.is_some()
    );
    if !mode.is_enabled() {
        return; // off → dormant, nothing spawned
    }
    if rpc_pool.is_none() {
        // FASE 2 adapter honesty note: without an HTTP RPC pool (non-mainnet /
        // RPC_HTTP_* unset) the adapter serves cache-fresh legs only — every
        // miss/stale leg skips as `missing_reserves` instead of backfilling.
        warn!(
            event = "route_discovery.adapter_no_rpc_pool",
            chain_id,
            "triangular reserves adapter runs cache-read-only — backfills disabled (honest degradation)"
        );
    }

    let impact_index = match impact_index {
        Some(i) => i,
        None => {
            warn!(
                event = "route_discovery.skipped",
                chain_id,
                reason = "no_impact_index",
                "route discovery enabled but ImpactIndex unavailable (orchestrator off) — R8 fail-honest"
            );
            return;
        }
    };

    let engine = StrategyApplicabilityEngine::load_or_default(&config_path());
    let cfg = WorkerConfig::from_env_and_engine(&engine);
    info!(
        event = "route_discovery.config",
        chain_id,
        interval_ms = cfg.interval_ms,
        max_routes_per_tick = cfg.finder.max_routes_per_tick,
        max_depth = cfg.finder.max_depth,
        max_pools_per_pair = cfg.finder.max_pools_per_pair,
        max_telemetry_per_tick = cfg.max_telemetry_per_tick,
        base_tokens = cfg.finder.base_tokens.len(),
    );

    tokio::spawn(async move {
        run_loop(
            redis,
            chain_id,
            rpc_pool,
            impact_index,
            engine,
            cfg,
            runner,
            orchestrator,
            cancel,
        )
        .await;
    });
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use crate::route_discovery::graph_builder::TokenGraph;
    use crate::route_discovery::types::{RouteDirection, RouteEdge};
    use crate::route_intent::ProtocolType;
    use std::collections::HashMap;

    fn addr(n: u64) -> Address {
        Address::from_low_u64_be(n)
    }

    fn dir_edge(pool: u64, ti: u64, to: u64, t0: u64, t1: u64, proto: ProtocolType) -> RouteEdge {
        RouteEdge {
            chain_id: 1,
            pool: addr(pool),
            token_in: addr(ti),
            token_out: addr(to),
            token0: addr(t0),
            token1: addr(t1),
            protocol: proto,
            fee_bps: Some(30),
            liquidity_hint: Some(1.0),
            log_weight: None,
            freshness_ts: 0,
            blk: 0,
            hot_token: false,
            direction: RouteDirection::from_in_token0(addr(ti), addr(t0)),
        }
    }

    /// Two V2 pools over (A,B) → a 2-cycle graph, plus one rejected pool.
    fn outcome_two_v2() -> GraphBuildOutcome {
        use crate::route_discovery::graph_builder::RejectedEdge;
        let mut edges = Vec::new();
        for &(p, t0, t1) in &[(0x10u64, 1u64, 2u64), (0x20, 1, 2)] {
            edges.push(dir_edge(p, t0, t1, t0, t1, ProtocolType::V2));
            edges.push(dir_edge(p, t1, t0, t0, t1, ProtocolType::V2));
        }
        let mut adjacency: HashMap<Address, Vec<usize>> = HashMap::new();
        for (i, e) in edges.iter().enumerate() {
            adjacency.entry(e.token_in).or_default().push(i);
        }
        GraphBuildOutcome {
            graph: TokenGraph { edges, adjacency },
            rejected: vec![RejectedEdge {
                pool: addr(0x99),
                reason: "missing_reserves".to_string(),
            }],
            pools_total: 3,
        }
    }

    #[test]
    fn evaluate_tick_annotates_and_emits() {
        let outcome = outcome_two_v2();
        let engine = StrategyApplicabilityEngine::default();
        let finder = RouteFinderConfig::default();
        let tick = evaluate_tick(&outcome, 1, &engine, &finder, 200, false, 42, "shadow");

        // Two opposite-order 2-cycles found.
        assert_eq!(tick.routes_found, 2);
        for r in &tick.routes {
            // Each annotated with applicability (dex_arb + flashloan apply to v2v2).
            assert!(r
                .applicable_strategies
                .iter()
                .any(|s| matches!(s, crate::strategy_label::StrategyLabel::DexArbV2V2)));
            assert!(!r.rejected_strategies.is_empty());
        }

        // Events: 2 candidate + 2 applicability + 1 rejected = 5.
        assert_eq!(tick.events.len(), 5);
        assert!(tick
            .events
            .iter()
            .any(|e| e["event"] == "route_discovery.rejected"));

        // Tick summary carries the algorithm + counts.
        assert_eq!(tick.tick_summary["event"], "route_discovery.tick");
        assert_eq!(tick.tick_summary["algorithm"], "dfs_bounded");
        assert_eq!(tick.tick_summary["routes_found"], 2);
        assert_eq!(tick.tick_summary["routes_dispatched"], 0);
        assert_eq!(tick.tick_summary["pools_total"], 3);
        assert_eq!(tick.tick_summary["edges_rejected"], 1);
        assert_eq!(tick.tick_summary["latency_ms"], 42);
        assert_eq!(tick.tick_summary["mode"], "shadow");

        // Multi-hop pass is wired + observe-only: field present, bounded by the cap,
        // and surfaced in the tick summary (R8 honest counters).
        assert!(tick.multi_hop_cycles_found <= finder.max_routes_per_tick);
        assert!(tick
            .tick_summary
            .get("multi_hop_profitable_cycles")
            .is_some());
        assert!(tick.tick_summary.get("multi_hop_v3_skipped").is_some());
    }

    #[test]
    fn evaluate_tick_respects_telemetry_cap() {
        let outcome = outcome_two_v2();
        let engine = StrategyApplicabilityEngine::default();
        let finder = RouteFinderConfig::default();
        // Cap at 1 candidate worth of telemetry.
        let tick = evaluate_tick(&outcome, 1, &engine, &finder, 1, false, 0, "shadow");
        assert_eq!(tick.telemetry_emitted, 1);
        // 1 candidate (2 events) + 1 rejected (rejected uses its own budget) = 3.
        let candidate_events = tick
            .events
            .iter()
            .filter(|e| e["event"] == "route_discovery.route_candidate")
            .count();
        assert_eq!(candidate_events, 1, "candidate telemetry capped at 1");
    }

    #[test]
    fn no_event_targets_opps_detected() {
        // Static guarantee: the worker's tick produces ONLY route_discovery.* /
        // route_intent.emitted events — never anything aimed at opps:detected.
        // Exercised with dispatch ENABLED so route_intent.emitted is covered too.
        let outcome = outcome_two_v2();
        let engine = StrategyApplicabilityEngine::default();
        let finder = RouteFinderConfig::default();
        let tick = evaluate_tick(&outcome, 1, &engine, &finder, 200, true, 0, "shadow");
        for e in tick
            .events
            .iter()
            .chain(std::iter::once(&tick.tick_summary))
        {
            let name = e["event"].as_str().unwrap_or_default();
            assert!(
                name.starts_with("route_discovery.") || name == "route_intent.emitted",
                "unexpected event {name}"
            );
            let serialized = serde_json::to_string(e).unwrap();
            assert!(!serialized.contains("opps:detected"));
        }
    }

    #[test]
    fn dispatch_enabled_plans_dex_arb_and_counts() {
        let outcome = outcome_two_v2();
        let engine = StrategyApplicabilityEngine::default();
        let finder = RouteFinderConfig::default();
        let tick = evaluate_tick(&outcome, 1, &engine, &finder, 200, true, 7, "shadow");
        // PR-ROUTE-04: Two V2V2 routes × {dex_arb, flashloan_arb} = 4 dispatched.
        // flashloan now dispatches via the omega_strategy_pack (polymorphic catch-all).
        assert_eq!(tick.routes_dispatched, 4);
        assert_eq!(tick.dispatch_intents.len(), 4);
        assert_eq!(tick.tick_summary["routes_dispatched"], 4);
        // route_intent.emitted events present, all shadow, none deferred.
        let emitted: Vec<_> = tick
            .events
            .iter()
            .filter(|e| e["event"] == "route_intent.emitted")
            .collect();
        assert_eq!(emitted.len(), 4);
        for e in emitted {
            assert_eq!(e["mode"], "shadow");
            assert!(e["dispatch_deferred"].is_null());
            // PR-ROUTE-04: both dex_arb_v2v2 and flashloan_arb strategies emit.
            let strat = e["strategy"].as_str().unwrap_or_default();
            assert!(
                strat == "dex_arb_v2v2" || strat == "flashloan_arb",
                "unexpected strategy {strat}"
            );
        }
    }

    #[test]
    fn base_token_parsing_skips_garbage() {
        let parsed = parse_base_tokens(&[
            format!("{:#x}", addr(1)),
            "not-an-address".to_string(),
            format!("{:#x}", addr(2)),
        ]);
        assert_eq!(parsed.len(), 2);
        assert_eq!(parsed[0], addr(1));
    }

    /// XLS-QB-03 (workbook step 9): the selected strategy's HopMask bounds the
    /// multi-hop expansion before enumeration, observably in the tick summary.
    /// MEV-01-015's mask (1) admits hop 2 only ⇒ the effective span narrows to
    /// (2,2) even though the knob range is (2,3). Default (None) keeps the
    /// knob span; an unknown MEV_ID skips honestly (R8).
    #[test]
    fn evaluate_tick_applies_strategy_hop_mask() {
        let outcome = outcome_two_v2();
        let engine = StrategyApplicabilityEngine::default();
        let base = RouteFinderConfig::default(); // min 2, max 3

        let mut finder = base.clone();
        finder.hop_mask_strategy_id = Some("MEV-01-015".to_string()); // mask 1 → hop 2 only
        let tick = evaluate_tick(&outcome, 1, &engine, &finder, 200, false, 0, "shadow");
        assert_eq!(tick.tick_summary["multi_hop_mask_strategy"], "MEV-01-015");
        assert_eq!(
            tick.tick_summary["multi_hop_hops_effective"],
            serde_json::json!((2u8, 2u8))
        );
        assert_eq!(tick.tick_summary["multi_hop_mask_skip"], false);

        let tick = evaluate_tick(&outcome, 1, &engine, &base, 200, false, 0, "shadow");
        assert!(tick.tick_summary["multi_hop_mask_strategy"].is_null());
        assert_eq!(
            tick.tick_summary["multi_hop_hops_effective"],
            serde_json::json!((2u8, 3u8))
        );
        assert_eq!(tick.tick_summary["multi_hop_mask_skip"], false);

        let mut finder = base.clone();
        finder.hop_mask_strategy_id = Some("MEV-99-999".to_string()); // unknown → honest skip
        let tick = evaluate_tick(&outcome, 1, &engine, &finder, 200, false, 0, "shadow");
        assert_eq!(tick.tick_summary["multi_hop_mask_skip"], true);
        assert_eq!(tick.multi_hop_cycles_found, 0);
    }
}
