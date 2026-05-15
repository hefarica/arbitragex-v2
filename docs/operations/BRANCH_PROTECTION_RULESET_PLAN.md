# Branch Protection / Ruleset Plan — `main`

**Status:** PROPOSED — pending explicit operator authorization.
**Authored:** 2026-05-15 (OMEGA-8 / M2 Capa 1 / Fase 3)
**Audit reference:** `omega8_audit/CAPA_1_DEVOPS_AUDIT_REPORT.md` — finding P0-1.
**DO NOT** apply via `gh api` or GitHub Settings until the operator
(Hector Fabio Riascos Castro) signs off in writing on the PR that ships
this document.

---

## 1. Evidence of risk

The Capa 1 DevOps audit confirmed:

- `main` is the deployment branch — every commit on `main` is a candidate
  for VPS deploy via `hardened-vps-deploy.yml`.
- No branch protection ruleset is currently configured on the GitHub repo.
- Force-push and direct-push to `main` are technically possible.
- A CI failure does not block merge: status checks are advisory.
- A single reviewer (or self-merge) can land code that touches
  `docker/compose.prod.yml`, `.github/workflows/hardened-vps-deploy.yml`,
  `backend/` and `contracts/` simultaneously.

The blast radius is the production VPS. Each unguarded merge to `main`
is a potential live-trading incident. Spec OMEGA Rule 02 forbids this
posture in production.

## 2. Required status checks

The following checks must be **required** before any merge to `main`.
Names are the GitHub Actions job names as they appear today.

| Workflow | Job name | Notes |
|----------|----------|-------|
| `rust.yml`           | `cargo check + clippy + test` | hard gate today |
| `typescript.yml`     | `tsc --noEmit (all workspaces)` | hard gate today |
| `security.yml`       | `cargo audit (Rust advisories)` | hard gate today |
| `security.yml`       | `gitleaks (secrets scan)` | hard gate today |
| `security.yml`       | `npm audit (prod deps, high+)` | hard gate today |
| `no-hardcode.yml`    | `lint`                      | hard gate today |
| `foundry.yml`        | `forge build + test`        | hard gate today |
| `dockerfile-audit.yml` | `audit Dockerfiles for complete COPY coverage` | hard gate today |
| `frontend-build.yml` | `next build (production)`   | created in M2 Fase 6 |
| `omega8-m3-grep-gates.yml` | `grep gates` | hard gate today |

**Recommended (advisory → required after 5 consecutive greens):**
- `e2e.yml — playwright` (currently `continue-on-error: true` per its own §7 risk-register note)

## 3. Ruleset rules

```
Require a pull request before merging
  Required approvals: 1
  Dismiss stale approvals when new commits are pushed: true
  Require review from Code Owners: false (no CODEOWNERS today; revisit when added)

Require status checks to pass before merging
  Require branches to be up to date before merging: true
  Required checks: (list from §2)

Require linear history: true        # squash-only — see §4
Require deployments to succeed: false
Block force pushes: true
Block deletions: true
Restrict creations: false           # creating new branches is fine
Restrict updates: true              # only via PR — admins still bypass with audit
Restrict deletions: true            # same as above

Bypass list:
  - Repository administrators (Hector Fabio Riascos Castro)
    * Justification: emergency rollback path. Each bypass must produce a
      Slack notice in #infra-changes within 1 hour.
```

## 4. Merge strategy: squash-only

The whole OMEGA workflow is built on conventional, single-purpose squash
commits (see PR #73, #74, #75, #76 history). The ruleset MUST disable
merge commits and rebase merges to enforce this. GitHub repo settings →
"Pull Requests" → uncheck "Allow merge commits", uncheck "Allow rebase
merging", keep "Allow squash merging".

## 5. `gh api` commands (DO NOT EXECUTE)

The following are reference commands. **Operator approval required.**

```bash
# Create the ruleset (preferred over legacy branch-protection API).
# JSON body is in this file; do NOT inline secrets.
gh api \
  --method POST \
  -H "Accept: application/vnd.github+json" \
  /repos/Riascos-arbitragex/arbitragex-v2/rulesets \
  --input docs/operations/branch-protection-ruleset.json
```

Sample `branch-protection-ruleset.json` (committed alongside this plan
for review; **never** posted until authorised):

```json
{
  "name": "Protect main",
  "target": "branch",
  "enforcement": "active",
  "conditions": {
    "ref_name": { "include": ["refs/heads/main"], "exclude": [] }
  },
  "rules": [
    { "type": "deletion" },
    { "type": "non_fast_forward" },
    { "type": "required_linear_history" },
    { "type": "pull_request",
      "parameters": {
        "required_approving_review_count": 1,
        "dismiss_stale_reviews_on_push": true,
        "require_code_owner_review": false,
        "require_last_push_approval": false,
        "required_review_thread_resolution": false
      }
    },
    { "type": "required_status_checks",
      "parameters": {
        "strict_required_status_checks_policy": true,
        "required_status_checks": [
          { "context": "cargo check + clippy + test" },
          { "context": "tsc --noEmit (all workspaces)" },
          { "context": "cargo audit (Rust advisories)" },
          { "context": "gitleaks (secrets scan)" },
          { "context": "npm audit (prod deps, high+)" },
          { "context": "lint" },
          { "context": "forge build + test" },
          { "context": "audit Dockerfiles for complete COPY coverage" },
          { "context": "grep gates" },
          { "context": "next build (production)" }
        ]
      }
    }
  ],
  "bypass_actors": [
    {
      "actor_id": 1,
      "actor_type": "RepositoryRole",
      "bypass_mode": "always"
    }
  ]
}
```

## 6. Rollback plan

If the ruleset blocks an emergency hotfix:

1. Operator (admin bypass) merges via `gh pr merge --squash` with explicit
   acknowledgement in PR description (`OPS_BYPASS_REASON: <incident-id>`).
2. Open a post-mortem ticket within 24 hours.
3. If the ruleset itself is the problem, disable via
   `gh api --method PUT /repos/.../rulesets/<id> -f enforcement=disabled`
   and reopen a corrective PR.

## 7. Why this PR does NOT apply the ruleset

OMEGA-8 / M2 Capa 1 spec rule 4 (REGLAS ABSOLUTAS):

> **NO modificar GitHub branch protection directamente sin autorización
> explícita del operador.**

This file ships the plan, the JSON body and the rationale so the operator
can audit every required check by name before signing off. Application is
a one-line `gh api` call once authorised; reverse is also one line.

---

**Owner:** Operator (Hector Fabio Riascos Castro)
**Reviewer:** OMEGA-8 / M3 DevOps audit lineage
**Apply-after-approval owner:** to be assigned in the approval comment.
