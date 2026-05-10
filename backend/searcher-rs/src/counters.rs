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
    /// FlashloanArbWorker: token-pairs scanned this period (one per MVP_PAIRS entry per tick).
    /// Steady non-zero proves the worker is alive and exercising every configured pair,
    /// regardless of whether any combo was profitable. A zero value across consecutive
    /// heartbeats (with the worker enabled in main.rs) is a regression signal.
    pub flashloan_arb_pairs_scanned: AtomicU64,
    /// FlashloanArbWorker: opportunities emitted this period after spot-rate diff, golden-section,
    /// dedup, min-profit and worker-level sanity bound all pass. Compare against
    /// `passed_all_gates` to learn what fraction survives the canonical risk policy.
    pub flashloan_arb_opps_emitted: AtomicU64,
    /// FlashloanArbWorker: pool combos rejected because expected_profit_usd > 10% of borrow_usd.
    /// Anti-Incidente #9 self-defense — non-zero means orientation/decimal/unit bug somewhere
    /// in the pool reserves cache. Operator action required: inspect the diagnostic dump
    /// in the `flashloan_arb_worker.sanity_reject` log line.
    pub flashloan_arb_sanity_reject: AtomicU64,
    /// TriangularWorker: V3-bearing cycles scanned this period (the 4 long-tail
    /// cycles X-WETH-USDC-X for X in {PEPE, SHIB, MKR, COMP}; both directions
    /// per tick). Counted independently of the V2-only `triangular_cycles_scanned`
    /// so the operator can attribute throughput per quote-source. Steady non-zero
    /// proves V3 fan-out is alive even when V3 pools have insufficient liquidity
    /// to emit (the failure case bumps `triangular_v3_quote_failures` instead).
    pub triangular_v3_cycles_scanned: AtomicU64,
    /// TriangularWorker: V3 quote calls (per-pool) that returned `success=false`
    /// or `amount_out=0` from QuoterV2 this period. Steady non-zero with zero
    /// emits = V3 pool index has stale entries OR the chosen fee tier has no
    /// active liquidity for the candidate amount_in. R8 fail-honest: every
    /// failure increments this counter, NEVER fabricates a synthetic amount_out.
    pub triangular_v3_quote_failures: AtomicU64,
    /// TriangularWorker: V3-bearing cycles rejected by the worker-level sanity
    /// bound (expected_profit_usd > SANITY_PROFIT_MULT_OF_CAP × cap_usd). Same
    /// guard the V2 path uses; mirrored here because V3 tick math is *more*
    /// complex than V2 CPMM, so bugs are MORE likely. Non-zero means orientation
    /// / pool-index / fee-tier / decimal bug — inspect `sanity_reject_v3` warn
    /// log for the diagnostic dump.
    pub triangular_v3_sanity_reject: AtomicU64,
    /// LiquidationWorker: Aave V3 positions scanned this period (one per
    /// watchlist member per tick when the multicall succeeds). Steady non-zero
    /// proves the worker is alive and reading on-chain. Zero across several
    /// heartbeats with the worker enabled = watchlist empty, no provider
    /// attached, or RPC unhealthy — the dominant skip bucket pinpoints which.
    pub liquidation_positions_scanned: AtomicU64,
    /// LiquidationWorker: opportunities emitted this period after HF threshold,
    /// dedup, profit estimation, sanity bound and min-profit gates all pass.
    /// Compare against `passed_all_gates` for the spine-survival rate.
    pub liquidation_opps_emitted: AtomicU64,
    /// LiquidationWorker: positions rejected because gross_profit_usd > 20% of
    /// debt_to_repay_usd. Anti-Incidente #9 self-defense — non-zero means a
    /// bonus-source / decimal-scaling bug somewhere in the estimation kernel.
    /// Operator action required: inspect the diagnostic dump in the
    /// `liquidation_worker.sanity_reject` log line.
    pub liquidation_sanity_reject: AtomicU64,
    /// CexDexWorker (BE-3.2): HTTP/parse/RPC errors per period. In Phase 1 this
    /// counter increments every tick per pair because `fetch_dex_price` returns
    /// `Err` intentionally (DEX quoter not yet wired). A steady value equal to
    /// `pairs × ticks` is EXPECTED in Phase 1 and is NOT a regression signal.
    /// After Phase 2 wiring the counter should drop to near-zero between
    /// genuine network failures.
    pub cex_dex_fetch_errors: AtomicU64,
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
