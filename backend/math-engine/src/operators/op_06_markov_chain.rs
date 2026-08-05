//! FUSILE: Implementacion propia -- Cadenas de Markov (cadena discreta sobre retornos)
//! Discretización en K=3 estados: down (r<-τ), flat (|r|≤τ), up (r>τ), con τ=0.001.
//!   C_{ij} = #{t : s_t=i, s_{t+1}=j},   P_{ij} = C_{ij}/Σ_ℓ C_{iℓ}   (row-stochastic)
//! Distribución estacionaria π: πP = π (autovector izquierdo de P en λ=1).
//! Autovalores: λ₁=1 (Perron-Frobenius), λ₂,λ₃ raíces de λ²-(tr(P)-1)λ+det(P)=0
//!   (factorizando λ₁=1 de la cúbica característica).
//! scalar_value = gap espectral γ = 1 - |λ₂|   (tasa de decorrelación del estado de
//!                retornos de la Variedad de Liquidez). π y H(π) emitidos en
//!                vector_result / metadata.
//! Categoria: stochastic
//!
//! R8 fail-honest: None si N<6 (<5 transiciones, insuficiente para poblar la matriz
//!                 3×3) o |λ₂| no finito.

use super::{MarketState, OperatorOutput, TopologicalOperator};
use std::collections::HashMap;

#[derive(Default)]
pub struct MarkovChainOperator;

impl MarkovChainOperator {
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

    /// Índice de estado K=3: 0=down, 1=flat, 2=up con umbral ±τ.
    #[inline]
    fn state_of(r: f64, tau: f64) -> usize {
        if r < -tau {
            0
        } else if r > tau {
            2
        } else {
            1
        }
    }
}

impl TopologicalOperator for MarkovChainOperator {
    fn id(&self) -> u8 {
        6
    }

    fn name(&self) -> &'static str {
        "Cadenas de Markov"
    }

    fn category(&self) -> &'static str {
        "stochastic"
    }

    fn evaluate(&self, state: &MarketState) -> OperatorOutput {
        const K: usize = 3;
        const THRESH: f64 = 0.001;

        let prices = Self::price_series(state);
        let returns = Self::simple_returns(&prices);
        let n = returns.len();

        let mut metadata = HashMap::new();
        metadata.insert("n".to_string(), n as f64);

        // N<6 ⇒ menos de 5 transiciones, insuficiente para una matriz 3×3.
        if n < 6 {
            metadata.insert("computed".to_string(), 0.0);
            metadata.insert("reason_insufficient_transitions".to_string(), 1.0);
            return OperatorOutput {
                operator_id: self.id(),
                operator_name: self.name().to_string(),
                scalar_value: None,
                vector_result: None,
                matrix_result: None,
                metadata,
            };
        }

        // Conteos de transición C[K][K].
        let mut c = [[0u64; K]; K];
        for w in returns.windows(2) {
            let i = Self::state_of(w[0], THRESH);
            let j = Self::state_of(w[1], THRESH);
            c[i][j] += 1;
        }

        // Matriz de transición row-stochastic P. Filas no visitadas → uniforme 1/K
        // (mantiene P bien definida y row-stochastic).
        let mut p = [[0.0f64; K]; K];
        for i in 0..K {
            let row_sum: u64 = c[i].iter().sum();
            if row_sum > 0 {
                for j in 0..K {
                    p[i][j] = c[i][j] as f64 / row_sum as f64;
                }
            } else {
                for j in 0..K {
                    p[i][j] = 1.0 / K as f64;
                }
            }
        }

        // Trazas y determinante de P para los autovalores ≠ 1.
        let tr = p[0][0] + p[1][1] + p[2][2];
        let det = p[0][0] * (p[1][1] * p[2][2] - p[1][2] * p[2][1])
            - p[0][1] * (p[1][0] * p[2][2] - p[1][2] * p[2][0])
            + p[0][2] * (p[1][0] * p[2][1] - p[1][1] * p[2][0]);
        // λ₂,λ₃ son raíces de λ²-(tr-1)λ+det = 0 (al factorizar λ₁=1).
        let b = tr - 1.0;
        let disc = b * b - 4.0 * det;
        let lam2_mod = if disc >= 0.0 {
            // Dos raíces reales: |λ₂| = máximo módulo.
            let sq = disc.sqrt();
            let r1 = (b + sq) / 2.0;
            let r2 = (b - sq) / 2.0;
            r1.abs().max(r2.abs())
        } else {
            // Par conjugado complejo a±bi: módulo común = √det (det>0 aquí).
            det.max(0.0).sqrt()
        };

        if !lam2_mod.is_finite() {
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

        // Gap espectral γ = 1 - |λ₂|, clampeado a [0,1].
        let spectral_gap = (1.0 - lam2_mod).clamp(0.0, 1.0);

        // Distribución estacionaria π vía iteración de potencia (π ← πP).
        let mut pi = [1.0 / K as f64; K];
        for _ in 0..2000 {
            let mut pi_next = [0.0f64; K];
            for j in 0..K {
                pi_next[j] = (0..K).map(|i| pi[i] * p[i][j]).sum();
            }
            let diff: f64 = (0..K).map(|i| (pi_next[i] - pi[i]).abs()).sum();
            pi = pi_next;
            if diff < 1e-15 {
                break;
            }
        }
        // Renormalizar por seguridad frente al redondeo.
        let pi_sum: f64 = pi.iter().sum();
        if pi_sum > 0.0 {
            for v in pi.iter_mut() {
                *v /= pi_sum;
            }
        }
        // Entropía estacionaria H(π) = -Σ π_i ln π_i   (convención 0 ln 0 := 0).
        let entropy: f64 = pi
            .iter()
            .copied()
            .filter(|p| *p > 0.0)
            .map(|p| -p * p.ln())
            .sum();

        metadata.insert("computed".to_string(), 1.0);
        metadata.insert("spectral_gap".to_string(), spectral_gap);
        metadata.insert("lambda2_mod".to_string(), lam2_mod);
        metadata.insert("stationary_entropy".to_string(), entropy);
        metadata.insert("k_states".to_string(), K as f64);
        metadata.insert("n_transitions".to_string(), (n - 1) as f64);

        OperatorOutput {
            operator_id: self.id(),
            operator_name: self.name().to_string(),
            scalar_value: Some(spectral_gap),
            vector_result: Some(pi.to_vec()),
            matrix_result: Some(p.iter().map(|row| row.to_vec()).collect()),
            metadata,
        }
    }
}
