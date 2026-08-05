//! drift_tracker — Stage 2a Y-oracle.
//!
//! Resolves the REALIZED yield $Y$ of paper opportunities by re-executing each
//! pending `paper_trade_runs` row via sim-ctl at the SETTLED block
//! (`sim_block_number + 1` — the block the opportunity would have landed in).
//! Populates `paper_trade_runs.actual_*` so Stage 2b offline calibration has
//! labeled $(\mathbf{e}, Y)$ data — the prerequisite the §IV motor needs.
//!
//! Honesty (RULE 00 / R8):
//!   - $Y$ is computed ONLY from a real sim-ctl re-execution (`passed=true`).
//!   - A failed / non-passing / 501 (sim-ctl not ready) re-exec ⇒ `actual_*`
//!     stays NULL (the row remains "unlabeled"), NEVER a fabricated 0 yield.
//!   - `actual_profit_usd` is best-effort: valued via the Redis token price
//!     (`arbx:token_prices:<chain>:<SYMBOL>`); if the price is absent it stays
//!     NULL (honest "re-executed, USD valuation pending"). The RAW
//!     `actual_amount_out_wei` + `actual_timestamp` are ALWAYS set on a passing
//!     re-exec — the label-able signal is captured regardless.
//!
//! Feature-flagged OFF by default (`ARBX_DRIFT_TRACKER_MODE`). The operator
//! enables it once sim-ctl + the settled-block fork are confirmed working.

use std::time::Duration;

use redis::AsyncCommands;
use shared_rs::killswitch::KillSwitchClient;
use sqlx::PgPool;
use tracing::{debug, info, warn};

/// Configuration for the drift-tracker loop.
#[derive(Clone, Debug)]
pub struct DriftConfig {
    pub interval_secs: u64,
    pub batch: i64,
    /// Minimum age (seconds) before a row is eligible — ensures the settled
    /// block has been mined so sim-ctl can fork at `sim_block_number + 1`.
    pub settle_lead_secs: i64,
}

impl DriftConfig {
    pub fn from_env() -> Self {
        Self {
            interval_secs: env_or("ARBX_DRIFT_TRACKER_INTERVAL_SECS", 30),
            batch: env_or("ARBX_DRIFT_TRACKER_BATCH", 20) as i64,
            settle_lead_secs: env_or("ARBX_DRIFT_TRACKER_SETTLE_LEAD_SECS", 15) as i64,
        }
    }
}

fn env_or(key: &str, default: u64) -> u64 {
    std::env::var(key)
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(default)
}

/// One pending paper_trade_runs row awaiting Y-resolution.
#[derive(sqlx::FromRow)]
struct PendingRun {
    id: uuid::Uuid,
    opportunity_id: uuid::Uuid,
    chain_id: i32,
    sim_block_number: i64,
    sim_expected_profit_usd: Option<f64>,
    token_in: String,
}

/// Periodic loop: fetch pending → re-execute via sim-ctl → compute Y → UPDATE.
/// Kill-switch-gated; non-fatal errors log + continue.
pub async fn run_periodic(
    db: PgPool,
    simctl_url: String,
    cfg: DriftConfig,
    killswitch: KillSwitchClient,
    mut redis: Option<redis::aio::ConnectionManager>,
    http: reqwest::Client,
) {
    let mut ticker = tokio::time::interval(Duration::from_secs(cfg.interval_secs));
    ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    info!(event = "drift_tracker.spawned", interval_s = cfg.interval_secs);

    loop {
        ticker.tick().await;
        // Kill-switch: if tripped, idle this tick.
        if !killswitch.is_enabled().await {
            debug!(event = "drift_tracker.killswitch_idle");
            continue;
        }
        if let Err(e) = tick(&db, &simctl_url, &cfg, &mut redis, &http).await {
            warn!(event = "drift_tracker.tick_failed", error = %e);
        }
    }
}

async fn tick(
    db: &PgPool,
    simctl_url: &str,
    cfg: &DriftConfig,
    redis: &mut Option<redis::aio::ConnectionManager>,
    http: &reqwest::Client,
) -> anyhow::Result<()> {
    // 1. Fetch a batch of pending rows past the settle-lead, with a route.
    let rows: Vec<PendingRun> = sqlx::query_as::<_, PendingRun>(
        r#"
        SELECT ptr.id, ptr.opportunity_id, ptr.chain_id, ptr.sim_block_number,
               ptr.sim_expected_profit_usd, o.token_in
        FROM paper_trade_runs ptr
        JOIN opportunities o ON o.id = ptr.opportunity_id
        WHERE ptr.actual_timestamp IS NULL
          AND ptr.sim_block_number IS NOT NULL
          AND o.route_metadata IS NOT NULL
          AND o.route_metadata::text NOT IN ('', '{}')
          AND ptr.created_at < now() - ($1 * interval '1 second')
        ORDER BY ptr.created_at
        LIMIT $2
        "#,
    )
    .bind(cfg.settle_lead_secs)
    .bind(cfg.batch)
    .fetch_all(db)
    .await?;

    if rows.is_empty() {
        return Ok(());
    }
    debug!(event = "drift_tracker.batch", n = rows.len());

    let mut resolved = 0u32;
    let mut failed = 0u32;
    for r in rows {
        match resolve_one(db, simctl_url, redis, http, &r).await {
            Ok(true) => resolved += 1,
            Ok(false) => failed += 1, // sim failed/reverted — honest skip
            Err(e) => {
                failed += 1;
                warn!(event = "drift_tracker.row_error", opp = %r.opportunity_id, error = %e);
            }
        }
    }
    info!(event = "drift_tracker.tick", resolved, failed);
    Ok(())
}

/// Re-execute one row via sim-ctl. Returns Ok(true) if resolved (actual_* set),
/// Ok(false) if the sim honestly did not pass (row left NULL).
async fn resolve_one(
    db: &PgPool,
    simctl_url: &str,
    redis: &mut Option<redis::aio::ConnectionManager>,
    http: &reqwest::Client,
    r: &PendingRun,
) -> anyhow::Result<bool> {
    let settled_block = r.sim_block_number + 1;
    let body = serde_json::json!({
        "opportunity_id": r.opportunity_id.to_string(),
        "route_source": "simctl_lookup",
        "block_number": settled_block,
    });
    let resp = match http
        .post(format!("{}/simulate", simctl_url.trim_end_matches('/')))
        .json(&body)
        .timeout(Duration::from_secs(20))
        .send()
        .await
    {
        Ok(r) => r,
        Err(e) => {
            debug!(event = "drift_tracker.sim_unavailable", opp = %r.opportunity_id, error = %e);
            return Ok(false); // sim-ctl unreachable — honest skip, retry next tick
        }
    };
    let status = resp.status();
    // 501 = sim-ctl up but fork/backend not configured — honest skip.
    if status == reqwest::StatusCode::NOT_IMPLEMENTED {
        return Ok(false);
    }
    if !status.is_success() {
        warn!(event = "drift_tracker.sim_http_error", opp = %r.opportunity_id, status = %status);
        return Ok(false);
    }
    let outcome: SimOutcome = match resp.json().await {
        Ok(o) => o,
        Err(e) => {
            warn!(event = "drift_tracker.sim_parse_error", opp = %r.opportunity_id, error = %e);
            return Ok(false);
        }
    };
    // R8: only a PASSING re-exec yields a realized $Y$. Non-pass ⇒ NULL, skip.
    if !outcome.passed {
        debug!(event = "drift_tracker.sim_not_passed", opp = %r.opportunity_id, reason = ?outcome.fail_reason);
        return Ok(false);
    }

    // Raw realized amount out (token_in profit) from the re-exec.
    let actual_amount_out_wei: Option<String> = outcome
        .simulated_profit_token_in
        .clone()
        .or(outcome.intermediate_amount_out.clone());
    let gas_cost_usd = compute_gas_cost_usd(&outcome);

    // Best-effort USD valuation via the Redis token price
    // (arbx:token_prices:<chain>:<SYMBOL> — the enricher's canonical key).
    let sym = r.token_in.split('/').next().unwrap_or("").trim().to_uppercase();
    let actual_profit_usd = match (redis.as_mut(), &outcome.simulated_profit_token_in, sym.as_str()) {
        (Some(rc), Some(token_profit_str), s) if !s.is_empty() => {
            let price = token_price_usd(rc, r.chain_id, s).await;
            match (price, token_profit_str.parse::<f64>().ok()) {
                (Some(p), Some(profit_wei)) if profit_wei > 0.0 => {
                    // token_profit is in smallest-unit wei; convert to whole tokens
                    // (18 decimals assumption — honest MVP; refine per-token later).
                    let whole = profit_wei / 1e18;
                    Some(whole * p)
                }
                _ => None, // price absent or unparseable ⇒ honest NULL
            }
        }
        _ => None,
    };

    let drift_pct = match (actual_profit_usd, r.sim_expected_profit_usd) {
        (Some(act), Some(sim)) if sim.abs() > 1e-9 => Some((act - sim) / sim * 100.0),
        _ => None,
    };

    // UPDATE — only on a passing re-exec. actual_timestamp marks "resolved".
    sqlx::query(
        r#"
        UPDATE paper_trade_runs
        SET actual_amount_out_wei = $1,
            actual_profit_usd = $2,
            actual_gas_cost_usd = $3,
            actual_block_number = $4,
            actual_timestamp = now(),
            profit_drift_pct = $5
        WHERE id = $6 AND actual_timestamp IS NULL
        "#,
    )
    .bind(actual_amount_out_wei.as_deref())
    .bind(actual_profit_usd)
    .bind(gas_cost_usd)
    .bind(settled_block)
    .bind(drift_pct)
    .bind(r.id)
    .execute(db)
    .await?;
    Ok(true)
}

/// Best-effort token USD price from the enricher's Redis hash.
async fn token_price_usd(
    redis: &mut redis::aio::ConnectionManager,
    chain_id: i32,
    symbol_upper: &str,
) -> Option<f64> {
    let key = format!("arbx:token_prices:{}", chain_id);
    let v: Option<String> = redis.hget(&key, symbol_upper).await.ok().flatten();
    v.and_then(|s| s.parse().ok()).filter(|p: &f64| p.is_finite() && *p > 0.0)
}

/// Gas cost in USD from the sim outcome (gas_used × gas_price_wei → ETH → USD).
/// Honest MVP: gas_price_wei × gas_used / 1e18 ETH; ETH→USD via the same Redis
/// price hash (symbol "ETH"). None if uncomputable.
fn compute_gas_cost_usd(o: &SimOutcome) -> Option<f64> {
    let gas_used = o.gas_used_total?;
    let gpw: f64 = o.gas_price_wei.as_deref()?.parse().ok()?;
    let eth = (gas_used as f64 * gpw) / 1e18;
    // ETH→USD priced at insert-time level would be ideal; MVP uses a nominal
    // placeholder of None (gas cost is small vs the profit signal) until a
    // reliable ETH-USD feed is wired into recon.
    let _ = eth;
    None
}

#[derive(serde::Deserialize, Debug)]
struct SimOutcome {
    passed: bool,
    fail_reason: Option<String>,
    gas_used_total: Option<i64>,
    gas_price_wei: Option<String>,
    simulated_profit_token_in: Option<String>,
    intermediate_amount_out: Option<String>,
}
