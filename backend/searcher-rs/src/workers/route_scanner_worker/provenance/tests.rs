//! Deterministic topology fixtures, not claimed market opportunities.
use super::*;
use crate::route_discovery::types::{RouteDirection, RouteEdge};
use crate::route_intent::{DetectionSource, ProtocolType};
use crate::workers::route_scanner_worker::cycle_to_intent;

fn addr(n: usize) -> Address {
    Address::from_low_u64_be(n as u64)
}

fn fixture(hops: usize) -> (TokenGraph, ProfitableCycle, Vec<PoolRef>) {
    let mut graph = TokenGraph::default();
    let mut pools = Vec::new();
    for i in 0..hops {
        let token_in = addr(i + 1);
        let token_out = addr((i + 1) % hops + 1);
        let pool = addr(100 + i);
        graph.edges.push(RouteEdge {
            chain_id: 1,
            pool,
            token_in,
            token_out,
            token0: token_in,
            token1: token_out,
            protocol: ProtocolType::V2,
            fee_bps: Some(30),
            liquidity_hint: None,
            log_weight: Some(-0.01),
            freshness_ts: 1_700_000_000,
            blk: 1000,
            hot_token: true,
            direction: RouteDirection::ZeroForOne,
        });
        pools.push(PoolRef {
            chain_id: 1,
            address: pool,
            dex_name: format!("fixture-dex-{i}"),
            protocol_type: ProtocolType::V2,
            token0: token_in,
            token1: token_out,
            fee_bps: Some(30),
        });
    }
    let cycle = ProfitableCycle {
        edges: (0..hops).collect(),
        sum_log_weight: -(hops as f64) * 0.01,
        hop_count: hops,
    };
    (graph, cycle, pools)
}

fn verify_hops(hops: usize) {
    let (graph, cycle, pools) = fixture(hops);
    let lookup = pools.iter().map(|p| ((p.chain_id, p.address), p)).collect();
    let mut intent = cycle_to_intent(&graph, 1, 1000, &cycle).unwrap();
    bind_dexes(&mut intent, &lookup).unwrap();
    assert_eq!(intent.observed_block(), Some(1000));
    assert_eq!(intent.legs.len(), hops);
    let path = intent.token_path().unwrap();
    assert_eq!(path.len(), hops + 1);
    assert_eq!(path.first(), path.last());
    for (i, leg) in intent.legs.iter().enumerate() {
        assert_eq!(leg.pool_hint, Some(pools[i].address));
        assert_eq!(leg.dex_hint.as_deref(), Some(pools[i].dex_name.as_str()));
        assert_eq!(leg.token_in, graph.edges[i].token_in);
        assert_eq!(leg.token_out, graph.edges[i].token_out);
    }
    let wire = serde_json::to_value(&intent).unwrap();
    let decoded: RouteIntent = serde_json::from_value(wire).unwrap();
    assert_eq!(decoded.observed_block(), Some(1000));
}

#[test]
fn two_hops_preserve_observed_provenance() {
    verify_hops(2);
}
#[test]
fn three_hops_preserve_observed_provenance() {
    verify_hops(3);
}
#[test]
fn four_hops_preserve_observed_provenance() {
    verify_hops(4);
}
#[test]
fn five_hops_preserve_observed_provenance() {
    verify_hops(5);
}

#[test]
fn old_wire_and_pending_transactions_do_not_gain_a_block() {
    let (graph, cycle, _) = fixture(2);
    let mut intent = cycle_to_intent(&graph, 1, 1000, &cycle).unwrap();
    let mut old = serde_json::to_value(&intent).unwrap();
    old.as_object_mut().unwrap().remove("observed_block_number");
    let decoded: RouteIntent = serde_json::from_value(old).unwrap();
    assert_eq!(decoded.observed_block(), None);
    intent.source_event = DetectionSource::PublicMempool;
    assert_eq!(intent.observed_block(), None);
    intent.source_event = DetectionSource::NewBlock;
    intent.observed_block_number = Some(0);
    assert_eq!(intent.observed_block(), None);
}

#[test]
fn broken_middle_is_rejected_even_when_outer_tokens_close() {
    let (mut graph, cycle, _) = fixture(3);
    graph.edges[1].token_in = addr(99);
    assert!(cycle_to_intent(&graph, 1, 1000, &cycle).is_none());
}

#[test]
fn pool_reuse_and_wrong_chain_are_rejected() {
    let (mut graph, cycle, _) = fixture(2);
    graph.edges[1].pool = graph.edges[0].pool;
    assert!(cycle_to_intent(&graph, 1, 1000, &cycle).is_none());
    let (mut graph, cycle, _) = fixture(2);
    graph.edges[1].chain_id = 137;
    assert!(cycle_to_intent(&graph, 1, 1000, &cycle).is_none());
    assert!(cycle_to_intent(&graph, 1, 0, &cycle).is_none());
}

#[test]
fn invalid_or_inconsistent_weights_do_not_manufacture_candidates() {
    let (graph, cycle, _) = fixture(2);
    for weight in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY, 0.0] {
        let mut bad = cycle.clone();
        bad.sum_log_weight = weight;
        assert!(cycle_to_intent(&graph, 1, 1000, &bad).is_none());
    }
    let mut positive = graph.clone();
    for edge in &mut positive.edges {
        edge.log_weight = Some(0.01);
    }
    assert!(cycle_to_intent(&positive, 1, 1000, &cycle).is_none());
    let mut bad = cycle;
    bad.hop_count = 5;
    assert!(cycle_to_intent(&graph, 1, 1000, &bad).is_none());
}

#[test]
fn partial_dex_binding_is_never_published() {
    let (graph, cycle, mut pools) = fixture(3);
    pools[2].dex_name = " ".to_owned();
    let lookup = pools.iter().map(|p| ((p.chain_id, p.address), p)).collect();
    let mut intent = cycle_to_intent(&graph, 1, 1000, &cycle).unwrap();
    assert_eq!(
        bind_dexes(&mut intent, &lookup),
        Err("missing_dex_identity")
    );
    assert!(intent.legs.iter().all(|leg| leg.dex_hint.is_none()));
}

#[test]
fn dex_binding_checks_pair_protocol_chain_and_fee() {
    let (graph, cycle, pools) = fixture(2);
    for mutation in 0..4 {
        let mut bad = pools.clone();
        match mutation {
            0 => bad[0].chain_id = 137,
            1 => bad[0].token0 = addr(99),
            2 => bad[0].protocol_type = ProtocolType::V3,
            _ => bad[0].fee_bps = Some(100),
        }
        let lookup = bad.iter().map(|p| ((1, p.address), p)).collect();
        let mut intent = cycle_to_intent(&graph, 1, 1000, &cycle).unwrap();
        assert_eq!(
            bind_dexes(&mut intent, &lookup),
            Err("pool_identity_mismatch")
        );
        assert!(intent.legs.iter().all(|leg| leg.dex_hint.is_none()));
    }
}
