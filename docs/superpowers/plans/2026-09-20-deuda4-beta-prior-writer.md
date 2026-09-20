# Deuda 4 — Beta Prior Per-Estrategia: Writer + Reader Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Close the last real implementation gap of Deuda 4: give `bayesian_priors` a per-strategy writer (consolidator from `scored_opportunities` labels) and load `PriorState` into the Gate-C emit path, so the Beta side of Bayesian calibration stops being permanently flat.

**Architecture:** A new `beta_priors.rs` module in searcher-rs (twin of `priors_cache.rs`, same patterns: background refresh task over the existing `PgPool`, change-detect, honest-degradation) does BOTH jobs per cycle: (1) UPSERT per-strategy Beta counts into `bayesian_priors` (full recompute — idempotent, drift-proof), then (2) read the table back into an in-memory `HashMap<String, PriorState>` that `score_and_publish` consults instead of hardcoded `None`. One micro-migration (122) relaxes `bayesian_priors.token_pair` NOT NULL (leftover from the pre-STRAT-IDENT-01 schema; table verified empty). The writer is gated default-OFF by `ARBX_BETA_PRIORS_MODE` (stage2_calibration precedent: PG mutations are operator-flipped, never implicit).

**Tech Stack:** Rust (tokio, sqlx, tracing), PostgreSQL, TypeScript/Express (copy-only change), vitest, cargo nextest-style test runner (`cargo test -p searcher-rs`).

## Global Constraints

- **RULE 00 / R8 (zero mocks, fail-honest):** labels come ONLY from real archived `scored_opportunities` columns (`rejection_reason` + `net_profit_usd`). Label semantics (operator decision 2026-09-20, amendment A): Y=0 ⇔ `rejection_reason` in the ECONOMIC taxonomy (`non_positive_profit`, `non_positive_net_usd`, `non_positive_gross_usd`, `kelly_negative_edge`, `gas_floor_breach` — exact strings verified at size_optimizer.rs:116-137) **or** `net_profit_usd <= 0` (computed loss, e.g. `NegativeNetProfit` paper-executor rows); Y=1 ⇔ `net_profit_usd > 0` (and NOT economic-rejected — the verdict overrides the sign); everything else (structural rejects with NULL net) = EXCLUDED. Never fabricate a prior. **(a8 ratification + precision, 2026-09-20):** verdict>sign must stay PERMANENT for `gas_floor_breach` and `kelly_negative_edge` — both can occur with computed net>0 (gas floor = net positive but < gas×kelly; Kelly = net positive but non-positive edge), so a post-(B) sign-relaxation would INVERT the gate's verdict on exactly those two; only `non_positive_profit`/`non_positive_net_usd`/`non_positive_gross_usd` may relax post-(B). The economic set is deliberately BROADER than `is_net_dependent()` (which omits `non_positive_gross_usd`) — a labeling choice, not a derivation; do not "derive" the set from that flag.
- **STRAT-IDENT-01:** calibration identity = `strategy_key` (cartridge stem / engine kind). `token_pair` NEVER enters the key — stays as context in the record.
- **§37 (one PR = one ID):** this PR closes the "Deuda 4 remainder — Beta-prior writer gap" (STRAT-IDENT-01 follow-up). No unrelated changes ride along.
- **Gate default-OFF:** the writer only runs when `ARBX_BETA_PRIORS_MODE` ∈ {on,1,true}. Flip on VPS is owned by session -61/operator (their .env, their deploy). We ship the flag off.
- **Claims:** searcher-rs is a8's area (aviso sent, claims `beta_priors.rs` new file + minimal wiring in `opportunity_emitter.rs`); recon/stage2_calibration is -61's front — DO NOT TOUCH. Merges/deploys are -61's.
- **Worktree:** all work in `C:\Users\HFRC\Desktop\arbitragex-v2-main (17)\.claude\worktrees\perf-89-edge-fe`, branch `feat/deuda4-beta-prior-writer` created from `origin/main` (NOT the shared tree).
- **Windows AppControl 4551 workaround (PROVEN, WO-7b):** all cargo commands MUST run with `CARGO_TARGET_DIR="C:\Users\HFRC\Desktop\arbitragex-v2-main (17)\backend\target"` from the worktree (reuses the main tree's warm target; never touches -61's branch).
- **Commits** end with `Co-Authored-By: Claude Code <noreply@anthropic.com>`; **PR description** ends with `🤖 Generated with [Claude Code](https://github.com/anthropic.com/claude-code)`.
- **No deploys, no .env edits, no live flips** in this plan.
- **Branch timing (orden de -61, 2026-09-20):** open `feat/deuda4-beta-prior-writer` ONLY AFTER -61 confirms #606 is merged (they are merging it now; the queue #606-#611 touches `opportunity_emitter.rs` territory). If starting earlier, rebase onto fresh `origin/main` before Task 3 (the emitter edit).

## Review decisions (locked from a8 + -61 responses, 2026-09-20)

1. **ON CONFLICT inference (a8 #1):** the UPSERT MUST use `ON CONFLICT (strategy_key) WHERE strategy_key IS NOT NULL DO UPDATE ...` — the WHERE is required for Postgres to infer the PARTIAL unique index `uq_bayesian_priors_strategy`. Without it: `no unique or exclusion constraint matching the ON CONFLICT specification`. The SQL in Task 2 is already written this way — keep it verbatim.
2. **Label population (a8 #2, ratified by -61):** ALL of `scored_opportunities` (accepted AND rejected), deduped to latest-per-opportunity, filtered to `net_profit_usd IS NOT NULL`. Semantics align with -61's chain: ECONOMIC/MARKET rejections (`non_positive_profit` 65%, `gas_floor_breach`, ...) carry a computed net ≤ 0 ⇒ Y:0; STRUCTURAL rejections (quote unavailable etc.) never computed economics ⇒ `net_profit_usd IS NULL` ⇒ excluded as ineligible, never counted as a loss (R8: absent label ≠ zero). This population decision is written into the module doc so reviewers can reject it independently.
3. **Full recompute vs watermark (a8 #3):** full-recompute is a DELIBERATE choice, not an oversight. `scored_opportunities` is bounded by retention (measured by -61 on 2026-09-20: ~60K rows / 15m rejection window; tiered retention migrations 116/117 cap growth). One aggregate query (index-assisted scan + DISTINCT ON + GROUP BY) over ≤ ~100K rows every 60s is negligible PG load, and recompute is drift-proof — incremental counters would need watermark persistence + crash recovery to avoid silent drift (exactly the class of bug §37 exists for). Escalation trigger, documented: if the table exceeds ~1M retained rows, switch to a `MAX(created_at)` watermark with periodic (daily) full reconciliation.
4. **OFF-parity invariance (a8 #4):** with `ARBX_BETA_PRIORS_MODE` off (or unset), `BetaPriorsCache::disabled()` holds `None` and `get()` returns `None` for every key — the 5th argument of `evaluate_paper_opportunity` is then EXACTLY `None`, byte-parity with today (§34.1 no math change while gated). This is proven at unit level by `mode_enabled` gate tests + `cache_disabled_stays_none` (Task 2); the emitter call site has no cheaper test than that because `score_and_publish` requires a live Redis `ConnectionManager`.
5. **θ₀≈0 clamp (-61 #2):** with wins≈0 across the population (today's reality: `non_positive_profit` dominates), the Beta posterior will clamp at the lower bound. That is HONEST no-signal, not a bug — report as-is, never blend toward a prior that isn't in the data.
6. **Y=0 starvation risk (-61 #1):** if ECONOMIC rejections were archived with `net_profit_usd = NULL`, the writer would silently starve Y=0 exactly like -61's drift-tracker did. Task 5 Step 0 verifies this BEFORE the PR ships. **EXECUTED 2026-09-20 — found the inverse defect (mislabel), not starvation:** 99.5% of `non_positive_profit` rejects archive `net_profit_usd > 0` (the raw POSITIVE detector estimate, NOT the optimizer's computed ≤ 0 value). Root cause: `size_optimizer.rs` computes the real net but it dies inside `OptimizeOutcome::Rejected` (bare enum, value never stamped into `opp.net_expected_profit_usd`); `opportunity_emitter.rs:578` then persists `net_expected_profit_usd.or(expected_profit_usd)` — the stale positive estimate. VPS evidence (~1h window): rejected=257,308 — `non_positive_profit` 95,958 rows with 95,520 carrying net>0 (false Y:1 under sign-of-net semantics); honest sources: `NegativeNetProfit` 280 rows (paper-executor path, net populated ≤ 0).
7. **OPERATOR DECISION (2026-09-20, via AskUserQuestion) — "Ambos: A ahora + B después":**
   - **(A) THIS PR (writer-side labels by `rejection_reason` taxonomy):** the economic-verdict override is today the ONLY defense against the mislabel — it lives 100% inside this module's CONSOLIDATE_SQL, zero changes to the searcher reject-path. Expected honest outcome with current data: y1≈0 ⇒ θ₀ clamps at the lower bound (decision #5 applies verbatim — honest no-signal, never blended).
   - **(B) a8's separate follow-up PR (reject-path net persistence):** extend `OptimizeRejectReason::Rejected` to carry `net_profit_usd: Option<f64>` (None = Err-path/I-O failure — must stay None, R8; Some(v≤0) = computed unprofitable), stamp at the two emit sites (orchestrator.rs + cartridge_boot.rs:1467) BEFORE `emit_rejected`. Forward-only, no backfill. Owned/queued by a8 on hot-path files — NOT in this PR.
   - **Consolidator cadence — "Refresh 300s":** table measured at 4.77M rows (~162K/h growth, ~30h retention). Full recompute every 300s (default `ARBX_BETA_PRIORS_REFRESH_SECS = 300`) = a ~1-3s aggregate scan every 5 min, <1% PG load. The ~1M-row watermark escalation trigger from decision #3 is already exceeded — resolved by this cadence choice + retention bounding, not by switching to incremental (drift-proof full recompute stays).

## Verified ground truth (do not re-derive)

- `bayesian_priors` schema (migration 097 + 108): `id, token_pair TEXT NOT NULL (UNIQUE constraint DROPPED by 108), log_odds DOUBLE PRECISION NOT NULL DEFAULT 0.0, observation_count BIGINT NOT NULL DEFAULT 0, profitable_count BIGINT NOT NULL DEFAULT 0, last_updated TIMESTAMPTZ NOT NULL DEFAULT now(), strategy_key TEXT (added by 108)` + partial unique index `uq_bayesian_priors_strategy ON (strategy_key) WHERE strategy_key IS NOT NULL`. **Table verified EMPTY, no writer exists.**
- `scored_opportunities` (097+108+109): has `strategy_key TEXT`, `emission_outcome TEXT`, `net_profit_usd`, `created_at`, `opportunity_id`; index `idx_scored_opportunities_strategy (strategy_key, created_at DESC) WHERE strategy_key IS NOT NULL`. Populated by api-server `scored-opportunities-archiver.ts` draining `arbx:scoring:scored` (59,638 rows on VPS as of 2026-09-20).
- `PriorState { observation_count: u64, profitable_count: u64, log_odds: f64 }` is `pub` in `scoring_pipeline.rs`; `compute_confidence` uses only the two counts (`log_odds` is dead code). `evaluate_paper_opportunity(..., prior: Option<PriorState>)` takes the prior BY VALUE.
- The emit path hardcodes `None` at `opportunity_emitter.rs` ~:594 with a STALE comment claiming the table is "keyed `token_pair UNIQUE`" — 108 already re-keyed it.
- `priors_cache.rs` §"NOT here" comment block (lines 29-38) makes the same stale claim.
- `sim-pipeline.ts:19,109` say "writer is a follow-up"; `sim-pipeline.test.ts` does NOT assert `prior_source` (copy change is test-safe).
- `PriorsCache` patterns to mirror exactly: `Arc<RwLock<Option<...>>>`, `spawn`/`spawn_opt`/`disabled`/`#[cfg(test)] from_*`, `MissedTickBehavior::Skip`, refresh env parsed with `.filter(|s| *s >= 5).unwrap_or(default)`, change-detect + `info!(event = "..._cache.updated")`, PG-down ⇒ retain last good + `debug!` on failure.

---

### Task 1: Migration 122 — relax `bayesian_priors.token_pair` NOT NULL

**Files:**
- Create: `database/migrations/122_bayesian_priors_token_pair_nullable.sql`

**Interfaces:**
- Consumes: schema left by migrations 097 + 108.
- Produces: `bayesian_priors.token_pair` nullable — the strategy-keyed writer can INSERT `token_pair = NULL` (or omit it). No other task touches DDL.

- [x] **Step 1: Verify the next free migration number**

Run: `ls database/migrations | sort | tail -3`
Expected: `120_service_credentials_envelope_encryption.sql` and `121_opportunities_detector_id_pipeline_latency.sql` are the last two. If a `122_*` already exists (another branch landed one), renumber THIS file to the next free slot and use that number everywhere in this plan.

- [x] **Step 2: Write the migration**

```sql
-- 122_bayesian_priors_token_pair_nullable.sql
-- Deuda 4 / STRAT-IDENT-01 follow-up: bayesian_priors rows are keyed by
-- strategy_key (unique partial index uq_bayesian_priors_strategy, migration
-- 108). token_pair is legacy identity from migration 097 (its UNIQUE
-- constraint was already dropped in 108); strategy-keyed rows written by the
-- searcher-rs beta_priors consolidator have no honest pair value (R8 — the
-- pair stays as context in the record, never as the calibration bucket).
--
-- Table verified EMPTY on every environment (no writer existed as of
-- 2026-09-20) — no data implications. Idempotent: DROP NOT NULL on an
-- already-nullable column is a no-op.

ALTER TABLE bayesian_priors
    ALTER COLUMN token_pair DROP NOT NULL;

COMMENT ON COLUMN bayesian_priors.strategy_key IS
    'STRAT-IDENT-01 identity (cartridge stem / engine kind). Unique via uq_bayesian_priors_strategy (partial). Writer: searcher-rs beta_priors consolidator (ARBX_BETA_PRIORS_MODE).';
```

- [x] **Step 3: Sanity-check the SQL against a scratch PG (optional but preferred)**

If any local PG is reachable (e.g. via the Postgres MCP, read-only will NOT work for DDL — skip instead), verify syntax mentally against migration 097's style. No local Docker (RULE 01). The real verification is -61's VPS migration run (disk-gated, their sequence).

- [x] **Step 4: Commit** (only after the #606-merged gate — see Global Constraints / Task 5 Step 0b)

```bash
git fetch origin
git checkout -b feat/deuda4-beta-prior-writer origin/main
git add database/migrations/122_bayesian_priors_token_pair_nullable.sql
git commit -m "$(cat <<'EOF'
feat(db): bayesian_priors.token_pair nullable for strategy-keyed writer (Deuda 4)

Co-Authored-By: Claude Code <noreply@anthropic.com>
EOF
)"
```

---

### Task 2: `beta_priors.rs` — reader/writer cache module (TDD)

**Files:**
- Create: `backend/searcher-rs/src/beta_priors.rs`
- Modify: `backend/searcher-rs/src/lib.rs` (or `main.rs` — wherever `pub mod priors_cache;` is declared; find with `grep -n "mod priors_cache" backend/searcher-rs/src/*.rs` and add the sibling line)

**Interfaces:**
- Consumes: `crate::scoring_pipeline::PriorState` (pub struct, fields `observation_count: u64, profitable_count: u64, log_odds: f64`); `sqlx::PgPool`.
- Produces:
  - `pub struct BetaPriorsCache` with `pub fn spawn(pool: PgPool) -> Self`, `pub fn spawn_opt(pool: &Option<PgPool>) -> Self`, `pub fn disabled() -> Self`, `pub fn get(&self, strategy_key: &str) -> Option<PriorState>`, `#[cfg(test)] pub(crate) fn from_map(m: Option<HashMap<String, PriorState>>) -> Self`.
  - `pub(crate) fn mode_enabled(v: Option<&str>) -> bool` and `pub(crate) fn build_map(rows: Vec<(String, i64, i64)>) -> HashMap<String, PriorState>` (pure, unit-tested).
  - Task 3 relies on: `BetaPriorsCache::spawn_opt(&pool)` in `OpportunityEmitter::new`, `BetaPriorsCache::disabled()` in `new_dry_run`, `self.beta_priors.get(&strategy_key)` at the scoring call site.

- [x] **Step 1: Write the failing tests (new file with tests first, impl stubs)**

Create `backend/searcher-rs/src/beta_priors.rs` containing ONLY the test module plus minimal stubs so it compiles and the tests FAIL (functions return wrong/empty values):

```rust
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
    fn build_map_clamps_negative_db_counts() {
        // BIGINT underflow / corrupted row defense: saturate, never panic.
        let m = build_map(vec![("weird".into(), -3, -1)]);
        assert_eq!(m.len(), 1);
        assert_eq!(m["weird"].observation_count, 0);
        assert_eq!(m["weird"].profitable_count, 0);
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
            PriorState { observation_count: 7, profitable_count: 3, log_odds: 0.0 },
        );
        let c = BetaPriorsCache::from_map(Some(m));
        let p = c.get("s").expect("present");
        assert_eq!((p.observation_count, p.profitable_count), (7, 3));
        assert!(c.get("absent").is_none());
    }
}
```

Add stubs ABOVE the tests so the crate compiles (stubs deliberately return values that make tests fail where semantics matter):

```rust
pub struct BetaPriorsCache; // placeholder — replaced in Step 3
impl BetaPriorsCache {
    pub fn get(&self, _k: &str) -> Option<PriorState> { None }
    pub fn disabled() -> Self { BetaPriorsCache }
    #[cfg(test)]
    pub(crate) fn from_map(_m: Option<std::collections::HashMap<String, PriorState>>) -> Self { BetaPriorsCache }
}
pub(crate) fn mode_enabled(_v: Option<&str>) -> bool { true } // wrong on purpose
pub(crate) fn build_map(_rows: Vec<(String, i64, i64)>) -> std::collections::HashMap<String, PriorState> { std::collections::HashMap::new() }
```

Also `use` statements at the top of the stub (Step 3 expands them):

```rust
use crate::scoring_pipeline::PriorState;
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use std::time::Duration;
use sqlx::PgPool;
use tracing::{debug, info};
```

Wire the module: add `pub mod beta_priors;` next to `mod priors_cache;` in the crate root (`grep -n "mod priors_cache" backend/searcher-rs/src/lib.rs backend/searcher-rs/src/main.rs`).

- [x] **Step 2: Run tests to verify they fail**

Run (from `backend/searcher-rs`, Bash):
```bash
CARGO_TARGET_DIR="C:/Users/HFRC/Desktop/arbitragex-v2-main (17)/backend/target" cargo test -p searcher-rs beta_priors 2>&1 | tail -20
```
Expected: COMPILE FAIL or test FAILURES (`mode_off_by_default_and_truthy_values_only` fails because stub returns true; `build_map_*` fail on empty map). If compile errors point at your stubs, fix the stubs (not the tests) until it compiles and fails on assertions.

- [x] **Step 3: Write the real implementation**

Replace the stubs with the full module (keep the tests untouched):

```rust
//! beta_priors — Deuda 4 / STRAT-IDENT-01 follow-up: the per-STRATEGY Beta
//! prior store (`bayesian_priors` → `PriorState`). Twin of `priors_cache.rs`
//! (which mirrors the §IV operator log-LR slice); this module owns the BETA
//! side: both the consolidator WRITER and the in-memory READER.
//!
//! ## Cycle (one pass, every ARBX_BETA_PRIORS_REFRESH_SECS)
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
//!    before persistence). Until a8's reject-path net-persistence fix (B)
//!    lands, the rejection_reason verdict is the ONLY honest Y:0 source.
//!    Full recompute beats incremental deltas: drift-proof, and bounded by
//!    the scored_opportunities retention window + the 108 strategy index
//!    (4.77M rows @ 2026-09-20 ⇒ refresh default 300s, operator-set).
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
use tracing::{debug, info};

/// Consolidator + reader SQL. The ON CONFLICT target must repeat the partial
/// index predicate (`WHERE strategy_key IS NOT NULL`) to infer
/// `uq_bayesian_priors_strategy` (migration 108).
const CONSOLIDATE_SQL: &str = r#"
WITH latest AS (
    SELECT DISTINCT ON (opportunity_id)
           opportunity_id, strategy_key, net_profit_usd, rejection_reason
    FROM scored_opportunities
    WHERE strategy_key IS NOT NULL
    ORDER BY opportunity_id, created_at DESC
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

/// Writer gate (stage2_calibration precedent: PG mutations are
/// operator-flipped, never implicit). Default OFF.
pub(crate) fn mode_enabled(v: Option<&str>) -> bool {
    matches!(
        v.map(|s| s.trim().to_ascii_lowercase()).as_deref(),
        Some("on") | Some("1") | Some("true")
    )
}

/// Pure: DB rows (strategy_key, observation_count, profitable_count) → the
/// in-memory prior map. Zero-observation strategies are dropped (None ⇒ flat
/// prior); negative counts saturate to 0; profitable is capped at
/// observations (never a Beta with β < 1). Pure — unit-testable, no I/O.
pub(crate) fn build_map(rows: Vec<(String, i64, i64)>) -> HashMap<String, PriorState> {
    let mut m = HashMap::with_capacity(rows.len());
    for (key, obs, prof) in rows {
        if key.is_empty() || obs <= 0 {
            continue;
        }
        let obs = obs as u64;
        let prof = prof.clamp(0, obs) as u64;
        m.insert(key, PriorState { observation_count: obs, profitable_count: prof, log_odds: 0.0 });
    }
    m
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
        let cache = Self { inner: Arc::new(RwLock::new(None)) };
        let refresh = cache.clone();
        // Operator decision 2026-09-20 ("Refresh 300s"): scored_opportunities
        // measured at 4.77M retained rows (~162K/h) — a full-recompute
        // aggregate every 300s keeps PG load <1% while staying drift-proof.
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
                    debug!(event = "beta_priors.refresh_failed", error = %e);
                }
            }
        });
        cache
    }

    /// No-writer/no-PG constructor: permanently-None (honest flat prior).
    pub fn disabled() -> Self {
        Self { inner: Arc::new(RwLock::new(None)) }
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
        Self { inner: Arc::new(RwLock::new(m)) }
    }

    /// This strategy's calibrated state; `None` = flat prior. Cheap: one map
    /// entry clone under a micro-lock (hot-path safe).
    pub fn get(&self, strategy_key: &str) -> Option<PriorState> {
        self.inner.read().ok().and_then(|g| g.as_ref()?.get(strategy_key).cloned())
    }
}

/// One cycle: consolidate (write) then read back. Overwrite the map only when
/// content changed (change-detect, priors_cache pattern).
async fn refresh_once(pool: &PgPool, cache: &BetaPriorsCache) -> anyhow::Result<()> {
    sqlx::query(CONSOLIDATE_SQL).execute(pool).await?;

    let rows: Vec<(String, i64, i64)> =
        sqlx::query_as(READ_SQL).fetch_all(pool).await?;
    let next = build_map(rows);

    {
        let mut g = cache
            .inner
            .write()
            .map_err(|_| anyhow::anyhow!("beta_priors lock poisoned"))?;
        if g.as_ref() != Some(&next) {
            let strategies = next.len();
            *g = Some(next);
            info!(event = "beta_priors.updated", strategies);
        }
    }
    Ok(())
}
```

(`build_map` uses `prof.clamp(0, obs)` — for `i64` prof against `i64 obs` before the `as u64` cast; write it as `let prof = prof.clamp(0, obs);` on i64 values then cast both. Adjust the exact expression if clippy complains — semantics must hold: 0 ≤ profitable ≤ observation.)

- [x] **Step 4: Run tests to verify they pass**

```bash
CARGO_TARGET_DIR="C:/Users/HFRC/Desktop/arbitragex-v2-main (17)/backend/target" cargo test -p searcher-rs beta_priors 2>&1 | tail -15
```
Expected: 6 passed, 0 failed.

- [x] **Step 5: Commit**

```bash
git add src/beta_priors.rs src/lib.rs
git commit -m "$(cat <<'EOF'
feat(searcher): beta_priors per-strategy Beta-prior writer + reader (Deuda 4)

Consolidates Gate-C labels from scored_opportunities into bayesian_priors
(full recompute, latest-per-opportunity dedup) and mirrors them into the
emit path as PriorState. Gated by ARBX_BETA_PRIORS_MODE (default off);
absent prior stays flat Beta(1,1) — R8 honest.

Co-Authored-By: Claude Code <no-reply@anthropic.com>
EOF
)"
```

(If the crate root is `main.rs`, `git add src/main.rs` instead of `src/lib.rs`.)

---

### Task 3: Wire the prior into the emit path + fix stale comments

**Files:**
- Modify: `backend/searcher-rs/src/opportunity_emitter.rs` (~:40 imports, ~:144 field, ~:168 spawn in `new`, ~:196 `disabled()` in `new_dry_run`, ~:585-600 scoring call site + stale comment)
- Modify: `backend/searcher-rs/src/priors_cache.rs` (lines 29-38, §"NOT here" comment — comment-only)

**Interfaces:**
- Consumes: `BetaPriorsCache::{spawn_opt, disabled, get}` (Task 2 exact signatures).
- Produces: `OpportunityEmitter` with a `beta_priors: BetaPriorsCache` field; `score_and_publish` passes `self.beta_priors.get(&strategy_key)` as the `prior` argument of `evaluate_paper_opportunity`. Downstream observable effect: emitted score records flip `source_context: "flat_prior" → "calibrated"` once the store has ≥1 observation for the strategy and the mode is ON.

- [x] **Step 1: Add the field + constructors**

In `opportunity_emitter.rs`:

Import (next to the existing `use crate::priors_cache::{section_iv_fold, PriorsCache};` at ~:40):
```rust
use crate::beta_priors::BetaPriorsCache;
```

Add the field to `pub struct OpportunityEmitter` right after `priors: PriorsCache,` (~:144), with doc comment:
```rust
    /// Deuda 4 Beta side: per-STRATEGY prior state mirrored from
    /// `bayesian_priors` (strategy-keyed since migration 108; the
    /// consolidator writer + this reader both live in `beta_priors`,
    /// gated by ARBX_BETA_PRIORS_MODE — default off ⇒ flat prior).
    beta_priors: BetaPriorsCache,
```

In `new()` (~:168), next to `let priors = PriorsCache::spawn_opt(&pool);`:
```rust
        let beta_priors = BetaPriorsCache::spawn_opt(&pool);
```
and add `beta_priors,` to the `Self { ... }` literal after `priors,`.

In `new_dry_run()` (~:196), after `priors: PriorsCache::disabled(),`:
```rust
            beta_priors: BetaPriorsCache::disabled(),
```

- [x] **Step 2: Replace the hardcoded None at the scoring call site**

Find the block (~:589-596) that currently reads:

```rust
        // Beta-side prior stays None — HONEST, audited 2026-08-29:
        // `bayesian_priors` has no writer AND is keyed `token_pair UNIQUE`
        // (pre-STRAT-IDENT-01 schema) while `PriorState` is per-STRATEGY.
        // Feeding pair-keyed priors would re-introduce the identity collapse
        // STRAT-IDENT-01 fixed ("the pair stays as context in the record —
        // never as the calibration bucket"). A `strategy_key` column + writer
        // is the follow-up; until then None is the wired-but-not-calibrated
        // truth. The §IV calibration surface (per-operator log-LR) IS live —
        // see the fold below. (BR-05 (2026-09-07): WO-07 port-back.)
```

and replace it with:

```rust
        // Beta-side prior: per-STRATEGY state mirrored from `bayesian_priors`
        // (strategy-keyed since migration 108; writer + reader = beta_priors
        // module, gated by ARBX_BETA_PRIORS_MODE — default OFF). Absent ⇒
        // None ⇒ flat Beta(1,1), the honest uncalibrated state (R8). The pair
        // never enters the key (STRAT-IDENT-01: context in the record, never
        // the calibration bucket). The §IV fold below is the orthogonal
        // per-operator surface (BR-05, WO-07 port-back).
```

Then in the `evaluate_paper_opportunity(...)` call right below, replace the trailing `None,` (5th argument, `prior`) with:

```rust
            self.beta_priors.get(&strategy_key),
```

(The call currently ends `Some(chain_id_i64),\n            None,\n        )` — the replaced argument is that final `None`.)

- [x] **Step 3: Fix the stale §"NOT here" comment in priors_cache.rs**

Replace lines 29-38 of `priors_cache.rs` (the whole `//! ## NOT here` block) with:

```rust
//! ## NOT here (split of duties, updated 2026-09-20)
//!
//! The per-STRATEGY Beta prior (`bayesian_priors` → `PriorState`) lives in
//! `beta_priors` (this crate): the table is strategy-keyed since migration
//! 108 (`uq_bayesian_priors_strategy` partial unique index) and that module
//! owns BOTH its consolidator writer and the in-memory reader feeding
//! `evaluate_paper_opportunity`. This module remains the §IV operator
//! log-LR slice ONLY (`math_operator_calibration`, written by recon's
//! stage2_calibration job — Stage 2b).
```

- [x] **Step 4: Verify — full searcher-rs suite + clippy + fmt**

```bash
CARGO_TARGET_DIR="C:/Users/HFRC/Desktop/arbitragex-v2-main (17)/backend/target" cargo test -p searcher-rs 2>&1 | tail -8
CARGO_TARGET_DIR="C:/Users/HFRC/Desktop/arbitragex-v2-main (17)/backend/target" cargo clippy -p searcher-rs --all-targets -- -D warnings 2>&1 | tail -5
CARGO_TARGET_DIR="C:/Users/HFRC/Desktop/arbitragex-v2-main (17)/backend/target" cargo fmt -p searcher-rs
```
Expected: all tests pass (existing emitter tests at `opportunity_emitter.rs:1035-1091` assert `source_context == "flat_prior"` for their fixtures — they construct via the test path where the cache is `disabled()`/empty, so they stay green); clippy 0 warnings; fmt applies any wrap fixes (re-run the test line if fmt touched files).

Note on wiring test coverage: `score_and_publish` requires a live Redis `ConnectionManager`, so the wiring (get + pass-through) has no unit test — same as the §IV fold wiring at :645 (covered by priors_cache tests + compiler). The `source_context` flip itself is already unit-covered by `scoring_pipeline.rs` test `calibrated_profitable_history_shifts_posterior_up`. This is the established precedent, not an omission.

- [x] **Step 5: Commit**

```bash
git add src/opportunity_emitter.rs src/priors_cache.rs
git commit -m "$(cat <<'EOF'
feat(searcher): wire per-strategy Beta prior into Gate-C emit path (Deuda 4)

score_and_publish now passes the strategy's PriorState (from beta_priors)
instead of hardcoded None; source_context flips to "calibrated" once the
store has observations. Stale token_pair-UNIQUE comments corrected
(migration 108 re-keyed the table in 2026-08).

Co-Authored-By: Claude Code <no-reply@anthropic.com>
EOF
)"
```

---

### Task 4: api-server copy — sim-pipeline "writer is a follow-up" is now false

**Files:**
- Modify: `backend/api-server/src/routes/sim-pipeline.ts` (:19 comment, :109 `prior_source`)

**Interfaces:**
- Consumes: nothing from Tasks 2-3 at runtime (this route only COUNTS rows).
- Produces: accurate operator-facing copy. No schema/type change; `sim-pipeline.test.ts` does not assert `prior_source` (verified).

- [x] **Step 1: Edit the two lines**

Line ~:19, replace:
```typescript
 * - `bayesian_priors` (per-strategy calibration store; writer is a follow-up):
```
with:
```typescript
 * - `bayesian_priors` (per-strategy calibration store; writer = searcher-rs
 *   beta_priors consolidator, gated by ARBX_BETA_PRIORS_MODE):
```

Line ~:109, replace:
```typescript
        prior_source: "bayesian_priors (per-strategy; writer pending follow-up)",
```
with:
```typescript
        prior_source: "bayesian_priors (per-strategy; writer = beta_priors, ARBX_BETA_PRIORS_MODE)",
```

- [x] **Step 2: Run the route's tests + typecheck**

From `backend/api-server` (Bash):
```bash
npx vitest run test/sim-pipeline-route.test.ts 2>/dev/null || npx vitest run src/routes/sim-pipeline.test.ts
```
(Use whichever path matches the repo layout: `ls src/routes/sim-pipeline.test.ts test/ | head`.)
Expected: all pass. Then typecheck per repo convention (`npx tsc --noEmit` if a tsconfig covers routes; if the FE/BE typecheck script differs, run `npm run -s typecheck` if defined).

- [x] **Step 3: Commit**

```bash
git add src/routes/sim-pipeline.ts
git commit -m "$(cat <<'EOF'
docs(api): sim-pipeline copy — bayesian_priors writer now exists (Deuda 4)

Co-Authored-By: Claude Code <noreply@anthropic.com>
EOF
)"
```

---

### Task 5: Pre-flight verification, full local gate, push, PR, avisos

**Files:** none (verification + delivery).

- [x] **Step 0: Pre-flight — Y=0 starvation check (read-only, -61's flag #1) — EXECUTED 2026-09-20, STOP clause fired, resolved by operator decision (Review decision #7)**

Executed on the VPS (read-only SELECTs). Outcome: NOT simple starvation — a CRITICAL MISLABEL: 99.5% of `non_positive_profit` rejects (95,520 of 95,958 in a ~1h window) archive `net_profit_usd > 0` (the raw positive detector estimate; the optimizer's computed ≤ 0 value never gets persisted — see Review decision #6 for the full evidence and root cause). The plan's STOP clause was executed: escalated to the operator and both peers. **Operator decision: "Ambos: A ahora + B después"** — Task 2's CONSOLIDATE_SQL was amended to the rejection_reason taxonomy (amendment A, this PR); a8 owns the reject-path net-persistence fix (amendment B, separate PR). Proceed on the amended SQL.

Also captured: `COUNT(*) = 4,769,123` (30h retention) → cadence decision "Refresh 300s" applied in Task 2.

- [x] **Step 0b: Confirm -61 merged #606 — EXECUTED 2026-09-20: MERGED** (confirmed by -61 in session). Branch creation is unblocked.

- [x] **Step 1: Full workspace gate (Rust + fmt + clippy)**

From `backend/searcher-rs` (Bash):
```bash
CARGO_TARGET_DIR="C:/Users/HFRC/Desktop/arbitragex-v2-main (17)/backend/target" cargo test -p searcher-rs 2>&1 | tail -6
CARGO_TARGET_DIR="C:/Users/HFRC/Desktop/arbitragex-v2-main (17)/backend/target" cargo clippy -p searcher-rs --all-targets -- -D warnings 2>&1 | tail -4
CARGO_TARGET_DIR="C:/Users/HFRC/Desktop/arbitragex-v2-main (17)/backend/target" cargo fmt -p searcher-rs -- --check
```
Expected: tests pass (incl. WO-7/WO-7b suites from #605/#611 — the branch is cut from `origin/main` AFTER those merged; if #611 has NOT merged yet, its tests simply aren't here — fine), clippy clean, fmt clean.

- [x] **Step 2: Verify branch discipline (§36)**

```bash
git branch --show-current   # MUST print feat/deuda4-beta-prior-writer
git log --oneline origin/main..HEAD
```
Expected: exactly the 4 commits from Tasks 1-4.

- [x] **Step 3: Push + PR**

```bash
git push -u origin feat/deuda4-beta-prior-writer
```
PR title: `feat: Deuda 4 — per-strategy Beta prior writer + reader (bayesian_priors)`
PR body: summary (writer/reader/migration/gate), label semantics (operator decision: Y=0 = veredicto económico por `rejection_reason` taxonomy O net computado ≤ 0; Y=1 = net>0 no-económico; NULL/estructurales excluidos; dedup latest-per-opportunity), the mislabel context (99.5% false-positive nets under non_positive_profit — taxonomy override is the defense until a8's (B) lands), cadence (refresh default 300s, 4.77M rows), deployment note (migration 122 + `ARBX_BETA_PRIORS_MODE` flip = -61/operator, default OFF = cero cambio de comportamiento), test plan checklist. End with:
```
🤖 Generated with [Claude Code](https://github.com/anthropic.com/claude-code)
```

- [x] **Step 4: Avisos (claims discipline)**

Send a8 the PR link + offer line-by-line review of the real commit (same pact as WO-7b). Send -61 the PR link + the two deploy-side items they own: run migration 123 (disk-gated, their sequence) and flip `ARBX_BETA_PRIORS_MODE=on` after their Stage 2 flips land. Wait for a8's review verdict before considering the loop closed; fix anything found in a follow-up commit on the same branch.

> **Execution log (2026-09-20):** all tasks 1-5 Steps 1-4 landed on branch `feat/deuda4-beta-prior-writer` → PR #618. The 122-slot escape hatch (Task 1 Step 1) fired: #610 (open, unmerged) took `122_route_discovery_outcomes_partitioned`, so the migration was renumbered to **123** in commit 028252cd after PR creation (-61 blocker). Post-merge Step 5 remains deferred to -61's deploy + operator mode flip.

- [ ] **Step 5: Post-merge verification checklist (deferred to -61's deploy; this session VERIFIES, never deploys)**

After merge + deploy + mode flip (all -61's), observe on VPS:
```bash
docker logs searcher-rs --tail 500 | grep -E 'beta_priors.(spawned|updated)'
docker exec postgres psql -U postgres -d arbitragex -c 'SELECT strategy_key, observation_count, profitable_count, last_updated FROM bayesian_priors ORDER BY observation_count DESC LIMIT 10;'
```
Expected: `beta_priors.spawned` once at boot; `beta_priors.updated` with strategies=N>0; table rows per strategy. Then in the emitted records (`arbx:scoring:scored` / scored_opportunities): `source_context: "calibrated"` for strategies with observations, `prior_log_odds != 0` for calibrated ones. While the mode stays OFF, ALL of this stays absent — that is the correct default, not a failure (R8).

---

## Self-Review (completed)

- **Spec coverage:** writer (Task 2 `CONSOLIDATE_SQL` + `refresh_once`), reader (Task 2 `build_map`/`get`), emit-path wiring (Task 3), schema blocker `token_pair NOT NULL` (Task 1), stale comments in `opportunity_emitter.rs` AND `priors_cache.rs` (Task 3), stale api-server copy (Task 4), gate default-off (Task 2 `mode_enabled` + Global Constraints), claims/avisos (Task 5), deploy/flip ownership (Task 5 Step 5). Gap check: `scoring-status.ts` also counts `bayesian_priors` — it needs NO change (its `COUNT(*) WHERE observation_count >= minObs` works unchanged with strategy-keyed rows).
- **Placeholder scan:** every step carries complete code/commands; no TBD/TODO; the only judgment call left to the implementer is `build_map`'s clamp expression note (explicit semantics given).
- **Type consistency:** `PriorState { observation_count: u64, profitable_count: u64, log_odds: f64 }` used identically in Tasks 2-3; `BetaPriorsCache::{spawn_opt(&Option<PgPool>), disabled(), get(&str) -> Option<PriorState>}` signatures match between Task 2 (produces) and Task 3 (consumes); migration number 122 verified free (with renumber step if raced).
