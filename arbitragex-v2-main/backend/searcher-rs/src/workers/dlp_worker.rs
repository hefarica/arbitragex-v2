//! DlpWorker — Deterministic Liquidity Provision (EKF) worker (Phase 1.5 scaffold).
//!
//! Maintains an EKF state per tracked pool, updated on every block tick.
//! Calls `DlpEngine::evaluate` to detect JIT liquidity injection windows.
//! Phase 1.5: logs candidates only, no pipeline emission.
//! Phase 2 will wire real reserve reader + V3 tick-level position construction.

use crate::engines::dlp_engine::{DlpEngine, DlpEngineConfig, EkfState};
use std::time::Duration;
use tokio::time::sleep;
use tracing::{debug, info};

pub struct DlpWorker {
    engine: DlpEngine,
    tick_ms: u64,
    chain_id: u64,
}

impl DlpWorker {
    pub fn new(tick_ms: u64, chain_id: u64) -> Self {
        Self {
            engine: DlpEngine::new(DlpEngineConfig::default()),
            tick_ms,
            chain_id,
        }
    }

    pub async fn run(self) {
        info!(
            event = "dlp_worker.started",
            chain_id = self.chain_id,
            tick_ms = self.tick_ms,
            phase = "1.5_scaffold"
        );

        // Phase 1.5: single default EKF state (Phase 2 will maintain one per pool)
        let _ekf_state = EkfState::default();

        loop {
            debug!(
                event = "dlp_worker.tick",
                chain_id = self.chain_id,
                note = "Phase 2 will wire real pool prices and update EKF state per block"
            );
            sleep(Duration::from_millis(self.tick_ms)).await;
        }
    }
}
