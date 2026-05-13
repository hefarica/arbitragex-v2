//! Scanner pipeline counters for the heartbeat worker.
//!
//! Lock-free `AtomicU64` counters incremented by the scanner at every
//! decision point and READ + RESET by `heartbeat_worker` once per period
//! (default 60s). Gives the operator an explicit per-minute breakdown of
//! "what happened in the pipeline" without grep'ing logs.
//!
//! B0.1 (2026-05-13) — Per-chain isolation registry:
//!
//! The original implementation kept a single process-global `ScannerCounters`
//! (`counters() -> &'static`). With multi-chain enabled, that meant Chain 1
//! and Chain 137 increments mixed into the same counter values — a footgun
//! flagged in the audit and recorded in `docs/superpowers/specs/...`. The
//! fix is a registry `HashMap<chain_id, Arc<ScannerCounters>>` accessed via
//! `chain_counters(chain_id)`. Each `run_chain(chain_id)` task writes only
//! to its own counters; heartbeat reads per-chain.
//!
//! Backward compatibility: `counters()` is retained as an alias for
//! `chain_counters(LEGACY_CHAIN_ID)`. Existing call sites that have not
//! been migrated still compile and emit to a designated "legacy/unscoped"
//! counter bucket. New code MUST use `chain_counters(chain_id)`.
//!
//! Usage (per-chain, preferred):
//!   chain_counters(chain_id).pending_received.fetch_add(1, Ordering::Relaxed);
//!
//! Usage (legacy, deprecated):
//!   counters().pending_received.fetch_add(1, Ordering::Relaxed);
//!
//! Heartbeat read (per-chain, atomic swap → 0 for delta semantics):
//!   chain_counters(chain_id).pending_received.swap(0, Ordering::Relaxed);

use once_cell::sync::Lazy;
use std::collections::HashMap;
use std::sync::atomic::AtomicU64;
use std::sync::{Arc, RwLock};

/// Sentinel chain id used by the legacy `counters()` API for code paths that
/// have not yet been migrated to per-chain. Reported as
/// "legacy_unscoped" in heartbeat aggregation so operators see how much
/// observability is still unattributed.
pub const LEGACY_CHAIN_ID: u64 = 0;

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
    /// Simulator Phase A.1+A.2 fail-closed semantics — candidates rejected at
    /// the simulator gate because no real REVM result could be produced.
    ///
    /// Increments when:
    /// - `v2-simulator` feature is disabled at compile time, OR
    /// - `simulator-v2` has no RPC URL configured for the candidate's chain, OR
    /// - the candidate has no executable calldata (the encoder lands in
    ///   Phase A.3 — until then every candidate falls into this bucket).
    ///
    /// Doctrine: this counter is **expected to be high** during the
    /// Phase A.1/A.2 transition. The legacy stub used to fabricate a "PASS"
    /// for every candidate (RULE 00 violation); fail-closed honesty means we
    /// reject instead of lying. Counter drops once Phase A.3 (calldata
    /// encoder against `ArbitrageExecutor.sol`) ships.
    pub simulator_fail_closed_rejected: AtomicU64,
    /// Simulator Phase A.1+A.2 — REVM ran and returned a successful result
    /// (`SimResult` with `net_profit_wei > 0`). Zero until A.3 encoder lands;
    /// non-zero afterwards proves REVM is actually executing real calldata.
    ///
    /// Forward-declared: Phase A.3 wires the increment site once the encoder
    /// produces `CandidateInput` with real calldata. Kept here so the metric
    /// schema is stable between PRs (heartbeat + Grafana panels can be wired
    /// against the field name today).
    #[allow(dead_code)]
    pub simulator_revm_success: AtomicU64,
    /// Simulator Phase A.1+A.2 — REVM ran and the transaction reverted.
    /// A non-zero value with real calldata is HEALTHY (means we're catching
    /// candidates that would have lost money before relay submit).
    ///
    /// Forward-declared: same rationale as `simulator_revm_success`.
    #[allow(dead_code)]
    pub simulator_revm_revert: AtomicU64,
    /// Phase A.2.5 — candidate reached the hot path with a per-chain
    /// `Arc<SimulatorV2>` available (RPC_HTTP_<chain_id> configured and pool
    /// healthy at boot) BUT the `OpportunityCandidate → simulator_v2::
    /// CandidateInput` encoder is not yet wired (Phase A.3 deliverable).
    ///
    /// This is the **honest interim state**: the simulator is reachable but
    /// the system has no way to encode an executable transaction from the
    /// abstract candidate. The candidate is rejected fail-closed; real REVM
    /// dispatch lands once the encoder ships.
    ///
    /// During Phase A.2.5 transition this counter should equal
    /// `simulator_fail_closed_rejected` for any chain that has a pool.
    pub simulator_v2_encoder_not_ready: AtomicU64,
    /// Phase A.2.5 — candidate reached the hot path on a chain that has NO
    /// `Arc<SimulatorV2>` available. Two root causes:
    ///   - `RPC_HTTP_<chain_id>` is not configured in env, OR
    ///   - the pool was empty / all providers unhealthy at boot.
    ///
    /// Distinct from `encoder_not_ready` because the remediation is different:
    /// here the operator must provide an RPC endpoint; there the operator
    /// waits for the encoder PR. The two values together account for every
    /// `SIM_DISABLED_FAIL_CLOSED` rejection.
    pub simulator_v2_no_simulator_for_chain: AtomicU64,
    /// Phase A.3.a — `OpportunityCandidate → RoundTripContext` encoder
    /// produced a valid context. Forward-declared: the runtime wire lands
    /// alongside the Phase A.3.b decimals provider + execute_round_trip
    /// orchestrator. Once the wire is live the count of successful encodings
    /// per period is the leading indicator that the system is ready to start
    /// actually simulating routes.
    #[allow(dead_code)]
    pub encoder_success_total: AtomicU64,
    /// Phase A.3.a — encoder rejected the candidate (any `SimEncoderError`
    /// variant). The reason tag is logged with the counter increment so the
    /// operator can attribute rejections to encoder gaps vs candidate gaps.
    #[allow(dead_code)]
    pub encoder_rejected_total: AtomicU64,
    /// Phase A.3.a — encoder rejected because `EXECUTOR_<chain_id>` env var
    /// missing / invalid / zero. Distinct from `encoder_rejected_total` so
    /// the operator can grep on this specific configuration gap.
    #[allow(dead_code)]
    pub encoder_missing_executor_total: AtomicU64,
    /// Phase A.3.a — encoder rejected because the decimals provider has no
    /// entry for `(chain_id, token_in)`. Pre-A.3.b this fires for every
    /// candidate (decimals provider is empty); post-A.3.b the count drops
    /// to the long-tail of unindexed tokens.
    #[allow(dead_code)]
    pub encoder_missing_decimals_total: AtomicU64,
    /// Phase A.3.a — encoder rejected because `amount_in` is NaN, ±Inf,
    /// non-positive, sub-wei, or overflows U256.
    #[allow(dead_code)]
    pub encoder_invalid_amount_total: AtomicU64,
    /// Phase A.3.a — encoder rejected because `dex_adapters` carries a
    /// label outside the Phase A.3.a allowlist (V2 + Sushi only).
    #[allow(dead_code)]
    pub encoder_unsupported_dex_total: AtomicU64,
    /// Phase A.3.a — encoder rejected because the candidate route shape is
    /// not the 2-leg round trip supported by Phase A.3.a (triangular,
    /// flashloan_arb, liquidation shapes all map here).
    #[allow(dead_code)]
    pub encoder_unsupported_route_shape_total: AtomicU64,
    /// Phase A.3.b — encoder produced a valid `RoundTripContext` and the
    /// hot path is fail-closed waiting for the `execute_round_trip` REVM
    /// orchestrator (Phase A.3.c). NO LONGER INCREMENTED in A.3.c — the
    /// orchestrator dispatch happens immediately after encoder OK, so the
    /// "pending" window is observationally zero. Kept here as a stable
    /// Prometheus metric name for dashboards built against earlier phases.
    #[allow(dead_code)]
    pub encoder_round_trip_executor_pending_total: AtomicU64,
    /// Phase A.3.b — encoder rejected because the parsed `token_in` is the
    /// zero address (caught by `parse_token_address` before the decimals
    /// provider is even consulted).
    pub encoder_zero_token_address_total: AtomicU64,
    /// Phase A.3.b — encoder rejected for an unhandled `SimEncoderError`
    /// variant not covered by the specific counters above. Should stay at
    /// zero in normal operation; non-zero indicates a new error variant
    /// has been added to `SimEncoderError` without a corresponding counter.
    pub encoder_other_rejected_total: AtomicU64,
    /// Phase A.3.b — every cached lookup that returned `Some(decimals)`.
    pub encoder_provider_cache_hit_total: AtomicU64,
    /// Phase A.3.b — every cached lookup that returned `None` (cold token).
    /// Bootstrap warm-up should bring this near zero for well-trafficked
    /// tokens within the first refresh tick (~60s after boot).
    pub encoder_provider_cache_miss_total: AtomicU64,
    /// Phase A.3.b — encoder hot-path gate skipped because no decimals
    /// provider was threaded into the scanner. Indicates a main.rs wiring
    /// regression — should stay at zero.
    pub encoder_missing_provider_total: AtomicU64,
    /// Phase A.3.c — orchestrator entered `execute_round_trip_revm`. The
    /// floor of every other A.3.c counter; useful for computing rates.
    pub round_trip_executor_started_total: AtomicU64,
    /// Phase A.3.c — REVM ran successfully AND all anti-fraud guards
    /// passed (gas_used > 0, trace_hash non-zero, net_profit_wei > 0).
    /// The single bucket that admits a candidate to the EvidenceGate as
    /// `SIM_SUCCESS`.
    pub round_trip_executor_success_total: AtomicU64,
    /// Phase A.3.c — REVM returned `SimError::Reverted`. Expected
    /// behaviour for the majority of candidates until A.3.c.2 lands
    /// ERC-20 prefund + role storage cheating (RULE 12 fail-honest).
    pub round_trip_executor_revert_total: AtomicU64,
    /// Phase A.3.c — REVM returned an infrastructure error
    /// (`SimError::Provider` / `NotImplemented`). Non-zero rate indicates
    /// an RPC / LazyDb issue.
    pub round_trip_executor_error_total: AtomicU64,
    /// Phase A.3.c — orchestrator rejected the candidate before reaching
    /// REVM (validation, config, gas-price overflow, etc.).
    pub round_trip_executor_rejected_pre_revm_total: AtomicU64,
    /// Phase A.3.c — `simulator.simulate()` returned success but the
    /// candidate failed the `net_profit_wei > 0` gate. Genuinely-non-
    /// profitable simulated routes land here.
    pub round_trip_executor_net_profit_non_positive_total: AtomicU64,
    /// Phase A.3.c — REVM succeeded but the orchestrator's anti-fraud
    /// guards rejected the result (zero gas or empty trace_hash). Non-
    /// zero indicates a SimulatorV2 contract change that needs review.
    pub round_trip_executor_antifraud_rejected_total: AtomicU64,
    /// Phase A.3.c — scanner-side counter for `tokio::task::spawn_blocking`
    /// returning `JoinError` (panic inside the orchestrator). Should
    /// always be zero.
    pub round_trip_executor_spawn_blocking_failed_total: AtomicU64,
}

/// Per-chain counter registry. Lazy-initialises `Arc<ScannerCounters>` on
/// first access for a given `chain_id`. Subsequent calls return the same
/// Arc. Lock-free atomics inside; the RwLock guards only the HashMap
/// (write only on first access per chain — read-mostly thereafter).
static REGISTRY: Lazy<RwLock<HashMap<u64, Arc<ScannerCounters>>>> =
    Lazy::new(|| RwLock::new(HashMap::new()));

/// Per-chain counter accessor. Each `run_chain(chain_id)` task should call
/// `chain_counters(chain_id)` and write only to its own counters. Heartbeat
/// reads per-chain via the same call.
pub fn chain_counters(chain_id: u64) -> Arc<ScannerCounters> {
    // Fast path: read lock, return existing Arc.
    if let Some(c) = REGISTRY.read().ok().and_then(|m| m.get(&chain_id).cloned()) {
        return c;
    }
    // Slow path: write lock, insert if still missing (another thread may have
    // raced us and inserted between read drop and write acquire).
    let mut map = REGISTRY.write().expect("counters registry write lock poisoned");
    map.entry(chain_id)
        .or_insert_with(|| Arc::new(ScannerCounters::default()))
        .clone()
}

/// Legacy backward-compat alias for code paths not yet migrated to per-chain.
/// Writes to `chain_counters(LEGACY_CHAIN_ID)` — a designated "unscoped"
/// bucket reported separately in heartbeat aggregation. New code MUST call
/// `chain_counters(chain_id)` instead.
pub fn counters() -> Arc<ScannerCounters> {
    chain_counters(LEGACY_CHAIN_ID)
}

/// List of chain ids that have at least one counter Arc allocated. Used by
/// aggregated readers to know which chains to walk without globally
/// summing (which would mutate counters via swap). Consumed by B0.5
/// readiness/agents endpoints to enumerate per-chain counter buckets.
#[allow(dead_code)] // Used by heartbeat aggregator + future B0.5 endpoint wire
pub fn registered_chain_ids() -> Vec<u64> {
    REGISTRY
        .read()
        .map(|m| {
            let mut ids: Vec<u64> = m.keys().copied().collect();
            ids.sort_unstable();
            ids
        })
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::Ordering;

    #[test]
    fn counters_legacy_alias_returns_same_arc() {
        // The legacy `counters()` must consistently point to the same chain-0
        // bucket so existing call sites still see their increments.
        let a = counters();
        let b = counters();
        assert!(Arc::ptr_eq(&a, &b), "counters() must return the same Arc");
    }

    #[test]
    fn counters_are_isolated_per_chain() {
        // CORE B0.1 contract: writes to chain X must NOT affect chain Y.
        let c1 = chain_counters(1);
        let c137 = chain_counters(137);
        // They must be distinct allocations.
        assert!(
            !Arc::ptr_eq(&c1, &c137),
            "chain 1 and chain 137 counters must be different Arc instances"
        );
        let base1 = c1.pending_received.load(Ordering::Relaxed);
        let base137 = c137.pending_received.load(Ordering::Relaxed);
        c1.pending_received.fetch_add(7, Ordering::Relaxed);
        assert_eq!(c1.pending_received.load(Ordering::Relaxed), base1 + 7);
        assert_eq!(
            c137.pending_received.load(Ordering::Relaxed),
            base137,
            "writing to chain 1 must NOT affect chain 137 counters"
        );
    }

    #[test]
    fn chain_1_does_not_affect_chain_137() {
        // Same contract, named to match the SOP's required test name.
        let c1 = chain_counters(1);
        let c137 = chain_counters(137);
        let snapshot_137_before = c137.gate_token_not_allowed.load(Ordering::Relaxed);
        c1.gate_token_not_allowed.fetch_add(42, Ordering::Relaxed);
        assert_eq!(
            c137.gate_token_not_allowed.load(Ordering::Relaxed),
            snapshot_137_before,
            "chain 1 increment leaked into chain 137"
        );
    }

    #[test]
    fn registered_chain_ids_includes_accessed_chains() {
        // After accessing chains 1 and 137, both should appear in the registry.
        let _ = chain_counters(1);
        let _ = chain_counters(137);
        let ids = registered_chain_ids();
        assert!(ids.contains(&1), "chain 1 missing from registry: {ids:?}");
        assert!(ids.contains(&137), "chain 137 missing from registry: {ids:?}");
    }

    #[test]
    fn legacy_counters_writes_to_chain_zero_bucket() {
        // The legacy `counters()` API maps to chain 0 (LEGACY_CHAIN_ID).
        // Code that has not been migrated still works but accumulates into
        // the "unscoped" bucket, which heartbeat reports separately.
        let legacy = counters();
        let chain0 = chain_counters(LEGACY_CHAIN_ID);
        assert!(
            Arc::ptr_eq(&legacy, &chain0),
            "counters() must alias chain_counters(LEGACY_CHAIN_ID)"
        );
    }

    #[test]
    fn counters_increment_and_swap_work() {
        // Use a fresh local ScannerCounters to avoid contamination from
        // other tests (since the registry is process-global).
        let local = ScannerCounters::default();
        local.pending_received.fetch_add(7, Ordering::Relaxed);
        local.pending_received.fetch_add(3, Ordering::Relaxed);
        let snapshot = local.pending_received.swap(0, Ordering::Relaxed);
        assert_eq!(snapshot, 10);
        assert_eq!(local.pending_received.load(Ordering::Relaxed), 0);
    }
}
