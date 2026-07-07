//! Aggregator task — periodic rollup of `executions` into
//! `strategy_scores` + `relay_scores` + anomaly check.
//!
//! Triggered every `cfg.aggregator_interval_seconds`.
//! Idempotent via UNIQUE (strategy_kind, chain_id, window_end) in strategy_scores.
//!
//! Sprint BE-3.5: after each `strategy_scores` upsert the aggregator publishes
//! a scoring update to Redis pub/sub channel
//! `arbx:scoring:updates:<chain_id>` so the prioritization-spine can adjust
//! `p_fail` in real time without waiting for the next SQL cache TTL (60 s).
//!
//! Publish is fire-and-forget: failure is logged at WARN but never stops the
//! aggregation cycle (R8 fail-honest — the spine always has the SQL fallback).

use crate::anomaly;
use chrono::{DateTime, Duration as ChronoDuration, Utc};
use redis::aio::ConnectionManager;
use redis::AsyncCommands;
use shared_rs::config::ReconCfg;
use shared_rs::killswitch::KillSwitchClient;
use sqlx::postgres::PgPool;
use std::time::Duration;
use tracing::{debug, info, warn};

/// Aggregate row returned from the upsert query so we can publish updates.
struct StrategyScoreRow {
    strategy_kind: String,
    chain_id: i32,
    success_rate: f64,
    revert_rate: f64,
    sample_count: i64,
}

/// Run the periodic aggregation loop.
///
/// `redis` is an optional connection manager for pub/sub publishing (BE-3.5).
/// Pass `None` to disable publishing; the loop is backward-compatible.
pub async fn run_periodic(
    pool: PgPool,
    cfg: ReconCfg,
    kill: KillSwitchClient,
    redis: Option<ConnectionManager>,
) {
    let mut ticker = tokio::time::interval(Duration::from_secs(cfg.aggregator_interval_seconds));
    ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    info!(
        event = "aggregator.started",
        interval_s = cfg.aggregator_interval_seconds
    );
    // Connection manager is cheaply cloneable (Arc-backed).
    let mut redis_opt = redis;
    loop {
        ticker.tick().await;
        let window_end = Utc::now();
        let window_start =
            window_end - ChronoDuration::hours(cfg.strategy_score_window_hours as i64);

        match aggregate_strategy_scores(&pool, window_start, window_end).await {
            Ok(scores) => {
                if let Some(ref mut redis_conn) = redis_opt {
                    for row in &scores {
                        publish_scoring_update(redis_conn, row, window_start, window_end).await;
                    }
                }
            }
            Err(e) => warn!(event = "aggregator.strategy_err", error = %e),
        }
        if let Err(e) = aggregate_relay_scores(&pool, window_start, window_end).await {
            warn!(event = "aggregator.relay_err", error = %e);
        }
        match anomaly::check_and_react(&pool, &cfg, &kill).await {
            Ok(n) => debug!(event = "aggregator.anomaly_pass", anomalies = n),
            Err(e) => warn!(event = "aggregator.anomaly_err", error = %e),
        }
    }
}

/// Publish a scoring update to Redis pub/sub.
/// Fire-and-forget: errors are logged at WARN and never propagated.
async fn publish_scoring_update(
    redis: &mut ConnectionManager,
    row: &StrategyScoreRow,
    window_start: DateTime<Utc>,
    window_end: DateTime<Utc>,
) {
    let channel = format!("arbx:scoring:updates:{}", row.chain_id);
    let payload = serde_json::json!({
        "strategy_kind": row.strategy_kind,
        "chain_id": row.chain_id,
        "success_rate": row.success_rate,
        "revert_rate": row.revert_rate,
        "sample_count": row.sample_count,
        "window_start": window_start.to_rfc3339(),
        "window_end": window_end.to_rfc3339(),
    })
    .to_string();

    let result: Result<i64, redis::RedisError> = redis.publish(&channel, &payload).await;
    match result {
        Ok(receivers) => debug!(
            event = "aggregator.scoring_published",
            channel = %channel,
            receivers,
            strategy_kind = %row.strategy_kind,
        ),
        Err(e) => warn!(
            event = "aggregator.scoring_publish_err",
            channel = %channel,
            strategy_kind = %row.strategy_kind,
            error = %e,
            "Redis pub/sub publish failed — spine will use SQL fallback"
        ),
    }
}

async fn aggregate_strategy_scores(
    pool: &PgPool,
    window_start: DateTime<Utc>,
    window_end: DateTime<Utc>,
) -> anyhow::Result<Vec<StrategyScoreRow>> {
    // RETURNING lets us publish exactly what was upserted without a second query.
    let rows = sqlx::query_as::<_, (String, i32, f64, f64, i64)>(
        r#"
        INSERT INTO strategy_scores
            (strategy_kind, chain_id, window_start, window_end,
             sample_count, success_rate, avg_profit_usd, revert_rate, score, enabled, updated_at)
        SELECT
            o.strategy_kind,
            o.chain_id,
            $1, $2,
            COUNT(*)::INT,
            COALESCE(SUM(CASE WHEN e.status='included' THEN 1 ELSE 0 END)::NUMERIC / NULLIF(COUNT(*),0), 0) AS success_rate,
            COALESCE(AVG(e.actual_profit_usd), 0) AS avg_profit,
            COALESCE(SUM(CASE WHEN e.status='reverted' THEN 1 ELSE 0 END)::NUMERIC / NULLIF(COUNT(*),0), 0) AS revert_rate,
            -- Simple adaptive score: success_rate * (1 - revert_rate) * 100 clamped
            GREATEST(0, LEAST(100,
                COALESCE(SUM(CASE WHEN e.status='included' THEN 1 ELSE 0 END)::NUMERIC / NULLIF(COUNT(*),0), 0) * 100.0
                - COALESCE(SUM(CASE WHEN e.status='reverted' THEN 1 ELSE 0 END)::NUMERIC / NULLIF(COUNT(*),0), 0) * 50.0
            )) AS score,
            TRUE,
            NOW()
        FROM executions e
        JOIN opportunities o ON o.id = e.opportunity_id
        WHERE e.submitted_at >= $1 AND e.submitted_at < $2
        GROUP BY o.strategy_kind, o.chain_id
        ON CONFLICT (strategy_kind, chain_id, window_end) DO UPDATE SET
            sample_count = EXCLUDED.sample_count,
            success_rate = EXCLUDED.success_rate,
            avg_profit_usd = EXCLUDED.avg_profit_usd,
            revert_rate = EXCLUDED.revert_rate,
            score = EXCLUDED.score,
            updated_at = NOW()
        RETURNING strategy_kind, chain_id,
                  success_rate::DOUBLE PRECISION,
                  revert_rate::DOUBLE PRECISION,
                  sample_count::BIGINT
        "#,
    )
    .bind(window_start)
    .bind(window_end)
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(
            |(strategy_kind, chain_id, success_rate, revert_rate, sample_count)| StrategyScoreRow {
                strategy_kind,
                chain_id,
                success_rate,
                revert_rate,
                sample_count,
            },
        )
        .collect())
}

async fn aggregate_relay_scores(
    pool: &PgPool,
    window_start: DateTime<Utc>,
    window_end: DateTime<Utc>,
) -> anyhow::Result<()> {
    sqlx::query(
        r#"
        INSERT INTO relay_scores
            (relay_name, chain_id, window_start, window_end,
             submitted, included, reverted, dropped,
             inclusion_rate, avg_latency_ms, score, enabled, updated_at)
        SELECT
            e.relay_name,
            o.chain_id,
            $1, $2,
            COUNT(*)::INT,
            SUM(CASE WHEN e.status='included' THEN 1 ELSE 0 END)::INT,
            SUM(CASE WHEN e.status='reverted' THEN 1 ELSE 0 END)::INT,
            SUM(CASE WHEN e.status IN ('dropped','replaced') THEN 1 ELSE 0 END)::INT,
            COALESCE(SUM(CASE WHEN e.status='included' THEN 1 ELSE 0 END)::NUMERIC / NULLIF(COUNT(*),0), 0),
            0,
            GREATEST(0, LEAST(100,
                COALESCE(SUM(CASE WHEN e.status='included' THEN 1 ELSE 0 END)::NUMERIC / NULLIF(COUNT(*),0), 0) * 100.0
            )),
            TRUE,
            NOW()
        FROM executions e
        JOIN opportunities o ON o.id = e.opportunity_id
        WHERE e.submitted_at >= $1 AND e.submitted_at < $2 AND e.relay_name IS NOT NULL
        GROUP BY e.relay_name, o.chain_id
        ON CONFLICT (relay_name, chain_id, window_end) DO UPDATE SET
            submitted = EXCLUDED.submitted,
            included = EXCLUDED.included,
            reverted = EXCLUDED.reverted,
            dropped = EXCLUDED.dropped,
            inclusion_rate = EXCLUDED.inclusion_rate,
            score = EXCLUDED.score,
            updated_at = NOW()
        "#,
    )
    .bind(window_start)
    .bind(window_end)
    .execute(pool)
    .await?;
    Ok(())
}
