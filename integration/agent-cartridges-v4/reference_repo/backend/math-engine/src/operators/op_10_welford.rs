//! FUSILE: Implementacion propia -- Varianza Online (Welford)
//! Formula (Welford online accumulation sobre la serie de retornos):
//!   M_k = M_{k-1} + (x_k − M_{k-1}) / k
//!   S_k = S_{k-1} + (x_k − M_{k-1})(x_k − M_k)
//!   s_N² = S_N / (N−1)        (varianza muestral insesgada)
//!   σ_N  = sqrt(s_N²)         (desviación estándar muestral = volatilidad)
//! con x_k = p_{k+1}/p_k − 1 (retornos simples de la columna 0 de price_matrix).
//! Categoria: numerical
//!
//! R8 fail-honest: N<2 retornos (<3 precios válidos) → scalar_value None
//! (denominador N−1 = 0); nunca un valor fabricado.

use super::{MarketState, OperatorOutput, TopologicalOperator};
use std::collections::HashMap;

#[derive(Default)]
pub struct WelfordOperator;

impl WelfordOperator {
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

impl TopologicalOperator for WelfordOperator {
    fn id(&self) -> u8 {
        10
    }

    fn name(&self) -> &'static str {
        "Welford"
    }

    fn category(&self) -> &'static str {
        "numerical"
    }

    fn evaluate(&self, state: &MarketState) -> OperatorOutput {
        let prices = Self::price_series(state);
        let returns = Self::simple_returns(&prices);
        let n = returns.len();

        // N<2 retornos ⇒ varianza muestral no estimable (denominador N−1 = 0).
        // Equivalentemente <3 precios válidos en la serie.
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

        // Welford online: media M_k y suma de desviaciones al cuadrado S_k (M2).
        let mut mean = 0.0_f64;
        let mut m2 = 0.0_f64;
        for (k, x) in returns.iter().copied().enumerate() {
            let kk = (k + 1) as f64;
            let delta = x - mean;
            mean += delta / kk;
            let delta2 = x - mean;
            m2 += delta * delta2;
        }

        // S_N puede ser marginalmente negativo por flotación → clampear a 0
        // (Welford garantiza S_N ≥ 0 en aritmética exacta).
        let m2 = if m2 < 0.0 { 0.0 } else { m2 };
        let variance = m2 / (n - 1) as f64; // s_N²
        let std_dev = variance.sqrt(); // σ_N

        let mut metadata = HashMap::new();
        metadata.insert("computed".to_string(), 1.0);
        metadata.insert("n".to_string(), n as f64);
        metadata.insert("mean".to_string(), mean);
        metadata.insert("variance".to_string(), variance);
        metadata.insert("std_dev".to_string(), std_dev);
        metadata.insert("m2".to_string(), m2);

        OperatorOutput {
            operator_id: self.id(),
            operator_name: self.name().to_string(),
            scalar_value: Some(std_dev),
            vector_result: Some(vec![mean, variance, n as f64]),
            matrix_result: None,
            metadata,
        }
    }
}
