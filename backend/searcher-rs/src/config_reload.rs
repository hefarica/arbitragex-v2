//! Chain config hot-reload subscriber (B1.c, 2026-05-13).
//!
//! Listens on the Redis pub/sub channel `arbx:config:chains:reload` for
//! events published by the api-server admin endpoint when an operator
//! creates / updates / disables a chain via the `/admin/chains` UI.
//!
//! ## Wire format (must mirror api-server `publishReload`)
//!
//! The publisher serialises:
//! ```json
//! {
//!   "action":      "created" | "updated" | "disabled",
//!   "chain_id":    1,
//!   "config_hash": "sha256 hex",
//!   "timestamp":   "2026-05-13T00:00:00.000Z"
//! }
//! ```
//!
//! Source of truth: `backend/api-server/src/routes/admin-chains.ts:131`.
//! If the publisher schema changes, update [`ChainReloadEvent`] in lockstep.
//!
//! ## Dedup discipline
//!
//! The subscriber keeps an in-memory `HashMap<chain_id, last_seen_hash>`.
//! Messages whose `config_hash` matches the last-seen value for that chain
//! are no-ops — they are duplicates from the per-chain channel, retry
//! storms, or unrelated mutations that did not change a runtime-relevant
//! field (e.g., notes-only edits, per the api-server `computeConfigHash`
//! payload which excludes notes by design).
//!
//! ## Lifecycle
//!
//! - Run via `tokio::spawn(reloader.run())` — non-blocking.
//! - Shutdown via [`CancellationToken`] passed at construction.
//! - On shutdown the subscriber exits at its next `select!` poll, well
//!   under the 30s graceful budget operators care about for restarts.
//!
//! ## Downstream rebuild — deferred to B1.d
//!
//! This MVP emits structured tracing events on each reload. The actual
//! `HashMap<chain_id, Arc<HttpRpcPool>>` rebuild + per-chain task
//! spawn/abort is deferred to a follow-up sprint because it requires
//! touching the scanner's main loop, which is out of scope for B1.c.
//! B1.c lands the listener infrastructure; B1.d wires the rebuild
//! handler that consumes [`SeenHashes`] and dispatches the chain task
//! supervisor.

use anyhow::{Context, Result};
use futures_util::StreamExt;
use serde::Deserialize;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio_util::sync::CancellationToken;
use tracing::{debug, info, warn};

/// The pub/sub channel the api-server publishes on.
/// Mirror of `admin-chains.ts:146` (global channel; the per-chain channel
/// `arbx:config:chains:<id>:reload` is intentionally NOT subscribed here —
/// a single global subscriber + dedup on `config_hash` covers the same
/// information without doubling message volume).
pub const CHAIN_RELOAD_CHANNEL: &str = "arbx:config:chains:reload";

/// Payload schema mirror of `publishReload` in admin-chains.ts:131.
/// `timestamp` is `serde(default)` so future publishers that omit it
/// (or publish over a partial schema during migration) still parse.
#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct ChainReloadEvent {
    pub action: String,
    pub chain_id: u64,
    pub config_hash: String,
    #[serde(default)]
    pub timestamp: String,
}

/// In-memory dedup state: chain_id → last-seen config_hash.
///
/// Wrapped in `RwLock` so downstream consumers (B1.d chain task
/// supervisor) can take a read snapshot without blocking the writer.
pub type SeenHashes = Arc<RwLock<HashMap<u64, String>>>;

/// Outcome of classifying a single reload event.
///
/// The decision is exposed publicly so unit tests and B1.d's supervisor
/// can react differentially (e.g., supervisor only triggers rebuild on
/// `Accepted`, ignores `DedupSkipped`, surfaces `Malformed` as an alert).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReloadDecision {
    /// First-seen (chain, hash) pair → state updated; downstream should
    /// rebuild the chain's runtime state.
    Accepted,
    /// `config_hash` matches last-seen for this chain → no-op.
    DedupSkipped,
    /// Payload was structurally invalid (empty hash, missing required
    /// field, etc.) → state unchanged, event ignored.
    Malformed,
}

pub struct ChainConfigReloader {
    redis_url: String,
    cancel: CancellationToken,
    seen: SeenHashes,
    pub event_tx: tokio::sync::broadcast::Sender<ChainReloadEvent>,
}

impl ChainConfigReloader {
    /// Construct a reloader. Does NOT open the Redis connection — that
    /// happens at the start of `run()` so construction stays cheap and
    /// infallible.
    pub fn new(redis_url: String, cancel: CancellationToken) -> Self {
        let (tx, _) = tokio::sync::broadcast::channel(100);
        Self {
            redis_url,
            cancel,
            seen: Arc::new(RwLock::new(HashMap::new())),
            event_tx: tx,
        }
    }

    /// Cloneable handle to the dedup map for downstream consumers (B1.d)
    /// who want to inspect "what config_hash am I currently running for
    /// chain X?". Returns an `Arc` clone — same underlying data.
    ///
    /// `#[allow(dead_code)]` until the B1.d chain task supervisor lands
    /// and starts consuming this handle. Tests already exercise it.
    #[allow(dead_code)]
    pub fn seen_hashes(&self) -> SeenHashes {
        self.seen.clone()
    }

    /// Pure dedup decision. No I/O. Exposed for unit tests and for
    /// downstream consumers that want to feed events from a non-Redis
    /// source (e.g., a replay test of historical events).
    pub async fn classify(&self, event: &ChainReloadEvent) -> ReloadDecision {
        if event.config_hash.is_empty() {
            return ReloadDecision::Malformed;
        }
        let mut map = self.seen.write().await;
        if let Some(existing) = map.get(&event.chain_id) {
            if existing == &event.config_hash {
                return ReloadDecision::DedupSkipped;
            }
        }
        map.insert(event.chain_id, event.config_hash.clone());
        ReloadDecision::Accepted
    }

    /// Run the subscriber until cancelled.
    ///
    /// Designed to live in a dedicated `tokio::spawn` task so it never
    /// blocks the scanner main thread. The function returns when:
    /// - The cancellation token is signalled (including during boot —
    ///   the pubsub connect and subscribe both race the token so a
    ///   SIGTERM during startup is honoured promptly), or
    /// - The pubsub stream ends (Redis closes the connection), or
    /// - The Redis client fails to open / subscribe at boot time.
    pub async fn run(self) -> Result<()> {
        info!(
            event = "chain_reload.boot",
            channel = CHAIN_RELOAD_CHANNEL,
            "starting chain config reload subscriber"
        );

        let client = redis::Client::open(self.redis_url.clone())
            .context("open Redis client for chain reload pub/sub")?;

        // Dedicated async pubsub connection. NOT the shared
        // ConnectionManager — redis-rs pubsub semantics require a
        // non-multiplexed connection per subscriber.
        //
        // Race the connect attempt against the cancellation token so
        // shutdown during boot returns promptly instead of waiting on
        // a TCP connect timeout that can be many seconds.
        let mut pubsub = tokio::select! {
            _ = self.cancel.cancelled() => {
                info!(event = "chain_reload.shutdown_during_boot", "cancelled before pubsub connect");
                return Ok(());
            }
            res = client.get_async_pubsub() => {
                res.context("get_async_pubsub for chain reload")?
            }
        };
        tokio::select! {
            _ = self.cancel.cancelled() => {
                info!(event = "chain_reload.shutdown_during_boot", "cancelled before subscribe");
                return Ok(());
            }
            res = pubsub.subscribe(CHAIN_RELOAD_CHANNEL) => {
                res.context("subscribe to chain reload channel")?;
            }
        };

        info!(
            event = "chain_reload.subscribed",
            channel = CHAIN_RELOAD_CHANNEL,
            "ready to receive chain config reload events"
        );

        let mut stream = pubsub.on_message();
        loop {
            tokio::select! {
                _ = self.cancel.cancelled() => {
                    info!(
                        event = "chain_reload.shutdown",
                        "cancellation signal received; exiting subscriber loop"
                    );
                    break;
                }
                msg = stream.next() => {
                    match msg {
                        Some(m) => {
                            let payload: String = match m.get_payload() {
                                Ok(s) => s,
                                Err(e) => {
                                    warn!(
                                        event = "chain_reload.payload_err",
                                        error = %e,
                                        "redis msg payload could not be coerced to String"
                                    );
                                    continue;
                                }
                            };
                            self.process_payload(&payload).await;
                        }
                        None => {
                            warn!(
                                event = "chain_reload.stream_closed",
                                "pubsub stream ended unexpectedly; exiting subscriber"
                            );
                            break;
                        }
                    }
                }
            }
        }
        Ok(())
    }

    async fn process_payload(&self, payload: &str) {
        let event: ChainReloadEvent = match serde_json::from_str(payload) {
            Ok(e) => e,
            Err(e) => {
                warn!(
                    event = "chain_reload.json_parse_err",
                    error = %e,
                    payload,
                    "could not deserialize reload event"
                );
                return;
            }
        };
        let decision = self.classify(&event).await;
        match decision {
            ReloadDecision::Accepted => {
                info!(
                    event = "chain_reload.accepted",
                    action = %event.action,
                    chain_id = event.chain_id,
                    config_hash = %event.config_hash,
                    "chain config change observed; downstream rebuild deferred to B1.d"
                );
                let _ = self.event_tx.send(event.clone());
            }
            ReloadDecision::DedupSkipped => {
                debug!(
                    event = "chain_reload.dedup_skipped",
                    chain_id = event.chain_id,
                    config_hash = %event.config_hash,
                    "duplicate config_hash; no-op"
                );
            }
            ReloadDecision::Malformed => {
                warn!(
                    event = "chain_reload.malformed",
                    payload, "config_hash empty; payload ignored"
                );
            }
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    fn ev(chain_id: u64, hash: &str, action: &str) -> ChainReloadEvent {
        ChainReloadEvent {
            action: action.into(),
            chain_id,
            config_hash: hash.into(),
            timestamp: "2026-05-13T00:00:00Z".into(),
        }
    }

    // ── classify() dedup behaviour ─────────────────────────────────────

    #[tokio::test]
    async fn first_event_per_chain_accepted() {
        let r = ChainConfigReloader::new("redis://_test".into(), CancellationToken::new());
        assert_eq!(
            r.classify(&ev(1, "h1", "created")).await,
            ReloadDecision::Accepted
        );
    }

    #[tokio::test]
    async fn duplicate_hash_skipped() {
        let r = ChainConfigReloader::new("redis://_test".into(), CancellationToken::new());
        let _ = r.classify(&ev(1, "h1", "created")).await;
        assert_eq!(
            r.classify(&ev(1, "h1", "updated")).await,
            ReloadDecision::DedupSkipped,
        );
    }

    #[tokio::test]
    async fn different_hash_same_chain_accepted() {
        let r = ChainConfigReloader::new("redis://_test".into(), CancellationToken::new());
        let _ = r.classify(&ev(1, "h1", "created")).await;
        assert_eq!(
            r.classify(&ev(1, "h2", "updated")).await,
            ReloadDecision::Accepted,
        );
    }

    #[tokio::test]
    async fn chains_tracked_independently() {
        let r = ChainConfigReloader::new("redis://_test".into(), CancellationToken::new());
        let _ = r.classify(&ev(1, "h1", "created")).await;
        let _ = r.classify(&ev(137, "h1", "created")).await;
        // Same hash but different chain → independent state; second
        // submission for each chain is a dedup-skip.
        assert_eq!(
            r.classify(&ev(137, "h1", "created")).await,
            ReloadDecision::DedupSkipped,
        );
        assert_eq!(
            r.classify(&ev(1, "h1", "created")).await,
            ReloadDecision::DedupSkipped,
        );
    }

    #[tokio::test]
    async fn empty_hash_is_malformed_and_state_unchanged() {
        let r = ChainConfigReloader::new("redis://_test".into(), CancellationToken::new());
        assert_eq!(
            r.classify(&ev(1, "", "created")).await,
            ReloadDecision::Malformed
        );
        let map = r.seen.read().await;
        assert!(
            !map.contains_key(&1),
            "Malformed must NOT update dedup state"
        );
    }

    #[tokio::test]
    async fn seen_hashes_exposes_writer_state() {
        let r = ChainConfigReloader::new("redis://_test".into(), CancellationToken::new());
        let handle = r.seen_hashes();
        let _ = r.classify(&ev(42, "deadbeef", "created")).await;
        let map = handle.read().await;
        assert_eq!(map.get(&42).map(String::as_str), Some("deadbeef"));
    }

    // ── ChainReloadEvent serde compatibility with publisher ────────────

    #[test]
    fn payload_full_schema_parses() {
        // Mirror of api-server publishReload (admin-chains.ts:131).
        let json = r#"{
            "action": "updated",
            "chain_id": 1,
            "config_hash": "abc123",
            "timestamp": "2026-05-13T00:00:00.000Z"
        }"#;
        let e: ChainReloadEvent = serde_json::from_str(json).unwrap();
        assert_eq!(e.action, "updated");
        assert_eq!(e.chain_id, 1);
        assert_eq!(e.config_hash, "abc123");
        assert_eq!(e.timestamp, "2026-05-13T00:00:00.000Z");
    }

    #[test]
    fn payload_missing_timestamp_still_parses() {
        let json = r#"{"action":"created","chain_id":42,"config_hash":"h"}"#;
        let e: ChainReloadEvent = serde_json::from_str(json).unwrap();
        assert_eq!(e.action, "created");
        assert_eq!(e.chain_id, 42);
        assert_eq!(e.config_hash, "h");
        assert_eq!(e.timestamp, "");
    }

    #[test]
    fn payload_garbage_rejected() {
        assert!(serde_json::from_str::<ChainReloadEvent>("not json").is_err());
        // Missing chain_id — required field.
        assert!(serde_json::from_str::<ChainReloadEvent>(
            "{\"action\":\"x\",\"config_hash\":\"h\"}"
        )
        .is_err());
    }

    // ── Cancellation behaviour ─────────────────────────────────────────

    #[tokio::test]
    async fn pre_cancelled_token_returns_promptly() {
        // We cancel BEFORE invoking run(). The Redis client open / pubsub
        // subscribe may fail on the bogus URL, OR the cancel branch may
        // fire first — either path is an acceptable shutdown. The
        // invariant we assert is the bounded return time: the subscriber
        // must never hang on a cancelled token.
        let cancel = CancellationToken::new();
        cancel.cancel();
        let r = ChainConfigReloader::new("redis://127.0.0.1:1/0".into(), cancel);
        let result = tokio::time::timeout(std::time::Duration::from_secs(2), r.run()).await;
        assert!(
            result.is_ok(),
            "run() must return within 2s on a pre-cancelled token"
        );
    }
}
