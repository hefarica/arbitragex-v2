//! FUSILE: Implementacion propia -- Reconstruccion de Bundle
//! Categoria: linear
//!
//! Reconstruccion conica de "legs" atomicos. Tomamos los primeros dos venues
//! (vectores de precio = filas de price_matrix) como legs y medimos la
//! colinealidad entre ellos via el coseno del angulo:
//!   cos θ = (a · b) / (|a| · |b|)
//! scalar_value = 1 − cos θ  (margen de reconstruccion: que tan no-colineales
//!                             son dos venues ⇒ factibilidad de Asimetria
//!                             Topologica reconstruible).
//!   cos θ ≈  1 ⇒ venues colineales (margen ≈ 0, sin Asimetria).
//!   cos θ ≈ −1 ⇒ venues opuestos (margen ≈ 2, maxima separacion).
//! vector_result = [cos_θ].
//!
//! R8 fail-honest: None si <2 venues o legs degenerados (norma ≈ 0) — nunca
//!                 un margen fabricado.

use super::{MarketState, OperatorOutput, TopologicalOperator};
use std::collections::HashMap;

#[derive(Default)]
pub struct BundleReconOperator;

impl BundleReconOperator {
    pub fn new() -> Self {
        Self
    }
}

impl TopologicalOperator for BundleReconOperator {
    fn id(&self) -> u8 {
        25
    }
    fn name(&self) -> &'static str {
        "Reconstruccion de Bundle"
    }
    fn category(&self) -> &'static str {
        "linear"
    }

    fn evaluate(&self, state: &MarketState) -> OperatorOutput {
        let mut metadata = HashMap::new();
        metadata.insert("n_venues".to_string(), state.price_matrix.len() as f64);

        // Fail-honest: <2 venues ⇒ no existe par de legs para reconstruir.
        if state.price_matrix.len() < 2 {
            metadata.insert("computed".to_string(), 0.0);
            metadata.insert("reason_insufficient_venues".to_string(), 1.0);
            return OperatorOutput {
                operator_id: self.id(),
                operator_name: self.name().to_string(),
                scalar_value: None,
                vector_result: None,
                matrix_result: None,
                metadata,
            };
        }

        let a = &state.price_matrix[0];
        let b = &state.price_matrix[1];
        // Longitud comun del par de legs (minimo por robustez ante matrices
        // mal formadas; una matrix canonica tiene filas de igual longitud).
        let m = a.len().min(b.len());
        metadata.insert("leg_dim".to_string(), m as f64);

        if m == 0 {
            metadata.insert("computed".to_string(), 0.0);
            metadata.insert("reason_degenerate_legs".to_string(), 1.0);
            return OperatorOutput {
                operator_id: self.id(),
                operator_name: self.name().to_string(),
                scalar_value: None,
                vector_result: None,
                matrix_result: None,
                metadata,
            };
        }

        let (mut dot, mut na2, mut nb2) = (0.0_f64, 0.0_f64, 0.0_f64);
        for i in 0..m {
            let ai = a[i];
            let bi = b[i];
            if !ai.is_finite() || !bi.is_finite() {
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
            dot += ai * bi;
            na2 += ai * ai;
            nb2 += bi * bi;
        }

        let na = na2.sqrt();
        let nb = nb2.sqrt();
        metadata.insert("norm_a".to_string(), na);
        metadata.insert("norm_b".to_string(), nb);

        // Legs degenerados (norma ≈ 0) ⇒ coseno no definido.
        if na < 1e-12 || nb < 1e-12 {
            metadata.insert("computed".to_string(), 0.0);
            metadata.insert("reason_degenerate_legs".to_string(), 1.0);
            return OperatorOutput {
                operator_id: self.id(),
                operator_name: self.name().to_string(),
                scalar_value: None,
                vector_result: None,
                matrix_result: None,
                metadata,
            };
        }

        let cos_theta = (dot / (na * nb)).clamp(-1.0, 1.0);
        let recon_margin = 1.0 - cos_theta;

        metadata.insert("computed".to_string(), 1.0);
        metadata.insert("cos_theta".to_string(), cos_theta);
        metadata.insert("reconstruction_margin".to_string(), recon_margin);

        OperatorOutput {
            operator_id: self.id(),
            operator_name: self.name().to_string(),
            scalar_value: Some(recon_margin),
            vector_result: Some(vec![cos_theta]),
            matrix_result: None,
            metadata,
        }
    }
}
