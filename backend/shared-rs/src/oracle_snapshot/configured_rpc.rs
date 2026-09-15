//! Bounded selection from the existing named RPC registry. Once selected, a
//! provider is fixed for the whole analysis: EVM and oracle reads share it.
//! This module only probes chain identity; it never signs or broadcasts.
use super::OracleRpc;
use anyhow::{anyhow, ensure, Result};
use serde_json::json;
use std::collections::{BTreeSet, VecDeque};
use std::time::Duration;
use tokio::task::JoinSet;

const MAX_PROVIDERS: usize = 32;
const MAX_IN_FLIGHT: usize = 3;
const HEDGE_DELAY: Duration = Duration::from_millis(150);
const PROBE_TIMEOUT: Duration = Duration::from_secs(2);
const SELECTION_TIMEOUT: Duration = Duration::from_secs(8);

fn configured_urls(raw: &str) -> Result<VecDeque<String>> {
    ensure!(raw.len() <= 65536, "oracle_rpc_config_too_large");
    let entries =
        crate::rpc_failover::parse_csv(raw).map_err(|_| anyhow!("oracle_rpc_config_invalid"))?;
    let mut seen = BTreeSet::new();
    let mut urls = VecDeque::new();
    for (_, url) in entries {
        let Ok(parsed) = reqwest::Url::parse(&url) else {
            continue;
        };
        if !matches!(parsed.scheme(), "http" | "https")
            || parsed.host_str().is_none()
            || parsed.fragment().is_some()
        {
            continue;
        }
        if seen.insert(parsed.as_str().to_owned()) {
            urls.push_back(parsed.to_string());
        }
    }
    ensure!(!urls.is_empty(), "oracle_rpc_no_http_provider");
    ensure!(urls.len() <= MAX_PROVIDERS, "oracle_rpc_provider_limit");
    Ok(urls)
}

fn chain_id(value: &serde_json::Value) -> Result<u64> {
    let digits = value
        .as_str()
        .and_then(|v| v.strip_prefix("0x"))
        .ok_or_else(|| anyhow!("oracle_rpc_chain_id_invalid"))?;
    ensure!(
        !digits.is_empty()
            && digits.len() <= 16
            && digits.bytes().all(|c| c.is_ascii_hexdigit())
            && (digits.len() == 1 || !digits.starts_with('0')),
        "oracle_rpc_chain_id_invalid"
    );
    u64::from_str_radix(digits, 16).map_err(|_| anyhow!("oracle_rpc_chain_id_invalid"))
}

async fn probe(url: String, chain: u64) -> Result<OracleRpc> {
    let rpc = OracleRpc::from_url(&url)?;
    let value = tokio::time::timeout(PROBE_TIMEOUT, rpc.call("eth_chainId", json!([])))
        .await
        .map_err(|_| anyhow!("oracle_rpc_probe_timeout"))??;
    ensure!(chain_id(&value)? == chain, "oracle_rpc_chain_mismatch");
    Ok(rpc)
}

async fn select(mut urls: VecDeque<String>, chain: u64) -> Result<OracleRpc> {
    let mut probes = JoinSet::new();
    // Start only the preferred provider. Hedge slowly, at most three requests
    // in flight, rather than fan out to every configured vendor per candidate.
    if let Some(url) = urls.pop_front() {
        probes.spawn(probe(url, chain));
    }
    let mut hedge = tokio::time::interval(HEDGE_DELAY);
    hedge.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    hedge.tick().await;
    loop {
        tokio::select! {
            result = probes.join_next() => {
                if let Some(Ok(Ok(rpc))) = result {
                    // Dropping the JoinSet also cancels pending HTTP probes.
                    probes.abort_all();
                    return Ok(rpc);
                }
                if let Some(url) = urls.pop_front() { probes.spawn(probe(url, chain)); }
                ensure!(!probes.is_empty(), "oracle_rpc_no_verified_provider");
            }
            _ = hedge.tick(), if !urls.is_empty() && probes.len() < MAX_IN_FLIGHT => {
                if let Some(url) = urls.pop_front() { probes.spawn(probe(url, chain)); }
            }
        }
    }
}

impl OracleRpc {
    /// Configuration-only entry point (also used by isolated integration tests).
    /// Network identity must match the requested chain before returning a client.
    /// Failed selection is not cached, so a recovered provider can be retried.
    pub async fn from_config(chain: u64, raw: &str) -> Result<Self> {
        ensure!(chain > 0, "oracle_rpc_chain_invalid");
        let urls = configured_urls(raw)?;
        tokio::time::timeout(SELECTION_TIMEOUT, select(urls, chain))
            .await
            .map_err(|_| anyhow!("oracle_rpc_selection_timeout"))?
    }

    /// Internal hand-off to SimulatorV2. Contains credentials: never log this.
    pub fn endpoint(&self) -> &str {
        &self.url
    }
}
