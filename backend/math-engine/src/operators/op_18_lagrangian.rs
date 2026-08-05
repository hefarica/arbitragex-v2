//! FUSILE: Implementacion propia -- Lagrangiano (L = T - V, accion estacionaria)
//! Categoria: physics
//!
//! Classical analytic mechanics on the return series. Kinetic energy
//! T = 0.5*var(returns) (market velocity / energy), potential energy
//! V = 0.5*(mean_return)^2 (drift potential). Lagrangian:
//!   L = T - V = 0.5*var(returns) - 0.5*mean_return^2
//! L > 0: kinetic-dominated (oscillatory / volatile regime).
//! L < 0: potential-dominated (trending regime).
//!
//! R8 fail-honest: None when fewer than 2 returns are available (need >= 2
//! returns to estimate variance).

use super::{MarketState, OperatorOutput, TopologicalOperator};
use std::collections::HashMap;

#[derive(Default)]
pub struct LagrangianOperator;

impl LagrangianOperator {
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

impl TopologicalOperator for LagrangianOperator {
    fn id(&self) -> u8 {
        18
    }
    fn name(&self) -> &'static str {
        "Lagrangiano"
    }
    fn category(&self) -> &'static str {
        "physics"
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

        let mean_return = returns.iter().sum::<f64>() / returns.len() as f64;
        let variance = returns
            .iter()
            .map(|r| (r - mean_return).powi(2))
            .sum::<f64>()
            / (returns.len() - 1) as f64;
        let kinetic_t = 0.5 * variance;
        let potential_v = 0.5 * mean_return.powi(2);
        let lagrangian = kinetic_t - potential_v;

        if !lagrangian.is_finite() {
            metadata.insert("reason_non_finite".to_string(), 1.0);
            return none_output(self.id(), self.name(), metadata);
        }

        metadata.insert("computed".to_string(), 1.0);
        metadata.insert("kinetic_t".to_string(), kinetic_t);
        metadata.insert("potential_v".to_string(), potential_v);
        metadata.insert("variance".to_string(), variance);
        metadata.insert("mean_return".to_string(), mean_return);
        metadata.insert("lagrangian".to_string(), lagrangian);

        OperatorOutput {
            operator_id: self.id(),
            operator_name: self.name().to_string(),
            scalar_value: Some(lagrangian),
            vector_result: Some(vec![lagrangian, kinetic_t, potential_v]),
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
