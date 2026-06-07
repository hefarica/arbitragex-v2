//! Telemetry for route discovery — Redis PUBLISH to `arbx:route_discovery:telemetry`.
//!
//! Every payload carries `algorithm` (Phase 1 = `dfs_bounded`) so consumers can
//! tell which finder produced a route. This is the **only** Redis write path of
//! the route-discovery subsystem — it PUBLISHes to one channel and never touches
//! `arbx:opps:detected` (that stream belongs to the native orchestrator).
//!
//! The event builders are pure (`-> serde_json::Value`) so they unit-test
//! without Redis; [`publish`] is the thin async sink.

use crate::route_discovery::canonicalizer::protocol_token;
use crate::route_discovery::graph_builder::RejectedEdge;
use crate::route_discovery::types::RouteCandidate;
use redis::aio::ConnectionManager;
use serde_json::{json, Value};
use tracing::warn;

/// The single channel this subsystem publishes to. Deliberately distinct from
/// `arbx:opps:detected` and `arbx:cartridge:telemetry`.
pub const ROUTE_DISCOVERY_TELEMETRY_CHANNEL: &str = "arbx:route_discovery:telemetry";

/// `route_discovery.tick` — one per loop iteration.
///
/// `routes_capped` is the honest truncation signal: `true` ⇒ the route cap
/// stopped enumeration early, so `routes_found` is incomplete and
/// `routes_dropped_for_cap` is only a lower bound (R8 fail-honest).
#[allow(clippy::too_many_arguments)]
pub fn tick_event(
    chain_id: u64,
    algorithm: &str,
    pools_total: usize,
    edges_built: usize,
    edges_rejected: usize,
    routes_found: usize,
    routes_dispatched: usize,
    telemetry_emitted: usize,
    routes_dropped_for_cap: usize,
    routes_capped: bool,
    pools_truncated: bool,
    latency_ms: u64,
    mode: &str,
) -> Value {
    json!({
        "event": "route_discovery.tick",
        "chain_id": chain_id,
        "algorithm": algorithm,
        "pools_total": pools_total,
        "edges_built": edges_built,
        "edges_rejected": edges_rejected,
        "routes_found": routes_found,
        "routes_dispatched": routes_dispatched,
        "telemetry_emitted": telemetry_emitted,
        "routes_dropped_for_cap": routes_dropped_for_cap,
        "routes_capped": routes_capped,
        // R8: a parallel pool was dropped by the per-pair branching cap this tick —
        // the route set is not provably exhaustive over the full pool universe.
        "pools_truncated": pools_truncated,
        "latency_ms": latency_ms,
        "mode": mode,
    })
}

/// `route_discovery.route_candidate` — topology of one discovered route.
pub fn route_candidate_event(chain_id: u64, algorithm: &str, c: &RouteCandidate) -> Value {
    json!({
        "event": "route_discovery.route_candidate",
        "chain_id": chain_id,
        "algorithm": algorithm,
        "route_hash": c.route_hash,
        "route_kind": c.route_kind.as_str(),
        "hops": c.hops,
        "tokens": c.tokens.iter().map(|a| format!("{a:#x}")).collect::<Vec<_>>(),
        "pools": c.pools.iter().map(|a| format!("{a:#x}")).collect::<Vec<_>>(),
        "protocols": c.protocols.iter().map(|p| protocol_token(*p)).collect::<Vec<_>>(),
        "fee_tiers": c.fee_tiers,
        "directions": c.directions.iter().map(|d| d.as_str()).collect::<Vec<_>>(),
    })
}

/// `route_discovery.strategy_applicability` — which strategies apply to a route.
pub fn strategy_applicability_event(chain_id: u64, c: &RouteCandidate) -> Value {
    json!({
        "event": "route_discovery.strategy_applicability",
        "chain_id": chain_id,
        "route_hash": c.route_hash,
        "route_kind": c.route_kind.as_str(),
        "applicable_strategies": c.applicable_strategies.iter().map(|s| s.as_str()).collect::<Vec<_>>(),
        "rejected_strategies": c.rejected_strategies.iter()
            .map(|r| json!({"strategy": r.strategy, "reason": r.reason}))
            .collect::<Vec<_>>(),
    })
}

/// `route_discovery.rejected` — a pool dropped from the graph, with the reason.
pub fn rejected_event(chain_id: u64, r: &RejectedEdge) -> Value {
    json!({
        "event": "route_discovery.rejected",
        "chain_id": chain_id,
        "pool": format!("{:#x}", r.pool),
        "reason": r.reason,
    })
}

/// `route_intent.emitted` — a route dispatched to a cartridge (shadow only).
/// Used by the dispatcher (later commit); lives here so the schema is one place.
pub fn route_intent_emitted_event(
    chain_id: u64,
    route_hash: &str,
    strategy: &str,
    mode: &str,
    dispatch_deferred: Option<&str>,
) -> Value {
    json!({
        "event": "route_intent.emitted",
        "chain_id": chain_id,
        "route_hash": route_hash,
        "strategy": strategy,
        "mode": mode,
        "dispatch_deferred": dispatch_deferred,
    })
}

/// Publish one payload to the route-discovery channel. Best-effort: a Redis
/// error is logged, never fatal (observe-only subsystem).
pub async fn publish(redis: &mut ConnectionManager, payload: &Value) {
    let json = match serde_json::to_string(payload) {
        Ok(s) => s,
        Err(e) => {
            warn!(event = "route_discovery.telemetry_serialize_failed", error = %e);
            return;
        }
    };
    let res: redis::RedisResult<()> = redis::cmd("PUBLISH")
        .arg(ROUTE_DISCOVERY_TELEMETRY_CHANNEL)
        .arg(&json)
        .query_async(redis)
        .await;
    if let Err(e) = res {
        warn!(event = "route_discovery.telemetry_publish_failed", error = %e);
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use crate::route_discovery::types::{RejectedStrategy, RouteDirection, RouteKind};
    use crate::route_intent::ProtocolType;
    use crate::strategy_label::StrategyLabel;
    use ethers::types::Address;

    fn sample_candidate() -> RouteCandidate {
        RouteCandidate {
            chain_id: 1,
            route_hash: "0xabc".to_string(),
            route_kind: RouteKind::V2V3,
            tokens: vec![Address::from_low_u64_be(1), Address::from_low_u64_be(2)],
            pools: vec![
                Address::from_low_u64_be(0x10),
                Address::from_low_u64_be(0x20),
            ],
            protocols: vec![ProtocolType::V2, ProtocolType::V3],
            fee_tiers: vec![Some(30), Some(500)],
            directions: vec![RouteDirection::ZeroForOne, RouteDirection::OneForZero],
            hops: 2,
            applicable_strategies: vec![StrategyLabel::DexArbV2V3, StrategyLabel::FlashloanArb],
            rejected_strategies: vec![RejectedStrategy {
                strategy: "liquidation".to_string(),
                reason: "strategy_not_route_based".to_string(),
            }],
            mode: "shadow".to_string(),
        }
    }

    #[test]
    fn channel_is_dedicated_and_not_opps() {
        assert_eq!(
            ROUTE_DISCOVERY_TELEMETRY_CHANNEL,
            "arbx:route_discovery:telemetry"
        );
        assert!(!ROUTE_DISCOVERY_TELEMETRY_CHANNEL.contains("opps"));
    }

    #[test]
    fn tick_event_carries_algorithm_and_counts() {
        let e = tick_event(
            1,
            "dfs_bounded",
            26,
            50,
            4,
            12,
            3,
            8,
            1,
            true,
            true,
            42,
            "shadow",
        );
        assert_eq!(e["event"], "route_discovery.tick");
        assert_eq!(e["algorithm"], "dfs_bounded");
        assert_eq!(e["pools_total"], 26);
        assert_eq!(e["routes_found"], 12);
        assert_eq!(e["routes_dispatched"], 3);
        assert_eq!(e["routes_dropped_for_cap"], 1);
        assert_eq!(e["routes_capped"], true);
        assert_eq!(e["pools_truncated"], true);
        assert_eq!(e["latency_ms"], 42);
        assert_eq!(e["mode"], "shadow");
    }

    #[test]
    fn route_candidate_event_serializes_topology() {
        let c = sample_candidate();
        let e = route_candidate_event(1, "dfs_bounded", &c);
        assert_eq!(e["event"], "route_discovery.route_candidate");
        assert_eq!(e["route_hash"], "0xabc");
        assert_eq!(e["route_kind"], "v2v3");
        assert_eq!(e["hops"], 2);
        assert_eq!(e["protocols"][0], "v2");
        assert_eq!(e["protocols"][1], "v3");
        assert_eq!(e["directions"][0], "0to1");
        assert_eq!(
            e["tokens"][0],
            format!("{:#x}", Address::from_low_u64_be(1))
        );
        assert_eq!(e["fee_tiers"][0], 30);
        assert_eq!(e["fee_tiers"][1], 500);
    }

    #[test]
    fn applicability_event_lists_labels_and_rejections() {
        let c = sample_candidate();
        let e = strategy_applicability_event(1, &c);
        assert_eq!(e["event"], "route_discovery.strategy_applicability");
        let appl = e["applicable_strategies"].as_array().unwrap();
        assert!(appl.iter().any(|v| v == "dex_arb_v2v3"));
        assert!(appl.iter().any(|v| v == "flashloan_arb"));
        assert_eq!(e["rejected_strategies"][0]["strategy"], "liquidation");
        assert_eq!(
            e["rejected_strategies"][0]["reason"],
            "strategy_not_route_based"
        );
    }

    #[test]
    fn rejected_event_carries_pool_and_reason() {
        let r = RejectedEdge {
            pool: Address::from_low_u64_be(0xbeef),
            reason: "missing_reserves".to_string(),
        };
        let e = rejected_event(1, &r);
        assert_eq!(e["event"], "route_discovery.rejected");
        assert_eq!(e["reason"], "missing_reserves");
        assert_eq!(
            e["pool"],
            format!("{:#x}", Address::from_low_u64_be(0xbeef))
        );
    }

    #[test]
    fn route_intent_emitted_event_shape() {
        let e = route_intent_emitted_event(1, "0xabc", "dex_arb_v2v3", "shadow", None);
        assert_eq!(e["event"], "route_intent.emitted");
        assert_eq!(e["strategy"], "dex_arb_v2v3");
        assert_eq!(e["mode"], "shadow");
        assert!(e["dispatch_deferred"].is_null());

        let e2 = route_intent_emitted_event(
            1,
            "0xdef",
            "triangular_arb",
            "shadow",
            Some("needs_triangular_adapter"),
        );
        assert_eq!(e2["dispatch_deferred"], "needs_triangular_adapter");
    }
}
