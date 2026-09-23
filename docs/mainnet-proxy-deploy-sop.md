# ArbitrageExecutor Mainnet Deploy — Master SOP & Historical Context

> **Status:** READY for mainnet (P0 security blockers closed 2026-07-03).
> **Operation type:** Irreversible mainnet broadcast (real gas, real deployment).
> **Authority:** Operator-only (hefarica). Claude NEVER executes this without
> explicit per-action GO + all prerequisites verified.

---

## 1. TL;DR — Why this document exists

The `ArbitrageExecutor` proxy is the **last physical artifact** needed for
G-SIM-1's real-simulation path to be end-to-end functional. Without it deployed
on Ethereum mainnet:

- B2c (`execute_multistep_revm`, merged in #271) produces `wrapped_calldata`
  targeting a contract that **does not exist on-chain** → every simulation
  reverts with "no contract at address".
- The sim↔broadcast parity invariant (the doctrinal gate that proves the
  calldata the sim validated is byte-identical to what searcher-rs would
  broadcast) **cannot be validated**.
- `ARBX_SIMULATOR_V2_READY` cannot be honestly flipped to `true`.

**This was not deployable before 2026-07-03** because the deploy script had
two P0 security holes that would have given the deployer EOA instant,
timelock-bypassing drain authority over all deployed contracts. Those are now
closed. This document records the full history so the decision to deploy is
made with complete context.

---

## 2. The role of the ArbitrageExecutor proxy

### What it is
A UUPS-upgradeable proxy (`ERC1967Proxy`) wrapping `ArbitrageExecutor.sol` —
the on-chain component that performs **flash-funded atomic arbitrage**:

```
                 ┌─────────────────────────────────────────────┐
                 │           ArbitrageExecutor (proxy)          │
   off-chain ───►│ executeArbitrageFlashFunded(tokenIn, amt,   │
   signer        │   route, minProfit)                          │
   (EXECUTOR     │   │                                          │
   _ROLE)        │   ▼ calls FlashLoanExecutor (EXECUTOR_ROLE)  │
                 │       │                                      │
                 │       ▼ Aave V3 flash loan → swaps → repay   │
                 │       │                                      │
                 │   ▼ retains spread (profit - premium)        │
                 └─────────────────────────────────────────────┘
```

### Why it exists (vs. off-chain execution)
1. **Atomicity**: the multi-hop swap sequence + flash-loan repay MUST land in
   one transaction or none — only an on-chain contract can guarantee this.
2. **Capital efficiency**: flash-funded means **zero capital at risk** during
   execution (the flash loan is the capital); the contract only retains the
   spread.
3. **Role-gated custody**: `EXECUTOR_ROLE` (off-chain signer),
   `UPGRADER_ROLE` (timelock), `DEFAULT_ADMIN_ROLE` (timelock) — separation
   of duties enforced in-code.

### The 4-contract system

| Contract | Role | Admin |
|----------|------|-------|
| **ArbitrageExecutor** (AE) | Runs the flash-funded arb; receives the spread | Timelock |
| **AllowanceManager** (AM) | Per-token/per-router spend caps (the "vault door") | Timelock |
| **FlashLoanExecutor** (FLE) | Borrows from Aave V3, calls AE, repays | Timelock |
| **AdminTimelock** | 24h delay on all admin/upgrade actions; multisig = proposer+executor | Self-admin (renounced from deployer) |

**Critical wiring** (done atomically in the deploy script):
- FLE holds `EXECUTOR_ROLE` on AE (SC-13 fund-handoff — the flash path calls
  `executeArbitrageFlashFunded` with FLE as `msg.sender`).
- AE references AM (post-deploy step 1).
- FLE references AE proxy + Aave V3 pool.

---

## 3. Historical analysis — why this wasn't deployed before

### The three eras of contract development

#### Era 1 — Initial development (commits `c9baa0e` → `864875f`)
- SC-10: `AdminTimelock` + fuzz tests
- SC-11: AdminTimelock proposer/canceller/delay-floor guards
- Deploy script `DeployMainnet.s.sol` created
- **State:** contracts existed but unaudited for custody correctness

#### Era 2 — Audit findings (commits `5b5ca32` → `b5cd145`)
- Admin-surface guards, takeover guards, proxy/init tests
- **SC-13** flash-loan fund-handoff gap discovered + fixed (`07349f4`, `12dbffd`)
- Symmetric outbound capital-retention guard (`b5cd145`)
- SC-16 determinism: FL proxy must target AE proxy (`d3c636f`)
- **State:** contracts hardened on the fund-flow axis, but **cross-DEX
  execution was still broken** and **role custody was still unsafe**

#### Era 3 — Functional + security P0 closure (2026-06-30 → 2026-07-03)

| Commit / PR | What it closed | Severity |
|-------------|----------------|----------|
| `d24c370` / **#224** | **Cross-DEX blocker**: AE couldn't execute 2-router arbs (backward leg spent unapproved `tokenOut` → `SwapFailed`). Fix: `_runRoute` approve-per-hop + on-chain intermediate. | P0 functional |
| `41cb0c6` / #245 | `ZeroIntermediate` adversarial test (proves the cross-DEX gate can't be bypassed) | P0 test |
| `e1000e3` / **#257** | **UPGRADER_ROLE → timelock atomic handoff**: deployer no longer keeps instant `upgradeToAndCall` that bypassed multisig + 24h delay. | **P0 security (drain vector)** |
| `29ba2ae` / **#258** | **DEFAULT_ADMIN renounce over AdminTimelock**: deployer could instant-`grantRole(PROPOSER/EXECUTOR)` defeating multisig separation. | **P0 security (governance bypass)** |

**This is the answer to "why only now":** before #257 + #258 (2026-07-03),
deploying to mainnet would have handed the deployer EOA:

1. An instant upgrade path (`UPGRADER_ROLE`) that bypassed the 24h timelock →
   a single deployer key compromise = drain of all deployed contracts.
2. Instant `PROPOSER/EXECUTOR` granting over the timelock itself → multisig
   separation of duties was decorative, not enforced.

**Deploying before 2026-07-03 would have been a doctrinal violation** (the
7-auditor sweep missed #257; the adversarial review caught it — this is why
the adversarial gate exists). The operator correctly chose "toward live
behind gates" and held the deploy until the P0s closed.

### Era 4 — Simulation wiring (2026-07-04, this session)
- **#271 B2c**: `execute_multistep_revm` wired into sim-ctl (REAL multi-step
  REVM, replacing the `calldata=Vec::new()` stub).
- **#272 step 6**: scanner captures route topology at detection time.
- **#273**: flip checklist for `ARBX_SIMULATOR_V2_READY`.

**Now** the simulation side is complete. The only missing artifact is the
**physical proxy deployment** on mainnet — which is finally safe to do.

---

## 4. What changed to make this safe (the P0 closure evidence)

### P0-1: UPGRADER_ROLE custody (`#257`)
**Before:** `DeployMainnet.s.sol:214-236` "atomic handoff" moved only
`DEFAULT_ADMIN_ROLE`; `_authorizeUpgrade` was gated solely on `UPGRADER_ROLE`
which stayed on the deployer EOA → instant-drain via `upgradeToAndCall`.

**After** (`DeployMainnet.s.sol:200-272`, the M10 + P0 block): for each of
AE, AM, FLE — atomically, inside the same broadcast:
1. `grantRole(UPGRADER_ROLE, timelock)`
2. `revokeRole(UPGRADER_ROLE, deployer)`
3. `grantRole(DEFAULT_ADMIN_ROLE, timelock)`
4. `revokeRole(DEFAULT_ADMIN_ROLE, deployer)`

Order is critical (comment in-script): granting to timelock BEFORE revoking
from deployer ensures the contract is never without an admin/upgrader (which
would brick it).

### P0-2: AdminTimelock DEFAULT_ADMIN renounce (`#258`)
**Before:** OZ `TimelockController` grants `DEFAULT_ADMIN_ROLE` to both
`address(this)` (self-admin) AND the `admin` arg (deployer). The script's
comment "admin is renounced below" was **false** — the deployer kept
`DEFAULT_ADMIN_ROLE` over the timelock itself → could
`grantRole(PROPOSER/EXECUTOR, deployer)` instantly (a direct AccessControl
call, NOT through the delay) → multisig defeated.

**After** (`DeployMainnet.s.sol:271`): `tl.renounceRole(DEFAULT_ADMIN_ROLE,
deployer)` — the deployer's bootstrap admin is dropped. The timelock's
self-admin (`address(this)`) survives, so the multisig can still manage roles
via `schedule+execute`.

### Validation
- `DeployMainnetRoleCustody.t.sol`: 8 tests (RED→GREEN on the fix; 6/8 fail
  pre-fix = the drain vector was live).
- `DeployMainnetTimelockAdminCustody.t.sol`: 2 tests (anti-brick verified:
  self-admin survives the renounce).
- Both run in CI on every contract change.

---

## 5. Deploy SOP — the exact procedure

> **Authority:** operator (hefarica). Claude assists with preparation +
> verification but does NOT broadcast without explicit GO.

### 5.1 Pre-deploy checklist (operator, read-only)

| # | Check | How to verify |
|---|-------|---------------|
| 1 | Foundry installed | `forge --version` (≥ nightly-2024-01) |
| 2 | `MULTISIG_ADDRESS` is a **contract** (Gnosis Safe), not an EOA, and differs from deployer | `cast code $MULTISIG` on mainnet returns `0x...` (non-empty) |
| 3 | Deployer EOA has ≥ 0.5 ETH | `cast balance $DEPLOYER` |
| 4 | Deployer key is a **hot key** (NOT the multisig key) | operational hygiene |
| 5 | `MAINNET_RPC_URL` is a dedicated endpoint (not the public one) | operator knowledge |
| 6 | `ETHERSCAN_API_KEY` set | for `--verify` |
| 7 | P1 blockers reviewed (§7) — operator accepts residual risk | sign-off in audit trail |

### 5.2 Environment (export before running)

```bash
export DEPLOYER_PRIVATE_KEY=0x<hot-key>        # NOT the multisig key
export MULTISIG_ADDRESS=0x<Safe>               # contract, validated in-script
export CONFIRM_MAINNET_DEPLOY=true             # explicit opt-in gate
export MAINNET_RPC_URL=https://...             # dedicated RPC
export ETHERSCAN_API_KEY=<key>                 # --verify
# Optional override (default is the verified Aave V3 mainnet pool):
# export AAVE_V3_POOL=0x87870Bca3F3fD6335C3F4ce8392D69350B4fA4E2
```

### 5.3 Dry run (MANDATORY — no broadcast)

```bash
cd contracts
forge script script/DeployMainnet.s.sol \
  --rpc-url $MAINNET_RPC_URL \
  -vvvv
```

**Verify in output:**
- `Chain ID : 1`
- `Deployer balance:` ≥ 0.5 ETH
- `Multisig :` matches your Gnosis Safe
- NO simulation errors
- The "Atomic role-custody transfer" block runs (grants + revokes for all 4 contracts)

**Do NOT proceed to live deploy if any check fails.**

### 5.4 Live deploy (broadcast — IRREVERSIBLE)

```bash
forge script script/DeployMainnet.s.sol \
  --rpc-url $MAINNET_RPC_URL \
  --broadcast \
  --verify \
  -vvvv
```

Foundry writes broadcast artifacts to `broadcast/DeployMainnet.s.sol/1/`.
**Save the 4 proxy addresses** from the output immediately:
```
ArbitrageExecutor proxy : 0x...
AllowanceManager proxy  : 0x...
FlashLoanExecutor proxy : 0x...
AdminTimelock proxy     : 0x...
```

### 5.5 Post-deploy wiring (mandatory — see contracts/DEPLOY.md §Post-deploy)

These are **multisig + 24h timelock** actions now (the deployer EOA has no
roles after the atomic handoff). They MUST be scheduled via the timelock:

1. `ArbitrageExecutor.setAllowanceManager(<AM proxy>)`
2. `ArbitrageExecutor.grantRole(EXECUTOR_ROLE, <off-chain signer>)`
3. `FlashLoanExecutor.grantRole(EXECUTOR_ROLE, <off-chain signer>)`
4. `ArbitrageExecutor.setTokenApproval(<WETH|USDC|...>, true)` (per token)
5. `ArbitrageExecutor.setRouterApproval(<UniV3Router|...>, true)` (per router)
6. `AllowanceManager.batchGrantAllowance([tokens], [routers], [amounts])`
7. (Optional) `FlashLoanExecutor.setReferralCode(<code>)`
8. (Optional) `FlashLoanExecutor.setBalancerVault(0xBA12...)`
9. **Verify custody** (read-only):
   ```bash
   cast call <AE> 'hasRole(bytes32,address)' <UPGRADER_ROLE_HASH> <timelock>  # → true
   cast call <AE> 'hasRole(bytes32,address)' <UPGRADER_ROLE_HASH> <deployer>  # → false
   ```

---

## 6. How this plugs into G-SIM-1 / B2c

Once the proxy is deployed, `ARBITRAGE_EXECUTOR` (the AE proxy address)
becomes a real value:

```
deploy proxy → AE proxy address
            → loaded as ARBITRAGE_EXECUTOR secret in paper-ops env
            → ops-b2c-activate.yml workflow upserts it to VPS .env
            → sim-ctl's run_real_simulation() encodes calldata targeting the REAL proxy
            → execute_multistep_revm validates against forked mainnet state (incl. the proxy)
            → wrapped_calldata is now byte-identical to what searcher-rs would broadcast
            → sim↔broadcast parity proven → ARBX_SIMULATOR_V2_READY can be honestly flipped
```

Without the proxy, step 3 onward fails. **The proxy is the keystone.**

---

## 7. Residual risks (P1 — NOT blockers, but acknowledged)

These do NOT prevent deploy but are tracked debt the operator accepts:

| Risk | Location | Impact |
|------|----------|--------|
| `no-hardcode` gate neutered (`exit 0` on violations) | `automation/tools/lint-no-hardcode.sh:141` | 61 live violations not enforced; could mask future hardcoding |
| `nonce_manager.rs:56` `refresh()` dead code | `backend/relays-client/` | Non-landed bundle burns local nonce → silent stop after first drop |
| Gas-price ceiling not enforced at send | `bundle_builder.rs:216` | Could overspend on gas spikes |
| `U256::as_u128` panic in value-cap guard | relays-client | Value > u128 max → panic (DoS) |
| `security.yml` RED (npm allowlist) | CI | Non-required check; advisory only |

**Mitigation posture:** these are all **paper-mode-contained** (no live
broadcast path active) until `ARBX_SIMULATOR_V2_READY` flips AND the live-exec
policy (`relays-client/src/live_exec_policy.rs`) is deliberately changed.
Neither is in scope for the proxy deploy.

---

## 8. Rollback (limited — deploy is irreversible)

The deploy is a one-way mainnet operation. However, because custody moved to
the timelock atomically:

- **Upgrade path** (e.g., to fix a bug in AE logic): schedule via multisig +
  24h delay → `upgradeToAndCall` on the proxy. This is the sanctioned path.
- **Pause/disable**: there is no `pause()` in the current contracts (tracked
  as separate work). Emergency response = upgrade that adds a circuit breaker.
- **Fund recovery**: funds in AE/AM are held behind `EXECUTOR_ROLE` — the
  multisig (via timelock) can move them.

**There is no "un-deploy".** The proxy addresses are permanent on mainnet.

---

## 9. Related references

- `contracts/script/DeployMainnet.s.sol` — the deploy script (audited)
- `contracts/DEPLOY.md` — operational runbook (pre/post wiring)
- `contracts/test/DeployMainnetRoleCustody.t.sol` — custody validation (8 tests)
- `contracts/test/DeployMainnetTimelockAdminCustody.t.sol` — timelock admin (2 tests)
- `docs/gsim1-simulator-v2-ready-flip-checklist.md` — the next gate after this
- PRs: #224 (cross-DEX), #245 (ZeroIntermediate test), #257 (UPGRADER), #258 (DEFAULT_ADMIN)
- Memory: `arbx-mainnet-readiness-dossier` (corrected 2026-07-04: P0s are MERGED, not open)

---

## 10. Decision record

**2026-07-04 — this document created.** The proxy deploy is now doctrinally
safe (P0s closed). The decision to deploy rests on:

1. ✅ Functional blocker resolved (#224 cross-DEX)
2. ✅ Security P0s resolved (#257, #258 custody)
3. ✅ Simulation path wired (#271 B2c, #272 scanner capture)
4. ✅ Flip checklist documented (#273)
5. ⏳ Operator provides: Gnosis Safe address, deployer hot key with ≥0.5 ETH,
   dedicated RPC, Etherscan key
6. ⏳ Operator accepts residual P1 risk (§7)

**No deploy proceeds without explicit operator GO on this specific action.**
