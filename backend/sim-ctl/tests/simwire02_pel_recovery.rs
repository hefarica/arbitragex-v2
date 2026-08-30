//! SIMWIRE-E2E-02 — PEL crash/retry recovery (live Redis).
//!
//! Validates the Redis Streams contract that makes consumer.rs's P2 recovery
//! real, at the protocol level with the EXACT commands the consumer uses:
//!
//!   1. A consumer (`crashed-c`) XREADGROUP `>` an entry, then dies before
//!      XACK — exactly what consumer.rs does on a transient infra error
//!      (`simulate_b2c` Err → `return Ok(())` with no XACK).
//!   2. The entry lands in the group PEL (XPENDING range form — the same
//!      command `observe_pel` uses for the pending_count/oldest_pending_ms
//!      gauges).
//!   3. PROOF OF THE DEFECT being fixed: a fresh `XREADGROUP ... >` NEVER
//!      redelivers the pending entry — without XAUTOCLAIM it is stuck
//!      forever (SIMWIRE-01's missing "retry").
//!   4. XAUTOCLAIM with MIN-IDLE + COUNT (the exact invocation shape of
//!      `recover_stale_pending`) reclaims the entry WITH its fields.
//!   5. After processing + XACK, the PEL is empty again.
//!
//! CI: runs for real under the integration-tests job (live redis:7). Outside
//! CI: connects to REDIS_URL (default redis://localhost:6379); if Redis is
//! unreachable the test SKIPS loudly — it never fabricates a pass.

use redis::AsyncCommands;

const GROUP: &str = "simwire02-g";

#[tokio::test]
async fn crashed_entry_is_reclaimed_via_xautoclaim_and_acked() {
    let redis_url = std::env::var("REDIS_URL").unwrap_or_else(|_| "redis://localhost:6379".into());
    let client = match redis::Client::open(redis_url.clone()) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("SKIP: bad REDIS_URL {redis_url:?}: {e}");
            return;
        }
    };
    let mut conn = match client.get_connection_manager().await {
        Ok(c) => c,
        Err(e) => {
            eprintln!("SKIP: Redis not reachable at {redis_url}: {e}");
            return;
        }
    };

    // Unique stream per run — never touches the production streams.
    let stream = format!("arbx:test:simwire02:pel:{}", uuid::Uuid::new_v4().simple());

    // Fresh group reading from 0 so the entry we add is deliverable.
    let _: () = redis::cmd("XGROUP")
        .arg("CREATE")
        .arg(&stream)
        .arg(GROUP)
        .arg("0")
        .arg("MKSTREAM")
        .query_async(&mut conn)
        .await
        .expect("XGROUP CREATE");

    let payload = r#"{"opportunity_id":"simwire02-fixture","chain_id":1}"#;
    let entry_id: String = redis::cmd("XADD")
        .arg(&stream)
        .arg("*")
        .arg("json")
        .arg(payload)
        .query_async(&mut conn)
        .await
        .expect("XADD");

    // 1) "Crashed" consumer reads the entry and never acks it.
    let delivered: Option<Vec<redis::Value>> = redis::cmd("XREADGROUP")
        .arg("GROUP")
        .arg(GROUP)
        .arg("crashed-c")
        .arg("COUNT")
        .arg(8)
        .arg("STREAMS")
        .arg(&stream)
        .arg(">")
        .query_async(&mut conn)
        .await
        .expect("XREADGROUP (crashed consumer)");
    let delivered = delivered.expect("entry must be delivered to the crashed consumer");
    assert_eq!(delivered.len(), 1, "one stream reply expected");

    // 2) The entry is in the PEL, owned by the crashed consumer.
    let pel: Vec<redis::Value> = redis::cmd("XPENDING")
        .arg(&stream)
        .arg(GROUP)
        .arg("-")
        .arg("+")
        .arg(64u64)
        .query_async(&mut conn)
        .await
        .expect("XPENDING range form");
    assert_eq!(pel.len(), 1, "exactly one pending entry expected");
    if let redis::Value::Bulk(f) = &pel[0] {
        assert!(
            f.len() >= 3,
            "XPENDING entry: [id, consumer, idle-ms, deliveries]"
        );
        assert_eq!(
            val_to_string(&f[0]),
            Some(entry_id.clone()),
            "PEL entry id must match the delivered entry"
        );
        assert_eq!(
            val_to_string(&f[1]).as_deref(),
            Some("crashed-c"),
            "PEL ownership must stay with the crashed consumer"
        );
    } else {
        panic!("XPENDING reply entry is not a Bulk array: {:?}", pel[0]);
    }

    // 3) The defect being fixed: `>` alone NEVER redelivers a pending entry.
    let not_redelivered: Option<Vec<redis::Value>> = redis::cmd("XREADGROUP")
        .arg("GROUP")
        .arg(GROUP)
        .arg("recoverer")
        .arg("COUNT")
        .arg(8)
        .arg("BLOCK")
        .arg(200u64)
        .arg("STREAMS")
        .arg(&stream)
        .arg(">")
        .query_async(&mut conn)
        .await
        .expect("XREADGROUP (fresh consumer)");
    assert!(
        not_redelivered.is_none(),
        "P2 fact: a pending entry is invisible to `>` — without XAUTOCLAIM it is stuck forever"
    );

    // 4) XAUTOCLAIM (same shape as consumer::recover_stale_pending) with
    //    MIN-IDLE 0 reclaims the entry WITH its fields.
    let reply: Vec<redis::Value> = redis::cmd("XAUTOCLAIM")
        .arg(&stream)
        .arg(GROUP)
        .arg("recoverer")
        .arg(0u64)
        .arg("0-0")
        .arg("COUNT")
        .arg(8)
        .query_async(&mut conn)
        .await
        .expect("XAUTOCLAIM");
    assert!(
        !reply.is_empty(),
        "XAUTOCLAIM reply: [cursor, entries, ...]"
    );
    assert_eq!(
        val_to_string(&reply[0]).as_deref(),
        Some("0-0"),
        "scan completed; no more entries"
    );
    let reclaimed = parse_entries(reply.get(1).unwrap_or(&redis::Value::Nil));
    assert_eq!(reclaimed.len(), 1, "the crashed entry must be reclaimed");
    assert_eq!(reclaimed[0].0, entry_id, "reclaimed id must match");
    assert_eq!(
        reclaimed[0].1.get("json"),
        Some(&payload.to_string()),
        "reclaimed entry must carry its original fields"
    );

    // 5) Processed (here: field extraction succeeded) → ACK → PEL empty.
    let acked: i64 = conn
        .xack(&stream, GROUP, &[reclaimed[0].0.as_str()])
        .await
        .expect("XACK");
    assert_eq!(acked, 1, "exactly one entry acked");
    let pel_after: Vec<redis::Value> = redis::cmd("XPENDING")
        .arg(&stream)
        .arg(GROUP)
        .arg("-")
        .arg("+")
        .arg(64u64)
        .query_async(&mut conn)
        .await
        .expect("XPENDING after ack");
    assert!(pel_after.is_empty(), "PEL must be empty after recovery+ack");

    // Cleanup.
    let _: redis::RedisResult<()> = redis::cmd("XGROUP")
        .arg("DESTROY")
        .arg(&stream)
        .arg(GROUP)
        .query_async(&mut conn)
        .await;
    let _: redis::RedisResult<()> = conn.del(&stream).await;
}

/// Minimal `redis::Value::Data` → String.
fn val_to_string(v: &redis::Value) -> Option<String> {
    match v {
        redis::Value::Data(s) => Some(String::from_utf8_lossy(s).to_string()),
        _ => None,
    }
}

/// Minimal mirror of consumer::parse_entry_array for field extraction.
fn parse_entries(
    entries: &redis::Value,
) -> Vec<(String, std::collections::HashMap<String, String>)> {
    let mut out = Vec::new();
    if let redis::Value::Bulk(list) = entries {
        for e in list {
            if let redis::Value::Bulk(parts) = e {
                if parts.len() != 2 {
                    continue;
                }
                let (Some(id), Some(redis::Value::Bulk(fields))) =
                    (val_to_string(&parts[0]), Some(&parts[1]))
                else {
                    continue;
                };
                let mut map = std::collections::HashMap::new();
                let mut i = 0;
                while i + 1 < fields.len() {
                    if let (Some(k), Some(v)) =
                        (val_to_string(&fields[i]), val_to_string(&fields[i + 1]))
                    {
                        map.insert(k, v);
                    }
                    i += 2;
                }
                out.push((id, map));
            }
        }
    }
    out
}
