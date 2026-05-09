//! Persistence for relays-client: INSERT executions, UPDATE opportunities,
//! UPSERT relay_scores. All in one transaction.

use anyhow::{Context, Result};
use shared_rs::contracts::{ExecutionResult, ExecutionStatus};
use sqlx::postgres::PgPool;
use sqlx::types::BigDecimal;
use std::str::FromStr;

pub async fn persist_execution(pool: &PgPool, r: &ExecutionResult, chain_id: i64) -> Result<()> {
    let status = status_str(&r.status);
    let gas_used: Option<BigDecimal> = r.gas_used_wei.as_deref().and_then(|s| BigDecimal::from_str(s).ok());
    let mut tx = pool.begin().await.context("begin")?;

    sqlx::query(
        r#"
        INSERT INTO executions (
            opportunity_id, relay_name, tx_hash, expected_profit_usd,
            actual_profit_usd, gas_used_wei, status, error_message, trace_id,
            submitted_at, confirmed_at, block_included
        ) VALUES ($1,$2,$3,NULL,$4,$5,$6,$7,$8,$9,$10,$11)
        ON CONFLICT (tx_hash) DO NOTHING
        "#,
    )
    .bind(r.opportunity_id)
    .bind(r.relay_used.as_deref().unwrap_or("unknown"))
    .bind(r.tx_hash.as_deref())
    .bind(r.actual_profit_usd)
    .bind(gas_used)
    .bind(status)
    .bind(r.error_message.as_deref())
    .bind(r.trace_id)
    .bind(r.submitted_at)
    .bind(r.submitted_at) // confirmed_at approximated to submitted_at for non-included
    .bind(r.block_included.map(|n| n as i64))
    .execute(&mut *tx)
    .await
    .context("insert execution")?;

    let next_status = match r.status {
        ExecutionStatus::Included => "executed",
        ExecutionStatus::Reverted => "failed",
        ExecutionStatus::Dropped | ExecutionStatus::Replaced => "failed",
        ExecutionStatus::NotSubmitted | ExecutionStatus::Submitted | ExecutionStatus::NotImplemented => "simulated", // leave as simulated (no on-chain action)
    };
    sqlx::query(
        r#"
        UPDATE opportunities
           SET status = $2, updated_at = NOW()
         WHERE id = $1 AND status IN ('simulated','executing')
        "#,
    )
    .bind(r.opportunity_id)
    .bind(next_status)
    .execute(&mut *tx)
    .await
    .context("update opp status")?;

    // UPSERT relay_scores window (window per hour rolling)
    if let Some(relay_name) = r.relay_used.as_deref() {
        let window_end = r.submitted_at;
        let window_start = r.submitted_at - chrono::Duration::hours(1);
        let (submitted, included, reverted, dropped) = match r.status {
            ExecutionStatus::Included => (1, 1, 0, 0),
            ExecutionStatus::Reverted => (1, 0, 1, 0),
            ExecutionStatus::Dropped | ExecutionStatus::Replaced => (1, 0, 0, 1),
            ExecutionStatus::Submitted => (1, 0, 0, 0),
            _ => (0, 0, 0, 0),
        };
        if submitted > 0 {
            sqlx::query(
                r#"
                INSERT INTO relay_scores (
                    relay_name, chain_id, window_start, window_end,
                    submitted, included, reverted, dropped,
                    inclusion_rate, score, enabled, updated_at
                ) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,
                    CASE WHEN $5 > 0 THEN ($6::numeric / $5) ELSE 0 END,
                    50.0, TRUE, NOW()
                )
                ON CONFLICT (relay_name, chain_id, window_end) DO UPDATE SET
                    submitted = relay_scores.submitted + EXCLUDED.submitted,
                    included  = relay_scores.included  + EXCLUDED.included,
                    reverted  = relay_scores.reverted  + EXCLUDED.reverted,
                    dropped   = relay_scores.dropped   + EXCLUDED.dropped,
                    inclusion_rate = CASE
                        WHEN (relay_scores.submitted + EXCLUDED.submitted) > 0
                        THEN (relay_scores.included + EXCLUDED.included)::numeric / (relay_scores.submitted + EXCLUDED.submitted)
                        ELSE 0 END,
                    updated_at = NOW()
                "#,
            )
            .bind(relay_name).bind(chain_id)
            .bind(window_start).bind(window_end)
            .bind(submitted).bind(included).bind(reverted).bind(dropped)
            .execute(&mut *tx)
            .await
            .context("upsert relay_scores")?;
        }
    }

    tx.commit().await.context("commit")?;
    Ok(())
}

fn status_str(s: &ExecutionStatus) -> &'static str {
    match s {
        ExecutionStatus::Submitted => "submitted",
        ExecutionStatus::Included => "included",
        ExecutionStatus::Reverted => "reverted",
        ExecutionStatus::Dropped => "dropped",
        ExecutionStatus::Replaced => "replaced",
        ExecutionStatus::NotImplemented => "not_implemented",
        ExecutionStatus::NotSubmitted => "not_submitted",
    }
}
