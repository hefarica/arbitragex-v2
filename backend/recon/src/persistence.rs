//! Persistence for recon: INSERT recon_reports + risk_events + UPDATE opportunity.

use anyhow::{Context, Result};
use shared_rs::contracts::ReconReport;
use sqlx::postgres::PgPool;
use sqlx::types::BigDecimal;
use std::str::FromStr;

fn opt_bd(s: &Option<String>) -> Option<BigDecimal> {
    s.as_deref().and_then(|v| BigDecimal::from_str(v).ok())
}

pub async fn persist_recon_report(pool: &PgPool, r: &ReconReport) -> Result<()> {
    let mut tx = pool.begin().await.context("begin")?;
    sqlx::query(
        r#"
        INSERT INTO recon_reports (
            opportunity_id, execution_id, tx_hash, chain_id,
            expected_amount_out_wei, actual_amount_out_wei,
            variance_native_units, variance_pct,
            expected_profit_usd, actual_profit_usd, pnl_source,
            actual_gas_used_wei, actual_gas_price_wei,
            fail_reason, trace_id, created_at
        ) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14,$15,$16)
        "#,
    )
    .bind(r.opportunity_id)
    .bind(r.execution_id)
    .bind(r.tx_hash.as_deref())
    .bind(r.chain_id as i32)
    .bind(opt_bd(&r.expected_amount_out_wei))
    .bind(opt_bd(&r.actual_amount_out_wei))
    .bind(opt_bd(&r.variance_native_units))
    .bind(r.variance_pct)
    .bind(r.expected_profit_usd)
    .bind(r.actual_profit_usd)
    .bind(&r.pnl_source)
    .bind(opt_bd(&r.actual_gas_used_wei))
    .bind(opt_bd(&r.actual_gas_price_wei))
    .bind(r.fail_reason.as_deref())
    .bind(r.trace_id)
    .bind(r.created_at)
    .execute(&mut *tx)
    .await
    .context("insert recon_report")?;

    sqlx::query(
        r#"
        UPDATE opportunities
           SET status = 'reconciled', updated_at = NOW()
         WHERE id = $1 AND status IN ('executed','simulated','executing')
        "#,
    )
    .bind(r.opportunity_id)
    .execute(&mut *tx)
    .await
    .context("update opp status")?;

    tx.commit().await.context("commit")?;
    Ok(())
}

pub async fn insert_risk_event(
    pool: &PgPool,
    event_type: &str,
    severity: &str,
    payload: serde_json::Value,
    trace_id: uuid::Uuid,
    opp_id: Option<uuid::Uuid>,
) -> Result<()> {
    sqlx::query(
        r#"
        INSERT INTO risk_events (event_type, severity, source_service, payload, trace_id, opportunity_id)
        VALUES ($1,$2,'recon',$3::jsonb,$4,$5)
        "#,
    )
    .bind(event_type)
    .bind(severity)
    .bind(payload)
    .bind(trace_id)
    .bind(opp_id)
    .execute(pool)
    .await
    .context("insert risk_event")?;
    Ok(())
}
