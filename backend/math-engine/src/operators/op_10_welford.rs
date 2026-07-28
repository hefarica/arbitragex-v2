//! FUSILE: Implementacion propia -- Varianza Online
//! Categoria: stochastic

use super::{MarketState, OperatorOutput, TopologicalOperator};
use std::collections::HashMap;

#[derive(Default)]
pub struct WelfordOperator;

impl WelfordOperator {
    pub fn new() -> Self {
        Self
    }
}

impl TopologicalOperator for WelfordOperator {
    fn id(&self) -> u8 {
        10
    }

    fn name(&self) -> &'static str {
        "Varianza Online"
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
