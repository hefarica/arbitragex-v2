# RECONCILIATION — local audit vs live `main`

> **INTERNAL.** Generated 2026-06-29. Resolves the provenance caveat in `MASTER_AUDIT.md` by verifying each finding against a fresh shallow clone of `github.com/hefarica/arbitragex-v2` at **`bb46845`** (Merge PR #215, 2026-06-28 — the current `main`).

## Headline

The local working copy at `…/arbitragex-v2-main (17)/arbitragex-v2-main/` is a **STALE snapshot predating PR #191 and #197**. Two of the audit's findings are **already fixed on `main`** and must be dropped from the backlog. **Six of the eight CRITICALs are CONFIRMED REAL on current `main`** and constitute the genuine live-readiness backlog. My local `price-validator` work (spec/plan/Phase 0–2/mig 098) is **not on `main`** (unpushed).

**➡ Future work must be based on a fresh `main` clone, not the local `(17)` copy.**

## Per-finding status on `main` (bb46845)

| Finding | Local audit | Status on `main` | Verdict |
|---------|-------------|------------------|---------|
| **C2 — Foundry CI rigged (`\|\| true`)** | rigged | **FIXED** — `foundry.yml:60` `run: forge build --sizes`, `:66` `run: forge test -vv` (no `\|\| true`); comments confirm "BLOCKING: test failures now fail CI. Previously masked by `\|\| true`". Only `forge install … \|\| true` remains (install step). | ✅ **DROP** (stale; fixed by #191). Minor: install-step `\|\| true` left. |
| **Domain 9 / DRIFT-A — specs fictional (18/100)** | fictional | **FIXED** — `openapi.yaml` killswitch body `{enabled}` ("NOT `active`", :477-478), 55 paths (not 8), `x-arbx-admin-token`; `asyncapi.yaml` "transport is **Socket.IO**", explicitly "NO raw-`ws` on :8081 and NO `/ws/*` channels", rooms + `subscribe:<room>`, handshake admin-gated. | ✅ **DROP/RE-SCORE** (stale; fixed by #197). Domain 9 on `main` ≈ 70+, not 18. |
| **C1 — Fabricated live-readiness cert chain** | real | **REAL** — `paper-shadow.yml:15-16` "# For now, we mock the logic / DAYS=14"; `no-regression.yml:14-16` echo "All checks passed!" + OMEGA-LIVE-DECLARATION; `dr-drill.yml:36,39,43` "Mock Live Deploy" + `sha256sum` labeled cosign. | 🔴 **STANDS** (top critical). |
| **C3/D1 — `.env.edge` live config committed** | real | **REAL + WORSE** — `git ls-files .env.edge` → tracked/committed; `.gitignore` does **NOT** list it (its own line 6 "Esta en .gitignore" is FALSE). Live `GAS_SPONSOR_PRIVATE_KEY=0x…`, "Capital expuesto controlado > 0". | 🔴 **STANDS, escalate** (committed live-key config in public repo). |
| **C4 — sim↔broadcast divergence** | real | **REAL** — `relays-client/src` has **0** refs to `ArbitrageExecutor`/`executeArbitrage`; `bundle_builder.rs:20,95,101` builds tx to `routers_for_chain(...).address`. | 🔴 **STANDS** (matches memory @bb46845). |
| **C5 — phantom mainnet-promotion schema** | real | **REAL** — `crucible_runs`: 0 migration files; `chains_runtime` (mig 061): 0 `mode` hits; `admin-promote-mainnet.ts:167` still `INSERT INTO chains_runtime (chain_id, mode, …)`. | 🔴 **STANDS**. |
| **C6 — solc/via-ir deploy drift** | real | **REAL** — `foundry.toml:10,13` solc 0.8.24/`via_ir=false` vs `Makefile:34,38` `SOLC_VERSION?=0.8.23` + `--use solc:… --via-ir`. | 🔴 **STANDS**. |
| **C7/D2 — VPS IP leak** | 60+ files | **REAL — 76 files** on `main` contain the IP. | 🔴 **STANDS** (matches memory "75+ files"). |
| **HIGH — hot path `router.call(payload)`** | line 253 | **REAL** — `ArbitrageExecutor.sol:411 (bool success,) = router.call(pld);` (line moved; #215 refactor). | 🔴 **STANDS** (line 411). |
| **HIGH — operator RBAC dead** | dead | **REAL** — `operatorIdentityMiddleware` has **1** occurrence in `backend/api-server/src` (definition only; no call site). | 🔴 **STANDS**. |
| **HIGH — AllowanceManager no per-trade cap** | real | **REAL** — `AllowanceManager.sol:64` still `MAX_SAFE_ALLOWANCE` ceiling; no per-trade/maxSpend bound. | 🔴 **STANDS**. |
| **price-validator (28/100)** | local work | **NOT ON MAIN** — no `backend/price-validator/` dir, no mig 098. My spec/plan/Phase 0–2 are local-only (unpushed). | ⚪ **local-only** — decide push vs keep. |

## Corrected critical backlog (verified on `main bb46845`)

Drop C2 and the specs-drift domain (already fixed upstream). The **real** live-readiness backlog, in safety order:

1. **C3/D1** — remove `.env.edge` from version control + history, rotate `GAS_SPONSOR_PRIVATE_KEY` and any treasury keys, add it to `.gitignore` for real. *(Its committed state is the single most urgent safety item.)*
2. **C7/D2** — repo-wide scrub of the VPS IP across 76 files.
3. **C5** — make the mainnet-promotion path fail-closed (phantom `crucible_runs` + `chains_runtime.mode`).
4. **C1** — delete/replace the fabricated certification chain (it can still emit a fake "certified for live").
5. **C4** — close sim↔broadcast calldata divergence before any live trade.
6. **C6** — fix multichain deploy determinism + governance wiring.
7. HIGHs: hot-path `router.call` + per-trade spend cap; operator RBAC (wire or remove); AllowanceManager fail-closed default.

## Recommended adjustment to the plan

- **`IMPLEMENTATION_PLAN.md` P2 (un-rig CI)** is **largely already done on `main`** (#191). Re-scope it to: (a) leave the blocking `forge build/test` as-is, (b) remove the residual `forge install … || true`, (c) the real remaining CI work is **C1** (the fabricated certification chain is separate from the un-rigged Foundry job and is still live).
- **P4 (specs isomorphism)** is **largely already done on `main`** (#197). Re-scope to: add the *blocking drift gate* (so the now-correct specs can't drift again) + the operator-RBAC + dev/prod-edge work, which are still real.
- Re-base all subsequent phases on a fresh `main` checkout; discard the local `(17)` snapshot for engineering (keep it only as the home of these audit docs + the unpushed price-validator work).
