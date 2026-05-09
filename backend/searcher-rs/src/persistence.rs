//! Postgres persistence for opportunities.
//!
//! Uses plain `sqlx::query` (not `query!`) to avoid the offline-data requirement
//! for compilation. Types are hand-bound.

use anyhow::Context;
use shared_rs::contracts::{Opportunity, StrategyKind};
use sqlx::{postgres::PgPool, types::BigDecimal};
use std::str::FromStr;

fn strategy_kind_str(k: &StrategyKind) -> &'static str {
    match k {
        StrategyKind::DexArb        => "dex_arb",
        StrategyKind::Triangular    => "triangular",
        StrategyKind::Backrun       => "backrun",
        StrategyKind::Liquidation   => "liquidation",
        StrategyKind::FlashloanArb  => "flashloan_arb",
    }
}

pub async fn insert_opportunity(pool: &PgPool, o: &Opportunity) -> anyhow::Result<()> {
    let amount_in_wei = BigDecimal::from_str(&o.amount_in_wei)
        .context("amount_in_wei to BigDecimal")?;
    // GAP-2 fix (2026-05-05): persist `rejection_reason` so the operator
    // can audit WHY each opp was rejected (TokenNotAllowed vs UnknownTokenPrice
    // vs AnomalousMath vs LowLiquidity vs ...). Pre-fix this column existed
    // in the schema but was never written → 0 rows in 24h had it populated,
    // making rejection analytics impossible.
    sqlx::query(
        r#"
        INSERT INTO opportunities (
            id, chain_id, strategy_kind, dex_a, dex_b, pair_symbol,
            token_in, token_out, amount_in_wei,
            expected_profit_usd, net_expected_profit_usd, roi_pct, risk_score,
            block_number, status, rejection_reason, trace_id, detected_at
        ) VALUES (
            $1, $2, $3, $4, $5, $6,
            $7, $8, $9,
            $10, $11, $12, $13,
            $14, 'detected', $15, $16, $17
        )
        ON CONFLICT (id) DO NOTHING
        "#,
    )
    .bind(o.id)
    .bind(o.chain_id as i64)
    .bind(strategy_kind_str(&o.strategy_kind))
    .bind(&o.dex_a)
    .bind(o.dex_b.as_deref())
    .bind(&o.pair_symbol)
    .bind(&o.token_in)
    .bind(&o.token_out)
    .bind(amount_in_wei)
    .bind(o.expected_profit_usd)
    .bind(o.net_expected_profit_usd)
    .bind(o.roi_pct)
    .bind(o.risk_score)
    .bind(o.block_number.map(|n| n as i64))
    .bind(o.rejection_reason.as_deref())
    .bind(o.trace_id)
    .bind(o.detected_at)
    .execute(pool)
    .await
    .context("insert opportunity")?;
    Ok(())
}
