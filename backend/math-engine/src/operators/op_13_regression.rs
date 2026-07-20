//! FUSILE: Implementacion propia -- Regresion Lineal/Logistica
//! Categoria: inference

use super::{MarketState, OperatorOutput, TopologicalOperator};
use std::collections::HashMap;

#[derive(Default)]
pub struct RegressionOperator;

impl RegressionOperator {
    pub fn new() -> Self {
        Self
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
