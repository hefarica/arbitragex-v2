# Migration history — OMEGA / ArbitrageX v2

Single source of truth for the **operational status** of each `*.sql` file
under `database/migrations/`. The bash driver `run_migrations.sh` iterates
files in lexicographic order and re-applies **every** file on every deploy
deliberately (no applied-state ledger — re-running everything is what lets
the deploy gate heal repo↔DB drift, e.g. the manually-applied 102 that the
pipeline had never run). Idempotency is therefore the migration author's
responsibility, and per `lint-migration-rerun-lock-safety.sh` so is
**rerun-lock-safety**: on tables with continuous live writers
(`opportunities`, `simulations`, `paper_trade_runs`) the no-op path of any
`CREATE INDEX` / `ALTER TABLE` / `DROP|CREATE TRIGGER` must be catalog-guarded
(`DO $$ IF NOT EXISTS(<catalog read>) THEN EXECUTE '...'`) because PostgreSQL
acquires the table lock **before** the `IF NOT EXISTS` check — an unguarded
no-op starves against the runner's FREEZE-01 `lock_timeout=10s` and aborts
the deploy (observed 2026-08-30, ac08da8b attempt 2). This document explains
the **non-obvious** parts of that history: gaps, intentional duplicates,
type conventions, and the forward-only doctrine.

> **Forward-only doctrine.** No migration is ever renumbered or renamed
> once applied. If a mistake is shipped, a new forward migration with the
> next available number fixes it. Renaming an applied file would orphan its
> checksum and force a reapply that may not be idempotent.

---

## Numbering gaps

### `064_*.sql` and `065_*.sql` — INTENTIONALLY ABSENT

Both numbers were reserved during M2 (omni entity registries planning) and
ultimately collapsed into `066_omni_entity_registries.sql` and
`067_config_hash_registry_drift_runtime_ack.sql` when the design was
unified. The reservations were never published; nothing was applied.

**Do NOT renumber later migrations to fill the gap.** The `schema_migrations`
ledger does not enforce consecutivity and operators reading this file are
expected to know that gaps exist.

---

## Lexicographic duplicates

### `012_edge_persistence.sql` and `012_recon_reports.sql`

Both ship the `012` prefix because they were authored in parallel during
sprint S0/S1 and merged the same day. They touch **disjoint table sets**
(no overlap), so applying them in either order is safe. The bash driver
applies them in filename lexicographic order: `012_edge_persistence.sql`
first, then `012_recon_reports.sql`.

**Do NOT rename either** post-application — the checksum in
`schema_migrations` would no longer match and the driver would attempt to
re-apply, producing `CREATE TABLE` errors.

---

## Type conventions for hash columns

The repository uses two physical types for sha256 hashes:

| Migration | Column                                | Type      | Reason                              |
|-----------|---------------------------------------|-----------|-------------------------------------|
| 067       | `runtime_ack.config_hash_*`           | `CHAR(64)`| New tables; tight 64-char hex.      |
| 067       | `config_hash_registry.hash_value`     | `CHAR(64)`| Same family.                        |
| Earlier   | `audit_log.*hash*`, various           | `TEXT`    | Legacy; predate the convention.     |

**New columns** SHOULD use `CHAR(64)` for sha256 hex storage. **Existing
columns** are NOT being migrated to `CHAR(64)` retroactively; the cost of
rewriting partition keys is not justified.

---

## Address columns

Three styles coexist:

- `VARCHAR(42)`            — legacy, predates the lower-case convention.
- `TEXT CHECK (regex)`     — preferred; enforces `^0x[a-f0-9]{40}$` (lowercased).
- `TEXT` (no check)        — temporary inserts during seed; tolerated.

New code SHOULD use the regex-checked variant. A future migration may
normalize the legacy `VARCHAR(42)` columns but the audit team needs to sign
off on the join-cost cascade first.

---

## ON DELETE behaviour summary

- **CASCADE** (parent row gone → child row meaningless): `simulations →
  opportunities`, `executions → opportunities`, `pool_reserves → pools`,
  `route_legs → routes`, `paper_trade_runs → opportunities`,
  `recon_reports → opportunities` and `→ executions`.
- **SET NULL** (parent optional / weak ref): `risk_events → opportunities`,
  `sed_*` chains, `rpc_endpoints / relay_endpoints → service_credentials`.
- **NO ACTION** (implicit; orphan tolerated): `routes → pools`. This is a
  P3 smell flagged in OMEGA-8/M3 Capa 2; routes are derived/ephemeral.

---

## OMEGA-8 / M3 additions (2026-05-15)

The M3 milestone added three migrations under the "capa 2 hardening" PR:

| File                                            | Purpose                                                  |
|-------------------------------------------------|----------------------------------------------------------|
| `069_runtime_ack_idempotency_unique.sql`        | UNIQUE (event_id, layer) + CHECK chain_id > 0 + partial idx for failures. Closes P0/P1 invariant I-2 enforcement. |
| `070_audit_event_pii_hardening.sql`             | Retroactively anonymizes `audit_event.ip_address` → CIDR; hashes `user_agent`. Closes P1-3 wired-in gap. |
| `071_capa2_p2_fixes.sql`                        | `arbx_prune_runtime_ack` retention helper (7d floor, 90d ceiling); `config_hash_registry` UNIQUE NULLS NOT DISTINCT (PG15+) or partial-index fallback. |

All three are forward-only and idempotent; re-running any of them on a
DB that already has them applied is a no-op.

**Rollback policy for M3 migrations**: documented inline in each `.sql`
header. None are automated. The dedupe archive table
`runtime_ack_dedupe_archive_069` is preserved forever (forensic).

---

## G-SIM-1 PR-B2b Fase 2 (A1) — 2026-07-04

| File                                    | Purpose                                                  |
|-----------------------------------------|----------------------------------------------------------|
| `099_opportunities_route_metadata.sql`  | Adds `route_metadata` JSONB column to `opportunities` storing complete route topology (`pool_addresses[]`, `token_addresses[]`, `dex_adapters[]`, `decimals{}`) for sim-ctl `OpportunityCandidate` reconstruction. GIN index on `pool_addresses`. Default `'{}'` for backward compat. |

A1 enrichment path data-source foundation. Forward-only and idempotent.

---

## G-SIM-1 FASE 2 (readiness evidence registry) — 2026-08-16

| File                                    | Purpose                                                  |
|-----------------------------------------|----------------------------------------------------------|
| `104_readiness_evidence.sql`            | Append-only evidence store for gate readiness checklists: `readiness_evidence` (latest row per `(gate_id, item_key)`, `status` constrained to `evidenced\|failed`) + sister `readiness_evidence_history` (PK `(gate_id, item_key, verified_at)`, never updated or deleted). Written only by api-server `POST /admin/readiness-evidence` (history insert → upsert, one transaction). Generalizes the `scripts/run_a4_fork_validation.sh` `gate_c_validation` INSERT + marker-file mechanism with freshness (30-day `is_fresh`, computed by readers) + provenance (`evidence_ref`, `verified_by`). |

Forward-only and idempotent. The FASE 3 verifier reads PG directly via its own
pool; `GET /admin/readiness-evidence` is a read-only operator convenience.

---

## STRAT-IDENT-01 (per-strategy scoring identity) — 2026-08-23

| File                                    | Purpose                                                  |
|-----------------------------------------|----------------------------------------------------------|
| `108_strategy_identity_scoring.sql`     | Re-keys the Gate-C calibration store per STRATEGY: `scored_opportunities.strategy_key` (nullable, no backfill — R8) + `bayesian_priors.strategy_key` with UNIQUE replacing the per-pair unique (table empty, no writer existed). Operator directive: each of the 264+ strategies declares its own applicable structures (primary/secondary operators) — scoring/calibration must accumulate per strategy, never per class (pair / router / family). |

Forward-only and idempotent. Paper-only telemetry.

---

## S4-03 (simulation label no-contamination gate) — 2026-08-29

| File                                    | Purpose                                                  |
|-----------------------------------------|----------------------------------------------------------|
| `111_paper_trade_runs_calibration_eligibility.sql` | S4 runbook (accepted 2026-08-29): `paper_trade_runs` gains `sim_fail_family` (S4-02 taxonomy structural\|economic\|market), `calibration_eligible` (FALSE = terminal structural failure — never a Stage 2b label, never retried), `sim_attempts` + `sim_last_attempt_at` (pending backoff 30s·2^min(n,7) for 501/parse-error attempts), plus a partial `CREATE INDEX CONCURRENTLY` (FREEZE-01 doctrine: populated live table) for the drift-tracker pending scan. Writer: recon drift-tracker (Capa B). |

Forward-only and idempotent. Paper-only telemetry. Depends on: 051 (paper_trade_runs), 099 (route_metadata).

---

## SIMWIRE-02b/02c (sim-ctl stream consumer hardening) — 2026-08-30

| File                                    | Purpose                                                  |
|-----------------------------------------|----------------------------------------------------------|
| `112_simulations_simulator_revm.sql`    | SIMWIRE-02b: `simulations.simulator` CHECK extended with `'revm'` (the 004 CHECK predates simulator-v2; without it every route-aware insert fails). SIMWIRE-02c P1-8: DROP+ADD wrapped in an explicit transaction — no constraint-less window under psql autocommit. Content edit post-merge is safe: the runner records by filename, VPS already skipped, fresh DBs get the atomic version. |
| `113_simulations_revm_idempotency.sql`  | SIMWIRE-02c P1-5: partial UNIQUE index `(opportunity_id) WHERE simulator='revm'` — XAUTOCLAIM redelivery of an entry whose final XACK failed after persist+XADD must not double-publish. Powers `insert_simulation`'s `ON CONFLICT … DO NOTHING` → `Ok(false)` → caller skips the downstream XADD (exactly-once). Partial on purpose: legacy anvil multi-attempt history stays untouched. `CREATE UNIQUE INDEX CONCURRENTLY` per FREEZE-01 doctrine (simulations is a populated live table; partial predicate matches zero pre-flip rows so the build is near-instant). |

Forward-only and idempotent. Paper-only telemetry. Depends on: 004 (simulations CHECK), 112 ('revm' allowed).

---

## GEN-CI-FAIL rerun-lock-safety retrofit — 2026-08-30

The deploy of `ac08da8b` (SIMWIRE-02c) aborted at the [4/9] MIGRATION GATE:
`CREATE INDEX IF NOT EXISTS idx_opp_status_time` on the live `opportunities`
table was cancelled by `lock_timeout` (10s) while the index already existed in
prod — PostgreSQL requests the table ShareLock **before** the `IF NOT EXISTS`
catalog check, so the runner's every-deploy re-run raced continuous searcher
INSERTs and lost. The same latent shape existed in every pre-retrofit
migration touching a hot table: bare `CREATE INDEX IF NOT EXISTS` (ShareLock),
`ALTER TABLE ... IF NOT EXISTS` / `DROP CONSTRAINT IF EXISTS`
(AccessExclusiveLock), `DROP|CREATE TRIGGER` (ShareRowExclusiveLock).

**Retrofit (content edits — safe under the ledger-less runner because the new
content is idempotent AND lock-free on the no-op path):** files `003`, `004`,
`025`, `033`, `049`, `051`, `054 §4a`, `091`, `099`, `100`, `102`, `103`,
`107`, `111`, `112` converted to catalog-guarded `DO $$ ... EXECUTE` blocks.
Fresh DBs get identical end-state; prod re-runs take no table lock.

**Doctrine going forward:** on hot tables, a genuinely-missing index on a
*populated* table is rebuilt by a dedicated `CREATE INDEX CONCURRENTLY` fixer
(the `105` pattern) — never by relaxing the guard or the runner's
`lock_timeout`. Enforced in CI by
`automation/tools/lint-migration-rerun-lock-safety.sh` (selftest via
`--selftest`); `ALTER TABLE <hot> SET|RESET (reloptions)` stays exempt
(ShareUpdateExclusiveLock does not conflict with writers).
