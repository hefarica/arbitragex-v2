//! FUSILE: Implementacion propia -- Descomposicion de Valores Singulares
//! Formula: A = U * Sigma * V^T
//! Categoria: spectral

use super::{MarketState, OperatorOutput, TopologicalOperator};
use nalgebra::{DMatrix, DVector};
use std::collections::HashMap;

#[derive(Default)]
pub struct SvdOperator;

impl SvdOperator {
    pub fn new() -> Self {
        Self
    }

    pub fn svd_jacobi(
        a: &DMatrix<f64>,
        max_iter: usize,
        tol: f64,
    ) -> (DMatrix<f64>, DVector<f64>, DMatrix<f64>) {
        let (m, n) = a.shape();
        let mut at = a.transpose() * a;
        let mut v = DMatrix::identity(n, n);

        for _ in 0..max_iter {
            let mut max_off = 0.0;
            for i in 0..n {
                for j in (i + 1)..n {
                    if at[(i, j)].abs() > max_off {
                        max_off = at[(i, j)].abs();
                    }
                }
            }
            if max_off < tol {
                break;
            }

            for i in 0..n {
                for j in (i + 1)..n {
                    if at[(i, j)].abs() > tol {
                        let tau = (at[(j, j)] - at[(i, i)]) / (2.0 * at[(i, j)]);
                        let t = if tau >= 0.0 {
                            1.0 / (tau + (1.0 + tau * tau).sqrt())
                        } else {
                            -1.0 / (-tau + (1.0 + tau * tau).sqrt())
                        };
                        let c = 1.0 / (1.0 + t * t).sqrt();
                        let s = t * c;

                        let aii = at[(i, i)];
                        let ajj = at[(j, j)];
                        let aij = at[(i, j)];
                        at[(i, i)] = c * c * aii - 2.0 * c * s * aij + s * s * ajj;
                        at[(j, j)] = s * s * aii + 2.0 * c * s * aij + c * c * ajj;
                        at[(i, j)] = 0.0;
                        at[(j, i)] = 0.0;

                        for k in 0..n {
                            if k != i && k != j {
                                let aik = at[(i, k)];
                                let ajk = at[(j, k)];
                                at[(i, k)] = c * aik - s * ajk;
                                at[(k, i)] = at[(i, k)];
                                at[(j, k)] = s * aik + c * ajk;
                                at[(k, j)] = at[(j, k)];
                            }
                        }

                        for k in 0..n {
                            let vki = v[(k, i)];
                            let vkj = v[(k, j)];
                            v[(k, i)] = c * vki - s * vkj;
                            v[(k, j)] = s * vki + c * vkj;
                        }
                    }
                }
            }
        }

        let mut sigma = DVector::zeros(n);
        for i in 0..n {
            sigma[i] = at[(i, i)].sqrt();
        }

        let mut u = a * &v;
        for j in 0..n {
            if sigma[j] > 1e-12 {
                for i in 0..m {
                    u[(i, j)] /= sigma[j];
                }
            }
        }
        (u, sigma, v)
    }
}

impl TopologicalOperator for SvdOperator {
    fn id(&self) -> u8 {
        1
    }
    fn name(&self) -> &'static str {
        "SVD"
    }
    fn category(&self) -> &'static str {
        "spectral"
    }

    fn evaluate(&self, state: &MarketState) -> OperatorOutput {
        let n = state.price_matrix.len();
        if n == 0 {
            return OperatorOutput {
                operator_id: self.id(),
                operator_name: self.name().to_string(),
                scalar_value: None,
                vector_result: None,
                matrix_result: None,
                metadata: HashMap::new(),
            };
        }
        let m = state.price_matrix[0].len();
        let mut data = Vec::with_capacity(n * m);
        for row in &state.price_matrix {
            for &val in row {
                data.push(val);
            }
        }
        let a = DMatrix::from_row_slice(n, m, &data);
        let (_u, sigma, _v) = Self::svd_jacobi(&a, 100, 1e-10);

        let sigma_vec: Vec<f64> = sigma.iter().cloned().collect();
        let mut metadata = HashMap::new();
        metadata.insert(
            "rank".to_string(),
            sigma.iter().filter(|&&s| s > 1e-10).count() as f64,
        );

        OperatorOutput {
            operator_id: self.id(),
            operator_name: self.name().to_string(),
            scalar_value: Some(sigma[0]),
            vector_result: Some(sigma_vec),
            matrix_result: None,
            metadata,
        }
    }
}
