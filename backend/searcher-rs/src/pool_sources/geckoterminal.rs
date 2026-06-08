//! GeckoTerminal pool source — top pools by 24h volume per network
//! (`GECKOTERMINAL_BASE_URL`, default https://api.geckoterminal.com/api/v2).
//!
//! Free tier ~30 req/min → pages are spaced ~2.1s. Token addresses come from the
//! JSON:API `relationships.base_token/quote_token` ids (form `<network>_<addr>`).
//! Read-only; the worker re-verifies every pool on-chain before persisting.

use crate::pool_candidate::{PoolCandidate, PoolEnumSource};
use anyhow::Context;
use serde::Deserialize;

#[derive(Deserialize)]
struct Resp {
    #[serde(default)]
    data: Vec<Pool>,
}

#[derive(Deserialize)]
struct Pool {
    #[serde(default)]
    attributes: Attrs,
    #[serde(default)]
    relationships: Rels,
}

#[derive(Deserialize, Default)]
struct Attrs {
    #[serde(default)]
    address: String,
    #[serde(default)]
    reserve_in_usd: Option<String>,
    #[serde(default)]
    volume_usd: Option<Vol>,
}

#[derive(Deserialize, Default)]
struct Vol {
    #[serde(default)]
    h24: Option<String>,
}

#[derive(Deserialize, Default)]
struct Rels {
    #[serde(default)]
    base_token: Option<RelData>,
    #[serde(default)]
    quote_token: Option<RelData>,
}

#[derive(Deserialize, Default)]
struct RelData {
    #[serde(default)]
    data: Option<RelId>,
}

#[derive(Deserialize, Default)]
struct RelId {
    #[serde(default)]
    id: String,
}

pub async fn fetch(
    chain_id: u64,
    top_n: usize,
    min_tvl_usd: f64,
) -> anyhow::Result<Vec<PoolCandidate>> {
    let Some(net) = gt_network(chain_id) else {
        return Ok(Vec::new()); // unsupported chain → contribute nothing
    };
    let base = std::env::var("GECKOTERMINAL_BASE_URL")
        .unwrap_or_else(|_| "https://api.geckoterminal.com/api/v2".to_string());
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(20))
        .user_agent("arbitragex-v2/searcher-rs (pool-enum; shadow)")
        .build()?;

    let mut out: Vec<PoolCandidate> = Vec::new();
    let pages = top_n.div_ceil(20).clamp(1, 10); // 20 pools/page, cap 10 pages
    for page in 1..=pages {
        let url = format!("{base}/networks/{net}/pools?page={page}&sort=h24_volume_usd_desc");
        let resp = client
            .get(&url)
            .header("accept", "application/json")
            .send()
            .await
            .context("geckoterminal GET")?;
        if !resp.status().is_success() {
            anyhow::bail!("geckoterminal HTTP {}", resp.status());
        }
        let parsed: Resp = resp.json().await.context("geckoterminal parse")?;
        if parsed.data.is_empty() {
            break;
        }
        for p in parsed.data {
            let addr = p.attributes.address.trim().to_ascii_lowercase();
            if !addr.starts_with("0x") || addr.len() != 42 {
                continue;
            }
            let tvl = p
                .attributes
                .reserve_in_usd
                .as_deref()
                .and_then(|s| s.parse::<f64>().ok())
                .unwrap_or(0.0);
            if !(tvl.is_finite() && tvl >= min_tvl_usd) {
                continue;
            }
            let vol = p
                .attributes
                .volume_usd
                .and_then(|v| v.h24)
                .and_then(|s| s.parse::<f64>().ok())
                .filter(|v| v.is_finite());
            let t0 = p
                .relationships
                .base_token
                .and_then(|r| r.data)
                .and_then(|d| rel_addr(&d.id));
            let t1 = p
                .relationships
                .quote_token
                .and_then(|r| r.data)
                .and_then(|d| rel_addr(&d.id));
            out.push(PoolCandidate {
                address: addr,
                token0: t0,
                token1: t1,
                fee_bps: None, // resolved on-chain
                tvl_usd: tvl,
                volume_usd_24h: vol,
                source: PoolEnumSource::GeckoTerminal,
            });
            if out.len() >= top_n {
                break;
            }
        }
        if out.len() >= top_n {
            break;
        }
        // Free tier ~30 req/min → space pages.
        tokio::time::sleep(std::time::Duration::from_millis(2_100)).await;
    }
    Ok(out)
}

/// GeckoTerminal token relationship id is `<network>_<address>`; take the address.
fn rel_addr(id: &str) -> Option<String> {
    let a = id.rsplit_once('_').map(|(_, a)| a).unwrap_or(id).trim();
    if a.starts_with("0x") && a.len() == 42 {
        Some(a.to_ascii_lowercase())
    } else {
        None
    }
}

fn gt_network(chain_id: u64) -> Option<&'static str> {
    Some(match chain_id {
        1 => "eth",
        56 => "bsc",
        137 => "polygon_pos",
        42161 => "arbitrum",
        10 => "optimism",
        8453 => "base",
        43114 => "avax",
        _ => return None,
    })
}
