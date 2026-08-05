//! FUSILE: Implementacion propia -- Modelos Ocultos de Markov (Gaussian HMM, forward)
//! HMM de S=2 estados con emisiones Gaussianas (parámetros fijos estimados de r_t):
//!   π = [0.5, 0.5],   A = [[0.9,0.1],[0.1,0.9]]
//!   μ₁ = mean-std,   μ₂ = mean+std,   var = var(r)   (ambos estados comparten var)
//! Forward escalado (estable numéricamente):
//!   α₁(j) = π_j · b_j(o₁)
//!   α_t(j) = [Σ_i α_{t-1}(i) A_{ij}] · b_j(o_t),   c_t = Σ_j α_t(j),   α_t /= c_t
//!   ln P(O|θ) = Σ_t ln c_t
//!   b_j(o) = N(o; μ_j, var) = (2π·var)^(-1/2) · exp(-(o-μ_j)²/(2·var))
//! scalar_value = ln P(O|θ)/N   (log-verosimilitud por muestra, invariante a longitud).
//! vector_result = α_T = P(s_T=i | O, θ)   (posterior sobre el régimen oculto actual).
//! Categoria: stochastic
//!
//! R8 fail-honest: None si N<5, var≈0 (emisión Gaussiana degenerada en una δ, la
//!                 log-verosimilitud es indefinida), P(O|θ)=0 en algún paso
//!                 (observación fuera de soporte, ln 0 = -∞) o ln P no finito.

use super::{MarketState, OperatorOutput, TopologicalOperator};
use std::collections::HashMap;

#[derive(Default)]
pub struct HMMOperator;

impl HMMOperator {
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

    /// Densidad de probabilidad Gaussiana N(o; μ, var).
    #[inline]
    fn gaussian_pdf(o: f64, mean: f64, var: f64) -> f64 {
        if var <= 0.0 {
            return 0.0;
        }
        let coeff = 1.0 / (2.0 * std::f64::consts::PI * var).sqrt();
        let exponent = -((o - mean).powi(2)) / (2.0 * var);
        coeff * exponent.exp()
    }
}

impl TopologicalOperator for HMMOperator {
    fn id(&self) -> u8 {
        7
    }

    fn name(&self) -> &'static str {
        "Modelos Ocultos de Markov"
    }

    fn category(&self) -> &'static str {
        "stochastic"
    }

    fn evaluate(&self, state: &MarketState) -> OperatorOutput {
        const S: usize = 2;

        let prices = Self::price_series(state);
        let returns = Self::simple_returns(&prices);
        let n = returns.len();

        let mut metadata = HashMap::new();
        metadata.insert("n".to_string(), n as f64);

        if n < 5 {
            metadata.insert("computed".to_string(), 0.0);
            metadata.insert("reason_insufficient_observations".to_string(), 1.0);
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
        let var = returns
            .iter()
            .map(|r| (r - mean).powi(2))
            .sum::<f64>()
            / n as f64;
        let std = var.max(0.0).sqrt();

        // var≈0 ⇒ emisión degenerada, la log-verosimilitud es indefinida.
        if !mean.is_finite() || !std.is_finite() || var < 1e-18 {
            metadata.insert("computed".to_string(), 0.0);
            metadata.insert("reason_degenerate_emission".to_string(), 1.0);
            return OperatorOutput {
                operator_id: self.id(),
                operator_name: self.name().to_string(),
                scalar_value: None,
                vector_result: None,
                matrix_result: None,
                metadata,
            };
        }

        // Parámetros fijos del HMM (init por method-of-moments de los retornos).
        let init_pi = [0.5_f64, 0.5];
        let trans = [[0.9, 0.1], [0.1, 0.9]];
        let means = [mean - std, mean + std];
        let vars = [var, var];

        // Forward escalado.
        let mut log_lik = 0.0_f64;
        let mut alpha = [0.0f64; S];
        for t in 0..n {
            let o = returns[t];
            let mut nxt = [0.0f64; S];
            for j in 0..S {
                let b = Self::gaussian_pdf(o, means[j], vars[j]);
                if t == 0 {
                    nxt[j] = init_pi[j] * b;
                } else {
                    let prev_mix: f64 = (0..S).map(|i| alpha[i] * trans[i][j]).sum();
                    nxt[j] = prev_mix * b;
                }
            }
            let scale = nxt[0] + nxt[1];
            if !scale.is_finite() || scale <= 0.0 {
                // P(O|θ)=0 ⇒ ln 0 = -∞ (observación fuera de soporte de toda emisión).
                metadata.insert("computed".to_string(), 0.0);
                metadata.insert("reason_zero_likelihood".to_string(), 1.0);
                return OperatorOutput {
                    operator_id: self.id(),
                    operator_name: self.name().to_string(),
                    scalar_value: None,
                    vector_result: None,
                    matrix_result: None,
                    metadata,
                };
            }
            log_lik += scale.ln();
            for v in nxt.iter_mut() {
                *v /= scale;
            }
            alpha = nxt;
        }

        let ll_per_sample = log_lik / n as f64;

        if !ll_per_sample.is_finite() {
            metadata.insert("computed".to_string(), 0.0);
            metadata.insert("reason_non_finite".to_string(), 1.0);
            return OperatorOutput {
                operator_id: self.id(),
                operator_name: self.name().to_string(),
                scalar_value: None,
                vector_result: None,
                matrix_result: None,
                metadata,
            };
        }

        // α_T normalizado = posterior P(s_T=i | O, θ) sobre el régimen oculto final.
        metadata.insert("computed".to_string(), 1.0);
        metadata.insert("log_likelihood".to_string(), log_lik);
        metadata.insert("ll_per_sample".to_string(), ll_per_sample);
        metadata.insert("n_states".to_string(), S as f64);
        metadata.insert("emission_var".to_string(), var);

        OperatorOutput {
            operator_id: self.id(),
            operator_name: self.name().to_string(),
            scalar_value: Some(ll_per_sample),
            vector_result: Some(vec![alpha[0], alpha[1]]),
            matrix_result: None,
            metadata,
        }
    }
}
