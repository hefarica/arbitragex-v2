//! RouteIntentDispatcher — turns applicable `RouteCandidate`s into shadow
//! cartridge evaluations, with an **explicit strategy** per dispatch.
//!
//! ## Hard invariants (NO-ACTIVE GUARANTEE)
//! - The ONLY downstream is [`crate::cartridge_boot::shadow_evaluate_intent`]
//!   (observe-only). The dispatcher NEVER calls `on_route_intent`,
//!   `process_candidate`, the emitter, or writes `arbx:opps:detected`.
//! - Only strategies with an **existing, enabled cartridge** are dispatched
//!   (`dex_arb`, `triangular_arb`). `flashloan_arb`/`stable_arb` (no cartridge)
//!   are surfaced in applicability telemetry but never dispatched.
//! - **Triangular** routes are dispatched like `dex_arb` (D-01, 2026-08-16):
//!   route_discovery's closed 3-hop cycles are the triangle source, and
//!   `cartridge_matches_intent` discriminates by shape — `triangular_arb` takes
//!   closed ≥3-leg cycles, `dex_arb` excludes them (a triangle is not a
//!   single-pair spread, the mis-evaluation the old deferral feared).
//!
//! Phase 1 builds intents with **no sizing** (`amount_in = 0`): the cartridge
//! prices the source pool in shadow (e.g. `v3_source_priced`) and returns no
//! sized opportunity — honest, no fabrication.

use crate::route_discovery::strategy_applicability::StrategyApplicabilityEngine;
use crate::route_discovery::types::RouteCandidate;
use crate::route_intent::{
    DetectionSource, RouteIntent, RouteIntentLeg, RouterKind, SwapExactMode,
};
use crate::strategy_label::StrategyLabel;
use ethers::types::{Address, H256, U256};
use std::str::FromStr;

/// Coarse config/cartridge name for a granular `StrategyLabel`.
pub fn coarse_name(label: StrategyLabel) -> &'static str {
    match label {
        StrategyLabel::DexArbV2V2
        | StrategyLabel::DexArbV2V3
        | StrategyLabel::DexArbV3V2
        | StrategyLabel::DexArbV3V3 => "dex_arb",
        StrategyLabel::TriangularArb | StrategyLabel::SpanningTreeArb => "triangular_arb",
        StrategyLabel::FlashloanArb => "flashloan_arb",
        StrategyLabel::Liquidation | StrategyLabel::LiquidationSnipe => "liquidation",
        StrategyLabel::CrossChainArb => "cross_chain_arb",
    }
}

/// One planned dispatch for a (route, strategy) pair.
///
/// `intent.is_some()` ⇒ actually shadow-evaluate it; `intent.is_none()` with
/// `dispatch_deferred = Some(..)` ⇒ telemetry-only (no current arm defers).
#[derive(Debug, Clone)]
pub struct DispatchPlan {
    pub route_hash: String,
    pub strategy_label: StrategyLabel,
    pub strategy_name: &'static str,
    pub intent: Option<RouteIntent>,
    pub dispatch_deferred: Option<String>,
}

/// Build a synthetic `RouteIntent` from a discovered route.
///
/// - `tx_hash` = the route_hash (a real keccak digest → traceable, non-colliding,
///   NOT a sentinel). `router`/`sender` are `zero` because a discovered route has
///   no originating router call — honest "N/A", and this intent is observe-only
///   (never persisted, never broadcast).
/// - `source_event = NewBlock` so `cartridge_matches_intent` routes it by
///   shape: `dex_arb` for 2-leg cycles, `triangular_arb` for closed ≥3-leg
///   cycles.
/// - `amount_in = 0`: Phase 1 has no sizing.
pub fn build_intent(c: &RouteCandidate) -> Option<RouteIntent> {
    // Guard every vector this function indexes by position (tokens/pools/
    // protocols). fee_tiers is read via .get(i) so a short fee_tiers is safe.
    if c.tokens.is_empty() || c.pools.len() != c.tokens.len() || c.protocols.len() != c.tokens.len()
    {
        return None;
    }
    let n = c.tokens.len();
    let mut legs = Vec::with_capacity(n);
    for i in 0..n {
        legs.push(RouteIntentLeg {
            token_in: c.tokens[i],
            token_out: c.tokens[(i + 1) % n],
            pool_hint: Some(c.pools[i]),
            dex_hint: None,
            fee_bps: c.fee_tiers.get(i).copied().flatten(),
            protocol_type: c.protocols[i],
        });
    }
    // route_hash is "0x" + 64 hex = exactly 32 bytes → a meaningful synthetic id.
    // Fail-honest (R8): if it ever fails to parse (broken upstream canonicalizer),
    // return None rather than fabricating an all-zeros H256 sentinel that would both
    // hide the corruption and collide across routes on a dedup keyed by tx_hash.
    let tx_hash = H256::from_str(&c.route_hash).ok()?;
    RouteIntent::new(
        c.chain_id,
        tx_hash,
        Address::zero(),
        RouterKind::Unknown,
        Address::zero(),
        legs,
        U256::zero(),
        None,
        SwapExactMode::ExactIn,
        DetectionSource::NewBlock,
    )
}

/// Plan the dispatches for one annotated candidate.
///
/// Iterates the candidate's `applicable_strategies`; only strategies whose
/// cartridge exists+enabled produce a plan. `dex_arb`/`triangular_arb` → real
/// intent; everything else (no cartridge) → skipped.
pub fn plan_dispatch(
    c: &RouteCandidate,
    engine: &StrategyApplicabilityEngine,
) -> Vec<DispatchPlan> {
    let mut plans = Vec::new();
    for label in &c.applicable_strategies {
        let name = coarse_name(*label);
        if !engine.has_cartridge(name) {
            continue; // flashloan_arb / stable_arb / disabled → telemetry-only, no dispatch
        }
        match name {
            "dex_arb" => {
                if let Some(intent) = build_intent(c) {
                    plans.push(DispatchPlan {
                        route_hash: c.route_hash.clone(),
                        strategy_label: *label,
                        strategy_name: name,
                        intent: Some(intent),
                        dispatch_deferred: None,
                    });
                }
            }
            "triangular_arb" => {
                // Closed 3-hop cycles are the triangle source (D-01): dispatch
                // for real. cartridge_matches_intent's shape gate keeps this
                // intent out of dex_arb (and swap shapes out of triangular).
                if let Some(intent) = build_intent(c) {
                    plans.push(DispatchPlan {
                        route_hash: c.route_hash.clone(),
                        strategy_label: *label,
                        strategy_name: name,
                        intent: Some(intent),
                        dispatch_deferred: None,
                    });
                }
            }
            // liquidation has a cartridge but is position-based; it is never in
            // a DEX route's applicable set anyway. Defensive no-op.
            _ => {}
        }
    }
    plans
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use crate::route_discovery::types::{RejectedStrategy, RouteDirection, RouteKind};
    use crate::route_intent::ProtocolType;

    fn addr(n: u64) -> Address {
        Address::from_low_u64_be(n)
    }

    fn candidate(kind: RouteKind, applicable: Vec<StrategyLabel>) -> RouteCandidate {
        let (tokens, pools, protocols, fee_tiers, directions, hops) = match kind {
            RouteKind::Triangular => (
                vec![addr(1), addr(2), addr(3)],
                vec![addr(0x10), addr(0x20), addr(0x30)],
                vec![ProtocolType::V2, ProtocolType::V2, ProtocolType::V2],
                vec![Some(30), Some(30), Some(30)],
                vec![
                    RouteDirection::ZeroForOne,
                    RouteDirection::ZeroForOne,
                    RouteDirection::OneForZero,
                ],
                3u8,
            ),
            _ => (
                vec![addr(1), addr(2)],
                vec![addr(0x10), addr(0x20)],
                vec![ProtocolType::V2, ProtocolType::V3],
                vec![Some(30), Some(500)],
                vec![RouteDirection::ZeroForOne, RouteDirection::OneForZero],
                2u8,
            ),
        };
        RouteCandidate {
            chain_id: 1,
            route_hash: format!("0x{}", "ab".repeat(32)),
            route_kind: kind,
            tokens,
            pools,
            protocols,
            fee_tiers,
            directions,
            hops,
            applicable_strategies: applicable,
            rejected_strategies: vec![RejectedStrategy {
                strategy: "liquidation".to_string(),
                reason: "strategy_not_route_based".to_string(),
            }],
            mode: "shadow".to_string(),
        }
    }

    #[test]
    fn build_intent_uses_route_hash_as_tx_hash_and_zero_amount() {
        let c = candidate(RouteKind::V2V3, vec![]);
        let intent = build_intent(&c).unwrap();
        assert_eq!(intent.legs.len(), 2);
        assert_eq!(intent.amount_in, U256::zero());
        assert_eq!(intent.source_event, DetectionSource::NewBlock);
        // First leg points at the first pool so shadow_evaluate_intent reads its reserves.
        assert_eq!(intent.legs[0].pool_hint, Some(addr(0x10)));
        assert_eq!(intent.legs[0].token_in, addr(1));
        assert_eq!(intent.legs[0].token_out, addr(2));
        // Closing leg returns to token0.
        assert_eq!(intent.legs[1].token_out, addr(1));
        // tx_hash is the route_hash digest, not zero.
        assert_ne!(intent.tx_hash, H256::zero());
    }

    #[test]
    fn dex_arb_two_cycle_is_dispatched() {
        let eng = StrategyApplicabilityEngine::default();
        let c = candidate(
            RouteKind::V2V3,
            vec![StrategyLabel::DexArbV2V3, StrategyLabel::FlashloanArb],
        );
        let plans = plan_dispatch(&c, &eng);
        // dex_arb dispatched (intent Some); flashloan_arb has no cartridge → skipped.
        assert_eq!(plans.len(), 1);
        assert_eq!(plans[0].strategy_name, "dex_arb");
        assert!(plans[0].intent.is_some());
        assert!(plans[0].dispatch_deferred.is_none());
    }

    #[test]
    fn triangular_cycle_is_dispatched() {
        let eng = StrategyApplicabilityEngine::default();
        let c = candidate(
            RouteKind::Triangular,
            vec![StrategyLabel::TriangularArb, StrategyLabel::FlashloanArb],
        );
        let plans = plan_dispatch(&c, &eng);
        assert_eq!(plans.len(), 1);
        assert_eq!(plans[0].strategy_name, "triangular_arb");
        assert!(plans[0].intent.is_some(), "triangular IS shadow-evaluated");
        assert!(plans[0].dispatch_deferred.is_none());
        // The dispatched intent is the closed 3-leg cycle ending on token0.
        let intent = plans[0].intent.as_ref().unwrap();
        assert_eq!(intent.legs.len(), 3);
        assert_eq!(intent.legs.first().unwrap().token_in, addr(1));
        assert_eq!(intent.legs.last().unwrap().token_out, addr(1));
    }

    #[test]
    fn flashloan_only_route_dispatches_nothing() {
        let eng = StrategyApplicabilityEngine::default();
        // Only flashloan applicable (no cartridge) → no plans.
        let c = candidate(RouteKind::V2V2, vec![StrategyLabel::FlashloanArb]);
        let plans = plan_dispatch(&c, &eng);
        assert!(plans.is_empty());
    }

    #[test]
    fn coarse_name_maps_all_dex_variants() {
        assert_eq!(coarse_name(StrategyLabel::DexArbV2V2), "dex_arb");
        assert_eq!(coarse_name(StrategyLabel::DexArbV3V3), "dex_arb");
        assert_eq!(coarse_name(StrategyLabel::TriangularArb), "triangular_arb");
        assert_eq!(coarse_name(StrategyLabel::FlashloanArb), "flashloan_arb");
        assert_eq!(coarse_name(StrategyLabel::Liquidation), "liquidation");
    }
}
