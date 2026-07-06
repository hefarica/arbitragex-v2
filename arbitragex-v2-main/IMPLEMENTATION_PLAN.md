# IMPLEMENTATION_PLAN — ArbitrageX v2 → Live-Readiness

> **INTERNAL — do not push as-is.** Generated 2026-06-29 from `MASTER_AUDIT.md` + `DRIFT_REPORT.md`. **Nothing in here is implemented yet — implementation is GATED on operator review (per the Phase-0 instruction). Phases are sequenced by safety, not by the prompt's numbering.**
>
> **Provenance gate:** Step 0 is to reconcile this checkout against live `main`. Several CRITICALs (rigged Foundry CI, fictional specs, mainnet lockout) are claimed fixed on `main` (PR #191/#197/#213). **Do not re-implement a fix that already exists upstream.** Run the audit against `main` first; treat the deltas, not the duplicates.

---

## 0. GOVERNANCE DECISIONS — operator must sign BEFORE any implementation (→ `DECISIONS_LOG.md`)

These are not engineering details; they change architecture/safety posture and are yours to own.

- **G1 — §32 (read-only/shadow forever) vs the live-enablement mandate.** The mission says "converge to live"; the project policy §32 says "never executor, never live." Memory `arbx-live-enablement-mandate` records you changed the objective to real mainnet-live **behind gates**. **Decide explicitly:** does this program proceed toward live (with the gate set in §8 below), or remain audit/shadow? Everything in Phases 2–3 and 7-live depends on this. *Default if unsigned: stay shadow; do audit + non-execution hardening only.*
- **G2 — Where do these control artifacts live?** They map the full attack surface + reference the leaked VPS IP. They must **not** land in the public GitHub repo unredacted. Options: (a) keep local-only (current), (b) move to a private repo, (c) commit redacted into `audits/` of the public repo. *Recommend (a)/(b).*
- **G3 — Retire the "Lexicón Absoluto."** The CLAUDE.md physics-jargon mandate obstructs honest audit and was disregarded here. Approve retiring it for engineering/audit work.
- **G4 — `.env.edge` live-config handling (C3/D1).** Confirm removal from version control + history scrub + treat its key slots as compromised (rotate). This is a committed live-capital config in a public repo.
- **G5 — Un-rigging CI is a one-way trust change.** Flipping `|| true`/`continue-on-error`/the fabricated certification to real gates **will turn `main` red** until the underlying failures are fixed. Confirm you want honest-red over fake-green (you do, per fail-honest doctrine) and that a red `main` during remediation is acceptable.

---

## 1. Critical path (safety-ordered) — overview

```
P0  Reconcile vs main + sign G1–G5                    (gate: governance signed)
P1  SAFETY & SECRETS QUARANTINE                        (closes C3/C7/D1/D2 + C1/C2 reachability)
P2  HONEST CI (un-rig + kill fabricated certification) (closes C1/C2; turns main honest-red)
P3  MONOREPO NORMALIZATION                             (prompt Phase 1)
P4  API/EDGE/SPEC ISOMORPHISM + dead control-plane     (prompt Phase 4; closes A*, B*, operator-RBAC)
P5  OBSERVABILITY FOR FIRST LIVE OP                    (prompt Phase 7-obs; closes C8)
P6  CONTRACTS PRODUCTION CLOSURE                       (prompt Phase 2; GATED by G1)
P7  SEARCHER/SIM/PUBLISH CLOSURE                       (prompt Phase 3; GATED by G1)
P8  DATABASE + PRICE-VALIDATOR                         (prompt Phase 6)
P9  FRONTEND CONSOLE COMPLETION                        (prompt Phase 5)
P10 CI/CD RELEASE + DR + LIVE-READINESS CERT (real)    (prompt Phase 7-release)
```

Rationale for the reorder: the audit shows the danger is **fake readiness + drift + a reachable phantom mainnet-promotion path**, not missing features. Make the system **honest** (P1–P2) before building on it, and make the **control plane and observability real** (P4–P5) before touching the capital path (P6–P7, gated by G1).

---

## 2. P1 — Safety & Secrets Quarantine `[gate: nothing else starts until green]`

**Closes:** C3/D1 (live `.env.edge`), C7/D2 (IP leak), reachability of C1/C2 (phantom mainnet-promotion).
**Tasks**
- Remove `.env.edge` from version control; scrub from git history; rotate every secret it referenced. (skill: `arbx-pre-edit-audit`, security-review)
- Repo-wide scrub of `[REDACTED-VPS-IP]` + `edge-arbx.ape-tv.net` → env placeholders (60+/29 files). (memory `arbx-public-repo-disclosure`: repo-wide is the real fix.)
- **Quarantine the phantom mainnet-promotion path**: `admin-promote-mainnet.ts` must fail-closed (not silently) while `crucible_runs`/`chains_runtime.mode` don't exist — it currently can be mounted and would write to non-existent schema. Keep it un-mounted (it already is) AND add a guard test.
**Acceptance:** no live private keys/treasury in any versioned file; `rg '[REDACTED-VPS-IP]'` over the repo returns 0 (or only an allowlisted internal doc); the promote path has an automated test proving it refuses to run against missing schema.
**Gates:** security-review pass; no-secret-leak check; fail-honest preserved.

## 3. P2 — Honest CI (un-rig + delete fabricated certification) `[GATED by G5]`

**Closes:** C1 (fabricated cert chain), C2 (rigged gates).
**Tasks**
- Delete/rewrite the fabricated chain: `no-regression.yml` (echo-only "certified for live"), `paper-shadow.yml` (`DAYS=14` mock), `dr-drill.yml` (mock deploy + fake cosign). Replace with real steps or remove the "certified" artifact entirely.
- Remove `|| true` and unjustified `continue-on-error` from `foundry.yml`, `ci.yml`, `typescript.yml`, `unit-tests.yml`, `e2e.yml`. Make the real gates blocking.
- `npm ci` (not `npm install` after deleting the lockfile) across CI for reproducibility.
**Acceptance:** every "gate" job actually fails the build on failure; there is **no** workflow that emits a live-readiness/certification artifact without running real validation; `main` reflects honest status (expected: red until P6–P10 land).
**Gates:** a reviewer confirms no remaining `|| true` on a meaningful step; CI status is now trustworthy.

## 4. P3 — Monorepo Normalization (prompt Phase 1)

**Closes:** D3/D4/D5/D7 (env/doc drift), reproducibility.
**Tasks:** canonical package manager (memory: npm; pnpm dead); make `.env.example` the true superset (add `ENRICHER_CHAINS`, `MINIO_*`, `SIM_SIGNER_ADDRESS`, `EXECUTOR_<chain>`, `FACTORY_ADDRESS`, `MULTISIG_ADDRESS`, `AAVE_V3_POOL`, staging vars); delete the fictional `docs/reference/env-vars.md` `AX_*` namespace; vendor or pin `contracts/lib` (gitignored+empty breaks reproducible contract builds); validate one reproducible local happy path. (skills: `arbx-no-hardcode-doctrine`, ecc-deployment-patterns)
**Acceptance:** `cp .env.example .env` boots dev compose without `${VAR:?required}` failures; contract suite builds from a clean checkout (vendored/pinned deps); no `AX_*` fiction.

## 5. P4 — API / Edge / Spec Isomorphism + dead control-plane (prompt Phase 4)

**Closes:** A1–A7, B1–B5, operator-RBAC-dead, `/api`↔`/api/v1`.
**Tasks**
- Wire `operatorIdentityMiddleware` so `/api/operator/*` works (or remove the RBAC layer + the FE screens that depend on it — don't ship an inert authz illusion on a control plane).
- **Reconcile the prod CF worker with dev-local**: one source-of-truth route allowlist; FE must not depend on dev-only routes (B2/B3). 
- Unify `/api` vs `/api/v1`; stop relying on edge rewrites to paper over server inconsistency.
- Regenerate OpenAPI/AsyncAPI **from runtime** (correct auth header, killswitch schema, Socket.IO rooms) or delete them; add a **blocking** spec-drift gate (oasdiff/Spectral + a response-vs-spec contract test).
- Add real edge security headers (CSP/HSTS/X-Frame-Options/X-Content-Type-Options) on the public surface; add tests to the prod worker (currently zero).
**Acceptance:** every FE-consumed route exists in the **prod** edge; `/api/operator/me` returns a real identity or the layer is removed; OpenAPI/AsyncAPI validate against runtime in CI; the public edge sets enforcing security headers; prod worker has a test suite.

## 6. P5 — Observability for first live op (prompt Phase 7-obs)

**Closes:** C8, simulation-failure alert dead, infra metrics.
**Tasks:** emit the missing metrics (`arbx_realized_pnl_usd`, `arbx_revert_gas_wasted_usd`, `arbx_sim_predicted/actual_profit_usd`, increment `arbx_simulation_total`); deploy node/cadvisor/postgres/redis exporters or remove their alerts; make `/health` (or a `/ready` probe) gate on DB/RPC/Redis; render Alertmanager receivers (PagerDuty/Slack) with a real envsubst step; add contract-event subscription health metrics.
**Acceptance:** a forced paper→live flip, a negative-PnL window, and a sim-failure burst each fire a real alert to a real receiver in staging; container/LB healthchecks reflect dependency health.

## 7. P6 — Contracts production closure (prompt Phase 2) `[GATED by G1]`

**Closes:** contracts HIGH set. **Do not start without G1 = "toward live".** Defer to gates: `arbx-contract-atomicity-rules`, `arbx-flash-loan-discipline`, `arbx-risk-limits-enforcement`, `arbx-simulation-mandatory`.
**Tasks:** resolve `router.call(payload)` vs typed `IDEXAdapter` (per prompt §9: if kept, it must be off the main product path, behind a feature flag, whitelist-reinforced, fork-tested, sim-mandatory); add a **per-trade spend cap** (AllowanceManager today only caps the standing ceiling); make the allowance gate **fail-closed by default** (currently `allowanceManager==address(0)` ⇒ off); land or remove the dYdX/UniV3 flash adapters (no reverting skeletons advertised as providers); add a storage-layout/upgrade-safety gate (OZ upgrades / `forge inspect`); real fork test of a full borrow→swap→repay round-trip; **export ABIs + addresses** (`abis/`, `addresses.json`, committed `broadcast/`) consumable by FE/BE; fix `DeployMultichain` governance (timelock→multisig, admin transfer) + solc/via-ir lockstep so CREATE2 is deterministic.
**Acceptance:** un-rigged Foundry CI green (unit+fuzz+invariant+fork+upgrade) from a clean checkout; per-trade spend cap enforced + tested; a fork test executes a real arbitrage; ABIs/addresses exported and consumed.

## 8. P7 — Searcher / Sim / Publish closure (prompt Phase 3) `[GATED by G1; observer-only PRESERVED]`

**Closes:** searcher/sim HIGH set; resolves the **sim↔broadcast divergence (C4)** — the single most important live-safety gap.
**Tasks:** keep `searcher-rs` observer-only (verified; do not regress); make simulation a **hard pre-emit gate** in the V1 path (today it persists+publishes even when `SIM_DISABLED_FAIL_CLOSED`); remove the legacy `PASS` whitelist (fake-pass surface); wire the true multi-leg forward+backward REVM round-trip (sequence_runner has no production caller); add real-chain CI validation of the simulator; **make the live broadcast path (`relays-client`) and the simulated path validate the SAME calldata** (close C4) — this is the gate for any live trade. Add replay/freshness/benchmarks. (gates: `arbx-net-profit-gate`, `arbx-simulation-mandatory`, `arbx-paper-trade-first`)
**Acceptance:** no opportunity is published/acted-on without a conclusive real-REVM SIM_SUCCESS; sim calldata == broadcast calldata (proven by a test); observer-only invariant test still green; relays-client has an integration/fork test + an automated mainnet-refusal test.

## 9. P8 — Database + price-validator (prompt Phase 6)

**Closes:** C3 (orphan validator_divergences), price-validator productization, schema/code drift.
**Tasks:** **fix the latent severity drift first** — mig 098 `CHECK (severity IN ('warn','severe','no_reference'))` vs the Rust `Severity` enum `Ok/Warn/Severe` (no `no_reference`); reconcile before persistence is wired (add `no_reference` handling or a `999_*` migration). Complete price-validator per its existing plan (`docs/superpowers/plans/2026-06-29-validator-offchain-price.md`): reference puller, persistence, auditor loop, annotator consumer-group on `arbx:opps:detected`, metadata enrichment, advisory `RECOMMEND_HALT` writer, boot wiring, Dockerfile, `/api/tokens/active`. New migrations `099+` only. Fix dev-default DB role passwords (fail-closed if `ARBX_*_PASSWORD` unset). Constraint: stays **async, read-only/shadow, advisory, no hot-path interference, no killswitch, never writes `arbx:token_prices`**. (this is the work already specced; resume after WSL gcc is enabled or run tests on VPS)
**Acceptance:** price-validator runs as a shadow service writing `validator_divergences`; severity contract consistent; orphan tables either wired or documented as intentional; migrations 099+ applied + tracked.

## 10. P9 — Frontend console completion (prompt Phase 5)

**Closes:** FE illusions (allocator/registry/deploy-pipeline), states.
**Tasks:** make each nav-linked critical screen either real (backed by a live route) or remove/clearly-mark it; loading/empty/degraded/explicit-upstream-error on all; integrate the ABI/address registry from P6; keep wallet read-only (unless G1 + a separate product decision); Vitest + Playwright for critical paths (added to the now-real e2e gate). (skill: `frontend-omni-ssot-analyzer`)
**Acceptance:** no nav-linked screen is an illusion; Playwright covers the critical operator flows; FE consumes real ABIs/addresses.

## 11. P10 — Release engineering + real DR + real live-readiness cert (prompt Phase 7-release)

**Closes:** artisanal deploy, fake DR, the (now-deleted) fake certification.
**Tasks:** activate `hardened-vps-deploy.yml` (signed manifest, dry-run, validated rollback); deploy the cosign-signed GHCR image (not VPS-side `git reset --hard`); real DR drill; real post-deploy smoke; a **genuine** live-readiness certification that runs the real gates (the 17-item `/api/readiness` doctrinal gate per memory `arbx-readiness-panel`, real paper-shadow accounting, real sim parity). Only this can emit a go-live signal.
**Acceptance:** a deploy is reproducible from a signed artifact with a tested rollback; the only "certified for live" output is one that ran every real gate green; DR drill actually exercises restore.

---

## 12. DEFINITION OF DONE — do NOT declare live-ready until ALL true

(Mirrors the prompt's DoD, made concrete against the findings.)
1. Contract product path safe + tested (per-trade spend cap, fail-closed allowance, fork+invariant green on **un-rigged** CI). 2. Flashloan multi-provider real (no reverting skeletons advertised). 3. Sim mandatory + fail-closed AND **sim calldata == broadcast calldata**. 4. searcher-rs end-to-end + observer-only intact + hard sim pre-emit gate. 5. API/edge/FE/OpenAPI/AsyncAPI aligned + drift gate blocking. 6. FE critical flows real + honest states. 7. price-validator runs as real shadow advisory. 8. DB supports all live features (no phantom tables on the promotion path). 9. Compose dev/prod reproducible from `.env.example`. 10. CI blocking + **honestly** green (no `|| true`, no fabricated cert). 11. Deploy + post-deploy smoke documented + validated from a signed artifact. 12. Rollback + incident + restore + secrets-rotation + contract-upgrade runbooks exist and were drilled. 13. Observability fires real PnL/loss/sim/paper-flip alerts to real receivers. 14. ABI/address registry exported + consumed. 15. Network/secrets matrix explicit; no live config or VPS IP in version control.

---

## 13. Next action (awaiting operator)

**STOP — per the Phase-0 instruction, no implementation proceeds until you review `MASTER_AUDIT.md` + `DRIFT_REPORT.md` + this plan and sign G1–G5.** On your sign-off I begin at **P1 (Safety & Secrets Quarantine)** — the only phase that is unambiguously safe and required regardless of the G1 live/shadow decision — using subagent-driven development with per-task spec + quality review and tests run in WSL/VPS (Smart App Control blocks local Windows test exes; see memory `arbx-wsl-test-runner`).
