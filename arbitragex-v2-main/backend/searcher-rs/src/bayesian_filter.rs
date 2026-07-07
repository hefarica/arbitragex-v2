//! Phase OMEGA 3.2 — Bayesian inference + adverse-selection filters.
//!
//! Pure-math primitives for the candidate-selection layer. Consumed (when
//! wired) by `prioritization-spine` evaluation to reject candidates whose
//! Bayesian posterior win-probability is too low OR whose pool exhibits
//! toxic order-flow signatures (PIN / VPIN above operator-set thresholds).
//!
//! ## Theory
//!
//! - **Beta-Binomial conjugate**: prior Beta(α, β), likelihood Binomial(n, p)
//!   → posterior Beta(α + wins, β + losses). Closed-form, no MCMC needed.
//! - **PIN (Probability of Informed Trading, Easley-O'Hara 1996)**:
//!   probability that an arriving trade is from an informed counterparty.
//!   Operator-tuned threshold (default 0.65 per SKILL_001) above which
//!   market-making should reduce quoting / switch to defensive sniping.
//! - **VPIN (Volume-Synchronized PIN, Easley-Lopez-Prado-O'Hara 2012)**:
//!   bucket-based volume-clock toxicity estimate. Replaces the Easley-Kiefer
//!   MLE with a simpler bucket-imbalance heuristic suitable for HFT loops.
//!
//! ## Anti-fraud invariants
//!
//! 1. Every public function returns `Option<…>` and rejects NaN, Inf,
//!    non-positive prior parameters, win/loss counts that exceed plausible
//!    ranges, and division-by-zero conditions. RULE 12 fail-honest: no
//!    silent "looks reasonable" defaults.
//! 2. Bayesian updates are POSITIVE-EVIDENCE only — `wins` and `losses`
//!    must be u64 (no negative counts). Passing both as zero returns the
//!    prior unchanged (the honest outcome of "no evidence yet").
//! 3. `decide_size_reduction_for_pin` returns a clamped factor in [0, 1].
//!    Operator's threshold + reduction strength are mandatory inputs; the
//!    function refuses to assume defaults.
//!
//! ## References
//!
//! - Easley, Kiefer, O'Hara, Paperman (1996). "Liquidity, information,
//!   and infrequently traded stocks." J. Finance 51, 1405-1436.
//! - Easley, Lopez de Prado, O'Hara (2012). "Flow toxicity and liquidity
//!   in a high-frequency world." Review of Financial Studies 25, 1457-1493.
//! - Glosten, Milgrom (1985). "Bid, ask and transaction prices in a
//!   specialist market with heterogeneously informed traders."

// ---------------------------------------------------------------------------
// Beta distribution (conjugate prior for Bernoulli win-rate)
// ---------------------------------------------------------------------------

/// Beta(α, β) distribution parameters. Both `alpha` and `beta` MUST be
/// strictly positive. A standard "uninformative" prior is Beta(1, 1) =
/// uniform on [0, 1]; SKILL_021 recommends Beta(2, 2) as a slightly
/// concentrated prior when the operator has weak domain knowledge that
/// the true win-rate is more likely near 0.5 than at the extremes.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BetaParams {
    pub alpha: f64,
    pub beta: f64,
}

impl BetaParams {
    /// Construct a `BetaParams` validated to have strictly positive
    /// `alpha` and `beta`. Returns `None` for NaN/Inf/non-positive values.
    pub fn new(alpha: f64, beta: f64) -> Option<Self> {
        if !alpha.is_finite() || !beta.is_finite() {
            return None;
        }
        if alpha <= 0.0 || beta <= 0.0 {
            return None;
        }
        Some(Self { alpha, beta })
    }

    /// Posterior mean E[p] = α / (α + β). Always finite for valid params.
    pub fn mean(&self) -> f64 {
        self.alpha / (self.alpha + self.beta)
    }

    /// Posterior variance Var[p] = αβ / ((α+β)² (α+β+1)).
    pub fn variance(&self) -> f64 {
        let sum = self.alpha + self.beta;
        (self.alpha * self.beta) / (sum * sum * (sum + 1.0))
    }

    /// Posterior standard deviation.
    pub fn std_dev(&self) -> f64 {
        self.variance().sqrt()
    }
}

/// Update a Beta prior with Bernoulli evidence: posterior = Beta(α + wins,
/// β + losses). Returns `None` if either count exceeds `u64::MAX / 2`
/// (would overflow when converted to f64 with full precision).
pub fn bayes_update(prior: BetaParams, wins: u64, losses: u64) -> Option<BetaParams> {
    // f64 has 53 bits of mantissa precision; counts beyond 2^53 lose
    // integer precision. We use 2^52 as a conservative cap.
    const CAP: u64 = 1u64 << 52;
    if wins > CAP || losses > CAP {
        return None;
    }
    let new_alpha = prior.alpha + wins as f64;
    let new_beta = prior.beta + losses as f64;
    BetaParams::new(new_alpha, new_beta)
}

/// Decide whether a candidate's posterior win-probability is high enough
/// to proceed. Returns `true` only if `posterior.mean() >= min_win_prob`
/// AND `posterior.std_dev() <= max_std_dev` (confidence guard).
///
/// `min_win_prob` and `max_std_dev` MUST be supplied by the operator —
/// no defaults. RULE 12: every threshold is explicit.
pub fn accept_by_posterior(
    posterior: &BetaParams,
    min_win_prob: f64,
    max_std_dev: f64,
) -> Option<bool> {
    if !min_win_prob.is_finite() || !(0.0..=1.0).contains(&min_win_prob) {
        return None;
    }
    if !max_std_dev.is_finite() || max_std_dev < 0.0 {
        return None;
    }
    Some(posterior.mean() >= min_win_prob && posterior.std_dev() <= max_std_dev)
}

// ---------------------------------------------------------------------------
// VPIN — Volume-Synchronized PIN (Easley-Lopez-Prado-O'Hara 2012)
// ---------------------------------------------------------------------------

/// Compute VPIN from a sequence of (buy_volume, sell_volume) buckets.
///
/// VPIN = sum(|buy_i - sell_i|) / sum(buy_i + sell_i)
///
/// Returns a value in `[0.0, 1.0]`. Higher = more toxic order flow.
/// Operator threshold (default 0.65 per SKILL_001) above which the
/// system should reduce participation.
///
/// Returns `None` when:
/// - `buckets` is empty
/// - any bucket has non-finite values
/// - total volume across all buckets is zero
pub fn vpin(buckets: &[(f64, f64)]) -> Option<f64> {
    if buckets.is_empty() {
        return None;
    }
    let mut imbalance_sum = 0.0;
    let mut volume_sum = 0.0;
    for &(buy, sell) in buckets {
        if !buy.is_finite() || !sell.is_finite() || buy < 0.0 || sell < 0.0 {
            return None;
        }
        imbalance_sum += (buy - sell).abs();
        volume_sum += buy + sell;
    }
    if volume_sum == 0.0 {
        return None;
    }
    let result = imbalance_sum / volume_sum;
    if !result.is_finite() {
        return None;
    }
    // VPIN is theoretically bounded to [0, 1]; floating-point may produce
    // 1.0 + eps. Clamp defensively.
    Some(result.clamp(0.0, 1.0))
}

/// Decide a size-reduction factor based on a measured PIN/VPIN value.
///
/// When `pin < threshold` returns 1.0 (no reduction). When
/// `pin >= threshold` returns `1.0 - reduction_strength` clamped to
/// `[0, 1]`. SKILL_001 suggests `threshold = 0.65, reduction_strength
/// = 0.6` (reduce 60% of quoting when PIN crosses 0.65).
///
/// Returns `None` if any parameter is invalid.
pub fn decide_size_reduction_for_pin(
    pin: f64,
    threshold: f64,
    reduction_strength: f64,
) -> Option<f64> {
    if !pin.is_finite() || !(0.0..=1.0).contains(&pin) {
        return None;
    }
    if !threshold.is_finite() || !(0.0..=1.0).contains(&threshold) {
        return None;
    }
    if !reduction_strength.is_finite() || !(0.0..=1.0).contains(&reduction_strength) {
        return None;
    }
    if pin < threshold {
        Some(1.0)
    } else {
        Some((1.0 - reduction_strength).clamp(0.0, 1.0))
    }
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

    // ── BetaParams construction ───────────────────────────────────────────

    #[test]
    fn beta_uniform_prior_constructs() {
        let b = BetaParams::new(1.0, 1.0).unwrap();
        assert!(approx(b.mean(), 0.5, 1e-12));
    }

    #[test]
    fn beta_rejects_zero_or_negative_params() {
        assert!(BetaParams::new(0.0, 1.0).is_none());
        assert!(BetaParams::new(1.0, 0.0).is_none());
        assert!(BetaParams::new(-1.0, 1.0).is_none());
    }

    #[test]
    fn beta_rejects_nan_or_inf() {
        assert!(BetaParams::new(f64::NAN, 1.0).is_none());
        assert!(BetaParams::new(1.0, f64::INFINITY).is_none());
    }

    // ── Posterior moments ────────────────────────────────────────────────

    #[test]
    fn beta_mean_known_value() {
        // Beta(3, 1): mean = 3/4 = 0.75.
        let b = BetaParams::new(3.0, 1.0).unwrap();
        assert!(approx(b.mean(), 0.75, 1e-12));
    }

    #[test]
    fn beta_variance_known_value() {
        // Beta(2, 2): var = (2*2)/((4)^2 * 5) = 4/80 = 0.05.
        let b = BetaParams::new(2.0, 2.0).unwrap();
        assert!(approx(b.variance(), 0.05, 1e-12));
    }

    #[test]
    fn beta_std_dev_is_sqrt_variance() {
        let b = BetaParams::new(2.0, 2.0).unwrap();
        let s = b.std_dev();
        assert!(approx(s * s, b.variance(), 1e-12));
    }

    // ── Bayesian update ──────────────────────────────────────────────────

    #[test]
    fn bayes_update_uniform_with_evidence() {
        // Prior Beta(1, 1) + 7 wins, 3 losses → Posterior Beta(8, 4).
        let prior = BetaParams::new(1.0, 1.0).unwrap();
        let post = bayes_update(prior, 7, 3).unwrap();
        assert!(approx(post.alpha, 8.0, 1e-12));
        assert!(approx(post.beta, 4.0, 1e-12));
        // Posterior mean = 8/12 ≈ 0.6667.
        assert!(approx(post.mean(), 8.0 / 12.0, 1e-12));
    }

    #[test]
    fn bayes_update_zero_evidence_returns_prior() {
        let prior = BetaParams::new(3.0, 2.0).unwrap();
        let post = bayes_update(prior, 0, 0).unwrap();
        assert_eq!(post, prior);
    }

    #[test]
    fn bayes_update_rejects_overflow_counts() {
        let prior = BetaParams::new(1.0, 1.0).unwrap();
        let too_big = (1u64 << 52) + 1;
        assert!(bayes_update(prior, too_big, 0).is_none());
        assert!(bayes_update(prior, 0, too_big).is_none());
    }

    // ── Decision: posterior thresholds ───────────────────────────────────

    #[test]
    fn accept_by_posterior_passes_when_mean_above_and_std_below() {
        // Beta(8, 4): mean = 0.667, std ≈ 0.131.
        let post = BetaParams::new(8.0, 4.0).unwrap();
        let accepted = accept_by_posterior(&post, 0.6, 0.2).unwrap();
        assert!(accepted);
    }

    #[test]
    fn accept_by_posterior_rejects_when_mean_below_threshold() {
        let post = BetaParams::new(2.0, 8.0).unwrap(); // mean = 0.2
        let accepted = accept_by_posterior(&post, 0.5, 1.0).unwrap();
        assert!(!accepted);
    }

    #[test]
    fn accept_by_posterior_rejects_when_std_too_high() {
        // Beta(1, 1): mean = 0.5, std ≈ 0.289 — too uncertain.
        let post = BetaParams::new(1.0, 1.0).unwrap();
        let accepted = accept_by_posterior(&post, 0.4, 0.1).unwrap();
        assert!(!accepted);
    }

    #[test]
    fn accept_by_posterior_rejects_invalid_threshold() {
        let post = BetaParams::new(1.0, 1.0).unwrap();
        assert!(accept_by_posterior(&post, 1.5, 0.1).is_none());
        assert!(accept_by_posterior(&post, f64::NAN, 0.1).is_none());
        assert!(accept_by_posterior(&post, 0.5, -0.1).is_none());
    }

    // ── VPIN ─────────────────────────────────────────────────────────────

    #[test]
    fn vpin_balanced_flow_near_zero() {
        // All buckets perfectly balanced — VPIN = 0.
        let buckets = vec![(100.0, 100.0); 10];
        let v = vpin(&buckets).unwrap();
        assert!(approx(v, 0.0, 1e-12));
    }

    #[test]
    fn vpin_one_sided_flow_equals_one() {
        // All buckets fully one-sided — VPIN = 1.
        let buckets = vec![(100.0, 0.0); 5];
        let v = vpin(&buckets).unwrap();
        assert!(approx(v, 1.0, 1e-12));
    }

    #[test]
    fn vpin_mixed_flow_in_range() {
        // 75% buy, 25% sell: |75-25|/(75+25) = 0.5 per bucket.
        let buckets = vec![(75.0, 25.0), (75.0, 25.0)];
        let v = vpin(&buckets).unwrap();
        assert!(approx(v, 0.5, 1e-12));
    }

    #[test]
    fn vpin_empty_buckets_rejected() {
        assert!(vpin(&[]).is_none());
    }

    #[test]
    fn vpin_zero_total_volume_rejected() {
        let buckets = vec![(0.0, 0.0); 5];
        assert!(vpin(&buckets).is_none());
    }

    #[test]
    fn vpin_negative_volume_rejected() {
        assert!(vpin(&[(-1.0, 1.0)]).is_none());
        assert!(vpin(&[(1.0, -1.0)]).is_none());
    }

    #[test]
    fn vpin_nan_volume_rejected() {
        assert!(vpin(&[(f64::NAN, 1.0)]).is_none());
    }

    // ── PIN-based size reduction ─────────────────────────────────────────

    #[test]
    fn pin_below_threshold_returns_full_size() {
        let factor = decide_size_reduction_for_pin(0.3, 0.65, 0.6).unwrap();
        assert!(approx(factor, 1.0, 1e-12));
    }

    #[test]
    fn pin_above_threshold_reduces_size() {
        // PIN 0.8 >= threshold 0.65, reduction 0.6 → factor = 0.4.
        let factor = decide_size_reduction_for_pin(0.8, 0.65, 0.6).unwrap();
        assert!(approx(factor, 0.4, 1e-12));
    }

    #[test]
    fn pin_at_threshold_triggers_reduction() {
        // PIN exactly at threshold → reduces (>= condition).
        let factor = decide_size_reduction_for_pin(0.65, 0.65, 0.5).unwrap();
        assert!(approx(factor, 0.5, 1e-12));
    }

    #[test]
    fn pin_full_reduction_clamps_to_zero() {
        let factor = decide_size_reduction_for_pin(0.9, 0.5, 1.0).unwrap();
        assert!(approx(factor, 0.0, 1e-12));
    }

    #[test]
    fn pin_invalid_inputs_rejected() {
        assert!(decide_size_reduction_for_pin(f64::NAN, 0.5, 0.5).is_none());
        assert!(decide_size_reduction_for_pin(0.5, 1.5, 0.5).is_none());
        assert!(decide_size_reduction_for_pin(0.5, 0.5, -0.1).is_none());
        assert!(decide_size_reduction_for_pin(-0.1, 0.5, 0.5).is_none());
    }
}
