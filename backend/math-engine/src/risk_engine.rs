//! Motor de Riesgo y Scoring para DeFi


/// Parámetros de la política de riesgo en vivo
#[derive(Debug, Clone)]
pub struct RiskPolicy {
    pub min_net_profit_usd: f64,
    pub min_net_roi_pct: f64,
    pub max_gas_cost_ratio: f64,    // gas_cost / gross_profit
    pub max_slippage_pct: f64,
    pub max_price_impact_pct: f64,
    pub min_liquidity_usd: f64,
    pub max_trade_size_usd: f64,
}

/// Detalles de la Oportunidad para evaluación de riesgo
#[derive(Debug, Clone)]
pub struct OpportunityRiskProfile {
    pub gross_profit_usd: f64,
    pub net_profit_usd: f64,
    pub net_roi_pct: f64,
    pub gas_cost_usd: f64,
    pub slippage_expected_pct: f64,
    pub price_impact_pct: f64,
    pub liquidity_available_usd: f64,
    pub trade_size_usd: f64,
    pub simulation_passed: bool,
    pub contracts_verified: bool,
}

pub enum RiskRejectionReason {
    NegativeNetProfit,
    NetRoiTooLow,
    GasCostTooHigh,
    SlippageTooHigh,
    PriceImpactTooHigh,
    InsufficientLiquidity,
    TradeSizeExceeded,
    SimulationFailed,
    UnverifiedContracts,
}

/// Valida una oportunidad contra la política de riesgo estricta.
pub fn validate_opportunity_risk(
    profile: &OpportunityRiskProfile,
    policy: &RiskPolicy,
) -> Result<(), RiskRejectionReason> {
    if profile.net_profit_usd <= policy.min_net_profit_usd {
        return Err(RiskRejectionReason::NegativeNetProfit);
    }
    if profile.net_roi_pct < policy.min_net_roi_pct {
        return Err(RiskRejectionReason::NetRoiTooLow);
    }
    
    let gas_ratio = if profile.gross_profit_usd > 0.0 {
        profile.gas_cost_usd / profile.gross_profit_usd
    } else {
        1.0
    };
    if gas_ratio > policy.max_gas_cost_ratio {
        return Err(RiskRejectionReason::GasCostTooHigh);
    }

    if profile.slippage_expected_pct > policy.max_slippage_pct {
        return Err(RiskRejectionReason::SlippageTooHigh);
    }
    if profile.price_impact_pct > policy.max_price_impact_pct {
        return Err(RiskRejectionReason::PriceImpactTooHigh);
    }
    if profile.liquidity_available_usd < policy.min_liquidity_usd {
        return Err(RiskRejectionReason::InsufficientLiquidity);
    }
    if profile.trade_size_usd > policy.max_trade_size_usd {
        return Err(RiskRejectionReason::TradeSizeExceeded);
    }
    if !profile.simulation_passed {
        return Err(RiskRejectionReason::SimulationFailed);
    }
    if !profile.contracts_verified {
        return Err(RiskRejectionReason::UnverifiedContracts);
    }

    Ok(())
}

/// Calcula el Valor Esperado (Expected Value - EV) de la operación
pub fn calc_expected_value(
    net_profit_usd: f64,
    failure_cost_usd: f64, // Ej: gas perdido si revierte
    probability_success: f64,
) -> f64 {
    let probability_failure = 1.0 - probability_success;
    (probability_success * net_profit_usd) - (probability_failure * failure_cost_usd)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_risk_validation_success() {
        let policy = RiskPolicy {
            min_net_profit_usd: 1.0,
            min_net_roi_pct: 0.1,
            max_gas_cost_ratio: 0.5,
            max_slippage_pct: 0.01,
            max_price_impact_pct: 0.05,
            min_liquidity_usd: 10_000.0,
            max_trade_size_usd: 5_000.0,
        };

        let profile = OpportunityRiskProfile {
            gross_profit_usd: 50.0,
            net_profit_usd: 20.0,
            net_roi_pct: 0.5,
            gas_cost_usd: 10.0, // Ratio: 10/50 = 0.2 < 0.5
            slippage_expected_pct: 0.005,
            price_impact_pct: 0.01,
            liquidity_available_usd: 50_000.0,
            trade_size_usd: 4_000.0,
            simulation_passed: true,
            contracts_verified: true,
        };

        assert!(validate_opportunity_risk(&profile, &policy).is_ok());
    }
    
    #[test]
    fn test_risk_validation_gas_too_high() {
        let policy = RiskPolicy {
            min_net_profit_usd: 1.0,
            min_net_roi_pct: 0.1,
            max_gas_cost_ratio: 0.5,
            max_slippage_pct: 0.01,
            max_price_impact_pct: 0.05,
            min_liquidity_usd: 10_000.0,
            max_trade_size_usd: 5_000.0,
        };

        let profile = OpportunityRiskProfile {
            gross_profit_usd: 50.0,
            net_profit_usd: 2.0,
            net_roi_pct: 0.5,
            gas_cost_usd: 40.0, // Ratio: 40/50 = 0.8 > 0.5 -> FAIL
            slippage_expected_pct: 0.005,
            price_impact_pct: 0.01,
            liquidity_available_usd: 50_000.0,
            trade_size_usd: 4_000.0,
            simulation_passed: true,
            contracts_verified: true,
        };

        assert!(matches!(
            validate_opportunity_risk(&profile, &policy),
            Err(RiskRejectionReason::GasCostTooHigh)
        ));
    }
}
