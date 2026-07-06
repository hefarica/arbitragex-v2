---
description: Audit a Git URL end-to-end (read-only) and generate an exact implementation scaffold — files, endpoints, frontend, backend, DB/Redis, WebSocket, tests, deploy, security, next commits. Audit/scaffold/shadow only — never activates executor, wallets, capital, or broadcasts.
argument-hint: "[git-url] (defaults to https://github.com/hefarica/arbitragex-v2.git)"
---

# /git-url-e2e-auditor-scaffold

Invoke the **git-url-e2e-auditor-scaffold** skill and run a full end-to-end audit
+ scaffold against the provided repository.

**Target repo:** `$ARGUMENTS`
(If empty, use the default: `https://github.com/hefarica/arbitragex-v2.git`)

## Hard constraints (READ-ONLY DOCTRINE — do not violate)

- READ-ONLY on the target: shallow clone into a temp dir, never push, never modify upstream.
- NO executor, NO wallets, NO private keys, NO capital, NO live trading.
- NO broadcasts: never run `cast send`, `forge script --broadcast`,
  `eth_sendRawTransaction`, `eth_sendBundle`, or any relay submit.
- NO live flips: never set `live: true` or `*_MODE=live`. Shadow / paper / read-only only.
- ZERO invented results: report only observed files. Missing = say "not found".
- NO-HARDCODE: scaffolded operator values are `process.env.*` placeholders, never literals.

## Steps

1. **Load the skill.** Follow `~/.claude/skills/git-url-e2e-auditor-scaffold/SKILL.md`.
2. **Reachability (Phase 0).** `git ls-remote --heads "$ARGUMENTS"`. If unreachable/auth-required, stop and report; if it is the current local checkout, audit the local copy and say so.
3. **Acquire (Phase 1).** Shallow read-only clone into a temp dir; record the resolved SHA + default branch.
4. **Inventory (Phase 2).** Detect languages, package managers, monorepo layout, frameworks, infra — using `resources/e2e-layer-matrix.md`.
5. **Gap analysis (Phase 3).** Classify every checklist item PRESENT / PARTIAL / MISSING with file-path evidence across the 10 layers, using `resources/audit-checklist.md`.
6. **Scaffold (Phase 4).** For each MISSING/PARTIAL item, emit an exact scaffold entry using the templates in `templates/` (file path, typed skeleton with `TODO(scaffold)`, env placeholders, accompanying test).
7. **Next commits (Phase 5).** Ordered, smallest-safe-slice-first commit plan with verification commands.
8. **Deliver (Phase 6).** Output the 10-item delivery format from `SKILL.md`. End with temp-clone cleanup.

## Domain awareness (ArbitrageX / QuantumX)

If the repo is ArbitrageX/QuantumX, also classify these features end-to-end:
strategy upload, strategy validation, shadow runner, route builder live,
ejecución shadow/read-only. Reference the matching `arbx-*` gates
(`arbx-net-profit-gate`, `arbx-paper-trade-first`, `arbx-simulation-mandatory`,
`arbx-risk-limits-enforcement`, `arbx-pre-execute-checklist`,
`arbx-no-hardcode-doctrine`) rather than re-deriving the rules.
