//! stage2_calibration — Stage 2b: the log-LR store writer (§IV dictamen).
//!
//! Consumes the Y-labels the Stage 2a drift-tracker lands in
//! `paper_trade_runs.actual_*`, joins them with the per-opportunity evidence
//! vector archived in `scored_opportunities.evidence_vector`, and upserts the
//! calibrated per-operator log likelihood-ratio into
//! `math_operator_calibration` (migration 103) — the store the §IV motor reads
//! as `posterior_log_odds = prior_log_odds + Σ_k (log_lr_k · e_k)`
//! (`searcher-rs/src/math_evidence.rs::evidence_posterior_log_odds`).
//!
//! With the store empty the motor honestly collapses to `flat_prior`; this job
//! is what flips `source_context` to `calibrated` — from REAL labeled data
//! only (RULE 00 / R8: no fabricated LRs, ever).
//!
//! ## Statistics — hierarchical shrinkage (partial pooling)
//!
//! N labeled events spread over 31 operator strata is sparse (N=500 ⇒ ~16 per
//! operator on average, and firing is far from uniform). A raw per-operator
//! win-rate would be noise, so each operator's posterior mean shrinks toward
//! the pooled base rate θ₀ (empirical-Bayes global prior):
//!
//! ```text
//! θ_k = (κ·θ₀ + wins_k) / (κ + n_k)        log_lr_k = logit(θ_k) − logit(θ₀)
//! ```
//!
//! κ = pseudo-events of prior strength (default 20): an operator with n=3 is
//! ~87% prior (3 events are not evidence), one with n=150 is ~88% empirical.
//! An operator with n=0 keeps log_lr = 0 (LR = e⁰ = 1 ⇒ no contribution — the
//! honest near-flat state).
//!
//! ## Design deviations from the operator's sketch (deliberate, documented)
//!
//! 1. **Per-operator store, not per (operator, cartridge)**: migration 103
//!    declares `operator_id` as PRIMARY KEY and the §IV motor consumes
//!    `Σ log_lr_k · e_k` per operator — the hierarchy collapses to
//!    operator → global. A cartridge-stratified store needs a schema change
//!    first; not smuggled in here.
//! 2. **Recompute-from-source, not accumulate-on-upsert**: every
//!    consolidation recomputes (n_k, wins_k) from ALL labeled rows and writes
//!    absolute values (`sample_count = EXCLUDED.sample_count`). Idempotent,
//!    restart-safe, and immune to the counter-drift the tally board taught us
//!    (recompute-from-registry lesson). The "incremental" cadence the
//!    doctrine asks for lives in the TRIGGER — consolidate every N newly
//!    labeled events (watermark = `calibrated_at`, set to the max
//!    `actual_timestamp` actually consolidated, so no row is ever skipped) —
//!    not in the arithmetic.
//! 3. **Continuous evidence caveat**: e_k is the operator's SCALAR output, not
//!    a boolean. This MVP computes (n_k, wins_k) over "operator fired" events
//!    (e_k ≠ 0) while the motor multiplies log_lr_k by the scalar — large
//!    |e_k| magnitudes amplify the contribution. Shrinkage + the θ clamp keep
//!    bounded values; refining to a scalar-weighted logistic fit is a
//!    follow-up with data on the table.
//!
//! ## Honesty gates
//!
//! - A label exists ONLY where `actual_profit_usd IS NOT NULL` (the
//!   drift-tracker writes it solely on a passing re-exec). Win ⇔ > 0.
//! - Rows resolved-but-unvalued (`actual_timestamp` set, USD NULL) are counted
//!   in the log as `unvalued` and excluded from the math — never imputed.
//! - `scored_opportunities` rows without `evidence_vector` cannot contribute
//!   to any operator (the (e,Y) pair is incomplete) — logged as `no_evidence`.
//! - Kill-switch-gated like every recon loop; feature-flagged OFF by default
//!   (`ARBX_STAGE2_CALIBRATION_MODE`).

use std::time::Duration;

use serde_json::Value as Json;
use shared_rs::killswitch::KillSwitchClient;
use sqlx::PgPool;
use tracing::{debug, info, warn};

/// Θ clamp keeps logit finite (and log-LR bounded ±~9.2). Math guard, not
/// operator config.
const THETA_EPS: f64 = 1e-4;

/// Number of canonical operators (op_01 … op_31; index = operator_id − 1).
const OPERATOR_COUNT: usize = 31;

/// Configuration for the Stage 2 calibration loop.
#[derive(Clone, Debug)]
pub struct Stage2Config {
    pub interval_secs: u64,
    /// Newly labeled events required before a consolidation runs.
    pub consolidate_every: i64,
    /// Prior strength κ in pseudo-events (shrinkage toward the pooled θ₀).
    pub prior_kappa: f64,
}

impl Stage2Config {
    pub fn from_env() -> Self {
        Self {
            interval_secs: env_u64("ARBX_STAGE2_CALIBRATION_INTERVAL_SECS", 60),
            consolidate_every: env_u64("ARBX_CALIBRATION_CONSOLIDATE_EVERY", 100) as i64,
            prior_kappa: env_f64("ARBX_CALIBRATION_PRIOR_KAPPA", 20.0),
        }
    }
}

fn env_u64(key: &str, default: u64) -> u64 {
    std::env::var(key)
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(default)
}

fn env_f64(key: &str, default: f64) -> f64 {
    std::env::var(key)
        .ok()
        .and_then(|v| v.parse().ok())
        .filter(|v: &f64| v.is_finite() && *v > 0.0)
        .unwrap_or(default)
}

/// One labeled (evidence, Y) pair.
#[derive(sqlx::FromRow)]
struct LabeledRow {
    y: f64,
    evidence: Option<Json>,
    actual_timestamp: chrono::DateTime<chrono::Utc>,
}

/// Periodic loop: count new labels since the watermark → consolidate every N.
/// Kill-switch-gated; non-fatal errors log + continue (next tick retries).
pub async fn run_periodic(db: PgPool, killswitch: KillSwitchClient, cfg: Stage2Config) {
    let mut ticker = tokio::time::interval(Duration::from_secs(cfg.interval_secs));
    ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    info!(
        event = "stage2_calibration.spawned",
        interval_s = cfg.interval_secs,
        consolidate_every = cfg.consolidate_every,
        prior_kappa = cfg.prior_kappa
    );

    loop {
        ticker.tick().await;
        if !killswitch.is_enabled().await {
            debug!(event = "stage2_calibration.killswitch_idle");
            continue;
        }
        if let Err(e) = tick(&db, &cfg).await {
            warn!(event = "stage2_calibration.tick_failed", error = %e);
        }
    }
}

async fn tick(db: &PgPool, cfg: &Stage2Config) -> anyhow::Result<()> {
    // Watermark = labels already consolidated. Stored as MAX(calibrated_at);
    // consolidate() writes it as the max actual_timestamp it actually folded
    // in, so a row is never skipped by a race between count and recompute.
    let watermark: Option<chrono::DateTime<chrono::Utc>> =
        sqlx::query_scalar("SELECT MAX(calibrated_at) FROM math_operator_calibration")
            .fetch_one(db)
            .await?;
    let watermark = watermark.unwrap_or(chrono::DateTime::<chrono::Utc>::UNIX_EPOCH);

    let (new_labels,): (i64,) = sqlx::query_as(
        r#"
        SELECT COUNT(*)
        FROM paper_trade_runs
        WHERE actual_timestamp IS NOT NULL
          AND actual_timestamp > $1
        "#,
    )
    .bind(watermark)
    .fetch_one(db)
    .await?;

    if new_labels < cfg.consolidate_every {
        debug!(
            event = "stage2_calibration.waiting",
            new_labels,
            needed = cfg.consolidate_every
        );
        return Ok(());
    }
    consolidate(db, cfg).await
}

/// Full recompute over ALL labeled (evidence, Y) pairs + idempotent upsert of
/// the 31 per-operator rows (n=0 operators persist log_lr=0 / sample_count=0 —
/// the store stays complete and the motor stays honest for them).
async fn consolidate(db: &PgPool, cfg: &Stage2Config) -> anyhow::Result<()> {
    // Latest evidence per opportunity (scored_opportunities.opportunity_id is
    // TEXT, paper_trade_runs.opportunity_id is UUID — cast on the FK side).
    let rows: Vec<LabeledRow> = sqlx::query_as::<_, LabeledRow>(
        r#"
        SELECT ptr.actual_profit_usd::FLOAT8                AS y,
               lat.evidence_vector                          AS evidence,
               ptr.actual_timestamp                         AS actual_timestamp
        FROM paper_trade_runs ptr
        JOIN LATERAL (
            SELECT so.evidence_vector
            FROM scored_opportunities so
            WHERE so.opportunity_id = ptr.opportunity_id::TEXT
              AND so.evidence_vector IS NOT NULL
            ORDER BY so.created_at DESC
            LIMIT 1
        ) lat ON true
        WHERE ptr.actual_timestamp IS NOT NULL
          AND ptr.actual_profit_usd IS NOT NULL
        "#,
    )
    .fetch_all(db)
    .await?;

    // Observability split: labels that cannot join an evidence vector.
    let (labeled_total, unvalued): (i64, i64) = sqlx::query_as(
        r#"
        SELECT COUNT(*) FILTER (WHERE actual_timestamp IS NOT NULL),
               COUNT(*) FILTER (WHERE actual_timestamp IS NOT NULL
                                  AND actual_profit_usd IS NULL)
        FROM paper_trade_runs
        "#,
    )
    .fetch_one(db)
    .await?;

    let no_evidence = labeled_total - unvalued - rows.len() as i64;

    let mut op_n = [0u64; OPERATOR_COUNT + 1]; // index by operator_id 1..=31
    let mut op_wins = [0u64; OPERATOR_COUNT + 1];
    let mut total_n = 0u64;
    let mut total_wins = 0u64;
    let mut watermark = chrono::DateTime::<chrono::Utc>::UNIX_EPOCH;

    for row in rows {
        if !row.y.is_finite() {
            continue; // NaN/Inf label — never folded into the math
        }
        let win = row.y > 0.0;
        total_n += 1;
        total_wins += u64::from(win);
        if row.actual_timestamp > watermark {
            watermark = row.actual_timestamp;
        }
        if let Some(Json::Array(arr)) = row.evidence {
            for (idx, v) in arr.iter().enumerate().take(OPERATOR_COUNT) {
                // "Fired" = a finite, non-zero scalar at slot idx (0.0 is the
                // builder's explicit "not computed" token).
                if let Some(f) = v.as_f64() {
                    if f.is_finite() && f != 0.0 {
                        op_n[idx + 1] += 1;
                        op_wins[idx + 1] += u64::from(win);
                    }
                }
            }
        }
    }

    if total_n == 0 {
        info!(
            event = "stage2_calibration.skipped_no_pairs",
            labeled_total,
            unvalued,
            no_evidence,
            "no joinable (evidence, Y) pairs — store left untouched (honest)"
        );
        return Ok(());
    }

    let theta0 = clamp_theta(total_wins as f64 / total_n as f64);

    let mut ids: Vec<i16> = Vec::with_capacity(OPERATOR_COUNT);
    let mut llrs: Vec<f64> = Vec::with_capacity(OPERATOR_COUNT);
    let mut ns: Vec<i64> = Vec::with_capacity(OPERATOR_COUNT);
    let mut ops_with_data = 0u32;
    let mut max_abs_llr = 0.0f64;
    for k in 1..=OPERATOR_COUNT {
        let (llr, n) = operator_log_lr(op_n[k], op_wins[k], theta0, cfg.prior_kappa);
        if n > 0 {
            ops_with_data += 1;
            max_abs_llr = max_abs_llr.max(llr.abs());
        }
        ids.push(k as i16);
        llrs.push(llr);
        ns.push(op_n[k] as i64);
    }

    // calibrated_at = the max label folded in (the true watermark — see tick()).
    sqlx::query(
        r#"
        INSERT INTO math_operator_calibration (operator_id, log_lr, sample_count, calibrated_at)
        SELECT op, llr, n, $4
        FROM UNNEST($1::int2[], $2::float8[], $3::int8[]) AS t(op, llr, n)
        ON CONFLICT (operator_id) DO UPDATE SET
            log_lr       = EXCLUDED.log_lr,
            sample_count = EXCLUDED.sample_count,
            calibrated_at = EXCLUDED.calibrated_at
        "#,
    )
    .bind(&ids)
    .bind(&llrs)
    .bind(&ns)
    .bind(watermark)
    .execute(db)
    .await?;

    info!(
        event = "stage2_calibration.consolidated",
        pairs = total_n,
        wins = total_wins,
        theta0,
        ops_with_data,
        max_abs_llr,
        unvalued,
        no_evidence,
        watermark = %watermark,
        "log-LR store upserted (recompute-from-source, shrinkage κ={})",
        cfg.prior_kappa
    );
    Ok(())
}

// ─── Pure statistics (unit-testable, no I/O) ─────────────────────────────────

/// Clamp a probability into [ε, 1−ε] so logit stays finite.
pub(crate) fn clamp_theta(p: f64) -> f64 {
    p.clamp(THETA_EPS, 1.0 - THETA_EPS)
}

/// logit(p) = ln(p / (1−p)); caller passes a pre-clamped p.
pub(crate) fn logit(p: f64) -> f64 {
    p.ln() - (1.0 - p).ln()
}

/// Shrunk posterior mean: θ_k = (κ·θ₀ + wins) / (κ + n).
/// n = 0 ⇒ θ₀ (pure prior — the operator has said nothing yet).
pub(crate) fn shrunk_theta(n: u64, wins: u64, theta0: f64, kappa: f64) -> f64 {
    let n = n as f64;
    let p_emp = if n > 0.0 { wins as f64 / n } else { theta0 };
    (kappa * theta0 + n * p_emp) / (kappa + n)
}

/// Per-operator log-LR contribution vs the pooled base rate:
/// log_lr = logit(clamp(θ_k)) − logit(θ₀). n = 0 ⇒ 0.0 (LR = 1 ⇒ no
/// contribution — honest near-flat until the operator has evidence).
pub(crate) fn operator_log_lr(n: u64, wins: u64, theta0: f64, kappa: f64) -> (f64, u64) {
    if n == 0 {
        return (0.0, 0);
    }
    let theta_k = clamp_theta(shrunk_theta(n, wins, clamp_theta(theta0), kappa));
    (logit(theta_k) - logit(clamp_theta(theta0)), n)
}

#[cfg(test)]
mod tests {
    use super::*;

    const KAPPA: f64 = 20.0;

    #[test]
    fn shrinkage_prior_dominates_sparse_operator() {
        // n=3, wins=3, θ₀=0.5, κ=20 ⇒ θ = (10 + 3) / 23 ≈ 0.5652:
        // the operator moved only ~13% toward its empirical 1.0 — 3 events
        // are not evidence.
        let theta = shrunk_theta(3, 3, 0.5, KAPPA);
        assert!((theta - 13.0 / 23.0).abs() < 1e-12);
    }

    #[test]
    fn shrinkage_empirical_dominates_dense_operator() {
        // n=150, wins=150 ⇒ θ = (10 + 150) / 170 ≈ 0.9412: ~88% empirical.
        let theta = shrunk_theta(150, 150, 0.5, KAPPA);
        assert!((theta - 160.0 / 170.0).abs() < 1e-12);
    }

    #[test]
    fn zero_n_operator_contributes_nothing() {
        let (llr, n) = operator_log_lr(0, 0, 0.5, KAPPA);
        assert_eq!(llr, 0.0);
        assert_eq!(n, 0);
    }

    #[test]
    fn better_than_base_gets_positive_log_lr() {
        // wins 80/100 vs θ₀=0.5 ⇒ θ_k=(10+80)/120=0.75 ⇒ log_lr=ln(3).
        let (llr, n) = operator_log_lr(100, 80, 0.5, KAPPA);
        assert_eq!(n, 100);
        assert!((llr - (3.0f64).ln()).abs() < 1e-9);
    }

    #[test]
    fn all_wins_clamps_to_finite_log_lr() {
        // Even a perfect record stays finite (θ clamped away from 1).
        let (llr, _) = operator_log_lr(10_000, 10_000, 0.5, KAPPA);
        assert!(llr.is_finite() && llr > 0.0 && llr < 25.0);
    }

    #[test]
    fn logit_is_symmetric_around_half() {
        assert!((logit(clamp_theta(0.25)) + logit(clamp_theta(0.75))).abs() < 1e-12);
    }
}
