// M11 allow: `reqwest::Client::builder().build().expect(...)` is infallible on
// all supported platforms (Linux/macOS/Windows) unless TLS initialisation
// fails (which would also break every outbound HTTPS call in the process).
// There is no meaningful recovery path — if TLS cannot initialise, the relay
// client cannot function regardless. `expect` with a descriptive message is
// the correct pattern here; upgrading to returning `anyhow::Result` from `new`
// would force every call site to handle an error that cannot be acted on.
// Test-module unwraps are similarly justified.
#![allow(clippy::unwrap_used, clippy::expect_used)]
//! Flashbots relay client.
//!
//! Submits `eth_sendBundle` to the Flashbots Relay with X-Flashbots-Signature
//! auth header.
//!
//! `FlashbotsClient` also implements `RelayBackend` (BE-06) so it can be
//! composed into a `MultiRelayClient` alongside BloXRoute and Titan backends.
//!
//! BE-05: `call_bundle` performs `eth_callBundle` re-simulation against the
//! latest state. Used by `SubmitEngine` as a final sanity check between
//! `build_and_sign` and `multi_relay.broadcast`.

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
        let url = std::env::var("FLASHBOTS_RELAY_URL")
            .ok()
            .filter(|v| !v.is_empty())?;
        Some(Self::new(url, timeout))
    }

    pub fn new(url: String, timeout: Duration) -> Self {
        let http = reqwest::Client::builder()
            .timeout(timeout)
            .build()
            .expect("reqwest client");
        Self { url, http }
    }

    /// Shared helper: sign a JSON body and POST it to the relay URL.
    ///
    /// Both `send_bundle` and `call_bundle` produce the identical
    /// `X-Flashbots-Signature` auth header — this avoids duplication.
    async fn signed_post(&self, signer: &Signer, body_json: String) -> Result<String> {
        let auth = signer
            .flashbots_auth_header(body_json.as_bytes())
            .await
            .context("flashbots auth")?;
        let res = self
            .http
            .post(&self.url)
            .header("content-type", "application/json")
            .header("X-Flashbots-Signature", auth)
            .body(body_json)
            .send()
            .await
            .context("http post to flashbots")?;

        let status = res.status();
        let text = res.text().await.context("read flashbots response body")?;
        debug!(event = "flashbots.response", status = status.as_u16());

        if !status.is_success() {
            anyhow::bail!("flashbots http {}: {}", status.as_u16(), text);
        }
        Ok(text)
    }

    /// Returns the Flashbots response with bundleHash on success.
    pub async fn send_bundle(
        &self,
        signer: &Signer,
        bundle: &SignedBundle,
    ) -> Result<BundleResponse> {
        let body = BundleRequest::from(bundle);
        let body_json = serde_json::to_string(&body).context("serialize bundle")?;
        let text = self.signed_post(signer, body_json).await?;
        let parsed: BundleResponse = serde_json::from_str(&text)
            .with_context(|| format!("parse flashbots response: {}", text))?;
        Ok(parsed)
    }

    /// BE-05: simulate the bundle against the latest chain state via
    /// `eth_callBundle` before committing to a live broadcast.
    ///
    /// `target_block` should be `current_block + 1` (the block the bundle
    /// is intended for). `stateBlockNumber` is always `"latest"` so the
    /// simulation reflects the freshest available state.
    ///
    /// # Note on race windows
    /// This is a CHECK, not a guarantee. Between the simulation and the builder
    /// picking up the bundle, additional blocks may land. Use this to catch
    /// obvious failures (reverted routes, slippage exceeded) which account for
    /// the majority of preventable losses — not as proof of inclusion.
    pub async fn call_bundle(
        &self,
        signer: &Signer,
        signed_tx_hex: &str,
        target_block: u64,
    ) -> Result<CallBundleResult> {
        let payload = serde_json::to_string(&CallBundleRequest {
            jsonrpc: "2.0",
            id: 1,
            method: "eth_callBundle",
            params: vec![CallBundleParams {
                txs: vec![signed_tx_hex.to_owned()],
                block_number: format!("0x{:x}", target_block),
                state_block_number: "latest".to_owned(),
            }],
        })
        .context("serialize eth_callBundle request")?;

        let text = self.signed_post(signer, payload).await?;

        let raw: CallBundleResponse = serde_json::from_str(&text)
            .with_context(|| format!("parse eth_callBundle response: {}", text))?;

        if let Some(rpc_err) = raw.error {
            anyhow::bail!(
                "eth_callBundle rpc error code={} msg={}",
                rpc_err.code,
                rpc_err.message
            );
        }

        let result = raw
            .result
            .ok_or_else(|| anyhow::anyhow!("eth_callBundle returned null result"))?;

        // coinbaseDiff is a hex string, e.g. "0x1a2b3c".
        let coinbase_diff_wei = parse_hex_u128(&result.coinbase_diff).unwrap_or(0);

        let tx_results = result
            .results
            .into_iter()
            .map(|r| TxCallResult {
                tx_hash: r.tx_hash.unwrap_or_default(),
                gas_used: r.gas_used,
                error: r.error,
                revert: r.revert,
            })
            .collect();

        Ok(CallBundleResult {
            bundle_hash: result.bundle_hash,
            coinbase_diff_wei,
            total_gas_used: result.total_gas_used,
            tx_results,
        })
    }
}

// ─── call_bundle types ───────────────────────────────────────────────────────

/// Parsed result of an `eth_callBundle` simulation (BE-05).
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct CallBundleResult {
    pub bundle_hash: Option<String>,
    /// Net coinbase payment (ETH-denominated wei). Reflects bundle profit to
    /// the builder — a proxy for expected profit, not the searcher's net P&L.
    pub coinbase_diff_wei: u128,
    pub total_gas_used: u64,
    pub tx_results: Vec<TxCallResult>,
}

impl CallBundleResult {
    /// Returns `true` when at least one transaction in the bundle would revert.
    ///
    /// The Flashbots relay populates either `error` (execution error) or
    /// `revert` (require/revert string) on failed transactions. A single
    /// failure means the entire atomic bundle would fail on-chain.
    pub fn any_failed(&self) -> bool {
        self.tx_results
            .iter()
            .any(|t| t.error.is_some() || t.revert.is_some())
    }
}

/// Per-transaction result from `eth_callBundle`.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct TxCallResult {
    pub tx_hash: String,
    pub gas_used: u64,
    /// Non-null when the tx hit an execution error (e.g. out of gas, invalid opcode).
    pub error: Option<String>,
    /// Non-null when the tx hit a Solidity `require`/`revert` with a reason string.
    pub revert: Option<String>,
}

// ─── serde types for eth_callBundle ─────────────────────────────────────────

#[derive(Debug, Serialize)]
struct CallBundleRequest {
    jsonrpc: &'static str,
    id: u64,
    method: &'static str,
    params: Vec<CallBundleParams>,
}

#[derive(Debug, Serialize)]
struct CallBundleParams {
    txs: Vec<String>,
    #[serde(rename = "blockNumber")]
    block_number: String,
    #[serde(rename = "stateBlockNumber")]
    state_block_number: String,
}

#[derive(Debug, Deserialize)]
struct CallBundleResponse {
    #[serde(default)]
    result: Option<CallBundleResultRaw>,
    #[serde(default)]
    error: Option<RpcError>,
}

#[derive(Debug, Deserialize)]
struct CallBundleResultRaw {
    #[serde(rename = "bundleHash", default)]
    bundle_hash: Option<String>,
    /// Hex string, e.g. "0x1a2b3c".
    #[serde(rename = "coinbaseDiff", default)]
    coinbase_diff: String,
    #[serde(rename = "totalGasUsed", default)]
    total_gas_used: u64,
    #[serde(default)]
    results: Vec<TxCallResultRaw>,
}

#[derive(Debug, Deserialize)]
struct TxCallResultRaw {
    #[serde(rename = "txHash", default)]
    tx_hash: Option<String>,
    #[serde(rename = "gasUsed", default)]
    gas_used: u64,
    #[serde(default)]
    error: Option<String>,
    #[serde(default)]
    revert: Option<String>,
}

/// Parse a `"0x…"` hex string into `u128`. Returns `None` on malformed input.
fn parse_hex_u128(s: &str) -> Option<u128> {
    let trimmed = s.trim_start_matches("0x").trim_start_matches("0X");
    // Empty string after stripping prefix (e.g. "" or bare "0x") represents 0.
    if trimmed.is_empty() {
        return Some(0u128);
    }
    u128::from_str_radix(trimmed, 16).ok()
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

// ─── Unit tests ──────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    /// Happy path: all fields present, no error/revert in any tx.
    /// Verifies coinbaseDiff hex parsing and any_failed() == false.
    #[test]
    fn test_call_bundle_response_parsing_success() {
        let json = r#"{
            "result": {
                "bundleHash": "0xabc123",
                "coinbaseDiff": "0x2386f26fc10000",
                "totalGasUsed": 210000,
                "results": [
                    {
                        "txHash": "0xdeadbeef",
                        "gasUsed": 210000,
                        "error": null,
                        "revert": null
                    }
                ]
            }
        }"#;

        let raw: CallBundleResponse = serde_json::from_str(json).expect("parse failed");
        let result_raw = raw.result.expect("missing result");

        let coinbase = parse_hex_u128(&result_raw.coinbase_diff).unwrap_or(0);
        // 0x2386f26fc10000 = 10_000_000_000_000_000 (0.01 ETH)
        assert_eq!(coinbase, 10_000_000_000_000_000u128);
        assert_eq!(result_raw.total_gas_used, 210_000);
        assert_eq!(result_raw.bundle_hash.as_deref(), Some("0xabc123"));

        let tx = &result_raw.results[0];
        assert_eq!(tx.gas_used, 210_000);
        assert!(tx.error.is_none());
        assert!(tx.revert.is_none());

        // Reconstruct into public type and check any_failed().
        let call_result = CallBundleResult {
            bundle_hash: result_raw.bundle_hash,
            coinbase_diff_wei: coinbase,
            total_gas_used: result_raw.total_gas_used,
            tx_results: result_raw
                .results
                .into_iter()
                .map(|r| TxCallResult {
                    tx_hash: r.tx_hash.unwrap_or_default(),
                    gas_used: r.gas_used,
                    error: r.error,
                    revert: r.revert,
                })
                .collect(),
        };
        assert!(
            !call_result.any_failed(),
            "success path must not flag any_failed"
        );
    }

    /// Revert path: tx has a non-null `revert` field.
    /// any_failed() must return true so the engine aborts broadcast.
    #[test]
    fn test_call_bundle_response_parsing_revert() {
        let json = r#"{
            "result": {
                "bundleHash": null,
                "coinbaseDiff": "0x0",
                "totalGasUsed": 50000,
                "results": [
                    {
                        "txHash": "0xdeadbeef",
                        "gasUsed": 50000,
                        "error": null,
                        "revert": "execution reverted: insufficient output amount"
                    }
                ]
            }
        }"#;

        let raw: CallBundleResponse = serde_json::from_str(json).expect("parse failed");
        let result_raw = raw.result.expect("missing result");

        let call_result = CallBundleResult {
            bundle_hash: result_raw.bundle_hash,
            coinbase_diff_wei: parse_hex_u128(&result_raw.coinbase_diff).unwrap_or(0),
            total_gas_used: result_raw.total_gas_used,
            tx_results: result_raw
                .results
                .into_iter()
                .map(|r| TxCallResult {
                    tx_hash: r.tx_hash.unwrap_or_default(),
                    gas_used: r.gas_used,
                    error: r.error,
                    revert: r.revert,
                })
                .collect(),
        };

        assert!(call_result.any_failed(), "revert path must flag any_failed");
        let reason = call_result.tx_results[0].revert.as_deref().unwrap_or("");
        assert!(
            reason.contains("insufficient output"),
            "revert reason not propagated: {reason}"
        );
    }

    /// Error path: tx has a non-null `error` field (e.g. out of gas).
    /// any_failed() must return true.
    #[test]
    fn test_call_bundle_response_parsing_error() {
        let json = r#"{
            "result": {
                "bundleHash": null,
                "coinbaseDiff": "0x0",
                "totalGasUsed": 500000,
                "results": [
                    {
                        "txHash": "0xdeadbeef",
                        "gasUsed": 500000,
                        "error": "out of gas",
                        "revert": null
                    }
                ]
            }
        }"#;

        let raw: CallBundleResponse = serde_json::from_str(json).expect("parse failed");
        let result_raw = raw.result.expect("missing result");

        let call_result = CallBundleResult {
            bundle_hash: result_raw.bundle_hash,
            coinbase_diff_wei: parse_hex_u128(&result_raw.coinbase_diff).unwrap_or(0),
            total_gas_used: result_raw.total_gas_used,
            tx_results: result_raw
                .results
                .into_iter()
                .map(|r| TxCallResult {
                    tx_hash: r.tx_hash.unwrap_or_default(),
                    gas_used: r.gas_used,
                    error: r.error,
                    revert: r.revert,
                })
                .collect(),
        };

        assert!(call_result.any_failed(), "error path must flag any_failed");
        let err_msg = call_result.tx_results[0].error.as_deref().unwrap_or("");
        assert!(
            err_msg.contains("out of gas"),
            "error message not propagated: {err_msg}"
        );
    }

    /// parse_hex_u128 handles well-formed and degenerate inputs.
    #[test]
    fn test_parse_hex_u128() {
        assert_eq!(parse_hex_u128("0x0"), Some(0u128));
        assert_eq!(
            parse_hex_u128("0x2386f26fc10000"),
            Some(10_000_000_000_000_000u128)
        );
        assert_eq!(parse_hex_u128("0X1F"), Some(31u128));
        assert_eq!(parse_hex_u128(""), Some(0u128)); // empty hex = 0
        assert_eq!(parse_hex_u128("0xGG"), None); // invalid hex digits
    }

    // -----------------------------------------------------------------------
    // Staging verification (ignored — live network call)
    // -----------------------------------------------------------------------

    /// G-SIM-1 checklist item 6 — REAL staging verification of the
    /// `eth_callBundle` integration against the Flashbots simulate-bundle
    /// endpoint.
    ///
    /// `#[ignore]`d because it performs a LIVE network round-trip. Run with:
    ///
    /// ```bash
    /// FLASHBOTS_STAGING_RPC_URL=<bare-mainnet-rpc> \
    /// cargo test -p relays-client staging_callbundle -- --ignored --nocapture
    /// ```
    ///
    /// ## What it proves (and what it deliberately does NOT do)
    ///
    /// * REAL `eth_callBundle` request signed with an EPHEMERAL throwaway key
    ///   (the anvil test-account #0 vector — never an operator wallet), for a
    ///   zero-value, zero-gas self-transfer signed by that throwaway key.
    ///   `eth_callBundle` only SIMULATES: nothing is broadcast, no capital is
    ///   exposed (paper-shadow doctrine §32).
    /// * The response parses through the production `FlashbotsClient::
    ///   call_bundle` path, carries `totalGasUsed > 0`, no tx-level
    ///   error/revert, and the derived `bundleGasPrice` (coinbaseDiff /
    ///   totalGasUsed, gwei) is within the configured staging limit
    ///   (`ARBX_STAGING_MAX_BUNDLE_GAS_PRICE_GWEI`, default 500).
    /// * Missing env PANICS (fail-honest) — the staging check can never be
    ///   recorded from an unconfigured run.
    ///
    /// Emits the machine-greppable `ETH_CALLBUNDLE_STAGING_OUTCOME=PASS` marker
    /// + a JSON detail line; `sim-staging-callbundle.yml` records the registry
    /// row from them.
    #[tokio::test]
    #[ignore = "live network call — requires FLASHBOTS_STAGING_RPC_URL (bare mainnet RPC)"]
    async fn staging_callbundle_against_flashbots_simulate_endpoint() {
        use crate::signer::Signer;
        use ethers::prelude::{Http, Middleware, Provider};
        use ethers::signers::{LocalWallet, Signer as _};
        use ethers::types::transaction::eip2718::TypedTransaction;
        use ethers::types::TransactionRequest;

        let rpc_url = std::env::var("FLASHBOTS_STAGING_RPC_URL")
            .ok()
            .filter(|v| !v.trim().is_empty())
            .unwrap_or_else(|| {
                panic!(
                    "set FLASHBOTS_STAGING_RPC_URL to a single bare mainnet RPC URL — \
                     the staging check resolves the target block from the real tip"
                )
            });
        let relay_url = std::env::var("FLASHBOTS_STAGING_RELAY_URL")
            .ok()
            .filter(|v| !v.trim().is_empty())
            .unwrap_or_else(|| "https://relay.flashbots.net".to_string());
        let max_bundle_gas_price_gwei: f64 = std::env::var("ARBX_STAGING_MAX_BUNDLE_GAS_PRICE_GWEI")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(500.0);

        // Ephemeral throwaway signer (anvil test-account #0 vector — the SAME
        // convention signer.rs tests use; never an operator key).
        const EPHEMERAL_STAGING_KEY: &str =
            "ac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80";
        let wallet: LocalWallet = EPHEMERAL_STAGING_KEY
            .parse()
            .expect("ephemeral staging key parses");
        let wallet = wallet.with_chain_id(1u64);
        let signer = Signer {
            address: wallet.address(),
            wallet,
            chain_id: 1,
        };

        // Resolve the target block from the real mainnet tip (bundle targets
        // the next block, exactly as SubmitEngine does before broadcast).
        let provider = Provider::<Http>::try_from(rpc_url.trim())
            .expect("FLASHBOTS_STAGING_RPC_URL must be an HTTP endpoint");
        let tip = provider
            .get_block_number()
            .await
            .expect("eth_blockNumber against FLASHBOTS_STAGING_RPC_URL failed");
        let target_block = tip.as_u64() + 1;

        // Zero-value, zero-gas self-transfer: the read-only staging probe.
        // gas_price 0 keeps the unfunded throwaway sender balance-check-clean
        // inside the simulation; nothing here could ever cost anything.
        let req = TransactionRequest::new()
            .from(signer.address)
            .to(signer.address)
            .value(ethers::types::U256::zero())
            .gas(21_000u64)
            .gas_price(ethers::types::U256::zero())
            .nonce(0u64);
        let typed: TypedTransaction = req.into();
        let sig = signer
            .wallet
            .sign_transaction(&typed)
            .await
            .expect("sign staging probe tx with ephemeral key");
        let raw_hex = format!("0x{}", hex::encode(typed.rlp_signed(&sig)));

        // The PRODUCTION client + method (BE-05), unmodified.
        let client = FlashbotsClient::new(
            relay_url.clone(),
            std::time::Duration::from_secs(20),
        );
        let result = client
            .call_bundle(&signer, &raw_hex, target_block)
            .await
            .expect("eth_callBundle against the staging relay endpoint failed");

        assert!(
            result.total_gas_used > 0,
            "eth_callBundle returned totalGasUsed=0 — the simulation did not execute"
        );
        assert!(
            !result.any_failed(),
            "eth_callBundle flagged the probe tx failed: {:?}",
            result.tx_results
        );
        let bundle_gas_price_gwei =
            result.coinbase_diff_wei as f64 / result.total_gas_used as f64 / 1e9;
        assert!(
            bundle_gas_price_gwei <= max_bundle_gas_price_gwei,
            "bundleGasPrice {bundle_gas_price_gwei} gwei exceeds the staging limit {max_bundle_gas_price_gwei} gwei"
        );

        let detail = serde_json::json!({
            "method": "eth_callBundle",
            "relay": relay_url,
            "target_block": target_block,
            "tip_block": tip.as_u64(),
            "total_gas_used": result.total_gas_used,
            "coinbase_diff_wei": result.coinbase_diff_wei,
            "bundle_gas_price_gwei": bundle_gas_price_gwei,
            "max_bundle_gas_price_gwei": max_bundle_gas_price_gwei,
            "signer": "ephemeral-anvil-test-account (never an operator wallet)",
            "probe": "zero-value zero-gas self-transfer (simulate-only, no broadcast)",
        });
        println!(
            "ETH_CALLBUNDLE_STAGING_OUTCOME=PASS relay={relay_url} target_block={target_block} \
             total_gas_used={} bundle_gas_price_gwei={bundle_gas_price_gwei:.6}",
            result.total_gas_used
        );
        println!("ETH_CALLBUNDLE_STAGING_JSON={detail}");
    }
}
