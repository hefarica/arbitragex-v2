//! Heartbeat Worker — periodic structured summary of pipeline state.
//!
//! Emits one `scanner.heartbeat` event every `period` seconds with:
//!   - Redis stream length (total + delta vs last tick) for `arbx:opps:detected`
//!   - PG opportunities inserted in the last `period` seconds
//!   - PG opportunities with `expected_profit_usd > 0` in the last `period`
//!
//! Why this exists: individual events (`candidate_enriched`, `gate_rejected`)
//! are sparse — sometimes a few per hour. Operators reading docker logs
//! between events cannot distinguish "idle" from "stuck". The heartbeat
//! gives a steady minute-by-minute pulse with deltas so the operator (and
//! external watchers) can spot pipeline degradation without grepping.
//!
//! Cost: 1 Redis XLEN + 1 PG count query per period (default 60s).

use crate::counters::counters;
use redis::aio::ConnectionManager;
use sqlx::{postgres::PgPool, Row};
use std::sync::atomic::Ordering;
use std::time::Duration;
use tokio::time::interval;
use tracing::{info, warn};

pub struct HeartbeatWorker {
    pub period: Duration,
}

impl HeartbeatWorker {
    pub fn new(period_secs: u64) -> Self {
        Self {
            period: Duration::from_secs(period_secs.max(1)),
        }
    }

    pub async fn run(&self, mut redis: ConnectionManager, db: Option<PgPool>) {
        let mut ticker = interval(self.period);
        // Skip the first immediate tick — gives downstream services time to
        // ingest and produces a meaningful delta on the first emit.
        ticker.tick().await;

        let mut last_redis_total: i64 = read_redis_xlen(&mut redis).await.unwrap_or_else(|e| {
            warn!(event = "heartbeat.redis_init_failed", error = %e);
            0
        });

        loop {
            ticker.tick().await;

            let now_redis_total = match read_redis_xlen(&mut redis).await {
                Ok(v) => v,
                Err(e) => {
                    warn!(event = "heartbeat.redis_failed", error = %e);
                    last_redis_total
                }
            };
            let redis_delta = now_redis_total.saturating_sub(last_redis_total);
            last_redis_total = now_redis_total;

            // PG counts are best-effort — when DB is absent or unhealthy we emit
            // -1 sentinels so the heartbeat never blocks pipeline observability
            // on persistence health.
            let (pg_inserted, pg_profit_pos): (i64, i64) = match db.as_ref() {
                Some(pool) => match read_pg_counts(pool, self.period.as_secs()).await {
                    Ok(pair) => pair,
                    Err(e) => {
                        warn!(event = "heartbeat.pg_failed", error = %e);
                        (-1, -1)
                    }
                },
                None => (-1, -1),
            };

            // Drain in-memory scanner counters via atomic swap → 0
            // so each heartbeat reports the delta for the just-elapsed period.
            // Lock-free; safe across all increment sites in scanner.rs.
            let c = counters();
            let pending = c.pending_received.swap(0, Ordering::Relaxed);
            let decoded = c.decoded_ok.swap(0, Ordering::Relaxed);
            let enriched_v2 = c.enriched_v2.swap(0, Ordering::Relaxed);
            let enriched_v3 = c.enriched_v3.swap(0, Ordering::Relaxed);
            let gate_token_na = c.gate_token_not_allowed.swap(0, Ordering::Relaxed);
            let gate_strat_dis = c.gate_strategy_disabled.swap(0, Ordering::Relaxed);
            let gate_no_cfg = c.gate_no_config.swap(0, Ordering::Relaxed);
            let gate_unk_price = c.gate_unknown_token_price.swap(0, Ordering::Relaxed);
            let gate_anom = c.gate_anomalous_math.swap(0, Ordering::Relaxed);
            let gate_other = c.gate_other_rejected.swap(0, Ordering::Relaxed);
            let passed = c.passed_all_gates.swap(0, Ordering::Relaxed);
            let db_ok = c.db_persisted.swap(0, Ordering::Relaxed);
            let db_err = c.db_errors.swap(0, Ordering::Relaxed);

            info!(
                event = "scanner.heartbeat",
                period_secs = self.period.as_secs(),
                redis_stream_total = now_redis_total,
                redis_stream_delta = redis_delta,
                pg_period_inserted = pg_inserted,
                pg_period_profit_pos = pg_profit_pos,
                // In-memory pipeline counters (delta this period).
                pending_received = pending,
                decoded_ok = decoded,
                enriched_v2 = enriched_v2,
                enriched_v3 = enriched_v3,
                gate_token_not_allowed = gate_token_na,
                gate_strategy_disabled = gate_strat_dis,
                gate_no_config = gate_no_cfg,
                gate_unknown_token_price = gate_unk_price,
                gate_anomalous_math = gate_anom,
                gate_other_rejected = gate_other,
                passed_all_gates = passed,
                db_persisted = db_ok,
                db_errors = db_err,
                "scanner pipeline heartbeat"
            );
        }
    }
}

async fn read_redis_xlen(redis: &mut ConnectionManager) -> redis::RedisResult<i64> {
    redis::cmd("XLEN")
        .arg("arbx:opps:detected")
        .query_async(redis)
        .await
}

async fn read_pg_counts(pool: &PgPool, period_secs: u64) -> Result<(i64, i64), sqlx::Error> {
    // period_secs is u64 from internal config — not user input, safe to inline.
    let sql = format!(
        "SELECT \
             COUNT(*)::int8 AS total, \
             COUNT(*) FILTER (WHERE expected_profit_usd > 0)::int8 AS profit_pos \
         FROM opportunities \
         WHERE detected_at > NOW() - INTERVAL '{} seconds'",
        period_secs
    );
    let row = sqlx::query(&sql).fetch_one(pool).await?;
    let total: i64 = row.try_get("total").unwrap_or(0);
    let profit_pos: i64 = row.try_get("profit_pos").unwrap_or(0);
    Ok((total, profit_pos))
}
