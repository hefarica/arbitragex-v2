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

/// Does this deployment carry the per-row measurement ledger (migration 126)?
///
/// `Absent` is a DEGRADED deployment, not an error: the value still lands, only
/// its classification cannot be recorded. The worker must never fail a row over
/// it — and never guess that the column exists.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GasLedger {
    Present,
    Absent,
}

/// Migration 126's `gas_measurement_state` domain, as a literal so a test can
/// assert every state this worker writes is inside the CHECK constraint — the
/// worker must not invent a sixth value and start failing UPDATEs at runtime.
const GAS_STATE_DOMAIN: [&str; 5] = [
    "not_attempted",
    "measured",
    "not_applicable",
    "unavailable",
    "impossible",
];

/// PASS arm classification: a real observation is `measured`; evaluated but not
/// computable is `unavailable`. The migration's own comment makes `unavailable`
/// a BUG SURFACE that must never be relabelled `not_applicable` — that relabel
/// would assert "no gas was burned" about a row nobody could compute.
fn gas_state_for_resolved(gas_cost_usd: Option<f64>) -> &'static str {
    if gas_cost_usd.is_some() {
        "measured"
    } else {
        "unavailable"
    }
}

/// ECONOMIC/MARKET reject: settled WITHOUT broadcast, so no gas was burned and
/// there is nothing to measure — migration 126's definition of
/// `not_applicable`, which it backfills from `sim_fail_family IN
/// ('economic','market')`. Same population, same label.
const GAS_STATE_NOT_APPLICABLE: &str = "not_applicable";

/// STRUCTURAL reject: `calibration_eligible = false`, so the drift scan can
/// never select the row again — structurally unresolvable ⇒ `impossible`,
/// mirroring the migration's `WHEN calibration_eligible IS FALSE` branch.
const GAS_STATE_IMPOSSIBLE: &str = "impossible";

/// The extra SET columns, with the placeholder index of the state value.
/// Present only when the column exists; the value is written by the SAME
/// statement, so no reader can ever observe a value without its classification
/// (invariants I3a/I3b of migration 126).
fn gas_state_set(state_placeholder: usize) -> String {
    format!(
        "gas_measurement_state = ${state_placeholder}, gas_measurement_updated_at = now()"
    )
}

/// SQL for the PASS arm. Pure so a test can assert the exactly-once guard and
/// the conditional classification without a database. Returns `(sql, writes_state)`.
fn resolved_update_sql(ledger: GasLedger) -> (String, bool) {
    let base = "UPDATE paper_trade_runs
                SET actual_amount_out_wei = $1,
                    actual_profit_usd = $2,
                    actual_gas_cost_usd = $3,
                    actual_block_number = $4,
                    actual_timestamp = now(),
                    profit_drift_pct = $5,
                    sim_last_attempt_at = now()";
    let tail = "WHERE id = $6 AND actual_timestamp IS NULL";
    match ledger {
        // $7 is the classification, bound last.
        GasLedger::Present => (format!("{base}, {} {tail}", gas_state_set(7)), true),
        GasLedger::Absent => (format!("{base} {tail}"), false),
    }
}

/// SQL for the ECONOMIC/MARKET arm ($4 = classification).
fn rejected_update_sql(ledger: GasLedger) -> (String, bool) {
    let base = "UPDATE paper_trade_runs
                SET actual_profit_usd = 0.0,
                    actual_block_number = $1,
                    actual_timestamp = now(),
                    sim_fail_family = $2,
                    sim_last_attempt_at = now()";
    let tail = "WHERE id = $3 AND actual_timestamp IS NULL";
    match ledger {
        GasLedger::Present => (format!("{base}, {} {tail}", gas_state_set(4)), true),
        GasLedger::Absent => (format!("{base} {tail}"), false),
    }
}

/// SQL for the STRUCTURAL arm ($2 = classification).
fn structural_update_sql(ledger: GasLedger) -> (String, bool) {
    let base = "UPDATE paper_trade_runs
                SET calibration_eligible = false,
                    sim_fail_family = 'structural',
                    sim_attempts = sim_attempts + 1,
                    sim_last_attempt_at = now()";
    let tail = "WHERE id = $1 AND actual_timestamp IS NULL";
    match ledger {
        GasLedger::Present => (format!("{base}, {} {tail}", gas_state_set(2)), true),
        GasLedger::Absent => (format!("{base} {tail}"), false),
    }
}

/// Read the catalog once per tick: a migration Data applies while recon is
/// running must start being used WITHOUT a restart, otherwise the worker would
/// keep leaving the classification unwritten and the A.6 denominator could
/// never converge.
async fn probe_gas_ledger(db: &PgPool) -> GasLedger {
    let row: Result<Option<(i32,)>, sqlx::Error> = sqlx::query_as(
        "SELECT 1
           FROM information_schema.columns
          WHERE table_schema = 'public'
            AND table_name = 'paper_trade_runs'
            AND column_name = 'gas_measurement_state'",
    )
    .fetch_optional(db)
    .await;
    match row {
        Ok(Some(_)) => GasLedger::Present,
        Ok(None) => GasLedger::Absent,
        // A failed catalog read is NOT evidence of presence: degrade to Absent
        // (the value still lands) instead of writing a column we did not see.
        Err(e) => {
            debug!(event = "drift_tracker.gas_ledger_probe_failed", error = %e);
            GasLedger::Absent
        }
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

    // Measurement-ledger capability (GAS-COVERAGE-01 / migration 126).
    let mut ledger = GasLedger::Absent;
    let mut ledger_announced = false;
    loop {
        ticker.tick().await;
        // Kill-switch: if tripped, idle this tick.
        if !killswitch.is_enabled().await {
            debug!(event = "drift_tracker.killswitch_idle");
            continue;
        }
        let observed = probe_gas_ledger(&db).await;
        if observed != ledger || !ledger_announced {
            // R10: an absent ledger is a NAMED condition, announced once per
            // transition instead of silently degrading every row.
            match observed {
                GasLedger::Present => info!(
                    event = "drift_tracker.gas_ledger_present",
                    column = "paper_trade_runs.gas_measurement_state"
                ),
                GasLedger::Absent => warn!(
                    event = "drift_tracker.gas_ledger_absent",
                    missing = "database/migrations/126_paper_trade_runs_gas_coverage_ledger.sql",
                    effect = "actual_gas_cost_usd lands WITHOUT its classification; the A.6 coverage denominator cannot narrow"
                ),
            }
            ledger = observed;
            ledger_announced = true;
        }
        if let Err(e) = tick(&db, &simctl_url, &cfg, &mut redis, &http, ledger).await {
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
    ledger: GasLedger,
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
        match resolve_one(db, simctl_url, redis, http, ledger, &r).await {
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
    ledger: GasLedger,
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
            let (sql, writes_state) = structural_update_sql(ledger);
            if writes_state {
                sqlx::query(&sql)
                    .bind(r.id)
                    .bind(GAS_STATE_IMPOSSIBLE)
                    .execute(db)
                    .await?;
            } else {
                sqlx::query(&sql).bind(r.id).execute(db).await?;
            }
            Ok(Attempt::StructuralNotEligible)
        }
        Attempt::NotPassed(family) => {
            // S4-02/S4-03: the market rejected the trade at the settled block —
            // terminal WITH label. Y = 0 EXACTLY (computed: a rejected
            // execution realized nothing). Amounts stay NULL (nothing was
            // realized); the family records WHY for Stage 2b stratification.
            // The label lands with its gas classification in ONE statement: the
            // market rejected the trade at the settled block, nothing was
            // broadcast, so there is no gas to measure (`not_applicable`).
            let (sql, writes_state) = rejected_update_sql(ledger);
            if writes_state {
                sqlx::query(&sql)
                    .bind(settled_block)
                    .bind(family.as_str())
                    .bind(r.id)
                    .bind(GAS_STATE_NOT_APPLICABLE)
                    .execute(db)
                    .await?;
            } else {
                sqlx::query(&sql)
                    .bind(settled_block)
                    .bind(family.as_str())
                    .bind(r.id)
                    .execute(db)
                    .await?;
            }
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
            // GAS-PRICE-ADAPTER-01: the native-coin USD leg of the gas cost.
            // A miss is logged with its EXACT reason and lands as SQL NULL —
            // never a nominal placeholder (R8). The coverage ledger classifies
            // the row from `actual_gas_cost_usd IS NULL` (migration 126, CASE
            // branch 1 promotes only a real value to `measured`), so an
            // uncomputable gas cost stays visible instead of being dressed up
            // as a measured zero.
            let gas_token_usd =
                match crate::gas_price::resolve(redis, crate::gas_price::chain_key_id(r.chain_id))
                    .await
                {
                    Ok(q) => {
                        debug!(
                            event = "drift_tracker.gas_price_resolved",
                            opp = %r.opportunity_id,
                            chain_id = r.chain_id,
                            field = %q.source_field,
                            usd_per_unit = q.usd_per_unit,
                            adapter = q.adapter_version
                        );
                        Some(q.usd_per_unit)
                    }
                    // An unmapped chain is a CONFIG defect, not a per-row fact:
                    // it must not hide at debug level. Volume is bounded by the
                    // batch size (<= ARBX_DRIFT_TRACKER_BATCH rows per tick).
                    Err(crate::gas_price::GasPriceMiss::UnsupportedChain { .. }) => {
                        warn!(
                            event = "drift_tracker.gas_price_unavailable",
                            opp = %r.opportunity_id,
                            chain_id = r.chain_id,
                            reason = "gas_price_unsupported_chain"
                        );
                        None
                    }
                    // Per-row, expected absence (no live field yet): debug
                    // detail + the aggregate coverage view stays the audit
                    // script's job (R9 — no per-item INFO in the hot loop).
                    Err(m) => {
                        debug!(
                            event = "drift_tracker.gas_price_unavailable",
                            opp = %r.opportunity_id,
                            chain_id = r.chain_id,
                            reason = m.reason(),
                            detail = %m.detail()
                        );
                        None
                    }
                };
            let gas_cost_usd = compute_gas_cost_usd(&outcome, gas_token_usd);

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

            // UPDATE — only on a passing re-exec. actual_timestamp marks
            // "resolved"; the gas VALUE and its CLASSIFICATION are written by
            // this same statement so no reader can observe one without the
            // other (migration 126 invariants I3a/I3b).
            let (sql, writes_state) = resolved_update_sql(ledger);
            if writes_state {
                sqlx::query(&sql)
                    .bind(actual_amount_out_wei.as_deref())
                    .bind(actual_profit_usd)
                    .bind(gas_cost_usd)
                    .bind(settled_block)
                    .bind(drift_pct)
                    .bind(r.id)
                    .bind(gas_state_for_resolved(gas_cost_usd))
                    .execute(db)
                    .await?;
            } else {
                sqlx::query(&sql)
                    .bind(actual_amount_out_wei.as_deref())
                    .bind(actual_profit_usd)
                    .bind(gas_cost_usd)
                    .bind(settled_block)
                    .bind(drift_pct)
                    .bind(r.id)
                    .execute(db)
                    .await?;
            }
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
    // Key string derived from the SHARED contract helper — a second literal
    // here would be a silent drift surface if the key scheme ever changes
    // (same producer, same identity: `gas_price` uses the same helper).
    let key =
        shared_rs::price_oracle::redis_token_prices_key(crate::gas_price::chain_key_id(chain_id));
    let v: Option<String> = redis.hget(&key, symbol_upper).await.ok().flatten();
    v.and_then(|s| s.parse().ok())
        .filter(|p: &f64| p.is_finite() && *p > 0.0)
}

/// Gas cost in USD from the sim outcome (gas_used × gas_price_wei → native → USD).
///
/// UNITS (verified against the producer chain, not assumed):
/// `gas_price_wei` is WEI per gas unit — `gas_oracle_worker` persists
/// `provider.get_gas_price()` into `arbx:gas_price_wei:<chain>`, which is exactly
/// what `sim-ctl::revm_backend` reads and hands to revm as `TxEnv.gas_price` —
/// and `gas_used_total` is a count of gas units. So
/// `gas_used × gas_price_wei / 1e18` is the native-coin amount, and the USD leg
/// is `native × usd_per_unit` with `usd_per_unit` resolved by
/// [`gas_price::resolve`] (the canonical price tower).
///
/// `None` in EITHER input is an honest NULL: uncomputable, not zero. There is no
/// nominal/placeholder price here — the previous `let _ = eth; None` stub is
/// what left `actual_gas_cost_usd` with no working producer (GAS-COVERAGE-01
/// defect (a)). `Some(0.0)` means computed and exactly zero (R8).
fn compute_gas_cost_usd(o: &SimOutcome, native_usd_per_unit: Option<f64>) -> Option<f64> {
    let gas_used = o.gas_used_total?;
    let gpw: f64 = o.gas_price_wei.as_deref()?.parse().ok()?;
    let native = (gas_used as f64 * gpw) / 1e18;
    let price = native_usd_per_unit?;
    let usd = native * price;
    usd.is_finite().then_some(usd)
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

    /// Outcome carrying the gas fields a real re-exec reports.
    fn gas_outcome(gas_used: i64, gas_price_wei: &str) -> SimOutcome {
        SimOutcome {
            passed: Some(true),
            fail_reason: None,
            gas_used_total: Some(gas_used),
            gas_price_wei: Some(gas_price_wei.to_string()),
            simulated_profit_token_in: None,
            intermediate_amount_out: None,
        }
    }

    // ── GAS-PRICE-ADAPTER-01: the gas-cost producer (GAP-A regression gate) ──

    /// The defect this closes: with a resolved native-coin price the function
    /// MUST return a value. Before the adapter, `compute_gas_cost_usd` returned
    /// `None` unconditionally (`let _ = eth; None`), so `actual_gas_cost_usd`
    /// could never be anything but SQL NULL — the A.6 breaker's numerator had
    /// no producer no matter how many runs accumulated. This test fails if that
    /// stub is ever restored.
    #[test]
    fn priced_gas_is_computed_not_stubbed_to_none() {
        // 210_000 gas at 20 gwei = 0.0042 native; at $2619.59 ⇒ $11.002278.
        let usd = compute_gas_cost_usd(&gas_outcome(210_000, "20000000000"), Some(2619.59));
        let expected = 210_000.0 * 20e9 / 1e18 * 2619.59;
        let got = usd.expect("gas cost must be computed when the price is known");
        assert!((got - expected).abs() < 1e-9, "got {got} want {expected}");
        assert!((got - 11.002278).abs() < 1e-6, "got {got}");
    }

    /// Absence stays absence: no price ⇒ NULL (not 0.0), and no gas data ⇒ NULL.
    #[test]
    fn unpriced_or_gasless_inputs_are_honest_nulls() {
        assert_eq!(
            compute_gas_cost_usd(&gas_outcome(210_000, "20000000000"), None),
            None,
            "no price source ⇒ NULL, never a placeholder number"
        );
        let mut no_gas = outcome(Some(true), None);
        no_gas.gas_price_wei = Some("20000000000".into());
        assert_eq!(compute_gas_cost_usd(&no_gas, Some(2619.59)), None);
        let mut no_price = outcome(Some(true), None);
        no_price.gas_used_total = Some(210_000);
        assert_eq!(compute_gas_cost_usd(&no_price, Some(2619.59)), None);
    }

    /// A computed zero is a ZERO, not a NULL (R8 distinguishes the two) — and
    /// an unparseable wei string is a NULL, never a silent 0.
    #[test]
    fn computed_zero_differs_from_unparseable() {
        assert_eq!(
            compute_gas_cost_usd(&gas_outcome(0, "20000000000"), Some(2619.59)),
            Some(0.0)
        );
        assert_eq!(
            compute_gas_cost_usd(&gas_outcome(210_000, "not-a-number"), Some(2619.59)),
            None
        );
        assert_eq!(
            compute_gas_cost_usd(&gas_outcome(210_000, ""), Some(2619.59)),
            None
        );
    }

    /// A poisoned price cannot poison the ledger: non-finite products are NULL.
    #[test]
    fn non_finite_products_are_rejected() {
        assert_eq!(
            compute_gas_cost_usd(&gas_outcome(210_000, "20000000000"), Some(f64::NAN)),
            None
        );
        assert_eq!(
            compute_gas_cost_usd(&gas_outcome(210_000, "20000000000"), Some(f64::INFINITY)),
            None
        );
    }

    /// Chain routing: the key the profit leg reads is derived from the shared
    /// contract helper, and a non-positive chain id can never alias chain 1.
    #[test]
    fn chain_key_id_is_derived_not_retyped() {
        assert_eq!(crate::gas_price::chain_key_id(1), 1);
        assert_eq!(crate::gas_price::chain_key_id(137), 137);
        assert_eq!(crate::gas_price::chain_key_id(0), 0);
        assert_eq!(crate::gas_price::chain_key_id(-7), 0);
        assert_eq!(
            shared_rs::price_oracle::redis_token_prices_key(crate::gas_price::chain_key_id(1)),
            "arbx:token_prices:1"
        );
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

    // ── GAS-COVERAGE-01 / migration 126: the measurement ledger ──────────────
    //
    // The defect this closes: the drift-tracker wrote `actual_gas_cost_usd`
    // while NOT writing its classification. Migration 126's own invariants say
    // a value must classify its own row (I3b) and a state must not claim a
    // value that is not there (I3a); the one writer of the value ignored both,
    // so the ledger could never converge and the A.6 coverage denominator could
    // never narrow.

    #[test]
    fn gas_state_domain_matches_the_migration_126_check_constraint() {
        // The CHECK in 126_…sql admits exactly these five values. A sixth would
        // make every worker UPDATE fail at runtime with a constraint violation.
        for s in [
            gas_state_for_resolved(None),
            gas_state_for_resolved(Some(1.0)),
            GAS_STATE_NOT_APPLICABLE,
            GAS_STATE_IMPOSSIBLE,
        ] {
            assert!(
                GAS_STATE_DOMAIN.contains(&s),
                "{s} is not a legal gas_measurement_state"
            );
        }
        assert_eq!(GAS_STATE_DOMAIN.len(), 5);
    }

    #[test]
    fn a_value_is_always_classified_measured() {
        // Invariant I3b: actual_gas_cost_usd IS NOT NULL ⇒ state = 'measured'.
        assert_eq!(gas_state_for_resolved(Some(11.002278)), "measured");
        // Including a computed ZERO: zero is a measurement, not an absence (R8).
        assert_eq!(gas_state_for_resolved(Some(0.0)), "measured");
    }

    #[test]
    fn an_uncomputable_value_is_unavailable_never_not_applicable() {
        // Invariant I3a + the migration's warning. Relabelling an uncomputable
        // row as `not_applicable` would assert "no gas was burned" about a row
        // nobody could compute — the false PASS the ledger exists to prevent.
        assert_eq!(gas_state_for_resolved(None), "unavailable");
        assert_ne!(gas_state_for_resolved(None), GAS_STATE_NOT_APPLICABLE);
        // A gasless outcome (no gas_used_total / no price) is exactly that case.
        let mut gasless = outcome(Some(true), None);
        gasless.gas_price_wei = Some("20000000000".into());
        assert_eq!(
            gas_state_for_resolved(compute_gas_cost_usd(&gasless, Some(2619.59))),
            "unavailable"
        );
    }

    #[test]
    fn every_arm_keeps_the_exactly_once_guard_in_both_ledger_variants() {
        // The guard is the whole durability story: a row is written once.
        // Appending a SET column must not disturb it, in EITHER variant.
        for ledger in [GasLedger::Present, GasLedger::Absent] {
            let (pass, _) = resolved_update_sql(ledger);
            assert!(
                pass.contains("WHERE id = $6 AND actual_timestamp IS NULL"),
                "PASS arm lost its exactly-once guard: {pass}"
            );
            let (rejected, _) = rejected_update_sql(ledger);
            assert!(
                rejected.contains("WHERE id = $3 AND actual_timestamp IS NULL"),
                "reject arm lost its exactly-once guard: {rejected}"
            );
            let (structural, _) = structural_update_sql(ledger);
            assert!(
                structural.contains("WHERE id = $1 AND actual_timestamp IS NULL"),
                "structural arm lost its exactly-once guard: {structural}"
            );
        }
    }

    #[test]
    fn the_ledger_column_appears_only_when_the_migration_is_applied() {
        // A deployment without migration 126 must keep working: referencing an
        // absent column would fail the statement and the row would never
        // resolve — a regression strictly worse than an unclassified value.
        for (present, absent) in [
            (resolved_update_sql(GasLedger::Present).0, resolved_update_sql(GasLedger::Absent).0),
            (
                rejected_update_sql(GasLedger::Present).0,
                rejected_update_sql(GasLedger::Absent).0,
            ),
            (
                structural_update_sql(GasLedger::Present).0,
                structural_update_sql(GasLedger::Absent).0,
            ),
        ] {
            assert!(present.contains("gas_measurement_state = $"), "{present}");
            assert!(present.contains("gas_measurement_updated_at = now()"), "{present}");
            assert!(!absent.contains("gas_measurement_state"), "{absent}");
        }
        assert_eq!(resolved_update_sql(GasLedger::Present).1, true);
        assert_eq!(resolved_update_sql(GasLedger::Absent).1, false);
    }

    #[test]
    fn the_state_placeholder_is_the_next_free_index_of_its_own_statement() {
        // Off-by-one here would bind the classification to the WRONG parameter
        // and silently write a wrong state (or a wrong id) — the whole point of
        // pinning the numbering.
        assert!(resolved_update_sql(GasLedger::Present)
            .0
            .contains("gas_measurement_state = $7"));
        assert!(rejected_update_sql(GasLedger::Present)
            .0
            .contains("gas_measurement_state = $4"));
        assert!(structural_update_sql(GasLedger::Present)
            .0
            .contains("gas_measurement_state = $2"));
    }
}
