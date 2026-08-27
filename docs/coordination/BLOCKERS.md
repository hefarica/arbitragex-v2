# BLOCKERS — evidence-based (FASE 0 verified @ main b151144)

## PAPER SHADOW (= PARTIAL, not DONE)
- **B-PS1 — paper data never surfaces.** executions written ONLY by live relays-client (`backend/relays-client/src/persistence.rs:13-44`); api-server bridge/paper/route-discovery sinks **dormant** (`ARBX_OPPS_BRIDGE_MODE`/`PAPER_ARCHIVER_MODE`/`ROUTE_DISCOVERY_OUTCOMES_SINK = off`, `index.ts:1489,1509`). → /executions,/recon,/operations honest-empty. Owner S2.
- **B-PS2 — SIM_SUCCESS near-unreachable.** `sim_prefund.rs`+`sim_multistep.rs` dead foundation (not called from scanner hot path); single-tx path reverts (no prefund). Owner S3 — being wired by #224.
- **B-PS3 — no runtime/fork proof.** cargo unrunnable locally (Win WDAC, see arbx-wsl-test-runner). Needs WSL/CI fork run.
- **B-PS4 — honest-display invariant has NO blocking CI test.** `—`-not-`$0.00` enforced only by `frontend/lib/store/types.ts:207-212` + an orphaned spec; `frontend/e2e` unwired, no `@playwright/test`. Owner S1 — **THIS SESSION'S NEXT LOT**.

## LIVE (= PREPARED-BUT-GATED)
- **B-LV1 — GATE_CROSSDEX_1A/PARITY (#224):** open, behind main; contract fix real+tested; parity sound but not byte-e2e-proven.
- **B-LV2 — GATE_M5 (#229/#231):** honesty-compliant, depends on #224.
- **B-LV3 — gates not all READY → live blocked by design (correct).**

## CROSS-CUTTING
- **B-X1 — migration 098 TRIPLE collision:** main `098_tokens_decimals_smallint.sql` vs #170 `098_seed_cartridge_apex_strategies.sql` vs memory'd validator-098. Must renumber to 099+.
- **B-X2 — TS-integration CI flake** (testcontainers Postgres race), non-required, noisy on many PRs. Needs S5 stabilization.

## HUMAN GATES (cannot cross without textual GO)
merges · Sepolia/mainnet deploy · signer/private-key/KMS · broadcast · on-chain approvals/grants · paper→live.
