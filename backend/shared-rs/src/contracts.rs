//! Canonical data contracts. Mirror `configs/schemas/*.json`.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum StrategyKind {
    DexArb,
    Triangular,
    Backrun,
    Liquidation,
    FlashloanArb,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Opportunity {
    pub id: Uuid,
    pub chain_id: u64,
    pub strategy_kind: StrategyKind,
    pub dex_a: String,
    pub dex_b: Option<String>,
    pub pair_symbol: String,
    pub token_in: String,
    pub token_out: String,
    /// big int as decimal string
    pub amount_in_wei: String,
    pub expected_profit_usd: f64,
    pub roi_pct: Option<f64>,
    pub risk_score: Option<f64>,
    pub block_number: Option<u64>,
    pub detected_at: DateTime<Utc>,
    pub trace_id: Uuid,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SimulatorKind {
    Anvil,
    Tenderly,
    Hardhat,
    NotImplemented,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SimulationResult {
    pub opportunity_id: Uuid,
    pub passed: bool,
    pub gas_estimate_wei: Option<String>,
    pub gas_price_wei: Option<String>,
    pub slippage_pct: Option<f64>,
    pub revert_risk_pct: Option<f64>,
    pub simulated_profit_usd: Option<f64>,
    pub simulator: SimulatorKind,
    pub fail_reason: Option<String>,
    pub simulated_at: DateTime<Utc>,
    pub trace_id: Uuid,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExecutionStatus {
    Submitted,
    Included,
    Reverted,
    Dropped,
    Replaced,
    NotImplemented,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionResult {
    pub opportunity_id: Uuid,
    pub status: ExecutionStatus,
    pub tx_hash: Option<String>,
    pub relay_used: Option<String>,
    pub block_included: Option<u64>,
    pub gas_used_wei: Option<String>,
    pub actual_profit_usd: Option<f64>,
    pub error_message: Option<String>,
    pub submitted_at: DateTime<Utc>,
    pub trace_id: Uuid,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReconReport {
    pub opportunity_id: Uuid,
    pub expected_profit_usd: f64,
    pub actual_profit_usd: f64,
    pub variance_usd: f64,
    pub variance_pct: f64,
    pub actual_gas_used_wei: Option<String>,
    pub notes: Option<String>,
    pub created_at: DateTime<Utc>,
    pub trace_id: Uuid,
}

/// Canonical 501 payload for unimplemented paths.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NotImplementedPayload {
    pub error: &'static str,
    pub requires: Vec<&'static str>,
    pub sprint: &'static str,
    pub detail: String,
}

impl NotImplementedPayload {
    pub fn new(requires: Vec<&'static str>, sprint: &'static str, detail: impl Into<String>) -> Self {
        Self { error: "not_implemented", requires, sprint, detail: detail.into() }
    }
}
