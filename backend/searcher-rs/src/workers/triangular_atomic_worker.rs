//! TriangularAtomicWorker — Atomic triangular decomposition worker (Phase 1.5 scaffold).
//!
//! Complements the existing `TriangularWorker` (Phase 9) with the APEX variant:
//! single-manifold atomic bundle construction for A→B→C→A cycles.
//! Phase 1.5: logs candidates only, no pipeline emission.
//! Phase 2 will wire real reserve reads and atomic calldata encoding.

use crate::engines::triangular_atomic_engine::{
    TriangularAtomicEngine, TriangularAtomicEngineConfig,
};
use std::time::Duration;
use tokio::time::sleep;
use tracing::{debug, info};

pub struct TriangularAtomicWorker {
    engine: TriangularAtomicEngine,
    tick_ms: u64,
    chain_id: u64,
}

impl TriangularAtomicWorker {
    pub fn new(tick_ms: u64, chain_id: u64) -> Self {
        Self {
            engine: TriangularAtomicEngine::new(TriangularAtomicEngineConfig::default()),
            tick_ms,
            chain_id,
        }
    }

    pub async fn run(self) {
        info!(
            event = "triangular_atomic_worker.started",
            chain_id = self.chain_id,
            tick_ms = self.tick_ms,
            phase = "1.5_scaffold"
        );

        loop {
            debug!(
                event = "triangular_atomic_worker.tick",
                chain_id = self.chain_id,
                note = "Phase 2 will wire real reserve reads and single-manifold cycle evaluation"
            );
            sleep(Duration::from_millis(self.tick_ms)).await;
        }
    }
}
