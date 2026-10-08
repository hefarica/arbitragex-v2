# OMEGA TASK BOARD

> From the FASE A forensic audit (re-verify; the repo moves). **PAPER ≈68% · LIVE ≈22% = PREPARED-BUT-GATED.**
> Signal types (V3 §12): CHECK-DERIVED unless labelled otherwise.

## Critical path (single-rooted on #224)
| # | Task | Owner | State | Note |
|---|---|---|---|---|
| 1 | Merge **#224** (cross-DEX 1a + Layer-C sim↔broadcast parity) into main | S4 / operator | BLOCKED | mergeStateStatus=BLOCKED, CI re-running; root blocker for CROSSDEX_1A + M5 + cross-DEX SIM_SUCCESS |
| 2 | Wire multistep/prefund REVM into `scanner.rs` behind `paper_mode=true` → real SIM_SUCCESS | S3 | BLOCKED on #1 | the on-chain fix it exercises lands via #224 |
| 3 | Close Shadow-mode telemetry (publish dry-run sims to the `paper-trade-archiver` stream) | S2 / S3 | PENDING | today only V1/V2 feed `paper_trade_runs` |
| 4 | Land #232 (operator risk presets) | operator | READY-on-required | 24 pass / 1 NON-required fail (TS-integration flake) |
| 5 | Land #235 (FE honesty), #226 (IP scrub, unblocked), #238 (E4) | operator | READY-on-required | sequence: #235 → #238 → #226 → #232 |
| 6 | Build M5 canary tooling + make #229 keyless (delete `deploy_live`) | S4 | PENDING (after #224) | E4 (#238) enforces keyless once merged |

## This session's lane (non-colliding, reversible)
- **DONE:** FASE A 8-gate audit + `ARBX_PATH_TO_LIVE_STATUS.md`; ethics-guard **E4 (#238)**; **#226** VPS_SSH_HOST reuse; this ledger; cargo-audit memory correction.
- **NEXT candidates (non-colliding):** residual-flake triage; cross-cutting safety guards; evidence runs (WSL2 cargo/forge/vitest); reconcile my FASE A status doc with S2's #236 readiness report.
