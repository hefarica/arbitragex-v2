use crate::decision::{ExecutionDecision, RejectReason};
use crate::evidence::OpportunityEvidence;

pub struct EvidenceGate {}

impl EvidenceGate {
    pub fn validate(evidence: &OpportunityEvidence) -> Option<RejectReason> {
        if evidence.net_expected_profit <= 0.0 {
            return Some(RejectReason::NegativeNetProfit);
        }
        // Phase A.1/A.2 — accept legacy "PASS" (until prioritization-spine v1
        // stub is fully removed) AND the new fail-honest "SIM_SUCCESS" sentinel
        // emitted by the simulator-v2 dispatch (Phase A.3+).
        //
        // Whitelist semantics: every other value — including the new
        // "SIM_DISABLED_FAIL_CLOSED" emitted while the encoder is pending —
        // maps to `RejectReason::SimulationFailed`. That is the only honest
        // outcome until Phase A.3 produces real REVM verdicts (RULE 12 + RULE 15).
        if !matches!(evidence.simulation_status.as_str(), "PASS" | "SIM_SUCCESS") {
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
    if EvidenceGate::validate(evidence).is_some() {
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
