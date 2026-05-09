//! Flashbots relay client.
//!
//! Submits `eth_sendBundle` to the Flashbots Relay with X-Flashbots-Signature
//! auth header.
//!
//! `FlashbotsClient` also implements `RelayBackend` (BE-06) so it can be
//! composed into a `MultiRelayClient` alongside BloXRoute and Titan backends.

use crate::bundle_builder::SignedBundle;
use crate::multi_relay::{RelayBackend, RelayError, RelayResponse};
use crate::signer::Signer;
use anyhow::{Context, Result};
use async_trait::async_trait;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::time::Duration;
use tracing::debug;

#[derive(Clone)]
pub struct FlashbotsClient {
    pub url: String,
    pub http: reqwest::Client,
}

impl FlashbotsClient {
    /// Construct from env vars `FLASHBOTS_RELAY_URL` + optional timeout.
    /// Returns `None` when the URL is absent — backend excluded from pool.
    ///
    /// NOTE: main.rs resolves the URL from the DB relay catalog first and calls
    /// `FlashbotsClient::new()` directly. `from_env` exists for completeness and
    /// test convenience; it is not called from the production boot path.
    #[allow(dead_code)]
    pub fn from_env(timeout: Duration) -> Option<Self> {
        let url = std::env::var("FLASHBOTS_RELAY_URL").ok().filter(|v| !v.is_empty())?;
        Some(Self::new(url, timeout))
    }

    pub fn new(url: String, timeout: Duration) -> Self {
        let http = reqwest::Client::builder()
            .timeout(timeout)
            .build()
            .expect("reqwest client");
        Self { url, http }
    }

    /// Returns the Flashbots response with bundleHash on success.
    pub async fn send_bundle(&self, signer: &Signer, bundle: &SignedBundle) -> Result<BundleResponse> {
        let body = BundleRequest::from(bundle);
        let body_json = serde_json::to_string(&body).context("serialize bundle")?;
        let auth = signer.flashbots_auth_header(body_json.as_bytes()).await
            .context("flashbots auth")?;
        let res = self.http.post(&self.url)
            .header("content-type", "application/json")
            .header("X-Flashbots-Signature", auth)
            .body(body_json)
            .send().await
            .context("send flashbots bundle")?;

        let status = res.status();
        let text = res.text().await.context("read flashbots response")?;
        debug!(event = "flashbots.response", status = status.as_u16());

        if !status.is_success() {
            anyhow::bail!("flashbots http {}: {}", status.as_u16(), text);
        }

        let parsed: BundleResponse = serde_json::from_str(&text)
            .with_context(|| format!("parse flashbots response: {}", text))?;
        Ok(parsed)
    }
}

#[derive(Debug, Serialize)]
struct BundleRequest {
    jsonrpc: &'static str,
    id: u64,
    method: &'static str,
    params: Vec<BundleParams>,
}

#[derive(Debug, Serialize)]
struct BundleParams {
    txs: Vec<String>,
    #[serde(rename = "blockNumber")]
    block_number: String,
}

impl From<&SignedBundle> for BundleRequest {
    fn from(b: &SignedBundle) -> Self {
        Self {
            jsonrpc: "2.0",
            id: 1,
            method: "eth_sendBundle",
            params: vec![BundleParams {
                txs: vec![b.tx_raw_hex.clone()],
                block_number: format!("0x{:x}", b.target_block),
            }],
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct BundleResponse {
    #[serde(default)]
    pub result: Option<BundleResult>,
    #[serde(default)]
    pub error: Option<RpcError>,
}

#[derive(Debug, Deserialize)]
pub struct BundleResult {
    #[serde(rename = "bundleHash")]
    pub bundle_hash: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct RpcError {
    pub code: i64,
    pub message: String,
}

// ─── RelayBackend impl ───────────────────────────────────────────────────────

/// Wraps the existing `send_bundle` method to satisfy the `RelayBackend` trait.
/// This is purely an adapter — the underlying HTTP logic is unchanged.
#[async_trait]
impl RelayBackend for FlashbotsClient {
    async fn send_bundle(
        &self,
        bundle: &SignedBundle,
        signer: &Signer,
    ) -> Result<RelayResponse, RelayError> {
        let resp = FlashbotsClient::send_bundle(self, signer, bundle)
            .await
            .map_err(|e| RelayError::Internal(e.to_string()))?;

        if let Some(err) = resp.error {
            return Err(RelayError::Rejected(format!(
                "rpc_error code={} msg={}",
                err.code, err.message
            )));
        }

        let bundle_hash = resp.result.and_then(|r| r.bundle_hash);
        Ok(RelayResponse {
            bundle_hash,
            relay_name: self.name().to_string(),
            submitted_at: Utc::now(),
        })
    }

    fn name(&self) -> &str {
        "flashbots"
    }
}
