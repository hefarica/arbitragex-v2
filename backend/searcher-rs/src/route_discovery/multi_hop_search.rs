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
//! Missing weights and non-finite values are skipped and counted. V3 edges
//! with a valid marginal price can participate; exact tick-aware sizing and
//! simulation remain downstream requirements. Exploration has independent
//! output/work budgets and an optional cooperative deadline.
//!
//! The kernel has no I/O, capital or execution. It is shared by the analysis
//! radar AND the canonical scanner, so funding-asset rotations are preserved
//! by default. Only the analysis caller enables rotation deduplication.

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

/// Result of a bounded multi-hop negative-cycle search. A cap is always
/// observable; a negative marginal log sum is NOT a sized net-profit quote.
#[derive(Debug, Clone, Default)]
pub struct MultiHopResult {
    pub cycles: Vec<ProfitableCycle>,
    pub capped: bool,
    /// Lower bound on eligible cycles omitted at the output cap.
    pub dropped_for_cap: usize,
    /// Missing weights encountered during traversal (legacy telemetry name).
    pub v3_skipped: usize,
    pub noise_dropped: usize,
    pub invalid_weights: usize,
    pub edge_visits: usize,
    pub work_limited: bool,
    pub time_limited: bool,
    pub duplicate_cycles: usize,
}

/// Independent work and time bounds: an output cap alone does not bound
/// a dense graph with no profitable cycles. Time is a cooperative deadline,
/// checked every 64 edge visits; it is not a real-time SLA guarantee.
#[derive(Debug, Clone, Copy)]
pub struct SearchLimits {
    pub max_edge_visits: usize,
    pub max_duration: Option<std::time::Duration>,
    /// Six bits: bit (h-2) permits hop length h, h in 2..=7.
    pub hop_mask: u8,
    /// Analysis-only deduplication. Keep false for execution discovery:
    /// rotations may need different borrowing assets and funding providers.
    pub deduplicate_rotations: bool,
}

pub const DEFAULT_MAX_EDGE_VISITS: usize = 100_000;

impl Default for SearchLimits {
    fn default() -> Self {
        Self {
            max_edge_visits: DEFAULT_MAX_EDGE_VISITS,
            max_duration: None,
            hop_mask: 0b11_1111,
            deduplicate_rotations: false,
        }
    }
}

/// Compatibility entry point with a deterministic work budget. Live callers
/// may additionally request a deadline and an exact strategy mask.
pub fn find_profitable_cycles(
    graph: &TokenGraph,
    min_hops: usize,
    max_hops: usize,
    max_cycles: usize,
) -> MultiHopResult {
    find_profitable_cycles_with_limits(
        graph,
        min_hops,
        max_hops,
        max_cycles,
        SearchLimits::default(),
    )
}

pub fn find_profitable_cycles_with_limits(
    graph: &TokenGraph,
    min_hops: usize,
    max_hops: usize,
    max_cycles: usize,
    limits: SearchLimits,
) -> MultiHopResult {
    let min_hops = min_hops.max(2);
    let max_hops = max_hops.min(7);
    if min_hops > max_hops || limits.hop_mask & 0b11_1111 == 0 {
        return MultiHopResult::default();
    }
    let interval_mask = (min_hops..=max_hops).fold(0u8, |mask, hops| mask | (1 << (hops - 2)));
    let effective_mask = limits.hop_mask & interval_mask;
    if effective_mask == 0 {
        return MultiHopResult::default();
    }
    let max_hops = 2 + (7 - effective_mask.leading_zeros() as usize);
    let limits = SearchLimits {
        hop_mask: effective_mask,
        ..limits
    };
    let mut walker = CycleSearch {
        graph,
        min_hops,
        max_hops,
        max_cycles,
        limits,
        began: std::time::Instant::now(),
        result: MultiHopResult::default(),
        seen: std::collections::HashSet::new(),
    };
    // HashMap order must not decide which starting assets receive the budget.
    let mut starts: Vec<_> = graph.adjacency.keys().copied().collect();
    starts.sort_unstable();
    let mut path = Vec::with_capacity(max_hops);
    let mut pools = Vec::with_capacity(max_hops);
    for start in starts {
        if walker.result.capped {
            break;
        }
        let mut visited = std::collections::HashSet::new();
        visited.insert(start);
        let mut dense_visited = graph.dense.as_ref().map(|_| {
            crate::route_discovery::dense_view::TokenBitSet::new(graph.dense_token_count() as u32)
        });
        if let (Some(bits), Some(id)) = (&mut dense_visited, graph.dense_token_id(&start)) {
            bits.set(id as u32);
        }
        walker.dfs(
            start,
            start,
            &mut path,
            &mut pools,
            &mut visited,
            &mut dense_visited,
            0.0,
        );
    }
    walker.result
}

struct CycleSearch<'g> {
    graph: &'g TokenGraph,
    min_hops: usize,
    max_hops: usize,
    max_cycles: usize,
    limits: SearchLimits,
    began: std::time::Instant,
    result: MultiHopResult,
    seen: std::collections::HashSet<Vec<usize>>,
}

impl CycleSearch<'_> {
    fn admit_work(&mut self) -> bool {
        if self.result.edge_visits >= self.limits.max_edge_visits {
            self.result.capped = true;
            self.result.work_limited = true;
            return false;
        }
        if self.result.edge_visits % 64 == 0
            && self
                .limits
                .max_duration
                .is_some_and(|limit| self.began.elapsed() >= limit)
        {
            self.result.capped = true;
            self.result.time_limited = true;
            return false;
        }
        self.result.edge_visits += 1;
        true
    }

    #[allow(clippy::too_many_arguments)]
    fn dfs(
        &mut self,
        start: Address,
        current: Address,
        path: &mut Vec<usize>,
        pools: &mut Vec<Address>,
        visited: &mut std::collections::HashSet<Address>,
        dense_visited: &mut Option<crate::route_discovery::dense_view::TokenBitSet>,
        sum: f64,
    ) {
        if path.len() >= self.max_hops {
            return;
        }
        if let (Some(row), Some(bits), Some(start_id)) = (
            self.graph.dense_neighbors(&current),
            dense_visited.as_ref(),
            self.graph.dense_token_id(&start),
        ) {
            if !row.has_unvisited_or(bits, start_id as u32) {
                return;
            }
        }
        // The graph is borrowed independently of mutable traversal state.
        let graph = self.graph;
        for index in graph.out_edge_indices(&current) {
            if !self.admit_work() {
                return;
            }
            let edge = &graph.edges[index];
            if pools.contains(&edge.pool) {
                continue;
            }
            let destination_id = graph.dense_token_id(&edge.token_out);
            let already_visited = match (dense_visited.as_ref(), destination_id) {
                (Some(bits), Some(id)) => bits.contains(id as u32),
                _ => visited.contains(&edge.token_out),
            };
            if edge.token_out != start && already_visited {
                continue;
            }
            let weight = match edge.log_weight {
                Some(weight) if weight.is_finite() => weight,
                Some(_) => {
                    self.result.invalid_weights += 1;
                    continue;
                }
                None => {
                    self.result.v3_skipped += 1;
                    continue;
                }
            };
            let next_sum = sum + weight;
            if !next_sum.is_finite() {
                self.result.invalid_weights += 1;
                continue;
            }
            path.push(index);
            pools.push(edge.pool);
            if edge.token_out == start {
                let hops = path.len();
                if hops >= self.min_hops
                    && self.limits.hop_mask & (1u8 << (hops - 2)) != 0
                    && next_sum < 0.0
                {
                    if next_sum > -1e-6 {
                        // Noise never consumes the output capacity.
                        self.result.noise_dropped += 1;
                    } else {
                        // Same-direction rotations share one key; reversed
                        // traversals and parallel pools remain distinct.
                        let rotation = path
                            .iter()
                            .enumerate()
                            .min_by_key(|(_, e)| *e)
                            .map(|(i, _)| i)
                            .unwrap_or(0);
                        let canonical: Vec<_> = path[rotation..]
                            .iter()
                            .chain(&path[..rotation])
                            .copied()
                            .collect();
                        if self.limits.deduplicate_rotations && self.seen.contains(&canonical) {
                            self.result.duplicate_cycles += 1;
                        } else if self.result.cycles.len() >= self.max_cycles {
                            self.result.capped = true;
                            self.result.dropped_for_cap += 1;
                        } else {
                            if self.limits.deduplicate_rotations {
                                self.seen.insert(canonical);
                            }
                            self.result.cycles.push(ProfitableCycle {
                                edges: path.clone(),
                                sum_log_weight: next_sum,
                                hop_count: hops,
                            });
                        }
                    }
                }
            } else {
                visited.insert(edge.token_out);
                if let (Some(bits), Some(id)) = (dense_visited.as_mut(), destination_id) {
                    bits.set(id as u32);
                }
                self.dfs(
                    start,
                    edge.token_out,
                    path,
                    pools,
                    visited,
                    dense_visited,
                    next_sum,
                );
                if let (Some(bits), Some(id)) = (dense_visited.as_mut(), destination_id) {
                    bits.clear(id as u32);
                }
                visited.remove(&edge.token_out);
            }
            path.pop();
            pools.pop();
            if self.result.capped {
                return;
            }
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

    #[test]
    fn work_budget_bounds_nonprofitable_dense_graph() {
        let mut edges = Vec::new();
        for a in 1..=22 {
            for b in 1..=22 {
                if a != b {
                    edges.push(edge(a, b, a * 100 + b, Some(0.1)));
                }
            }
        }
        let graph = graph_from(edges);
        let result = find_profitable_cycles_with_limits(
            &graph,
            2,
            7,
            100,
            SearchLimits {
                max_edge_visits: 37,
                ..Default::default()
            },
        );
        assert_eq!(result.edge_visits, 37);
        assert!(result.capped && result.work_limited);
        assert!(result.cycles.is_empty());
    }

    #[test]
    fn mask_excludes_interior_hop_and_rotations_deduplicate() {
        let graph = graph_from(vec![
            edge(1, 2, 10, Some(-0.1)),
            edge(2, 1, 11, Some(-0.1)),
            edge(2, 3, 12, Some(-0.1)),
            edge(3, 1, 13, Some(-0.1)),
            edge(3, 4, 14, Some(-0.1)),
            edge(4, 1, 15, Some(-0.1)),
        ]);
        let result = find_profitable_cycles_with_limits(
            &graph,
            2,
            4,
            100,
            SearchLimits {
                hop_mask: 0b000101,
                deduplicate_rotations: true,
                ..Default::default()
            },
        );
        assert_eq!(result.cycles.len(), 2);
        assert!(result
            .cycles
            .iter()
            .all(|c| c.hop_count == 2 || c.hop_count == 4));
        assert!(result.duplicate_cycles > 0);
    }

    #[test]
    fn nonfinite_weights_never_produce_profit() {
        for bad in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            let graph = graph_from(vec![edge(1, 2, 10, Some(bad)), edge(2, 1, 11, Some(-0.1))]);
            let result = find_profitable_cycles(&graph, 2, 7, 100);
            assert!(result.cycles.is_empty());
            assert!(result.invalid_weights > 0);
        }
    }

    #[test]
    fn noise_does_not_fill_cycle_capacity() {
        let graph = graph_from(vec![
            edge(1, 2, 10, Some(-1e-9)),
            edge(2, 1, 11, Some(-1e-9)),
            edge(3, 4, 12, Some(-0.1)),
            edge(4, 3, 13, Some(-0.1)),
        ]);
        let result = find_profitable_cycles_with_limits(
            &graph,
            2,
            7,
            1,
            SearchLimits {
                deduplicate_rotations: true,
                ..Default::default()
            },
        );
        assert_eq!(result.cycles.len(), 1);
        assert!(result.cycles[0].sum_log_weight < -0.1);
        assert!(result.noise_dropped > 0);
        assert!(!result.capped);
    }

    #[test]
    fn invalid_hop_interval_is_empty_and_zero_deadline_is_observable() {
        let graph = graph_from(vec![edge(1, 2, 10, Some(-0.1)), edge(2, 1, 11, Some(-0.1))]);
        assert!(find_profitable_cycles(&graph, 7, 2, 100).cycles.is_empty());
        let absent = find_profitable_cycles_with_limits(
            &graph,
            2,
            3,
            100,
            SearchLimits {
                hop_mask: 0b100000,
                max_edge_visits: 0,
                ..Default::default()
            },
        );
        assert_eq!(absent.edge_visits, 0);
        assert!(!absent.capped);
        let result = find_profitable_cycles_with_limits(
            &graph,
            2,
            7,
            100,
            SearchLimits {
                max_duration: Some(std::time::Duration::ZERO),
                ..Default::default()
            },
        );
        assert!(result.time_limited && result.capped);
        assert_eq!(result.edge_visits, 0);
    }

    #[test]
    fn capped_search_is_repeatable_for_identical_snapshot() {
        let edges = vec![
            edge(3, 4, 12, Some(-0.1)),
            edge(4, 3, 13, Some(-0.1)),
            edge(1, 2, 10, Some(-0.1)),
            edge(2, 1, 11, Some(-0.1)),
        ];
        let a = find_profitable_cycles(&graph_from(edges.clone()), 2, 7, 1);
        let b = find_profitable_cycles(&graph_from(edges), 2, 7, 1);
        assert_eq!(a.cycles, b.cycles);
    }

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
            hot_token: false,
            direction: RouteDirection::ZeroForOne,
        }
    }

    fn graph_from(edges: Vec<RouteEdge>) -> TokenGraph {
        let mut adjacency: HashMap<Address, Vec<usize>> = HashMap::new();
        for (i, e) in edges.iter().enumerate() {
            adjacency.entry(e.token_in).or_default().push(i);
        }
        TokenGraph {
            edges,
            adjacency,
            dense: None,
        }
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

    /// ARBX-0019: the SAME walker run over the dense O(1) view must find the
    /// SAME cycles as the HashMap fallback above (differential at the walker
    /// level — existing tests exercise the fallback, this one the CSR path).
    /// ARBX-0006 drift-fix: comparison is set-based (see inside).
    #[test]
    fn dense_view_finds_identical_cycles_as_hashmap_fallback() {
        let edges = vec![
            edge(0xA, 0xB, 1, Some(-0.02)),
            edge(0xB, 0xA, 2, Some(-0.01)),
            edge(0xB, 0xC, 3, Some(-0.02)),
            edge(0xC, 0xA, 4, Some(-0.02)),
        ];
        let fallback = graph_from(edges.clone());
        let mut dense = graph_from(edges);
        dense.build_dense(1);
        assert!(dense
            .dense_out_indices(&Address::from_low_u64_be(0xB))
            .is_some());

        let r_hash = find_profitable_cycles(&fallback, 2, 7, 100);
        let r_dense = find_profitable_cycles(&dense, 2, 7, 100);
        assert_eq!(r_hash.capped, r_dense.capped);
        assert_eq!(r_hash.cycles.len(), r_dense.cycles.len());
        // The walker iterates start tokens in HashMap order, which differs
        // BETWEEN the two graph instances (RandomState seeds) — emission
        // order is not a stable contract. The differential contract is the
        // SET of cycles: canonicalize by edge sequence, then compare.
        let mut h = r_hash.cycles;
        let mut d = r_dense.cycles;
        h.sort_by_key(|c| c.edges.clone());
        d.sort_by_key(|c| c.edges.clone());
        assert_eq!(h, d, "identical cycle SET both views");
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
