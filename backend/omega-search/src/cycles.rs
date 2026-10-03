//! Cycle representation, canonicalization, bounded enumeration and the
//! exhaustive oracle.
//!
//! PROMPT §6: *"Combina enumeración acotada, rutas candidatas diversas,
//! deduplicación, cotización y refinamiento. Justifica podas con cotas válidas
//! para el dominio. Toda poda heurística debe ser identificable y medirse contra
//! búsqueda exhaustiva en grafos de prueba pequeños."*
//!
//! Two entry points implement exactly that pairing:
//!
//! * [`enumerate_bounded`] — bounded DFS with an explicit cycle cap and work
//!   budget. Every prune is counted under a *named* rule in
//!   [`PruneAccounting::rejected_by`], so no pruning is invisible.
//! * [`enumerate_exhaustive`] — the same walker with **no cap and no heuristic
//!   prune**, i.e. the oracle. Because both share one code path, a difference
//!   between them is attributable to the caps alone, which is what makes the
//!   comparison meaningful.
//!
//! ## Why a bounded DFS and not Bellman–Ford / Dijkstra / Johnson
//!
//! PROMPT §6 forbids applying them where their preconditions fail. Bellman–Ford
//! on marginal negative-log rate weights finds cycles in *that* model; it cannot
//! resolve fixed fees, price impact, sizes or settlement, and Dijkstra/Johnson
//! reweighting require non-negative (or potential-adjustable) weights that a
//! directed multigraph with fixed per-cycle costs does not provide. Enumeration
//! combined with exact finite-size evaluation is the honest pair, and the
//! marginal-weight model survives only as a *candidate generator* — see
//! [`crate::engine::marginal_product_verdict`] for its exact integer form.
//!
//! ## Canonicalization
//!
//! A cycle is stored as its edge sequence, rotated so the
//! lexicographically-smallest edge key `(token_in, pool_id, zero_for_one)` comes
//! first. A **reversed** traversal is *not* a rotation: it is a different
//! economic route (different swap directions, different fees paid) and is kept
//! as a distinct cycle — matching `route_discovery::canonicalizer` in
//! `searcher-rs`, which preserves the reverse direction as a distinct hash.

use crate::graph::{EdgeId, GraphPolicy, MultiGraph, PoolIndex, TokenId};
use std::collections::{BTreeMap, BTreeSet};

/// A closed cycle in canonical (rotation-normalized) form.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Cycle {
    /// Edge ids in traversal order; the cycle closes from the last edge's
    /// `token_out` back to the first edge's `token_in`.
    pub edges: Vec<EdgeId>,
}

impl Cycle {
    pub fn hops(&self) -> usize {
        self.edges.len()
    }

    /// Stable dedup key (the canonical edge sequence itself).
    pub fn key(&self) -> &[EdgeId] {
        &self.edges
    }

    /// Pool ids traversed, in order (with repeats if the policy allowed them).
    pub fn pool_ids(&self, g: &MultiGraph) -> Vec<u64> {
        self.edges.iter().map(|e| g.edge(*e).pool_id).collect()
    }

    /// Distinct pool indices, sorted — the footprint used for diversity.
    pub fn pool_footprint(&self, g: &MultiGraph) -> BTreeSet<PoolIndex> {
        self.edges.iter().map(|e| g.edge(*e).pool).collect()
    }

    pub fn touches_pool(&self, g: &MultiGraph, pool_id: u64) -> bool {
        self.edges.iter().any(|e| g.edge(*e).pool_id == pool_id)
    }

    pub fn start_token(&self, g: &MultiGraph) -> Option<TokenId> {
        self.edges.first().map(|e| g.edge(*e).token_in)
    }

    pub fn token_path(&self, g: &MultiGraph) -> Vec<TokenId> {
        self.edges.iter().map(|e| g.edge(*e).token_in).collect()
    }
}

/// Edge key used for rotation canonicalization and deterministic ordering.
fn edge_key(g: &MultiGraph, id: EdgeId) -> (TokenId, u64, bool) {
    let e = g.edge(id);
    (e.token_in, e.pool_id, e.zero_for_one)
}

/// Canonicalize an edge sequence into a [`Cycle`], or `None` when the sequence
/// is not a *simple closed* cycle.
///
/// Rejects: empty/1-hop input, broken connectivity, non-simple node visits.
/// Accepts: any rotation of a valid cycle (normalized to the minimal rotation).
pub fn canonicalize(g: &MultiGraph, edges: &[EdgeId]) -> Option<Cycle> {
    if edges.len() < 2 {
        return None;
    }
    for (i, id) in edges.iter().enumerate() {
        let here = g.edge(*id);
        let next = g.edge(edges[(i + 1) % edges.len()]);
        if here.token_out != next.token_in {
            return None;
        }
    }
    let mut nodes: BTreeSet<TokenId> = BTreeSet::new();
    for id in edges {
        if !nodes.insert(g.edge(*id).token_in) {
            return None; // not a simple cycle
        }
    }

    let n = edges.len();
    let mut best: Option<Vec<EdgeId>> = None;
    for rot in 0..n {
        let mut candidate: Vec<EdgeId> = Vec::with_capacity(n);
        for k in 0..n {
            candidate.push(edges[(rot + k) % n]);
        }
        let take = match &best {
            None => true,
            Some(current) => {
                let lhs: Vec<_> = candidate.iter().map(|e| edge_key(g, *e)).collect();
                let rhs: Vec<_> = current.iter().map(|e| edge_key(g, *e)).collect();
                lhs < rhs
            }
        };
        if take {
            best = Some(candidate);
        }
    }
    best.map(|e| Cycle { edges: e })
}

/// Why enumeration stopped early. `None` in [`PruneAccounting::truncated`] means
/// the walk covered the whole authorized space.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum TruncationReason {
    /// The cycle cap was reached.
    CycleCap,
    /// The work budget (edge visits) was exhausted.
    WorkBudget,
}

/// Enumeration budget. [`EnumerateLimits::unbounded`] is the oracle setting.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EnumerateLimits {
    pub max_cycles: usize,
    pub max_work_units: u64,
}

impl EnumerateLimits {
    pub const fn new(max_cycles: usize, max_work_units: u64) -> Self {
        EnumerateLimits {
            max_cycles,
            max_work_units,
        }
    }

    pub const fn unbounded() -> Self {
        EnumerateLimits {
            max_cycles: usize::MAX,
            max_work_units: u64::MAX,
        }
    }
}

impl Default for EnumerateLimits {
    fn default() -> Self {
        EnumerateLimits::new(5_000, 200_000)
    }
}

/// Identifiable audit trail of every prune applied during a walk.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PruneAccounting {
    /// Cycles emitted (canonical, deduplicated).
    pub cycles_emitted: usize,
    /// Edge visits spent.
    pub work_units_used: u64,
    /// Per-rule prune counts, keyed by a stable rule name.
    pub rejected_by: BTreeMap<&'static str, u64>,
    /// `None` when the walk was exhaustive over the authorized space.
    pub truncated: Option<TruncationReason>,
}

impl PruneAccounting {
    pub fn count_of(&self, rule: &str) -> u64 {
        self.rejected_by.get(rule).copied().unwrap_or(0)
    }

    pub fn total_pruned(&self) -> u64 {
        self.rejected_by.values().sum()
    }
}

/// Stable rule names, so tests and telemetry never match on free text.
pub mod prune_rules {
    pub const HOP_LENGTH_NOT_AUTHORIZED: &str = "hop_length_not_authorized";
    pub const SAME_POOL_REUSE: &str = "same_pool_reuse";
    pub const DUPLICATE_ROTATION: &str = "duplicate_rotation";
    pub const NON_SIMPLE_OR_BROKEN: &str = "non_simple_or_broken";
    pub const ALREADY_VISITED: &str = "already_visited";
    pub const START_NOT_MINIMUM: &str = "start_not_minimum";
    pub const INTERIOR_MASK_EXCLUDED: &str = "interior_mask_excluded";
    pub const MAX_HOPS_EXCEEDED: &str = "max_hops_exceeded";
    pub const CYCLE_CAP: &str = "cycle_cap";
    pub const WORK_BUDGET: &str = "work_budget";
}

struct Walker<'a> {
    g: &'a MultiGraph,
    policy: &'a GraphPolicy,
    limits: EnumerateLimits,
    cycles: Vec<Cycle>,
    seen: BTreeSet<Vec<EdgeId>>,
    work: u64,
    prunes: BTreeMap<&'static str, u64>,
    truncated: Option<TruncationReason>,
    stopped: bool,
    max_hops: usize,
}

impl<'a> Walker<'a> {
    fn new(g: &'a MultiGraph, policy: &'a GraphPolicy, limits: EnumerateLimits) -> Self {
        let max_hops = policy.hop_lengths.max_allowed().unwrap_or(0) as usize;
        Walker {
            g,
            policy,
            limits,
            cycles: Vec::new(),
            seen: BTreeSet::new(),
            work: 0,
            prunes: BTreeMap::new(),
            truncated: None,
            stopped: false,
            max_hops,
        }
    }

    fn prune(&mut self, rule: &'static str) {
        *self.prunes.entry(rule).or_insert(0) += 1;
    }

    fn run(&mut self) {
        if self.max_hops == 0 {
            return; // empty hop mask: nothing is authorized, honest empty result
        }
        let starts: Vec<TokenId> = match &self.policy.start_tokens {
            Some(s) => s.iter().copied().collect(),
            None => self.g.tokens().to_vec(),
        };
        for start in starts {
            if self.stopped {
                break;
            }
            let mut path: Vec<EdgeId> = Vec::new();
            let mut visited: BTreeSet<TokenId> = BTreeSet::new();
            visited.insert(start);
            self.walk(start, start, &mut path, &mut visited);
        }
    }

    fn walk(
        &mut self,
        start: TokenId,
        current: TokenId,
        path: &mut Vec<EdgeId>,
        visited: &mut BTreeSet<TokenId>,
    ) {
        let g = self.g; // `&'a MultiGraph` is Copy: detaches from `self` borrow
        let out: Vec<EdgeId> = g.out_edges(current).to_vec();
        for eid in out {
            if self.stopped {
                return;
            }
            if self.work >= self.limits.max_work_units {
                self.prune(prune_rules::WORK_BUDGET);
                self.truncated = Some(TruncationReason::WorkBudget);
                self.stopped = true;
                return;
            }
            self.work += 1;
            let e = *g.edge(eid);

            if e.token_out == start {
                let hops = path.len() + 1;
                if !self.policy.hop_lengths.allows(hops as u8) {
                    self.prune(prune_rules::HOP_LENGTH_NOT_AUTHORIZED);
                    continue;
                }
                if !self.policy.allow_same_pool_twice {
                    let mut pools: Vec<u64> = path.iter().map(|p| g.edge(*p).pool_id).collect();
                    pools.push(e.pool_id);
                    let unique: BTreeSet<u64> = pools.iter().copied().collect();
                    if unique.len() != pools.len() {
                        self.prune(prune_rules::SAME_POOL_REUSE);
                        continue;
                    }
                }
                let mut raw = path.clone();
                raw.push(eid);
                match canonicalize(g, &raw) {
                    None => self.prune(prune_rules::NON_SIMPLE_OR_BROKEN),
                    Some(canon) => {
                        let key = canon.edges.clone();
                        if self.seen.contains(&key) {
                            self.prune(prune_rules::DUPLICATE_ROTATION);
                        } else if self.cycles.len() >= self.limits.max_cycles {
                            self.prune(prune_rules::CYCLE_CAP);
                            self.truncated = Some(TruncationReason::CycleCap);
                            self.stopped = true;
                            return;
                        } else {
                            self.seen.insert(key);
                            self.cycles.push(canon);
                        }
                    }
                }
                continue;
            }

            // Interior step. Requiring `token_out > start` makes `start` the
            // minimum node of the cycle, which visits every cycle exactly once
            // instead of once per rotation.
            if e.token_out <= start {
                self.prune(prune_rules::START_NOT_MINIMUM);
                continue;
            }
            if visited.contains(&e.token_out) {
                self.prune(prune_rules::ALREADY_VISITED);
                continue;
            }
            if let Some(interior) = &self.policy.interior_allowlist {
                if !interior.contains(&e.token_out) {
                    self.prune(prune_rules::INTERIOR_MASK_EXCLUDED);
                    continue;
                }
            }
            if path.len() + 2 > self.max_hops {
                self.prune(prune_rules::MAX_HOPS_EXCEEDED);
                continue;
            }

            path.push(eid);
            visited.insert(e.token_out);
            self.walk(start, e.token_out, path, visited);
            visited.remove(&e.token_out);
            path.pop();
        }
    }

    fn finish(self) -> (Vec<Cycle>, PruneAccounting) {
        let accounting = PruneAccounting {
            cycles_emitted: self.cycles.len(),
            work_units_used: self.work,
            rejected_by: self.prunes,
            truncated: self.truncated,
        };
        (self.cycles, accounting)
    }
}

/// Bounded enumeration under `policy`. Every prune is counted; truncation is
/// reported rather than hidden.
pub fn enumerate_bounded(
    g: &MultiGraph,
    policy: &GraphPolicy,
    limits: EnumerateLimits,
) -> (Vec<Cycle>, PruneAccounting) {
    let mut walker = Walker::new(g, policy, limits);
    walker.run();
    walker.finish()
}

/// Exhaustive enumeration over the authorized space — the oracle for §14.
///
/// Same walker and same policy as [`enumerate_bounded`], but with no cycle cap
/// and no work budget, so a difference between the two is attributable to the
/// budget alone. Exponential in the token count by nature: intended for small
/// test graphs and verification runs, never for the live pool graph.
pub fn enumerate_exhaustive(g: &MultiGraph, policy: &GraphPolicy) -> Vec<Cycle> {
    let mut walker = Walker::new(g, policy, EnumerateLimits::unbounded());
    walker.run();
    walker.finish().0
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use crate::graph::{HopLengthMask, PoolSpec, ProtocolVersion};
    use crate::units::{Bps, Raw};

    fn p(id: u64, t0: TokenId, t1: TokenId) -> PoolSpec {
        PoolSpec {
            pool_id: id,
            version: ProtocolVersion::V2,
            token0: t0,
            token1: t1,
            fee_bps: Bps::new(30).unwrap(),
            reserve0: Raw::new(1_000_000),
            reserve1: Raw::new(1_000_000),
            available: true,
        }
    }

    fn graph(pools: Vec<PoolSpec>) -> MultiGraph {
        MultiGraph::build(pools, &GraphPolicy::permissive_2_to_7())
    }

    #[test]
    fn triangle_is_found_once_and_canonical_rotation_is_stable() {
        let g = graph(vec![p(10, 1, 2), p(11, 2, 3), p(12, 3, 1)]);
        let policy = GraphPolicy::permissive_2_to_7();
        let (cycles, acc) = enumerate_bounded(&g, &policy, EnumerateLimits::unbounded());
        // Forward and reverse of the same triangle are distinct routes.
        assert_eq!(cycles.len(), 2, "one triangle, two orientations");
        assert!(cycles.iter().all(|c| c.hops() == 3));
        assert_eq!(acc.truncated, None);
        // Canonical form starts at the minimal edge key, whatever the DFS order.
        for c in &cycles {
            let keys: Vec<_> = c.edges.iter().map(|e| edge_key(&g, *e)).collect();
            let mut sorted = keys.clone();
            sorted.sort();
            assert_eq!(keys[0], sorted[0], "rotation is minimal");
        }
    }

    #[test]
    fn bounded_with_generous_limits_equals_exhaustive() {
        let g = graph(vec![
            p(10, 1, 2),
            p(11, 2, 3),
            p(12, 3, 1),
            p(13, 1, 3),
            p(14, 3, 4),
            p(15, 4, 1),
        ]);
        let policy = GraphPolicy::permissive_2_to_7();
        let oracle: BTreeSet<Vec<EdgeId>> = enumerate_exhaustive(&g, &policy)
            .into_iter()
            .map(|c| c.edges)
            .collect();
        let (bounded, acc) = enumerate_bounded(&g, &policy, EnumerateLimits::unbounded());
        let got: BTreeSet<Vec<EdgeId>> = bounded.into_iter().map(|c| c.edges).collect();
        assert_eq!(got, oracle);
        assert_eq!(acc.truncated, None);
    }

    #[test]
    fn cycle_cap_truncates_and_is_reported() {
        let g = graph(vec![
            p(10, 1, 2),
            p(11, 2, 3),
            p(12, 3, 1),
            p(13, 1, 3),
            p(14, 3, 4),
            p(15, 4, 1),
        ]);
        let policy = GraphPolicy::permissive_2_to_7();
        let (cycles, acc) = enumerate_bounded(&g, &policy, EnumerateLimits::new(1, u64::MAX));
        assert_eq!(cycles.len(), 1);
        assert_eq!(acc.truncated, Some(TruncationReason::CycleCap));
        assert_eq!(acc.count_of(prune_rules::CYCLE_CAP), 1);
    }

    #[test]
    fn hop_mask_narrows_the_authorized_space() {
        let g = graph(vec![p(10, 1, 2), p(11, 2, 3), p(12, 3, 1)]);

        // A domain authorized for 2 hops only never even walks toward a 3-cycle:
        // the depth guard fires first and is counted under its own rule. The
        // walker does not silently "find and discard" what it was told to skip.
        let only_two = GraphPolicy {
            hop_lengths: HopLengthMask::only(2),
            ..GraphPolicy::default()
        };
        let (cycles, acc) = enumerate_bounded(&g, &only_two, EnumerateLimits::unbounded());
        assert!(cycles.is_empty(), "a 3-cycle is out of a 2-hop-only domain");
        assert!(acc.count_of(prune_rules::MAX_HOPS_EXCEEDED) > 0);

        // A mask with a hole refuses a closing edge whose length it does not
        // authorize, even though the walk was deep enough to reach it.
        let only_three = GraphPolicy {
            hop_lengths: HopLengthMask::only(3),
            ..GraphPolicy::default()
        };
        let (cycles, acc) = enumerate_bounded(&g, &only_three, EnumerateLimits::unbounded());
        assert_eq!(cycles.len(), 2, "both orientations of the triangle");
        assert!(acc.count_of(prune_rules::HOP_LENGTH_NOT_AUTHORIZED) > 0);
    }

    #[test]
    fn same_pool_reuse_is_excluded_by_default_and_counted() {
        // A single pool over (1,2): 1→2→1 reuses pool 10.
        let g = graph(vec![p(10, 1, 2)]);
        let policy = GraphPolicy::permissive_2_to_7();
        let (cycles, acc) = enumerate_bounded(&g, &policy, EnumerateLimits::unbounded());
        assert!(cycles.is_empty());
        assert_eq!(acc.count_of(prune_rules::SAME_POOL_REUSE), 1);

        let allowed = GraphPolicy {
            allow_same_pool_twice: true,
            ..GraphPolicy::permissive_2_to_7()
        };
        let (cycles, _) = enumerate_bounded(&g, &allowed, EnumerateLimits::unbounded());
        assert_eq!(cycles.len(), 1, "the wash cycle exists when policy allows it");
    }

    #[test]
    fn interior_mask_excludes_configured_tokens() {
        let g = graph(vec![p(10, 1, 2), p(11, 2, 3), p(12, 3, 1)]);
        let blocked = GraphPolicy {
            interior_allowlist: Some([1u64, 3u64].into_iter().collect()),
            ..GraphPolicy::permissive_2_to_7()
        };
        let (cycles, acc) = enumerate_bounded(&g, &blocked, EnumerateLimits::unbounded());
        assert!(cycles.is_empty(), "token 2 may not be an interior hop");
        assert!(acc.count_of(prune_rules::INTERIOR_MASK_EXCLUDED) > 0);
    }

    #[test]
    fn work_budget_bounds_a_dense_graph_without_panicking() {
        let mut pools = Vec::new();
        for a in 1..=5u64 {
            for b in (a + 1)..=5u64 {
                pools.push(p(100 + a * 10 + b, a, b));
            }
        }
        let g = graph(pools);
        let policy = GraphPolicy::permissive_2_to_7();
        let (cycles, acc) = enumerate_bounded(&g, &policy, EnumerateLimits::new(usize::MAX, 25));
        assert!(acc.work_units_used <= 26);
        assert_eq!(acc.truncated, Some(TruncationReason::WorkBudget));
        assert!(!cycles.is_empty(), "some cycles are still published honestly");
    }

    #[test]
    fn canonicalize_rejects_broken_and_non_simple_sequences() {
        let g = graph(vec![p(10, 1, 2), p(11, 2, 3), p(12, 3, 1)]);
        assert_eq!(canonicalize(&g, &[]), None);
        assert_eq!(canonicalize(&g, &[0]), None);
        // 1→2 then 1→2 again: does not connect.
        assert_eq!(canonicalize(&g, &[0, 0]), None);
    }
}
