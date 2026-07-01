# Mainnet Readiness — Gated Go/No-Go Dossier

> **Doctrine:** Mainnet is **NOT prohibited** — it is **permitted only as a final controlled phase**,
> after PAPER, SHADOW and SEPOLIA close with complete evidence, with no critical failures, no blocking
> security debt, a KMS/HSM-backed signer, and **explicit human approval by hefarica**. This document
> **prepares, validates and audits** the mainnet path end-to-end. It executes **nothing**: no real
> transactions, no capital, no broadcast, no signer enablement, no lifting of the mainnet code-lock.
>
> **Status:** 🔴 **NO-GO (gated)** — 3 of 18 conditions fully met, 8 hard blockers open. Mainnet is
> currently and correctly **code-locked** (see condition 11). Even Sepolia (condition 3) is not yet executed.

- **Base audited:** `github/main` @ `eec065b0` (the real deployed main).
- **Method:** 7 parallel read-only auditors, every claim grounded in `file:line`. Two most-consequential
  findings independently re-verified against live CI/scripts (see §5).
- **This doc is the pre-sign-off artifact for A.9.** Findings must be confirmed by their owners before
  any sign-off. Nothing here authorizes a flip.

---

## 1. Verdict

| | Count | Conditions |
|---|---|---|
| ✅ **MET** | 3 | 1 PAPER-closed · 2 SHADOW-closed · 13 kill-switch |
| 🟡 **PARTIAL** | 7 | 4 verify · 9 no-secrets · 10 no-hardcoded-keys · 12 breakers · 14 capital-cap · 15 gas/slippage · 17 evidence-pack |
| 🔴 **NOT MET / PENDING** | 8 | 3 Sepolia-deploy · 5 A.4-fork · 6 A.9-signoff · 7 CI-green · 8 security-green · 11 signer/KMS · 16 rollback · 18 hefarica-approval |

**Mainnet cannot be enabled today** — not merely by policy but by code: `live_exec_policy.rs`
refuses `chain_id==1` unconditionally in the current testnet-first phase. Reaching mainnet requires a
*deliberate* code change to lift that lock, which **must not happen** before conditions 7, 8, 11, 16 close
and hefarica signs off.

---

## 2. The 18-condition gate (grounded)

| # | Condition | Status | Evidence (`file:line`) | Owner |
|---|---|---|---|---|
| 1 | PAPER closed w/ evidence | ✅ | deployed & runtime-verified; `configs/app.toml:20` `paper_mode=true` default; webapp validated (fail-honest, live-flip BLOCKED) | done |
| 2 | SHADOW closed (`live_allowed=false`) | ✅ | default-deny `live_exec_policy.rs`; observer-only searcher (`chain_client.rs:59`); capital-key panic (`searcher-rs/main.rs:212`) | done |
| 3 | Sepolia deploy under protected env | 🔴 | env `sepolia-deploy` created (reviewer=hefarica) ✓; BUT canonical `hardened-vps-deploy.yml:89-98` has `environment: production` commented; deploy not run; #229 draft | S4 + operator |
| 4 | grants/approvals/selector/routes/contracts/backend/frontend/observability verified | 🟡 | contracts mature (`ArbitrageExecutor.sol`); observability green-gated (`readiness/verifiers/index.ts:45,56`); BUT grants need post-deploy on-chain `cast` verify; PnL canary telemetry missing | S2/S3/S4 |
| 5 | A.4 fork approved | 🔴 | `multistep_fork.rs` is mainnet-address-hardcoded → needs **Sepolia fixture post-deploy**; `bundle_builder.rs:443` defers ordering proof to "M5 fork validation" (not done) | S3 + operator |
| 6 | A.9 sign-off | 🔴 | this doc is the pre-sign-off; not yet signed | operator |
| 7 | CI green | 🔴 | **`security.yml` = FAILURE on main @ 2026-07-01** (audit allowlist expired 2026-06-30) — see §5 | S5 + operator |
| 8 | Security checks green | 🔴 | no-hardcode gate **neutered** (`lint-no-hardcode.sh:141` `exit 0` on violations) + **61 live violations**; gitleaks `continue-on-error:true` (`security.yml:144`); allowlist expired — §5 | S5 |
| 9 | No secrets exposed | 🟡 | signer hygiene good (`signer.rs:56` scrubbed Debug); BUT gitleaks non-blocking; prod VPS IP hardcoded in 18 workflow files (33×) in a PUBLIC repo | S5 |
| 10 | No hardcoded keys | 🟡 | no hardcoded private keys found (`compose.prod.yml` `${VAR:?required}`); BUT no-hardcode enforcement gate non-functional (§5) | S5 |
| 11 | Signer/KMS/HSM defined+validated | 🔴 | **raw local `LocalWallet` from `FLASHBOTS_SIGNER_KEY` (`signer.rs:18`) — NO KMS/HSM**; `KMS_KEY_ID=ABSENT_BY_POLICY`; mainnet **code-locked** (`live_exec_policy.rs:84`). THE architectural blocker | operator + S3 |
| 12 | Circuit breakers active | 🟡 | API breakers real (`consumer.ts:85,140`) + RPC breakers; BUT drawdown/gas-burn/latency breakers are **pure math with no caller** (`risk_ledger.rs` 0 callers); pre-execute Check 12 reads a Redis key with **no writer** (inert, false assurance) | S3 |
| 13 | Kill switch active | ✅ | real+wired+fail-closed: `submit_engine.rs:237`, `pre_execute_checklist.rs` Check 1, auto-trip `recon/anomaly.rs:74`, UI + e2e | done |
| 14 | Capital limits configured | 🟡 | per-bundle principal cap enforced fail-closed `max_value_eth=1.0` (`bundle_builder.rs:125`); BUT **no aggregate/portfolio cap** across `max_parallel_executions=8`; no on-chain cap | S3 |
| 15 | Gas/slippage/risk limits configured | 🟡 | slippage/min-profit/gas-safety-3× enforced (`risk_engine.rs`, `size_optimizer.rs:412`); BUT **`max_gas_price_gwei=200` NOT enforced on live broadcast tx** (`bundle_builder.rs:216`); possible price-impact unit bug (`config_aware.rs:157`) | S3 |
| 16 | Rollback runbook ready | 🔴 | fragmented: real auto-rollback only in "legacy" `deploy.yml:246`; canonical `hardened-vps-deploy.yml:727` points at **`hardened-vps-rollback.yml` that does not exist**; DR runbooks partly K8s/AWS-fictional vs the real single-VPS docker-compose | S5 + operator |
| 17 | Evidence pack | 🟡 | this doc + readiness verifiers attach evidence; BUT rollback evidence cannot prove a *validated* rollback (dangling workflow) | orchestrator |
| 18 | Explicit hefarica approval | 🔴 | human gate — pending | hefarica |

---

## 3. The 11 mainnet-readiness artifacts

### 3.1 Deployment plan
Sepolia-first, then **minimal-mainnet canary**. On-chain deploy is mature: `DeployMainnet.s.sol` gates on
`CONFIRM_MAINNET_DEPLOY` (L58) + `block.chainid==1` (L70) + multisig-is-contract (L79) + deployer balance
≥0.5 ETH (L104), and performs an **atomic admin→timelock handoff in the same broadcast** (L214-236,
`AdminTimelock` minDelay=86400). **Gap:** `DeployMultichain.deployOnChain` does NOT do the atomic handoff
(`DeployMultichain.s.sol:548`) — **mandate `DeployMainnet.s.sol` for L1**, never the multichain script.

### 3.2 Dry-run
M5 dry-run procedure exists (`docs/m5-sepolia-runbook.md`). Runs with **no secrets/no broadcast**. Must
confirm the deploy job declares `environment: sepolia-deploy` so GitHub pauses for hefarica's approval.
**Not yet executed.**

### 3.3 Fork simulation (A.4)
`multistep_fork.rs` is **mainnet-address + storage-layout hardcoded** (WETH/USDC/UniV2/Sushi, chain 1).
A.4 on Sepolia requires a **Sepolia fixture authored AFTER deploy** (token addrs + storage slots) — not a
chain-id one-liner. `bundle_builder.rs:443-449` explicitly defers the M1-barrier-orders-first proof to
this M5 fork validation. **Blocker for condition 5.**

### 3.4 Risk matrix
| Risk | Bound today | Gap |
|---|---|---|
| Per-trade oversize | `max_trade_size_usd`=effective_capital (`risk_engine.rs:72`) + Kelly 2% NAV | — |
| Per-bundle principal | `max_value_eth=1.0` fail-closed (`bundle_builder.rs:125`) | no aggregate across 8 parallel |
| Net-loss route | on-chain revert (`ArbitrageExecutor.sol:443-447`) + net-USD gas floor 3× (`size_optimizer.rs:412`) | — |
| Gas-price spike | forecast-time cap only | **not enforced at send** (`bundle_builder.rs:216`) |
| Slippage | `max_slippage_pct=1.5` (`risk_engine.rs:63`) | — |
| Drawdown / gas-burn | math exists (`risk_ledger.rs`) | **no caller — breaker cannot trip** |

### 3.5 Gas model
Gas cost is computed from the operator gas strategy and gated pre-broadcast; tx caps gas **units** at
`GAS_LIMIT_CEILING=30M` (`bundle_builder.rs:202`). **Blocker:** the configured gas-**price** ceiling
(`max_gas_price_gwei=200`) is consumed only by the scoring layer, **not** by the live `max_fee` set
(`base_fee*2+priority`, no ceiling check). A canary must add a hard `max_fee <= f(max_gas_price_gwei)` gate.

### 3.6 Rollback plan
Non-mainnet-grade today (condition 16). Required before go-live: one **canonical, protected,
rollback-capable** deploy workflow — build `hardened-vps-rollback.yml` (currently a dangling reference),
or wire the working `deploy.yml` rollback into the canonical path; reconcile the K8s/AWS-fictional DR
runbooks to the real docker-compose stack; wire kill-switch-engage into the deploy-failure path.

### 3.7 Monitoring plan
Prod ships a full stack (Prometheus/Grafana/Alertmanager→PagerDuty+Slack/Loki/Thanos/Vault,
`compose.prod.yml:415-748`), observability is a hard live-flip gate (`readiness/verifiers/index.ts:45,56`),
and breaker-state/kill-switch/revert-rate dashboards are grounded on emitted metrics. **Canary gap:**
`arbx_realized_pnl_usd` / `arbx_actual_profit_usd` / `arbx_sim_predicted_profit_usd` /
`arbx_revert_gas_wasted_usd` / `arbx_paper_mode_active` are **never emitted** → 5 alerts dead, incl. the
safety-critical **`PaperModeDisabled`** (cannot fire on a paper→live flip). Emit these before a canary.

### 3.8 Emergency-stop plan
Kill-switch is **READY** (condition 13): admin endpoint → Redis → checked before every broadcast, auto-trip
on high revert-rate, UI + e2e, fail-closed default `true`. **Gaps:** capital-protective breakers not wired
(3.4); manual `/admin/circuit_breakers/:name/{trip,reset}` publishes to a channel with **no subscriber**;
no graceful-pause (only hard-halt).

### 3.9 Capital exposure limits
Per-candidate + per-bundle caps are real and fail-closed (3.4). **Two gaps:** (a) **no aggregate/portfolio
ceiling** — 8 concurrent bundles × `max_value_eth` with no total-at-risk cap; (b) no on-chain absolute cap
(loss bounded by net-negative-revert + off-chain sizing only). Mainnet canary must set a small explicit
aggregate cap.

### 3.10 Signer isolation
**Strong isolation, blocking KMS gap.** Signing is physically confined to `relays-client`; `searcher-rs`
hard-panics if any of 8 capital-key envs is set and is observer-only; the M1 `LiveExecPolicy` is
default-deny, refuses `chain_id==1` unconditionally, is enforced as the first statement of
`build_and_sign` (`bundle_builder.rs:110`). **Blocker (condition 11):** the signer is a **raw local
`LocalWallet`** — no KMS/HSM, no multisig, no per-broadcast human-auth in code. A KMS/HSM (or hardware
multisig) signer must land **before** the mainnet code-lock is lifted.

### 3.11 Final checklist
See §2 (18 conditions) + §4 (blocker queue). **Go/No-Go = ALL 18 green + hefarica approval.**

---

## 4. Blocker queue (prioritized, with owners)

**P0 — must close before any mainnet flip:**
1. **KMS/HSM signer** (cond 11) — operator decision + S3 integration. Do NOT lift the mainnet code-lock first.
2. **`security.yml` RED / expired audit allowlist** (cond 7,8) — operator decides: dependency-upgrade sprint vs re-justify+extend BOTH allowlists (npm+cargo). Not auto-bumpable (rubber-stamping forbidden).
3. **No-hardcode gate neutered + 61 live violations** (cond 8,10) — triage the 61 (fix real, allow-list false-positives like `tests/**/*.spec.ts`), then flip `lint-no-hardcode.sh:141` to `exit 1`.
4. **Rollback path** (cond 16) — build the canonical protected rollback workflow; reconcile DR runbooks.

**P1 — must close before canary:**
5. Gas-price ceiling not enforced at send (cond 15) — `relays-client` `max_fee` gate.
6. No aggregate exposure cap (cond 14) — risk-layer total-at-risk ceiling.
7. Capital-protective breakers not wired (cond 12) — give `risk_ledger`/rolling-breakers a live I/O caller; give Check 12 a writer.
8. Canary telemetry not emitted (cond 4,17) — emit the 5 `arbx_*` PnL/paper-mode/gas-loss metrics → revives the dead alerts, incl. `PaperModeDisabled`.

**P2 — hygiene before sign-off:**
9. Prod VPS IP hardcoded in 18 public workflows — scrub to a secret/var.
10. ADR-0005 stale (claims auto-rollback that doesn't exist); DR-drill K8s/AWS fiction; runbook drift (`rpc-down.md`/`OPS-9` missing).
11. `DeployMultichain` atomic-handoff parity; confirm the `max_price_impact_pct` unit (0.05 → 0.05% vs 5%).

**Gated sequence (unchanged, human-owned):** #229 (S4 → READY) → Sepolia deploy under protected-env
approval → grants/approvals/selector on-chain-verified → A.4 fork (Sepolia fixture) → close P0s → A.9
sign-off → hefarica explicit approval → minimal-mainnet canary with a small aggregate cap.

---

## 5. Verified-against-reality addendum (independent re-check, 2026-07-01)

Two findings were re-verified directly (not just auditor-reported):

- **`security.yml` is RED on `main` right now.** `gh run list --workflow security.yml --branch main` →
  `failure @ 2026-07-01T10:37`. Root cause: `.github/npm-audit-allowlist.json` `expires:2026-06-30`
  (+ the cargo-audit `RUSTSEC_IGNORES` cliff in `security.yml:56`) elapsed; `npm-audit-gate.mjs:83`
  `process.exit(1)` once expired. **Not a merge-blocker** (`security.yml` is not in branch-protection's
  required contexts) but a real security-green failure (conditions 7,8).
- **No-hardcode gate is neutered AND main has 61 live violations.** Running `lint-no-hardcode.sh` on
  `eec065b0` prints **61 violations** yet returns **exit 0** (`lint-no-hardcode.sh:141`). So flipping to
  `exit 1` naïvely would turn the check RED (the 61 are mostly allow-list false-positives, e.g.
  `tests/e2e/**/*.spec.ts` not anchored, UX example constants). The fix is a *triage-then-enforce*, not a
  one-line flip.

Required branch-protection contexts (9): CodeQL · lint-and-test-{frontend,contracts,rust,node(22)} ·
lint · Doctrine grep gates · audit Dockerfiles · PII wireado recursive gates.

---

## 6. Sign-off (A.9) — to be completed by the operator

- [ ] All P0 closed with evidence
- [ ] Conditions 3,5,7,8,11,16 flipped to ✅ with linked evidence
- [ ] KMS/HSM signer validated on Sepolia
- [ ] Canary aggregate-capital cap set and enforced
- [ ] Rollback rehearsed on Sepolia
- [ ] **hefarica explicit written approval for a minimal-mainnet canary**

*Until every box is checked, the mainnet code-lock (`live_exec_policy.rs`) stays in place. This document
executes nothing.*
