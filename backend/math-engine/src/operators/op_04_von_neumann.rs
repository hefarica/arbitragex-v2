//! FUSILE: Implementacion propia -- Entropia de Von Neumann (entrelazamiento)
//! Formula: ρ = Σ / Tr(Σ)  (matriz de densidad reducida del mercado, Tr ρ = 1);
//!          S(ρ) = -Tr(ρ ln ρ) = -Σ_k λ̃_k ln λ̃_k,   con  0·ln0 := 0.
//! Rango: 0 ≤ S(ρ) ≤ ln m.
//! Físicamente: S→0 ⇒ varianza en un solo modo (señal de Asimetría Topológica
//!              coherente, régimen favorable de detección);
//!              S→ln m ⇒ varianza difusa (manifold descorrelacionado, sin eje).
//! Categoria: spectral
//!
//! R8 fail-honest: None si n<2, m<1, Tr(Σ)≈0 (no se puede normalizar ρ=Σ/0),
//!                 Σ no PSD (algún λ_k < -ε ⇒ ln indefinido), o entradas no finitas.

use super::{MarketState, OperatorOutput, TopologicalOperator};
use nalgebra::{DMatrix, linalg::SymmetricEigen};
use std::collections::HashMap;

#[derive(Default)]
pub struct VonNeumannOperator;

impl VonNeumannOperator {
    pub fn new() -> Self {
        Self
    }
}

impl TopologicalOperator for VonNeumannOperator {
    fn id(&self) -> u8 {
        4
    }
    fn name(&self) -> &'static str {
        "Entropia de Entrelazamiento"
    }
    fn category(&self) -> &'static str {
        "spectral"
    }

    fn evaluate(&self, state: &MarketState) -> OperatorOutput {
        let mut metadata = HashMap::new();
        match covariance_eigenvalues(state) {
            Some((eigs, trace)) => {
                let m = eigs.len() as f64;
                // λ̃_k = λ_k / Tr(Σ)  (autovalores de la matriz de densidad ρ).
                let mut s = 0.0_f64;
                let mut normalized: Vec<f64> = Vec::with_capacity(eigs.len());
                for &lam in &eigs {
                    let p = lam / trace;
                    normalized.push(p);
                    // -p ln p, con 0 ln 0 := 0 (p=0 ⇒ término 0).
                    if p > 0.0 {
                        s -= p * p.ln();
                    }
                }
                metadata.insert("computed".to_string(), 1.0);
                metadata.insert("entropy_nats".to_string(), s);
                metadata.insert("max_entropy_ln_m".to_string(), m.ln());
                // Pureza Tr(ρ²) = Σ λ̃²; =1 ⇔ estado puro (S=0).
                let purity: f64 = normalized.iter().map(|p| p * p).sum();
                metadata.insert("purity".to_string(), purity);
                OperatorOutput {
                    operator_id: self.id(),
                    operator_name: self.name().to_string(),
                    scalar_value: Some(s), // S(ρ) en nats
                    vector_result: Some(normalized),
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
            *e = 0.0; // clamp numérico de una PSD
        }
    }
    eigs.sort_by(|a, b| b.partial_cmp(a).unwrap_or(std::cmp::Ordering::Equal));
    Some((eigs, trace))
}
