//! Multi-relay broadcast — BE-06.
//!
//! Sends the same signed bundle to N relays in parallel via `join_all`.
//! First inclusion on-chain wins; relays that also accepted are harmless —
//! the signed tx cannot be included twice. The chain is the canonical source
//! of truth; submit_engine calls `wait_for_inclusion` once after broadcast.
//!
//! # Adding a new backend
//! 1. Create `relay_<name>.rs` implementing `RelayBackend`.
//! 2. Add `if let Some(c) = <Name>Client::from_env() { backends.push(Arc::new(c)); }` in main.rs.
//!
//! # Env vars
//! - `BLOXROUTE_AUTH_HEADER`  — BloXRoute `Authorization` header value.
//! - `TITAN_BUILDER_URL`      — Titan builder endpoint URL.
//! - `TITAN_AUTH_HEADER`      — Titan `Authorization` header value (requires TITAN_BUILDER_URL).

use crate::bundle_builder::SignedBundle;
use crate::signer::Signer;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use futures_util::future::join_all;
use std::sync::Arc;
use std::time::Duration;
use tracing::{info, warn};

// ─── Error ──────────────────────────────────────────────────────────────────

#[derive(Debug, thiserror::Error)]
pub enum RelayError {
    /// The backend is not configured (missing env var / API key).
    /// Kept for backends that check credentials on first call rather than at construction.
    #[allow(dead_code)]
    #[error("relay not configured: {0}")]
    NotConfigured(&'static str),
    /// HTTP transport error.
    #[error("http error: {0}")]
    Http(#[from] reqwest::Error),
    /// Relay rejected the bundle (non-2xx or RPC error body).
    #[error("relay rejected: {0}")]
    Rejected(String),
    /// JSON serialization/deserialization error.
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),
    /// Internal error.
    #[error("internal: {0}")]
    Internal(String),
}

// ─── Trait ──────────────────────────────────────────────────────────────────

/// Abstraction over any relay backend (Flashbots, BloXRoute, Titan, …).
///
/// Implementors are expected to be `Send + Sync` and held behind `Arc`.
/// The signer is passed on each call so backends that require per-request
/// auth signatures (e.g. Flashbots `X-Flashbots-Signature`) can access it;
/// backends that use a static API key may ignore it.
#[async_trait]
pub trait RelayBackend: Send + Sync {
    /// Submit a signed bundle to this relay.
    async fn send_bundle(
        &self,
        bundle: &SignedBundle,
        signer: &Signer,
    ) -> Result<RelayResponse, RelayError>;

    /// Human-readable name used in logs and `relay_used` fields.
    fn name(&self) -> &str;
}

// ─── Response ────────────────────────────────────────────────────────────────

/// Successful submission record from a single relay.
#[derive(Debug, Clone)]
pub struct RelayResponse {
    /// Bundle hash returned by the relay, if any.
    pub bundle_hash: Option<String>,
    /// Which relay produced this response.
    pub relay_name: String,
    /// Timestamp at which the relay accepted the bundle.
    /// Carried for future audit / persistence (not yet read by submit_engine).
    #[allow(dead_code)]
    pub submitted_at: DateTime<Utc>,
}

// ─── MultiRelayClient ────────────────────────────────────────────────────────

/// Broadcasts the same bundle to all configured backends in parallel.
pub struct MultiRelayClient {
    pub backends: Vec<Arc<dyn RelayBackend>>,
    /// Per-backend deadline. Backends that do not respond within this window
    /// are recorded as timeout failures; they do not block the result.
    pub timeout_ms: u64,
}

impl MultiRelayClient {
    /// Submit `bundle` to all backends concurrently.
    ///
    /// Returns as soon as all futures settle (success, error, or timeout).
    /// The caller should treat `result.successes.len() >= 1` as "at least one
    /// relay accepted"; `wait_for_inclusion` then polls the chain.
    pub async fn broadcast(&self, bundle: &SignedBundle, signer: &Signer) -> MultiRelayResult {
        let timeout = Duration::from_millis(self.timeout_ms);

        // Build one future per backend. Each future is independent — no shared
        // mutable state. Cloning Arc<dyn RelayBackend> is O(1).
        let futures: Vec<_> = self
            .backends
            .iter()
            .map(|backend| {
                let backend = backend.clone();
                // Bundle is borrowed; we need an owned reference for the async
                // closure. `SignedBundle` fields are all owned Strings/u64/H256
                // so cloning is cheap relative to the network call.
                let tx_raw_hex = bundle.tx_raw_hex.clone();
                let target_block = bundle.target_block;
                let opportunity_id = bundle.opportunity_id;
                let tx_hash = bundle.tx_hash;
                let from = bundle.from;
                let nonce = bundle.nonce;
                let value_wei = bundle.value_wei;
                async move {
                    let owned_bundle = SignedBundle {
                        opportunity_id,
                        target_block,
                        tx_raw_hex,
                        tx_hash,
                        from,
                        nonce,
                        value_wei,
                    };
                    let result =
                        tokio::time::timeout(timeout, backend.send_bundle(&owned_bundle, signer))
                            .await;
                    (backend.name().to_string(), result)
                }
            })
            .collect();

        let outcomes = join_all(futures).await;

        let mut successes: Vec<RelayResponse> = Vec::new();
        let mut failures: Vec<(String, String)> = Vec::new();

        for (name, outer) in outcomes {
            match outer {
                Ok(Ok(resp)) => {
                    info!(
                        event = "multi_relay.success",
                        relay = %name,
                        bundle_hash = ?resp.bundle_hash,
                        "bundle accepted by relay"
                    );
                    successes.push(resp);
                }
                Ok(Err(e)) => {
                    warn!(
                        event = "multi_relay.error",
                        relay = %name,
                        error = %e,
                        "relay rejected bundle"
                    );
                    failures.push((name, e.to_string()));
                }
                Err(_elapsed) => {
                    warn!(
                        event = "multi_relay.timeout",
                        relay = %name,
                        timeout_ms = self.timeout_ms,
                        "relay did not respond within timeout"
                    );
                    failures.push((name, format!("timeout_{}ms", self.timeout_ms)));
                }
            }
        }

        MultiRelayResult {
            successes,
            failures,
        }
    }

    /// Comma-joined names of all configured backends (for logging at boot).
    pub fn backend_names(&self) -> String {
        self.backends
            .iter()
            .map(|b| b.name())
            .collect::<Vec<_>>()
            .join(",")
    }
}

// ─── Result ──────────────────────────────────────────────────────────────────

/// Aggregated result of a broadcast round.
pub struct MultiRelayResult {
    /// Relays that accepted the bundle.
    pub successes: Vec<RelayResponse>,
    /// Relays that rejected or timed out, with an error description.
    pub failures: Vec<(String, String)>,
}

impl MultiRelayResult {
    /// True when at least one relay accepted the bundle.
    pub fn any_success(&self) -> bool {
        !self.successes.is_empty()
    }

    /// Comma-joined names of all relays that accepted (for `relay_used` field).
    pub fn relay_used_str(&self) -> String {
        self.successes
            .iter()
            .map(|r| r.relay_name.as_str())
            .collect::<Vec<_>>()
            .join(",")
    }

    /// First accepted bundle hash, if any relay returned one.
    pub fn first_bundle_hash(&self) -> Option<&str> {
        self.successes.iter().find_map(|r| r.bundle_hash.as_deref())
    }
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)] // M11: test module
    use super::*;
    use chrono::Utc;
    use ethers::types::{Address, H256};

    /// A configurable mock backend for unit tests.
    struct MockRelayBackend {
        name: &'static str,
        /// If Some(hash), the call succeeds returning that hash.
        /// If None, the call returns RelayError::Rejected("mock_fail").
        success_hash: Option<&'static str>,
        /// If > 0, sleep for this many ms to simulate latency / trigger timeout.
        delay_ms: u64,
    }

    #[async_trait]
    impl RelayBackend for MockRelayBackend {
        async fn send_bundle(
            &self,
            _bundle: &SignedBundle,
            _signer: &Signer,
        ) -> Result<RelayResponse, RelayError> {
            if self.delay_ms > 0 {
                tokio::time::sleep(Duration::from_millis(self.delay_ms)).await;
            }
            match self.success_hash {
                Some(h) => Ok(RelayResponse {
                    bundle_hash: Some(h.to_string()),
                    relay_name: self.name.to_string(),
                    submitted_at: Utc::now(),
                }),
                None => Err(RelayError::Rejected("mock_fail".to_string())),
            }
        }

        fn name(&self) -> &str {
            self.name
        }
    }

    fn make_bundle() -> SignedBundle {
        SignedBundle {
            opportunity_id: uuid::Uuid::new_v4(),
            target_block: 20_000_000,
            tx_raw_hex: "0xdeadbeef".to_string(),
            tx_hash: H256::zero(),
            from: Address::zero(),
            nonce: 0,
            value_wei: ethers::types::U256::zero(),
        }
    }

    fn make_signer() -> Signer {
        // Use a known throwaway key for unit tests (never connected to mainnet).
        let wallet: ethers::signers::LocalWallet =
            "4c0883a69102937d6231471b5dbb6e538eba2ef2d4d01b66b8fd03ef9a2fe5b0"
                .parse()
                .expect("valid test key");
        let wallet = ethers::signers::Signer::with_chain_id(wallet, 1u64);
        let address = ethers::signers::Signer::address(&wallet);
        Signer {
            wallet,
            address,
            chain_id: 1,
        }
    }

    /// Single backend returns success → 1 success, 0 failures.
    #[tokio::test]
    async fn test_broadcast_one_backend_success() {
        let client = MultiRelayClient {
            backends: vec![Arc::new(MockRelayBackend {
                name: "mock_ok",
                success_hash: Some("0xabc"),
                delay_ms: 0,
            })],
            timeout_ms: 1000,
        };
        let result = client.broadcast(&make_bundle(), &make_signer()).await;
        assert_eq!(result.successes.len(), 1, "expected 1 success");
        assert_eq!(result.failures.len(), 0, "expected 0 failures");
        assert!(result.any_success());
        assert_eq!(result.first_bundle_hash(), Some("0xabc"));
    }

    /// All 3 backends fail → 0 successes, 3 failures.
    #[tokio::test]
    async fn test_broadcast_all_fail() {
        let client = MultiRelayClient {
            backends: vec![
                Arc::new(MockRelayBackend {
                    name: "r1",
                    success_hash: None,
                    delay_ms: 0,
                }),
                Arc::new(MockRelayBackend {
                    name: "r2",
                    success_hash: None,
                    delay_ms: 0,
                }),
                Arc::new(MockRelayBackend {
                    name: "r3",
                    success_hash: None,
                    delay_ms: 0,
                }),
            ],
            timeout_ms: 1000,
        };
        let result = client.broadcast(&make_bundle(), &make_signer()).await;
        assert_eq!(result.successes.len(), 0);
        assert_eq!(result.failures.len(), 3);
        assert!(!result.any_success());
        assert_eq!(result.first_bundle_hash(), None);
    }

    /// 2 succeed, 1 fails → mixed outcome.
    #[tokio::test]
    async fn test_broadcast_partial_success() {
        let client = MultiRelayClient {
            backends: vec![
                Arc::new(MockRelayBackend {
                    name: "ok1",
                    success_hash: Some("0x111"),
                    delay_ms: 0,
                }),
                Arc::new(MockRelayBackend {
                    name: "fail",
                    success_hash: None,
                    delay_ms: 0,
                }),
                Arc::new(MockRelayBackend {
                    name: "ok2",
                    success_hash: Some("0x222"),
                    delay_ms: 0,
                }),
            ],
            timeout_ms: 1000,
        };
        let result = client.broadcast(&make_bundle(), &make_signer()).await;
        assert_eq!(result.successes.len(), 2);
        assert_eq!(result.failures.len(), 1);
        // relay_used_str contains both successful names.
        let used = result.relay_used_str();
        assert!(used.contains("ok1"), "used={used}");
        assert!(used.contains("ok2"), "used={used}");
    }

    /// Backend takes longer than timeout → captured as timeout failure.
    #[tokio::test]
    async fn test_broadcast_timeout() {
        let client = MultiRelayClient {
            backends: vec![Arc::new(MockRelayBackend {
                name: "slow",
                success_hash: Some("will_never_arrive"),
                delay_ms: 200, // will sleep 200ms
            })],
            timeout_ms: 50, // timeout at 50ms
        };
        let result = client.broadcast(&make_bundle(), &make_signer()).await;
        assert_eq!(
            result.successes.len(),
            0,
            "timeout must not produce a success"
        );
        assert_eq!(result.failures.len(), 1);
        assert!(
            result.failures[0].1.contains("timeout"),
            "failure message must mention timeout: {}",
            result.failures[0].1
        );
    }
}
