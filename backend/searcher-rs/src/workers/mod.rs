//! Worker orchestrator. Sub-proyecto-1 (Real Profit Signal): only RpcHealthWorker
//! and PoolSyncWorker are real. RouteDiscoveryWorker and SimulationWorker stubs
//! were deleted because they emitted fake telemetry without doing work.
//! HftMempoolListener and ExecutionWorker stubs are kept but not spawned.

pub mod execution_worker;
pub mod flashloan_arb_worker;
pub mod heartbeat_worker;
pub mod hft_mempool_listener;
pub mod liquidation_worker;
pub mod pool_sync_worker;
pub mod price_worker;
pub mod rpc_health_worker;
pub mod triangular_worker;

use sqlx::PgPool;
use tracing::{error, info};

/// Default PoolSyncWorker tick — aligned to Ethereum block time (12s) so we
/// don't poll faster than chain state can change. Pre-2026-05-07 this was
/// hardcoded to 5000ms which paid for ~17,280 multicalls/day with no
/// additional information returned vs a 12s cadence (block-time bound).
/// Operators can override via `POOL_SYNC_INTERVAL_MS` for chains with faster
/// finality (e.g. L2s with sub-second blocks).
pub const DEFAULT_POOL_SYNC_INTERVAL_MS: u64 = 12_000;

/// Default RpcHealthWorker tick — health checks are cheap (1 eth_blockNumber
/// per RPC = ~10 CU) but constant. 5s is fine for fail-fast detection without
/// material CU cost. Override via `RPC_HEALTH_INTERVAL_MS`.
pub const DEFAULT_RPC_HEALTH_INTERVAL_MS: u64 = 5_000;

fn read_interval_ms_env(key: &str, default: u64) -> u64 {
    match std::env::var(key) {
        Ok(v) => v.trim().parse::<u64>().unwrap_or_else(|_| {
            tracing::warn!(
                event = "worker_orchestrator.interval_env_invalid",
                key,
                value = %v,
                default,
                "ignoring invalid env value, using default"
            );
            default
        }),
        Err(_) => default,
    }
}

pub struct WorkerOrchestrator {
    pub god_protocol_active: bool,
    pub kernel_bypass_enabled: bool,
}

impl WorkerOrchestrator {
    pub fn new(god_protocol_active: bool, kernel_bypass_enabled: bool) -> Self {
        Self {
            god_protocol_active,
            kernel_bypass_enabled,
        }
    }

    pub async fn start_all(
        &self,
        chain_id: u64,
        rpc_http_url: String,
        db: Option<PgPool>,
        redis: redis::aio::ConnectionManager,
    ) {
        info!(event = "worker_orchestrator.boot", chain_id, god_protocol = self.god_protocol_active, kernel_bypass = self.kernel_bypass_enabled);

        let rpc_health_ms = read_interval_ms_env("RPC_HEALTH_INTERVAL_MS", DEFAULT_RPC_HEALTH_INTERVAL_MS);
        let rpc_worker = rpc_health_worker::RpcHealthWorker::new(rpc_health_ms);
        tokio::spawn(async move {
            rpc_worker.start().await;
        });

        if let Some(db_pool) = db {
            let pool_sync_ms = read_interval_ms_env("POOL_SYNC_INTERVAL_MS", DEFAULT_POOL_SYNC_INTERVAL_MS);
            info!(
                event = "worker_orchestrator.pool_sync_interval",
                chain_id,
                interval_ms = pool_sync_ms,
                "PoolSyncWorker tick (block-time aligned by default)"
            );
            let pool_worker = pool_sync_worker::PoolSyncWorker::new(pool_sync_ms, chain_id);
            let redis_clone = redis.clone();
            tokio::spawn(async move {
                if let Err(e) = pool_worker.run(rpc_http_url, db_pool, redis_clone).await {
                    error!(event = "pool_sync.terminated", chain_id, error = %e);
                }
            });
            info!(event = "worker_orchestrator.pool_sync_started", chain_id);
        } else {
            info!(event = "worker_orchestrator.pool_sync_skipped", reason = "no_db_pool");
        }
    }
}
