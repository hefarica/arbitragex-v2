//! Nonce manager.
//! In-memory `(chain_id, address) → next_nonce` with a Semaphore per-address
//! to serialize increments. On first use refreshes from RPC; on each submit,
//! increments locally. On nonce mismatch, caller triggers refresh.

use anyhow::{Context, Result};
use ethers::prelude::*;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;

pub struct NonceManager {
    provider: Arc<Provider<Http>>,
    state: Arc<Mutex<HashMap<(u64, Address), u64>>>,
}

impl NonceManager {
    pub fn new(provider: Arc<Provider<Http>>) -> Self {
        Self { provider, state: Arc::new(Mutex::new(HashMap::new())) }
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
    pub async fn refresh(&self, chain_id: u64, addr: Address) -> Result<u64> {
        let n = self.fetch(addr).await?;
        let mut g = self.state.lock().await;
        g.insert((chain_id, addr), n);
        Ok(n)
    }

    async fn fetch(&self, addr: Address) -> Result<u64> {
        let count = self.provider.get_transaction_count(addr, None)
            .await.context("eth_getTransactionCount")?;
        Ok(count.as_u64())
    }
}
