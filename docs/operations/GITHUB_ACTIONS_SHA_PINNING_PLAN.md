# GitHub Actions SHA Pinning Plan

**Status:** PROPOSED — staged for a follow-up PR.
**Authored:** 2026-05-15 (OMEGA-8 / M2 Capa 1 / Fase 8)
**Audit reference:** `omega8_audit/CAPA_1_DEVOPS_AUDIT_REPORT.md` — findings P1-1, P1-6.

This document inventories every GitHub Actions reference in the
repository, classifies the supply-chain risk, and stages the pinning
work. Per OMEGA-8 / M2 spec rule 5, **no SHA may be applied without
verifying it from the action's official GitHub repo**; that verification
must be performed by an operator with network access at the time of
pinning, not by the autonomous M2 agent.

This PR (M2) therefore ships the plan + the audit table only. A separate
PR `feature/omega8-m2-actions-sha-pinning` will land the actual SHAs
after operator verification.

---

## 1. Inventory (as of `feature/omega8-m2-capa1-devops-hardening` HEAD)

| Action | Versions in tree | Owner | Trust tier | Pinning priority |
|--------|------------------|-------|------------|------------------|
| `actions/checkout`           | `@v4`, `@v6` | GitHub (org) | T1 (1st party) | unify on `@v6`; SHA pin optional |
| `actions/setup-node`         | `@v6`        | GitHub (org) | T1            | SHA pin optional |
| `actions/upload-artifact`    | `@v4`, `@v7` | GitHub (org) | T1            | unify on `@v7`; SHA pin optional |
| `Swatinem/rust-cache`        | `@v2`        | Swatinem     | T2 (org-led)  | **REQUIRED** — pin SHA |
| `dtolnay/rust-toolchain`     | `@stable`    | dtolnay      | T2            | **REQUIRED** — pin SHA (`@stable` is a mutable branch ref) |
| `foundry-rs/foundry-toolchain` | `@v1`     | foundry-rs   | T2            | **REQUIRED** — pin SHA |
| `gitleaks/gitleaks-action`   | `@v2`        | gitleaks     | T2            | **REQUIRED** — pin SHA |

Notes:
- T1 ("first-party") actions ship from `github.com/actions/*`. They are the
  least likely vector for a supply-chain takeover; SHA pinning is best
  practice but not the hard P1 blocker.
- T2 ("org-led") actions live in third-party orgs/users. A compromise of
  the maintainer account would allow malicious code to ship under the
  same `@vN` tag because Git tags are mutable. SHA pinning closes that
  window — once an SHA is in our workflow, only the workflow change
  itself can land a new version.

## 2. Version drift to fix first (cheap, no SHA required)

Some workflows still use older majors. Unify on the latest before pinning
(otherwise we'll pin obsolete code):

- `audit-vps-wiring.yml:48`           — `actions/checkout@v4` → `@v6`
- `audit-vps-wiring.yml:607`          — `actions/upload-artifact@v4` → `@v7`
- `audit.yml:255`                     — `actions/upload-artifact@v4` → `@v7`
- `c10-f1-recovery-step14-only.yml:241` — `actions/upload-artifact@v4` → `@v7`
- `deploy-frontend.yml:150`           — `actions/checkout@v4` → `@v6`
- `hardened-vps-audit.yml:536`        — `actions/upload-artifact@v4` → `@v7`
- `hardened-vps-baseline.yml:167`     — `actions/upload-artifact@v4` → `@v7`
- `hardened-vps-deploy.yml:791`       — `actions/upload-artifact@v4` → `@v7`
- `sync-vps-metadata.yml:152`         — `actions/checkout@v4` → `@v6`
- `sync-vps-metadata.yml:729`         — `actions/upload-artifact@v4` → `@v7`

All of these are VPS-flavoured deploy/audit workflows. M2 does NOT touch
them (per spec rule on no VPS contact); they should be unified together
with the SHA pinning PR after operator review.

## 3. Recommended pinning procedure

For each T2 action above:

1. Open `github.com/<owner>/<action>/tags` in a browser.
2. Click the version tag we use, copy the commit SHA from the release page.
3. Cross-check that SHA appears under `github.com/<owner>/<action>/commits`
   on the default branch at or before the release date.
4. Replace `@vN` with `@<sha>` and add a trailing comment `# vN.M.P (verified YYYY-MM-DD)`.
5. Stage in `dependabot.yml` under `package-ecosystem: "github-actions"` so
   future bumps surface as PRs (dependabot rewrites SHA + comment automatically).

### Example diff

```yaml
# before
- uses: gitleaks/gitleaks-action@v2

# after
- uses: gitleaks/gitleaks-action@cb7149b9b57195b609c63e8518d2c4f3eb536d31 # v2.3.7 (verified 2026-05-15)
```

## 4. Dependabot already covers GitHub Actions

`dependabot.yml` declares `package-ecosystem: github-actions` with daily
checks. Once SHA pinning lands, Dependabot maintains it: each minor/major
bump arrives as a PR with the new SHA + comment update.

## 5. Acceptance criteria for the follow-up PR

The `feature/omega8-m2-actions-sha-pinning` PR is acceptable when:

1. Every T2 action above is pinned by SHA.
2. The `@v4` / `@v7` drift in §2 is resolved.
3. Each new SHA carries a `# vN.M.P (verified YYYY-MM-DD)` comment.
4. `dependabot.yml` lists `github-actions` (already true today).
5. The workflows still parse (`yamllint`, plus a CI dry-run on `pull_request`).
6. No T1 (`actions/*`) action regressed in version.

## 6. Why this PR does NOT do the pinning

The M2 agent runs without network access to GitHub's tag API and would
have to guess SHAs from cached training data. Spec rule §5 forbids that
("No pinnear a SHA desconocido sin verificar fuente oficial"). Shipping
the doc + dependabot configuration is the auditable half; pinning is the
other half that requires operator interaction.

---

**Owner:** Operator
**Reviewer:** OMEGA-8 / Capa 1 audit lineage
