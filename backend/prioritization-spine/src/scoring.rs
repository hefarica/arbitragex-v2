use crate::types::{OpportunityCandidate, OpportunityScore};
use crate::evidence::OpportunityEvidence;
use crate::errors::ScoringError;

pub trait OpportunityScorer {
    fn score(&self, candidate: &OpportunityCandidate, evidence: &OpportunityEvidence) -> Result<OpportunityScore, ScoringError>;
}

pub struct PrioritizationEngine {
    pub min_profit_threshold: f64,
}

impl OpportunityScorer for PrioritizationEngine {
    fn score(&self, _candidate: &OpportunityCandidate, evidence: &OpportunityEvidence) -> Result<OpportunityScore, ScoringError> {
        let net_expected = evidence.gross_profit - evidence.gas_cost - evidence.bribe - evidence.flashloan_fee;
        if net_expected <= 0.0 {
            return Err(ScoringError::NegativeProfit);
        }

        let final_score = (net_expected * evidence.landing_probability * evidence.liquidity_confidence) 
                        / (evidence.state_freshness_ms as f64 * evidence.token_risk_score).max(1.0);

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
