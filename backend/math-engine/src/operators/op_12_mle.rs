//! FUSILE: Implementacion propia -- Maxima Verosimilitud (MLE)
//! Estima los parámetros (μ, σ) de una Normal sobre los retornos logarítmicos:
//!   μ̂ = (1/n)Σ r_i      (media MLE)
//!   σ̂² = (1/n)Σ (r_i − μ̂)²  (varianza MLE, sesgada)
//! Categoria: inference
//!
//! R8 fail-honest: sin datos suficientes, devuelve scalar_value: None.

use super::{MarketState, OperatorOutput, TopologicalOperator};
use std::collections::HashMap;

#[derive(Default)]
pub struct MLEOperator;

impl MLEOperator {
    pub fn new() -> Self {
        Self
    }
}

impl TopologicalOperator for MLEOperator {
    fn id(&self) -> u8 {
        12
    }

    fn name(&self) -> &'static str {
        "Maxima Verosimilitud"
    }

    fn category(&self) -> &'static str {
        "inference"
    }

    fn evaluate(&self, state: &MarketState) -> OperatorOutput {
        let prices: Vec<f64> = state
            .price_matrix
            .iter()
            .filter_map(|row| row.first().copied())
            .filter(|p| p.is_finite() && *p > 0.0)
            .collect();

        let rets: Vec<f64> = prices
            .windows(2)
            .filter(|w| w[0] > 0.0)
            .map(|w| (w[1] / w[0]).ln())
            .collect();

        if rets.len() < 2 {
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

        let n = rets.len() as f64;
        let mu = rets.iter().sum::<f64>() / n;
        let var = rets.iter().map(|r| (r - mu).powi(2)).sum::<f64>() / n;
        let sigma = var.sqrt();

        // Log-verosimilitud de la Normal evaluada en (μ̂, σ̂) — medida de ajuste.
        let ll: f64 = if sigma > 1e-12 {
            rets.iter()
                .map(|r| {
                    -0.5 * ((r - mu) / sigma).powi(2)
                        - sigma.ln()
                        - 0.5 * (2.0 * std::f64::consts::PI).ln()
                })
                .sum()
        } else {
            f64::NEG_INFINITY
        };

        let mut metadata = HashMap::new();
        metadata.insert("computed".to_string(), 1.0);
        metadata.insert("mu_hat".to_string(), mu);
        metadata.insert("sigma_hat".to_string(), sigma);
        metadata.insert("log_likelihood".to_string(), ll);
        metadata.insert("n".to_string(), n);

        OperatorOutput {
            operator_id: self.id(),
            operator_name: self.name().to_string(),
            // Magnitud principal: drift estimado (μ̂) — la tendencia MLE.
            scalar_value: Some(mu),
            vector_result: Some(vec![mu, sigma]),
            matrix_result: None,
            metadata,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn st(prices: &[f64]) -> MarketState {
        MarketState {
            price_matrix: prices.iter().map(|p| vec![*p]).collect(),
            liquidity_reserves: Vec::new(),
            gas_price_gwei: 20.0,
            block_timestamp: 0,
            block_number: 0,
            features: HashMap::new(),
        }
    }

    #[test]
    fn mle_estimates_positive_drift_on_uptrend() {
        let op = MLEOperator::new();
        let out = op.evaluate(&st(&[100.0, 101.0, 102.0, 103.0, 104.0, 105.0]));
        let mu = out.scalar_value.unwrap();
        assert!(mu > 0.0, "uptrend must yield positive MLE drift (got {mu})");
        assert!(out.metadata.get("sigma_hat").copied().unwrap() >= 0.0);
    }

    #[test]
    fn none_on_insufficient() {
        let op = MLEOperator::new();
        let out = op.evaluate(&st(&[100.0]));
        assert!(out.scalar_value.is_none());
    }
}
