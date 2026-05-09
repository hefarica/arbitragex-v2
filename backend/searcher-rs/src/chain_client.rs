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
use ethers::providers::{Middleware, Provider, StreamExt, SubscriptionStream, Ws};
use ethers::types::{Transaction, H256};
use futures_util::Stream;
use serde_json::{json, Value};
use std::sync::Arc;
use std::time::Duration;
use tokio::time::timeout;
use tracing::{info, warn};

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
    ///
    /// FIREHOSE — every pending tx in the upstream's view. On Alchemy this
    /// costs ~10 CU per delivered hash plus a follow-up `eth_getTransactionByHash`
    /// (26 CU) to fetch the body. Prefer `subscribe_pending_filtered_txs` when
    /// the upstream is Alchemy and we already know the routers we care about.
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

    /// Subscribe to pending transactions filtered upstream by `to` address,
    /// using Alchemy's `alchemy_pendingTransactions` topic. The relay drops
    /// every tx whose `to` is not in `to_addresses` *before* delivery, so we
    /// pay no CU for the rejects. Each delivered event includes the full
    /// `Transaction` body, which removes the follow-up `eth_getTransactionByHash`
    /// round-trip the firehose path needs.
    pub async fn subscribe_pending_filtered_txs(
        &self,
        to_addresses: &[String],
    ) -> anyhow::Result<SubscriptionStream<'_, Ws, Transaction>> {
        if to_addresses.is_empty() {
            anyhow::bail!("subscribe_pending_filtered_txs: empty allowlist would unfilter the stream");
        }
        let params: Value = json!([
            "alchemy_pendingTransactions",
            {
                "toAddress": to_addresses,
                "hashesOnly": false
            }
        ]);
        let sub: SubscriptionStream<'_, Ws, Transaction> = self
            .provider
            .subscribe(params)
            .await
            .context("subscribe alchemy_pendingTransactions")?;
        info!(
            event = "chain_client.subscribed_filtered",
            chain_id = self.chain_id,
            allowlist_size = to_addresses.len(),
            "filtered mempool subscription active"
        );
        Ok(sub)
    }

    pub async fn get_tx(&self, hash: H256) -> anyhow::Result<Option<Transaction>> {
        Ok(self.provider.get_transaction(hash).await?)
    }
}

/// Heuristic: does the WS URL belong to Alchemy? Used by the scanner to decide
/// between the filtered (`alchemy_pendingTransactions`) and the firehose
/// (`newPendingTransactions`) code paths. We don't probe — wrong guess just
/// triggers a fallback to firehose with a warn log.
pub fn is_alchemy_endpoint(url: &str) -> bool {
    let u = url.to_ascii_lowercase();
    u.contains("alchemy.com") || u.contains("alchemyapi.io") || u.contains("g.alchemy")
}

/// Mempool coverage mode — selected by operator via `ARBX_MEMPOOL_MODE`.
///
/// Cost vs visibility tradeoff:
/// - `Disabled`: no WS subscription. Detection runs purely off block-based
///   workers (PoolSync / Triangular / Flashloan / Liquidation).
///   Cost on Alchemy: ~5% of firehose. Loses JIT / sandwich /
///   pre-confirmation visibility.
/// - `Filtered`: `alchemy_pendingTransactions` with upstream `toAddress`
///   allowlist. Pays only for txs whose `to` is in the
///   allowlist. ~25% of firehose cost.
/// - `Firehose`: `newPendingTransactions`. Full mempool visibility.
///   Highest cost — Alchemy bills per delivered hash plus a
///   follow-up `eth_getTransactionByHash` per match.
/// - `Auto`: pick `Filtered` when the upstream is Alchemy AND the
///   allowlist is non-empty; else fall back to `Firehose`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MempoolMode {
    Disabled,
    Filtered,
    Firehose,
    Auto,
}

impl MempoolMode {
    pub const fn as_str(&self) -> &'static str {
        match self {
            MempoolMode::Disabled => "disabled",
            MempoolMode::Filtered => "filtered",
            MempoolMode::Firehose => "firehose",
            MempoolMode::Auto     => "auto",
        }
    }

    /// Read `ARBX_MEMPOOL_MODE` from env. Defaults to `Auto`. Unknown values
    /// fall back to `Auto` with a warning.
    pub fn from_env() -> Self {
        match std::env::var("ARBX_MEMPOOL_MODE") {
            Ok(v) => match v.trim().to_ascii_lowercase().as_str() {
                "disabled" | "off" | "none" => MempoolMode::Disabled,
                "filtered" | "alchemy"      => MempoolMode::Filtered,
                "firehose" | "all" | "raw"  => MempoolMode::Firehose,
                "auto" | ""                 => MempoolMode::Auto,
                _ => {
                    warn!(
                        event = "chain_client.mempool_mode_unknown",
                        value = %v,
                        "ARBX_MEMPOOL_MODE has unknown value; defaulting to 'auto'"
                    );
                    MempoolMode::Auto
                }
            },
            Err(_) => MempoolMode::Auto,
        }
    }

    /// Resolve `Auto` to a concrete mode based on endpoint capability and
    /// allowlist size. Explicit `Disabled` / `Filtered` / `Firehose` pass
    /// through unchanged so the operator's choice always wins.
    pub fn resolve(self, is_alchemy: bool, allowlist_size: usize) -> Self {
        match self {
            MempoolMode::Auto => {
                if is_alchemy && allowlist_size > 0 {
                    MempoolMode::Filtered
                } else {
                    MempoolMode::Firehose
                }
            }
            other => other,
        }
    }
}

/// Parse `ARBX_MEMPOOL_ALLOWLIST` (CSV of `0x`-prefixed 20-byte hex strings)
/// into the lowercase form Alchemy expects in `toAddress`. Invalid entries
/// are dropped with a per-entry warn log so a single typo doesn't blank the
/// list. Empty / unset env var returns an empty Vec.
///
/// Use this env var to add aggregator routers (1inch, 0x, Paraswap, ...)
/// without recompiling — no-hardcode doctrine.
pub fn parse_extra_allowlist_from_env() -> Vec<String> {
    let raw = match std::env::var("ARBX_MEMPOOL_ALLOWLIST") {
        Ok(v) => v,
        Err(_) => return Vec::new(),
    };
    let mut out = Vec::new();
    for entry in raw.split(',') {
        let s = entry.trim().to_ascii_lowercase();
        if s.is_empty() {
            continue;
        }
        let valid = s.len() == 42
            && s.starts_with("0x")
            && s[2..].chars().all(|c| c.is_ascii_hexdigit());
        if !valid {
            warn!(
                event = "chain_client.allowlist_invalid_entry",
                value = %s,
                "skipping malformed router address in ARBX_MEMPOOL_ALLOWLIST (expected 0x + 40 hex chars)"
            );
            continue;
        }
        out.push(s);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn alchemy_endpoint_recognition() {
        assert!(is_alchemy_endpoint("wss://eth-mainnet.g.alchemy.com/v2/abc"));
        assert!(is_alchemy_endpoint("wss://eth-mainnet.alchemyapi.io/v2/xyz"));
        assert!(is_alchemy_endpoint("https://Eth-Mainnet.g.Alchemy.com/v2/X"));
        assert!(!is_alchemy_endpoint("wss://mainnet.infura.io/ws/v3/abc"));
        assert!(!is_alchemy_endpoint("wss://rpc.flashbots.net"));
    }

    #[test]
    fn mempool_mode_resolves_auto_to_filtered_on_alchemy_with_allowlist() {
        assert_eq!(MempoolMode::Auto.resolve(true, 4), MempoolMode::Filtered);
    }

    #[test]
    fn mempool_mode_resolves_auto_to_firehose_off_alchemy() {
        assert_eq!(MempoolMode::Auto.resolve(false, 4), MempoolMode::Firehose);
    }

    #[test]
    fn mempool_mode_resolves_auto_to_firehose_with_empty_allowlist() {
        assert_eq!(MempoolMode::Auto.resolve(true, 0), MempoolMode::Firehose);
    }

    #[test]
    fn mempool_mode_explicit_choice_passes_through() {
        assert_eq!(MempoolMode::Disabled.resolve(true, 4), MempoolMode::Disabled);
        assert_eq!(MempoolMode::Filtered.resolve(false, 0), MempoolMode::Filtered);
        assert_eq!(MempoolMode::Firehose.resolve(true, 4), MempoolMode::Firehose);
    }

    #[test]
    fn parse_allowlist_accepts_well_formed_csv() {
        std::env::set_var(
            "ARBX_MEMPOOL_ALLOWLIST",
            "0x111111125421ca6dc452d289314280a0f8842a65, 0xDEF1C0DED9BEC7F1A1670819833240F027B25EFF",
        );
        let v = parse_extra_allowlist_from_env();
        assert_eq!(v.len(), 2);
        assert_eq!(v[0], "0x111111125421ca6dc452d289314280a0f8842a65");
        assert_eq!(v[1], "0xdef1c0ded9bec7f1a1670819833240f027b25eff");
        std::env::remove_var("ARBX_MEMPOOL_ALLOWLIST");
    }

    #[test]
    fn parse_allowlist_drops_malformed_entries() {
        std::env::set_var(
            "ARBX_MEMPOOL_ALLOWLIST",
            "0xabc,not-an-address,0x111111125421ca6dc452d289314280a0f8842a65",
        );
        let v = parse_extra_allowlist_from_env();
        assert_eq!(v.len(), 1);
        assert_eq!(v[0], "0x111111125421ca6dc452d289314280a0f8842a65");
        std::env::remove_var("ARBX_MEMPOOL_ALLOWLIST");
    }
}
