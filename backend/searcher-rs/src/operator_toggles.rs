//! Operator-toggle propagation (CHECKLIST item 5c, 2026-09-20).
//!
//! The frontend toggles operators via the math-engine HTTP API, but that
//! `ApiState` and the `Arc<OperatorRegistry>` embedded in searcher-rs are
//! SEPARATE processes/instances — `OperatorRegistry::dispatch` consults no
//! disabled-set. The math-engine API now persists its disabled-set to Redis
//! (`arbx:ops:disabled`) on every toggle; this module polls that key and
//! exposes `is_disabled()` so every dispatch site in the detection path
//! honors the operator's toggles.
//!
//! WO-FE10 (F13, 2026-09-21): the payload is versioned by the writer —
//! `{"revision": N, "request_id": "...", "disabled": [ids]}` with a monotonic
//! N from `INCR arbx:ops:disabled:rev`. This loop records the revision it
//! APPLIED and acks it back to `arbx:ops:disabled:applied`, closing the loop
//! requested→persisted→applied→reflected (observable via math-engine
//! `GET /api/operators/toggles/status`). Legacy bare-array payloads (pre-FE10
//! writer) are still honored with revision 0 = unknown.
//!
//! Semantics: a disabled operator is SKIPPED — same as "cannot compute"
//! (R8 fail-honest: no fabricated values, the operator simply does not run).
//! Fail-open on read errors (an unreachable Redis must not blind detection):
//! the last known set stays in effect.

use std::collections::HashSet;
use std::sync::{OnceLock, RwLock};
use std::time::Duration;

use tracing::{info, warn};

/// Redis key written by math-engine's toggle endpoint. ONE canonical format.
pub const DISABLED_OPS_KEY: &str = "arbx:ops:disabled";
/// Searcher-side ack: revision of the disabled-set this process last applied.
pub const DISABLED_OPS_APPLIED_KEY: &str = "arbx:ops:disabled:applied";

/// Versioned payload written by math-engine since WO-FE10.
#[derive(Debug, Clone, serde::Deserialize)]
pub struct DisabledSetPayload {
    pub revision: u64,
    pub request_id: Option<String>,
    pub disabled: Vec<u8>,
}

struct DisabledOps {
    set: RwLock<HashSet<u8>>,
    applied: RwLock<AppliedRevision>,
}

#[derive(Debug, Clone, Default)]
struct AppliedRevision {
    revision: u64,
    request_id: Option<String>,
}

static DISABLED: OnceLock<DisabledOps> = OnceLock::new();

fn global() -> &'static DisabledOps {
    DISABLED.get_or_init(|| DisabledOps {
        set: RwLock::new(HashSet::new()),
        applied: RwLock::new(AppliedRevision::default()),
    })
}

/// Whether an operator ID is soft-disabled via the math-engine toggle.
/// Process-global: every dispatch site in searcher-rs sees the same answer.
pub fn is_disabled(id: u8) -> bool {
    global()
        .set
        .read()
        .expect("disabled-ops lock poisoned")
        .contains(&id)
}

/// Revision of the disabled-set this process last applied (0 = never / legacy
/// payload without revision). Exposed for the reflected side of F13.
pub fn applied_revision() -> u64 {
    global()
        .applied
        .read()
        .expect("disabled-ops lock poisoned")
        .revision
}

/// request_id of the last applied toggle action, when the writer sent one.
pub fn applied_request_id() -> Option<String> {
    global()
        .applied
        .read()
        .expect("disabled-ops lock poisoned")
        .request_id
        .clone()
}

fn store(payload: &DisabledSetPayload) {
    {
        let mut applied = global().applied.write().expect("disabled-ops lock poisoned");
        applied.revision = payload.revision;
        applied.request_id = payload.request_id.clone();
    }
    let mut set = global().set.write().expect("disabled-ops lock poisoned");
    let before = set.len();
    *set = payload.disabled.iter().copied().collect();
    if set.len() != before {
        info!(
            event = "ops_toggle.set_updated",
            revision = payload.revision,
            disabled_count = set.len(),
            "operator disabled-set refreshed from Redis"
        );
    }
}

/// Boot-load + poll loop. Spawned once per process (scanner::run). Toggles are
/// human-rate actions, so a 5s poll is eventual-consistent by design — no
/// pub/sub fan-out complexity on the detection hot path.
pub async fn run_poll_loop(
    redis: redis::aio::ConnectionManager,
    cancel: tokio_util::sync::CancellationToken,
) {
    // ConnectionManager is cheap to clone (Arc inside); used for the ack
    // write-back without disturbing the poll connection.
    let ack_conn = redis.clone();
    loop {
        let raw: Result<Option<String>, _> = redis::cmd("GET")
            .arg(DISABLED_OPS_KEY)
            .query_async(&mut redis.clone())
            .await;
        match raw {
            // Absent key = nobody toggled anything: empty set, revision 0.
            Ok(None) => store(&DisabledSetPayload {
                revision: 0,
                request_id: None,
                disabled: Vec::new(),
            }),
            Ok(Some(raw)) if raw.is_empty() => store(&DisabledSetPayload {
                revision: 0,
                request_id: None,
                disabled: Vec::new(),
            }),
            Ok(Some(raw)) => match parse_payload(raw.as_bytes()) {
                Some(payload) => {
                    let revision = payload.revision;
                    let request_id = payload.request_id.clone();
                    store(&payload);
                    // F13 ack (best-effort): tell the channel WHICH revision
                    // this process applied. 0 (legacy payload) is skipped —
                    // acking an unknown revision would be noise, not signal.
                    if revision > 0 {
                        let ack: Result<(), _> = redis::cmd("SET")
                            .arg(DISABLED_OPS_APPLIED_KEY)
                            .arg(revision.to_string())
                            .query_async(&mut ack_conn.clone())
                            .await;
                        if let Err(e) = ack {
                            warn!(
                                event = "ops_toggle.ack_failed",
                                error = %e,
                                "applied-revision ack not written (poll continues)"
                            );
                        }
                    }
                    info!(
                        event = "ops_toggle.applied",
                        revision = revision,
                        request_id = request_id.as_deref().unwrap_or(""),
                        "disabled-set applied"
                    );
                }
                None => warn!(
                    event = "ops_toggle.payload_invalid",
                    "keeping last known disabled-set (fail-open)"
                ),
            },
            Err(e) => warn!(
                event = "ops_toggle.redis_read_failed",
                error = %e,
                "keeping last known disabled-set (fail-open)"
            ),
        }
        if tokio::select! {
            _ = cancel.cancelled() => true,
            _ = tokio::time::sleep(Duration::from_secs(5)) => false,
        } {
            info!(event = "ops_toggle.poll_stopped", "cancelled");
            return;
        }
    }
}

/// Accepts the versioned object (FE10+) and the legacy bare array. Returns
/// None on malformed payloads so the caller can keep the last known set.
fn parse_payload(bytes: &[u8]) -> Option<DisabledSetPayload> {
    let value: serde_json::Value = serde_json::from_slice(bytes).ok()?;
    if value.is_array() {
        let disabled = serde_json::from_value::<Vec<u8>>(value).ok()?;
        return Some(DisabledSetPayload {
            revision: 0,
            request_id: None,
            disabled,
        });
    }
    serde_json::from_value(value).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn disabled_set_roundtrip() {
        store(&DisabledSetPayload {
            revision: 1,
            request_id: Some("req-1".into()),
            disabled: vec![3u8, 7u8],
        });
        assert!(is_disabled(3));
        assert!(is_disabled(7));
        assert!(!is_disabled(1));
        assert_eq!(applied_revision(), 1);
        assert_eq!(applied_request_id().as_deref(), Some("req-1"));
        store(&DisabledSetPayload {
            revision: 2,
            request_id: None,
            disabled: Vec::new(),
        });
        assert!(!is_disabled(3));
        assert_eq!(applied_revision(), 2);
        assert_eq!(applied_request_id(), None);
    }

    #[test]
    fn parses_versioned_and_legacy_payloads() {
        let versioned = parse_payload(br#"{"revision":7,"request_id":"abc","disabled":[1,2]}"#)
            .expect("versioned payload must parse");
        assert_eq!(versioned.revision, 7);
        assert_eq!(versioned.request_id.as_deref(), Some("abc"));
        assert_eq!(versioned.disabled, vec![1, 2]);

        let legacy = parse_payload(br#"[4,5]"#).expect("legacy array must parse");
        assert_eq!(legacy.revision, 0);
        assert_eq!(legacy.request_id, None);
        assert_eq!(legacy.disabled, vec![4, 5]);

        assert!(parse_payload(br#"{"unexpected":true}"#).is_none());
        assert!(parse_payload(b"not json").is_none());
    }
}
