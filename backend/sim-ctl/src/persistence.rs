//! Persistence for sim-ctl: writes a row per simulation attempt and updates
//! the opportunity state. Transactional so both land or neither does.

use anyhow::{Context, Result};
use shared_rs::contracts::{SimulationResult, SimulatorKind};
use sqlx::postgres::PgPool;
use sqlx::types::BigDecimal;
use std::str::FromStr;

pub async fn insert_simulation(pool: &PgPool, r: &SimulationResult) -> Result<()> {
    let sim_str = simulator_str(&r.simulator);
    let gas_est: Option<BigDecimal> = r.gas_estimate_wei.as_deref()
        .and_then(|s| BigDecimal::from_str(s).ok());
    let gas_price: Option<BigDecimal> = r.gas_price_wei.as_deref()
        .and_then(|s| BigDecimal::from_str(s).ok());

    let mut tx = pool.begin().await.context("begin tx")?;

    sqlx::query(
        r#"
        INSERT INTO simulations (
            opportunity_id, simulator, gas_estimate_wei, gas_price_wei,
            slippage_pct, revert_risk_pct, simulated_profit_usd,
            passed, fail_reason, trace_id, simulated_at
        ) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11)
        "#,
    )
    .bind(r.opportunity_id)
    .bind(sim_str)
    .bind(gas_est)
    .bind(gas_price)
    .bind(r.slippage_pct)
    .bind(r.revert_risk_pct)
    .bind(r.simulated_profit_usd)
    .bind(r.passed)
    .bind(r.fail_reason.as_deref())
    .bind(r.trace_id)
    .bind(r.simulated_at)
    .execute(&mut *tx)
    .await
    .context("insert simulation")?;

    // Advance opportunity state. 'simulated' if passed, else 'rejected'.
    let next_status = if r.passed { "simulated" } else { "rejected" };
    let reject_reason = if r.passed { None } else { r.fail_reason.clone() };
    sqlx::query(
        r#"
        UPDATE opportunities
           SET status = $2,
               rejection_reason = COALESCE($3, rejection_reason),
               updated_at = NOW()
         WHERE id = $1
           AND status IN ('validated','scored','detected')
        "#,
    )
    .bind(r.opportunity_id)
    .bind(next_status)
    .bind(reject_reason)
    .execute(&mut *tx)
    .await
    .context("update opportunity status")?;

    tx.commit().await.context("commit tx")?;
    Ok(())
}

fn simulator_str(k: &SimulatorKind) -> &'static str {
    match k {
        SimulatorKind::Anvil => "anvil",
        SimulatorKind::Tenderly => "tenderly",
        SimulatorKind::Hardhat => "hardhat",
        SimulatorKind::Revm => "revm",
        SimulatorKind::NotImplemented => "not_implemented",
    }
}
