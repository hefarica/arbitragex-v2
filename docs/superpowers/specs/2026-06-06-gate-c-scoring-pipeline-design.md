# Gate-C Scoring Pipeline + A.4/A.5 Validation — Design Spec

- **Date:** 2026-06-06
- **Branch:** `feat/gate-c-scoring-pipeline` (off `feat/cartridge-hotpath-shadow`)
- **Status:** Approved-with-adjustments (brainstorming). Awaiting spec review → writing-plans.
- **Posture:** shadow / paper / read-only. **Zero capital, zero signer, zero broadcast, no live trading, no contract edits, no private keys.**

---

## 1. Context & Goal

The dashboard surfaces three Gate-C blockers from **hardcoded constants** in
`backend/api-server/src/routes/scoring-status.ts` (the route receives a `pg.Pool`
but never queries it):

1. `scoring_pipeline_not_wired` (HIGH) — Bayesian + Kelly primitives are compiled
   into `searcher-rs` but never invoked per opportunity; no `ConfidenceScore` is produced.
2. `a4_fork_validation_not_executed` (CRITICAL) — scoring outputs are untrustworthy
   against mainnet state until the ignored `multistep_fork` test passes on a real RPC + executor.
3. `a5_paper_shadow_not_executed` (HIGH) — without continuous paper-shadow, posterior
   priors cannot be calibrated to real-flow base rates; scoring runs on flat priors only.

**Goal:** convert these three hand-maintained `false` constants into **evidence-based
truth** the api-server derives at runtime, by (a) producing + persisting a real
`ConfidenceScore` per paper opportunity via the repo-native Redis-stream → archiver
path, (b) a reproducible A.4 fork-validation gate that flips only on real test
evidence recorded in the DB, and (c) an A.5 paper-shadow activation gate that
calibrates priors from real flow. **Scoring may be wired before it is calibrated —
the dashboard must say exactly which stage it is in.**

### Recon facts (verified, evidence-based)
- `accept_by_posterior(posterior: &BetaParams, min_win_prob: f64, max_std_dev: f64) -> Option<bool>` → `Some(mean ≥ min && std ≤ max)`.
- `compute_position_size(nav_wei: U256, fraction: f64) -> Option<U256>` — **wei**, ppm integer math.
- `bayes_update(prior: BetaParams, wins: u64, losses: u64) -> Option<BetaParams>`; `kelly_fraction(win_prob, gain_on_win, loss_on_loss) -> Option<f64>`.
- Emission path: `OpportunityEmitter::emit_accepted` (`opportunity_emitter.rs:193`). `dispatch_orchestrator_and_classify` is **REVM-sim classification only**.
- Persistence pattern: Rust `XADD` → Redis stream → api-server TS archiver → Postgres (`paper-trade-archiver.ts`, registered at `index.ts:1245`, gated `ARBX_PAPER_ARCHIVER_MODE`, dormant by default).
- Next free migration = **097** (096 highest; 072 is taken). `scored_opportunities` / `bayesian_priors` do not exist.
- `multistep_fork.rs` exists (ignored test). GitHub remote = `github` (`hefarica/arbitragex-v2`); `origin` = VPS bare backup.
- Prior partial: commit `9914b3f` added `scoring.rs` (per-sim `compute_confidence`, log-only). It stays as a complementary lower-level signal; this spec adds the real Gate-C persistence layer and does not duplicate its math.

---

## 2. Doctrinal guarantees (INVIOLABLE)
- No mocks, no fabricated `ConfidenceScore`, no fabricated posterior, no fabricated Kelly, no invented prices, no fabricated opportunities.
- No signer / no broadcast / no live trading / no capital. Executor touched read-only only (A.4 config).
- No contract edits, no private keys, no wallets. RiskGate not bypassed.
- Blockers never hidden behind false flags; validations never disabled; errors never converted to success.
- Existing flows (`paper_opportunity`, `opportunities`, `route_discovery_outcomes`, the real emitter) unbroken; `arbx:opps:detected` invariant intact.
- Hot path never blocks on Postgres (scoring persistence is fire-and-forget XADD; archiver is decoupled).

---

## 3. Architecture

### 3.1 Data model — `database/migrations/097_scored_opportunities_gate_c.sql`
Forward-only, idempotent (`IF NOT EXISTS` / `CREATE OR REPLACE`), with SQL `COMMENT`s
marking everything **paper-only Gate-C telemetry, not live execution**.

- **`scored_opportunities`** — `id BIGSERIAL PK, opportunity_id TEXT NOT NULL, token_pair TEXT NOT NULL,
  posterior_prob DOUBLE PRECISION NOT NULL, kelly_fraction DOUBLE PRECISION NOT NULL,
  recommended_usd DOUBLE PRECISION NOT NULL, net_profit_usd DOUBLE PRECISION,
  bayesian_accepted BOOLEAN NOT NULL DEFAULT true, prior_log_odds DOUBLE PRECISION,
  chain_id INTEGER, source_context TEXT, scoring_mode TEXT NOT NULL DEFAULT 'paper',
  created_at TIMESTAMPTZ NOT NULL DEFAULT now()`. Indexes: `(token_pair, created_at DESC)`,
  `(created_at DESC)`, `(opportunity_id)`.
- **`bayesian_priors`** — `id BIGSERIAL PK, token_pair TEXT UNIQUE, log_odds DOUBLE PRECISION NOT NULL DEFAULT 0.0,
  observation_count BIGINT NOT NULL DEFAULT 0, profitable_count BIGINT NOT NULL DEFAULT 0,
  last_updated TIMESTAMPTZ NOT NULL DEFAULT now()`. Seeded/updated by A.5.
- **`gate_c_validation`** (beyond blueprint — the channel by which A.4 evidence reaches the
  VPS api-server, which cannot read a local repo marker file) — `id BIGSERIAL PK,
  gate TEXT NOT NULL, status TEXT NOT NULL, evidence_ref TEXT, created_at TIMESTAMPTZ NOT NULL DEFAULT now()`.
- **`gate_c_metrics`** VIEW — `token_pair, total_scored, accepted_total, net_positive_total,
  avg_posterior, avg_kelly_fraction, last_scored_at, first_scored_at` grouped by `token_pair` (blueprint verbatim).

### 3.2 Rust adapter — `backend/searcher-rs/src/scoring_pipeline.rs` (new)
Adaptation layer over the real primitives; does **not** rewrite the scanner.

```rust
pub struct ConfidenceScore {
    pub posterior_prob: f64,
    pub kelly_fraction: f64,
    pub recommended_position_usd: f64, // HYPOTHETICAL PAPER capital, NOT real money
    pub bayesian_accepted: bool,
    pub prior_log_odds: f64,
}
pub struct ScoringPipeline { /* GateCScoringConfig */ }
pub async fn evaluate_paper_opportunity(
    opportunity_id, token_pair, net_profit_usd: Option<f64>, chain_id, source_context,
    prior: Option<PriorState>,
) -> anyhow::Result<ConfidenceScore>;
```

- Prior → `BetaParams`: calibrated `α = profitable_count + 1`, `β = (observation_count − profitable_count) + 1`;
  else **flat** prior with `source_context='flat_prior'`.
- `posterior_prob = prior.mean()` — **honest, no sim-fold, no fabricated win**. Calibration
  comes only from A.5 updating `bayesian_priors`. Flat prior ⇒ ≈0.5 ⇒ dashboard "warming, not calibrated".
- `bayesian_accepted = accept_by_posterior(&prior, MIN_POSTERIOR, MAX_STD).unwrap_or(false)`.
- `kelly_fraction = kelly_fraction(posterior_prob, gain, loss).map(|f| f.min(KELLY_FRACTION_CAP)).unwrap_or(0.0)`.
- **`recommended_position_usd`** = `compute_position_size(U256::from(capital_usd_micros), kelly_fraction)? / 1e6`,
  where `capital_usd_micros = ARBX_KELLY_MAX_CAPITAL_USD × 1_000_000`. **DOC (adjustment #4):** this calls the
  real wei primitive on a **hypothetical paper bankroll cap** (`ARBX_KELLY_MAX_CAPITAL_USD`) — it is a
  paper sizing figure, **never real capital, never a position taken**. Documented prominently in the module
  doc-comment, the struct field comment, and `SCORING_PIPELINE_GATE_C.md`.
- `prior_log_odds = ln(mean/(1−mean))` clamped finite. No `unwrap`/`expect`; never panics.

### 3.3 Wire-point — real emit path (not `dispatch`)
In `OpportunityEmitter::emit_accepted`, guarded by `ARBX_SCORING_ENABLED` (default true), with comment:

```rust
// Gate C: scoring pipeline wired at the real paper opportunity emission path.
// dispatch_orchestrator_and_classify is simulation classification only in this repo revision.
```

Per emitted paper opportunity: load prior (if any) → `evaluate_paper_opportunity` →
**`XADD arbx:scoring:scored`** (fire-and-forget, non-fatal). Scoring is **ADVISORY**: it NEVER skips the
`opps:detected` publish — that stream always receives the opportunity (RULE 00 + invariant).
`ARBX_SCORING_HARD_GATE=true` only records an advisory `scoring.hard_gate.flagged` log (what a future LIVE
execution layer would gate); it does **not** change emission in this shadow build. Default `false` (adjustment #3).
Log events: `scoring.pipeline.wired` (boot), `scoring.evaluate.start`, `scoring.bayesian.accepted`,
`scoring.bayesian.rejected`, `scoring.kelly.computed`, `scoring.confidence.persisted`,
`scoring.confidence.persist_failed`, `scoring.paper_opportunity.annotated`, `scoring.hard_gate.skipped_emit`.

### 3.4 Persistence — repo-native stream → archiver
New `backend/api-server/src/routes/scored-opportunities-archiver.ts`, modeled on
`PaperTradeArchiver`: consumes `arbx:scoring:scored` → parameterized `INSERT` into
`scored_opportunities`, non-fatal, ack. **Registered at api-server boot (adjustment #5)**
in `index.ts` next to `paperArchiver` (line ~1245), gated default-off by
`ARBX_SCORING_ARCHIVER_MODE=on` (mirrors `ARBX_PAPER_ARCHIVER_MODE`). The hot path never touches PG.

### 3.5 Dashboard truth — rewrite `scoring-status.ts` to query the pool
The route already receives `pool`; it will now use it. New granular Gate-C fields are added
alongside the existing (back-compat) `scoring_status` enum.

---

## 4. State machines (the three blockers — adjustments #1 and #6)

### 4.1 `scoring_pipeline_wired` (adjustment #1 — NOT "rows-only")
Structural evidence, independent of whether the market is currently producing opportunities:
- module compiled (workspace-verified constant, now genuinely true once wired),
- stream `arbx:scoring:scored` configured (env present / default),
- archiver ENABLED — `ARBX_SCORING_ARCHIVER_MODE=on` (the api-server reads its own env; a dormant
  archiver means nothing consumes the stream, so claiming "wired" would be cosmetic — report BLOCKED honestly),
- migration applied (`to_regclass('scored_opportunities') IS NOT NULL`, queried),
- **plus** runtime evidence flag if `≥1` row exists (`recent_scored_count`, `last_scored_at`).

States:
- `BLOCKED` — migration absent **OR** archiver dormant (no consumer).
- `WIRED_NO_RUNTIME_SAMPLE` — migration applied **and** archiver enabled, **no row yet** (market quiet; not a failure).
- `WIRED_RUNTIME_SAMPLE` — rows present (runtime proof; trumps the flags).

`scoring_pipeline_wired = true` once structurally wired (migration applied **AND** archiver enabled)
OR rows exist — a quiet market does not re-block, but a dormant archiver honestly does.

### 4.2 A.4 fork validation
- `A4_PENDING` — no `gate_c_validation` row with `gate='a4_fork_validation', status='passed'`.
- `A4_PASSED` — such a row exists (written only when the ignored test actually passed). Never resolved by code alone.

### 4.3 A.5 paper-shadow (adjustment #6 — four states; never call "calibrated" prematurely)
Derived from `paper_trade_runs` + `scored_opportunities` recency/span + `bayesian_priors`:
- `PAPER_SHADOW_NOT_STARTED` — no shadow activity (no recent scored rows / no shadow markers).
- `PAPER_SHADOW_WARMING` — shadow running, accumulating, but span `< ARBX_PAPER_SHADOW_MIN_DAYS` (7)
  **or** `total_scored < ARBX_CALIBRATION_MIN_SCORED` (100).
- `CALIBRATED_CANDIDATE` — span `≥ ARBX_PAPER_SHADOW_MIN_DAYS` **and** `total_scored ≥ ARBX_CALIBRATION_MIN_SCORED`,
  but `bayesian_priors` not yet promoted (no/insufficient calibrated priors).
- `CALIBRATED` — `bayesian_priors` has rows with `observation_count ≥ ARBX_CALIBRATION_MIN_OBSERVATIONS` (30).

(All thresholds env-tunable with the defaults shown; concrete so the state is unambiguous and never
claimed early.)

The dashboard response exposes `scoring_pipeline_state`, `a4_state`, `a5_state` as
distinct fields so the truth is unambiguous: *wired, A4 pending, paper-shadow warming, calibrated.*

---

## 5. Scripts

### 5.1 `scripts/run_a4_fork_validation.sh`
1. Require `RPC_HTTP_1` and `EXECUTOR_1` (else exit 1, clear message).
2. Run `cargo test --manifest-path backend/Cargo.toml --package searcher-rs multistep_fork -- --ignored --nocapture`.
3. Tee output to `audits/gate-c/a4_fork_validation_<timestamp>.log`.
4. On fail → exit 1.
5. On pass → write marker `audits/gate-c/A4_FORK_VALIDATION_PASSED` **and (adjustment #2)**:
   if `DATABASE_URL` is set and `psql` reachable, **execute** the
   `INSERT INTO gate_c_validation(gate,status,evidence_ref) VALUES('a4_fork_validation','passed',<log>)`
   automatically; otherwise **print** the exact manual `INSERT` for the operator to run on the VPS DB.
6. No contract edits, no executor deploy, no mainnet, fork validation only.

### 5.2 `scripts/activate_paper_shadow.sh`
1. Require marker `audits/gate-c/A4_FORK_VALIDATION_PASSED` (else abort).
2. Export/document paper-shadow envs: `ARBX_PAPER_MODE=true`, `ARBX_PAPER_TRADE=true`,
   `ARBX_ORCHESTRATOR_MODE=shadow`, `ARBX_PAPER_SHADOW_ENABLED=true`, `ARBX_SCORING_ENABLED=true`,
   `ARBX_SCORING_HARD_GATE=false`, `ARBX_PAPER_SHADOW_MIN_DAYS=7`.
3. Verify DB: `paper_trade_runs` count/MAX, `scored_opportunities` count/AVG(posterior)/MAX.
4. Print the day-7 `gate_c_metrics` query.
5. No live trading, no signing, no broadcast.

---

## 6. Environment variables (all default-safe)
```
ARBX_SCORING_ENABLED=true
ARBX_SCORING_HARD_GATE=false
ARBX_BAYESIAN_MIN_POSTERIOR=0.50
ARBX_BAYESIAN_MAX_STD=1.00
ARBX_KELLY_FRACTION=0.25
ARBX_KELLY_MAX_CAPITAL_USD=5000
ARBX_SCORING_ARCHIVER_MODE=off     # archiver dormant unless 'on' (mirrors ARBX_PAPER_ARCHIVER_MODE)
ARBX_SCORING_STREAM=arbx:scoring:scored
ARBX_PAPER_SHADOW_MIN_DAYS=7
ARBX_CALIBRATION_MIN_SCORED=100        # total scored rows before CALIBRATED_CANDIDATE
ARBX_CALIBRATION_MIN_OBSERVATIONS=30   # per-pair prior observations before CALIBRATED
```

---

## 7. Tests
- **Rust** (`scoring_pipeline` unit): flat-prior produces a `ConfidenceScore`; `HARD_GATE=false`
  emits (advisory true); `HARD_GATE=true` flags rejected candidates (advisory false) **but still produces + emits the score**;
  `recommended_position_usd` USD conversion is positive and capped; no panic on degenerate prior.
- **TS** (`scoring-status`): evidence-based logic with a mock pool — table present + archiver registered
  ⇒ `WIRED_NO_RUNTIME_SAMPLE`; rows present ⇒ runtime sample; `gate_c_validation` a4 row ⇒ `A4_PASSED`;
  recency ⇒ correct A.5 state. Archiver insert mapping test.
- **Bash**: A.4 fails without `RPC_HTTP_1`/`EXECUTOR_1`; A.5 fails without A.4 marker; A.5 prints envs with marker.
- Commands: `cargo fmt --check`, `cargo check -p searcher-rs --locked`,
  `cargo test -p searcher-rs scoring --lib`, `… bayesian --lib`, `… kelly --lib`, `… --lib`;
  api-server `vitest scoring-status`.

---

## 8. Files

**Created:**
- `database/migrations/097_scored_opportunities_gate_c.sql`
- `backend/searcher-rs/src/scoring_pipeline.rs`
- `backend/api-server/src/routes/scored-opportunities-archiver.ts`
- `scripts/run_a4_fork_validation.sh`
- `scripts/activate_paper_shadow.sh`
- `docs/gate-c/SCORING_PIPELINE_GATE_C.md`
- (tests) `scoring_pipeline` test module; `scored-opportunities-archiver.test.ts`; bash test assertions.

**Modified:**
- `backend/searcher-rs/src/lib.rs` + `main.rs` (declare `mod scoring_pipeline`).
- `backend/searcher-rs/src/opportunity_emitter.rs` (wire `emit_accepted`).
- `backend/api-server/src/index.ts` (register the archiver at boot, ~line 1245).
- `backend/api-server/src/routes/scoring-status.ts` (+ `.test.ts`) — evidence-based rewrite.
- `.env.example` (Gate-C env block).

---

## 9. GitHub
Branch `feat/gate-c-scoring-pipeline` off `feat/cartridge-hotpath-shadow`; commit; **push to `github`**;
open PR `feat(gate-c): wire Bayesian/Kelly scoring telemetry and paper-shadow validation`. PR body:
what it resolves, what it does NOT activate (zero capital/signer/broadcast), how to validate, commands run,
A.4 state (pending until script run with real RPC+executor), A.5 state (not started).

---

## 10. Divergences from the blueprint (and why)
1. **Persistence = stream → archiver**, not direct Rust PG insert (repo pattern; hot-path-safe). *Approved.*
2. **Migration 097**, not 072 (072 taken).
3. **Added `gate_c_validation`** table so A.4 evidence is readable by the VPS api-server (cannot read a local marker file).
4. `recommended_usd` derived via the real wei primitive from a **hypothetical USD paper cap** (no price oracle).
5. `scoring_pipeline_wired` is **structural**, with `WIRED_NO_RUNTIME_SAMPLE` before the first row (so quiet markets don't re-block) — adjustment #1.
6. A.5 uses a **four-state** machine; "calibrated" is never claimed without a real window — adjustment #6.

---

## 11. Risks & residual
- A.4 cannot run here (needs real `RPC_HTTP_1` + `EXECUTOR_1` + archive node) — operator/infra-gated; the script makes it reproducible.
- A.5 needs ≥7 days real shadow — time-gated; cannot be code-completed now.
- ERC-20 storage-layout assumptions in `multistep_fork` may need per-token tuning (A.4 surfaces this).
- Flat-prior scores are intentionally uninformative until A.5 — documented, not a defect.

---

## 12. Acceptance criteria
`scoring_pipeline_not_wired` has a real solution (module + wired emitter + migration + archiver + ≥1 row OR
passing unit test); `ConfidenceScore` produced per paper opportunity; `scored_opportunities` exists; A.4 + A.5
have reproducible scripts; the dashboard distinguishes wired vs calibrated via distinct evidence-derived states;
no live trading / signing / broadcast / capital; tests pass; docs exist; GitHub branch + commit + PR updated.
