//! UniqueRouteFinder — bounded DFS (2–3 hops) over the live token graph.
//!
//! Enumerates **closed cycles** (an arb realizes its yield by returning to the
//! start token) discovered *from the live graph*, not from any fixed list:
//! - **2-cycle** `A→B→A` over two distinct pools ⇒ DEX cross-arb
//!   (`V2V2`/`V2V3`/`V3V2`/`V3V3`).
//! - **3-cycle** `A→B→C→A` ⇒ `Triangular`, discovered by traversal (the
//!   `MVP_CYCLES` constant is only a future *seed/ordering* hint, never the
//!   source of truth).
//!
//! ## Invariants
//! - No pool is reused within a cycle; no token repeats except the closing one.
//! - Opposite traversal directions are **distinct** routes (canonical hash
//!   preserves direction); same-direction rotations **dedup** to one route.
//! - **Bounded**: depth ∈ {2, `max_depth`}, `max_pools_per_pair` caps parallel
//!   pools between two tokens, `max_routes_per_tick` caps total output. When the
//!   route cap is hit, enumeration stops early and `capped` is set so consumers
//!   know the result set is **incomplete** (R8 fail-honest — we signal
//!   truncation rather than implying completeness). `dropped_for_cap` is a
//!   *lower bound* on suppressed cycles: it counts only cycles dropped at the
//!   emission point, not the whole unexplored subtrees/start-tokens abandoned
//!   once the cap is reached.
//!
//! Phase 1 is topology-only: candidates carry no sizing/profit, and
//! `applicable_strategies`/`rejected_strategies` are filled later by the
//! `strategy_applicability` engine.

use crate::route_discovery::canonicalizer::canonicalize;
use crate::route_discovery::graph_builder::TokenGraph;
use crate::route_discovery::types::{RouteCandidate, RouteDirection, RouteEdge, RouteKind};
use crate::route_intent::ProtocolType;
use ethers::types::Address;
use std::collections::{HashMap, HashSet};

/// Tunables for the bounded DFS. Defaults mirror the env caps in the plan.
#[derive(Debug, Clone)]
pub struct RouteFinderConfig {
    /// Maximum cycle length in hops (default 3 → 2- and 3-cycles).
    pub max_depth: u8,
    /// Maximum parallel pools explored between any two tokens (branching cap).
    pub max_pools_per_pair: usize,
    /// Maximum candidates emitted per tick (anti-explosion).
    pub max_routes_per_tick: usize,
    /// Start tokens; empty ⇒ every token in the graph is a start.
    pub base_tokens: Vec<Address>,
    /// Discovery mode stamped onto each candidate (e.g. `"shadow"`).
    pub mode: String,
}

impl Default for RouteFinderConfig {
    fn default() -> Self {
        Self {
            max_depth: 3,
            max_pools_per_pair: 8,
            max_routes_per_tick: 500,
            base_tokens: Vec::new(),
            mode: "shadow".to_string(),
        }
    }
}

/// Output of one finder pass.
#[derive(Debug, Clone, Default)]
pub struct RouteFinderOutcome {
    pub routes: Vec<RouteCandidate>,
    /// Lower bound on cycles suppressed by `max_routes_per_tick` — counts only
    /// cycles dropped at the emission point. NOT a complete total: whole
    /// subtrees/start-tokens abandoned once the cap is hit are not counted.
    pub dropped_for_cap: usize,
    /// `true` when the route cap stopped enumeration early ⇒ the route set is
    /// **incomplete** (R8 fail-honest: signal truncation, don't imply completeness).
    pub capped: bool,
}

struct FinderState<'g> {
    graph: &'g TokenGraph,
    chain_id: u64,
    cfg: &'g RouteFinderConfig,
    seen: HashSet<String>,
    results: Vec<RouteCandidate>,
    dropped_for_cap: usize,
    capped: bool,
}

impl<'g> FinderState<'g> {
    /// Out-edges leaving `token`, capped to `max_pools_per_pair` per destination.
    fn collect_out_edges(&self, token: Address) -> Vec<RouteEdge> {
        let mut per_pair: HashMap<Address, usize> = HashMap::new();
        let mut out = Vec::new();
        for e in self.graph.out_edges(&token) {
            let c = per_pair.entry(e.token_out).or_insert(0);
            if *c >= self.cfg.max_pools_per_pair {
                continue;
            }
            *c += 1;
            out.push(e.clone());
        }
        out
    }

    fn dfs(
        &mut self,
        start: Address,
        current: Address,
        path: &mut Vec<RouteEdge>,
        pools_used: &mut HashSet<Address>,
        visited_tokens: &mut HashSet<Address>,
    ) {
        if self.results.len() >= self.cfg.max_routes_per_tick {
            self.capped = true; // stopped exploring this subtree because the cap is full
            return;
        }
        let depth = path.len();
        if depth >= self.cfg.max_depth as usize {
            return; // no more edges may be taken
        }

        // Clone out-edges first so we don't hold an immutable borrow of the
        // graph across the mutable `self` recursion.
        let out = self.collect_out_edges(current);
        for edge in out {
            if pools_used.contains(&edge.pool) {
                continue; // never reuse a pool within a cycle
            }
            let nt = edge.token_out;
            let new_depth = depth + 1;

            if nt == start {
                // Closes a cycle of length `new_depth`. Emit when 2 ≤ len ≤ max.
                if (2..=self.cfg.max_depth as usize).contains(&new_depth) {
                    path.push(edge.clone());
                    self.try_emit(path);
                    path.pop();
                }
                continue; // a simple cycle closes exactly once — don't extend
            }

            if visited_tokens.contains(&nt) {
                continue; // no repeated intermediate token
            }
            // Only recurse if another edge can still be taken to reach a close.
            if new_depth < self.cfg.max_depth as usize {
                path.push(edge.clone());
                pools_used.insert(edge.pool);
                visited_tokens.insert(nt);
                self.dfs(start, nt, path, pools_used, visited_tokens);
                visited_tokens.remove(&nt);
                pools_used.remove(&edge.pool);
                path.pop();
            }
            // else: new_depth == max_depth and nt != start → can't close → prune
        }
    }

    fn try_emit(&mut self, path: &[RouteEdge]) {
        if self.results.len() >= self.cfg.max_routes_per_tick {
            self.dropped_for_cap += 1;
            self.capped = true;
            return;
        }
        let l = path.len();
        let tokens: Vec<Address> = path.iter().map(|e| e.token_in).collect();
        let pools: Vec<Address> = path.iter().map(|e| e.pool).collect();
        let protocols: Vec<ProtocolType> = path.iter().map(|e| e.protocol).collect();
        let fee_tiers: Vec<Option<u32>> = path.iter().map(|e| e.fee_bps).collect();
        let directions: Vec<RouteDirection> = path.iter().map(|e| e.direction).collect();

        // Canonicalize FIRST (rotates to the smallest start token), then derive
        // route_kind from the canonical protocol order — so the same physical
        // cycle dedups regardless of which token discovery started from.
        let canon = match canonicalize(
            self.chain_id,
            &tokens,
            &pools,
            &protocols,
            &fee_tiers,
            &directions,
        ) {
            Some(c) => c,
            None => return,
        };

        // Graph holds only V2/V3 edges, so a 2-hop classify is always Some; bail safely otherwise.
        let route_kind = match RouteKind::classify(&canon.protocols) {
            Some(k) => k,
            None => return,
        };

        // Dedup on the canonical hash: rotations collapse, inverses are kept.
        if !self.seen.insert(canon.route_hash.clone()) {
            return;
        }

        self.results.push(RouteCandidate {
            chain_id: self.chain_id,
            route_hash: canon.route_hash,
            route_kind,
            tokens: canon.tokens,
            pools: canon.pools,
            protocols: canon.protocols,
            fee_tiers: canon.fee_tiers,
            directions: canon.directions,
            hops: l as u8,
            applicable_strategies: Vec::new(),
            rejected_strategies: Vec::new(),
            mode: self.cfg.mode.clone(),
        });
    }
}

/// Enumerate unique closed cycles over `graph` using the bounded DFS.
pub fn find_routes(
    graph: &TokenGraph,
    chain_id: u64,
    cfg: &RouteFinderConfig,
) -> RouteFinderOutcome {
    let mut state = FinderState {
        graph,
        chain_id,
        cfg,
        seen: HashSet::new(),
        results: Vec::new(),
        dropped_for_cap: 0,
        capped: false,
    };

    let starts: Vec<Address> = if cfg.base_tokens.is_empty() {
        graph.tokens().cloned().collect()
    } else {
        cfg.base_tokens
            .iter()
            .filter(|t| graph.adjacency.contains_key(*t))
            .cloned()
            .collect()
    };

    for start in starts {
        if state.results.len() >= cfg.max_routes_per_tick {
            state.capped = true; // remaining start tokens abandoned — set truncated
            break;
        }
        let mut path: Vec<RouteEdge> = Vec::new();
        let mut pools_used: HashSet<Address> = HashSet::new();
        let mut visited: HashSet<Address> = HashSet::new();
        visited.insert(start);
        state.dfs(start, start, &mut path, &mut pools_used, &mut visited);
    }

    RouteFinderOutcome {
        routes: state.results,
        dropped_for_cap: state.dropped_for_cap,
        capped: state.capped,
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    fn addr(n: u64) -> Address {
        Address::from_low_u64_be(n)
    }

    fn dir_edge(pool: u64, ti: u64, to: u64, t0: u64, t1: u64, proto: ProtocolType) -> RouteEdge {
        RouteEdge {
            chain_id: 1,
            pool: addr(pool),
            token_in: addr(ti),
            token_out: addr(to),
            token0: addr(t0),
            token1: addr(t1),
            protocol: proto,
            fee_bps: Some(30),
            liquidity_hint: Some(1.0),
            log_weight: None,
            freshness_ts: 0,
            blk: 0,
            direction: RouteDirection::from_in_token0(addr(ti), addr(t0)),
        }
    }

    /// Build a graph from `(pool, token0, token1, protocol)` rows (both
    /// directions generated per pool).
    fn graph_from(pools: &[(u64, u64, u64, ProtocolType)]) -> TokenGraph {
        let mut edges = Vec::new();
        for &(p, t0, t1, proto) in pools {
            edges.push(dir_edge(p, t0, t1, t0, t1, proto));
            edges.push(dir_edge(p, t1, t0, t0, t1, proto));
        }
        let mut adjacency: HashMap<Address, Vec<usize>> = HashMap::new();
        for (i, e) in edges.iter().enumerate() {
            adjacency.entry(e.token_in).or_default().push(i);
        }
        TokenGraph { edges, adjacency }
    }

    fn kinds(o: &RouteFinderOutcome) -> Vec<RouteKind> {
        let mut k: Vec<RouteKind> = o.routes.iter().map(|r| r.route_kind).collect();
        k.sort_by_key(|x| x.as_str());
        k
    }

    #[test]
    fn two_cycle_v2v2_found_once_across_starts() {
        use ProtocolType::V2;
        // Two distinct V2 pools over the same pair (A,B).
        let g = graph_from(&[(0x10, 1, 2, V2), (0x20, 1, 2, V2)]);
        let o = find_routes(&g, 1, &RouteFinderConfig::default());
        // A→B(p10)→A(p20) and A→B(p20)→A(p10) are opposite orders → 2 distinct
        // routes; starting from B dedups to the same two.
        assert_eq!(o.routes.len(), 2, "two opposite-order 2-cycles");
        for r in &o.routes {
            assert_eq!(r.hops, 2);
            assert_eq!(r.route_kind, RouteKind::V2V2);
            assert_eq!(r.pools.len(), 2);
            assert_ne!(r.pools[0], r.pools[1], "distinct pools");
        }
        // The two routes are inverses → different hashes.
        assert_ne!(o.routes[0].route_hash, o.routes[1].route_hash);
    }

    #[test]
    fn two_cycle_mixed_protocols_yield_v2v3_and_v3v2() {
        use ProtocolType::{V2, V3};
        let g = graph_from(&[(0x10, 1, 2, V2), (0x20, 1, 2, V3)]);
        let o = find_routes(&g, 1, &RouteFinderConfig::default());
        assert_eq!(o.routes.len(), 2);
        assert_eq!(kinds(&o), vec![RouteKind::V2V3, RouteKind::V3V2]);
    }

    #[test]
    fn triangular_discovered_from_graph_both_directions() {
        use ProtocolType::V2;
        // Triangle A-B, B-C, C-A.
        let g = graph_from(&[(0x10, 1, 2, V2), (0x20, 2, 3, V2), (0x30, 3, 1, V2)]);
        let o = find_routes(&g, 1, &RouteFinderConfig::default());
        // Clockwise A→B→C→A and counter-clockwise A→C→B→A: 2 distinct triangulars.
        let tri: Vec<_> = o
            .routes
            .iter()
            .filter(|r| r.route_kind == RouteKind::Triangular)
            .collect();
        assert_eq!(tri.len(), 2, "both cycle directions discovered from the graph");
        for r in &tri {
            assert_eq!(r.hops, 3);
            assert_eq!(r.tokens.len(), 3);
            assert_eq!(r.pools.len(), 3);
            // pools all distinct
            let mut ps = r.pools.clone();
            ps.sort();
            ps.dedup();
            assert_eq!(ps.len(), 3);
        }
        assert_ne!(tri[0].route_hash, tri[1].route_hash);
    }

    #[test]
    fn single_pool_cannot_form_a_cycle() {
        use ProtocolType::V2;
        // One pool only — a 2-cycle would need to reuse it → none.
        let g = graph_from(&[(0x10, 1, 2, V2)]);
        let o = find_routes(&g, 1, &RouteFinderConfig::default());
        assert!(o.routes.is_empty());
    }

    #[test]
    fn open_path_with_no_closing_edge_yields_nothing() {
        use ProtocolType::V2;
        // A-B, B-C but NO C-A and only one A-B pool → no closable cycle.
        let g = graph_from(&[(0x10, 1, 2, V2), (0x20, 2, 3, V2)]);
        let o = find_routes(&g, 1, &RouteFinderConfig::default());
        assert!(o.routes.is_empty());
    }

    #[test]
    fn max_depth_2_excludes_triangulars() {
        use ProtocolType::V2;
        let g = graph_from(&[(0x10, 1, 2, V2), (0x20, 2, 3, V2), (0x30, 3, 1, V2)]);
        let cfg = RouteFinderConfig {
            max_depth: 2,
            ..Default::default()
        };
        let o = find_routes(&g, 1, &cfg);
        // Each pair has a single pool → no 2-cycle possible, and triangulars are
        // excluded by depth → zero routes.
        assert!(o.routes.is_empty());
    }

    #[test]
    fn max_routes_per_tick_caps_and_counts_overflow() {
        use ProtocolType::V2;
        let g = graph_from(&[(0x10, 1, 2, V2), (0x20, 1, 2, V2), (0x30, 1, 2, V2)]);
        // 3 pools over (A,B) → several ordered 2-cycles; cap to 1.
        let cfg = RouteFinderConfig {
            max_routes_per_tick: 1,
            ..Default::default()
        };
        let o = find_routes(&g, 1, &cfg);
        assert_eq!(o.routes.len(), 1);
        assert!(o.dropped_for_cap >= 1, "overflow counted, not silently dropped");
        assert!(o.capped, "cap hit → result set flagged incomplete (R8 fail-honest)");
    }

    #[test]
    fn uncapped_run_is_not_flagged() {
        use ProtocolType::V2;
        let g = graph_from(&[(0x10, 1, 2, V2), (0x20, 1, 2, V2)]);
        let o = find_routes(&g, 1, &RouteFinderConfig::default());
        assert!(!o.capped, "ample cap → complete result set, not flagged");
        assert_eq!(o.dropped_for_cap, 0);
    }

    #[test]
    fn base_tokens_filter_restricts_starts() {
        use ProtocolType::V2;
        // Triangle; restrict starts to a token NOT in the graph → nothing.
        let g = graph_from(&[(0x10, 1, 2, V2), (0x20, 2, 3, V2), (0x30, 3, 1, V2)]);
        let cfg = RouteFinderConfig {
            base_tokens: vec![addr(999)],
            ..Default::default()
        };
        let o = find_routes(&g, 1, &cfg);
        assert!(o.routes.is_empty());

        // Restrict to A: every triangle route passes through A, so still found.
        let cfg2 = RouteFinderConfig {
            base_tokens: vec![addr(1)],
            ..Default::default()
        };
        let o2 = find_routes(&g, 1, &cfg2);
        assert_eq!(
            o2.routes
                .iter()
                .filter(|r| r.route_kind == RouteKind::Triangular)
                .count(),
            2
        );
    }

    #[test]
    fn candidates_are_topology_only() {
        use ProtocolType::V2;
        let g = graph_from(&[(0x10, 1, 2, V2), (0x20, 1, 2, V2)]);
        let o = find_routes(&g, 1, &RouteFinderConfig::default());
        for r in &o.routes {
            assert!(r.applicable_strategies.is_empty());
            assert!(r.rejected_strategies.is_empty());
            assert_eq!(r.mode, "shadow");
            assert!(r.route_hash.starts_with("0x"));
        }
    }
}
