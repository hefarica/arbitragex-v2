# OMEGA BLOCKERS (re-verified ground truth, V3 §7)

| ID | Plane | Blocker | Evidence | Owner |
|----|-------|---------|----------|-------|
| **B1** | LIVE (root) | cross-DEX 1a fix NOT on main — only in PR #224 (unmerged). Deployed executor still cannot run 2-router cross-DEX arbs. | `git show origin/main:contracts/src/ArbitrageExecutor.sol` → 0 matches for `TokenOutRetentionViolation`/`AliasedTwoLegRoute` | S4 / operator |
| **B2** | PAPER | `scanner.rs` only dispatches single-tx `execute_round_trip_revm` (reverts by design — no EXECUTOR_ROLE/balance/approval); the multistep/prefund REVM path is BUILT (`sim_multistep.rs`,`sim_prefund.rs`,`sequence_runner.rs`) but UNWIRED → ~0 SIM_SUCCESS (0 is honestly reported). | scanner.rs has no reference to `sim_multistep::` | S3 |
| **B3** | PAPER | Shadow-mode telemetry: in `OrchestratorMode::Shadow` the searcher writes nothing to `paper_trade_runs` (only V1/V2 feed the archiver). | `paper-trade-archiver.ts` header NOTE | S2 / S3 |
| **B4** | M5 / LIVE | #229 is NOT keyless — `deploy_live` injects `DEPLOYER_PRIVATE_KEY` → `--broadcast` via `deploy-m5.sh` (hidden from a yaml-only grep). | `gh pr diff 229`; E4 (#238) flags `deploy-m5.sh:8/49/54` | S4 |
| **B5** | CI hygiene | cargo-audit `RUSTSEC_IGNORES` allowlist **expires 2026-06-30**; the gating `rust.yml` + `cargo audit` are NOT in branch-protection `required_status_checks`. | `security.yml` comment; branch-protection API | S5 |
| **B6** | operator-console | `operatorIdentityMiddleware` is DEAD (never wired) → `/api/operator/*` perma-401 `OPERATOR_MISSING_IDENTITY`. | git grep finds 0 `app.use/router.use` of it | S1 / S2 |
| **B7** | non-blocking | #232's only red = NON-required `TypeScript integration (live Postgres+Redis)` ECONNRESET testcontainer flake — does NOT gate merge. | `gh pr checks 232` (24 pass / 1 non-required fail) | — |
