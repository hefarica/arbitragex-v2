//! FUSILE: Implementacion propia -- Divergencia de Kullback-Leibler
//! Formula: D_KL(P‖Q) = Σ P(x)·ln(P(x)/Q(x)) entre la distribución observada P
//! y una distribución de referencia Q (uniforme si no se provee).
//! Categoria: inference
//!
//! Mide cuánto diverge la distribución de precios entre venues (o vs uniforme).
//! R8 fail-honest: sin datos suficientes, devuelve scalar_value: None.

use super::{MarketState, OperatorOutput, TopologicalOperator};
use std::collections::HashMap;

#[derive(Default)]
pub struct KlDivergenceOperator;

impl KlDivergenceOperator {
    pub fn new() -> Self {
        Self
    }

    /// Histograma normalizado de la serie (P).
    fn distribution(values: &[f64], bins: usize) -> Option<Vec<f64>> {
        if values.len() < 2 || bins == 0 {
            return None;
        }
        let min = values.iter().cloned().fold(f64::INFINITY, f64::min);
        let max = values.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
        if (max - min).abs() < 1e-12 {
            return None; // distribución degenerada (un solo valor)
        }
        let width = (max - min) / bins as f64;
        let mut hist = vec![0.0f64; bins];
        for &v in values {
            let idx = (((v - min) / width).floor() as usize).min(bins - 1);
            hist[idx] += 1.0;
        }
        let total = values.len() as f64;
        Some(hist.iter().map(|c| c / total).collect())
    }

    /// D_KL(P‖Q) con suavizado de Laplace para evitar ln(0).
    fn kl(p: &[f64], q: &[f64]) -> f64 {
        const EPS: f64 = 1e-10;
        p.iter()
            .zip(q.iter())
            .map(|(&pi, &qi)| {
                let pi = pi + EPS;
                let qi = qi + EPS;
                pi * (pi / qi).ln()
            })
            .sum()
    }
}

impl TopologicalOperator for KlDivergenceOperator {
    fn id(&self) -> u8 {
        14
    }

    fn name(&self) -> &'static str {
        "Divergencia KL"
    }

    fn category(&self) -> &'static str {
        "inference"
    }

    fn evaluate(&self, state: &MarketState) -> OperatorOutput {
        // Distribución P: precios del activo 0 a través de las filas.
        let prices: Vec<f64> = state
            .price_matrix
            .iter()
            .filter_map(|row| row.first().copied())
            .filter(|p| p.is_finite() && *p > 0.0)
            .collect();

        const BINS: usize = 8;
        let p = match Self::distribution(&prices, BINS) {
            Some(d) => d,
            None => {
                return OperatorOutput {
                    operator_id: self.id(),
                    operator_name: self.name().to_string(),
                    scalar_value: None,
                    vector_result: None,
                    matrix_result: None,
                    metadata: {
                        let mut m = HashMap::new();
                        m.insert("computed".to_string(), 0.0);
                        m.insert("reason_degenerate_distribution".to_string(), 1.0);
                        m
                    },
                }
            }
        };

        // Q: uniforme de referencia (no-information baseline).
        let q = vec![1.0 / BINS as f64; BINS];
        let d_kl = Self::kl(&p, &q);

        let mut metadata = HashMap::new();
        metadata.insert("computed".to_string(), 1.0);
        metadata.insert("d_kl".to_string(), d_kl);
        metadata.insert("bins".to_string(), BINS as f64);
        metadata.insert("n_observations".to_string(), prices.len() as f64);

        OperatorOutput {
            operator_id: self.id(),
            operator_name: self.name().to_string(),
            scalar_value: Some(d_kl),
            vector_result: Some(p),
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
    fn uniform_distribution_low_divergence() {
        let op = KlDivergenceOperator::new();
        // Spread uniforme → D_KL baja vs uniforme.
        let out = op.evaluate(&st(&[100.0, 101.0, 102.0, 103.0, 104.0, 105.0, 106.0, 107.0]));
        let d = out.scalar_value.unwrap();
        assert!(d >= 0.0, "KL divergence must be non-negative");
        assert_eq!(out.metadata.get("computed"), Some(&1.0));
    }

    #[test]
    fn concentrated_distribution_high_divergence() {
        let op = KlDivergenceOperator::new();
        // Casi todos iguales + un outlier → alta divergencia vs uniforme.
        let uniform = op.evaluate(&st(&[100.0, 101.0, 102.0, 103.0, 104.0, 105.0, 106.0, 107.0]));
        let skewed = op.evaluate(&st(&[100.0, 100.0, 100.0, 100.0, 100.0, 100.0, 100.0, 200.0]));
        assert!(
            skewed.scalar_value.unwrap() > uniform.scalar_value.unwrap(),
            "concentrated dist should diverge more than uniform"
        );
    }

    #[test]
    fn none_on_degenerate() {
        let op = KlDivergenceOperator::new();
        let out = op.evaluate(&st(&[100.0, 100.0])); // single value
        assert!(out.scalar_value.is_none());
    }
}
