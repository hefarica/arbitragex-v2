# Gate C — Scoring Pipeline + A.4/A.5 Validation

**Posture:** shadow / paper / read-only. **Zero capital, zero signer, zero broadcast, no live trading.**
`recommended_usd` is a **hypothetical paper figure**, never real money.

This document explains how the three dashboard blockers are resolved with **verifiable
evidence**, not flipped constants. Scoring can be **wired before it is calibrated** — the
dashboard reports exactly which stage it is in.

---

## 1. The three blockers

| Blocker | Was | Now resolved by |
|---|---|---|
| `scoring_pipeline_not_wired` (HIGH) | Bayesian + Kelly primitives compiled but never invoked per opportunity | `scoring_pipeline.rs` invoked from `OpportunityEmitter::emit_accepted`; a `ConfidenceScore` is produced + XADDed per paper opportunity |
| `a4_fork_validation_not_executed` (CRITICAL) | scoring untrusted vs mainnet until A.4 passes | `scripts/run_a4_fork_validation.sh` runs the ignored `multistep_fork` test and records a `gate_c_validation` row only on a real pass |
| `a5_paper_shadow_not_executed` (HIGH) | flat priors only; no calibration from real flow | `scripts/activate_paper_shadow.sh` + the four-state A.5 machine; priors calibrate from accumulated `scored_opportunities` / `bayesian_priors` |

---

## 2. Architecture (repo-native, hot-path-safe)

```
searcher-rs  OpportunityEmitter::emit_accepted
   └─ scoring_pipeline::evaluate_paper_opportunity  (bayes_update + accept_by_posterior
                                                      + kelly_fraction + compute_position_size)
        └─ XADD arbx:scoring:scored   (fire-and-forget, NON-FATAL, never blocks emission)
              └─ api-server  ScoredOpportunitiesArchiver  (passive consumer group)
                    └─ INSERT scored_opportunities  (ON CONFLICT(stream_id) DO NOTHING)
                          └─ GET /api/v1/scoring/status  (evidence-derived states)
```

- The **hot path never touches Postgres** — it only XADDs to Redis. The archiver (api-server)
  is the sole writer, mirroring `paper-trade-archiver.ts` / `route-discovery-outcome-sink.ts`.
- The XADD and the archiver are both **non-fatal**: a Redis/DB error never breaks emission.

---

## 3. Why no premature hard-gate (flat priors)

With no calibrated history (`paper_trade_runs = 0`), every per-pair posterior is the **flat
Beta(1,1) prior** (mean 0.5). Hard-gating emission on that would reject everything and empty
the dashboard. Therefore:

- `ARBX_SCORING_HARD_GATE=false` by **default** — scoring is **advisory**; rejected candidates
  still flow downstream (RULE-00 transparency). `opps:detected` is unchanged.
- `posterior_prob` is built from REAL history via `bayes_update(Beta(1,1), profitable, unprofitable)`.
  No win is fabricated; flat prior ⇒ `source_context = "flat_prior"`. Once A.5 populates
  `bayesian_priors`, `source_context = "calibrated"`.
- Setting `ARBX_SCORING_HARD_GATE=true` (operator opt-in) skips the active downstream emit for
  Bayesian-rejected candidates **but still persists the score** (telemetry never dropped).

`recommended_position_usd` reuses the real wei primitive `compute_position_size` over a
**hypothetical** paper bankroll (`ARBX_KELLY_MAX_CAPITAL_USD`), then converts micro-dollars back
to USD. It is a paper sizing figure only.

---

## 4. Dashboard states (`GET /api/v1/scoring/status`)

Derived from runtime evidence (Postgres), never constants:

- `scoring_pipeline_state`: `BLOCKED` (migration 097 absent) → `WIRED_NO_RUNTIME_SAMPLE`
  (wired, no row yet — a quiet market is **not** a failure) → `WIRED_RUNTIME_SAMPLE` (≥1 row).
- `a4_state`: `A4_PENDING` → `A4_PASSED` (a `gate_c_validation` row exists).
- `a5_state`: `PAPER_SHADOW_NOT_STARTED` → `PAPER_SHADOW_WARMING` → `CALIBRATED_CANDIDATE`
  (≥ `ARBX_PAPER_SHADOW_MIN_DAYS` span **and** ≥ `ARBX_CALIBRATION_MIN_SCORED` rows) → `CALIBRATED`
  (per-pair priors with `observation_count ≥ ARBX_CALIBRATION_MIN_OBSERVATIONS`).

A blocker only appears in `blocked_reasons` while it is genuinely unresolved.

---

## 5. Environment variables

| Var | Default | Meaning |
|---|---|---|
| `ARBX_SCORING_ENABLED` | `true` | Produce a ConfidenceScore per paper opportunity |
| `ARBX_SCORING_HARD_GATE` | `false` | Block emission of Bayesian-rejected candidates (still persists score) |
| `ARBX_BAYESIAN_MIN_POSTERIOR` | `0.50` | Posterior-mean floor for `bayesian_accepted` |
| `ARBX_BAYESIAN_MAX_STD` | `1.00` | Posterior-std ceiling for `bayesian_accepted` |
| `ARBX_KELLY_FRACTION` | `0.25` | Cap on the applied Kelly fraction |
| `ARBX_KELLY_MAX_CAPITAL_USD` | `5000` | **Hypothetical paper** bankroll for `recommended_usd` |
| `ARBX_SCORING_GAIN_ON_WIN` | `2.0` | Kelly gain-on-win input |
| `ARBX_SCORING_LOSS_ON_LOSS` | `1.0` | Kelly loss-on-loss input |
| `ARBX_SCORING_ARCHIVER_MODE` | `off` | api-server archiver dormant unless `on` |
| `ARBX_PAPER_SHADOW_MIN_DAYS` | `7` | A.5 span before `CALIBRATED_CANDIDATE` |
| `ARBX_CALIBRATION_MIN_SCORED` | `100` | A.5 scored-row volume before `CALIBRATED_CANDIDATE` |
| `ARBX_CALIBRATION_MIN_OBSERVATIONS` | `30` | Per-pair prior observations before `CALIBRATED` |

---

## 6. Migration

`database/migrations/097_scored_opportunities_gate_c.sql` (forward-only, idempotent):
`scored_opportunities`, `bayesian_priors`, `gate_c_validation`, and the `gate_c_metrics` view.
Apply with the standard migrator (`automation/scripts/migrate.sh`) on the VPS.

---

## 7. How to validate

```bash
# Rust adapter + emitter wiring
cargo test -p searcher-rs scoring_pipeline --lib --manifest-path backend/Cargo.toml
cargo test -p searcher-rs --lib opportunity_emitter --manifest-path backend/Cargo.toml

# Dashboard evidence logic
npm --prefix backend/api-server run test -- scoring-status

# A.4 / A.5 script guards
bash scripts/gate_c_scripts_test.sh

# A.4 fork validation (needs a REAL archive RPC + deployed executor)
RPC_HTTP_1=<archive_rpc> EXECUTOR_1=<0xexecutor> [DATABASE_URL=...] \
  bash scripts/run_a4_fork_validation.sh

# A.5 paper-shadow activation (after A.4 passes)
[DATABASE_URL=...] bash scripts/activate_paper_shadow.sh
```

---

## 8. How to read the dashboard

`GET /api/v1/scoring/status` → look at `scoring_pipeline_state`, `a4_state`, `a5_state`,
`recent_scored_count`, `last_scored_at`, `calibrated_pairs`, and `blocked_reasons`. "All wired"
shows as `scoring_status: "enabled"` once the table exists and the pipeline is wired.

---

## 9. How to turn it off

- `ARBX_SCORING_ENABLED=false` — emitter stops scoring (emission unchanged).
- `ARBX_SCORING_ARCHIVER_MODE=off` (default) — api-server stops persisting (stream still trims via MAXLEN).
- Revert the branch — the only runtime change with defaults is an extra non-fatal XADD + log.

---

## 10. Doctrinal guarantees

- No mocks, no fabricated scores/posteriors/Kelly, no invented prices, no fabricated opportunities.
- No signer, no broadcast, no live trading, no capital, no contract edits, no private keys.
- RiskGate not bypassed; blockers never hidden; validations never disabled; errors never converted to success.
- `arbx:opps:detected` invariant intact (default config adds only a non-fatal scoring XADD).
