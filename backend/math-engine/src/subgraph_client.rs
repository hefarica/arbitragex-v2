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
// Top-N pool ranking by TVL (proactive coverage — Step 3)
// ---------------------------------------------------------------------------

/// Which subgraph entity to rank: Uniswap-V3 `pools` (by `totalValueLockedUSD`)
/// or Uniswap-V2 `pairs` (by `reserveUSD`). The two share an otherwise identical
/// row shape, so one ranker serves both.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SubgraphPoolKind {
    /// Uniswap-V3 concentrated-liquidity `pools` entity.
    V3,
    /// Uniswap-V2 constant-product `pairs` entity.
    V2,
}

impl SubgraphPoolKind {
    /// Root collection field (`pools` / `pairs`).
    fn entity(self) -> &'static str {
        match self {
            SubgraphPoolKind::V3 => "pools",
            SubgraphPoolKind::V2 => "pairs",
        }
    }
    /// TVL field used for ordering / thresholding.
    fn tvl_field(self) -> &'static str {
        match self {
            SubgraphPoolKind::V3 => "totalValueLockedUSD",
            SubgraphPoolKind::V2 => "reserveUSD",
        }
    }
}

/// One pool/pair ranked by TVL, with just enough metadata for the enumeration
/// worker to dedup, resolve the factory, run the token-safety screen, and hand
/// off to on-chain hydration. **Nothing here is trusted blindly** — the address
/// is re-verified on-chain by `PoolDiscoveryService.hydrate_and_persist_pool`
/// before any pool is activated (the subgraph is a *candidate source*, not truth).
#[derive(Debug, Clone)]
pub struct RankedPool {
    /// Pool / pair address (lowercase hex, `0x`-prefixed as the subgraph returns it).
    pub address: String,
    /// token0 address (lowercase hex).
    pub token0: String,
    /// token1 address (lowercase hex).
    pub token1: String,
    /// token0 symbol (best-effort; `""` if the subgraph omitted it).
    pub token0_symbol: String,
    /// token1 symbol (best-effort; `""` if the subgraph omitted it).
    pub token1_symbol: String,
    /// token0 decimals (defaults to 18 when the subgraph omitted it).
    pub token0_decimals: u8,
    /// token1 decimals (defaults to 18 when the subgraph omitted it).
    pub token1_decimals: u8,
    /// Raw V3 `feeTier` in pips (e.g. `3000` = 0.30%); `None` for V2 pairs
    /// (V2 carries an implicit 0.30% = 3000-pip fee the consumer applies).
    pub fee_tier: Option<u32>,
    /// Total value locked in USD (V3 `totalValueLockedUSD` / V2 `reserveUSD`).
    /// Used **only** for ranking/thresholding — never for profit math.
    pub tvl_usd: f64,
    /// Which subgraph entity produced this row.
    pub kind: SubgraphPoolKind,
}

/// The Graph caps `first` at 1000; clamp so a large `top_n` never errors the query.
const SUBGRAPH_MAX_FIRST: usize = 1000;

/// Build the GraphQL ranking query (pure — unit-testable without network).
///
/// `min_tvl_usd` is floored to an integer string for the `_gt` predicate
/// (The Graph compares `BigDecimal`/`BigInt` against decimal strings).
fn build_ranking_query(kind: SubgraphPoolKind, first: usize, min_tvl_usd: f64) -> String {
    let entity = kind.entity();
    let tvl = kind.tvl_field();
    let first = first.clamp(1, SUBGRAPH_MAX_FIRST);
    let min_tvl = if min_tvl_usd.is_finite() && min_tvl_usd > 0.0 {
        min_tvl_usd.floor() as u64
    } else {
        0
    };
    // V3 carries `feeTier`; V2 pairs have no per-pair fee field (fixed 0.30%).
    let fee_field = match kind {
        SubgraphPoolKind::V3 => " feeTier",
        SubgraphPoolKind::V2 => "",
    };
    format!(
        r#"{{ {entity}(first: {first}, orderBy: {tvl}, orderDirection: desc, where: {{ {tvl}_gt: "{min_tvl}" }}) {{ id{fee_field} {tvl} token0 {{ id symbol decimals }} token1 {{ id symbol decimals }} }} }}"#
    )
}

/// Parse a token sub-object `{ id, symbol, decimals }`. Returns `None` when the
/// address is missing/empty (fail-honest — the row is skipped). `decimals`
/// tolerates both string (`"18"`, BigInt) and numeric JSON; defaults to 18.
fn parse_token(v: &serde_json::Value) -> Option<(String, String, u8)> {
    let id = v.get("id")?.as_str()?.trim().to_lowercase();
    if id.is_empty() {
        return None;
    }
    let symbol = v
        .get("symbol")
        .and_then(|s| s.as_str())
        .unwrap_or("")
        .to_string();
    let decimals = v
        .get("decimals")
        .map(|d| {
            d.as_str()
                .and_then(|s| s.parse::<u8>().ok())
                .or_else(|| d.as_u64().map(|n| n as u8))
                .unwrap_or(18)
        })
        .unwrap_or(18);
    Some((id, symbol, decimals))
}

/// Parse the ranking response into `RankedPool`s (pure — unit-testable).
///
/// Fail-honest per row: any entry missing an `id` or either token is **skipped**
/// rather than fabricated. A missing/empty collection yields an empty vec.
fn parse_ranked_pools(body: &serde_json::Value, kind: SubgraphPoolKind) -> Vec<RankedPool> {
    let entity = kind.entity();
    let tvl_field = kind.tvl_field();
    let rows = match body
        .pointer(&format!("/data/{entity}"))
        .and_then(|v| v.as_array())
    {
        Some(arr) => arr,
        None => return Vec::new(),
    };
    let mut out = Vec::with_capacity(rows.len());
    for row in rows {
        let address = match row.get("id").and_then(|v| v.as_str()) {
            Some(s) if !s.trim().is_empty() => s.trim().to_lowercase(),
            _ => continue, // no address → skip (fail-honest)
        };
        let (token0, token0_symbol, token0_decimals) = match row.get("token0").and_then(parse_token)
        {
            Some(t) => t,
            None => continue, // missing token0 metadata → skip
        };
        let (token1, token1_symbol, token1_decimals) = match row.get("token1").and_then(parse_token)
        {
            Some(t) => t,
            None => continue, // missing token1 metadata → skip
        };
        let fee_tier = match kind {
            SubgraphPoolKind::V3 => row.get("feeTier").and_then(|f| {
                f.as_str()
                    .and_then(|s| s.parse::<u32>().ok())
                    .or_else(|| f.as_u64().map(|n| n as u32))
            }),
            SubgraphPoolKind::V2 => None,
        };
        let tvl_usd = row
            .get(tvl_field)
            .and_then(|v| v.as_str())
            .and_then(|s| s.parse::<f64>().ok())
            .unwrap_or(0.0);
        out.push(RankedPool {
            address,
            token0,
            token1,
            token0_symbol,
            token1_symbol,
            token0_decimals,
            token1_decimals,
            fee_tier,
            tvl_usd,
            kind,
        });
    }
    out
}

/// Fetch the top-`first` pools/pairs by TVL on `chain_id` from The Graph.
///
/// # Returns
/// - `Ok(Some(pools))` — subgraph configured & responded (the vec may be empty
///   if nothing clears `min_tvl_usd`; that is a *real* zero, R8-honest).
/// - `Ok(None)` — `ARBX_SUBGRAPH_URL_<chain>` absent (unconfigured → caller logs & skips).
/// - `Err(e)` — network failure, non-2xx, or a GraphQL `errors` array in the body.
///
/// No-hardcode: endpoint from env; `first` / `min_tvl_usd` are operator inputs.
/// Read-only HTTP — no signer, no capital, no broadcast (NO-ACTIVE).
pub async fn fetch_top_pools_by_tvl(
    chain_id: u64,
    kind: SubgraphPoolKind,
    first: usize,
    min_tvl_usd: f64,
) -> Result<Option<Vec<RankedPool>>> {
    let Some(url) = subgraph_url_for_chain(chain_id) else {
        return Ok(None); // unconfigured → R8 None
    };

    let gql_query = build_ranking_query(kind, first, min_tvl_usd);
    let body = serde_json::json!({ "query": gql_query });

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(15))
        .build()?;

    let resp = client.post(&url).json(&body).send().await?;
    if !resp.status().is_success() {
        return Err(anyhow::anyhow!(
            "subgraph ranking HTTP {} for chain {}",
            resp.status(),
            chain_id
        ));
    }

    let json: serde_json::Value = resp.json().await?;

    // The Graph returns HTTP 200 with a top-level `errors` array on query faults.
    if let Some(errors) = json.get("errors") {
        let has_errors = errors
            .as_array()
            .map(|a| !a.is_empty())
            .unwrap_or(!errors.is_null());
        if has_errors {
            return Err(anyhow::anyhow!(
                "subgraph ranking GraphQL errors for chain {}: {}",
                chain_id,
                errors
            ));
        }
    }

    Ok(Some(parse_ranked_pools(&json, kind)))
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

    // --- Top-N TVL ranking (Step 3) -------------------------------------------

    #[test]
    fn test_build_ranking_query_v3_orders_by_tvl_desc() {
        let q = build_ranking_query(SubgraphPoolKind::V3, 5, 1000.0);
        assert!(q.contains("pools("), "must query the pools entity: {q}");
        assert!(q.contains("orderBy: totalValueLockedUSD"), "query: {q}");
        assert!(q.contains("orderDirection: desc"), "query: {q}");
        assert!(q.contains("first: 5"), "query: {q}");
        assert!(
            q.contains(r#"totalValueLockedUSD_gt: "1000""#),
            "min-tvl predicate floored to integer string: {q}"
        );
        assert!(q.contains("feeTier"), "V3 must request feeTier: {q}");
    }

    #[test]
    fn test_build_ranking_query_v2_uses_pairs_reserveusd() {
        let q = build_ranking_query(SubgraphPoolKind::V2, 3, 0.0);
        assert!(q.contains("pairs("), "must query the pairs entity: {q}");
        assert!(q.contains("orderBy: reserveUSD"), "query: {q}");
        assert!(q.contains(r#"reserveUSD_gt: "0""#), "zero floor → \"0\": {q}");
        assert!(!q.contains("feeTier"), "V2 pairs have no feeTier field: {q}");
    }

    #[test]
    fn test_build_ranking_query_clamps_first_to_subgraph_cap() {
        let q = build_ranking_query(SubgraphPoolKind::V3, 100_000, 0.0);
        assert!(
            q.contains(&format!("first: {SUBGRAPH_MAX_FIRST}")),
            "first must clamp to the subgraph cap {SUBGRAPH_MAX_FIRST}: {q}"
        );
    }

    #[test]
    fn test_parse_ranked_pools_v3() {
        let body = serde_json::json!({
            "data": { "pools": [
                {
                    "id": "0xAbC0000000000000000000000000000000000001",
                    "feeTier": "3000",
                    "totalValueLockedUSD": "12345678.9",
                    "token0": { "id": "0xToKeN0000000000000000000000000000000Aaa", "symbol": "WETH", "decimals": "18" },
                    "token1": { "id": "0xToKeN0000000000000000000000000000000Bbb", "symbol": "USDC", "decimals": "6" }
                }
            ]}
        });
        let pools = parse_ranked_pools(&body, SubgraphPoolKind::V3);
        assert_eq!(pools.len(), 1);
        let p = &pools[0];
        assert_eq!(
            p.address, "0xabc0000000000000000000000000000000000001",
            "address must be lowercased"
        );
        assert_eq!(p.fee_tier, Some(3000));
        assert_eq!(p.token0_decimals, 18);
        assert_eq!(p.token1_decimals, 6);
        assert_eq!(p.token0_symbol, "WETH");
        assert_eq!(p.token1_symbol, "USDC");
        assert!((p.tvl_usd - 12_345_678.9).abs() < 1.0, "tvl parsed: {}", p.tvl_usd);
        assert_eq!(p.kind, SubgraphPoolKind::V3);
    }

    #[test]
    fn test_parse_ranked_pools_v2_has_no_fee_tier() {
        let body = serde_json::json!({
            "data": { "pairs": [
                {
                    "id": "0xPair000000000000000000000000000000000001",
                    "reserveUSD": "999.5",
                    "token0": { "id": "0xaaa0000000000000000000000000000000000001", "symbol": "A", "decimals": "18" },
                    "token1": { "id": "0xbbb0000000000000000000000000000000000002", "symbol": "B", "decimals": "18" }
                }
            ]}
        });
        let pools = parse_ranked_pools(&body, SubgraphPoolKind::V2);
        assert_eq!(pools.len(), 1);
        assert_eq!(pools[0].fee_tier, None, "V2 carries no feeTier");
        assert!((pools[0].tvl_usd - 999.5).abs() < 0.01);
        assert_eq!(pools[0].address, "0xpair000000000000000000000000000000000001");
    }

    #[test]
    fn test_parse_ranked_pools_skips_malformed_rows() {
        let body = serde_json::json!({
            "data": { "pools": [
                // row 0: no pool id → skipped
                { "feeTier": "500", "totalValueLockedUSD": "100",
                  "token0": { "id": "0x1" }, "token1": { "id": "0x2" } },
                // row 1: token0 missing id → skipped
                { "id": "0xbad00000000000000000000000000000000000a1", "feeTier": "500", "totalValueLockedUSD": "100",
                  "token0": { "symbol": "X", "decimals": "18" },
                  "token1": { "id": "0xbbb0000000000000000000000000000000000002", "symbol": "B", "decimals": "18" } },
                // row 2: fully formed → survives
                { "id": "0xGood0000000000000000000000000000000000a3", "feeTier": "500", "totalValueLockedUSD": "100",
                  "token0": { "id": "0xaaa0000000000000000000000000000000000001", "symbol": "A", "decimals": "18" },
                  "token1": { "id": "0xbbb0000000000000000000000000000000000002", "symbol": "B", "decimals": "18" } }
            ]}
        });
        let pools = parse_ranked_pools(&body, SubgraphPoolKind::V3);
        assert_eq!(pools.len(), 1, "only the fully-formed row survives (fail-honest)");
        assert_eq!(pools[0].address, "0xgood0000000000000000000000000000000000a3");
    }

    #[test]
    fn test_parse_ranked_pools_empty_when_data_absent_or_error() {
        let empty = serde_json::json!({ "data": { "pools": [] } });
        assert!(parse_ranked_pools(&empty, SubgraphPoolKind::V3).is_empty());
        let errored = serde_json::json!({ "errors": [{ "message": "boom" }] });
        assert!(parse_ranked_pools(&errored, SubgraphPoolKind::V3).is_empty());
    }
}
