//! SvsWorker — Stochastic Volatility Stabilization worker (Phase 1.5 scaffold).
//!
//! Reads pool price history from Redis on every tick, detects macro volatility
//! shocks, and calls `SvsEngine::evaluate`. Phase 1.5: logs candidates only,
//! no pipeline emission. Phase 2 will wire real reserve reader + emission.

use crate::engines::svs_engine::{SvsEngine, SvsEngineConfig};
use std::time::Duration;
use tokio::time::sleep;
use tracing::{debug, info};

pub struct SvsWorker {
    engine: SvsEngine,
    tick_ms: u64,
    chain_id: u64,
}

impl SvsWorker {
    pub fn new(tick_ms: u64, chain_id: u64) -> Self {
        Self {
            engine: SvsEngine::new(SvsEngineConfig::default()),
            tick_ms,
            chain_id,
        }
    }

    pub async fn run(self) {
        info!(
            event = "svs_worker.started",
            chain_id = self.chain_id,
            tick_ms = self.tick_ms,
            phase = "1.5_scaffold"
        );

        loop {
            debug!(
                event = "svs_worker.tick",
                chain_id = self.chain_id,
                note = "Phase 2 will wire real volatility signals from reserve reader"
            );
            sleep(Duration::from_millis(self.tick_ms)).await;
        }
    }
}
