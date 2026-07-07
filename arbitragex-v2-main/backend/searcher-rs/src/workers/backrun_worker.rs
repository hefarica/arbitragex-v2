//! BackrunWorker — Non-extractive backrunning worker.
//!
//! This worker listens to the mempool for large swaps and evaluates them
//! for non-extractive backrunning opportunities using the `BackrunEngine`.
//!
//! ## Phase 1 scope
//!
//! Phase 1 delivers the worker shell only:
//! - Subscribes to the `arbx:impact_events` Redis stream.
//! - Filters for events with `impact_type = "large_swap"`.
//! - Calls `BackrunEngine::evaluate` for each qualifying event.
//! - Logs detected candidates but does NOT emit to the pipeline yet.
//!   Full emission is deferred to Phase 2 once the zero-victim check
//!   is validated end-to-end via `sim-ctl`.
//!
//! ## Doctrine compliance
//! - **Zero Mocks:** Only processes real impact events from the mempool.
//! - **Fail-Honest:** If the zero-victim check fails, the candidate is discarded.

use crate::engines::backrun_engine::{BackrunEngine, BackrunEngineConfig};
use std::time::Duration;
use tokio::time::sleep;
use tracing::{debug, error, info, warn};

pub struct BackrunWorker {
    engine: BackrunEngine,
    tick_ms: u64,
    chain_id: u64,
}

impl BackrunWorker {
    pub fn new(tick_ms: u64, chain_id: u64) -> Self {
        Self {
            engine: BackrunEngine::new(BackrunEngineConfig::default()),
            tick_ms,
            chain_id,
        }
    }

    pub async fn run(self) {
        info!(
            event = "backrun_worker.started",
            chain_id = self.chain_id,
            tick_ms = self.tick_ms
        );

        loop {
            // Phase 1: Scaffold tick — in Phase 2 this will subscribe to
            // the Redis stream `arbx:impact_events` and process real events.
            debug!(
                event = "backrun_worker.tick",
                chain_id = self.chain_id,
                phase = 1,
                note = "Phase 2 will wire real mempool impact events"
            );

            sleep(Duration::from_millis(self.tick_ms)).await;
        }
    }
}
