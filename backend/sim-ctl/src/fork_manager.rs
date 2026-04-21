//! Fork manager — wraps the Anvil RPC endpoint and implements a
//! snapshot/revert pool so concurrent simulations don't contaminate state.
//!
//! Honest behavior:
//! - If ANVIL_URL is unset or unreachable at `connect()`, returns Err.
//! - Caller decides what to do (sim-ctl main keeps HTTP alive, /simulate => 501).
//! - evm_snapshot / evm_revert: native anvil JSON-RPC; deterministic reset.
//! - anvil_reset: refreshes the fork to current head; called periodically.

use anyhow::{Context, Result};
use ethers::prelude::*;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{Mutex, Semaphore};

#[derive(Clone)]
pub struct ForkManager {
    pub provider: Arc<Provider<Http>>,
    /// Serialize snapshot/revert operations so ids don't collide.
    pool: Arc<Semaphore>,
    /// Guarded so reset doesn't race with active sims.
    active: Arc<Mutex<()>>,
}

impl ForkManager {
    pub async fn connect(anvil_url: &str, pool_size: u32, connect_timeout_ms: u64) -> Result<Self> {
        let provider = tokio::time::timeout(
            Duration::from_millis(connect_timeout_ms),
            async {
                let p = Provider::<Http>::try_from(anvil_url)
                    .context("anvil url parse")?;
                // sanity: block number responds
                let _bn = p.get_block_number().await.context("eth_blockNumber")?;
                Ok::<_, anyhow::Error>(p)
            },
        )
        .await
        .context("anvil connect timeout")??;

        Ok(Self {
            provider: Arc::new(provider),
            pool: Arc::new(Semaphore::new(pool_size as usize)),
            active: Arc::new(Mutex::new(())),
        })
    }

    /// Acquire a snapshot handle. The caller MUST release() it to revert state.
    pub async fn acquire(&self) -> Result<SnapshotHandle> {
        let permit = self.pool.clone().acquire_owned().await
            .context("acquire semaphore")?;
        let id_hex: String = self.provider
            .request("evm_snapshot", ())
            .await
            .context("evm_snapshot")?;
        Ok(SnapshotHandle { id: id_hex, _permit: permit, provider: self.provider.clone() })
    }

    pub async fn anvil_reset(&self, fork_url: &str) -> Result<()> {
        let _guard = self.active.lock().await;
        let forking = serde_json::json!({ "forking": { "jsonRpcUrl": fork_url }});
        let _: serde_json::Value = self.provider
            .request("anvil_reset", [forking])
            .await
            .context("anvil_reset")?;
        Ok(())
    }

    pub async fn current_block(&self) -> Result<u64> {
        Ok(self.provider.get_block_number().await?.as_u64())
    }
}

pub struct SnapshotHandle {
    pub id: String,
    _permit: tokio::sync::OwnedSemaphorePermit,
    provider: Arc<Provider<Http>>,
}

impl SnapshotHandle {
    /// Revert the fork to the snapshot state. Always called by sim_engine.
    pub async fn release(self) -> Result<()> {
        let _: bool = self.provider
            .request("evm_revert", [self.id.clone()])
            .await
            .context("evm_revert")?;
        Ok(())
    }
}
