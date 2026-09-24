use crate::errors::ScoringError;
use crate::evidence::OpportunityEvidence;
use crate::types::{OpportunityCandidate, OpportunityScore};

pub trait OpportunityScorer {
    fn score(
        &self,
        candidate: &OpportunityCandidate,
        evidence: &OpportunityEvidence,
    ) -> Result<OpportunityScore, ScoringError>;
}

pub struct PrioritizationEngine {
    pub min_profit_threshold: f64,
}

impl OpportunityScorer for PrioritizationEngine {
    fn score(
        &self,
        _candidate: &OpportunityCandidate,
        evidence: &OpportunityEvidence,
    ) -> Result<OpportunityScore, ScoringError> {
        let net_expected =
            evidence.gross_profit - evidence.gas_cost - evidence.bribe - evidence.flashloan_fee;
        // MATH-07 fix (2026-09-24): NaN passed through `<= 0.0` (false for
        // NaN) and produced a NaN final_score that broke the ranker's total
        // order. Now: any non-finite input is an explicit Err, never a NaN.
        if !net_expected.is_finite() {
            return Err(ScoringError::InvalidEvidence);
        }
        if net_expected <= 0.0 {
            return Err(ScoringError::NegativeProfit);
        }
        if !evidence.landing_probability.is_finite()
            || !evidence.liquidity_confidence.is_finite()
            || !evidence.token_risk_score.is_finite()
        {
            return Err(ScoringError::InvalidEvidence);
        }

        // MATH-07 (second half): the raw division by state_freshness_ms makes
        // score ∝ 1/freshness — a 1ms-old $1 opportunity outranks a 50ms-old
        // $10 one by 5×. Normalize freshness to a bounded [0,1] decay
        // (exp(−ms/τ), τ = 60s): fresh ≈ 1.0, 60s-old ≈ 0.37, stale → 0.
        // The denominator floor (max 1.0) still guards against div-by-0.
        const FRESHNESS_TAU_MS: f64 = 60_000.0;
        let freshness_factor = (-(evidence.state_freshness_ms as f64) / FRESHNESS_TAU_MS).exp();
        let final_score = net_expected
            * evidence.landing_probability
            * evidence.liquidity_confidence
            * freshness_factor
            / evidence.token_risk_score.max(1.0);

        if !final_score.is_finite() {
            return Err(ScoringError::InvalidEvidence);
        }

        Ok(OpportunityScore {
            net_expected_profit: net_expected,
            landing_probability: evidence.landing_probability,
            state_freshness: evidence.state_freshness_ms as f64,
            liquidity_confidence: evidence.liquidity_confidence,
            execution_atomicity: 1.0,
            computational_cost: 1.0,
            reversal_risk: 1.0,
            slippage_risk: 1.0,
            gas_volatility_risk: 1.0,
            token_risk: evidence.token_risk_score,
            final_score,
        })
    }
}
