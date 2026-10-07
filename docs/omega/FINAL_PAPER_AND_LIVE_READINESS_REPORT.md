# ArbitrageX v2 — Final Paper & Live Readiness Report

**Date:** 2026-06-30 · **Author:** Claude (Opus) session 3ea4d3f4 · **Canonical repo:** `hefarica/arbitragex-v2` (`arbitragex_v2_productivo_full`)
**Doctrine:** fail-honest, zero-mocks, fail-closed-until-gated. No false greens in this report.

---

## 0. VERDICT (one line)

- **PAPER SHADOW: ~READY** — pipeline live + honest (fail-closed/idle is correct, not broken).
- **LIVE: PREPARED-BUT-GATED** — all code complete & CI-green; the single structural gate to land it is **merging PR #224**, which is blocked by a **merge-window coordination issue**, not by code.

---

## 1. Initial state (start of this session)
- Canonical main was at `bb46845`. Open drafts: #216 (AMM vectors), #217 (FUSILE docs), #218 (M2 carry-through Part-1, inert). Several latent findings: a structural sim↔broadcast false-green ("HIGH"), the executor cross-DEX 2-router blocker, cargo-audit red (rmcp/anyhow advisories).
- Local Rust build/test is WDAC-blocked (verify via CI). `cargo update` works locally (no compile).

## 2. What was already resolved (entering)
- Readiness panels existed (`/api/readiness` 17-item doctrinal + `readiness-steps` 4-step + `readiness-extras` `/blockers` + `/decision`).
- The REVM fork harness + `simulator-v2` (real revm, not a stub) + the A.4 `multistep_fork` test already existed.
- The cross-DEX blocker was already reproduced + fixed ("1a", commit `b285a0e`) on a branch.

## 3. What this session did (real, evidenced)
| PR | What | State |
|----|------|-------|
| **#223** | green cargo audit — justified `--ignore RUSTSEC-2026-0189` (rmcp non-applicable: stdio-only build) + `cargo update -p anyhow` 1.0.102→1.0.103 (RUSTSEC-2026-0190 real patch) | **MERGED** |
| **#221** | readiness honesty — `G-SIM-1` yellow→red when simulator-v2 is a stub (reordered before the metrics query so it's red even idle); TDD 3-fail→33-pass | **MERGED** |
| **#216** | math-engine external canonical AMM vectors + delete dead `*0.99` V3 placeholder | OPEN (CI green) |
| **#217** | FUSILE source-governance policy doc | **MERGED** |
| **#218** | M2 carry-through Part-1 (inert `ExecPayload`) | **CLOSED** — superseded by #224 |
| **#224** | M2 fund-path: cross-DEX `_runRoute` fix + `ValidatedPlan` verbatim sim↔broadcast parity | OPEN, **14/14 required green**, BEHIND |
| **#229** | M5 Sepolia validation pipeline (manual, fail-closed, no keys in CI) | OPEN (draft) |

Plus: deep reconciliation of #224 vs #218 (6-agent fan-out), and a **4-lens adversarial contract-security review of #224 = GO** (reentrancy/flash-atomicity, approval-hygiene, weird-token/capital-drain, access-control/economic — zero exploits within the threat model). External professional audit still required before mainnet capital.

## 4. PAPER SHADOW — operational truth (evidence)
- **#221 merged** → `G-SIM-1` now honestly **red** when simulator-v2 isn't activated (was a yellow that understated a structural gap).
- Live-probed `edge-arbx.ape-tv.net` this session: `/api/health` ok (uptime days); `/metrics` live; dapp served (Cloudflare→edge→Next.js); `/api/readiness` = `{green:14, yellow:2, red:1}` + `flip_blocked`; `/api/readiness/decision` = **verdict NO_GO**, `go_live:false`, `submit_enabled:false`, `paper_mode:true`, `capital_exposure_usd:0`; `/api/readiness/blockers` = `executor_1_missing`(critical)…
- **RULE-00 verified in prod:** `/api/opportunities/live` → `items:[] count:0`; `/api/rpc/status` → `chains:[] count:0` — honest empty states, zero mocks.
- `simulator-v2` is **real revm** (`SimulatorV2::simulate` → LazyDb → revm_runner; `execute_multistep_revm` → sequence_runner CacheDB), NOT a stub — but **opt-in** (`ARBX_USE_SIMULATOR_V2`), so the live path is **fail-closed** (no SIM_SUCCESS) until activated + contracts deployed. This is the correct posture, not a bug.
- **Honest debt:** the probed deploy is idle (0 chains loaded); `G-PAP-1` red (no ≥7-day paper run yet). "Paper 100%" is **not** claimed until a continuous paper-shadow run with chains loaded + simulator-v2 activated produces real paper executions.

## 5. LIVE — PREPARED-BUT-GATED (gate map, fail-honest)
> **GATE_CROSSDEX_1A** — **READY-in-#224.** `_runRoute` approve-per-hop (leg0 tokenIn/amountIn; leg1 tokenOut by exact intermediate delta; `forceApprove(0)` reset; `UnsupportedRouteLength`/`ZeroIntermediate`/`TokenOutRetentionViolation` fail-closed; SC-12/SC-13 preserved). Repro red→green (`ArbitrageExecutorCrossDexRepro.t.sol`). `forge fork test (mainnet)` passes. 4-lens adversarial review = GO.
>
> **GATE_PARITY_224 (Layer C)** — **READY-in-#224.** Producer seal RETARGETED to `sim_multistep::execute_multistep_revm` (wrapped-flash); fail-closes if `wrapped_calldata` missing; builds `ValidatedPlan{wrapped_calldata}`, persists `arbx:validated_plan:<opp.id>` only on SIM_SUCCESS; broadcast sends it VERBATIM (`bundle_builder::verbatim_broadcast_calldata`); byte-parity by construction + unit test. Seal verified intact on every rebased head (×6: 9c809b8→…→209415e). `execute_round_trip_revm` no longer the seal.
>
> **GATE_RUST_CI** — **READY.** #223 merged → cargo audit green on main. #224 = 14/14 required green (the only historical fail was the external advisory now fixed; `Rust integration` flaky is NON-required and #162 fixed its PG-ready race).
>
> **GATE_M5_SEPOLIA** — **READY-in-#229 (draft).** `m5-sepolia-validation.yml` (manual `workflow_dispatch`, Sepolia-only, dry-run default, live gated by sentinel + `sepolia-deploy` environment, hard chainId guard, **no keys in CI, no broadcast in CI**) + `deploy-m5.sh` + `validate-m5.sh` (delegates to existing `run_a4_fork_validation.sh`). p95-latency + 10-route gates honestly marked **NOT-INSTRUMENTED** (not faked green).
>
> **GATE_RISKGATE / OPERATOR_CONSOLE** — partially READY (readiness panels honest; risk caps `max_value_eth=0.0` paper). Full operator-console E2E not re-verified this session.
>
> **GATE_PATH_TO_MAIN** — **BLOCKED on the #224 merge window** (see §7). `#226` (P0 IP scrub functional) separately GATED on operator VPS secrets.

## 6. Multi-session awareness (honest, inferred from real repo evidence)
I have **no direct channel** to other Claude sessions/workspaces; I infer parallel activity **only** from real artifacts (PRs, branches, merge actors, CI). Observed:
- Parallel merges landed during this session — all `mergedBy=hefarica`: #222/#225/#227 (P0 security), #142/#139/#162/#230 (dependabot + CI guards). This indicates an active operator-driven merge stream (possibly other sessions feeding PRs).
- Open parallel workstreams (not mine): #232 (operator risk presets), #226 (P0 IP scrub), #220/#145/#144 (dependabot), #170 (cartridge APEX), #137/#136/#127 (governance/dapp/strategy-pack), branch `omega/m2-net-usd-gate@cf97153` (a net-USD follow-up).
- **Conflicts avoided:** I touched only my batch's files; #224's critical files (scanner.rs/bundle_builder/validated_plan/ArbitrageExecutor.sol) were verified non-overlapping with #221(TS)/#216(math-engine)/#217(docs) and with the parallel-merged P0/dependabot PRs (clean rebases, ×6).
- **Uncertainty stated:** I cannot confirm what any other Claude session is editing in real time; this is inferred from committed evidence only.

## 7. THE single live blocker (root cause, evidenced)
#224 (which carries GATE_CROSSDEX_1A + GATE_PARITY_224) is **green + seal-verified** but cannot land because of **`require_up_to_date=true` + a ~20-min CI + continuous parallel merges**. Across 6 attempts, #224 was re-behinded 6 times, each by an operator merge within minutes:

| attempt | re-behinded by | merged by |
|---|---|---|
| 1–6 | #221, #217, #162, #142, #230, #139 | hefarica |

This is a **merge-coordination** problem, not a code problem. Auto-merge is **disabled** in the repo (`enablePullRequestAutoMerge=false`), so a quiet manual window is required.

## 8. Validations executed this session (evidence)
- #223/#221/#216/#217/#229: CI required checks green (verified via `gh pr checks` filtered to the 14 required contexts).
- #224: `forge build+test`, `forge fork test (mainnet)`, `cargo check+clippy+test`, `Rust tests`, `lint-and-test-contracts`, CodeQL, TS suite — all green on its rebased heads.
- #221 local vitest: red→green (3 fail on old verifier → 33/33 on new), PF restored.
- #224 seal markers re-verified read-only on every rebased head.
- Live curl probes of the deployed edge (read-only).

## 9. Paper state (final): **~READY**, fail-closed/idle, honest. Not "100%" until a continuous run with chains loaded + simulator-v2 activated yields real paper executions over ≥7 days (G-PAP-1).

## 10. Live state (final): **PREPARED-BUT-GATED.** Code-complete; fail-closed-until-M5; paper_mode + M1 (mainnet physically refused) intact. No broadcast, no signer, no capital touched.

## 11. Live blockers (exact)
1. **#224 not merged** — needs a ~20-min window with no other merges (operator-controlled).
2. **#216** still open (low-risk, AMM vectors).
3. **FLE+AE not deployed** to any chain (`EXECUTOR_<chain>`/`FLASHLOAN_EXECUTOR_<chain>` unset) → no SIM_SUCCESS (fail-closed).
4. **No ≥7-day paper run** (G-PAP-1 red).
5. **External professional contract audit** pending (pre-mainnet-capital).
6. **#226** gated on operator VPS secrets.

## 12. Exact next steps (activation, gated)
1. **Operator merges #224** in a quiet window (no other merges ~20 min): `gh pr update-branch 224` → wait 14 required green → `gh pr merge 224 --squash --delete-branch`.
2. Merge #216, #229.
3. Operator deploys FLE+AE to **Sepolia** (keys outside CI) → set `EXECUTOR_1`/`FLASHLOAN_EXECUTOR_1`/`RPC_HTTP_1`.
4. Run A.4 fork validation (`run_a4_fork_validation.sh` / #229 workflow) → real `A4_OUTCOME`.
5. Activate `ARBX_USE_SIMULATOR_V2` → continuous paper-shadow ≥7 days (clears G-PAP-1).
6. External professional contract audit.
7. Operator cert(c) + KMS/HSM signer → minimal mainnet canary, fail-closed, rollback ready.

---

*No false greens. PAPER ~ready; LIVE prepared-but-gated; the single gate to advance is the #224 merge window, which only the operator can provide (auto-merge disabled, parallel merges break it).*
