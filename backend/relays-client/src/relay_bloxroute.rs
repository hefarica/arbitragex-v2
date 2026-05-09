//! BloXRoute relay backend — BE-06 stub.
//!
//! Submits `eth_sendBundle` to BloXRoute's MEV relay endpoint using a static
//! `Authorization` header (no per-request crypto signing required).
//!
//! # Configuration
//! Set `BLOXROUTE_AUTH_HEADER` to the full header value provided by BloXRoute
//! on account creation (format varies by plan — see their documentation).
//!
//! If the env var is absent, `BloXRouteClient::from_env()` returns `None`
//! and the backend is silently excluded from the `MultiRelayClient`'s pool.
//! No fake data is served. No panic occurs.
//!
//! # Reference
//! POST https://mev.api.blxrbdn.com — body: eth_sendBundle JSON-RPC 2.0 payload
//! Header: `Authorization: <BLOXROUTE_AUTH_HEADER>`

use crate::bundle_builder::SignedBundle;
use crate::multi_relay::{RelayBackend, RelayError, RelayResponse};
use crate::signer::Signer;
use async_trait::async_trait;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::time::Duration;
use tracing::debug;

const BLOXROUTE_MEV_URL: &str = "https://mev.api.blxrbdn.com";

pub struct BloXRouteClient {
    auth_header: String,
    http: reqwest::Client,
    url: String,
}

impl BloXRouteClient {
    /// Construct from environment. Returns `None` when `BLOXROUTE_AUTH_HEADER` is absent
    /// or empty — do not add this backend to the multi-relay pool in that case.
    pub fn from_env() -> Option<Self> {
        let auth = std::env::var("BLOXROUTE_AUTH_HEADER").ok().filter(|v| !v.is_empty())?;
        let url = std::env::var("BLOXROUTE_RELAY_URL")
            .ok()
            .filter(|v| !v.is_empty())
            .unwrap_or_else(|| BLOXROUTE_MEV_URL.to_string());
        let http = reqwest::Client::builder()
            .timeout(Duration::from_secs(8))
            .build()
            .expect("reqwest client for bloxroute");
        Some(Self { auth_header: auth, http, url })
    }
}

#[async_trait]
impl RelayBackend for BloXRouteClient {
    async fn send_bundle(
        &self,
        bundle: &SignedBundle,
        _signer: &Signer,
    ) -> Result<RelayResponse, RelayError> {
        // BloXRoute accepts the Flashbots eth_sendBundle JSON-RPC format.
        let body = BundleRequest {
            jsonrpc: "2.0",
            id: 1,
            method: "eth_sendBundle",
            params: vec![BundleParams {
                txs: vec![bundle.tx_raw_hex.clone()],
                block_number: format!("0x{:x}", bundle.target_block),
            }],
        };
        let body_json = serde_json::to_string(&body)?;

        debug!(
            event = "bloxroute.sending",
            target_block = bundle.target_block,
            url = %self.url,
        );

        let res = self
            .http
            .post(&self.url)
            .header("content-type", "application/json")
            .header("Authorization", &self.auth_header)
            .body(body_json)
            .send()
            .await?;

        let status = res.status();
        let text = res.text().await?;

        debug!(event = "bloxroute.response", status = status.as_u16());

        if !status.is_success() {
            return Err(RelayError::Rejected(format!("http {}: {}", status.as_u16(), text)));
        }

        let parsed: BundleResponse = serde_json::from_str(&text).map_err(|e| {
            RelayError::Rejected(format!("parse_error: {e} body={text}"))
        })?;

        if let Some(err) = parsed.error {
            return Err(RelayError::Rejected(format!(
                "rpc_error code={} msg={}",
                err.code, err.message
            )));
        }

        let bundle_hash = parsed.result.and_then(|r| r.bundle_hash);

        Ok(RelayResponse {
            bundle_hash,
            relay_name: self.name().to_string(),
            submitted_at: Utc::now(),
        })
    }

    fn name(&self) -> &str {
        "bloxroute"
    }
}

// ─── Wire types ─────────────────────────────────────────────────────────────

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

#[derive(Debug, Deserialize)]
struct BundleResponse {
    #[serde(default)]
    result: Option<BundleResult>,
    #[serde(default)]
    error: Option<RpcError>,
}

#[derive(Debug, Deserialize)]
struct BundleResult {
    #[serde(rename = "bundleHash")]
    bundle_hash: Option<String>,
}

#[derive(Debug, Deserialize)]
struct RpcError {
    code: i64,
    message: String,
}
