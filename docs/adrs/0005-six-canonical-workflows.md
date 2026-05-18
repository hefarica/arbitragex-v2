# Six Canonical CI/CD Workflows

* Status: accepted
* Date: 2026-05-18
* Deciders: @hefarica
* Consulted: C-RUNNER, K-CORE
* Informed: All team members

## Context and Problem Statement

CI/CD was ad-hoc with inconsistent checks across PRs. Some PRs had security audits, others didn't. There was no clear definition of "ready to merge" and no automated enforcement of quality gates.

## Decision Drivers

* Consistent quality gates for every PR
* Security must not be optional
* Build artifacts must be signed and traceable
* Deployment must be automated and reversible

## Considered Options

* Option A: Single mega-workflow — simple but inflexible
* Option B: Event-driven workflows — complex but flexible
* Option C: Six canonical workflows — balanced separation of concerns

## Decision Outcome

Chosen option: "Option C — Six Canonical Workflows", because each workflow has a single responsibility.

### Consequences

* Good, because clear separation of concerns
* Good, because each workflow can be updated independently
* Good, because branch protection can enforce all six
* Bad, because more files to maintain
* Bad, because cross-workflow dependencies require orchestration

## The Six Workflows

1. **ci.yml** — Lint, typecheck, build, test (Node + Rust + Contracts)
2. **security.yml** — npm audit, cargo audit, gitleaks, trivy, dependency review
3. **docker-build.yml** — Multi-platform build, SBOM, cosign keyless signing
4. **deploy-vps.yml** — SSH deploy, healthchecks, automatic rollback
5. **e2e.yml** — Playwright tests with banned pattern checks
6. **codeql.yml** — SAST for JavaScript/TypeScript, Rust, Solidity

## More Information

* All workflows are in `.github/workflows/`
* Branch protection requires all six to pass
* No `continue-on-error` anywhere
