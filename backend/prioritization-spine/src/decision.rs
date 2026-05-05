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
    /// always math bugs. Tracked in anti_reincidencia.md Incidente #7.
    AnomalousMath,
    /// BUG-2 fix: a token in the candidate (token_in or token_out) has no
    /// USD price resolvable via `shared_rs::price_oracle::ConfigPriceOracle`
    /// — i.e. it's not the operator's base token, not in the operator's
    /// `token_prices_usd` map, and not in the hardcoded stablecoin trust
    /// list. The spine refuses to invent a price (R8 Fail-Honest); operator
    /// closes the gap by populating `token_prices_usd` for that symbol.
    UnknownTokenPrice,
}

pub trait ExecutionDecisionEngine {
    fn decide(&self, score: f64) -> ExecutionDecision;
}
