//! Route-discovery core types — Phase 1 (topology-only radar).
//!
//! These types describe *discovered routes* (closed cycles) over the live pool
//! graph. Phase 1 carries **no sizing and no profit** — only topology,
//! classification, and provenance.
//!
//! `RouteEdge::log_weight` is stored *forward-compatibly* for the Phase 2 MMBF
//! line-graph (`−log((1−fee)·rate)`), but Phase 1 NEVER uses it for ranking or
//! profit — it is metadata. (RULE 00 / R8: nothing is fabricated; an unknown
//! weight is `None`, never a sentinel `0.0`.)
//!
//! ## Cycle representation
//!
//! A closed cycle of `hops` swaps is stored as parallel vectors of length
//! `hops`, all in traversal order, where the cycle returns from the last token
//! back to `tokens[0]`:
//! - `tokens[i]`     — the *input* token of hop `i` (distinct nodes; no closing repeat)
//! - `pools[i]`      — the pool that swaps `tokens[i] → tokens[(i+1) % hops]`
//! - `protocols[i]`  — that pool's protocol family
//! - `fee_tiers[i]`  — that pool's fee in bps (`None` when not known)
//! - `directions[i]` — whether hop `i` consumes token0 or token1 of its pool
//!
//! So a 2-cycle `A→B→A` is `tokens=[A,B]`, `pools=[p0,p1]` (closes `B→A` via
//! `p1`); a triangular `A→B→C→A` is `tokens=[A,B,C]`, `pools=[p0,p1,p2]`.

use crate::route_intent::{ProtocolType, RouteIntent};
use crate::strategy_label::StrategyLabel;
use ethers::types::Address;
use serde::{Deserialize, Serialize};

/// Shape of a discovered route, derived from the protocols of its pools.
///
/// The four `V*` variants are 2-cycles (DEX cross-arb); `Triangular` is a
/// 3-cycle discovered from the live graph; `MultiHop` is **reserved for
/// Phase 2** (MMBF, ≥4 hops) and is never produced in Phase 1.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum RouteKind {
    #[serde(rename = "v2v2")]
    V2V2,
    #[serde(rename = "v2v3")]
    V2V3,
    #[serde(rename = "v3v2")]
    V3V2,
    #[serde(rename = "v3v3")]
    V3V3,
    #[serde(rename = "triangular")]
    Triangular,
    #[serde(rename = "multihop")]
    MultiHop,
}

impl RouteKind {
    /// Stable lowercase token used in telemetry and the canonical hash input.
    pub fn as_str(self) -> &'static str {
        match self {
            RouteKind::V2V2 => "v2v2",
            RouteKind::V2V3 => "v2v3",
            RouteKind::V3V2 => "v3v2",
            RouteKind::V3V3 => "v3v3",
            RouteKind::Triangular => "triangular",
            RouteKind::MultiHop => "multihop",
        }
    }

    /// Derive the 2-cycle kind from the ordered protocols of the two pools
    /// (`p0` = buy/first pool, `p1` = sell/second pool).
    ///
    /// Returns `None` for any protocol mix outside the V2/V3 universe
    /// (Curve / Balancer / Unknown) — Phase 1 only classifies V2/V3 cross-arb.
    pub fn from_two_protocols(p0: ProtocolType, p1: ProtocolType) -> Option<Self> {
        match (p0, p1) {
            (ProtocolType::V2, ProtocolType::V2) => Some(RouteKind::V2V2),
            (ProtocolType::V2, ProtocolType::V3) => Some(RouteKind::V2V3),
            (ProtocolType::V3, ProtocolType::V2) => Some(RouteKind::V3V2),
            (ProtocolType::V3, ProtocolType::V3) => Some(RouteKind::V3V3),
            _ => None,
        }
    }

    /// `true` when this is a two-leg DEX cross-arb shape.
    pub fn is_two_cycle(self) -> bool {
        matches!(
            self,
            RouteKind::V2V2 | RouteKind::V2V3 | RouteKind::V3V2 | RouteKind::V3V3
        )
    }
}

/// Orientation of a single swap hop relative to its pool's `(token0, token1)`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RouteDirection {
    /// Hop consumes `token0`, emits `token1`.
    ZeroForOne,
    /// Hop consumes `token1`, emits `token0`.
    OneForZero,
}

impl RouteDirection {
    /// Stable token used in telemetry and the canonical hash input.
    pub fn as_str(self) -> &'static str {
        match self {
            RouteDirection::ZeroForOne => "0to1",
            RouteDirection::OneForZero => "1to0",
        }
    }

    /// Resolve the hop direction from the input token and the pool's `token0`.
    ///
    /// `token_in == token0` ⇒ `ZeroForOne`, otherwise `OneForZero`. The caller
    /// guarantees `token_in` is one of the pool's two tokens.
    pub fn from_in_token0(token_in: Address, token0: Address) -> Self {
        if token_in == token0 {
            RouteDirection::ZeroForOne
        } else {
            RouteDirection::OneForZero
        }
    }
}

/// A single directed swap edge of the token graph (one pool, one orientation).
///
/// Each pool yields **two** edges (`token0→token1` and `token1→token0`). All
/// magnitude fields are honest: `liquidity_hint`/`log_weight` are `None` when
/// they cannot be computed from fresh on-chain data, never a fabricated `0.0`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RouteEdge {
    pub chain_id: u64,
    pub pool: Address,
    pub token_in: Address,
    pub token_out: Address,
    pub token0: Address,
    pub token1: Address,
    pub protocol: ProtocolType,
    pub fee_bps: Option<u32>,
    /// Coarse liquidity magnitude for low-liquidity rejection / ordering.
    /// V2: `r0+r1` normalized by decimals; V3: active `liquidity`. `None` when
    /// reserves/slot0 are missing.
    pub liquidity_hint: Option<f64>,
    /// **Phase-2 forward-compat only** — `−log((1−fee)·rate)` (MMBF edge weight).
    /// `None` when the rate is not computable. Phase 1 does NOT rank on this.
    pub log_weight: Option<f64>,
    /// Unix epoch seconds of the underlying reserves/slot0 snapshot.
    pub freshness_ts: u64,
    /// Block number of the snapshot (`0` for V3 — slot0 carries no block).
    pub blk: u64,
    pub direction: RouteDirection,
}

/// A strategy that was considered for a route but does not apply, with the
/// machine-readable reason (e.g. `requires_3_legs`, `strategy_not_route_based`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RejectedStrategy {
    pub strategy: StrategyLabel,
    pub reason: String,
}

/// A discovered, canonicalized, classified route — the radar's primary output.
///
/// Phase 1 invariant: this contains **no amount, no profit, no sizing**. It is
/// pure topology + classification + provenance.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RouteCandidate {
    pub chain_id: u64,
    pub route_hash: String,
    pub route_kind: RouteKind,
    /// Distinct nodes in canonical traversal order (length `hops`); the cycle
    /// closes `tokens[hops-1] → tokens[0]`.
    pub tokens: Vec<Address>,
    pub pools: Vec<Address>,
    pub protocols: Vec<ProtocolType>,
    pub fee_tiers: Vec<Option<u32>>,
    pub directions: Vec<RouteDirection>,
    pub hops: u8,
    pub applicable_strategies: Vec<StrategyLabel>,
    pub rejected_strategies: Vec<RejectedStrategy>,
    /// Discovery mode that produced this candidate (e.g. `"shadow"`).
    pub mode: String,
}

/// A route paired with the explicit strategy it is being dispatched as, plus the
/// `RouteIntent` handed to `shadow_evaluate_intent`.
///
/// `dispatch_deferred` is set when the route is classified + applicable but
/// cannot reach a cartridge yet (e.g. triangular needs a pool-data adapter) —
/// honest, telemetry-only; the dispatcher does NOT fabricate an evaluation.
#[derive(Debug, Clone)]
pub struct DispatchedRoute {
    pub route_hash: String,
    pub strategy: StrategyLabel,
    pub intent: RouteIntent,
    pub dispatch_deferred: Option<String>,
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn route_kind_str_roundtrip_via_serde() {
        for (k, s) in [
            (RouteKind::V2V2, "v2v2"),
            (RouteKind::V2V3, "v2v3"),
            (RouteKind::V3V2, "v3v2"),
            (RouteKind::V3V3, "v3v3"),
            (RouteKind::Triangular, "triangular"),
            (RouteKind::MultiHop, "multihop"),
        ] {
            assert_eq!(k.as_str(), s);
            let json = serde_json::to_string(&k).unwrap();
            assert_eq!(json, format!("\"{s}\""));
        }
    }

    #[test]
    fn from_two_protocols_covers_v2_v3_matrix_and_rejects_others() {
        use ProtocolType::*;
        assert_eq!(RouteKind::from_two_protocols(V2, V2), Some(RouteKind::V2V2));
        assert_eq!(RouteKind::from_two_protocols(V2, V3), Some(RouteKind::V2V3));
        assert_eq!(RouteKind::from_two_protocols(V3, V2), Some(RouteKind::V3V2));
        assert_eq!(RouteKind::from_two_protocols(V3, V3), Some(RouteKind::V3V3));
        // Curve / Balancer / Unknown are out of the Phase-1 universe.
        assert_eq!(RouteKind::from_two_protocols(V2, Curve), None);
        assert_eq!(RouteKind::from_two_protocols(Balancer, V3), None);
        assert_eq!(RouteKind::from_two_protocols(Unknown, Unknown), None);
    }

    #[test]
    fn direction_from_in_token0() {
        let t0 = Address::from_low_u64_be(0x11);
        let t1 = Address::from_low_u64_be(0x22);
        assert_eq!(
            RouteDirection::from_in_token0(t0, t0),
            RouteDirection::ZeroForOne
        );
        assert_eq!(
            RouteDirection::from_in_token0(t1, t0),
            RouteDirection::OneForZero
        );
        assert_eq!(RouteDirection::ZeroForOne.as_str(), "0to1");
        assert_eq!(RouteDirection::OneForZero.as_str(), "1to0");
    }

    #[test]
    fn two_cycle_predicate() {
        assert!(RouteKind::V2V2.is_two_cycle());
        assert!(RouteKind::V3V3.is_two_cycle());
        assert!(!RouteKind::Triangular.is_two_cycle());
        assert!(!RouteKind::MultiHop.is_two_cycle());
    }
}
