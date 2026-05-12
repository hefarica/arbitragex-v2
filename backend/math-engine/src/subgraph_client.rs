//! Subgraph client for fetching Uniswap V3 tick distributions.
//!
//! Operator configures endpoints via environment:
//!   `ARBX_SUBGRAPH_URL_<chain_id>` — e.g. `ARBX_SUBGRAPH_URL_1` for Ethereum mainnet.
//!
//! R8 fail-honest: if the env var is absent or the query returns no pool,
//! the caller receives `None` and falls back to the Sprint-A first-order approximation.
//! Nothing is synthesised.

use anyhow::Result;
use serde::Deserialize;

// ---------------------------------------------------------------------------
// Public types
// ---------------------------------------------------------------------------

/// One tick entry as returned by the subgraph.
#[derive(Debug, Clone, Deserialize)]
pub struct TickInfo {
    /// Tick index (signed integer).
    #[serde(rename = "tickIdx")]
    pub tick_idx: i32,
    /// Net liquidity change when price crosses this tick (i128 serialised as decimal string).
    #[serde(rename = "liquidityNet")]
    pub liquidity_net: String,
    /// Total liquidity referencing this tick (u128 serialised as decimal string).
    #[serde(rename = "liquidityGross")]
    pub liquidity_gross: String,
}

/// Snapshot of a V3 pool's tick distribution, sufficient for tick-walking simulation.
#[derive(Debug, Clone)]
pub struct PoolTickDistribution {
    /// Pool address (lowercase hex).
    pub pool_addr: String,
    /// Current tick index (from `slot0`).
    pub current_tick: i32,
    /// Current sqrtPriceX96 (from `slot0`), as u128.
    pub sqrt_price_x96: u128,
    /// Active liquidity at current tick (uint128).
    pub liquidity: u128,
    /// Tick entries sorted ascending by `tick_idx`.
    pub ticks: Vec<TickInfo>,
}

// ---------------------------------------------------------------------------
// Endpoint resolution
// ---------------------------------------------------------------------------

/// Returns the configured subgraph URL for `chain_id`, or `None` if absent.
///
/// R8 fail-honest: callers that receive `None` must fall back to the proxy approximation.
pub fn subgraph_url_for_chain(chain_id: u64) -> Option<String> {
    std::env::var(format!("ARBX_SUBGRAPH_URL_{chain_id}")).ok()
}

// ---------------------------------------------------------------------------
// Tick fetcher
// ---------------------------------------------------------------------------

/// Fetch the tick distribution for a V3 `pool_addr` on `chain_id` from The Graph.
///
/// # Returns
/// - `Ok(Some(dist))` — subgraph responded with pool data.
/// - `Ok(None)` — env var absent OR subgraph returned no pool for this address (R8).
/// - `Err(e)` — network / parse failure (caller decides whether to propagate or demote to None).
///
/// # Subgraph query
/// ```graphql
/// {
///   pool(id: "<addr>") {
///     tick
///     sqrtPrice
///     liquidity
///     ticks(first: 200, orderBy: tickIdx) {
///       tickIdx
///       liquidityNet
///       liquidityGross
///     }
///   }
/// }
/// ```
pub async fn fetch_pool_ticks(
    chain_id: u64,
    pool_addr: &str,
) -> Result<Option<PoolTickDistribution>> {
    let Some(url) = subgraph_url_for_chain(chain_id) else {
        // R8: unconfigured → None, caller falls back to proxy.
        return Ok(None);
    };

    let gql_query = format!(
        r#"{{ pool(id: "{}") {{ tick sqrtPrice liquidity ticks(first: 200, orderBy: tickIdx) {{ tickIdx liquidityNet liquidityGross }} }} }}"#,
        pool_addr.to_lowercase()
    );
    let body = serde_json::json!({ "query": gql_query });

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()?;

    let resp = client.post(&url).json(&body).send().await?;

    if !resp.status().is_success() {
        return Err(anyhow::anyhow!(
            "subgraph HTTP {} for chain {}",
            resp.status(),
            chain_id
        ));
    }

    let body: serde_json::Value = resp.json().await?;

    let pool = match body.pointer("/data/pool") {
        Some(p) if !p.is_null() => p,
        _ => return Ok(None), // pool not indexed / wrong address → R8 None
    };

    let current_tick = pool["tick"]
        .as_str()
        .unwrap_or("0")
        .parse::<i32>()
        .unwrap_or(0);

    let sqrt_price_x96 = pool["sqrtPrice"]
        .as_str()
        .unwrap_or("0")
        .parse::<u128>()
        .unwrap_or(0);

    let liquidity = pool["liquidity"]
        .as_str()
        .unwrap_or("0")
        .parse::<u128>()
        .unwrap_or(0);

    // Subgraph ticks are already ordered by tickIdx (ascending) per the query.
    let ticks: Vec<TickInfo> = serde_json::from_value(pool["ticks"].clone()).unwrap_or_default();

    Ok(Some(PoolTickDistribution {
        pool_addr: pool_addr.to_lowercase(),
        current_tick,
        sqrt_price_x96,
        liquidity,
        ticks,
    }))
}

// ---------------------------------------------------------------------------
// Unit tests (sync — no network)
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_subgraph_url_absent_returns_none() {
        // This env var is not set in CI / unit test environments.
        let url = subgraph_url_for_chain(999_999);
        assert!(url.is_none(), "unconfigured chain must return None (R8)");
    }

    #[test]
    fn test_tick_info_deserializes_correctly() {
        // The Graph returns tickIdx as a JSON integer, liquidityNet/Gross as decimal strings
        // (int128 values exceed JS safe integer range so the subgraph serialises them as strings).
        let json = r#"{"tickIdx":100,"liquidityNet":"500000","liquidityGross":"1000000"}"#;
        let tick: TickInfo = serde_json::from_str(json).expect("deserialize TickInfo");
        assert_eq!(tick.tick_idx, 100);
        assert_eq!(tick.liquidity_net, "500000");
        assert_eq!(tick.liquidity_gross, "1000000");
    }

    #[test]
    fn test_tick_info_negative_liquidity_net_deserializes() {
        // liquidityNet can be negative (i128 as string).  tickIdx is a plain integer.
        let json = r#"{"tickIdx":-60,"liquidityNet":"-250000","liquidityGross":"250000"}"#;
        let tick: TickInfo = serde_json::from_str(json).expect("deserialize negative TickInfo");
        assert_eq!(tick.tick_idx, -60);
        assert_eq!(tick.liquidity_net, "-250000");
    }
}
