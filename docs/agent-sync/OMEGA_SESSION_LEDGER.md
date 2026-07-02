# OMEGA Session Ledger — coordination point of truth

> Consolidated on-main coordination index for the 5-session OMEGA engagement. Purpose: sync work,
> prevent collisions, keep the phase state honest. Grounded in a live `gh`/`git` sync on **2026-07-01**.
> This file executes nothing; it coordinates.

## Base truth
- **Authoritative repo:** `github.com/hefarica/arbitragex-v2`, branch `main` @ **`eec065b0`** (2026-07-01).
- ⚠️ **Do NOT trust `productivo_full`'s `origin` remote** — it is `arbx-git:/opt/git/...`, a VPS bare
  mirror that was **464 commits stale**. Always fetch the `github` remote (`github/main`) for main truth.
- **Mainnet posture:** gated (NOT prohibited) AND **code-locked** — `backend/relays-client/src/live_exec_policy.rs:84`
  refuses `chain_id==1` unconditionally. Lifting requires a deliberate code change + all 18 gates + hefarica.

## Phase state
| Phase | State | Evidence |
|---|---|---|
| PAPER | ✅ closed | deployed + runtime-verified + webapp validated (fail-honest, live-flip BLOCKED) |
| SHADOW | ✅ closed | observer-only, `live_allowed=false`, real opps rejected honestly |
| SEPOLIA readiness | 🟡 gated | runbook merged (#231/#247), M5 pipeline MERGED (#229), env `sepolia-deploy` (reviewer=hefarica), `SEPOLIA_RPC_URL` secret set, **DRY-RUN PASSED (run 28568548667, forge build + DeployTestnet sim, no broadcast)**; LIVE waiting operator `DEPLOYER_PRIVATE_KEY`+wallet+ETH (+`ETHERSCAN_API_KEY` or verify=false) + env approval |
| MAINNET readiness | 🔴 NO-GO | dossier #248: 3/18 met, 6 partial, 9 not-met, 5 P0 |

## Coordination artifacts (⚠️ proliferation — operator should consolidate)
Four coordination/readiness ledgers already exist as **unmerged** PRs; this file is the on-main consolidator:
- **#248** `docs/mainnet-readiness.md` — the gated go/no-go dossier (OPEN, current mainnet truth).
- **#241** `docs/omega/coordination/` — 5-session ledger (OPEN).
- **#243** `docs/omega-coordination-ledger` — omega multi-session ledger (DRAFT).
- **#239** `docs/omega` path-to-live status (OPEN).
- **#236** `docs/omega-final-readiness` — final readiness report (DRAFT).
- **Recommendation (operator):** pick ONE canonical location (this `docs/agent-sync/` or #241), close the rest to stop churn.

## Files in this ledger
`ACTIVE_WORKSTREAMS.md` · `BLOCKERS.md` · `OWNER_MATRIX.md` · `NEXT_ACTIONS.md` · `DECISION_LOG.md` ·
`TEST_EVIDENCE.md` · `NO_COLLISION_RULES.md`
