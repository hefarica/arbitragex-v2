//! `AnvilBackend` — wraps the existing `SimEngine` behind `SimulatorBackend`.
//!
//! This is a zero-cost adapter: it delegates every call to `SimEngine::simulate`
//! unchanged. Existing Anvil behaviour is fully preserved.

use async_trait::async_trait;
use shared_rs::contracts::{Opportunity, SimulationResult};
use std::sync::Arc;

use crate::sim_engine::SimEngine;
use crate::simulator_backend::{SimulationError, SimulatorBackend};

/// Anvil-based simulator. Delegates to `SimEngine` which performs
/// snapshot → eth_call → revert against an Anvil fork.
pub struct AnvilBackend {
    pub engine: Arc<SimEngine>,
}

impl AnvilBackend {
    pub fn new(engine: Arc<SimEngine>) -> Self {
        Self { engine }
    }
}

#[async_trait]
impl SimulatorBackend for AnvilBackend {
    async fn simulate(&self, opp: &Opportunity) -> Result<SimulationResult, SimulationError> {
        // SimEngine::simulate never panics — returns SimulationResult on all paths.
        Ok(self.engine.simulate(opp).await)
    }

    fn name(&self) -> &str {
        "anvil"
    }
}
