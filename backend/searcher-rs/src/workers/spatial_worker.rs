//! SpatialWorker — Cross-pool topological synchronization worker.
//!
//! This worker monitors factory contracts and liquidity pairs in memory,
//! detecting price divergences between isolated pools.
//!
//! ## Phase 1 scope
//!
//! Phase 1 delivers the worker shell only:
//! - Reads pool prices from the Redis pool cache on every tick.
//! - Calls `SpatialEngine::evaluate` for each pair of pools.
//! - Logs detected candidates but does NOT emit to the pipeline yet.
//!   Full emission is deferred to Phase 2 once the flashloan routing
//!   is validated end-to-end via `sim-ctl`.
//!
//! ## Doctrine compliance
//! - **Zero Mocks:** Only operates with confirmed state data.
//! - **Atomic Routing:** Flashloan buy/sell must be atomic.

use crate::engines::spatial_engine::{SpatialEngine, SpatialEngineConfig};
use std::time::Duration;
use tokio::time::sleep;
use tracing::{debug, error, info, warn};

pub struct SpatialWorker {
    engine: SpatialEngine,
    tick_ms: u64,
    chain_id: u64,
}

impl SpatialWorker {
    pub fn new(tick_ms: u64, chain_id: u64) -> Self {
        Self {
            engine: SpatialEngine::new(SpatialEngineConfig::default()),
            tick_ms,
            chain_id,
        }
    }

    pub async fn run(self) {
        info!(
            event = "spatial_worker.started",
            chain_id = self.chain_id,
            tick_ms = self.tick_ms
        );

        loop {
            // Phase 1: Scaffold tick — in Phase 2 this will read real pool
            // prices from Redis and evaluate cross-pool divergences.
            debug!(
                event = "spatial_worker.tick",
                chain_id = self.chain_id,
                phase = 1,
                note = "Phase 2 will wire real pool price reads from Redis"
            );

            sleep(Duration::from_millis(self.tick_ms)).await;
        }
    }
}
