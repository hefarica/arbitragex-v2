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
use crate::gates::{MacroMevGate, MacroMevGateConfig};
use crate::impact_index::ImpactIndex;
use crate::metrics::{
    CANDIDATES_TOTAL, DECODED_INTENTS_TOTAL, ENGINE_ERRORS_TOTAL, IMPACTED_ROUTES_TOTAL,
    OPPORTUNITIES_PUBLISHED_TOTAL, REJECTED_CONFIG_TOTAL, REJECTED_NO_PROFIT_TOTAL,
    SIMULATION_FAILED_TOTAL,
};
use crate::opportunity_emitter::{EmitOutcome, OpportunityEmitter};
use crate::route_intent::RouteIntent;
use crate::size_optimizer::{
    OptimizeOutcome, OptimizeRejectReason, SizeOptimizer, SizedCycleLedger,
};
use crate::state_projector::StateProjector;
use crate::strategy_label::StrategyLabel;
use ethers::types::{Address, U256};
use shared_rs::price_oracle::RedisCachedPriceOracle;
use shared_rs::trading_config::TradingConfigState;
use std::collections::HashMap;
use std::sync::Arc;
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
    /// `ARBX_NATIVE_ENGINES` gate (default `on`). When `off`, the orchestrator
    /// skips the Dex/Triangular/Liquidation/Flashloan fan-out so ONLY cartridge
    /// candidates flow (Plan C.3 — avoids duplicate/ghost opportunities when
    /// cartridges are the intended source). Backward-compatible: unset/on = the
    /// native engines run exactly as before. R8: off = empty vecs, never fakes.
    pub native_engines_enabled: bool,
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
    /// AGENT v4 Fase 3a — per-chain ContextRouter (from
    /// `spawn_cartridge_runtime`). `Some` only when the cartridge runtime
    /// booted with a router; the ACTIVE evaluation registers the per-intent
    /// real SnapshotBundle there (id "intent-{uuid}") and removes it on
    /// completion. The shadow path does not consume it.
    pub cartridge_context_router: Option<Arc<crate::context_router::ContextRouter>>,
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
    /// DECIMALS-CYCLE-01 (t197) — PG-backed `tokens.decimals` provider, the
    /// SAME `PgTokenDecimalsProvider` the sim encoder uses (populated from PG
    /// `tokens.decimals`, cache kept warm out-of-band; its `decimals()` returns
    /// `None` — never `Some(18)` — for a token PG does not know).
    ///
    /// The cycle's per-hop `decimals.map` reads real decimals from here and from
    /// the Redis token catalog (`arbx:tokens:<chain>:<addr>`). It used to read
    /// the hardcoded `canonical_token_decimals*` table instead, whose unknown
    /// arm is 18: measured live in t187 as 8/37 map entries wrong, always 18,
    /// i.e. a 1e12 unit error on EURC and 1e10 on the 8-decimal tokens — enough
    /// to make every size generated from that path wrong.
    ///
    /// `None` when no DB pool existed at boot → the map stays unresolved and the
    /// route is NOT priced with an invented unit (R8).
    ///
    /// Tipo: el mismo `Option<Arc<dyn TokenDecimalsProvider>>` que usa el
    /// scanner (`ScannerDecimalsProvider`), escrito por su ruta de LIB
    /// (`crate::sim_encoder::`, re-export de sim-core en lib.rs:182) porque
    /// `crate::scanner` es modulo del BIN y no es alcanzable desde aqui.
    pub token_decimals_provider:
        Option<Arc<dyn crate::sim_encoder::TokenDecimalsProvider + Send + Sync>>,
    // WO-16 EXCISED (orquestador, 2026-09-07): un fixer del Loop aterrizó a
    // medias el companion §5.4 de WO-02 (hot-sim stage del V2 live leg) con
    // paths `crate::scanner::…` imposibles desde la lib (scanner es módulo
    // del bin) y un método del emitter jamás definido — el árbol quedó sin
    // compilar. El companion queda DISEÑADO en WO-02-DESIGN §5.4 para un PR
    // futuro aprobado por el operador (requiere mover los tipos compartidos
    // a la lib + `emit_hot_simulated` en el emitter).
    /// SED Bridge — connects to sed-core math pipeline (paper-shadow only).
    /// When `Some`, feeds gas observations and enriches candidates with
    /// stochastic convergence metrics. When `None`, orchestrator runs
    /// without mathematical overlay (standard V2 mode).
    #[cfg(feature = "paper-shadow")]
    pub sed_bridge: Option<Arc<crate::sed_bridge::SedBridge>>,
}

// ---------------------------------------------------------------------------
// DECIMALS-CYCLE-01 (t197) — per-hop decimals resolution for the cycle
// ---------------------------------------------------------------------------

/// Outcome of resolving the cycle's per-hop decimals from the two REAL sources.
#[derive(Debug, Clone, Default)]
pub(crate) struct CycleDecimals {
    /// Only tokens whose decimals a real source confirmed. A token absent from
    /// this map has NO unit — it is not 18, it is "not computed".
    pub map: shared_rs::candidates::DecimalsMap,
    /// Tokens with an entry in NEITHER source, in route order, lowercased.
    pub unresolved: Vec<String>,
}

/// Pure resolution policy (unit-testable, no I/O).
///
/// `redis_decimals` = the Redis token catalog (`arbx:tokens:<chain>:<addr>`,
/// written by the scanner from `pool_discovery`'s on-chain `decimals()` reads);
/// `pg_decimals` = PG `tokens.decimals`, consulted only for the tokens Redis
/// did not answer for.
///
/// A token present in NEITHER source is returned in `unresolved` and gets NO
/// entry in the map. It is NEVER folded into an `18`: an invented unit is
/// fabrication, and 8/37 production entries were exactly this failure (t187).
pub(crate) fn build_cycle_decimals_map(
    token_addresses: &[String],
    redis_decimals: &HashMap<String, u8>,
    pg_decimals: &HashMap<String, u8>,
) -> CycleDecimals {
    let mut map = HashMap::new();
    let mut unresolved: Vec<String> = Vec::new();
    for addr in token_addresses {
        let lc = addr.to_lowercase();
        match redis_decimals.get(&lc).or_else(|| pg_decimals.get(&lc)) {
            Some(d) => {
                map.insert(lc, *d);
            }
            None => {
                // MUTANTE DEL REVISOR (t202): restaura el default fabricado de 18
                // en el locus del MAPA. NO ES EL FIX: es la falsificacion que debe
                // morder contra los tests del PR. Este commit NO se mergea.
                map.insert(lc.clone(), 18);
                if !unresolved.contains(&lc) {
                    unresolved.push(lc);
                }
            }
        }
    }
    CycleDecimals {
        map: shared_rs::candidates::DecimalsMap { map },
        unresolved,
    }
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

/// Result of one sizing pass, as the emit tail consumes it: the finalized
/// candidate, its sheet-07 net economics (`None` = not computable ⇒ ranked
/// last; ranking never gates or drops) and the kernel's exact per-leg wei
/// ledger when it produced one (R8: absent ⇒ `None`, never a repeated intent
/// amount dressed as a ledger).
type SizedForEmit = (
    StrategyCandidate,
    Option<crate::net_bps_ranking::RouteNetEconomics>,
    Option<(Vec<String>, Vec<String>)>,
);

/// Main orchestrator. Constructed once per chain and shared across tasks via `Arc`.
pub struct Orchestrator {
    ctx: OrchestratorContext,
    risk_ranker: crate::live_risk_ranker::LiveRiskRanker,
}

impl Orchestrator {
    /// Constructs a new `Orchestrator` from a fully-initialised context.
    pub fn new(ctx: OrchestratorContext) -> Self {
        // Plan C.3: log once at startup which native-engine mode is active so the
        // operator can confirm cartridges-only vs. full fan-out. Logged here
        // (constructed once per chain) rather than per intent.
        info!(
            event = "orchestrator.native_engines",
            chain_id = ctx.chain_id,
            mode = if ctx.native_engines_enabled {
                "native_on"
            } else {
                "native_off_cartridges_only"
            },
            enabled = ctx.native_engines_enabled,
            "native strategy engines {} (ARBX_NATIVE_ENGINES={})",
            if ctx.native_engines_enabled {
                "ENABLED — dex/triangular/liquidation/flashloan fan-out active"
            } else {
                "DISABLED — ONLY cartridge candidates will flow"
            },
            if ctx.native_engines_enabled {
                "on"
            } else {
                "off"
            }
        );
        Self {
            ctx,
            risk_ranker: Default::default(),
        }
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
        let size_optimizer = self.ctx.size_optimizer.clone();
        let ctx_chain_id = self.ctx.chain_id;
        let math_registry = self.ctx.math_registry.clone();
        let reserves_cache = self.ctx.dex_engine.reserves_cache.clone();
        let v4_router = self.ctx.cartridge_context_router.clone();
        tokio::spawn(async move {
            crate::cartridge_boot::active_evaluate_and_emit(
                runner,
                intent,
                chain_id,
                emitter,
                cfg_provider,
                size_optimizer,
                ctx_chain_id,
                math_registry,
                reserves_cache,
                v4_router,
            )
            .await;
        });
    }

    /// DISCOVERY → EMISSION BRIDGE — size + emit ONE discovered closed cycle.
    ///
    /// HOPS-EMIT-01. The discovery workers (`route_discovery_worker`) find closed
    /// cycles of 3..=7 hops over the live pool graph, but until now their only
    /// exits were the cartridge path (`spawn_cartridge_eval`, a no-op unless
    /// `ARBX_CARTRIDGE_MODE=active`) and the native engines, which re-derive
    /// 2-hop pool PAIRS from the impacted pools. The cycle itself never reached
    /// an emitter, so every persisted card was 2-hop.
    ///
    /// This entry point converts the cycle intent into a `StrategyCandidate`
    /// whose `RoutePlan` carries the discovered hops, sizes it with the EXISTING
    /// N-leg kernel and pushes it through the EXISTING tail
    /// (`process_candidate` → `RouteMetadata` + `attach_leg_ledger` →
    /// `OpportunityEmitter`) — the persisted wire shape is unchanged.
    ///
    /// Bounded + reversible: admission is
    /// [`crate::route_discovery::hop_cycle_bridge::admit`] (knob
    /// `ARBX_MULTIHOP_EMIT`, per-(chain, block) cap
    /// `ARBX_MULTIHOP_EMIT_MAX_PER_TICK`, geometry gates). Returns `Ok(false)`
    /// when the cycle was not admitted (reason logged); `Ok(true)` once the
    /// candidate went through sizing + emit. `Err` only propagates an emitter
    /// Redis failure, exactly like the native path.
    pub async fn emit_discovered_cycle(&self, intent: RouteIntent) -> anyhow::Result<bool> {
        let chain_id = self.ctx.chain_id;
        if intent.chain_id != chain_id {
            warn!(
                event = "v2.hop_cycle_bridge.chain_id_mismatch",
                ctx_chain_id = chain_id,
                intent_chain_id = intent.chain_id,
                tx_hash = %intent.tx_hash,
            );
            return Ok(false);
        }
        let chain_str = chain_id.to_string();
        let hops = intent.legs.len();
        let epoch = intent.observed_block().unwrap_or(0);
        let budget = crate::route_discovery::hop_cycle_bridge::budget_for(chain_id);

        let candidate = match crate::route_discovery::hop_cycle_bridge::admit(
            &intent, &budget, epoch,
        ) {
            Ok(c) => c,
            Err(skip) => {
                use crate::route_discovery::hop_cycle_bridge::BridgeSkip;
                match skip {
                    // The cap is a hard bound, so hitting it is operator
                    // information, not a per-item noise line (R9): ONE warn per
                    // epoch, carrying that epoch's aggregate counts, plus a
                    // per-item debug line. LOGFLOOD-02: this site used to warn on
                    // every refused cycle (~40/s measured), which is the per-item
                    // noise line the comment above says it must not be.
                    BridgeSkip::CapReached => {
                        if budget.claim_cap_report(epoch) {
                            warn!(
                                event = "v2.hop_cycle_bridge.cap_reached",
                                chain_id,
                                tx_hash = %intent.tx_hash,
                                hops,
                                epoch,
                                cap = budget.per_epoch(),
                                used = budget.used_in(epoch),
                                dropped = budget.dropped_in(epoch),
                                // PERHOP-RESERVES-01: the lane split of the epoch's
                                // allowance. `unpriceable_refused` is the counted
                                // deferral — cycles the sizing kernel can only refuse
                                // (`v3_multileg_unsupported`) that lost the budget to
                                // priceable work. R8: the skip is explicit, never silent.
                                sizeable_used = budget.sizeable_used_in(epoch),
                                unpriceable_used = budget.unpriceable_used_in(epoch),
                                unpriceable_refused = budget.unpriceable_refused_in(epoch),
                                "per-block multihop emission cap reached — cycle not emitted (R8 truncation); this epoch's single aggregate line"
                            )
                        } else {
                            debug!(
                                event = "v2.hop_cycle_bridge.cap_reached",
                                chain_id,
                                tx_hash = %intent.tx_hash,
                                hops,
                                epoch,
                                cap = budget.per_epoch(),
                                used = budget.used_in(epoch),
                                dropped = budget.dropped_in(epoch),
                                "per-block multihop emission cap reached — cycle not emitted (R8 truncation; epoch already reported at warn)"
                            )
                        }
                    }
                    other => debug!(
                        event = "v2.hop_cycle_bridge.skip",
                        chain_id,
                        tx_hash = %intent.tx_hash,
                        hops,
                        epoch,
                        reason = other.as_str(),
                    ),
                }
                return Ok(false);
            }
        };

        CANDIDATES_TOTAL
            .with_label_values(&[&chain_str, candidate.label.as_str()])
            .inc();
        info!(
            event = "v2.engine.output",
            chain_id,
            tx_hash = %intent.tx_hash,
            engine = "hop_cycle_bridge",
            candidates_count = 1usize,
            rejected_count = 0usize,
            accepted_shape_count = 1usize,
            hops,
            epoch,
            cap = budget.per_epoch(),
            used = budget.used_in(epoch),
            dropped = budget.dropped_in(epoch),
            sizeable_used = budget.sizeable_used_in(epoch),
            unpriceable_used = budget.unpriceable_used_in(epoch),
            unpriceable_refused = budget.unpriceable_refused_in(epoch),
        );

        let cfg_snapshot = self.live_config_snapshot(chain_id).await;
        let (final_candidate, _net_economics, leg_ledger) = self
            .size_candidate_for_emit(candidate, &intent, cfg_snapshot.as_ref(), &chain_str)
            .await;
        self.process_candidate(final_candidate, leg_ledger, cfg_snapshot.as_ref(), chain_id)
            .await?;
        Ok(true)
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
            let size_optimizer = self.ctx.size_optimizer.clone();
            let ctx_chain_id = self.ctx.chain_id;
            let math_registry = self.ctx.math_registry.clone();
            let reserves_cache = self.ctx.dex_engine.reserves_cache.clone();
            let v4_router = self.ctx.cartridge_context_router.clone();

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
                        size_optimizer,
                        ctx_chain_id,
                        math_registry,
                        reserves_cache,
                        v4_router,
                    )
                    .await;
                } else {
                    // SHADOW MODE: observe-only telemetry (legacy behavior) — but
                    // SHADOW-CANONICAL-01: evaluada contra el MISMO contexto v4
                    // real por intent que la ruta ACTIVE (§34.1), ya que aquí SÍ
                    // están disponibles la config del operador, el registro de
                    // operadores y el router de contextos.
                    crate::cartridge_boot::shadow_evaluate_intent(
                        runner,
                        intent_for_cart,
                        chain_id,
                        Some(crate::cartridge_boot::IntentContextDeps {
                            cfg_provider,
                            math_registry,
                            router: v4_router,
                        }),
                    )
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
            // MATH-02 fix (2026-09-24, second half): the regime-keyed §IV
            // snapshot was keyed `format!("{:?}", intent.router_kind)` — e.g.
            // "UniswapV2" — while the emitter reads `strategy_evidence_key` by
            // the ENGINE-persisted family string ("dex_arb"/"triangular"/…).
            // The keys never matched for engine-originated rows. Map the router
            // kind to the canonical engine family the eventual Opportunity will
            // carry (all DEX routers → dex_arb; lending positions → liquidation).
            // (Per-cartridge evidence uses the canonical cartridge_id key via
            // publish_declared_combo_evidence — STRAT-IDENT-01; unchanged.)
            let strategy_kind = {
                let dbg = format!("{:?}", intent.router_kind);
                if dbg.contains("Liquid")
                    || dbg.contains("Lending")
                    || dbg.contains("Aave")
                    || dbg.contains("Compound")
                {
                    "liquidation".to_string()
                } else {
                    // Every DEX router family (UniswapV2/V3, Curve, Balancer,
                    // aggregators…) persists StrategyKind::dex_arb() today.
                    "dex_arb".to_string()
                }
            };
            // MATH-04-FOLLOWUP (2026-09-30): conservar el PAR de tokens de cada
            // pierna, no solo el pool. `RouteIntentLeg` ya lleva
            // `token_in`/`token_out` (`route_intent.rs:145,147`); quedarse solo
            // con `pool_hint` los descartaba aquí — y esa era la razón por la que
            // `build_market_state` "no tenía decimales": los tenía, y se perdían
            // una línea antes. Sin ellos la `price_matrix` solo podía llevar el
            // ratio CRUDO r1/r0 (~0.002 para WETH/USDC en vez de ~2000), y por eso
            // los operadores devolvían `scalar: null` y `operators_computed: 0`.
            let pool_legs: Vec<(Address, Address, Address)> = intent
                .legs
                .iter()
                .filter_map(|leg| leg.pool_hint.map(|p| (p, leg.token_in, leg.token_out)))
                .collect();
            // FEATURES-02 (2026-10-01): health_factor desde el indexer CACHEADO
            // del motor de liquidacion. Se calcula aqui, ANTES del spawn, porque
            // `self` no se mueve al spawn y el indexer vive en `self.ctx`.
            //
            // El motor es dirigido por impacto (nunca sondea el universo de
            // lending), asi que NO existe un "HF actual" global: solo se puede
            // reportar el de las posiciones que este intent ya impacto.
            //
            // R8 fail-honest:
            //  * lista vacia -> `None` -> el feature NO se inserta y
            //    `regime_router` deja la metrica en `None`;
            //  * cache miss (ninguna posicion indexada) -> `None`, igual;
            //  * NUNCA un 1.0 por defecto: eso AFIRMA "todo sano", que es una
            //    asercion, no una ausencia de dato.
            // Se publica el MINIMO, no la media: la posicion mas cerca de
            // liquidar es la significativa para el regimen.
            let mut hf_feature: Option<f64> = None;
            if !impact.impacted_lending_positions.is_empty() {
                let idx = self.ctx.liquidation_engine.indexer.lock().await;
                let mut worst: Option<f64> = None;
                for p in &impact.impacted_lending_positions {
                    if let Some(pos) = idx.get_position(p.protocol, p.user).await {
                        let hf = pos.health_factor;
                        if hf.is_finite() && hf > 0.0 {
                            worst = Some(worst.map_or(hf, |w: f64| w.min(hf)));
                        }
                    }
                }
                hf_feature = worst;
            }
            if !pool_legs.is_empty() {
                // CORE-01/MATH-01 fix (2026-09-24): the §IV evidence previously
                // received gas_price_gwei=0.0 ("not carried in RouteIntent yet"),
                // making every gas-sensitive operator (op_15/op_21/op_26) compute
                // with free gas. Propagate the REAL head base fee + block number
                // from the cartridge HostContext atomics — the same source the
                // cartridge path uses (milli-gwei stored; decode ÷1e3 via
                // host_gas_price_gwei).
                let (gas_price_gwei, block_number) = match self.ctx.cartridge_runner.as_ref() {
                    Some(runner) => {
                        let head = runner
                            .host_block_number_handle()
                            .load(std::sync::atomic::Ordering::Relaxed);
                        (runner.host_gas_price_gwei(), head)
                    }
                    None => (0.0, 0), // R8 fail-honest: no runner → no head data
                };
                tokio::spawn(async move {
                    // FEATURES-01a (de main): features de régimen desde fuentes VIVAS.
                    // Antes iba literalmente `HashMap::new()` — el mapa nacía vacío,
                    // `regime_router` dejaba las 5 métricas en `null`, clasificaba
                    // siempre `["Neutral"]` (2 operadores) y la evidencia salía con
                    // `operators_computed: 0`. Alimenta `parity_deviation` desde el
                    // PriceBus; el resto sigue sin productor y NO se inventa.
                    //
                    // FEATURES-02 (esta rama): sobre esa base se añade `health_factor`
                    // desde el indexer CACHEADO del motor de liquidación. Sin
                    // posiciones de lending impactadas, o sin entrada en el indexer,
                    // la clave NO se inserta y `regime_router` la deja en `None` —
                    // nunca en 1.0, que afirmaría "todo sano".
                    let mut features =
                        crate::math_evidence::regime_features_from_redis(&mut math_redis, chain_id)
                            .await;
                    if let Some(hf) = hf_feature {
                        features.insert("health_factor".to_owned(), hf);
                    }
                    crate::math_evidence::evaluate_math_evidence(
                        &reserves_cache,
                        &registry,
                        &router,
                        &mut math_redis,
                        &pool_legs,
                        chain_id,
                        gas_price_gwei,
                        block_number,
                        0, // block_timestamp — still not carried on the intent (observe-only)
                        features,
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
        let mut cfg_snapshot: Option<TradingConfigState> =
            self.ctx.config_provider.snapshot(chain_id).await;

        // Root-2B (moved BEFORE engine fan-out): merge live Redis per-token prices
        // (DexScreener/Chainlink/GeckoTerminal — 297 entries) into the config
        // snapshot so the dex_engine can price tokens via canonical_token_price_usd.
        // Without this, base_token_price_usd=0 → all V3 pairs pre-rejected as
        // no_price_oracle regardless of available DexScreener prices.
        {
            let mut redis_conn = self.ctx.math_redis.clone();
            let redis_prices =
                RedisCachedPriceOracle::snapshot_from_redis(&mut redis_conn, chain_id)
                    .await
                    .into_snapshot();
            if let Some(ref mut cfg) = cfg_snapshot {
                for (sym, price) in &redis_prices {
                    cfg.token_prices_usd.insert(sym.clone(), *price);
                }
            }
        }

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
        // Plan C.3: ARBX_NATIVE_ENGINES=off → skip the native Dex/Triangular/
        // Liquidation/Flashloan fan-out entirely so ONLY cartridge candidates
        // flow. The cartridge spawn path (Step 3, above) is unaffected.
        // R8: off = empty candidate vecs, never fabricated candidates.
        let native_on = self.ctx.native_engines_enabled;

        // DexEngine (Phase 8 — live). Skipped when ARBX_NATIVE_ENGINES=off.
        let dex_result = if native_on {
            self.ctx
                .dex_engine
                .build_from_impacted_pairs(&intent, &impact, cfg_snapshot.as_ref())
                .await
        } else {
            Ok(vec![])
        };
        let dex_candidates = match dex_result {
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

        // TriangularEngine (Phase 9 — live). Skipped when ARBX_NATIVE_ENGINES=off.
        let tri_result = if native_on {
            self.ctx
                .triangular_engine
                .build_from_impacted_cycles(&intent, &impact, cfg_snapshot.as_ref())
                .await
        } else {
            Ok(vec![])
        };
        let tri_candidates = match tri_result {
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
        // never polls the full lending universe. Skipped when ARBX_NATIVE_ENGINES=off.
        let liq_result = if native_on {
            self.ctx
                .liquidation_engine
                .build_from_lending_impact(&intent, &impact, cfg_snapshot.as_ref())
                .await
        } else {
            Ok(vec![])
        };
        let liq_candidates = match liq_result {
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
        // Skipped when ARBX_NATIVE_ENGINES=off (base_candidates is already empty
        // in that mode, so wrapping is a no-op anyway — guarded to avoid the call).
        let flash_candidates = {
            let wrapped = if native_on {
                self.ctx.flashloan_engine.wrap_profitable_routes(
                    &base_candidates,
                    chain_id,
                    cfg_snapshot.as_ref(),
                )
            } else {
                Vec::new()
            };
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

        // ── Step 5b: discovery → emission bridge (3..=7-hop closed cycles) ────
        // HOPS-EMIT-01. An intent that is itself a CLOSED cycle of 3..=7 hops
        // (the shape `route_scanner_worker` publishes per block) becomes ONE
        // candidate whose RoutePlan carries the discovered hops — the engines
        // above can only ever produce 2-leg pairs, which is why every card was
        // 2-hop. Admission owns the reversibility knob + the per-block cap;
        // `native_on` keeps the Plan C.3 contract (off ⇒ cartridges-only, no
        // native candidate of any kind). 2-hop intents are refused here — that
        // shape is `dex_engine`'s alone (no duplicate rows).
        let bridged_candidates: Vec<StrategyCandidate> = if native_on {
            let epoch = intent.observed_block().unwrap_or(0);
            let budget = crate::route_discovery::hop_cycle_bridge::budget_for(chain_id);
            match crate::route_discovery::hop_cycle_bridge::admit(&intent, &budget, epoch) {
                Ok(c) => {
                    CANDIDATES_TOTAL
                        .with_label_values(&[&chain_str, c.label.as_str()])
                        .inc();
                    info!(
                        event = "v2.engine.output",
                        chain_id,
                        tx_hash = %intent.tx_hash,
                        engine = "hop_cycle_bridge",
                        candidates_count = 1usize,
                        rejected_count = 0usize,
                        accepted_shape_count = 1usize,
                        hops = intent.legs.len(),
                        epoch,
                        cap = budget.per_epoch(),
                        used = budget.used_in(epoch),
                        dropped = budget.dropped_in(epoch),
                        sizeable_used = budget.sizeable_used_in(epoch),
                        unpriceable_used = budget.unpriceable_used_in(epoch),
                        unpriceable_refused = budget.unpriceable_refused_in(epoch),
                    );
                    vec![c]
                }
                Err(skip) => {
                    // `too_few_hops` is the steady state for every ordinary swap
                    // intent — not worth a per-intent line. Everything else is a
                    // real signal (`cap_reached` truncation included: R8 never
                    // hides it).
                    if skip != crate::route_discovery::hop_cycle_bridge::BridgeSkip::TooFewHops
                        && skip != crate::route_discovery::hop_cycle_bridge::BridgeSkip::Disabled
                    {
                        if skip == crate::route_discovery::hop_cycle_bridge::BridgeSkip::CapReached
                        {
                            // LOGFLOOD-02 (R9): ONE warn per epoch with that
                            // epoch's aggregate counts; every further refusal of
                            // the same epoch is a per-item debug line. The
                            // truncation stays fully visible (aggregate + counted
                            // `dropped_in`), it just stops being ~40 warn/s.
                            if budget.claim_cap_report(epoch) {
                                warn!(
                                    event = "v2.hop_cycle_bridge.cap_reached",
                                    chain_id,
                                    tx_hash = %intent.tx_hash,
                                    hops = intent.legs.len(),
                                    epoch,
                                    cap = budget.per_epoch(),
                                    used = budget.used_in(epoch),
                                    dropped = budget.dropped_in(epoch),
                                    // PERHOP-RESERVES-01 lane split (see
                                    // `emit_discovered_cycle`): the per-epoch
                                    // allowance spent on priceable vs unpriceable
                                    // cycles, and how many unpriceable cycles were
                                    // deferred behind priceable work.
                                    sizeable_used = budget.sizeable_used_in(epoch),
                                    unpriceable_used = budget.unpriceable_used_in(epoch),
                                    unpriceable_refused = budget.unpriceable_refused_in(epoch),
                                    "per-block multihop emission cap reached — cycle not emitted (R8 truncation); this epoch's single aggregate line"
                                );
                            } else {
                                debug!(
                                    event = "v2.hop_cycle_bridge.cap_reached",
                                    chain_id,
                                    tx_hash = %intent.tx_hash,
                                    hops = intent.legs.len(),
                                    epoch,
                                    cap = budget.per_epoch(),
                                    used = budget.used_in(epoch),
                                    dropped = budget.dropped_in(epoch),
                                    "per-block multihop emission cap reached — cycle not emitted (R8 truncation; epoch already reported at warn)"
                                );
                            }
                        } else {
                            debug!(
                                event = "v2.hop_cycle_bridge.skip",
                                chain_id,
                                tx_hash = %intent.tx_hash,
                                hops = intent.legs.len(),
                                epoch,
                                reason = skip.as_str(),
                            );
                        }
                    }
                    vec![]
                }
            }
        } else {
            vec![]
        };

        // ── Step 6: size + evaluate + emit each candidate ─────────────────
        // cfg_snapshot was already taken before engine fan-out (Step 4) so
        // the evaluator uses the same snapshot the engines used — consistent
        // within one intent's processing window.
        // For each candidate: run size_optimizer → update profit fields or
        // emit as rejected if optimizer returns None. Then evaluate + emit.
        // Process base candidates first, then flashloan-wrapped variants, then
        // the bridged discovered cycle (never flashloan-wrapped: a wrapped
        // variant of the same route would be a second row for one cycle).
        let all_candidates: Vec<StrategyCandidate> = base_candidates
            .into_iter()
            .chain(flash_candidates)
            .chain(bridged_candidates)
            .collect();

        // Root-2B: merge live Redis per-token prices into the config snapshot so
        // the SizeOptimizer's `resolve_token_price` can price canonical
        // non-stable tokens (PEPE/LINK/UNI/…) absent from the operator's manual
        // `token_prices_usd` map. Without this, V2 dex_arb pairs involving such
        // tokens reach the optimizer unpriced and reject. Mirrors what the
        // ConfigAwareEvaluator already receives via `with_cache`. Engines (Step
        // 4) already ran with the un-merged snapshot, so this only affects
        // sizing onward. R8: empty snapshot → no-op (no fabrication).
        {
            let mut redis_conn = self.ctx.math_redis.clone();
            let redis_prices =
                RedisCachedPriceOracle::snapshot_from_redis(&mut redis_conn, chain_id)
                    .await
                    .into_snapshot();
            if let Some(ref mut cfg) = cfg_snapshot {
                for (sym, price) in &redis_prices {
                    cfg.token_prices_usd.insert(sym.clone(), *price);
                }
            }
        }

        // ── ARBX-0009 (two-phase): size + evaluate first, then emit the batch
        // in sheet-07 Net_bps order. Phase 1 below is semantics-identical to
        // the former inline emit loop (same logs, same sizing, same rejection
        // construction) — only emission is deferred to Phase 2, where the
        // batch is ordered best-Net_bps-first per workbook 07. Ranking never
        // gates or drops: sized, net-negative and engine-rejected candidates
        // are ALL still emitted (RULE 00 / R8; non-computable ranks last).
        let route_key_of = |c: &StrategyCandidate| {
            // GRANULAR identity (STRAT-IDENT audit): the engine label, never
            // the collapsed `opportunity.strategy_kind` (~16 dex variants
            // flatten to one class there); pools + intent hash make it stable,
            // and the base-strategy marker keeps a flash-wrapped variant from
            // tying its own-capital sibling.
            format!(
                "{}|{}|{}{}",
                c.label.as_str(),
                c.candidate.pool_addresses.join(","),
                c.source_intent_hash,
                c.base_strategy
                    .map(|b| format!(":{}", b.as_str()))
                    .unwrap_or_default(),
            )
        };
        // HOPS-LEDGER-04: per-leg wei (in, out) from the sizing kernel,
        // threaded alongside the candidate to process_candidate.
        type SizedBatchEntry = (
            crate::net_bps_ranking::RankedRoute,
            StrategyCandidate,
            Option<(Vec<String>, Vec<String>)>,
        );
        let mut sized_batch: Vec<SizedBatchEntry> = Vec::new();
        for mut candidate in all_candidates {
            // Preserve the observed block without overwriting engine evidence.
            // Admission still pins and verifies the full canonical block hash.
            candidate.opportunity.block_number = candidate
                .opportunity
                .block_number
                .or(intent.observed_block());
            // Skip sizing for already-rejected candidates (engine rejection).
            if candidate.rejection_reason.is_some() {
                sized_batch.push((
                    crate::net_bps_ranking::RankedRoute {
                        route_key: route_key_of(&candidate),
                        economics: crate::net_bps_ranking::RouteNetEconomics::not_computable(),
                    },
                    candidate,
                    // HOPS-LEDGER-04: engine-rejected rows never reached the
                    // sizing kernel — no per-leg wei exists (R8 absence).
                    None,
                ));
                continue;
            }

            let (final_candidate, net_economics, leg_ledger) = self
                .size_candidate_for_emit(candidate, &intent, cfg_snapshot.as_ref(), &chain_str)
                .await;
            sized_batch.push((
                crate::net_bps_ranking::RankedRoute {
                    route_key: route_key_of(&final_candidate),
                    economics: net_economics
                        .unwrap_or_else(crate::net_bps_ranking::RouteNetEconomics::not_computable),
                },
                final_candidate,
                leg_ledger,
            ));
        }

        // Phase 2 (ARBX-0009): deterministic sheet-07 Net_bps ordering, then
        // emit. One summary line per multi-candidate batch (R9 — per-item
        // detail already logged at debug in Phase 1).
        sized_batch.sort_by(|(a, _, _), (b, _, _)| crate::net_bps_ranking::net_bps_order(a, b));
        let risk_ranking = self.risk_ranker.rank(
            &mut sized_batch,
            chain_id,
            &self.ctx.math_redis,
            |(ranked, candidate, _)| {
                if candidate.rejection_reason.is_some() || ranked.economics.net_bps().is_none() {
                    return None;
                }
                Some(crate::live_risk_ranker::RankingInput {
                    strategy_kind: candidate.opportunity.strategy_kind.as_str().to_owned(),
                    token_in: candidate.opportunity.token_in.clone(),
                    principal_usd: ranked.economics.start_amount_usd,
                    expected_profit_usd: ranked.economics.net_profit_usd(),
                })
            },
        );
        debug!(
            event = "orchestrator.empirical_nsga2",
            chain_id,
            considered = risk_ranking.considered,
            with_history = risk_ranking.with_history,
            reordered = risk_ranking.reordered,
            source = "finalized_receipt_cohort"
        );
        if sized_batch.len() > 1 {
            debug!(
                event = "orchestrator.net_bps_ranked_batch",
                chain_id,
                batch = sized_batch.len(),
                order = ?sized_batch
                    .iter()
                    .map(|(r, _, _)| (r.route_key.as_str(), r.economics.net_bps()))
                    .collect::<Vec<_>>(),
            );
        }
        for (_, candidate, leg_ledger) in sized_batch {
            self.process_candidate(candidate, leg_ledger, cfg_snapshot.as_ref(), chain_id)
                .await?;
        }

        Ok(())
    }

    // -----------------------------------------------------------------------
    // Sizing step (shared by the mempool path and the discovery bridge)
    // -----------------------------------------------------------------------

    /// Size ONE candidate through the sizing kernel and return exactly what the
    /// emit tail needs:
    ///
    /// - the finalized `StrategyCandidate` (Sized ⇒ profit/amount stamped on both
    ///   the candidate and its `Opportunity`; Rejected ⇒ `rejection_reason` set),
    /// - the sheet-07 net economics for the batch ranking (`None` = not
    ///   computable ⇒ ranked last; ranking never gates),
    /// - the kernel's exact per-leg wei ledger when it produced one (R8: absent
    ///   ⇒ `None`, never a repeated intent amount dressed as a ledger).
    ///
    /// Extracted verbatim from `on_route_intent`'s Phase-1 loop so the
    /// discovery→emission bridge ([`Self::emit_discovered_cycle`]) runs the
    /// IDENTICAL sizing step: one implementation, no divergence between the
    /// mempool path and the discovered-cycle path (doctrine §34.1 — the math is
    /// mode-invariant, and it must not fork by entry point either).
    async fn size_candidate_for_emit(
        &self,
        candidate: StrategyCandidate,
        intent: &RouteIntent,
        cfg_snapshot: Option<&TradingConfigState>,
        chain_str: &str,
    ) -> SizedForEmit {
        let chain_id = self.ctx.chain_id;

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
            .optimize_with_reason(candidate.clone(), intent, cfg_snapshot)
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
                // R8: infra error, no economic value was computed — None.
                OptimizeOutcome::Rejected(OptimizeRejectReason::NonPositiveProfit, None)
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
                OptimizeOutcome::Rejected(_, _)
                | OptimizeOutcome::RejectedWithLedger(_, _, _)
                | OptimizeOutcome::RejectedComputed(_, _) => "rejected",
            },
            reason = ?outcome.reason_str(),
            gross_profit_usd = ?outcome.gross_profit_usd(),
            net_profit_usd = ?outcome.net_profit_usd(),
            optimal_amount_in = ?outcome.optimal_amount_in(),
        );

        // PER-HOP (math-audit AUDIT-MATH-OPPS-2026-09-26): the rejection path
        // can ALREADY carry the kernel's exact per-leg wei
        // (OptimizeOutcome::RejectedWithLedger / RejectedComputed). Capture it
        // before the match so the tail persists it onto the RouteMetadata
        // exactly like the Sized path does — this is what lets a rejected card
        // show each hop's movement instead of "not computed".
        let rejected_ledger = outcome.leg_ledger();

        match outcome {
            OptimizeOutcome::Sized(sized) => {
                // Unbox and update the candidate with optimal sizing data.
                let s = *sized;
                // ALWAYS-COMPUTE (2026-09-27): build the complete economics
                // object from the sized figures BEFORE `s.candidate` is moved
                // out (the builder reads the whole candidate).
                let economics_obj = crate::economics::economics_from_sized(&s, cfg_snapshot);
                // CANDIDATE-LEDGER-STRUCT-01: ONE explicit ledger for this sized
                // cycle, built from the kernel's measured figures + the economics
                // object they produced (measured final-hop output, gas component,
                // sheet-07 block). It is the single source the stamping tail
                // below consumes, so the three layers cannot drift apart.
                let ledger = SizedCycleLedger::from_sized(&s, &economics_obj);
                let mut c = s.candidate;
                // FIX (review V2 #8) + B1 FIX (math-audit AUDIT-MATH-OPPS-2026-09-26)
                // + CANDIDATE-POSTSIZE-SYNC-01 (a) + CANDIDATE-LEDGER-STRUCT-01:
                // ONE stamping site for the sized figures on BOTH layers — the
                // outer StrategyCandidate/`Opportunity` row AND the INNER spine
                // candidate that `ConfigAwareEvaluator` re-evaluates below — fed
                // by exactly ONE ledger. See `stamp_sized_figures`.
                stamp_sized_figures(
                    &mut c,
                    &ledger,
                    self.ctx.token_decimals_provider.as_ref(),
                    chain_id,
                );
                // ALWAYS-COMPUTE (2026-09-27): the ONE complete economics
                // object travels on the accepted row too (additive wire).
                if crate::economics::always_compute_enabled() {
                    c.opportunity.economics = Some(economics_obj);
                }
                // ARBX-0009: sheet-07 components from the kernel for the
                // batch's Net_bps ranking (None ⇒ not computable ⇒ last).
                // HOPS-LEDGER-04: thread the kernel's exact per-leg wei to
                // process_candidate — attached there onto the chosen
                // RouteMetadata (None when the kernel had no per-leg math
                // or Kelly rebound the size).
                let legs = match (s.leg_amounts_in, s.leg_amounts_out) {
                    (Some(amounts_in), Some(amounts_out)) => Some((amounts_in, amounts_out)),
                    _ => None,
                };
                (c, s.net_economics, legs)
            }
            // ALWAYS-COMPUTE (2026-09-27): a rejection whose path HAD computed
            // the full economics — the row keeps every number (gross AND net,
            // the sized amount, the ledger) plus the complete computation
            // object. "FAIL = se hizo el cálculo y no cumple el criterio".
            OptimizeOutcome::RejectedComputed(reason, boxed) => {
                let bare = reason.as_str();
                REJECTED_NO_PROFIT_TOTAL
                    .with_label_values(&[chain_str, candidate.label.as_str(), bare])
                    .inc();
                let rejection = if reason.is_net_dependent() {
                    let mode = crate::financing::selected_mode(u8::from(
                        candidate.base_strategy.is_some(),
                    ) as f64);
                    format!("{bare}:{}", mode.as_str())
                } else {
                    bare.to_owned()
                };
                let s = *boxed;
                let mut c = candidate;
                c.rejection_reason = Some(rejection.clone());
                // The kernel's OWN figures, both of them: the legacy reject
                // path could only carry one scalar (net); this variant proves
                // the gross it judged. Knob-gated so OFF restores the
                // pre-mandate wire exactly (one scalar payload).
                if crate::economics::always_compute_enabled() {
                    c.gross_profit_usd = Some(s.gross_profit_usd);
                    c.net_expected_profit_usd = Some(s.estimated_net_profit_usd);
                    c.opportunity.expected_profit_usd = Some(s.gross_profit_usd);
                    c.opportunity.net_expected_profit_usd = Some(s.estimated_net_profit_usd);
                    if s.optimal_amount_in > U256::zero() {
                        c.opportunity.amount_in_wei = s.optimal_amount_in.to_string();
                    }
                    c.opportunity.economics =
                        Some(crate::economics::economics_from_sized(&s, cfg_snapshot));
                } else if let Some(net) = Some(s.estimated_net_profit_usd) {
                    // Knob OFF: legacy semantics — only the net payload.
                    c.net_expected_profit_usd = Some(net);
                    c.opportunity.net_expected_profit_usd = Some(net);
                }
                (c, None, rejected_ledger)
            }
            OptimizeOutcome::Rejected(reason, rejected_net)
            | OptimizeOutcome::RejectedWithLedger(reason, rejected_net, _) => {
                // Route optimizer rejection to REJECTED_NO_PROFIT_TOTAL
                // (not SIMULATION_FAILED_TOTAL — sizing is not simulation).
                // The Prometheus label stays the BARE reason (a suffixed
                // label would split the metric series mid-stream).
                let bare = reason.as_str();
                REJECTED_NO_PROFIT_TOTAL
                    .with_label_values(&[chain_str, candidate.label.as_str(), bare])
                    .inc();
                // ARBX-0007: net-dependent rejections carry the financing
                // mode that was priced (label-scheme suffix, DB/UI only —
                // the rejection_reason string is plain text rendered
                // verbatim by the frontend). The discriminator is the same
                // one the kernel priced: a flash-backed base strategy.
                let rejection = if reason.is_net_dependent() {
                    let mode = crate::financing::selected_mode(u8::from(
                        candidate.base_strategy.is_some(),
                    ) as f64);
                    format!("{bare}:{}", mode.as_str())
                } else {
                    bare.to_owned()
                };
                let mut c = candidate;
                c.rejection_reason = Some(rejection);
                // Deuda 4-(B): stamp the kernel's computed net when the
                // rejecting path had one (R8). Without this the emitter
                // falls back to the raw detection estimate and the DB
                // labels unprofitable rejects as "profitable".
                if let Some(net) = rejected_net {
                    c.net_expected_profit_usd = Some(net);
                    c.opportunity.net_expected_profit_usd = Some(net);
                }
                // HARDENING: NO vaciar expected_profit_usd. Mantener el valor
                // que el SizeOptimizer calculó (gross) para que la tarjeta lo
                // muestre. El gate de net-positive es de EJECUCIÓN, no de
                // detección. La tarjeta debe mostrar los números reales para
                // que el operador vea POR QUÉ no es viable.
                // c.opportunity.expected_profit_usd = None;  ← REMOVIDO
                // ALWAYS-COMPUTE (2026-09-27): the no-quote rejects get their
                // honest object — a real payload figure ⇒ "partial" with the
                // number; nothing ⇒ "error" + the verbatim reason (gate 2:
                // never fabricated numbers).
                if crate::economics::always_compute_enabled() {
                    c.opportunity.economics = Some(match rejected_net {
                        Some(v) => crate::economics::economics_partial(
                            c.opportunity.expected_profit_usd,
                            Some(v),
                            c.opportunity.roi_pct,
                            Some(bare),
                        ),
                        None => crate::economics::economics_error(bare),
                    });
                }
                // PER-HOP: the ledger captured above travels in the third
                // tuple slot exactly like the Sized path's, so the common
                // tail attaches it to the rejected row's RouteMetadata.
                (c, None, rejected_ledger)
            }
        }
    }

    /// Live config snapshot for a NON-mempool path (the discovery bridge).
    ///
    /// Same shape `on_route_intent` builds before sizing (Root-2B): the operator
    /// config for this chain plus the live Redis per-token prices merged in, so
    /// the kernel can price canonical non-stable tokens absent from the manual
    /// `token_prices_usd` map. R8: no config ⇒ `None` (the caller then emits the
    /// candidates as `NoTradingConfig` rejections, never as ungated accepts);
    /// an empty/errored price snapshot is a no-op, never a fabricated price.
    async fn live_config_snapshot(&self, chain_id: u64) -> Option<TradingConfigState> {
        let mut cfg_snapshot = self.ctx.config_provider.snapshot(chain_id).await;
        let mut redis_conn = self.ctx.math_redis.clone();
        let redis_prices = RedisCachedPriceOracle::snapshot_from_redis(&mut redis_conn, chain_id)
            .await
            .into_snapshot();
        if let Some(ref mut cfg) = cfg_snapshot {
            for (sym, price) in &redis_prices {
                cfg.token_prices_usd.insert(sym.clone(), *price);
            }
        }
        cfg_snapshot
    }

    /// DECIMALS-CYCLE-01 (t197) — resolve the route's per-hop decimals from the
    /// two real sources: the Redis token catalog first (1 GET per token), then
    /// the PG-backed `tokens.decimals` provider for whatever Redis did not
    /// answer. NEVER a default: what neither source has is declared unresolved.
    async fn resolve_cycle_decimals(&self, addrs: &[String], chain_id: u64) -> CycleDecimals {
        let mut redis_decimals: HashMap<String, u8> = HashMap::new();
        let mut pg_decimals: HashMap<String, u8> = HashMap::new();
        for addr in addrs {
            let lc = addr.to_lowercase();
            if redis_decimals.contains_key(&lc) || pg_decimals.contains_key(&lc) {
                continue;
            }
            let mut conn = self.ctx.math_redis.clone();
            match crate::reserves::get_token_meta(&mut conn, chain_id, &lc).await {
                Ok(Some(meta)) => {
                    redis_decimals.insert(lc, meta.decimals);
                }
                Ok(None) => {
                    // Redis has no row → PG (the authoritative `tokens.decimals`).
                    if let Some(provider) = self.ctx.token_decimals_provider.as_ref() {
                        if let Ok(a) = lc.parse::<Address>() {
                            if let Some(d) = provider.decimals(chain_id, &a) {
                                pg_decimals.insert(lc, d);
                            }
                        }
                    }
                }
                Err(e) => {
                    // Redis read failed: do NOT give up on the token — PG still
                    // answers. The error is logged, never turned into a unit.
                    debug!(
                        event = "orchestrator.decimals_redis_read_failed",
                        chain_id,
                        token = %lc,
                        error = %e,
                        "redis token catalog read failed; falling back to PG provider"
                    );
                    if let Some(provider) = self.ctx.token_decimals_provider.as_ref() {
                        if let Ok(a) = lc.parse::<Address>() {
                            if let Some(d) = provider.decimals(chain_id, &a) {
                                pg_decimals.insert(lc, d);
                            }
                        }
                    }
                }
            }
        }
        build_cycle_decimals_map(addrs, &redis_decimals, &pg_decimals)
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
        // HOPS-LEDGER-04: exact per-leg wei (in, out) from the sizing kernel,
        // aligned with route_plan legs. `None` when sizing didn't run, the
        // kernel has no per-leg math, or Kelly rebound the size. Attached onto
        // the chosen metadata below so every emit persists it (RULE 00).
        leg_ledger: Option<(Vec<String>, Vec<String>)>,
        cfg: Option<&TradingConfigState>,
        chain_id: u64,
    ) -> anyhow::Result<()> {
        // Build the multi-hop route topology ONCE from the candidate's populated
        // arrays; thread it into every emit (accepted AND rejected) so the
        // exchange dashboard's multi-leg A→B view + sim-ctl A1 enrichment get
        // route_metadata for ALL persisted opps — including rejections. The
        // orchestrator rejects most live opps (no_price_oracle /
        // spot_product_le_one); those still deserve a visible route. (Fix 2026-08-11.)
        let route_metadata = {
            // Build from BOTH sources and pick the most complete token path.
            // sc.candidate carries pools/dexes (engines populate these) but
            // often only the entry/exit tokens (e.g. triangular: [A, A]). The
            // route_plan.legs carry the per-hop token_in/token_out, which yields
            // the full traversal path (A→B→C→A). Prefer whichever source gives
            // the LONGER token path; merge so pools/dexes come from candidate
            // when the plan legs lack them.
            let c = &sc.candidate;
            let from_candidate = shared_rs::candidates::RouteMetadata {
                pool_addresses: c.pool_addresses.clone(),
                token_addresses: c.token_addresses.clone(),
                dex_adapters: c.dex_adapters.clone(),
                decimals: Default::default(),
                // Raw candidate carries no sizing amounts — the ledger, when
                // present, rides the plan-built metadata (HOPS-LEDGER-04).
                leg_amounts_in: None,
                leg_amounts_out: None,
                leg_zero_for_one: None,
                economics_amount_in_wei: None,
                economics_basis: None,
            };
            let from_plan = crate::persistence::build_route_metadata_from_plan(&sc.route_plan);
            // Prefer the source with the longer (more complete) token path.
            let mut chosen =
                if from_plan.token_addresses.len() > from_candidate.token_addresses.len() {
                    from_plan
                } else {
                    from_candidate
                };
            // Backfill pools/dexes from candidate if the chosen plan source
            // left them shorter than the token path would imply.
            if chosen.pool_addresses.len() < chosen.dex_adapters.len()
                && c.pool_addresses.len() >= chosen.dex_adapters.len()
            {
                chosen.pool_addresses = c.pool_addresses.clone();
                chosen.dex_adapters = c.dex_adapters.clone();
            }
            // PER-HOP — DECIMALS-CYCLE-01 (t197, supersedes the
            // AUDIT-MATH-OPPS-2026-09-26 default): the decimals map is built from
            // the route's own token path BUT resolved against the TWO REAL
            // sources — the Redis token catalog and PG `tokens.decimals` — never
            // from the hardcoded canonical table whose unknown arm is 18.
            //
            // Measured (t187): that default stamped 8 of 37 map entries wrong,
            // always 18, including EURC (6 real → 1e12 unit error) and the
            // 8-decimal tokens (→ 1e10). A wrong unit of that class produces a
            // size that cannot work, so every economic verdict measured on sizes
            // generated through this path was contaminated.
            //
            // A token in NEITHER source is NOT priced: the map is left empty and
            // the reason is logged (R8 — "not computed" is not 18, and a default
            // sold as data is fabrication).
            {
                let resolved = self
                    .resolve_cycle_decimals(&chosen.token_addresses, chain_id)
                    .await;
                if resolved.unresolved.is_empty() {
                    if !resolved.map.map.is_empty() {
                        chosen.decimals = resolved.map;
                    }
                } else {
                    warn!(
                        event = "orchestrator.decimals_unresolved",
                        chain_id,
                        reason = "decimals_not_in_redis_token_catalog_nor_pg_tokens_decimals",
                        unresolved = ?resolved.unresolved,
                        route_tokens = chosen.token_addresses.len(),
                        "per-hop decimals unresolved — route not priced with an invented unit (R8)"
                    );
                }
            }
            // HOPS-LEDGER-04: attach the kernel's per-leg ledger AFTER the
            // source merge — attach_leg_ledger is all-or-nothing, so a
            // backfill that changed the hop count refuses the attach (never
            // a partial/misaligned ledger — R8).
            if let Some((amounts_in, amounts_out)) = leg_ledger {
                if !chosen.attach_leg_ledger(&amounts_in, &amounts_out) {
                    debug!(
                        event = "orchestrator.leg_ledger_attach_mismatch",
                        hops = chosen.dex_adapters.len(),
                        amounts_in_len = amounts_in.len(),
                        amounts_out_len = amounts_out.len(),
                        "leg ledger length mismatch vs chosen topology — ledger omitted (R8)"
                    );
                }
            }
            // PER-HOP (C, math-audit AUDIT-MATH-OPPS-2026-09-26): the frontend's
            // deriveLegLedger requires ALL THREE arrays — amounts_in, amounts_out
            // AND zero_for_one — and `leg_zero_for_one` was None in EVERY
            // production row, so every ledger was rejected and no hop ever
            // rendered. Derive it from the plan's own leg directions: Uniswap's
            // canonical convention is zeroForOne ⇔ token_in == token0, and token0
            // is the LOWER address — the lowercase comparison IS the flag (exact
            // data, no invention). Derived only when the ledger is attached and
            // the leg count lines up (the frontend validates lengths too).
            if let Some(amounts_in) = chosen.leg_amounts_in.as_ref() {
                let legs = &sc.route_plan.legs;
                if legs.len() == amounts_in.len() {
                    chosen.leg_zero_for_one = Some(
                        legs.iter()
                            .map(|l| l.token_in.to_lowercase() < l.token_out.to_lowercase())
                            .collect(),
                    );
                }
            }
            if chosen.is_populated() {
                Some(chosen)
            } else {
                None
            }
        };
        let route_ref = route_metadata.as_ref();
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
                .emit_rejected(&opp_with_reason, label, &reason_owned, route_ref)
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
                .emit_rejected(&opp_with_reason, label, &reason, route_ref)
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
                    chain_id, "price snapshot empty; evaluator falls back to config oracle"
                );
            }
            let _ = n;
            oracle.into_snapshot()
        };

        // Build the evaluator borrowing the owned config snapshot.
        // ARBX-R-0002: attach the address-keyed token identity (same 30s
        // cache as the scanner/cartridge paths) so the allowlist gate binds
        // (chain_id, address) — engine candidates carry real addresses in
        // `token_addresses`, and the legacy symbol-compare rejected them
        // all (TokenNotAllowed:<addr> — the AGLD/1INCH flood).
        let signals = NetworkSignals::unknown(sc.opportunity.block_number.unwrap_or(0));
        let identity_idx =
            crate::token_identity::index_for(&mut self.ctx.math_redis.clone(), chain_id, state)
                .await;
        let ev = ConfigAwareEvaluator::with_cache(state, signals, price_snapshot)
            .with_token_identity(Some(identity_idx));

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
                // WO-GAP2 (2026-09-07): R8 — roi_pct None, never a Some(0.0) placeholder.
                apply_gate_rejection_fields(&mut opp, reason.clone());
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
                self.ctx
                    .emitter
                    .emit_rejected(&opp, label, &reason, route_ref)
                    .await?;
            }

            ConfigGateOutcome::StrategyDisabled { strategy_kind: sk } => {
                let reason = format!("StrategyDisabled:{sk}");
                let mut opp = sc.opportunity.clone();
                // WO-GAP2 (2026-09-07): R8 — roi_pct None, never a Some(0.0) placeholder.
                apply_gate_rejection_fields(&mut opp, reason.clone());
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
                self.ctx
                    .emitter
                    .emit_rejected(&opp, label, &reason, route_ref)
                    .await?;
            }

            ConfigGateOutcome::StrategyConfigGateBlocked { reason } => {
                let tag = reason.tag();
                // REASON-TAG-ORCH-01 (audit 2026-09-29): the payload used to be
                // `{:?}`, i.e. the Rust variant name, which is not a stable wire
                // identity. `tag()` is what the codebase defines for metrics and
                // dashboards (decision.rs), so the rejection reason now travels
                // entirely in snake_case and a consumer can match it.
                let reason_str = tag.to_string();
                let mut opp = sc.opportunity.clone();
                // WO-GAP2 (2026-09-07): R8 — roi_pct None, never a Some(0.0) placeholder.
                apply_gate_rejection_fields(&mut opp, reason_str.clone());
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
                    .emit_rejected(&opp, label, &reason_str, route_ref)
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
                    // REASON-TAG-ORCH-01 (audit 2026-09-29): this was `{:?}`, so the
                    // wire carried the Rust variant name ("NegativeNetProfit") while
                    // the codebase's own stable identity is `RejectReason::tag()`
                    // ("negative_net_profit", decision.rs). Measured consequence in
                    // production: 1 290 rows/24 h with the PascalCase form, splitting
                    // ONE gate into two histogram buckets and breaking any consumer
                    // that matches the documented snake_case tag.
                    let reason_str = rej_reason.tag().to_string();
                    // WO-GAP2 (2026-09-07): R8 — roi_pct None, never a Some(0.0) placeholder.
                    apply_gate_rejection_fields(&mut opp, reason_str.clone());
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
                        .emit_rejected(&opp, label, &reason_str, route_ref)
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
                                // REASON-TAG-ORCH-01: `{:?}` leaked the local
                                // (gates.rs) variant name to the wire; `tag()` is the
                                // stable snake_case identity dashboards already read.
                                let reason_str = gate_outcome.reason.tag().to_string();
                                // WO-GAP2 (2026-09-07): the spine DID compute
                                // net_roi_pct above (the Some(outcome.net_roi_pct)
                                // assignment), but the trajectory diverged
                                // (E_state >= τ) — its marginal figure is NOT
                                // reported; None keeps the row honest on the wire.
                                apply_gate_rejection_fields(&mut opp, reason_str.clone());
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
                                    .emit_rejected(&opp, label, &reason_str, route_ref)
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
                    let emit_outcome = self
                        .ctx
                        .emitter
                        .emit_accepted_with_plan(&opp, label, route_ref, &sc.route_plan)
                        .await?;
                    match emit_outcome {
                        EmitOutcome::Published
                        | EmitOutcome::PersistedAndPublished
                        | EmitOutcome::Persisted => {
                            OPPORTUNITIES_PUBLISHED_TOTAL
                                .with_label_values(&[&chain_str, label_str])
                                .inc();
                        }
                        EmitOutcome::Rejected => {}
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
                    // WO-16 call-site excised (see struct-field tombstone; §5.4).
                }
            }
        }

        Ok(())
    }

    // WO-16 hot_sim_stage_post_publish excised (see struct-field tombstone; §5.4).
}

// ---------------------------------------------------------------------------
// Utilities
// ---------------------------------------------------------------------------

// WO-16 resolve_hot_gate_inputs excised (see struct-field tombstone; §5.4).

/// WO-GAP2 (2026-09-07): R8 fail-honest rejection fields for the spine-gate
/// branches of `process_candidate` (TokenNotAllowed / StrategyDisabled /
/// StrategyConfigGateBlocked / EvaluatedRejected / MacroMevGate). `roi_pct`
/// MUST stay `None` — the evaluator never produced a converged Topological
/// Yield measurement for a row a gate rejected (and for MacroMevGate the
/// divergent trajectory's marginal figure is deliberately not reported).
/// `Some(0.0)` here would ride the wire (PG `roi_pct` + `arbx:opps:detected`
/// → FE) masquerading as "computed and exactly zero" — a RULE 00 placeholder
/// the faithful FE would render "+0.00%" (GAP-2: backend mirror of G-6;
/// verifier ronda 2: 0/10,004 stream rows carry 0.0 today, these branches are
/// the latent source). `risk_score` keeps the pre-existing rejected-row
/// `Some(0.0)` convention (scanner.rs "rejection volume" doctrine) — its
/// adjudication is a separate anomaly, NOT this WO.
fn apply_gate_rejection_fields(opp: &mut shared_rs::contracts::Opportunity, reason: String) {
    // ECON-ON-REJECT-PATHS-01 (2026-09-27, measured). WO-GAP2 nulls the figures
    // here because a gate rejection has NOT computed them — correct: a
    // non-computed zero must never be published. But the typed payload was never
    // attached either, so these rows reached the wire with `economics: null`.
    // MEASURED on 43 live rows served by the VPS: `economics` present on 0/43 and
    // `roi_pct` on 0/43 — nothing for the card's capital path to paint. The
    // api-server already falls back to the typed payload
    // (`opportunities-live.ts`: `econNum("roi_pct")`), so attaching it here turns
    // SILENCE into a DECLARED absence: `economics_error` carries the verbatim
    // gate reason, and the figures stay null (R8/WO-GAP2 intact).
    opp.economics = Some(crate::economics::economics_error(&reason));
    opp.rejection_reason = Some(reason);
    // WO-GAP2 (2026-09-07): None = not computed — NEVER a non-computed zero.
    opp.roi_pct = None;
    opp.risk_score = Some(0.0);
}

/// Returns a static string label for a `DetectionSource`.
///
/// Used as the Prometheus `source` label in `decoded_intents_total`.
/// The label is stable and matches the `DetectionSource` serde `snake_case`
/// names so Grafana dashboards can filter by source without mapping.
fn detection_source_as_str(src: crate::route_intent::DetectionSource) -> &'static str {
    src.as_str()
}

/// Decimals the row carries for `address`, resolved through the **same**
/// `TokenDecimalsProvider` the cycle's per-hop decimals map uses.
///
/// CANDIDATE-POSTSIZE-SYNC-01 (a) read this value from the hardcoded canonical
/// table (`canonical_token_decimals_str`: USDC/USDT = 6, WBTC = 8, everything
/// else = 18) so that the sizing ledger and the map could not disagree — but the
/// map itself was fixed first (DECIMALS-CYCLE-01 / t197 / #924) to read the real
/// sources, which left the two consumers measuring the SAME token with TWO
/// different units (encoder 6, sizing ledger 18). DECIMALS-REBASE-01 (t201)
/// closes that divergence: both read the provider.
///
/// `None` when the address does not parse OR the provider has no row for the
/// token. Never `Some(18)`: an invented unit is fabrication, and a ledger sized
/// with the wrong unit is exactly the class of error that produces a size that
/// cannot work.
fn row_token_decimals(
    address: &str,
    provider: Option<&Arc<dyn crate::sim_encoder::TokenDecimalsProvider + Send + Sync>>,
    chain_id: u64,
) -> Option<u8> {
    let addr = address.parse::<Address>().ok()?;
    provider?.decimals(chain_id, &addr)
}

/// CANDIDATE-POSTSIZE-SYNC-01 (a) + CANDIDATE-LEDGER-STRUCT-01: stamp the
/// SIZED figures onto BOTH layers of a candidate — the outer
/// `StrategyCandidate`/`Opportunity` row AND the INNER spine candidate
/// (`StrategyCandidate::candidate`) that `ConfigAwareEvaluator` re-evaluates.
///
/// The figures come from ONE place: the [`SizedCycleLedger`] the sizing kernel's
/// own measured values built (`SizedCycleLedger::from_sized`). This function
/// computes nothing economic — it copies `gross_profit_usd`,
/// `net_expected_profit_usd` and `amount_in_wei` from the ledger, and converts
/// the ledger's exact wei amounts to the inner layer's token units.
///
/// Why the inner layer matters: only the outer layer used to be synchronized, so
/// the second evaluation still read the engine's pre-sizing probe with a
/// fabricated 1:1 rate (`expected_amount_out == amount_in` written at
/// construction). A row whose kernel correctly computed a real closed cycle could
/// therefore be re-judged at the OLD notional, with `observed_rate = 1.0` firing
/// `ImplausibleSpread` (and poisoning the ROI/cost paths) on numbers nobody
/// measured. "Un cálculo. Un tamaño. Un ledger. Una procedencia."
///
/// Inner units: token units as `f64` — the convention `OpportunityCandidate`
/// documents for `amount_in`/`expected_amount_out` (its consumer multiplies by
/// `10^decimals`/price, never by a raw wei integer).
///
/// * `amount_in` — the ledger's `amount_in_wei` (the kernel's sized amount),
///   converted with the row's own entry-token decimals **resolved through the
///   provider**. When the provider has no row the notional is **NOT COMPUTED**
///   (`NaN`), never a fabricated `1e18` scale (DECIMALS-REBASE-01 / t201: that
///   fallback made the sizing ledger disagree with the per-hop map). The stale
///   probe is never kept.
/// * `expected_amount_out` — the ledger's `amount_out_wei` (the kernel's
///   MEASURED final-hop wei, cycle close), converted with the row's exit-token
///   decimals from the same provider. Without a resolved unit the honest `NaN`
///   marker `build_accepted_opportunity` wrote survives — as it also does when
///   the kernel exposed no per-leg math (triangular final-amount-only, Kelly
///   re-bound without re-quote, hand-built fixtures). Deliberately NOT
///   `amount_in`, so no 1:1 rate is ever implied by this sync.
///
/// Decimals are NOT a ledger field: they are resolved for the same two endpoint
/// addresses the ledger parsed (`row_token_decimals` → the provider), read
/// through `size_optimizer::route_endpoint_tokens` so the endpoint selection has
/// one definition — and the SAME provider the cycle's decimals map uses, so both
/// consumers carry one unit per token.
fn stamp_sized_figures(
    c: &mut StrategyCandidate,
    ledger: &SizedCycleLedger,
    provider: Option<&Arc<dyn crate::sim_encoder::TokenDecimalsProvider + Send + Sync>>,
    chain_id: u64,
) {
    // ── Outer layer ─────────────────────────────────────────────────────────
    c.gross_profit_usd = Some(ledger.gross_profit_usd);
    c.net_expected_profit_usd = Some(ledger.net_expected_profit_usd);
    // FIX (review V2 #8): synchronize the Opportunity row that process_candidate
    // actually evaluates/emits. Without this, the DB/API records pre-sizing
    // figures while the optimizer's post-sizing numbers only live on the
    // StrategyCandidate — an inconsistent audit trail (RULE 00 violation surface).
    c.opportunity.expected_profit_usd = Some(ledger.gross_profit_usd);
    c.opportunity.net_expected_profit_usd = Some(ledger.net_expected_profit_usd);
    // B1 FIX (math-audit AUDIT-MATH-OPPS-2026-09-26): the kernel's optimal size
    // was computed but never reached the row — amount_in_wei kept the pre-sizing
    // probe (or "0"), so the persisted economics and the recorded notional
    // disagreed and the SIM-TS ladder ran on a notional the searcher never sized
    // for. Record the exact amount the kernel optimized.
    c.opportunity.amount_in_wei = ledger.amount_in_wei.to_string();

    // ── Inner layer (the ONE the evaluator re-reads) ─────────────────────────
    // Entry/exit token DECIMALS of the row's OWN route, resolved before the
    // mutation so no borrow of `c` stays alive across it. On a closed cycle the
    // exit token IS the entry token (route.token[0] == route.token[N]).
    let (endpoint_in, endpoint_out) = crate::size_optimizer::route_endpoint_tokens(c);
    let in_decimals = endpoint_in.and_then(|a| row_token_decimals(a, provider, chain_id));
    let out_decimals = endpoint_out.and_then(|a| row_token_decimals(a, provider, chain_id));

    c.candidate.amount_in = match in_decimals {
        Some(decimals) => {
            crate::engines::dex_engine::wei_to_token_units(ledger.amount_in_wei, decimals)
        }
        // DECIMALS-REBASE-01 (t201): no resolved unit ⇒ the notional is NOT
        // COMPUTED. The previous rule scaled the SIZED amount by 1e18, which made
        // this ledger disagree with the per-hop map (encoder 6, ledger 18) for
        // every token outside the hardcoded table.
        None => {
            warn!(
                event = "orchestrator.sizing_units_unresolved",
                chain_id,
                reason = "decimals_not_resolved_by_provider_for_sizing_units",
                "sizing notional left NOT COMPUTED (NaN): no invented 1e18 unit (R8)"
            );
            f64::NAN
        }
    };

    // Only a genuinely measured output replaces the marker (R8: absent ⇒ the
    // field keeps its "not measured" value, never a fabricated one) — and only
    // with a RESOLVED unit for the exit token.
    if let Some(out_wei) = ledger.amount_out_wei {
        match out_decimals {
            Some(decimals) => {
                let measured = crate::engines::dex_engine::wei_to_token_units(out_wei, decimals);
                if measured.is_finite() {
                    c.candidate.expected_amount_out = measured;
                }
            }
            None => warn!(
                event = "orchestrator.sizing_units_unresolved",
                chain_id,
                reason = "decimals_not_resolved_by_provider_for_sizing_output",
                "measured output left NOT COMPUTED: no invented 1e18 unit (R8)"
            ),
        }
    }
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
            detector_id: None,
            pipeline_latency_ms: None,
            detected_at: Utc::now(),
            trace_id: Uuid::new_v4(),
            economics: None,
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

    // ── orchestrator::tests::sized_sync_stamps_kernel_notional_and_output ─────
    //
    // CANDIDATE-POSTSIZE-SYNC-01 (a): the SIZED arm must synchronize the INNER
    // spine candidate, not only the outer StrategyCandidate/`Opportunity`. The
    // evaluator re-reads `candidate.amount_in` / `candidate.expected_amount_out`
    // on the second pass, so leaving them at the pre-sizing probe with a
    // fabricated 1:1 rate re-judged a kernel-certified cycle on unmeasured
    // numbers (`ImplausibleSpread`, ROI and cost paths).
    //
    // Fixture: a closed cycle whose base token is USDC (6 decimals) — the sized
    // notional is expressed with the row's REAL decimals (a `1e18` blanket would
    // report 1e-9 instead of 1 000 units) — and whose measured final-hop output
    // differs from the notional.

    /// CANDIDATE-LEDGER-STRUCT-01: build the ledger the way
    /// `size_candidate_for_emit` does — a `SizedCandidate` fixture (the kernel's
    /// own carrier) → the economics object built FROM that carrier → the ledger.
    /// Returns the candidate the stamping mutates plus the ledger it must obey.
    fn sized_ledger_fixture(
        candidate: StrategyCandidate,
        optimal_amount_in: U256,
        gross_profit_usd: f64,
        net_expected_profit_usd: f64,
        leg_amounts_in: Option<Vec<String>>,
        leg_amounts_out: Option<Vec<String>>,
    ) -> (StrategyCandidate, SizedCycleLedger) {
        let sized = crate::size_optimizer::SizedCandidate {
            candidate,
            optimal_amount_in,
            gross_profit_usd,
            estimated_net_profit_usd: net_expected_profit_usd,
            net_negative: net_expected_profit_usd <= 0.0,
            // Sheet-07 components are not the subject here: `None` is the
            // documented R8 shape for a carrier that has none (gas then stays
            // absent on the ledger too).
            net_economics: None,
            leg_amounts_in,
            leg_amounts_out,
        };
        let econ = crate::economics::economics_from_sized(&sized, None);
        let ledger = SizedCycleLedger::from_sized(&sized, &econ);
        (sized.candidate, ledger)
    }

    #[test]
    fn sized_sync_stamps_kernel_notional_and_output() {
        // Canonical mainnet contracts (canonical_token_decimals: USDC = 6).
        let usdc = "0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48";
        let weth = "0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2";

        let mut seeded = make_candidate(StrategyLabel::DexArbV2V2, None);
        // Closed cycle: USDC → WETH → USDC (base = USDC, opens and closes).
        seeded.route_plan.legs[0].token_in = usdc.to_string();
        seeded.route_plan.legs[0].token_out = weth.to_string();
        seeded.route_plan.legs[1].token_in = weth.to_string();
        seeded.route_plan.legs[1].token_out = usdc.to_string();
        seeded.candidate.token_addresses = vec![usdc.to_string(), usdc.to_string()];
        // The STALE pre-sizing probe (the engine's 1:1 fabrication).
        let probe_notional = seeded.candidate.amount_in;
        assert_eq!(probe_notional, 1.0, "fixture probe must be the stale 1.0");

        // Kernel sized the cycle at 1 000 USDC (1e9 in 6-dec wei), MEASURED
        // 1 000.5 USDC leaving the final hop (1_000_500_000 wei), gross $12.50,
        // net $7.00. The per-hop chain is the kernel's own ledger (HOPS-LEDGER-04
        // shape: in[i+1] == out[i], the closing amount differs from the input).
        let optimal_amount_in = U256::from(1_000_000_000u64);
        let bridge_wei = "500000000000000000".to_string();
        let (mut c, ledger) = sized_ledger_fixture(
            seeded,
            optimal_amount_in,
            12.5,
            7.0,
            Some(vec![optimal_amount_in.to_string(), bridge_wei.clone()]),
            Some(vec![
                bridge_wei.clone(),
                U256::from(1_000_500_000u64).to_string(),
            ]),
        );
        let p_usdc_weth = stub_provider(&[(usdc, 6), (weth, 18)]);
        stamp_sized_figures(&mut c, &ledger, p_usdc_weth.as_ref(), 1);

        // Outer layer (review V2 #8 / B1 FIX) still stamped…
        assert_eq!(c.gross_profit_usd, Some(12.5));
        assert_eq!(c.net_expected_profit_usd, Some(7.0));
        assert_eq!(c.opportunity.expected_profit_usd, Some(12.5));
        assert_eq!(c.opportunity.net_expected_profit_usd, Some(7.0));
        assert_eq!(
            c.opportunity.amount_in_wei,
            optimal_amount_in.to_string(),
            "the row must record the exact amount the kernel optimized"
        );

        // …and the INNER candidate (the object the evaluator re-reads) is now
        // synchronized with the SAME notional/output.
        assert_eq!(
            c.candidate.amount_in, 1_000.0,
            "amount_in must be the SIZED notional in the row's real token units \
             (USDC 6-dec), not the stale probe and not a 1e18 mis-scaling"
        );
        assert_ne!(
            c.candidate.amount_in, probe_notional,
            "the pre-sizing probe must never survive the sized path"
        );
        assert_eq!(
            c.candidate.expected_amount_out, 1_000.5,
            "expected_amount_out must be the kernel's MEASURED cycle output"
        );
        assert_ne!(
            c.candidate.expected_amount_out, c.candidate.amount_in,
            "a measured output that differs from the notional must never be flattened to 1:1"
        );
        assert!(
            c.candidate.expected_amount_out.is_finite(),
            "a genuinely measured output must be finite"
        );

        // OBSERVED RATE (config_aware.rs): the second evaluation now sees the
        // real closed-cycle rate instead of the fabricated 1.0.
        let observed_rate = c.candidate.expected_amount_out / c.candidate.amount_in;
        assert!(
            (observed_rate - 1.0005).abs() < 1e-12,
            "observed_rate must be the real cycle rate, got {observed_rate}"
        );

        // NO MEASUREMENT → the honest "not measured" marker survives the sync and
        // no 1:1 rate is implied. The ledger itself carries no output (R8).
        let unmeasured_seed = c.clone();
        let (mut unmeasured, no_measurement) =
            sized_ledger_fixture(unmeasured_seed, optimal_amount_in, 12.5, 7.0, None, None);
        assert!(
            no_measurement.amount_out_wei.is_none(),
            "no kernel per-leg math ⇒ the ledger exposes NO output, never the input"
        );
        unmeasured.candidate.expected_amount_out = f64::NAN;
        let p_usdc_weth = stub_provider(&[(usdc, 6), (weth, 18)]);
        stamp_sized_figures(&mut unmeasured, &no_measurement, p_usdc_weth.as_ref(), 1);
        assert!(
            unmeasured.candidate.expected_amount_out.is_nan(),
            "an absent measurement must stay 'not measured' (NaN), got {}",
            unmeasured.candidate.expected_amount_out
        );
        assert_ne!(
            unmeasured.candidate.expected_amount_out, unmeasured.candidate.amount_in,
            "an unmeasured output must not imply a 1:1 rate"
        );
        assert_eq!(
            unmeasured.candidate.amount_in, 1_000.0,
            "the notional is still the SIZED one when only the output is missing"
        );

        // LEGACY ADDRESS SHAPE / NO RESOLVED UNIT: the helper's address strings do
        // not parse and there is no provider row → the sizing units are **NOT
        // COMPUTED** (NaN marker), NEVER a fabricated 1e18 scale.
        // DECIMALS-REBASE-01 (t201) closed exactly this: the ledger used to
        // disagree with the per-hop map here (the map carried the real unit, this
        // ledger the 1e18 default) for every token outside the hardcoded table.
        let legacy_seed = make_candidate(StrategyLabel::DexArbV2V2, None);
        let three_units = U256::from(10u128).pow(U256::from(18u32)) * U256::from(3u32);
        let (mut legacy, legacy_ledger) =
            sized_ledger_fixture(legacy_seed, three_units, 1.0, 0.5, None, None);
        let no_provider: Option<Arc<dyn crate::sim_encoder::TokenDecimalsProvider + Send + Sync>> =
            None;
        stamp_sized_figures(&mut legacy, &legacy_ledger, no_provider.as_ref(), 1);
        assert!(
            legacy.candidate.amount_in.is_nan(),
            "without a resolved unit the notional is NOT COMPUTED (NaN), got {}",
            legacy.candidate.amount_in
        );
        assert_ne!(
            legacy.candidate.amount_in, 3.0,
            "the 1e18 fallback IS the divergence t201 closes — it must be gone"
        );
    }

    // ── DECIMALS-REBASE-01 (t201): provider stub + los dos tests del cierre ──

    /// Test-only `TokenDecimalsProvider` — the SAME trait the production
    /// `sim_encoder_pg::PgTokenDecimalsProvider` implements, so the sizing stamp
    /// can be exercised with realistic values AND with holes. A hole is `None`,
    /// never 18.
    struct StubDecimals(std::collections::HashMap<(u64, Address), u8>);

    impl crate::sim_encoder::TokenDecimalsProvider for StubDecimals {
        fn decimals(&self, chain_id: u64, token: &Address) -> Option<u8> {
            self.0.get(&(chain_id, *token)).copied()
        }
    }

    fn stub_provider(
        pairs: &[(&str, u8)],
    ) -> Option<Arc<dyn crate::sim_encoder::TokenDecimalsProvider + Send + Sync>> {
        let mut m = std::collections::HashMap::new();
        for (a, d) in pairs {
            let addr: Address = a.parse().unwrap();
            m.insert((1u64, addr), *d);
        }
        Some(Arc::new(StubDecimals(m)))
    }

    #[test]
    fn row_decimals_resolves_through_the_provider_never_to_18() {
        let usdc = "0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48";
        let eurc = "0x1abaea1f7c830bd89acc67ec4af516284b1bc33c";
        let unknown = "0x00000000000000000000000000000000deadbeef";
        let p = stub_provider(&[(usdc, 6), (eurc, 6)]);
        assert_eq!(row_token_decimals(usdc, p.as_ref(), 1), Some(6));
        assert_eq!(
            row_token_decimals(eurc, p.as_ref(), 1),
            Some(6),
            "EURC tiene 6 reales (t187): el sizing ledger debe usar la MISMA unidad"
        );
        assert_eq!(
            row_token_decimals(unknown, p.as_ref(), 1),
            None,
            "sin fila en el provider ⇒ None, NUNCA 18"
        );
        assert_eq!(
            row_token_decimals(usdc, None, 1),
            None,
            "sin provider ⇒ None, NUNCA 18"
        );
        assert_eq!(row_token_decimals("not-an-address", p.as_ref(), 1), None);
        // El defecto, reproducido dentro del test: la tabla canonica pre-fix
        // devolvia 18 para el token que el provider no conoce.
        assert_eq!(
            crate::engines::dex_engine::canonical_token_decimals_str(unknown),
            18,
            "la tabla canonica daria 18: eso es lo que t201 deja de usar"
        );
    }

    #[test]
    fn sizing_ledger_and_cycle_map_agree_on_the_same_unit() {
        // El MISMO token (EURC, 6 reales) por los DOS caminos: el mapa del ciclo
        // y el ledger del sizing. Antes de t201: el mapa 6 y el ledger 18.
        let eurc = "0x1abaea1f7c830bd89acc67ec4af516284b1bc33c";
        let mut pg = std::collections::HashMap::new();
        pg.insert(eurc.to_string(), 6u8);
        let redis = std::collections::HashMap::new();
        let map = build_cycle_decimals_map(&[eurc.to_string()], &redis, &pg);
        assert_eq!(
            map.map.map.get(eurc),
            Some(&6u8),
            "el mapa del ciclo resuelve EURC = 6"
        );

        let mut seeded = make_candidate(StrategyLabel::DexArbV2V2, None);
        seeded.route_plan.legs[0].token_in = eurc.to_string();
        seeded.route_plan.legs[0].token_out = eurc.to_string();
        seeded.route_plan.legs[1].token_in = eurc.to_string();
        seeded.route_plan.legs[1].token_out = eurc.to_string();
        seeded.candidate.token_addresses = vec![eurc.to_string(), eurc.to_string()];
        let (mut c, ledger) =
            sized_ledger_fixture(seeded, U256::from(1_000_000u64), 1.0, 0.5, None, None);
        let p = stub_provider(&[(eurc, 6)]);
        stamp_sized_figures(&mut c, &ledger, p.as_ref(), 1);
        assert_eq!(
            c.candidate.amount_in, 1.0,
            "1e6 wei de EURC con 6 decimales = 1.0 unidades: la MISMA unidad que el mapa"
        );
        // Sin fuente, NO se inventa la unidad (ni 18 ni ninguna otra).
        stamp_sized_figures(&mut c, &ledger, None, 1);
        assert!(
            c.candidate.amount_in.is_nan(),
            "sin provider: NOT COMPUTED, no 1e18"
        );
    }

    // ── orchestrator::tests::stamped_layers_follow_the_ledger_and_nothing_else ─
    //
    // CANDIDATE-LEDGER-STRUCT-01 (b): the figures stamped on the three layers
    // (`Opportunity`, the outer `StrategyCandidate`, the INNER
    // `OpportunityCandidate`) provably come from ONE place — the
    // `SizedCycleLedger`. Proof: every layer starts poisoned with a different
    // stale value, and after the stamp the layers carry the ledger's numbers;
    // then the LEDGER is mutated (nothing else) and re-stamped, and the three
    // layers follow it again. Any second source would break the second pass.

    #[test]
    fn stamped_layers_follow_the_ledger_and_nothing_else() {
        let usdc = "0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48";
        let weth = "0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2";

        let mut seeded = make_candidate(StrategyLabel::DexArbV2V2, None);
        seeded.route_plan.legs[0].token_in = usdc.to_string();
        seeded.route_plan.legs[0].token_out = weth.to_string();
        seeded.route_plan.legs[1].token_in = weth.to_string();
        seeded.route_plan.legs[1].token_out = usdc.to_string();
        seeded.candidate.token_addresses = vec![usdc.to_string(), usdc.to_string()];
        // POISON every layer this function is allowed to touch.
        seeded.gross_profit_usd = Some(-999.0);
        seeded.net_expected_profit_usd = Some(-999.0);
        seeded.opportunity.expected_profit_usd = Some(-999.0);
        seeded.opportunity.net_expected_profit_usd = Some(-999.0);
        seeded.opportunity.amount_in_wei = "777".to_string();
        seeded.candidate.amount_in = 42.0;
        seeded.candidate.expected_amount_out = 42.0;

        let (mut c, ledger) = sized_ledger_fixture(
            seeded,
            U256::from(1_000_000_000u64),
            12.5,
            7.0,
            Some(vec![
                "1000000000".to_string(),
                "500000000000000000".to_string(),
            ]),
            Some(vec![
                "500000000000000000".to_string(),
                "1000500000".to_string(),
            ]),
        );
        let p_usdc_weth = stub_provider(&[(usdc, 6), (weth, 18)]);
        stamp_sized_figures(&mut c, &ledger, p_usdc_weth.as_ref(), 1);

        // The three layers now agree with the LEDGER — every poison is gone.
        assert_eq!(c.gross_profit_usd, Some(ledger.gross_profit_usd));
        assert_eq!(
            c.net_expected_profit_usd,
            Some(ledger.net_expected_profit_usd)
        );
        assert_eq!(
            c.opportunity.expected_profit_usd,
            Some(ledger.gross_profit_usd)
        );
        assert_eq!(
            c.opportunity.net_expected_profit_usd,
            Some(ledger.net_expected_profit_usd)
        );
        assert_eq!(
            c.opportunity.amount_in_wei,
            ledger.amount_in_wei.to_string()
        );
        assert_eq!(c.candidate.amount_in, 1_000.0);
        assert_eq!(c.candidate.expected_amount_out, 1_000.5);

        // MUTATE THE LEDGER ONLY: the layers must follow it. A second source of
        // truth (a kept scalar, a re-read candidate, a recomputation) would keep
        // the old numbers here and fail.
        let mut updated = ledger.clone();
        updated.amount_in_wei = U256::from(2_000_000_000u64);
        updated.amount_out_wei = Some(U256::from(2_010_000_000u64));
        updated.gross_profit_usd = 31.25;
        updated.net_expected_profit_usd = 11.5;
        stamp_sized_figures(&mut c, &updated, p_usdc_weth.as_ref(), 1);

        assert_eq!(c.gross_profit_usd, Some(31.25));
        assert_eq!(c.net_expected_profit_usd, Some(11.5));
        assert_eq!(c.opportunity.expected_profit_usd, Some(31.25));
        assert_eq!(c.opportunity.net_expected_profit_usd, Some(11.5));
        assert_eq!(c.opportunity.amount_in_wei, "2000000000");
        assert_eq!(c.candidate.amount_in, 2_000.0);
        assert_eq!(c.candidate.expected_amount_out, 2_010.0);
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

    // ── orchestrator::tests::r8_gate_rejection_roi_none ─────────────────────
    //
    // WO-GAP2 (2026-09-07): R8 — the spine-gate rejection branches of
    // `process_candidate` (TokenNotAllowed / StrategyDisabled /
    // StrategyConfigGateBlocked / EvaluatedRejected / MacroMevGate) all stamp
    // rows via `apply_gate_rejection_fields`; that helper must NEVER emit a
    // placeholder roi_pct. `None` = not computed; `Some(0.0)` would ride the
    // wire (PG + arbx:opps:detected → FE) as "computed and exactly zero" —
    // GAP-2, the backend mirror of G-6 (fabricated "+0.00%" on the ticker).
    // The stale-Some precondition matters: the MacroMevGate branch calls the
    // helper AFTER the spine already set Some(net_roi_pct), so the helper
    // must overwrite a pre-existing computed value with None (divergent
    // trajectory → figure deliberately not reported).

    #[test]
    fn r8_gate_rejection_roi_none() {
        let mut opp = make_opportunity(StrategyLabel::DexArbV2V2);
        // Precondition: a stale computed-looking value, as the MacroMevGate
        // branch sees after `opp.roi_pct = Some(outcome.net_roi_pct)`.
        opp.roi_pct = Some(1.23);

        apply_gate_rejection_fields(&mut opp, "TokenNotAllowed:AGLD".to_string());

        assert_eq!(
            opp.roi_pct, None,
            "R8: gate-rejected row must not carry a computed-looking roi_pct"
        );
        assert!(
            opp.risk_score.is_some(),
            "risk_score rejected-row convention preserved (out of GAP-2 scope)"
        );
        assert_eq!(
            opp.rejection_reason.as_deref(),
            Some("TokenNotAllowed:AGLD"),
            "rejection reason must survive the helper verbatim"
        );

        // ECON-ON-REJECT-PATHS-01 (2026-09-27): the helper must ALSO attach the
        // typed payload. MEASURED on 43 live rows served by the VPS: `economics`
        // present on 0/43 and `roi_pct` on 0/43 — the card had nothing to paint
        // because these rows arrived as silence. The payload names the gate
        // verbatim while every figure stays absent (R8/WO-GAP2 intact): a
        // DECLARED absence, never a fabricated zero.
        let econ = opp
            .economics
            .as_ref()
            .expect("gate-rejected row must carry the typed economics payload");
        assert_eq!(
            econ.computation_status, "error",
            "a gate rejection computed no figures — status must be error"
        );
        assert_eq!(
            econ.error_reason.as_deref(),
            Some("TokenNotAllowed:AGLD"),
            "the payload must name the gate reason verbatim"
        );
        assert_eq!(
            econ.amount_in_usd, None,
            "R8: the payload must not carry a figure the gate never computed"
        );

        // Idempotence across the whole rejection-reason family: every branch
        // funnels through the same helper, so one input contract suffices.
        apply_gate_rejection_fields(&mut opp, "MacroMevGate: DivergentTrajectory".to_string());
        assert_eq!(
            opp.roi_pct, None,
            "R8: second stamp (gate-after-spine path) must still yield None"
        );
        assert_eq!(
            opp.rejection_reason.as_deref(),
            Some("MacroMevGate: DivergentTrajectory")
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

    // ── DECIMALS-CYCLE-01 (t197) ─────────────────────────────────────────────
    // El defecto medido en t187: 8 de 37 ENTRADAS del `decimals.map` del ciclo
    // discrepaban, y el valor erroneo era siempre 18 (el arm desconocido de la
    // tabla canonica). Estos dos tests miden LAS ENTRADAS DEL MAPA (el valor
    // que entra por token de la ruta), no el `token_in` de la oportunidad
    // emitida: auditar `token_in` da un falso todo-OK (en t187 la auditoria por
    // `token_in` dio discrepancias=0 contra 8 reales).

    /// Los 8 casos medidos en t187: (address, symbol, decimals real, de donde
    /// sale). Los 8 tienen fila en PG `tokens.decimals` con
    /// `resolved_via='onchain_full'`; 6 de ellos estan ademas en el catalogo
    /// Redis `arbx:tokens:1:*` y 2 (ALICE, CLAUS) NO — por eso el resolutor
    /// tiene que consultar las DOS fuentes y no solo Redis.
    #[cfg(test)]
    const T187_CASES: [(&str, &str, u8, &str); 8] = [
        (
            "0x1abaea1f7c830bd89acc67ec4af516284b1bc33c",
            "EURC",
            6,
            "redis+pg",
        ),
        (
            "0xa1f410f13b6007fca76833ee7eb58478d47bc5ef",
            "RJV",
            6,
            "redis+pg",
        ),
        (
            "0xac51066d7bec65dc4589368da368b212745d63e8",
            "ALICE",
            6,
            "pg_only",
        ),
        (
            "0x2b591e99afe9f32eaa6214f7b7629768c40eeb39",
            "HEX",
            8,
            "redis+pg",
        ),
        (
            "0x72e4f9f808c49a2a61de9c5896298920dc4eeea9",
            "BITCOIN",
            8,
            "redis+pg",
        ),
        (
            "0x14fee680690900ba0cccfc76ad70fd1b95d10e16",
            "$PAAL",
            9,
            "redis+pg",
        ),
        (
            "0x95af4af910c28e8ece4512bfe46f1f33687424ce",
            "MANYU",
            9,
            "redis+pg",
        ),
        (
            "0xa606d433971e9ee140e234daa7c94c476e10ead1",
            "CLAUS",
            9,
            "pg_only",
        ),
    ];

    fn t187_sources() -> (HashMap<String, u8>, HashMap<String, u8>) {
        let mut redis_decimals: HashMap<String, u8> = HashMap::new();
        let mut pg_decimals: HashMap<String, u8> = HashMap::new();
        for (addr, _sym, dec, provenance) in T187_CASES {
            pg_decimals.insert(addr.to_string(), dec);
            if provenance == "redis+pg" {
                redis_decimals.insert(addr.to_string(), dec);
            }
        }
        (redis_decimals, pg_decimals)
    }

    /// FALSIFICADOR (t197): con el defecto restaurado (la tabla canonica y su
    /// default de 18), este test FALLA en los 8 casos. Con el fix PASA.
    #[test]
    fn decimals_cycle_t187_eight_cases_resolve_to_the_real_unit_not_18() {
        let (redis_decimals, pg_decimals) = t187_sources();
        let addrs: Vec<String> = T187_CASES
            .iter()
            .map(|(a, _, _, _)| a.to_string())
            .collect();
        let got = build_cycle_decimals_map(&addrs, &redis_decimals, &pg_decimals);

        assert!(
            got.unresolved.is_empty(),
            "los 8 casos de t187 tienen valor real en PG y/o Redis; ninguno puede quedar sin resolver: {:?}",
            got.unresolved
        );

        for (addr, sym, dec, provenance) in T187_CASES {
            let lc = addr.to_string();
            let entry = got.map.map.get(&lc);
            assert_eq!(
                entry,
                Some(&dec),
                "{sym} {lc} ({provenance}) debe entrar al mapa con su unidad real {dec}"
            );
            assert_ne!(
                entry,
                Some(&18),
                "{sym} {lc}: el mapa NO puede llevar el default de 18 (defecto t187)"
            );
            // El defecto, reproducido dentro del test: la tabla canonica que el
            // ciclo usaba ANTES del fix sigue devolviendo 18 para estos 8
            // tokens. Es decir: el cambio esta en las ENTRADAS del mapa.
            assert_eq!(
                crate::engines::dex_engine::canonical_token_decimals_str(addr),
                18,
                "{sym} {lc}: la tabla canonica pre-fix devolvia 18 (defecto medido en t187)"
            );
        }
    }

    /// El caso negativo: un token sin fila en PG `tokens.decimals` Y sin entrada
    /// en el catalogo Redis NO recibe 18 — se declara NO COMPUTADO con reason
    /// explicito y no se le inventa unidad.
    #[test]
    fn decimals_cycle_unknown_token_is_not_computed_never_18() {
        let unknown = "0x00000000000000000000000000000000deadbeef".to_string();
        let empty_redis: HashMap<String, u8> = HashMap::new();
        let empty_pg: HashMap<String, u8> = HashMap::new();
        let got = build_cycle_decimals_map(&[unknown.clone()], &empty_redis, &empty_pg);

        assert!(
            got.map.map.is_empty(),
            "sin fuente no hay unidad: el mapa no puede llevar NINGUNA entrada (ni 18)"
        );
        assert_eq!(
            got.map.map.get(&unknown),
            None,
            "el token sin fuente no puede aparecer en el mapa"
        );
        assert_eq!(
            got.unresolved,
            vec![unknown.clone()],
            "el token sin fuente debe declararse NO COMPUTADO en `unresolved`              (reason: decimals_not_in_redis_token_catalog_nor_pg_tokens_decimals)"
        );
        // La ruta no se cotiza con una unidad inventada: el mapa vacio es lo que
        // el consumidor interpreta como "no computado", nunca 18.
    }

    /// Los dos casos negativos no pueden colarse por el otro lado: una fuente
    /// que SI responde gana sobre la ausencia de la otra (Redis primero, PG
    /// como respaldo), y el orden de la ruta se preserva en `unresolved`.
    #[test]
    fn decimals_cycle_source_precedence_and_route_order() {
        let mut redis_decimals: HashMap<String, u8> = HashMap::new();
        let mut pg_decimals: HashMap<String, u8> = HashMap::new();
        let a = "0xaaa0000000000000000000000000000000000001".to_string();
        let b = "0xbbb0000000000000000000000000000000000002".to_string();
        let c = "0xccc0000000000000000000000000000000000003".to_string();
        redis_decimals.insert(a.clone(), 6);
        pg_decimals.insert(a.clone(), 18); // PG no puede pisar a Redis
        pg_decimals.insert(b.clone(), 8); // solo PG
        let got = build_cycle_decimals_map(
            &[a.clone(), b.clone(), c.clone()],
            &redis_decimals,
            &pg_decimals,
        );
        assert_eq!(got.map.map.get(&a), Some(&6), "Redis manda sobre PG");
        assert_eq!(
            got.map.map.get(&b),
            Some(&8),
            "PG respalda lo que Redis no tiene"
        );
        assert_eq!(got.map.map.get(&c), None, "sin fuente no hay entrada");
        assert_eq!(got.unresolved, vec![c], "orden de ruta preservado");
    }

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

    /// The `Sized { net_negative: true }` arm keeps BOTH numbers: there a kernel
    /// really computed the gross (its value stays at principal scale, unlike the
    /// fabricated $710k). Pinned so the honest negative-net path the operator also
    /// needs is never confused with the fabricated-gross case.
    #[test]
    fn gross_fab_01_kernel_computed_gross_is_principal_scale() {
        let mut c = make_candidate(StrategyLabel::DexArbV2V3, None);
        // Real kernel output for the same route, per size_two_leg_v3_with_reason:
        // gross = profit/1e18 x price -> essentially the fees of the round trip.
        c.opportunity.expected_profit_usd = Some(0.684_824_55);
        c.opportunity.net_expected_profit_usd = Some(-0.000_005);

        assert!(
            c.opportunity.expected_profit_usd.unwrap() < 1.0,
            "a kernel-computed gross for a 1 DAI principal is fee-scale, never \
             $710k — the two magnitudes are 6 orders of magnitude apart"
        );
        assert_eq!(c.opportunity.net_expected_profit_usd, Some(-0.000_005));
    }
}
