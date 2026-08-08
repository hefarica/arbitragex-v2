//! FUSILE: Implementacion propia -- Control Optimo Pontryagin (PMP)
//! Categoria: control
//!
//! Maximum-principle Hamiltonian applied to the return series. Drift
//! mu = mean return, myopic costate lambda = mu, quadratic price-impact cost
//! with unit size = 0.5*var(returns). Extremal Hamiltonian value:
//!   H* = mu*lambda - 0.5*var(returns) = mu^2 - 0.5*sigma^2
//! H* > 0: drift dominates volatility (trending, favourable control regime).
//! H* < 0: volatility dominates (oscillatory / uncertain regime).
//!
//! R8 fail-honest: None when fewer than 2 returns are available (need >= 2
//! returns to estimate variance).

use super::{MarketState, OperatorOutput, TopologicalOperator};
use std::collections::HashMap;

#[derive(Default)]
pub struct PontryaginOperator;

impl PontryaginOperator {
    pub fn new() -> Self {
        Self
    }

    fn price_series(state: &MarketState) -> Vec<f64> {
        if state.price_matrix.is_empty() {
            return Vec::new();
        }
        state
            .price_matrix
            .iter()
            .filter_map(|row| row.first().copied())
            .filter(|p| p.is_finite() && *p > 0.0)
            .collect()
    }

    fn simple_returns(prices: &[f64]) -> Vec<f64> {
        prices
            .windows(2)
            .filter(|w| w[0] > 0.0)
            .map(|w| w[1] / w[0] - 1.0)
            .filter(|r| r.is_finite())
            .collect()
    }
}

impl TopologicalOperator for PontryaginOperator {
    fn id(&self) -> u8 {
        17
    }
    fn name(&self) -> &'static str {
        "Pontryagin"
    }
    fn category(&self) -> &'static str {
        "control"
    }

    fn evaluate(&self, state: &MarketState) -> OperatorOutput {
        let prices = Self::price_series(state);
        let returns = Self::simple_returns(&prices);

        let mut metadata = HashMap::new();
        metadata.insert("computed".to_string(), 0.0);
        metadata.insert("n".to_string(), prices.len() as f64);

        // Need >= 2 returns to estimate variance.
        if returns.len() < 2 {
            metadata.insert("reason_insufficient_data".to_string(), 1.0);
            return none_output(self.id(), self.name(), metadata);
        }

        let mu = returns.iter().sum::<f64>() / returns.len() as f64;
        let impact =
            returns.iter().map(|r| (r - mu).powi(2)).sum::<f64>() / (returns.len() - 1) as f64;
        let lambda = mu; // myopic costate prior = drift
        let h_star = mu * lambda - 0.5 * impact;

        if !h_star.is_finite() {
            metadata.insert("reason_non_finite".to_string(), 1.0);
            return none_output(self.id(), self.name(), metadata);
        }

        metadata.insert("computed".to_string(), 1.0);
        metadata.insert("mu_drift".to_string(), mu);
        metadata.insert("lambda_costate".to_string(), lambda);
        metadata.insert("impact_variance".to_string(), impact);
        metadata.insert("h_star".to_string(), h_star);

        OperatorOutput {
            operator_id: self.id(),
            operator_name: self.name().to_string(),
            scalar_value: Some(h_star),
            vector_result: Some(vec![h_star, mu, impact]),
            matrix_result: None,
            metadata,
        }
    }
}

fn none_output(
    operator_id: u8,
    operator_name: &'static str,
    metadata: HashMap<String, f64>,
) -> OperatorOutput {
    OperatorOutput {
        operator_id,
        operator_name: operator_name.to_string(),
        scalar_value: None,
        vector_result: None,
        matrix_result: None,
        metadata,
    }
}
