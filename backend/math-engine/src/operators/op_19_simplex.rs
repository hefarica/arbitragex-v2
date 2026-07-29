//! FUSILE: Implementacion propia -- Programacion Lineal (Simplex)
//! Formula: max cᵀx  s.t.  A·x ≤ b,  x ≥ 0
//! Resuelve la asignación óptima de capital entre venues/activos sujeto a
//! constraints de capital total y caps por venue.
//! Categoria: optimization
//!
//! Implementación: método Simplex por tableau (Dantzig), suficiente para los
//! tamaños de problema del hot-path (n_venues × n_assets pequeños).
//! R8 fail-honest: sin constraints válidas, devuelve scalar_value: None.

use super::{MarketState, OperatorOutput, TopologicalOperator};
use nalgebra::{DMatrix, DVector};
use std::collections::HashMap;

#[derive(Default)]
pub struct SimplexOperator;

impl SimplexOperator {
    pub fn new() -> Self {
        Self
    }

    /// Resuelve max cᵀx s.t. A x ≤ b, x ≥ 0 por el método Simplex (tableau).
    /// Devuelve (valor_objetivo, x*) o None si infactible/no acotado.
    pub fn simplex(c: &DVector<f64>, a: &DMatrix<f64>, b: &DVector<f64>) -> Option<(f64, DVector<f64>)> {
        let (m, n) = a.shape();
        if m == 0 || n == 0 || b.len() != m || c.len() != n {
            return None;
        }
        if b.iter().any(|&bi| bi < 0.0) {
            return None; // RHS negativa requiere fase 1; fuera del hot-path simple
        }

        // Tableau: [A | I | b], fila objetivo: [-c | 0 | 0]
        let mut t = DMatrix::zeros(m + 1, n + m + 1);
        for i in 0..m {
            for j in 0..n {
                t[(i, j)] = a[(i, j)];
            }
            t[(i, n + i)] = 1.0; // slack
            t[(i, n + m)] = b[i];
        }
        for j in 0..n {
            t[(m, j)] = -c[j];
        }

        let total_cols = n + m;
        for _ in 0..10_000 {
            // Columna pivote: el coeficiente más negativo en la fila objetivo.
            let mut pivot_col = None;
            let mut min_val = -1e-9;
            for j in 0..total_cols {
                if t[(m, j)] < min_val {
                    min_val = t[(m, j)];
                    pivot_col = Some(j);
                }
            }
            let pc = match pivot_col {
                Some(c) => c,
                None => break, // óptimo
            };
            // Fila pivote: razón mínima b_i / a_i(pc) con a_i(pc) > 0.
            let mut pivot_row = None;
            let mut min_ratio = f64::INFINITY;
            for i in 0..m {
                let aij = t[(i, pc)];
                if aij > 1e-12 {
                    let ratio = t[(i, n + m)] / aij;
                    if ratio < min_ratio {
                        min_ratio = ratio;
                        pivot_row = Some(i);
                    }
                }
            }
            let pr = match pivot_row {
                Some(r) => r,
                None => return None, // no acotado
            };
            // Pivoteo (Gauss-Jordan).
            let pv = t[(pr, pc)];
            for j in 0..=n + m {
                t[(pr, j)] /= pv;
            }
            for i in 0..=m {
                if i != pr {
                    let factor = t[(i, pc)];
                    for j in 0..=n + m {
                        t[(i, j)] -= factor * t[(pr, j)];
                    }
                }
            }
        }

        // Extraer x* (variables de decisión originales).
        let mut x = DVector::zeros(n);
        for j in 0..n {
            // ¿es columna básica (unitaria)?
            let mut basic_row = None;
            let mut is_unit = true;
            for i in 0..m {
                let v = t[(i, j)];
                if (v - 1.0).abs() < 1e-9 {
                    if basic_row.is_some() {
                        is_unit = false;
                        break;
                    }
                    basic_row = Some(i);
                } else if v.abs() > 1e-9 {
                    is_unit = false;
                    break;
                }
            }
            if is_unit {
                if let Some(r) = basic_row {
                    x[j] = t[(r, n + m)];
                }
            }
        }
        let obj = t[(m, n + m)];
        if !obj.is_finite() {
            return None;
        }
        Some((obj, x))
    }
}

impl TopologicalOperator for SimplexOperator {
    fn id(&self) -> u8 {
        19
    }

    fn name(&self) -> &'static str {
        "Programacion Lineal"
    }

    fn category(&self) -> &'static str {
        "optimization"
    }

    fn evaluate(&self, state: &MarketState) -> OperatorOutput {
        // Problema: asignar capital entre los assets (columnas) para maximizar
        // el valor esperado (media por asset), sujeto a capital total (de
        // features["max_capital"] o default 1.0) y caps por asset (reserves).
        let n_assets = state.price_matrix.first().map(|r| r.len()).unwrap_or(0);
        if n_assets == 0 || state.price_matrix.is_empty() {
            return OperatorOutput {
                operator_id: self.id(),
                operator_name: self.name().to_string(),
                scalar_value: None,
                vector_result: None,
                matrix_result: None,
                metadata: {
                    let mut m = HashMap::new();
                    m.insert("computed".to_string(), 0.0);
                    m.insert("reason_no_assets".to_string(), 1.0);
                    m
                },
            };
        }

        // c_j = media del asset j (valor esperado) a través de las filas.
        let n_rows = state.price_matrix.len() as f64;
        let mut c = DVector::zeros(n_assets);
        for j in 0..n_assets {
            let sum: f64 = state
                .price_matrix
                .iter()
                .map(|row| row.get(j).copied().unwrap_or(0.0))
                .sum();
            c[j] = sum / n_rows;
        }

        let max_capital = state.features.get("max_capital").copied().unwrap_or(1.0);

        // Constraints: sum(x) ≤ max_capital; x_j ≤ reserve_j (si hay reserves).
        let has_reserves = state.liquidity_reserves.len() >= n_assets;
        let m = 1 + if has_reserves { n_assets } else { 0 };
        let mut a = DMatrix::zeros(m, n_assets);
        let mut b = DVector::zeros(m);
        for j in 0..n_assets {
            a[(0, j)] = 1.0;
        }
        b[0] = max_capital;
        if has_reserves {
            for j in 0..n_assets {
                a[(1 + j, j)] = 1.0;
                b[1 + j] = state.liquidity_reserves[j].0.max(0.0);
            }
        }

        match Self::simplex(&c, &a, &b) {
            Some((obj, x)) => {
                let mut metadata = HashMap::new();
                metadata.insert("computed".to_string(), 1.0);
                metadata.insert("objective".to_string(), obj);
                metadata.insert("max_capital".to_string(), max_capital);
                OperatorOutput {
                    operator_id: self.id(),
                    operator_name: self.name().to_string(),
                    scalar_value: Some(obj),
                    vector_result: Some(x.iter().cloned().collect()),
                    matrix_result: None,
                    metadata,
                }
            }
            None => OperatorOutput {
                operator_id: self.id(),
                operator_name: self.name().to_string(),
                scalar_value: None,
                vector_result: None,
                matrix_result: None,
                metadata: {
                    let mut m = HashMap::new();
                    m.insert("computed".to_string(), 0.0);
                    m.insert("reason_lp_infeasible_or_unbounded".to_string(), 1.0);
                    m
                },
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn simplex_maximizes_linear_objective() {
        // max 3x + 2y  s.t.  x + y ≤ 4, x,y ≥ 0 → obj = 12 at x=4,y=0.
        let c = DVector::from_vec(vec![3.0, 2.0]);
        let a = DMatrix::from_row_slice(1, 2, &[1.0, 1.0]);
        let b = DVector::from_vec(vec![4.0]);
        let (obj, _x) = SimplexOperator::simplex(&c, &a, &b).unwrap();
        assert!((obj - 12.0).abs() < 1e-6, "expected obj 12 (got {obj})");
    }

    #[test]
    fn simplex_respects_per_asset_cap() {
        // max 3x + 2y  s.t.  x+y ≤ 100, x ≤ 4 → x=4, y=96 → obj = 3·4 + 2·96 = 204.
        // Without the x≤4 cap, obj would be 300 (x=100) — the cap binds x.
        let c = DVector::from_vec(vec![3.0, 2.0]);
        let a = DMatrix::from_row_slice(2, 2, &[1.0, 1.0, 1.0, 0.0]);
        let b = DVector::from_vec(vec![100.0, 4.0]);
        let (obj, x) = SimplexOperator::simplex(&c, &a, &b).unwrap();
        assert!((obj - 204.0).abs() < 1e-6, "expected obj 204 (got {obj})");
        assert!(x[0] <= 4.0 + 1e-6, "x must respect its cap (got {})", x[0]);
    }
}
