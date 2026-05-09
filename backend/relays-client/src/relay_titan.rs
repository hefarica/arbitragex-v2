//! Titan Builder relay backend — BE-06 stub.
//!
//! Submits `eth_sendBundle` to Titan's builder endpoint. Titan is MEV-Boost
//! compatible and accepts the standard `eth_sendBundle` JSON-RPC payload.
//! Auth uses a bearer token in the `Authorization` header.
//!
//! # Configuration
//! Both env vars must be set for this backend to activate:
//! - `TITAN_BUILDER_URL`  — full HTTPS endpoint (e.g. `https://rpc.titanbuilder.xyz`)
//! - `TITAN_AUTH_HEADER`  — value for the `Authorization` header (e.g. `Bearer <token>`)
//!
//! If either var is absent, `TitanClient::from_env()` returns `None` and the
//! backend is silently excluded. No fake data. No panic.

use crate::bundle_builder::SignedBundle;
use crate::multi_relay::{RelayBackend, RelayError, RelayResponse};
use crate::signer::Signer;
use async_trait::async_trait;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::time::Duration;
use tracing::debug;

pub struct TitanClient {
    url: String,
    auth_header: String,
    http: reqwest::Client,
}

impl TitanClient {
    /// Construct from environment. Returns `None` when any required env var is absent.
    pub fn from_env() -> Option<Self> {
        let url = std::env::var("TITAN_BUILDER_URL").ok().filter(|v| !v.is_empty())?;
        let auth = std::env::var("TITAN_AUTH_HEADER").ok().filter(|v| !v.is_empty())?;
        let http = reqwest::Client::builder()
            .timeout(Duration::from_secs(8))
            .build()
            .expect("reqwest client for titan");
        Some(Self { url, auth_header: auth, http })
    }
}

#[async_trait]
impl RelayBackend for TitanClient {
    async fn send_bundle(
        &self,
        bundle: &SignedBundle,
        _signer: &Signer,
    ) -> Result<RelayResponse, RelayError> {
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
            event = "titan.sending",
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

        debug!(event = "titan.response", status = status.as_u16());

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
        "titan"
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
