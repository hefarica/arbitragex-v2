//! RPC Health Worker — periodically pings each provider in the configured
//! `HttpRpcPool` with `eth_blockNumber` to refresh latency, last-block, and
//! `ProviderState` (Healthy / Degraded / Open). Provides observability into
//! RPC vendor liveness without depending on production traffic to surface
//! degraded providers.
//!
//! No fabricated data: if `RPC_HTTP_<chain_id>` is not set, the worker logs
//! once at boot and idles (RULE 00). Pre-2026-05-07 this file shipped with a
//! `mock_rpcs` constant — that violation was deleted as part of the same
//! sweep that removed the `expected_profit_usd` random injection.

use alloy::providers::Provider as AlloyProvider;
use shared_rs::{HttpRpcPool, ProviderState};
use std::time::{Duration, Instant};
use tokio::time::{sleep, timeout};
use tracing::{info, warn};

/// Latency above this threshold flips an entry to `Degraded`. Below it (and
/// no recent error) flips back to `Healthy`. Open state is set by the call
/// path's circuit breaker, not here — this worker only writes Healthy /
/// Degraded based on observed latency.
const DEGRADED_LATENCY_MS: u64 = 500;
/// Per-ping timeout. Anything beyond this is treated as a soft failure (the
/// entry stays in its previous state, we just don't update latency this tick).
const PING_TIMEOUT_MS: u64 = 2_000;
/// Exponential moving average factor for latency smoothing.
/// `ewma_new = α·sample + (1-α)·ewma_old`.
const LATENCY_EWMA_ALPHA: f64 = 0.3;

// TODO(BE-02 follow-up): this worker duplicates HttpRpcPool::spawn_health_loop now that
// the pool is retained as Arc<HttpRpcPool> in main.rs (BE-02 Step 2). The pool's own
// health loop already runs EWMA + drift + circuit rotation on HEALTH_CHECK_INTERVAL (15s).
// This worker runs an independent eth_blockNumber ping on its own interval (default 5s).
// Consider removing this worker once Step 2 is validated in production to avoid
// double-counting CU usage against the RPC provider.

pub struct RpcHealthWorker {
    pub check_interval: Duration,
}

impl RpcHealthWorker {
    pub fn new(check_interval_ms: u64) -> Self {
        Self {
            check_interval: Duration::from_millis(check_interval_ms),
        }
    }

    /// Reads `ARBX_PRIMARY_CHAIN_ID` (default 1) and pings the corresponding
    /// `RPC_HTTP_<chain_id>` pool every `check_interval`. If the pool is not
    /// configured, the worker idles with a periodic warn log so the operator
    /// notices misconfiguration.
    pub async fn start(&self) {
        let chain_id: u64 = std::env::var("ARBX_PRIMARY_CHAIN_ID")
            .ok()
            .and_then(|v| v.trim().parse::<u64>().ok())
            .unwrap_or(1);

        info!(
            event = "rpc_health.boot",
            chain_id, "rpc health worker starting"
        );

        let pool = match HttpRpcPool::from_env(chain_id).await {
            Ok(Some(p)) => p,
            Ok(None) => {
                warn!(
                    event = "rpc_health.no_pool",
                    chain_id,
                    env_key = format!("RPC_HTTP_{chain_id}"),
                    "RPC_HTTP not configured; rpc health worker will idle"
                );
                self.idle_loop().await;
                return;
            }
            Err(e) => {
                warn!(
                    event = "rpc_health.pool_invalid",
                    chain_id,
                    error = %e,
                    "RPC pool failed to build; rpc health worker will idle"
                );
                self.idle_loop().await;
                return;
            }
        };

        info!(
            event = "rpc_health.pool_ready",
            chain_id,
            providers = pool.entries.len(),
            "monitoring RPC providers"
        );

        loop {
            for entry in &pool.entries {
                let started = Instant::now();
                let res = timeout(
                    Duration::from_millis(PING_TIMEOUT_MS),
                    entry.provider.get_block_number(),
                )
                .await;

                match res {
                    Ok(Ok(block)) => {
                        // alloy 1.0: get_block_number() returns u64 directly.
                        let block: u64 = block;
                        let latency_ms = started.elapsed().as_millis() as u64;
                        // EWMA update — read current, blend, store.
                        let prev_ewma = entry.snapshot_latency_ms();
                        let new_ewma = if prev_ewma == 0 {
                            latency_ms
                        } else {
                            ((LATENCY_EWMA_ALPHA * latency_ms as f64)
                                + ((1.0 - LATENCY_EWMA_ALPHA) * prev_ewma as f64))
                                .round() as u64
                        };
                        entry
                            .latency_ms_ewma
                            .store(new_ewma, std::sync::atomic::Ordering::Relaxed);
                        entry
                            .last_block
                            .store(block, std::sync::atomic::Ordering::Relaxed);

                        // Update state: Degraded if EWMA above threshold, else Healthy.
                        // Open state is reserved for the circuit breaker — we don't
                        // unilaterally close it from here.
                        let prev = entry.snapshot_state();
                        if prev != ProviderState::Open {
                            let target = if new_ewma > DEGRADED_LATENCY_MS {
                                ProviderState::Degraded
                            } else {
                                ProviderState::Healthy
                            };
                            if target != prev {
                                entry.set_state(target);
                                info!(
                                    event = "rpc_health.state_change",
                                    name = %entry.name,
                                    chain_id,
                                    prev = %prev.as_str(),
                                    new = %target.as_str(),
                                    latency_ewma_ms = new_ewma,
                                );
                            }
                        }

                        info!(
                            event = "rpc_health.ping",
                            name = %entry.name,
                            chain_id,
                            block,
                            latency_ms,
                            latency_ewma_ms = new_ewma,
                            state = %entry.snapshot_state().as_str(),
                        );
                    }
                    Ok(Err(e)) => {
                        warn!(
                            event = "rpc_health.ping_err",
                            name = %entry.name,
                            chain_id,
                            error = %e,
                        );
                    }
                    Err(_) => {
                        warn!(
                            event = "rpc_health.ping_timeout",
                            name = %entry.name,
                            chain_id,
                            timeout_ms = PING_TIMEOUT_MS,
                        );
                    }
                }
            }
            sleep(self.check_interval).await;
        }
    }

    async fn idle_loop(&self) {
        let mut ticker = tokio::time::interval(Duration::from_secs(60));
        loop {
            ticker.tick().await;
            warn!(
                event = "rpc_health.idle",
                "rpc health worker idle — no RPC pool configured"
            );
        }
    }
}
