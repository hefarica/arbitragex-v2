//! FUSILE: Implementacion propia -- Liquidez Justo a Tiempo
//! Categoria: atomic

use super::{MarketState, OperatorOutput, TopologicalOperator};
use std::collections::HashMap;

pub struct JitLiquidityOperator;

impl JitLiquidityOperator {
    pub fn new() -> Self {
        Self
    }
}

impl TopologicalOperator for JitLiquidityOperator {
    fn id(&self) -> u8 {
        28
    }

    fn name(&self) -> &'static str {
        "Liquidez Justo a Tiempo"
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
