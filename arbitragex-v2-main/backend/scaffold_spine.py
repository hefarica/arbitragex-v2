import os

spine_dir = r"c:\Users\HFRC\Desktop\arbitragex_v2_productivo_full\backend\prioritization-spine"
os.makedirs(os.path.join(spine_dir, "src"), exist_ok=True)
os.makedirs(os.path.join(spine_dir, "tests"), exist_ok=True)

cargo_toml = """[package]
name = "prioritization-spine"
version.workspace = true
edition.workspace = true
rust-version.workspace = true

[dependencies]
shared-rs = { workspace = true }
serde = { workspace = true }
serde_json = { workspace = true }
thiserror = { workspace = true }
tracing = { workspace = true }
config = { workspace = true }
chrono = { workspace = true }
"""
with open(os.path.join(spine_dir, "Cargo.toml"), "w", encoding='utf-8') as f:
    f.write(cargo_toml)

lib_rs = """pub mod types;
pub mod evidence;
pub mod scoring;
pub mod gates;
pub mod decision;
pub mod feedback;
pub mod errors;

pub use types::*;
pub use evidence::*;
pub use scoring::*;
pub use gates::*;
pub use decision::*;
pub use errors::*;
"""
with open(os.path.join(spine_dir, "src", "lib.rs"), "w", encoding='utf-8') as f:
    f.write(lib_rs)

types_rs = """use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpportunityCandidate {
    pub route_fingerprint: String,
    pub pool_addresses: Vec<String>,
    pub token_addresses: Vec<String>,
    pub dex_adapters: Vec<String>,
    pub amount_in: f64,
    pub expected_amount_out: f64,
    pub gross_profit: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpportunityScore {
    pub net_expected_profit: f64,
    pub landing_probability: f64,
    pub state_freshness: f64,
    pub liquidity_confidence: f64,
    pub execution_atomicity: f64,
    pub computational_cost: f64,
    pub reversal_risk: f64,
    pub slippage_risk: f64,
    pub gas_volatility_risk: f64,
    pub token_risk: f64,
    pub final_score: f64,
}
"""
with open(os.path.join(spine_dir, "src", "types.rs"), "w", encoding='utf-8') as f:
    f.write(types_rs)

evidence_rs = """use serde::{Deserialize, Serialize};
use crate::decision::{ExecutionDecision, RejectReason};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpportunityEvidence {
    pub chain_id: u64,
    pub block_number: u64,
    pub rpc_url_hash: String,
    pub rpc_latency_ms: u64,
    pub state_read_timestamp: i64,
    pub pool_addresses: Vec<String>,
    pub token_addresses: Vec<String>,
    pub dex_adapters: Vec<String>,
    pub route_fingerprint: String,
    pub amount_in: f64,
    pub expected_amount_out: f64,
    pub min_amount_out: f64,
    pub gross_profit: f64,
    pub gas_units_estimated: u64,
    pub gas_price: f64,
    pub gas_cost: f64,
    pub bribe: f64,
    pub flashloan_fee: f64,
    pub net_expected_profit: f64,
    pub roi_net: f64,
    pub simulation_status: String,
    pub simulation_trace_hash: Option<String>,
    pub bundle_simulation_status: Option<String>,
    pub token_risk_score: f64,
    pub liquidity_confidence: f64,
    pub state_freshness_ms: u64,
    pub landing_probability: f64,
    pub final_score: f64,
    pub decision: ExecutionDecision,
    pub reject_reason: Option<RejectReason>,
}
"""
with open(os.path.join(spine_dir, "src", "evidence.rs"), "w", encoding='utf-8') as f:
    f.write(evidence_rs)

decision_rs = """use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ExecutionDecision {
    Execute,
    SimulateDeeper,
    Hold,
    Reject,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum RejectReason {
    NegativeNetProfit,
    LowLandingProbability,
    StaleState,
    LowLiquidity,
    ExcessiveSlippage,
    HighGasVolatility,
    HighTokenRisk,
    SimulationFailed,
    BundleSimulationFailed,
    MissingEvidence,
    RouteTooLong,
    PoolNotTrusted,
    RpcUnhealthy,
    OracleMismatch,
    ReorgRiskTooHigh,
}

pub trait ExecutionDecisionEngine {
    fn decide(&self, score: f64) -> ExecutionDecision;
}
"""
with open(os.path.join(spine_dir, "src", "decision.rs"), "w", encoding='utf-8') as f:
    f.write(decision_rs)

scoring_rs = """use crate::types::{OpportunityCandidate, OpportunityScore};
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
"""
with open(os.path.join(spine_dir, "src", "scoring.rs"), "w", encoding='utf-8') as f:
    f.write(scoring_rs)

gates_rs = """use crate::types::OpportunityCandidate;
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
"""
with open(os.path.join(spine_dir, "src", "gates.rs"), "w", encoding='utf-8') as f:
    f.write(gates_rs)

errors_rs = """use thiserror::Error;

#[derive(Error, Debug)]
pub enum ScoringError {
    #[error("Negative expected profit")]
    NegativeProfit,
    #[error("Missing data required for scoring")]
    MissingData,
}

#[derive(Error, Debug)]
pub enum GateError {
    #[error("Validation failed")]
    ValidationFailed,
}
"""
with open(os.path.join(spine_dir, "src", "errors.rs"), "w", encoding='utf-8') as f:
    f.write(errors_rs)

feedback_rs = """pub trait FeedbackLoop {
    fn record_outcome(&self, outcome_hash: &str) -> Result<(), String>;
}
"""
with open(os.path.join(spine_dir, "src", "feedback.rs"), "w", encoding='utf-8') as f:
    f.write(feedback_rs)

print("Scaffolded prioritization-spine crate.")
