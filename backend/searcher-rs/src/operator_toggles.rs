//! Operator-toggle propagation (CHECKLIST item 5c, 2026-09-20).
//!
//! The frontend toggles operators via the math-engine HTTP API, but that
//! `ApiState` and the `Arc<OperatorRegistry>` embedded in searcher-rs are
//! SEPARATE processes/instances — `OperatorRegistry::dispatch` consults no
//! disabled-set. The math-engine API now persists its disabled-set to Redis
//! (`arbx:ops:disabled`, JSON array of operator IDs) on every toggle; this
//! module polls that key and exposes `is_disabled()` so every dispatch site in
//! the detection path honors the operator's toggles.
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

struct DisabledOps {
    set: RwLock<HashSet<u8>>,
}

static DISABLED: OnceLock<DisabledOps> = OnceLock::new();

fn global() -> &'static DisabledOps {
    DISABLED.get_or_init(|| DisabledOps {
        set: RwLock::new(HashSet::new()),
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

pub(crate) fn store(ids: &[u8]) {
    let mut set = global().set.write().expect("disabled-ops lock poisoned");
    let before = set.len();
    *set = ids.iter().copied().collect();
    if set.len() != before {
        info!(
            event = "ops_toggle.set_updated",
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
    loop {
        let raw: Result<Vec<u8>, _> = redis::cmd("GET")
            .arg(DISABLED_OPS_KEY)
            .query_async(&mut redis.clone())
            .await;
        match raw {
            Ok(bytes) if bytes.is_empty() => store(&[]),
            Ok(bytes) => match serde_json::from_slice::<Vec<u8>>(&bytes) {
                Ok(ids) => store(&ids),
                Err(e) => warn!(
                    event = "ops_toggle.payload_invalid",
                    error = %e,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn disabled_set_roundtrip() {
        store(&[3u8, 7u8]);
        assert!(is_disabled(3));
        assert!(is_disabled(7));
        assert!(!is_disabled(1));
        store(&[]);
        assert!(!is_disabled(3));
    }
}
