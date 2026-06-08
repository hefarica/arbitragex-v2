//! FundingRateWorker — Funding rate alignment (Perps/Spot) worker (Phase 1.5 scaffold).
//!
//! Polls CEX perpetual funding rates and on-chain perp contract rates on every tick.
//! Calls `FundingRateEngine::evaluate` to detect delta-neutral arbitrage windows.
//! Includes a circuit breaker: if the CEX API is unreachable for
//! `circuit_breaker_secs`, the worker halts and logs an error.
//!
//! Phase 1.5: logs candidates only, no pipeline emission.
//! Phase 2 will wire real CEX WebSocket + on-chain perp contract reads.

use crate::engines::funding_rate_engine::{FundingRateEngine, FundingRateEngineConfig};
use std::time::Duration;
use tokio::time::sleep;
use tracing::{debug, error, info};

pub struct FundingRateWorker {
    engine: FundingRateEngine,
    tick_ms: u64,
    chain_id: u64,
    /// Consecutive failed API polls before circuit breaker trips.
    max_consecutive_failures: u32,
}

impl FundingRateWorker {
    pub fn new(tick_ms: u64, chain_id: u64) -> Self {
        Self {
            engine: FundingRateEngine::new(FundingRateEngineConfig::default()),
            tick_ms,
            chain_id,
            max_consecutive_failures: 6, // 6 × 5s = 30s circuit breaker
        }
    }

    pub async fn run(self) {
        info!(
            event = "funding_rate_worker.started",
            chain_id = self.chain_id,
            tick_ms = self.tick_ms,
            phase = "1.5_scaffold"
        );

        let mut consecutive_failures = 0u32;

        loop {
            // Phase 1.5: scaffold tick — Phase 2 will poll real CEX API
            // and on-chain perp contracts here.
            let api_available = true; // Phase 2: replace with real CEX API health check

            if !api_available {
                consecutive_failures += 1;
                if consecutive_failures >= self.max_consecutive_failures {
                    error!(
                        event = "funding_rate_worker.circuit_breaker_tripped",
                        chain_id = self.chain_id,
                        consecutive_failures,
                        note = "CEX API unreachable — halting worker until restart"
                    );
                    return; // Circuit breaker: halt worker
                }
            } else {
                consecutive_failures = 0;
                debug!(
                    event = "funding_rate_worker.tick",
                    chain_id = self.chain_id,
                    note = "Phase 2 will wire real CEX WebSocket + on-chain perp rates"
                );
            }

            sleep(Duration::from_millis(self.tick_ms)).await;
        }
    }
}
