//! Redis Streams consumer-group reader for the token enricher.
//!
//! Reads from the `arbx:opps:detected` stream using consumer-group
//! `enricher` / consumer `enricher-1`.  Each message is expected to carry a
//! JSON payload under the field key `"json"` with at least:
//!
//! ```json
//! { "chain_id": 1, "token_in": "0x...", "token_out": "0x..." }
//! ```
//!
//! Successfully parsed messages are yielded as `(chain_id: u64, token_in:
//! String, token_out: String)` triples.  Poison messages (parse failure or
//! missing required fields) are logged as warnings and ACKed so they do not
//! block the PEL forever.
//!
//! ## Connection model
//!
//! Uses `redis::aio::MultiplexedConnection` (available with the `tokio-comp`
//! feature already in Cargo.toml; does NOT require the `connection-manager`
//! feature).
//!
//! ## Delivery semantics
//!
//! Both `read_one_batch` and `drain_pel` return a `(triples, entry_ids)` tuple.
//! The caller MUST:
//! 1. Process `triples` via `process_batch`.
//! 2. Call `ack_batch(entry_ids)` AFTER `process_batch` completes (success or error).
//!
//! This unified ACK-after-process contract applies to BOTH call sites.  If the
//! process crashes between either return and `ack_batch`, the entries remain in
//! the PEL and will be re-delivered on the next startup via `drain_pel`.  The
//! reconciliation tick (`find_unresolved_tokens`) acts as the
//! eventual-durability backstop — any token that slips through will be resolved
//! on the next reconciliation cycle.
//!
//! ## PEL / crash-recovery
//!
//! On startup the consumer drains its PEL (pending entries from a
//! previous crashed session) via `drain_pel` before switching to `">"`.
//! `drain_pel` uses delivery-id `"0"` which returns entries already
//! delivered-but-not-ACKed to this consumer.  It loops until the reply is
//! empty (exhausted), then returns.
//!
//! `drain_pel` returns `(triples, ids)` — the SAME contract as
//! `read_one_batch`.  The caller MUST call `ack_batch(ids).await` AFTER
//! `process_batch` completes (success or error).  If the process crashes
//! between `drain_pel` return and `ack_batch`, the PEL entries remain
//! unACKed; the next startup's `drain_pel` re-fetches and reprocesses them.
//! Poison messages are included in `ids` so they are ACKed by the caller
//! and never block the PEL forever.
//!
//! ## Multi-replica safety (Defect #6 — DEFERRED)
//!
//! TODO: derive CONSUMER from hostname for multi-replica safety.
//! If two replicas both use `"enricher-1"`, Redis treats them as one logical
//! consumer.  ACKs from either replica clear PEL entries for both — messages
//! are not duplicated, but PEL attribution is meaningless.  Fix when we need
//! horizontal scaling.

use anyhow::{Context, Result};
use redis::streams::{StreamReadOptions, StreamReadReply};
use redis::{AsyncCommands, Value};
use serde_json::Value as JsonValue;
use tracing::{debug, info, warn};

const STREAM: &str = "arbx:opps:detected";
const GROUP: &str = "enricher";
// TODO: derive CONSUMER from hostname for multi-replica safety.
const CONSUMER: &str = "enricher-1";
/// How long `XREADGROUP` blocks waiting for new messages (milliseconds).
const BLOCK_MS: usize = 2_000;
/// Maximum messages to fetch per `XREADGROUP` call.
const BATCH_SIZE: usize = 50;

/// Async Redis Streams consumer that wraps a multiplexed connection and
/// exposes a `read_one_batch()` loop-primitive and a `drain_pel()` startup method.
pub struct EnricherConsumer {
    conn: redis::aio::MultiplexedConnection,
}

impl EnricherConsumer {
    /// Connect to Redis and ensure the consumer group exists.
    ///
    /// `XGROUP CREATE … MKSTREAM` creates the stream and group atomically if
    /// neither exists yet.  `BUSYGROUP` (group already exists) is expected on
    /// normal restarts and is silently ignored.  Any other error is logged as
    /// a warning but does not prevent startup — the consumer will attempt to
    /// read anyway and fail explicitly there if the connection is broken.
    pub async fn connect(redis_url: &str) -> Result<Self> {
        let client = redis::Client::open(redis_url).context("redis::Client::open")?;
        let mut conn = client
            .get_multiplexed_async_connection()
            .await
            .context("get_multiplexed_async_connection")?;

        // XGROUP CREATE <stream> <group> $ MKSTREAM
        // "$" means "start delivering only new messages" — existing history is
        // left for other consumers or the reconciliation tick to process.
        match redis::cmd("XGROUP")
            .arg("CREATE")
            .arg(STREAM)
            .arg(GROUP)
            .arg("$")
            .arg("MKSTREAM")
            .query_async::<_, ()>(&mut conn)
            .await
        {
            Ok(_) => info!(
                event = "enricher.group_created",
                stream = STREAM,
                group = GROUP
            ),
            Err(ref e) if e.to_string().contains("BUSYGROUP") => {
                debug!(
                    event = "enricher.group_already_exists",
                    stream = STREAM,
                    group = GROUP
                )
            }
            Err(e) => warn!(
                event = "enricher.group_create_failed",
                stream = STREAM,
                group = GROUP,
                err = %e
            ),
        }

        Ok(Self { conn })
    }

    // -----------------------------------------------------------------------
    // Private: parse helper — no ACK
    // -----------------------------------------------------------------------

    /// Parse a `StreamReadReply` into triples + entry IDs, WITHOUT ACKing.
    ///
    /// Returns `(triples, ids)`:
    /// - `triples`: valid `(chain_id, token_in_lc, token_out_lc)` tuples.
    /// - `ids`: ALL entry IDs in the reply (both valid and poison), to be
    ///   passed to `ack_batch` by the caller AFTER processing.
    ///
    /// Poison entries (missing payload, bad JSON, zero chain_id, empty
    /// addresses) are logged and included in `ids` so they get ACKed and
    /// do not block the PEL forever.
    fn parse(&self, reply: StreamReadReply) -> (Vec<(u64, String, String)>, Vec<String>) {
        let mut out: Vec<(u64, String, String)> = Vec::new();
        let mut ids_to_ack: Vec<String> = Vec::new();

        for stream_key in &reply.keys {
            info!(
                event = "enricher.stream_batch_read",
                stream = %stream_key.key,
                count = stream_key.ids.len()
            );
            for entry in &stream_key.ids {
                // Collect ALL ids for caller-side ACK (including poison).
                ids_to_ack.push(entry.id.clone());

                // Extract the JSON payload from the `"json"` field — the key the
                // searcher publisher actually writes (publisher.rs: `XADD
                // arbx:opps:detected … json <payload>`) and the same key the
                // api-server paper-trade-archiver reads. Previously "payload",
                // which never matched → every opp logged missing_payload_field
                // (flooding Loki → promtail 429/unhealthy).
                // In redis 0.24, map values are `Value::Data(Vec<u8>)`.
                let payload_bytes: Option<&[u8]> = entry.map.get("json").and_then(|v| match v {
                    Value::Data(b) => Some(b.as_slice()),
                    _ => None,
                });

                let payload_bytes = match payload_bytes {
                    Some(b) => b,
                    None => {
                        warn!(
                            event = "enricher.missing_payload_field",
                            id = %entry.id
                        );
                        continue;
                    }
                };

                match serde_json::from_slice::<JsonValue>(payload_bytes) {
                    Ok(v) => {
                        let chain = v["chain_id"].as_u64().unwrap_or(0);
                        let ti = v["token_in"].as_str().unwrap_or("").to_lowercase();
                        let to = v["token_out"].as_str().unwrap_or("").to_lowercase();
                        if chain > 0 && !ti.is_empty() && !to.is_empty() {
                            out.push((chain, ti, to));
                        } else {
                            warn!(
                                event = "enricher.malformed_payload",
                                chain = chain,
                                ti = %ti,
                                to = %to,
                                id = %entry.id
                            );
                        }
                    }
                    Err(e) => {
                        warn!(
                            event = "enricher.json_parse_failed",
                            err = %e,
                            id = %entry.id
                        );
                    }
                }
            }
        }

        (out, ids_to_ack)
    }

    // -----------------------------------------------------------------------
    // Public: XACK helper
    // -----------------------------------------------------------------------

    /// ACK a list of entry IDs in the consumer group.
    ///
    /// Called by main.rs AFTER `process_batch` completes (success or error)
    /// to implement ACK-after-process semantics (see module-level doc).
    pub async fn ack_batch(&mut self, ids: Vec<String>) -> Result<()> {
        if ids.is_empty() {
            return Ok(());
        }
        let ack_result: redis::RedisResult<i64> = self.conn.xack(STREAM, GROUP, &ids).await;
        if let Err(e) = ack_result {
            warn!(event = "enricher.xack_error", err = %e);
        }
        Ok(())
    }

    // -----------------------------------------------------------------------
    // Public: drain PEL on startup (I1 — crash recovery)
    // -----------------------------------------------------------------------

    /// Drain the consumer's PEL (Pending Entry List) from a previous crashed session.
    ///
    /// Uses delivery-id `"0"` (not `">"`) so Redis returns entries that were
    /// already delivered to this consumer but never ACKed.  Loops until the
    /// reply is empty.
    ///
    /// MUST be called BEFORE the main loop — this is the application-level startup
    /// behaviour that makes crash recovery safe.  Without draining the PEL,
    /// unprocessed entries from the previous session are re-delivered only if
    /// the consumer explicitly requests them; they would otherwise sit in the
    /// PEL forever.
    ///
    /// ## ACK contract (unified with `read_one_batch`)
    ///
    /// Returns `(triples, ids)`.  The caller MUST call `ack_batch(ids).await`
    /// AFTER `process_batch` completes (success or error).  This gives the same
    /// crash-recovery guarantee as the live consumer arm: if the process crashes
    /// between `drain_pel` return and `ack_batch`, the PEL entries remain
    /// unACKed and will be re-fetched on the next startup.
    ///
    /// Poison messages are included in `ids` so the caller ACKs them and they
    /// never block the PEL indefinitely.
    pub async fn drain_pel(
        &mut self,
        batch_count: usize,
    ) -> Result<(Vec<(u64, String, String)>, Vec<String>)> {
        let mut all_triples: Vec<(u64, String, String)> = Vec::new();
        let mut all_ids: Vec<String> = Vec::new();

        // PEL paging cursor. Reading the PEL with a FIXED delivery-id "0" returns the
        // SAME pending entries on every call — XACK is deferred to the caller, so the
        // PEL never shrinks during this loop, `total_entries` is never 0, and the loop
        // spins forever (observed: ~1000 reads/sec, never reaching the live consumer).
        // Fix: page forward. Start at "0", then advance the cursor to the last entry id
        // seen; XREADGROUP history-reads return pending entries with id STRICTLY greater
        // than the cursor, so each read yields the next page. The pass terminates on an
        // empty or partial page. XACK still happens in the caller (crash-recovery intact).
        let mut cursor = "0".to_string();
        loop {
            let opts = StreamReadOptions::default()
                .group(GROUP, CONSUMER)
                .count(batch_count);
            // No .block() here — PEL (non-">") reads return immediately.

            let reply: StreamReadReply =
                match self.conn.xread_options(&[STREAM], &[cursor.as_str()], &opts).await {
                    Ok(r) => r,
                    Err(e) => {
                        warn!(event = "enricher.drain_pel_error", err = %e);
                        break;
                    }
                };

            // Empty reply = PEL fully paged.
            let total_entries: usize = reply.keys.iter().map(|k| k.ids.len()).sum();
            if total_entries == 0 {
                break;
            }

            // Advance the cursor to the last entry id in this page BEFORE consuming it,
            // so the next read returns the pending entries AFTER this page.
            let next_cursor = reply
                .keys
                .iter()
                .filter_map(|k| k.ids.last())
                .map(|e| e.id.clone())
                .next_back();

            let (triples, ids) = self.parse(reply);
            // Accumulate both — NO internal ACK here.
            // ACK is deferred to caller (main.rs) via ack_batch, AFTER process_batch.
            all_triples.extend(triples);
            all_ids.extend(ids);

            match next_cursor {
                // Full page → keep paging from the last id (strictly-greater semantics).
                Some(id) if total_entries >= batch_count => cursor = id,
                // Partial/last page (or no id) → PEL drained in a single pass.
                _ => break,
            }
        }

        info!(
            event = "enricher.pel_accumulated",
            triples = all_triples.len(),
            ids = all_ids.len()
        );
        Ok((all_triples, all_ids))
    }

    // -----------------------------------------------------------------------
    // Public: single blocking read (used in main loop select!)
    // -----------------------------------------------------------------------

    /// Issue one `XREADGROUP COUNT N BLOCK 2000` call and return parsed triples
    /// plus the entry IDs to ACK.
    ///
    /// **Caller contract (ACK-after-process)**: the caller MUST:
    /// 1. Call `process_batch` on the returned triples.
    /// 2. Call `consumer.ack_batch(ids).await` AFTER `process_batch` returns
    ///    (on both success and error paths).
    ///
    /// This ensures entries are not lost if the process crashes between read and
    /// process.  See module-level doc for the full delivery-semantics rationale.
    ///
    /// Returns an empty `Vec` and empty `Vec` on timeout (normal idle path).
    /// Returns `Err` on unrecoverable connection failure.
    pub async fn read_one_batch(&mut self) -> Result<(Vec<(u64, String, String)>, Vec<String>)> {
        let opts = StreamReadOptions::default()
            .group(GROUP, CONSUMER)
            .count(BATCH_SIZE)
            .block(BLOCK_MS);

        let reply: StreamReadReply = match self.conn.xread_options(&[STREAM], &[">"], &opts).await {
            Ok(r) => r,
            Err(e) => {
                warn!(event = "enricher.xreadgroup_error", err = %e);
                // Brief back-off before caller retries.
                tokio::time::sleep(std::time::Duration::from_millis(500)).await;
                return Err(anyhow::anyhow!("xreadgroup error: {e}"));
            }
        };

        if reply.keys.is_empty() {
            return Ok((vec![], vec![]));
        }

        Ok(self.parse(reply))
    }
}
