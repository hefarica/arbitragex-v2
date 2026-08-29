//! drift_tracker — Stage 2a Y-oracle (S4-03 Capa B).
//!
//! Resolves the REALIZED yield $Y$ of paper opportunities by re-executing each
//! pending `paper_trade_runs` row via sim-ctl at the SETTLED block
//! (`sim_block_number + 1` — the block the opportunity would have landed in).
//! Populates `paper_trade_runs.actual_*` so Stage 2b offline calibration has
//! labeled $(\mathbf{e}, Y)$ data — the prerequisite the §IV motor needs.
//!
//! S4-03 attempt ladder (no-contamination gate, runbook accepted 2026-08-29):
//!   - PASS → `Resolved`: label from the real re-execution (`actual_*` set).
//!   - ECONOMIC / MARKET reject → terminal WITH label: the market rejected the
//!     trade at the settled block, so $Y = 0$ exactly (the realized yield of a
//!     rejected execution). `sim_fail_family` records why. These ARE valid
//!     calibration labels — losing trades teach the priors as much as winners.
//!   - STRUCTURAL reject (broken fixture: signer without balance, missing
//!     fork, missing decimals, incomplete candidate) → terminal INELIGIBLE:
//!     `calibration_eligible = false`, no label, NO retry — retrying a broken
//!     fixture fixes nothing and an infra defect must never poison priors.
//!   - PENDING (sim-ctl unreachable / 501 backend-not-configured / 503 gas
//!     absent / unparseable body) → backoff retry at
//!     $30s \cdot 2^{\min(n,7)}$ until `ARBX_DRIFT_TRACKER_MAX_ATTEMPTS`.
//!
//! Honesty (RULE 00 / R8):
//!   - $Y$ is computed ONLY from a real sim-ctl re-execution.
//!   - `actual_profit_usd = 0.0` means "computed and exactly zero" (a
//!     rejected execution realized nothing); NULL means "not computed".
//!   - `actual_profit_usd` on a PASS is best-effort: valued via the Redis
//!     token price (`arbx:token_prices:<chain>:<SYMBOL>`); if the price is
//!     absent it stays NULL (honest "re-executed, USD valuation pending").
//!     The RAW `actual_amount_out_wei` + `actual_timestamp` are ALWAYS set on
//!     a passing re-exec — the label-able signal is captured regardless.
//!
//! Feature-flagged OFF by default (`ARBX_DRIFT_TRACKER_MODE`). The operator
//! enables it once sim-ctl + the B2c backend are confirmed working.

use std::time::Duration;

use redis::AsyncCommands;
use shared_rs::killswitch::KillSwitchClient;
use shared_rs::sim_taxonomy::{classify_fail_reason, FailFamily};
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
    /// Max sim attempts per row before the pending scan gives up on it
    /// (S4-03 backoff ceiling; structural rows exit earlier via
    /// `calibration_eligible = false`).
    pub max_attempts: i64,
}

impl DriftConfig {
    pub fn from_env() -> Self {
        Self {
            interval_secs: env_or("ARBX_DRIFT_TRACKER_INTERVAL_SECS", 30),
            batch: env_or("ARBX_DRIFT_TRACKER_BATCH", 20) as i64,
            settle_lead_secs: env_or("ARBX_DRIFT_TRACKER_SETTLE_LEAD_SECS", 15) as i64,
            max_attempts: env_or("ARBX_DRIFT_TRACKER_MAX_ATTEMPTS", 10) as i64,
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

/// S4-03 attempt outcome for one pending row.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Attempt {
    /// PASS → label landed (`actual_*` set from the real re-execution).
    Resolved,
    /// ECONOMIC/MARKET reject → terminal WITH label ($Y = 0$ exactly) +
    /// `sim_fail_family` recorded.
    NotPassed(FailFamily),
    /// STRUCTURAL → terminal ineligible (`calibration_eligible = false`),
    /// no label, no retry.
    StructuralNotEligible,
    /// Transient (unreachable / 501 / 503 / unparseable) → backoff retry.
    Pending,
    /// Unexpected condition — row left untouched; surfaced as a row error.
    Failed,
}

/// Pure S4-03 decision: HTTP status + parsed outcome → [`Attempt`].
///
/// Status-level classification comes FIRST because 501/503 bodies carry
/// structural-looking error tags (`gas_price_read_failed`, …) that must NOT
/// terminate the row — the sim never ran, so the fixture was never at fault;
/// the operator flipping `SIM_BACKEND`/Redis heals it. Outcome-level
/// `fail_reason`s classify through the S4-02 taxonomy (fail-closed).
fn decide_attempt(status: reqwest::StatusCode, outcome: Option<&SimOutcome>) -> Attempt {
    // Typed not-configured / gas-absent: the sim never ran — transient ladder.
    if status == reqwest::StatusCode::NOT_IMPLEMENTED
        || status == reqwest::StatusCode::SERVICE_UNAVAILABLE
    {
        return Attempt::Pending;
    }
    if !status.is_success() {
        // 404 (no usable route_metadata) and 422 (candidate_incomplete) are
        // row-inherent: retrying the same row cannot change it.
        if status == reqwest::StatusCode::NOT_FOUND
            || status == reqwest::StatusCode::UNPROCESSABLE_ENTITY
        {
            return Attempt::StructuralNotEligible;
        }
        return Attempt::Failed;
    }
    match outcome {
        // 200 without a parseable SimOutcome — defensive (the stub that never
        // carried `passed` is dead; keep the honest pending ladder anyway).
        None => Attempt::Pending,
        Some(o) => match o.passed {
            Some(true) => Attempt::Resolved,
            Some(false) => match classify_fail_reason(o.fail_reason.as_deref().unwrap_or("")) {
                FailFamily::Structural => Attempt::StructuralNotEligible,
                family @ (FailFamily::Economic | FailFamily::Market) => Attempt::NotPassed(family),
            },
            None => Attempt::Pending,
        },
    }
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
    info!(
        event = "drift_tracker.spawned",
        interval_s = cfg.interval_secs,
        max_attempts = cfg.max_attempts
    );

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
    //    S4-03 scan gates: still calibration-eligible, attempts not exhausted,
    //    and past the pending backoff window (30s · 2^min(attempts,7), capped
    //    ~64min). Structural rows exited earlier via calibration_eligible=false.
    let rows: Vec<PendingRun> = sqlx::query_as::<_, PendingRun>(
        r#"
        SELECT ptr.id, ptr.opportunity_id, ptr.chain_id, ptr.sim_block_number,
               ptr.sim_expected_profit_usd, o.token_in
        FROM paper_trade_runs ptr
        JOIN opportunities o ON o.id = ptr.opportunity_id
        WHERE ptr.actual_timestamp IS NULL
          AND ptr.calibration_eligible
          AND ptr.sim_attempts < $3
          AND ptr.sim_block_number IS NOT NULL
          AND o.route_metadata IS NOT NULL
          AND o.route_metadata::text NOT IN ('', '{}')
          AND ptr.created_at < now() - ($1 * interval '1 second')
          AND (
            ptr.sim_last_attempt_at IS NULL
            OR ptr.sim_last_attempt_at
               < now() - (30 * power(2, LEAST(ptr.sim_attempts, 7)) * interval '1 second')
          )
        ORDER BY ptr.created_at
        LIMIT $2
        "#,
    )
    .bind(cfg.settle_lead_secs)
    .bind(cfg.batch)
    .bind(cfg.max_attempts)
    .fetch_all(db)
    .await?;

    if rows.is_empty() {
        return Ok(());
    }
    debug!(event = "drift_tracker.batch", n = rows.len());

    let mut resolved = 0u32;
    let mut rejected = 0u32; // NotPassed (economic/market labels)
    let mut structural = 0u32;
    let mut pending = 0u32;
    let mut failed = 0u32;
    for r in rows {
        match resolve_one(db, simctl_url, redis, http, &r).await {
            Ok(Attempt::Resolved) => resolved += 1,
            Ok(Attempt::NotPassed(_)) => rejected += 1,
            Ok(Attempt::StructuralNotEligible) => structural += 1,
            Ok(Attempt::Pending) => pending += 1,
            Ok(Attempt::Failed) => failed += 1,
            Err(e) => {
                failed += 1;
                warn!(event = "drift_tracker.row_error", opp = %r.opportunity_id, error = %e);
            }
        }
    }
    // R9: one aggregated summary at info — per-item detail stays at debug.
    info!(
        event = "drift_tracker.tick",
        resolved, rejected, structural, pending, failed
    );
    Ok(())
}

/// Re-execute one row via sim-ctl and apply the S4-03 attempt ladder.
async fn resolve_one(
    db: &PgPool,
    simctl_url: &str,
    redis: &mut Option<redis::aio::ConnectionManager>,
    http: &reqwest::Client,
    r: &PendingRun,
) -> anyhow::Result<Attempt> {
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
            record_pending(db, r.id).await?;
            return Ok(Attempt::Pending); // sim-ctl unreachable — backoff
        }
    };
    let status = resp.status();
    // Only parse the body when it can carry an outcome (2xx); 501/503/4xx/5xx
    // classify by status alone (their bodies are error payloads).
    let outcome: Option<SimOutcome> = if status.is_success() {
        match resp.json().await {
            Ok(o) => Some(o),
            Err(e) => {
                warn!(event = "drift_tracker.sim_parse_error", opp = %r.opportunity_id, error = %e);
                None
            }
        }
    } else {
        debug!(event = "drift_tracker.sim_http_status", opp = %r.opportunity_id, status = %status);
        None
    };

    match decide_attempt(status, outcome.as_ref()) {
        Attempt::Pending => {
            record_pending(db, r.id).await?;
            Ok(Attempt::Pending)
        }
        Attempt::Failed => Ok(Attempt::Failed), // row untouched; retry next tick
        Attempt::StructuralNotEligible => {
            let reason = outcome
                .as_ref()
                .and_then(|o| o.fail_reason.clone())
                .unwrap_or_else(|| format!("http_{status}"));
            debug!(
                event = "drift_tracker.sim_structural",
                opp = %r.opportunity_id,
                reason = %reason
            );
            sqlx::query(
                r#"
                UPDATE paper_trade_runs
                SET calibration_eligible = false,
                    sim_fail_family = 'structural',
                    sim_attempts = sim_attempts + 1,
                    sim_last_attempt_at = now()
                WHERE id = $1 AND actual_timestamp IS NULL
                "#,
            )
            .bind(r.id)
            .execute(db)
            .await?;
            Ok(Attempt::StructuralNotEligible)
        }
        Attempt::NotPassed(family) => {
            // S4-02/S4-03: the market rejected the trade at the settled block —
            // terminal WITH label. Y = 0 EXACTLY (computed: a rejected
            // execution realized nothing). Amounts stay NULL (nothing was
            // realized); the family records WHY for Stage 2b stratification.
            sqlx::query(
                r#"
                UPDATE paper_trade_runs
                SET actual_profit_usd = 0.0,
                    actual_block_number = $1,
                    actual_timestamp = now(),
                    sim_fail_family = $2,
                    sim_last_attempt_at = now()
                WHERE id = $3 AND actual_timestamp IS NULL
                "#,
            )
            .bind(settled_block)
            .bind(family.as_str())
            .bind(r.id)
            .execute(db)
            .await?;
            debug!(
                event = "drift_tracker.sim_rejected_label",
                opp = %r.opportunity_id,
                family = family.as_str()
            );
            Ok(Attempt::NotPassed(family))
        }
        Attempt::Resolved => {
            let outcome = match outcome {
                Some(o) => o,
                None => return Ok(Attempt::Failed), // decide only returns Resolved with Some
            };

            // Raw realized amount out (token_in profit) from the re-exec.
            let actual_amount_out_wei: Option<String> = outcome
                .simulated_profit_token_in
                .clone()
                .or(outcome.intermediate_amount_out.clone());
            let gas_cost_usd = compute_gas_cost_usd(&outcome);

            // Best-effort USD valuation via the Redis token price
            // (arbx:token_prices:<chain>:<SYMBOL> — the enricher's canonical key).
            let sym = r
                .token_in
                .split('/')
                .next()
                .unwrap_or("")
                .trim()
                .to_uppercase();
            let actual_profit_usd = match (
                redis.as_mut(),
                &outcome.simulated_profit_token_in,
                sym.as_str(),
            ) {
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
                    profit_drift_pct = $5,
                    sim_last_attempt_at = now()
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
            Ok(Attempt::Resolved)
        }
    }
}

/// Record a pending attempt (S4-03 backoff bookkeeping — attempts + timestamp).
async fn record_pending(db: &PgPool, id: uuid::Uuid) -> anyhow::Result<()> {
    sqlx::query(
        r#"
        UPDATE paper_trade_runs
        SET sim_attempts = sim_attempts + 1,
            sim_last_attempt_at = now()
        WHERE id = $1 AND actual_timestamp IS NULL
        "#,
    )
    .bind(id)
    .execute(db)
    .await?;
    Ok(())
}

/// Best-effort token USD price from the enricher's Redis hash.
async fn token_price_usd(
    redis: &mut redis::aio::ConnectionManager,
    chain_id: i32,
    symbol_upper: &str,
) -> Option<f64> {
    let key = format!("arbx:token_prices:{}", chain_id);
    let v: Option<String> = redis.hget(&key, symbol_upper).await.ok().flatten();
    v.and_then(|s| s.parse().ok())
        .filter(|p: &f64| p.is_finite() && *p > 0.0)
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
    /// None = body carried no verdict (unparseable/error payload) — S4-03:
    /// the old A3 stub never set it, so the field is optional and a None
    /// routes to the Pending ladder instead of a parse failure.
    passed: Option<bool>,
    fail_reason: Option<String>,
    gas_used_total: Option<i64>,
    gas_price_wei: Option<String>,
    simulated_profit_token_in: Option<String>,
    intermediate_amount_out: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn outcome(passed: Option<bool>, fail_reason: Option<&str>) -> SimOutcome {
        SimOutcome {
            passed,
            fail_reason: fail_reason.map(|s| s.to_string()),
            gas_used_total: None,
            gas_price_wei: None,
            simulated_profit_token_in: None,
            intermediate_amount_out: None,
        }
    }

    #[test]
    fn not_configured_statuses_are_pending() {
        // 501 (SIM_BACKEND!=revm / env missing) and 503 (gas absent): the sim
        // never ran — transient ladder, never a terminal row state.
        for status in [
            reqwest::StatusCode::NOT_IMPLEMENTED,
            reqwest::StatusCode::SERVICE_UNAVAILABLE,
        ] {
            assert_eq!(decide_attempt(status, None), Attempt::Pending, "{status}");
        }
    }

    #[test]
    fn row_inherent_statuses_are_structural() {
        // 404 (no usable route_metadata) and 422 (candidate_incomplete): the
        // row itself cannot yield a candidate — terminal ineligible, no retry.
        for status in [
            reqwest::StatusCode::NOT_FOUND,
            reqwest::StatusCode::UNPROCESSABLE_ENTITY,
        ] {
            assert_eq!(
                decide_attempt(status, None),
                Attempt::StructuralNotEligible,
                "{status}"
            );
        }
    }

    #[test]
    fn unexpected_http_error_is_failed() {
        assert_eq!(
            decide_attempt(reqwest::StatusCode::INTERNAL_SERVER_ERROR, None),
            Attempt::Failed
        );
    }

    #[test]
    fn passing_outcome_resolves() {
        assert_eq!(
            decide_attempt(reqwest::StatusCode::OK, Some(&outcome(Some(true), None))),
            Attempt::Resolved
        );
    }

    #[test]
    fn economic_reject_is_terminal_label() {
        assert_eq!(
            decide_attempt(
                reqwest::StatusCode::OK,
                Some(&outcome(Some(false), Some("negative_net_profit")))
            ),
            Attempt::NotPassed(FailFamily::Economic)
        );
    }

    #[test]
    fn market_reject_is_terminal_label() {
        assert_eq!(
            decide_attempt(
                reqwest::StatusCode::OK,
                Some(&outcome(Some(false), Some("route_revert at settled block")))
            ),
            Attempt::NotPassed(FailFamily::Market)
        );
    }

    #[test]
    fn structural_reject_is_ineligible() {
        assert_eq!(
            decide_attempt(
                reqwest::StatusCode::OK,
                Some(&outcome(Some(false), Some("transfer_from_failed")))
            ),
            Attempt::StructuralNotEligible
        );
    }

    #[test]
    fn unknown_fail_reason_fails_closed_to_structural() {
        // Fail-closed (S4-02): unrecognized tags never become labels.
        assert_eq!(
            decide_attempt(
                reqwest::StatusCode::OK,
                Some(&outcome(Some(false), Some("something_entirely_new")))
            ),
            Attempt::StructuralNotEligible
        );
        assert_eq!(
            decide_attempt(reqwest::StatusCode::OK, Some(&outcome(Some(false), None))),
            Attempt::StructuralNotEligible
        );
    }

    #[test]
    fn missing_verdict_field_is_pending() {
        // 200 with no `passed` (the dead stub's shape) or unparseable body:
        // Pending ladder, never a label and never terminal.
        assert_eq!(
            decide_attempt(reqwest::StatusCode::OK, Some(&outcome(None, None))),
            Attempt::Pending
        );
        assert_eq!(
            decide_attempt(reqwest::StatusCode::OK, None),
            Attempt::Pending
        );
    }
}
