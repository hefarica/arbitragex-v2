//! Trust Wallet asset URL builder + HEAD verifier.
//!
//! Builds the canonical Trust Wallet logo URL using EIP-55 checksum casing
//! and verifies asset existence via a HEAD request against the GitHub raw CDN.
//!
//! Honors R8 fail-honest: returns `None` when the asset cannot be confirmed
//! (404 or rate-limited) rather than fabricating a URL we cannot guarantee.

use alloy_primitives::Address;
use anyhow::Result;
use reqwest::Client;
use std::time::Duration;
use tracing::{debug, warn};

/// Map an EVM chain id to the directory segment Trust Wallet uses under
/// `blockchains/<path>/assets/`. Returns `None` for unsupported chains.
pub fn chain_path(chain_id: u64) -> Option<&'static str> {
    match chain_id {
        1     => Some("ethereum"),
        42161 => Some("arbitrum"),
        10    => Some("optimism"),
        8453  => Some("base"),
        137   => Some("polygon"),
        56    => Some("smartchain"),
        _     => None,
    }
}

/// Build the Trust Wallet logo URL using EIP-55 checksum casing.
/// Returns `None` for unsupported chains.
pub fn checksum_url_for(chain_id: u64, address: Address) -> Option<String> {
    let path = chain_path(chain_id)?;
    // alloy_primitives::Address::to_checksum(None) emits canonical EIP-55
    // (not EIP-1191 chain-specific) — exactly what Trust Wallet expects.
    let checksum = address.to_checksum(None);
    Some(format!(
        "https://raw.githubusercontent.com/trustwallet/assets/master/blockchains/{path}/assets/{checksum}/logo.png"
    ))
}

/// HTTP client that verifies Trust Wallet logo URLs via HEAD request.
pub struct TrustWalletClient {
    http: Client,
    auth_token: Option<String>,
}

impl TrustWalletClient {
    pub fn new(github_token: Option<String>) -> Result<Self> {
        let http = Client::builder().timeout(Duration::from_secs(8)).build()?;
        Ok(Self { http, auth_token: github_token })
    }

    /// Returns `Some(url)` if the asset exists (HEAD 200). `None` on 404 or rate-limit.
    /// Errs only on network failure.
    pub async fn verify(&self, chain_id: u64, address: Address) -> Result<Option<String>> {
        let Some(url) = checksum_url_for(chain_id, address) else { return Ok(None); };
        let mut req = self.http.head(&url);
        if let Some(t) = &self.auth_token {
            req = req.header("authorization", format!("Bearer {t}"));
        }
        let resp = req.send().await?;
        match resp.status().as_u16() {
            200 => Ok(Some(url)),
            404 => {
                debug!(event = "trustwallet.not_found", %url);
                Ok(None)
            }
            403 => {
                let reset = resp
                    .headers()
                    .get("x-ratelimit-reset")
                    .and_then(|v| v.to_str().ok())
                    .unwrap_or("?");
                warn!(event = "trustwallet.rate_limited", reset = %reset);
                // Treat as not-found for now; reconciliation will retry.
                Ok(None)
            }
            other => {
                warn!(event = "trustwallet.unexpected_status", status = other);
                Ok(None)
            }
        }
    }
}
