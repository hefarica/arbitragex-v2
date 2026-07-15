use async_trait::async_trait;
use crate::engines::dex_engine::DexEngine;
use crate::thermodynamics::potential_field::PotentialField;
use crate::thermodynamics::{PermittedCycle, PotentialGradient};
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
    async fn sample(&self, token_in: &str, token_out: &str) -> Option<PotentialGradient> {
        // TODO(scaffold): wire to DexEngine quote path
        Some(PotentialGradient {
            token_in: token_in.to_string(),
            token_out: token_out.to_string(),
            potential_delta_usd: 0.0,
            venue_in: "uniswap_v3".to_string(),
            venue_out: "uniswap_v3".to_string(),
        })
    }

    async fn reconcile(&self, _cycle: &PermittedCycle) {
        // TODO(scaffold): compare expected vs actual DEX quote
    }
}
