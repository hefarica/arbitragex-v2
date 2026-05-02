//! Motor de ROI (Return on Investment) para Arbitraje DeFi
//!
//! Todas las funciones calculan rentabilidad neta tras deducir:
//! - Gas estimado (en USD).
//! - Comisiones de Flashloan.
//! - Protocol fees (DEX fees, ya descontados usualmente en amount_out).
//! - Slippage/Failure buffers.

use crate::DefiArbitrageOutcome;

/// Parámetros necesarios para calcular la rentabilidad neta final.
#[derive(Debug, Clone)]
pub struct RoiCalculationParams {
    pub amount_in_usd: f64,
    pub expected_amount_out_usd: f64,
    pub expected_gas_cost_usd: f64,
    pub flashloan_fee_pct: f64,
    pub max_slippage_pct: f64,
    pub failure_risk_buffer_usd: f64,
}

/// Calcula el ROI bruto y neto de una oportunidad DeFi (Estrategia General).
pub fn calc_net_profit_and_roi(params: &RoiCalculationParams) -> DefiArbitrageOutcome {
    let gross_profit_usd = params.expected_amount_out_usd - params.amount_in_usd;
    
    // Asumimos que si usamos flashloan, pagamos un % del capital prestado (amount_in_usd)
    let flashloan_fee_usd = params.amount_in_usd * params.flashloan_fee_pct;
    
    // Consideramos el slippage máximo para calcular el peor caso aceptable.
    let slippage_cost_usd = params.expected_amount_out_usd * params.max_slippage_pct;
    
    // Net Profit puro = Lo que entra - Lo que salió - Gas - Costos de préstamo - Buffer de slippage - Buffer de riesgo
    let net_profit_usd = params.expected_amount_out_usd 
        - params.amount_in_usd 
        - params.expected_gas_cost_usd 
        - flashloan_fee_usd 
        - slippage_cost_usd 
        - params.failure_risk_buffer_usd;

    // Si usamos flashloan, nuestro capital requerido propio es virtualmente 0, 
    // pero para ROI consideramos el capital prestado.
    let capital_required = params.amount_in_usd;
    let net_roi_pct = if capital_required > 0.0 {
        (net_profit_usd / capital_required) * 100.0
    } else {
        0.0
    };

    let is_viable = net_profit_usd > 0.0;
    
    // Puntuación básica (será refinada por risk_engine o un scoring_engine)
    let opportunity_score = if is_viable {
        // Ejemplo simple: 50 base + 5 puntos por cada 0.1% de ROI neto, max 100
        let base = 50.0;
        let bonus = (net_roi_pct / 0.1) * 5.0;
        (base + bonus).min(100.0)
    } else {
        0.0
    };

    DefiArbitrageOutcome {
        is_viable,
        gross_profit_usd,
        net_profit_usd,
        expected_amount_out: params.expected_amount_out_usd, // Normalizado a USD para la struct común
        gas_cost_usd: params.expected_gas_cost_usd,
        flashloan_fee_usd,
        slippage_expected_pct: params.max_slippage_pct,
        total_capital_required_usd: params.amount_in_usd,
        net_roi_pct,
        opportunity_score,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_roi_calculation_profitable() {
        let params = RoiCalculationParams {
            amount_in_usd: 10_000.0,
            expected_amount_out_usd: 10_050.0, // $50 gross profit (0.5%)
            expected_gas_cost_usd: 5.0,
            flashloan_fee_pct: 0.0009, // 0.09% Aave v3 fee
            max_slippage_pct: 0.001, // 0.1%
            failure_risk_buffer_usd: 1.0,
        };

        let result = calc_net_profit_and_roi(&params);
        
        // Flashloan fee = 10_000 * 0.0009 = $9.0
        // Slippage buffer = 10_050 * 0.001 = $10.05
        // Net profit = 10_050 - 10_000 - 5.0 - 9.0 - 10.05 - 1.0 = $24.95
        
        assert!(result.is_viable);
        assert!((result.net_profit_usd - 24.95).abs() < 0.001);
        assert!((result.net_roi_pct - 0.2495).abs() < 0.001);
    }
    
    #[test]
    fn test_roi_calculation_not_profitable() {
        let params = RoiCalculationParams {
            amount_in_usd: 10_000.0,
            expected_amount_out_usd: 10_010.0, // $10 gross profit
            expected_gas_cost_usd: 15.0, // El gas se come la ganancia
            flashloan_fee_pct: 0.0009,
            max_slippage_pct: 0.0,
            failure_risk_buffer_usd: 0.0,
        };

        let result = calc_net_profit_and_roi(&params);
        assert!(!result.is_viable);
        assert!(result.net_profit_usd < 0.0);
    }
}
