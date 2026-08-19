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
    /// Enumeration policy (SHADOW-NO-ROUTE-CAPS, 2026-08-18). `BoundedLegacy`
    /// is the original per-tick capped DFS (for the live hot path, where the
    /// cap bounds emission work). `DeferNeverDrop` NEVER loses a route: the
    /// budget only decides where to PAUSE — the traversal cursor persists
    /// across ticks and resumes exactly where it stopped, so the enumeration
    /// is exhaustive over 2..=max_depth cycles in finite ticks (operator
    /// directive: shadow must never cap routes).
    pub policy: CapPolicy,
}

/// Enumeration policy — see `RouteFinderConfig::policy`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CapPolicy {
    /// Original behaviour: per-tick cap, dropped routes lost until restart.
    #[default]
    BoundedLegacy,
    /// Budget-deferred exhaustive enumeration with a persistent cursor.
    DeferNeverDrop,
}

impl Default for RouteFinderConfig {
    fn default() -> Self {
        Self {
            max_depth: 3,
            max_pools_per_pair: 8,
            max_routes_per_tick: 500,
            base_tokens: Vec::new(),
            mode: "shadow".to_string(),
            policy: CapPolicy::default(),
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
    /// `true` when `max_pools_per_pair` dropped one or more parallel pools between
    /// some token pair during traversal. Distinct from `capped` (which is about the
    /// total route cap): even with `capped == false`, a `pools_truncated == true`
    /// means "all cycles over the *retained* pools were explored, but some pools were
    /// excluded by the per-pair branching cap" — so the set is not provably exhaustive
    /// over the full pool universe (R8: don't let a per-pair drop masquerade as complete).
    pub pools_truncated: bool,
}

struct FinderState<'g> {
    graph: &'g TokenGraph,
    chain_id: u64,
    cfg: &'g RouteFinderConfig,
    seen: HashSet<String>,
    results: Vec<RouteCandidate>,
    dropped_for_cap: usize,
    capped: bool,
    pools_truncated: bool,
}

impl<'g> FinderState<'g> {
    /// Out-edges leaving `token`, capped to `max_pools_per_pair` per destination.
    /// Returns `(edges, truncated)` where `truncated` is `true` iff at least one
    /// parallel pool was dropped by the per-pair cap — so the caller can flag the
    /// result set as not-provably-exhaustive (R8) instead of silently swallowing it.
    fn collect_out_edges(&self, token: Address) -> (Vec<RouteEdge>, bool) {
        let mut per_pair: HashMap<Address, usize> = HashMap::new();
        let mut out = Vec::new();
        let mut truncated = false;
        for e in self.graph.out_edges(&token) {
            let c = per_pair.entry(e.token_out).or_insert(0);
            if *c >= self.cfg.max_pools_per_pair {
                truncated = true; // a real pool exists but was excluded by the branching cap
                continue;
            }
            *c += 1;
            out.push(e.clone());
        }
        (out, truncated)
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
        let (out, truncated) = self.collect_out_edges(current);
        if truncated {
            self.pools_truncated = true; // R8: a parallel pool was dropped here
        }
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
        if let Some(c) = build_candidate(self.chain_id, &self.cfg.mode, path) {
            // Dedup on the canonical hash: rotations collapse, inverses kept.
            if self.seen.insert(c.route_hash.clone()) {
                self.results.push(c);
            }
        }
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
        pools_truncated: false,
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
        pools_truncated: state.pools_truncated,
    }
}

/// Shared candidate construction (canonicalize → classify → build), used by
/// BOTH engines so the legacy DFS and the incremental engine can never drift
/// on emission semantics. Returns `None` when canonicalization/classification
/// rejects the path (R8: nothing fabricated). Dedup is the CALLER's concern —
/// legacy dedups per tick, the incremental engine per ladder.
fn build_candidate(chain_id: u64, mode: &str, path: &[RouteEdge]) -> Option<RouteCandidate> {
    let l = path.len();
    let tokens: Vec<Address> = path.iter().map(|e| e.token_in).collect();
    let pools: Vec<Address> = path.iter().map(|e| e.pool).collect();
    let protocols: Vec<ProtocolType> = path.iter().map(|e| e.protocol).collect();
    let fee_tiers: Vec<Option<u32>> = path.iter().map(|e| e.fee_bps).collect();
    let directions: Vec<RouteDirection> = path.iter().map(|e| e.direction).collect();

    // Canonicalize FIRST (rotates to the smallest start token), then derive
    // route_kind from the canonical protocol order — so the same physical
    // cycle dedups regardless of which token discovery started from.
    let canon = canonicalize(
        chain_id,
        &tokens,
        &pools,
        &protocols,
        &fee_tiers,
        &directions,
    )?;
    // Graph holds only V2/V3 edges, so a 2-hop classify is always Some; bail safely otherwise.
    let route_kind = RouteKind::classify(&canon.protocols)?;

    Some(RouteCandidate {
        chain_id,
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
        mode: mode.to_string(),
    })
}

// ---------------------------------------------------------------------------
// Incremental engine — DeferNeverDrop (SHADOW-NO-ROUTE-CAPS, 2026-08-18)
// ---------------------------------------------------------------------------

/// One explicit-DFS stack frame: the paused `for edge in out` loop of the
/// recursive engine. `edge_taken = None` identifies the root frame (the start
/// token itself — nothing to restore when it pops).
struct Frame {
    /// Rotated out-edge snapshot for the node this frame sits on. Snapshotted
    /// on push so mid-ladder rotation-epoch changes cannot reorder a live
    /// traversal (rotation only advances between ladders).
    out: Vec<RouteEdge>,
    /// Index of the next sibling edge to try when execution returns here.
    next: usize,
    /// The edge that led INTO this frame's node (`None` for the root).
    edge_taken: Option<RouteEdge>,
}

/// Output of one incremental tick. NOTHING here means "routes were lost" —
/// `deferred` means the budget paused the pass and the cursor persists.
#[derive(Debug)]
pub struct IncrementalOutcome {
    /// Candidates emitted THIS tick (≤ the tick budget).
    pub routes: Vec<RouteCandidate>,
    /// Budget paused the pass mid-traversal — next tick resumes at the cursor.
    pub deferred: bool,
    /// The full 2..=max_depth ladder COMPLETED this tick (exhaustive over the
    /// retained pools): every closed cycle was emitted exactly once across the
    /// ladder's ticks. A new ladder starts on the next tick.
    pub ladder_complete: bool,
    /// Current iterative-deepening depth (2..=max_depth).
    pub depth_pass: u8,
    /// Rotation epoch — increments once per completed ladder, rotating which
    /// parallel pools per token-pair participate next ladder.
    pub rotation_epoch: u64,
    /// Some pair had more parallel pools than the branching cap this tick —
    /// the retained window rotates per ladder (exhaustive across ladders).
    pub pools_rotated: bool,
    /// Total candidates emitted so far in the CURRENT ladder (across ticks).
    pub pass_emitted_total: usize,
    /// Human-readable cursor for telemetry, e.g. `"d3@t12:f2"`.
    pub cursor: String,
}

/// Persistent exhaustive route finder (`CapPolicy::DeferNeverDrop`).
///
/// The tick budget NEVER drops a route: it only decides where the traversal
/// PAUSES. The explicit DFS stack, start-token index, per-ladder dedup set and
/// iterative-deepening depth live in `self` across `next_tick` calls, so the
/// enumeration continues exactly where it stopped. Exhaustiveness contract:
/// over a static graph, the union of every tick's routes within one ladder
/// equals the complete set of closed cycles of length 2..=max_depth over the
/// retained (rotated) pool set — pinned by the H1 test against a legacy
/// uncapped run. A graph-generation change restarts the ladder (the topology
/// changed; re-validating from depth 2 is correct, not a loss).
pub struct UniqueRouteFinder {
    cfg: RouteFinderConfig,
    /// Rotation epoch lives on the FINDER so it survives ladder folds.
    rotation_epoch: u64,
    pass: Option<PassState>,
}

struct PassState {
    graph_gen: u64,
    depth_pass: u8,
    starts: Vec<Address>,
    start_idx: usize,
    stack: Vec<Frame>,
    /// Canonical hashes emitted this LADDER (persists across ticks AND depth
    /// passes so a shorter cycle is never re-emitted by a deeper pass).
    seen: HashSet<String>,
    pools_rotated_ever: bool,
    pass_emitted_total: usize,
    // Traversal state, maintained in lockstep with `stack`:
    path: Vec<RouteEdge>,
    pools_used: HashSet<Address>,
    visited: HashSet<Address>,
}

impl UniqueRouteFinder {
    pub fn new(cfg: RouteFinderConfig) -> Self {
        Self {
            cfg,
            rotation_epoch: 0,
            pass: None,
        }
    }

    /// Configuration snapshot (read-only access for callers that need the
    /// bounds, e.g. the worker's telemetry).
    pub fn config(&self) -> &RouteFinderConfig {
        &self.cfg
    }

    /// Out-edges of `token` with the per-pair branching cap applied as a
    /// ROTATED window: pairs with more parallel pools than the cap retain a
    /// deterministic window offset by `rotation_epoch`, so every parallel pool
    /// participates in some ladder (exhaustive across ladders, never silently
    /// dropped forever).
    fn rotated_out_edges(
        graph: &TokenGraph,
        token: &Address,
        cap: usize,
        rotation_epoch: u64,
        rotated: &mut bool,
    ) -> Vec<RouteEdge> {
        let mut by_pair: HashMap<Address, Vec<RouteEdge>> = HashMap::new();
        let mut pair_order: Vec<Address> = Vec::new();
        for e in graph.out_edges(token) {
            let entry = by_pair.entry(e.token_out).or_insert_with(|| {
                pair_order.push(e.token_out);
                Vec::new()
            });
            entry.push(e.clone());
        }
        let mut out = Vec::new();
        for pair in pair_order {
            let group = &by_pair[&pair];
            if group.len() <= cap {
                out.extend(group.iter().cloned());
                continue;
            }
            *rotated = true; // a real parallel pool exists outside this ladder's window
            let offset = (rotation_epoch % group.len() as u64) as usize;
            for i in 0..cap {
                out.push(group[(offset + i) % group.len()].clone());
            }
        }
        out
    }

    /// One tick of the exhaustive enumeration. Continues from the persisted
    /// cursor; emits at most `budget` candidates; defers (never drops) the rest.
    pub fn next_tick(
        &mut self,
        graph: &TokenGraph,
        chain_id: u64,
        graph_gen: u64,
        budget: usize,
    ) -> IncrementalOutcome {
        let max_depth = self.cfg.max_depth.max(2);
        // (Re)start the ladder when there is no pass or the graph changed.
        if self.pass.as_ref().map(|p| p.graph_gen) != Some(graph_gen) {
            let starts: Vec<Address> = if self.cfg.base_tokens.is_empty() {
                graph.tokens().cloned().collect()
            } else {
                self.cfg
                    .base_tokens
                    .iter()
                    .filter(|t| graph.adjacency.contains_key(*t))
                    .cloned()
                    .collect()
            };
            self.pass = Some(PassState {
                graph_gen,
                depth_pass: 2,
                starts,
                start_idx: 0,
                stack: Vec::new(),
                seen: HashSet::new(),
                pools_rotated_ever: false,
                pass_emitted_total: 0,
                path: Vec::new(),
                pools_used: HashSet::new(),
                visited: HashSet::new(),
            });
        }
        let cfg = self.cfg.clone();
        let rotation_epoch = self.rotation_epoch;
        let pass = self.pass.as_mut().expect("pass initialized above");
        let mut routes: Vec<RouteCandidate> = Vec::new();
        let mut deferred = false;
        let mut ladder_complete = false;
        let cap = cfg.max_pools_per_pair.max(1);
        let budget = budget.max(1);

        // ── Explicit DFS step loop — the paused recursion ──────────────────
        // Invariants mirrored 1:1 from the recursive engine: no pool reuse in
        // a cycle, no repeated intermediate token, closure only back to the
        // start, prune when the next hop cannot close within depth_pass.
        'tick: loop {
            // Root frame for the current start token when the stack is empty.
            if pass.stack.is_empty() {
                if pass.start_idx >= pass.starts.len() {
                    // Depth pass complete → deepen (seen persists: shorter
                    // cycles dedup out on the deeper pass) or finish ladder.
                    if pass.depth_pass >= max_depth {
                        ladder_complete = true;
                        break 'tick;
                    }
                    pass.depth_pass += 1;
                    pass.start_idx = 0;
                    continue 'tick;
                }
                let start = pass.starts[pass.start_idx];
                let out = Self::rotated_out_edges(
                    graph,
                    &start,
                    cap,
                    rotation_epoch,
                    &mut pass.pools_rotated_ever,
                );
                pass.stack.push(Frame {
                    out,
                    next: 0,
                    edge_taken: None,
                });
                continue 'tick;
            }

            // Budget check at the recursion-entry equivalent: stop exploring
            // NEW siblings; the stack IS the resume state.
            if routes.len() >= budget {
                deferred = true;
                break 'tick;
            }

            let exhaust_current = {
                let frame = pass.stack.last().expect("non-empty checked above");
                frame.next >= frame.out.len()
            };
            if exhaust_current {
                // Subtree exhausted → pop and restore traversal state.
                let popped = pass.stack.pop().expect("non-empty checked above");
                match popped.edge_taken {
                    Some(e) => {
                        pass.visited.remove(&e.token_out);
                        pass.pools_used.remove(&e.pool);
                        pass.path.pop();
                    }
                    None => {
                        // Root frame popped → this start token is done.
                        pass.start_idx += 1;
                        pass.visited.clear();
                        pass.pools_used.clear();
                        pass.path.clear();
                    }
                }
                continue 'tick;
            }

            let edge = {
                let frame = pass.stack.last_mut().expect("non-empty checked above");
                let e = frame.out[frame.next].clone();
                frame.next += 1;
                e
            };
            let start = pass.starts[pass.start_idx];
            let nt = edge.token_out;
            let nd = pass.path.len() + 1;

            if pass.pools_used.contains(&edge.pool) {
                continue 'tick; // never reuse a pool within a cycle
            }
            if nt == start {
                // Closes a cycle of length nd. Emit when within the ladder.
                if (2..=max_depth as usize).contains(&nd) {
                    pass.path.push(edge.clone());
                    if let Some(c) = build_candidate(chain_id, &cfg.mode, &pass.path) {
                        if pass.seen.insert(c.route_hash.clone()) {
                            routes.push(c);
                            pass.pass_emitted_total += 1;
                        }
                    }
                    pass.path.pop();
                }
                continue 'tick; // a simple cycle closes exactly once
            }
            if pass.visited.contains(&nt) {
                continue 'tick; // no repeated intermediate token
            }
            if nd < pass.depth_pass as usize {
                // Descend: push child frame, extend traversal state.
                let out = Self::rotated_out_edges(
                    graph,
                    &nt,
                    cap,
                    rotation_epoch,
                    &mut pass.pools_rotated_ever,
                );
                pass.path.push(edge.clone());
                pass.pools_used.insert(edge.pool);
                pass.visited.insert(nt);
                pass.stack.push(Frame {
                    out,
                    next: 0,
                    edge_taken: Some(edge),
                });
            }
            // else: nd == depth_pass and nt != start → cannot close → prune.
        }

        let cursor = format!(
            "d{}@t{}:f{}",
            pass.depth_pass,
            pass.start_idx,
            pass.stack.len()
        );
        if ladder_complete {
            // Fold the completed ladder: rotate the parallel-pool window and
            // start fresh next tick (re-validates the whole topology — with a
            // static graph the next ladder re-emits the same exhaustive set).
            let summary = IncrementalOutcome {
                routes,
                deferred: false,
                ladder_complete: true,
                depth_pass: max_depth,
                rotation_epoch,
                pools_rotated: pass.pools_rotated_ever,
                pass_emitted_total: pass.pass_emitted_total,
                cursor,
            };
            self.rotation_epoch = rotation_epoch.wrapping_add(1);
            self.pass = None; // next_tick reinitializes with the bumped epoch
            return summary;
        }

        IncrementalOutcome {
            routes,
            deferred,
            ladder_complete: false,
            depth_pass: pass.depth_pass,
            rotation_epoch,
            pools_rotated: pass.pools_rotated_ever,
            pass_emitted_total: pass.pass_emitted_total,
            cursor,
        }
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
        assert_eq!(
            tri.len(),
            2,
            "both cycle directions discovered from the graph"
        );
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
        assert!(
            o.dropped_for_cap >= 1,
            "overflow counted, not silently dropped"
        );
        assert!(
            o.capped,
            "cap hit → result set flagged incomplete (R8 fail-honest)"
        );
    }

    #[test]
    fn uncapped_run_is_not_flagged() {
        use ProtocolType::V2;
        let g = graph_from(&[(0x10, 1, 2, V2), (0x20, 1, 2, V2)]);
        let o = find_routes(&g, 1, &RouteFinderConfig::default());
        assert!(!o.capped, "ample cap → complete result set, not flagged");
        assert_eq!(o.dropped_for_cap, 0);
        // 2 pools < default max_pools_per_pair (8) → nothing dropped per-pair.
        assert!(!o.pools_truncated, "no per-pair drop → not flagged");
    }

    #[test]
    fn per_pair_cap_sets_pools_truncated_flag() {
        use ProtocolType::V2;
        // Two parallel pools over (A,B); cap to 1 per pair → ≥1 pool excluded.
        let g = graph_from(&[(0x10, 1, 2, V2), (0x20, 1, 2, V2)]);
        let cfg = RouteFinderConfig {
            max_pools_per_pair: 1,
            ..Default::default()
        };
        let o = find_routes(&g, 1, &cfg);
        assert!(
            o.pools_truncated,
            "per-pair cap dropped a real pool → must be flagged (R8 fail-honest)"
        );
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

    // ── H1 (SHADOW-NO-ROUTE-CAPS): exhaustividad matemática del engine ──────
    //
    // El oráculo es el propio motor legacy SIN cap (budget enorme): su
    // conjunto exhaustivo == la UNIÓN de los ticks incrementales con budget
    // mínimo. Si DeferNeverDrop perdiera UNA sola ruta, este test falla.

    fn defer_cfg(max_depth: u8) -> RouteFinderConfig {
        RouteFinderConfig {
            max_depth,
            max_pools_per_pair: 8,
            max_routes_per_tick: 500, // budget per TICK is set per-test below
            base_tokens: Vec::new(),
            mode: "shadow".to_string(),
            policy: CapPolicy::DeferNeverDrop,
        }
    }

    /// Grafo con 2- y 3-ciclos de verdad: triángulo A-B-C + paralelo D-E.
    fn triangle_graph() -> TokenGraph {
        use ProtocolType::V2;
        graph_from(&[
            (0x10, 1, 2, V2),
            (0x20, 1, 2, V2), // paralelo sobre (A,B)
            (0x30, 2, 3, V2),
            (0x40, 1, 3, V2),
            (0x50, 4, 5, V2),
            (0x60, 4, 5, V2),
        ])
    }

    #[test]
    fn h1_exhaustive_union_equals_uncapped_legacy() {
        let g = triangle_graph();
        let depth = 3u8;
        // Oráculo: legacy con cap imposible de alcanzar en este grafo.
        let mut legacy_cfg = defer_cfg(depth);
        legacy_cfg.policy = CapPolicy::BoundedLegacy;
        legacy_cfg.max_routes_per_tick = 100_000;
        let oracle: HashSet<String> = find_routes(&g, 1, &legacy_cfg)
            .routes
            .iter()
            .map(|r| r.route_hash.clone())
            .collect();
        assert!(!oracle.is_empty(), "oracle must find routes");

        // Sujeto: incremental con budget 2 (forzando MANY defers).
        let mut f = UniqueRouteFinder::new(defer_cfg(depth));
        let mut union: HashSet<String> = HashSet::new();
        let mut ticks = 0usize;
        loop {
            let out = f.next_tick(&g, 1, 42, 2);
            assert!(out.routes.len() <= 2, "budget respected");
            for r in &out.routes {
                assert!(
                    union.insert(r.route_hash.clone()),
                    "no route emitted twice in a ladder"
                );
            }
            ticks += 1;
            if out.ladder_complete {
                break;
            }
            assert!(
                out.deferred,
                "mid-ladder ticks must be deferred (budget < total)"
            );
            assert!(ticks < 10_000, "ladder must terminate");
        }
        assert_eq!(
            union, oracle,
            "H1: incremental union == uncapped legacy set"
        );
        assert!(
            ticks > 1,
            "budget 2 must have forced multiple ticks (defers happened)"
        );
    }

    #[test]
    fn h1_cursor_advances_new_routes_each_tick() {
        let g = triangle_graph();
        let mut f = UniqueRouteFinder::new(defer_cfg(3));
        let t1 = f.next_tick(&g, 1, 42, 2);
        assert!(t1.deferred);
        let t2 = f.next_tick(&g, 1, 42, 2);
        assert!(t2.deferred || t2.ladder_complete);
        // The second tick must NOT re-emit the first tick's routes (cursor advanced).
        let s1: HashSet<&str> = t1.routes.iter().map(|r| r.route_hash.as_str()).collect();
        for r in &t2.routes {
            assert!(
                !s1.contains(r.route_hash.as_str()),
                "cursor must advance, not repeat"
            );
        }
    }

    #[test]
    fn h1_graph_change_restarts_ladder_without_duplicates() {
        let g = triangle_graph();
        let mut f = UniqueRouteFinder::new(defer_cfg(3));
        let _ = f.next_tick(&g, 1, 42, 2);
        // Same graph content, different generation → restart (defensive path).
        let mut union: HashSet<String> = HashSet::new();
        let out = f.next_tick(&g, 1, 99, 2);
        assert!(!out.ladder_complete || out.deferred);
        for r in &out.routes {
            union.insert(r.route_hash.clone()); // restart-call routes count too
        }
        // Completing the restarted ladder is still exhaustive (H1 oracle).
        let mut legacy_cfg = defer_cfg(3);
        legacy_cfg.policy = CapPolicy::BoundedLegacy;
        legacy_cfg.max_routes_per_tick = 100_000;
        let oracle: HashSet<String> = find_routes(&g, 1, &legacy_cfg)
            .routes
            .into_iter()
            .map(|r| r.route_hash)
            .collect();
        loop {
            let o = f.next_tick(&g, 1, 99, 2);
            for r in &o.routes {
                union.insert(r.route_hash.clone());
            }
            if o.ladder_complete {
                break;
            }
        }
        assert_eq!(union, oracle);
    }

    #[test]
    fn h1_parallel_pools_rotate_across_ladders() {
        // 4 parallel V2 pools over one pair — cap 2 retains a rotated window.
        use ProtocolType::V2;
        let g = graph_from(&[
            (0x11, 1, 2, V2),
            (0x22, 1, 2, V2),
            (0x33, 1, 2, V2),
            (0x44, 1, 2, V2),
        ]);
        // cap 2 vs 4 parallel pools → rotated window per ladder.
        let mut cfg = defer_cfg(2);
        cfg.max_pools_per_pair = 2;
        let mut f = UniqueRouteFinder::new(cfg);
        let mut seen_pools: HashSet<Address> = HashSet::new();
        for _ladder in 0..3 {
            loop {
                let o = f.next_tick(&g, 1, 42, 100);
                for r in &o.routes {
                    for p in &r.pools {
                        seen_pools.insert(*p);
                    }
                }
                if o.ladder_complete {
                    assert!(
                        o.pools_rotated,
                        "4 parallel pools vs cap 2 must flag rotation"
                    );
                    break;
                }
            }
        }
        // Across rotated ladders EVERY parallel pool participates in some route.
        assert_eq!(
            seen_pools.len(),
            4,
            "rotation must eventually cover all parallel pools"
        );
    }

    #[test]
    fn depth_floor_iterative_deepening_reaches_max_depth() {
        // Observable deepening: with budget 2, the FIRST tick must still be in
        // the depth-2 pass emitting ONLY 2-hop cycles; deeper cycles arrive in
        // later ticks once the ladder deepens (a pure-3-cycle graph would
        // finish depth 2 inside the first call and only then surface depth 3).
        let g = triangle_graph(); // has 2-cycles (AB, DE) AND 3-cycles
        let mut f = UniqueRouteFinder::new(defer_cfg(3));
        let first = f.next_tick(&g, 1, 42, 2);
        assert!(first.deferred, "tiny budget defers mid-depth-2 pass");
        assert_eq!(first.depth_pass, 2, "ladder starts at depth 2");
        for r in &first.routes {
            assert_eq!(r.hops, 2, "depth-2 pass emits only 2-hop cycles");
        }
        let mut saw_deeper = false;
        loop {
            let o = f.next_tick(&g, 1, 42, 2);
            if o.routes.iter().any(|r| r.hops == 3) {
                saw_deeper = true;
            }
            if o.ladder_complete {
                break;
            }
        }
        assert!(
            saw_deeper,
            "3-hop cycles found once the ladder deepens past 2"
        );
    }
}
