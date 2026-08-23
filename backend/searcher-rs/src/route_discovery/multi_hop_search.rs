//! Multi-hop negative-cycle search (2–7 hops) over the token graph — Phase 3 (Session A).
//!
//! Spec: `docs/superpowers/specs/2026-06-07-mmbf-multi-hop-2-7-route-genius.md`.
//!
//! Two pieces:
//!   1. `LineGraph` — the line graph L(G): each directed `RouteEdge` becomes a node;
//!      edge i → j exists iff `edges[i].token_out == edges[j].token_in` and they are
//!      NOT the same pool (no immediate same-pool reuse). This is the structure MMBF
//!      (arXiv:2406.16573) consumes; built here + unit-tested for node/edge counts.
//!   2. `find_profitable_cycles` — a bounded negative-`log_weight` cycle search that
//!      returns closed cycles of length 2..=max_hops whose `Σ log_weight < 0` (i.e.
//!      `Π (1-fee)·rate > 1` ⇒ theoretical arbitrage). This is the correct, testable
//!      first increment; MMBF over L(G) is the later scale optimization (task 3.2b).
//!
//! FAIL-HONEST (R8): edges whose `log_weight` is `None` (V3 — concentrated liquidity
//! sizing deferred) are SKIPPED and counted in `v3_skipped`, never assigned a fake
//! weight. Hitting the route cap sets `capped = true` (never silently truncates).
//!
//! NO-ACTIVE: pure analysis over a graph snapshot — no Redis, no RPC, no execution,
//! no capital. Fully unit-testable offline (RULE 01 safe).

use crate::route_discovery::graph_builder::TokenGraph;
use ethers::types::Address;

/// The line graph L(G): nodes are indices into `TokenGraph.edges`; `successors[i]`
/// lists the edge indices that may legally follow edge `i` in a route.
#[derive(Debug, Clone)]
pub struct LineGraph {
    /// Number of nodes (= number of directed edges in the source graph).
    pub node_count: usize,
    /// Adjacency: successors[i] = edges that can follow edge i (token_out→token_in match,
    /// different pool).
    pub successors: Vec<Vec<usize>>,
    /// Total directed L(G) edges (Σ successors.len()).
    pub edge_count: usize,
}

/// Build the line graph from a token graph. O(E + Σ out-degree²) in the worst case;
/// bounded in practice by the per-pair pool cap upstream.
pub fn build_line_graph(graph: &TokenGraph) -> LineGraph {
    let n = graph.edges.len();
    let mut successors: Vec<Vec<usize>> = vec![Vec::new(); n];
    let mut edge_count = 0usize;

    // Index edges by their token_in so we can find followers of each edge in O(deg).
    use std::collections::HashMap;
    let mut by_token_in: HashMap<Address, Vec<usize>> = HashMap::new();
    for (i, e) in graph.edges.iter().enumerate() {
        by_token_in.entry(e.token_in).or_default().push(i);
    }

    for (i, e) in graph.edges.iter().enumerate() {
        if let Some(followers) = by_token_in.get(&e.token_out) {
            for &j in followers {
                if i == j {
                    continue;
                }
                // No immediate same-pool reuse (would be a no-op / wash within one pool).
                if graph.edges[j].pool == e.pool {
                    continue;
                }
                successors[i].push(j);
                edge_count += 1;
            }
        }
    }

    LineGraph {
        node_count: n,
        successors,
        edge_count,
    }
}

/// A discovered closed cycle: the ordered edge indices and the summed log weight.
#[derive(Debug, Clone, PartialEq)]
pub struct ProfitableCycle {
    /// Edge indices (into `TokenGraph.edges`) in traversal order.
    pub edges: Vec<usize>,
    /// Σ log_weight over the cycle. Negative ⇒ Π (1-fee)·rate > 1 ⇒ theoretical arb.
    pub sum_log_weight: f64,
    /// Number of hops (= edges.len()), in 2..=max_hops.
    pub hop_count: usize,
}

/// Result of a bounded multi-hop negative-cycle search, with honest caps/skip counters.
#[derive(Debug, Clone)]
pub struct MultiHopResult {
    pub cycles: Vec<ProfitableCycle>,
    /// True if the route cap was hit before the search exhausted the graph (R8 honesty).
    pub capped: bool,
    /// Cycles dropped because the cap was reached.
    pub dropped_for_cap: usize,
    /// Edges skipped because `log_weight` was `None` (V3 sizing pending — R8 honesty).
    pub v3_skipped: usize,
    /// PR-ROUTE-06: cycles dropped because |sum_log_weight| was below the
    /// 1e-6 noise floor (floating-point / stale-reserve artifact, no economic basis).
    pub noise_dropped: usize,
}

/// Find profitable (negative summed `log_weight`) closed cycles of length
/// `min_hops..=max_hops` starting and ending at the same token, via bounded DFS.
/// Honest: skips `None`-weight edges (V3) and caps the result count.
/// `min_hops`/`max_hops` are the XLS-CANON-01 `Min_Hops`/`Max_Hops` knobs
/// (01_CONFIG workbook; canonical floor 2, ceiling 7).
pub fn find_profitable_cycles(
    graph: &TokenGraph,
    min_hops: usize,
    max_hops: usize,
    max_cycles: usize,
) -> MultiHopResult {
    let max_hops = max_hops.clamp(2, 7);
    let min_hops = min_hops.clamp(2, max_hops);
    let mut out: Vec<ProfitableCycle> = Vec::new();
    let mut capped = false;
    let mut dropped_for_cap = 0usize;
    let mut v3_skipped = 0usize;

    // Try each token as a cycle start. The start token is the route's base asset.
    let start_tokens: Vec<Address> = graph.adjacency.keys().copied().collect();

    for start in start_tokens {
        if capped {
            break;
        }
        // DFS state: current token, path of edge indices, pools used, summed weight.
        let mut path: Vec<usize> = Vec::with_capacity(max_hops);
        let mut pools: Vec<Address> = Vec::with_capacity(max_hops);
        // PR-ROUTE-05: visited_tokens prevents degenerate bowtie cycles where the
        // same intermediate token is bought/sold twice without economic basis
        // (e.g. A→B→C→B→A). The start token is inserted once; intermediate tokens
        // are inserted on entry and removed on backtrack (mirroring
        // unique_route_finder.rs:197-199). MMBF theory is preserved: any non-simple
        // negative cycle decomposes into simple negative cycles at least as
        // profitable (fewer fees) — pruning to simple cycles loses zero realizable
        // yield. The cartridge MEV-01-019 already states this constraint.
        use std::collections::HashSet;
        let mut visited_tokens: HashSet<Address> = HashSet::new();
        visited_tokens.insert(start);
        dfs_cycles(
            graph,
            start,
            start,
            &mut path,
            &mut pools,
            &mut visited_tokens,
            0.0,
            min_hops,
            max_hops,
            max_cycles,
            &mut out,
            &mut capped,
            &mut dropped_for_cap,
            &mut v3_skipped,
        );
    }

    // PR-ROUTE-06: drop cycles whose profitability is floating-point / stale-reserve
    // noise. The fee floor alone is ~0.3% (30 bps) per leg; a cycle whose
    // |sum_log_weight| is below 1e-6 is either a rounding artifact or a stale
    // reserve snapshot that briefly looked negative. Such "profitable" cycles
    // have no economic basis — they would never survive the net-profit gate
    // (gas alone dwarfs a 1e-6 log spread). Honest prune (R8): not fabricated,
    // just excluded from the observe-only counter that feeds telemetry.
    const MIN_LOG_PROFIT: f64 = 1e-6;
    let before = out.len();
    out.retain(|c| c.sum_log_weight.abs() >= MIN_LOG_PROFIT);
    let noise_dropped = before - out.len();

    MultiHopResult {
        cycles: out,
        capped,
        dropped_for_cap,
        v3_skipped,
        noise_dropped,
    }
}

#[allow(clippy::too_many_arguments)]
fn dfs_cycles(
    graph: &TokenGraph,
    start: Address,
    current: Address,
    path: &mut Vec<usize>,
    pools: &mut Vec<Address>,
    visited_tokens: &mut std::collections::HashSet<Address>,
    sum_w: f64,
    min_hops: usize,
    max_hops: usize,
    max_cycles: usize,
    out: &mut Vec<ProfitableCycle>,
    capped: &mut bool,
    dropped_for_cap: &mut usize,
    v3_skipped: &mut usize,
) {
    if path.len() >= max_hops {
        return;
    }
    for e in graph.out_edges(&current) {
        // Resolve this edge's index (out_edges yields refs; recover index via adjacency).
        // We re-derive the index by pointer position to keep RouteEdge unmodified.
        let idx = match graph.edges.iter().position(|x| std::ptr::eq(x, e)) {
            Some(i) => i,
            None => continue,
        };

        // Skip immediate same-pool reuse.
        if pools.last() == Some(&e.pool) {
            continue;
        }
        // PR-ROUTE-05: anti-degeneracy — a non-start token already in the path
        // means the cycle revisits an intermediate token (bowtie shape like
        // A→B→C→B→A). Block it: the same token bought/sold twice mid-cycle has
        // no economic basis beyond wash-fee-burning. The start token is the
        // only allowed revisit (it closes the cycle, handled below).
        if e.token_out != start && visited_tokens.contains(&e.token_out) {
            continue;
        }
        // R8: V3 (or any) edge without a computable weight is skipped, never faked.
        let w = match e.log_weight {
            Some(w) => w,
            None => {
                *v3_skipped += 1;
                continue;
            }
        };

        let next_sum = sum_w + w;
        path.push(idx);
        pools.push(e.pool);

        if e.token_out == start && path.len() >= min_hops {
            // Closed cycle. Profitable iff Σ log_weight < 0.
            if next_sum < 0.0 {
                if out.len() >= max_cycles {
                    *capped = true;
                    *dropped_for_cap += 1;
                } else {
                    out.push(ProfitableCycle {
                        edges: path.clone(),
                        sum_log_weight: next_sum,
                        hop_count: path.len(),
                    });
                }
            }
            // Do not extend past a closed cycle on the start token.
        } else if e.token_out != start {
            visited_tokens.insert(e.token_out);
            dfs_cycles(
                graph,
                start,
                e.token_out,
                path,
                pools,
                visited_tokens,
                next_sum,
                min_hops,
                max_hops,
                max_cycles,
                out,
                capped,
                dropped_for_cap,
                v3_skipped,
            );
            visited_tokens.remove(&e.token_out);
        }

        path.pop();
        pools.pop();

        if *capped {
            return;
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::route_discovery::graph_builder::TokenGraph;
    use crate::route_discovery::types::{RouteDirection, RouteEdge};
    use crate::route_intent::ProtocolType;
    use ethers::types::Address;
    use std::collections::HashMap;

    fn addr(n: u64) -> Address {
        Address::from_low_u64_be(n)
    }

    /// Build a RouteEdge with a chosen log_weight (None = V3-style, skipped).
    fn edge(token_in: u64, token_out: u64, pool: u64, log_weight: Option<f64>) -> RouteEdge {
        RouteEdge {
            chain_id: 1,
            pool: addr(pool),
            token_in: addr(token_in),
            token_out: addr(token_out),
            token0: addr(token_in),
            token1: addr(token_out),
            protocol: if log_weight.is_some() {
                ProtocolType::V2
            } else {
                ProtocolType::V3
            },
            fee_bps: Some(30),
            liquidity_hint: Some(1_000_000.0),
            log_weight,
            freshness_ts: 1_700_000_000,
            blk: 20_000_000,
            direction: RouteDirection::ZeroForOne,
        }
    }

    fn graph_from(edges: Vec<RouteEdge>) -> TokenGraph {
        let mut adjacency: HashMap<Address, Vec<usize>> = HashMap::new();
        for (i, e) in edges.iter().enumerate() {
            adjacency.entry(e.token_in).or_default().push(i);
        }
        TokenGraph { edges, adjacency }
    }

    #[test]
    fn line_graph_links_by_token_out_to_token_in_diff_pool() {
        // A->B (pool1), B->C (pool2): L(G) has exactly one edge (e0 -> e1).
        let g = graph_from(vec![
            edge(0xA, 0xB, 1, Some(-0.01)),
            edge(0xB, 0xC, 2, Some(-0.01)),
        ]);
        let lg = build_line_graph(&g);
        assert_eq!(lg.node_count, 2);
        assert_eq!(lg.edge_count, 1);
        assert_eq!(lg.successors[0], vec![1]);
        assert!(lg.successors[1].is_empty());
    }

    #[test]
    fn line_graph_excludes_same_pool_reuse() {
        // A->B and B->A on the SAME pool must NOT be linked (no wash within one pool).
        let g = graph_from(vec![
            edge(0xA, 0xB, 1, Some(-0.01)),
            edge(0xB, 0xA, 1, Some(-0.01)), // same pool 1
        ]);
        let lg = build_line_graph(&g);
        assert_eq!(lg.edge_count, 0, "same-pool reverse must not link");
    }

    #[test]
    fn finds_profitable_two_hop_spatial_cycle() {
        // A->B (pool1) then B->A (pool2): a 2-hop spatial cycle. Make the sum negative.
        let g = graph_from(vec![
            edge(0xA, 0xB, 1, Some(-0.02)),
            edge(0xB, 0xA, 2, Some(-0.01)), // different pool → valid spatial backrun
        ]);
        let r = find_profitable_cycles(&g, 2, 7, 100);
        assert!(!r.capped);
        assert!(
            r.cycles
                .iter()
                .any(|c| c.hop_count == 2 && c.sum_log_weight < 0.0),
            "a 2-hop cross-pool cycle with negative sum must be found"
        );
    }

    #[test]
    fn finds_profitable_three_hop_cycle_but_not_flat() {
        // Profitable triangle A->B->C->A, all distinct pools, Σ<0.
        let profitable = graph_from(vec![
            edge(0xA, 0xB, 1, Some(-0.02)),
            edge(0xB, 0xC, 2, Some(-0.02)),
            edge(0xC, 0xA, 3, Some(-0.02)),
        ]);
        let r = find_profitable_cycles(&profitable, 2, 7, 100);
        assert!(
            r.cycles.iter().any(|c| c.hop_count == 3),
            "profitable triangle must be found"
        );

        // Flat triangle Σ=0 (not < 0) → no cycle.
        let flat = graph_from(vec![
            edge(0xA, 0xB, 1, Some(0.01)),
            edge(0xB, 0xC, 2, Some(0.01)),
            edge(0xC, 0xA, 3, Some(0.01)),
        ]);
        let r2 = find_profitable_cycles(&flat, 2, 7, 100);
        assert!(
            r2.cycles.is_empty(),
            "positive-sum (lossy) cycle must NOT be reported"
        );
    }

    #[test]
    fn v3_edges_are_skipped_honestly_not_faked() {
        // A->B is V3 (log_weight=None). It cannot be sized → skipped + counted.
        let g = graph_from(vec![
            edge(0xA, 0xB, 1, None), // V3, no weight
            edge(0xB, 0xA, 2, Some(-0.05)),
        ]);
        let r = find_profitable_cycles(&g, 2, 7, 100);
        assert!(r.v3_skipped > 0, "V3 edge must be counted as skipped (R8)");
        assert!(
            r.cycles.is_empty(),
            "cannot complete a cycle through an unsizable V3 leg"
        );
    }

    #[test]
    fn cap_is_honest() {
        // Two independent profitable 2-cycles, cap=1 → capped=true, dropped>0.
        let g = graph_from(vec![
            edge(0xA, 0xB, 1, Some(-0.02)),
            edge(0xB, 0xA, 2, Some(-0.02)),
            edge(0xC, 0xD, 3, Some(-0.02)),
            edge(0xD, 0xC, 4, Some(-0.02)),
        ]);
        let r = find_profitable_cycles(&g, 2, 7, 1);
        assert!(r.capped, "hitting the cap must set capped=true");
        assert!(r.dropped_for_cap >= 1);
        assert_eq!(r.cycles.len(), 1);
    }
}
