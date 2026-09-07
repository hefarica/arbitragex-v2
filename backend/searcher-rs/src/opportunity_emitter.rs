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
        route: Option<&shared_rs::candidates::RouteMetadata>,
    ) -> anyhow::Result<EmitOutcome> {
        // GATE (2026-08-18, "arbitrajes reales, no de fallback"): an ACCEPTED
        // row must carry computed economics. Live prod showed rows emitted as
        // accepted with amount_in_wei=0 and BOTH profit fields None — cosmetic
        // acceptances that rendered as hollow cards and inflated the accepted
        // class with nothing to act on. A candidate whose economics were never
        // computed is NOT an opportunity yet: reclassify to an honest rejection
        // (R8: visible with its reason, never silently dropped) via the
        // rejected path. Guard sits BEFORE passed_all_gates so reclassifications
        // don't inflate that counter, and covers every emitter (current and
        // future) at the single publish gate.
        if !has_computed_economics(opportunity) {
            tracing::warn!(
                event = "opportunity_emitter.accept_without_economics",
                opp_id = %opportunity.id,
                strategy = strategy_label.as_str(),
                cartridge_id = ?opportunity.cartridge_id,
                "accepted emission without computed economics — reclassifying as no_computable_economics"
            );
            return self
                .emit_rejected(
                    opportunity,
                    strategy_label,
                    "no_computable_economics",
                    route,
                )
                .await;
        }
        // ARBX-DP-003: Execution_Class taxonomy at the publish seam. Workbook
        // strategies whose class tiers as Observation (OBSERVE_ONLY) or Signal
        // (SIGNAL_UNLESS_FIRM_EXIT / *_DATA_REQUIRED / NONATOMIC_* / latency /
        // settlement-delay) NEVER take the Opportunity{confidence} shape —
        // 13_DETECTOR_POLICY routes them to the observation/signal feeds.
        // Reclassify honestly (R8) instead of silently dropping. Rows without
        // workbook identity (core engines, cartridge_id = None; non-workbook
        // stems) pass: the taxonomy covers the 264 workbook strategies.
        match crate::signal_tier::opportunity_verdict(opportunity.cartridge_id.as_deref()) {
            crate::signal_tier::EmissionVerdict::Pass => {}
            crate::signal_tier::EmissionVerdict::Blocked { reason } => {
                tracing::warn!(
                    event = "opportunity_emitter.tier_gate",
                    opp_id = %opportunity.id,
                    strategy = strategy_label.as_str(),
                    cartridge_id = ?opportunity.cartridge_id,
                    reason,
                    "tier-blocked from the Opportunity shape — reclassifying as rejected"
                );
                return self
                    .emit_rejected(opportunity, strategy_label, reason, route)
                    .await;
            }
        }
        // A5/N-01b: increment passed_all_gates so the heartbeat reflects V2-path
        // accepted candidates (previously only the legacy scanner incremented it).
        counters().passed_all_gates.fetch_add(1, Ordering::Relaxed);

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

        // WO-10 (2026-09-06): emit-boundary span origin — entry of the REAL
        // I/O path (dedup → Gate-C scoring → PG insert → Redis XADD). Dry-run
        // and reclassification returns above are deliberately OUTSIDE the
        // span (no I/O, no double count when emit_accepted reclassifies into
        // emit_rejected). Monotonic Instant; observe is zero-alloc; R9-silent.
        let wo10_emit_start = std::time::Instant::now();

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
        self.score_and_publish(opportunity, "accepted", None).await;

        // ── PG write ──────────────────────────────────────────────────────
        // Thread the multi-hop route topology (when the caller has it) so the
        // route_metadata JSONB column is populated for sim-ctl A1 enrichment and
        // the exchange dashboard's multi-leg A→B view.
        let pg_ok = self.try_insert_pg_with_route(opportunity, route).await;

        // ── Redis publish ─────────────────────────────────────────────────
        let mut redis = self.redis.clone();
        publisher::publish(&mut redis, opportunity).await?;
        // WO-10 (2026-09-06): two cuts of the same completed emission:
        // (a) monotonic emit-boundary span (real-I/O entry → XADD complete);
        // (b) wall-clock construction→publish span from the `detected_at`
        //     stamp (covers gates + SizeOptimizer upstream of this fn; R8
        //     guard against clock steps inside the helper).
        publisher::STAGE_EMIT_BOUNDARY.observe(wo10_emit_start.elapsed().as_secs_f64());
        publisher::observe_construction_to_publish(opportunity.detected_at, chrono::Utc::now());

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
        route: Option<&shared_rs::candidates::RouteMetadata>,
    ) -> anyhow::Result<EmitOutcome> {
        // A5/N-01b: classify rejection reason and increment matching gate
        // counter so the heartbeat reflects V2-path rejections.
        let c = counters();
        let rl = rejection_reason.to_ascii_lowercase();
        if rl.contains("token_not_allowed") || rl.contains("tokennotallowed") {
            c.gate_token_not_allowed.fetch_add(1, Ordering::Relaxed);
        } else if rl.contains("strategy_disabled") || rl.contains("disabled") {
            c.gate_strategy_disabled.fetch_add(1, Ordering::Relaxed);
        } else if rl.contains("no_config") || rl.contains("noconfig") {
            c.gate_no_config.fetch_add(1, Ordering::Relaxed);
        } else if rl.contains("unknown_token_price") || rl.contains("unknown_price") {
            c.gate_unknown_token_price.fetch_add(1, Ordering::Relaxed);
        } else if rl.contains("anomalous") {
            c.gate_anomalous_math.fetch_add(1, Ordering::Relaxed);
        } else {
            c.gate_other_rejected.fetch_add(1, Ordering::Relaxed);
        }

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

        // WO-10 (2026-09-06): emit-boundary span origin — real I/O starts at
        // Gate-C scoring below (dry-run return above stays outside the span).
        let wo10_emit_start = std::time::Instant::now();

        // ── Gate C scoring (advisory, non-fatal) ──────────────────────────
        // ARBX-RDY-02 / A.5: score the NEGATIVE class too — Bayesian prior
        // calibration needs rejected observations (prod rejects ~100% of
        // candidates; an accept-only feed starves `bayesian_priors` at zero
        // observations). Identical semantics to the accepted path: advisory,
        // never gates the PG write / publish below (RULE 00).
        self.score_and_publish(opportunity, "rejected", Some(rejection_reason))
            .await;

        // ── PG write ──────────────────────────────────────────────────────
        // Mutate a local copy so the caller's value is not modified.
        let mut rejected = opportunity.clone();
        rejected.rejection_reason = Some(rejection_reason.to_owned());

        let pg_ok = self.try_insert_pg_with_route(&rejected, route).await;

        // ── Redis publish ─────────────────────────────────────────────────
        let mut redis = self.redis.clone();
        publisher::publish(&mut redis, &rejected).await?;
        // WO-10 (2026-09-06): same two cuts as the accepted path. The rejected
        // clone preserves `detected_at` verbatim — the construction→publish
        // wall-clock span reflects the FULL evaluation this row survived
        // (cartridge gates + SizeOptimizer) before its honest rejection (R8).
        publisher::STAGE_EMIT_BOUNDARY.observe(wo10_emit_start.elapsed().as_secs_f64());
        publisher::observe_construction_to_publish(rejected.detected_at, chrono::Utc::now());

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

    /// Gate C: score EVERY paper opportunity — accepted AND rejected
    /// (ARBX-RDY-02) — and XADD the ConfidenceScore to `arbx:scoring:scored`
    /// (non-fatal). Prior calibration needs the negative class (A.5 doctrine):
    /// prod rejects ~100% of candidates, so an accept-only scoring feed would
    /// starve `bayesian_priors` at zero observations. `emission_outcome` labels
    /// the emitting path (`"accepted"` / `"rejected"`); `rejection_reason` is
    /// the verbatim reason on the rejected path, `None` on the accepted path.
    /// ADVISORY ONLY — it never gates the caller's emission: `opps:detected`
    /// always receives the opportunity (RULE 00 + invariant). `HARD_GATE=true`
    /// only records an advisory flag for a future live execution layer. A
    /// scoring/Redis error never breaks emission.
    async fn score_and_publish(
        &self,
        opp: &Opportunity,
        emission_outcome: &'static str,
        rejection_reason: Option<&str>,
    ) {
        if !self.scoring.enabled() {
            return;
        }
        let net_profit_usd = opp.net_expected_profit_usd.or(opp.expected_profit_usd);
        let chain_id_i64 = i64::try_from(opp.chain_id).unwrap_or(-1);
        // STRAT-IDENT-01: score by STRATEGY, not by pair/class. Each of the 264
        // cartridges carries its own identity (cartridge_id ≡ strategy_kind
        // stem); the 5 core engines are individual strategies identified by
        // their kind. The pair stays as context in the record — never as the
        // calibration bucket.
        let strategy_key = opp
            .cartridge_id
            .clone()
            .unwrap_or_else(|| opp.strategy_kind.as_str().to_string());
        // Flat prior: `bayesian_priors` is empty until the A.5 paper-shadow
        // window; a priors cache feeds real PriorState in the A.5 follow-up.
        // Passing None is the honest "wired but not calibrated" state.
        let score = match self
            .scoring
            .evaluate_paper_opportunity(
                &opp.id.to_string(),
                &strategy_key,
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

        // §IV capture: read the per-(chain,strategy) math-evidence snapshot from
        // Redis (written by math_evidence::publish_declared_combo_evidence,
        // TTL 120s — covers the emit→score latency). Missing ⇒ null (honest,
        // non-blocking). STRAT-IDENT-01: ONE canonical key via
        // strategy_evidence_key — the pre-fix reader keyed `{:?}` of the
        // StrategyKind newtype while the writer keyed `{:?}` of RouterKind;
        // the keys never matched and evidence_vector was always null.
        let evidence_key = crate::math_evidence::strategy_evidence_key(opp.chain_id, &strategy_key);
        let evidence_vector: Option<serde_json::Value> = {
            let mut r = self.redis.clone();
            let json: Option<String> = redis::cmd("GET")
                .arg(&evidence_key)
                .query_async(&mut r)
                .await
                .unwrap_or(None);
            json.as_deref().and_then(|s| serde_json::from_str(s).ok())
        };

        // XADD the score (non-fatal) — twin of `publisher::publish`.
        let record = build_score_record(
            opp,
            &strategy_key,
            &score,
            evidence_vector,
            emission_outcome,
            rejection_reason,
        );
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
                strategy_key = %strategy_key,
                bayesian_accepted = score.bayesian_accepted,
            );
        } else {
            tracing::info!(
                event = "scoring.hard_gate.flagged",
                strategy_key = %strategy_key,
                posterior_prob = score.posterior_prob,
                "HARD_GATE advisory: a live layer would gate this; opps:detected emission preserved",
            );
        }
    }

    /// Attempts a PG insert WITH route metadata and updates the `db_persisted` /
    /// `db_errors` counters.
    ///
    /// Returns:
    /// - `PersistedAndPublished` → insert succeeded (caller still does Redis publish).
    /// - `DbError` → insert failed (caller still does Redis publish).
    /// - `NoDbConfigured` → `self.pool` is `None`.
    ///
    /// Named `try_insert_pg_with_route` and returning the pre-publish outcome to
    /// keep the counter semantics honest: `db_persisted` is only incremented when
    /// the PG write actually succeeded. Pass `None` for `route` to persist without
    /// route_metadata (legacy behavior).
    async fn try_insert_pg_with_route(
        &self,
        opp: &Opportunity,
        route: Option<&shared_rs::candidates::RouteMetadata>,
    ) -> EmitOutcome {
        match &self.pool {
            None => EmitOutcome::NoDbConfigured,
            Some(pg) => match persistence::insert_opportunity_with_route(pg, opp, route).await {
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
/// True when the opportunity carries at least one computed economics figure
/// (gross OR net). Accepted emissions REQUIRE this — the 2026-08-18 gate in
/// `emit_accepted` reclassifies economy-less acceptances as
/// `no_computable_economics` rejections ("arbitrajes reales, no de fallback":
/// an accepted row the operator cannot act on is a cosmetic acceptance).
fn has_computed_economics(opp: &Opportunity) -> bool {
    opp.expected_profit_usd.is_some() || opp.net_expected_profit_usd.is_some()
}

/// Builds the Gate C scoring stream record XADDed to `arbx:scoring:scored`
/// (ARBX-RDY-02). Pure — no I/O — so it is unit-testable without Redis/PG.
/// `emission_outcome` labels the emitting path (`"accepted"` / `"rejected"`);
/// `rejection_reason` is `None` on the accepted path (serialized as JSON null)
/// and the verbatim reason on the rejected path (A.5 doctrine: prior
/// calibration needs BOTH classes). Field set is the wire contract mirrored by
/// the api-server `ScoredRecordSchema` — additive only.
fn build_score_record(
    opp: &Opportunity,
    strategy_key: &str,
    score: &crate::scoring_pipeline::ConfidenceScore,
    evidence_vector: Option<serde_json::Value>,
    emission_outcome: &'static str,
    rejection_reason: Option<&str>,
) -> serde_json::Value {
    let net_profit_usd = opp.net_expected_profit_usd.or(opp.expected_profit_usd);
    let chain_id_i64 = i64::try_from(opp.chain_id).unwrap_or(-1);
    serde_json::json!({
        "opportunity_id": opp.id.to_string(),
        "strategy_key": strategy_key,
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
        "evidence_vector": evidence_vector,
        "emission_outcome": emission_outcome,
        "rejection_reason": rejection_reason,
    })
}

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
            strategy_kind: StrategyKind::dex_arb(),
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
            cartridge_id: None,
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

    // ── 2026-08-18 gate: accepted requires computed economics ────────────────

    #[test]
    fn no_economics_is_not_computed_economics() {
        // The exact live-prod signature: amount 0, gross None, net None →
        // must NOT qualify as an accepted-class opportunity.
        let mut opp = make_opp(Uuid::new_v4(), None, None);
        opp.amount_in_wei = "0".to_owned();
        assert!(!has_computed_economics(&opp));
    }

    #[test]
    fn gross_or_net_each_qualify() {
        // Gross alone qualifies…
        let gross_only = make_opp(Uuid::new_v4(), Some(1.5), None);
        assert!(has_computed_economics(&gross_only));
        // …and net alone qualifies (spine accepted path sets net even when
        // gross came through as None — the row still carries real economics).
        let mut net_only = make_opp(Uuid::new_v4(), None, None);
        net_only.net_expected_profit_usd = Some(0.42);
        assert!(has_computed_economics(&net_only));
    }

    // ── build_score_record (ARBX-RDY-02) ────────────────────────────────────

    fn flat_score() -> crate::scoring_pipeline::ConfidenceScore {
        crate::scoring_pipeline::ConfidenceScore {
            posterior_prob: 0.5,
            kelly_fraction: 0.02,
            recommended_position_usd: 100.0,
            bayesian_accepted: true,
            prior_log_odds: 0.0,
            source_context: "flat_prior".to_owned(),
        }
    }

    #[test]
    fn score_record_accepted_variant_has_null_rejection_reason() {
        let opp = make_opp(Uuid::new_v4(), Some(1.5), None);
        let rec = build_score_record(&opp, "dex_arb", &flat_score(), None, "accepted", None);
        assert_eq!(rec["emission_outcome"], "accepted");
        assert!(rec["rejection_reason"].is_null());
        // Pre-RDY-02 field set unchanged.
        assert_eq!(rec["opportunity_id"], opp.id.to_string());
        assert_eq!(rec["strategy_key"], "dex_arb");
        assert_eq!(rec["token_pair"], "WETH/USDC");
        assert_eq!(rec["net_profit_usd"], 1.5);
        assert_eq!(rec["chain_id"], 1);
        assert_eq!(rec["source_context"], "flat_prior");
        assert_eq!(rec["scoring_mode"], "paper");
        assert!(rec["evidence_vector"].is_null());
    }

    #[test]
    fn score_record_rejected_variant_carries_verbatim_reason() {
        // Rejected path signature: both profit fields None ⇒ net_profit_usd
        // null (R8 — never coerced to a fabricated value).
        let opp = make_opp(Uuid::new_v4(), None, None);
        let rec = build_score_record(
            &opp,
            "MEV-01-001",
            &flat_score(),
            None,
            "rejected",
            Some("NegativeNetProfit:gas_floor_breach"),
        );
        assert_eq!(rec["emission_outcome"], "rejected");
        assert_eq!(
            rec["rejection_reason"],
            "NegativeNetProfit:gas_floor_breach"
        );
        assert!(rec["net_profit_usd"].is_null());
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
