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
use crate::cycle_index::CycleIndex;
use crate::dirty_consumer::{DirtyDrain, OnceSource};
use crate::dirty_signal;
use crate::impact_index::ImpactIndex;
use crate::latency_budget::{LatencyLog, Stage};
use crate::orchestrator::Orchestrator;
use crate::pair_index::{DenseIdBuilder, TokenKey};
use crate::route_discovery::graph_builder::{build_graph, GraphBuildConfig, GraphBuildOutcome};
use crate::route_discovery::lat_candidates::{self, CandidateSample};
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
    /// ARBX-FE-EMIT-09: per-candidate stage samples in `routes` order
    /// (annotation + dispatch planning → `gates_us`; the run loop's adapter
    /// pass enriches `reprice_us` in place). EMPTY whenever `phases` was
    /// `None` — the pure `evaluate_tick` contract stays clock-free AND
    /// sample-free. Selection to top-K + wire injection happens in the run
    /// loop, never here.
    pub lat_candidate_samples: Vec<CandidateSample>,
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
    evaluate_tick_phased(
        outcome,
        chain_id,
        engine,
        finder,
        max_telemetry_per_tick,
        dispatch_enabled,
        latency_ms,
        mode,
        None,
    )
}

/// XLS-QB-07b (workbook 10_LATENCY): `evaluate_tick` + per-stage latency
/// sampling. The `phases` sink receives `(Stage, elapsed_micros)` at each
/// compute phase that maps to a budget row: the 2-leg `find_routes`
/// emissions → `Pair` (ARBX-0010 `DiscoveryTimings` split — see the
/// pair-split note inside) with the remainder of both enumerations →
/// `Expand`, applicability annotation + dispatch planning → `Gates`,
/// event building → `Emit`. `None` keeps the function clock-free and
/// deterministic (the pure `evaluate_tick` contract the unit tests rely
/// on); `Some` is the async loop's real-`Instant` wiring. EMIT-09: `Some`
/// ALSO enables per-candidate capture — the samples ride
/// `TickOutput::lat_candidate_samples`, never the stage sink.
#[allow(clippy::too_many_arguments)]
pub fn evaluate_tick_phased(
    outcome: &GraphBuildOutcome,
    chain_id: u64,
    engine: &StrategyApplicabilityEngine,
    finder: &RouteFinderConfig,
    max_telemetry_per_tick: usize,
    dispatch_enabled: bool,
    latency_ms: u64,
    mode: &str,
    mut phases: Option<&mut dyn FnMut(Stage, u64)>,
) -> TickOutput {
    let t_enum = Instant::now();
    let found = find_routes(&outcome.graph, chain_id, finder);
    // ARBX-0010 pair-split: `DiscoveryTimings` attributes the wall-time
    // segment between two consecutive DFS emissions to the hop class of the
    // cycle emitted LAST (l == 2 ⇒ Pair). The recorder contract is
    // MICROSECONDS, so the nanosecond split converts once and Expand keeps
    // `total − pair`: the identity Pair + Expand == the wall time that
    // previously landed entirely in Expand holds exactly (sub-µs pair
    // segments truncate to an honest 0).
    let total_enum_us = t_enum.elapsed().as_micros() as u64;
    let pair_us = (found.timings.pair_ns / 1_000) as u64;
    let mut routes = found.routes;
    if let Some(sink) = phases.as_deref_mut() {
        sink(Stage::Pair, pair_us);
        sink(Stage::Expand, total_enum_us.saturating_sub(pair_us));
    }

    // Annotate each candidate with strategy applicability.
    // EMIT-09: the same `phases.is_some()` that arms the stage sink arms
    // per-candidate capture — one flag, one contract (pure ticks stay
    // clock-free AND sample-free).
    let capture = phases.is_some();
    let mut lat_samples: Vec<CandidateSample> = Vec::new();
    let t_gates = Instant::now();
    for c in routes.iter_mut() {
        let t_c = capture.then(Instant::now);
        let appl = engine.evaluate(c.route_kind);
        c.applicable_strategies = appl.applicable;
        c.rejected_strategies = appl.rejected;
        if let Some(t) = t_c {
            lat_samples.push(CandidateSample {
                route_hash: c.route_hash.clone(),
                route_kind: c.route_kind.as_str().to_string(),
                hops: c.hops,
                gates_us: t.elapsed().as_micros() as u64,
                reprice_us: None,
            });
        }
    }
    if let Some(sink) = phases.as_deref_mut() {
        sink(Stage::Gates, t_gates.elapsed().as_micros() as u64);
    }

    let t_emit = Instant::now();
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
    if let Some(sink) = phases.as_deref_mut() {
        sink(Stage::Emit, t_emit.elapsed().as_micros() as u64);
    }

    // Dispatch planning (only when a cartridge runner is available). Capped at
    // max_telemetry_per_tick so neither shadow-eval spawns nor route_intent.
    // emitted events explode under a large graph.
    let t_plan = Instant::now();
    let mut dispatch_intents: Vec<RouteIntent> = Vec::new();
    let mut routes_dispatched = 0usize;
    if dispatch_enabled {
        let mut dispatch_budget = 0usize;
        'routes: for (ri, c) in routes.iter().enumerate() {
            // EMIT-09: time the planning CALL (the plans' downstream event
            // pushes are Emit, not planning) and record BEFORE iterating —
            // the budget `break 'routes` cannot lose the record.
            let t_plan_c = capture.then(Instant::now);
            let plans = plan_dispatch(c, engine);
            if let Some(t) = t_plan_c {
                // Index invariant: the annotation loop pushed exactly one
                // sample per route in this same `routes` order.
                if let Some(s) = lat_samples.get_mut(ri) {
                    s.gates_us += t.elapsed().as_micros() as u64;
                }
            }
            for plan in plans {
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
    if let Some(sink) = phases.as_deref_mut() {
        sink(Stage::Gates, t_plan.elapsed().as_micros() as u64);
    }

    // Multi-hop profitable-cycle pass (additive, observe-only). Runs the
    // negative-`log_weight` cycle finder over the SAME graph snapshot, bounded by
    // the already-configured DFS depth (no new explosion surface) and the per-tick
    // route cap. Fail-honest: V3 (None-weight) legs are skipped inside the finder.
    // XLS-CANON-01: Min_Hops/Max_Hops knobs flow through the finder bounds.
    let t_mh = Instant::now();
    let mh_max_hops = (finder.max_depth as usize).clamp(2, 7);
    let mh_min_hops = (finder.min_depth as usize).clamp(2, mh_max_hops);
    // XLS-QB-03 (workbook step 9 StrategyMask) + ARBX-0021 (col Status): the
    // selected strategy's workbook Status decides WHETHER the pass may expand
    // at all (NEEDS_ROUTE_DATA / NO_COMPATIBLE_ROUTE / unknown ⇒ no expansion
    // — a strategy without route data never fabricates cycles to look busy),
    // then its HopMask bounds the expansion — both O(1) lookups per tick,
    // BEFORE enumeration. Empty intersection ⇒ honest skip with a reason (R8),
    // never a silently-empty search.
    let mh_status = finder
        .hop_mask_strategy_id
        .as_ref()
        .map(|id| crate::strategy_dispatch_status::disposition(id));
    // ARBX-0026 (sheet 13_DETECTOR_POLICY): the strategy's DETECTOR policy —
    // the family hop envelope INTERSECTS the HopMask bounds so a strategy can
    // never escape its family; clamped/emptied keep any envelope bite
    // observable (canonical workbook data: Allowed_Hops ⊆ Hop_Use ⇒ identity).
    let mh_policy = finder
        .hop_mask_strategy_id
        .as_deref()
        .and_then(crate::detector_policy::policy_for_strategy);
    let (mh_mask_strategy, mh_bounds, mh_family_clamped, mh_family_emptied) =
        match &finder.hop_mask_strategy_id {
            Some(id) if mh_status.is_some_and(|d| d.may_expand()) => {
                let strat = crate::strategy_hop_mask::admissible_hop_bounds(
                    id,
                    mh_min_hops as u8,
                    mh_max_hops as u8,
                );
                let enveloped =
                    strat.and_then(|b| crate::detector_policy::envelope_hop_bounds(id, Some(b)));
                (
                    Some(id.clone()),
                    enveloped,
                    strat != enveloped && enveloped.is_some(),
                    strat.is_some() && enveloped.is_none(),
                )
            }
            // Status forbids expansion (or the MEV_ID is unknown) — reason lands
            // in tick_summary below.
            Some(id) => (Some(id.clone()), None, false, false),
            None => (
                None,
                Some((mh_min_hops as u8, mh_max_hops as u8)),
                false,
                false,
            ),
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
    if let Some(sink) = phases {
        sink(Stage::Expand, t_mh.elapsed().as_micros() as u64);
    }

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
    // ARBX-0021: the selected strategy's workbook Status disposition + the
    // derived per-status census (79/174/8/3 COMPUTED from the generated
    // table — workbook drift changes these numbers, never a hardcoded list).
    tick_summary["multi_hop_status"] = serde_json::json!(mh_status.map(|d| d.reason()));
    tick_summary["multi_hop_status_skip_reason"] =
        serde_json::json!(mh_status.filter(|d| !d.may_expand()).map(|d| d.reason()));
    // ARBX-TW-005: the selected strategy's Execution_Class — execution-
    // precondition annotation. When the skip reason is needs_route_data, the
    // class is the SPECIFIC missing precondition (R8: "blocked" is
    // observable as WHAT it needs, e.g. NONATOMIC_BRIDGE_REQUIRED), not
    // generic noise.
    tick_summary["multi_hop_execution_class"] = serde_json::json!(finder
        .hop_mask_strategy_id
        .as_deref()
        .and_then(crate::strategy_execution_class::execution_class));
    tick_summary["multi_hop_needs_class"] = serde_json::json!(if mh_status
        == Some(crate::strategy_dispatch_status::Disposition::NoCandidateNeedsRouteData)
    {
        finder
            .hop_mask_strategy_id
            .as_deref()
            .and_then(crate::strategy_execution_class::execution_class)
    } else {
        None
    });
    // ARBX-0026 (sheet 13_DETECTOR_POLICY): the four policy dimensions of the
    // selected strategy's detector, consumed GENERICALLY (no per-detector
    // branch) — graph family, family hop envelope, hot-seed admission and
    // the universal Do_Not_Do guard. `family_clamped` stays false while the
    // workbook's Allowed_Hops ⊆ Hop_Use invariant holds (drift tripwire).
    tick_summary["multi_hop_detector"] = serde_json::json!(mh_policy.map(|p| p.detector_id));
    tick_summary["multi_hop_graph_policy"] =
        serde_json::json!(mh_policy.map(|p| p.graph_policy.as_str()));
    tick_summary["multi_hop_family_hops"] = serde_json::json!(mh_policy.map(|p| p.hop_bounds()));
    tick_summary["multi_hop_hot_seed"] = serde_json::json!(mh_policy.map(|p| p.hot_seed.as_str()));
    tick_summary["multi_hop_may_seed"] =
        serde_json::json!(mh_policy.map(|p| p.hot_seed.may_seed()));
    tick_summary["multi_hop_do_not"] =
        serde_json::json!(mh_policy.map(|_| crate::detector_policy::do_not_rules()[0]));
    tick_summary["multi_hop_family_clamped"] = serde_json::json!(mh_family_clamped);
    tick_summary["multi_hop_family_skip_reason"] =
        serde_json::json!(mh_family_emptied.then_some("family_envelope_empty"));
    // ARBX-DP-002 (sheet 13 Required_Data): data-availability gate for the
    // selected detector, measured from THIS tick's pre-math build artifacts
    // (the graph the search just consumed). NeedsData ⇒ zero admitted pools
    // ⇒ that search necessarily yielded nothing — the gate makes the WHY
    // observable (R8) and the Do_Not_Do guard stands: never an approximation
    // substitute. Surfaces without a runtime adapter report `not_tracked`.
    let rd_coverage = crate::required_data_gate::TickDataCoverage::from_counts(
        outcome.pools_total,
        outcome.rejected.len(),
    );
    tick_summary["required_data_gate"] = serde_json::json!(mh_policy.map(|p| {
        let v = crate::required_data_gate::verdict(p, &rd_coverage);
        serde_json::json!({
            "detector": p.detector_id,
            "surface": p.example_surface.as_str(),
            "verdict": v.as_str(),
            "reason": v.reason(),
            "required_data": p.required_data,
            // ARBX-DP-003: the emission-tier feed field (null = honest
            // unknown — class outside the closed 29-token vocabulary).
            "tier": crate::signal_tier::tier_for_execution_class(p.execution_class)
                .map(|t| t.as_str()),
        })
    }));
    // R9 summary census (not per-item logs): why pools yielded no edges.
    if !outcome.rejected.is_empty() {
        let mut census: std::collections::BTreeMap<&str, usize> = std::collections::BTreeMap::new();
        for r in &outcome.rejected {
            *census.entry(r.reason.as_str()).or_insert(0) += 1;
        }
        tick_summary["graph_rejected_reasons"] = serde_json::json!(census);
    }
    // ARBX-DP-004 (HotSeedClassifier → DetectorMask): this tick's event
    // evidence is the dirty-pool drain (reserve updates) — the mask says
    // which of the 60 detectors that kind may wake (dispatch selectivity
    // primitive: never 60 × 264 × pool × block; a bare block admits 0).
    let hs_mask =
        crate::hot_seed_mask::detector_mask(crate::hot_seed_mask::HotSeedEvent::PoolReserveUpdate);
    tick_summary["detector_mask"] = serde_json::json!({
        "event": "pool_reserve_update",
        "admitted": crate::hot_seed_mask::admitted_count(hs_mask),
        "total": crate::detector_policy::DETECTOR_POLICIES.len(),
        "selected_admitted": mh_policy.map(|p| crate::hot_seed_mask::admits_policy(hs_mask, p)),
    });
    let sc = crate::strategy_dispatch_status::status_counts();
    tick_summary["strategy_status_counts"] = serde_json::json!({
        "route_ready": sc[0].1,
        "needs_route_data": sc[1].1,
        "observe_only": sc[2].1,
        "no_compatible_route": sc[3].1,
    });

    TickOutput {
        routes_found: routes.len(),
        routes,
        events,
        tick_summary,
        dispatch_intents,
        routes_dispatched,
        telemetry_emitted,
        multi_hop_cycles_found,
        lat_candidate_samples: lat_samples,
    }
}

/// Record one stage sample into the tick's [`LatencyLog`]. The only failure
/// mode is recording outside a cycle — `run_loop`'s begin/end pairing makes
/// it impossible, so the `Err` is logged at debug and never panics (hot
/// loop; the recorder itself stays pure and testable).
fn lat_record(log: &mut LatencyLog, stage: Stage, t: std::time::Instant) {
    if let Err(e) = log.record(stage, t.elapsed().as_micros() as u64) {
        debug!(event = "route_discovery.latency_record_rejected", stage = stage.key(), err = %e);
    }
}

/// ARBX-XLANG-01: the run loop's per-tick scan telemetry, as plain data.
/// Exists so [`inject_scan_telemetry`] is unit-testable — the wire contract
/// the frontend validates under `.strict()` must be provable in CI, not only
/// observable in prod.
pub(crate) struct ScanTelemetry {
    pub adapter_cache_hit: usize,
    pub adapter_backfill_ok: usize,
    pub adapter_backfill_fail: usize,
    pub adapter_budget_exhausted: usize,
    pub drain_stats: crate::dirty_consumer::DrainStats,
    pub drain_register_reject: usize,
    pub dirty_seeds_now: usize,
    pub adapter_scoped_skip: usize,
    /// F_e prefilter knob — OFF emits NOTHING (dormant is dormant, R8).
    pub fe_prefilter: bool,
    pub fe_prefilter_pass: usize,
    pub fe_prefilter_below_reference: usize,
    pub fe_prefilter_uncomputed: usize,
    pub fe_prefilter_map_fail: usize,
    pub fe_anchor_applied: bool,
    /// R9 dirty re-eval knob — OFF emits nothing.
    pub dirty_reeval: bool,
    pub scoped_reeval_cycles: usize,
    pub scoped_reeval_routes: usize,
    pub scoped_cycle_map_fail: usize,
    pub sla_ms: f64,
    pub lat_candidates_cap: usize,
}

/// ARBX-XLANG-01: inject the scan-conditions telemetry block into the tick
/// summary (adapter counters, drain/dirty histogram, F_e prefilter, scoped
/// re-eval, latency budget KPIs, per-candidate rows). Extracted verbatim from
/// the run loop — its ONLY production caller — so the cross-language wire
/// contract can be golden-tested: the committed fixtures under
/// `frontend/lib/apex/schemas/__tests__/fixtures/route-discovery-tick/` are
/// regenerated from THIS code path and parsed by the frontend Zod mirror
/// (`RouteDiscoveryTickSummarySchema`, `.strict()` — an unknown or
/// type-mismatched key rejects the WHOLE payload; that class of drift once
/// left the telemetry room accepting nothing).
fn inject_scan_telemetry(tick: &mut TickOutput, lat_log: &LatencyLog, s: &ScanTelemetry) {
    // Adapter counters ride the tick summary (same injection pattern as
    // the multi_hop_* fields).
    tick.tick_summary["adapter_cache_hit"] = serde_json::json!(s.adapter_cache_hit);
    tick.tick_summary["adapter_backfill_ok"] = serde_json::json!(s.adapter_backfill_ok);
    tick.tick_summary["adapter_backfill_fail"] = serde_json::json!(s.adapter_backfill_fail);
    tick.tick_summary["adapter_budget_exhausted"] = serde_json::json!(s.adapter_budget_exhausted);
    // ARBX-0003 (XLS-QB-05b/05c/05-009): drain histogram + scoped
    // re-evaluation telemetry — the R9 contract (per-item at debug!,
    // counters here + the ONE info summary below).
    tick.tick_summary["drain_drained"] = serde_json::json!(s.drain_stats.drained);
    tick.tick_summary["drain_unknown_pool"] = serde_json::json!(s.drain_stats.unknown_pool);
    tick.tick_summary["drain_invalid_pair"] = serde_json::json!(s.drain_stats.invalid_pair);
    tick.tick_summary["drain_already_dirty"] = serde_json::json!(s.drain_stats.already_dirty);
    tick.tick_summary["drain_seeded"] = serde_json::json!(s.drain_stats.seeded);
    tick.tick_summary["drain_evicted"] = serde_json::json!(s.drain_stats.evicted);
    tick.tick_summary["drain_register_reject"] = serde_json::json!(s.drain_register_reject);
    tick.tick_summary["dirty_seeds"] = serde_json::json!(s.dirty_seeds_now);
    tick.tick_summary["adapter_scoped_skip"] = serde_json::json!(s.adapter_scoped_skip);
    // ARBX-0024 (REQ-QB-008): F_e prefilter histogram — knob ON only
    // (OFF emits NOTHING: dormant is dormant). The regression contract
    // "signal-no-reemplaza-netgate": below_reference routes were never
    // proved unprofitable by the exact net gate — they were signaled
    // out BEFORE the proof; PASS authority never moved.
    if s.fe_prefilter {
        tick.tick_summary["fe_prefilter_evaluated"] = serde_json::json!(
            s.fe_prefilter_pass
                + s.fe_prefilter_below_reference
                + s.fe_prefilter_uncomputed
                + s.fe_prefilter_map_fail
        );
        tick.tick_summary["fe_prefilter_pass"] = serde_json::json!(s.fe_prefilter_pass);
        tick.tick_summary["fe_prefilter_below_reference"] =
            serde_json::json!(s.fe_prefilter_below_reference);
        tick.tick_summary["fe_prefilter_uncomputed"] = serde_json::json!(s.fe_prefilter_uncomputed);
        tick.tick_summary["fe_prefilter_map_fail"] = serde_json::json!(s.fe_prefilter_map_fail);
        // ARBX-0023: the numéraire the F_e state was filled in — true =
        // the dynamic anchor chosen this tick; false = the stream's raw
        // units (no scoreable anchor / unpriced anchor — fail-open, R8).
        tick.tick_summary["fe_prefilter_anchor_dynamic"] = serde_json::json!(s.fe_anchor_applied);
    }
    if s.dirty_reeval {
        tick.tick_summary["scoped_reeval"] = serde_json::json!(true);
        tick.tick_summary["scoped_reeval_cycles"] = serde_json::json!(s.scoped_reeval_cycles);
        tick.tick_summary["scoped_reeval_routes"] = serde_json::json!(s.scoped_reeval_routes);
        tick.tick_summary["scoped_cycle_map_fail"] = serde_json::json!(s.scoped_cycle_map_fail);
    }

    // XLS-QB-07b (10_LATENCY): budget KPIs ride the tick summary — the
    // `lat.*` Actual columns (windowed p50/p95 + signed headroom) and the
    // PASS gate vs the canonical SLA knob. Honest absence: `null` = not
    // computed (no samples yet), never a fabricated 0. The Emit row and
    // `lat.total` lag one tick: this summary is serialized before this
    // tick's publishes are timed.
    let lat_rows: Vec<Value> = lat_log
        .snapshot()
        .iter()
        .map(|(k, s)| {
            serde_json::json!({
                "key": k,
                "target_ms": s.target_ms,
                "p50_us": s.p50_us,
                // FE-LAT-003: the frontend percentile row needs the full
                // p50/p90/p95/p99 quartet (p90/p99 are NOT workbook
                // 10_LATENCY columns — that sheet pins p50/p95/headroom;
                // these ride the same nearest-rank kernel).
                "p90_us": s.p90_us,
                "p95_us": s.p95_us,
                "p99_us": s.p99_us,
                "headroom_p95_us": s.headroom_p95_us,
            })
        })
        .collect();
    tick.tick_summary["lat_stages"] = serde_json::json!(lat_rows);
    tick.tick_summary["lat_pass_p95"] = serde_json::json!(lat_log.pass_p95(s.sla_ms));
    tick.tick_summary["lat_cycles"] = serde_json::json!(lat_log.cycle_count());
    // ARBX-FE-EMIT-09 (FE-0037 §45 unblocker): per-candidate rows + the
    // once-per-tick honesty block, riding the SAME tick summary the FE
    // already polls (useRouteTick / GET /api/route-discovery/tick) — no
    // new endpoint, no new poll. Selection happens at the caller, after
    // the adapter pass enriched `reprice_us`, so the top-K is over final
    // totals.
    let lat_cand_sel = lat_candidates::select_top_k(
        std::mem::take(&mut tick.lat_candidate_samples),
        s.lat_candidates_cap,
    );
    tick.tick_summary["lat_candidates"] = lat_candidates::rows_value(&lat_cand_sel);
    tick.tick_summary["lat_candidates_meta"] = lat_candidates::meta_value(&lat_cand_sel);
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
    // XLS-QB-07b (workbook 10_LATENCY): the discovery latency budget
    // instrument. Stage mapping for the current periodic scan — 1:1 where a
    // distinct code path exists: Decode = pools snapshot read; State = graph
    // build; Reprice = reserves adapter pass (backfill RPC rides inside it —
    // upper bound until the event-driven split); Expand = both enumerations;
    // Gates = annotation + dispatch planning; Emit = event build + telemetry
    // publishes. `Pair`/`Refine` have NO separate work in the scan → honest
    // 0 (R8: 0 = ran no work; None = not computed). SLA from the canonical
    // knob `discovery_sla_ms` (01_CONFIG r20). Window = bounded telemetry
    // memory only, never a workbook constant.
    let knobs = crate::canonical_knobs::CanonicalKnobs::from_env();
    let dirty_reeval = knobs.dirty_reeval_enabled;
    // ARBX-0024 (REQ-QB-008): F_e cycle prefilter — SIGNAL, never proof.
    let fe_prefilter = knobs.fe_prefilter_enabled;
    let sla_ms = knobs.discovery_sla_ms;
    let lat_window = env_usize("ARBX_ROUTE_DISCOVERY_LATENCY_WINDOW", 512).max(1);
    // EMIT-09: per-candidate rows are a PAYLOAD bound (top-K by total_us),
    // not a sampling bound — capture covers every route. Clamped to >= 1 so a
    // stray 0 cannot disable emission while capture keeps paying its cost.
    let lat_candidates_cap = env_usize(
        "ARBX_ROUTE_DISCOVERY_LAT_CANDIDATES_CAP",
        lat_candidates::DEFAULT_CAP,
    )
    .max(1);
    let mut lat_log = LatencyLog::with_window(lat_window);
    // ── ARBX-0003 (XLS-QB-05b/05c + knob QB-05-009): dirty-pool drain
    // consumer + scoped re-evaluation state. The drain ALWAYS observes
    // (marks + seeds + telemetry); the knob gates ONLY the scoped reserve
    // re-evaluation (XLS-QB-03 default-OFF pattern). Dense token ids live
    // for one builder epoch (pair_index contract): when the pool
    // snapshot's topology signature changes the drain is rebuilt — the
    // mark/queue reset is honest (a new universe is a new state version).
    let drain_queue_cap = env_usize("ARBX_ROUTE_DISCOVERY_DRAIN_QUEUE", 64).max(1);
    let mut drain_state: Option<(DenseIdBuilder, DirtyDrain<OnceSource>)> = None;
    let mut drain_topology_sig: u64 = 0;
    let mut drain_epoch_initialized = false;
    let mut drain_last_block: u64 = 0;
    // ── ARBX-0024 (XLS-QB-06b / REQ-QB-008): F_e prefilter state. Knob OFF
    // (default) ⇒ fully dormant — no Redis reads, no fe_prefilter_* KPIs,
    // deployed behavior identical. When ON the prefilter needs an
    // address→symbol map to bridge the price stream (HGETALL
    // `arbx:token_prices:<chain>`, symbol-keyed) onto the drain epoch's
    // dense ids; symbols are static per address, so the universe is
    // snapshot ONCE PER EPOCH (`reserves::scan_token_universe`, the same
    // ARBX-0018 source token_identity resolves against) — never per tick.
    let mut fe_symbol_by_addr: std::collections::HashMap<String, String> =
        std::collections::HashMap::new();
    let mut fe_symbols_sig: Option<u64> = None;
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

        lat_log.begin_cycle();

        let t0 = Instant::now();
        let t_decode = Instant::now();
        let pools = { impact_index.read().await.all_pools() };
        let now_ts = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        lat_record(&mut lat_log, Stage::Decode, t_decode);

        // ── ARBX-0003: drain the dirty-pool signal (writer side:
        // pool_sync_worker SADDs changed-only pools; its TTL is the set's
        // leak backstop). ONE SMEMBERS per tick, observe-phase —
        // deliberately non-destructive. Deliberately OUTSIDE the latency
        // stage map: no workbook stage claims signal drain, so its cost
        // stays out of every stage's numbers (R8-honest, not hidden).
        let dirty_members: Vec<String> = match redis::cmd("SMEMBERS")
            .arg(dirty_signal::dirty_pools_key(chain_id))
            .query_async(&mut redis)
            .await
        {
            Ok(m) => m,
            Err(e) => {
                debug!(
                    event = "route_discovery.dirty_drain_read_failed",
                    chain_id,
                    err = %e
                );
                Vec::new() // fail-honest: empty drain this tick, never fabricated
            }
        };
        let sig = topology_signature(&pools);
        let mut drain_register_reject = 0usize;
        let drain_stats = if !drain_epoch_initialized || sig != drain_topology_sig {
            // New epoch: the distinct-token count sizes the engine bitset
            // first; pool ids are assigned in registration order.
            let mut seen = std::collections::HashSet::new();
            for p in &pools {
                seen.insert(p.token0);
                seen.insert(p.token1);
            }
            let mut builder = DenseIdBuilder::new();
            let mut drain = DirtyDrain::new(
                seen.len(),
                pools.len(),
                drain_queue_cap,
                OnceSource::new(dirty_members),
            );
            for p in &pools {
                let i = builder.insert(
                    TokenKey {
                        chain_id,
                        address: p.token0,
                    },
                    true,
                );
                let j = builder.insert(
                    TokenKey {
                        chain_id,
                        address: p.token1,
                    },
                    true,
                );
                if let Err(e) = drain.register_pool(&format!("{:#x}", p.address), i, j) {
                    drain_register_reject += 1;
                    debug!(
                        event = "route_discovery.dirty_drain_register_rejected",
                        chain_id,
                        pool = format!("{:#x}", p.address),
                        err = %e
                    );
                }
            }
            drain_topology_sig = sig;
            drain_epoch_initialized = true;
            let stats = drain.drain_tick(chain_id);
            drain_state = Some((builder, drain));
            stats
        } else {
            let (_, drain) = drain_state
                .as_mut()
                .expect("drain_state built on the first tick of the epoch");
            drain.replace_source(OnceSource::new(dirty_members));
            drain.drain_tick(chain_id)
        };

        let t_state = Instant::now();
        let outcome: GraphBuildOutcome =
            build_graph(&mut redis, chain_id, &pools, now_ts, &cfg.graph).await;
        let latency_ms = t0.elapsed().as_millis() as u64;
        lat_record(&mut lat_log, Stage::State, t_state);

        let mut tick = evaluate_tick_phased(
            &outcome,
            chain_id,
            &engine,
            &cfg.finder,
            cfg.max_telemetry_per_tick,
            dispatch_enabled,
            latency_ms,
            "shadow",
            Some(&mut |stage: Stage, micros: u64| {
                if let Err(e) = lat_log.record(stage, micros) {
                    debug!(
                        event = "route_discovery.latency_record_rejected",
                        stage = stage.key(),
                        err = %e
                    );
                }
            }),
        );

        // ── ARBX-0003: scoped reserve re-evaluation (AC: "re-eval SOLO
        // sucios, sin rebuild global"). Knob OFF (default) ⇒ observe-only:
        // the drain marked + seeded above and the adapter below serves
        // every triangular route exactly as today. Knob ON ⇒ the bounded
        // seed queue IS the scope: cycles containing a dirty pair
        // (CycleIndex over THIS tick's triangular routes in dense ids)
        // re-price through the adapter; the rest are skipped with a
        // counted reason (R8 — scoped-out is a decision, never a silent
        // drop). CycleIndex build failure ⇒ fail-OPEN to full service
        // with a warn (never a partial scope).
        let mut scoped_hashes: Option<std::collections::HashSet<String>> = None;
        let mut scoped_reeval_cycles = 0usize;
        let mut scoped_reeval_routes = 0usize;
        let mut scoped_cycle_map_fail = 0usize;
        if dirty_reeval {
            if let Some((builder, drain)) = drain_state.as_mut() {
                let mut seeds = Vec::new();
                while let Some(pair) = drain.pop_seed() {
                    seeds.push(pair);
                }
                if !seeds.is_empty() {
                    let mut cycles: Vec<Vec<usize>> = Vec::new();
                    let mut hash_by_cycle: Vec<String> = Vec::new();
                    for c in tick
                        .routes
                        .iter()
                        .filter(|c| c.route_kind == RouteKind::Triangular)
                    {
                        if let Some(cyc) = dense_cycle(chain_id, &c.tokens, builder) {
                            hash_by_cycle.push(c.route_hash.clone());
                            cycles.push(cyc);
                        }
                    }
                    match CycleIndex::build(builder.len(), cycles) {
                        Ok(ix) => {
                            let affected = ix.affected_cycles(seeds.iter().copied());
                            let set: std::collections::HashSet<String> = hash_by_cycle
                                .iter()
                                .enumerate()
                                .filter(|(ci, _)| affected.contains(ci))
                                .map(|(_, h)| h.clone())
                                .collect();
                            scoped_reeval_cycles = affected.len();
                            scoped_reeval_routes = set.len();
                            scoped_hashes = Some(set);
                        }
                        Err(e) => {
                            scoped_cycle_map_fail = 1;
                            warn!(
                                event = "route_discovery.scoped_cycle_index_failed",
                                chain_id,
                                err = %e
                            );
                        }
                    }
                }
            }
        }

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
        // ARBX-0003: routes the scoped re-evaluation left out (knob ON only).
        let mut adapter_scoped_skip = 0usize;
        // ARBX-0024: F_e prefilter histogram (knob ON only). below_reference
        // is the only dropping reason; pass/uncomputed/map_fail all proceed.
        let mut fe_prefilter_pass = 0usize;
        let mut fe_prefilter_below_reference = 0usize;
        let mut fe_prefilter_uncomputed = 0usize;
        let mut fe_prefilter_map_fail = 0usize;
        // ARBX-0023: whether the F_e state was filled in the DYNAMIC anchor's
        // quote units (vs the stream's raw units — no selection or unpriced
        // anchor). Published with the histogram above.
        let mut fe_anchor_applied = false;
        let mut fe_skipped_hashes: Vec<H256> = Vec::new();
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

        // ARBX-0003: block-anchored state version — marks made before we
        // learned the block moved are reset once (honest 1-tick lag: the
        // block read happens after the drain by Decode/State ordering).
        if current_block != drain_last_block {
            if let Some((_, drain)) = drain_state.as_mut() {
                drain.begin_state_version();
            }
            drain_last_block = current_block;
        }

        let bridge = RedisRpcBridge {
            redis: redis.clone(),
            rpc_pool: rpc_pool.clone(),
        };

        // ── ARBX-0024 (XLS-QB-06b / REQ-QB-008): F_e cycle prefilter inputs.
        // The market context (symbol universe scan — signature-cached per
        // topology epoch — and ONE HGETALL of the price stream per tick,
        // `arbx:token_prices:<chain>`, written by price_worker — Alchemy
        // + Coingecko, no new RPC) is read UNCONDITIONALLY: the F_e prefilter
        // consumes it under its knob, and the quote-anchor publisher
        // (EMIT-02 Layer-2) needs the same priced graph regardless — the
        // anchor rides every tick, never gated by `fe_prefilter` (default
        // OFF). Missing data ⇒ the prefilter fails OPEN (route passes) with
        // a counted reason, never a silent drop (R8); the anchor, with no
        // scoreable token, simply publishes nothing (endpoint 503 honest).
        // Prices anchor to `current_block` via the r25 monotonic advance
        // (0 = fetch failed ⇒ stays put).
        let mut fe_state: Option<crate::fe_normalization::QuoteState> = None;
        let mut fe_rate_by_pool: std::collections::HashMap<
            ethers::core::types::Address,
            Option<f64>,
        > = std::collections::HashMap::new();
        {
            if fe_symbols_sig != Some(sig) {
                match crate::reserves::scan_token_universe(&mut redis, chain_id, 20_000).await {
                    Ok(rows) => {
                        fe_symbol_by_addr =
                            rows.into_iter().map(|t| (t.address, t.symbol)).collect();
                        fe_symbols_sig = Some(sig);
                    }
                    Err(e) => {
                        // One warn per epoch: no retry inside the epoch, the
                        // symbols refresh with the next topology change.
                        fe_symbols_sig = Some(sig);
                        warn!(
                            event = "route_discovery.fe_symbol_universe_failed",
                            chain_id,
                            err = %e
                        );
                    }
                }
            }
            let prices: std::collections::HashMap<String, f64> = match redis::cmd("HGETALL")
                .arg(format!("arbx:token_prices:{}", chain_id))
                // `query_async` is <C, T>: `_` lets the ConnectionManager bind,
                // the turbofish pins T (inference can't see it past filter_map).
                .query_async::<_, std::collections::HashMap<String, String>>(&mut redis)
                .await
            {
                Ok(raw) => raw
                    .into_iter()
                    .filter_map(|(sym, v)| match v.parse::<f64>() {
                        Ok(p) if p.is_finite() && p > 0.0 => Some((sym.to_ascii_uppercase(), p)),
                        _ => None, // poisoned row — skipped, never fabricated
                    })
                    .collect(),
                Err(e) => {
                    debug!(
                        event = "route_discovery.fe_price_read_failed",
                        chain_id,
                        err = %e
                    );
                    std::collections::HashMap::new()
                }
            };
            // ARBX-0023 (05_QUOTE_BASE r13/r16): the anchor selected THIS
            // tick, kept for the F_e evaluation below — that state is filled
            // in the DYNAMIC anchor's quote units (score over any fixed
            // list, r16). `None` (no scoreable token) or an unpriced anchor
            // falls back to the stream's raw units, counted: the numéraire
            // is comparison orientation (r12), never a trading-direction
            // restriction (r15).
            let mut fe_anchor_symbol: Option<String> = None;
            // ── EMIT-02 Layer-2: quote-anchor selection over THIS tick's
            // priced graph (axes derivation + honesty model documented in
            // quote_anchor_runtime). Runs every tick — never gated by the
            // F_e prefilter knob. With no scoreable token the selection is
            // `None` and NOTHING is published (the endpoint 503s honestly
            // once the TTL lapses — R8). Best-effort like the other signal
            // writers: the writer warns, never fails the tick.
            {
                use crate::quote_anchor_runtime::{
                    select_quote_anchor, write_quote_anchor_snapshot, QuoteAnchorEdgeStat,
                };
                use crate::quote_score::QuoteWeights;
                // ONE stat per pool: the first directed edge seen defines the
                // orientation (a↔b) — magnitude fields are per-pool and
                // direction-symmetric except `log_weight`, which the runtime
                // core re-orients per sorted pair key anyway.
                let mut per_pool: std::collections::HashMap<String, QuoteAnchorEdgeStat> =
                    std::collections::HashMap::new();
                for e in &outcome.graph.edges {
                    let pool_hex = format!("{:#x}", e.pool).to_ascii_lowercase();
                    per_pool.entry(pool_hex.clone()).or_insert_with(|| {
                        let proto = match e.protocol {
                            crate::route_intent::ProtocolType::V2 => "v2",
                            crate::route_intent::ProtocolType::V3 => "v3",
                            crate::route_intent::ProtocolType::Curve => "curve",
                            crate::route_intent::ProtocolType::Balancer => "balancer",
                            crate::route_intent::ProtocolType::Unknown => "unknown",
                        };
                        QuoteAnchorEdgeStat {
                            pool: pool_hex,
                            token_a: format!("{:#x}", e.token_in).to_ascii_lowercase(),
                            token_b: format!("{:#x}", e.token_out).to_ascii_lowercase(),
                            protocol: proto.to_string(),
                            fee_bps: e.fee_bps,
                            liquidity_hint: e.liquidity_hint,
                            log_weight: e.log_weight,
                        }
                    });
                }
                let weights = QuoteWeights::from_knobs(&knobs);
                let edges: Vec<QuoteAnchorEdgeStat> = per_pool.into_values().collect();
                if let Some(sel) =
                    select_quote_anchor(chain_id, &edges, &fe_symbol_by_addr, &prices, &weights)
                {
                    // ARBX-0023: the evaluation path consumes the SAME
                    // selection the publisher writes — one chooser, one
                    // anchor, no divergence between what the API reports and
                    // what the prefilter normalizes against.
                    fe_anchor_symbol = Some(sel.anchor_row().0.symbol.clone());
                    write_quote_anchor_snapshot(
                        &mut redis,
                        chain_id,
                        &sel,
                        &weights,
                        current_block,
                    )
                    .await;
                }
            }
            if fe_prefilter {
                if let Some((builder, _)) = drain_state.as_ref() {
                    let mut qs = crate::fe_normalization::QuoteState::new(chain_id, builder.len());
                    // ARBX-0023 (05 r13/r16): fill the state in the DYNAMIC
                    // anchor's units — the selection this same tick published.
                    // No anchor / unpriced anchor ⇒ the stream's raw units,
                    // counted (`fe_anchor_applied` stays false): the ratios
                    // F_e consumes are re-denomination-invariant (r15, pinned
                    // by test in quote_anchor_runtime), so the numéraire
                    // never gates a trading direction.
                    let prices_eval = match &fe_anchor_symbol {
                        Some(sym) => {
                            match crate::quote_anchor_runtime::redenominate_to_anchor(&prices, sym)
                            {
                                Some(p) => {
                                    fe_anchor_applied = true;
                                    p
                                }
                                None => prices.clone(),
                            }
                        }
                        None => prices.clone(),
                    };
                    for (key, id, _allowed) in builder.snapshot() {
                        let addr = format!("{:#x}", key.address).to_ascii_lowercase();
                        let Some(sym) = fe_symbol_by_addr.get(&addr) else {
                            continue; // no identity row — price stays unset (R8)
                        };
                        let Some(price) = prices_eval.get(&sym.to_ascii_uppercase()) else {
                            continue; // stream has no quote for it — stays unset
                        };
                        if let Err(e) = qs.set_price(id, *price) {
                            debug!(
                                event = "route_discovery.fe_price_set_rejected",
                                chain_id,
                                id,
                                err = ?e
                            );
                        }
                    }
                    qs.advance_block(current_block);
                    fe_state = Some(qs);
                }
                fe_rate_by_pool = outcome
                    .graph
                    .edges
                    .iter()
                    .map(|e| (e.pool, e.log_weight.map(|w| (-w).exp())))
                    .collect();
                // ── EMIT-06b (FE-MASTER P5 follow-up): publish the per-pair
                // directed alpha (r15 — forward/reverse never collapse) as
                // ONE atomic hash per tick (`arbx:pairs:alpha:<chain>`),
                // consumed by GET /api/pairs. Same lane/knob as the
                // prefilter above: knob OFF ⇒ nothing written ⇒ the hash
                // lapses ⇒ the endpoint keeps serving its honest nulls. The
                // pair's directed rate is the BEST edge across its parallel
                // pools (max executable rate — the FE-0019 "mejores edges"
                // language); rows whose tokens fail dense mapping are
                // counted, never fabricated (R8).
                {
                    use crate::pair_alpha_runtime::{write_pair_alpha_snapshot, PairAlphaRow};
                    if let (Some(qs), Some((builder, _))) = (&fe_state, drain_state.as_ref()) {
                        let mut best_rate: std::collections::HashMap<(Address, Address), f64> =
                            std::collections::HashMap::new();
                        let mut pair_set: std::collections::HashSet<(Address, Address)> =
                            std::collections::HashSet::new();
                        for e in &outcome.graph.edges {
                            let Some(w) = e.log_weight else {
                                continue;
                            };
                            let rate = (-w).exp();
                            let entry = best_rate.entry((e.token_in, e.token_out)).or_insert(rate);
                            if rate > *entry {
                                *entry = rate;
                            }
                            // Address Ord is byte order — the same ascending
                            // order the reader's canonical field groups by.
                            let canonical = if e.token_in < e.token_out {
                                (e.token_in, e.token_out)
                            } else {
                                (e.token_out, e.token_in)
                            };
                            pair_set.insert(canonical);
                        }
                        let mut rows: Vec<(String, String, PairAlphaRow)> = Vec::new();
                        let mut alpha_map_fail = 0usize;
                        for (a, b) in &pair_set {
                            let (Some(ia), Some(ib)) = (
                                builder.id(&TokenKey {
                                    chain_id,
                                    address: *a,
                                }),
                                builder.id(&TokenKey {
                                    chain_id,
                                    address: *b,
                                }),
                            ) else {
                                alpha_map_fail += 1; // drifted out of the epoch — counted, R8
                                continue;
                            };
                            let rate_ab = best_rate.get(&(*a, *b)).copied();
                            let rate_ba = best_rate.get(&(*b, *a)).copied();
                            match qs.pair_alpha(ia, ib, rate_ab, rate_ba) {
                                Ok(pa) => rows.push((
                                    format!("{:#x}", a).to_ascii_lowercase(),
                                    format!("{:#x}", b).to_ascii_lowercase(),
                                    PairAlphaRow {
                                        forward: pa.forward.map(|edge| edge.f_e),
                                        reverse: pa.reverse.map(|edge| edge.f_e),
                                    },
                                )),
                                Err(_) => alpha_map_fail += 1,
                            }
                        }
                        if alpha_map_fail > 0 {
                            debug!(
                                event = "route_discovery.pair_alpha_map_fail",
                                chain_id,
                                count = alpha_map_fail
                            );
                        }
                        write_pair_alpha_snapshot(&mut redis, chain_id, &rows).await;
                    }
                }
            }
        }
        // XLS-QB-07b: Reprice = the reserves adapter pass. Backfill
        // multicalls ride inside it — the workbook excludes remote RPC from
        // this budget, so this row is an UPPER bound until the event-driven
        // split (headroom underestimates, never overestimates).
        let t_reprice = Instant::now();
        // EMIT-09: per-route reprice upper bound. Inert while no samples
        // exist (pure/test ticks) — zero timing cost on that path.
        let samples_live = !tick.lat_candidate_samples.is_empty();
        for c in tick
            .routes
            .iter()
            .filter(|c| c.route_kind == RouteKind::Triangular)
        {
            if let Some(set) = &scoped_hashes {
                if !set.contains(&c.route_hash) {
                    adapter_scoped_skip += 1;
                    continue; // scoped out: not dirty-affected this tick (counted, R8)
                }
            }
            // EMIT-09: the timed segment starts AFTER the scoped-skip — the
            // F_e prefilter math below rides INSIDE it (upper bound, the same
            // caveat the aggregate lat.reprice row declares). The exits that
            // `continue` before `ensure_reserves` (scoped above, F_e-dropped,
            // malformed legs) never record: `reprice_us` stays ABSENT for
            // them, which IS their state this tick (R8 presence-of-key).
            let t_route = samples_live.then(Instant::now);
            // ARBX-0024 (REQ-QB-008): F_e cycle prefilter — SIGNAL, never
            // proof. A computable ln-alpha ≤ 0 means this cycle cannot beat
            // the reference in any orientation-consistent sizing, so the
            // adapter's reserve work (and, once #350 lands, the dispatch)
            // is skipped with a counted reason. Anything uncomputable
            // (missing anchor price, unpriced edge, no dense mapping,
            // bounds misuse) ⇒ fail-OPEN: the route proceeds untouched —
            // the exact net gate stays the only PASS authority.
            if let (Some(qs), Some((builder, _))) = (&fe_state, drain_state.as_ref()) {
                let mut fe_dropped = false;
                match dense_cycle(chain_id, &c.tokens, builder) {
                    Some(ids) if ids.len() >= 2 => {
                        let rate_pairs = fe_rate_pairs(&ids, &c.pools, &fe_rate_by_pool);
                        // cycle_ln_alpha expects the walk CLOSED, so the
                        // start id is repeated at the end.
                        let mut closed = ids.clone();
                        closed.push(ids[0]);
                        let rate_of = |a: usize, b: usize| -> Option<f64> {
                            rate_pairs.get(&(a, b)).copied()
                        };
                        match qs.cycle_ln_alpha(&closed, &rate_of) {
                            Ok(Some(alpha)) if alpha <= 0.0 => {
                                fe_prefilter_below_reference += 1;
                                fe_dropped = true;
                            }
                            Ok(Some(_)) => fe_prefilter_pass += 1,
                            Ok(None) => fe_prefilter_uncomputed += 1,
                            Err(_) => fe_prefilter_map_fail += 1,
                        }
                    }
                    _ => fe_prefilter_uncomputed += 1, // no dense mapping — fail-open
                }
                if fe_dropped {
                    if let Ok(h) = H256::from_str(&c.route_hash) {
                        fe_skipped_hashes.push(h);
                    }
                    continue;
                }
            }
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
            // EMIT-09: every `ensure_reserves` arm (AllFresh / Backfilled /
            // Skipped) completed a real adapter traversal for this route —
            // including reserve-failure skips — so the measured upper bound
            // lands; only the pre-`continue` exits above stayed absent.
            if let Some(t) = t_route {
                if let Some(s) = tick
                    .lat_candidate_samples
                    .iter_mut()
                    .find(|s| s.route_hash == c.route_hash)
                {
                    s.reprice_us = Some(t.elapsed().as_micros() as u64);
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

        // ARBX-0024: same enforcement point for F_e-prefiltered routes — a
        // SEPARATE gate from skipped_hashes so adapter telemetry stays pure
        // (a below-reference cycle is a math signal, not a reserve failure).
        if !fe_skipped_hashes.is_empty() {
            let (kept, dropped) = gate_dispatch_intents(
                std::mem::take(&mut tick.dispatch_intents),
                &fe_skipped_hashes,
            );
            tick.dispatch_intents = kept;
            if dropped > 0 {
                tick.routes_dispatched = tick.routes_dispatched.saturating_sub(dropped);
                tick.tick_summary["routes_dispatched"] = serde_json::json!(tick.routes_dispatched);
            }
        }

        // ARBX-XLANG-01: adapter counters + drain/F_e/scoped/lat telemetry
        // live in `inject_scan_telemetry` (pure, golden-tested) — the wire
        // contract the frontend validates verbatim must be provable in CI.
        lat_record(&mut lat_log, Stage::Reprice, t_reprice);
        let dirty_seeds_now = drain_state
            .as_ref()
            .map(|(_, d)| d.dirty_seeds())
            .unwrap_or(0);
        inject_scan_telemetry(
            &mut tick,
            &lat_log,
            &ScanTelemetry {
                adapter_cache_hit,
                adapter_backfill_ok,
                adapter_backfill_fail,
                adapter_budget_exhausted,
                drain_stats,
                drain_register_reject,
                dirty_seeds_now,
                adapter_scoped_skip,
                fe_prefilter,
                fe_prefilter_pass,
                fe_prefilter_below_reference,
                fe_prefilter_uncomputed,
                fe_prefilter_map_fail,
                fe_anchor_applied,
                dirty_reeval,
                scoped_reeval_cycles,
                scoped_reeval_routes,
                scoped_cycle_map_fail,
                sla_ms,
                lat_candidates_cap,
            },
        );

        let t_emit = Instant::now();
        for ev in &tick.events {
            telemetry::publish(&mut redis, ev).await;
        }
        for ev in &adapter_events {
            telemetry::publish(&mut redis, ev).await;
        }
        telemetry::publish(&mut redis, &tick.tick_summary).await;
        // EMIT-05: same payload, durable form — the pub/sub channel above is
        // only visible to live subscribers; GET /api/route-discovery/tick
        // reads this snapshot (TTL ~60s ⇒ a dead loop expires into an
        // honest 404, never a stale-forever funnel).
        telemetry::set_tick_snapshot(&mut redis, chain_id, &tick.tick_summary).await;
        lat_record(&mut lat_log, Stage::Emit, t_emit);
        let lat_total_us = lat_log.end_cycle();

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
            lat_total_us = ?lat_total_us,
            lat_pass_p95 = ?lat_log.pass_p95(sla_ms),
            drain_drained = drain_stats.drained,
            dirty_seeds = dirty_seeds_now,
            scoped_reeval_cycles,
            adapter_scoped_skip,
            fe_prefilter_below_reference,
        );
    }
}

/// ARBX-0003: order-independent topology signature of the pool snapshot —
/// the drain epoch anchor (dense token ids are only meaningful within one
/// builder epoch, pair_index contract). Order-independent so a reshuffled
/// snapshot with the same pools does NOT rebuild the epoch.
fn topology_signature(pools: &[crate::impact_index::PoolRef]) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut ids: Vec<String> = pools
        .iter()
        .map(|p| format!("{:#x}|{:#x}|{:#x}", p.address, p.token0, p.token1))
        .collect();
    ids.sort_unstable();
    let mut h = std::collections::hash_map::DefaultHasher::new();
    ids.hash(&mut h);
    h.finish()
}

/// ARBX-0003: map one route's token cycle into the drain epoch's dense ids.
/// `None` when any token was never registered this epoch (topology drift),
/// the route is not a cycle (< 3 nodes), or the chain key mismatches — the
/// honest skip, never a partial mapping that would under-scope re-eval.
fn dense_cycle(chain_id: u64, tokens: &[Address], ids: &DenseIdBuilder) -> Option<Vec<usize>> {
    if tokens.len() < 3 {
        return None;
    }
    tokens
        .iter()
        .map(|a| {
            ids.id(&TokenKey {
                chain_id,
                address: *a,
            })
        })
        .collect()
}

/// ARBX-0024: per-leg fee-inclusive rates keyed by (src,dst) dense ids for
/// one candidate cycle. Legs run `tokens[i] → tokens[(i+1) % n]` over
/// `pools[i]`; a pool with no computable `log_weight` (or a ragged pools
/// vector) simply leaves its pair ABSENT — `cycle_ln_alpha` then returns
/// `Ok(None)` for the whole cycle and the caller fails OPEN (R8: unknown
/// is not a drop).
fn fe_rate_pairs(
    ids: &[usize],
    pools: &[Address],
    rate_by_pool: &std::collections::HashMap<Address, Option<f64>>,
) -> std::collections::HashMap<(usize, usize), f64> {
    let mut pairs = std::collections::HashMap::new();
    for (i, id) in ids.iter().enumerate() {
        let next = ids[(i + 1) % ids.len()];
        if let Some(rate) = pools
            .get(i)
            .and_then(|p| rate_by_pool.get(p).copied().flatten())
        {
            pairs.insert((*id, next), rate);
        }
    }
    pairs
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
            graph: TokenGraph {
                edges,
                adjacency,
                dense: None,
            },
            rejected: vec![RejectedEdge {
                pool: addr(0x99),
                reason: "missing_reserves".to_string(),
            }],
            pools_total: 3,
        }
    }

    /// XLS-QB-07b: the phased sink receives stage samples in canonical phase
    /// order (pair-split + enumeration → annotation → event build → dispatch
    /// planning → multi-hop; `Pair` since the ARBX-0010 `DiscoveryTimings`
    /// split). µs are machine-dependent — assert ORDER, never magnitude; the
    /// wrapper `evaluate_tick` stays clock-free (None → deterministic).
    #[test]
    fn evaluate_tick_phased_reports_stage_samples() {
        let outcome = outcome_two_v2();
        let engine = StrategyApplicabilityEngine::default();
        let finder = RouteFinderConfig::default();
        let mut seen: Vec<Stage> = Vec::new();
        let tick = evaluate_tick_phased(
            &outcome,
            1,
            &engine,
            &finder,
            200,
            false,
            42,
            "shadow",
            Some(&mut |stage: Stage, _micros: u64| seen.push(stage)),
        );
        assert_eq!(tick.routes_found, 2, "same behavior as the wrapper");
        assert_eq!(
            seen,
            vec![
                Stage::Pair,   // ARBX-0010 pair-split: l==2 hop segment of the DFS
                Stage::Expand, // find_routes (remainder: total − pair)
                Stage::Gates,  // applicability annotation
                Stage::Emit,   // event build
                Stage::Gates,  // dispatch planning (body skipped — ~0 sample)
                Stage::Expand, // multi-hop pass
            ]
        );
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
            serde_json::json!([2u8, 2u8])
        );
        assert_eq!(tick.tick_summary["multi_hop_mask_skip"], false);

        let tick = evaluate_tick(&outcome, 1, &engine, &base, 200, false, 0, "shadow");
        assert!(tick.tick_summary["multi_hop_mask_strategy"].is_null());
        assert_eq!(
            tick.tick_summary["multi_hop_hops_effective"],
            serde_json::json!([2u8, 3u8])
        );
        assert_eq!(tick.tick_summary["multi_hop_mask_skip"], false);

        let mut finder = base.clone();
        finder.hop_mask_strategy_id = Some("MEV-99-999".to_string()); // unknown → honest skip
        let tick = evaluate_tick(&outcome, 1, &engine, &finder, 200, false, 0, "shadow");
        assert_eq!(tick.tick_summary["multi_hop_mask_skip"], true);
        assert_eq!(tick.multi_hop_cycles_found, 0);
    }

    /// ARBX-0021 (col Status): the selected strategy's workbook Status gates
    /// expansion BEFORE the HopMask. Canonical exemplars from the workbook:
    /// MEV-04-001 is NEEDS_ROUTE_DATA (no cycles — no data, no fabricated
    /// route), MEV-03-029 is OBSERVE_ONLY (expansion RUNS for telemetry —
    /// observe-only is not a skip), and every tick carries the derived
    /// 79/174/8/3 census (computed from the table, never hardcoded IDs).
    #[test]
    fn evaluate_tick_applies_strategy_dispatch_status() {
        let outcome = outcome_two_v2();
        let engine = StrategyApplicabilityEngine::default();
        let base = RouteFinderConfig::default(); // min 2, max 3

        // NEEDS_ROUTE_DATA → no expansion, observable reason, 0 cycles.
        let mut finder = base.clone();
        finder.hop_mask_strategy_id = Some("MEV-04-001".to_string()); // NEEDS_ROUTE_DATA
        let tick = evaluate_tick(&outcome, 1, &engine, &finder, 200, false, 0, "shadow");
        assert_eq!(tick.tick_summary["multi_hop_status"], "needs_route_data");
        assert_eq!(
            tick.tick_summary["multi_hop_status_skip_reason"],
            "needs_route_data"
        );
        assert_eq!(tick.tick_summary["multi_hop_mask_skip"], true);
        assert_eq!(tick.multi_hop_cycles_found, 0);
        // TW-005: the skip carries the SPECIFIC execution precondition class.
        assert_eq!(
            tick.tick_summary["multi_hop_execution_class"],
            "DETERMINISTIC_IF_REDEEMABLE"
        );
        assert_eq!(
            tick.tick_summary["multi_hop_needs_class"],
            "DETERMINISTIC_IF_REDEEMABLE"
        );

        // OBSERVE_ONLY → expansion runs (telemetry sí), no skip reason.
        // The multihop finder only counts weighted edges (`None` ⇒ v3_skipped,
        // R8) and `outcome_two_v2` builds `log_weight: None` — attach real
        // weights here so the expansion has something to enumerate. Asymmetric
        // (one profitable direction), and the finder emits one cycle PER START
        // TOKEN (no cross-start dedupe): the profitable pair lands twice.
        let mut outcome = outcome_two_v2();
        for e in outcome.graph.edges.iter_mut() {
            e.log_weight = Some(if e.pool == addr(0x10) && e.token_in == addr(1) {
                -0.02 // 1→2 via pool 0x10 (winning leg)
            } else if e.pool == addr(0x20) && e.token_in == addr(2) {
                -0.01 // 2→1 via pool 0x20 (winning leg)
            } else {
                0.02 // the losing legs (both directions sum positive)
            });
        }
        let mut finder = base.clone();
        finder.hop_mask_strategy_id = Some("MEV-03-029".to_string()); // OBSERVE_ONLY
        let tick = evaluate_tick(&outcome, 1, &engine, &finder, 200, false, 0, "shadow");
        assert_eq!(tick.tick_summary["multi_hop_status"], "observe_only");
        assert!(tick.tick_summary["multi_hop_status_skip_reason"].is_null());
        assert_eq!(tick.tick_summary["multi_hop_mask_skip"], false);
        // The two-V2-pool fixture's single profitable pair (1→2 via 0x10,
        // 2→1 via 0x20), enumerated once per start token.
        assert_eq!(tick.multi_hop_cycles_found, 2);
        // TW-005: class is annotated; needs_class stays null (not a skip).
        assert_eq!(
            tick.tick_summary["multi_hop_execution_class"],
            "OBSERVE_ONLY"
        );
        assert!(tick.tick_summary["multi_hop_needs_class"].is_null());

        // Unknown MEV_ID → fail-closed with the honest reason.
        let mut finder = base.clone();
        finder.hop_mask_strategy_id = Some("MEV-99-999".to_string());
        let tick = evaluate_tick(&outcome, 1, &engine, &finder, 200, false, 0, "shadow");
        assert_eq!(tick.tick_summary["multi_hop_status"], "unknown_strategy");
        assert_eq!(
            tick.tick_summary["multi_hop_status_skip_reason"],
            "unknown_strategy"
        );

        // Derived census on every tick (79/174/8/3 — computed, not hardcoded).
        assert_eq!(
            tick.tick_summary["strategy_status_counts"]["route_ready"],
            79
        );
        assert_eq!(
            tick.tick_summary["strategy_status_counts"]["needs_route_data"],
            174
        );
        assert_eq!(
            tick.tick_summary["strategy_status_counts"]["observe_only"],
            8
        );
        assert_eq!(
            tick.tick_summary["strategy_status_counts"]["no_compatible_route"],
            3
        );
    }

    /// ARBX-0026 (sheet 13_DETECTOR_POLICY): the four policy dimensions of
    /// the selected strategy's detector are consumed GENERICALLY per tick —
    /// MEV-01-015 (R_CLOSED_CYCLE, ROUTE_READY, Allowed_Hops {2}) carries its
    /// detector's graph family, family envelope (identity: {2} ⊆ 2..=7 ⇒ no
    /// clamp), hot-seed admission and the universal Do_Not_Do guard; the
    /// OBSERVE detector is graph OBSERVE_ONLY + telemetry-only seed (never
    /// may_seed); an unknown MEV_ID resolves no policy at all.
    #[test]
    fn evaluate_tick_applies_detector_policy() {
        let outcome = outcome_two_v2();
        let engine = StrategyApplicabilityEngine::default();
        let base = RouteFinderConfig::default(); // min 2, max 3

        // ROUTE_READY under R_CLOSED_CYCLE → full policy annotation, envelope
        // is the identity (no clamp) and expansion stays bounded by the mask.
        let mut finder = base.clone();
        finder.hop_mask_strategy_id = Some("MEV-01-015".to_string());
        let tick = evaluate_tick(&outcome, 1, &engine, &finder, 200, false, 0, "shadow");
        assert_eq!(tick.tick_summary["multi_hop_detector"], "R_CLOSED_CYCLE");
        assert_eq!(
            tick.tick_summary["multi_hop_graph_policy"],
            // Workbook 13 col Graph_Policy for R_CLOSED_CYCLE: the dirty-edge
            // closed-cycle sentence (NOT the generic family-adapter row).
            "dirty pair/edge → closed-cycle/order route search"
        );
        assert_eq!(
            tick.tick_summary["multi_hop_family_hops"],
            serde_json::json!([2u8, 7u8])
        );
        assert_eq!(
            tick.tick_summary["multi_hop_hot_seed"],
            "Spread/log-alpha/depth dislocation"
        );
        assert_eq!(tick.tick_summary["multi_hop_may_seed"], true);
        assert!(tick.tick_summary["multi_hop_do_not"]
            .as_str()
            .expect("do_not present")
            .contains("generic spot-price spread"));
        // Identity: mask bounds {2} sit inside the family envelope ⇒ no clamp.
        assert_eq!(
            tick.tick_summary["multi_hop_hops_effective"],
            serde_json::json!([2u8, 2u8])
        );
        assert_eq!(tick.tick_summary["multi_hop_family_clamped"], false);
        assert!(tick.tick_summary["multi_hop_family_skip_reason"].is_null());

        // OBSERVE_ONLY detector → telemetry-only seed, never may_seed.
        let mut finder = base.clone();
        finder.hop_mask_strategy_id = Some("MEV-03-029".to_string());
        let tick = evaluate_tick(&outcome, 1, &engine, &finder, 200, false, 0, "shadow");
        assert_eq!(tick.tick_summary["multi_hop_detector"], "OBSERVE");
        assert!(tick.tick_summary["multi_hop_graph_policy"]
            .as_str()
            .expect("graph policy")
            .starts_with("OBSERVE_ONLY"));
        assert_eq!(tick.tick_summary["multi_hop_may_seed"], false);
        assert_eq!(
            tick.tick_summary["multi_hop_hot_seed"],
            "No hot opportunity seed; telemetry evidence only"
        );

        // Unknown MEV_ID → no policy resolved (fail-closed, honest nulls).
        let mut finder = base.clone();
        finder.hop_mask_strategy_id = Some("MEV-99-999".to_string());
        let tick = evaluate_tick(&outcome, 1, &engine, &finder, 200, false, 0, "shadow");
        assert!(tick.tick_summary["multi_hop_detector"].is_null());
        assert!(tick.tick_summary["multi_hop_graph_policy"].is_null());
        assert!(tick.tick_summary["multi_hop_hot_seed"].is_null());
        assert!(tick.tick_summary["multi_hop_do_not"].is_null());
        assert_eq!(tick.tick_summary["multi_hop_family_clamped"], false);
    }

    #[test]
    fn dense_cycle_maps_registered_tokens_and_skips_drift() {
        let a1 = Address::from_str("0x0000000000000000000000000000000000000001").unwrap();
        let a2 = Address::from_str("0x0000000000000000000000000000000000000002").unwrap();
        let a3 = Address::from_str("0x0000000000000000000000000000000000000003").unwrap();
        let mut ids = DenseIdBuilder::new();
        let i = ids.insert(
            TokenKey {
                chain_id: 1,
                address: a1,
            },
            true,
        );
        let j = ids.insert(
            TokenKey {
                chain_id: 1,
                address: a2,
            },
            true,
        );
        let k = ids.insert(
            TokenKey {
                chain_id: 1,
                address: a3,
            },
            true,
        );
        assert_eq!(dense_cycle(1, &[a1, a2, a3], &ids), Some(vec![i, j, k]));
        // Chain mismatch ⇒ the key never resolves (honest None).
        assert!(dense_cycle(2, &[a1, a2, a3], &ids).is_none());
        // Unregistered token ⇒ topology drift ⇒ None, never partial.
        let drift = Address::from_str("0x00000000000000000000000000000000000000ff").unwrap();
        assert!(dense_cycle(1, &[a1, a2, drift], &ids).is_none());
        // Not a cycle (2 nodes) ⇒ None.
        assert!(dense_cycle(1, &[a1, a2], &ids).is_none());
    }

    #[test]
    fn topology_signature_ignores_pool_order() {
        let mk = |addr: &str, t0: Address, t1: Address| crate::impact_index::PoolRef {
            chain_id: 1,
            address: Address::from_str(addr).unwrap(),
            dex_name: "probe".into(),
            protocol_type: crate::route_intent::ProtocolType::V2,
            token0: t0,
            token1: t1,
            fee_bps: None,
        };
        let a1 = Address::from_str("0x0000000000000000000000000000000000000001").unwrap();
        let a2 = Address::from_str("0x0000000000000000000000000000000000000002").unwrap();
        let p_a = mk("0x00000000000000000000000000000000000000aa", a1, a2);
        let p_b = mk("0x00000000000000000000000000000000000000bb", a2, a1);
        assert_eq!(
            topology_signature(&[p_a.clone(), p_b.clone()]),
            topology_signature(&[p_b, p_a]),
            "same universe, reshuffled ⇒ same epoch"
        );
    }

    /// ARBX-0024: the per-leg rate map covers the FULL closed walk —
    /// including the wrap leg `tokens[n-1] → tokens[0]` — and a pool with
    /// no computable rate leaves its pair absent (the cycle then reads as
    /// uncomputable and the prefilter fails OPEN, never drops).
    #[test]
    fn fe_rate_pairs_covers_wrap_leg_and_skips_unpriced_pools() {
        let pools = [addr(0x10), addr(0x20), addr(0x30)];
        let rate_by_pool: HashMap<Address, Option<f64>> = HashMap::from([
            (addr(0x10), Some(10.0)),
            (addr(0x20), None), // unpriced edge (no log_weight)
            (addr(0x30), Some(0.5)),
        ]);
        let ids = [7usize, 8, 9];
        let pairs = fe_rate_pairs(&ids, &pools, &rate_by_pool);
        assert_eq!(pairs.get(&(7, 8)), Some(&10.0), "leg 0 over pool 0x10");
        assert!(!pairs.contains_key(&(8, 9)), "unpriced pool ⇒ absent pair");
        assert_eq!(pairs.get(&(9, 7)), Some(&0.5), "wrap leg over pool 0x30");
    }

    /// ARBX-0024 (AC "F_e conocidos" + regression "signal-no-reemplaza-
    /// netgate"): the exact decision table the adapter loop applies. A
    /// losing triangle (one leg below its reference) computes a negative
    /// ln-alpha → the SIGNAL drops it from the adapter/dispatch pass; a
    /// winning one passes; any hole (missing anchor price) is NOT a drop.
    /// The alpha is only ever a prefilter bit — PASS authority stays with
    /// the amount-aware exact net gate (fees+gas+financing+tip+risk+sim),
    /// which these asserts deliberately never touch.
    #[test]
    fn fe_prefilter_known_triangles_signal_vs_proof() {
        use crate::fe_normalization::QuoteState;
        // USDC-anchored references: P_Q = [1, 3000, 60000] over ids 0..2.
        let mut qs = QuoteState::new(1, 3);
        qs.set_price(0, 1.0).unwrap();
        qs.set_price(1, 3000.0).unwrap();
        qs.set_price(2, 60000.0).unwrap();

        let decide = |qs: &QuoteState, rates: &[(usize, usize, f64)]| -> &'static str {
            let pairs: HashMap<(usize, usize), f64> =
                rates.iter().map(|&(a, b, r)| ((a, b), r)).collect();
            let closed = [0usize, 1, 2, 0];
            let rate_of = |a: usize, b: usize| -> Option<f64> { pairs.get(&(a, b)).copied() };
            match qs.cycle_ln_alpha(&closed, &rate_of).unwrap() {
                Some(alpha) if alpha <= 0.0 => "below_reference",
                Some(_) => "pass",
                None => "uncomputed",
            }
        };

        // Losing triangle: the WBTC leg returns only 59900 USDC per WBTC
        // (reference 60000) — product 59900/60000 < 1 ⇒ negative alpha.
        assert_eq!(
            decide(&qs, &[(0, 1, 1.0 / 3000.0), (1, 2, 0.05), (2, 0, 59900.0)]),
            "below_reference",
            "one below-reference leg ⇒ the whole cycle is signaled out"
        );
        // Winning triangle: the same shape with the WBTC leg at 60300.
        assert_eq!(
            decide(&qs, &[(0, 1, 1.0 / 3000.0), (1, 2, 0.05), (2, 0, 60300.0)]),
            "pass",
            "alpha > 0 survives the prefilter — the net gate decides the rest"
        );
        // Missing anchor price ⇒ uncomputed ⇒ fail-OPEN (a hole is not a
        // verdict; the route proceeds and the exact net gate stays judge).
        let mut holed = QuoteState::new(1, 3);
        holed.set_price(0, 1.0).unwrap();
        holed.set_price(1, 3000.0).unwrap(); // id 2 (WBTC) never priced
        assert_eq!(
            decide(
                &holed,
                &[(0, 1, 1.0 / 3000.0), (1, 2, 0.05), (2, 0, 60300.0)]
            ),
            "uncomputed",
            "R8: missing reference is not computable, never a fabricated drop"
        );
    }
    // ── ARBX-XLANG-01: cross-language wire-contract golden ────────────────
    // The tick_summary JSON is validated VERBATIM by the frontend Zod mirror
    // (`RouteDiscoveryTickSummarySchema`, `.strict()` — an unknown key or a
    // type mismatch rejects the WHOLE payload; that drift class once left
    // the telemetry room accepting nothing). These fixtures are the SINGLE
    // source of truth both languages assert against:
    //   - this test regenerates them from the REAL emission path
    //     (evaluate_tick + inject_scan_telemetry, controlled inputs) and
    //     asserts byte equality on every rerun;
    //   - the vitest contract test parses the SAME committed files
    //     (frontend/lib/apex/schemas/__tests__/contract-route-discovery-tick.test.ts).
    // Any intentional wire change ⇒ regen ⇒ the Zod mirror must follow or
    // CI fails on the frontend side. Regenerate with:
    //   ARBX_REGEN_GOLDEN=1 cargo test -p searcher-rs xlang_golden
    fn golden_fixture_dir() -> std::path::PathBuf {
        // searcher-rs → backend → repo root → frontend tree.
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../frontend/lib/apex/schemas/__tests__/fixtures/route-discovery-tick")
    }

    fn golden_lat_log() -> LatencyLog {
        let mut log = LatencyLog::with_window(64);
        log.begin_cycle();
        for (stage, us) in [
            (Stage::Decode, 1_100),
            (Stage::State, 1_200),
            (Stage::Reprice, 1_300),
            (Stage::Pair, 1_400),
            (Stage::Expand, 1_500),
            (Stage::Refine, 1_600),
            (Stage::Gates, 1_700),
            (Stage::Emit, 1_800),
        ] {
            log.record(stage, us).expect("seed sample in-cycle");
        }
        log.end_cycle();
        log
    }

    fn golden_tick_payload(policy_selected: bool, knobs_on: bool) -> Value {
        let outcome = outcome_two_v2(); // carries 1 rejected pool → census key
        let engine = StrategyApplicabilityEngine::default();
        let mut finder = RouteFinderConfig::default();
        if policy_selected {
            // MEV-01-015: real workbook strategy (mask 1 → hop 2) so the
            // do_not/family/policy keys carry their Some(..) wire shape.
            finder.hop_mask_strategy_id = Some("MEV-01-015".to_string());
        }
        let mut tick = evaluate_tick(&outcome, 1, &engine, &finder, 200, false, 42, "shadow");
        if knobs_on {
            // One traversed candidate so lat_candidates carries a REAL row
            // (route_kind from the closed Zod vocabulary, reprice present —
            // total_us = Σ stages, the producer-coherence contract).
            tick.lat_candidate_samples.push(
                crate::route_discovery::lat_candidates::CandidateSample {
                    route_hash: "0xdeadbeef".to_string(),
                    route_kind: "v2v2".to_string(),
                    hops: 2,
                    gates_us: 900,
                    reprice_us: Some(700),
                },
            );
        }
        let seeded_log = golden_lat_log();
        let empty_log = LatencyLog::with_window(64); // no samples → honest nulls
        let lat_log: &LatencyLog = if knobs_on { &seeded_log } else { &empty_log };
        inject_scan_telemetry(
            &mut tick,
            lat_log,
            &ScanTelemetry {
                adapter_cache_hit: 3,
                adapter_backfill_ok: 2,
                adapter_backfill_fail: 1,
                adapter_budget_exhausted: 0,
                drain_stats: crate::dirty_consumer::DrainStats {
                    drained: 11,
                    unknown_pool: 1,
                    invalid_pair: 2,
                    already_dirty: 3,
                    seeded: 4,
                    evicted: 0,
                },
                drain_register_reject: 1,
                dirty_seeds_now: 4,
                adapter_scoped_skip: 5,
                fe_prefilter: knobs_on,
                fe_prefilter_pass: 7,
                fe_prefilter_below_reference: 2,
                fe_prefilter_uncomputed: 1,
                fe_prefilter_map_fail: 0,
                fe_anchor_applied: true,
                dirty_reeval: knobs_on,
                scoped_reeval_cycles: 3,
                scoped_reeval_routes: 6,
                scoped_cycle_map_fail: 1,
                sla_ms: 29.0,
                lat_candidates_cap: 10,
            },
        );
        tick.tick_summary
    }

    fn assert_golden(name: &str, payload: &Value) {
        let path = golden_fixture_dir().join(name);
        let rendered = format!("{}\n", serde_json::to_string_pretty(payload).unwrap());
        if std::env::var("ARBX_REGEN_GOLDEN").is_ok() {
            std::fs::create_dir_all(golden_fixture_dir()).unwrap();
            std::fs::write(&path, &rendered).unwrap();
            return;
        }
        let committed = std::fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("golden {name} missing ({e}) — run ARBX_REGEN_GOLDEN=1 cargo test -p searcher-rs xlang_golden"));
        assert_eq!(
            committed, rendered,
            "tick wire contract drifted from committed golden {name} — if intentional, \
             regenerate AND update the frontend Zod mirror + its contract test"
        );
    }

    #[test]
    fn xlang_golden_tick_contract() {
        // Full-dress tick: policy selected, both knobs ON, latency samples
        // and one per-candidate row — every conditional group present.
        assert_golden("full.json", &golden_tick_payload(true, true));
        // Dormant tick: default finder (policy-less → do_not null), knobs
        // OFF (fe/scoped groups ABSENT — absence is a real backend state),
        // empty latency log (honest nulls, lat_pass_p95 None).
        assert_golden("knobs-off.json", &golden_tick_payload(false, false));
    }
}
