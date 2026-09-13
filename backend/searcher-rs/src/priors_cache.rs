//! priors_cache — Stage 2c (read side): the §IV operator log-LR slice.
//!
//! Loads `math_operator_calibration` (written by recon's `stage2_calibration`
//! job — Stage 2b) into memory on a timer and feeds the §IV posterior fold in
//! the emission hot path:
//!
//! ```text
//! posterior_log_odds = prior_log_odds + Σ_k (log_lr_k · e_k)
//! ```
//!
//! Until this cache existed the §IV primitives (`evidence_posterior_log_odds`)
//! had NO production call site: even a fully calibrated store was invisible.
//! The emitter captured the evidence vector for the archive but never applied
//! the calibration. This module is the missing consumer.
//!
//! ## Honesty / failure semantics (RULE 00 / R8)
//!
//! - Store empty (never consolidated) ⇒ `calibration() = None` ⇒ the fold is
//!   skipped and `calibration_applied = false` — the honest "nothing
//!   calibrated yet" state, identical to pre-cache behavior.
//! - PG unreachable on refresh ⇒ the LAST GOOD slice is retained
//!   (stale-but-real beats fabricated-empty; the store only advances every
//!   `consolidate_every` labels, so staleness is bounded by that cadence).
//! - PG not configured (`pool = None`, dev/single-node) ⇒ `disabled()` — a
//!   permanently-None cache; no I/O, no task spawned.
//! - Read-only: emits nothing, gates nothing, takes no risk — a pure mirror
//!   of a store that recon owns. No kill-switch needed.
//!
//! ## NOT here (audited gap, deliberately excluded)
//!
//! The per-STRATEGY Beta prior (`bayesian_priors` → `PriorState`) is NOT
//! cached: the table is keyed `token_pair UNIQUE` (pre-STRAT-IDENT-01 schema)
//! while `PriorState` is per-STRATEGY — "the pair stays as context in the
//! record, never as the calibration bucket". Feeding pair-keyed priors would
//! re-introduce the identity collapse STRAT-IDENT-01 fixed. Until the table
//! gains `strategy_key` + a writer, the Beta side honestly stays `None`
//! (flat) at the `evaluate_paper_opportunity` call site. The §IV fold below
//! is the calibration surface that DOES have a writer (Stage 2b, this repo).

use std::sync::{Arc, RwLock};
use std::time::Duration;

use sqlx::PgPool;
use tracing::{debug, info};

/// Number of canonical operators (op_01 … op_31; slot = operator_id − 1).
const OPERATOR_COUNT: usize = 31;

/// Read handle shared with the emitter. `None` inside = no calibration
/// available (store empty, PG down since boot, or PG not configured).
#[derive(Clone)]
pub struct PriorsCache {
    inner: Arc<RwLock<Option<Vec<f64>>>>,
}

impl PriorsCache {
    /// Spawn the periodic refresh task over an existing pool.
    pub fn spawn(pool: PgPool) -> Self {
        let cache = Self {
            inner: Arc::new(RwLock::new(None)),
        };
        let refresh = cache.clone();
        let refresh_secs = std::env::var("ARBX_PRIORS_REFRESH_SECS")
            .ok()
            .and_then(|v| v.parse::<u64>().ok())
            .filter(|s| *s >= 5)
            .unwrap_or(30);
        tokio::spawn(async move {
            let mut ticker = tokio::time::interval(Duration::from_secs(refresh_secs));
            ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
            info!(event = "priors_cache.spawned", refresh_secs);
            loop {
                ticker.tick().await;
                if let Err(e) = refresh_once(&pool, &refresh).await {
                    debug!(event = "priors_cache.refresh_failed", error = %e);
                }
            }
        });
        cache
    }

    /// No-PG constructor: a permanently-None cache (honest flat prior).
    pub fn disabled() -> Self {
        Self {
            inner: Arc::new(RwLock::new(None)),
        }
    }

    /// `spawn` over an optional pool — `None` ⇒ `disabled()`.
    pub fn spawn_opt(pool: &Option<PgPool>) -> Self {
        match pool {
            Some(p) => Self::spawn(p.clone()),
            None => Self::disabled(),
        }
    }

    /// Test-only constructor with a fixed slice.
    #[cfg(test)]
    pub(crate) fn from_slice(v: Option<Vec<f64>>) -> Self {
        Self {
            inner: Arc::new(RwLock::new(v)),
        }
    }

    /// Snapshot of the 31 per-operator log-LR slice; `None` = flat prior.
    /// Cheap: clones a 31-slot Vec under a micro-lock (hot-path safe).
    pub fn calibration(&self) -> Option<Vec<f64>> {
        self.inner
            .read()
            .ok()
            .and_then(|g| g.as_ref().map(|v| v.clone()))
    }
}

/// Load the store; overwrite the slice only when the content changed. Mirrors
/// the store exactly: empty table ⇒ None; a table with all |log_lr| ≤ ε is
/// still mirrored (the §IV fold itself classifies it flat — same verdict).
async fn refresh_once(pool: &PgPool, cache: &PriorsCache) -> anyhow::Result<()> {
    let rows: Vec<(i16, f64)> = sqlx::query_as(
        "SELECT operator_id, log_lr FROM math_operator_calibration ORDER BY operator_id",
    )
    .fetch_all(pool)
    .await?;

    let next = if rows.is_empty() {
        None
    } else {
        let mut v = vec![0.0f64; OPERATOR_COUNT];
        for (id, lr) in rows {
            if id >= 1 && (id as usize) <= OPERATOR_COUNT && lr.is_finite() {
                v[(id - 1) as usize] = lr;
            }
        }
        Some(v)
    };

    {
        let mut g = cache
            .inner
            .write()
            .map_err(|_| anyhow::anyhow!("priors_cache lock poisoned"))?;
        if *g != next {
            let calibrated = next
                .as_ref()
                .is_some_and(|v| v.iter().any(|x| x.abs() > 1e-12));
            *g = next;
            info!(event = "priors_cache.updated", calibrated);
        }
    }
    Ok(())
}

// ─── §IV fold (pure, unit-testable) ──────────────────────────────────────────

/// Result of applying the §IV posterior to one opportunity.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct SectionIvFold {
    /// `prior_log_odds + Σ_k (log_lr_k · e_k)`; `None` when either side (evidence
    /// snapshot, calibration slice) is absent — the honest "not computable".
    pub posterior_log_odds: Option<f64>,
    /// `true` when any |log_lr_k| > ε participated in the fold.
    pub calibration_applied: bool,
}

/// Applies the §IV posterior over one opportunity's captured evidence:
/// `posterior_log_odds = prior_log_odds + Σ_k (log_lr_k · e_k)` via the real
/// `math_evidence::evidence_posterior_log_odds` primitive.
///
/// - `posterior_log_odds = None` + `calibration_applied = false` when either
///   side is absent (no evidence snapshot, or no calibration slice).
/// - An all-zero slice yields `Some(prior_log_odds)` + `false` — LR = e⁰ = 1
///   contributes nothing (the honest flat state).
///
/// `evidence` is the archived JSON array (31 slots, 0.0 = not computed);
/// non-finite entries are treated as not computed. Pure — no I/O.
pub(crate) fn section_iv_fold(
    prior_log_odds: f64,
    evidence: &Option<serde_json::Value>,
    calibration: &Option<Vec<f64>>,
) -> SectionIvFold {
    let Some(cal) = calibration else {
        return SectionIvFold {
            posterior_log_odds: None,
            calibration_applied: false,
        };
    };
    let Some(serde_json::Value::Array(arr)) = evidence else {
        return SectionIvFold {
            posterior_log_odds: None,
            calibration_applied: false,
        };
    };
    let mut e = vec![0.0f64; OPERATOR_COUNT];
    for (i, v) in arr.iter().enumerate().take(OPERATOR_COUNT) {
        if let Some(f) = v.as_f64() {
            if f.is_finite() {
                e[i] = f;
            }
        }
    }
    let (lo, ctx) = crate::math_evidence::evidence_posterior_log_odds(prior_log_odds, &e, cal);
    SectionIvFold {
        posterior_log_odds: Some(lo),
        calibration_applied: ctx == "calibrated",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn evidence(vals: &[f64]) -> Option<serde_json::Value> {
        Some(serde_json::json!(vals))
    }

    #[test]
    fn fold_absent_calibration_is_honest_none() {
        let f = section_iv_fold(0.1, &evidence(&[0.5, 0.0]), &None);
        assert_eq!(
            f,
            SectionIvFold {
                posterior_log_odds: None,
                calibration_applied: false
            }
        );
    }

    #[test]
    fn fold_absent_evidence_is_honest_none() {
        let f = section_iv_fold(0.1, &None, &Some(vec![0.0; 31]));
        assert!(f.posterior_log_odds.is_none());
        assert!(!f.calibration_applied);
    }

    #[test]
    fn fold_all_zero_slice_contributes_nothing_but_is_computable() {
        // LR = e^0 = 1 per operator ⇒ posterior == prior, flat.
        let f = section_iv_fold(0.25, &evidence(&[1.0, 2.0]), &Some(vec![0.0; 31]));
        assert_eq!(f.posterior_log_odds, Some(0.25));
        assert!(!f.calibration_applied);
    }

    #[test]
    fn fold_with_calibration_shifts_posterior() {
        // Slot 0 (op_01): log_lr = ln(3); e_0 = 1.0 ⇒ +ln(3) over the prior.
        let mut cal = vec![0.0f64; 31];
        cal[0] = 3.0f64.ln();
        let f = section_iv_fold(0.0, &evidence(&[1.0]), &Some(cal));
        let lo = f.posterior_log_odds.expect("fold computable");
        assert!(f.calibration_applied);
        assert!((lo - 3.0f64.ln()).abs() < 1e-12);
    }

    #[test]
    fn cache_disabled_stays_none() {
        let c = PriorsCache::disabled();
        assert!(c.calibration().is_none());
    }

    #[test]
    fn cache_from_slice_roundtrips() {
        let mut v = vec![0.0f64; 31];
        v[7] = 0.42; // op_08
        let c = PriorsCache::from_slice(Some(v));
        assert_eq!(c.calibration().expect("slice present")[7], 0.42);
    }
}
