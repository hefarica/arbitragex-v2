//! FUSILE: Implementacion propia -- Proceso Markoviano de Salto-Difusion
//! Categoria: stochastic

use super::{MarketState, OperatorOutput, TopologicalOperator};
use std::collections::HashMap;

#[derive(Default)]
pub struct PDMPOperator;

impl PDMPOperator {
    pub fn new() -> Self {
        Self
    }
}

impl TopologicalOperator for PDMPOperator {
    fn id(&self) -> u8 {
        5
    }

    fn name(&self) -> &'static str {
        "Proceso Markoviano de Salto-Difusion"
    }

    fn category(&self) -> &'static str {
        "stochastic"
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
