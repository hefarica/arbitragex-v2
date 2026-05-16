# Production Deploy Anatomy

**OMEGA-102 Delta Pack · 2026-05-16**
**Canonical workflow:** `.github/workflows/hardened-vps-deploy.yml` (already in repo, OMEGA-8/M2/M3 work)
**Companion docs:** `INCIDENT_RUNBOOK.md` (INC-03: deploy mid-flight failure), `ROTATION_POLICY.md`, `OPERATOR_RUNBOOK.md`

## Why this doc exists

The OMEGA-102 PR body proposed a new `deploy-production.yml` workflow. After audit, the repo already has `hardened-vps-deploy.yml` (PR #72) that covers the same scope, hardened through the OMEGA-8 work. **Authoring a second deploy workflow would duplicate and create ambiguity about which one is canonical.**

This document instead **annotates** the existing workflow with the 7-gate model from OMEGA-102 + R8 declared limits. It is the operator's reference when reading the workflow, when reading deploy logs, and when handling INC-03.

If `hardened-vps-deploy.yml` materially diverges from the 7-gate model below, **this document is wrong, not the workflow**. Update the doc, do not retrofit the workflow.

## The 7 gates — what each one actually does

A production deploy passes through 7 gates in sequence. Failure of any gate halts the deploy and (depending on gate) triggers rollback.

| # | Gate | What it verifies | What happens on failure |
|---|---|---|---|
| **1** | Confirm environment | Operator @hefarica has approved the GitHub Environment `production-vps` for this run. Without approval, the workflow waits indefinitely (or times out, depending on env config). | Workflow stays in "Waiting" state. Operator can either approve or cancel. No mutation. |
| **2** | CI green | Every required status check is green on the exact SHA being deployed. Driven by `scripts/setup_branch_protection.sh --enforce-required-checks` (once enforced). | Workflow fails before touching artifacts. No mutation. |
| **3** | Build images | `docker buildx build --platform linux/amd64,linux/arm64` for each service. Images pushed to `ghcr.io/hefarica/arbitragex-v2/<service>:<sha>`. Each image is then signed with `cosign --keyless` and an SBOM is generated via Syft; the SBOM is scanned by Trivy. | Workflow fails. No image becomes "release-tagged" in GHCR. Previous deployment remains live. |
| **4** | Pre-deploy snapshot | Captures recoverable state BEFORE any mutation: `pg_dump` of PostgreSQL, `BGSAVE` of Redis, `tar` of nginx state, and the on-chain vault state (executor contract storage proof, if applicable). Snapshots are stored on the VPS at `/opt/arbitragex-v2/snapshots/<sha>/`. | Workflow fails. Previous deployment remains live. |
| **5** | Deploy green slot | SSH to VPS, `docker compose -f docker-compose.prod.green.yml up -d <services>`, run `sqlx migrate` (if pending). The green slot is NOT yet serving traffic — only the blue slot is. | Workflow fails. Green slot is torn down. Blue (previous) remains serving. |
| **6** | Smoke tests | 7 HTTP checks against the green slot (run from the VPS itself or a sidecar runner): `/status`, `/health`, `/api/opportunities/live`, `/api/executions/recent`, killswitch verification, websocket connectivity, edge worker handshake. | Workflow fails. Green slot is torn down. Blue remains serving. |
| **7** | Cutover OR rollback | If smoke tests passed: nginx upstream swap (`blue → green`) within a single config reload. If any check between 3 and 6 failed: `scripts/rollback.sh` runs automatically, restoring blue to canonical and discarding green. | If cutover itself fails (rare — nginx reload error), see INC-03: operator must SSH and manually restore. |

## Trigger procedure (operator)

```bash
# Confirm the SHA you intend to deploy (must be on main):
COMMIT_SHA=$(git rev-parse origin/main)
echo "deploying: $COMMIT_SHA"

# Trigger:
gh workflow run hardened-vps-deploy.yml --ref main \
  -f confirm_sha="$COMMIT_SHA" \
  -f confirm_environment=production-vps
```

Approve the GH Environment prompt when GitHub asks (this is gate 1). The workflow then proceeds through gates 2-7 unattended.

There are NO valid alternative trigger paths:
- ❌ `--admin` flag (bypasses branch protection)
- ❌ `git push` directly to `main` from operator workstation
- ❌ Manual SSH `docker compose up` on VPS
- ❌ Webhook from any external system

Any of those bypasses are incident class **INC-09** (branch protection bypassed) and require postmortem.

## Success metrics

A deploy is considered successful when:

1. All 7 gates pass green within the workflow's timeout (typically 30 min total).
2. nginx reload completes without error.
3. For 15 minutes post-cutover, every health check on the green slot continues to pass (no rollback triggered by post-deploy monitoring).
4. `recon` job detects no anomalous balance drift attributable to the new deploy.

If any of (3) or (4) fails post-cutover, that is INC-03 with a rollback path.

## R8 — Declared limits of this deploy model

Per fail-honest doctrine, here is what the 7-gate model does NOT guarantee:

1. **Cosign keyless OIDC** requires a healthy GitHub Actions OIDC token. If GitHub's OIDC provider has an outage, builds will fail at gate 3. There is no local-keyless fallback configured. Mitigation: a planned outage window only.

2. **TEE attestation** is assumed to live on a SGX/SEV-SNP-capable VPS for the signing path. If the VPS does not have hardware TEE, the attestation step in `hardened-vps-deploy.yml` will be a stub. Operator must verify VPS hardware before relying on the attestation envelope.

3. **Blue/green requires ~8GB of headroom RAM** on the VPS to run both slots simultaneously during gates 5-7. If the VPS is RAM-constrained, the workflow falls back to a `recreate` strategy (stop blue → start green), which has a brief downtime window (~30s). The fallback is automatic but should be confirmed via VPS sizing before each major release.

4. **Smoke tests validate HTTP surface**, NOT functional correctness of MEV strategy logic. A green deploy can still produce incorrect opportunity scoring or unsafe execution decisions. Correctness validation lives in:
   - `cargo test --workspace` (in CI, gate 2)
   - `shadow-replay` continuous evaluation (out of band, NOT part of deploy)
   - Operator-driven canary on a single strategy after deploy
   If shadow-replay or canary detects regression, escalate to INC-03 or INC-08 as appropriate.

5. **Database rollback** has a Recovery Point Objective of **15 minutes** (the `pg_dump` cadence between gate 4 and a hypothetical rollback). Transactions written between snapshot and rollback are lost. There is no point-in-time recovery configured beyond this. Mitigation: WAL-shipping to off-site is a separate workstream (not OMEGA-102 scope).

These are not flaws — they are the **explicit operating envelope**. If the operator needs guarantees outside this envelope, that is a separate engineering project, not a deploy-workflow tweak.

## Relationship to OMEGA-8

OMEGA-8/M2 (PR #78) added `hardened-vps-deploy.yml`. OMEGA-8/M3 (PR #76) hardened the DB + Redis path that gate 4 depends on. OMEGA-8/M4 (PR #80) hardened backend services so smoke tests at gate 6 cover the surface. This doc + the rest of OMEGA-102 delta pack are the **operator-facing layer** on top of that hardening.
