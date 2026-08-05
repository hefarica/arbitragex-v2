//! FUSILE: Implementacion propia -- Analisis de Componentes Principales (PCA)
//! Formula: covarianza muestral Σ = (1/(n-1)) X̃ᵀ X̃;  Σ = QΛQᵀ (teorema espectral);
//!          ρ₁ = λ_max / Tr(Σ)  (fracción de varianza del eje sistémico dominante).
//! Físicamente: ρ₁→1 ⇒ el mercado se mueve por un solo modo (Asimetría Topológica
//!              concentrada, régimen favorable); ρ₁→1/m ⇒ varianza difusa (sin eje).
//! Categoria: spectral
//!
//! R8 fail-honest: scalar_value = None si n<2, m<1, Tr(Σ)≈0, filas inconsistentes
//!                 o entradas no finitas. Nunca un Some fabricado.

use super::{MarketState, OperatorOutput, TopologicalOperator};
use nalgebra::{DMatrix, linalg::SymmetricEigen};
use std::collections::HashMap;

#[derive(Default)]
pub struct PCAOperator;

impl PCAOperator {
    pub fn new() -> Self {
        Self
    }
}

impl TopologicalOperator for PCAOperator {
    fn id(&self) -> u8 {
        2
    }
    fn name(&self) -> &'static str {
        "Analisis de Componentes Principales"
    }
    fn category(&self) -> &'static str {
        "spectral"
    }

    fn evaluate(&self, state: &MarketState) -> OperatorOutput {
        let mut metadata = HashMap::new();
        match covariance_eigenvalues(state) {
            Some((eigs, trace)) => {
                let rho1 = eigs[0] / trace; // λ_max / Tr(Σ)
                metadata.insert("computed".to_string(), 1.0);
                metadata.insert("trace".to_string(), trace);
                metadata.insert("lambda_max".to_string(), eigs[0]);
                metadata.insert("n_components".to_string(), eigs.len() as f64);
                OperatorOutput {
                    operator_id: self.id(),
                    operator_name: self.name().to_string(),
                    scalar_value: Some(rho1),
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

/// Covarianza muestral (m×m, simétrica PSD) de price_matrix centrada por columna
/// + sus autovalores ordenados desc. Devuelve (autovalores, traza). None si
/// n<2, m<1, filas inconsistentes, entradas no finitas, o traza ≈ 0.
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
    // Media por columna.
    let mut means = vec![0.0_f64; m];
    for row in &state.price_matrix {
        for (j, &v) in row.iter().enumerate() {
            means[j] += v;
        }
    }
    for mj in means.iter_mut() {
        *mj /= n as f64;
    }
    // Matriz centrada X̃.
    let mut centered = Vec::with_capacity(n * m);
    for row in &state.price_matrix {
        for (j, &v) in row.iter().enumerate() {
            centered.push(v - means[j]);
        }
    }
    let x = DMatrix::from_row_slice(n, m, &centered);
    // Σ = (1/(n-1)) X̃ᵀ X̃  (simétrica PSD por construcción).
    let cov = x.transpose() * &x / ((n - 1) as f64);
    let trace = cov.trace();
    if !trace.is_finite() || trace.abs() < 1e-12 {
        return None;
    }
    // Teorema espectral: autovalores reales ≥ 0 (PSD).
    let eig = SymmetricEigen::new(cov);
    let mut eigs: Vec<f64> = eig.eigenvalues.iter().copied().collect();
    for e in eigs.iter_mut() {
        if !e.is_finite() {
            return None;
        }
        if *e < 0.0 {
            *e = 0.0; // clamp tiny-negativos numéricos de una PSD
        }
    }
    eigs.sort_by(|a, b| b.partial_cmp(a).unwrap_or(std::cmp::Ordering::Equal));
    Some((eigs, trace))
}
