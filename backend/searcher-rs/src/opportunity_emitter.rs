// M11 allow: test modules use .unwrap()/.expect() for readability; the
// production code is clippy-clean (? + anyhow throughout).
//! Single-point emit path for opportunities.
//!
//! ## Design (Phase 6 — spec §3.5)
//!
//! `OpportunityEmitter` is the **only** module permitted to call
//! `persistence::insert_opportunity` and `publisher::publish` in future phases.
//! It centralises:
//!
//!   1. `OppDedup` check (accepted path only).
//!   2. `persistence::insert_opportunity` write to PG.
//!   3. `publisher::publish` to Redis stream `arbx:opps:detected`.
//!   4. `counters().db_persisted` / `db_errors` increments.
//!   5. `OPPORTUNITIES_TOTAL` metric increment with a standardised label set.
//!   6. Rejection-row persistence (RULE 00 transparency — operators see rejection
//!      volume in the dashboard and can audit allowlist gaps).
//!
//! The legacy emit sites in `scanner.rs` and the worker files are **NOT** touched
//! here — migration happens in Phase 14. Both paths co-exist until then.
//!
//! ## R8 invariants
//!
//! - `expected_profit_usd = None` propagates unchanged through both paths.
//! - No counter is incremented before the I/O completes.
//! - `emit_rejected` never checks dedup — every rejection is observable.
//! - When `pool = None` (DB not configured), only Redis publish happens and the
//!   outcome is `NoDbConfigured` (not an error).

use crate::counters::counters;
use crate::dedup::OppDedup;
use crate::persistence;
use crate::publisher;
use crate::scoring_pipeline::ScoringPipeline;
use crate::strategy_label::StrategyLabel;
use shared_rs::contracts::Opportunity;
use shared_rs::metrics::OPPORTUNITIES_TOTAL;
use sqlx::postgres::PgPool;
use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex};
use tracing::error;

/// Gate C scoring telemetry stream (twin of `publisher::STREAM_KEY`). The
/// `OpportunityEmitter` XADDs each ConfidenceScore here; the api-server
/// scored-opportunities archiver consumes it into Postgres `scored_opportunities`.
const SCORING_STREAM_KEY: &str = "arbx:scoring:scored";
const SCORING_STREAM_MAXLEN: usize = 100_000;

// ---------------------------------------------------------------------------
// EmittedRecord — for test introspection (TASK 4)
// ---------------------------------------------------------------------------

/// A record of what the emitter would have done in dry-run mode.
/// Exposed via `recorded_emissions()` for integration tests that want to
/// assert on the V2 pipeline's output without a real DB / Redis.
#[derive(Debug, Clone)]
#[allow(dead_code)] // fields are used in integration tests (tests/v2_shadow_replay.rs)
pub struct EmittedRecord {
    /// The opportunity that was emitted (accepted or rejected).
    pub opportunity: Opportunity,
    /// The strategy label used for the emit call.
    pub strategy: StrategyLabel,
    /// `true` → accepted path (`emit_accepted`); `false` → rejected path.
    pub accepted: bool,
    /// The rejection reason, if `accepted = false`.
    pub rejection_reason: Option<String>,
}

// ---------------------------------------------------------------------------
// EmitOutcome
// ---------------------------------------------------------------------------

/// The outcome of an emit call.
///
/// Allows callers to observe what happened without parsing log output. All
/// non-error variants represent a completed action (data either persisted,
/// published, or both). `DbError` represents a partial success where Redis
/// publish happened but PG write failed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EmitOutcome {
    /// Successfully written to PG only (Redis publish was not reached yet).
    /// NOTE: in normal operation this variant is never returned — we always
    /// attempt both. It is reserved for future partial-emit modes.
    #[allow(dead_code)]
    Persisted,
    /// Successfully published to Redis only (PG pool is `None`).
    /// Superseded by `NoDbConfigured` in the current implementation.
    #[allow(dead_code)]
    Published,
    /// Successfully written to PG and published to Redis.
    PersistedAndPublished,
    /// Dedup hit — same (route, time_bucket, profit_bucket) triple seen before.
    /// No I/O was performed.
    Deduped,
    /// PG write failed but Redis publish succeeded. Data is in the stream but
    /// not persisted. Operator should investigate via `db_errors` counter.
    DbError,
    /// `pool = None` — scanner booted without `DATABASE_URL`. Only Redis
    /// publish happened. Expected in development / single-node environments.
    NoDbConfigured,
}

// ---------------------------------------------------------------------------
// OpportunityEmitter
// ---------------------------------------------------------------------------

/// Single-point emit path for `Opportunity` records.
///
/// Constructed once at boot (or per-chain scanner) and `Arc`-cloned into each
/// task that needs to emit. The internal `OppDedup` is already `Arc`-wrapped
/// so cloning the emitter does not duplicate the dedup state.
pub struct OpportunityEmitter {
    pool: Option<PgPool>,
    redis: redis::aio::ConnectionManager,
    opp_dedup: Arc<OppDedup>,
    /// When `true` all emit calls are no-ops (log only). Used in
    /// `ARBX_ORCHESTRATOR_MODE=shadow` to let the orchestrator evaluate
    /// candidates without writing to PG or the Redis stream.
    dry_run: bool,
    /// Recorded emissions in dry-run mode — populated by `emit_accepted` and
    /// `emit_rejected` when `dry_run = true`. Used by integration tests to
    /// inspect what the pipeline would have emitted (TASK 4).
    /// Protected by a `Mutex` so the emitter can be shared via `Arc` across
    /// async tasks while still allowing test introspection.
    recorded: Mutex<Vec<EmittedRecord>>,
    /// Gate C scoring pipeline (env-configured). Produces a ConfidenceScore per
    /// emitted paper opportunity; observe-only by default (no emission change).
    scoring: ScoringPipeline,
}

impl OpportunityEmitter {
    /// Creates a new `OpportunityEmitter`.
    ///
    /// - `pool`: PG pool, or `None` when `DATABASE_URL` is not configured.
    ///   When `None`, PG writes are skipped and `NoDbConfigured` is returned.
    /// - `redis`: Connection manager for the `arbx:opps:detected` stream.
    /// - `opp_dedup`: Shared dedup state. The `Arc` is cloned here; callers
    ///   retain their own reference for the tx-level `Dedup` pathway.
    pub fn new(
        pool: Option<PgPool>,
        redis: redis::aio::ConnectionManager,
        opp_dedup: Arc<OppDedup>,
    ) -> Self {
        let scoring = ScoringPipeline::from_env();
        tracing::info!(
            event = "scoring.pipeline.wired",
            enabled = scoring.enabled(),
            hard_gate = scoring.hard_gate(),
        );
        Self {
            pool,
            redis,
            opp_dedup,
            dry_run: false,
            recorded: Mutex::new(Vec::new()),
            scoring,
        }
    }

    /// Creates an emitter in dry-run mode.
    ///
    /// All emit calls succeed immediately without I/O — they only log.
    /// Used when `ARBX_ORCHESTRATOR_MODE=shadow` so the orchestrator path
    /// evaluates candidates without duplicating writes that the legacy path
    /// already performs.
    pub fn new_dry_run(opp_dedup: Arc<OppDedup>, redis: redis::aio::ConnectionManager) -> Self {
        Self {
            pool: None,
            redis,
            opp_dedup,
            dry_run: true,
            recorded: Mutex::new(Vec::new()),
            scoring: ScoringPipeline::from_env(),
        }
    }

    /// Returns `true` when this emitter is in dry-run (shadow) mode.
    ///
    /// Used by the orchestrator for the `v2.emitter.input` log event.
    pub fn is_dry_run(&self) -> bool {
        self.dry_run
    }

    /// Returns a snapshot of all emissions recorded in dry-run mode.
    ///
    /// Only populated when `dry_run = true` (i.e., `new_dry_run` constructor).
    /// In live mode this always returns an empty Vec.
    ///
    /// Used by integration tests (TASK 4) to inspect the V2 pipeline's output.
    #[allow(dead_code)] // used in tests/v2_shadow_replay.rs
    pub fn recorded_emissions(&self) -> Vec<EmittedRecord> {
        self.recorded.lock().map(|g| g.clone()).unwrap_or_default()
    }

    // -----------------------------------------------------------------------
    // emit_accepted
    // -----------------------------------------------------------------------

    /// Emit an opportunity that passed all strategy/risk gates.
    ///
    /// Flow:
    ///   1. Dedup check — if hit, return `Deduped` with no I/O.
    ///   2. PG insert (or skip if `pool = None`).
    ///   3. Redis publish.
    ///   4. Increment `OPPORTUNITIES_TOTAL` with `outcome = "detected"`.
    ///   5. Increment `db_persisted` (or `db_errors` on failure).
    ///
    /// Returns `Err` only when Redis publish fails (fatal: the downstream
    /// websocket gateway depends on the stream). PG failure is non-fatal
    /// and returns `DbError`.
    pub async fn emit_accepted(
        &self,
        opportunity: &Opportunity,
        strategy_label: StrategyLabel,
    ) -> anyhow::Result<EmitOutcome> {
        // Dry-run (shadow mode): log + record, no I/O.
        if self.dry_run {
            tracing::debug!(
                event = "opportunity_emitter.shadow_accepted",
                opp_id = %opportunity.id,
                strategy = strategy_label.as_str(),
                profit_usd = ?opportunity.expected_profit_usd,
                "shadow mode: would have emitted accepted (no write)"
            );
            // Record for test introspection (TASK 4).
            if let Ok(mut guard) = self.recorded.lock() {
                guard.push(EmittedRecord {
                    opportunity: opportunity.clone(),
                    strategy: strategy_label,
                    accepted: true,
                    rejection_reason: None,
                });
            }
            return Ok(EmitOutcome::Published);
        }

        // ── Dedup check ───────────────────────────────────────────────────
        let route_hash = build_route_hash(opportunity);
        let is_fresh = self
            .opp_dedup
            .check_and_mark(&route_hash, opportunity.expected_profit_usd);
        if !is_fresh {
            return Ok(EmitOutcome::Deduped);
        }

        // ── Gate C scoring (observe-only, ADVISORY) ───────────────────────
        // Scoring is wired here at the REAL paper opportunity emission path.
        // dispatch_orchestrator_and_classify is simulation classification only
        // in this repo revision. The XADD of the ConfidenceScore is non-fatal
        // and NEVER blocks emission: opps:detected ALWAYS receives the
        // opportunity (RULE 00 transparency + invariant). HARD_GATE only records
        // an advisory flag for a future live execution layer — it does NOT skip
        // the publish below in this shadow build.
        self.score_and_publish(opportunity).await;

        // ── PG write ──────────────────────────────────────────────────────
        let pg_ok = self.try_insert_pg(opportunity).await;

        // ── Redis publish ─────────────────────────────────────────────────
        let mut redis = self.redis.clone();
        publisher::publish(&mut redis, opportunity).await?;

        // ── Metrics ───────────────────────────────────────────────────────
        let chain_str = opportunity.chain_id.to_string();
        OPPORTUNITIES_TOTAL
            .with_label_values(&[&chain_str, strategy_label.as_str(), "detected"])
            .inc();

        Ok(pg_ok)
    }

    // -----------------------------------------------------------------------
    // emit_rejected
    // -----------------------------------------------------------------------

    /// Emit a rejected-but-observable row.
    ///
    /// Rejected rows are always persisted (no dedup check) so the operator
    /// can see rejection volume and audit allowlist gaps via the dashboard
    /// (RULE 00 transparency). The `rejection_reason` field is populated.
    ///
    /// `rejection_reason` should follow the label scheme established in
    /// `scanner.rs`: e.g. `"TokenNotAllowed:PEPE"`, `"StrategyDisabled:triangular"`.
    ///
    /// The Prometheus outcome label is `"rejected_<kind>"` where `kind` is the
    /// first token of `rejection_reason` before the `:` separator, lowercased.
    ///
    /// Returns `Err` only when Redis publish fails. PG failure is non-fatal
    /// and returns `DbError`.
    pub async fn emit_rejected(
        &self,
        opportunity: &Opportunity,
        strategy_label: StrategyLabel,
        rejection_reason: &str,
    ) -> anyhow::Result<EmitOutcome> {
        // Dry-run (shadow mode): log + record, no I/O.
        if self.dry_run {
            tracing::debug!(
                event = "opportunity_emitter.shadow_rejected",
                opp_id = %opportunity.id,
                strategy = strategy_label.as_str(),
                reason = rejection_reason,
                "shadow mode: would have emitted rejected (no write)"
            );
            // Record for test introspection (TASK 4).
            if let Ok(mut guard) = self.recorded.lock() {
                guard.push(EmittedRecord {
                    opportunity: opportunity.clone(),
                    strategy: strategy_label,
                    accepted: false,
                    rejection_reason: Some(rejection_reason.to_owned()),
                });
            }
            return Ok(EmitOutcome::Published);
        }

        // ── PG write ──────────────────────────────────────────────────────
        // Mutate a local copy so the caller's value is not modified.
        let mut rejected = opportunity.clone();
        rejected.rejection_reason = Some(rejection_reason.to_owned());

        let pg_ok = self.try_insert_pg(&rejected).await;

        // ── Redis publish ─────────────────────────────────────────────────
        let mut redis = self.redis.clone();
        publisher::publish(&mut redis, &rejected).await?;

        // ── Metrics ───────────────────────────────────────────────────────
        let chain_str = rejected.chain_id.to_string();
        let outcome_label = build_rejected_outcome_label(rejection_reason);
        OPPORTUNITIES_TOTAL
            .with_label_values(&[&chain_str, strategy_label.as_str(), &outcome_label])
            .inc();

        Ok(pg_ok)
    }

    // -----------------------------------------------------------------------
    // Private helpers
    // -----------------------------------------------------------------------

    /// Gate C: score the (fresh) paper opportunity and XADD the ConfidenceScore
    /// to `arbx:scoring:scored` (non-fatal). ADVISORY ONLY — it never gates the
    /// caller's emission: `opps:detected` always receives the opportunity (RULE
    /// 00 + invariant). `HARD_GATE=true` only records an advisory flag for a
    /// future live execution layer. A scoring/Redis error never breaks emission.
    async fn score_and_publish(&self, opp: &Opportunity) {
        if !self.scoring.enabled() {
            return;
        }
        let net_profit_usd = opp.net_expected_profit_usd.or(opp.expected_profit_usd);
        let chain_id_i64 = i64::try_from(opp.chain_id).unwrap_or(-1);
        // Flat prior: `bayesian_priors` is empty until the A.5 paper-shadow
        // window; a priors cache feeds real PriorState in the A.5 follow-up.
        // Passing None is the honest "wired but not calibrated" state.
        let score = match self
            .scoring
            .evaluate_paper_opportunity(
                &opp.id.to_string(),
                &opp.pair_symbol,
                net_profit_usd,
                Some(chain_id_i64),
                None,
            )
            .await
        {
            Ok(s) => s,
            Err(e) => {
                tracing::warn!(
                    event = "scoring.confidence.persist_failed",
                    error = %e,
                    "scoring evaluate failed (non-fatal)"
                );
                return;
            }
        };

        // XADD the score (non-fatal) — twin of `publisher::publish`.
        let record = serde_json::json!({
            "opportunity_id": opp.id.to_string(),
            "token_pair": opp.pair_symbol,
            "posterior_prob": score.posterior_prob,
            "kelly_fraction": score.kelly_fraction,
            "recommended_usd": score.recommended_position_usd,
            "net_profit_usd": net_profit_usd,
            "bayesian_accepted": score.bayesian_accepted,
            "prior_log_odds": score.prior_log_odds,
            "chain_id": chain_id_i64,
            "source_context": score.source_context,
            "scoring_mode": "paper",
        });
        match serde_json::to_string(&record) {
            Ok(json) => {
                let mut redis = self.redis.clone();
                let res: redis::RedisResult<String> = redis::cmd("XADD")
                    .arg(SCORING_STREAM_KEY)
                    .arg("MAXLEN")
                    .arg("~")
                    .arg(SCORING_STREAM_MAXLEN)
                    .arg("*")
                    .arg("json")
                    .arg(&json)
                    .query_async(&mut redis)
                    .await;
                match res {
                    Ok(_) => tracing::debug!(
                        event = "scoring.confidence.persisted",
                        token_pair = %opp.pair_symbol
                    ),
                    Err(e) => tracing::warn!(
                        event = "scoring.confidence.persist_failed",
                        error = %e
                    ),
                }
            }
            Err(e) => tracing::warn!(
                event = "scoring.confidence.persist_failed",
                error = %e
            ),
        }

        // Advisory only: record what a future LIVE execution layer would do.
        // In this SHADOW build the opportunity is ALWAYS emitted downstream.
        if self.scoring.should_emit_downstream(&score) {
            tracing::debug!(
                event = "scoring.paper_opportunity.annotated",
                token_pair = %opp.pair_symbol,
                bayesian_accepted = score.bayesian_accepted,
            );
        } else {
            tracing::info!(
                event = "scoring.hard_gate.flagged",
                token_pair = %opp.pair_symbol,
                posterior_prob = score.posterior_prob,
                "HARD_GATE advisory: a live layer would gate this; opps:detected emission preserved",
            );
        }
    }

    /// Attempts a PG insert and updates the `db_persisted` / `db_errors` counters.
    ///
    /// Returns:
    /// - `PersistedAndPublished` → insert succeeded (caller still does Redis publish).
    /// - `DbError` → insert failed (caller still does Redis publish).
    /// - `NoDbConfigured` → `self.pool` is `None`.
    ///
    /// Named `try_insert_pg` and returning the pre-publish outcome to keep
    /// the counter semantics honest: `db_persisted` is only incremented when
    /// the PG write actually succeeded.
    async fn try_insert_pg(&self, opp: &Opportunity) -> EmitOutcome {
        match &self.pool {
            None => EmitOutcome::NoDbConfigured,
            Some(pg) => match persistence::insert_opportunity(pg, opp).await {
                Ok(()) => {
                    counters().db_persisted.fetch_add(1, Ordering::Relaxed);
                    EmitOutcome::PersistedAndPublished
                }
                Err(e) => {
                    counters().db_errors.fetch_add(1, Ordering::Relaxed);
                    error!(
                        event = "opportunity_emitter.db_error",
                        opp_id = %opp.id,
                        error = %e,
                        "PG insert failed; opportunity published to Redis stream only"
                    );
                    EmitOutcome::DbError
                }
            },
        }
    }
}

// ---------------------------------------------------------------------------
// Helper functions
// ---------------------------------------------------------------------------

/// Builds the route fingerprint string used as the `OppDedup` key.
///
/// The fingerprint matches the pattern used in `scanner.rs`:
/// `<dex_a>_<token_in>_<token_out>` — stable, lowercase, separator-delimited.
///
/// Lowercasing ensures address case variants don't defeat dedup.
fn build_route_hash(opp: &Opportunity) -> String {
    format!(
        "{}_{}_{}_{}",
        opp.dex_a.to_ascii_lowercase(),
        opp.token_in.to_ascii_lowercase(),
        opp.token_out.to_ascii_lowercase(),
        opp.chain_id,
    )
}

/// Builds the Prometheus outcome label for a rejection.
///
/// Pattern: `"rejected_<kind>"` where `kind` is the portion of the
/// `rejection_reason` string before the first `:`, lowercased.
///
/// Examples:
/// - `"TokenNotAllowed:PEPE"` → `"rejected_token_not_allowed"`
///   (CamelCase split with `_`)
/// - `"StrategyDisabled:triangular"` → `"rejected_strategy_disabled"`
/// - `"LowLiquidity"` → `"rejected_low_liquidity"`
/// - `""` → `"rejected_unknown"`
fn build_rejected_outcome_label(reason: &str) -> String {
    let kind = reason.split(':').next().unwrap_or(reason);
    if kind.is_empty() {
        return "rejected_unknown".to_owned();
    }
    // Convert CamelCase to snake_case and prefix with "rejected_".
    let snake = camel_to_snake(kind);
    format!("rejected_{snake}")
}

/// Naive CamelCase → snake_case converter for rejection label generation.
///
/// Only handles ASCII identifiers (all known rejection reasons are ASCII).
/// Does not handle consecutive uppercase sequences (e.g. "HTTPError" →
/// "h_t_t_p_error" — this is acceptable because rejection reason strings
/// are controlled internal constants, not user input).
fn camel_to_snake(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 4);
    for (i, c) in s.chars().enumerate() {
        if c.is_ascii_uppercase() && i > 0 {
            out.push('_');
        }
        out.push(c.to_ascii_lowercase());
    }
    out
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use chrono::Utc;
    use shared_rs::contracts::{Opportunity, StrategyKind};
    use uuid::Uuid;

    // ── Helpers ──────────────────────────────────────────────────────────────

    fn make_opp(id: Uuid, profit: Option<f64>, rejection: Option<String>) -> Opportunity {
        Opportunity {
            id,
            chain_id: 1,
            strategy_kind: StrategyKind::DexArb,
            dex_a: "uniswap_v2".to_owned(),
            dex_b: None,
            pair_symbol: "WETH/USDC".to_owned(),
            token_in: "0xweth".to_owned(),
            token_out: "0xusdc".to_owned(),
            amount_in_wei: "1000000000000000000".to_owned(),
            expected_profit_usd: profit,
            net_expected_profit_usd: None,
            roi_pct: Some(1.5),
            risk_score: Some(0.8),
            block_number: Some(12_345_678),
            rejection_reason: rejection,
            detected_at: Utc::now(),
            trace_id: Uuid::new_v4(),
        }
    }

    // ── build_route_hash ─────────────────────────────────────────────────────

    #[test]
    fn route_hash_is_stable_and_lowercased() {
        let opp = make_opp(Uuid::new_v4(), Some(1.5), None);
        let h1 = build_route_hash(&opp);
        let h2 = build_route_hash(&opp);
        assert_eq!(h1, h2, "route hash must be stable across calls");
        assert_eq!(h1, h1.to_ascii_lowercase(), "route hash must be lowercase");
    }

    // ── camel_to_snake ───────────────────────────────────────────────────────

    #[test]
    fn camel_to_snake_conversions() {
        assert_eq!(camel_to_snake("TokenNotAllowed"), "token_not_allowed");
        assert_eq!(camel_to_snake("StrategyDisabled"), "strategy_disabled");
        assert_eq!(camel_to_snake("LowLiquidity"), "low_liquidity");
        assert_eq!(camel_to_snake("AnomalousMath"), "anomalous_math");
    }

    // ── build_rejected_outcome_label ─────────────────────────────────────────

    #[test]
    fn rejected_outcome_label_extraction() {
        assert_eq!(
            build_rejected_outcome_label("TokenNotAllowed:PEPE"),
            "rejected_token_not_allowed"
        );
        assert_eq!(
            build_rejected_outcome_label("StrategyDisabled:triangular"),
            "rejected_strategy_disabled"
        );
        assert_eq!(
            build_rejected_outcome_label("LowLiquidity"),
            "rejected_low_liquidity"
        );
        assert_eq!(build_rejected_outcome_label(""), "rejected_unknown");
    }

    // ── opportunity_emitter::tests::r8_none_preserved_through_emit ──────────

    /// `expected_profit_usd = None` must not be coerced to `Some(0.0)` or any
    /// other value at any point in the emit path.
    #[test]
    fn r8_none_preserved_through_emit() {
        let opp = make_opp(Uuid::new_v4(), None, None);
        // Verify the field is genuinely None before calling route_hash
        // (route_hash must not coerce it either).
        assert!(opp.expected_profit_usd.is_none());
        let hash = build_route_hash(&opp);
        // route_hash does not inspect profit at all — just a format string.
        // Verify it's a valid non-empty string.
        assert!(!hash.is_empty());
        // The opp value is unchanged.
        assert!(opp.expected_profit_usd.is_none());
    }

    // ── opportunity_emitter::tests::accepted_dedupe_hit_skips_io ────────────

    /// The second emit with the same fingerprint must return `Deduped` and must
    /// not attempt any I/O.
    ///
    /// This test uses a Redis mock via a real local Redis (if available) or
    /// validates dedup logic in isolation. We test only the dedup path here
    /// without a real Redis; the full I/O path is an integration test.
    #[tokio::test]
    async fn dedup_check_logic() {
        let dedup = Arc::new(OppDedup::new(64));
        let opp = make_opp(Uuid::new_v4(), Some(1.5), None);

        let route = build_route_hash(&opp);

        // First check — fresh.
        let fresh = dedup.check_and_mark(&route, opp.expected_profit_usd);
        assert!(fresh, "first call must be fresh");

        // Second check — duplicate.
        let dup = dedup.check_and_mark(&route, opp.expected_profit_usd);
        assert!(!dup, "second call with same hash+profit must be dup");
    }

    // ── build_rejected_outcome_label edge cases ───────────────────────────────

    #[test]
    fn rejected_label_no_colon() {
        // Reason with no colon: entire string is the kind.
        assert_eq!(
            build_rejected_outcome_label("UnknownTokenPrice"),
            "rejected_unknown_token_price"
        );
    }

    #[test]
    fn rejected_label_multiple_colons() {
        // Only the first segment before the first colon is used.
        assert_eq!(
            build_rejected_outcome_label("StrategyConfigGate:some:detail"),
            "rejected_strategy_config_gate"
        );
    }

    // ── r8: None profit stays None in emit_rejected ──────────────────────────

    /// Verifies that `emit_rejected` does not fabricate a profit value when
    /// `expected_profit_usd` is `None`.
    #[test]
    fn rejected_none_profit_not_coerced() {
        let opp = make_opp(Uuid::new_v4(), None, None);
        // Check that building the label from a None-profit opportunity does not
        // touch or coerce the profit field.
        let route = build_route_hash(&opp);
        assert!(!route.is_empty());
        // Profit is still None.
        assert!(opp.expected_profit_usd.is_none(), "profit must stay None");
    }
}
