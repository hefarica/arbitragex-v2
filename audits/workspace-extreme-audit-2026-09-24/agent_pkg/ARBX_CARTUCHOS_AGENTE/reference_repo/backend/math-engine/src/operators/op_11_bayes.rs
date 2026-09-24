//! FUSILE: Implementacion propia -- Inferencia Bayesiana (Beta-Binomial)
//! Formula: posterior P(θ|D) = Beta(α + wins, β + losses) sobre la tasa de éxito θ.
//!   media posterior = (α + wins) / (α + β + wins + losses)
//! Categoria: inference
//!
//! R8 fail-honest: sin historial de éxitos/derrotas en `features`, devuelve
//! scalar_value: None (no computado) — nunca un posterior fabricado.

use super::{MarketState, OperatorOutput, TopologicalOperator};
use std::collections::HashMap;

#[derive(Default)]
pub struct BayesOperator;

impl BayesOperator {
    pub fn new() -> Self {
        Self
    }
}

impl TopologicalOperator for BayesOperator {
    fn id(&self) -> u8 {
        11
    }

    fn name(&self) -> &'static str {
        "Inferencia Bayesiana"
    }

    fn category(&self) -> &'static str {
        "inference"
    }

    fn evaluate(&self, state: &MarketState) -> OperatorOutput {
        let wins = state.features.get("bayes_wins").copied().unwrap_or(0.0);
        let losses = state.features.get("bayes_losses").copied().unwrap_or(0.0);
        let alpha0 = state
            .features
            .get("bayes_prior_alpha")
            .copied()
            .unwrap_or(1.0);
        let beta0 = state
            .features
            .get("bayes_prior_beta")
            .copied()
            .unwrap_or(1.0);

        if wins + losses < 1.0 {
            return OperatorOutput {
                operator_id: self.id(),
                operator_name: self.name().to_string(),
                scalar_value: None,
                vector_result: None,
                matrix_result: None,
                metadata: {
                    let mut m = HashMap::new();
                    m.insert("computed".to_string(), 0.0);
                    m.insert("reason_no_history".to_string(), 1.0);
                    m
                },
            };
        }

        let alpha = alpha0 + wins;
        let beta = beta0 + losses;
        let mean = alpha / (alpha + beta);
        let var = (alpha * beta) / ((alpha + beta).powi(2) * (alpha + beta + 1.0));
        let std = var.sqrt();

        let mut metadata = HashMap::new();
        metadata.insert("computed".to_string(), 1.0);
        metadata.insert("posterior_mean".to_string(), mean);
        metadata.insert("posterior_alpha".to_string(), alpha);
        metadata.insert("posterior_beta".to_string(), beta);
        metadata.insert("posterior_std".to_string(), std);
        metadata.insert("wins".to_string(), wins);
        metadata.insert("losses".to_string(), losses);

        OperatorOutput {
            operator_id: self.id(),
            operator_name: self.name().to_string(),
            scalar_value: Some(mean),
            vector_result: Some(vec![mean, std]),
            matrix_result: None,
            metadata,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn st(feats: &[(&str, f64)]) -> MarketState {
        MarketState {
            price_matrix: Vec::new(),
            liquidity_reserves: Vec::new(),
            gas_price_gwei: 20.0,
            block_timestamp: 0,
            block_number: 0,
            features: feats.iter().map(|(k, v)| (k.to_string(), *v)).collect(),
        }
    }

    #[test]
    fn posterior_shifts_with_evidence() {
        let op = BayesOperator::new();
        let out = op.evaluate(&st(&[("bayes_wins", 8.0), ("bayes_losses", 2.0)]));
        let mean = out.scalar_value.unwrap();
        assert!(
            (mean - 0.75).abs() < 1e-9,
            "posterior mean should be 0.75 (got {mean})"
        );
        assert_eq!(out.metadata.get("computed"), Some(&1.0));
    }

    #[test]
    fn none_without_history() {
        let op = BayesOperator::new();
        let out = op.evaluate(&st(&[]));
        assert!(out.scalar_value.is_none());
        assert_eq!(out.metadata.get("computed"), Some(&0.0));
    }
}
