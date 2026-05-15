# Migration history — OMEGA / ArbitrageX v2

Single source of truth for the **operational status** of each `*.sql` file
under `database/migrations/`. The bash driver `run_migrations.sh` iterates
files in lexicographic order and applies any whose checksum is absent from
`schema_migrations`. This document explains the **non-obvious** parts of
that history: gaps, intentional duplicates, type conventions, and the
forward-only doctrine.

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
