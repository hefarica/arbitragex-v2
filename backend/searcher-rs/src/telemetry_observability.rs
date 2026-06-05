//! Observer telemetry — Redis PUBLISH to `arbx:telemetry:observability`.
//!
//! Best-effort, OBSERVE-ONLY divergence signal. searcher-rs reacts to the real
//! node's `newHeads`; the shadow simulation pins to a block while it evaluates.
//! This channel reports each observed real-node head (number + hash) and flags
//! `divergent=true` when consecutive heads break chain continuity — a reorg
//! (same/lower number, different hash) or a parent-hash break — i.e. the chain
//! the shadow sim simulated against was reorganized. A dashboard subscribes and
//! alerts on `divergent`.
//!
//! NOTE (honest scope): searcher-rs tracks block NUMBER end-to-end and does not
//! expose a per-tick *shadow* block hash, so this compares CONSECUTIVE real-node
//! heads (node-side chain-continuity / reorg detection) rather than a literal
//! shadow-hash vs node-hash pair. That captures exactly the dangerous case — the
//! sim having simulated against a now-orphaned block.
//!
//! The event builder is pure (`-> serde_json::Value`) so it unit-tests without
//! Redis; [`publish`] is the thin async sink. A Redis error is logged, NEVER
//! fatal (observer-only; must never block detection).

use redis::aio::ConnectionManager;
use serde_json::{json, Value};
use tracing::warn;

/// The single channel this module publishes to. Distinct from
/// `arbx:opps:detected`, `arbx:route_discovery:telemetry`, and
/// `arbx:signals:convergence`.
pub const OBSERVABILITY_CHANNEL: &str = "arbx:telemetry:observability";

/// `telemetry.block_head` — one per observed real-node head.
///
/// `divergent=true` ⇒ a reorg / parent-hash break vs the previous head: the
/// shadow sim may have evaluated against an orphaned chain state. `kind` names
/// the divergence (`reorg_same_or_lower_number` | `parent_hash_break` | `none`).
#[allow(clippy::too_many_arguments)]
pub fn block_head_event(
    chain_id: u64,
    block_number: u64,
    block_hash: &str,
    parent_hash: &str,
    prev_hash: &str,
    prev_number: u64,
    divergent: bool,
    kind: &str,
    ts_ms: u64,
) -> Value {
    json!({
        "event": "telemetry.block_head",
        "chain_id": chain_id,
        "block_number": block_number,
        "block_hash": block_hash,
        "parent_hash": parent_hash,
        "prev_hash": prev_hash,
        "prev_number": prev_number,
        "divergent": divergent,
        "kind": kind,
        "ts_ms": ts_ms,
    })
}

/// Publish one payload to the observability channel. Best-effort: a Redis error
/// is logged, never fatal (observe-only module).
pub async fn publish(redis: &mut ConnectionManager, payload: &Value) {
    let json = match serde_json::to_string(payload) {
        Ok(s) => s,
        Err(e) => {
            warn!(event = "telemetry.observability_serialize_failed", error = %e);
            return;
        }
    };
    let res: redis::RedisResult<()> = redis::cmd("PUBLISH")
        .arg(OBSERVABILITY_CHANNEL)
        .arg(&json)
        .query_async(redis)
        .await;
    if let Err(e) = res {
        warn!(event = "telemetry.observability_publish_failed", error = %e);
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn divergent_event_has_canonical_shape() {
        let ev = block_head_event(
            1,
            21_000_000,
            "0xaaa",
            "0xbbb",
            "0xccc",
            20_999_999,
            true,
            "parent_hash_break",
            1_700_000_000_000,
        );
        assert_eq!(ev["event"], "telemetry.block_head");
        assert_eq!(ev["divergent"], true);
        assert_eq!(ev["kind"], "parent_hash_break");
        assert_eq!(ev["block_number"], 21_000_000u64);
    }
}
