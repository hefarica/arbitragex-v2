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

    /// Classify a cycle from its **canonical** protocol sequence (post-rotation).
    ///
    /// Deriving the kind from the canonical order (rather than the discovery
    /// order) is what lets the same physical cycle dedup across start tokens.
    /// 2 hops → the V2/V3 matrix; 3 hops → `Triangular`; ≥4 → `MultiHop`
    /// (Phase 2). Returns `None` for a 2-hop mix outside V2/V3 or empty input.
    pub fn classify(protocols: &[ProtocolType]) -> Option<Self> {
        match protocols.len() {
            2 => RouteKind::from_two_protocols(protocols[0], protocols[1]),
            // Symmetric V2/V3 gate: the 2-hop path whitelists V2/V3 via
            // `from_two_protocols`; the 3-hop path must do the same, else a future
            // graph that admits Curve/Balancer/Unknown edges would mislabel a
            // mixed cycle as a valid `Triangular`. Phase 1 is V2/V3-only.
            3 if protocols
                .iter()
                .all(|p| matches!(p, ProtocolType::V2 | ProtocolType::V3)) =>
            {
                Some(RouteKind::Triangular)
            }
            3 => None,
            n if n >= 4 => Some(RouteKind::MultiHop),
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
    /// Coarse liquidity magnitude (TVL proxy) for low-liquidity rejection and
    /// per-pair top-K ordering. V2: `r0+r1` normalized by decimals; V3: virtual
    /// reserves `L/√P + L·√P` normalized — unit-consistent with V2 so parallel
    /// pools of both protocols rank comparably. `None` when reserves/slot0
    /// are missing.
    pub liquidity_hint: Option<f64>,
    /// MMBF edge weight `−ln((1−fee)·rate)` — computed for **both** protocols
    /// (V2 from reserves, V3 from slot0 `sqrtPriceX96`; RU-2). Consumed by the
    /// per-pair top-K ranking (`best net rate` tie-break) and the multi-hop
    /// negative-cycle search. `None` when the rate is not computable — never
    /// a fabricated value.
    pub log_weight: Option<f64>,
    /// Unix epoch seconds of the underlying reserves/slot0 snapshot.
    pub freshness_ts: u64,
    /// Block number of the snapshot (`0` for V3 — slot0 carries no block).
    pub blk: u64,
    pub direction: RouteDirection,
}

/// A strategy that was considered for a route but does not apply, with the
/// machine-readable reason (e.g. `requires_3_legs`, `strategy_not_route_based`).
///
/// `strategy` is the **coarse** config name (`dex_arb` / `triangular_arb` /
/// `flashloan_arb` / `stable_arb` / `liquidation`) rather than a granular
/// `StrategyLabel`, because a rejection like "dex_arb does not do triangular"
/// has no single granular label, and the telemetry format uses coarse names.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RejectedStrategy {
    pub strategy: String,
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

    #[test]
    fn classify_three_hop_gates_on_v2_v3_only() {
        use ProtocolType::{Curve, V2, V3};
        // All-V2 / mixed-V2V3 triangulars classify.
        assert_eq!(
            RouteKind::classify(&[V2, V2, V2]),
            Some(RouteKind::Triangular)
        );
        assert_eq!(
            RouteKind::classify(&[V2, V3, V2]),
            Some(RouteKind::Triangular)
        );
        // A 3-cycle containing a non-V2/V3 leg must NOT be mislabeled Triangular —
        // symmetric with the 2-hop whitelist (R8: don't classify what we can't price).
        assert_eq!(RouteKind::classify(&[V2, Curve, V2]), None);
        assert_eq!(RouteKind::classify(&[Curve, Curve, Curve]), None);
        // 2-hop whitelist unchanged; ≥4 → MultiHop; empty → None.
        assert_eq!(RouteKind::classify(&[V2, Curve]), None);
        assert_eq!(
            RouteKind::classify(&[V2, V2, V2, V2]),
            Some(RouteKind::MultiHop)
        );
        assert_eq!(RouteKind::classify(&[]), None);
    }
}
