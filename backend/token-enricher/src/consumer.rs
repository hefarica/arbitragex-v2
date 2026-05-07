//! Redis Streams consumer-group reader for the token enricher.
//!
//! Reads from the `arbx:opps:detected` stream using consumer-group
//! `enricher` / consumer `enricher-1`.  Each message is expected to carry a
//! JSON payload under the field key `"payload"` with at least:
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
//! ## PEL / crash-recovery (TODO)
//!
//! On startup the consumer should first drain its PEL (pending entries from a
//! previous crashed session) before switching to `">"`.  This is not
//! implemented in Task 7.
//!
//! TODO: drain PEL on startup with `XREADGROUP GROUP enricher enricher-1
//! COUNT N STREAMS arbx:opps:detected 0` before entering the `">"` loop.
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
/// exposes a `run()` loop.
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
        let client = redis::Client::open(redis_url)
            .context("redis::Client::open")?;
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
            Ok(_) => info!(event = "enricher.group_created", stream = STREAM, group = GROUP),
            Err(ref e) if e.to_string().contains("BUSYGROUP") => {
                debug!(event = "enricher.group_already_exists", stream = STREAM, group = GROUP)
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

    /// Run the consumer loop, calling `on_batch` for each successfully parsed
    /// batch of `(chain_id, token_in_lc, token_out_lc)` triples.
    ///
    /// The loop:
    /// 1. Issues `XREADGROUP GROUP enricher enricher-1 COUNT 50 BLOCK 2000 STREAMS arbx:opps:detected >`.
    /// 2. Parses each stream entry's `"payload"` field as JSON.
    /// 3. Calls `on_batch` with the parsed triples.
    /// 4. ACKs all messages in the batch (including poison messages — see
    ///    doc above: we log and skip rather than re-delivering indefinitely).
    ///
    /// Returns `Err` only on unrecoverable connection failures.
    pub async fn run<F, Fut>(&mut self, mut on_batch: F) -> Result<()>
    where
        F: FnMut(Vec<(u64, String, String)>) -> Fut,
        Fut: std::future::Future<Output = Result<()>>,
    {
        let opts = StreamReadOptions::default()
            .group(GROUP, CONSUMER)
            .count(BATCH_SIZE)
            .block(BLOCK_MS);

        loop {
            let reply: StreamReadReply = match self
                .conn
                .xread_options(&[STREAM], &[">"], &opts)
                .await
            {
                Ok(r) => r,
                Err(e) => {
                    warn!(event = "enricher.xreadgroup_error", err = %e);
                    // Brief back-off before retrying to avoid tight error loops.
                    tokio::time::sleep(std::time::Duration::from_millis(500)).await;
                    continue;
                }
            };

            // `reply.keys` is empty when BLOCK timeout fires with no messages.
            // This is the normal idle path — loop again immediately.
            if reply.keys.is_empty() {
                continue;
            }

            let mut out: Vec<(u64, String, String)> = Vec::new();
            let mut ids_to_ack: Vec<String> = Vec::new();

            for stream_key in &reply.keys {
                info!(
                    event = "enricher.stream_batch_read",
                    stream = %stream_key.key,
                    count = stream_key.ids.len()
                );
                for entry in &stream_key.ids {
                    ids_to_ack.push(entry.id.clone());

                    // Extract the JSON payload from the `"payload"` field.
                    // In redis 0.24, map values are `Value::Data(Vec<u8>)`.
                    let payload_bytes: Option<&[u8]> = entry
                        .map
                        .get("payload")
                        .and_then(|v| match v {
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
                            let ti = v["token_in"]
                                .as_str()
                                .unwrap_or("")
                                .to_lowercase();
                            let to = v["token_out"]
                                .as_str()
                                .unwrap_or("")
                                .to_lowercase();
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

            // Deliver parsed batch to the caller before ACKing.  If on_batch
            // returns Err we still ACK — the error is reported to the caller
            // but we do NOT re-deliver (repeated delivery of a structurally
            // valid message that the handler can't process would be an
            // infinite retry loop).
            if !out.is_empty() {
                if let Err(e) = on_batch(out).await {
                    warn!(event = "enricher.on_batch_error", err = %e);
                }
            }

            // ACK all entries (including poison ones) to drain the PEL.
            if !ids_to_ack.is_empty() {
                let ack_result: redis::RedisResult<i64> = self
                    .conn
                    .xack(STREAM, GROUP, &ids_to_ack)
                    .await;
                if let Err(e) = ack_result {
                    warn!(event = "enricher.xack_error", err = %e);
                }
            }
        }
    }
}
