//! RouteGraphBuilder — builds the live token graph from `ImpactIndex` + Redis.
//!
//! Nodes are tokens; edges are *directed* swaps (one pool yields two edges, one
//! per orientation). Magnitudes are honest (RULE 00 / R8): a pool with missing
//! or stale on-chain data does NOT enter the graph — it is recorded as a
//! rejection with a machine-readable reason, never fabricated.
//!
//! The pure classifier [`build_edges_for_pool`] is separated from the async
//! Redis fetch so it can be unit-tested deterministically (the caller injects
//! `now_ts`).
//!
//! ## `log_weight` (Phase-2 forward-compat only)
//! For **V2** we compute `−ln((1−fee)·rate)` per direction from reserves
//! (cheap, exact). For **V3** the precise spot rate needs `sqrtPriceX96` tick
//! math; per the plan's guardrail #8 we leave `log_weight = None` for V3 in
//! Phase 1 rather than ship incomplete pricing. Phase 1 ranks on nothing — the
//! weight is stored only so the Phase-2 MMBF can consume it without rebuilding.

use crate::impact_index::PoolRef;
use crate::reserves::{
    get_reserves, get_token_meta, get_v3_slot0, ReservesEntry, TokenMeta, V3Slot0Entry,
};
use crate::route_discovery::types::{RouteDirection, RouteEdge};
use crate::route_intent::ProtocolType;
use ethers::types::Address;
use redis::aio::ConnectionManager;
use std::collections::HashMap;

/// Tunables for graph construction. Staleness and liquidity thresholds; the
/// richer per-strategy config arrives with the YAML in a later commit.
#[derive(Debug, Clone)]
pub struct GraphBuildConfig {
    /// Reject an edge whose snapshot `ts` is older than this many seconds.
    /// Secondary guard: Redis already TTLs reserves/slot0 (~30s), so an expired
    /// entry surfaces as `missing_*` first.
    pub max_age_secs: u64,
    /// Reject when `liquidity_hint < min_liquidity_hint`. `0.0` disables the
    /// check (default) — Phase 1 keeps thin pools as topology.
    pub min_liquidity_hint: f64,
}

impl Default for GraphBuildConfig {
    fn default() -> Self {
        Self {
            max_age_secs: 120,
            min_liquidity_hint: 0.0,
        }
    }
}

/// A pool that did not yield edges, with the reason (telemetry: `route_discovery.rejected`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RejectedEdge {
    pub pool: Address,
    pub reason: String,
}

/// The live token graph: a flat edge list plus an out-edge adjacency index
/// (`token_in → indices into edges`).
#[derive(Debug, Clone, Default)]
pub struct TokenGraph {
    pub edges: Vec<RouteEdge>,
    pub adjacency: HashMap<Address, Vec<usize>>,
}

impl TokenGraph {
    /// Out-edges leaving `token` (edges whose `token_in == token`).
    pub fn out_edges(&self, token: &Address) -> impl Iterator<Item = &RouteEdge> {
        self.adjacency
            .get(token)
            .into_iter()
            .flat_map(move |idxs| idxs.iter().map(move |&i| &self.edges[i]))
    }

    /// Distinct tokens that have at least one out-edge.
    pub fn tokens(&self) -> impl Iterator<Item = &Address> {
        self.adjacency.keys()
    }
}

/// Result of a full graph build over one chain's pool set.
#[derive(Debug, Clone, Default)]
pub struct GraphBuildOutcome {
    pub graph: TokenGraph,
    pub rejected: Vec<RejectedEdge>,
    pub pools_total: usize,
}

/// Normalize a raw integer reserve by token decimals into a float magnitude.
fn normalize(raw: f64, decimals: u8) -> f64 {
    raw / 10f64.powi(decimals as i32)
}

/// MMBF edge weight `−ln((1−fee)·rate)`. Returns `None` if the argument is
/// non-positive (cannot take the log) — honest, never a sentinel.
///
/// Also returns `None` if the result is non-finite (`±∞`/`NaN`). Without this,
/// an extreme-but-finite rate (e.g. a pathologically imbalanced pool, or a rate
/// that overflows `f64` to `∞`) would yield `Some(-∞)` — a phantom
/// "infinitely profitable" edge that makes ANY cycle through it look like a
/// guaranteed arb. R8 fail-honest: a weight we cannot compute finitely is
/// `None` (edge keeps its topology, no Phase-2 weight), never a fabricated value.
fn log_weight(fee: f64, rate: f64) -> Option<f64> {
    let arg = (1.0 - fee) * rate;
    if arg > 0.0 {
        let w = -arg.ln();
        if w.is_finite() {
            Some(w)
        } else {
            None
        }
    } else {
        None
    }
}

/// Classify one pool into its two directed edges, or a rejection reason.
///
/// Pure: all I/O (Redis fetches) is done by the caller and passed in. `now_ts`
/// is injected for deterministic staleness tests.
pub fn build_edges_for_pool(
    pool: &PoolRef,
    reserves: Option<&ReservesEntry>,
    slot0: Option<&V3Slot0Entry>,
    meta0: Option<&TokenMeta>,
    meta1: Option<&TokenMeta>,
    now_ts: u64,
    cfg: &GraphBuildConfig,
) -> Result<(RouteEdge, RouteEdge), String> {
    if pool.token0 == pool.token1 || pool.token0.is_zero() || pool.token1.is_zero() {
        return Err("invalid_pool_shape".to_string());
    }
    let dec0 = meta0
        .ok_or_else(|| "missing_token_metadata".to_string())?
        .decimals;
    let dec1 = meta1
        .ok_or_else(|| "missing_token_metadata".to_string())?
        .decimals;

    // (liquidity_hint, log_weight token0→token1, log_weight token1→token0, ts, blk)
    let (liquidity_hint, lw_01, lw_10, ts, blk): (f64, Option<f64>, Option<f64>, u64, u64) =
        match pool.protocol_type {
            ProtocolType::V2 => {
                let r = reserves.ok_or_else(|| "missing_reserves".to_string())?;
                if now_ts.saturating_sub(r.ts) > cfg.max_age_secs {
                    return Err("stale_reserves".to_string());
                }
                let r0 =
                    r.r0.parse::<f64>()
                        .map_err(|_| "invalid_reserves".to_string())?;
                let r1 =
                    r.r1.parse::<f64>()
                        .map_err(|_| "invalid_reserves".to_string())?;
                // Reject non-finite reserves: a corrupted cache entry whose decimal string
                // exceeds f64::MAX parses to `∞` and would otherwise sail past the `> 0.0`
                // guard, poisoning rate/liquidity math with infinities (R8 fail-honest).
                if !r0.is_finite() || !r1.is_finite() {
                    return Err("invalid_reserves".to_string());
                }
                if r0 <= 0.0 || r1 <= 0.0 {
                    return Err("missing_reserves".to_string());
                }
                let n0 = normalize(r0, dec0);
                let n1 = normalize(r1, dec1);
                // V2 fee: explicit bps when known, else canonical 0.30%.
                let fee = pool.fee_bps.map(|b| b as f64 / 10_000.0).unwrap_or(0.003);
                (
                    n0 + n1,
                    log_weight(fee, n1 / n0),
                    log_weight(fee, n0 / n1),
                    r.ts,
                    r.blk,
                )
            }
            ProtocolType::V3 => {
                let s = slot0.ok_or_else(|| "missing_slot0".to_string())?;
                if now_ts.saturating_sub(s.ts) > cfg.max_age_secs {
                    return Err("stale_slot0".to_string());
                }
                let liq = s
                    .liquidity
                    .parse::<f64>()
                    .map_err(|_| "invalid_slot0".to_string())?;
                if liq <= 0.0 {
                    return Err("missing_slot0".to_string());
                }
                // V3 spot-rate weight deferred to Phase 2 (guardrail #8): None.
                (liq, None, None, s.ts, 0u64)
            }
            ProtocolType::Curve | ProtocolType::Balancer | ProtocolType::Unknown => {
                return Err("unsupported_protocol".to_string());
            }
        };

    if cfg.min_liquidity_hint > 0.0 && liquidity_hint < cfg.min_liquidity_hint {
        return Err("low_liquidity".to_string());
    }

    let edge_01 = RouteEdge {
        chain_id: pool.chain_id,
        pool: pool.address,
        token_in: pool.token0,
        token_out: pool.token1,
        token0: pool.token0,
        token1: pool.token1,
        protocol: pool.protocol_type,
        fee_bps: pool.fee_bps,
        liquidity_hint: Some(liquidity_hint),
        log_weight: lw_01,
        freshness_ts: ts,
        blk,
        direction: RouteDirection::ZeroForOne,
    };
    let edge_10 = RouteEdge {
        token_in: pool.token1,
        token_out: pool.token0,
        log_weight: lw_10,
        direction: RouteDirection::OneForZero,
        ..edge_01.clone()
    };
    Ok((edge_01, edge_10))
}

/// Build the full token graph for one chain from a pool set + live Redis caches.
///
/// Fetches token metadata and the per-protocol price snapshot for each pool,
/// then classifies via [`build_edges_for_pool`]. Pools with missing/stale data
/// land in `rejected` (R8 fail-honest). Best-effort: a Redis error on a single
/// key is treated as "absent" for that pool (it rejects with the corresponding
/// `missing_*` reason) rather than aborting the whole build.
pub async fn build_graph(
    redis: &mut ConnectionManager,
    chain_id: u64,
    pools: &[PoolRef],
    now_ts: u64,
    cfg: &GraphBuildConfig,
) -> GraphBuildOutcome {
    let mut edges: Vec<RouteEdge> = Vec::new();
    let mut rejected: Vec<RejectedEdge> = Vec::new();

    for pool in pools {
        let pool_lower = format!("{:#x}", pool.address);
        let t0_lower = format!("{:#x}", pool.token0);
        let t1_lower = format!("{:#x}", pool.token1);

        let meta0 = get_token_meta(redis, chain_id, &t0_lower)
            .await
            .ok()
            .flatten();
        let meta1 = get_token_meta(redis, chain_id, &t1_lower)
            .await
            .ok()
            .flatten();

        let reserves = if pool.protocol_type == ProtocolType::V2 {
            get_reserves(redis, chain_id, &pool_lower)
                .await
                .ok()
                .flatten()
        } else {
            None
        };
        let slot0 = if pool.protocol_type == ProtocolType::V3 {
            get_v3_slot0(redis, chain_id, &pool_lower)
                .await
                .ok()
                .flatten()
        } else {
            None
        };

        match build_edges_for_pool(
            pool,
            reserves.as_ref(),
            slot0.as_ref(),
            meta0.as_ref(),
            meta1.as_ref(),
            now_ts,
            cfg,
        ) {
            Ok((e0, e1)) => {
                edges.push(e0);
                edges.push(e1);
            }
            Err(reason) => rejected.push(RejectedEdge {
                pool: pool.address,
                reason,
            }),
        }
    }

    let mut adjacency: HashMap<Address, Vec<usize>> = HashMap::new();
    for (i, e) in edges.iter().enumerate() {
        adjacency.entry(e.token_in).or_default().push(i);
    }

    GraphBuildOutcome {
        graph: TokenGraph { edges, adjacency },
        rejected,
        pools_total: pools.len(),
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    fn addr(n: u64) -> Address {
        Address::from_low_u64_be(n)
    }

    fn meta(decimals: u8) -> TokenMeta {
        TokenMeta {
            symbol: "T".to_string(),
            decimals,
            is_stablecoin: false,
        }
    }

    fn v2_pool() -> PoolRef {
        PoolRef {
            chain_id: 1,
            address: addr(0xabc),
            dex_name: "uniswap-v2".to_string(),
            protocol_type: ProtocolType::V2,
            token0: addr(1),
            token1: addr(2),
            fee_bps: Some(30),
        }
    }

    fn v3_pool() -> PoolRef {
        PoolRef {
            chain_id: 1,
            address: addr(0xdef),
            dex_name: "uniswap-v3".to_string(),
            protocol_type: ProtocolType::V3,
            token0: addr(1),
            token1: addr(2),
            fee_bps: Some(500),
        }
    }

    fn reserves(ts: u64) -> ReservesEntry {
        ReservesEntry {
            r0: "1000000000".to_string(),
            r1: "2000000000".to_string(),
            token0_addr: Some(format!("{:#x}", addr(1))),
            blk: 100,
            ts,
        }
    }

    fn slot0(ts: u64) -> V3Slot0Entry {
        V3Slot0Entry {
            sqrt_price_x96: "1772712074874819459120282715246463".to_string(),
            liquidity: "548640024015773269".to_string(),
            ts,
        }
    }

    #[test]
    fn v2_pool_yields_two_directed_edges_with_log_weight() {
        let p = v2_pool();
        let (e0, e1) = build_edges_for_pool(
            &p,
            Some(&reserves(1000)),
            None,
            Some(&meta(6)),
            Some(&meta(6)),
            1000,
            &GraphBuildConfig::default(),
        )
        .unwrap();
        assert_eq!(e0.token_in, addr(1));
        assert_eq!(e0.token_out, addr(2));
        assert_eq!(e0.direction, RouteDirection::ZeroForOne);
        assert_eq!(e1.token_in, addr(2));
        assert_eq!(e1.token_out, addr(1));
        assert_eq!(e1.direction, RouteDirection::OneForZero);
        assert!(e0.liquidity_hint.unwrap() > 0.0);
        // V2 carries a forward-compat log_weight in both directions.
        assert!(e0.log_weight.is_some());
        assert!(e1.log_weight.is_some());
        assert_eq!(e0.protocol, ProtocolType::V2);
        assert_eq!(e0.blk, 100);
    }

    #[test]
    fn v3_pool_yields_edges_with_liquidity_but_no_log_weight() {
        let p = v3_pool();
        let (e0, e1) = build_edges_for_pool(
            &p,
            None,
            Some(&slot0(1000)),
            Some(&meta(6)),
            Some(&meta(18)),
            1000,
            &GraphBuildConfig::default(),
        )
        .unwrap();
        assert!(e0.liquidity_hint.unwrap() > 0.0);
        // Guardrail #8: V3 log_weight deferred to Phase 2.
        assert!(e0.log_weight.is_none());
        assert!(e1.log_weight.is_none());
        assert_eq!(e0.protocol, ProtocolType::V3);
        assert_eq!(e0.blk, 0); // V3 slot0 carries no block.
    }

    #[test]
    fn v2_missing_reserves_rejects() {
        let p = v2_pool();
        let err = build_edges_for_pool(
            &p,
            None,
            None,
            Some(&meta(6)),
            Some(&meta(6)),
            1000,
            &GraphBuildConfig::default(),
        )
        .unwrap_err();
        assert_eq!(err, "missing_reserves");
    }

    #[test]
    fn v3_missing_slot0_rejects() {
        let p = v3_pool();
        let err = build_edges_for_pool(
            &p,
            None,
            None,
            Some(&meta(6)),
            Some(&meta(18)),
            1000,
            &GraphBuildConfig::default(),
        )
        .unwrap_err();
        assert_eq!(err, "missing_slot0");
    }

    #[test]
    fn stale_snapshot_rejects() {
        let p = v2_pool();
        // ts=10, now=10_000, max_age=120 → stale.
        let err = build_edges_for_pool(
            &p,
            Some(&reserves(10)),
            None,
            Some(&meta(6)),
            Some(&meta(6)),
            10_000,
            &GraphBuildConfig::default(),
        )
        .unwrap_err();
        assert_eq!(err, "stale_reserves");

        let p3 = v3_pool();
        let err3 = build_edges_for_pool(
            &p3,
            None,
            Some(&slot0(10)),
            Some(&meta(6)),
            Some(&meta(18)),
            10_000,
            &GraphBuildConfig::default(),
        )
        .unwrap_err();
        assert_eq!(err3, "stale_slot0");
    }

    #[test]
    fn missing_token_metadata_rejects() {
        let p = v2_pool();
        let err = build_edges_for_pool(
            &p,
            Some(&reserves(1000)),
            None,
            None,
            Some(&meta(6)),
            1000,
            &GraphBuildConfig::default(),
        )
        .unwrap_err();
        assert_eq!(err, "missing_token_metadata");
    }

    #[test]
    fn unsupported_protocol_rejects() {
        let mut p = v2_pool();
        p.protocol_type = ProtocolType::Curve;
        let err = build_edges_for_pool(
            &p,
            Some(&reserves(1000)),
            None,
            Some(&meta(6)),
            Some(&meta(6)),
            1000,
            &GraphBuildConfig::default(),
        )
        .unwrap_err();
        assert_eq!(err, "unsupported_protocol");
    }

    #[test]
    fn invalid_pool_shape_rejects() {
        let mut p = v2_pool();
        p.token1 = p.token0; // token0 == token1
        let err = build_edges_for_pool(
            &p,
            Some(&reserves(1000)),
            None,
            Some(&meta(6)),
            Some(&meta(6)),
            1000,
            &GraphBuildConfig::default(),
        )
        .unwrap_err();
        assert_eq!(err, "invalid_pool_shape");
    }

    #[test]
    fn non_finite_reserves_reject_and_never_yield_infinite_weight() {
        // A corrupted reserve string that parses to f64::INFINITY ("1e400") must be
        // rejected, NOT silently admitted as a Some(-inf) "infinitely profitable" edge.
        let p = v2_pool();
        let mut r = reserves(1000);
        r.r0 = "1e400".to_string(); // parses to f64::INFINITY
        let err = build_edges_for_pool(
            &p,
            Some(&r),
            None,
            Some(&meta(6)),
            Some(&meta(6)),
            1000,
            &GraphBuildConfig::default(),
        )
        .unwrap_err();
        assert_eq!(err, "invalid_reserves");
        // And log_weight itself never returns a non-finite value.
        assert_eq!(log_weight(0.003, f64::INFINITY), None);
        assert_eq!(log_weight(0.003, 0.0), None);
        assert!(log_weight(0.003, 1.0).unwrap().is_finite());
    }

    #[test]
    fn low_liquidity_rejects_when_threshold_set() {
        let p = v2_pool();
        let cfg = GraphBuildConfig {
            max_age_secs: 120,
            min_liquidity_hint: 1e30, // absurdly high → reject
        };
        let err = build_edges_for_pool(
            &p,
            Some(&reserves(1000)),
            None,
            Some(&meta(6)),
            Some(&meta(6)),
            1000,
            &cfg,
        )
        .unwrap_err();
        assert_eq!(err, "low_liquidity");
    }

    #[test]
    fn adjacency_indexes_out_edges_by_token_in() {
        // Build a tiny graph by hand from two accepted edges.
        let p = v2_pool();
        let (e0, e1) = build_edges_for_pool(
            &p,
            Some(&reserves(1000)),
            None,
            Some(&meta(6)),
            Some(&meta(6)),
            1000,
            &GraphBuildConfig::default(),
        )
        .unwrap();
        let edges = vec![e0, e1];
        let mut adjacency: HashMap<Address, Vec<usize>> = HashMap::new();
        for (i, e) in edges.iter().enumerate() {
            adjacency.entry(e.token_in).or_default().push(i);
        }
        let g = TokenGraph { edges, adjacency };
        assert_eq!(g.out_edges(&addr(1)).count(), 1);
        assert_eq!(g.out_edges(&addr(2)).count(), 1);
        assert_eq!(g.out_edges(&addr(999)).count(), 0);
        assert_eq!(g.tokens().count(), 2);
    }
}
