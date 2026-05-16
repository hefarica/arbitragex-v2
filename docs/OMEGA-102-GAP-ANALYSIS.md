# OMEGA-102 · Gap Analysis (Path B delta pack)

**Generated:** 2026-05-16
**Author:** OMEGA agent team (Claude Opus 4.7) for @hefarica
**Mode:** Audit + delta (Path B) per operator decision
**Branch:** `feat/omega102-delta-pack` off `main`
**Companion docs:** all the OMEGA-102 delta docs in this PR

## Why this doc

The OMEGA-102 PR body (received 2026-05-16) proposed **16 artefacts** to materialize. Before authoring all of them verbatim, this team audited the repo state and found significant divergence between the PR body's assumptions and the actual codebase. This document is the honest record of:

1. What the PR body said
2. What the repo actually has
3. What this PR delivers
4. What this PR deliberately does NOT deliver, and why
5. Open follow-up items for the operator

The goal is **zero fabrication**. Every artefact in this PR is grounded in actual repo state, and every artefact the PR body proposed but is NOT here has an explicit reason documented below.

## What the PR body proposed vs. what the repo had

| # | PR body item | Status before this PR | Decision | Reason |
|---|---|---|---|---|
| 1 | `.github/workflows/codeql.yml` | absent | **AUTHORED** | Genuinely missing. Complements `security.yml`. |
| 2 | `.github/workflows/deploy-production.yml` | covered by `hardened-vps-deploy.yml` (PR #72 OMEGA-8/M2) | **SKIPPED** | Authoring a second workflow would duplicate and create ambiguity. Documented existing one via `docs/DEPLOY_PRODUCTION.md` instead. |
| 3 | `.github/workflows/dependabot.yml` | covered by `.github/dependabot.yml` (in repo) | **SKIPPED** | Dependabot config lives at `.github/dependabot.yml`, not in `workflows/`. Existing config is already well-tuned (includes major-bump ignores for hot-path crates). |
| 4 | `.github/workflows/policy-check.yml` | absent | **AUTHORED** | Genuinely missing. Hourly visibility + secret-scanning + CODEOWNERS check. |
| 5 | `patches/01_lru_bump.patch` (lru 0.12 → 0.13, RUSTSEC-2026-0002) | `lru = "0.12"` confirmed in `backend/Cargo.toml:64`, lockfile pins `0.12.5` | **SKIPPED — see follow-up #1** | The repo's own `.github/dependabot.yml` explicitly **ignores major bumps for `lru`** because it "touches hot-path concurrency / HTTP middleware and require dedicated benchmark sprints". Applying the bump unilaterally would violate the documented dependency policy. |
| 6 | `patches/02_postcss_override.patch` (postcss <8.5.10, GHSA-qx2v-qp2m-jg93) | frontend uses `@tailwindcss/postcss ^4.3.0` (Tailwind 4 chain) | **SKIPPED — see follow-up #2** | The PR body assumes Next 14 with old postcss chain. The frontend has already moved to Tailwind 4 which uses a different PostCSS pipeline. `npm audit` is the right tool to confirm whether the vuln is still live. |
| 7 | `scripts/apply_omega102_fixes.sh` | absent | **SKIPPED** | This script's purpose was to apply the two patches above. Since both patches are skipped (with reason), the script has nothing to do. |
| 8 | `scripts/smoke-tests.sh` (7 checks post-deploy) | absent | **SKIPPED — see follow-up #3** | Smoke tests require real VPS endpoints, real authentication, and knowledge of the docker-compose layout. Authoring without verification = fabrication. The existing `hardened-vps-deploy.yml` already runs health checks; documenting their semantics via `docs/DEPLOY_PRODUCTION.md` is the delta this PR provides. |
| 9 | `scripts/rollback.sh` (DB + nginx + slot) | absent | **SKIPPED — see follow-up #3** | Same reason as smoke-tests.sh: requires real VPS topology. The existing deploy workflow already has rollback logic; a manual emergency rollback script needs verification against actual VPS state. |
| 10 | `scripts/setup_branch_protection.sh` | absent | **AUTHORED** | Genuinely missing. Idempotent. 14 required-check names parameterized. |
| 11 | `config/CODEOWNERS` | absent (neither at root nor at `.github/`) | **AUTHORED at `.github/CODEOWNERS`** | Canonical location for GitHub. |
| 12 | `config/pre-commit-config.yaml` | absent | **AUTHORED at `.pre-commit-config.yaml` (root)** | Canonical location for pre-commit tooling. |
| 13 | `docs/INCIDENT_RUNBOOK.md` | absent (`OPERATOR_RUNBOOK.md` exists but different scope) | **AUTHORED** | Genuinely missing. 10 incident classes (INC-01 to INC-10) + killswitch + tribunal close-out. |
| 14 | `docs/ROTATION_POLICY.md` | absent | **AUTHORED** | Genuinely missing. Secrets inventory + 90/180/365 cadences + 2026-2027 calendar. |
| 15 | `docs/DEPLOY_PRODUCTION.md` | absent | **AUTHORED** | Genuinely missing. Annotates existing `hardened-vps-deploy.yml` with the 7-gate model + R8 declared limits. |
| 16 | `docs/REPO_VISIBILITY_POLICY.md` | absent | **AUTHORED** | Genuinely missing. PRIVATE rule + privatize procedure + monitoring + INC-05 link. |

**Score:** 9 authored, 7 skipped with documented reasons.

## What this PR delivers

```
.github/
├── CODEOWNERS                          ← authored
└── workflows/
    ├── codeql.yml                      ← authored
    └── policy-check.yml                ← authored

.pre-commit-config.yaml                 ← authored (root)

docs/
├── DEPLOY_PRODUCTION.md                ← authored
├── INCIDENT_RUNBOOK.md                 ← authored
├── OMEGA-102-GAP-ANALYSIS.md           ← this file
├── REPO_VISIBILITY_POLICY.md           ← authored
└── ROTATION_POLICY.md                  ← authored

scripts/
└── setup_branch_protection.sh          ← authored (chmod +x)
```

## What this PR explicitly does NOT deliver

These follow-ups need operator decision before the corresponding artefact can be authored honestly.

### Follow-up #1 · `lru` RUSTSEC-2026-0002

**State:** `lru = "0.12"` in `backend/Cargo.toml`, lockfile pinned at `0.12.5`. Advisory `RUSTSEC-2026-0002` (IterMut unsound) applies.

**Conflict:** `.github/dependabot.yml` explicitly ignores major bumps for `lru`:

```yaml
ignore:
  - dependency-name: "lru"
    update-types: ["version-update:semver-major"]
```

with the rationale (as commented in dependabot.yml):

> "Added 2026-05-10 (audit re-run review): lru, dashmap, tower-http — these touch hot-path concurrency / HTTP middleware and require dedicated benchmark sprints."

**Decision needed from operator:**

- **(A) Run the benchmark sprint** — schedule a focused effort to validate `lru 0.13` against the hot-path workloads (mempool ingest, prioritization spine, simulator routing cache). If results match or exceed `0.12.5`, then update dependabot.yml to remove the major-bump ignore and merge the upgrade as a normal Dependabot PR.
- **(B) Document the residual risk** — keep `lru 0.12.5` with an explicit `cargo audit --ignore RUSTSEC-2026-0002 --reason "hot-path benchmark sprint pending, see issue #N"` and a target date.
- **(C) Inspect the advisory's actual exploitability** — the advisory is for `IterMut` unsoundness. If the codebase does NOT use `IterMut` on `lru::LruCache` (grep `IterMut` and `iter_mut`), the practical risk may be zero and (B) is the right answer.

This PR does NOT take any of these decisions for the operator.

### Follow-up #2 · `postcss` GHSA-qx2v-qp2m-jg93

**State:** frontend uses `@tailwindcss/postcss ^4.3.0`. Tailwind 4 has a different PostCSS internal pipeline than Next 14.

**Action needed:** run `cd frontend && npm audit --audit-level=high` in CI and confirm whether the advisory still surfaces. If yes, the patch (override `postcss ^8.5.10`) may apply; if no, the PR body's assumption is obsolete.

This is a 30-second check the operator can do. This PR does not pre-empt the result.

### Follow-up #3 · `smoke-tests.sh` + `rollback.sh`

**State:** the existing `hardened-vps-deploy.yml` workflow runs health checks inline (gate 6) and has rollback logic on gate 7 failure. The OMEGA-102 PR body proposed standalone scripts.

**Decision needed:** does the operator want **manual operator-runnable** versions of these (for emergency use outside CI), or are the in-workflow versions sufficient?

If yes, the operator should provide:
- The actual public endpoint URLs of the staging + prod environments
- The docker-compose layout on the VPS (`docker-compose.prod.blue.yml` / `.green.yml` / etc.)
- The nginx state directory path
- The expected rollback target (last-green tag pointer)

Then a follow-up PR can author these scripts grounded in real state.

### Follow-up #4 · rustfmt drift at `v2_shadow_replay.rs:436`

**State:** PR body claimed drift exists. This PR did not run `cargo fmt --check` locally (Rust toolchain not present in this session). Lines 430-445 inspected manually appeared normal.

**Action needed:** operator runs `cd backend && cargo fmt --all -- --check` and either confirms drift (then `cargo fmt --all` + commit) or invalidates the PR body's claim.

### Follow-up #5 · Constants outside `chains.rs`

**State:** PR body claimed constants need refactoring into `chains.rs`. The current `backend/shared-rs/src/chains.rs` has a router catalog. Without a `scripts/lint_no_hardcode.sh` (PR body proposed but absent from spec content), the specific violation is undefined.

**Action needed:** operator clarifies which constants. Most likely candidates from a grep would be hex-literal addresses in service code outside `chains.rs`. A follow-up PR can author both the lint script and the refactor once the target set is identified.

### Follow-up #6 · `ANVIL_FORK_URL` secret + reporter path

**State:** `foundry.yml` workflow exists and likely uses `ANVIL_FORK_URL`. PR body documents this as an item for the operator to set.

**Action needed:** operator confirms `ANVIL_FORK_URL` is set as a GH Actions secret (`gh secret list`) and that the reporter path in `foundry.yml` is correct. No code change required, just verification.

## Compliance with operator doctrine

- **Zero-Mocks (CLAUDE.md RULE-00):** every authored artefact references real paths from the actual repo tree. No fabricated endpoints. No imaginary scripts.
- **No-Hardcode:** every script reads token / URL / repo name from env vars with explicit failure messages on absence.
- **Fail-Honest (R8):** every doc has a "what this does NOT cover" section. This gap-analysis doc IS the R8 close-out for the whole PR.
- **Lexicón Absoluto:** docs use `Asimetría Topológica` where the operator's lexicon applies; CI/CD-only terminology (workflow, gate, smoke test) is retained as-is because those are infra terms not DeFi-finance terms.
- **OMEGA Team orchestration:** authoring agent: Claude Opus 4.7; co-author trailer applied in commit.

## Order of recommended next operator actions

1. **Read this gap analysis** end-to-end. Push back on any item where the agent's call seems wrong.
2. **Merge this PR** (after review). It introduces no breaking changes, only adds CI workflows, docs, and a config. The `pre-commit` hooks require operator opt-in (`pre-commit install`); they don't fire automatically on merge.
3. **Decide follow-up #1** (`lru` policy) — most consequential. Open a sub-issue.
4. **Run follow-up #2** (`npm audit`) — 30 seconds.
5. **Run follow-up #4** (`cargo fmt --check`) — 30 seconds.
6. **Privatize the repo** (`gh repo edit ... --visibility private`) and enable secret scanning + push protection (Fase 0 of original OMEGA-102 plan).
7. **Run `bash scripts/setup_branch_protection.sh`** (bootstrap mode, no `--enforce-required-checks` yet).
8. **Open follow-up issues** for #3 (smoke/rollback scripts) and #5 (constants refactor) for separate, scoped PRs.
9. **After first all-green merge:** run `bash scripts/setup_branch_protection.sh --enforce-required-checks` to lock in the 14 required checks.

That sequence completes the spirit of the OMEGA-102 rollout while respecting the repo's actual state.
