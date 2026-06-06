//! Confidence scoring — wires the dead-code Bayesian posterior gate
//! (`bayesian_filter`) + Kelly sizing (`kelly_sizing`) primitives into the
//! per-opportunity scanner path, producing a `ConfidenceScore` on every paper
//! opportunity.
//!
//! ## Posture (OBSERVE-ONLY / shadow)
//! This is **advisory**: it produces + logs a `ConfidenceScore` for every
//! opportunity but does **NOT** gate emission — rejected candidates still flow
//! to the dashboard (RULE-00 transparency), so the system is never artificially
//! degraded. `recommended_size_wei` is a paper figure only; this module has NO
//! signer, makes NO broadcast, and moves NO capital (it just computes numbers).
//!
//! ## Honesty with no history
//! With `paper_trade_runs=0` there is no per-strategy win/loss history to seed a
//! posterior, so we fold THIS opportunity's REVM round-trip result (`sim_passed`)
//! in as a single Bernoulli observation against the operator's configured prior.
//! That yields a real per-opportunity posterior (a passed sim shifts E[p] up, a
//! failed one down) without fabricating data. Once A.5 paper-shadow accumulates
//! outcomes, the prior can be calibrated to real base rates (a follow-up).

use ethers::types::U256;

use crate::bayesian_filter::{accept_by_posterior, bayes_update, BetaParams};
use crate::kelly_sizing::{compute_position_size, kelly_fraction};

const ENV_PRIOR_ALPHA: &str = "ARBX_BAYESIAN_PRIOR_ALPHA";
const ENV_PRIOR_BETA: &str = "ARBX_BAYESIAN_PRIOR_BETA";
const ENV_MIN_POSTERIOR: &str = "ARBX_BAYESIAN_MIN_POSTERIOR";
const ENV_MAX_STD: &str = "ARBX_BAYESIAN_MAX_STD";
const ENV_KELLY_FRACTION: &str = "ARBX_KELLY_FRACTION";
const ENV_GAIN_ON_WIN: &str = "ARBX_SCORING_GAIN_ON_WIN";
const ENV_LOSS_ON_LOSS: &str = "ARBX_SCORING_LOSS_ON_LOSS";
const ENV_BANKROLL_WEI: &str = "ARBX_PAPER_BANKROLL_WEI";

// Conservative defaults. SKILL_021: Beta(2,2) is a weakly-informative prior
// (mean 0.5, mildly concentrated). Kelly fraction is a defensive cap; sizing is
// paper-only so this never risks capital.
const DEFAULT_PRIOR_ALPHA: f64 = 2.0;
const DEFAULT_PRIOR_BETA: f64 = 2.0;
const DEFAULT_MIN_POSTERIOR: f64 = 0.55;
const DEFAULT_MAX_STD: f64 = 0.25;
const DEFAULT_KELLY_FRACTION: f64 = 0.05; // 5% — never aggressive
const DEFAULT_GAIN_ON_WIN: f64 = 2.0;
const DEFAULT_LOSS_ON_LOSS: f64 = 1.0;
const DEFAULT_BANKROLL_WEI: u128 = 1_000_000_000_000_000_000; // 1e18 = 1 ETH paper NAV

/// Operator-tunable scoring parameters (all from env, conservative defaults).
#[derive(Clone, Debug)]
pub struct ScoringConfig {
    pub prior: BetaParams,
    pub min_posterior: f64,
    pub max_std: f64,
    pub kelly_fraction: f64,
    pub gain_on_win: f64,
    pub loss_on_loss: f64,
    pub bankroll_wei: U256,
}

impl ScoringConfig {
    pub fn from_env() -> Self {
        let alpha = env_f64(ENV_PRIOR_ALPHA, DEFAULT_PRIOR_ALPHA);
        let beta = env_f64(ENV_PRIOR_BETA, DEFAULT_PRIOR_BETA);
        // Validate the prior; fall back to the (valid) Beta(2,2) default via a
        // direct struct literal so we never need unwrap/expect (denied in bin/lib).
        let prior = BetaParams::new(alpha, beta).unwrap_or(BetaParams {
            alpha: DEFAULT_PRIOR_ALPHA,
            beta: DEFAULT_PRIOR_BETA,
        });
        let bankroll = env_u128(ENV_BANKROLL_WEI, DEFAULT_BANKROLL_WEI).max(1);
        Self {
            prior,
            min_posterior: env_f64(ENV_MIN_POSTERIOR, DEFAULT_MIN_POSTERIOR).clamp(0.0, 1.0),
            max_std: env_f64(ENV_MAX_STD, DEFAULT_MAX_STD).max(0.0),
            kelly_fraction: env_f64(ENV_KELLY_FRACTION, DEFAULT_KELLY_FRACTION).clamp(0.0, 1.0),
            gain_on_win: env_f64(ENV_GAIN_ON_WIN, DEFAULT_GAIN_ON_WIN).max(f64::MIN_POSITIVE),
            loss_on_loss: env_f64(ENV_LOSS_ON_LOSS, DEFAULT_LOSS_ON_LOSS).max(f64::MIN_POSITIVE),
            bankroll_wei: U256::from(bankroll),
        }
    }
}

/// The confidence attached to a paper opportunity. Advisory only.
#[derive(Clone, Debug, PartialEq)]
pub struct ConfidenceScore {
    /// Posterior win-probability E[p] after folding this opportunity's sim result.
    pub posterior_prob: f64,
    /// Posterior standard deviation (uncertainty).
    pub posterior_std: f64,
    /// Advisory Bayesian-gate verdict (posterior mean >= floor AND std <= cap).
    /// Does NOT gate emission.
    pub bayesian_accepted: bool,
    /// Posterior-informed Kelly fraction actually applied (capped).
    pub kelly_fraction: f64,
    /// Advisory recommended paper position size (wei). NEVER moves capital.
    pub recommended_size_wei: U256,
}

/// Compute a `ConfidenceScore` for one opportunity. `sim_passed` is the REVM
/// round-trip result, folded in as a single Bernoulli observation against the
/// prior. Invokes all four scoring primitives; never fails (degrades to safe
/// defaults), never moves capital.
pub fn compute_confidence(sim_passed: bool, cfg: &ScoringConfig) -> ConfidenceScore {
    let (wins, losses) = if sim_passed {
        (1u64, 0u64)
    } else {
        (0u64, 1u64)
    };
    // bayes_update(prior, 1, 0)/(prior, 0, 1) is always Some for these counts.
    let posterior = bayes_update(cfg.prior, wins, losses).unwrap_or(cfg.prior);
    let posterior_prob = posterior.mean();
    let posterior_std = posterior.std_dev();
    let bayesian_accepted =
        accept_by_posterior(&posterior, cfg.min_posterior, cfg.max_std).unwrap_or(false);

    // Posterior-informed Kelly fraction, capped at the configured ceiling.
    let kf = kelly_fraction(posterior_prob, cfg.gain_on_win, cfg.loss_on_loss)
        .map(|f| f.min(cfg.kelly_fraction))
        .unwrap_or(0.0);
    let recommended_size_wei = compute_position_size(cfg.bankroll_wei, kf).unwrap_or(U256::zero());

    ConfidenceScore {
        posterior_prob,
        posterior_std,
        bayesian_accepted,
        kelly_fraction: kf,
        recommended_size_wei,
    }
}

fn env_f64(key: &str, default: f64) -> f64 {
    std::env::var(key)
        .ok()
        .and_then(|v| v.trim().parse::<f64>().ok())
        .filter(|v| v.is_finite())
        .unwrap_or(default)
}
fn env_u128(key: &str, default: u128) -> u128 {
    std::env::var(key)
        .ok()
        .and_then(|v| v.trim().parse().ok())
        .unwrap_or(default)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cfg() -> ScoringConfig {
        ScoringConfig {
            prior: BetaParams::new(2.0, 2.0).unwrap(),
            min_posterior: 0.55,
            max_std: 0.25,
            kelly_fraction: 0.05,
            gain_on_win: 2.0,
            loss_on_loss: 1.0,
            bankroll_wei: U256::from(1_000_000_000_000_000_000u128),
        }
    }

    #[test]
    fn passed_sim_shifts_posterior_up_failed_down() {
        let pass = compute_confidence(true, &cfg());
        let fail = compute_confidence(false, &cfg());
        // Beta(2,2) -> passed Beta(3,2) mean 0.6; failed Beta(2,3) mean 0.4.
        assert!((pass.posterior_prob - 0.6).abs() < 1e-9);
        assert!((fail.posterior_prob - 0.4).abs() < 1e-9);
        assert!(pass.posterior_prob > fail.posterior_prob);
    }

    #[test]
    fn size_is_positive_and_capped() {
        let c = compute_confidence(true, &cfg());
        // Kelly capped at 0.05 of 1e18 = 5e16 wei max; must be > 0 and <= cap.
        assert!(c.recommended_size_wei > U256::zero());
        assert!(c.recommended_size_wei <= U256::from(50_000_000_000_000_000u128));
        assert!(c.kelly_fraction <= 0.05);
    }

    #[test]
    fn never_panics_on_degenerate_prior_via_struct_literal() {
        // A degenerate prior can't occur through from_env (it validates), but the
        // compute path must stay finite for any valid BetaParams.
        let mut c = cfg();
        c.prior = BetaParams {
            alpha: 1.0,
            beta: 1.0,
        };
        let s = compute_confidence(false, &c);
        assert!(s.posterior_prob.is_finite() && s.posterior_std.is_finite());
    }

    #[test]
    fn advisory_accept_does_not_imply_emission() {
        // Documented invariant: bayesian_accepted is advisory. A failed sim with a
        // low posterior is simply not "accepted", but the worker still emits it.
        let fail = compute_confidence(false, &cfg());
        assert!(!fail.bayesian_accepted); // 0.4 < 0.55 floor
    }
}
