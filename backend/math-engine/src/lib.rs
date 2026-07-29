//! ArbitrageX Math-Physics Engine
//!
//! Doctrina de Aislamiento Topologico:
//! - 31 operadores matematicos, cada uno en su propio archivo
//! - Acoplamiento exclusivo via trait TopologicalOperator
//! - 264 vectores estrategicos MEV
//! - Matriz de proyeccion 264x31

pub mod control;
pub mod matrix;
pub mod operators;
pub mod strategies;

#[cfg(feature = "api")]
pub mod api;

// Legacy modules preserved for backward compatibility
pub mod amm_math;
pub mod risk_engine;
pub mod roi_engine;
pub mod route_math;
pub mod subgraph_client;

// Re-export core types
pub use control::regime_router::{Regime, RegimeMetrics, RegimeRouter, RegimeThresholds};
pub use operators::{MarketState, OperatorOutput, OperatorRegistry, TopologicalOperator};
pub use strategies::{StrategyOutput, StrategyProfile, StrategyRegistry, TopologicalStrategy};

/// Arbirtage outcome type (legacy, preserved for consumers)
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
