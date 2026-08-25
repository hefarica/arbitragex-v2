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
//! ## `log_weight` (RU-2: real for both protocols)
//! For **V2** we compute `−ln((1−fee)·rate)` per direction from reserves
//! (cheap, exact). For **V3** we compute it from the slot0 snapshot via
//! `amm_math::v3_spot_snapshot`: marginal rate `(sqrtPriceX96/2^96)²` adjusted
//! by token decimals (the price AT the active tick — depth across ticks stays
//! on the QuoterV2/sizing path). A snapshot we cannot price (zero/unparseable
//! sqrtPrice, non-finite magnitude) rejects the pool with `invalid_slot0` —
//! never a synthetic weight (R8).

use crate::amm_math::v3_spot_snapshot;
use crate::impact_index::PoolRef;
use crate::pair_index::{DenseIdBuilder, TokenKey};
use crate::reserves::{
    get_reserves, get_token_meta, get_v3_slot0, ReservesEntry, TokenMeta, V3Slot0Entry,
};
use crate::route_discovery::dense_view::{DenseAdjacency, MembershipRows};
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
    /// Hub-token SYMBOLS for the hot-token classification (workbook
    /// `03_GRAFO_POOLS` col P: pool is hot when either side ∈ this set).
    /// Default = the workbook formula's set {WETH, USDC, WBTC, USDT}. Override
    /// per-chain via `ARBX_GRAPH_HOT_TOKENS` (comma-separated).
    pub hot_tokens: Vec<String>,
    /// When `true`, prune non-hot edges from the graph entirely (concentration
    /// pruning — Network-Analysis-of-Uniswap core). Default `false`: the
    /// classification is stamped on every edge but the topology is unchanged;
    /// flipping it is an explicit operator decision (`ARBX_GRAPH_HOT_TOKEN_ONLY`).
    pub hot_token_only: bool,
}

impl Default for GraphBuildConfig {
    fn default() -> Self {
        Self {
            max_age_secs: 120,
            min_liquidity_hint: 0.0,
            hot_tokens: DEFAULT_HOT_TOKENS.iter().map(|s| s.to_string()).collect(),
            hot_token_only: false,
        }
    }
}

/// The workbook `03_GRAFO_POOLS` col-P hub set (the exact IF/OR formula's
/// tokens): the liquidity-concentration core of the graph.
pub const DEFAULT_HOT_TOKENS: [&str; 4] = ["WETH", "USDC", "WBTC", "USDT"];

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
    /// ARBX-0019: dense O(1) out-edge view (CSR + dense ids from
    /// `pair_index::DenseIdBuilder`). `None` until [`TokenGraph::build_dense`]
    /// runs — the HashMap `adjacency` stays the source of truth; this view
    /// only accelerates reads (built FROM the same edge list, never mutated
    /// independently).
    pub dense: Option<DenseView>,
}

/// The dense view of a [`TokenGraph`] (ARBX-0019/0020): one shared dense id
/// space over every token TOUCHED by an edge (`token_in` ∪ `token_out`, the
/// `pair_index` contract — reusable by `PairBuckets` later), the CSR out-edge
/// index, and — when the budget policy accepts N — per-token destination
/// bitset rows for O(1) edge-membership.
#[derive(Debug, Clone, Default)]
pub struct DenseView {
    builder: DenseIdBuilder,
    id_of: HashMap<Address, u32>,
    csr: DenseAdjacency,
    /// ARBX-0020: `has_edge` rows (bitset) while
    /// `dense_view::membership_bitset_fits(N)`; `None` for large/sparse N —
    /// the CSR scan inside `dense_has_edge` is the fallback.
    membership: Option<MembershipRows>,
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

    /// ARBX-0019/0020: (re)build the dense O(1) view from the CURRENT edge
    /// list. Idempotent; `adjacency` is left untouched (source of truth until
    /// the equivalence gate at WP-F). Ids assign in first-seen order over
    /// every token touched by an edge (`token_in` ∪ `token_out` — sink tokens
    /// get an id so `dense_has_edge` sees them) — deterministic regardless of
    /// pool iteration order.
    pub fn build_dense(&mut self, chain_id: u64) -> &mut Self {
        let mut builder = DenseIdBuilder::new();
        let mut id_of: HashMap<Address, u32> = HashMap::new();
        let mut assign =
            |addr: Address, builder: &mut DenseIdBuilder, id_of: &mut HashMap<Address, u32>| {
                *id_of.entry(addr).or_insert_with(|| {
                    builder.insert(
                        TokenKey {
                            chain_id,
                            address: addr,
                        },
                        true,
                    ) as u32
                })
            };
        let mut sources = Vec::with_capacity(self.edges.len());
        let mut dests = Vec::with_capacity(self.edges.len());
        for e in &self.edges {
            sources.push(assign(e.token_in, &mut builder, &mut id_of));
            dests.push(assign(e.token_out, &mut builder, &mut id_of));
        }
        let csr = DenseAdjacency::from_edge_sources(builder.len(), &sources);
        let membership = MembershipRows::build(builder.len(), &sources, &dests);
        self.dense = Some(DenseView {
            builder,
            id_of,
            csr,
            membership,
        });
        self
    }

    /// ARBX-0019: O(1) out-edge indices for `token` from the dense view.
    /// `None` when the view is absent or the token is unknown to the graph;
    /// a KNOWN token with no out-edges (sink) yields an empty slice — the
    /// same observable answer the HashMap path gives.
    pub fn dense_out_indices(&self, token: &Address) -> Option<&[u32]> {
        let view = self.dense.as_ref()?;
        let id = *view.id_of.get(token)? as usize;
        Some(view.csr.out_edge_indices(id))
    }

    /// ARBX-0020: O(1) edge-membership — `true` iff some edge runs
    /// `from → to`. Bitset rows when the budget accepts N, CSR out-edge scan
    /// when it does not (large/sparse fallback, workbook col G). `None` when
    /// the dense view is absent OR either token is unknown to the graph —
    /// the caller falls back to `adjacency` (same contract as
    /// `dense_out_indices`; known tokens always get a definitive answer).
    pub fn dense_has_edge(&self, from: &Address, to: &Address) -> Option<bool> {
        let view = self.dense.as_ref()?;
        let f = *view.id_of.get(from)?;
        let t = view.id_of.get(to).copied()?;
        match &view.membership {
            Some(rows) => Some(rows.has_edge(f, t)),
            None => Some(
                view.csr
                    .out_edge_indices(f as usize)
                    .iter()
                    .any(|&e| self.edges[e as usize].token_out == *to),
            ),
        }
    }

    /// Dense id of `token` in the shared `pair_index` id space (ARBX-0028
    /// `PairBuckets` consumes the same ids). `None` without a view or for a
    /// token no edge touches.
    pub fn dense_token_id(&self, token: &Address) -> Option<usize> {
        let view = self.dense.as_ref()?;
        (*view.id_of.get(token)? as usize).into()
    }

    /// Size of the dense token universe (tokens touched by ≥1 edge).
    pub fn dense_token_count(&self) -> usize {
        self.dense.as_ref().map(|v| v.builder.len()).unwrap_or(0)
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
                let liq_raw: u128 = s
                    .liquidity
                    .parse()
                    .map_err(|_| "invalid_slot0".to_string())?;
                if liq_raw == 0 {
                    return Err("missing_slot0".to_string());
                }
                let sp_raw: u128 = s
                    .sqrt_price_x96
                    .parse()
                    .map_err(|_| "invalid_slot0".to_string())?;
                // Real V3 spot-rate weight (RU-2): (√P)² from slot0 → rate →
                // −ln((1−fee)·rate), the same formula as V2. Fail-honest: a
                // snapshot we cannot price (zero/uninitialized sqrtPrice,
                // non-finite magnitudes) rejects the pool — never a synthetic
                // weight (R8).
                let snap = v3_spot_snapshot(sp_raw, liq_raw, dec0, dec1)
                    .ok_or_else(|| "invalid_slot0".to_string())?;
                // V3 fee tier arrives in the on-chain uint24 convention —
                // millionths (100/500/3000/10000 pips from PG `pools.fee_tier`),
                // NOT the V2 basis-point convention (500 pips = 0.05%, not 5%).
                let fee = pool
                    .fee_bps
                    .map(|p| p as f64 / 1_000_000.0)
                    .unwrap_or(0.003); // modal tier 3000 pips = 0.30%
                (
                    snap.virtual_reserves_hint,
                    log_weight(fee, snap.rate_01),
                    log_weight(fee, 1.0 / snap.rate_01),
                    s.ts,
                    0u64,
                )
            }
            ProtocolType::Curve | ProtocolType::Balancer | ProtocolType::Unknown => {
                return Err("unsupported_protocol".to_string());
            }
        };

    if cfg.min_liquidity_hint > 0.0 && liquidity_hint < cfg.min_liquidity_hint {
        return Err("low_liquidity".to_string());
    }

    // Hot-token classification (workbook 03 col P): either side's SYMBOL in the
    // hub set. A missing meta/symbol is NOT hot (R8: unknown ≠ hot).
    let hot_token = [meta0, meta1]
        .into_iter()
        .flatten()
        .any(|m| cfg.hot_tokens.iter().any(|h| h == &m.symbol));
    if cfg.hot_token_only && !hot_token {
        return Err("non_hot_token_edge".to_string());
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
        hot_token,
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

    // ARBX-0019: dense O(1) view built from the SAME edge list — the
    // HashMap above stays the source of truth (equivalence gate: WP-F).
    let mut graph = TokenGraph {
        edges,
        adjacency,
        dense: None,
    };
    graph.build_dense(chain_id);

    GraphBuildOutcome {
        graph,
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
    fn v3_pool_yields_edges_with_real_log_weight() {
        // USDC(6)/WETH(18) 0.05% pool: sqrtPriceX96 ≈ 1.7727e33 → rate_01
        // (WETH per USDC) ≈ 5.0054e-4. RU-2: the V3 edges now carry the SAME
        // −ln((1−fee)·rate) weight as V2, not a deferred None.
        let p = v3_pool(); // fee_bps = Some(500) — uint24 pips = 0.05%
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
        let w0 = e0.log_weight.expect("V3 zero_for_one weight");
        let w1 = e1.log_weight.expect("V3 one_for_zero weight");
        assert!(w0.is_finite() && w1.is_finite());
        // USDC→WETH: tiny rate → large positive weight (~7.60).
        assert!(
            (7.5..7.7).contains(&w0),
            "w0={w0} expected ~7.60 (rate ≈ 5.0054e-4 WETH/USDC)"
        );
        // WETH→USDC: huge rate → large negative weight (~−7.60).
        assert!((-7.7..-7.5).contains(&w1), "w1={w1} expected ~−7.60");
        assert_eq!(e0.protocol, ProtocolType::V3);
        assert_eq!(e0.blk, 0); // V3 slot0 carries no block.
    }

    #[test]
    fn v3_log_weight_sum_pins_fee_in_pips_not_bps() {
        // −ln((1−f)·r) − ln((1−f)/r) = −2·ln(1−f) — the rate cancels, so this
        // pins the FEE alone. fee_bps=Some(500) is the uint24 tier (pips):
        // 500/1e6 = 0.05% ⇒ sum ≈ 0.0010003. If it were misread as bps
        // (500/1e4 = 5%) the sum would be ≈ 0.1026 — 100× off.
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
        let sum = e0.log_weight.unwrap() + e1.log_weight.unwrap();
        let expected = -2.0 * (1.0 - 0.0005f64).ln();
        assert!(
            (sum - expected).abs() < 1e-9,
            "sum={sum} expected={expected} (fee must be 500 pips = 0.05%)"
        );
    }

    #[test]
    fn v3_at_unit_price_weight_is_minus_fee_log() {
        // sqrtPriceX96 = 2^96 with equal decimals → rate exactly 1.0 ⇒ both
        // directions weigh −ln(1−fee) = 0.000500125 (fee 500 pips).
        let p = v3_pool();
        let s = V3Slot0Entry {
            sqrt_price_x96: (1u128 << 96).to_string(),
            liquidity: "1000000000000000000".to_string(),
            ts: 1000,
        };
        let (e0, e1) = build_edges_for_pool(
            &p,
            None,
            Some(&s),
            Some(&meta(18)),
            Some(&meta(18)),
            1000,
            &GraphBuildConfig::default(),
        )
        .unwrap();
        let expected = -(1.0 - 0.0005f64).ln();
        assert!((e0.log_weight.unwrap() - expected).abs() < 1e-12);
        assert!((e1.log_weight.unwrap() - expected).abs() < 1e-12);
        // Virtual-reserves hint at 1:1 with L = 1e18, 18 decimals: x_v = y_v =
        // 1.0 token ⇒ hint = 2.0 (unit-consistent with the V2 r0+r1 hint).
        assert!((e0.liquidity_hint.unwrap() - 2.0).abs() < 1e-9);
    }

    #[test]
    fn v3_unpriceable_snapshot_rejects_never_synthetic() {
        // Zero sqrtPrice (uninitialized), garbage, or > u128 (uint160 overflow
        // guard) — all reject with `invalid_slot0`; zero liquidity keeps the
        // historical `missing_slot0` reason. None of these may yield an edge
        // with a fabricated weight (R8).
        let p = v3_pool();
        for (sqrt, reason) in [
            ("0", "invalid_slot0"),
            ("not-a-number", "invalid_slot0"),
            (
                "340282366920938463463374607431768211456", // 2^128 > u128::MAX
                "invalid_slot0",
            ),
        ] {
            let s = V3Slot0Entry {
                sqrt_price_x96: sqrt.to_string(),
                liquidity: "548640024015773269".to_string(),
                ts: 1000,
            };
            let err = build_edges_for_pool(
                &p,
                None,
                Some(&s),
                Some(&meta(6)),
                Some(&meta(18)),
                1000,
                &GraphBuildConfig::default(),
            )
            .unwrap_err();
            assert_eq!(err, reason, "sqrt={sqrt}");
        }
        let s0liq = V3Slot0Entry {
            sqrt_price_x96: "1772712074874819459120282715246463".to_string(),
            liquidity: "0".to_string(),
            ts: 1000,
        };
        let err = build_edges_for_pool(
            &p,
            None,
            Some(&s0liq),
            Some(&meta(6)),
            Some(&meta(18)),
            1000,
            &GraphBuildConfig::default(),
        )
        .unwrap_err();
        assert_eq!(err, "missing_slot0");
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
            ..Default::default()
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

    /// XLS-GRAPH-01 (workbook 03 col P): a pool with either side in the hub set
    /// is hot; unknown symbols are NOT hot (R8); both directed edges carry the
    /// same classification; the prune rejects non-hot pools with an explicit
    /// reason when enabled.
    #[test]
    fn hot_token_classification_and_prune() {
        let p = v2_pool();
        let mut weth = meta(18);
        weth.symbol = "WETH".to_string();
        let base = GraphBuildConfig::default();

        // Either side hot → both edges classified hot.
        let (e0, e1) = build_edges_for_pool(
            &p,
            Some(&reserves(1000)),
            None,
            Some(&weth),
            Some(&meta(6)),
            1000,
            &base,
        )
        .unwrap();
        assert!(e0.hot_token);
        assert!(e1.hot_token);

        // No hub side → not hot, and kept when the prune is OFF (default:
        // topology unchanged).
        let (e0, _) = build_edges_for_pool(
            &p,
            Some(&reserves(1000)),
            None,
            Some(&meta(6)),
            Some(&meta(6)),
            1000,
            &base,
        )
        .unwrap();
        assert!(!e0.hot_token);

        // Missing meta never reaches classification — the pool rejects earlier
        // with its own reason (`missing_token_metadata`), so "unknown symbol"
        // in practice means "symbol not in the hub set" (covered above).
        let err = build_edges_for_pool(
            &p,
            Some(&reserves(1000)),
            None,
            None,
            Some(&meta(6)),
            1000,
            &base,
        )
        .unwrap_err();
        assert_eq!(err, "missing_token_metadata");

        // Prune ON → non-hot pool rejected with the machine-readable reason.
        let pruning = GraphBuildConfig {
            hot_token_only: true,
            ..Default::default()
        };
        let err = build_edges_for_pool(
            &p,
            Some(&reserves(1000)),
            None,
            Some(&meta(6)),
            Some(&meta(6)),
            1000,
            &pruning,
        )
        .unwrap_err();
        assert_eq!(err, "non_hot_token_edge");

        // Prune ON + hub side → accepted (hub topology survives the prune).
        assert!(build_edges_for_pool(
            &p,
            Some(&reserves(1000)),
            None,
            Some(&weth),
            Some(&meta(6)),
            1000,
            &pruning,
        )
        .is_ok());
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
        let g = TokenGraph {
            edges,
            adjacency,
            dense: None,
        };
        assert_eq!(g.out_edges(&addr(1)).count(), 1);
        assert_eq!(g.out_edges(&addr(2)).count(), 1);
        assert_eq!(g.out_edges(&addr(999)).count(), 0);
        assert_eq!(g.tokens().count(), 2);
    }

    // ── ARBX-0019: dense view equivalence + bench vs HashMap ─────────────

    /// Deterministic synthetic-graph edge literal (shared by the ARBX-0019/0020
    /// dense-view tests): V2, fee 30 bps, priced, zero freshness fields.
    fn synth_edge(pool: u64, ti: u64, to: u64) -> RouteEdge {
        RouteEdge {
            chain_id: 1,
            pool: Address::from_low_u64_be(pool),
            token_in: Address::from_low_u64_be(ti),
            token_out: Address::from_low_u64_be(to),
            token0: Address::from_low_u64_be(ti),
            token1: Address::from_low_u64_be(to),
            protocol: crate::route_intent::ProtocolType::V2,
            fee_bps: Some(30),
            liquidity_hint: Some(1.0),
            log_weight: Some(-0.003),
            freshness_ts: 0,
            blk: 0,
            hot_token: false,
            direction: RouteDirection::ZeroForOne,
        }
    }

    /// Deterministic synthetic graph: `n` tokens, ~`deg` out-edges each
    /// (workbook Avg_Active_Degree = 6), no rand dependency.
    fn synth_graph(n: u64, deg: u64) -> TokenGraph {
        let mut edges = Vec::new();
        for t in 0..n {
            for d in 0..deg {
                let out = (t * 31 + d * 7 + 1) % n;
                if out == t {
                    continue; // no self-loops in the token graph
                }
                edges.push(synth_edge(t * deg + d + 1, t + 1, out + 1));
            }
        }
        let mut adjacency: HashMap<Address, Vec<usize>> = HashMap::new();
        for (i, e) in edges.iter().enumerate() {
            adjacency.entry(e.token_in).or_default().push(i);
        }
        let mut g = TokenGraph {
            edges,
            adjacency,
            dense: None,
        };
        g.build_dense(1);
        g
    }

    /// The dense view agrees with the HashMap adjacency for EVERY token —
    /// same edge indices, same order (the "equivalencia demostrada" AC:
    /// pinned at data level here, full swap-over only at the final gate).
    #[test]
    fn dense_view_matches_hashmap_adjacency_exactly() {
        let g = synth_graph(24, 6);
        assert_eq!(g.dense_token_count(), g.adjacency.len());
        for token in g.adjacency.keys() {
            let hash_view: Vec<usize> = g.adjacency.get(token).cloned().unwrap_or_default();
            let dense_view_ix: Vec<usize> = g
                .dense_out_indices(token)
                .map(|ix| ix.iter().map(|&i| i as usize).collect())
                .unwrap_or_default();
            assert_eq!(hash_view, dense_view_ix, "token {token:?}: views disagree");
        }
        // A token with no out-edges: None from the dense view, empty from
        // the HashMap — callers treat both as "no out-edges".
        assert!(g.dense_out_indices(&addr(9999)).is_none());
        assert_eq!(g.out_edges(&addr(9999)).count(), 0);
        // Dense ids are a bijection over adjacency keys (first-seen order).
        let mut ids: Vec<usize> = g
            .adjacency
            .keys()
            .filter_map(|t| g.dense_token_id(t))
            .collect();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), g.adjacency.len());
        assert_eq!(ids.first().copied(), Some(0));
        assert_eq!(ids.last().copied(), Some(g.adjacency.len() - 1));
    }

    /// Bench vs HashMap (AC "bench vs HashMap registrado"): measures both
    /// out-edge resolution paths over the synthetic graph — the legacy
    /// HashMap `get` and the dense CSR slice — and asserts they resolve the
    /// SAME edges on every rep (the bench doubles as a differential). The
    /// recorded numbers live in TEST_EVIDENCE/registry (std::time, no
    /// criterion dep — Cero Dependencias Obesas).
    #[test]
    fn bench_dense_vs_hashmap_out_edges() {
        for &(n, deg) in &[(24u64, 6u64), (512, 6), (2048, 6)] {
            let g = synth_graph(n, deg);
            let tokens: Vec<Address> = g.adjacency.keys().copied().collect();
            let reps = 200u32;

            // Warm both paths (allocator + caches).
            for t in &tokens {
                let _ = g.adjacency.get(t).map(|v| v.len());
                let _ = g.dense_out_indices(t).map(|s| s.len());
            }

            let t0 = std::time::Instant::now();
            let mut hm_total = 0usize;
            for _ in 0..reps {
                for t in &tokens {
                    if let Some(ix) = g.adjacency.get(t) {
                        hm_total += ix.len();
                    }
                }
            }
            let hm_ns = t0.elapsed().as_nanos() / (reps as u128 * tokens.len() as u128);

            let t1 = std::time::Instant::now();
            let mut dn_total = 0usize;
            for _ in 0..reps {
                for t in &tokens {
                    if let Some(ix) = g.dense_out_indices(t) {
                        dn_total += ix.len();
                    }
                }
            }
            let dn_ns = t1.elapsed().as_nanos() / (reps as u128 * tokens.len() as u128);

            assert_eq!(
                hm_total, dn_total,
                "differential: both resolve the same edges"
            );
            println!(
                "ARBX-0019-BENCH n={n} edges={} | hashmap {hm_ns} ns/lookup | dense {dn_ns} ns/lookup",
                g.edges.len()
            );
        }
    }

    // ── ARBX-0020: bitset/CSR membership with large-N fallback ───────────

    /// `dense_has_edge` == the HashMap adjacency ground truth for EVERY pair
    /// (bitset path — n=24 is deep inside the budget).
    #[test]
    fn dense_has_edge_matches_hashmap_membership() {
        let g = synth_graph(24, 6);
        assert!(
            g.dense.as_ref().unwrap().membership.is_some(),
            "n=24 must take the bitset path"
        );
        for from in g.adjacency.keys() {
            for to in g.adjacency.keys() {
                let via_hash = g
                    .adjacency
                    .get(from)
                    .map(|ix| ix.iter().any(|&i| g.edges[i].token_out == *to))
                    .unwrap_or(false);
                assert_eq!(
                    g.dense_has_edge(from, to),
                    Some(via_hash),
                    "pair {from:?}->{to:?}: membership disagrees with source of truth"
                );
            }
        }
        // Unknown tokens answer None (caller falls back to `adjacency`,
        // which also yields false — honest absence, no panic).
        assert_eq!(g.dense_has_edge(&addr(9999), &addr(1)), None);
        assert_eq!(g.dense_has_edge(&addr(1), &addr(9999)), None);
    }

    /// n=3000 exceeds the bitset budget (crossover 2880/2881) → the rows are
    /// skipped and `dense_has_edge` answers via the CSR out-edge scan. Sampled
    /// pairs (every 17th) keep the debug-profile runtime bounded.
    #[test]
    fn dense_has_edge_csr_fallback_for_large_n() {
        let g = synth_graph(3000, 6);
        assert!(
            g.dense.as_ref().unwrap().membership.is_none(),
            "n=3000 must trip the budget policy (CSR fallback)"
        );
        let tokens: Vec<Address> = g.adjacency.keys().copied().collect();
        let mut checked = 0usize;
        let mut hits = 0usize;
        for from in &tokens {
            for to in tokens.iter().step_by(17) {
                let via_hash = g
                    .adjacency
                    .get(from)
                    .map(|ix| ix.iter().any(|&k| g.edges[k].token_out == *to))
                    .unwrap_or(false);
                assert_eq!(
                    g.dense_has_edge(from, to),
                    Some(via_hash),
                    "CSR fallback disagrees at pair {from:?}->{to:?}"
                );
                checked += 1;
                hits += usize::from(via_hash);
            }
        }
        assert!(checked > 5_000, "sample large enough to mean anything");
        assert!(hits > 0, "sample must contain at least one true edge");
    }

    /// The dense universe covers SINK tokens (out-degree 0): they get an id,
    /// an empty out-slice, and truthful membership answers.
    #[test]
    fn build_dense_universe_covers_sink_tokens() {
        let edges = vec![
            synth_edge(1, 0xA, 0xB),
            synth_edge(2, 0xB, 0xC),
            synth_edge(3, 0xA, 0xC),
        ];
        let mut adjacency: HashMap<Address, Vec<usize>> = HashMap::new();
        for (i, e) in edges.iter().enumerate() {
            adjacency.entry(e.token_in).or_default().push(i);
        }
        let mut g = TokenGraph {
            edges,
            adjacency,
            dense: None,
        };
        g.build_dense(1);

        assert_eq!(g.dense_token_count(), 3, "A, B, C — C is a sink");
        let c = addr(0xC);
        assert!(g.dense_token_id(&c).is_some(), "sink token has an id");
        assert_eq!(
            g.dense_out_indices(&c).map(<[u32]>::len),
            Some(0),
            "sink out-slice is empty (Some), not None"
        );
        assert_eq!(g.dense_has_edge(&addr(0xB), &c), Some(true));
        assert_eq!(g.dense_has_edge(&c, &addr(0xA)), Some(false));
        assert_eq!(g.dense_has_edge(&addr(0xA), &addr(0xB)), Some(true));
    }
}
