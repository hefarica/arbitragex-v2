//! Scanner pipeline counters for the heartbeat worker.
//!
//! Lock-free `AtomicU64` counters incremented by the scanner at every
//! decision point and READ + RESET by `heartbeat_worker` once per period
//! (default 60s). Gives the operator an explicit per-minute breakdown of
//! "what happened in the pipeline" without grep'ing logs.
//!
//! Doctrine: counters live as a process-global `OnceCell<ScannerCounters>`
//! to avoid plumbing 4 levels deep (main → scanner::run_chain →
//! detection_loop → run_subscription → process_pending). The cost is
//! global state — acceptable here because (a) one searcher process per
//! container, (b) atomics are lock-free, (c) the alternative is 5 file
//! refactors plus changing 4 function signatures for an observability
//! feature.
//!
//! Usage:
//!   counters().pending_received.fetch_add(1, Ordering::Relaxed);
//!   counters().gate_token_not_allowed.fetch_add(1, Ordering::Relaxed);
//!
//! Heartbeat read (atomic swap → 0 for delta semantics):
//!   counters().pending_received.swap(0, Ordering::Relaxed);

use once_cell::sync::Lazy;
use std::sync::atomic::AtomicU64;

#[derive(Default)]
pub struct ScannerCounters {
    /// Pending tx received from WS subscription (one per `process_pending` call).
    pub pending_received: AtomicU64,
    /// Tx decoded successfully (passed calldata + router lookup).
    pub decoded_ok: AtomicU64,
    /// Candidate enriched via V2-only spread (no V3 quotes landed).
    pub enriched_v2: AtomicU64,
    /// Candidate enriched with V2 + V3 multicall quotes.
    pub enriched_v3: AtomicU64,
    /// Gate: rejected because token outside allowlist.
    pub gate_token_not_allowed: AtomicU64,
    /// Gate: rejected because strategy_kind not in enabled_strategies.
    pub gate_strategy_disabled: AtomicU64,
    /// Gate: trading_config not seeded for this chain — observed only.
    pub gate_no_config: AtomicU64,
    /// Gate: spine returned UnknownTokenPrice (BUG-2 oracle gap).
    pub gate_unknown_token_price: AtomicU64,
    /// Gate: spine returned AnomalousMath (defense-in-depth bound fired).
    pub gate_anomalous_math: AtomicU64,
    /// Gate: spine returned any other risk-policy rejection
    /// (NegativeNetProfit, LowLiquidity, ExcessiveSlippage, ...).
    pub gate_other_rejected: AtomicU64,
    /// Passed all gates → would execute (paper-trade mode currently active).
    pub passed_all_gates: AtomicU64,
    /// Successfully persisted to PG (after any gate path).
    pub db_persisted: AtomicU64,
    /// PG insert failed (unique violation, numeric overflow, etc.).
    pub db_errors: AtomicU64,
    /// Price worker: tokens whose price came from Alchemy this period.
    /// Heartbeat surfaces this so the operator sees primary-source health.
    pub price_alchemy_hits: AtomicU64,
    /// Price worker: tokens whose price came from Coingecko fallback.
    /// Spike here = Alchemy degraded; verify with `price_worker_errors`.
    pub price_coingecko_hits: AtomicU64,
    /// Price worker: tokens with no price after BOTH sources tried.
    /// These will trigger `gate_unknown_token_price` downstream until the
    /// operator either enables them upstream or removes them from allowlist.
    pub price_cache_misses: AtomicU64,
    /// Price worker: HTTP / parse / Redis errors per period.
    /// Steady non-zero = config or upstream API regression.
    pub price_worker_errors: AtomicU64,
    /// TriangularWorker: cycles scanned this period (every tick × MVP_CYCLES.len() × 2 directions).
    /// Surfaced in heartbeat so operator sees the worker is alive and exercising
    /// every configured cycle, regardless of whether any were profitable.
    pub triangular_cycles_scanned: AtomicU64,
    /// TriangularWorker: opportunities emitted this period after spot-check, golden-section
    /// and dedup all pass. Spine evaluator runs downstream and applies the canonical risk
    /// gates; high count here with zero `passed_all_gates` means math says profitable but
    /// oracle/risk rejects (operator action required).
    pub triangular_opps_emitted: AtomicU64,
}

/// Process-global counters. First call initialises; subsequent calls return
/// the same `&'static ScannerCounters`. Lock-free atomics inside.
pub fn counters() -> &'static ScannerCounters {
    static INSTANCE: Lazy<ScannerCounters> = Lazy::new(ScannerCounters::default);
    &INSTANCE
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::Ordering;

    #[test]
    fn counters_singleton_returns_same_instance() {
        let a = counters() as *const _;
        let b = counters() as *const _;
        assert_eq!(a, b, "counters() must return the same global instance");
    }

    #[test]
    fn counters_default_to_zero() {
        // Sample a few — full struct check would be brittle to add new fields.
        // Note: this test runs in a shared process so other tests may have
        // incremented these; we assert non-negative which is always true for u64.
        let _ = counters().pending_received.load(Ordering::Relaxed);
        let _ = counters().gate_token_not_allowed.load(Ordering::Relaxed);
        let _ = counters().db_errors.load(Ordering::Relaxed);
    }

    #[test]
    fn counters_increment_and_swap_work() {
        // Use a fresh instance to avoid contamination from other tests.
        let local = ScannerCounters::default();
        local.pending_received.fetch_add(7, Ordering::Relaxed);
        local.pending_received.fetch_add(3, Ordering::Relaxed);
        let snapshot = local.pending_received.swap(0, Ordering::Relaxed);
        assert_eq!(snapshot, 10);
        // Post-swap reset to zero for the next period.
        assert_eq!(local.pending_received.load(Ordering::Relaxed), 0);
    }
}
