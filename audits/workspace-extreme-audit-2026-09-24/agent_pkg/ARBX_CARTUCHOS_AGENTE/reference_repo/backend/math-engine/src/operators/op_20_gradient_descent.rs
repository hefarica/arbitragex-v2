//! FUSILE: Implementacion propia -- Descenso de Gradiente
//! Minimiza J(θ) = Σ_i (returns_i − θ)² sobre el escalar θ (estadística simple:
//! el minimizador es θ* = mean(returns)). Ley de descenso (gradiente promediado,
//! forma canónica equivalente a θ_{t+1} = θ_t + η·(mean − θ_t)):
//!   ∇J·(1/n) = (1/n) Σ_i (returns_i − θ) = mean − θ
//!   θ_{t+1} = θ_t − η·g_t,   g_t = θ_t − mean   ⇒   θ_{t+1} = θ_t + η·(mean − θ_t)
//! Con η = 0.1 y 100 iteraciones θ_t converge geométricamente a mean(returns):
//!   θ_t = mean·(1 − (1−η)^t)
//! scalar_value = θ* (≈ media de los retornos). Categoria: optimization
//!
//! R8 fail-honest: N<2 retornos, retornos no finitos, o divergencia ⇒ None.

use super::{MarketState, OperatorOutput, TopologicalOperator};
use std::collections::HashMap;

#[derive(Default)]
pub struct GradientDescentOperator;

impl GradientDescentOperator {
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

impl TopologicalOperator for GradientDescentOperator {
    fn id(&self) -> u8 {
        20
    }

    fn name(&self) -> &'static str {
        "Gradient Descent"
    }

    fn category(&self) -> &'static str {
        "optimization"
    }

    fn evaluate(&self, state: &MarketState) -> OperatorOutput {
        let prices = Self::price_series(state);
        let returns = Self::simple_returns(&prices);
        let n = returns.len();

        // N<2 ⇒ gradiente de J no estimable.
        if n < 2 {
            return OperatorOutput {
                operator_id: self.id(),
                operator_name: self.name().to_string(),
                scalar_value: None,
                vector_result: None,
                matrix_result: None,
                metadata: {
                    let mut m = HashMap::new();
                    m.insert("computed".to_string(), 0.0);
                    m.insert("reason_insufficient_data".to_string(), 1.0);
                    m
                },
            };
        }

        // Retornos no finitos ⇒ J indefinida.
        if returns.iter().any(|r| !r.is_finite()) {
            return OperatorOutput {
                operator_id: self.id(),
                operator_name: self.name().to_string(),
                scalar_value: None,
                vector_result: None,
                matrix_result: None,
                metadata: {
                    let mut m = HashMap::new();
                    m.insert("computed".to_string(), 0.0);
                    m.insert("reason_non_finite_returns".to_string(), 1.0);
                    m
                },
            };
        }

        let nf = n as f64;
        let sum: f64 = returns.iter().copied().sum();
        let mean = sum / nf;

        // Descenso de gradiente: θ_{t+1} = θ_t + η·(mean − θ_t), θ_0 = 0.
        let eta = 0.1_f64;
        let iters = 100_usize;
        let mut theta = 0.0_f64;
        let mut grad_norm = (mean - theta).abs();
        for _ in 0..iters {
            // g_t = θ_t − mean (gradiente normalizado); θ ← θ − η·g_t.
            let grad = theta - mean;
            theta -= eta * grad; // = theta + eta·(mean − theta)
            if !theta.is_finite() {
                return OperatorOutput {
                    operator_id: self.id(),
                    operator_name: self.name().to_string(),
                    scalar_value: None,
                    vector_result: None,
                    matrix_result: None,
                    metadata: {
                        let mut m = HashMap::new();
                        m.insert("computed".to_string(), 0.0);
                        m.insert("reason_divergence".to_string(), 1.0);
                        m
                    },
                };
            }
            grad_norm = (mean - theta).abs();
        }

        // ‖∇J‖ literal = |dJ/dθ| = 2n·|mean − θ|; grad_norm (normalizado) = |mean − θ|.
        let objective_grad_norm = 2.0 * nf * grad_norm;

        let mut metadata = HashMap::new();
        metadata.insert("computed".to_string(), 1.0);
        metadata.insert("n".to_string(), nf);
        metadata.insert("mean".to_string(), mean);
        metadata.insert("grad_norm".to_string(), grad_norm);
        metadata.insert("objective_grad_norm".to_string(), objective_grad_norm);
        metadata.insert("iters".to_string(), iters as f64);
        metadata.insert("eta".to_string(), eta);

        OperatorOutput {
            operator_id: self.id(),
            operator_name: self.name().to_string(),
            scalar_value: Some(theta),
            vector_result: Some(vec![theta, mean, grad_norm]),
            matrix_result: None,
            metadata,
        }
    }
}
