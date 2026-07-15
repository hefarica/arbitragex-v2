use async_trait::async_trait;
use crate::engines::liquidation_engine::LiquidationEngine;
use crate::thermodynamics::potential_field::PotentialField;
use crate::thermodynamics::{PermittedCycle, PotentialGradient};
use std::sync::Arc;

pub struct LiquidationPotentialAdapter {
    #[allow(dead_code)]
    engine: Arc<LiquidationEngine>,
}

impl LiquidationPotentialAdapter {
    pub fn new(engine: Arc<LiquidationEngine>) -> Self {
        Self { engine }
    }
}

#[async_trait]
impl PotentialField for LiquidationPotentialAdapter {
    async fn sample(&self, _token_in: &str, _token_out: &str) -> Option<PotentialGradient> {
        // TODO(scaffold): wire to LiquidationEngine position scanner
        None
    }

    async fn reconcile(&self, _cycle: &PermittedCycle) {}
}
