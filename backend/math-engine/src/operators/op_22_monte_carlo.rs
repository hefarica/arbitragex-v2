//! FUSILE: Implementacion propia -- Simulacion Estocastica
//! Categoria: simulation

use super::{MarketState, OperatorOutput, TopologicalOperator};
use std::collections::HashMap;

pub struct MonteCarloOperator;

impl MonteCarloOperator {
    pub fn new() -> Self {
        Self
    }
}

impl TopologicalOperator for MonteCarloOperator {
    fn id(&self) -> u8 {
        22
    }

    fn name(&self) -> &'static str {
        "Simulacion Estocastica"
    }

    fn category(&self) -> &'static str {
        "simulation"
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
