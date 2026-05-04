//! Worker orchestrator. Sub-proyecto-1 (Real Profit Signal): only RpcHealthWorker
//! and PoolSyncWorker are real. RouteDiscoveryWorker and SimulationWorker stubs
//! were deleted because they emitted fake telemetry without doing work.
//! HftMempoolListener and ExecutionWorker stubs are kept but not spawned.

pub mod execution_worker;
pub mod hft_mempool_listener;
pub mod pool_sync_worker;
pub mod rpc_health_worker;

use sqlx::PgPool;
use tracing::{error, info};

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

        let rpc_worker = rpc_health_worker::RpcHealthWorker::new(5000);
        tokio::spawn(async move {
            rpc_worker.start().await;
        });

        if let Some(db_pool) = db {
            let pool_worker = pool_sync_worker::PoolSyncWorker::new(5000, chain_id);
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
