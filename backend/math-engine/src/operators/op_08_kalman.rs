//! FUSILE: Implementacion propia -- Filtro de Kalman (mid-price latente)
//! Modelo (camino aleatorio sobre el mid-price latente x, observación z = precio):
//!   x_k = x_{k-1} + w_k,  w_k ~ N(0, Q)     (Q = Var(Δp): ruido de proceso)
//!   z_k = x_k     + v_k,  v_k ~ N(0, R)     (R = Var(p):  ruido de observación)
//! Predict:  x̂⁻ = x̂ ,  P⁻ = P + Q
//! Update:   K = P⁻/(P⁻+R),  x̂ = x̂⁻ + K(z_k − x̂⁻),  P = (1−K)P⁻
//! scalar_value = |ν_K| / √(P⁻_K + R)  (innovación final normalizada = z-score de
//!              mispricing del último precio respecto al mid filtrado). Grande ⇒
//!              el último precio es un outlier de mispricing (señal de Asimetría).
//! Categoria: stochastic
//!
//! R8 fail-honest: None si n<3 (se necesitan ≥2 diferencias para estimar Q),
//!                 Var(p)≈0 (R=0 ⇒ no se puede normalizar la innovación), o
//!                 entradas no finitas.

use super::{MarketState, OperatorOutput, TopologicalOperator};
use std::collections::HashMap;

#[derive(Default)]
pub struct KalmanOperator;

impl KalmanOperator {
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
            .filter(|p| p.is_finite())
            .collect()
    }

    fn variance(xs: &[f64]) -> f64 {
        if xs.len() < 2 {
            return 0.0;
        }
        let mean = xs.iter().sum::<f64>() / xs.len() as f64;
        xs.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / (xs.len() - 1) as f64
    }
}

impl TopologicalOperator for KalmanOperator {
    fn id(&self) -> u8 {
        8
    }
    fn name(&self) -> &'static str {
        "Filtro de Kalman"
    }
    fn category(&self) -> &'static str {
        "stochastic"
    }

    fn evaluate(&self, state: &MarketState) -> OperatorOutput {
        let prices = Self::price_series(state);
        let mut metadata = HashMap::new();
        metadata.insert("n".to_string(), prices.len() as f64);

        // n<3 ⇒ no hay ≥2 diferencias para estimar Q.
        if prices.len() < 3 {
            metadata.insert("computed".to_string(), 0.0);
            metadata.insert("reason_insufficient_data".to_string(), 1.0);
            return OperatorOutput {
                operator_id: self.id(),
                operator_name: self.name().to_string(),
                scalar_value: None,
                vector_result: None,
                matrix_result: None,
                metadata,
            };
        }

        // R = Var(precios): ruido de observación.
        let r = Self::variance(&prices);
        // Q = Var(Δp): ruido de proceso (paso del camino aleatorio).
        let diffs: Vec<f64> = prices.windows(2).map(|w| w[1] - w[0]).collect();
        let q = Self::variance(&diffs);

        if !r.is_finite() || !q.is_finite() || r < 1e-12 {
            metadata.insert("computed".to_string(), 0.0);
            metadata.insert("reason_zero_observation_variance".to_string(), 1.0);
            return OperatorOutput {
                operator_id: self.id(),
                operator_name: self.name().to_string(),
                scalar_value: None,
                vector_result: None,
                matrix_result: None,
                metadata,
            };
        }

        // Filtro: init x̂ = p_0, P = R (misma incertidumbre que la observación).
        let mut x_hat = prices[0];
        let mut p_est = r;
        let mut last_innov = 0.0_f64;
        let mut last_pred_var = r;
        for z in prices.iter().skip(1) {
            // Predict
            let x_pred = x_hat;
            let p_pred = p_est + q;
            // Innovation
            let innov = z - x_pred;
            // Kalman gain + update
            let k = p_pred / (p_pred + r);
            x_hat = x_pred + k * innov;
            p_est = (1.0 - k) * p_pred;
            last_innov = innov;
            last_pred_var = p_pred;
        }

        // z-score de mispricing del último precio (innovación normalizada).
        let innov_std = (last_pred_var + r).max(1e-12).sqrt();
        let mispricing_z = (last_innov.abs()) / innov_std;

        metadata.insert("computed".to_string(), 1.0);
        metadata.insert("filtered_mid".to_string(), x_hat);
        metadata.insert("process_noise_q".to_string(), q);
        metadata.insert("observation_noise_r".to_string(), r);
        metadata.insert("last_innovation".to_string(), last_innov);
        metadata.insert("mispricing_z".to_string(), mispricing_z);

        OperatorOutput {
            operator_id: self.id(),
            operator_name: self.name().to_string(),
            scalar_value: Some(mispricing_z),
            vector_result: Some(vec![x_hat, mispricing_z, last_innov]),
            matrix_result: None,
            metadata,
        }
    }
}
