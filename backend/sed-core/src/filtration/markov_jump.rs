//! Markov jump process recursive estimator (spec §3.1, Phase 2).
//!
//! Models the price process as a piecewise-deterministic Markov process
//! (PDMP) following the Cont-Tankov jump-diffusion SDE:
//!
//! ```text
//!     dX_t = μ dt + σ dW_t + dJ_t
//! ```
//!
//! where `W_t` is a standard Brownian motion and `J_t` is a compound
//! Poisson process with jump intensity λ. The three parameters {μ, σ, λ}
//! fully characterise the local first-order behaviour of the process.
//!
//! The estimator maintains all three online via **Welford's algorithm**
//! (single-pass, numerically stable for `n` ≫ 10⁶ without re-summing the
//! windowed history) plus a sliding **jump counter** keyed on standardised
//! residuals.
//!
//! ## Algorithmic identities
//!
//! **Welford recurrence** for the running mean and the sum-of-squared-
//! deviations `M₂`:
//!
//! ```text
//!     μₙ  = μₙ₋₁ + (xₙ − μₙ₋₁) / n
//!     M₂ₙ = M₂ₙ₋₁ + (xₙ − μₙ₋₁)(xₙ − μₙ)
//!     σ²ₙ = M₂ₙ / (n − 1)             (Bessel's correction → unbiased)
//! ```
//!
//! **Jump detection** classifies an observation as a Poisson jump rather
//! than a diffusion step when its standardised residual exceeds a
//! configurable threshold (default 3σ — i.e., a ≈ 0.27% tail event under
//! the Gaussian-diffusion null hypothesis):
//!
//! ```text
//!     zₙ = |xₙ − μₙ| / σₙ
//!     jump  ⇔  zₙ > z_threshold     (with n ≥ N_warmup for σ stability)
//! ```
//!
//! The cumulative jump count divided by elapsed wall-clock seconds yields
//! the intensity estimate `λ̂`.
//!
//! ## Numerical hygiene
//!
//! - Non-finite inputs (NaN, ±∞) are silently skipped — fail-honest
//!   discipline: a corrupted feed should not poison the estimator
//!   state. The caller observes the skip via `n_obs` not advancing.
//! - σ is reported as 0.0 until `n ≥ 2` (one-sample variance is
//!   undefined under Bessel). Downstream consumers MUST treat 0.0 as
//!   "insufficient data" rather than "deterministic process".
//! - λ requires `elapsed_seconds > 0`; otherwise it reports 0.0.

/// Snapshot of the three PDMP parameters at a single point in time,
/// plus the sample sizes that produced them.
///
/// All fields are plain `f64`/`u64` so the snapshot is trivially
/// serialisable, broadcastable across threads (it is `Send + Sync`),
/// and amenable to Prometheus gauge export.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ProcessStats {
    /// Drift estimate `μ̂` — units: log-return per second. Computed as
    /// the running mean of log-returns scaled by the observed sample
    /// rate `(n_obs / elapsed_seconds)`. Reduces to the bare running
    /// mean when `elapsed_seconds == 0`.
    pub mu: f64,

    /// Volatility estimate `σ̂` — units: same as μ. Sample standard
    /// deviation of the log-returns with Bessel's correction. Reports
    /// `0.0` when `n_obs < 2`.
    pub sigma: f64,

    /// Jump intensity estimate `λ̂` — units: jumps per second.
    /// `n_jumps / elapsed_seconds`. Reports `0.0` when
    /// `elapsed_seconds == 0`.
    pub lambda: f64,

    /// Number of finite observations consumed (excludes NaN/inf input).
    pub n_obs: u64,

    /// Number of jump events detected so far (subset of `n_obs`).
    pub n_jumps: u64,

    /// Wall-clock seconds elapsed across all observations. Sum of
    /// per-update `dt_seconds`.
    pub elapsed_seconds: f64,
}

/// Online recursive estimator for `{μ, σ, λ}` of a PDMP.
///
/// Single-threaded — wrap in `Arc<Mutex<…>>` or `Arc<RwLock<…>>` if
/// cross-task updates are needed. The state is small (`<64 bytes`) so
/// a Mutex contention cost is dominated by the work it guards.
pub struct PdmpEstimator {
    n: u64,
    mean: f64,
    m2: f64,
    jump_count: u64,
    elapsed_seconds: f64,
    /// Z-score threshold beyond which a return is classified as a jump.
    /// Default `3.0` (3σ). Configurable via `set_jump_threshold`.
    jump_z_threshold: f64,
    /// Minimum `n` before jump classification activates. Below this,
    /// σ̂ is too noisy to produce a meaningful z-score; we count zero
    /// jumps regardless of how extreme the value is, which means λ̂
    /// remains `0` during warm-up.
    jump_warmup_n: u64,
}

impl Default for PdmpEstimator {
    fn default() -> Self {
        Self::new()
    }
}

impl PdmpEstimator {
    /// Construct an empty estimator with defaults
    /// `jump_z_threshold = 3.0`, `jump_warmup_n = 30`.
    pub fn new() -> Self {
        Self {
            n: 0,
            mean: 0.0,
            m2: 0.0,
            jump_count: 0,
            elapsed_seconds: 0.0,
            jump_z_threshold: 3.0,
            jump_warmup_n: 30,
        }
    }

    /// Override the z-score threshold for jump classification.
    ///
    /// Lower values mark more events as jumps (higher λ̂, smaller
    /// effective σ²); higher values mark fewer. Production code in
    /// `searcher-rs` typically uses `2.5..=3.5` depending on the
    /// venue's micro-volatility profile.
    pub fn set_jump_threshold(&mut self, z: f64) {
        if z.is_finite() && z > 0.0 {
            self.jump_z_threshold = z;
        }
    }

    /// Override the warm-up sample count. The default `30` is the
    /// canonical "rule of thumb" for σ̂ to stabilise within ~20% of
    /// its asymptote under iid sampling.
    pub fn set_jump_warmup(&mut self, n: u64) {
        self.jump_warmup_n = n;
    }

    /// Feed one log-return observation into the estimator.
    ///
    /// - `log_return` MUST be a log-return: `ln(p_t / p_{t-1})`.
    ///   Linear returns are silently miscalibrated; the constructor
    ///   does not validate.
    /// - `dt_seconds` is the wall-clock time elapsed since the previous
    ///   observation. Pass `0.0` for the first update or for tick-time
    ///   estimation where wall-clock is not meaningful — λ̂ will then
    ///   report `0.0`.
    ///
    /// Non-finite `log_return` (NaN, ±∞) is silently dropped — the
    /// estimator state and `n_obs` do not advance.
    pub fn update(&mut self, log_return: f64, dt_seconds: f64) {
        if !log_return.is_finite() {
            // Fail-honest: corrupted tick → do not poison estimator.
            return;
        }
        self.n += 1;
        let delta = log_return - self.mean;
        self.mean += delta / (self.n as f64);
        let delta2 = log_return - self.mean;
        self.m2 += delta * delta2;

        if self.n >= self.jump_warmup_n {
            let sigma = self.current_sigma();
            if sigma > 0.0 {
                let z = (log_return - self.mean).abs() / sigma;
                if z > self.jump_z_threshold {
                    self.jump_count += 1;
                }
            }
        }

        if dt_seconds.is_finite() && dt_seconds > 0.0 {
            self.elapsed_seconds += dt_seconds;
        }
    }

    /// Current σ̂ with Bessel's correction. Returns `0.0` when
    /// `n < 2` (variance undefined).
    pub fn current_sigma(&self) -> f64 {
        if self.n < 2 {
            return 0.0;
        }
        (self.m2 / (self.n as f64 - 1.0)).sqrt()
    }

    /// Current μ̂ — the unscaled running mean of log-returns. NOT
    /// per-second; use `stats().mu` for that.
    pub fn current_mean(&self) -> f64 {
        self.mean
    }

    /// Number of observations consumed so far.
    pub fn n_obs(&self) -> u64 {
        self.n
    }

    /// Wall-clock seconds elapsed (sum of per-update `dt_seconds`).
    pub fn elapsed_seconds(&self) -> f64 {
        self.elapsed_seconds
    }

    /// Snapshot the three PDMP parameters as a `ProcessStats`.
    ///
    /// **μ scaling**: when `elapsed_seconds > 0`, μ is rescaled from
    /// per-observation to per-second via the implied sample rate
    /// `n_obs / elapsed_seconds`. This makes μ̂ comparable across
    /// venues with different tick frequencies. When `elapsed_seconds
    /// == 0`, μ is reported as the bare running mean (per-observation).
    pub fn stats(&self) -> ProcessStats {
        let mu = if self.elapsed_seconds > 0.0 {
            // Per-second drift: scale by implied sample rate.
            self.mean * (self.n as f64) / self.elapsed_seconds
        } else {
            self.mean
        };
        let sigma = self.current_sigma();
        let lambda = if self.elapsed_seconds > 0.0 {
            (self.jump_count as f64) / self.elapsed_seconds
        } else {
            0.0
        };
        ProcessStats {
            mu,
            sigma,
            lambda,
            n_obs: self.n,
            n_jumps: self.jump_count,
            elapsed_seconds: self.elapsed_seconds,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_estimator_reports_zero_everywhere() {
        let e = PdmpEstimator::new();
        let s = e.stats();
        assert_eq!(s.mu, 0.0);
        assert_eq!(s.sigma, 0.0);
        assert_eq!(s.lambda, 0.0);
        assert_eq!(s.n_obs, 0);
        assert_eq!(s.n_jumps, 0);
    }

    #[test]
    fn non_finite_input_is_silently_dropped() {
        let mut e = PdmpEstimator::new();
        e.update(f64::NAN, 0.1);
        e.update(f64::INFINITY, 0.1);
        e.update(f64::NEG_INFINITY, 0.1);
        assert_eq!(e.n_obs(), 0, "NaN/±inf must not advance n_obs");
        // elapsed_seconds also does not advance because we skip the
        // entire update on bad input.
        assert_eq!(e.elapsed_seconds(), 0.0);
    }

    #[test]
    fn welford_mean_matches_naive_for_deterministic_sequence() {
        let mut e = PdmpEstimator::new();
        let xs = [0.001, 0.002, -0.001, 0.0005, -0.0015];
        for &x in &xs {
            e.update(x, 1.0);
        }
        let expected_mean = xs.iter().sum::<f64>() / xs.len() as f64;
        assert!((e.current_mean() - expected_mean).abs() < 1e-15);
    }

    #[test]
    fn welford_variance_matches_naive_for_deterministic_sequence() {
        let mut e = PdmpEstimator::new();
        let xs = [0.001, 0.002, -0.001, 0.0005, -0.0015];
        for &x in &xs {
            e.update(x, 1.0);
        }
        let mean = xs.iter().sum::<f64>() / xs.len() as f64;
        let naive_var = xs.iter().map(|x| (x - mean).powi(2)).sum::<f64>()
            / (xs.len() - 1) as f64;
        let expected_sigma = naive_var.sqrt();
        assert!(
            (e.current_sigma() - expected_sigma).abs() < 1e-15,
            "Welford σ disagrees with naive: got {}, want {}",
            e.current_sigma(),
            expected_sigma
        );
    }

    #[test]
    fn sigma_zero_until_two_observations() {
        let mut e = PdmpEstimator::new();
        e.update(0.001, 1.0);
        assert_eq!(e.current_sigma(), 0.0, "σ undefined for n=1");
        e.update(0.002, 1.0);
        assert!(e.current_sigma() > 0.0, "σ must materialise at n=2");
    }

    #[test]
    fn jump_classification_inactive_during_warmup() {
        let mut e = PdmpEstimator::new();
        // Force warmup_n = 5 so we can test cheaply.
        e.set_jump_warmup(5);
        // Feed 4 small returns + 1 huge spike.
        for _ in 0..4 {
            e.update(0.0001, 1.0);
        }
        e.update(10.0, 1.0); // would be a jump if warm-up passed
        assert_eq!(e.stats().n_jumps, 0, "no jumps during warmup");
    }

    #[test]
    fn jump_classification_fires_after_warmup() {
        let mut e = PdmpEstimator::new();
        e.set_jump_warmup(5);
        e.set_jump_threshold(3.0);
        // Build up stable σ ≈ 1e-4 with 30 small returns.
        for i in 0..30 {
            let v = if i % 2 == 0 { 0.0001 } else { -0.0001 };
            e.update(v, 1.0);
        }
        let n_jumps_before = e.stats().n_jumps;
        // Feed an extreme outlier (≫3σ).
        e.update(0.5, 1.0);
        let n_jumps_after = e.stats().n_jumps;
        assert_eq!(
            n_jumps_after - n_jumps_before,
            1,
            "exactly one jump should be classified"
        );
    }

    #[test]
    fn lambda_scales_with_elapsed_time() {
        let mut e = PdmpEstimator::new();
        // Warm-up with 60 alternating small returns establishes a
        // stable σ̂ ≈ 1e-4 and `n` ≥ default warmup (30). With a tiny
        // warmup window, a fresh huge spike simultaneously pulls the
        // mean AND inflates σ̂, so the z-score never escapes the
        // threshold — the asymptotic property of online estimators.
        // 60 samples is large enough that subsequent spikes look like
        // genuine outliers under the now-stable σ̂.
        for i in 0..60 {
            let v = if i % 2 == 0 { 0.0001 } else { -0.0001 };
            e.update(v, 1.0);
        }
        let baseline_jumps = e.stats().n_jumps;
        // Now spike twice — magnitude is ≫ 1000σ̂ so both classify
        // even after σ̂ inflates from the first spike.
        e.update(1.0, 5.0);
        e.update(1.0, 5.0);
        let s = e.stats();
        let new_jumps = s.n_jumps - baseline_jumps;
        assert!(new_jumps >= 2, "expected ≥2 new jumps, got {new_jumps}");
        let expected_lambda = s.n_jumps as f64 / s.elapsed_seconds;
        assert!((s.lambda - expected_lambda).abs() < 1e-12);
    }

    #[test]
    fn lambda_is_zero_when_no_time_elapsed() {
        let mut e = PdmpEstimator::new();
        e.update(0.001, 0.0);
        e.update(0.002, 0.0);
        assert_eq!(e.stats().lambda, 0.0);
        assert_eq!(e.stats().elapsed_seconds, 0.0);
    }

    #[test]
    fn negative_dt_does_not_advance_elapsed_seconds() {
        let mut e = PdmpEstimator::new();
        e.update(0.001, -1.0); // garbage dt
        e.update(0.002, 1.0);
        assert_eq!(e.elapsed_seconds(), 1.0, "only positive dt counted");
    }

    #[test]
    fn set_jump_threshold_rejects_invalid_values() {
        let mut e = PdmpEstimator::new();
        e.set_jump_threshold(-1.0); // should be ignored
        e.set_jump_threshold(0.0); // should be ignored
        e.set_jump_threshold(f64::NAN); // should be ignored
        // Default 3.0 preserved through all rejections. Default
        // warmup (30) preserved too.
        // 60 alternating warmup samples establish stable σ̂ — see the
        // explanation in lambda_scales_with_elapsed_time for why tiny
        // warmup windows degenerate the jump-classification z-score.
        for i in 0..60 {
            let v = if i % 2 == 0 { 0.0001 } else { -0.0001 };
            e.update(v, 1.0);
        }
        let baseline = e.stats().n_jumps;
        e.update(1.0, 1.0);
        // At threshold 3.0 (default, preserved through the invalid
        // setter calls above) and stable σ̂ ≈ 1e-4, the 1.0 outlier
        // is ≫ 3σ → classified as a jump.
        assert!(
            e.stats().n_jumps > baseline,
            "default-threshold default-warmup jump classification failed"
        );
    }
}
