//! FUSILE: Implementacion propia -- Procesos de Levy (índice de estabilidad α, MoM)
//! Estimador method-of-moments sobre la kurtosis cruda κ de los retornos simples:
//!   μ = mean(r),   σ² = (1/N)Σ(r-μ)²,   μ₄ = (1/N)Σ(r-μ)⁴,   κ = μ₄/σ⁴
//! Para un proceso α-estable la kurtosis vincula α con el peso de las colas:
//!   κ = 3 (límite Gaussiano) ⇒ α = 2;   κ > 3 (colas pesadas) ⇒ α < 2.
//!   α = clamp( 2 - (κ-3)/3 , 1.0, 2.0 )
//! scalar_value = α   (índice de estabilidad / actividad de salto de
//!                Blumenthal-Getoor). α→2: Variedad casi Gaussiana (varianza finita);
//!                α<2: colas pesadas, riesgo de Topological Yield dominado por saltos.
//! Categoria: stochastic
//!
//! R8 fail-honest: None si N<5, σ²≈0 (kurtosis κ = μ₄/σ⁴ indefinida) o κ no finita.

use super::{MarketState, OperatorOutput, TopologicalOperator};
use std::collections::HashMap;

#[derive(Default)]
pub struct LevyOperator;

impl LevyOperator {
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

impl TopologicalOperator for LevyOperator {
    fn id(&self) -> u8 {
        9
    }

    fn name(&self) -> &'static str {
        "Procesos de Levy"
    }

    fn category(&self) -> &'static str {
        "stochastic"
    }

    fn evaluate(&self, state: &MarketState) -> OperatorOutput {
        let prices = Self::price_series(state);
        let returns = Self::simple_returns(&prices);
        let n = returns.len();

        let mut metadata = HashMap::new();
        metadata.insert("n".to_string(), n as f64);

        if n < 5 {
            metadata.insert("computed".to_string(), 0.0);
            metadata.insert("reason_insufficient_samples".to_string(), 1.0);
            return OperatorOutput {
                operator_id: self.id(),
                operator_name: self.name().to_string(),
                scalar_value: None,
                vector_result: None,
                matrix_result: None,
                metadata,
            };
        }

        let mean = returns.iter().sum::<f64>() / n as f64;
        let m2 = returns
            .iter()
            .map(|r| (r - mean).powi(2))
            .sum::<f64>()
            / n as f64;
        let m4 = returns
            .iter()
            .map(|r| (r - mean).powi(4))
            .sum::<f64>()
            / n as f64;

        // σ²≈0 ⇒ kurtosis κ = μ₄/σ⁴ indefinida (retornos degenerados/constantes).
        if !mean.is_finite() || !m2.is_finite() || !m4.is_finite() || m2 < 1e-18 {
            metadata.insert("computed".to_string(), 0.0);
            metadata.insert("reason_degenerate_returns".to_string(), 1.0);
            return OperatorOutput {
                operator_id: self.id(),
                operator_name: self.name().to_string(),
                scalar_value: None,
                vector_result: None,
                matrix_result: None,
                metadata,
            };
        }

        let kurtosis = m4 / (m2 * m2); // kurtosis cruda (Gaussiana ⇒ 3)
        if !kurtosis.is_finite() {
            metadata.insert("computed".to_string(), 0.0);
            metadata.insert("reason_non_finite_kurtosis".to_string(), 1.0);
            return OperatorOutput {
                operator_id: self.id(),
                operator_name: self.name().to_string(),
                scalar_value: None,
                vector_result: None,
                matrix_result: None,
                metadata,
            };
        }

        // α-stable MoM: κ=3⇒α=2 (Gaussiana), κ>3⇒α<2 (colas pesadas). Clamp [1,2].
        let alpha = (2.0 - (kurtosis - 3.0) / 3.0).clamp(1.0, 2.0);

        metadata.insert("computed".to_string(), 1.0);
        metadata.insert("alpha".to_string(), alpha);
        metadata.insert("kurtosis".to_string(), kurtosis);
        metadata.insert("m2".to_string(), m2);
        metadata.insert("m4".to_string(), m4);
        metadata.insert("mean".to_string(), mean);

        OperatorOutput {
            operator_id: self.id(),
            operator_name: self.name().to_string(),
            scalar_value: Some(alpha),
            vector_result: Some(vec![alpha, kurtosis, m2.sqrt()]),
            matrix_result: None,
            metadata,
        }
    }
}
