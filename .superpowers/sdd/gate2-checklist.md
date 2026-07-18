# Gate 2 — CI / Supply Chain Hardening Checklist

> **Status:** In Progress (partially applied, pending operator actions)
> **Target:** Transform CI from advisory/false-green to blocking/required-check grade

---

## Changes Applied

### 1. GitHub Actions Pinned to SHA

| Workflow | Action | Before | After |
|----------|--------|--------|-------|
| ci.yml | actions/checkout | `@v5` | `@93cb6efe18208431cddfb8368fd83d5badbf9bfd` |
| docker-build.yml | actions/checkout | `@v5` | `@93cb6efe18208431cddfb8368fd83d5badbf9bfd` |
| e2e.yml | actions/checkout | `@v5` | `@93cb6efe18208431cddfb8368fd83d5badbf9bfd` |
| foundry.yml | actions/checkout | `@v5` | `@93cb6efe18208431cddfb8368fd83d5badbf9bfd` |
| frontend-build.yml | actions/checkout | `@v5` | `@93cb6efe18208431cddfb8368fd83d5badbf9bfd` |
| integration-tests.yml | actions/checkout | `@v5` | `@93cb6efe18208431cddfb8368fd83d5badbf9bfd` |
| rust.yml | actions/checkout | `@v6` | `@df4cb1c069e1874edd31b4311f1884172cec0e10` |
| security.yml | actions/checkout | `@v5` | `@93cb6efe18208431cddfb8368fd83d5badbf9bfd` |
| semiotic.yml | actions/checkout | `@v5` | `@93cb6efe18208431cddfb8368fd83d5badbf9bfd` |
| typescript.yml | actions/checkout | `@v5` | `@93cb6efe18208431cddfb8368fd83d5badbf9bfd` |
| unit-tests.yml | actions/checkout | `@v5` | `@93cb6efe18208431cddfb8368fd83d5badbf9bfd` |
| wallet-security.yml | actions/checkout | `@v5` | `@93cb6efe18208431cddfb8368fd83d5badbf9bfd` |
| codeql.yml | actions/checkout | `@v5` | `@93cb6efe18208431cddfb8368fd83d5badbf9bfd` |
| codeql.yml | codeql-action/init | `@v4` | `@66235dd0e4afa86b13b7702a92ec90713d4e8683` |
| codeql.yml | codeql-action/autobuild | `@v4` | `@66235dd0e4afa86b13b7702a92ec90713d4e8683` |
| codeql.yml | codeql-action/analyze | `@v4` | `@66235dd0e4afa86b13b7702a92ec90713d4e8683` |
| security.yml | gitleaks-action | `@v2` | `@e0c47f4f8be36e29cdc102c57e68cb5cbf0e8d1e # v3` |

### 2. npm install → npm ci

| Workflow | Before | After |
|----------|--------|-------|
| ci.yml | `npm install` | `npm ci --no-audit --no-fund` |
| frontend-build.yml | `npm install` | `npm ci --no-audit --no-fund` |

### 3. Gitleaks Hardening

- **Removed** `continue-on-error: true` from gitleaks job
- **Added** post-gitleaks diff scan step
- **Documented** semantic validation rationale for ContractAdminPanel exclusion in `security.test.ts`

### 4. Rust / Foundry CI

- **Removed** `continue-on-error` from rust fmt, clippy, and test jobs
- **Removed** `continue-on-error` from Foundry test job

---

## Operator Actions Required

- [ ] Configure branch protection on `main` to require these checks:
  - `ci-gate` (ci.yml)
  - `cargo audit` (security.yml)
  - `gitleaks` (security.yml)
  - `rust-check` (rust.yml)
  - `foundry-test` (foundry.yml)
  - `frontend-build` (frontend-build.yml)
  - `typescript-check` (typescript.yml)
  - `unit-tests` (unit-tests.yml)
  - `integration-tests` (integration-tests.yml)
  - `e2e` (e2e.yml)
  - `codeql` (codeql.yml)
  - `wallet-security` (wallet-security.yml)
- [ ] Rotate leaked PAT (4 credentials leaked 2026-06-15 — see memory)
- [ ] Install SBOM tools: `cargo cyclonedx`, `npm install -g @cyclonedx/cyclonedx-npm`
- [ ] Enable GitHub Attestations or configure Sigstore cosign for attestation

---

## Remaining Risks

| Risk | Severity | Mitigation |
|------|----------|------------|
| Docker image tags still mutable in compose files | High | Pin to digests in `docker/compose.prod.yml` (Gate 6 covers this) |
| No SBOM generation yet | Medium | Add optional steps once tools installed |
| Branch protection not configured yet | High | Operator must enable in GitHub settings |
| ContractAdminPanel exclusion by filename could regress | Low | Semantic test in `security.test.ts` guards against mainnet leakage |

---

## Acceptance Criteria

- [ ] All workflows pass on a test PR with no `continue-on-error` bypass
- [ ] `npm ci` succeeds with frozen lockfile
- [ ] Gitleaks blocks on any new secret pattern
- [ ] Branch protection requires all listed checks
- [ ] SBOM generated and attached to release
