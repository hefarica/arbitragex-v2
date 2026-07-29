//! FUSILE: Implementacion propia -- Regresion Lineal por Minimos Cuadrados
//! Formula: β̂ = (XᵀX)⁻¹ Xᵀy, con X = [1, t] (intercepto + tiempo como regresor).
//! Predice la tendencia (pendiente) de la serie de precios, no el nivel.
//! Categoria: inference
//!
//! R8 fail-honest: sin datos suficientes o sistema singular, devuelve
//! scalar_value: None (no computado) — nunca una pendiente fabricada.

use super::{MarketState, OperatorOutput, TopologicalOperator};
use nalgebra::{DMatrix, DVector};
use std::collections::HashMap;

#[derive(Default)]
pub struct RegressionOperator;

impl RegressionOperator {
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
}

impl TopologicalOperator for RegressionOperator {
    fn id(&self) -> u8 {
        13
    }

    fn name(&self) -> &'static str {
        "Regresion Lineal/Logistica"
    }

    fn category(&self) -> &'static str {
        "inference"
    }

    fn evaluate(&self, state: &MarketState) -> OperatorOutput {
        let prices = Self::price_series(state);
        let n = prices.len();
        if n < 3 {
            return OperatorOutput {
                operator_id: self.id(),
                operator_name: self.name().to_string(),
                scalar_value: None,
                vector_result: None,
                matrix_result: None,
                metadata: {
                    let mut m = HashMap::new();
                    m.insert("computed".to_string(), 0.0);
                    m.insert("reason_insufficient_data".to_string(), 1.0);
                    m
                },
            };
        }

        // X = [1, t] (n × 2); y = precios (n). Regresor = índice temporal, así la
        // pendiente captura la TENDENCIA (no el nivel, que estaría sesgado).
        let mut x = DMatrix::zeros(n, 2);
        let mut y = DVector::zeros(n);
        for (i, &price) in prices.iter().enumerate() {
            x[(i, 0)] = 1.0;
            x[(i, 1)] = i as f64;
            y[i] = price;
        }

        // β̂ = (XᵀX)⁻¹ Xᵀy. Resolver por least-squares (nalgebra::linalg).
        let xtx = x.transpose() * &x;
        let xty = x.transpose() * &y;

        // Sistema 2×2 — resolver de forma cerrada si es no-singular.
        let a = xtx[(0, 0)];
        let b = xtx[(0, 1)];
        let c = xtx[(1, 0)];
        let d = xtx[(1, 1)];
        let det = a * d - b * c;

        if det.abs() < 1e-12 {
            return OperatorOutput {
                operator_id: self.id(),
                operator_name: self.name().to_string(),
                scalar_value: None,
                vector_result: None,
                matrix_result: None,
                metadata: {
                    let mut m = HashMap::new();
                    m.insert("computed".to_string(), 0.0);
                    m.insert("reason_singular".to_string(), 1.0);
                    m
                },
            };
        }

        let e0 = xty[0];
        let e1 = xty[1];
        // Inversa 2×2: (1/det)·[[d, -b],[-c, a]]
        let intercept = (d * e0 - b * e1) / det;
        let slope = (-c * e0 + a * e1) / det;

        // R² (bondad de ajuste) — qué tan bien la recta explica la serie.
        let y_mean = y.iter().sum::<f64>() / n as f64;
        let mut ss_tot = 0.0;
        let mut ss_res = 0.0;
        for (i, &price) in prices.iter().enumerate() {
            let pred = intercept + slope * i as f64;
            ss_res += (price - pred).powi(2);
            ss_tot += (price - y_mean).powi(2);
        }
        let r_squared = if ss_tot > 1e-12 {
            1.0 - ss_res / ss_tot
        } else {
            0.0
        };

        let direction = if slope > 0.0 { "UP" } else if slope < 0.0 { "DOWN" } else { "FLAT" };

        let mut metadata = HashMap::new();
        metadata.insert("computed".to_string(), 1.0);
        metadata.insert("slope".to_string(), slope);
        metadata.insert("intercept".to_string(), intercept);
        metadata.insert("r_squared".to_string(), r_squared);
        metadata.insert(
            "direction".to_string(),
            match direction {
                "UP" => 1.0,
                "DOWN" => -1.0,
                _ => 0.0,
            },
        );

        OperatorOutput {
            operator_id: self.id(),
            operator_name: self.name().to_string(),
            // Magnitud principal: pendiente normalizada por el precio medio
            // (tendencia relativa por paso temporal, comparable entre activos).
            scalar_value: Some(if y_mean.abs() > 1e-12 {
                slope / y_mean
            } else {
                slope
            }),
            vector_result: Some(vec![slope, intercept, r_squared]),
            matrix_result: None,
            metadata,
        }
    }
}
