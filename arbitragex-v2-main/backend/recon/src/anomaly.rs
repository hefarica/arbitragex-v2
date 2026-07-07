//! Anomaly detector — rolling revert rate per strategy+chain.
//! If threshold exceeded:
//!   - Writes a risk_event (severity=critical).
//!   - If auto_trip_on_high_revert_rate: triggers kill_switch.
//!
//! Runs as part of the aggregator task.

use shared_rs::config::ReconCfg;
use shared_rs::killswitch::KillSwitchClient;
use sqlx::postgres::PgPool;
use sqlx::Row;
use tracing::{info, warn};

pub async fn check_and_react(
    pool: &PgPool,
    cfg: &ReconCfg,
    kill: &KillSwitchClient,
) -> anyhow::Result<u64> {
    let rows = sqlx::query(
        r#"
        SELECT o.strategy_kind, o.chain_id,
               COUNT(*) FILTER (WHERE e.status = 'reverted')::INT8 AS reverts,
               COUNT(*)::INT8 AS total
        FROM executions e
        JOIN opportunities o ON o.id = e.opportunity_id
        WHERE e.submitted_at >= NOW() - make_interval(mins => $1)
        GROUP BY o.strategy_kind, o.chain_id
        HAVING COUNT(*) >= $2
        "#,
    )
    .bind(cfg.anomaly_window_minutes as i32)
    .bind(cfg.anomaly_min_samples as i64)
    .fetch_all(pool)
    .await?;

    let mut anomalies = 0u64;
    for r in rows {
        let strategy: String = r.try_get("strategy_kind").unwrap_or_default();
        let chain: i32 = r.try_get("chain_id").unwrap_or(0);
        let reverts: i64 = r.try_get("reverts").unwrap_or(0);
        let total: i64 = r.try_get("total").unwrap_or(0);
        let rate_pct = if total > 0 {
            (reverts as f64 / total as f64) * 100.0
        } else {
            0.0
        };
        if rate_pct <= cfg.anomaly_revert_rate_pct {
            continue;
        }
        anomalies += 1;
        warn!(event = "anomaly.high_revert_rate", strategy = %strategy,
              chain_id = chain, rate_pct, reverts, total);

        // Insert risk_event (B0.3 2026-05-13: chain_id column required for traceability).
        let _ = sqlx::query(
            r#"
            INSERT INTO risk_events (event_type, severity, source_service, payload, chain_id)
            VALUES ('degradation', 'critical', 'recon', $1::jsonb, $2)
            "#,
        )
        .bind(serde_json::json!({
            "kind": "high_revert_rate",
            "strategy_kind": strategy,
            "chain_id": chain,
            "rate_pct": rate_pct,
            "reverts": reverts,
            "total": total,
            "window_minutes": cfg.anomaly_window_minutes,
        }))
        .bind(chain as i64)
        .execute(pool)
        .await;

        if cfg.auto_trip_on_high_revert_rate {
            let reason = format!("auto_trip:revert_rate:{strategy}:{rate_pct:.1}%");
            if let Err(e) = kill
                .set(true, Some(reason.clone()), Some("recon_auto".to_string()))
                .await
            {
                warn!(event = "anomaly.kill_switch_set_failed", error = %e);
            } else {
                info!(event = "anomaly.kill_switch_tripped", reason);
            }
        }
    }
    Ok(anomalies)
}
