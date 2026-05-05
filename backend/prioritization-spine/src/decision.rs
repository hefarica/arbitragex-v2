use serde::{Deserialize, Serialize};

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
    /// Defense-in-depth: math layer produced ROI > 999% or |gross_profit| > $1M.
    /// Real HFT MEV ROI distribution is 0.05% – 2%; outliers above 999% are
    /// always math bugs (currently BUG-2: token-blind USD valuation in
    /// `profit_token_to_usd`). Tracked in anti_reincidencia.md Incidente #7.
    AnomalousMath,
}

pub trait ExecutionDecisionEngine {
    fn decide(&self, score: f64) -> ExecutionDecision;
}
