//! Worker orchestrator. Sub-proyecto-1 (Real Profit Signal): only RpcHealthWorker
//! and PoolSyncWorker are real. RouteDiscoveryWorker and SimulationWorker stubs
//! were deleted because they emitted fake telemetry without doing work.
//! HftMempoolListener and ExecutionWorker stubs are kept but not spawned.

pub mod cex_dex_worker;
pub mod execution_worker;
pub mod flashloan_arb_worker;
pub mod gas_oracle_worker;
pub mod heartbeat_worker;
pub mod hft_mempool_listener;
pub mod jit_v3_worker;
pub mod liquidation_worker;
pub mod pool_sync_worker;
pub mod price_worker;
pub mod rpc_health_worker;
pub mod triangular_worker;

use shared_rs::rpc_failover::HttpRpcPool;
use sqlx::PgPool;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::task::JoinHandle;
use tracing::{error, info, warn};

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

    /// Spawn per-chain worker sets for every chain_id in `chain_ids`.
    ///
    /// Each chain gets an independent set of orchestrated workers (RpcHealth,
    /// GasOracle, PoolSync) backed by its own `HttpRpcPool` entry from
    /// `rpc_pools`. Chains without a pool entry are skipped with a warn log
    /// (R8 fail-honest: no RPC configured → workers don't start, heartbeat
    /// surfaces the gap rather than hiding it with fake telemetry).
    ///
    /// Returns the `JoinHandle`s of the per-chain spawned tasks so the caller
    /// can optionally join/cancel them. Callers that don't need lifecycle control
    /// may discard the return value.
    pub async fn start_all_multichain(
        &self,
        chain_ids: Vec<u64>,
        rpc_pools: &HashMap<u64, Arc<HttpRpcPool>>,
        db: Option<PgPool>,
        redis: redis::aio::ConnectionManager,
    ) -> Vec<JoinHandle<()>> {
        let mut handles = Vec::with_capacity(chain_ids.len());
        for chain_id in chain_ids {
            let pool = rpc_pools.get(&chain_id).cloned();
            if pool.is_none() {
                warn!(
                    event = "worker_orchestrator.chain_skipped",
                    chain_id,
                    reason = "no_rpc_pool",
                    "RPC_HTTP_{chain_id} not configured — workers for this chain will not start (R8 fail-honest)"
                );
            }
            // WorkerOrchestrator carries only two bool flags — safe to clone
            // the config values for each spawned task.
            let god = self.god_protocol_active;
            let kernel = self.kernel_bypass_enabled;
            let db_c = db.clone();
            let redis_c = redis.clone();
            let handle = tokio::spawn(async move {
                WorkerOrchestrator::new(god, kernel)
                    .start_all(chain_id, pool, db_c, redis_c)
                    .await;
            });
            handles.push(handle);
        }
        handles
    }

    pub async fn start_all(
        &self,
        chain_id: u64,
        rpc_pool: Option<Arc<HttpRpcPool>>,
        db: Option<PgPool>,
        redis: redis::aio::ConnectionManager,
    ) {
        info!(event = "worker_orchestrator.boot", chain_id, god_protocol = self.god_protocol_active, kernel_bypass = self.kernel_bypass_enabled);

        let rpc_health_ms = read_interval_ms_env("RPC_HEALTH_INTERVAL_MS", DEFAULT_RPC_HEALTH_INTERVAL_MS);
        let rpc_worker = rpc_health_worker::RpcHealthWorker::new(rpc_health_ms);
        tokio::spawn(async move {
            rpc_worker.start().await;
        });

        // Gas oracle worker — CODE-3: stamps arbx:gas_price_ts:<chain_id> so the
        // Pre-Execute Checklist Check 6 (gas_price_fresh) unblocks broadcast when
        // paper_mode=false. The key is a LIVENESS signal: absent / stale key means
        // the gas oracle died and the checklist correctly blocks execution (fail-closed).
        let gas_oracle_ms = read_interval_ms_env(
            "GAS_ORACLE_INTERVAL_MS",
            gas_oracle_worker::DEFAULT_INTERVAL_MS,
        );
        if let Some(ref gas_rpc_pool) = rpc_pool {
            let gas_pool = Arc::clone(gas_rpc_pool);
            let gas_redis = redis.clone();
            let gas_chain = chain_id;
            tokio::spawn(async move {
                gas_oracle_worker::GasOracleWorker::new(gas_oracle_ms, gas_chain)
                    .run(gas_pool, gas_redis)
                    .await;
            });
            info!(event = "worker_orchestrator.gas_oracle_started", chain_id, interval_ms = gas_oracle_ms);
        } else {
            info!(
                event = "worker_orchestrator.gas_oracle_skipped",
                chain_id,
                reason = "no_rpc_pool",
                "gas oracle not started — Check 6 will block until RPC pool is configured"
            );
        }

        if let (Some(db_pool), Some(pool)) = (db, rpc_pool) {
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
                if let Err(e) = pool_worker.run(pool, db_pool, redis_clone).await {
                    error!(event = "pool_sync.terminated", chain_id, error = %e);
                }
            });
            info!(event = "worker_orchestrator.pool_sync_started", chain_id);
        } else {
            info!(event = "worker_orchestrator.pool_sync_skipped", reason = "no_db_pool_or_rpc");
        }
    }
}
