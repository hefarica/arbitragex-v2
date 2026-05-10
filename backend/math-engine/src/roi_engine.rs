//! Motor de ROI (Return on Investment) para Arbitraje DeFi
//!
//! Todas las funciones calculan rentabilidad neta tras deducir:
//! - Gas estimado (en USD).
//! - Comisiones de Flashloan.
//! - Protocol fees (DEX fees, ya descontados usualmente en amount_out).
//! - Slippage/Failure buffers.
//! - [Sprint A] LP fees explícitas, price impact real, capital cost, ops overhead.

use crate::DefiArbitrageOutcome;

/// Parámetros necesarios para calcular la rentabilidad neta final.
///
/// ### Sprint A additions (components 2, 3, 6, 7)
/// - `lp_fees_usd`: Explicit LP fee sum aggregated across route hops.
///   Caller computes: `Σ(amount_through_leg_usd × fee_bps / 10_000)`.
///   Pass `0.0` when route leg detail is unavailable — conservative
///   assumption is that DEX fees are already reflected in `expected_amount_out`.
///
/// - `price_impact_pct`: Real price-impact percentage from pool reserves (V2).
///   When `> 0.0`, replaces `max_slippage_pct` as the slippage cost basis.
///   When `== 0.0`, the engine falls back to `max_slippage_pct` (proxy).
///   V3 callers: pass `0.0` until tick-math is wired; see follow-up note below.
///
/// - `capital_cost_usd`: Opportunity cost of capital locked during execution.
///   Formula: `amount_in_usd × (rate_annual_pct / 100.0) × (block_time_s / 31_536_000.0)`.
///   Flash-loan strategies: caller passes `0.0` (capital is atomically borrowed
///   and returned in same tx; no lock-up period).
///
/// - `ops_overhead_usd`: Amortised infra/server cost per attempt.
///   Operator-configured; default `$0.01`. Passes through unchanged from config.
///
/// ### Sprint B additions (component 4 — statistical failure probability)
/// - `p_fail`: Statistical failure probability from `strategy_scores` table.
///   `Some(p)` → failure buffer = `p × expected_gas_cost_usd` (expected loss
///   from reverts — a reverted tx still burns gas).
///   `None` → fall back to legacy proxy: `amount_in_usd × failure_risk_buffer_pct`.
///   Source: `recon` learning loop writes `revert_rate` per (strategy_kind,
///   chain_id, window); spine pre-fetches the latest row with `sample_count ≥ 10`.
///   `None` also applies at cold-start or when no row exists yet (R8 fail-honest).
///
/// ### Sprint C additions (component 5 — copy-trade / front-run probability)
/// - `p_copied`: Heuristic probability that this opportunity will be copied or
///   front-run by competing searchers, sourced from `dex_chain_metrics.volume_24h_usd`
///   for the weakest-volume pool in the route (the "weakest-link" competition floor).
///   Formula: `p = min(p_copied_max, log10(vol / threshold) × 0.1).max(0.0)`.
///   `Some(p)` → cost = `p × gross_profit_usd_proxy` (deducted from net profit).
///   `None` → no `dex_chain_metrics` row for the pool; cost NOT added (R8 fail-honest).
///   The cap (`p_copied_max`, default 0.5) is applied by the CALLER before passing
///   `p_copied` here — the math engine applies the value as-is without re-clamping.
///
/// ### Follow-up items (NOT in Sprint A)
/// - V3 real price impact (tick-math): `price_impact_pct` will be populated with
///   actual V3 impact when the tick-traversal helper lands in Sprint B.
/// - Per-chain block time for `capital_cost_usd`: currently ETH (12s) is the only
///   value used by `config_aware.rs`. A `block_time_s_per_chain()` helper will
///   replace the constant in Sprint B to support ARB/Base/OP (2s).
#[derive(Debug, Clone)]
pub struct RoiCalculationParams {
    pub amount_in_usd: f64,
    pub expected_amount_out_usd: f64,
    pub expected_gas_cost_usd: f64,
    pub flashloan_fee_pct: f64,
    pub max_slippage_pct: f64,
    /// Legacy flat proxy for failure buffer (Sprint A).
    /// Used when `p_fail` is `None`. Formula: `amount_in_usd × failure_risk_buffer_pct`.
    /// Set by the caller from `config.failure_risk_buffer_pct × amount_in_usd`.
    pub failure_risk_buffer_usd: f64,

    // --- Component 2: Explicit LP fees across route hops ---
    /// Sum of `amount_through_leg_usd × fee_bps_leg / 10_000` for all legs.
    /// `0.0` is safe when unavailable — fees are then implicitly absorbed by
    /// the spread between `amount_in_usd` and `expected_amount_out_usd`.
    pub lp_fees_usd: f64,

    // --- Component 3: Real V2 price impact (replaces max_slippage proxy) ---
    /// Price-impact percentage (0.0–100.0 scale) from pool reserves.
    /// `calc_univ2_price_impact()` in `amm_math.rs` returns this as a fraction
    /// (0.0–1.0); multiply by 100.0 before storing here.
    /// When `0.0`, the engine falls back to `max_slippage_pct`.
    pub price_impact_pct: f64,

    // --- Component 6: Opportunity cost of capital ---
    /// `amount_in_usd × (annual_rate / 100) × (block_time_s / 31_536_000)`.
    /// Caller sets to `0.0` for flash-loan strategies (no lock-up).
    pub capital_cost_usd: f64,

    // --- Component 7: Amortised ops/infra overhead ---
    /// Per-attempt infra cost ($0.01 default from `trading_config`).
    pub ops_overhead_usd: f64,

    // --- Component 4 (Sprint B): Statistical failure probability ---
    /// Pre-fetched revert probability from `strategy_scores` (0.0–1.0).
    /// `Some(p)` → `failure_risk_buffer_usd` is IGNORED; buffer computed as
    ///              `p × expected_gas_cost_usd` (expected gas burned on reverts).
    /// `None`    → falls back to `failure_risk_buffer_usd` proxy (Sprint A).
    ///
    /// Sourced per `(strategy_kind, chain_id)` from the learning loop; caller
    /// (spine entry point) pre-fetches once per tick and passes here.
    /// R8: `None` when `sample_count < 10` or no row exists — never invent.
    pub p_fail: Option<f64>,

    // --- Component 5 (Sprint C): copy-trade / front-run probability ---
    /// Pre-computed copy-trade probability from the weakest pool's 24h volume.
    /// Caller applies the `p_copied_max` cap before passing; the math engine
    /// treats the value as final (no re-clamping here).
    ///
    /// `Some(p)` → `copied_buffer_usd = p × gross_profit_usd_proxy`
    ///             where `gross_profit_usd_proxy = expected_amount_out_usd - amount_in_usd`
    ///             (pre-cost gross). Deducted from net profit.
    /// `None`    → no `dex_chain_metrics` row for the pool; no cost added (R8).
    pub p_copied: Option<f64>,

    // --- Component 8 (C2 fix, audit re-run #2 2026-05-10): relay bribe ---
    /// Estimated relay/builder bribe in USD (the `coinbaseDiff` the searcher
    /// must pay to win bundle inclusion on Ethereum mainnet).
    ///
    /// Sourced from the EWMA of observed `coinbase_diff_wei` values stored in
    /// Redis key `arbx:relay_fee_ewma:{chain_id}:{strategy_kind}` and converted
    /// to USD using the current ETH price. Cold-start (absent key): caller
    /// applies the doctrine floor `max(gross × 0.05, $0.50)` before passing
    /// here. The math engine deducts this value as-is.
    ///
    /// Ethereum mainnet only: L2 sequencer inclusion is deterministic; passing
    /// `0.0` for L2 chains is correct. Callers determine the chain.
    ///
    /// This is component 8 in the cost model. Previously absent, it caused a
    /// systematic 10-50% overstatement of net profit on mainnet.
    pub estimated_relay_fee_usd: f64,
}

/// Calcula el ROI bruto y neto de una oportunidad DeFi (Estrategia General).
///
/// ### Net profit formula (Sprint A + B + C + C2)
/// ```text
/// net_profit =
///     expected_amount_out_usd
///   - amount_in_usd
///   - expected_gas_cost_usd        // component 1 (existing)
///   - flashloan_fee_usd            // existing: flashloan_fee_pct × amount_in
///   - lp_fees_usd                  // component 2 (Sprint A)
///   - effective_slippage_usd       // component 3 (Sprint A: price_impact or max_slippage)
///   - failure_cost_usd             // component 4 (Sprint B: statistical or proxy)
///   - capital_cost_usd             // component 6 (Sprint A)
///   - ops_overhead_usd             // component 7 (Sprint A)
///   - copied_buffer_usd            // component 5 (Sprint C: p_copied × gross_proxy)
///   - estimated_relay_fee_usd      // component 8 (C2 fix: Flashbots coinbaseDiff EWMA)
/// ```
pub fn calc_net_profit_and_roi(params: &RoiCalculationParams) -> DefiArbitrageOutcome {
    let gross_profit_usd = params.expected_amount_out_usd - params.amount_in_usd;

    // Flash-loan fee: percentage of borrowed capital.
    let flashloan_fee_usd = params.amount_in_usd * params.flashloan_fee_pct;

    // Component 3: use real price-impact when available (> 0), else fall back to
    // max_slippage_pct proxy. Both inputs express a fraction of expected_amount_out.
    let effective_slippage_usd = if params.price_impact_pct > 0.0 {
        params.expected_amount_out_usd * (params.price_impact_pct / 100.0)
    } else {
        params.expected_amount_out_usd * params.max_slippage_pct
    };

    // The slippage percentage stored in the outcome reflects the ACTUAL fraction
    // used for cost calculation so downstream evidence is honest.
    let effective_slippage_pct = if params.price_impact_pct > 0.0 {
        params.price_impact_pct / 100.0
    } else {
        params.max_slippage_pct
    };

    // Component 4 (Sprint B): statistical vs proxy failure buffer.
    //
    // Statistical path (p_fail = Some(p)):
    //   Expected loss from reverts = p × gas_cost.
    //   A reverted tx burns gas but yields no output; the expected cost is
    //   the probability-weighted gas expenditure.  The proxy
    //   (amount_in × buffer_pct) is a cruder capital-fraction approximation.
    //
    // Proxy path (p_fail = None):
    //   Use the pre-computed `failure_risk_buffer_usd` passed by the caller,
    //   which equals `amount_in_usd × config.failure_risk_buffer_pct`.
    //   Preserves exact Sprint A behavior for cold-start / new strategies.
    let failure_cost_usd = match params.p_fail {
        Some(p) => p * params.expected_gas_cost_usd,
        None => params.failure_risk_buffer_usd,
    };

    // Component 5 (Sprint C): copy-trade / front-run probability buffer.
    //
    // `gross_profit_usd` is the pre-cost gross spread; the heuristic assumes
    // that in the worst case the competing searcher captures the full gross
    // profit if they copy-trade. Expected loss = p × gross.
    //
    // `None` → R8 fail-honest: no dex_chain_metrics row; zero cost added.
    let copied_buffer_usd = match params.p_copied {
        Some(p) => p * gross_profit_usd,
        None => 0.0,
    };

    // Full deterministic net profit deducting all components (Sprints A + B + C + C2).
    let net_profit_usd = params.expected_amount_out_usd
        - params.amount_in_usd
        - params.expected_gas_cost_usd          // component 1
        - flashloan_fee_usd                     // component from flashloan_fee_pct
        - params.lp_fees_usd                    // component 2 (Sprint A)
        - effective_slippage_usd                // component 3 (Sprint A)
        - failure_cost_usd                      // component 4 (Sprint B: statistical or proxy)
        - params.capital_cost_usd               // component 6 (Sprint A)
        - params.ops_overhead_usd               // component 7 (Sprint A)
        - copied_buffer_usd                     // component 5 (Sprint C)
        - params.estimated_relay_fee_usd;       // component 8 (C2 fix: relay bribe EWMA)

    let capital_required = params.amount_in_usd;
    let net_roi_pct = if capital_required > 0.0 {
        (net_profit_usd / capital_required) * 100.0
    } else {
        0.0
    };

    let is_viable = net_profit_usd > 0.0;

    let opportunity_score = if is_viable {
        let base = 50.0;
        let bonus = (net_roi_pct / 0.1) * 5.0;
        (base + bonus).min(100.0)
    } else {
        0.0
    };

    DefiArbitrageOutcome {
        is_viable,
        gross_profit_usd,
        net_profit_usd,
        expected_amount_out: params.expected_amount_out_usd,
        gas_cost_usd: params.expected_gas_cost_usd,
        flashloan_fee_usd,
        slippage_expected_pct: effective_slippage_pct,
        total_capital_required_usd: params.amount_in_usd,
        net_roi_pct,
        opportunity_score,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ----------------------------------------------------------------
    // Helper: baseline params with all Sprint A components zero.
    // ----------------------------------------------------------------
    fn baseline() -> RoiCalculationParams {
        RoiCalculationParams {
            amount_in_usd: 10_000.0,
            expected_amount_out_usd: 10_050.0, // $50 gross
            expected_gas_cost_usd: 5.0,
            flashloan_fee_pct: 0.0009, // 0.09% → $9
            max_slippage_pct: 0.001,   // 0.1%  → $10.05
            failure_risk_buffer_usd: 1.0,
            // Sprint A — all zero by default
            lp_fees_usd: 0.0,
            price_impact_pct: 0.0,
            capital_cost_usd: 0.0,
            ops_overhead_usd: 0.0,
            // Sprint B — None → proxy fallback preserved
            p_fail: None,
            // Sprint C — None → no copy-trade cost
            p_copied: None,
            // C2 — zero by default (relay fee not yet wired at baseline)
            estimated_relay_fee_usd: 0.0,
        }
    }

    // ----------------------------------------------------------------
    // 1. Regression guard: all new components zero → same result as before.
    // ----------------------------------------------------------------
    #[test]
    fn all_zero_new_components_match_legacy_behavior() {
        let params = baseline();
        let result = calc_net_profit_and_roi(&params);

        // flashloan_fee = 10_000 × 0.0009 = $9.0
        // slippage      = 10_050 × 0.001  = $10.05
        // net           = 10_050 - 10_000 - 5.0 - 9.0 - 10.05 - 1.0 = $24.95
        assert!(result.is_viable);
        assert!(
            (result.net_profit_usd - 24.95).abs() < 0.001,
            "legacy net_profit expected $24.95, got {}",
            result.net_profit_usd,
        );
        assert!(
            (result.net_roi_pct - 0.2495).abs() < 0.001,
            "legacy net_roi_pct expected 0.2495%, got {}",
            result.net_roi_pct,
        );
    }

    // ----------------------------------------------------------------
    // 2. Non-zero lp_fees_usd reduces net profit by exact amount.
    // ----------------------------------------------------------------
    #[test]
    fn nonzero_lp_fees_reduces_net_profit() {
        let base_net = calc_net_profit_and_roi(&baseline()).net_profit_usd;
        let mut params = baseline();
        params.lp_fees_usd = 3.50;
        let result = calc_net_profit_and_roi(&params);
        // Net must be exactly $3.50 less than the baseline.
        assert!(
            (result.net_profit_usd - (base_net - 3.50)).abs() < 1e-9,
            "lp_fees_usd=$3.50 should reduce net by exactly $3.50; \
             base={} new={} expected={}",
            base_net,
            result.net_profit_usd,
            base_net - 3.50,
        );
    }

    // ----------------------------------------------------------------
    // 3. price_impact_pct > 0 overrides max_slippage_pct.
    // ----------------------------------------------------------------
    #[test]
    fn price_impact_pct_overrides_max_slippage_when_positive() {
        // Baseline slippage_cost = 10_050 × 0.001 = $10.05
        // With price_impact_pct = 2.0: slippage_cost = 10_050 × 0.02 = $201.00
        // The difference in net profit should be exactly $201 - $10.05 = $190.95
        let base_net = calc_net_profit_and_roi(&baseline()).net_profit_usd;
        let mut params = baseline();
        params.price_impact_pct = 2.0; // 2% price impact
        let result = calc_net_profit_and_roi(&params);

        let expected_slippage_with_impact = 10_050.0 * 0.02; // $201.00
        let expected_slippage_proxy = 10_050.0 * 0.001;      // $10.05
        let expected_net = base_net - (expected_slippage_with_impact - expected_slippage_proxy);

        assert!(
            (result.net_profit_usd - expected_net).abs() < 1e-6,
            "price_impact override: expected net={:.6} got {:.6}",
            expected_net,
            result.net_profit_usd,
        );
        // Outcome reflects the real impact fraction, not the proxy.
        assert!(
            (result.slippage_expected_pct - 0.02).abs() < 1e-9,
            "slippage_expected_pct should equal price_impact_pct/100 when impact > 0"
        );
    }

    // ----------------------------------------------------------------
    // 4. price_impact_pct == 0 falls back to max_slippage_pct proxy.
    // ----------------------------------------------------------------
    #[test]
    fn zero_price_impact_uses_max_slippage_proxy() {
        let params = baseline(); // price_impact_pct = 0.0
        let result = calc_net_profit_and_roi(&params);
        // slippage_expected_pct must equal max_slippage_pct when no real impact.
        assert!(
            (result.slippage_expected_pct - 0.001).abs() < 1e-9,
            "fallback slippage expected 0.001, got {}",
            result.slippage_expected_pct,
        );
    }

    // ----------------------------------------------------------------
    // 5. capital_cost_usd > 0 for non-flash reduces net profit.
    // ----------------------------------------------------------------
    #[test]
    fn nonzero_capital_cost_reduces_net_profit() {
        // Simulate ETH mainnet, 0.5% APR, 12s block:
        // capital_cost = 10_000 × 0.005 × (12 / 31_536_000) ≈ $0.0000019
        // Use a rounded observable value to make the assertion legible.
        let base_net = calc_net_profit_and_roi(&baseline()).net_profit_usd;
        let capital_cost = 0.05; // $0.05 for readability
        let mut params = baseline();
        params.capital_cost_usd = capital_cost;
        let result = calc_net_profit_and_roi(&params);
        assert!(
            (result.net_profit_usd - (base_net - capital_cost)).abs() < 1e-9,
            "capital_cost_usd=${} should reduce net by exact amount",
            capital_cost,
        );
    }

    // ----------------------------------------------------------------
    // 6. ops_overhead_usd = $0.01 reduces net profit by exactly $0.01.
    // ----------------------------------------------------------------
    #[test]
    fn ops_overhead_reduces_net_profit_by_default_value() {
        let base_net = calc_net_profit_and_roi(&baseline()).net_profit_usd;
        let mut params = baseline();
        params.ops_overhead_usd = 0.01;
        let result = calc_net_profit_and_roi(&params);
        assert!(
            (result.net_profit_usd - (base_net - 0.01)).abs() < 1e-9,
            "ops_overhead_usd=$0.01 should reduce net by exactly $0.01; \
             base={} new={}",
            base_net,
            result.net_profit_usd,
        );
    }

    // ----------------------------------------------------------------
    // 7. Combined: exact arithmetic check.
    //    gross=$100, gas=$2, flashloan=$0, lp=$1, slippage(impact)=$1.10,
    //    failure=$0, capital=$0.50, ops=$0.01  →  net=$95.39
    // ----------------------------------------------------------------
    #[test]
    fn combined_all_components_exact_arithmetic() {
        // amount_in = $1000, expected_out = $1100 → gross = $100
        // gas = $2.00
        // flashloan_fee_pct = 0.0 → fee = $0
        // lp_fees_usd = $1.00
        // price_impact_pct = 0.1 → slippage_cost = $1100 × 0.001 = $1.10
        // failure_risk_buffer = $0.00
        // capital_cost_usd = $0.50
        // ops_overhead_usd  = $0.01
        // net = $100 - $2 - $0 - $1 - $1.10 - $0 - $0.50 - $0.01 = $95.39
        let params = RoiCalculationParams {
            amount_in_usd: 1_000.0,
            expected_amount_out_usd: 1_100.0,
            expected_gas_cost_usd: 2.0,
            flashloan_fee_pct: 0.0,
            max_slippage_pct: 0.005, // unused — price_impact takes over
            failure_risk_buffer_usd: 0.0,
            lp_fees_usd: 1.0,
            price_impact_pct: 0.1, // 0.1% → $1100 × 0.001 = $1.10
            capital_cost_usd: 0.50,
            ops_overhead_usd: 0.01,
            p_fail: None,
            p_copied: None, // Sprint C: no copy-trade cost in this combined test
            estimated_relay_fee_usd: 0.0, // C2: zero so legacy arithmetic is preserved
        };
        let result = calc_net_profit_and_roi(&params);

        // gross = $100
        assert!((result.gross_profit_usd - 100.0).abs() < 1e-9, "gross={}", result.gross_profit_usd);

        // net = 1100 - 1000 - 2 - 0 - 1 - 1.10 - 0 - 0.50 - 0.01 = $95.39
        let expected_net = 1_100.0 - 1_000.0 - 2.0 - 0.0 - 1.0 - 1.10 - 0.0 - 0.50 - 0.01;
        assert!(
            (result.net_profit_usd - expected_net).abs() < 1e-6,
            "combined net expected {:.6} got {:.6}",
            expected_net,
            result.net_profit_usd,
        );
        assert!(result.is_viable, "combined $95.39 net must be viable");
    }

    // ----------------------------------------------------------------
    // Legacy tests (preserved exactly — regression guard)
    // ----------------------------------------------------------------
    #[test]
    fn test_roi_calculation_profitable() {
        let params = RoiCalculationParams {
            amount_in_usd: 10_000.0,
            expected_amount_out_usd: 10_050.0,
            expected_gas_cost_usd: 5.0,
            flashloan_fee_pct: 0.0009,
            max_slippage_pct: 0.001,
            failure_risk_buffer_usd: 1.0,
            lp_fees_usd: 0.0,
            price_impact_pct: 0.0,
            capital_cost_usd: 0.0,
            ops_overhead_usd: 0.0,
            p_fail: None,
            p_copied: None,
            estimated_relay_fee_usd: 0.0,
        };

        let result = calc_net_profit_and_roi(&params);

        // flashloan_fee = 10_000 * 0.0009 = $9.0
        // slippage buffer = 10_050 * 0.001 = $10.05
        // net profit = 10_050 - 10_000 - 5.0 - 9.0 - 10.05 - 1.0 = $24.95
        assert!(result.is_viable);
        assert!((result.net_profit_usd - 24.95).abs() < 0.001);
        assert!((result.net_roi_pct - 0.2495).abs() < 0.001);
    }

    // ----------------------------------------------------------------
    // Sprint B: p_fail statistical integration tests (Component 4)
    // ----------------------------------------------------------------

    /// p_fail = None → proxy fallback used; result identical to baseline
    /// (Sprint A behavior preserved).
    #[test]
    fn p_fail_none_uses_proxy_fallback() {
        // baseline() already has p_fail: None and failure_risk_buffer_usd: 1.0
        let params = baseline();
        let result = calc_net_profit_and_roi(&params);
        // net = 10_050 - 10_000 - 5 - 9 - 10.05 - 1.0 = $24.95
        assert!(
            (result.net_profit_usd - 24.95).abs() < 0.001,
            "p_fail=None must use proxy; expected net=$24.95, got {}",
            result.net_profit_usd,
        );
    }

    /// p_fail = Some(0.0) → zero failure cost; no buffer subtracted.
    /// net must be exactly `failure_risk_buffer_usd` ($1.0) MORE than baseline.
    #[test]
    fn p_fail_zero_adds_no_failure_cost() {
        let base_net = calc_net_profit_and_roi(&baseline()).net_profit_usd; // $24.95
        let mut params = baseline();
        params.p_fail = Some(0.0);
        let result = calc_net_profit_and_roi(&params);
        // Statistical buffer = 0.0 × $5 gas = $0; proxy was $1.0; diff = +$1.0
        assert!(
            (result.net_profit_usd - (base_net + 1.0)).abs() < 1e-9,
            "p_fail=0.0 should add $0 failure cost; expected {} got {}",
            base_net + 1.0,
            result.net_profit_usd,
        );
    }

    /// p_fail = Some(0.1) → buffer = 0.1 × gas_cost_usd = 0.1 × $5 = $0.50.
    /// Net must be exactly baseline + proxy_buffer($1.0) - statistical_buffer($0.50).
    #[test]
    fn p_fail_ten_pct_costs_fraction_of_gas() {
        let base_net = calc_net_profit_and_roi(&baseline()).net_profit_usd; // $24.95
        let mut params = baseline();
        params.p_fail = Some(0.1);
        let result = calc_net_profit_and_roi(&params);
        // statistical buffer = 0.1 × $5 = $0.50; proxy was $1.0 → net gains $0.50
        let expected_net = base_net + 1.0 - 0.50; // = $25.45
        assert!(
            (result.net_profit_usd - expected_net).abs() < 1e-9,
            "p_fail=0.1 → buffer=$0.50; expected net={} got {}",
            expected_net,
            result.net_profit_usd,
        );
    }

    /// p_fail = Some(1.0) → buffer = 1.0 × gas = $5.  Every tx reverts.
    /// Also tests that with very thin spread the opportunity becomes non-viable.
    #[test]
    fn p_fail_one_costs_full_gas_and_may_kill_opportunity() {
        // Build params where p_fail=1.0 makes net clearly negative.
        // amount_in=$100, out=$101 (gross=$1), gas=$5, p_fail=1.0 → buffer=$5
        // net = 1 - 5 - 5 = -$9 (negative → not viable)
        let params = RoiCalculationParams {
            amount_in_usd: 100.0,
            expected_amount_out_usd: 101.0,
            expected_gas_cost_usd: 5.0,
            flashloan_fee_pct: 0.0,
            max_slippage_pct: 0.0,
            failure_risk_buffer_usd: 0.0, // proxy is 0; only statistical matters
            lp_fees_usd: 0.0,
            price_impact_pct: 0.0,
            capital_cost_usd: 0.0,
            ops_overhead_usd: 0.0,
            p_fail: Some(1.0),
            p_copied: None,
            estimated_relay_fee_usd: 0.0,
        };
        let result = calc_net_profit_and_roi(&params);
        // statistical buffer = 1.0 × $5 = $5; net = 101 - 100 - 5 - 5 = -$9
        assert!(
            !result.is_viable,
            "p_fail=1.0 should make this opportunity non-viable, got is_viable=true"
        );
        assert!(
            result.net_profit_usd < 0.0,
            "p_fail=1.0 should produce negative net profit, got {}",
            result.net_profit_usd,
        );
        // Exact: net = 1 - 5 - 5 = -9
        assert!(
            (result.net_profit_usd - (-9.0)).abs() < 1e-9,
            "p_fail=1.0 exact net expected -$9, got {}",
            result.net_profit_usd,
        );
    }

    #[test]
    fn test_roi_calculation_not_profitable() {
        let params = RoiCalculationParams {
            amount_in_usd: 10_000.0,
            expected_amount_out_usd: 10_010.0,
            expected_gas_cost_usd: 15.0,
            flashloan_fee_pct: 0.0009,
            max_slippage_pct: 0.0,
            failure_risk_buffer_usd: 0.0,
            lp_fees_usd: 0.0,
            price_impact_pct: 0.0,
            capital_cost_usd: 0.0,
            ops_overhead_usd: 0.0,
            p_fail: None,
            p_copied: None,
            estimated_relay_fee_usd: 0.0,
        };

        let result = calc_net_profit_and_roi(&params);
        assert!(!result.is_viable);
        assert!(result.net_profit_usd < 0.0);
    }

    // ----------------------------------------------------------------
    // Sprint C: p_copied heuristic tests (Component 5)
    // ----------------------------------------------------------------

    /// p_copied = None → no copy-trade buffer added; net identical to baseline.
    #[test]
    fn p_copied_none_no_cost_added() {
        let params = baseline(); // p_copied: None
        let result = calc_net_profit_and_roi(&params);
        // net = 10_050 - 10_000 - 5 - 9 - 10.05 - 1.0 = $24.95 (same as legacy)
        assert!(
            (result.net_profit_usd - 24.95).abs() < 0.001,
            "p_copied=None should add zero cost; expected net=$24.95, got {}",
            result.net_profit_usd,
        );
    }

    /// p_copied = Some(0.0) → zero copy-trade buffer; net identical to None path.
    #[test]
    fn p_copied_zero_no_cost_added() {
        let base_net = calc_net_profit_and_roi(&baseline()).net_profit_usd;
        let mut params = baseline();
        params.p_copied = Some(0.0);
        let result = calc_net_profit_and_roi(&params);
        // copied_buffer = 0.0 × gross → $0; no change from None baseline
        assert!(
            (result.net_profit_usd - base_net).abs() < 1e-9,
            "p_copied=0.0 should add $0 cost; expected net={} got {}",
            base_net,
            result.net_profit_usd,
        );
    }

    /// p_copied = Some(0.1) with $100 gross → copied_buffer = $10; net reduced by $10.
    #[test]
    fn p_copied_ten_pct_reduces_net_by_fraction_of_gross() {
        // Clean setup: $1000 in, $1100 out ($100 gross), no other costs.
        let params = RoiCalculationParams {
            amount_in_usd: 1_000.0,
            expected_amount_out_usd: 1_100.0,
            expected_gas_cost_usd: 0.0,
            flashloan_fee_pct: 0.0,
            max_slippage_pct: 0.0,
            failure_risk_buffer_usd: 0.0,
            lp_fees_usd: 0.0,
            price_impact_pct: 0.0,
            capital_cost_usd: 0.0,
            ops_overhead_usd: 0.0,
            p_fail: None,
            p_copied: Some(0.1),
            estimated_relay_fee_usd: 0.0,
        };
        let result = calc_net_profit_and_roi(&params);
        // gross = $100; copied_buffer = 0.1 × $100 = $10; net = $100 - $10 = $90
        assert!(
            (result.net_profit_usd - 90.0).abs() < 1e-9,
            "p_copied=0.1, gross=$100 → buffer=$10, net expected $90, got {}",
            result.net_profit_usd,
        );
    }

    /// When the caller passes p_copied > p_copied_max, the CALLER must cap it.
    /// This test verifies that the math engine applies the value as-is —
    /// here we pass Some(0.7) and expect 70% of gross to be deducted (no re-clamping
    /// inside the engine). The caller (config_aware.rs) is responsible for clamping.
    #[test]
    fn p_copied_capped_at_max_by_caller() {
        // $1000 in, $1100 out ($100 gross), no other costs.
        // Caller should clamp to config p_copied_max=0.5; here we pass 0.7 to
        // verify the engine uses it verbatim (no silent re-clamping inside engine).
        let params = RoiCalculationParams {
            amount_in_usd: 1_000.0,
            expected_amount_out_usd: 1_100.0,
            expected_gas_cost_usd: 0.0,
            flashloan_fee_pct: 0.0,
            max_slippage_pct: 0.0,
            failure_risk_buffer_usd: 0.0,
            lp_fees_usd: 0.0,
            price_impact_pct: 0.0,
            capital_cost_usd: 0.0,
            ops_overhead_usd: 0.0,
            p_fail: None,
            p_copied: Some(0.7), // engine applies 70% as-is
            estimated_relay_fee_usd: 0.0,
        };
        let result = calc_net_profit_and_roi(&params);
        // copied_buffer = 0.7 × $100 = $70; net = $100 - $70 = $30
        assert!(
            (result.net_profit_usd - 30.0).abs() < 1e-9,
            "engine passes p_copied=0.7 verbatim: buffer=$70, net expected $30, got {}",
            result.net_profit_usd,
        );
    }

    /// Combined Sprint A + B + C: all components active with exact arithmetic.
    ///
    /// Setup:
    ///   amount_in   = $1000, expected_out = $1100 → gross = $100
    ///   gas         = $2.00
    ///   flashloan   = 0%    → $0
    ///   lp_fees     = $1.00  (component 2)
    ///   price_impact = 0.1% → $1100 × 0.001 = $1.10  (component 3)
    ///   p_fail      = Some(0.1) → 0.1 × $2 gas = $0.20  (component 4)
    ///   capital_cost = $0.50  (component 6)
    ///   ops_overhead = $0.01  (component 7)
    ///   p_copied    = Some(0.1) → 0.1 × $100 gross = $10.00  (component 5)
    ///
    ///   net = $100 - $2 - $0 - $1 - $1.10 - $0.20 - $0.50 - $0.01 - $10.00 = $85.19
    #[test]
    fn combined_sprint_abc_all_components_exact_arithmetic() {
        let params = RoiCalculationParams {
            amount_in_usd: 1_000.0,
            expected_amount_out_usd: 1_100.0,
            expected_gas_cost_usd: 2.0,
            flashloan_fee_pct: 0.0,
            max_slippage_pct: 0.005, // unused — price_impact takes over
            failure_risk_buffer_usd: 0.0, // unused — p_fail takes over
            lp_fees_usd: 1.0,
            price_impact_pct: 0.1,   // 0.1% → $1100 × 0.001 = $1.10
            capital_cost_usd: 0.50,
            ops_overhead_usd: 0.01,
            p_fail: Some(0.1),       // 0.1 × $2 gas = $0.20
            p_copied: Some(0.1),     // 0.1 × $100 gross = $10.00
            estimated_relay_fee_usd: 0.0, // C2: zero to preserve legacy arithmetic
        };
        let result = calc_net_profit_and_roi(&params);

        assert!((result.gross_profit_usd - 100.0).abs() < 1e-9, "gross={}", result.gross_profit_usd);

        // Exact formula:
        // 1100 - 1000            = $100 gross
        // - $2.00 gas            → component 1
        // - $0.00 flashloan      → component (flashloan_fee_pct=0)
        // - $1.00 lp_fees        → component 2
        // - $1.10 slippage       → component 3 (1100 × 0.001)
        // - $0.20 failure        → component 4 (0.1 × $2 gas)
        // - $0.50 capital_cost   → component 6
        // - $0.01 ops_overhead   → component 7
        // - $10.00 copied_buffer → component 5 (0.1 × $100 gross)
        //                        = $85.19
        let expected_net = 1_100.0 - 1_000.0
            - 2.0       // gas
            - 0.0       // flashloan
            - 1.0       // lp_fees
            - 1.10      // slippage (1100 × 0.001)
            - 0.20      // failure (0.1 × 2.0)
            - 0.50      // capital_cost
            - 0.01      // ops_overhead
            - 10.00;    // copied_buffer (0.1 × 100.0 gross)
        assert!(
            (result.net_profit_usd - expected_net).abs() < 1e-6,
            "Sprint A+B+C combined: expected net={:.6} got {:.6}",
            expected_net,
            result.net_profit_usd,
        );
        assert!(result.is_viable, "net=${} must be viable", result.net_profit_usd);
    }

    // ----------------------------------------------------------------
    // C2 fix (audit re-run #2 2026-05-10): relay_fee_usd integration tests
    // (Component 8 — Flashbots coinbaseDiff EWMA)
    // ----------------------------------------------------------------

    /// C2: non-zero relay_fee_usd reduces net profit by exactly that amount vs
    /// the same params with relay_fee=0.0.
    ///
    /// This is the canonical regression guard: a $5 relay bribe on a $100 gross
    /// opportunity reduces net profit by exactly $5 — no more, no less.
    #[test]
    fn c2_relay_fee_reduces_net_by_exact_amount() {
        // Baseline: $1000 in, $1100 out ($100 gross), no other costs.
        let base_params = RoiCalculationParams {
            amount_in_usd: 1_000.0,
            expected_amount_out_usd: 1_100.0,
            expected_gas_cost_usd: 0.0,
            flashloan_fee_pct: 0.0,
            max_slippage_pct: 0.0,
            failure_risk_buffer_usd: 0.0,
            lp_fees_usd: 0.0,
            price_impact_pct: 0.0,
            capital_cost_usd: 0.0,
            ops_overhead_usd: 0.0,
            p_fail: None,
            p_copied: None,
            estimated_relay_fee_usd: 0.0, // baseline: no relay fee
        };
        let base_result = calc_net_profit_and_roi(&base_params);

        // With relay fee of $5:
        let relay_fee_usd = 5.0_f64;
        let mut fee_params = base_params.clone();
        fee_params.estimated_relay_fee_usd = relay_fee_usd;
        let fee_result = calc_net_profit_and_roi(&fee_params);

        // Gross must be identical (relay fee is a cost, not a gross-reduction).
        assert!(
            (fee_result.gross_profit_usd - base_result.gross_profit_usd).abs() < 1e-9,
            "gross must be identical regardless of relay_fee; \
             base={} fee={}",
            base_result.gross_profit_usd,
            fee_result.gross_profit_usd,
        );

        // Net must be reduced by exactly relay_fee_usd.
        let net_delta = base_result.net_profit_usd - fee_result.net_profit_usd;
        assert!(
            (net_delta - relay_fee_usd).abs() < 1e-9,
            "C2: net profit must decrease by exactly relay_fee_usd=${:.2}; \
             delta was {:.9}",
            relay_fee_usd,
            net_delta,
        );
    }

    /// C2: with a relay fee that consumes all profit, opportunity becomes non-viable.
    ///
    /// $100 gross, $99 relay fee → net = $1 (barely viable).
    /// $100 gross, $101 relay fee → net = -$1 (not viable).
    #[test]
    fn c2_relay_fee_can_kill_opportunity() {
        let make = |relay_fee: f64| RoiCalculationParams {
            amount_in_usd: 1_000.0,
            expected_amount_out_usd: 1_100.0,
            expected_gas_cost_usd: 0.0,
            flashloan_fee_pct: 0.0,
            max_slippage_pct: 0.0,
            failure_risk_buffer_usd: 0.0,
            lp_fees_usd: 0.0,
            price_impact_pct: 0.0,
            capital_cost_usd: 0.0,
            ops_overhead_usd: 0.0,
            p_fail: None,
            p_copied: None,
            estimated_relay_fee_usd: relay_fee,
        };

        let barely_viable = calc_net_profit_and_roi(&make(99.0));
        assert!(
            barely_viable.is_viable,
            "relay_fee=$99 on $100 gross → net=$1 → should be viable"
        );
        assert!(
            (barely_viable.net_profit_usd - 1.0).abs() < 1e-9,
            "net expected $1, got {}",
            barely_viable.net_profit_usd,
        );

        let non_viable = calc_net_profit_and_roi(&make(101.0));
        assert!(
            !non_viable.is_viable,
            "relay_fee=$101 on $100 gross → net=-$1 → must NOT be viable"
        );
        assert!(
            (non_viable.net_profit_usd - (-1.0)).abs() < 1e-9,
            "net expected -$1, got {}",
            non_viable.net_profit_usd,
        );
    }
}
