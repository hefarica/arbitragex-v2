pub mod adapters;
pub mod bayesian_stator;
pub mod capital_quantum;
pub mod carnot_orchestrator;
pub mod entropy_sink;
pub mod impedance_tensor;
pub mod liquidity_curvature;
pub mod potential_field;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PotentialGradient {
    pub token_in: String,
    pub token_out: String,
    pub potential_delta_usd: f64,
    pub venue_in: String,
    pub venue_out: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DissipationMetrics {
    pub gas_usd: f64,
    pub fee_bps: u32,
    pub latency_ms: u64,
    pub decoherence_usd: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PermittedCycle {
    pub id: String,
    pub chain_id: u64,
    pub detected_at: chrono::DateTime<chrono::Utc>,
    pub eta: f64,
    pub work_extracted_usd: f64,
    pub heat_in_usd: f64,
    pub heat_out_usd: f64,
    pub gradient: PotentialGradient,
    pub dissipation: DissipationMetrics,
    pub status: CycleStatus,
    pub rejection_reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CycleStatus {
    Detected,
    Simulated,
    PaperExecuted,
    Rejected,
}

#[derive(Debug, Clone)]
pub struct ThermodynamicCycle {
    pub gradient: PotentialGradient,
    pub heat_in_usd: f64,
    pub impedance: ImpedanceSnapshot,
}

#[derive(Debug, Clone)]
pub struct ImpedanceSnapshot {
    pub gas_usd: f64,
    pub fee_bps: u32,
    pub latency_ms: u64,
    pub decoherence_usd: f64,
}

#[derive(Debug, thiserror::Error)]
pub enum EntropyError {
    #[error("cycle violates second law: eta <= 0")]
    SecondLawViolation,
    #[error("negative work extracted")]
    NegativeWork,
}
