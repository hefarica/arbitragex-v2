# Deploy Workflows — Canonical Map

**Authored:** 2026-05-15 (OMEGA-8 / M2 Capa 1 / Fase 11)
**Audit reference:** `omega8_audit/CAPA_1_DEVOPS_AUDIT_REPORT.md` — P2/P3 cleanup.

The repo contains several deploy / VPS-touch workflows, the result of an
iterative M1/M2/M3/M4/M5 deploy hardening effort. Without a clear map,
the wrong workflow can be triggered by accident with the wrong SHA, the
wrong cache flag, or the wrong target. This file is the canonical
reference; treat anything not listed here as historical.

## 1. Canonical deploy path (M5 hardened)

| Workflow file | Purpose | Trigger | Status |
|---------------|---------|---------|--------|
| `.github/workflows/hardened-vps-deploy.yml` | Full prod deploy with audit gates (M5) | `workflow_dispatch` only | **CANONICAL — use this** |
| `.github/workflows/hardened-vps-audit.yml`  | Read-only forensic audit of the prod VPS | `workflow_dispatch` only | **CANONICAL audit path** |
| `.github/workflows/hardened-vps-baseline.yml` | Baseline state snapshot for diff against future audits | `workflow_dispatch` only | **CANONICAL baseline** |

The hardened-vps-deploy workflow ships:
- Inline change-type guards (database/migrations requires `require_db_backup_done=true`).
- SSH connectivity precheck.
- Execution lock acquisition.
- Audit trail attached to the run.

**Operator note:** A pending PR (#75) renames the SSH secret used here
from `VPS_SSH_PRIVATE_KEY` to `VPS_SSH_KEY`. Until that lands the
workflow will fail at the SSH step. M2 Fase 1 deliberately did NOT
duplicate that fix.

## 2. Legacy / surgical workflows (DO NOT use for full deploy)

| Workflow file | Reason kept | When to use |
|---------------|-------------|-------------|
| `.github/workflows/deploy.yml` | Earlier APEX deploy variant; still works | Emergency rollback path if hardened deploy is itself broken. Document the bypass in #infra-changes. |
| `.github/workflows/deploy-edge-only.yml` | Hot-swap edge container only | Edge worker hotfix without rebuilding hot-path Rust services. |
| `.github/workflows/deploy-edge-only-v2.yml` | Same as v1 with hard subtree sync (`git checkout -- edge/`) | Edge hotfix when working tree on VPS has accumulated stash drift. |
| `.github/workflows/deploy-frontend.yml` | Frontend-only deploy | Next.js hotfix without touching backend. |

All four are `workflow_dispatch` only — they cannot fire on push or PR.

## 3. Pure diagnostic / read-only workflows

| Workflow file | Purpose |
|---------------|---------|
| `.github/workflows/audit.yml`               | Generic audit |
| `.github/workflows/audit-wiring.yml`        | Frontend ↔ Edge ↔ API wiring audit |
| `.github/workflows/audit-vps-wiring.yml`    | Wiring audit on the live VPS |
| `.github/workflows/diag-cookie-emission.yml`| Cookie emission diagnostics |
| `.github/workflows/probe-admin-session.yml` | Admin session probe |
| `.github/workflows/probe-cookies-deep.yml`  | Deep cookie diagnostics |
| `.github/workflows/sync-vps-metadata.yml`   | Pull metadata from the VPS |
| `.github/workflows/verify-admin-session-wiring.yml` | Admin session wiring verification |
| `.github/workflows/action-a-plus.yml`       | "A+ posture" verification |
| `.github/workflows/action-a-plus-v2.yml`    | A+ v2 |
| `.github/workflows/c10-f1-recovery-step14-only.yml` | Targeted C10/F1 recovery step |

All are `workflow_dispatch` only. They observe VPS state; they do not
mutate the production stack.

## 4. CI workflows (no VPS contact)

| Workflow | Gate |
|----------|------|
| `rust.yml`             | cargo check + clippy + lib tests |
| `typescript.yml`       | tsc --noEmit + vitest (hard gate as of M2 Fase 7) |
| `unit-tests.yml`       | Rust + TS unit tests parity |
| `foundry.yml`          | forge build + test + fork test (fork-guarded as of M2 Fase 5) |
| `security.yml`         | cargo audit + gitleaks + npm audit |
| `no-hardcode.yml`      | Doctrine lint |
| `dockerfile-audit.yml` | COPY-coverage verifier |
| `omega8-m3-grep-gates.yml` | Capa 2 invariant grep gates |
| `frontend-build.yml`   | `next build` production-like (new in M2 Fase 6) |
| `monitoring-config.yml`| promtool check config + rules (new in M2 Fase 10) |
| `e2e.yml`              | Playwright smoke (advisory until 5 consecutive greens) |

## 5. Legacy file at repo root: `docker-compose.edge.yml`

`docker-compose.edge.yml` (282 lines, OMEGA SOP-EDGE-001 §5.2 header)
predates the unified `docker/compose.{dev,prod}.yml` split. It is NOT
referenced by any workflow today (verified via grep on the M2 branch).
Two options for the operator:

- **Keep as historical reference** — leave a deprecation banner at the
  top stating "non-canonical; use docker/compose.prod.yml". M2 ships this
  banner (see the file diff).
- **Remove entirely** — requires a separate PR to ensure no internal docs
  or runbooks link to it. Out of scope for M2.

## 6. Spec-derived rule

Before triggering ANY workflow that contains "deploy" or "vps" in the
filename, the operator must:

1. Confirm the SHA matches what passed CI on `main`.
2. Confirm any required secret rename (e.g. `VPS_SSH_KEY` post PR #75)
   has actually landed.
3. Announce the deploy in #infra-changes.
4. Post the run URL in the same channel within 5 minutes.

The hardened deploy enforces (1) via an inline guard. (2-4) are operator
discipline.
