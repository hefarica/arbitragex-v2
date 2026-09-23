//! FUSILE: Implementacion propia -- Equilibrio de Nash
//! Formula: sigma* in argmax u_i(sigma_i, sigma_{-i}*) para todo i
//! Categoria: game_theory

use super::{MarketState, OperatorOutput, TopologicalOperator};
use nalgebra::DMatrix;
use std::collections::HashMap;

#[derive(Default)]
pub struct NashOperator;

impl NashOperator {
    pub fn new() -> Self {
        Self
    }

    pub fn solve_pure_2x2(
        row_payoffs: &DMatrix<f64>,
        col_payoffs: &DMatrix<f64>,
    ) -> Option<(usize, usize)> {
        let (rows, cols) = row_payoffs.shape();
        if rows != 2 || cols != 2 {
            return None;
        }
        for r in 0..rows {
            for c in 0..cols {
                let row_best = (0..rows).all(|rp| row_payoffs[(rp, c)] <= row_payoffs[(r, c)]);
                let col_best = (0..cols).all(|cp| col_payoffs[(r, cp)] <= col_payoffs[(r, c)]);
                if row_best && col_best {
                    return Some((r, c));
                }
            }
        }
        None
    }

    pub fn solve_mixed_2x2(
        row_payoffs: &DMatrix<f64>,
        col_payoffs: &DMatrix<f64>,
    ) -> Option<(f64, f64)> {
        let a = row_payoffs;
        let b = col_payoffs;
        let denom_col = a[(0, 0)] - a[(0, 1)] - a[(1, 0)] + a[(1, 1)];
        if denom_col.abs() < 1e-12 {
            return None;
        }
        let p = (a[(1, 1)] - a[(0, 1)]) / denom_col;
        if !(0.0..=1.0).contains(&p) {
            return None;
        }
        let denom_row = b[(0, 0)] - b[(1, 0)] - b[(0, 1)] + b[(1, 1)];
        if denom_row.abs() < 1e-12 {
            return None;
        }
        let q = (b[(1, 1)] - b[(1, 0)]) / denom_row;
        if !(0.0..=1.0).contains(&q) {
            return None;
        }
        Some((p, q))
    }
}

impl TopologicalOperator for NashOperator {
    fn id(&self) -> u8 {
        24
    }
    fn name(&self) -> &'static str {
        "Nash"
    }
    fn category(&self) -> &'static str {
        "game_theory"
    }

    fn evaluate(&self, _state: &MarketState) -> OperatorOutput {
        let row_payoffs = DMatrix::from_row_slice(2, 2, &[2.0, 0.0, 3.0, 1.0]);
        let col_payoffs = DMatrix::from_row_slice(2, 2, &[2.0, 3.0, 0.0, 1.0]);
        let pure = NashOperator::solve_pure_2x2(&row_payoffs, &col_payoffs);
        let mixed = NashOperator::solve_mixed_2x2(&row_payoffs, &col_payoffs);

        let mut metadata = HashMap::new();
        metadata.insert(
            "has_pure".to_string(),
            if pure.is_some() { 1.0 } else { 0.0 },
        );
        metadata.insert(
            "has_mixed".to_string(),
            if mixed.is_some() { 1.0 } else { 0.0 },
        );
        if let Some((p, q)) = mixed {
            metadata.insert("row_mixed_p".to_string(), p);
            metadata.insert("col_mixed_q".to_string(), q);
        }

        OperatorOutput {
            operator_id: self.id(),
            operator_name: self.name().to_string(),
            scalar_value: mixed.map(|(p, _)| p),
            vector_result: None,
            matrix_result: None,
            metadata,
        }
    }
}
