//! Directed multigraph: nodes are tokens, edges are `(pool, direction, version)`.
//!
//! PROMPT §6: *"Construye un multigrafo dirigido con aristas específicas por
//! pool, dirección y versión. Respeta allowlists, liquidez, disponibilidad y
//! máscaras de hops."*
//!
//! Every policy rejection is recorded with a **machine-readable reason**. The
//! reasons are deliberately distinct for distinct facts:
//!
//! | reason                  | fact                                                     |
//! |-------------------------|----------------------------------------------------------|
//! | `zero_reserve`          | a reserve was *computed* and is exactly zero              |
//! | `reserve_below_floor`   | a reserve was computed and is below the configured floor  |
//! | `venue_unavailable`     | the venue reports the pool paused/disabled                |
//! | `version_not_authorized`| the protocol version is outside the authorized set        |
//! | `token_not_allowlisted` | a leg references a token outside the allowlist            |
//! | `degenerate_pool`       | `token0 == token1` — not a tradeable pair                 |
//!
//! A pool rejected as `zero_reserve` is *not* the same state as a pool whose
//! reserves are missing: this crate only accepts `PoolSpec`s that already carry
//! concrete reserves, so "missing" never reaches the graph and cannot be
//! silently rendered as zero.

use crate::units::{Bps, Raw};
use std::collections::{BTreeMap, BTreeSet};

/// Token identity inside the engine. The adapter maps `Address → TokenId`.
pub type TokenId = u64;
/// Index into [`MultiGraph::pools`].
pub type PoolIndex = usize;
/// Index into [`MultiGraph::edges`].
pub type EdgeId = usize;

/// Protocol family **and** version of a pool. Version is part of edge identity:
/// two pools over the same token pair on different versions are two edges.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ProtocolVersion {
    /// Constant-product (`x·y=k`) deployment, V2 and forks.
    V2,
    /// Concentrated liquidity with an explicit fee tier.
    V3,
    /// StableSwap-like invariant (Newton iteration on `D`).
    CurveLike,
    /// Weighted/stable multi-asset vault.
    BalancerLike,
    /// Version not resolved from source. Carried honestly, not guessed.
    Unknown,
}

impl ProtocolVersion {
    pub const fn as_str(self) -> &'static str {
        match self {
            ProtocolVersion::V2 => "v2",
            ProtocolVersion::V3 => "v3",
            ProtocolVersion::CurveLike => "curve_like",
            ProtocolVersion::BalancerLike => "balancer_like",
            ProtocolVersion::Unknown => "unknown",
        }
    }

    /// Versions whose swap curve this crate can evaluate **exactly**.
    ///
    /// Only V2-style constant product has a closed form implemented here. The
    /// others are representable in the graph (so enumeration and policy are
    /// honest about their existence) but evaluating them must go through the
    /// protocol's own quoter — see `eval::EvalError::CurveNotImplemented`.
    pub const fn is_exactly_quotable_here(self) -> bool {
        matches!(self, ProtocolVersion::V2)
    }
}

/// A pool as the search engine needs it: identity, version, fee and reserves.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PoolSpec {
    /// Venue pool identifier (address hashed to `u64` by the adapter, or the
    /// raw address where it fits).
    pub pool_id: u64,
    pub version: ProtocolVersion,
    pub token0: TokenId,
    pub token1: TokenId,
    pub fee_bps: Bps,
    pub reserve0: Raw,
    pub reserve1: Raw,
    /// Venue-reported availability. `false` = paused/disabled by the venue.
    pub available: bool,
}

impl PoolSpec {
    pub fn reserve_of(&self, token: TokenId) -> Option<Raw> {
        if token == self.token0 {
            Some(self.reserve0)
        } else if token == self.token1 {
            Some(self.reserve1)
        } else {
            None
        }
    }

    pub fn other(&self, token: TokenId) -> Option<TokenId> {
        if token == self.token0 {
            Some(self.token1)
        } else if token == self.token1 {
            Some(self.token0)
        } else {
            None
        }
    }
}

/// One directed swap edge: a pool traversed in one orientation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Edge {
    pub id: EdgeId,
    pub pool: PoolIndex,
    pub pool_id: u64,
    pub version: ProtocolVersion,
    pub token_in: TokenId,
    pub token_out: TokenId,
    pub fee_bps: Bps,
    /// `true` = consumes `token0`, emits `token1`.
    pub zero_for_one: bool,
    /// Reserve on the input side at build time (policy input for the floor).
    pub reserve_in: Raw,
}

/// Which cycle lengths are in scope.
///
/// PROMPT §6 authorizes the **2–7 hop** range *where the vigente specification
/// authorizes it* and explicitly forbids turning it into a universal rule.
/// Therefore the range is a *named policy constructor*
/// ([`HopLengthMask::authorized_2_to_7`]) and never a hardcoded constant of the
/// walker: a domain authorized for 2 hops only must be given `only(2)` and the
/// engine must obey.
///
/// `Default` is the **empty** mask: nothing is authorized until a caller
/// authorizes an explicit range. Fail-closed by construction.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct HopLengthMask(u16);

impl HopLengthMask {
    /// Lowest hop count the vigente specification authorizes for closed cycles.
    pub const MIN_AUTHORIZED: u8 = 2;
    /// Highest hop count the vigente specification authorizes for closed cycles.
    pub const MAX_AUTHORIZED: u8 = 7;
    /// Widest representable hop count in this bitmask.
    pub const MAX_REPRESENTABLE: u8 = 15;

    /// The 2–7 range, as authorized by the vigente specification for closed
    /// cycles. This is a *policy choice by the caller*, not an engine default.
    pub const fn authorized_2_to_7() -> Self {
        let mut bits: u16 = 0;
        let mut h: u8 = Self::MIN_AUTHORIZED;
        while h <= Self::MAX_AUTHORIZED {
            bits |= 1u16 << h;
            h += 1;
        }
        HopLengthMask(bits)
    }

    pub const fn none() -> Self {
        HopLengthMask(0)
    }

    /// A single authorized length (e.g. a domain that only does 2-leg cross-arb).
    pub const fn only(hops: u8) -> Self {
        if hops > Self::MAX_REPRESENTABLE {
            return HopLengthMask(0);
        }
        HopLengthMask(1u16 << hops)
    }

    /// `[min, max]` inclusive, clamped to the representable range.
    pub fn range(min: u8, max: u8) -> Self {
        let mut mask = HopLengthMask::none();
        let mut h = min;
        while h <= max && h <= Self::MAX_REPRESENTABLE {
            mask.insert(h);
            h += 1;
        }
        mask
    }

    pub fn insert(&mut self, hops: u8) {
        if hops <= Self::MAX_REPRESENTABLE {
            self.0 |= 1u16 << hops;
        }
    }

    pub const fn allows(self, hops: u8) -> bool {
        hops <= Self::MAX_REPRESENTABLE && (self.0 & (1u16 << hops)) != 0
    }

    pub const fn is_empty(self) -> bool {
        self.0 == 0
    }

    /// Highest authorized length, or `None` when the mask is empty.
    pub const fn max_allowed(self) -> Option<u8> {
        let mut h = Self::MAX_REPRESENTABLE;
        loop {
            if (self.0 & (1u16 << h)) != 0 {
                return Some(h);
            }
            if h == 0 {
                return None;
            }
            h -= 1;
        }
    }

    pub fn lengths(self) -> Vec<u8> {
        (0..=Self::MAX_REPRESENTABLE)
            .filter(|h| self.allows(*h))
            .collect()
    }
}

/// Everything that decides which pools become edges.
#[derive(Debug, Clone, Default)]
pub struct GraphPolicy {
    /// When `Some`, both legs of a pool must be in the set.
    pub token_allowlist: Option<BTreeSet<TokenId>>,
    /// Liquidity floor on the *input* side of each edge.
    pub min_reserve_in: Option<Raw>,
    /// Authorized cycle lengths (see [`HopLengthMask`]).
    pub hop_lengths: HopLengthMask,
    /// Interior-node mask: tokens allowed at positions `1..n-1` of a cycle.
    pub interior_allowlist: Option<BTreeSet<TokenId>>,
    /// When `Some`, only these protocol versions build edges.
    pub allowed_versions: Option<BTreeSet<ProtocolVersion>>,
    /// When `Some`, enumeration only starts from these tokens.
    pub start_tokens: Option<BTreeSet<TokenId>>,
    /// Keep only the **two** deepest parallel edges per
    /// `(token_in, token_out, version, fee)` key.
    ///
    /// **Measured limitation (do not "fix" this to one survivor).** A 2-hop
    /// cross-DEX cycle needs *two distinct pools over the same pair*: pool `X`
    /// for `A→B` and pool `Y` for `B→A`. Reducing a parallel group to its single
    /// deepest edge therefore deletes every 2-cycle the group could form —
    /// measured in `tests/engine_contract.rs`, where a one-survivor version
    /// returned *no* candidate while exhaustive enumeration found the optimum.
    /// Keeping two survivors preserves 2-cycle formation while still collapsing
    /// a large parallel set.
    ///
    /// It remains a **heuristic**, not a proven-safe reduction: for cycles of 3+
    /// hops a dropped pool can still be the missing middle leg, which is why the
    /// rule is opt-in, counted under its own name, and measured against
    /// exhaustive enumeration rather than assumed correct.
    pub dominance_prune: bool,
    /// Allow the same pool to be traversed twice in one cycle.
    ///
    /// Default `false`: `A→B` and `B→A` through one pool is a wash that only
    /// pays fees, and the repo's own line graph excludes same-pool reuse.
    pub allow_same_pool_twice: bool,
}

impl GraphPolicy {
    /// Policy with every filter open and the 2–7 range authorized.
    ///
    /// Note this is *a policy*, not an engine default: callers that need a
    /// narrower domain must construct their own.
    pub fn permissive_2_to_7() -> Self {
        GraphPolicy {
            hop_lengths: HopLengthMask::authorized_2_to_7(),
            ..Default::default()
        }
    }
}

/// Why a pool did not contribute edges.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum PoolReject {
    DegeneratePool,
    ZeroReserve,
    ReserveBelowFloor,
    VenueUnavailable,
    VersionNotAuthorized,
    TokenNotAllowlisted,
}

impl PoolReject {
    pub const fn as_str(self) -> &'static str {
        match self {
            PoolReject::DegeneratePool => "degenerate_pool",
            PoolReject::ZeroReserve => "zero_reserve",
            PoolReject::ReserveBelowFloor => "reserve_below_floor",
            PoolReject::VenueUnavailable => "venue_unavailable",
            PoolReject::VersionNotAuthorized => "version_not_authorized",
            PoolReject::TokenNotAllowlisted => "token_not_allowlisted",
        }
    }
}

/// A pool-level rejection with the pool it belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RejectedPool {
    pub pool: PoolIndex,
    pub pool_id: u64,
    pub reason: PoolReject,
}

/// The directed multigraph.
#[derive(Debug, Clone)]
pub struct MultiGraph {
    pools: Vec<PoolSpec>,
    edges: Vec<Edge>,
    /// Token → outgoing edge ids, sorted deterministically.
    out: BTreeMap<TokenId, Vec<EdgeId>>,
    tokens: Vec<TokenId>,
    rejected: Vec<RejectedPool>,
    dominance_dropped: u64,
}

impl MultiGraph {
    /// Build the graph from pools under `policy`.
    ///
    /// Each accepted pool contributes **two** directed edges (`0→1`, `1→0`);
    /// each edge carries the pool's version, so the multigraph keeps parallel
    /// pools of the same pair on different versions as distinct edges.
    pub fn build(pools: Vec<PoolSpec>, policy: &GraphPolicy) -> Self {
        let mut edges: Vec<Edge> = Vec::new();
        let mut rejected: Vec<RejectedPool> = Vec::new();
        let mut kept_pool_indices: BTreeSet<PoolIndex> = BTreeSet::new();
        let mut rejected_pool_indices: BTreeSet<PoolIndex> = BTreeSet::new();

        let reserve_ok = |v: Raw| -> Option<PoolReject> {
            if v.is_zero() {
                return Some(PoolReject::ZeroReserve);
            }
            if let Some(floor) = policy.min_reserve_in {
                if v < floor {
                    return Some(PoolReject::ReserveBelowFloor);
                }
            }
            None
        };

        for (idx, pool) in pools.iter().enumerate() {
            let mut reason: Option<PoolReject> = None;

            if pool.token0 == pool.token1 {
                reason = Some(PoolReject::DegeneratePool);
            } else if !pool.available {
                reason = Some(PoolReject::VenueUnavailable);
            } else if let Some(allowed) = &policy.allowed_versions {
                if !allowed.contains(&pool.version) {
                    reason = Some(PoolReject::VersionNotAuthorized);
                }
            } else if let Some(allow) = &policy.token_allowlist {
                if !allow.contains(&pool.token0) || !allow.contains(&pool.token1) {
                    reason = Some(PoolReject::TokenNotAllowlisted);
                }
            }

            if reason.is_none() {
                // Both directions must clear the liquidity floor: an edge whose
                // input side has no liquidity cannot be quoted at all.
                if let Some(r) = reserve_ok(pool.reserve0) {
                    reason = Some(r);
                } else if let Some(r) = reserve_ok(pool.reserve1) {
                    reason = Some(r);
                }
            }

            match reason {
                Some(r) => {
                    rejected.push(RejectedPool {
                        pool: idx,
                        pool_id: pool.pool_id,
                        reason: r,
                    });
                    rejected_pool_indices.insert(idx);
                }
                None => {
                    kept_pool_indices.insert(idx);
                    edges.push(Edge {
                        id: edges.len(),
                        pool: idx,
                        pool_id: pool.pool_id,
                        version: pool.version,
                        token_in: pool.token0,
                        token_out: pool.token1,
                        fee_bps: pool.fee_bps,
                        zero_for_one: true,
                        reserve_in: pool.reserve0,
                    });
                    edges.push(Edge {
                        id: edges.len(),
                        pool: idx,
                        pool_id: pool.pool_id,
                        version: pool.version,
                        token_in: pool.token1,
                        token_out: pool.token0,
                        fee_bps: pool.fee_bps,
                        zero_for_one: false,
                        reserve_in: pool.reserve1,
                    });
                }
            }
        }

        // Identifiable heuristic prune. For a single hop, a pool with a larger
        // input reserve and an identical fee weakly dominates a shallower one
        // (output is nondecreasing in `r_out` and nonincreasing in `r_in`). Over
        // a whole cycle that argument does not transfer, so this is opt-in,
        // counted, and measured against exhaustive enumeration.
        //
        // `KEEP_TOP = 2` is load-bearing, not a tuning knob: a 2-hop cycle over
        // one pair needs two DIFFERENT pools over that pair, so keeping a single
        // survivor would delete the very structure that produces the
        // opportunity (measured — see the `dominance_prune` docs).
        const KEEP_TOP_PARALLEL: usize = 2;
        let mut dominance_dropped: u64 = 0;
        if policy.dominance_prune {
            let mut groups: BTreeMap<(TokenId, TokenId, ProtocolVersion, u32), Vec<Edge>> =
                BTreeMap::new();
            for e in edges.iter().copied() {
                groups
                    .entry((e.token_in, e.token_out, e.version, e.fee_bps.get()))
                    .or_default()
                    .push(e);
            }
            let mut survivors: Vec<Edge> = Vec::with_capacity(edges.len());
            for group in groups.values_mut() {
                // Deepest first; exact tie broken by lower pool id for determinism.
                group.sort_by(|a, b| {
                    b.reserve_in
                        .cmp(&a.reserve_in)
                        .then_with(|| a.pool_id.cmp(&b.pool_id))
                });
                let kept = group.len().min(KEEP_TOP_PARALLEL);
                dominance_dropped += (group.len() - kept) as u64;
                survivors.extend(group.iter().take(kept).copied());
            }
            survivors.sort_by_key(|e| e.id);
            edges = survivors;
        }

        // Re-index edges densely so `EdgeId` stays an index into `edges`, and
        // re-point each edge's `pool` to the pool table (unchanged) while
        // keeping `id` consistent.
        for (new_id, e) in edges.iter_mut().enumerate() {
            e.id = new_id;
        }

        let mut out: BTreeMap<TokenId, Vec<EdgeId>> = BTreeMap::new();
        for e in &edges {
            out.entry(e.token_in).or_default().push(e.id);
        }
        for list in out.values_mut() {
            list.sort_by_key(|id| {
                let e = &edges[*id];
                (e.token_out, e.pool_id, e.zero_for_one)
            });
        }

        let mut tokens: Vec<TokenId> = out.keys().copied().collect();
        tokens.sort_unstable();

        MultiGraph {
            pools,
            edges,
            out,
            tokens,
            rejected,
            dominance_dropped,
        }
    }

    pub fn pools(&self) -> &[PoolSpec] {
        &self.pools
    }

    pub fn pool(&self, idx: PoolIndex) -> &PoolSpec {
        &self.pools[idx]
    }

    pub fn edges(&self) -> &[Edge] {
        &self.edges
    }

    pub fn edge(&self, id: EdgeId) -> &Edge {
        &self.edges[id]
    }

    pub fn out_edges(&self, token: TokenId) -> &[EdgeId] {
        self.out.get(&token).map(|v| v.as_slice()).unwrap_or(&[])
    }

    pub fn tokens(&self) -> &[TokenId] {
        &self.tokens
    }

    /// Pools that produced no edge, each with its reason.
    pub fn rejected_pools(&self) -> &[RejectedPool] {
        &self.rejected
    }

    /// Count of parallel edges removed by the dominance prune.
    pub fn dominance_dropped(&self) -> u64 {
        self.dominance_dropped
    }

    /// Pools that actually hold at least one edge.
    pub fn kept_pool_indices(&self) -> BTreeSet<PoolIndex> {
        self.edges.iter().map(|e| e.pool).collect()
    }

    /// Pools present in the input but rejected by policy.
    pub fn rejected_pool_indices(&self) -> BTreeSet<PoolIndex> {
        self.rejected.iter().map(|r| r.pool).collect()
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    fn pool(id: u64, t0: TokenId, t1: TokenId, r0: i128, r1: i128) -> PoolSpec {
        PoolSpec {
            pool_id: id,
            version: ProtocolVersion::V2,
            token0: t0,
            token1: t1,
            fee_bps: Bps::new(30).unwrap(),
            reserve0: Raw::new(r0),
            reserve1: Raw::new(r1),
            available: true,
        }
    }

    #[test]
    fn each_pool_yields_two_directed_edges_with_its_version() {
        let g = MultiGraph::build(vec![pool(7, 1, 2, 1_000, 2_000)], &GraphPolicy::permissive_2_to_7());
        assert_eq!(g.edges().len(), 2);
        assert_eq!(g.tokens(), &[1, 2]);
        assert_eq!(g.out_edges(1).len(), 1);
        assert_eq!(g.out_edges(2).len(), 1);
        let fwd = g.edge(g.out_edges(1)[0]);
        assert_eq!((fwd.token_in, fwd.token_out, fwd.zero_for_one), (1, 2, true));
        assert_eq!(fwd.version, ProtocolVersion::V2);
        assert_eq!(fwd.reserve_in, Raw::new(1_000));
    }

    #[test]
    fn rejection_reasons_keep_zero_floor_absence_and_availability_distinct() {
        let mut zero_reserve = pool(1, 1, 2, 0, 500);
        let below_floor = pool(2, 3, 4, 10, 500);
        let mut unavailable = pool(3, 5, 6, 1_000, 1_000);
        unavailable.available = false;
        let degenerate = pool(4, 7, 7, 1_000, 1_000);
        zero_reserve.available = true;

        let policy = GraphPolicy {
            min_reserve_in: Some(Raw::new(100)),
            ..GraphPolicy::permissive_2_to_7()
        };
        let g = MultiGraph::build(
            vec![zero_reserve, below_floor, unavailable, degenerate],
            &policy,
        );

        assert!(g.edges().is_empty());
        let reasons: BTreeMap<u64, PoolReject> = g
            .rejected_pools()
            .iter()
            .map(|r| (r.pool_id, r.reason))
            .collect();
        assert_eq!(reasons[&1], PoolReject::ZeroReserve);
        assert_eq!(reasons[&2], PoolReject::ReserveBelowFloor);
        assert_eq!(reasons[&3], PoolReject::VenueUnavailable);
        assert_eq!(reasons[&4], PoolReject::DegeneratePool);
    }

    #[test]
    fn parallel_pools_and_versions_stay_distinct_edges() {
        let mut v3 = pool(2, 1, 2, 900, 900);
        v3.version = ProtocolVersion::V3;
        let g = MultiGraph::build(
            vec![pool(1, 1, 2, 1_000, 2_000), v3],
            &GraphPolicy::permissive_2_to_7(),
        );
        assert_eq!(g.edges().len(), 4, "two pools × two directions");
        let versions: BTreeSet<ProtocolVersion> = g.edges().iter().map(|e| e.version).collect();
        assert_eq!(versions.len(), 2);
    }

    #[test]
    fn dominance_prune_is_opt_in_counted_and_deterministic() {
        // Three parallel pools over the same pair with an identical fee. Two
        // survive (the deepest); one is dropped per direction.
        let shallow = pool(9, 1, 2, 100, 100);
        let mid = pool(4, 1, 2, 1_000, 1_000);
        let deep = pool(3, 1, 2, 5_000, 5_000);

        let open = MultiGraph::build(
            vec![shallow.clone(), mid.clone(), deep.clone()],
            &GraphPolicy::permissive_2_to_7(),
        );
        assert_eq!(open.edges().len(), 6);
        assert_eq!(open.dominance_dropped(), 0, "opt-in: off by default");

        let policy = GraphPolicy {
            dominance_prune: true,
            ..GraphPolicy::permissive_2_to_7()
        };
        let pruned = MultiGraph::build(vec![shallow, mid, deep], &policy);
        assert_eq!(pruned.edges().len(), 4, "two survivors × two directions");
        assert_eq!(pruned.dominance_dropped(), 2);
        assert!(pruned.edges().iter().all(|e| e.pool_id == 3 || e.pool_id == 4));

        // Determinism: rebuilding yields the same survivors in the same order.
        let again = MultiGraph::build(
            vec![
                pool(9, 1, 2, 100, 100),
                pool(4, 1, 2, 1_000, 1_000),
                pool(3, 1, 2, 5_000, 5_000),
            ],
            &policy,
        );
        let ids_a: Vec<usize> = pruned.edges().iter().map(|e| e.id).collect();
        let ids_b: Vec<usize> = again.edges().iter().map(|e| e.id).collect();
        let pools_a: Vec<u64> = pruned.edges().iter().map(|e| e.pool_id).collect();
        let pools_b: Vec<u64> = again.edges().iter().map(|e| e.pool_id).collect();
        assert_eq!(ids_a, ids_b);
        assert_eq!(pools_a, pools_b);
    }

    #[test]
    fn keeping_two_survivors_preserves_the_ability_to_form_a_two_cycle() {
        // A 2-hop cycle needs two DIFFERENT pools over the same pair. Keeping
        // exactly one survivor would make every 2-cycle unreachable; the prune
        // must keep enough structure for the opportunity to still exist.
        let policy = GraphPolicy {
            dominance_prune: true,
            ..GraphPolicy::permissive_2_to_7()
        };
        let g = MultiGraph::build(
            vec![
                pool(9, 1, 2, 100, 100),
                pool(4, 1, 2, 1_000, 1_000),
                pool(3, 1, 2, 5_000, 5_000),
            ],
            &policy,
        );
        let (cycles, _) =
            crate::cycles::enumerate_bounded(&g, &policy, crate::cycles::EnumerateLimits::unbounded());
        assert_eq!(cycles.len(), 2, "both orientations of the surviving pair");
    }

    #[test]
    fn hop_length_mask_is_policy_not_a_universal_constant() {
        let m = HopLengthMask::authorized_2_to_7();
        assert_eq!(m.lengths(), vec![2, 3, 4, 5, 6, 7]);
        assert_eq!(m.max_allowed(), Some(7));
        assert!(!m.allows(1));
        assert!(!m.allows(8));
        let narrow = HopLengthMask::only(2);
        assert!(narrow.allows(2) && !narrow.allows(3));
        assert!(HopLengthMask::none().is_empty());
        assert_eq!(HopLengthMask::none().max_allowed(), None);
    }
}
