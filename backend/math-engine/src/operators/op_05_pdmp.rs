//! FUSILE: Implementacion propia -- Proceso Markoviano de Salto-Difusion (Merton)
//! Modelo (sobre los retornos simples r_t):
//!   dX = μ dt + σ dW + dJ,   J_t = Σ_{k=1}^{N_t} Y_k,   N_t ~ Poisson(λ_J t)
//! Estimador threshold-MLE (separación bulk/salto):
//!   μ = mean(r),  σ² = var(r),  umbral μ ± 2σ
//!   J = {t : |r_t - μ| > 2σ},   λ_J = |J|/N   (intensidad de salto por muestra)
//! scalar_value = λ_J (intensidad de salto estimada). |J|=0 ⇒ Some(0.0) legítimo
//!                (régimen de pura difusión, R8: computado y exactamente cero).
//! Categoria: stochastic
//!
//! R8 fail-honest: None si N<5 (muestras insuficientes), σ²≈0 (retornos degenerados,
//!                 el umbral 2σ colapsa y difusión/salto no son separables), o
//!                 parámetros no finitos.

use super::{MarketState, OperatorOutput, TopologicalOperator};
use std::collections::HashMap;

#[derive(Default)]
pub struct PDMPOperator;

impl PDMPOperator {
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

impl TopologicalOperator for PDMPOperator {
    fn id(&self) -> u8 {
        5
    }

    fn name(&self) -> &'static str {
        "Proceso Markoviano de Salto-Difusion"
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

        // N<5 ⇒ muestras insuficientes para estimar la difusión y separar saltos.
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
        // Varianza muestral (denominador n-1, consistente con op_08 Kalman).
        let var = returns
            .iter()
            .map(|r| (r - mean).powi(2))
            .sum::<f64>()
            / (n - 1) as f64;
        let sigma = var.max(0.0).sqrt();

        // σ²≈0 ⇒ retornos degenerados/constantes: el umbral 2σ colapsa y la
        // descomposición difusión/salto no es identificable.
        if !mean.is_finite() || !sigma.is_finite() || var < 1e-18 {
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

        // Umbral μ ± 2σ: |r_t - μ| > 2σ marca una realización de salto
        // (la contribución impulsiva que genera la kurtosis excesiva).
        let thresh = 2.0 * sigma;
        let jump_count = returns.iter().filter(|r| (*r - mean).abs() > thresh).count() as f64;
        let lambda_j = jump_count / n as f64; // intensidad de salto por muestra

        metadata.insert("computed".to_string(), 1.0);
        metadata.insert("lambda_j".to_string(), lambda_j);
        metadata.insert("mu".to_string(), mean);
        metadata.insert("sigma".to_string(), sigma);
        metadata.insert("variance".to_string(), var);
        metadata.insert("jump_count".to_string(), jump_count);
        metadata.insert("bulk_count".to_string(), n as f64 - jump_count);

        OperatorOutput {
            operator_id: self.id(),
            operator_name: self.name().to_string(),
            scalar_value: Some(lambda_j),
            vector_result: Some(vec![lambda_j, sigma, mean]),
            matrix_result: None,
            metadata,
        }
    }
}
