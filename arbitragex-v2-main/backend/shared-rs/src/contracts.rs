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
    /// **GROSS** profit in USD as emitted by scanner.rs — before gas, relay fees,
    /// LP fees, slippage, failure buffer, or any other cost component.
    /// Set by `compute_gross_usd_for_spread()` (renamed from `compute_usd_profit_for_spread`).
    /// None when the oracle could not price the tokens (R8 fail-honest).
    ///
    /// **Do NOT use this field as the net profit figure.** It overstates realised
    /// profit by 20-40% on Ethereum mainnet (relay bribe alone is 10-50% of gross).
    /// The canonical net figure after all 8 cost components is
    /// `net_expected_profit_usd` (populated by the spine evaluator). The
    /// pre-execute checklist Check 7 uses `net_expected_profit_usd`; this field
    /// is the DB-persistent gross column and must not be removed or renamed.
    pub expected_profit_usd: Option<f64>,
    /// Net profit in USD after ALL cost components (gas + LP fees + slippage +
    /// flash-loan fee + failure buffer + capital cost + ops overhead).
    /// Populated by the spine evaluator (`calc_net_profit_and_roi`).
    /// None for opportunities that bypassed the spine path (cold-start, pre-spine rows).
    /// submit_engine Check 7 uses this field; falls back to `expected_profit_usd`
    /// when None (R8 fail-honest — same behaviour as before spine path ran).
    #[serde(default)]
    pub net_expected_profit_usd: Option<f64>,
    pub roi_pct: Option<f64>,
    pub risk_score: Option<f64>,
    pub block_number: Option<u64>,
    /// Diagnostic reason when the opportunity was rejected by a pre-execution
    /// gate (allowlist, strategy, math, risk policy). NULL when the opp passed
    /// all gates OR when a row predates the BUG-2/3 + observability sprint.
    /// Stored as plain text for flexibility — frontend renders verbatim.
    /// See `RejectReason` enum in `prioritization-spine::decision` for the
    /// canonical set of values.
    #[serde(default)]
    pub rejection_reason: Option<String>,
    pub detected_at: DateTime<Utc>,
    pub trace_id: Uuid,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SimulatorKind {
    Anvil,
    Tenderly,
    Hardhat,
    /// In-memory REVM simulator (simulator-v2 crate). Opt-in via SIM_BACKEND=revm.
    Revm,
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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ExecutionStatus {
    Submitted,
    Included,
    Reverted,
    Dropped,
    Replaced,
    NotImplemented,
    /// S5+: paper-mode or pre-submit reject (e.g., value cap exceeded).
    /// No on-chain side effect; `tx_hash` is always null.
    NotSubmitted,
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
    pub execution_id: Option<Uuid>,
    pub tx_hash: Option<String>,
    pub chain_id: u64,

    pub expected_amount_out_wei: Option<String>,
    pub actual_amount_out_wei: Option<String>,
    pub variance_native_units: Option<String>,
    pub variance_pct: Option<f64>,

    pub expected_profit_usd: f64,
    pub actual_profit_usd: f64,
    pub pnl_source: String,

    pub actual_gas_used_wei: Option<String>,
    pub actual_gas_price_wei: Option<String>,
    pub fail_reason: Option<String>,
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
    pub fn new(
        requires: Vec<&'static str>,
        sprint: &'static str,
        detail: impl Into<String>,
    ) -> Self {
        Self {
            error: "not_implemented",
            requires,
            sprint,
            detail: detail.into(),
        }
    }
}
