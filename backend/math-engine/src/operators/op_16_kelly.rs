//! FUSILE: Implementacion propia -- Criterio de Kelly
//! Formula: f* = (b·p − q) / b
//!   b = avg_win / avg_loss  (odds reales de la ventana de retornos)
//!   p = fracción de retornos positivos (probabilidad de éxito)
//!   q = 1 − p
//! Categoria: optimization
//!
//! R8 fail-honest: sin datos suficientes (o sin pérdidas para definir b),
//! devuelve scalar_value: None (no computado) — nunca una fracción fabricada.

use super::{MarketState, OperatorOutput, TopologicalOperator};
use std::collections::HashMap;

#[derive(Default)]
pub struct KellyOperator;

impl KellyOperator {
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
            .collect()
    }
}

impl TopologicalOperator for KellyOperator {
    fn id(&self) -> u8 {
        16
    }

    fn name(&self) -> &'static str {
        "Criterio de Kelly"
    }

    fn category(&self) -> &'static str {
        "optimization"
    }

    fn evaluate(&self, state: &MarketState) -> OperatorOutput {
        let prices = Self::price_series(state);
        let returns = Self::simple_returns(&prices);

        // Necesitamos retornos positivos Y negativos para definir p, b.
        let wins: Vec<f64> = returns.iter().copied().filter(|r| *r > 0.0).collect();
        let losses: Vec<f64> = returns.iter().copied().filter(|r| *r < 0.0).collect();

        if returns.len() < 2 || wins.is_empty() || losses.is_empty() {
            return OperatorOutput {
                operator_id: self.id(),
                operator_name: self.name().to_string(),
                scalar_value: None,
                vector_result: None,
                matrix_result: None,
                metadata: {
                    let mut m = HashMap::new();
                    m.insert("computed".to_string(), 0.0);
                    m.insert("reason_insufficient_win_loss".to_string(), 1.0);
                    m
                },
            };
        }

        let p = wins.len() as f64 / returns.len() as f64;
        let q = 1.0 - p;
        let avg_win = wins.iter().sum::<f64>() / wins.len() as f64;
        let avg_loss = losses.iter().map(|r| r.abs()).sum::<f64>() / losses.len() as f64;

        // b = odds reales (ganancia media / pérdida media). Si avg_loss≈0 no
        // hay riesgo medible → no computable (evita división por ~0).
        if avg_loss < 1e-12 {
            return OperatorOutput {
                operator_id: self.id(),
                operator_name: self.name().to_string(),
                scalar_value: None,
                vector_result: None,
                matrix_result: None,
                metadata: {
                    let mut m = HashMap::new();
                    m.insert("computed".to_string(), 0.0);
                    m.insert("reason_zero_avg_loss".to_string(), 1.0);
                    m
                },
            };
        }
        let b = avg_win / avg_loss;

        // f* = (b·p − q) / b, clamp a [0,1]. Valores negativos (edge negativo)
        // se claman a 0 (no apostar). Full Kelly es agresivo; el consumidor
        // decide si aplica fracción (½-Kelly) downstream.
        let kelly = (b * p - q) / b;
        let f_star = kelly.clamp(0.0, 1.0);

        let mut metadata = HashMap::new();
        metadata.insert("computed".to_string(), 1.0);
        metadata.insert("f_star".to_string(), f_star);
        metadata.insert("kelly_raw".to_string(), kelly);
        metadata.insert("p".to_string(), p);
        metadata.insert("q".to_string(), q);
        metadata.insert("b_odds".to_string(), b);
        metadata.insert("avg_win".to_string(), avg_win);
        metadata.insert("avg_loss".to_string(), avg_loss);

        OperatorOutput {
            operator_id: self.id(),
            operator_name: self.name().to_string(),
            scalar_value: Some(f_star),
            vector_result: Some(vec![f_star, p, b]),
            matrix_result: None,
            metadata,
        }
    }
}
