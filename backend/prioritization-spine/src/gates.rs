use crate::types::OpportunityCandidate;
use crate::evidence::OpportunityEvidence;
use crate::decision::{ExecutionDecision, RejectReason};

pub struct EvidenceGate {}

impl EvidenceGate {
    pub fn validate(evidence: &OpportunityEvidence) -> Option<RejectReason> {
        if evidence.net_expected_profit <= 0.0 {
            return Some(RejectReason::NegativeNetProfit);
        }
        if evidence.simulation_status != "PASS" {
            return Some(RejectReason::SimulationFailed);
        }
        if evidence.landing_probability < 0.5 {
            return Some(RejectReason::LowLandingProbability);
        }
        if evidence.state_freshness_ms > 12000 {
            return Some(RejectReason::StaleState);
        }
        None
    }
}

pub fn can_execute(evidence: &OpportunityEvidence, shadow_mode: bool) -> ExecutionDecision {
    if let Some(reason) = EvidenceGate::validate(evidence) {
        return ExecutionDecision::Reject;
    }
    
    if shadow_mode {
        return ExecutionDecision::Hold;
    }

    if evidence.final_score > 80.0 {
        ExecutionDecision::Execute
    } else {
        ExecutionDecision::SimulateDeeper
    }
}
