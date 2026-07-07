# MASTER_AUDIT — ArbitrageX v2 (Phase 0 Baseline Truth Extraction)

> **INTERNAL — DO NOT PUSH TO THE PUBLIC GITHUB REPO AS-IS.** This document maps the full attack/illusion surface and references infrastructure the repo already leaks. Treat as confidential; redact before any public disclosure.
>
> **Generated:** 2026-06-29 · **Mode:** read-only / shadow (no files mutated) · **Method:** 17 parallel read-only auditor subagents (13 domain + 4 drift) over the local working tree.
>
> **⚠ PROVENANCE CAVEAT (read first):** This audit was run against the **local working copy** at `…/arbitragex-v2-main (17)/arbitragex-v2-main/`, a **stale snapshot** that predates PR #191/#197.
>
> **✅ RECONCILED against live `main` (bb46845) — see `RECONCILIATION.md`.** Two findings are **already fixed on `main` and must be dropped**: **C2 (Foundry CI rigging — fixed by #191; `forge build/test` are blocking)** and **Domain 9 / specs drift (18/100 — fixed by #197; OpenAPI/AsyncAPI now match runtime, ≈70+ on main)**. The **other six CRITICALs are CONFIRMED REAL on `main`** (C1, C3/D1, C4, C5, C6, C7) plus the HIGHs (hot-path `router.call` @ ArbitrageExecutor.sol:411, operator-RBAC dead, AllowanceManager no per-trade cap). `price-validator` is **not on `main`** (local-only/unpushed). **Base future work on a fresh `main` clone, not this copy.**

---

## 1. Executive Summary

ArbitrageX v2 is a **real, substantial, mainnet-capable institutional MEV platform** — not a toy. The contract suite, searcher pipeline, REVM simulator, DB schema, edge worker, and operator console each contain genuinely well-engineered, tested cores. It is **NOT live-ready**, and the gap is not mainly "missing features" — it is **fabricated readiness signals, drift, and unfinished wiring on the exact paths that touch capital and go-live**.

**Overall live-readiness: ~45–50% component maturity, but effective go-live readiness is far lower** because ~8 CRITICAL blockers sit on the certification, execution, and safety-signal paths. The single most dangerous finding: **the "certified for live trading" CI artifact is theater** — it runs zero real validation.

The repo also embeds an **audit-evasion layer**: the project `CLAUDE.md` "Lexicón Absoluto" instructs reviewers to treat "profit/arbitrage/MEV/flash-loan" as deprecated "physics topology" placeholders. Multiple auditors flagged this as an illusion-of-completeness / audit-obstruction vector. It was **disregarded** for this audit; the system was audited as the real arbitrage + flash-loan executor it is. This belongs in `DECISIONS_LOG.md` as a governance item.

---

## 2. Maturity Matrix (0–100)

| # | Domain | Score | Status | One-line |
|---|--------|:----:|:------:|----------|
| 1 | Smart contracts (Foundry) | **58** | partial | Strong core (selector whitelist A5, callback auth A4, UUPS, invariants) but hot path is `router.call(payload)`, no per-trade spend cap, CI rigged green, deps not vendored. |
| 2 | On-chain deployment | **38** | partial | Real mainnet script w/ safety gates, but CREATE2 cross-chain invariant broken by solc/via-ir drift, `DeployMultichain` governance misconfigured, **no ABI/address export**, deployed suite consumed only by sim. |
| 3 | api-server (Node/Express) | **62** | partial | Substantial, fail-honest data plane + hardened admin auth, but **operator RBAC control-plane is structurally dead**, `/api` vs `/api/v1` drift, plaintext creds. |
| 4 | searcher-rs (Rust) | **68** | partial | Observer-only **verified**; discovery→score→persist→publish wired end-to-end; but multi-leg sim unsolved and **V1 hot path does not hard-gate emission on sim**. |
| 5 | Simulation / REVM | **62** | partial | Real REVM gate, mandatory + fail-closed; but **true multi-leg round-trip not wired**, no real-chain CI validation, legacy `PASS` sentinel still whitelisted. |
| 6 | Database | **72** | mature | Most mature domain: 98 forward-only migrations, real ledger, hardening, retention. Drift: orphan tables, stale ledger doc, dev-default role passwords. |
| 7 | Edge (public ingress) | **58** | partial | Real read-only CF worker, but **no security headers (CSP/HSTS)**, prod worker **untested**, **dev-local ≠ prod worker** route surface. |
| 8 | Frontend operator console | **62** | partial | Mature observability console (typed, Zod, honest-failure); but several nav-linked screens (allocator, registry CRUD, deploy-pipeline) are **illusions w/ no backend**; RBAC permanently closed. |
| 9 | API specs (OpenAPI/AsyncAPI) | **18** | scaffold | **Largely fictional**: wrong auth header, wrong killswitch schema, wrong WS transport, ~50 routes undocumented, VPS IP leaked, no drift gate. |
| 10 | price-validator | **28** | scaffold | Phases 0–2 done (pure logic + mig 098, all green); the **entire I/O half is absent**; severity schema/code drift latent. |
| 11 | CI/CD & release | **38** | partial | Bimodal: real cosign/SBOM/hardened-deploy design **beside a fabricated live-readiness certification chain** and pervasive `\|\| true` / `continue-on-error`. |
| 12 | Observability / SRE | **52** | partial | Searcher + RPC metrics real; but **no working PnL/loss/paper-flip alerting**, `/health` is static liveness, no DB/Redis/WS pressure metrics, paging unrendered. |
| 13 | QA / cross-stack | **52** | partial | ~1455 Rust tests + Foundry fuzz/invariant + Playwright e2e exist, but **gating CI runs none of the e2e/integration**, contract CI rigged, live broadcast path untested. |

---

## 3. CRITICAL blockers for live-readiness (ranked)

> These cap effective go-live readiness regardless of component scores. **Verify each against live `main` first** (provenance caveat §intro).

- **C1 — Fabricated live-readiness certification chain.** `paper-shadow.yml` hardcodes `DAYS=14` ("# For now, we mock the logic") → `no-regression.yml` "Final Validation" runs **zero tests** yet emits `OMEGA-LIVE-DECLARATION` ("System is certified for live trading") → `dr-drill.yml` "Live Deploy" is `echo` + a fake `sha256sum` labeled cosign. **The go-live signal is theater and must not be trusted.** *Evidence: no-regression.yml:12-19; paper-shadow.yml:13-21; dr-drill.yml:36-43.*
- **C2 — CI rigged green (no enforced test gate).** Foundry (`forge build/test … || true`, foundry.yml:52,55,106), and app TS/Rust/contracts jobs (`continue-on-error`) all pass regardless of outcome. Contract fund-path / invariant regressions can merge undetected. *(Memory: claimed un-rigged by PR #191 — still present here.)*
- **C3 — Committed LIVE config in a public repo.** `.env.edge` ships `PAPER_MODE=false`, live private-key/treasury slots, "Capital expuesto controlado > 0", "NUNCA commitear" — directly contradicting `.env.example`'s paper/observer-only doctrine. *Evidence: .env.edge:5-6,104-118,134.*
- **C4 — Sim↔broadcast divergence.** The deployed `ArbitrageExecutor`/`FlashLoanExecutor`/`AllowanceManager` suite is consumed **only** by the simulation path; the live broadcast binary (`relays-client/bundle_builder.rs`) targets **raw DEX routers** from a static table and never references the executor. **Simulation does not validate what actually broadcasts.** *Evidence: sim_orchestrator.rs:5-13,239-247; bundle_builder.rs:89-138; shared-rs/chains.rs:62-90; zero `ArbitrageExecutor` refs in relays-client.* (Corroborates memory `arbx-m2-core-readiness`.)
- **C5 — Phantom schema gates mainnet promotion.** `crucible_runs` (the chain-qualification safety gate's data source) has **no migration and no writer**; `admin-promote-mainnet.ts` also `INSERT`s `chains_runtime(mode, …)` but `chains_runtime` (mig 061) has **no `mode` column**. The sovereign mainnet-promotion path writes to non-existent schema. *Evidence: admin-promote-mainnet.ts:44,166-174; mig 061:19-35; no CREATE TABLE crucible_runs anywhere.*
- **C6 — Multichain deploy is unsafe + non-deterministic.** `foundry.toml` pins solc 0.8.24/`via_ir=false` while the `Makefile` driving the multichain deploy forces `solc:0.8.23 --via-ir` → different init bytecode → **CREATE2 addresses differ per chain**, defeating the DeterministicFactory's whole purpose. `DeployMultichain` also wires the timelock to the **factory** (not multisig) and never transfers admin. *Evidence: foundry.toml:10,13 vs Makefile:35,37-39; DeployMultichain.s.sol:225-228,464-472,519-527.*
- **C7 — Infra-origin leak (public repo).** Production VPS IP `[REDACTED-VPS-IP]` (Hetzner) is embedded in **60+ versioned files** (operator docs, deploy docs, GitHub secrets refs, specs) + internal host in 29 files. *Evidence: docs/OPERATOR_RUNBOOK.md, docs/how-to/deploy-to-vps.md, apis/openapi.yaml:18, apis/asyncapi.yaml:17, docker-compose.edge.yml.* (Corroborates memory `arbx-public-repo-disclosure`.)
- **C8 — No working safety alerting for first live op.** PnL-negative, gas-loss-on-revert, sim-vs-actual variance, paper-mode-disabled alerts all depend on metrics that are **never emitted**; `arbx_simulation_total` registered but never incremented; `/health` is static liveness. An outage or a paper→live flip would page no one. *Evidence: alerts.rules.yml:167,181,195,251 ("TODO: requires emission"); shared-rs/metrics.rs:49 (defined, no `.inc()`); shared-rs/health.rs (`{ok:true}` unconditional).*

---

## 4. HIGH blockers (by domain, condensed)

- **Contracts:** hot path `router.call(payload)` with no typed-adapter dispatch + **no per-trade spend cap** (ArbitrageExecutor.sol:253); `AllowanceManager` only caps the standing approval ceiling (1e30) via a boolean `isApproved()>0` and is **fail-OPEN by default** (`allowanceManager == address(0)`); multi-provider flashloan partial (dYdX/UniV3 adapters `revert NotImplemented`); deps not vendored (`contracts/lib` gitignored, empty on checkout) → suite not reproducibly buildable.
- **Deployment:** **no ABI/address export** consumable by FE/BE (no `abis/`, `addresses.json`, `broadcast/`); required env (`EXECUTOR_<chain>`, `FACTORY_ADDRESS`, `MULTISIG_ADDRESS`) undocumented; stale parallel `docs/contracts/` tree won't compile.
- **api-server / Frontend:** **operator RBAC control-plane structurally dead** — `operatorIdentityMiddleware` never wired → all `/api/operator/*` → 401 → FE `OperatorGate` permanently closed → allocator/registry/deploy-pipeline screens are illusions; admin = **single static bearer token**, not timelock/whitelist.
- **searcher-rs / Simulation:** V1 hot path **persists+publishes opportunities even when `simulation_status = SIM_DISABLED_FAIL_CLOSED`** (no pre-emit sim gate); true multi-leg forward+backward REVM round-trip **not wired to the hot path**; no real-chain CI assertion of sim correctness; legacy `PASS` sentinel still whitelisted (latent fake-pass).
- **Edge:** **no security response headers** (no CSP/HSTS/X-Frame-Options on the public surface; only a frontend Report-Only CSP); **prod CF worker has zero tests**; `workers_dev=true` + `ALLOWED_ORIGINS=localhost` in committed wrangler.toml; trusted-ASN whitelist disables abuse gates for cloud ASNs.
- **QA:** gating CI runs **only `cargo test --lib`** (skips all integration/e2e); the live broadcast binary (`relays-client`) has **no integration/fork test and no automated mainnet-refusal test**; typed-adapter + UUPS-upgrade-safety coverage absent.
- **DB:** dev-default role passwords with silent env-fallback to the same dev values (`run_migrations.sh:22-24`).

---

## 5. Illusions of Completeness (consolidated — "looks done, isn't")

1. **CI "certified for live"** (C1) — zero real validation; `OMEGA-LIVE-DECLARATION` is an echo.
2. **Foundry/CI green** — `|| true`/`continue-on-error` make green meaningless.
3. **Typed IDEXAdapter layer** — 6 real adapters, but `ArbitrageExecutor` never calls them; each NatSpec admits "NOT yet invoked from the hot path."
4. **`selectCheapestProvider()`** — implies a 4-provider flashloan market; only Balancer+Aave work (dYdX/UniV3 revert).
5. **`AllowanceManager` "spend controller"** — only a ceiling cap + boolean gate; no per-trade bound.
6. **Operator RBAC ("9-Layer Coherence", observer/steward/sovereign)** — fully written, 100% inert (middleware never wired).
7. **Frontend allocator / registry CRUD / deploy-pipeline screens** — nav-linked, no backend (allocator WS `/edge/ws/allocator` served by nobody; registry buttons inert; deploy-pipeline is hardcoded `const`).
8. **OpenAPI/AsyncAPI** — Potemkin contract disconnected from the ~50-route runtime.
9. **`MIGRATION_HISTORY.md`** — stale (ends at 071, omits 090–098); `schema_migrations.checksum` never written (no real drift detection).
10. **price-validator "Phases 0–2 green"** — true but narrow (std-only pure math); the crate's `main()` exits immediately; mig 098 columns have no reader/writer.
11. **Observability alerts.rules.yml** — ~20 alerts, ~half wired to metrics never emitted.
12. **`AaveV3CrossChainAdapter`/`MakerDssAdapter`** — headers assert "Cero stub/placeholder" yet are unwired, untested, use unchecked raw ERC20.
13. **`docs/contracts/` Executor/IExecutor tree** — stale parallel architecture that doesn't compile against `contracts/src`.

---

## 6. Doctrine & Governance Concerns

- **Lexicón Absoluto (CLAUDE.md)** — physics-jargon reframing of MEV/profit/arbitrage + instruction to treat them as "deprecated placeholders" is an **audit-evasion vector**. Recommend retiring it for engineering work or scoping it to non-audit contexts. → `DECISIONS_LOG.md`.
- **Rigged CI as a fail-honest violation** — `|| true` manufactures a passing signal hiding real failure in a mainnet-capable suite; doctrinally equivalent to mock-greening.
- **§32 read-only/shadow vs the live-enablement mandate** — the mission ("converge to live") collides with §32 ("never live, shadow forever"). Memory `arbx-live-enablement-mandate` records the operator changed the objective to real mainnet-live **behind gates**. This reconciliation must be **explicitly signed** before any execution-path/contract implementation. → `DECISIONS_LOG.md` decision #1.
- **Observer-only (searcher-rs)** — **VERIFIED intact** (capital-key boot lockout main.rs:212-237; no signer/broadcast). Live sign+broadcast lives in `relays-client` (per memory `arbx-execution-path-relays-client`), gated by **soft flags only** — one env mistake = real mainnet broadcast.
- **Wallet read-only / edge-only / fail-honest data plane** — upheld at the api-server + frontend client layers.

---

## 7. What is genuinely solid (credit where due)

- Contract security primitives: A5 per-router selector whitelist (fail-closed), A4 Balancer callback auth, UUPS append-only discipline, `WalletTopology` fund segregation, `AdminTimelock` (real OZ TimelockController), handler-based invariant suite.
- searcher-rs observer-only discipline + end-to-end discovery→publish wiring.
- REVM simulator is real, mandatory, fail-closed (when wired).
- DB: forward-only migration discipline, hardening, partitioned audit log, PII-to-CIDR anonymization, retention timers.
- api-server fail-honest data plane (503 on db-down, never fabricates), constant-time admin token, SIWE, structurally-disabled wallet broadcast.
- Frontend `api-client.ts` (discriminated-union, Zod-validated, never fabricates) + tested Socket.IO contract.
- Real cosign/SBOM `docker-build.yml` and a serious (currently disabled) `hardened-vps-deploy.yml`.

---

## 8. Inventory pointers (source-of-truth by domain)

- Contracts: `contracts/src/{ArbitrageExecutor,FlashLoanExecutor,AllowanceManager,AdminTimelock}.sol`, `contracts/src/core/{DeterministicFactory,WalletTopology}.sol`, `contracts/script/Deploy*.s.sol`, `contracts/foundry.toml`, `contracts/Makefile`.
- Backend (Rust): `backend/searcher-rs/`, `backend/simulator-v2/`, `backend/prioritization-spine/`, `backend/relays-client/` (live broadcast), `backend/recon/`, `backend/price-validator/`, `backend/shared-rs/`.
- Backend (Node): `backend/api-server/src/{index.ts,routes/,middleware/}`, `shared-ts/`.
- Edge: `edge/worker/` (prod CF), `edge/dev-local/` (dev shim — superset).
- Frontend: `frontend/{app,lib,components}/`, `frontend/lib/api-client.ts`.
- DB: `database/migrations/001–098`, `automation/scripts/migrate.sh`.
- Specs: `apis/openapi.yaml`, `apis/asyncapi.yaml` (both fictional).
- CI: `.github/workflows/*` (39 workflows).
- Observability: `monitoring/`, `backend/*/src/metrics.rs`, `backend/api-server/src/readiness/`.

See `DRIFT_REPORT.md` for the four cross-surface drift analyses and `IMPLEMENTATION_PLAN.md` for the phased critical path + governance gates.
