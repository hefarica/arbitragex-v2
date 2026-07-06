//! DeFiLlama pool source — top pools by TVL from the yields API
//! (`DEFILLAMA_BASE_URL`/pools, default https://yields.llama.fi).
//!
//! Best-effort: DeFiLlama's `pool` id is only SOMETIMES an on-chain address, so
//! we keep the 0x-address-shaped rows and let the worker's on-chain factory +
//! token0/1 re-verification reject anything bogus (fail-honest — never trusted
//! blindly). `underlyingTokens` provides the token hints when present.

use crate::pool_candidate::{PoolCandidate, PoolEnumSource};
use anyhow::Context;
use serde::Deserialize;

#[derive(Deserialize)]
struct Resp {
    #[serde(default)]
    data: Vec<Row>,
}

#[derive(Deserialize)]
struct Row {
    #[serde(default)]
    chain: String,
    #[serde(default)]
    pool: String,
    #[serde(rename = "tvlUsd", default)]
    tvl_usd: Option<f64>,
    #[serde(rename = "volumeUsd1d", default)]
    volume_usd_1d: Option<f64>,
    #[serde(rename = "underlyingTokens", default)]
    underlying_tokens: Option<Vec<String>>,
}

pub async fn fetch(
    chain_id: u64,
    top_n: usize,
    min_tvl_usd: f64,
) -> anyhow::Result<Vec<PoolCandidate>> {
    let Some(chain_name) = llama_chain(chain_id) else {
        return Ok(Vec::new()); // unsupported chain → contribute nothing
    };
    let base = std::env::var("DEFILLAMA_BASE_URL")
        .unwrap_or_else(|_| "https://yields.llama.fi".to_string());
    let url = format!("{base}/pools");

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(20))
        .user_agent("arbitragex-v2/searcher-rs (pool-enum; shadow)")
        .build()?;
    let resp = client
        .get(&url)
        .header("accept", "application/json")
        .send()
        .await
        .context("defillama GET")?;
    if !resp.status().is_success() {
        anyhow::bail!("defillama HTTP {}", resp.status());
    }
    let parsed: Resp = resp.json().await.context("defillama parse")?;

    let mut out = Vec::new();
    for r in parsed.data {
        if !r.chain.eq_ignore_ascii_case(chain_name) {
            continue;
        }
        let addr = r.pool.trim().to_ascii_lowercase();
        // Keep only on-chain-address-shaped ids (0x + 40 hex); the rest are
        // DeFiLlama-internal yield ids we can't enumerate.
        if !addr.starts_with("0x") || addr.len() != 42 {
            continue;
        }
        let tvl = r.tvl_usd.unwrap_or(0.0);
        if !(tvl.is_finite() && tvl >= min_tvl_usd) {
            continue;
        }
        let toks = r.underlying_tokens.unwrap_or_default();
        out.push(PoolCandidate {
            address: addr,
            token0: toks.first().map(|t| t.to_ascii_lowercase()),
            token1: toks.get(1).map(|t| t.to_ascii_lowercase()),
            fee_bps: None, // resolved on-chain
            tvl_usd: tvl,
            volume_usd_24h: r.volume_usd_1d.filter(|v| v.is_finite()),
            source: PoolEnumSource::DeFiLlama,
        });
        if out.len() >= top_n {
            break;
        }
    }
    Ok(out)
}

/// DeFiLlama's `chain` field is a capitalised name. Unknown chain → `None`.
fn llama_chain(chain_id: u64) -> Option<&'static str> {
    Some(match chain_id {
        1 => "Ethereum",
        56 => "BSC",
        137 => "Polygon",
        42161 => "Arbitrum",
        10 => "Optimism",
        8453 => "Base",
        43114 => "Avalanche",
        _ => return None,
    })
}
