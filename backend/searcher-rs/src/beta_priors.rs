//! beta_priors — Deuda 4 / STRAT-IDENT-01 follow-up: the per-STRATEGY Beta
//! prior store (`bayesian_priors` → `PriorState`). Twin of `priors_cache.rs`
//! (which mirrors the §IV operator log-LR slice); this module owns the BETA
//! side: both the consolidator WRITER and the in-memory READER.
//!
//! ## Cycle (one pass, every ARBX_BETA_PRIORS_REFRESH_SECS, default 300)
//!
//! 1. WRITE (full recompute, idempotent): consolidate per-strategy Beta
//!    counts from the archived Gate-C labels in `scored_opportunities` and
//!    UPSERT into `bayesian_priors`. POPULATION (review-locked 2026-09-20,
//!    a8 + -61): ALL emission outcomes (accepted AND rejected), deduped to
//!    the LATEST score per opportunity_id, filtered to labeled rows.
//!    Label semantics (operator decision 2026-09-20, "A ahora + B después";
//!    RULE 00 / R8): Y=0 ⇔ economic-reject verdict OR computed net ≤ 0 ·
//!    Y=1 ⇔ net_profit_usd > 0 (non-economic) · NULL-net non-economic rows
//!    excluded (structural rejects never computed economics — ineligible,
//!    never a loss). WHY the taxonomy override: 99.5% of non_positive_profit
//!    rejects archive net_profit_usd > 0 (the raw POSITIVE detector estimate
//!    — the optimizer's computed ≤ 0 value dies inside the Rejected enum
//!    before persistence). Until the reject-path net-persistence fix lands,
//!    the rejection_reason verdict is the ONLY honest Y:0 source.
//!    VERDICT > SIGN IS PERMANENT for `gas_floor_breach` and
//!    `kelly_negative_edge` (a8, 2026-09-20): both occur with computed
//!    net > 0 (gas floor = net positive but < gas×kelly; Kelly = net
//!    positive but non-positive edge) — a sign-based label would INVERT the
//!    gate's verdict on exactly those. The economic set is deliberately
//!    BROADER than `is_net_dependent()` (which omits non_positive_gross_usd):
//!    a labeling choice, not a derivation — do not "derive" it from that
//!    flag. Full recompute of whatever `scored_opportunities` still retains,
//!    plus a PRUNE of strategies whose rows aged out of retention (a frozen
//!    count would be stale-but-fake — a8 review WARN 2026-09-20: the UPSERT
//!    alone never deletes). Cost: one scan of the retained set (4.77M rows @
//!    2026-09-20) per pass — UNMEASURED against PG capacity; mitigated by the
//!    default-OFF gate and the operator-set cadence (refresh default 300s).
//! 2. READ back the table into `HashMap<String, PriorState>`. Strategies with
//!    zero observations are dropped — `get()` returning None ⇒ flat prior
//!    (identical to pre-module behavior, the honest uncalibrated state).
//!
//! ## Honesty / failure semantics (mirrors priors_cache)
//!
//! - `ARBX_BETA_PRIORS_MODE` not truthy (default) ⇒ `disabled()` — no task,
//!   no I/O, `get()` always None: today's behavior, byte-for-byte.
//! - PG unreachable on refresh ⇒ last good map retained (stale-but-real).
//! - PG not configured ⇒ `disabled()`.
//! - Read of a strategy with no row ⇒ None ⇒ flat Beta(1,1).
//! - The prior never gates emission (Gate C is advisory; RULE 00 invariant:
//!   `opps:detected` always receives the opportunity).

use crate::scoring_pipeline::PriorState;
use sqlx::PgPool;
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use std::time::Duration;
use tracing::{info, warn};

/// Consolidator + reader SQL. The label CASE encodes the review-locked
/// taxonomy: the economic verdict OVERRIDES the archived net (which is the
/// stale positive detector estimate for economic rejects until the
/// reject-path net-persistence fix lands); computed sign applies only to
/// non-economic rows (NULL net ⇒ NULL y ⇒ excluded from both counts).
/// `COUNT(y)` counts non-NULL labels only. The ON CONFLICT target must
/// repeat the partial index predicate (`WHERE strategy_key IS NOT NULL`) to
/// infer `uq_bayesian_priors_strategy` (migration 108). `id DESC` breaks
/// created_at ties (same-archiver-tx batches) so the DISTINCT ON pick is
/// deterministic (a8 review NOTE 2026-09-20).
const CONSOLIDATE_SQL: &str = r#"
WITH latest AS (
    SELECT DISTINCT ON (opportunity_id)
           opportunity_id, strategy_key, net_profit_usd, rejection_reason
    FROM scored_opportunities
    WHERE strategy_key IS NOT NULL
    ORDER BY opportunity_id, created_at DESC, id DESC
),
labeled AS (
    SELECT strategy_key,
           CASE
               WHEN rejection_reason IN ('non_positive_profit',
                                         'non_positive_net_usd',
                                         'non_positive_gross_usd',
                                         'kelly_negative_edge',
                                         'gas_floor_breach') THEN 0
               WHEN net_profit_usd > 0 THEN 1
               WHEN net_profit_usd <= 0 THEN 0
           END AS y
    FROM latest
)
INSERT INTO bayesian_priors
    (strategy_key, token_pair, observation_count, profitable_count, last_updated)
SELECT strategy_key,
       NULL::text,
       COUNT(y),
       COUNT(y) FILTER (WHERE y = 1),
       now()
FROM labeled
GROUP BY strategy_key
ON CONFLICT (strategy_key) WHERE strategy_key IS NOT NULL DO UPDATE SET
    observation_count = EXCLUDED.observation_count,
    profitable_count  = EXCLUDED.profitable_count,
    last_updated      = EXCLUDED.last_updated
"#;

const READ_SQL: &str = r#"
SELECT strategy_key, observation_count, profitable_count
FROM bayesian_priors
WHERE strategy_key IS NOT NULL AND observation_count > 0
"#;

/// Prune strategy-keyed rows whose source rows aged out of retention: the
/// UPSERT alone never deletes, so without this a strategy's counts would
/// FREEZE forever at their last value once scored_opportunities drops its
/// rows (a8 review WARN 2026-09-20). Legacy token_pair-keyed rows
/// (strategy_key IS NULL) are not this writer's business.
const PRUNE_SQL: &str = r#"
DELETE FROM bayesian_priors bp
WHERE bp.strategy_key IS NOT NULL
  AND NOT EXISTS (
      SELECT 1 FROM scored_opportunities so
      WHERE so.strategy_key = bp.strategy_key
  )
"#;

/// Writer gate (stage2_calibration precedent: PG mutations are
/// operator-flipped, never implicit). Default OFF.
pub(crate) fn mode_enabled(v: Option<&str>) -> bool {
    matches!(
        v.map(|s| s.trim().to_ascii_lowercase()).as_deref(),
        Some("on") | Some("1") | Some("true")
    )
}

/// Pure: DB rows (strategy_key, observation_count, profitable_count) → the
/// in-memory prior map. Rows with empty key or obs ≤ 0 (zero or corrupt
/// negative) are dropped (None ⇒ flat prior); profitable is clamped to
/// [0, obs] (never a Beta with β < 1). Pure — unit-testable, no I/O.
pub(crate) fn build_map(rows: Vec<(String, i64, i64)>) -> HashMap<String, PriorState> {
    let mut m = HashMap::with_capacity(rows.len());
    for (key, obs, prof) in rows {
        if key.is_empty() || obs <= 0 {
            continue;
        }
        let prof = prof.clamp(0, obs);
        m.insert(
            key,
            PriorState {
                observation_count: obs as u64,
                profitable_count: prof as u64,
                log_odds: 0.0,
            },
        );
    }
    m
}

/// Change-detect on the two counts (PriorState has no PartialEq — log_odds
/// is dead code and f64 equality is noise; comparing the honest counts is
/// exactly the change we log).
fn map_changed(cur: &HashMap<String, PriorState>, next: &HashMap<String, PriorState>) -> bool {
    cur.len() != next.len()
        || cur.iter().any(|(k, v)| match next.get(k) {
            Some(n) => {
                n.observation_count != v.observation_count
                    || n.profitable_count != v.profitable_count
            }
            None => true,
        })
}

/// Read handle shared with the emitter. `None` inside = no calibration
/// (mode off, PG not configured, PG down since boot, or table empty).
#[derive(Clone)]
pub struct BetaPriorsCache {
    inner: Arc<RwLock<Option<HashMap<String, PriorState>>>>,
}

impl BetaPriorsCache {
    /// Spawn the periodic consolidate+refresh task over an existing pool.
    /// Honors `ARBX_BETA_PRIORS_MODE` (default off ⇒ `disabled()`).
    pub fn spawn(pool: PgPool) -> Self {
        if !mode_enabled(std::env::var("ARBX_BETA_PRIORS_MODE").ok().as_deref()) {
            return Self::disabled();
        }
        let cache = Self {
            inner: Arc::new(RwLock::new(None)),
        };
        let refresh = cache.clone();
        // Operator decision 2026-09-20 ("Refresh 300s"): scored_opportunities
        // measured at 4.77M retained rows (~162K/h) — a full-recompute
        // aggregate every 300s; per-pass PG cost is UNMEASURED (a8 review
        // NOTE 2026-09-20) — bounded by retention, gated OFF by default,
        // cadence operator-tunable.
        let refresh_secs = std::env::var("ARBX_BETA_PRIORS_REFRESH_SECS")
            .ok()
            .and_then(|v| v.parse::<u64>().ok())
            .filter(|s| *s >= 5)
            .unwrap_or(300);
        tokio::spawn(async move {
            let mut ticker = tokio::time::interval(Duration::from_secs(refresh_secs));
            ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
            info!(event = "beta_priors.spawned", refresh_secs);
            loop {
                ticker.tick().await;
                if let Err(e) = refresh_once(&pool, &refresh).await {
                    // warn (not debug): a failed refresh means the map goes
                    // stale (get() silently serves last-good / flat priors) —
                    // one line per refresh period max, R9-safe (a8 review
                    // NOTE 2026-09-20: silent degradation was invisible).
                    warn!(event = "beta_priors.refresh_failed", error = %e);
                }
            }
        });
        cache
    }

    /// No-writer/no-PG constructor: permanently-None (honest flat prior).
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

    /// Test-only constructor with a fixed map.
    #[cfg(test)]
    pub(crate) fn from_map(m: Option<HashMap<String, PriorState>>) -> Self {
        Self {
            inner: Arc::new(RwLock::new(m)),
        }
    }

    /// This strategy's calibrated state; `None` = flat prior. Cheap: one map
    /// entry clone under a micro-lock (hot-path safe).
    pub fn get(&self, strategy_key: &str) -> Option<PriorState> {
        self.inner
            .read()
            .ok()
            .and_then(|g| g.as_ref()?.get(strategy_key).cloned())
    }
}

/// One cycle: consolidate (upsert + prune) then read back. Overwrite the map
/// only when content changed (change-detect, priors_cache pattern).
async fn refresh_once(pool: &PgPool, cache: &BetaPriorsCache) -> anyhow::Result<()> {
    sqlx::query(CONSOLIDATE_SQL).execute(pool).await?;
    sqlx::query(PRUNE_SQL).execute(pool).await?;

    let rows: Vec<(String, i64, i64)> = sqlx::query_as(READ_SQL).fetch_all(pool).await?;
    let next = build_map(rows);

    {
        let mut g = cache
            .inner
            .write()
            .map_err(|_| anyhow::anyhow!("beta_priors lock poisoned"))?;
        let changed = match g.as_ref() {
            None => !next.is_empty(),
            Some(cur) => map_changed(cur, &next),
        };
        if changed {
            let strategies = next.len();
            *g = Some(next);
            info!(event = "beta_priors.updated", strategies);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scoring_pipeline::PriorState;

    #[test]
    fn mode_off_by_default_and_truthy_values_only() {
        assert!(!mode_enabled(None));
        assert!(!mode_enabled(Some("")));
        assert!(!mode_enabled(Some("off")));
        assert!(!mode_enabled(Some("false")));
        assert!(mode_enabled(Some("on")));
        assert!(mode_enabled(Some("1")));
        assert!(mode_enabled(Some("true")));
        assert!(mode_enabled(Some("TRUE"))); // case-insensitive
    }

    #[test]
    fn build_map_keeps_only_observed_strategies() {
        let m = build_map(vec![
            ("carb_tri_weth_01".into(), 100, 80),
            ("engine_dex_arb_v3v3".into(), 5, 0),
            ("zero_obs".into(), 0, 0), // filtered: no honest observations
        ]);
        assert_eq!(m.len(), 2);
        let p = m.get("carb_tri_weth_01").expect("present");
        assert_eq!(p.observation_count, 100);
        assert_eq!(p.profitable_count, 80);
        // 5 obs / 0 wins kept — the honest all-losses strategy.
        assert_eq!(m["engine_dex_arb_v3v3"].observation_count, 5);
    }

    #[test]
    fn build_map_drops_negative_db_counts() {
        // BIGINT underflow / corrupted row defense: a negative observation
        // count is not honest data — drop (⇒ None ⇒ flat prior), never panic,
        // never fabricate a saturated row.
        let m = build_map(vec![("weird".into(), -3, -1)]);
        assert!(m.is_empty());
        assert!(!m.contains_key("weird"));
    }

    #[test]
    fn build_map_caps_profitable_at_observations() {
        let m = build_map(vec![("cap".into(), 10, 25)]);
        assert_eq!(m["cap"].profitable_count, 10);
    }

    #[test]
    fn cache_disabled_stays_none() {
        // OFF-parity invariance (a8 #4, §34.1): mode OFF ⇒ cache None ⇒
        // `get()` None for EVERY key ⇒ the 5th arg of
        // evaluate_paper_opportunity is EXACTLY None — byte-parity with the
        // pre-module emit path. This test IS the no-regression proof.
        let c = BetaPriorsCache::disabled();
        assert!(c.get("anything").is_none());
        assert!(c.get("").is_none());
    }

    #[test]
    fn cache_from_map_clones_entry() {
        let mut m = std::collections::HashMap::new();
        m.insert(
            "s".to_string(),
            PriorState {
                observation_count: 7,
                profitable_count: 3,
                log_odds: 0.0,
            },
        );
        let c = BetaPriorsCache::from_map(Some(m));
        let p = c.get("s").expect("present");
        assert_eq!((p.observation_count, p.profitable_count), (7, 3));
        assert!(c.get("absent").is_none());
    }
}
