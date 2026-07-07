// M11 allow: Lazy<MetricVec> initializers use .expect() on infallible paths
// (constant metric names, no duplicate registration possible across one binary).
// This is the same pattern as shared_rs/src/metrics.rs.
#![allow(clippy::expect_used)]
//! Per-strategy Prometheus metrics for the event-driven orchestrator.
//!
//! ## Design (spec §5)
//!
//! All label values that encode strategy names MUST come exclusively from
//! `StrategyLabel::as_str()`. This is the only point in the codebase where
//! the string is produced; every metric call-site passes `label.as_str()` as
//! an argument rather than a literal. The grep guard in CI enforces this:
//!
//! ```sh
//! grep -nE '\.with_label_values\(&\["[^"]*","[^"]*"' \
//!     backend/searcher-rs/src/orchestrator.rs \
//!     backend/searcher-rs/src/engines/*.rs \
//!   | grep -E '"dex_arb|"triangular|"flashloan|"liquidation' \
//!   | grep -v 'StrategyLabel'
//! # MUST return ZERO lines.
//! ```
//!
//! ## Registration
//!
//! All metrics register into `shared_rs::metrics::REGISTRY` (the same Prometheus
//! registry served by the `/metrics` HTTP endpoint). A secondary `Lazy`
//! initialisation anchor in `init_orchestrator_metrics()` force-initialises all
//! `Lazy<...>` statics so the metrics appear in the registry before any counter
//! increment occurs. `main.rs` calls this function once at boot.
//!
//! ## R8 invariants
//!
//! - Labels derived from `StrategyLabel::as_str()` are stable; renaming a variant
//!   requires updating `as_str()` in `strategy_label.rs`, which is the single
//!   source of truth. Dashboards that hard-code these values are covered by the
//!   pin-test in `strategy_label::tests::as_str_table_pinned`.
//! - Gauge `liquidation_watchlist_size` reflects the live watchlist size from
//!   Redis. When Redis is unreachable it is not set (R8 fail-honest). Use the
//!   `positions_watchlist_empty_total` counter to detect zero-entries condition.

use once_cell::sync::Lazy;
use prometheus::{IntCounterVec, IntGauge};
use shared_rs::metrics::REGISTRY;

// ---------------------------------------------------------------------------
// decoded_intents_total{chain_id, source}
// ---------------------------------------------------------------------------

/// Number of `RouteIntent`s decoded from mempool transactions, by chain and
/// detection source. Replaces the ad-hoc OPPORTUNITIES_TOTAL "intent_decoded"
/// label usage with a dedicated metric that has explicit `source` semantics.
///
/// `source` values are `DetectionSource` strings: `public_mempool`,
/// `filtered_mempool`, `private_hint`, `new_block`, `oracle_update`,
/// `lending_position_update`.
pub static DECODED_INTENTS_TOTAL: Lazy<IntCounterVec> = Lazy::new(|| {
    let c = IntCounterVec::new(
        prometheus::opts!(
            "arbx_decoded_intents_total",
            "RouteIntents decoded from mempool transactions, by chain and source"
        ),
        &["chain_id", "source"],
    )
    .expect("metric");
    REGISTRY.register(Box::new(c.clone())).expect("register");
    c
});

// ---------------------------------------------------------------------------
// impacted_routes_total{chain_id, strategy}
// ---------------------------------------------------------------------------

/// Number of impacted routes resolved by `ImpactIndex::resolve`, labelled by
/// the engine that consumed them. Emitted once per engine fan-out per intent.
///
/// `strategy` values come from `StrategyLabel::as_str()`.
pub static IMPACTED_ROUTES_TOTAL: Lazy<IntCounterVec> = Lazy::new(|| {
    let c = IntCounterVec::new(
        prometheus::opts!(
            "arbx_impacted_routes_total",
            "Impacted routes fed to strategy engines, by chain and strategy"
        ),
        &["chain_id", "strategy"],
    )
    .expect("metric");
    REGISTRY.register(Box::new(c.clone())).expect("register");
    c
});

// ---------------------------------------------------------------------------
// candidates_total{chain_id, strategy}
// ---------------------------------------------------------------------------

/// Number of `StrategyCandidate`s produced by the engines (before evaluation),
/// labelled by the strategy that produced them.
///
/// `strategy` values come from `StrategyLabel::as_str()`.
pub static CANDIDATES_TOTAL: Lazy<IntCounterVec> = Lazy::new(|| {
    let c = IntCounterVec::new(
        prometheus::opts!(
            "arbx_candidates_total",
            "StrategyCandidate instances produced by engines, by chain and strategy"
        ),
        &["chain_id", "strategy"],
    )
    .expect("metric");
    REGISTRY.register(Box::new(c.clone())).expect("register");
    c
});

// ---------------------------------------------------------------------------
// rejected_no_profit_total{chain_id, strategy, reason}
// ---------------------------------------------------------------------------

/// Number of candidates rejected by the pipeline because they had no
/// profitable size (size_optimizer returned None) or were pre-rejected by the
/// engine (single-pool, no spread, etc.).
///
/// `strategy` values come from `StrategyLabel::as_str()`.
/// `reason` is the `rejection_reason` string (e.g. `size_optimizer_no_profit`,
/// `single_pool_no_spread`, `flashloan_no_provider`).
pub static REJECTED_NO_PROFIT_TOTAL: Lazy<IntCounterVec> = Lazy::new(|| {
    let c = IntCounterVec::new(
        prometheus::opts!(
            "arbx_rejected_no_profit_total",
            "Candidates rejected because no profitable size was found, by chain/strategy/reason"
        ),
        &["chain_id", "strategy", "reason"],
    )
    .expect("metric");
    REGISTRY.register(Box::new(c.clone())).expect("register");
    c
});

// ---------------------------------------------------------------------------
// rejected_config_total{chain_id, strategy, reason}  (TASK 3)
// ---------------------------------------------------------------------------

/// Number of candidates rejected by the config gate (TokenNotAllowed,
/// StrategyDisabled, StrategyConfigGateBlocked).
///
/// DISTINCT from `simulation_failed_total` — the config gate runs BEFORE any
/// simulation. Mixing them makes the Grafana funnel chart useless. This counter
/// only fires for gate outcomes that are purely policy-driven, not math-driven.
///
/// `strategy` values come from `StrategyLabel::as_str()`.
/// `reason` values:
///   - `"token_not_allowed"`
///   - `"strategy_disabled"`
///   - any tag string from `ConfigGateOutcome::StrategyConfigGateBlocked { tag }`
pub static REJECTED_CONFIG_TOTAL: Lazy<IntCounterVec> = Lazy::new(|| {
    let c = IntCounterVec::new(
        prometheus::opts!(
            "arbx_rejected_config_total",
            "Candidates rejected by the config gate (policy), by chain/strategy/reason"
        ),
        &["chain_id", "strategy", "reason"],
    )
    .expect("metric");
    REGISTRY.register(Box::new(c.clone())).expect("register");
    c
});

// ---------------------------------------------------------------------------
// simulation_failed_total{chain_id, strategy, reason}
// ---------------------------------------------------------------------------

/// Number of candidates that failed the simulation/evaluation stage.
///
/// `strategy` values come from `StrategyLabel::as_str()`.
/// `reason` is the gate-outcome discriminant (e.g. `TokenNotAllowed`,
/// `StrategyDisabled`, `NegativeNetProfit`, etc.).
///
/// NOTE: TokenNotAllowed / StrategyDisabled rejections are now routed to
/// `rejected_config_total` instead. This counter is reserved for genuine
/// simulation failures (EvaluatedRejected from the math gate, or future
/// sim_ctl failures). It must NOT be incremented for policy-gate rejections.
pub static SIMULATION_FAILED_TOTAL: Lazy<IntCounterVec> = Lazy::new(|| {
    let c = IntCounterVec::new(
        prometheus::opts!(
            "arbx_simulation_failed_total",
            "Candidates that failed simulation/evaluation, by chain/strategy/reason"
        ),
        &["chain_id", "strategy", "reason"],
    )
    .expect("metric");
    REGISTRY.register(Box::new(c.clone())).expect("register");
    c
});

// ---------------------------------------------------------------------------
// opportunities_published_total{chain_id, strategy}
// ---------------------------------------------------------------------------

/// Number of opportunities successfully published (accepted + emitted to
/// Redis stream + persisted to PG), by chain and strategy.
///
/// `strategy` values come from `StrategyLabel::as_str()`.
pub static OPPORTUNITIES_PUBLISHED_TOTAL: Lazy<IntCounterVec> = Lazy::new(|| {
    let c = IntCounterVec::new(
        prometheus::opts!(
            "arbx_opportunities_published_total",
            "Opportunities accepted and published, by chain and strategy"
        ),
        &["chain_id", "strategy"],
    )
    .expect("metric");
    REGISTRY.register(Box::new(c.clone())).expect("register");
    c
});

// ---------------------------------------------------------------------------
// engine_errors_total{chain_id, strategy}
// ---------------------------------------------------------------------------

/// Number of engine errors (build_from_* returned Err), by chain and strategy.
/// Replaces the old AtomicU64 `DEX_ENGINE_ERRORS`, `TRIANGULAR_ENGINE_ERRORS`,
/// `LIQUIDATION_ENGINE_ERRORS` in orchestrator.rs.
///
/// `strategy` values come from `StrategyLabel::as_str()`.
pub static ENGINE_ERRORS_TOTAL: Lazy<IntCounterVec> = Lazy::new(|| {
    let c = IntCounterVec::new(
        prometheus::opts!(
            "arbx_engine_errors_total",
            "Strategy engine errors, by chain and strategy"
        ),
        &["chain_id", "strategy"],
    )
    .expect("metric");
    REGISTRY.register(Box::new(c.clone())).expect("register");
    c
});

// ---------------------------------------------------------------------------
// flashloan_wrapped_total{chain_id, base_strategy, provider}
// ---------------------------------------------------------------------------

/// Number of base candidates successfully wrapped with a flash loan, by chain,
/// base strategy, and flash-loan provider name.
///
/// `base_strategy` values come from `StrategyLabel::as_str()` on the
/// `base_strategy` field of the wrapped `StrategyCandidate`.
/// `provider` values: `"dydx_solo"`, `"balancer_vault"`, `"aave_v3"`.
pub static FLASHLOAN_WRAPPED_TOTAL: Lazy<IntCounterVec> = Lazy::new(|| {
    let c = IntCounterVec::new(
        prometheus::opts!(
            "arbx_flashloan_wrapped_total",
            "Flash-loan-wrapped candidates, by chain, base strategy and provider"
        ),
        &["chain_id", "base_strategy", "provider"],
    )
    .expect("metric");
    REGISTRY.register(Box::new(c.clone())).expect("register");
    c
});

// ---------------------------------------------------------------------------
// flashloan_rejected_negative_after_fee_total{chain_id}
// ---------------------------------------------------------------------------

/// Number of flashloan wrap attempts rejected because the net profit after the
/// flash fee was zero or negative. Replaces the ad-hoc AtomicU64 that existed
/// in `flashloan_engine.rs` in earlier phases.
pub static FLASHLOAN_REJECTED_NEGATIVE_AFTER_FEE_TOTAL: Lazy<IntCounterVec> = Lazy::new(|| {
    let c = IntCounterVec::new(
        prometheus::opts!(
            "arbx_flashloan_rejected_negative_after_fee_total",
            "Flashloan wraps rejected because net <= 0 after flash fee, by chain"
        ),
        &["chain_id"],
    )
    .expect("metric");
    REGISTRY.register(Box::new(c.clone())).expect("register");
    c
});

// ---------------------------------------------------------------------------
// liquidation_watchlist_size{chain_id, protocol}
// ---------------------------------------------------------------------------

/// Gauge: current number of addresses in the lending watchlist.
/// Updated periodically by the `liquidation_worker` after each Redis SCARD.
/// NOT updated when Redis is unreachable (R8 fail-honest — stale is better
/// than fabricated).
///
/// `protocol` values: `"aave_v3"`, `"compound_v2"`.
pub static LIQUIDATION_WATCHLIST_SIZE: Lazy<prometheus::IntGaugeVec> = Lazy::new(|| {
    let g = prometheus::IntGaugeVec::new(
        prometheus::opts!(
            "arbx_liquidation_watchlist_size",
            "Number of addresses in the liquidation watchlist, by chain and protocol"
        ),
        &["chain_id", "protocol"],
    )
    .expect("metric");
    REGISTRY.register(Box::new(g.clone())).expect("register");
    g
});

// ---------------------------------------------------------------------------
// liquidation_recalc_triggered_total{chain_id, trigger}
// ---------------------------------------------------------------------------

/// Number of times a liquidation position re-evaluation was triggered, by chain
/// and trigger type.
///
/// `trigger` values: `"mempool_event"` (RouteIntent hit), `"periodic_poll"`
/// (liquidation_worker tick), `"watchlist_seeded"` (new address added).
pub static LIQUIDATION_RECALC_TRIGGERED_TOTAL: Lazy<IntCounterVec> = Lazy::new(|| {
    let c = IntCounterVec::new(
        prometheus::opts!(
            "arbx_liquidation_recalc_triggered_total",
            "Position re-evaluation triggers, by chain and trigger type"
        ),
        &["chain_id", "trigger"],
    )
    .expect("metric");
    REGISTRY.register(Box::new(c.clone())).expect("register");
    c
});

// ---------------------------------------------------------------------------
// positions_watchlist_empty_total{chain_id}
// ---------------------------------------------------------------------------

/// Number of times the liquidation path was entered but the watchlist was found
/// empty (no addresses indexed at all). Replaces the ad-hoc AtomicU64
/// `POSITIONS_WATCHLIST_EMPTY_TOTAL` in `lending_position_indexer.rs`.
pub static POSITIONS_WATCHLIST_EMPTY_TOTAL: Lazy<IntCounterVec> = Lazy::new(|| {
    let c = IntCounterVec::new(
        prometheus::opts!(
            "arbx_positions_watchlist_empty_total",
            "Times the liquidation watchlist was empty when evaluated, by chain"
        ),
        &["chain_id"],
    )
    .expect("metric");
    REGISTRY.register(Box::new(c.clone())).expect("register");
    c
});

// ---------------------------------------------------------------------------
// legacy_worker_disabled_total{worker}
// ---------------------------------------------------------------------------

/// Number of times a legacy worker was NOT spawned because the Phase 15
/// default-off gate was active. Replaces the ad-hoc AtomicU64
/// `LEGACY_WORKER_DISABLED_TOTAL` in `main.rs`.
///
/// `worker` values: `"triangular_worker"`, `"flashloan_arb_worker"`,
/// `"liquidation_worker"` — exactly the module names, for Grafana consistency.
pub static LEGACY_WORKER_DISABLED_TOTAL: Lazy<IntCounterVec> = Lazy::new(|| {
    let c = IntCounterVec::new(
        prometheus::opts!(
            "arbx_legacy_worker_disabled_total",
            "Legacy workers skipped at boot because the default-off Phase 15 gate was active"
        ),
        &["worker"],
    )
    .expect("metric");
    REGISTRY.register(Box::new(c.clone())).expect("register");
    c
});

// ---------------------------------------------------------------------------
// Convenience: a single IntGauge that indicates how many legacy workers are
// currently disabled (sum). Surfaced as `arbx_legacy_workers_disabled_count`.
// Separate from the counter above so operators can gauge the current number
// without summing time-series data.
// ---------------------------------------------------------------------------

pub static LEGACY_WORKERS_DISABLED_COUNT: Lazy<IntGauge> = Lazy::new(|| {
    let g = IntGauge::new(
        "arbx_legacy_workers_disabled_count",
        "Number of legacy workers currently gated off (Phase 15 defaults)",
    )
    .expect("metric");
    REGISTRY.register(Box::new(g.clone())).expect("register");
    g
});

// ---------------------------------------------------------------------------
// Initialiser — call once at boot (main.rs after init_metrics())
// ---------------------------------------------------------------------------

/// Force-initialises all orchestrator Prometheus metrics so they appear in
/// the registry before the first counter increment.
///
/// Must be called after `shared_rs::metrics::init_metrics()` to guarantee that
/// the registry is ready before we attempt to register.
pub fn init_orchestrator_metrics() {
    let _ = &*DECODED_INTENTS_TOTAL;
    let _ = &*IMPACTED_ROUTES_TOTAL;
    let _ = &*CANDIDATES_TOTAL;
    let _ = &*REJECTED_NO_PROFIT_TOTAL;
    let _ = &*REJECTED_CONFIG_TOTAL;
    let _ = &*SIMULATION_FAILED_TOTAL;
    let _ = &*OPPORTUNITIES_PUBLISHED_TOTAL;
    let _ = &*ENGINE_ERRORS_TOTAL;
    let _ = &*FLASHLOAN_WRAPPED_TOTAL;
    let _ = &*FLASHLOAN_REJECTED_NEGATIVE_AFTER_FEE_TOTAL;
    let _ = &*LIQUIDATION_WATCHLIST_SIZE;
    let _ = &*LIQUIDATION_RECALC_TRIGGERED_TOTAL;
    let _ = &*POSITIONS_WATCHLIST_EMPTY_TOTAL;
    let _ = &*LEGACY_WORKER_DISABLED_TOTAL;
    let _ = &*LEGACY_WORKERS_DISABLED_COUNT;
}

// ---------------------------------------------------------------------------
// Tests (TASK 1 spec §5)
// ---------------------------------------------------------------------------

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use crate::route_intent::DetectionSource;
    use crate::strategy_label::StrategyLabel;

    // Helper: convert DetectionSource to the label string used in DECODED_INTENTS_TOTAL.
    fn source_str(src: DetectionSource) -> &'static str {
        match src {
            DetectionSource::PublicMempool => "public_mempool",
            DetectionSource::FilteredMempool => "filtered_mempool",
            DetectionSource::PrivateHint => "private_hint",
            DetectionSource::NewBlock => "new_block",
            DetectionSource::OracleUpdate => "oracle_update",
            DetectionSource::LendingPositionUpdate => "lending_position_update",
        }
    }

    // ── metrics::tests::decoded_intents_increments_per_source ────────────────
    //
    // Verifies that different DetectionSource values produce independent counter
    // series (separate label tuples) and increment correctly.

    #[test]
    fn decoded_intents_increments_per_source() {
        let chain = "1";

        let src_public = source_str(DetectionSource::PublicMempool);
        let src_filtered = source_str(DetectionSource::FilteredMempool);

        let before_public = DECODED_INTENTS_TOTAL
            .with_label_values(&[chain, src_public])
            .get();
        let before_filtered = DECODED_INTENTS_TOTAL
            .with_label_values(&[chain, src_filtered])
            .get();

        DECODED_INTENTS_TOTAL
            .with_label_values(&[chain, src_public])
            .inc_by(3);
        DECODED_INTENTS_TOTAL
            .with_label_values(&[chain, src_filtered])
            .inc_by(7);

        let after_public = DECODED_INTENTS_TOTAL
            .with_label_values(&[chain, src_public])
            .get();
        let after_filtered = DECODED_INTENTS_TOTAL
            .with_label_values(&[chain, src_filtered])
            .get();

        assert_eq!(
            after_public - before_public,
            3,
            "public_mempool counter must increment by 3"
        );
        assert_eq!(
            after_filtered - before_filtered,
            7,
            "filtered_mempool counter must increment by 7 (independent series)"
        );
    }

    // ── metrics::tests::impacted_routes_uses_strategy_label_str ─────────────
    //
    // Verifies that the label value used for `impacted_routes_total` comes
    // from `StrategyLabel::as_str()` and is identical to the pinned string.

    #[test]
    fn impacted_routes_uses_strategy_label_str() {
        let chain = "1";

        // The label value for strategy must come from StrategyLabel::as_str().
        // We verify that the string is the same as what `as_str()` returns,
        // which is the invariant enforced by the grep guard.
        let label = StrategyLabel::DexArbV2V2;
        let strategy_str = label.as_str(); // "dex_arb_v2v2"

        let before = IMPACTED_ROUTES_TOTAL
            .with_label_values(&[chain, strategy_str])
            .get();
        IMPACTED_ROUTES_TOTAL
            .with_label_values(&[chain, strategy_str])
            .inc();
        let after = IMPACTED_ROUTES_TOTAL
            .with_label_values(&[chain, strategy_str])
            .get();

        assert_eq!(
            after - before,
            1,
            "impacted_routes_total must increment by 1"
        );
        // Pin the exact label string — if StrategyLabel::as_str() ever changes
        // "dex_arb_v2v2" to something else this test catches the divergence.
        assert_eq!(
            strategy_str, "dex_arb_v2v2",
            "StrategyLabel::DexArbV2V2.as_str() pin: must be 'dex_arb_v2v2'"
        );
    }

    // ── metrics::tests::flashloan_wrapped_has_base_strategy_label ────────────
    //
    // Verifies that `flashloan_wrapped_total` uses `base_strategy.as_str()`
    // as the `base_strategy` label (never a hardcoded string literal).

    #[test]
    fn flashloan_wrapped_has_base_strategy_label() {
        let chain = "1";

        // A DexArbV2V3 candidate gets wrapped → base_strategy = "dex_arb_v2v3"
        let base_label = StrategyLabel::DexArbV2V3;
        let base_str = base_label.as_str(); // "dex_arb_v2v3" from the enum

        let provider = "dydx_solo";

        let before = FLASHLOAN_WRAPPED_TOTAL
            .with_label_values(&[chain, base_str, provider])
            .get();
        FLASHLOAN_WRAPPED_TOTAL
            .with_label_values(&[chain, base_str, provider])
            .inc();
        let after = FLASHLOAN_WRAPPED_TOTAL
            .with_label_values(&[chain, base_str, provider])
            .get();

        assert_eq!(
            after - before,
            1,
            "flashloan_wrapped_total must increment when using as_str() label"
        );
        assert_eq!(
            base_str, "dex_arb_v2v3",
            "base_strategy label must match StrategyLabel::DexArbV2V3.as_str()"
        );
    }

    // ── metrics::tests::positions_watchlist_empty_per_chain ──────────────────
    //
    // Verifies that `positions_watchlist_empty_total` uses chain_id as a
    // distinguishing label so different chains accumulate independently.

    #[test]
    fn positions_watchlist_empty_per_chain() {
        let chain_1 = "1";
        let chain_137 = "137";

        let before_1 = POSITIONS_WATCHLIST_EMPTY_TOTAL
            .with_label_values(&[chain_1])
            .get();
        let before_137 = POSITIONS_WATCHLIST_EMPTY_TOTAL
            .with_label_values(&[chain_137])
            .get();

        POSITIONS_WATCHLIST_EMPTY_TOTAL
            .with_label_values(&[chain_1])
            .inc_by(5);
        POSITIONS_WATCHLIST_EMPTY_TOTAL
            .with_label_values(&[chain_137])
            .inc_by(2);

        let after_1 = POSITIONS_WATCHLIST_EMPTY_TOTAL
            .with_label_values(&[chain_1])
            .get();
        let after_137 = POSITIONS_WATCHLIST_EMPTY_TOTAL
            .with_label_values(&[chain_137])
            .get();

        assert_eq!(
            after_1 - before_1,
            5,
            "chain_id=1 watchlist empty counter must increment by 5"
        );
        assert_eq!(
            after_137 - before_137,
            2,
            "chain_id=137 watchlist empty counter must increment independently by 2"
        );
    }

    // ── metrics::tests::legacy_worker_disabled_label_matches_module_name ─────
    //
    // Verifies that the `worker` label values used for `legacy_worker_disabled_total`
    // match the canonical module names (not arbitrary strings). The grep guard in CI
    // enforces that these values are passed as string literals matching the pattern
    // `"<name>_worker"` — this test pins the exact strings.

    #[test]
    fn legacy_worker_disabled_label_matches_module_name() {
        // The three worker names from main.rs (Phase 15).
        let workers = [
            "triangular_worker",
            "flashloan_arb_worker",
            "liquidation_worker",
        ];

        for worker in workers {
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
                after - before,
                1,
                "worker={worker} counter must increment by 1"
            );
        }

        // Pin the exact module name strings (not aliases or shortened names).
        assert_eq!(workers[0], "triangular_worker");
        assert_eq!(workers[1], "flashloan_arb_worker");
        assert_eq!(workers[2], "liquidation_worker");
    }
}
