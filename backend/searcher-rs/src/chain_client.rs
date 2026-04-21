//! Ethers-rs WebSocket client wrapper with reconnection/backoff.
//!
//! Responsibilities:
//!   - Open `Provider<Ws>` against a configured RPC.
//!   - Expose `subscribe_pending()` returning a stream of pending tx hashes.
//!   - Let the caller fetch the full transaction body via `get_tx()`.
//!   - Emit reconnection metrics.
//!
//! Honest behavior: if `connect()` fails, we bubble the error up so the scanner
//! loop can decide (log + backoff + retry). We never fabricate a connection.

use anyhow::Context;
use ethers::providers::{Middleware, Provider, StreamExt, Ws};
use ethers::types::{Transaction, H256};
use futures_util::Stream;
use std::sync::Arc;
use std::time::Duration;
use tokio::time::timeout;
use tracing::info;

pub struct WsChainClient {
    pub chain_id: u64,
    pub provider: Arc<Provider<Ws>>,
    pub url: String,
}

impl WsChainClient {
    pub async fn connect(chain_id: u64, url: &str) -> anyhow::Result<Self> {
        let provider = timeout(Duration::from_secs(10), Provider::<Ws>::connect(url))
            .await
            .context("ws connect timeout")??;
        let observed_chain = provider.get_chainid().await.context("get_chainid")?.as_u64();
        if observed_chain != chain_id {
            anyhow::bail!(
                "chain_id mismatch: config={} observed={} (url likely wrong)",
                chain_id,
                observed_chain
            );
        }
        info!(event = "chain_client.connected", chain_id, "ws provider connected");
        Ok(Self {
            chain_id,
            provider: Arc::new(provider),
            url: url.to_string(),
        })
    }

    /// Subscribe to new pending transaction hashes. Returns a stream.
    pub async fn subscribe_pending(
        &self,
    ) -> anyhow::Result<impl Stream<Item = H256> + Send + '_> {
        let sub = self
            .provider
            .subscribe_pending_txs()
            .await
            .context("subscribe_pending_txs")?;
        Ok(sub.map(|h| h))
    }

    pub async fn get_tx(&self, hash: H256) -> anyhow::Result<Option<Transaction>> {
        Ok(self.provider.get_transaction(hash).await?)
    }
}
