//! Constructed unit graphs, never production quotes or execution evidence.
//! A negative marginal cycle is NOT proof of positive sized net profit.
use super::graph_builder::TokenGraph;
use super::multi_hop_search::{
    find_profitable_cycles, find_profitable_cycles_with_limits, SearchLimits,
};
use super::types::{RouteDirection, RouteEdge};
use crate::route_intent::ProtocolType;
use ethers::types::Address;
use std::collections::HashSet;

fn ring(hops: usize, weight: Option<f64>) -> TokenGraph {
    let mut graph = TokenGraph::default();
    for i in 0..hops {
        let token_in = Address::from_low_u64_be(i as u64 + 1);
        let token_out = Address::from_low_u64_be(((i + 1) % hops) as u64 + 1);
        graph.adjacency.entry(token_in).or_default().push(i);
        graph.edges.push(RouteEdge {
            chain_id: 1,
            pool: Address::from_low_u64_be(100 + i as u64),
            token_in,
            token_out,
            token0: token_in.min(token_out),
            token1: token_in.max(token_out),
            protocol: ProtocolType::V2,
            fee_bps: Some(30),
            liquidity_hint: Some(1_000_000.0),
            log_weight: weight,
            freshness_ts: 1_700_000_000,
            blk: 20_000_000,
            hot_token: false,
            direction: if token_in < token_out {
                RouteDirection::ZeroForOne
            } else {
                RouteDirection::OneForZero
            },
        });
    }
    graph
}

fn assert_ring(hops: usize) {
    let graph = ring(hops, Some(-0.01));
    let result = find_profitable_cycles(&graph, 2, 5, 100);
    // Execution discovery retains every possible funding-asset rotation.
    assert_eq!(result.cycles.len(), hops);
    assert!(!result.capped);
    for cycle in &result.cycles {
        assert_eq!(cycle.hop_count, hops);
        assert_eq!(cycle.edges.len(), hops);
        let pools: HashSet<_> = cycle.edges.iter().map(|&i| graph.edges[i].pool).collect();
        assert_eq!(pools.len(), hops);
        for i in 0..hops {
            let current = &graph.edges[cycle.edges[i]];
            let next = &graph.edges[cycle.edges[(i + 1) % hops]];
            assert_eq!(current.token_out, next.token_in);
            assert_eq!(current.chain_id, 1);
            assert_eq!(current.blk, 20_000_000);
        }
        assert!((cycle.sum_log_weight + 0.01 * hops as f64).abs() < 1e-12);
    }
    let mut dense = graph.clone();
    dense.build_dense(1);
    assert_eq!(
        find_profitable_cycles(&dense, 2, 5, 100).cycles,
        result.cycles
    );
}

#[test]
fn two_hops_preserve_edges_pools_and_funding_rotations() {
    assert_ring(2);
}

#[test]
fn three_hops_preserve_edges_pools_and_funding_rotations() {
    assert_ring(3);
}

#[test]
fn four_hops_preserve_edges_pools_and_funding_rotations() {
    assert_ring(4);
}

#[test]
fn five_hops_preserve_edges_pools_and_funding_rotations() {
    assert_ring(5);
}

#[test]
fn all_four_lengths_reject_lossy_missing_and_invalid_weights() {
    for hops in 2..=5 {
        for weight in [Some(0.01), None, Some(f64::NAN), Some(f64::INFINITY)] {
            let mut graph = ring(hops, Some(-0.01));
            // A positive leg dominates all other negative marginal weights.
            graph.edges[1].log_weight = weight.map(|w| w * 100.0);
            let result = find_profitable_cycles(&graph, 2, 5, 100);
            assert!(result.cycles.is_empty());
            assert!(!result.capped);
            if weight.is_none() {
                assert!(result.v3_skipped > 0);
            } else if weight.is_some_and(|w| !w.is_finite()) {
                assert!(result.invalid_weights > 0);
            }
        }
    }
}

#[test]
fn all_four_lengths_reject_repeated_pools_without_fabricating_legs() {
    for hops in 2..=5 {
        let mut graph = ring(hops, Some(-0.01));
        graph.edges[1].pool = graph.edges[0].pool;
        assert!(find_profitable_cycles(&graph, 2, 5, 100).cycles.is_empty());
    }
}

#[test]
fn masks_and_analysis_dedup_preserve_the_requested_hop_length() {
    for hops in 2..=5 {
        let graph = ring(hops, Some(-0.01));
        let selected = find_profitable_cycles_with_limits(
            &graph,
            2,
            7,
            100,
            SearchLimits {
                hop_mask: 1 << (hops - 2),
                deduplicate_rotations: true,
                ..Default::default()
            },
        );
        assert_eq!(selected.cycles.len(), 1);
        assert_eq!(selected.cycles[0].hop_count, hops);
        assert_eq!(selected.duplicate_cycles, hops - 1);
        let excluded = find_profitable_cycles_with_limits(
            &graph,
            2,
            7,
            100,
            SearchLimits {
                hop_mask: 1 << 5,
                ..Default::default()
            },
        );
        assert!(excluded.cycles.is_empty());
    }
}

#[test]
fn work_exhaustion_is_visible_for_each_hop_length() {
    for hops in 2..=5 {
        let graph = ring(hops, Some(-0.01));
        let result = find_profitable_cycles_with_limits(
            &graph,
            2,
            5,
            100,
            SearchLimits {
                max_edge_visits: 1,
                ..Default::default()
            },
        );
        assert_eq!(result.edge_visits, 1);
        assert!(result.cycles.is_empty());
        assert!(result.capped && result.work_limited);
    }
}
