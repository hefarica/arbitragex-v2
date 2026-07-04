//! Redis Streams consumer: arbx:opps:simulated -> execute -> persist -> XACK.
//!
//! Dead-letter policy (Pkg #5): a persist failure used to be swallowed
//! (`Ok(())` without XACK), leaving the entry forever in the PEL → an
//! unkillable poison message blocked the consumer. We now track per-message
//! delivery count in `arbx:opps:simulated:retries:<id>` (INCR + EXPIRE) and,
//! once it exceeds `ARBX_DLQ_MAX_RETRIES` (default 3), XADD the message to
//! `arbx:opps:simulated:dlq` with the failure reason and XACK the original so
//! the consumer progresses. No message is lost; none loops forever.

use crate::persistence::persist_execution;
use crate::submit_engine::SubmitEngine;
use anyhow::{Context, Result};
use redis::aio::ConnectionManager;
use redis::AsyncCommands;
use shared_rs::contracts::Opportunity;
use sqlx::postgres::PgPool;
use std::time::Duration;
use tracing::{debug, error, info, warn};

const STREAM: &str = "arbx:opps:simulated";
const GROUP: &str = "relays-client-g0";
const DLQ_STREAM: &str = "arbx:opps:simulated:dlq";
const EXECUTED_STREAM: &str = "arbx:opps:executed";
/// Per-message retry counter key template: `arbx:opps:simulated:retries:<id>`.
const RETRY_KEY_PREFIX: &str = "arbx:opps:simulated:retries:";
/// TTL on the retry counter so the key is reaped after the message stops
/// being redelivered (e.g. once it finally XACKs or is DLQ'd). Long enough to
/// cover the redelivery window, short enough to avoid unbounded key growth.
const RETRY_KEY_TTL_SECS: u64 = 86_400; // 24h

/// Default ceiling before a poison message is routed to the DLQ.
const DEFAULT_DLQ_MAX_RETRIES: u64 = 3;

/// Dead-letter / retry configuration. Resolved once at startup.
#[derive(Debug, Clone, Copy)]
pub struct DlqConfig {
    pub max_retries: u64,
}

impl DlqConfig {
    /// Resolve from process env. `ARBX_DLQ_MAX_RETRIES` (u64). Falls back to
    /// [`DEFAULT_DLQ_MAX_RETRIES`] when unset or unparseable — never panics.
    pub fn from_env() -> Self {
        let raw = std::env::var("ARBX_DLQ_MAX_RETRIES").ok();
        Self::from_raw(raw.as_deref())
    }

    /// Pure constructor (testable without touching process env).
    pub fn from_raw(max_retries: Option<&str>) -> Self {
        let max_retries = max_retries
            .and_then(|s| s.trim().parse::<u64>().ok())
            .filter(|&n| n > 0)
            .unwrap_or(DEFAULT_DLQ_MAX_RETRIES);
        Self { max_retries }
    }
}

/// Outcome of a persist-failure retry evaluation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DlqAction {
    /// Deliveries so far are within budget — leave unacked, retry on the next
    /// XAUTOCLAIM/XCLAIM cycle.
    Retry,
    /// Budget exhausted — route to the DLQ and XACK the original.
    DeadLetter,
}

/// Pure decision: retry while `deliveries <= max_retries`, else dead-letter.
///
/// `deliveries` is the count AFTER the just-failed attempt (i.e. includes it),
/// so with `max_retries = 3` the message is retried on deliveries 1, 2, 3 and
/// dead-lettered on the 4th failure. This gives exactly `max_retries` retries
/// before the DLQ.
pub fn decide_dlq(deliveries: u64, max_retries: u64) -> DlqAction {
    if deliveries <= max_retries {
        DlqAction::Retry
    } else {
        DlqAction::DeadLetter
    }
}

/// Build the field pairs for the DLQ XADD, in insertion order.
///
/// Pure (no Redis) so it can be unit-tested against the exact payload an
/// operator will see when inspecting the DLQ stream.
pub fn build_dlq_fields(
    original_id: &str,
    original_json: &str,
    error_reason: &str,
    deliveries: u64,
) -> Vec<(&'static str, String)> {
    vec![
        ("original_id", original_id.to_string()),
        ("payload", original_json.to_string()),
        ("error", error_reason.to_string()),
        ("retries", deliveries.to_string()),
        (
            "dlq_reason",
            "persist_execution_exhausted_retries".to_string(),
        ),
    ]
}

pub struct Consumer {
    pub redis: ConnectionManager,
    pub pool: PgPool,
    pub engine: SubmitEngine,
    pub consumer_name: String,
    pub dlq: DlqConfig,
}

impl Consumer {
    pub async fn run(mut self) -> Result<()> {
        self.ensure_group().await.ok();
        info!(event = "relays_consumer.started", stream = STREAM, group = GROUP,
              consumer = %self.consumer_name, dlq_max_retries = self.dlq.max_retries);
        loop {
            if let Err(e) = self.read_batch().await {
                error!(event = "relays_consumer.batch_err", error = %e);
                tokio::time::sleep(Duration::from_secs(1)).await;
            }
        }
    }

    async fn ensure_group(&mut self) -> Result<()> {
        let res: redis::RedisResult<()> = redis::cmd("XGROUP")
            .arg("CREATE")
            .arg(STREAM)
            .arg(GROUP)
            .arg("$")
            .arg("MKSTREAM")
            .query_async(&mut self.redis)
            .await;
        match res {
            Ok(_) => {
                info!(event = "relays_consumer.group_created");
                Ok(())
            }
            Err(e) if e.to_string().contains("BUSYGROUP") => Ok(()),
            Err(e) => Err(e.into()),
        }
    }

    async fn read_batch(&mut self) -> Result<()> {
        let reply: Option<Vec<redis::Value>> = redis::cmd("XREADGROUP")
            .arg("GROUP")
            .arg(GROUP)
            .arg(&self.consumer_name)
            .arg("COUNT")
            .arg(4)
            .arg("BLOCK")
            .arg(2000)
            .arg("STREAMS")
            .arg(STREAM)
            .arg(">")
            .query_async(&mut self.redis)
            .await?;
        let Some(reply) = reply else {
            return Ok(());
        };
        for stream_entry in reply {
            if let redis::Value::Bulk(v) = stream_entry {
                if v.len() != 2 {
                    continue;
                }
                let entries = match &v[1] {
                    redis::Value::Bulk(e) => e.clone(),
                    _ => continue,
                };
                for e in entries {
                    if let redis::Value::Bulk(parts) = e {
                        if parts.len() != 2 {
                            continue;
                        }
                        let id = match &parts[0] {
                            redis::Value::Data(s) => String::from_utf8_lossy(s).to_string(),
                            _ => continue,
                        };
                        let fields = match &parts[1] {
                            redis::Value::Bulk(f) => f.clone(),
                            _ => continue,
                        };
                        // A persist failure is now handled inside process_one
                        // (bounded retry → DLQ + XACK); only transport/parse
                        // errors propagate and trigger the backoff sleep above.
                        if let Err(e) = self.process_one(id, fields).await {
                            error!(event = "relays_consumer.process_err", error = %e);
                        }
                    }
                }
            }
        }
        Ok(())
    }

    async fn process_one(&mut self, id: String, kv: Vec<redis::Value>) -> Result<()> {
        let Some(json) = extract_field(&kv, "json") else {
            warn!(event = "relays_consumer.no_json", id = %id);
            let _: () = self
                .redis
                .xack::<_, _, &str, ()>(STREAM, GROUP, &[id.as_str()])
                .await?;
            return Ok(());
        };
        let opp: Opportunity = match serde_json::from_str(&json) {
            Ok(o) => o,
            Err(e) => {
                warn!(event = "relays_consumer.parse_err", id = %id, error = %e);
                let _: () = self
                    .redis
                    .xack::<_, _, &str, ()>(STREAM, GROUP, &[id.as_str()])
                    .await?;
                return Ok(());
            }
        };

        let result = self.engine.execute(&opp).await;
        debug!(event = "relays_consumer.executed", opp = %opp.id, status = ?result.status);

        if let Err(e) = persist_execution(&self.pool, &result, opp.chain_id as i64).await {
            // Bounded retry: count this failure and either leave the entry
            // unacked (retry on next claim) or route to the DLQ + XACK.
            self.handle_persist_failure(&id, &json, &e.to_string())
                .await?;
            return Ok(());
        }

        // Publish to arbx:opps:executed for recon (S6).
        let payload = serde_json::json!({
            "opportunity": opp,
            "execution": result,
        });
        let payload_s = serde_json::to_string(&payload).unwrap_or_default();
        let _: redis::RedisResult<String> = redis::cmd("XADD")
            .arg(EXECUTED_STREAM)
            .arg("MAXLEN")
            .arg("~")
            .arg(10_000)
            .arg("*")
            .arg("json")
            .arg(payload_s)
            .query_async(&mut self.redis)
            .await;

        let _: () = self
            .redis
            .xack::<_, _, &str, ()>(STREAM, GROUP, &[id.as_str()])
            .await
            .context("xack")?;
        Ok(())
    }

    /// Bounded-retry / dead-letter handling for a persist failure.
    ///
    /// Redis-native accounting: INCR `arbx:opps:simulated:retries:<id>` (the
    /// returned value IS the post-increment delivery count) and EXPIRE it so
    /// the key is reaped once redelivery stops. Then either:
    ///   * `Retry` — return Ok leaving the entry unacked (PEL retains it →
    ///     redelivered on the next XAUTOCLAIM/XCLAIM cycle), or
    ///   * `DeadLetter` — XADD a durable record to the DLQ stream and XACK the
    ///     original so the consumer progresses.
    ///
    /// Both DLQ Redis calls are best-effort: if XADD succeeds but XACK fails,
    /// the message will be redelivered and re-DLQ'd (idempotent evidence — the
    /// operator sees a duplicate DLQ entry, never a lost one). XADD failure
    /// propagates so the consumer doesn't silently drop a poison message.
    async fn handle_persist_failure(
        &mut self,
        id: &str,
        original_json: &str,
        error_reason: &str,
    ) -> Result<()> {
        let retry_key = format!("{RETRY_KEY_PREFIX}{id}");
        // INCR returns the value AFTER incrementing — deliveries includes the
        // attempt that just failed.
        let deliveries: u64 = redis::cmd("INCR")
            .arg(&retry_key)
            .query_async(&mut self.redis)
            .await
            .context("incr retry counter")?;
        // Refresh TTL on every failure so the key outlives the redelivery window.
        let _: redis::RedisResult<()> = redis::cmd("EXPIRE")
            .arg(&retry_key)
            .arg(RETRY_KEY_TTL_SECS)
            .query_async(&mut self.redis)
            .await;

        match decide_dlq(deliveries, self.dlq.max_retries) {
            DlqAction::Retry => {
                warn!(event = "relays_consumer.persist_retry", id = %id,
                      deliveries = deliveries, max_retries = self.dlq.max_retries,
                      error = %error_reason);
                Ok(())
            }
            DlqAction::DeadLetter => {
                error!(event = "relays_consumer.persist_dlq", id = %id,
                       deliveries = deliveries, max_retries = self.dlq.max_retries,
                       error = %error_reason);
                let fields = build_dlq_fields(id, original_json, error_reason, deliveries);
                let mut xadd = redis::cmd("XADD");
                xadd.arg(DLQ_STREAM)
                    .arg("MAXLEN")
                    .arg("~")
                    .arg(10_000)
                    .arg("*");
                for (k, v) in &fields {
                    xadd.arg(k).arg(v);
                }
                let _: String = xadd
                    .query_async(&mut self.redis)
                    .await
                    .context("xadd dlq")?;
                // Original is now safely in the DLQ — ack it so the consumer
                // moves on. A failure here surfaces (propagates) rather than
                // silently looping.
                let _: () = self
                    .redis
                    .xack::<_, _, &str, ()>(STREAM, GROUP, &[id])
                    .await
                    .context("xack after dlq")?;
                Ok(())
            }
        }
    }
}

fn extract_field(kv: &[redis::Value], name: &str) -> Option<String> {
    let mut i = 0;
    while i + 1 < kv.len() {
        let k = match &kv[i] {
            redis::Value::Data(s) => std::str::from_utf8(s).ok()?.to_string(),
            _ => return None,
        };
        let v = match &kv[i + 1] {
            redis::Value::Data(s) => String::from_utf8_lossy(s).to_string(),
            _ => return None,
        };
        if k == name {
            return Some(v);
        }
        i += 2;
    }
    None
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)] // test module — panics are acceptable
mod tests {
    use super::*;

    // ---- DlqConfig::from_raw ----

    #[test]
    fn dlq_config_unset_falls_back_to_default() {
        let cfg = DlqConfig::from_raw(None);
        assert_eq!(cfg.max_retries, DEFAULT_DLQ_MAX_RETRIES);
    }

    #[test]
    fn dlq_config_garbage_falls_back_to_default() {
        let cfg = DlqConfig::from_raw(Some("not-a-number"));
        assert_eq!(cfg.max_retries, DEFAULT_DLQ_MAX_RETRIES);
    }

    #[test]
    fn dlq_config_zero_falls_back_to_default() {
        // 0 retries is nonsensical (would DLQ on the first failure with no
        // retry budget) — treat it as misconfigured and use the default.
        let cfg = DlqConfig::from_raw(Some("0"));
        assert_eq!(cfg.max_retries, DEFAULT_DLQ_MAX_RETRIES);
    }

    #[test]
    fn dlq_config_valid_override_respected() {
        let cfg = DlqConfig::from_raw(Some("5"));
        assert_eq!(cfg.max_retries, 5);
    }

    #[test]
    fn dlq_config_whitespace_trimmed() {
        let cfg = DlqConfig::from_raw(Some("  7  "));
        assert_eq!(cfg.max_retries, 7);
    }

    // ---- decide_dlq: the retry-vs-DLQ boundary ----

    /// (a) A persist failure within MAX_RETRIES leaves the message unacked
    ///     (retry). With max_retries=3, deliveries 1, 2, 3 all retry.
    #[test]
    fn within_max_retries_yields_retry() {
        let max = 3;
        for deliveries in 1..=max {
            assert_eq!(
                decide_dlq(deliveries, max),
                DlqAction::Retry,
                "deliveries={deliveries} (<= max={max}) must retry, not DLQ"
            );
        }
    }

    /// (b) Reaching MAX_RETRIES routes to the DLQ + XACKs the original. With
    ///     max_retries=3, the 4th failure (deliveries=4) is the first DLQ.
    #[test]
    fn at_max_retries_plus_one_yields_deadletter() {
        let max = 3;
        assert_eq!(
            decide_dlq(max + 1, max),
            DlqAction::DeadLetter,
            "deliveries=max+1 must DLQ"
        );
        // and every subsequent delivery stays in the DLQ lane
        assert_eq!(decide_dlq(max + 5, max), DlqAction::DeadLetter);
    }

    #[test]
    fn decide_dlq_boundary_is_inclusive_on_retry_side() {
        // deliveries == max_retries is the LAST retry (inclusive), not the
        // first DLQ. This guarantees exactly `max_retries` retry attempts.
        assert_eq!(decide_dlq(3, 3), DlqAction::Retry);
        assert_eq!(decide_dlq(4, 3), DlqAction::DeadLetter);
    }

    #[test]
    fn decide_dlq_max_one_dlqs_on_second_failure() {
        // Edge: max_retries=1 → one retry (deliveries=1), DLQ on the 2nd.
        assert_eq!(decide_dlq(1, 1), DlqAction::Retry);
        assert_eq!(decide_dlq(2, 1), DlqAction::DeadLetter);
    }

    // ---- build_dlq_fields: the durable DLQ payload an operator inspects ----

    #[test]
    fn dlq_fields_carry_id_payload_error_retries_and_reason() {
        let fields = build_dlq_fields("1234-0", "{\"id\":\"a\"}", "db down", 4);
        let map: std::collections::HashMap<&str, String> = fields.into_iter().collect();
        assert_eq!(map.get("original_id").unwrap(), "1234-0");
        assert_eq!(map.get("payload").unwrap(), "{\"id\":\"a\"}");
        assert_eq!(map.get("error").unwrap(), "db down");
        assert_eq!(map.get("retries").unwrap(), "4");
        assert_eq!(
            map.get("dlq_reason").unwrap(),
            "persist_execution_exhausted_retries"
        );
    }

    #[test]
    fn dlq_fields_count_is_stable() {
        // Operators / recon tooling parse positionally; lock the field count.
        let fields = build_dlq_fields("x", "{}", "e", 9);
        assert_eq!(fields.len(), 5, "DLQ record must have exactly 5 fields");
    }

    /// Sanity: the retry-key prefix + id form the documented counter key. This
    /// is the key an operator would `redis-cli GET`/`TTL` to inspect a looping
    /// message mid-retry — lock the shape so docs stay honest.
    #[test]
    fn retry_key_shape_is_documented() {
        assert_eq!(
            format!("{RETRY_KEY_PREFIX}{}", "abc-0"),
            "arbx:opps:simulated:retries:abc-0"
        );
    }
}
