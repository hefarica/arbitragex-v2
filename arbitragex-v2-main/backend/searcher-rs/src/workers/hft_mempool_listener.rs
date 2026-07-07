//! HFT Mempool Listener — Skill 082: TCP Kernel Bypassing
//!
//! Listens to pending mempool transactions for arbitrage opportunity detection.
//! In production this would use io_uring / XDP for zero-copy packet capture.
//! Currently a stub that spawns a placeholder task.

use tracing::info;

#[allow(dead_code)]
pub struct HftMempoolListener {
    pub enabled: bool,
}

#[allow(dead_code)]
impl HftMempoolListener {
    pub fn new(enabled: bool) -> Self {
        Self { enabled }
    }

    pub async fn start(&self) {
        if !self.enabled {
            info!("[HftMempoolListener] Kernel-bypass mode disabled — skipping.");
            return;
        }
        info!("[HftMempoolListener] Placeholder — real implementation requires XDP/io_uring.");
    }
}
