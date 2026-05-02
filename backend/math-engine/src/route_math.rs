//! Matemáticas para enrutamiento (Multi-hop, Bellman-Ford, Cycle Detection)

/// Representa una pierna (salto) dentro de una ruta de arbitraje.
#[derive(Debug, Clone)]
pub struct RouteLeg {
    pub token_in: String,
    pub token_out: String,
    pub rate: f64,          // Ej: 1 Token_In = X Token_Out (antes de fees, o incluyendo fees dependiendo del contexto)
    pub fee_pct: f64,
    pub gas_cost_usd: f64,
}

/// Calcula el producto de tasas a lo largo de una ruta para determinar si existe una oportunidad bruta.
pub fn calc_route_rate_product(legs: &[RouteLeg]) -> f64 {
    let mut product = 1.0;
    for leg in legs {
        product *= leg.rate * (1.0 - leg.fee_pct);
    }
    product
}

/// Convierte una tasa en un "peso" para algoritmos de detección de ciclos de peso negativo (Bellman-Ford).
/// Si queremos maximizar el producto r1 * r2 * r3 > 1, 
/// equivale a minimizar -ln(r1) - ln(r2) - ln(r3) < 0.
pub fn rate_to_bellman_ford_weight(rate: f64, fee_pct: f64) -> f64 {
    if rate <= 0.0 {
        return f64::INFINITY;
    }
    let effective_rate = rate * (1.0 - fee_pct);
    if effective_rate <= 0.0 {
        return f64::INFINITY;
    }
    -(effective_rate.ln())
}

/// Valida de forma rápida si una ruta (ej: A -> B -> C -> A) es rentable en bruto.
/// Es el paso previo antes de hacer la simulación profunda de Liquidez y Slippage.
pub fn is_profitable_cycle(legs: &[RouteLeg], total_risk_buffer_pct: f64) -> bool {
    let product = calc_route_rate_product(legs);
    // Para ser rentable, el producto debe ser > 1.0 + buffers
    product > (1.0 + total_risk_buffer_pct)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_profitable_cycle() {
        let legs = vec![
            RouteLeg { token_in: "USDC".to_string(), token_out: "WETH".to_string(), rate: 1.0/2000.0, fee_pct: 0.003, gas_cost_usd: 2.0 },
            RouteLeg { token_in: "WETH".to_string(), token_out: "WBTC".to_string(), rate: 2000.0/60000.0, fee_pct: 0.003, gas_cost_usd: 2.0 },
            RouteLeg { token_in: "WBTC".to_string(), token_out: "USDC".to_string(), rate: 65000.0, fee_pct: 0.003, gas_cost_usd: 2.0 },
        ];
        
        // (1/2000) * (2000/60000) * 65000 = 65000 / 60000 = 1.0833
        // Fees: 0.997^3 = 0.991
        // Expected product = 1.0833 * 0.991 = 1.0736 (7.36% profit)
        let product = calc_route_rate_product(&legs);
        assert!(product > 1.07);
        
        assert!(is_profitable_cycle(&legs, 0.02)); // is 7% > 2% buffer? Yes
    }
}
