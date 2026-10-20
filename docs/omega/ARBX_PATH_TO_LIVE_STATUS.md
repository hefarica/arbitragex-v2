# ArbitrageX v2 — Path-to-Live Status (OMEGA loop snapshot)

> **Signal honesty (OMEGA cl.6).** Every claim is labeled:
> `CHECK-DERIVED` (from live `gh` CI/check results) · `CODE-VERIFIED` (read from the
> repo at the cited `file:line` / commit SHA) · `MANUAL/DOCTRINAL` (a hand-set value,
> not telemetry) · `UNVERIFIED/STALE`. **No number here is runtime telemetry.**
> This is a coordination snapshot + handoff board, **not** a "DONE" certificate.

**Base:** `origin/main @ 242fed4` (2026-06-30 01:55Z) · **Evidence:** OMEGA Cycle-1
forensic audit `wmpsrrsfx` (7 read-only Workflow subagents, 705k tok) + direct `gh`/`git`.
**Author:** OMEGA orchestrator session (one of 5). Coordination is by **repo evidence
only** — there is no direct inter-session channel; nothing here claims a remote "consult".

---

## 1. Six-gate map

| Gate | State | Basis | Evidence (file:line / run) | Gap to READY |
|---|---|---|---|---|
| **GATE_RUST_CI** | 🟢 READY | CHECK-DERIVED | `cargo check+clippy+test`, `Rust tests`, `Rust integration`, `cargo audit` all PASS on #224/#233; clippy `-D warnings` real; no `continue-on-error` on gating steps | RUSTSEC allowlist expiry 2026-06-30 → re-justify |
| **GATE_CROSSDEX_1A** | 🟢 READY (on #224, **not on main**) | CODE-VERIFIED | `ArbitrageExecutor.sol` `_runRoute` 6 invariants verified (leg0 tokenIn :412; leg1 intermediate-delta :419-424; `UnsupportedRouteLength` :375; `ZeroIntermediate` :421; `TokenNotApproved` M8 :367; SC-13 both tokens via `TokenOutRetentionViolation` :437; `AliasedTwoLegRoute` :384); 8/8 cross-DEX + 102/102 executor `forge test` pass; contract CI green | merge to main (HUMAN GATE) + UUPS redeploy; one missing **direct** `ZeroIntermediate` test |
| **GATE_LAYER_C_224** | 🟢 parity READY / 🟡 merge HELD | CODE-VERIFIED + CHECK-DERIVED | byte-parity = single-encode + verbatim carry (`sim_multistep.rs` L381/420 → REVM exec → `wrapped_calldata` on passed=true → `ValidatedPlan` → Redis JSON → `bundle_builder.rs` verbatim → tx.data); tests CI-executed PASS; `bundle_builder` fail-closed (M1 first, .to=FLE, reject empty/wrong-selector, value=0, spend-cap) | merge #224 (HUMAN GATE); **net-USD gate not wired in producer** (LIVE blocker); no single cross-crate e2e join test |
| **GATE_PR_LIVE_CI / #232** | 🟡 EN_CURSO | CHECK-DERIVED | required contexts green; the only RED (`TypeScript integration`) is **NOT a required context** | analyze(rust) complete + up-to-date |
| **GATE_M5_SEPOLIA** | 🟢 READY-for-operator / 🟡 no run | CODE-VERIFIED | runbook #231 complete & accurate vs code (4 contracts, manual grants incl SC-13, `setRouterSelectorApproval` gap flagged, Aave-Sepolia marked PENDING honestly, `paper_mode=true`, observer-only); **0 keys / 0 broadcast in CI** (3-layer observer-only) | operator deploy + key custody (HUMAN GATE); Aave-Sepolia on-chain verify; `multistep_fork.rs` chain-1 hardcode |
| **GATE_PAPER_CORE** | 🟡 EN_CURSO | mixed | backend observer-only HARD (boot-panic on 8 capital keys); REVM **real** (LazyDb fork, not stub); 6/8 operator pages real+edge+Zod+fail-honest; CI required green | net-USD wiring (S3); SIM_SUCCESS fail-closed until M5 fork (S4); `/operator/presets` + `/workspace-progress` routes (S1); ProgressRealCard 4 MANUAL % |

---

## 2. Milestone chain (immutable, M5→M0)

| Milestone | Meaning | State | Blocked by |
|---|---|---|---|
| **M5** | Sepolia testnet, observer-only sim | READY-for-operator | operator deploy + key custody (HUMAN GATE) |
| M4–M1 | testnet→minimal-mainnet certification stages | NOT STARTED | M5 + #224 merge |
| **M0** | mainnet LIVE controlled | **PREPARED-BUT-GATED** | all of the above + net-USD gate + operator sign-off |

No milestone is green. None can open while the prior is red (cl.10). LIVE = **PREPARED-BUT-GATED**.

---

## 3. Per-session ownership (inferred from PR/branch evidence — NOT direct consultation)

| Session | Domain | Active branches/PRs (evidence: updated 2026-06-30) | Hot surfaces — do not edit from another session |
|---|---|---|---|
| **S1** | Frontend/UI/Console/E2E | #232 `feat/operator-risk-presets`, #235 `hardening/progress-panel-honesty`, #236 `docs/omega-final-readiness` | `frontend/app/operator/*`, `frontend/features/home/ProgressRealCard.tsx` |
| **S2** | Backend/API/DB/Realtime | (TS-integration flake area; api-server) | `backend/api-server/**`, testcontainer setup |
| **S3** | Rust/Scanner/Sim/Scoring | #216 `feat/amm-external-vectors`, #224 (rust half) | `backend/searcher-rs/**`, `backend/math-engine/**`, `prioritization-spine/**` |
| **S4** | Contracts/Executor/M5/Live | #224 (contract half), #231/#229 M5 | `contracts/src/ArbitrageExecutor.sol`, `scripts/*m5*`, `multistep_fork.rs` |
| **S5** | CI/CD/Security/Infra/Release | #226 `hardening/p0-ip-scrub-functional`, ethics-guard (#230 merged) | `.github/workflows/**`, `docs/policies/**`, `docs/security/**` |

> 18 git worktrees active in the canonical clone (`productivo_full`) — sessions work via
> worktrees. `productivo_full` HEAD advanced `6467448 → f6dd3dd` during this snapshot =
> live concurrent session activity (observed, not disturbed).

---

## 4. Exact LIVE blockers (PREPARED-BUT-GATED)

1. **#224 merge** — HUMAN GATE (fund-path). Lands `GATE_CROSSDEX_1A` + `GATE_LAYER_C_224` on main. Engineer-complete, 0 red required checks. Needs UUPS redeploy plan post-merge.
2. **net-USD-of-gas gate wiring into the REVM producer** — S3, fund-path-sensitive. `compute_profit_usd` exists+tested (`round_trip_executor.rs`) but the producer (`sim_multistep` success path) gates only GROSS `retained_spread>0`. LIVE blocker per `sim_multistep.rs:794-796`.
3. **M5 Sepolia execution** — HUMAN GATE (operator + protected key / KMS/HSM). Runbook ready; no on-chain run yet.
4. **Operator key custody (KMS/HSM)** — out-of-repo; assumed, not provisioned.

## 5. Human gates pending
- Merge: **#224** (fund-path), **#237** (mev-ethics banner strip), #232/#216 (operator review on #216 = fund-path-adjacent AMM math — auto-merge **disarmed** by this session).
- M5 Sepolia deploy; mainnet promote; any signer/broadcast.

## 6. Open handoffs (for the owning session)
- **→ S1:** React-19 `JSX.Element`→`React.JSX.Element` pre-empt in 5 files (`app/admin/signin/page.tsx:28`, `app/omega-s5/registry/[entity]/page.tsx:29`, `…/RegistryPageClient.tsx:94`, `components/AdminSessionBadge.tsx:33`, `components/operator/OperatorGate.tsx:59`). Verified **free of every open PR**. `OperatorGate.tsx` imports only `{ ReactNode }` → also add a `React`/`type JSX` import. Verify with your `tsc` (node_modules present in your tree). Pre-empts the required `tsc` going red when #220 (@types/react 19) merges.
- **→ Operator:** **doctrinal JIT inconsistency** — `docs/policies/mev-ethics.md` ALLOWS JIT liquidity, but `STATUS_REPORT.md` + the `arbx-mev-ethics-gate` skill say "JIT-V3 PROHIBIDO / JIT-displacement predatory". Adjudicate (not an agent decision).
- **→ S4:** add a **direct** `ZeroIntermediate` adversarial test (gate present in code `:421`, forge-build-verified, but no red→green assertion).
- **→ S2/S5:** the `TypeScript integration` flake (PG testcontainer `ECONNRESET` errno -104 at `opportunities-live.test.ts:87`) persists **despite #162's wait-for-ready**; needs deeper retry/healthcheck. NON-required → does not block merges, but masks real regressions.

## 7. What is NOT a blocker (anti-theater)
- **cargo-audit / rmcp RUSTSEC-2026-0189:** RESOLVED — allowlisted in `1edd63cc` (PR #223) with valid rationale (`mcp-sim-engine` sole consumer, `serve(stdio())`, `transport-streamable-http-server` not compiled). CHECK-DERIVED green on main + #224. Not bypassed.
- The flaky `TypeScript integration` is **not** a required context → never blocks a merge.

## 8. Final determination
- **PAPER SHADOW = NOT-YET-DONE (~substrate complete, honest).** Remaining gaps are owned by active sessions (S1 routes/panel, S3 net-USD, S4 M5-fork) — not by this session.
- **LIVE = PREPARED-BUT-GATED.** Engineering of `CROSSDEX_1A`, `LAYER_C` parity, `RUST_CI`, and the M5 runbook is READY/ready-for-operator; the binding blockers are **human gates** (#224 merge, M5 deploy, key custody) + one fund-path wiring (net-USD). No claim exceeds the evidence.
