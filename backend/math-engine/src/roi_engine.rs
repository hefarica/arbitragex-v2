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
}

/// Calcula el ROI bruto y neto de una oportunidad DeFi (Estrategia General).
///
/// ### Net profit formula (Sprint A)
/// ```text
/// net_profit =
///     expected_amount_out_usd
///   - amount_in_usd
///   - expected_gas_cost_usd        // component 1 (existing)
///   - flashloan_fee_usd            // existing: flashloan_fee_pct × amount_in
///   - lp_fees_usd                  // component 2 (NEW)
///   - effective_slippage_usd       // component 3 (NEW: price_impact or max_slippage)
///   - failure_risk_buffer_usd      // existing component 4 proxy
///   - capital_cost_usd             // component 6 (NEW)
///   - ops_overhead_usd             // component 7 (NEW)
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

    // Full deterministic net profit deducting all 7 cost components.
    let net_profit_usd = params.expected_amount_out_usd
        - params.amount_in_usd
        - params.expected_gas_cost_usd     // component 1
        - flashloan_fee_usd                // component from flashloan_fee_pct
        - params.lp_fees_usd              // component 2 (NEW)
        - effective_slippage_usd           // component 3 (NEW — replaces old slippage_cost_usd)
        - params.failure_risk_buffer_usd   // component 4 proxy (existing)
        - params.capital_cost_usd          // component 6 (NEW)
        - params.ops_overhead_usd;         // component 7 (NEW)

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
    //    gross=$100, gas=$2, flashloan=$0, lp=$1, slippage(impact)=$1,
    //    failure=$0, capital=$0.50, ops=$0.01  →  net=$95.49
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
        };

        let result = calc_net_profit_and_roi(&params);

        // flashloan_fee = 10_000 * 0.0009 = $9.0
        // slippage buffer = 10_050 * 0.001 = $10.05
        // net profit = 10_050 - 10_000 - 5.0 - 9.0 - 10.05 - 1.0 = $24.95
        assert!(result.is_viable);
        assert!((result.net_profit_usd - 24.95).abs() < 0.001);
        assert!((result.net_roi_pct - 0.2495).abs() < 0.001);
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
        };

        let result = calc_net_profit_and_roi(&params);
        assert!(!result.is_viable);
        assert!(result.net_profit_usd < 0.0);
    }
}
