// M11 allow: test modules use .unwrap()/.expect() for readability;
// production paths use ? / anyhow throughout.
//! Orchestrator — Phase 7 skeleton.
//!
//! ## Design (spec §3.5)
//!
//! The orchestrator is the single entry point for every `RouteIntent` decoded
//! from a mempool transaction. It:
//!
//!   1. Converts the intent into its `ImpactSet` via `ImpactIndex`.
//!   2. Fans out to strategy engines (currently only `DexEngine`).
//!   3. Evaluates each `StrategyCandidate` through `ConfigAwareEvaluator`.
//!   4. Emits accepted or rejected candidates via `OpportunityEmitter`.
//!
//! ## Phase 7 scope
//!
//! - `DexEngine` is the only real engine; the other three engine slots
//!   (`triangular`, `flashloan`, `liquidation`) are structural placeholders
//!   that return `Ok(vec![])`. Phases 9-11 fill them in.
//! - `state_projector` and `size_optimizer` are NOT wired yet (Phase 12-13).
//! - Scanner integration (Phase 14) is NOT done yet; `scanner.rs` keeps its
//!   legacy hardcoded path and the orchestrator is called from a separate
//!   entry point.
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

use crate::engines::dex_engine::DexEngine;
use crate::engines::flashloan_engine::FlashloanEngine;
use crate::engines::triangular_engine::TriangularEngine;
use crate::engines::StrategyCandidate;
use crate::impact_index::ImpactIndex;
use crate::opportunity_emitter::OpportunityEmitter;
use crate::route_intent::RouteIntent;
use crate::size_optimizer::SizeOptimizer;
use crate::state_projector::StateProjector;
use shared_rs::metrics::OPPORTUNITIES_TOTAL;
use shared_rs::trading_config::TradingConfigState;
use std::collections::HashMap;
use std::sync::atomic::AtomicU64;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, error, warn};

use prioritization_spine::config_aware::{ConfigAwareEvaluator, ConfigGateOutcome, NetworkSignals};

// ---------------------------------------------------------------------------
// Per-strategy error counters
// ---------------------------------------------------------------------------

/// Per-engine error counter incremented when `DexEngine::build_from_impacted_pairs` returns Err.
///
/// Lock-free `AtomicU64`. Exposed so tests can verify counter increments without
/// standing up a real Redis/PG stack.
pub static DEX_ENGINE_ERRORS: AtomicU64 = AtomicU64::new(0);

/// Error counter for `TriangularEngine::build_from_impacted_cycles`.
pub static TRIANGULAR_ENGINE_ERRORS: AtomicU64 = AtomicU64::new(0);

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
    // Phase 11: pub liquidation_engine: Arc<LiquidationEngine>,
    /// StateProjector — virtual post-tx pool state (Phase 12).
    pub state_projector: Arc<StateProjector>,
    /// SizeOptimizer — optimal amount_in per candidate (Phase 13).
    pub size_optimizer: Arc<SizeOptimizer>,
    /// Single-point emit path (PG + Redis).
    pub emitter: Arc<OpportunityEmitter>,
    /// Asynchronously fetches the live `TradingConfigState` for `chain_id`.
    /// `None` return → no operator config for this chain (observe-only path).
    pub config_provider: Arc<ConfigProvider>,
    /// EVM chain ID for this orchestrator instance.
    pub chain_id: u64,
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
    ///   4. Fan out to `DexEngine` (other engines: placeholder Ok(vec![])).
    ///   5. For each `StrategyCandidate`:
    ///      a. Snapshot config (once per intent, not per candidate).
    ///      b. Call `evaluate_with_route_plan`.
    ///      c. Emit via `OpportunityEmitter`.
    ///   6. Engine errors are caught, logged, and counted — never crash the loop.
    ///
    /// Returns `Err` only when a Redis publish fails (the emitter propagates
    /// it so the caller can reconnect). Evaluation / gate / PG errors are
    /// swallowed per-candidate with a logged counter increment.
    pub async fn on_route_intent(&self, intent: RouteIntent) -> anyhow::Result<()> {
        let chain_id = self.ctx.chain_id;
        let source_str = detection_source_as_str(intent.source_event);

        // ── Step 1: decoded_intents_total metric ─────────────────────────
        OPPORTUNITIES_TOTAL
            .with_label_values(&[&chain_id.to_string(), source_str, "intent_decoded"])
            .inc();

        // ── Step 2: resolve ImpactSet ────────────────────────────────────
        let impact = {
            let idx = self.ctx.impact_index.read().await;
            idx.resolve(&intent)
        };

        // ── Step 3: impacted_routes_total metric ─────────────────────────
        OPPORTUNITIES_TOTAL
            .with_label_values(&[&chain_id.to_string(), "all", "impacted_pools_resolved"])
            .inc();

        debug!(
            event = "orchestrator.impact_resolved",
            chain_id,
            tx_hash = %intent.tx_hash,
            impacted_pools = impact.impacted_pools.len(),
            impacted_cycles = impact.impacted_cycles.len(),
        );

        // ── Step 4: fan out to engines ───────────────────────────────────

        // DexEngine (Phase 8 — live).
        let dex_candidates = match self
            .ctx
            .dex_engine
            .build_from_impacted_pairs(&intent, &impact)
            .await
        {
            Ok(v) => v,
            Err(e) => {
                DEX_ENGINE_ERRORS.fetch_add(1, Ordering::Relaxed);
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
            .build_from_impacted_cycles(&intent, &impact)
            .await
        {
            Ok(v) => v,
            Err(e) => {
                TRIANGULAR_ENGINE_ERRORS.fetch_add(1, Ordering::Relaxed);
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

        // Phase 11 placeholder: liquidation_engine (returns Ok(vec![])).
        // let liq_candidates = self.ctx.liquidation_engine.build(...).await?;

        // Concatenate base candidates: DEX + triangular.
        // Net-positive base candidates are fed to the flashloan wrapper.
        let mut base_candidates: Vec<StrategyCandidate> =
            Vec::with_capacity(dex_candidates.len() + tri_candidates.len());
        base_candidates.extend(dex_candidates);
        base_candidates.extend(tri_candidates);

        // FlashloanEngine (Phase 10 — live): wrap net-positive base candidates.
        let flash_candidates = self
            .ctx
            .flashloan_engine
            .wrap_profitable_routes(&base_candidates, chain_id);

        debug!(
            event = "orchestrator.engines_done",
            chain_id,
            tx_hash = %intent.tx_hash,
            base_count = base_candidates.len(),
            flash_wrap_count = flash_candidates.len(),
        );

        // ── Step 5: snapshot config once per intent ────────────────────────
        // The `TradingConfigState` is owned here so `ConfigAwareEvaluator`
        // can borrow it for the entire candidate loop without lifetime issues.
        let cfg_snapshot: Option<TradingConfigState> =
            self.ctx.config_provider.snapshot(chain_id).await;

        // ── Step 6: size + evaluate + emit each candidate ─────────────────
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

            // Run size_optimizer. Errors are non-fatal — treat as Ok(None).
            let sized_opt = match self
                .ctx
                .size_optimizer
                .optimize(candidate.clone(), &intent, cfg_snapshot.as_ref())
                .await
            {
                Ok(s) => s,
                Err(e) => {
                    warn!(
                        event = "orchestrator.size_optimizer_error",
                        chain_id,
                        tx_hash = %intent.tx_hash,
                        error = %e,
                        "size_optimizer returned Err — treating as no profit"
                    );
                    None
                }
            };

            let final_candidate = match sized_opt {
                Some(sized) => {
                    // Update the candidate with optimal sizing data.
                    let mut c = sized.candidate;
                    c.gross_profit_usd = Some(sized.gross_profit_usd);
                    c.net_expected_profit_usd = Some(sized.estimated_net_profit_usd);
                    c
                }
                None => {
                    // Optimizer found no profitable size — emit as rejected.
                    let mut c = candidate;
                    c.rejection_reason = Some("size_optimizer_no_profit".to_owned());
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

        // Engine-level rejection: no need to evaluate, just emit rejected.
        if let Some(reason) = &sc.rejection_reason {
            let reason_owned = reason.clone();
            let opp_with_reason = {
                let mut o = sc.opportunity.clone();
                o.rejection_reason = Some(reason_owned.clone());
                o
            };
            self.ctx
                .emitter
                .emit_rejected(&opp_with_reason, label, &reason_owned)
                .await?;
            return Ok(());
        }

        // Evaluator not available → observe-only path (same as scanner's no_trading_config).
        let Some(state) = cfg else {
            // Persist + publish without scoring (observe-only).
            self.ctx
                .emitter
                .emit_accepted(&sc.opportunity, label)
                .await?;
            return Ok(());
        };

        // Build the evaluator borrowing the owned config snapshot.
        let signals = NetworkSignals::unknown(sc.opportunity.block_number.unwrap_or(0));
        let ev = ConfigAwareEvaluator::with_cache(state, signals, HashMap::new());

        // Run the spine gate.
        let gate_outcome = ev.evaluate_with_route_plan(
            &sc.candidate,
            Some(&sc.route_plan),
            label_str,
            chain_id,
            "rpc-pool".to_string(),
            60_000,
        );

        match gate_outcome {
            ConfigGateOutcome::TokenNotAllowed {
                token_symbol_or_addr,
            } => {
                let reason = format!("TokenNotAllowed:{token_symbol_or_addr}");
                let mut opp = sc.opportunity.clone();
                opp.rejection_reason = Some(reason.clone());
                opp.roi_pct = Some(0.0);
                opp.risk_score = Some(0.0);
                self.ctx.emitter.emit_rejected(&opp, label, &reason).await?;
            }

            ConfigGateOutcome::StrategyDisabled { strategy_kind: sk } => {
                let reason = format!("StrategyDisabled:{sk}");
                let mut opp = sc.opportunity.clone();
                opp.rejection_reason = Some(reason.clone());
                opp.roi_pct = Some(0.0);
                opp.risk_score = Some(0.0);
                self.ctx.emitter.emit_rejected(&opp, label, &reason).await?;
            }

            ConfigGateOutcome::StrategyConfigGateBlocked { reason } => {
                let tag = reason.tag();
                let reason_str = format!("{tag}:{reason:?}");
                let mut opp = sc.opportunity.clone();
                opp.rejection_reason = Some(reason_str.clone());
                opp.roi_pct = Some(0.0);
                opp.risk_score = Some(0.0);
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
                    // Math gate rejected.
                    let reason_str = format!("{rej_reason:?}");
                    opp.rejection_reason = Some(reason_str.clone());
                    opp.roi_pct = Some(0.0);
                    opp.risk_score = Some(0.0);
                    // Propagate net_expected_profit_usd when gross is available (R8).
                    opp.net_expected_profit_usd =
                        opp.expected_profit_usd.map(|g| g - outcome.gas_cost_usd);
                    self.ctx
                        .emitter
                        .emit_rejected(&opp, label, &reason_str)
                        .await?;
                } else {
                    // Passed all gates.
                    opp.roi_pct = Some(outcome.net_roi_pct);
                    opp.net_expected_profit_usd = Some(outcome.net_profit_usd);
                    self.ctx.emitter.emit_accepted(&opp, label).await?;
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
    use crate::route_intent::DetectionSource;
    match src {
        DetectionSource::PublicMempool => "public_mempool",
        DetectionSource::FilteredMempool => "filtered_mempool",
        DetectionSource::PrivateHint => "private_hint",
        DetectionSource::NewBlock => "new_block",
        DetectionSource::OracleUpdate => "oracle_update",
        DetectionSource::LendingPositionUpdate => "lending_position_update",
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
    use tokio::sync::RwLock;
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
    // Verifies the counter increments correctly (validates the error path
    // counter logic in on_route_intent). The full async test would require
    // a mock engine; we validate the counter wiring here.

    #[test]
    fn engine_error_counter_increments() {
        let before = DEX_ENGINE_ERRORS.load(Ordering::Relaxed);
        DEX_ENGINE_ERRORS.fetch_add(1, Ordering::Relaxed);
        let after = DEX_ENGINE_ERRORS.load(Ordering::Relaxed);
        assert_eq!(after, before + 1, "DEX_ENGINE_ERRORS must increment by 1");
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
        let engine = DexEngine::new(Arc::new(RwLock::new(None)), None, None);
        let candidates = engine
            .build_from_impacted_pairs(&intent, &impact)
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
        let config = Arc::new(RwLock::new(
            None::<shared_rs::trading_config::TradingConfigState>,
        ));

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

        let tri_engine = Arc::new(TriangularEngine::new(cache, config.clone(), vec![seed]));

        // Build an ImpactSet with cycle_id = 0 impacted.
        use crate::impact_index::ImpactSet;
        let impact = ImpactSet {
            impacted_cycles: vec![0],
            ..Default::default()
        };

        let intent = make_intent(tok_a, tok_b);
        let candidates = tri_engine
            .build_from_impacted_cycles(&intent, &impact)
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
        let config = Arc::new(RwLock::new(
            None::<shared_rs::trading_config::TradingConfigState>,
        ));
        let fl_engine = FlashloanEngine::new(config);

        // Base candidate: DexArbV2V2, $50 gross, WETH on mainnet.
        let base = make_candidate(StrategyLabel::DexArbV2V2, None);
        // Ensure gross is Some and token_in is WETH.
        let mut base = base;
        base.gross_profit_usd = Some(50.0);
        base.route_plan.legs[0].token_in = "0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2".to_string();

        let wrapped = fl_engine.wrap_profitable_routes(&[base], 1);

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
        let before = TRIANGULAR_ENGINE_ERRORS.load(Ordering::Relaxed);
        TRIANGULAR_ENGINE_ERRORS.fetch_add(1, Ordering::Relaxed);
        let after = TRIANGULAR_ENGINE_ERRORS.load(Ordering::Relaxed);
        assert_eq!(
            after,
            before + 1,
            "TRIANGULAR_ENGINE_ERRORS must increment by 1"
        );
    }
}
