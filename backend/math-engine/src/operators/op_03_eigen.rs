//! FUSILE: Implementacion propia -- Autovalores (radio espectral de la covarianza)
//! Formula: Σ = (1/(n-1)) X̃ᵀ X̃ (simétrica PSD);  Σq_k = λ_k q_k;
//!          λ_max = ‖Σ‖₂ = sup_{‖x‖=1} xᵀΣx  (autovalor dominante / radio espectral).
//! Físicamente: λ_max es la energía del modo sistémico de mayor varianza
//!              (eje de Acoplamiento Topológico dominante en la Variedad de Liquidez).
//! Categoria: spectral
//!
//! R8 fail-honest: scalar_value = None si n<2, m<1, Tr(Σ)≈0, filas inconsistentes
//!                 o entradas no finitas. Singularity (algún λ_k=0) NO fuerza None:
//!                 λ_max de una PSD está bien definido aunque rango < m.

use super::{MarketState, OperatorOutput, TopologicalOperator};
use nalgebra::{linalg::SymmetricEigen, DMatrix};
use std::collections::HashMap;

#[derive(Default)]
pub struct EigenOperator;

impl EigenOperator {
    pub fn new() -> Self {
        Self
    }
}

impl TopologicalOperator for EigenOperator {
    fn id(&self) -> u8 {
        3
    }
    fn name(&self) -> &'static str {
        "Autovalores y Autovectores"
    }
    fn category(&self) -> &'static str {
        "spectral"
    }

    fn evaluate(&self, state: &MarketState) -> OperatorOutput {
        let mut metadata = HashMap::new();
        match covariance_eigenvalues(state) {
            Some((eigs, trace)) => {
                metadata.insert("computed".to_string(), 1.0);
                metadata.insert("trace".to_string(), trace);
                metadata.insert("lambda_max".to_string(), eigs[0]);
                // κ(Σ) = λ_max/λ_min: número de condición espectral (metadata).
                if let Some(&lam_min) = eigs.last() {
                    if lam_min > 1e-12 {
                        metadata.insert("condition_number".to_string(), eigs[0] / lam_min);
                    }
                }
                metadata.insert(
                    "numerical_rank".to_string(),
                    eigs.iter().filter(|&&e| e > 1e-10).count() as f64,
                );
                OperatorOutput {
                    operator_id: self.id(),
                    operator_name: self.name().to_string(),
                    scalar_value: Some(eigs[0]), // λ_max = radio espectral
                    vector_result: Some(eigs),
                    matrix_result: None,
                    metadata,
                }
            }
            None => {
                metadata.insert("computed".to_string(), 0.0);
                metadata.insert("reason_insufficient_data".to_string(), 1.0);
                OperatorOutput {
                    operator_id: self.id(),
                    operator_name: self.name().to_string(),
                    scalar_value: None,
                    vector_result: None,
                    matrix_result: None,
                    metadata,
                }
            }
        }
    }
}

/// Covarianza muestral (m×m, PSD) + autovalores desc. (mismo contrato que op_02).
fn covariance_eigenvalues(state: &MarketState) -> Option<(Vec<f64>, f64)> {
    let n = state.price_matrix.len();
    if n < 2 {
        return None;
    }
    let m = state.price_matrix[0].len();
    if m < 1 {
        return None;
    }
    for row in &state.price_matrix {
        if row.len() != m {
            return None;
        }
        if row.iter().any(|v| !v.is_finite()) {
            return None;
        }
    }
    let mut means = vec![0.0_f64; m];
    for row in &state.price_matrix {
        for (j, &v) in row.iter().enumerate() {
            means[j] += v;
        }
    }
    for mj in means.iter_mut() {
        *mj /= n as f64;
    }
    let mut centered = Vec::with_capacity(n * m);
    for row in &state.price_matrix {
        for (j, &v) in row.iter().enumerate() {
            centered.push(v - means[j]);
        }
    }
    let x = DMatrix::from_row_slice(n, m, &centered);
    let cov = x.transpose() * &x / ((n - 1) as f64);
    let trace = cov.trace();
    if !trace.is_finite() || trace.abs() < 1e-12 {
        return None;
    }
    let eig = SymmetricEigen::new(cov);
    let mut eigs: Vec<f64> = eig.eigenvalues.iter().copied().collect();
    for e in eigs.iter_mut() {
        if !e.is_finite() {
            return None;
        }
        if *e < 0.0 {
            *e = 0.0;
        }
    }
    eigs.sort_by(|a, b| b.partial_cmp(a).unwrap_or(std::cmp::Ordering::Equal));
    Some((eigs, trace))
}
