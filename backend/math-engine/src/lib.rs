//! ArbitrageX DeFi Math Engine
//! 
//! Pure, deterministic mathematical functions for DeFi arbitrage.
//! Zero side-effects, zero hardcoded values. All inputs are provided by the caller.

pub mod amm_math;
pub mod roi_engine;
pub mod route_math;
pub mod risk_engine;

/// Represents a DeFi arbitrage financial outcome.
#[derive(Debug, Clone, PartialEq)]
pub struct DefiArbitrageOutcome {
    pub is_viable: bool,
    pub gross_profit_usd: f64,
    pub net_profit_usd: f64,
    pub expected_amount_out: f64,
    pub gas_cost_usd: f64,
    pub flashloan_fee_usd: f64,
    pub slippage_expected_pct: f64,
    pub total_capital_required_usd: f64,
    pub net_roi_pct: f64,
    pub opportunity_score: f64,
}
