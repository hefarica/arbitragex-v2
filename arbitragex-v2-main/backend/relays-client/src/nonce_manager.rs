//! Nonce manager.
//! In-memory `(chain_id, address) → next_nonce` with a Semaphore per-address
//! to serialize increments. On first use refreshes from RPC via the pool's
//! `with_retry` — so circuit breaker + EWMA failover fire on RPC errors.
//! On each submit, increments locally. On nonce mismatch, caller triggers
//! refresh.
//!
//! NOTE: `with_retry` retries on a *different* provider on the first failure.
//! For `eth_getTransactionCount` this is safe: the returned value is the
//! on-chain pending nonce, identical regardless of which healthy provider
//! responds. The local cache then takes over for all subsequent increments
//! within the same epoch, so no second RPC call fires until a refresh.

use alloy::primitives::Address as AlloyAddress;
use alloy::providers::Provider as AlloyProvider;
use anyhow::{Context, Result};
use ethers::types::Address;
use shared_rs::rpc_failover::HttpRpcPool;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;

pub struct NonceManager {
    /// Shared pool — routes `eth_getTransactionCount` through `with_retry` so
    /// the circuit breaker and EWMA-ranked failover activate on RPC errors.
    pool: Arc<HttpRpcPool>,
    state: Arc<Mutex<HashMap<(u64, Address), u64>>>,
}

impl NonceManager {
    pub fn new(pool: Arc<HttpRpcPool>) -> Self {
        Self {
            pool,
            state: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Returns the next nonce for this (chain, address) and increments the local counter.
    pub async fn next(&self, chain_id: u64, addr: Address) -> Result<u64> {
        let mut g = self.state.lock().await;
        let key = (chain_id, addr);
        let cur = match g.get(&key) {
            Some(&n) => n,
            None => {
                let n = self.fetch(addr).await?;
                g.insert(key, n);
                n
            }
        };
        g.insert(key, cur + 1);
        Ok(cur)
    }

    /// Force-refresh from RPC. Call on nonce-mismatch error.
    #[allow(dead_code)]
    pub async fn refresh(&self, chain_id: u64, addr: Address) -> Result<u64> {
        let n = self.fetch(addr).await?;
        let mut g = self.state.lock().await;
        g.insert((chain_id, addr), n);
        Ok(n)
    }

    /// Fetch the on-chain pending nonce via `pool.with_retry`.
    ///
    /// Retry semantics: on the first provider failure the pool tries the next
    /// best EWMA-ranked provider. If both fail it returns `PoolError::AllUnhealthy`
    /// which propagates as an `anyhow::Error` to the caller.
    ///
    /// Alloy 1.0: `get_transaction_count(address)` returns `u64` directly (no
    /// `.as_u64()` needed). The ethers `H160` address is converted to alloy
    /// `Address` via `from_slice` on the raw bytes — both types are `[u8; 20]`.
    async fn fetch(&self, addr: Address) -> Result<u64> {
        let alloy_addr = AlloyAddress::from_slice(addr.as_bytes());
        let count = self
            .pool
            .with_retry(|provider| async move {
                provider
                    .get_transaction_count(alloy_addr)
                    .await
                    .map_err(|e| anyhow::anyhow!("eth_getTransactionCount: {e}"))
            })
            .await
            .context("nonce_manager.fetch via rpc_pool")?;
        // alloy 1.0 get_transaction_count returns u64 directly.
        Ok(count)
    }
}
