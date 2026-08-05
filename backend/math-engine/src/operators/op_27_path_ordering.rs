//! FUSILE: Implementacion propia -- Ordenamiento de Camino
//! Categoria: combinatorics
//!
//! Ordenamiento optimo (buy-low sell-high) sobre el precio de un mismo asset a
//! traves de venues. Sea p_i = price_matrix[i][0] (precio del asset 0 en venue i).
//! Se localizan:
//!   argmin_i p_i  (venue de compra barata) y  argmax_i p_i  (venue de venta cara).
//! scalar_value = (max_p − min_p) / min_p  (spread bruto del mejor camino simple).
//! vector_result = [min_idx, max_idx].
//!
//! R8 fail-honest: None si <2 venues validas o min_p ≤ 0 (division indefinida).
//!                 Precios identicos ⇒ spread 0.0 es Some(0.0) honesto, NO None.

use super::{MarketState, OperatorOutput, TopologicalOperator};
use std::collections::HashMap;

#[derive(Default)]
pub struct PathOrderingOperator;

impl PathOrderingOperator {
    pub fn new() -> Self {
        Self
    }
}

impl TopologicalOperator for PathOrderingOperator {
    fn id(&self) -> u8 {
        27
    }
    fn name(&self) -> &'static str {
        "Ordenamiento de Camino"
    }
    fn category(&self) -> &'static str {
        "combinatorics"
    }

    fn evaluate(&self, state: &MarketState) -> OperatorOutput {
        let mut metadata = HashMap::new();
        metadata.insert("n_venues".to_string(), state.price_matrix.len() as f64);

        // Precios del asset 0 por venue (filtrados finitos).
        let prices: Vec<(usize, f64)> = state
            .price_matrix
            .iter()
            .enumerate()
            .filter_map(|(i, row)| row.first().copied().filter(|p| p.is_finite()).map(|p| (i, p)))
            .collect();
        metadata.insert("valid_venues".to_string(), prices.len() as f64);

        // Fail-honest: <2 venues validas ⇒ ningun camino buy/sell posible.
        if prices.len() < 2 {
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

        // argmin / argmax sobre precios validos.
        let mut min_idx = prices[0].0;
        let mut min_p = prices[0].1;
        let mut max_idx = prices[0].0;
        let mut max_p = prices[0].1;
        for &(idx, p) in prices.iter().skip(1) {
            if p < min_p {
                min_p = p;
                min_idx = idx;
            }
            if p > max_p {
                max_p = p;
                max_idx = idx;
            }
        }

        metadata.insert("min_price".to_string(), min_p);
        metadata.insert("max_price".to_string(), max_p);
        metadata.insert("min_venue".to_string(), min_idx as f64);
        metadata.insert("max_venue".to_string(), max_idx as f64);

        // min_p ≤ 0 ⇒ spread relativo indefinido (division por ~0 o negativo).
        if min_p <= 0.0 {
            metadata.insert("computed".to_string(), 0.0);
            metadata.insert("reason_degenerate_prices".to_string(), 1.0);
            return OperatorOutput {
                operator_id: self.id(),
                operator_name: self.name().to_string(),
                scalar_value: None,
                vector_result: None,
                matrix_result: None,
                metadata,
            };
        }

        let spread = (max_p - min_p) / min_p;
        metadata.insert("computed".to_string(), 1.0);
        metadata.insert("gross_spread".to_string(), spread);

        OperatorOutput {
            operator_id: self.id(),
            operator_name: self.name().to_string(),
            scalar_value: Some(spread),
            vector_result: Some(vec![min_idx as f64, max_idx as f64]),
            matrix_result: None,
            metadata,
        }
    }
}
