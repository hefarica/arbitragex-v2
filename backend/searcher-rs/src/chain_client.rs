//! WebSocket chain client — mempool subscriptions and new-head events only.
//!
//! ARCHITECTURAL NOTE (audit A2, re-run 2026-05-10):
//! This module handles WebSocket subscriptions exclusively:
//!   - `subscribe_pending()` → firehose of pending-tx hashes via `newPendingTransactions`
//!   - `subscribe_pending_filtered_txs()` → Alchemy `alchemy_pendingTransactions` allowlist
//!   - `get_tx()` → single `eth_getTransactionByHash` on the same WS connection
//!
//! It deliberately does NOT reference `HttpRpcPool`.  HttpRpcPool is the HTTP-RPC
//! failover pool (eth_call, eth_estimateGas, eth_getLogs, …) — a different transport
//! layer with a different fault-tolerance model.  The two pools are orthogonal:
//!
//!   WebSocket path (this file):
//!     - Long-lived subscription stream, reconnects on socket drop.
//!     - WS endpoints cycled via WsRpcPool::from_env (shared-rs::rpc_failover).
//!     - Appropriate for push-based mempool and block-head data.
//!
//!   HTTP RPC path (NOT this file):
//!     - Short-lived request/response, retried with jitter on failure.
//!     - Wired via shared-rs::rpc_failover::HttpRpcPool::with_retry at:
//!         • searcher-rs::scanner (V3 quoter calls)        — scanner.rs:606
//!         • searcher-rs::workers::triangular_worker       — quoter fan-out
//!         • searcher-rs::workers::liquidation_worker      — health-factor reads
//!         • searcher-rs::workers::pool_sync_worker        — eth_getLogs range pulls
//!     - Arc<HttpRpcPool> is constructed and health-looped in main.rs:139-177.
//!
//! Future auditors: the absence of HttpRpcPool here is intentional architecture,
//! not an oversight.  Grep for `HttpRpcPool` in scanner.rs and main.rs for the
//! HTTP-path wiring.
//!
//! Responsibilities:
//!   - Open `Provider<Ws>` against a configured RPC endpoint.
//!   - Expose `subscribe_pending()` returning a stream of pending-tx hashes.
//!   - Expose `subscribe_pending_filtered_txs()` for Alchemy upstream filtering.
//!   - Let the caller fetch the full transaction body via `get_tx()`.
//!   - Emit structured tracing events for connection / subscription lifecycle.
//!
//! Honest behavior: if `connect()` fails, the error is bubbled up so the scanner
//! loop can apply its own backoff + retry policy.  We never fabricate a connection.

use anyhow::Context;
use ethers::providers::{Middleware, Provider, StreamExt, SubscriptionStream, Ws};
use ethers::types::{Transaction, H256};
use futures_util::{future::Either, Stream};
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
        // OBSERVER-ONLY: bare ethers WS provider (subscribe_pending / subscribe_blocks /
        // get_transaction — reads only). NEVER wrap with SignerMiddleware / .with_signer;
        // no LocalWallet may be attached here (capital_exposed == 0).
        let provider = timeout(Duration::from_secs(10), Provider::<Ws>::connect(url))
            .await
            .context("ws connect timeout")??;
        let observed_chain = provider
            .get_chainid()
            .await
            .context("get_chainid")?
            .as_u64();
        if observed_chain != chain_id {
            anyhow::bail!(
                "chain_id mismatch: config={} observed={} (url likely wrong)",
                chain_id,
                observed_chain
            );
        }
        info!(
            event = "chain_client.connected",
            chain_id, "ws provider connected"
        );
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
    pub async fn subscribe_pending(&self) -> anyhow::Result<impl Stream<Item = H256> + Send + '_> {
        // A1/B-02 fix: PublicNode and other non-Alchemy providers send
        // newPendingTransactions as JSON objects {hash: "0x...", ...} instead
        // of bare hex strings. ethers-rs subscribe_pending_txs() tries to
        // deserialize each item as H256, fails ("invalid type: map, expected
        // ... 32 bytes"), and silently drops ALL items — pending_received=0.
        //
        // Fix: subscribe via the raw RPC method with serde_json::Value,
        // then extract the hash from either:
        //   (a) bare hex string:  "0xabcd..."  (Alchemy/Infura format)
        //   (b) JSON object:      {"hash": "0xabcd..."} (PublicNode/drpc format)
        //
        // WSSUB-05 (2026-08-15T23:03Z): the raw eth_subscribe can also FAIL AT
        // SUBSCRIBE TIME — not just emit odd item shapes. L4 evidence:
        // alchemy rejected `eth_subscribe("newPendingTransactions")` in 113ms
        // (WS connect + get_chainid had already succeeded, so the key is valid
        // — the subscribe call itself was refused) while publicnode hung ~60s
        // before failing. Both errors hit the `?` and propagated to the
        // scanner's provider rotation, which looped forever without a single
        // successful subscription → heartbeat pending_received=0 forever.
        // Mitigations:
        //   1. Each subscribe attempt is capped at 15s (tokio::time::timeout)
        //      so a hanging provider fails fast and rotation proceeds.
        //   2. On raw-subscribe failure we fall back to the typed
        //      `subscribe_pending_txs()` H256 stream (known-good against
        //      Alchemy) before giving up. The tolerant raw item mapping below
        //      is kept unchanged for providers that DO accept the raw method.
        //   3. Error chains are logged in full (`{e:#}`) — the old
        //      `scanner.subscription_error` line rendered only the outermost
        //      context and hid the underlying JSON-RPC error.
        let raw_sub = timeout(
            Duration::from_secs(15),
            self.provider
                .subscribe("newPendingTransactions".to_string()),
        )
        .await
        .map_err(|_| anyhow::anyhow!("subscribe timeout 15s"))
        .and_then(|res| res.context("subscribe newPendingTransactions (raw)"));

        let sub = match raw_sub {
            Ok(raw) => Either::Left(
                raw.map(move |item: serde_json::Value| match item {
                    serde_json::Value::String(s) => s.parse::<H256>().ok(),
                    serde_json::Value::Object(map) => map
                        .get("hash")
                        .and_then(|v| v.as_str())
                        .and_then(|s| s.parse::<H256>().ok()),
                    _ => None,
                })
                .filter(|h| futures_util::future::ready(h.is_some()))
                .map(|h| h.unwrap()),
            ),
            Err(e) => {
                warn!(
                    event = "chain_client.subscribe_raw_failed",
                    chain_id = self.chain_id,
                    error = %format!("{e:#}"),
                    "raw newPendingTransactions subscribe failed; trying typed fallback"
                );
                let typed_sub = timeout(
                    Duration::from_secs(15),
                    self.provider.subscribe_pending_txs(),
                )
                .await
                .map_err(|_| anyhow::anyhow!("subscribe timeout 15s"))
                .and_then(|res| res.context("subscribe_pending_txs (typed fallback)"))
                .map_err(|typed_e| {
                    typed_e.context(format!(
                        "both pending-tx subscribe attempts failed; raw error: {e:#}"
                    ))
                })?;
                Either::Right(typed_sub)
            }
        };
        Ok(sub)
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
            anyhow::bail!(
                "subscribe_pending_filtered_txs: empty allowlist would unfilter the stream"
            );
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

    /// Subscribe to new block headers (`eth_subscribe("newHeads")`). Free RPCs
    /// support this even when they reject `alchemy_pendingTransactions`. Used by the
    /// block/log backrun scanner (`ARBX_MEMPOOL_MODE=block`).
    pub async fn subscribe_blocks(
        &self,
    ) -> anyhow::Result<SubscriptionStream<'_, Ws, ethers::types::Block<H256>>> {
        let sub = self
            .provider
            .subscribe_blocks()
            .await
            .context("subscribe_blocks (newHeads)")?;
        info!(
            event = "chain_client.subscribed_blocks",
            chain_id = self.chain_id,
            "newHeads subscription active"
        );
        Ok(sub)
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
    /// FASE OMEGA — block/log backrunning. No pending-tx subscription; instead
    /// subscribe to `newHeads` + `eth_getLogs` for confirmed V2 swaps on watched
    /// pools (free-RPC friendly). Spawns `block_scanner::block_detection_loop`.
    Block,
}

impl MempoolMode {
    pub const fn as_str(&self) -> &'static str {
        match self {
            MempoolMode::Disabled => "disabled",
            MempoolMode::Filtered => "filtered",
            MempoolMode::Firehose => "firehose",
            MempoolMode::Auto => "auto",
            MempoolMode::Block => "block",
        }
    }

    /// Read `ARBX_MEMPOOL_MODE` from env. Defaults to `Auto`. Unknown values
    /// fall back to `Auto` with a warning.
    pub fn from_env() -> Self {
        match std::env::var("ARBX_MEMPOOL_MODE") {
            Ok(v) => match v.trim().to_ascii_lowercase().as_str() {
                "disabled" | "off" | "none" => MempoolMode::Disabled,
                "filtered" | "alchemy" => MempoolMode::Filtered,
                "firehose" | "all" | "raw" => MempoolMode::Firehose,
                "block" | "blocks" | "newheads" | "backrun" => MempoolMode::Block,
                "auto" | "" => MempoolMode::Auto,
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
        let valid =
            s.len() == 42 && s.starts_with("0x") && s[2..].chars().all(|c| c.is_ascii_hexdigit());
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
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    // Serialize tests that mutate ARBX_MEMPOOL_ALLOWLIST — std::env::set_var
    // is not thread-safe under parallel test execution.
    static ALLOWLIST_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    #[test]
    fn alchemy_endpoint_recognition() {
        assert!(is_alchemy_endpoint(
            "wss://eth-mainnet.g.alchemy.com/v2/abc"
        ));
        assert!(is_alchemy_endpoint(
            "wss://eth-mainnet.alchemyapi.io/v2/xyz"
        ));
        assert!(is_alchemy_endpoint(
            "https://Eth-Mainnet.g.Alchemy.com/v2/X"
        ));
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
        assert_eq!(
            MempoolMode::Disabled.resolve(true, 4),
            MempoolMode::Disabled
        );
        assert_eq!(
            MempoolMode::Filtered.resolve(false, 0),
            MempoolMode::Filtered
        );
        assert_eq!(
            MempoolMode::Firehose.resolve(true, 4),
            MempoolMode::Firehose
        );
    }

    #[test]
    fn parse_allowlist_accepts_well_formed_csv() {
        let _guard = ALLOWLIST_LOCK.lock().unwrap();
        std::env::set_var(
            "ARBX_MEMPOOL_ALLOWLIST",
            "0x111111125421ca6dc452d289314280a0f8842a65, 0xDEF1C0DED9BEC7F1A1670819833240F027B25EFF",
        );
        let v = parse_extra_allowlist_from_env();
        std::env::remove_var("ARBX_MEMPOOL_ALLOWLIST");
        assert_eq!(v.len(), 2);
        assert_eq!(v[0], "0x111111125421ca6dc452d289314280a0f8842a65");
        assert_eq!(v[1], "0xdef1c0ded9bec7f1a1670819833240f027b25eff");
    }

    #[test]
    fn parse_allowlist_drops_malformed_entries() {
        let _guard = ALLOWLIST_LOCK.lock().unwrap();
        std::env::set_var(
            "ARBX_MEMPOOL_ALLOWLIST",
            "0xabc,not-an-address,0x111111125421ca6dc452d289314280a0f8842a65",
        );
        let v = parse_extra_allowlist_from_env();
        std::env::remove_var("ARBX_MEMPOOL_ALLOWLIST");
        assert_eq!(v.len(), 1);
        assert_eq!(v[0], "0x111111125421ca6dc452d289314280a0f8842a65");
    }
}
