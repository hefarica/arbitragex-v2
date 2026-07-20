//! FUSILE: Implementacion propia -- Teoria de Colas
//! Categoria: queueing

use super::{MarketState, OperatorOutput, TopologicalOperator};
use std::collections::HashMap;

#[derive(Default)]
pub struct QueueingOperator;

impl QueueingOperator {
    pub fn new() -> Self {
        Self
    }
}

impl TopologicalOperator for QueueingOperator {
    fn id(&self) -> u8 {
        23
    }

    fn name(&self) -> &'static str {
        "Teoria de Colas"
    }

    fn category(&self) -> &'static str {
        "queueing"
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
