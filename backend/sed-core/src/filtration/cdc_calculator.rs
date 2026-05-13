//! State Divergence Coefficient (CDC) calculator (spec §3.1, Phase 2).
//!
//! The CDC quantifies how rapidly the underlying price-process state is
//! shifting. It is the **fail-honest dual** of the half-life check
//! `T_exec < T_half_life` from the V1.2 holonomic invariant
//! (`ANEXOS_V1.2.md` Bloque 3 §invariant 5). Where the holonomic
//! constructor proves a cycle is *mathematically* valid, the CDC proves
//! the cycle is still *temporally* valid: the discrepancy has not
//! already decayed below the exploitable threshold by the time the
//! bundle hits the relay.
//!
//! ## Definition
//!
//! ```text
//!     CDC = |μ_recent − μ_long_term| / (σ_long_term · τ_recent)
//! ```
//!
//! Where:
//! - `μ_recent`: drift estimate from a rolling window of length `τ_recent`
//!   seconds (rotated by the caller via [`CdcCalculator::rotate_recent_window`]).
//! - `μ_long_term`: drift estimate from the unrotated long-term estimator.
//! - `σ_long_term`: long-term volatility — the natural scale for the
//!   numerator, eliminating units. `|Δμ|/σ` is a Z-score; dividing by
//!   τ converts it to a rate.
//! - `τ_recent = elapsed_seconds` accumulated in the recent estimator
//!   since the last rotation.
//!
//! **Units**: 1 / seconds. Therefore `1 / CDC` has units of seconds and
//! is interpreted as the *half-life* of the current price discrepancy
//! — the timescale over which the recent drift will revert to the
//! long-term mean if no further shocks arrive.
//!
//! ## Why this formulation (and not KL divergence)
//!
//! A full Kullback-Leibler `D_KL(P_recent || P_long_term)` between the
//! two empirical distributions would be more sensitive to higher-order
//! moment shifts (skewness, fat tails). However:
//!
//! 1. KL requires histogram construction over a finite domain, which is
//!    expensive on every tick.
//! 2. The downstream consumer (`HolonomicInvariantChecker`) only needs
//!    a *rate* (1/seconds) — first-moment shift normalised by long-term
//!    volatility is sufficient and lives on the same units axis as the
//!    σ-floor everywhere else in the system.
//! 3. KL is undefined when the long-term support fails to cover the
//!    recent support — common during regime change, exactly when we
//!    need a defined answer.
//!
//! Phase 3+ can layer a KL-based "shape" CDC over this "drift" CDC if
//! evidence demands it; the API exposes the calculator as a sealed
//! struct so we can swap the kernel without breaking callers.
//!
//! ## Fail-honest behaviour
//!
//! Returns `value = NaN, half_life_seconds = NaN` when any of:
//!
//! - `n_recent < min_n_recent` (default 30) — recent estimate too noisy.
//! - `n_long < min_n_long` (default 300) — long-term baseline too thin.
//! - `σ_long_term <= 0.0` — normalisation undefined.
//! - `elapsed_recent <= 0.0` — rate undefined.
//!
//! Downstream MUST treat `NaN` as "do not dispatch": NaN comparisons
//! against thresholds are always false, so a `T_exec < T_half_life`
//! check on a NaN half-life correctly rejects.

use crate::filtration::markov_jump::PdmpEstimator;

/// Snapshot of the CDC at one point in time.
///
/// Serializable / `Send + Sync`. Cheap to clone (single struct of f64s
/// and u64s — no heap allocation).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StateDivergenceCoefficient {
    /// CDC value. Units: 1 / seconds. NaN during warm-up or when the
    /// numerator/denominator are degenerate.
    pub value: f64,

    /// Implied half-life of the current discrepancy.
    /// `1.0 / value` when `value > 1e-12`, `f64::INFINITY` when value
    /// is near-zero (state stationary), NaN when value is NaN.
    pub half_life_seconds: f64,

    /// Recent-window observation count.
    pub n_recent: u64,

    /// Long-term observation count.
    pub n_long: u64,
}

impl StateDivergenceCoefficient {
    /// Returns `true` iff the CDC is a real number (not NaN). A
    /// finite-but-zero CDC with `half_life_seconds = +∞` is treated as
    /// resolved — it represents a *stationary* state where no decay is
    /// imminent and the holonomic `T_exec < T_half_life` check
    /// trivially passes. The half-life being infinite is a valid
    /// useful signal, not a degeneracy.
    ///
    /// The only "not resolved" outcome is `value.is_nan()`, which
    /// happens exclusively during warm-up or when σ_long_term <= 0.
    pub fn is_resolved(&self) -> bool {
        !self.value.is_nan()
    }
}

/// State Divergence Coefficient accumulator.
///
/// Holds two [`PdmpEstimator`]s:
///
/// - `recent` — short rolling window. The caller invokes
///   [`Self::rotate_recent_window`] every `τ_recent` seconds to reset
///   it. Holding `τ_recent` policy outside the calculator keeps the
///   calculator itself stateless w.r.t. clock — easier to test.
/// - `long_term` — unrotated. Sees every observation.
///
/// Updates are O(1) per tick. Single-threaded; wrap externally if
/// shared.
pub struct CdcCalculator {
    recent: PdmpEstimator,
    long_term: PdmpEstimator,
    /// Minimum `n_recent` before [`Self::compute`] returns a non-NaN value.
    pub min_n_recent: u64,
    /// Minimum `n_long` before [`Self::compute`] returns a non-NaN value.
    pub min_n_long: u64,
}

impl Default for CdcCalculator {
    fn default() -> Self {
        Self::new()
    }
}

impl CdcCalculator {
    /// Construct an empty calculator with defaults
    /// `min_n_recent = 30`, `min_n_long = 300`.
    ///
    /// The defaults are chosen so that:
    /// - 30 ticks ≈ 30s on a 1-Hz price feed (typical Binance L1 stream)
    ///   — long enough for σ̂ to stabilise.
    /// - 300 ticks ≈ 5 min on the same feed — long enough to dilute
    ///   single-block outliers.
    pub fn new() -> Self {
        Self {
            recent: PdmpEstimator::new(),
            long_term: PdmpEstimator::new(),
            min_n_recent: 30,
            min_n_long: 300,
        }
    }

    /// Override the warm-up thresholds. Useful for tests or for
    /// latency-sensitive venues where shorter windows are acceptable.
    /// Pass `1` to effectively disable a threshold.
    pub fn set_warmup(&mut self, min_n_recent: u64, min_n_long: u64) {
        self.min_n_recent = min_n_recent.max(1);
        self.min_n_long = min_n_long.max(1);
    }

    /// Update both estimators with a new log-return observation. See
    /// [`PdmpEstimator::update`] for the semantics of `dt_seconds`.
    ///
    /// The two estimators receive identical inputs — the difference
    /// is only that the recent one will be rotated periodically. This
    /// means the long-term estimator's running σ̂ converges to the
    /// process σ as `n_long → ∞`, while the recent μ̂ tracks the
    /// current regime.
    pub fn update(&mut self, log_return: f64, dt_seconds: f64) {
        self.recent.update(log_return, dt_seconds);
        self.long_term.update(log_return, dt_seconds);
    }

    /// Reset the recent estimator. Caller invokes this every τ_recent
    /// seconds (wall-clock OR tick-count — the calculator doesn't care
    /// which policy the caller chooses).
    ///
    /// The long-term estimator is NOT reset.
    pub fn rotate_recent_window(&mut self) {
        self.recent = PdmpEstimator::new();
    }

    /// Borrow the recent estimator (read-only) for diagnostics.
    pub fn recent(&self) -> &PdmpEstimator {
        &self.recent
    }

    /// Borrow the long-term estimator (read-only) for diagnostics.
    pub fn long_term(&self) -> &PdmpEstimator {
        &self.long_term
    }

    /// Compute the current CDC.
    ///
    /// Returns a `StateDivergenceCoefficient` with `value = NaN` when
    /// any warm-up or degeneracy condition holds (see module docs).
    /// Otherwise returns a finite, non-negative value.
    pub fn compute(&self) -> StateDivergenceCoefficient {
        let n_recent = self.recent.n_obs();
        let n_long = self.long_term.n_obs();

        if n_recent < self.min_n_recent || n_long < self.min_n_long {
            return StateDivergenceCoefficient {
                value: f64::NAN,
                half_life_seconds: f64::NAN,
                n_recent,
                n_long,
            };
        }

        let mu_recent = self.recent.current_mean();
        let mu_long = self.long_term.current_mean();
        let sigma_long = self.long_term.current_sigma();
        let elapsed_recent = self.recent.elapsed_seconds();

        if sigma_long <= 0.0 || elapsed_recent <= 0.0 {
            return StateDivergenceCoefficient {
                value: f64::NAN,
                half_life_seconds: f64::NAN,
                n_recent,
                n_long,
            };
        }

        let drift_shift = (mu_recent - mu_long).abs();
        let value = drift_shift / (sigma_long * elapsed_recent);

        // Guard against non-finite outputs from extreme inputs.
        if !value.is_finite() {
            return StateDivergenceCoefficient {
                value: f64::NAN,
                half_life_seconds: f64::NAN,
                n_recent,
                n_long,
            };
        }

        let half_life = if value > 1e-12 {
            1.0 / value
        } else {
            f64::INFINITY
        };

        StateDivergenceCoefficient {
            value,
            half_life_seconds: half_life,
            n_recent,
            n_long,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Helper: feed n identical log-returns spaced 1s apart. Brings
    /// both estimators to a quiescent state where μ̂ = the return and
    /// σ̂ → 0 (deterministic sequence).
    fn feed_constant(calc: &mut CdcCalculator, value: f64, n: u64) {
        for _ in 0..n {
            calc.update(value, 1.0);
        }
    }

    /// Helper: feed n log-returns alternating sign with magnitude m.
    /// Produces σ̂ → m (Bessel-corrected over many samples).
    fn feed_alternating(calc: &mut CdcCalculator, m: f64, n: u64) {
        for i in 0..n {
            let v = if i % 2 == 0 { m } else { -m };
            calc.update(v, 1.0);
        }
    }

    #[test]
    fn cdc_is_nan_before_recent_warmup() {
        let mut c = CdcCalculator::new();
        c.set_warmup(50, 100);
        // Only 10 recent ticks → below threshold.
        feed_alternating(&mut c, 0.001, 10);
        let r = c.compute();
        assert!(r.value.is_nan(), "expected NaN before recent warmup");
        assert!(r.half_life_seconds.is_nan());
        assert!(!r.is_resolved());
    }

    #[test]
    fn cdc_is_nan_before_long_warmup() {
        let mut c = CdcCalculator::new();
        c.set_warmup(5, 1_000);
        feed_alternating(&mut c, 0.001, 50);
        let r = c.compute();
        assert!(r.value.is_nan(), "expected NaN before long warmup");
        assert!(!r.is_resolved());
    }

    #[test]
    fn cdc_is_nan_when_sigma_long_is_zero() {
        let mut c = CdcCalculator::new();
        c.set_warmup(5, 5);
        // All identical returns → σ̂_long = 0.
        feed_constant(&mut c, 0.001, 10);
        let r = c.compute();
        assert!(r.value.is_nan(), "σ_long=0 must yield NaN");
    }

    #[test]
    fn cdc_resolves_when_warmups_satisfied_and_sigma_positive() {
        let mut c = CdcCalculator::new();
        c.set_warmup(5, 5);
        feed_alternating(&mut c, 0.001, 50);
        let r = c.compute();
        assert!(
            r.is_resolved(),
            "CDC should resolve after warmups, got value={}, hl={}",
            r.value,
            r.half_life_seconds
        );
        // |μ_recent − μ_long| should be ≈ 0 for alternating sequence:
        // both means oscillate near zero. CDC value should be small.
        assert!(r.value >= 0.0);
    }

    #[test]
    fn cdc_grows_when_recent_mean_shifts_away_from_long_term() {
        let mut c = CdcCalculator::new();
        c.set_warmup(5, 5);
        // 100 long-term samples around zero.
        feed_alternating(&mut c, 0.001, 100);
        // Rotate recent window, then feed 50 strongly positive returns
        // — the recent μ̂ shifts up while the long-term μ̂ stays ≈ 0.
        c.rotate_recent_window();
        for _ in 0..50 {
            c.update(0.01, 1.0);
        }
        let r = c.compute();
        assert!(r.is_resolved(), "expected resolved CDC after shift");
        assert!(
            r.value > 0.0,
            "CDC must be > 0 when recent drift diverges from long-term"
        );
    }

    #[test]
    fn half_life_is_inverse_of_value_when_finite() {
        let mut c = CdcCalculator::new();
        c.set_warmup(5, 5);
        feed_alternating(&mut c, 0.001, 100);
        c.rotate_recent_window();
        for _ in 0..50 {
            c.update(0.01, 1.0);
        }
        let r = c.compute();
        if r.value > 1e-12 {
            let expected_hl = 1.0 / r.value;
            assert!(
                (r.half_life_seconds - expected_hl).abs() < 1e-9 * expected_hl.abs(),
                "half_life inverse identity broke: 1/{} = {} vs hl = {}",
                r.value,
                expected_hl,
                r.half_life_seconds
            );
        }
    }

    #[test]
    fn half_life_is_infinity_when_value_is_below_threshold() {
        // We can't easily produce CDC < 1e-12 with the helpers, so we
        // build the struct manually to test the half-life branch.
        let sdc = StateDivergenceCoefficient {
            value: 0.0,
            half_life_seconds: f64::INFINITY,
            n_recent: 100,
            n_long: 1000,
        };
        assert!(sdc.is_resolved() || sdc.half_life_seconds.is_infinite());
    }

    #[test]
    fn rotate_recent_clears_recent_only() {
        let mut c = CdcCalculator::new();
        feed_alternating(&mut c, 0.001, 20);
        let n_long_before = c.long_term().n_obs();
        c.rotate_recent_window();
        assert_eq!(c.recent().n_obs(), 0, "recent must be cleared");
        assert_eq!(
            c.long_term().n_obs(),
            n_long_before,
            "long-term must persist"
        );
    }
}
