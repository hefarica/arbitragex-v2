//! Phase OMEGA — Kelly Criterion + V3 Concentrated Liquidity Math.
//!
//! Institutional-grade position sizing for the MEV searcher. Implements
//! Kelly Criterion (Kelly 1956, Thorp 1969) with **fractional Kelly**
//! defensive scaling — the Top-1% HFT standard. Top firms never run full
//! Kelly because the long-tail of estimation error in `win_prob` makes
//! full-Kelly variance unacceptable; fractional Kelly (0.25×–0.5×) keeps
//! the compounding rate near optimal while bounding drawdown variance.
//!
//! ## Pure module — no I/O
//!
//! All functions in this module are pure (input → output, no side effects).
//! They can be called from any context: sync, async, hot-path, tests,
//! background workers. No allocations except for f64 arithmetic results.
//!
//! ## Anti-fraud invariants
//!
//! 1. NEVER returns `Some(f)` from `kelly_fraction` for nonsense input.
//!    NaN/Inf/negative probability/probability > 1/zero loss all return
//!    `None`. The caller MUST treat `None` as "do not size — reject the
//!    candidate" (RULE 12 fail-honest).
//! 2. `fractional_kelly` ALWAYS clamps to `[0, max_per_trade_fraction]`
//!    even when full Kelly says go bigger. The hard cap is non-negotiable.
//! 3. `compute_position_size` rejects when `nav` is non-positive or when
//!    the computed wei amount overflows U256.
//! 4. V3 tick math: `tick_to_sqrt_price_x96` uses the canonical Uniswap
//!    formula `sqrt(1.0001^tick) * 2^96` — NOT a hand-rolled approximation.
//!
//! ## References
//!
//! - Kelly, J. L. (1956). "A new interpretation of information rate."
//! - Thorp, E. O. (1969). "Optimal gambling systems for favorable games."
//! - MacLean, Thorp, Ziemba (2010). "The Kelly Capital Growth Investment Criterion."
//! - Uniswap V3 whitepaper §6.1 (tick / sqrtPriceX96 formulas).

use ethers::types::U256;

// ---------------------------------------------------------------------------
// Kelly Criterion
// ---------------------------------------------------------------------------

/// Compute the full Kelly fraction `f*` for a bet with two outcomes.
///
/// `win_prob` ∈ (0, 1): probability of winning.
/// `gain_on_win` > 0: fractional gain on win (e.g., 0.05 = +5%).
/// `loss_on_loss` > 0: fractional loss on loss (e.g., 0.03 = -3%).
///
/// Returns `Some(f*)` with `f*` ∈ [0, 1], or `None` for invalid input.
///
/// Formula: `f* = (p * gain - (1-p) * loss) / (gain * loss)`
///        = `p / loss - (1-p) / gain`
///
/// Clamped to `[0, 1]` — Kelly never recommends betting more than 100% of
/// capital on a single outcome (that would imply leverage > 1× which we
/// don't allow for paper-mode safety).
pub fn kelly_fraction(win_prob: f64, gain_on_win: f64, loss_on_loss: f64) -> Option<f64> {
    if !win_prob.is_finite() || !gain_on_win.is_finite() || !loss_on_loss.is_finite() {
        return None;
    }
    if !(0.0..=1.0).contains(&win_prob) {
        return None;
    }
    if gain_on_win <= 0.0 || loss_on_loss <= 0.0 {
        return None;
    }
    let p = win_prob;
    let q = 1.0 - p;
    // The compact form: f* = p/loss - q/gain.
    let raw = p / loss_on_loss - q / gain_on_win;
    if !raw.is_finite() {
        return None;
    }
    Some(raw.clamp(0.0, 1.0))
}

/// Apply fractional Kelly with a hard per-trade cap.
///
/// `kelly_multiplier` ∈ (0, 1]: defensive scaling (institutional default
/// 0.25 — "quarter Kelly"). Smaller values reduce variance at the cost of
/// slightly slower compounding.
///
/// `max_per_trade_fraction` ∈ (0, 1]: hard cap on capital per trade
/// regardless of what Kelly says. Defaults at the operator's discretion.
/// SKILL_037 stress-test thresholds suggest DD > 10% triggers size
/// reduction; a conservative `max_per_trade_fraction = 0.02` (2% of NAV)
/// stays well below that trigger.
///
/// Returns `Some(f)` with `f` ∈ `[0, max_per_trade_fraction]`, or `None`
/// when any input is invalid.
pub fn fractional_kelly(
    win_prob: f64,
    gain_on_win: f64,
    loss_on_loss: f64,
    kelly_multiplier: f64,
    max_per_trade_fraction: f64,
) -> Option<f64> {
    if !kelly_multiplier.is_finite() || !max_per_trade_fraction.is_finite() {
        return None;
    }
    if kelly_multiplier <= 0.0 || kelly_multiplier > 1.0 {
        return None;
    }
    if max_per_trade_fraction <= 0.0 || max_per_trade_fraction > 1.0 {
        return None;
    }
    let f_full = kelly_fraction(win_prob, gain_on_win, loss_on_loss)?;
    let f_scaled = f_full * kelly_multiplier;
    Some(f_scaled.min(max_per_trade_fraction))
}

/// Translate a Kelly fraction into a concrete `amount_in_wei` given the
/// current NAV.
///
/// `nav_wei` is the operator's total net asset value in `token_in` wei.
/// The returned wei amount is `floor(nav_wei × fraction)`.
///
/// Returns `None` if `fraction` is invalid, `nav_wei` is zero, or the
/// multiplication overflows U256.
pub fn compute_position_size(nav_wei: U256, fraction: f64) -> Option<U256> {
    if !fraction.is_finite() || !(0.0..=1.0).contains(&fraction) {
        return None;
    }
    if nav_wei.is_zero() {
        return None;
    }
    // Convert fraction to ppm (parts per million) using safe integer math
    // to avoid f64 precision loss on large NAVs.
    // ppm range: [0, 1_000_000]. The cast to u64 is safe after the bound
    // check on `fraction`.
    let ppm = (fraction * 1_000_000.0).floor();
    if !ppm.is_finite() || !(0.0..=1_000_000.0).contains(&ppm) {
        return None;
    }
    let ppm_u256 = U256::from(ppm as u64);
    // amount = nav × ppm / 1_000_000. Use checked arithmetic.
    let scaled = nav_wei.checked_mul(ppm_u256)?;
    let amount = scaled / U256::from(1_000_000u64);
    if amount.is_zero() {
        // Sub-ppm sizing rounds to zero; reject so the caller surfaces it
        // honestly instead of attempting a no-op trade.
        return None;
    }
    Some(amount)
}

// ---------------------------------------------------------------------------
// Uniswap V3 Concentrated Liquidity Math
// ---------------------------------------------------------------------------

/// Convert a V3 tick index to `sqrtPriceX96` using the canonical formula
/// `sqrt(1.0001^tick) * 2^96`.
///
/// Returns `None` for ticks outside `[MIN_TICK, MAX_TICK]` (the bounds
/// enforced by Uniswap V3 — beyond these the price would overflow).
///
/// Note: this uses f64 intermediate arithmetic for clarity. For
/// production-grade integer-exact precision Uniswap uses a fixed-point
/// LUT (`TickMath::getSqrtRatioAtTick`); the f64 form here is accurate
/// to ~13 decimal digits which is sufficient for sizing decisions but
/// not for swap-amount calculations.
pub fn tick_to_sqrt_price_x96_f64(tick: i32) -> Option<f64> {
    // V3 limits: ticks beyond these would overflow uint160 sqrtPriceX96.
    const MIN_TICK: i32 = -887_272;
    const MAX_TICK: i32 = 887_272;
    if !(MIN_TICK..=MAX_TICK).contains(&tick) {
        return None;
    }
    // 1.0001^tick = exp(tick * ln(1.0001))
    let log_base = (1.0001_f64).ln();
    let ratio = (tick as f64 * log_base).exp();
    if !ratio.is_finite() || ratio <= 0.0 {
        return None;
    }
    let sqrt_ratio = ratio.sqrt();
    // 2^96 ≈ 7.9228e28
    let q96 = 2.0_f64.powi(96);
    let value = sqrt_ratio * q96;
    if !value.is_finite() {
        return None;
    }
    Some(value)
}

/// Compute the V3 in-range "L" (liquidity) value supplied by a deposit
/// of `amount0` and `amount1` of token0 and token1 into a position
/// bracketed by `[tick_lower, tick_upper]` at current `tick_current`.
///
/// Returns `None` when bracket is invalid, current is outside the
/// bracket, or one of the f64 conversions overflows.
///
/// Uniswap V3 whitepaper §6.1:
///   L = min(L0, L1) where
///   L0 = amount0 × (sqrt(P_a) × sqrt(P_b)) / (sqrt(P_b) - sqrt(P_a))
///   L1 = amount1 / (sqrt(P_b) - sqrt(P_a))
///
/// when `P_a ≤ P_current ≤ P_b` (in-range position).
pub fn v3_liquidity_from_amounts_in_range(
    tick_lower: i32,
    tick_upper: i32,
    tick_current: i32,
    amount0: f64,
    amount1: f64,
) -> Option<f64> {
    if tick_lower >= tick_upper {
        return None;
    }
    if !(tick_lower..=tick_upper).contains(&tick_current) {
        return None;
    }
    if !amount0.is_finite() || !amount1.is_finite() || amount0 < 0.0 || amount1 < 0.0 {
        return None;
    }
    let sqrt_p_a = tick_to_sqrt_price_x96_f64(tick_lower)?;
    let sqrt_p_b = tick_to_sqrt_price_x96_f64(tick_upper)?;
    let denom = sqrt_p_b - sqrt_p_a;
    if denom <= 0.0 {
        return None;
    }
    let l0 = amount0 * (sqrt_p_a * sqrt_p_b) / denom;
    let l1 = amount1 / denom;
    let result = l0.min(l1);
    if !result.is_finite() || result < 0.0 {
        return None;
    }
    Some(result)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    fn approx(a: f64, b: f64, tol: f64) -> bool {
        (a - b).abs() < tol
    }

    // ── Kelly correctness ─────────────────────────────────────────────────

    /// Classic 50/50 with 2:1 gain — Kelly says bet 25% of capital.
    /// f* = p/loss - q/gain = 0.5/1 - 0.5/2 = 0.5 - 0.25 = 0.25
    #[test]
    fn kelly_classic_50_50_2_to_1_returns_quarter() {
        let f = kelly_fraction(0.5, 2.0, 1.0).unwrap();
        assert!(approx(f, 0.25, 1e-12), "f* = {f}, expected 0.25");
    }

    /// 60/40 with even 1:1 odds — Kelly says bet 20% of capital.
    /// f* = 0.6/1 - 0.4/1 = 0.6 - 0.4 = 0.2
    #[test]
    fn kelly_60_40_even_odds_returns_20_percent() {
        let f = kelly_fraction(0.6, 1.0, 1.0).unwrap();
        assert!(approx(f, 0.2, 1e-12), "f* = {f}, expected 0.2");
    }

    /// Unfavorable bet (negative edge) clamps to 0 — never recommend a
    /// negative position.
    #[test]
    fn kelly_unfavorable_clamps_to_zero() {
        // 40/60 with even odds: f* = 0.4 - 0.6 = -0.2 → clamp to 0.
        let f = kelly_fraction(0.4, 1.0, 1.0).unwrap();
        assert_eq!(f, 0.0);
    }

    /// Certain win (p = 1) with any odds → bet 100% of capital.
    /// f* = 1/loss - 0/gain = 1/loss, clamped to 1.0.
    #[test]
    fn kelly_certain_win_returns_one() {
        let f = kelly_fraction(1.0, 0.5, 0.5).unwrap();
        assert_eq!(f, 1.0);
    }

    /// Certain loss (p = 0) → never bet.
    #[test]
    fn kelly_certain_loss_returns_zero() {
        let f = kelly_fraction(0.0, 1.0, 1.0).unwrap();
        assert_eq!(f, 0.0);
    }

    // ── Kelly rejection paths ─────────────────────────────────────────────

    #[test]
    fn kelly_nan_inputs_rejected() {
        assert_eq!(kelly_fraction(f64::NAN, 1.0, 1.0), None);
        assert_eq!(kelly_fraction(0.5, f64::NAN, 1.0), None);
        assert_eq!(kelly_fraction(0.5, 1.0, f64::NAN), None);
    }

    #[test]
    fn kelly_infinite_inputs_rejected() {
        assert_eq!(kelly_fraction(f64::INFINITY, 1.0, 1.0), None);
        assert_eq!(kelly_fraction(0.5, f64::INFINITY, 1.0), None);
    }

    #[test]
    fn kelly_negative_probability_rejected() {
        assert_eq!(kelly_fraction(-0.1, 1.0, 1.0), None);
    }

    #[test]
    fn kelly_probability_above_one_rejected() {
        assert_eq!(kelly_fraction(1.1, 1.0, 1.0), None);
    }

    #[test]
    fn kelly_zero_gain_rejected() {
        assert_eq!(kelly_fraction(0.5, 0.0, 1.0), None);
    }

    #[test]
    fn kelly_zero_loss_rejected() {
        assert_eq!(kelly_fraction(0.5, 1.0, 0.0), None);
    }

    #[test]
    fn kelly_negative_gain_rejected() {
        assert_eq!(kelly_fraction(0.5, -0.1, 1.0), None);
    }

    // ── Fractional Kelly ──────────────────────────────────────────────────

    /// Quarter-Kelly with the classic 50/50 2:1 bet → 0.25 × 0.25 = 0.0625
    /// of capital. With a 2% per-trade cap, clamps to 2%.
    #[test]
    fn fractional_kelly_clamps_to_per_trade_cap() {
        let f = fractional_kelly(0.6, 1.0, 1.0, 0.25, 0.02).unwrap();
        // Full Kelly = 0.2, quarter = 0.05, cap = 0.02 → result 0.02
        assert!(approx(f, 0.02, 1e-12), "f = {f}, expected 0.02 (clamped)");
    }

    /// Half-Kelly with a 5% per-trade cap and a small edge → no clamping.
    #[test]
    fn fractional_kelly_no_clamp_when_below_cap() {
        // Full Kelly for 55/45 1:1 = 0.1. Half = 0.05. Cap 0.05 → no clamp.
        let f = fractional_kelly(0.55, 1.0, 1.0, 0.5, 0.05).unwrap();
        assert!(approx(f, 0.05, 1e-9), "f = {f}, expected 0.05");
    }

    #[test]
    fn fractional_kelly_rejects_invalid_multiplier() {
        assert_eq!(fractional_kelly(0.5, 1.0, 1.0, 0.0, 0.02), None);
        assert_eq!(fractional_kelly(0.5, 1.0, 1.0, 1.1, 0.02), None);
        assert_eq!(fractional_kelly(0.5, 1.0, 1.0, -0.1, 0.02), None);
    }

    #[test]
    fn fractional_kelly_rejects_invalid_cap() {
        assert_eq!(fractional_kelly(0.5, 1.0, 1.0, 0.25, 0.0), None);
        assert_eq!(fractional_kelly(0.5, 1.0, 1.0, 0.25, 1.5), None);
    }

    // ── Position size translation ─────────────────────────────────────────

    #[test]
    fn compute_position_size_1_eth_5_percent() {
        // 1 ETH NAV × 5% = 0.05 ETH = 5 × 10^16 wei
        let nav = U256::from(10u64).pow(U256::from(18u64));
        let amount = compute_position_size(nav, 0.05).unwrap();
        assert_eq!(amount, U256::from(50_000_000_000_000_000u128));
    }

    #[test]
    fn compute_position_size_rejects_zero_nav() {
        assert_eq!(compute_position_size(U256::zero(), 0.5), None);
    }

    #[test]
    fn compute_position_size_rejects_invalid_fraction() {
        let nav = U256::from(10u64).pow(U256::from(18u64));
        assert_eq!(compute_position_size(nav, -0.1), None);
        assert_eq!(compute_position_size(nav, 1.1), None);
        assert_eq!(compute_position_size(nav, f64::NAN), None);
    }

    #[test]
    fn compute_position_size_rejects_sub_ppm_underflow() {
        // 1 wei NAV × 1e-7 fraction = 1e-7 wei → ppm = 0 → amount = 0 → None
        let nav = U256::from(1u64);
        assert_eq!(compute_position_size(nav, 1e-7), None);
    }

    // ── V3 tick math ──────────────────────────────────────────────────────

    /// Tick 0 corresponds to price 1.0, so sqrtPriceX96 = 1 × 2^96.
    #[test]
    fn tick_zero_maps_to_2_pow_96() {
        let s = tick_to_sqrt_price_x96_f64(0).unwrap();
        let expected = 2.0_f64.powi(96);
        assert!(
            approx(s, expected, expected * 1e-12),
            "s = {s}, expected ≈ {expected}"
        );
    }

    /// Tick math is monotonically increasing.
    #[test]
    fn tick_to_sqrt_is_monotonic() {
        let s_neg = tick_to_sqrt_price_x96_f64(-100).unwrap();
        let s_zero = tick_to_sqrt_price_x96_f64(0).unwrap();
        let s_pos = tick_to_sqrt_price_x96_f64(100).unwrap();
        assert!(s_neg < s_zero);
        assert!(s_zero < s_pos);
    }

    /// Out-of-range ticks return None.
    #[test]
    fn tick_out_of_range_rejected() {
        assert!(tick_to_sqrt_price_x96_f64(-887_273).is_none());
        assert!(tick_to_sqrt_price_x96_f64(887_273).is_none());
    }

    /// Liquidity from amounts: in-range happy path.
    #[test]
    fn v3_liquidity_in_range_returns_some() {
        let l = v3_liquidity_from_amounts_in_range(-100, 100, 0, 1.0, 1.0);
        assert!(l.is_some());
        assert!(l.unwrap() > 0.0);
    }

    /// Out-of-range current tick rejected.
    #[test]
    fn v3_liquidity_out_of_range_rejected() {
        // tick_current=500 > tick_upper=100
        assert!(v3_liquidity_from_amounts_in_range(-100, 100, 500, 1.0, 1.0).is_none());
    }

    /// Inverted bracket rejected.
    #[test]
    fn v3_liquidity_inverted_bracket_rejected() {
        // lower > upper
        assert!(v3_liquidity_from_amounts_in_range(100, -100, 0, 1.0, 1.0).is_none());
    }
}
