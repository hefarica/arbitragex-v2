//! FUSILE: Implementacion propia -- Cadenas de Markov
//! Categoria: stochastic

use super::{MarketState, OperatorOutput, TopologicalOperator};
use std::collections::HashMap;

#[derive(Default)]
pub struct MarkovChainOperator;

impl MarkovChainOperator {
    pub fn new() -> Self {
        Self
    }
}

impl TopologicalOperator for MarkovChainOperator {
    fn id(&self) -> u8 {
        6
    }

    fn name(&self) -> &'static str {
        "Cadenas de Markov"
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
            scalar_value: None, // fail-honest: stub does not compute (was a fabricated Some(0.0))
            vector_result: None,
            matrix_result: None,
            metadata,
        }
    }
}
