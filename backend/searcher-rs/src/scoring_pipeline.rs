//! Gate C — confidence scoring pipeline (adapter over the real primitives).
//!
//! This is the **adaptation layer** that wires the dead-code Bayesian posterior
//! gate (`bayesian_filter`) + Kelly sizing (`kelly_sizing`) into the real PAPER
//! opportunity emission path, producing a [`ConfidenceScore`] per opportunity.
//! It is NOT a rewrite of the scanner — `OpportunityEmitter::emit_accepted`
//! invokes [`ScoringPipeline::evaluate_paper_opportunity`] and XADDs the result
//! to the Redis stream `arbx:scoring:scored`; a passive api-server archiver
//! consumes it into Postgres `scored_opportunities`.
//!
//! ## Posture (OBSERVE-ONLY / shadow — INVIOLABLE)
//! Advisory only. With `ARBX_SCORING_HARD_GATE=false` (default) it NEVER blocks
//! emission — rejected candidates still flow downstream (RULE-00 transparency),
//! so flat priors never empty the dashboard. No signer, no broadcast, no capital.
//!
//! ## `recommended_position_usd` is HYPOTHETICAL PAPER capital
//! It is computed by feeding a hypothetical paper bankroll cap
//! (`ARBX_KELLY_MAX_CAPITAL_USD`, scaled to integer micro-dollars) through the
//! REAL `kelly_sizing::compute_position_size` wei primitive, then converting the
//! resulting micro-dollars back to USD. It is a paper sizing figure ONLY — never
//! real money, never a position taken, never broadcast. The wei primitive is
//! reused (not reimplemented) so the integer ppm math is identical to production.
//!
//! ## Honesty with no history (flat prior)
//! The posterior is built from per-pair history via the REAL
//! `bayesian_filter::bayes_update(Beta(1,1), profitable, unprofitable)`. With no
//! calibrated prior (`observation_count == 0`) this collapses to the uniform
//! Beta(1,1) flat prior (mean 0.5) and `source_context = "flat_prior"`. No win is
//! fabricated. Calibration only happens once the A.5 paper-shadow window
//! populates `bayesian_priors` — at which point `source_context = "calibrated"`.

use ethers::types::U256;

use crate::bayesian_filter::{accept_by_posterior, bayes_update, BetaParams};
use crate::kelly_sizing::{compute_position_size, kelly_fraction};

// Uniform / uninformative flat prior (Beta(1,1)). Const struct literal so we
// never need unwrap/expect (denied crate-wide outside tests).
const FLAT_PRIOR: BetaParams = BetaParams {
    alpha: 1.0,
    beta: 1.0,
};

// --- env keys -------------------------------------------------------------
const ENV_ENABLED: &str = "ARBX_SCORING_ENABLED";
const ENV_HARD_GATE: &str = "ARBX_SCORING_HARD_GATE";
const ENV_MIN_POSTERIOR: &str = "ARBX_BAYESIAN_MIN_POSTERIOR";
const ENV_MAX_STD: &str = "ARBX_BAYESIAN_MAX_STD";
const ENV_KELLY_FRACTION: &str = "ARBX_KELLY_FRACTION";
const ENV_KELLY_MAX_CAPITAL_USD: &str = "ARBX_KELLY_MAX_CAPITAL_USD";
const ENV_GAIN_ON_WIN: &str = "ARBX_SCORING_GAIN_ON_WIN";
const ENV_LOSS_ON_LOSS: &str = "ARBX_SCORING_LOSS_ON_LOSS";

// --- conservative defaults (match the spec) -------------------------------
const DEFAULT_MIN_POSTERIOR: f64 = 0.50;
const DEFAULT_MAX_STD: f64 = 1.00;
const DEFAULT_KELLY_FRACTION: f64 = 0.25;
const DEFAULT_KELLY_MAX_CAPITAL_USD: f64 = 5000.0;
const DEFAULT_GAIN_ON_WIN: f64 = 2.0;
const DEFAULT_LOSS_ON_LOSS: f64 = 1.0;

/// Per-pair prior state loaded from the `bayesian_priors` table (or `None` when
/// the pair has never been calibrated → flat prior).
#[derive(Clone, Copy, Debug)]
pub struct PriorState {
    pub observation_count: u64,
    pub profitable_count: u64,
    pub log_odds: f64,
}

/// Operator-tunable Gate-C scoring parameters (all env-driven, safe defaults).
#[derive(Clone, Copy, Debug)]
pub struct GateCScoringConfig {
    pub enabled: bool,
    pub hard_gate: bool,
    pub min_posterior: f64,
    pub max_std: f64,
    pub kelly_fraction_cap: f64,
    pub kelly_max_capital_usd: f64,
    pub gain_on_win: f64,
    pub loss_on_loss: f64,
}

impl GateCScoringConfig {
    pub fn from_env() -> Self {
        Self {
            enabled: env_bool(ENV_ENABLED, true),
            hard_gate: env_bool(ENV_HARD_GATE, false),
            min_posterior: env_f64(ENV_MIN_POSTERIOR, DEFAULT_MIN_POSTERIOR).clamp(0.0, 1.0),
            max_std: env_f64(ENV_MAX_STD, DEFAULT_MAX_STD).max(0.0),
            kelly_fraction_cap: env_f64(ENV_KELLY_FRACTION, DEFAULT_KELLY_FRACTION).clamp(0.0, 1.0),
            kelly_max_capital_usd: env_f64(
                ENV_KELLY_MAX_CAPITAL_USD,
                DEFAULT_KELLY_MAX_CAPITAL_USD,
            )
            .max(0.0),
            gain_on_win: env_f64(ENV_GAIN_ON_WIN, DEFAULT_GAIN_ON_WIN).max(f64::MIN_POSITIVE),
            loss_on_loss: env_f64(ENV_LOSS_ON_LOSS, DEFAULT_LOSS_ON_LOSS).max(f64::MIN_POSITIVE),
        }
    }
}

/// The confidence attached to one PAPER opportunity. Advisory only.
#[derive(Clone, Debug, PartialEq)]
pub struct ConfidenceScore {
    /// Posterior win-probability E[p] for the pair (flat 0.5 until calibrated).
    pub posterior_prob: f64,
    /// Posterior-informed Kelly fraction actually applied (capped).
    pub kelly_fraction: f64,
    /// HYPOTHETICAL paper position size (USD). NEVER real capital.
    pub recommended_position_usd: f64,
    /// Advisory Bayesian-gate verdict. Does NOT gate emission unless HARD_GATE.
    pub bayesian_accepted: bool,
    /// ln(p/(1-p)) of the posterior mean (0.0 at the flat prior).
    pub prior_log_odds: f64,
    /// `"flat_prior"` when uncalibrated, `"calibrated"` once history exists.
    pub source_context: String,
}

/// Gate-C scoring pipeline. Holds the parsed config; cheap to clone/copy.
#[derive(Clone, Copy, Debug)]
pub struct ScoringPipeline {
    cfg: GateCScoringConfig,
}

impl ScoringPipeline {
    pub fn from_env() -> Self {
        Self {
            cfg: GateCScoringConfig::from_env(),
        }
    }

    pub fn config(&self) -> &GateCScoringConfig {
        &self.cfg
    }

    pub fn enabled(&self) -> bool {
        self.cfg.enabled
    }

    pub fn hard_gate(&self) -> bool {
        self.cfg.hard_gate
    }

    /// ADVISORY predicate: whether a future LIVE execution layer would emit this
    /// downstream. `HARD_GATE=false` (default) ⇒ always true. `HARD_GATE=true`
    /// ⇒ only when the Bayesian gate accepted it. In the current SHADOW build
    /// this is informational only — `opps:detected` always receives the
    /// opportunity (RULE 00). The score is always produced + persisted.
    pub fn should_emit_downstream(&self, score: &ConfidenceScore) -> bool {
        !self.cfg.hard_gate || score.bayesian_accepted
    }

    /// Compute a [`ConfidenceScore`] for one paper opportunity. `prior` is the
    /// per-pair calibrated state (or `None` ⇒ flat prior). Invokes the real
    /// primitives; never fails (degrades to safe defaults), never moves capital.
    ///
    /// Async to match the wiring point (`emit_accepted` is async) and to allow a
    /// future async prior-loader; the math itself is pure.
    #[allow(clippy::unused_async)]
    pub async fn evaluate_paper_opportunity(
        &self,
        opportunity_id: &str,
        token_pair: &str,
        net_profit_usd: Option<f64>,
        chain_id: Option<i64>,
        prior: Option<PriorState>,
    ) -> anyhow::Result<ConfidenceScore> {
        tracing::debug!(
            event = "scoring.evaluate.start",
            opportunity_id,
            token_pair,
            chain_id,
            net_profit_usd,
            calibrated = prior.map(|p| p.observation_count > 0).unwrap_or(false),
        );
        let score = compute_confidence(&self.cfg, prior.as_ref());
        if score.bayesian_accepted {
            tracing::debug!(
                event = "scoring.bayesian.accepted",
                token_pair,
                posterior_prob = score.posterior_prob
            );
        } else {
            tracing::debug!(
                event = "scoring.bayesian.rejected",
                token_pair,
                posterior_prob = score.posterior_prob
            );
        }
        tracing::debug!(
            event = "scoring.kelly.computed",
            token_pair,
            kelly_fraction = score.kelly_fraction,
            recommended_position_usd = score.recommended_position_usd,
        );
        Ok(score)
    }
}

/// Pure scoring core (sync, fully testable). Builds the posterior from history
/// via the real `bayes_update`, runs the real `accept_by_posterior`, the real
/// `kelly_fraction`, and the real `compute_position_size` (over a hypothetical
/// paper bankroll). Never panics; no unwrap/expect.
fn compute_confidence(cfg: &GateCScoringConfig, prior: Option<&PriorState>) -> ConfidenceScore {
    let (wins, losses, source_context) = match prior {
        Some(p) if p.observation_count > 0 => {
            let unprofitable = p.observation_count.saturating_sub(p.profitable_count);
            (p.profitable_count, unprofitable, "calibrated")
        }
        _ => (0u64, 0u64, "flat_prior"),
    };
    // bayes_update(Beta(1,1), wins, losses) = Beta(1+wins, 1+losses). At (0,0)
    // this is exactly the flat prior. Uses the real primitive.
    let posterior = bayes_update(FLAT_PRIOR, wins, losses).unwrap_or(FLAT_PRIOR);
    let posterior_prob = posterior.mean();
    let bayesian_accepted =
        accept_by_posterior(&posterior, cfg.min_posterior, cfg.max_std).unwrap_or(false);

    let kf = kelly_fraction(posterior_prob, cfg.gain_on_win, cfg.loss_on_loss)
        .map(|f| f.min(cfg.kelly_fraction_cap))
        .unwrap_or(0.0);

    let recommended_position_usd = recommended_usd_from_kelly(cfg.kelly_max_capital_usd, kf);
    let prior_log_odds = log_odds(posterior_prob);

    ConfidenceScore {
        posterior_prob,
        kelly_fraction: kf,
        recommended_position_usd,
        bayesian_accepted,
        prior_log_odds,
        source_context: source_context.to_string(),
    }
}

/// Convert a Kelly fraction into a HYPOTHETICAL paper USD size by reusing the
/// real wei primitive: feed `capital_usd` as integer micro-dollars into
/// `compute_position_size`, then scale the result back to USD. Paper only.
fn recommended_usd_from_kelly(capital_usd: f64, fraction: f64) -> f64 {
    if !capital_usd.is_finite() || capital_usd <= 0.0 {
        return 0.0;
    }
    let micros = (capital_usd * 1_000_000.0).floor();
    if !micros.is_finite() || micros < 1.0 {
        return 0.0;
    }
    let nav = U256::from(micros as u128);
    match compute_position_size(nav, fraction) {
        // value ≤ nav ≤ u128::MAX, so low_u128() is exact (no panic, no unwrap).
        Some(sz) => (sz.low_u128() as f64) / 1_000_000.0,
        None => 0.0,
    }
}

/// ln(p/(1-p)) with clamping so the log-odds stay finite at the extremes.
fn log_odds(p: f64) -> f64 {
    let clamped = p.clamp(1e-9, 1.0 - 1e-9);
    (clamped / (1.0 - clamped)).ln()
}

fn env_f64(key: &str, default: f64) -> f64 {
    std::env::var(key)
        .ok()
        .and_then(|v| v.trim().parse::<f64>().ok())
        .filter(|v| v.is_finite())
        .unwrap_or(default)
}

fn env_bool(key: &str, default: bool) -> bool {
    match std::env::var(key) {
        Ok(v) => matches!(
            v.trim().to_ascii_lowercase().as_str(),
            "1" | "true" | "yes" | "on"
        ),
        Err(_) => default,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cfg() -> GateCScoringConfig {
        GateCScoringConfig {
            enabled: true,
            hard_gate: false,
            min_posterior: 0.50,
            max_std: 1.00,
            kelly_fraction_cap: 0.25,
            kelly_max_capital_usd: 5000.0,
            gain_on_win: 2.0,
            loss_on_loss: 1.0,
        }
    }

    #[test]
    fn flat_prior_produces_score() {
        let s = compute_confidence(&cfg(), None);
        assert_eq!(s.source_context, "flat_prior");
        // Beta(1,1) mean = 0.5; log-odds 0.
        assert!((s.posterior_prob - 0.5).abs() < 1e-9);
        assert!(s.prior_log_odds.abs() < 1e-9);
        assert!(s.posterior_prob.is_finite() && s.recommended_position_usd.is_finite());
    }

    #[test]
    fn recommended_usd_positive_and_capped() {
        let s = compute_confidence(&cfg(), None);
        // Flat posterior 0.5: kelly = 0.5/1 - 0.5/2 = 0.25, capped at 0.25.
        assert!((s.kelly_fraction - 0.25).abs() < 1e-9);
        // 0.25 * $5000 = $1250 hypothetical paper size; > 0 and ≤ capital.
        assert!(s.recommended_position_usd > 0.0);
        assert!(s.recommended_position_usd <= 5000.0);
        assert!((s.recommended_position_usd - 1250.0).abs() < 1.0);
    }

    #[test]
    fn hard_gate_false_never_blocks() {
        let p = ScoringPipeline { cfg: cfg() };
        // Even an explicitly rejected score emits when hard_gate is off.
        let rejected = ConfidenceScore {
            posterior_prob: 0.1,
            kelly_fraction: 0.0,
            recommended_position_usd: 0.0,
            bayesian_accepted: false,
            prior_log_odds: -2.0,
            source_context: "flat_prior".to_string(),
        };
        assert!(p.should_emit_downstream(&rejected));
    }

    #[test]
    fn hard_gate_true_flags_rejected_advisory() {
        let mut c = cfg();
        c.hard_gate = true;
        let p = ScoringPipeline { cfg: c };
        let rejected = ConfidenceScore {
            posterior_prob: 0.1,
            kelly_fraction: 0.0,
            recommended_position_usd: 0.0,
            bayesian_accepted: false,
            prior_log_odds: -2.0,
            source_context: "calibrated".to_string(),
        };
        let accepted = ConfidenceScore {
            bayesian_accepted: true,
            ..rejected.clone()
        };
        assert!(!p.should_emit_downstream(&rejected)); // advisory: would-gate in a live layer
        assert!(p.should_emit_downstream(&accepted));
        // ...but the score is still a real value (telemetry preserved); in the
        // shadow build opps:detected emission is unaffected by this flag.
        assert_eq!(rejected.posterior_prob, 0.1);
    }

    #[test]
    fn calibrated_profitable_history_shifts_posterior_up() {
        // 80 wins / 100 obs → Beta(81,21) mean ≈ 0.794 > flat 0.5.
        let prior = PriorState {
            observation_count: 100,
            profitable_count: 80,
            log_odds: 0.0,
        };
        let s = compute_confidence(&cfg(), Some(&prior));
        assert_eq!(s.source_context, "calibrated");
        assert!(s.posterior_prob > 0.7);
        assert!(s.bayesian_accepted); // mean 0.79 ≥ 0.50 floor
    }

    #[test]
    fn degenerate_inputs_never_panic() {
        let mut c = cfg();
        c.kelly_max_capital_usd = 0.0; // no bankroll
        let s = compute_confidence(&c, None);
        assert_eq!(s.recommended_position_usd, 0.0);
        assert!(s.posterior_prob.is_finite() && s.prior_log_odds.is_finite());
    }
}
