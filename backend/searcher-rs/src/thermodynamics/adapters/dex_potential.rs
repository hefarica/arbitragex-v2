use crate::engines::dex_engine::DexEngine;
use crate::thermodynamics::potential_field::PotentialField;
use crate::thermodynamics::{PermittedCycle, PotentialGradient};
use async_trait::async_trait;
use std::sync::Arc;

pub struct DexPotentialAdapter {
    #[allow(dead_code)]
    engine: Arc<DexEngine>,
}

impl DexPotentialAdapter {
    pub fn new(engine: Arc<DexEngine>) -> Self {
        Self { engine }
    }
}

#[async_trait]
impl PotentialField for DexPotentialAdapter {
    async fn sample(&self, _token_in: &str, _token_out: &str) -> Option<PotentialGradient> {
        // TODO(scaffold): wire to DexEngine quote path
        // R8 (STATIC-DEBT 2026-08-31): a fabricated Some(0.0) claims "computed
        // and exactly zero" — a lie while unwired. None = not computed. The
        // sibling adapters (liquidation_potential) already did this correctly.
        None
    }

    async fn reconcile(&self, _cycle: &PermittedCycle) {
        // TODO(scaffold): compare expected vs actual DEX quote
    }
}
