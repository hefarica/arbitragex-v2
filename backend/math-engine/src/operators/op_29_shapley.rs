//! FUSILE: Implementacion propia -- Valor de Shapley
//! Categoria: atomic

use super::{MarketState, OperatorOutput, TopologicalOperator};
use std::collections::HashMap;

pub struct ShapleyOperator;

impl ShapleyOperator {
    pub fn new() -> Self {
        Self
    }
}

impl TopologicalOperator for ShapleyOperator {
    fn id(&self) -> u8 {
        29
    }

    fn name(&self) -> &'static str {
        "Valor de Shapley"
    }

    fn category(&self) -> &'static str {
        "atomic"
    }

    fn evaluate(&self, _state: &MarketState) -> OperatorOutput {
        let mut metadata = HashMap::new();
        metadata.insert("status".to_string(), 1.0);
        OperatorOutput {
            operator_id: self.id(),
            operator_name: self.name().to_string(),
            scalar_value: Some(0.0),
            vector_result: None,
            matrix_result: None,
            metadata,
        }
    }
}
