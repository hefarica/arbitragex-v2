//! Persistence for relays-client: INSERT executions, UPDATE opportunities,
//! UPSERT relay_scores. All in one transaction.
//!
//! BE-3.4: also exposes `insert_paper_trade_run` — records every opportunity
//! suppressed by the paper-mode gate so drift can be tracked later.

use anyhow::{Context, Result};
use sha2::{Digest, Sha256};
use shared_rs::contracts::{ExecutionResult, ExecutionStatus, Opportunity};
use sqlx::postgres::PgPool;
use sqlx::types::BigDecimal;
use std::str::FromStr;

pub async fn persist_execution(pool: &PgPool, r: &ExecutionResult, chain_id: i64) -> Result<()> {
    let status = status_str(&r.status);
    let gas_used: Option<BigDecimal> = r
        .gas_used_wei
        .as_deref()
        .and_then(|s| BigDecimal::from_str(s).ok());
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
        ExecutionStatus::NotSubmitted
        | ExecutionStatus::Submitted
        | ExecutionStatus::NotImplemented => "simulated", // leave as simulated (no on-chain action)
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

/// BE-3.4: Insert a paper-trade run record for an opportunity that was
/// suppressed by the paper-mode gate.
///
/// The `sim_*` fields are populated from the opportunity row. The `actual_*`
/// fields are left NULL and will be backfilled by a future drift-tracker
/// worker that replays the route against current chain state.
///
/// Uses `sqlx::query()` (runtime-bound) intentionally: `sqlx::query!()` macro
/// requires an offline `.sqlx/` cache generated by `cargo sqlx prepare`, which
/// is not yet configured in CI. Runtime binding is type-safe at the DB level
/// and avoids scope creep into the sqlx tooling setup for this batch.
///
/// Errors are non-fatal: a failure to record a paper-trade run must never
/// block the main execution path. Callers should log the error and proceed.
pub async fn insert_paper_trade_run(pool: &PgPool, opp: &Opportunity) -> Result<()> {
    // Bind the canonical snake_case inner string — `format!("{:?}")` on the
    // newtype would emit `StrategyKind("dex_arb")` and corrupt the column
    // (contracts.rs: "persistence binds `strategy_kind.as_str()`").
    let strategy_kind_str = opp.strategy_kind.as_str().to_string();
    // sim_expected_profit_usd: use net when available (spine-computed);
    // fall back to gross. R8 fail-honest: if both are None, record 0.0 —
    // this signals "uncomputed" in the paper-trade row without blocking insert.
    let sim_profit: f64 = opp
        .net_expected_profit_usd
        .or(opp.expected_profit_usd)
        .unwrap_or(0.0);

    // PAPERLEDGER-08 / R8 fail-honest gas derivation — see `derived_gas_cost_usd`.
    let sim_gas_cost_usd = derived_gas_cost_usd(opp);

    // LATLED-01: populate execution_time_ms at write time — uniform across all
    // three call sites in submit_engine (measured up to the actual ledger
    // write, not up to the gate decision).
    let execution_time_ms = detection_to_ledger_ms(opp.detected_at, chrono::Utc::now());

    sqlx::query(
        r#"
        INSERT INTO paper_trade_runs (
            opportunity_id,
            chain_id,
            strategy_kind,
            sim_expected_profit_usd,
            sim_gas_cost_usd,
            sim_block_number,
            reason,
            route_hash,
            execution_time_ms
        ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
        "#,
    )
    .bind(opp.id)
    .bind(opp.chain_id as i32)
    .bind(&strategy_kind_str)
    .bind(sim_profit)
    .bind(sim_gas_cost_usd)
    .bind(opp.block_number.map(|n| n as i64))
    .bind(opp.rejection_reason.as_deref())
    .bind(route_hash(opp))
    .bind(execution_time_ms)
    .execute(pool)
    .await
    .context("insert paper_trade_run")?;

    Ok(())
}

/// PAPERLEDGER-08 / R8 fail-honest gas derivation (mirrors the TS Shadow
/// Archiver A3/R8-03 in paper-trade-archiver.ts): gas is EMBEDDED inside
/// `net_expected_profit_usd` (spine 8-component cost model), so
/// `sim_gas_cost_usd = gross − net` when BOTH are present (clamped ≥ 0 — net
/// can exceed gross only via numeric noise; rounded to 6 dp for the
/// NUMERIC(18,6) column). Either missing → None: never fabricate a cost the
/// spine did not compute.
fn derived_gas_cost_usd(opp: &Opportunity) -> Option<f64> {
    match (opp.expected_profit_usd, opp.net_expected_profit_usd) {
        (Some(gross), Some(net)) => Some(((gross - net).max(0.0) * 1e6).round() / 1e6),
        _ => None,
    }
}

/// LATLED-01: detection→ledger latency for `paper_trade_runs.execution_time_ms`.
///
/// Wall-clock ms from the opportunity's `detected_at` (stamped by the scanner
/// at detection) to the moment the paper-run row is written. This is the
/// pipeline-latency leg of the A.5 daily audit (revert rate / latency / sim
/// error rate) — the column sat at 0/591,753 rows until 2026-08-23. The TS
/// Shadow Archiver (`paper-trade-archiver.ts detectionToLedgerMs`) computes
/// the same quantity at its write; both MUST stay in the same semantics or
/// `AVG(execution_time_ms)` (sed-status.ts) mixes incomparable populations.
///
/// R8 fail-honest: clock skew (detected_at in the future) records 0, never a
/// negative number; values clamp to i32 for the INTEGER column.
fn detection_to_ledger_ms(
    detected_at: chrono::DateTime<chrono::Utc>,
    now: chrono::DateTime<chrono::Utc>,
) -> i32 {
    now.signed_duration_since(detected_at)
        .num_milliseconds()
        .clamp(0, i32::MAX as i64) as i32
}

/// Deterministic route fingerprint — EXACT MIRROR of the TS Shadow Archiver's
/// `routeHash()` (backend/api-server/src/routes/paper-trade-archiver.ts):
/// canonical `chain_id|strategy_kind|dex_a|dex_b|token_in|token_out` → sha256 →
/// `"rh:" + first 32 hex chars`. paper_trade_runs has TWO writers (the Redis
/// stream archiver in TS and this Rust paper-mode terminus path); both MUST
/// produce the same fingerprint for the same route or route-grouped analytics
/// split in two. This is a display/grouping identifier, not a crypto
/// commitment. sha2 is already a workspace dependency — no new external dep.
fn route_hash(opp: &Opportunity) -> String {
    let canonical = format!(
        "{}|{}|{}|{}|{}|{}",
        opp.chain_id,
        opp.strategy_kind.as_str(),
        opp.dex_a,
        opp.dex_b.as_deref().unwrap_or(""),
        opp.token_in,
        opp.token_out,
    );
    let hex_str = hex::encode(Sha256::digest(canonical.as_bytes()));
    format!("rh:{}", &hex_str[..32])
}

#[cfg(test)]
mod tests {
    use super::*;
    use shared_rs::contracts::StrategyKind;
    use uuid::Uuid;

    fn sample_opp() -> Opportunity {
        Opportunity {
            id: Uuid::new_v4(),
            chain_id: 1,
            strategy_kind: StrategyKind::flashloan_arb(),
            dex_a: "uniswap-v2".to_string(),
            dex_b: Some("sushi".to_string()),
            pair_symbol: "WETH/USDC".to_string(),
            // Canonical tokens referenced from the shared catalog consts (no
            // inline 0x literals — HARDCODE-10).
            token_in: shared_rs::chains::WETH_MAINNET.to_string(),
            token_out: shared_rs::chains::USDC_MAINNET.to_string(),
            amount_in_wei: "1000000000000000000".to_string(),
            expected_profit_usd: Some(10.0),
            net_expected_profit_usd: Some(7.5),
            roi_pct: None,
            risk_score: None,
            block_number: Some(123),
            rejection_reason: None,
            cartridge_id: None,
            detected_at: chrono::Utc::now(),
            trace_id: Uuid::new_v4(),
        }
    }

    /// Parity lock with the TS archiver format: `rh:` + 32 lowercase hex chars.
    #[test]
    fn route_hash_matches_archiver_format() {
        let h = route_hash(&sample_opp());
        assert!(h.starts_with("rh:"));
        assert_eq!(h.len(), 3 + 32);
        assert!(h[3..]
            .chars()
            .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase()));
    }

    /// dex_b participates in the fingerprint (None ≡ "" like the archiver's `?? ""`).
    #[test]
    fn route_hash_distinguishes_routes() {
        let mut a = sample_opp();
        let mut b = sample_opp();
        b.dex_b = None;
        assert_ne!(route_hash(&a), route_hash(&b));
        a.dex_b = Some("sushi".to_string());
        b.dex_b = Some("sushi".to_string());
        assert_eq!(route_hash(&a), route_hash(&b));
    }

    /// Gas = gross − net (both present); None when either is missing.
    #[test]
    fn gas_derivation_is_fail_honest() {
        let mut opp = sample_opp();
        assert_eq!(derived_gas_cost_usd(&opp), Some(2.5));
        opp.net_expected_profit_usd = None;
        assert_eq!(derived_gas_cost_usd(&opp), None);
        opp.net_expected_profit_usd = Some(11.0); // net > gross → clamp to 0, not negative
        assert_eq!(derived_gas_cost_usd(&opp), Some(0.0));
    }

    /// LATLED-01: latency = now − detected_at; clock skew clamps to 0, never
    /// negative; i32 saturation for the INTEGER column.
    #[test]
    fn detection_to_ledger_ms_measures_elapsed() {
        let now = chrono::Utc::now();
        let detected = now - chrono::Duration::milliseconds(1500);
        assert_eq!(detection_to_ledger_ms(detected, now), 1500);
    }

    #[test]
    fn detection_to_ledger_ms_skew_clamps_to_zero() {
        let now = chrono::Utc::now();
        let future_detected = now + chrono::Duration::seconds(5);
        assert_eq!(detection_to_ledger_ms(future_detected, now), 0);
    }

    #[test]
    fn detection_to_ledger_ms_saturates_at_i32_max() {
        let now = chrono::Utc::now();
        let ancient = now - chrono::Duration::days(3650);
        assert_eq!(detection_to_ledger_ms(ancient, now), i32::MAX);
    }
}
