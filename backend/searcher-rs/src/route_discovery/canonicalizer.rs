//! Route canonicalization + deterministic `route_hash`.
//!
//! ## What the hash collapses vs. preserves
//!
//! - **Same-direction rotation collapses** to one hash: `A→B→C→A` and
//!   `B→C→A→B` are the same cycle observed from a different start token, so they
//!   must dedup to a single route. We achieve this by rotating the cycle to its
//!   lexicographically-smallest start token before hashing.
//! - **Reverse direction is PRESERVED** as a distinct route: clockwise
//!   `A→B→C→A` and counter-clockwise `A→C→B→A` are economically different
//!   (buy-here/sell-there vs the inverse), so the `direction_seq` makes their
//!   canonical inputs — and therefore their hashes — differ. We never reverse a
//!   cycle during canonicalization.
//!
//! ## Determinism
//!
//! The hash input is built ONLY from integer/string topology fields
//! (chain_id, route_kind, token/pool addresses, protocol tags, fee tiers,
//! directions). No floats, no timestamps, no liquidity — so the same topology
//! always yields the same hash, on any host, across restarts.

use crate::route_discovery::types::{RouteDirection, RouteKind};
use crate::route_intent::ProtocolType;
use ethers::types::Address;
use ethers::utils::keccak256;

/// Stable lowercase tag for a protocol family used in the canonical input.
pub fn protocol_token(p: ProtocolType) -> &'static str {
    match p {
        ProtocolType::V2 => "v2",
        ProtocolType::V3 => "v3",
        ProtocolType::Curve => "curve",
        ProtocolType::Balancer => "balancer",
        ProtocolType::Unknown => "unknown",
    }
}

/// The canonicalized cycle: the rotation chosen as canonical, plus its hash.
///
/// All vectors are length `hops`, rotated together so `tokens[0]` is the
/// smallest start token. `route_hash` is `0x`-prefixed keccak256 of the
/// canonical input string.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Canonical {
    pub route_hash: String,
    pub tokens: Vec<Address>,
    pub pools: Vec<Address>,
    pub protocols: Vec<ProtocolType>,
    pub fee_tiers: Vec<Option<u32>>,
    pub directions: Vec<RouteDirection>,
}

/// Rotate a slice so element `k` becomes index 0 (cyclic left-rotation by `k`).
fn rotate<T: Clone>(v: &[T], k: usize) -> Vec<T> {
    let n = v.len();
    (0..n).map(|i| v[(i + k) % n].clone()).collect()
}

/// Render the `|`-joined canonical input string for one (already-rotated) cycle.
fn render(
    chain_id: u64,
    route_kind: RouteKind,
    tokens: &[Address],
    pools: &[Address],
    protocols: &[ProtocolType],
    fee_tiers: &[Option<u32>],
    directions: &[RouteDirection],
) -> String {
    let token_seq = tokens
        .iter()
        .map(|a| format!("{a:#x}"))
        .collect::<Vec<_>>()
        .join(",");
    let pool_seq = pools
        .iter()
        .map(|a| format!("{a:#x}"))
        .collect::<Vec<_>>()
        .join(",");
    let protocol_seq = protocols
        .iter()
        .map(|p| protocol_token(*p))
        .collect::<Vec<_>>()
        .join(",");
    let fee_seq = fee_tiers
        .iter()
        .map(|f| f.map(|x| x.to_string()).unwrap_or_else(|| "none".to_string()))
        .collect::<Vec<_>>()
        .join(",");
    let dir_seq = directions
        .iter()
        .map(|d| d.as_str())
        .collect::<Vec<_>>()
        .join(",");
    format!(
        "{chain_id}|{}|{token_seq}|{pool_seq}|{protocol_seq}|{fee_seq}|{dir_seq}",
        route_kind.as_str()
    )
}

/// Hex-encode a keccak256 digest as a `0x`-prefixed lowercase string.
fn hash_input(input: &str) -> String {
    format!("0x{}", hex::encode(keccak256(input.as_bytes())))
}

/// Canonicalize a closed cycle and compute its deterministic `route_hash`.
///
/// All input vectors must have equal length `hops ≥ 2`, in traversal order
/// (hop `i` swaps `tokens[i] → tokens[(i+1) % hops]` via `pools[i]`). The cycle
/// is rotated to the start that yields the lexicographically-smallest canonical
/// input; reverse orderings are never generated, so opposite-direction cycles
/// keep distinct hashes.
///
/// Returns `None` if the vectors are empty or length-mismatched (caller bug).
pub fn canonicalize(
    chain_id: u64,
    route_kind: RouteKind,
    tokens: &[Address],
    pools: &[Address],
    protocols: &[ProtocolType],
    fee_tiers: &[Option<u32>],
    directions: &[RouteDirection],
) -> Option<Canonical> {
    let n = tokens.len();
    if n < 2
        || pools.len() != n
        || protocols.len() != n
        || fee_tiers.len() != n
        || directions.len() != n
    {
        return None;
    }

    // Pick the rotation whose rendered canonical input is lexicographically
    // smallest. Because addresses render to fixed-width `0x…` hex and the start
    // token differs per rotation in a simple cycle, this is a stable choice.
    let mut best_k = 0usize;
    let mut best_input = render(
        chain_id, route_kind, tokens, pools, protocols, fee_tiers, directions,
    );
    for k in 1..n {
        let candidate = render(
            chain_id,
            route_kind,
            &rotate(tokens, k),
            &rotate(pools, k),
            &rotate(protocols, k),
            &rotate(fee_tiers, k),
            &rotate(directions, k),
        );
        if candidate < best_input {
            best_input = candidate;
            best_k = k;
        }
    }

    Some(Canonical {
        route_hash: hash_input(&best_input),
        tokens: rotate(tokens, best_k),
        pools: rotate(pools, best_k),
        protocols: rotate(protocols, best_k),
        fee_tiers: rotate(fee_tiers, best_k),
        directions: rotate(directions, best_k),
    })
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    fn addr(n: u64) -> Address {
        Address::from_low_u64_be(n)
    }

    // A canonical triangular cycle A(1)→B(2)→C(3)→A, all V2, fee 30, directions
    // chosen arbitrarily but consistently. Returns the 5 parallel vectors.
    #[allow(clippy::type_complexity)]
    fn tri_cycle() -> (
        Vec<Address>,
        Vec<Address>,
        Vec<ProtocolType>,
        Vec<Option<u32>>,
        Vec<RouteDirection>,
    ) {
        (
            vec![addr(1), addr(2), addr(3)],
            vec![addr(0x10), addr(0x20), addr(0x30)],
            vec![ProtocolType::V2, ProtocolType::V2, ProtocolType::V2],
            vec![Some(30), Some(30), Some(30)],
            vec![
                RouteDirection::ZeroForOne,
                RouteDirection::ZeroForOne,
                RouteDirection::OneForZero,
            ],
        )
    }

    #[test]
    fn hash_is_deterministic() {
        let (t, p, pr, f, d) = tri_cycle();
        let a = canonicalize(1, RouteKind::Triangular, &t, &p, &pr, &f, &d).unwrap();
        let b = canonicalize(1, RouteKind::Triangular, &t, &p, &pr, &f, &d).unwrap();
        assert_eq!(a.route_hash, b.route_hash);
        assert!(a.route_hash.starts_with("0x"));
        assert_eq!(a.route_hash.len(), 2 + 64);
    }

    #[test]
    fn same_direction_rotation_collapses_to_one_hash() {
        let (t, p, pr, f, d) = tri_cycle();
        // Rotate the WHOLE cycle by 1: start at B instead of A. Same cycle,
        // same direction → must produce the same hash.
        let rt = rotate(&t, 1);
        let rp = rotate(&p, 1);
        let rpr = rotate(&pr, 1);
        let rf = rotate(&f, 1);
        let rd = rotate(&d, 1);

        let base = canonicalize(1, RouteKind::Triangular, &t, &p, &pr, &f, &d).unwrap();
        let rotated =
            canonicalize(1, RouteKind::Triangular, &rt, &rp, &rpr, &rf, &rd).unwrap();
        assert_eq!(base.route_hash, rotated.route_hash);
        // Canonical rotation must also pin the same start token for both.
        assert_eq!(base.tokens, rotated.tokens);
    }

    #[test]
    fn reverse_direction_is_preserved_as_distinct_hash() {
        let (t, p, pr, f, d) = tri_cycle();
        let forward = canonicalize(1, RouteKind::Triangular, &t, &p, &pr, &f, &d).unwrap();

        // Counter-clockwise traversal A→C→B→A: reverse the node order and the
        // per-hop arrays, and flip each direction. Economically distinct route.
        let rt = vec![addr(1), addr(3), addr(2)];
        let rp = vec![addr(0x30), addr(0x20), addr(0x10)];
        let rpr = vec![ProtocolType::V2, ProtocolType::V2, ProtocolType::V2];
        let rf = vec![Some(30), Some(30), Some(30)];
        let rd = vec![
            RouteDirection::ZeroForOne,
            RouteDirection::OneForZero,
            RouteDirection::ZeroForOne,
        ];
        let reverse = canonicalize(1, RouteKind::Triangular, &rt, &rp, &rpr, &rf, &rd).unwrap();
        assert_ne!(forward.route_hash, reverse.route_hash);
    }

    #[test]
    fn flipping_a_single_direction_changes_the_hash() {
        let (t, p, pr, f, mut d) = tri_cycle();
        let base = canonicalize(1, RouteKind::Triangular, &t, &p, &pr, &f, &d).unwrap();
        d[0] = match d[0] {
            RouteDirection::ZeroForOne => RouteDirection::OneForZero,
            RouteDirection::OneForZero => RouteDirection::ZeroForOne,
        };
        let flipped = canonicalize(1, RouteKind::Triangular, &t, &p, &pr, &f, &d).unwrap();
        assert_ne!(base.route_hash, flipped.route_hash);
    }

    #[test]
    fn different_pool_or_fee_changes_the_hash() {
        let (t, p, pr, f, d) = tri_cycle();
        let base = canonicalize(1, RouteKind::Triangular, &t, &p, &pr, &f, &d).unwrap();

        let mut p2 = p.clone();
        p2[1] = addr(0x99);
        let other_pool = canonicalize(1, RouteKind::Triangular, &t, &p2, &pr, &f, &d).unwrap();
        assert_ne!(base.route_hash, other_pool.route_hash);

        let mut f2 = f.clone();
        f2[1] = Some(100);
        let other_fee = canonicalize(1, RouteKind::Triangular, &t, &p, &pr, &f2, &d).unwrap();
        assert_ne!(base.route_hash, other_fee.route_hash);
    }

    #[test]
    fn two_cycle_canonicalizes_and_distinguishes_pool_order() {
        // A↔B via two pools. Same-direction rotation (start at B) collapses;
        // swapping which pool is bought first is a *reverse* and stays distinct.
        let t = vec![addr(5), addr(7)];
        let pools_ab = vec![addr(0xa1), addr(0xb2)];
        let pr = vec![ProtocolType::V2, ProtocolType::V3];
        let f = vec![Some(30), Some(500)];
        let d = vec![RouteDirection::ZeroForOne, RouteDirection::OneForZero];
        let base = canonicalize(1, RouteKind::V2V3, &t, &pools_ab, &pr, &f, &d).unwrap();

        // Buy on b2 first, sell on a1: distinct economic route.
        let pools_ba = vec![addr(0xb2), addr(0xa1)];
        let pr2 = vec![ProtocolType::V3, ProtocolType::V2];
        let f2 = vec![Some(500), Some(30)];
        let swapped = canonicalize(1, RouteKind::V3V2, &t, &pools_ba, &pr2, &f2, &d).unwrap();
        assert_ne!(base.route_hash, swapped.route_hash);

        assert!(base.route_hash.starts_with("0x"));
    }

    #[test]
    fn rejects_malformed_input() {
        let t = vec![addr(1), addr(2)];
        let p = vec![addr(0x10)]; // mismatched length
        assert!(canonicalize(
            1,
            RouteKind::V2V2,
            &t,
            &p,
            &[ProtocolType::V2, ProtocolType::V2],
            &[Some(30), Some(30)],
            &[RouteDirection::ZeroForOne, RouteDirection::OneForZero],
        )
        .is_none());
        assert!(canonicalize(
            1,
            RouteKind::V2V2,
            &[addr(1)],
            &[addr(0x10)],
            &[ProtocolType::V2],
            &[Some(30)],
            &[RouteDirection::ZeroForOne],
        )
        .is_none());
    }
}
