//! FUSILE: Implementacion propia -- Ordenamiento Topologico de Rutas
//! Categoria: atomic

use super::{MarketState, OperatorOutput, TopologicalOperator};
use std::collections::HashMap;

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
        "Ordenamiento Topologico de Rutas"
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
