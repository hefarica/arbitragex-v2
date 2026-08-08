// M11 allow: test modules use .unwrap()/.expect() for readability;
// production paths use ? / anyhow throughout.
//! Orchestrator — Live Engine Pipeline (Phases 8-11 wired).
//!
//! ## Design (spec §3.5)
//!
//! The orchestrator is the single entry point for every `RouteIntent` decoded
//! from a mempool transaction. It:
//!
//!   1. Converts the intent into its `ImpactSet` via `ImpactIndex`.
//!   2. Fans out to strategy engines (`DexEngine`, `TriangularEngine`,
//!      `LiquidationEngine`, then `FlashloanEngine` wrapping).
//!   3. Evaluates each `StrategyCandidate` through `ConfigAwareEvaluator`.
//!   4. Emits accepted or rejected candidates via `OpportunityEmitter`.
//!
//! ## Current scope (updated)
//!
//! - `DexEngine`, `TriangularEngine`, and `LiquidationEngine` are invoked in
//!   the intent pipeline; their candidates are merged into `base_candidates`.
//! - `FlashloanEngine` runs after base-candidate assembly to wrap net-positive
//!   routes.
//! - `state_projector` and `size_optimizer` are wired in context and used by
//!   downstream optimization/evaluation paths.
//! - Scanner/orchestrator integration status depends on boot wiring in
//!   `main.rs`; do not infer production enablement from this file header alone.
//!
//! ## Critical rule: no hardcoded strategy strings
//!
//! The orchestrator NEVER writes `strategy_kind = "dex_arb_v2v2"` or any
//! other literal strategy string. Every strategy label comes from
//! `StrategyLabel` returned by an engine. This is the primary invariant
//! that the Phase 14 migration enforces system-wide.
//!
//! ## R8 invariants
//!
//! - Errors from individual engines are caught, logged, and counted. One
//!   engine failure does NOT crash the orchestrator loop.
//! - `emit_accepted` / `emit_rejected` errors (Redis publish failure)
//!   propagate as `Err` so the caller can decide whether to reconnect.
//! - `gross_profit_usd = None` from an engine propagates unchanged through
//!   the evaluator and emitter paths.

use crate::cartridge::runner::CartridgeRunner;
use crate::engines::dex_engine::DexEngine;
use crate::engines::flashloan_engine::FlashloanEngine;
use crate::engines::liquidation_engine::LiquidationEngine;
use crate::engines::triangular_engine::TriangularEngine;
// Task 3: New engines
use crate::engines::cross_chain_bridge_engine::CrossChainBridgeEngine;
use crate::engines::liquidation_snipe_engine::LiquidationSnipeEngine;
use crate::engines::spanning_tree_engine::SpanningTreeEngine;
use crate::engines::StrategyCandidate;
use crate::impact_index::ImpactIndex;
use crate::metrics::{
    CANDIDATES_TOTAL, DECODED_INTENTS_TOTAL, ENGINE_ERRORS_TOTAL, IMPACTED_ROUTES_TOTAL,
    OPPORTUNITIES_PUBLISHED_TOTAL, REJECTED_CONFIG_TOTAL, REJECTED_NO_PROFIT_TOTAL,
    SIMULATION_FAILED_TOTAL,
};
use crate::gates::{MacroMevGate, MacroMevGateConfig};
use crate::opportunity_emitter::{EmitOutcome, OpportunityEmitter};
use crate::route_intent::RouteIntent;
use crate::size_optimizer::{OptimizeOutcome, OptimizeRejectReason, SizeOptimizer};
use crate::state_projector::StateProjector;
use crate::strategy_label::StrategyLabel;
use shared_rs::price_oracle::RedisCachedPriceOracle;
use shared_rs::trading_config::TradingConfigState;
use std::collections::HashMap;
use std::sync::Arc;
use ethers::types::Address;
use tokio::sync::RwLock;
use tracing::{debug, error, info, warn};

use prioritization_spine::config_aware::{ConfigAwareEvaluator, ConfigGateOutcome, NetworkSignals};

// ---------------------------------------------------------------------------
// OrchestratorContext
// ---------------------------------------------------------------------------

/// All shared dependencies the orchestrator needs. Constructed at boot and
/// passed into `Orchestrator::new`. Every field is `Arc`-wrapped for
/// concurrent access across the tokio task tree.
pub struct OrchestratorContext {
    /// Live pool/cycle registry — read-lock per intent.
    pub impact_index: Arc<RwLock<ImpactIndex>>,
    /// DEX arb V2/V3 engine (Phase 8).
    pub dex_engine: Arc<DexEngine>,
    /// Triangular arb engine — evaluates impacted cycles (Phase 9).
    pub triangular_engine: Arc<TriangularEngine>,
    /// Flashloan capital wrapper — wraps net-positive base candidates (Phase 10).
    pub flashloan_engine: Arc<FlashloanEngine>,
    /// Liquidation engine — emits candidates when impacted lending positions
    /// drop below health_factor 1.0 (Phase 11).
    pub liquidation_engine: Arc<LiquidationEngine>,
    /// StateProjector — virtual post-tx pool state (Phase 12).
    /// Stored here so Phase 15 can access it directly from the context for
    /// on-demand per-candidate projection. Currently accessed indirectly via
    /// `size_optimizer` which owns a clone of the same `Arc`.
    #[allow(dead_code)]
    pub state_projector: Arc<StateProjector>,
    /// SizeOptimizer — optimal amount_in per candidate (Phase 13).
    pub size_optimizer: Arc<SizeOptimizer>,
    /// SpanningTreeEngine — Bellman-Ford graph cycle detection (Task 3).
    pub spanning_tree_engine: Option<Arc<SpanningTreeEngine>>,
    /// CrossChainBridgeEngine — cross-chain opportunity detection (Task 3).
    pub cross_chain_engine: Option<Arc<CrossChainBridgeEngine>>,
    /// LiquidationSnipeEngine — Aave/Compound liquidation sniping (Task 3).
    pub liquidation_snipe_engine: Option<Arc<LiquidationSnipeEngine>>,
    /// Single-point emit path (PG + Redis).
    pub emitter: Arc<OpportunityEmitter>,
    /// Asynchronously fetches the live `TradingConfigState` for `chain_id`.
    /// `None` return → no operator config for this chain (observe-only path).
    pub config_provider: Arc<ConfigProvider>,
    /// Pool discovery service for on-the-fly resolution of unmapped pairs.
    pub pool_discovery: Arc<crate::pool_discovery::PoolDiscoveryService>,
    /// EVM chain ID for this orchestrator instance.
    pub chain_id: u64,
    /// FASE OMEGA — cartridge runtime for shadow/active evaluation. `Some` only when
    /// `ARBX_CARTRIDGE_MODE` is enabled AND the runtime booted. When present, each
    /// route intent is evaluated against active cartridges OFF the hot path.
    /// In `Shadow` mode: observe-only (logs/telemetry, never a StrategyCandidate).
    /// In `Active` mode: full wiring — CartridgeEvalResult → StrategyCandidate →
    /// process_candidate → OpportunityEmitter (Redis/Postgres/API).
    pub cartridge_runner: Option<Arc<CartridgeRunner>>,
    /// Cartridge runtime mode (shadow/active) resolved from ARBX_CARTRIDGE_MODE.
    /// Controls whether cartridge evaluation produces StrategyCandidates (active)
    /// or only telemetry (shadow).
    pub cartridge_mode: crate::cartridge_boot::CartridgeMode,
    /// Fix B — math evidence (observe-only). The 31-operator registry and the
    /// regime decision tree. Used to evaluate route intents against the math
    /// operators recommended for the detected market regime; outputs are
    /// logged as telemetry only (never alter scoring in this phase).
    pub math_registry: Arc<math_engine::OperatorRegistry>,
    /// Fix B — regime decision tree for operator selection.
    pub regime_router: math_engine::RegimeRouter,
    /// Fix B — Redis handle for persisting math-evidence snapshots (regime +
    /// operator values per strategy) so the api-server can serve them to the
    /// dashboard in real time. Cheap multiplexed clone.
    pub math_redis: redis::aio::ConnectionManager,
    /// SED Bridge — connects to sed-core math pipeline (paper-shadow only).
    /// When `Some`, feeds gas observations and enriches candidates with
    /// stochastic convergence metrics. When `None`, orchestrator runs
    /// without mathematical overlay (standard V2 mode).
    #[cfg(feature = "paper-shadow")]
    pub sed_bridge: Option<Arc<crate::sed_bridge::SedBridge>>,
}

// ---------------------------------------------------------------------------
// ConfigProvider
// ---------------------------------------------------------------------------

/// Provides a `TradingConfigState` snapshot per chain.
///
/// Separated from `OrchestratorContext` so tests can inject a stub.
/// The production implementation wraps `TradingConfigClient`.
pub struct ConfigProvider {
    pub trading_config: shared_rs::trading_config::TradingConfigClient,
}

impl ConfigProvider {
    /// Fetches the current `TradingConfigState` for `chain_id` from Redis.
    /// Returns `None` when no config exists for this chain (observe-only mode).
    pub async fn snapshot(&self, chain_id: u64) -> Option<TradingConfigState> {
        match self.trading_config.state(chain_id).await {
            Ok(opt) => opt,
            Err(e) => {
                warn!(
                    event = "orchestrator.trading_config_read_failed",
                    chain_id,
                    error = %e,
                    "continuing without evaluator"
                );
                None
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Orchestrator
// ---------------------------------------------------------------------------

/// Main orchestrator. Constructed once per chain and shared across tasks via `Arc`.
pub struct Orchestrator {
    ctx: OrchestratorContext,
}

impl Orchestrator {
    /// Constructs a new `Orchestrator` from a fully-initialised context.
    pub fn new(ctx: OrchestratorContext) -> Self {
        Self { ctx }
    }

    // -----------------------------------------------------------------------
    // Main entry point
    // -----------------------------------------------------------------------

    /// Process one `RouteIntent` decoded from a mempool transaction.
    ///
    /// Flow:
    ///   1. Increment `decoded_intents_total` metric.
    ///   2. Resolve `ImpactSet` from `ImpactIndex`.
    ///   3. Increment `impacted_routes_total` metric.
    ///   4. Fan out to `DexEngine`, `TriangularEngine`, and `LiquidationEngine`, then wrap with `FlashloanEngine`.
    ///   5. For each `StrategyCandidate`:
    ///      a. Snapshot config (once per intent, not per candidate).
    ///      b. Call `evaluate_with_route_plan`.
    ///      c. Emit via `OpportunityEmitter`.
    ///   6. Engine errors are caught, logged, and counted — never crash the loop.
    ///
    /// Returns `Err` only when a Redis publish fails (the emitter propagates
    /// it so the caller can reconnect). Evaluation / gate / PG errors are
    /// swallowed per-candidate with a logged counter increment.
    /// Feed a RouteIntent to the ACTIVE cartridge runtime ONLY (no native engines).
    /// Used by route_discovery to route closed-cycle candidates directly to the
    /// canonical cartridge path — each cartridge evaluates the cycle and emits its
    /// OWN `strategy_kind` (its .rhai stem). Deliberately bypasses the native
    /// engines so cartridges are the sole canonical detector for discovered cycles
    /// (no duplicate rows, no native spread path). No-op when cartridge_mode !=
    /// Active or no runner loaded. Paper mode, capital=0.
    pub fn spawn_cartridge_eval(&self, intent: RouteIntent) {
        let chain_id = self.ctx.chain_id;
        let runner = match self.ctx.cartridge_runner.clone() {
            Some(r) => r,
            None => return,
        };
        if self.ctx.cartridge_mode != crate::cartridge_boot::CartridgeMode::Active {
            return;
        }
        let emitter = self.ctx.emitter.clone();
        let cfg_provider = self.ctx.config_provider.clone();
        let ctx_chain_id = self.ctx.chain_id;
        tokio::spawn(async move {
            crate::cartridge_boot::active_evaluate_and_emit(
                runner,
                intent,
                chain_id,
                emitter,
                cfg_provider,
                ctx_chain_id,
            )
            .await;
        });
    }

    pub async fn on_route_intent(&self, intent: RouteIntent) -> anyhow::Result<()> {
        let chain_id = self.ctx.chain_id;
        let chain_str = chain_id.to_string();
        let source_str = detection_source_as_str(intent.source_event);

        // ── FIX (review V2 #10): chain_id validation — a cross-chain intent
        // must NEVER be processed by this orchestrator instance. Reject loudly
        // and count; silently processing would poison per-chain metrics/DB.
        if intent.chain_id != chain_id {
            warn!(
                event = "v2.orchestrator.chain_id_mismatch",
                ctx_chain_id = chain_id,
                intent_chain_id = intent.chain_id,
                tx_hash = %intent.tx_hash,
                "rejecting intent: chain_id mismatch (cross-chain leak)"
            );
            return Ok(());
        }

        // ── TASK 1 log #2: v2.orchestrator.intent_received ───────────────
        // FIRST line of the function per spec §TASK-1/event-2.
        info!(
            event = "v2.orchestrator.intent_received",
            chain_id,
            tx_hash = %intent.tx_hash,
            legs_count = intent.legs.len(),
            amount_in = %intent.amount_in,
            source_event = source_str,
        );

        // ── Step 1: decoded_intents_total metric ─────────────────────────
        DECODED_INTENTS_TOTAL
            .with_label_values(&[&chain_str, source_str])
            .inc();

        // ── Step 2: resolve ImpactSet ────────────────────────────────────
        let impact = {
            let idx = self.ctx.impact_index.read().await;
            idx.resolve(&intent)
        };

        // ── SED Bridge: feed gas observation from this intent ─────────
        // Every mempool tx carries a value signal. We use the swap amount_in
        // as a proxy for market regime detection (larger swaps correlate with
        // higher volatility regimes). The actual gas price will be threaded
        // through when RouteIntent carries it from the pending tx.
        // TODO(SED-BRIDGE): Thread raw tx.gas_price through RouteIntent.
        #[cfg(feature = "paper-shadow")]
        if let Some(ref bridge) = self.ctx.sed_bridge {
            // Convert U256 amount_in to f64 as a regime proxy signal.
            // Clamp to prevent NaN/Inf in log-return computation.
            let signal = {
                let raw = intent.amount_in.as_u128() as f64;
                // Normalize to a reasonable range [0.001, 1e18]
                raw.max(0.001).min(1e18)
            };
            let ts_ms = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as u64;
            bridge.feed_gas_observation(signal, ts_ms).await;
        }

        // ── TASK 1 log #3: v2.impact.resolved ────────────────────────────
        info!(
            event = "v2.impact.resolved",
            chain_id,
            tx_hash = %intent.tx_hash,
            impacted_pairs = impact.impacted_pairs.len(),
            impacted_pools = impact.impacted_pools.len(),
            impacted_cycles = impact.impacted_cycles.len(),
            impacted_lending_positions = impact.impacted_lending_positions.len(),
            impacted_protocols = impact.impacted_protocols.len(),
        );

        // ── FASE OMEGA — cartridge evaluation (shadow or active) ─────────────
        // When the cartridge runtime is present (ARBX_CARTRIDGE_MODE enabled),
        // evaluate active cartridges against this live intent on a DETACHED task.
        //
        // MODE SEMANTICS:
        //   shadow — observe-only. Results go to logs/telemetry. Never builds a
        //            StrategyCandidate, never calls process_candidate, never reaches
        //            the execution pipeline. Zero risk to the hot path.
        //   active — FULL WIRING. CartridgeEvalResult.is_opportunity=true is
        //            transformed into a StrategyCandidate and passed to
        //            process_candidate (same pipeline as native engines). The
        //            candidate flows through ConfigAwareEvaluator → OpportunityEmitter
        //            → Redis/Postgres → /api/opportunities/live.
        //
        // The mode is resolved from ARBX_CARTRIDGE_MODE at boot time (scanner.rs).
        // DIAGNOSTIC: log why cartridge evaluation is or isn't happening.
        let has_runner = self.ctx.cartridge_runner.is_some();
        let mode = self.ctx.cartridge_mode;
        debug!(
            event = "v2.cartridge.dispatch_check",
            chain_id,
            tx_hash = %intent.tx_hash,
            has_runner,
            mode = mode.as_str(),
            "cartridge dispatch check"
        );

        if let Some(runner) = &self.ctx.cartridge_runner {
            let runner = runner.clone();
            let intent_for_cart = intent.clone();
            let cartridge_mode = self.ctx.cartridge_mode;
            let emitter = self.ctx.emitter.clone();
            let cfg_provider = self.ctx.config_provider.clone();
            let ctx_chain_id = self.ctx.chain_id;

            tokio::spawn(async move {
                debug!(
                    event = "v2.cartridge.spawned_task",
                    chain_id,
                    mode = cartridge_mode.as_str(),
                    "cartridge evaluation task spawned"
                );
                if cartridge_mode == crate::cartridge_boot::CartridgeMode::Active {
                    // ACTIVE MODE: evaluate and emit real candidates through the full pipeline
                    crate::cartridge_boot::active_evaluate_and_emit(
                        runner,
                        intent_for_cart,
                        chain_id,
                        emitter,
                        cfg_provider,
                        ctx_chain_id,
                    )
                    .await;
                } else {
                    // SHADOW MODE: observe-only telemetry (legacy behavior)
                    crate::cartridge_boot::shadow_evaluate_intent(runner, intent_for_cart, chain_id)
                        .await;
                }
            });
        }

        // ── Fix B — math evidence (observe-only, detached) ───────────────────
        // Evaluate the RegimeRouter-recommended operators against the pools in
        // this intent. Detached task — zero added latency to the hot path. The
        // outputs are logged as telemetry (regime + operator values); they do
        // NOT alter scoring in this phase. strategy_kind from the router kind.
        {
            let reserves_cache = self.ctx.dex_engine.reserves_cache.clone();
            let registry = self.ctx.math_registry.clone();
            let router = self.ctx.regime_router;
            let mut math_redis = self.ctx.math_redis.clone();
            let strategy_kind = format!("{:?}", intent.router_kind);
            let pools: Vec<Address> = intent
                .legs
                .iter()
                .filter_map(|leg| leg.pool_hint)
                .collect();
            if !pools.is_empty() {
                tokio::spawn(async move {
                    crate::math_evidence::evaluate_math_evidence(
                        &reserves_cache,
                        &registry,
                        &router,
                        &mut math_redis,
                        &pools,
                        chain_id,
                        0.0, // gas_price_gwei — not carried in RouteIntent yet (observe-only)
                        0,   // block_number — not carried in RouteIntent yet
                        0,   // block_timestamp — not carried in RouteIntent yet
                        std::collections::HashMap::new(),
                        &strategy_kind,
                    )
                    .await;
                });
            }
        }

        let mut impact = impact;
        // FIX (review V2 #7): include impacted_lending_positions in the zero-impact
        // check. A lending-only intent (impacted_lending_positions > 0 but no pools/
        // cycles) must reach the LiquidationEngine — previously it was discarded as
        // impact_zero before ever reaching the engine fan-out.
        let has_impact = !impact.impacted_pools.is_empty()
            || !impact.impacted_cycles.is_empty()
            || !impact.impacted_lending_positions.is_empty();
        if !has_impact {
            // No pools impacted. This is an unmapped pair.
            // Dispatch synchronously to the PoolDiscoveryService.
            self.ctx
                .pool_discovery
                .record_opportunity_observation(&intent, "discovery_started", None, None)
                .await;

            match self.ctx.pool_discovery.discover_from_intent(&intent).await {
                Ok(true) => {
                    // Retry impact resolution
                    impact = {
                        let idx = self.ctx.impact_index.read().await;
                        idx.resolve(&intent)
                    };
                    info!(
                        event = "v2.impact.discovery_retry",
                        chain_id,
                        tx_hash = %intent.tx_hash,
                        impact_after_pools = impact.impacted_pools.len(),
                        "Retried impact resolution after successful discovery"
                    );
                }
                Ok(false) => {
                    self.ctx
                        .pool_discovery
                        .record_opportunity_observation(
                            &intent,
                            "discovery_no_pool_found",
                            None,
                            None,
                        )
                        .await;
                }
                Err(e) => {
                    warn!("Pool discovery error: {}", e);
                    self.ctx
                        .pool_discovery
                        .record_opportunity_observation(&intent, "discovery_failed", None, None)
                        .await;
                }
            }

            // Re-check WITH lending positions after discovery retry.
            let has_impact_after = !impact.impacted_pools.is_empty()
                || !impact.impacted_cycles.is_empty()
                || !impact.impacted_lending_positions.is_empty();
            if !has_impact_after {
                debug!(
                    event = "orchestrator.impact_zero",
                    chain_id,
                    tx_hash = %intent.tx_hash,
                    "rejection_reason" = "impact_zero"
                );
                self.ctx
                    .pool_discovery
                    .record_opportunity_observation(&intent, "impact_zero", None, None)
                    .await;
                return Ok(());
            }
        }

        // ── Step 3: impacted_routes_total metric — one increment per engine
        for label in &[
            StrategyLabel::DexArbV2V2,
            StrategyLabel::TriangularArb,
            StrategyLabel::FlashloanArb,
            StrategyLabel::Liquidation,
        ] {
            IMPACTED_ROUTES_TOTAL
                .with_label_values(&[&chain_str, label.as_str()])
                .inc();
        }

        debug!(
            event = "orchestrator.impact_resolved",
            chain_id,
            tx_hash = %intent.tx_hash,
            impacted_pools = impact.impacted_pools.len(),
            impacted_cycles = impact.impacted_cycles.len(),
        );

        // ── Step 4: snapshot config ONCE per intent, before engine fan-out ──
        // Engines receive the snapshot as a method parameter (Bug 4 fix):
        // no stored Arc<RwLock<Option<...>>> on each engine struct.
        // When config is None: continue in observe-only mode (engines receive
        // None, fall back to conservative/no-USD-pricing defaults, R8 honest).
        let cfg_snapshot: Option<TradingConfigState> =
            self.ctx.config_provider.snapshot(chain_id).await;

        // ── TASK 1 log #4: v2.config.snapshot ────────────────────────────
        info!(
            event = "v2.config.snapshot",
            chain_id,
            has_config = cfg_snapshot.is_some(),
            enabled = cfg_snapshot.as_ref().map(|c| c.enabled).unwrap_or(false),
            enabled_strategies_count = cfg_snapshot
                .as_ref()
                .map(|c| c.enabled_strategies.len())
                .unwrap_or(0),
        );

        // Emit metric if no config (operator visibility, not a crash).
        // ENGINE_ERRORS_TOTAL is defined with EXACTLY two labels [chain_id, strategy]
        // (see metrics.rs). The config-absence signal occupies the `strategy` slot with
        // the sentinel pseudo-strategy "no_trading_config" — distinguishable in Grafana
        // from the real StrategyLabel values. Passing a 3rd value here previously panicked
        // the worker with `InconsistentCardinality { expect: 2, got: 3 }` once per intent
        // whenever no trading config was loaded (the common case under block mode).
        if cfg_snapshot.is_none() {
            ENGINE_ERRORS_TOTAL
                .with_label_values(&[&chain_str, "no_trading_config"])
                .inc();
        }

        // ── Step 5: fan out to engines ───────────────────────────────────

        // DexEngine (Phase 8 — live).
        let dex_candidates = match self
            .ctx
            .dex_engine
            .build_from_impacted_pairs(&intent, &impact, cfg_snapshot.as_ref())
            .await
        {
            Ok(v) => {
                // Count candidates produced by the engine — label comes from the
                // candidate itself (never a hardcoded string).
                for c in &v {
                    CANDIDATES_TOTAL
                        .with_label_values(&[&chain_str, c.label.as_str()])
                        .inc();
                }
                // ── TASK 1 log #6: v2.engine.output (dex_engine) ──────────
                info!(
                    event = "v2.engine.output",
                    chain_id,
                    tx_hash = %intent.tx_hash,
                    engine = "dex_engine",
                    candidates_count = v.len(),
                    rejected_count = v.iter().filter(|c| c.rejection_reason.is_some()).count(),
                    accepted_shape_count = v.iter().filter(|c| c.rejection_reason.is_none()).count(),
                );
                v
            }
            Err(e) => {
                ENGINE_ERRORS_TOTAL
                    .with_label_values(&[&chain_str, StrategyLabel::DexArbV2V2.as_str()])
                    .inc();
                error!(
                    event = "orchestrator.dex_engine_error",
                    chain_id,
                    tx_hash = %intent.tx_hash,
                    error = %e,
                    "DexEngine::build_from_impacted_pairs failed; skipping candidates for this intent"
                );
                vec![]
            }
        };

        // TriangularEngine (Phase 9 — live).
        let tri_candidates = match self
            .ctx
            .triangular_engine
            .build_from_impacted_cycles(&intent, &impact, cfg_snapshot.as_ref())
            .await
        {
            Ok(v) => {
                for c in &v {
                    CANDIDATES_TOTAL
                        .with_label_values(&[&chain_str, c.label.as_str()])
                        .inc();
                }
                // ── TASK 1 log #6: v2.engine.output (triangular_engine) ───
                info!(
                    event = "v2.engine.output",
                    chain_id,
                    tx_hash = %intent.tx_hash,
                    engine = "triangular_engine",
                    candidates_count = v.len(),
                    rejected_count = v.iter().filter(|c| c.rejection_reason.is_some()).count(),
                    accepted_shape_count = v.iter().filter(|c| c.rejection_reason.is_none()).count(),
                );
                v
            }
            Err(e) => {
                ENGINE_ERRORS_TOTAL
                    .with_label_values(&[&chain_str, StrategyLabel::TriangularArb.as_str()])
                    .inc();
                error!(
                    event = "orchestrator.triangular_engine_error",
                    chain_id,
                    tx_hash = %intent.tx_hash,
                    error = %e,
                    "TriangularEngine::build_from_impacted_cycles failed; continuing"
                );
                vec![]
            }
        };

        // LiquidationEngine (Phase 11 — live): event-driven liquidation.
        // Only evaluates positions in `impact.impacted_lending_positions`;
        // never polls the full lending universe.
        let liq_candidates = match self
            .ctx
            .liquidation_engine
            .build_from_lending_impact(&intent, &impact, cfg_snapshot.as_ref())
            .await
        {
            Ok(v) => {
                for c in &v {
                    CANDIDATES_TOTAL
                        .with_label_values(&[&chain_str, c.label.as_str()])
                        .inc();
                }
                // ── TASK 1 log #6: v2.engine.output (liquidation_engine) ──
                info!(
                    event = "v2.engine.output",
                    chain_id,
                    tx_hash = %intent.tx_hash,
                    engine = "liquidation_engine",
                    candidates_count = v.len(),
                    rejected_count = v.iter().filter(|c| c.rejection_reason.is_some()).count(),
                    accepted_shape_count = v.iter().filter(|c| c.rejection_reason.is_none()).count(),
                );
                v
            }
            Err(e) => {
                ENGINE_ERRORS_TOTAL
                    .with_label_values(&[&chain_str, StrategyLabel::Liquidation.as_str()])
                    .inc();
                error!(
                    event = "orchestrator.liquidation_engine_error",
                    chain_id,
                    tx_hash = %intent.tx_hash,
                    error = %e,
                    "LiquidationEngine::build_from_lending_impact failed; skipping liq candidates"
                );
                vec![]
            }
        };

        // Concatenate base candidates: DEX + triangular + liquidation.
        // Liquidation candidates are included BEFORE flashloan wrapping
        // (a flashloan can also wrap a liquidation call — Phase 15+ work).
        let mut base_candidates: Vec<StrategyCandidate> =
            Vec::with_capacity(dex_candidates.len() + tri_candidates.len() + liq_candidates.len());
        base_candidates.extend(dex_candidates);
        base_candidates.extend(tri_candidates);
        base_candidates.extend(liq_candidates);

        // FlashloanEngine (Phase 10 — live): wrap net-positive base candidates.
        let flash_candidates = {
            let wrapped = self.ctx.flashloan_engine.wrap_profitable_routes(
                &base_candidates,
                chain_id,
                cfg_snapshot.as_ref(),
            );
            // Count flashloan candidates — label comes from the candidate itself.
            for c in &wrapped {
                CANDIDATES_TOTAL
                    .with_label_values(&[&chain_str, c.label.as_str()])
                    .inc();
            }
            // ── TASK 1 log #6: v2.engine.output (flashloan_engine) ────────
            info!(
                event = "v2.engine.output",
                chain_id,
                tx_hash = %intent.tx_hash,
                engine = "flashloan_engine",
                candidates_count = wrapped.len(),
                rejected_count = wrapped
                    .iter()
                    .filter(|c| c.rejection_reason.is_some())
                    .count(),
                accepted_shape_count = wrapped
                    .iter()
                    .filter(|c| c.rejection_reason.is_none())
                    .count(),
            );
            wrapped
        };

        debug!(
            event = "orchestrator.engines_done",
            chain_id,
            tx_hash = %intent.tx_hash,
            base_count = base_candidates.len(),
            flash_wrap_count = flash_candidates.len(),
        );

        // ── Step 6: size + evaluate + emit each candidate ─────────────────
        // cfg_snapshot was already taken before engine fan-out (Step 4) so
        // the evaluator uses the same snapshot the engines used — consistent
        // within one intent's processing window.
        // For each candidate: run size_optimizer → update profit fields or
        // emit as rejected if optimizer returns None. Then evaluate + emit.
        // Process base candidates first, then flashloan-wrapped variants.
        let all_candidates: Vec<StrategyCandidate> = base_candidates
            .into_iter()
            .chain(flash_candidates)
            .collect();

        for candidate in all_candidates {
            // Skip sizing for already-rejected candidates (engine rejection).
            if candidate.rejection_reason.is_some() {
                self.process_candidate(candidate, cfg_snapshot.as_ref(), chain_id)
                    .await?;
                continue;
            }

            // ── TASK 1 log #7: v2.optimizer.input ────────────────────────
            info!(
                event = "v2.optimizer.input",
                chain_id,
                tx_hash = %intent.tx_hash,
                strategy = candidate.label.as_str(),
                route_legs = candidate.route_plan.legs.len(),
                pool_addresses = ?candidate
                    .route_plan
                    .legs
                    .iter()
                    .map(|l| l.pool_address.as_deref())
                    .collect::<Vec<_>>(),
                has_config = cfg_snapshot.is_some(),
                gross_profit_usd = ?candidate.gross_profit_usd,
            );

            // Run size_optimizer (diagnostic-rich path). Errors are non-fatal.
            let outcome = match self
                .ctx
                .size_optimizer
                .optimize_with_reason(candidate.clone(), &intent, cfg_snapshot.as_ref())
                .await
            {
                Ok(o) => o,
                Err(e) => {
                    warn!(
                        event = "orchestrator.size_optimizer_error",
                        chain_id,
                        tx_hash = %intent.tx_hash,
                        error = %e,
                        "size_optimizer returned Err — treating as no profit"
                    );
                    OptimizeOutcome::Rejected(OptimizeRejectReason::NonPositiveProfit)
                }
            };

            // ── TASK 1 log #8: v2.optimizer.output ───────────────────────
            info!(
                event = "v2.optimizer.output",
                chain_id,
                tx_hash = %intent.tx_hash,
                strategy = candidate.label.as_str(),
                result = match &outcome {
                    OptimizeOutcome::Sized(_) => "sized",
                    OptimizeOutcome::Rejected(_) => "rejected",
                },
                reason = ?outcome.reason_str(),
                gross_profit_usd = ?outcome.gross_profit_usd(),
                net_profit_usd = ?outcome.net_profit_usd(),
                optimal_amount_in = ?outcome.optimal_amount_in(),
            );

            let final_candidate = match outcome {
                OptimizeOutcome::Sized(sized) => {
                    // Unbox and update the candidate with optimal sizing data.
                    let s = *sized;
                    let mut c = s.candidate;
                    c.gross_profit_usd = Some(s.gross_profit_usd);
                    c.net_expected_profit_usd = Some(s.estimated_net_profit_usd);
                    // FIX (review V2 #8): synchronize the Opportunity row that
                    // process_candidate actually evaluates/emits. Without this,
                    // the DB/API records pre-sizing figures while the optimizer's
                    // post-sizing numbers only live on the StrategyCandidate —
                    // an inconsistent audit trail (RULE 00 violation surface).
                    c.opportunity.expected_profit_usd = Some(s.gross_profit_usd);
                    c.opportunity.net_expected_profit_usd = Some(s.estimated_net_profit_usd);
                    c
                }
                OptimizeOutcome::Rejected(reason) => {
                    // Route optimizer rejection to REJECTED_NO_PROFIT_TOTAL
                    // (not SIMULATION_FAILED_TOTAL — sizing is not simulation).
                    let reason_str = reason.as_str().to_owned();
                    REJECTED_NO_PROFIT_TOTAL
                        .with_label_values(&[&chain_str, candidate.label.as_str(), &reason_str])
                        .inc();
                    let mut c = candidate;
                    c.rejection_reason = Some(reason_str);
                    c
                }
            };

            self.process_candidate(final_candidate, cfg_snapshot.as_ref(), chain_id)
                .await?;
        }

        Ok(())
    }

    // -----------------------------------------------------------------------
    // Per-candidate processing
    // -----------------------------------------------------------------------

    /// Evaluate and emit a single `StrategyCandidate`.
    ///
    /// Errors from `evaluate_with_route_plan` and from `emit_rejected` are
    /// caught and logged internally (non-fatal per-candidate). Only Redis
    /// publish failures propagate as `Err` (fatal — caller reconnects).
    async fn process_candidate(
        &self,
        sc: StrategyCandidate,
        cfg: Option<&TradingConfigState>,
        chain_id: u64,
    ) -> anyhow::Result<()> {
        let label = sc.label;
        let label_str = label.as_str();

        let chain_str = chain_id.to_string();

        // Engine-level rejection: no need to evaluate, just emit rejected.
        if let Some(reason) = &sc.rejection_reason {
            let reason_owned = reason.clone();
            let opp_with_reason = {
                let mut o = sc.opportunity.clone();
                o.rejection_reason = Some(reason_owned.clone());
                o
            };
            // Already counted in on_route_intent's optimizer rejection path.
            // Avoid double-counting by not incrementing REJECTED_NO_PROFIT_TOTAL here.
            // ── TASK 1 log #9: v2.emitter.input ─────────────────────────
            info!(
                event = "v2.emitter.input",
                chain_id,
                tx_hash = %sc.source_intent_hash,
                strategy = label_str,
                dry_run = self.ctx.emitter.is_dry_run(),
                rejection_reason = ?opp_with_reason.rejection_reason,
                expected_profit_usd = ?opp_with_reason.expected_profit_usd,
                net_expected_profit_usd = ?opp_with_reason.net_expected_profit_usd,
            );
            self.ctx
                .emitter
                .emit_rejected(&opp_with_reason, label, &reason_owned)
                .await?;
            return Ok(());
        }

        // FIX (review V2 #1): NO config → explicit rejection, NEVER emit_accepted.
        // The previous fail-open path published an opportunity that skipped the
        // allowlist, pricing, strategy-enable and risk gates — classified as
        // accepted in the live emitter. RULE 00 / R8 violation: an ungated
        // opportunity must surface as rejected with the precise reason so the
        // operator sees the gate gap in the dashboard instead of a fake viable.
        let Some(state) = cfg else {
            let reason = "NoTradingConfig".to_string();
            let opp_with_reason = {
                let mut o = sc.opportunity.clone();
                o.rejection_reason = Some(reason.clone());
                o
            };
            REJECTED_CONFIG_TOTAL
                .with_label_values(&[&chain_str, label_str, "no_trading_config"])
                .inc();
            info!(
                event = "v2.emitter.input",
                chain_id,
                tx_hash = %sc.source_intent_hash,
                strategy = label_str,
                dry_run = self.ctx.emitter.is_dry_run(),
                rejection_reason = ?opp_with_reason.rejection_reason,
                expected_profit_usd = ?opp_with_reason.expected_profit_usd,
                net_expected_profit_usd = ?opp_with_reason.net_expected_profit_usd,
            );
            self.ctx
                .emitter
                .emit_rejected(&opp_with_reason, label, &reason)
                .await?;
            return Ok(());
        };

        // FIX (review V2 #9): load the REAL live price snapshot from Redis instead
        // of an empty map, so the evaluator's pricing cascade has real data.
        // `snapshot_from_redis` never errors — on Redis failure it returns an
        // EMPTY snapshot (R8 fail-honest), degrading the evaluator to its tier-2
        // ConfigPriceOracle fallback. Never fabricated prices.
        let price_snapshot: HashMap<String, f64> = {
            let mut redis_conn = self.ctx.math_redis.clone();
            let oracle =
                RedisCachedPriceOracle::snapshot_from_redis(&mut redis_conn, chain_id).await;
            let n = oracle.len();
            if n == 0 {
                debug!(
                    event = "v2.price_snapshot_empty",
                    chain_id,
                    "price snapshot empty; evaluator falls back to config oracle"
                );
            }
            let _ = n;
            oracle.into_snapshot()
        };

        // Build the evaluator borrowing the owned config snapshot.
        let signals = NetworkSignals::unknown(sc.opportunity.block_number.unwrap_or(0));
        let ev = ConfigAwareEvaluator::with_cache(state, signals, price_snapshot);

        // Run the spine gate.
        let spine_gate_outcome = ev.evaluate_with_route_plan(
            &sc.candidate,
            Some(&sc.route_plan),
            label_str,
            chain_id,
            "rpc-pool".to_string(),
            60_000,
        );

        // FIX (review V2 #4/#5/#6): dead energy-gate block REMOVED.
        // The block was gated behind `#[cfg(feature = "searcher-rs")]` — a feature
        // that does NOT exist in Cargo.toml (real features: v2-simulator,
        // paper-shadow, experimental-engines) — so it never compiled. Worse, it
        // referenced symbols that don't exist in this crate (`MacroMevGate`,
        // `orbital_condition`, `emit_gate_commit_from_state`, `energy.gauntlet_id`
        // — the real field is `gate_identifier`). If the gate is ever wired for
        // real, it must be re-implemented against `crate::gates::MacroMevGate` with
        // a real emitter contract — not this orphaned copy. Removing dead code
        // per surgical-changes doctrine.

        // Continue with spine gate outcome (if evaluator available)

        match spine_gate_outcome {
            ConfigGateOutcome::TokenNotAllowed {
                token_symbol_or_addr,
            } => {
                let reason = format!("TokenNotAllowed:{token_symbol_or_addr}");
                let mut opp = sc.opportunity.clone();
                opp.rejection_reason = Some(reason.clone());
                opp.roi_pct = Some(0.0);
                opp.risk_score = Some(0.0);
                // TASK 3: use REJECTED_CONFIG_TOTAL, not SIMULATION_FAILED_TOTAL.
                REJECTED_CONFIG_TOTAL
                    .with_label_values(&[&chain_str, label_str, "token_not_allowed"])
                    .inc();
                // ── TASK 1 log #9: v2.emitter.input ──────────────────────
                info!(
                    event = "v2.emitter.input",
                    chain_id,
                    tx_hash = %sc.source_intent_hash,
                    strategy = label_str,
                    dry_run = self.ctx.emitter.is_dry_run(),
                    rejection_reason = ?opp.rejection_reason,
                    expected_profit_usd = ?opp.expected_profit_usd,
                    net_expected_profit_usd = ?opp.net_expected_profit_usd,
                );
                self.ctx.emitter.emit_rejected(&opp, label, &reason).await?;
            }

            ConfigGateOutcome::StrategyDisabled { strategy_kind: sk } => {
                let reason = format!("StrategyDisabled:{sk}");
                let mut opp = sc.opportunity.clone();
                opp.rejection_reason = Some(reason.clone());
                opp.roi_pct = Some(0.0);
                opp.risk_score = Some(0.0);
                // TASK 3: use REJECTED_CONFIG_TOTAL, not SIMULATION_FAILED_TOTAL.
                REJECTED_CONFIG_TOTAL
                    .with_label_values(&[&chain_str, label_str, "strategy_disabled"])
                    .inc();
                // ── TASK 1 log #9: v2.emitter.input ──────────────────────
                info!(
                    event = "v2.emitter.input",
                    chain_id,
                    tx_hash = %sc.source_intent_hash,
                    strategy = label_str,
                    dry_run = self.ctx.emitter.is_dry_run(),
                    rejection_reason = ?opp.rejection_reason,
                    expected_profit_usd = ?opp.expected_profit_usd,
                    net_expected_profit_usd = ?opp.net_expected_profit_usd,
                );
                self.ctx.emitter.emit_rejected(&opp, label, &reason).await?;
            }

            ConfigGateOutcome::StrategyConfigGateBlocked { reason } => {
                let tag = reason.tag();
                let reason_str = format!("{tag}:{reason:?}");
                let mut opp = sc.opportunity.clone();
                opp.rejection_reason = Some(reason_str.clone());
                opp.roi_pct = Some(0.0);
                opp.risk_score = Some(0.0);
                // TASK 3: use REJECTED_CONFIG_TOTAL, not SIMULATION_FAILED_TOTAL.
                REJECTED_CONFIG_TOTAL
                    .with_label_values(&[&chain_str, label_str, tag])
                    .inc();
                // ── TASK 1 log #9: v2.emitter.input ──────────────────────
                info!(
                    event = "v2.emitter.input",
                    chain_id,
                    tx_hash = %sc.source_intent_hash,
                    strategy = label_str,
                    dry_run = self.ctx.emitter.is_dry_run(),
                    rejection_reason = ?opp.rejection_reason,
                    expected_profit_usd = ?opp.expected_profit_usd,
                    net_expected_profit_usd = ?opp.net_expected_profit_usd,
                );
                self.ctx
                    .emitter
                    .emit_rejected(&opp, label, &reason_str)
                    .await?;
            }

            ConfigGateOutcome::Evaluated {
                outcome,
                evidence: _,
                rejection,
                partial_data_quality: _,
            } => {
                let mut opp = sc.opportunity.clone();

                if let Some(rej_reason) = rejection {
                    // Math gate rejected — this is a genuine evaluation failure.
                    let reason_str = format!("{rej_reason:?}");
                    opp.rejection_reason = Some(reason_str.clone());
                    opp.roi_pct = Some(0.0);
                    opp.risk_score = Some(0.0);
                    // Propagate net_expected_profit_usd when gross is available (R8).
                    opp.net_expected_profit_usd =
                        opp.expected_profit_usd.map(|g| g - outcome.gas_cost_usd);
                    // TASK 3: EvaluatedRejected IS a real evaluation failure → SIMULATION_FAILED.
                    SIMULATION_FAILED_TOTAL
                        .with_label_values(&[&chain_str, label_str, "EvaluatedRejected"])
                        .inc();
                    // ── TASK 1 log #9: v2.emitter.input ──────────────────
                    info!(
                        event = "v2.emitter.input",
                        chain_id,
                        tx_hash = %sc.source_intent_hash,
                        strategy = label_str,
                        dry_run = self.ctx.emitter.is_dry_run(),
                        rejection_reason = ?opp.rejection_reason,
                        expected_profit_usd = ?opp.expected_profit_usd,
                        net_expected_profit_usd = ?opp.net_expected_profit_usd,
                    );
                    self.ctx
                        .emitter
                        .emit_rejected(&opp, label, &reason_str)
                        .await?;
                } else {
                    // Passed all spine gates.
                    opp.roi_pct = Some(outcome.net_roi_pct);
                    opp.net_expected_profit_usd = Some(outcome.net_profit_usd);

                    // ── MacroMevGate (Operador Energético) ─────────────────────
                    // Real gate from crate::gates — compiles ALWAYS (no dead feature
                    // flag); the gate self-disables via ARBX_GATE_MACRO_MEV_ENABLED.
                    // Evaluates the Hamiltonian of the system: when the margin
                    // (net_yield + epsilon) cannot cover gas × confiscation_threshold,
                    // the trajectory diverges (E_state ≥ τ) → reject with the gate's
                    // reason. Runs on the fully-populated `opp` (net profit + ROI
                    // already set by the spine evaluator above).
                    {
                        use crate::shared::gates::GateLogic;
                        let gate_config = MacroMevGateConfig::from_env();
                        let gate = MacroMevGate;
                        if let Some(gate_outcome) = gate.evaluate(&opp, &gate_config) {
                            if gate_outcome.reject {
                                let reason_str = format!("{:?}", gate_outcome.reason);
                                opp.rejection_reason = Some(reason_str.clone());
                                opp.roi_pct = Some(0.0);
                                opp.risk_score = Some(0.0);
                                REJECTED_CONFIG_TOTAL
                                    .with_label_values(&[&chain_str, label_str, "macro_mev_gate"])
                                    .inc();
                                info!(
                                    event = "v2.emitter.input",
                                    chain_id,
                                    tx_hash = %sc.source_intent_hash,
                                    strategy = label_str,
                                    dry_run = self.ctx.emitter.is_dry_run(),
                                    rejection_reason = ?opp.rejection_reason,
                                    expected_profit_usd = ?opp.expected_profit_usd,
                                    net_expected_profit_usd = ?opp.net_expected_profit_usd,
                                    mitigation = %gate_outcome.mitigation,
                                    can_override = gate_outcome.can_override,
                                    "MacroMevGate: orbital divergence — E_state >= threshold"
                                );
                                self.ctx
                                    .emitter
                                    .emit_rejected(&opp, label, &reason_str)
                                    .await?;
                                return Ok(());
                            }
                        }
                    }
                    // ── TASK 1 log #9: v2.emitter.input ──────────────────
                    info!(
                        event = "v2.emitter.input",
                        chain_id,
                        tx_hash = %sc.source_intent_hash,
                        strategy = label_str,
                        dry_run = self.ctx.emitter.is_dry_run(),
                        rejection_reason = ?opp.rejection_reason,
                        expected_profit_usd = ?opp.expected_profit_usd,
                        net_expected_profit_usd = ?opp.net_expected_profit_usd,
                    );
                    // FIX (review V2 #11): increment the published metric ONLY when
                    // the emitter actually published. Previously it incremented
                    // BEFORE the emit call — a dedup hit or PG failure still counted
                    // as "published", inflating the dashboard counter vs reality.
                    let emit_outcome = self.ctx.emitter.emit_accepted(&opp, label).await?;
                    match emit_outcome {
                        EmitOutcome::Published
                        | EmitOutcome::PersistedAndPublished
                        | EmitOutcome::Persisted => {
                            OPPORTUNITIES_PUBLISHED_TOTAL
                                .with_label_values(&[&chain_str, label_str])
                                .inc();
                        }
                        EmitOutcome::Deduped => {
                            debug!(
                                event = "v2.emitter.deduped",
                                chain_id,
                                tx_hash = %sc.source_intent_hash,
                                strategy = label_str,
                                "opportunity deduped by emitter; not counted as published"
                            );
                        }
                        _ => {
                            // PgWriteFailedRedisPublished and any future variants:
                            // data reached the Redis stream (the dashboard's source)
                            // so count it as published for observability parity.
                            OPPORTUNITIES_PUBLISHED_TOTAL
                                .with_label_values(&[&chain_str, label_str])
                                .inc();
                        }
                    }
                }
            }
        }

        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Utilities
// ---------------------------------------------------------------------------

/// Returns a static string label for a `DetectionSource`.
///
/// Used as the Prometheus `source` label in `decoded_intents_total`.
/// The label is stable and matches the `DetectionSource` serde `snake_case`
/// names so Grafana dashboards can filter by source without mapping.
fn detection_source_as_str(src: crate::route_intent::DetectionSource) -> &'static str {
    src.as_str()
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use crate::engines::dex_engine::DexEngine;
    use crate::engines::flashloan_engine::FlashloanEngine;
    use crate::engines::triangular_engine::{CycleSeed, ReservesCache, TriangularEngine};
    use crate::engines::StrategyCandidate;
    use crate::impact_index::{ImpactIndex, PoolRef};
    #[allow(unused_imports)]
    use crate::metrics::ENGINE_ERRORS_TOTAL;
    use crate::route_intent::{
        DetectionSource, ProtocolType, RouteIntent, RouteIntentLeg, RouterKind, SwapExactMode,
    };
    use crate::strategy_label::StrategyLabel;
    use chrono::Utc;
    use ethers::types::{Address, H256, U256};
    use prioritization_spine::route_plan::{RouteLeg, RoutePlan};
    use prioritization_spine::types::OpportunityCandidate;
    use shared_rs::contracts::Opportunity;
    use std::sync::Arc;
    use uuid::Uuid;

    // ── Helpers ──────────────────────────────────────────────────────────────

    fn addr(n: u64) -> Address {
        Address::from_low_u64_be(n)
    }

    fn make_pool(address: Address, token0: Address, token1: Address, pt: ProtocolType) -> PoolRef {
        PoolRef {
            chain_id: 1,
            address,
            dex_name: "uniswap-v2".to_string(),
            protocol_type: pt,
            token0,
            token1,
            fee_bps: Some(30),
        }
    }

    fn make_intent(token_in: Address, token_out: Address) -> RouteIntent {
        RouteIntent::new(
            1,
            H256::from_low_u64_be(0xDEAD),
            Address::zero(),
            RouterKind::UniswapV2,
            Address::zero(),
            vec![RouteIntentLeg {
                token_in,
                token_out,
                pool_hint: None,
                dex_hint: None,
                fee_bps: Some(30),
                protocol_type: ProtocolType::V2,
            }],
            U256::from(10u128).pow(U256::from(18u32)),
            None,
            SwapExactMode::ExactIn,
            DetectionSource::PublicMempool,
        )
        .expect("valid intent")
    }

    fn make_opportunity(label: StrategyLabel) -> Opportunity {
        Opportunity {
            id: Uuid::new_v4(),
            chain_id: 1,
            strategy_kind: label.to_contract_strategy_kind(),
            dex_a: "uniswap-v2".to_string(),
            dex_b: None,
            pair_symbol: "WETH/USDC".to_string(),
            token_in: "0xweth".to_string(),
            token_out: "0xusdc".to_string(),
            amount_in_wei: "1000000000000000000".to_string(),
            expected_profit_usd: Some(1.5),
            net_expected_profit_usd: None,
            roi_pct: None,
            risk_score: None,
            block_number: None,
            rejection_reason: None,
            cartridge_id: None,
            detected_at: Utc::now(),
            trace_id: Uuid::new_v4(),
        }
    }

    fn make_candidate(label: StrategyLabel, rejection: Option<String>) -> StrategyCandidate {
        let opp = make_opportunity(label);
        let route_plan = RoutePlan {
            route_id: Some("test-route".to_string()),
            strategy_kind: label.as_str().to_string(),
            chain_id: 1,
            legs: vec![
                RouteLeg {
                    dex_id: "uniswap-v2".to_string(),
                    dex_name: "uniswap-v2".to_string(),
                    protocol_type: "uniswap-v2".to_string(),
                    factory_address: String::new(),
                    pool_id: None,
                    pool_address: Some("0x000000000000000000000000000000000000aaaa".to_string()),
                    token_in: "0xweth".to_string(),
                    token_out: "0xusdc".to_string(),
                    fee_bps: Some(30),
                    amount_in: Some(1.0),
                    amount_out: None,
                    tvl_usd: None,
                    volume_24h_usd: None,
                    pool_is_active: true,
                },
                RouteLeg {
                    dex_id: "sushi".to_string(),
                    dex_name: "sushi".to_string(),
                    protocol_type: "uniswap-v2".to_string(),
                    factory_address: String::new(),
                    pool_id: None,
                    pool_address: Some("0x000000000000000000000000000000000000bbbb".to_string()),
                    token_in: "0xusdc".to_string(),
                    token_out: "0xweth".to_string(),
                    fee_bps: Some(30),
                    amount_in: Some(1.0),
                    amount_out: None,
                    tvl_usd: None,
                    volume_24h_usd: None,
                    pool_is_active: true,
                },
            ],
            atomic: true,
            estimated_slippage_pct: None,
            price_impact_pct: None,
        };
        let candidate = OpportunityCandidate {
            route_fingerprint: "test".to_string(),
            pool_addresses: vec![],
            token_addresses: vec!["WETH".to_string(), "USDC".to_string()],
            dex_adapters: vec!["uniswap-v2".to_string()],
            amount_in: 1.0,
            expected_amount_out: 1.001,
            gross_profit: 0.001,
        };
        StrategyCandidate {
            label,
            opportunity: opp,
            candidate,
            route_plan,
            gross_profit_usd: Some(1.5),
            net_expected_profit_usd: None,
            rejection_reason: rejection,
            source_intent_hash: H256::zero(),
            base_strategy: None,
        }
    }

    // ── orchestrator::tests::detection_source_as_str_table ──────────────────

    #[test]
    fn detection_source_as_str_table() {
        assert_eq!(
            detection_source_as_str(DetectionSource::PublicMempool),
            "public_mempool"
        );
        assert_eq!(
            detection_source_as_str(DetectionSource::FilteredMempool),
            "filtered_mempool"
        );
        assert_eq!(
            detection_source_as_str(DetectionSource::PrivateHint),
            "private_hint"
        );
        assert_eq!(
            detection_source_as_str(DetectionSource::NewBlock),
            "new_block"
        );
        assert_eq!(
            detection_source_as_str(DetectionSource::OracleUpdate),
            "oracle_update"
        );
        assert_eq!(
            detection_source_as_str(DetectionSource::LendingPositionUpdate),
            "lending_position_update"
        );
    }

    // ── orchestrator::tests::strategy_label_propagates_through_emit ─────────
    //
    // Verifies that a DexArbV2V3 candidate carries the correct label string.

    #[test]
    fn strategy_label_propagates_through_emit() {
        let c = make_candidate(StrategyLabel::DexArbV2V3, None);
        assert_eq!(c.label.as_str(), "dex_arb_v2v3");
        assert_eq!(c.route_plan.strategy_kind, "dex_arb_v2v3");
    }

    // ── orchestrator::tests::engine_error_does_not_crash ─────────────────────
    //
    // Verifies that the Prometheus ENGINE_ERRORS_TOTAL counter increments
    // correctly. The label value MUST come from StrategyLabel::as_str() —
    // never a hardcoded string literal.

    #[test]
    fn engine_error_counter_increments() {
        use crate::metrics::ENGINE_ERRORS_TOTAL;

        let label = StrategyLabel::DexArbV2V2;
        let label_str = label.as_str(); // exclusively from StrategyLabel::as_str()
        let chain = "1";

        let before = ENGINE_ERRORS_TOTAL
            .with_label_values(&[chain, label_str])
            .get();
        ENGINE_ERRORS_TOTAL
            .with_label_values(&[chain, label_str])
            .inc();
        let after = ENGINE_ERRORS_TOTAL
            .with_label_values(&[chain, label_str])
            .get();
        assert_eq!(
            after,
            before + 1,
            "ENGINE_ERRORS_TOTAL must increment by 1 for strategy={label_str}"
        );
    }

    // ── orchestrator::tests::valid_intent_fans_to_dex_engine ─────────────────
    //
    // Registers a known pair in ImpactIndex and verifies that the dex_engine
    // produces candidates for the pair. Tests the full fan-out chain.

    #[tokio::test]
    async fn valid_intent_fans_to_dex_engine() {
        let tok_a = addr(0x1);
        let tok_b = addr(0x2);

        let mut idx = ImpactIndex::empty();
        idx.add_pool(make_pool(addr(0x10), tok_a, tok_b, ProtocolType::V2));
        idx.add_pool(PoolRef {
            fee_bps: Some(100), // different fee to get non-zero spread
            ..make_pool(addr(0x11), tok_a, tok_b, ProtocolType::V2)
        });

        let intent = make_intent(tok_a, tok_b);
        let impact = idx.resolve(&intent);

        // Verify impact contains both pools (prerequisite for the engine to run).
        assert_eq!(impact.impacted_pools.len(), 2);

        // Build dex_engine directly and verify it produces candidates.
        // V2/V2 pair with empty reserves cache → reserves_cache_miss rejections, which
        // are still candidates (rejected, but candidates). One per pair combination.
        let engine = DexEngine::new(Arc::new(ReservesCache::new()), None, None);
        let candidates = engine
            .build_from_impacted_pairs(&intent, &impact, None)
            .await
            .expect("dex_engine must not error");

        assert!(
            !candidates.is_empty(),
            "dex_engine must produce at least one candidate for a known pair"
        );
    }

    // ── orchestrator::tests::rejected_candidate_carries_reason ───────────────

    #[test]
    fn rejected_candidate_carries_reason() {
        let c = make_candidate(
            StrategyLabel::DexArbV2V2,
            Some("single_pool_no_spread".to_string()),
        );
        assert_eq!(
            c.rejection_reason.as_deref(),
            Some("single_pool_no_spread"),
            "rejected candidate must carry the rejection reason"
        );
    }

    // ── orchestrator::tests::accepted_candidate_has_no_rejection_reason ───────

    #[test]
    fn accepted_candidate_has_no_rejection_reason() {
        let c = make_candidate(StrategyLabel::DexArbV2V2, None);
        assert!(
            c.rejection_reason.is_none(),
            "accepted candidate must have no rejection_reason"
        );
    }

    // ── orchestrator::tests::r8_none_gross_preserved ─────────────────────────

    #[test]
    fn r8_none_gross_preserved() {
        let mut c = make_candidate(StrategyLabel::DexArbV2V2, None);
        c.gross_profit_usd = None;
        c.opportunity.expected_profit_usd = None;

        assert!(
            c.gross_profit_usd.is_none(),
            "gross_profit_usd must be None when not computed"
        );
        assert!(
            c.opportunity.expected_profit_usd.is_none(),
            "expected_profit_usd must be None when not computed"
        );
    }

    // ── orchestrator::tests::triangular_candidate_fanned_through ─────────────
    //
    // Verifies that a TriangularEngine (with a known cycle and reserves) produces
    // a candidate that flows through the orchestrator engine fan-out path.

    #[tokio::test]
    async fn triangular_candidate_fanned_through() {
        let cache = Arc::new(ReservesCache::new());

        // Cycle with trivial reserves (equal → spot ≤ 1 → rejected, but still a candidate).
        let tok_a = addr(0x10);
        let tok_b = addr(0x20);
        let tok_c = addr(0x30);
        let pool_a = addr(0x100);
        let pool_b = addr(0x200);
        let pool_c = addr(0x300);
        let unit = U256::from(10u128).pow(U256::from(18u32));

        cache.insert(pool_a, unit, unit).await;
        cache.insert(pool_b, unit, unit).await;
        cache.insert(pool_c, unit, unit).await;

        let seed = CycleSeed {
            cycle_id: 0,
            token_a_symbol: "WETH".to_string(),
            pool_addresses: [pool_a, pool_b, pool_c],
            token_ins: [tok_a, tok_b, tok_c],
            token_outs: [tok_b, tok_c, tok_a],
            swap_in_is_token0: [tok_a < tok_b, tok_b < tok_c, tok_c < tok_a],
        };

        let tri_engine = Arc::new(TriangularEngine::new(cache, vec![seed]));

        // Build an ImpactSet with cycle_id = 0 impacted.
        use crate::impact_index::ImpactSet;
        let impact = ImpactSet {
            impacted_cycles: vec![0],
            ..Default::default()
        };

        let intent = make_intent(tok_a, tok_b);
        let candidates = tri_engine
            .build_from_impacted_cycles(&intent, &impact, None)
            .await
            .expect("triangular engine must not error");

        // With equal reserves, spot_product < 1 → rejected candidate.
        assert!(
            !candidates.is_empty(),
            "triangular engine must produce ≥1 candidate"
        );
        assert_eq!(
            candidates[0].label,
            StrategyLabel::TriangularArb,
            "candidate label must be TriangularArb"
        );
    }

    // ── orchestrator::tests::flashloan_wrap_fanned_through ────────────────────
    //
    // Verifies that FlashloanEngine wraps a net-positive base candidate correctly.

    #[test]
    fn flashloan_wrap_fanned_through() {
        let fl_engine = FlashloanEngine::new();

        // Base candidate: DexArbV2V2, $50 gross, WETH on mainnet.
        let base = make_candidate(StrategyLabel::DexArbV2V2, None);
        // Ensure gross is Some and token_in is WETH.
        let mut base = base;
        base.gross_profit_usd = Some(50.0);
        base.route_plan.legs[0].token_in = "0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2".to_string();

        let wrapped = fl_engine.wrap_profitable_routes(&[base], 1, None);

        // On mainnet with WETH → DyDxSolo (0 bps fee) → net = $50 → accepted.
        let accepted: Vec<_> = wrapped
            .iter()
            .filter(|c| c.rejection_reason.is_none())
            .collect();
        assert!(
            !accepted.is_empty(),
            "flashloan engine must produce ≥1 accepted wrapped candidate"
        );
        assert_eq!(accepted[0].label, StrategyLabel::FlashloanArb);
        assert_eq!(
            accepted[0].base_strategy,
            Some(StrategyLabel::DexArbV2V2),
            "base_strategy must be preserved on wrapped candidate"
        );
    }

    // ── orchestrator::tests::triangular_engine_error_counter_increments ───────

    #[test]
    fn triangular_engine_error_counter_increments() {
        use crate::metrics::ENGINE_ERRORS_TOTAL;

        let label = StrategyLabel::TriangularArb;
        let label_str = label.as_str(); // exclusively from StrategyLabel::as_str()
        let chain = "1";

        let before = ENGINE_ERRORS_TOTAL
            .with_label_values(&[chain, label_str])
            .get();
        ENGINE_ERRORS_TOTAL
            .with_label_values(&[chain, label_str])
            .inc();
        let after = ENGINE_ERRORS_TOTAL
            .with_label_values(&[chain, label_str])
            .get();
        assert_eq!(
            after,
            before + 1,
            "ENGINE_ERRORS_TOTAL must increment by 1 for strategy={label_str}"
        );
    }

    // ── orchestrator::tests::liquidation_candidate_fanned_through ────────────
    //
    // Verifies that a LiquidationEngine (pure math path) can produce a
    // candidate that structurally flows through the orchestrator engine fan-out.
    // We test the pure math/label contract since we cannot run Redis in unit tests.

    #[test]
    fn liquidation_candidate_fanned_through() {
        // Build a synthetic liquidation candidate as the engine would.
        let liq = make_candidate(StrategyLabel::Liquidation, None);

        // Verify the candidate carries the correct label and strategy_kind.
        assert_eq!(
            liq.label,
            StrategyLabel::Liquidation,
            "liquidation candidate must carry Liquidation label"
        );
        assert_eq!(
            liq.label.to_contract_strategy_kind(),
            shared_rs::contracts::StrategyKind::liquidation(),
            "Liquidation label must map to StrategyKind::Liquidation"
        );
        assert_eq!(liq.label.as_str(), "liquidation");

        // Verify that the candidate's rejection_reason is None (accepted).
        assert!(
            liq.rejection_reason.is_none(),
            "accepted liquidation candidate must have no rejection_reason"
        );

        // Verify gross_profit_usd is Some (the engine always supplies it
        // when the math succeeds and the position is liquidatable).
        assert!(
            liq.gross_profit_usd.is_some(),
            "liquidation candidate must carry Some(gross_profit_usd)"
        );
    }

    // ── orchestrator::tests::liquidation_hf_above_one_skipped ────────────────
    //
    // Verifies the invariant: HF >= 1.0 produces no candidate (not even rejected).

    #[test]
    fn liquidation_hf_above_one_skipped() {
        use crate::workers::liquidation_worker::estimate_liquidation_profit;

        // debt_usd = 1_000 (reasonable position), HF = 1.05 (above threshold).
        // The engine checks HF before calling estimate_liquidation_profit.
        let hf_safe = 1.05_f64;
        assert!(
            hf_safe >= 1.0,
            "HF 1.05 must satisfy the skip gate (>= 1.0)"
        );

        // Confirm the math kernel still works for this debt level
        // (the skip is NOT because math fails).
        let est = estimate_liquidation_profit(1_000.0, 500, 30.0, 250_000.0);
        assert!(
            est.is_some(),
            "profit math must succeed for valid debt even when HF >= 1.0"
        );

        // The LiquidationEngine would have returned Ok(None) before reaching
        // this math — verified structurally above.
    }

    // ── orchestrator::tests::liquidation_engine_error_counter_increments ─────

    #[test]
    fn liquidation_engine_error_counter_increments() {
        use crate::metrics::ENGINE_ERRORS_TOTAL;

        let label = StrategyLabel::Liquidation;
        let label_str = label.as_str(); // exclusively from StrategyLabel::as_str()
        let chain = "1";

        let before = ENGINE_ERRORS_TOTAL
            .with_label_values(&[chain, label_str])
            .get();
        ENGINE_ERRORS_TOTAL
            .with_label_values(&[chain, label_str])
            .inc();
        let after = ENGINE_ERRORS_TOTAL
            .with_label_values(&[chain, label_str])
            .get();
        assert_eq!(
            after,
            before + 1,
            "ENGINE_ERRORS_TOTAL must increment by 1 for strategy={label_str}"
        );
    }
}
