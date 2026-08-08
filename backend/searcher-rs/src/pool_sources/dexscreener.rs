//! DexScreener pool source — enrichment + enumeration via the free (no-key) API.
//!
//! ## Role in the fallback cycle (operator directive, 2026-08-07)
//! DexScreener is a HIGH-FIDELITY complement to the on-chain anchor. Unlike
//! DeFiLlama (whose yield IDs are mostly non-address) it returns real on-chain
//! `pairAddress`es with `liquidity.usd` and `volume.h24`, same quality tier as
//! GeckoTerminal. It is used two ways:
//!
//!   1. **Enumeration** — `/token-pairs/v1/{chainId}/{tokenAddress}` lists every
//!      pair indexed for a pivot token (WETH/USDC/USDT). Pivoting on the deepest
//!      tokens surfaces the high-volume pools that matter for closed-cycle
//!      detection without paging an unbounded "all pools" list (DexScreener has
//!      none). Every returned pair is a candidate.
//!   2. **Enrichment** — when the anchor (Alchemy) proposes a pool whose TVL is
//!      unknown (tvl=0), DexScreener's `/latest/dex/pairs/{chainId}/{pairAddress}`
//!      fills `liquidity.usd` + `volume.h24` so the worker ranks it correctly.
//!
//! ## Doctrine (RULE 00 / R8 / §33)
//! - Read-only HTTP, no key, no auth. Free tier tolerates bursts but throttles
//!   aggressive polling (HTTP 429) — the per-source circuit breaker + the tick
//!   spacing handle this.
//! - On-chain pool/token re-verification still happens at persistence (factory +
//!   token match anti-spoofing). This source only *proposes* candidates and
//!   *suggests* TVL/volume; the worker never trusts a DexScreener address blindly.
//! - Fail-honest: a missing field, a non-`0x` address, or a parse failure for a
//!   pair simply skips that pair — never a fabricated candidate.

use crate::pool_candidate::{PoolCandidate, PoolEnumSource};
use anyhow::Context;
use serde::Deserialize;
use std::time::Duration;
use tracing::warn;

/// Default DexScreener API base.
const DEFAULT_BASE: &str = "https://api.dexscreener.com";

/// Pivot tokens used for enumeration — the deepest, most-traded base/quote
/// assets. Enumerating pairs of these surfaces the pools that dominate volume
/// and closed-cycle opportunities. On chains where a token is absent the lookup
/// simply returns nothing (fail-honest). Addresses are lowercase canonical.
fn pivot_tokens(chain_id: u64) -> Vec<&'static str> {
    match chain_id {
        // Ethereum mainnet / L2s that share canonical WETH/USDC/USDT.
        1 | 10 | 42161 | 8453 => vec![
            "0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2", // WETH
            "0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48", // USDC
            "0xdac17f958d2ee523a2206206994597c13d831ec7", // USDT
        ],
        // Polygon: WMATIC/USDC/USDT.
        137 => vec![
            "0x0d500b1d8e8ef31e21c99d1db9a6444d3adf1270", // WMATIC
            "0x2791bca1f2de4661ed88a30c99a7a9449aa84174", // USDC
            "0xc2132d05d31c914a87c6611c10748aeb04b58e8f", // USDT
        ],
        _ => Vec::new(),
    }
}

/// Map an ArbitrageX numeric chain_id to DexScreener's network string id.
fn ds_chain(chain_id: u64) -> Option<&'static str> {
    match chain_id {
        1 => Some("ethereum"),
        10 => Some("optimism"),
        137 => Some("polygon"),
        8453 => Some("base"),
        42161 => Some("arbitrum"),
        _ => None,
    }
}

/// DexScreener pair shape (subset — only fields we consume).
#[derive(Deserialize, Default)]
struct DsPair {
    #[serde(default)]
    pair_address: String,
    #[serde(default)]
    base_token: DsToken,
    #[serde(default)]
    quote_token: DsToken,
    #[serde(default)]
    liquidity: DsLiquidity,
    #[serde(default)]
    volume: DsVolume,
}

#[derive(Deserialize, Default)]
struct DsToken {
    #[serde(default)]
    address: String,
}

#[derive(Deserialize, Default)]
struct DsLiquidity {
    #[serde(default)]
    usd: Option<f64>,
}

#[derive(Deserialize, Default)]
struct DsVolume {
    #[serde(default, rename = "h24")]
    h24: Option<f64>,
}

/// Enumerate pools by pivoting on deep tokens: list every pair indexed for each
/// pivot token on the chain. Returns TVL-ranked `PoolCandidate`s. Read-only.
pub async fn fetch(
    chain_id: u64,
    top_n: usize,
    min_tvl_usd: f64,
) -> anyhow::Result<Vec<PoolCandidate>> {
    let Some(net) = ds_chain(chain_id) else {
        return Ok(Vec::new()); // unsupported chain → contribute nothing
    };
    let base = std::env::var("DEXSCREENER_BASE_URL")
        .unwrap_or_else(|_| DEFAULT_BASE.to_string());
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(20))
        .user_agent("arbitragex-v2/searcher-rs (pool-enum; shadow)")
        .build()?;

    let mut out: Vec<PoolCandidate> = Vec::new();
    let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();
    let pivots = pivot_tokens(chain_id);
    for tok in &pivots {
        if out.len() >= top_n {
            break;
        }
        // /latest/dex/tokens/{tokenAddress} — returns `{"pairs": [...]}` with
        // pairAddress/baseToken/quoteToken/liquidity/volume per pair. (The
        // `/token-pairs/v1/{chain}/{token}` path 404s for these pivots; the
        // tokens endpoint is the reliable, documented one.)
        let url = format!("{base}/latest/dex/tokens/{tok}");
        let resp = match client.get(&url).header("accept", "application/json").send().await {
            Ok(r) => r,
            Err(e) => {
                warn!(
                    event = "poolenum.dexscreener.fetch_err",
                    chain_id, pivot = tok, error = %e,
                    "DexScreener tokens GET failed — skipping pivot (fail-honest)"
                );
                continue;
            }
        };
        if !resp.status().is_success() {
            warn!(
                event = "poolenum.dexscreener.http",
                chain_id,
                pivot = tok,
                status = resp.status().as_u16(),
                "DexScreener non-2xx — skipping pivot (circuit handles backoff)"
            );
            // Surface the HTTP error so the per-source circuit breaker can count it.
            anyhow::bail!("dexscreener HTTP {}", resp.status());
        }
        #[derive(Deserialize)]
        struct TokenResp {
            #[serde(default)]
            pairs: Vec<DsPair>,
        }
        let parsed: TokenResp = match resp.json().await {
            Ok(v) => v,
            Err(e) => {
                warn!(event = "poolenum.dexscreener.parse_err", chain_id, pivot = tok, error = %e);
                continue;
            }
        };
        for p in parsed.pairs {
            if out.len() >= top_n {
                break;
            }
            let addr = p.pair_address.trim().to_ascii_lowercase();
            if !addr.starts_with("0x") || addr.len() != 42 || !seen.insert(addr.clone()) {
                continue;
            }
            let tvl = p.liquidity.usd.filter(|v| v.is_finite()).unwrap_or(0.0);
            if tvl > 0.0 && tvl < min_tvl_usd {
                continue;
            }
            out.push(PoolCandidate {
                address: addr,
                token0: norm_token(&p.base_token.address),
                token1: norm_token(&p.quote_token.address),
                fee_bps: None, // resolved on-chain during hydration
                tvl_usd: tvl,
                volume_usd_24h: p.volume.h24.filter(|v| v.is_finite()),
                source: PoolEnumSource::DexScreener,
            });
        }
    }
    Ok(out)
}

/// Enrich a single pool's TVL/volume from DexScreener's pair lookup. Returns
/// `(tvl_usd, volume_usd_24h)` or `None` when the pool is not indexed / lookup
/// fails (fail-honest — caller keeps whatever it already had).
///
/// `/latest/dex/pairs/{chainId}/{pairAddress}` returns a `{"pairs": [...]}` object.
pub async fn enrich_pool(
    chain_id: u64,
    pool_address: &str,
) -> anyhow::Result<Option<(f64, Option<f64>)>> {
    let Some(net) = ds_chain(chain_id) else {
        return Ok(None);
    };
    let base = std::env::var("DEXSCREENER_BASE_URL")
        .unwrap_or_else(|_| DEFAULT_BASE.to_string());
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(15))
        .user_agent("arbitragex-v2/searcher-rs (enrich; shadow)")
        .build()?;
    let url = format!("{base}/latest/dex/pairs/{net}/{pool_address}");
    let resp = client
        .get(&url)
        .header("accept", "application/json")
        .send()
        .await
        .context("dexscreener enrich GET")?;
    if !resp.status().is_success() {
        return Ok(None); // not indexed / rate-limited → leave existing values
    }
    #[derive(Deserialize)]
    struct Wrap {
        #[serde(default)]
        pairs: Vec<DsPair>,
    }
    let parsed: Wrap = resp.json().await.context("dexscreener enrich parse")?;
    let p = match parsed.pairs.into_iter().next() {
        Some(p) => p,
        None => return Ok(None),
    };
    let tvl = p.liquidity.usd.filter(|v| v.is_finite()).unwrap_or(0.0);
    let vol = p.volume.h24.filter(|v| v.is_finite());
    Ok(Some((tvl, vol)))
}

fn norm_token(s: &str) -> Option<String> {
    let t = s.trim();
    if t.is_empty() {
        None
    } else {
        Some(t.to_ascii_lowercase())
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn ds_chain_maps_known() {
        assert_eq!(ds_chain(1), Some("ethereum"));
        assert_eq!(ds_chain(42161), Some("arbitrum"));
        assert_eq!(ds_chain(999), None);
    }

    #[test]
    fn pivot_tokens_nonempty_on_known_chains() {
        assert!(!pivot_tokens(1).is_empty());
        assert!(!pivot_tokens(137).is_empty());
        assert!(pivot_tokens(999).is_empty());
    }

    #[test]
    fn norm_token_trims_and_lowercases() {
        assert_eq!(norm_token("  0xABC  "), Some("0xabc".into()));
        assert_eq!(norm_token(""), None);
    }
}
