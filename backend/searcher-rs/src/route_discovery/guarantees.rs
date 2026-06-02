//! Cross-module safety guarantees for route discovery (test-only).
//!
//! These tests pin the **NO-ACTIVE GUARANTEE** and the **opps:detected-untouched**
//! invariant at the suite level, spanning the worker + telemetry + dispatcher so a
//! regression in any one of them fails here. The runtime counterpart (XLEN of
//! `arbx:opps:detected` unchanged before/after a shadow tick) is asserted during
//! deploy verification.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::route_discovery::graph_builder::{GraphBuildOutcome, RejectedEdge, TokenGraph};
use crate::route_discovery::route_discovery_worker::evaluate_tick;
use crate::route_discovery::strategy_applicability::StrategyApplicabilityEngine;
use crate::route_discovery::telemetry::{
    rejected_event, route_candidate_event, route_intent_emitted_event,
    strategy_applicability_event, tick_event, ROUTE_DISCOVERY_TELEMETRY_CHANNEL,
};
use crate::route_discovery::types::{RouteCandidate, RouteDirection, RouteEdge, RouteKind};
use crate::route_discovery::unique_route_finder::RouteFinderConfig;
use crate::route_discovery::RouteDiscoveryMode;
use crate::route_intent::{DetectionSource, ProtocolType};
use ethers::types::{Address, U256};
use std::collections::HashMap;

const ALLOWED_EVENTS: [&str; 5] = [
    "route_discovery.tick",
    "route_discovery.route_candidate",
    "route_discovery.strategy_applicability",
    "route_discovery.rejected",
    "route_intent.emitted",
];

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

/// Outcome with two V2 pools over (A,B) (→ 2-cycles) plus one rejected pool.
fn routable_outcome() -> GraphBuildOutcome {
    let mut edges = Vec::new();
    for &(p, t0, t1) in &[(0x10u64, 1u64, 2u64), (0x20, 1, 2)] {
        edges.push(dir_edge(p, t0, t1, t0, t1, ProtocolType::V2));
        edges.push(dir_edge(p, t1, t0, t0, t1, ProtocolType::V2));
    }
    let mut adjacency: HashMap<Address, Vec<usize>> = HashMap::new();
    for (i, e) in edges.iter().enumerate() {
        adjacency.entry(e.token_in).or_default().push(i);
    }
    GraphBuildOutcome {
        graph: TokenGraph { edges, adjacency },
        rejected: vec![RejectedEdge {
            pool: addr(0x99),
            reason: "missing_reserves".to_string(),
        }],
        pools_total: 3,
    }
}

#[test]
fn route_discovery_mode_has_no_active_variant() {
    // Exhaustive match: if an `Active` variant were ever added, this fails to
    // compile — pinning the type-level NO-ACTIVE guarantee.
    for m in [RouteDiscoveryMode::Off, RouteDiscoveryMode::Shadow] {
        match m {
            RouteDiscoveryMode::Off => assert!(!m.is_enabled()),
            RouteDiscoveryMode::Shadow => assert!(m.is_enabled()),
        }
        assert_ne!(m.as_str(), "active");
    }
}

#[test]
fn telemetry_channel_is_dedicated_not_opps() {
    assert_eq!(ROUTE_DISCOVERY_TELEMETRY_CHANNEL, "arbx:route_discovery:telemetry");
    assert!(!ROUTE_DISCOVERY_TELEMETRY_CHANNEL.contains("opps"));
}

#[test]
fn every_telemetry_builder_avoids_opps_detected() {
    let c = RouteCandidate {
        chain_id: 1,
        route_hash: format!("0x{}", "cd".repeat(32)),
        route_kind: RouteKind::V2V3,
        tokens: vec![addr(1), addr(2)],
        pools: vec![addr(0x10), addr(0x20)],
        protocols: vec![ProtocolType::V2, ProtocolType::V3],
        fee_tiers: vec![Some(30), Some(500)],
        directions: vec![RouteDirection::ZeroForOne, RouteDirection::OneForZero],
        hops: 2,
        applicable_strategies: vec![],
        rejected_strategies: vec![],
        mode: "shadow".to_string(),
    };
    let events = vec![
        tick_event(1, "dfs_bounded", 1, 2, 0, 1, 0, 1, 0, 5, "shadow"),
        route_candidate_event(1, "dfs_bounded", &c),
        strategy_applicability_event(1, &c),
        rejected_event(
            1,
            &RejectedEdge {
                pool: addr(0x99),
                reason: "missing_reserves".to_string(),
            },
        ),
        route_intent_emitted_event(1, "0xabc", "dex_arb_v2v3", "shadow", None),
    ];
    for e in &events {
        let name = e["event"].as_str().unwrap();
        assert!(ALLOWED_EVENTS.contains(&name), "unexpected event {name}");
        let s = serde_json::to_string(e).unwrap();
        assert!(!s.contains("opps:detected"), "event {name} mentions opps:detected");
        assert!(!s.contains("opps_detected"));
    }
}

#[test]
fn dispatch_off_produces_no_intents_or_emitted_events() {
    let outcome = routable_outcome();
    let engine = StrategyApplicabilityEngine::default();
    let finder = RouteFinderConfig::default();
    let tick = evaluate_tick(&outcome, 1, &engine, &finder, 200, false, 0, "shadow");
    assert!(tick.routes_found > 0, "graph is routable");
    assert_eq!(tick.routes_dispatched, 0);
    assert!(tick.dispatch_intents.is_empty());
    assert!(tick
        .events
        .iter()
        .all(|e| e["event"] != "route_intent.emitted"));
    assert_eq!(tick.tick_summary["routes_dispatched"], 0);
}

#[test]
fn shadow_tick_stamps_algorithm_dfs_bounded_everywhere() {
    let outcome = routable_outcome();
    let engine = StrategyApplicabilityEngine::default();
    let finder = RouteFinderConfig::default();
    let tick = evaluate_tick(&outcome, 1, &engine, &finder, 200, true, 0, "shadow");
    // Every event that carries an algorithm field must say dfs_bounded.
    for e in tick.events.iter().chain(std::iter::once(&tick.tick_summary)) {
        if let Some(alg) = e.get("algorithm") {
            assert_eq!(alg, "dfs_bounded");
        }
        let name = e["event"].as_str().unwrap();
        assert!(ALLOWED_EVENTS.contains(&name), "unexpected event {name}");
    }
    assert_eq!(tick.tick_summary["mode"], "shadow");
    assert_eq!(tick.tick_summary["algorithm"], "dfs_bounded");
}

#[test]
fn all_dispatch_intents_are_observe_only_newblock_zero_amount() {
    let outcome = routable_outcome();
    let engine = StrategyApplicabilityEngine::default();
    let finder = RouteFinderConfig::default();
    let tick = evaluate_tick(&outcome, 1, &engine, &finder, 200, true, 0, "shadow");
    assert!(tick.routes_dispatched > 0);
    for intent in &tick.dispatch_intents {
        // NewBlock → cartridge_matches_intent routes ONLY to dex_arb.
        assert_eq!(intent.source_event, DetectionSource::NewBlock);
        // Phase 1 has NO sizing.
        assert_eq!(intent.amount_in, U256::zero());
        assert!(!intent.legs.is_empty());
    }
}
