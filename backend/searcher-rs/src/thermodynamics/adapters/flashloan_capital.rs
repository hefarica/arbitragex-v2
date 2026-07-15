use crate::engines::flashloan_engine::FlashloanEngine;
use crate::thermodynamics::capital_quantum::CapitalQuantum;
use rust_decimal::Decimal;
use std::sync::Arc;

pub struct FlashloanCapitalAdapter {
    #[allow(dead_code)]
    engine: Arc<FlashloanEngine>,
}

impl FlashloanCapitalAdapter {
    pub fn new(engine: Arc<FlashloanEngine>) -> Self {
        Self { engine }
    }
}

impl CapitalQuantum for FlashloanCapitalAdapter {
    fn notional(&self) -> Decimal {
        Decimal::ONE
    }

    fn token(&self) -> &str {
        "WETH"
    }

    fn venue(&self) -> &str {
        "aave_v3"
    }
}
