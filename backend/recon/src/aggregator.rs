//! Aggregator task — periodic rollup of `executions` into
//! `strategy_scores` + `relay_scores` + anomaly check.
//!
//! Triggered every `cfg.aggregator_interval_seconds`.
//! Idempotent via UNIQUE (strategy_kind, chain_id, window_end) in strategy_scores.

use crate::anomaly;
use chrono::{DateTime, Duration as ChronoDuration, Utc};
use shared_rs::config::ReconCfg;
use shared_rs::killswitch::KillSwitchClient;
use sqlx::postgres::PgPool;
use std::time::Duration;
use tracing::{debug, info, warn};

pub async fn run_periodic(
    pool: PgPool,
    cfg: ReconCfg,
    kill: KillSwitchClient,
) {
    let mut ticker = tokio::time::interval(Duration::from_secs(cfg.aggregator_interval_seconds));
    ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    info!(event = "aggregator.started", interval_s = cfg.aggregator_interval_seconds);
    loop {
        ticker.tick().await;
        let window_end = Utc::now();
        let window_start = window_end - ChronoDuration::hours(cfg.strategy_score_window_hours as i64);

        if let Err(e) = aggregate_strategy_scores(&pool, window_start, window_end).await {
            warn!(event = "aggregator.strategy_err", error = %e);
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

async fn aggregate_strategy_scores(
    pool: &PgPool,
    window_start: DateTime<Utc>,
    window_end: DateTime<Utc>,
) -> anyhow::Result<()> {
    sqlx::query(
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
        "#,
    )
    .bind(window_start)
    .bind(window_end)
    .execute(pool)
    .await?;
    Ok(())
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
