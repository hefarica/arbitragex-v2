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
use crate::route_discovery::graph_builder::{build_graph, GraphBuildConfig, GraphBuildOutcome};
use crate::route_discovery::route_intent_dispatcher::plan_dispatch;
use crate::route_discovery::strategy_applicability::StrategyApplicabilityEngine;
use crate::route_discovery::telemetry;
use crate::route_discovery::types::RouteCandidate;
use crate::route_discovery::unique_route_finder::{find_routes, RouteFinderConfig};
use crate::route_discovery::RouteDiscoveryMode;
use crate::route_intent::RouteIntent;
use ethers::types::Address;
use redis::aio::ConnectionManager;
use serde_json::Value;
use std::path::PathBuf;
use std::str::FromStr;
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tokio::sync::RwLock;
use tokio_util::sync::CancellationToken;
use tracing::{info, warn};

/// Phase-1 algorithm tag stamped onto all telemetry.
pub const ALGORITHM: &str = "dfs_bounded";

const DEFAULT_INTERVAL_MS: u64 = 12_000;
const DEFAULT_MAX_ROUTES_PER_TICK: usize = 500;
const DEFAULT_MAX_TELEMETRY_PER_TICK: usize = 200;
const DEFAULT_MAX_AGE_SECS: u64 = 120;

fn env_usize(key: &str, default: usize) -> usize {
    std::env::var(key).ok().and_then(|v| v.parse().ok()).unwrap_or(default)
}
fn env_u64(key: &str, default: u64) -> u64 {
    std::env::var(key).ok().and_then(|v| v.parse().ok()).unwrap_or(default)
}
fn env_u8(key: &str, default: u8) -> u8 {
    std::env::var(key).ok().and_then(|v| v.parse().ok()).unwrap_or(default)
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
        let max_routes =
            env_usize("ARBX_ROUTE_DISCOVERY_MAX_ROUTES_PER_TICK", DEFAULT_MAX_ROUTES_PER_TICK);
        let max_telemetry = env_usize(
            "ARBX_ROUTE_DISCOVERY_MAX_TELEMETRY_PER_TICK",
            DEFAULT_MAX_TELEMETRY_PER_TICK,
        );
        // env > yaml > built-in default for the two finder bounds.
        let max_pools_per_pair = env_usize(
            "ARBX_ROUTE_DISCOVERY_MAX_POOLS_PER_PAIR",
            disc.max_pools_per_pair.max(1),
        );
        let max_depth = env_u8("ARBX_ROUTE_DISCOVERY_MAX_DEPTH", disc.max_depth.max(2));
        let interval_ms = env_u64("ARBX_ROUTE_DISCOVERY_INTERVAL_MS", DEFAULT_INTERVAL_MS);
        let max_age_secs = env_u64("ARBX_ROUTE_DISCOVERY_MAX_AGE_SECS", DEFAULT_MAX_AGE_SECS);

        Self {
            interval_ms,
            max_telemetry_per_tick: max_telemetry,
            graph: GraphBuildConfig {
                max_age_secs,
                min_liquidity_hint: disc.min_liquidity_hint,
            },
            finder: RouteFinderConfig {
                max_depth,
                max_pools_per_pair,
                max_routes_per_tick: max_routes,
                base_tokens: parse_base_tokens(&disc.base_tokens),
                mode: "shadow".to_string(),
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
    let mut rejected_emitted = 0usize;
    for r in outcome.rejected.iter() {
        if rejected_emitted >= max_telemetry_per_tick {
            break;
        }
        events.push(telemetry::rejected_event(chain_id, r));
        rejected_emitted += 1;
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

    let tick_summary = telemetry::tick_event(
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
        latency_ms,
        mode,
    );

    TickOutput {
        routes_found: routes.len(),
        routes,
        events,
        tick_summary,
        dispatch_intents,
        routes_dispatched,
        telemetry_emitted,
    }
}

/// The async loop: tick on an interval until `cancel` fires.
async fn run_loop(
    mut redis: ConnectionManager,
    chain_id: u64,
    impact_index: Arc<RwLock<ImpactIndex>>,
    engine: StrategyApplicabilityEngine,
    cfg: WorkerConfig,
    runner: Option<Arc<CartridgeRunner>>,
    cancel: CancellationToken,
) {
    let dispatch_enabled = runner.is_some();
    let mut ticker = tokio::time::interval(Duration::from_millis(cfg.interval_ms.max(1_000)));
    ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    info!(
        event = "route_discovery.started",
        chain_id,
        interval_ms = cfg.interval_ms,
        max_depth = cfg.finder.max_depth,
        max_routes_per_tick = cfg.finder.max_routes_per_tick,
        max_pools_per_pair = cfg.finder.max_pools_per_pair,
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

        let tick = evaluate_tick(
            &outcome,
            chain_id,
            &engine,
            &cfg.finder,
            cfg.max_telemetry_per_tick,
            dispatch_enabled,
            latency_ms,
            "shadow",
        );

        for ev in &tick.events {
            telemetry::publish(&mut redis, ev).await;
        }
        telemetry::publish(&mut redis, &tick.tick_summary).await;

        let routes_found = tick.routes_found;
        let routes_dispatched = tick.routes_dispatched;
        let telemetry_emitted = tick.telemetry_emitted;

        // Execute the planned shadow evaluations. THE ONLY downstream is
        // shadow_evaluate_intent (observe-only); spawned fire-and-forget exactly
        // like the orchestrator's shadow fan-out. It never writes opps:detected.
        if let Some(r) = &runner {
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
    impact_index: Option<Arc<RwLock<ImpactIndex>>>,
    runner: Option<Arc<CartridgeRunner>>,
    cancel: CancellationToken,
) {
    let mode = RouteDiscoveryMode::from_env();
    info!(
        event = "route_discovery.mode",
        chain_id,
        mode = mode.as_str(),
        dispatch_enabled = runner.is_some()
    );
    if !mode.is_enabled() {
        return; // off → dormant, nothing spawned
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
        run_loop(redis, chain_id, impact_index, engine, cfg, runner, cancel).await;
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
        for e in tick.events.iter().chain(std::iter::once(&tick.tick_summary)) {
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
        // Two V2V2 routes → dex_arb dispatched for each (flashloan has no cartridge).
        assert_eq!(tick.routes_dispatched, 2);
        assert_eq!(tick.dispatch_intents.len(), 2);
        assert_eq!(tick.tick_summary["routes_dispatched"], 2);
        // route_intent.emitted events present, all shadow, none deferred (dex_arb).
        let emitted: Vec<_> = tick
            .events
            .iter()
            .filter(|e| e["event"] == "route_intent.emitted")
            .collect();
        assert_eq!(emitted.len(), 2);
        for e in emitted {
            assert_eq!(e["mode"], "shadow");
            assert!(e["dispatch_deferred"].is_null());
            assert_eq!(e["strategy"], "dex_arb_v2v2");
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
}
