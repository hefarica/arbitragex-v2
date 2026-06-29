# M5 — Sepolia Deploy Runbook + Operator Checklist

> **Status:** DRAFT (pre-execution). Execute ONLY after an explicit operator GO **and** after PR #224 + #228 are merged to `main` with `main` CI green.
> **Mode:** observer-only validation. NO mainnet, NO real capital, NO broadcast enablement.
> **Chain:** Sepolia (`chain_id = 11155111`).

This runbook makes the M2 fix (executor cross-DEX `_runRoute` + carry-validated-calldata) **executable on a real testnet** so the wrapped-flash sim can reach a genuine `SIM_SUCCESS` against a deployed, wired `FlashLoanExecutor` + `ArbitrageExecutor`.

---

## ⚠️ Audit findings baked into this runbook

1. **`DeployTestnet.s.sol` performs NO in-script grants** — unlike `DeployMainnet.s.sol` (which does the SC-13 `FLE→AE` grant in-script). On Sepolia **every** role grant + approval is a **manual** post-deploy step (Sections 2–4).
2. **`setRouterSelectorApproval` is absent from all three deploy-script checklists** — but the red Foundry repro proved a cross-DEX 2-leg route reverts `AE_RouterSelectorNotApproved` unless **both** routers' swap selectors are whitelisted (Section 4).
3. **The e2e fork harness `multistep_fork.rs` is hard-coded to chain 1** (`RPC_HTTP_1` / `EXECUTOR_1` / `FLASHLOAN_EXECUTOR_1`). For Sepolia, either point those chain-1 env names at the Sepolia fork (Section 7, option A) or parameterise the harness (small follow-up).

---

## Section 0 — Preconditions (secrets / credentials / balances)

```
[ ] PR #224 merged + PR #228 merged + main CI green
[ ] DEPLOYER_PRIVATE_KEY  — testnet deployer key (NOT mainnet). Holds DEFAULT_ADMIN/ADMIN_ROLE initially.
[ ] SEPOLIA_RPC           — Sepolia RPC URL (Alchemy/Infura). chain_id 11155111.
[ ] ETHERSCAN_API_KEY     — for --verify on Sepolia Etherscan.
[ ] AAVE_POOL_ADDRESS     — Aave V3 Sepolia Pool.
                            CANDIDATE: 0x6Ae43d3271ff6888e7Fc43Fd7321a503ff738951
                            ⚠️ PENDING MANUAL CONFIRMATION against https://aave.com/docs
                            (testnet markets are redeployed; verify .code.length > 0 on-chain).
[ ] EXECUTION_SIGNER      — the off-chain EOA that triggers requestFlashLoan ("the caller").
[ ] Sepolia ETH faucet balance on deployer + signer (gas).
```

---

## Section 1 — Deploy sequence

```bash
cd contracts
export DEPLOYER_PRIVATE_KEY=0x...  SEPOLIA_RPC=https://...  ETHERSCAN_API_KEY=...
export AAVE_POOL_ADDRESS=0x...     # confirmed in Section 0

forge script script/DeployTestnet.s.sol:DeployTestnet \
  --rpc-url "$SEPOLIA_RPC" --broadcast --verify --slow
```

Deploys four UUPS proxies (ArbitrageExecutor, AllowanceManager, FlashLoanExecutor[init: deployer, aavePool, AE proxy], AdminTimelock[60s]). **No in-script grants.** Record from the log:

```
AE  = <ArbitrageExecutor proxy>
AM  = <AllowanceManager proxy>
FLE = <FlashLoanExecutor proxy>
TL  = <AdminTimelock proxy>
```

---

## Section 2 — Post-deploy grants (ALL manual; do as DEPLOYER, BEFORE transferring ADMIN)

`EXECUTOR_ROLE = keccak256("EXECUTOR_ROLE")`. `grantRole` is callable by the deployer (DEFAULT_ADMIN). Do all grants/approvals while the deployer still holds ADMIN; transfer ADMIN to the timelock LAST.

```bash
AE=<...>;  FLE=<...>;  SIGNER=<EXECUTION_SIGNER>
ROLE=$(cast keccak "EXECUTOR_ROLE")   # ~0xd8aa0f31...469e63

# (a) signer → EXECUTOR_ROLE on AE
cast send $AE  "grantRole(bytes32,address)" $ROLE $SIGNER --rpc-url $SEPOLIA_RPC --private-key $DEPLOYER_PRIVATE_KEY
# (b) caller→FLE EXECUTOR_ROLE (the role the sim override flags)
cast send $FLE "grantRole(bytes32,address)" $ROLE $SIGNER --rpc-url $SEPOLIA_RPC --private-key $DEPLOYER_PRIVATE_KEY
# (c) SC-13: FLE proxy → EXECUTOR_ROLE on AE  (MANUAL on testnet; in-script only on DeployMainnet)
cast send $AE  "grantRole(bytes32,address)" $ROLE $FLE --rpc-url $SEPOLIA_RPC --private-key $DEPLOYER_PRIVATE_KEY
```

---

## Section 3 — Token + Router approvals

```bash
TOKENIN=<tokenIn>;  TOKENOUT=<intermediate tokenOut>;  RA=<forward_router>;  RB=<backward_router>
cast send $AE "setTokenApproval(address,bool)"  $TOKENIN  true --rpc-url $SEPOLIA_RPC --private-key $DEPLOYER_PRIVATE_KEY
cast send $AE "setTokenApproval(address,bool)"  $TOKENOUT true --rpc-url $SEPOLIA_RPC --private-key $DEPLOYER_PRIVATE_KEY
cast send $AE "setRouterApproval(address,bool)" $RA true --rpc-url $SEPOLIA_RPC --private-key $DEPLOYER_PRIVATE_KEY
cast send $AE "setRouterApproval(address,bool)" $RB true --rpc-url $SEPOLIA_RPC --private-key $DEPLOYER_PRIVATE_KEY
```

---

## Section 4 — 🔴 setRouterSelectorApproval (THE GAP — without it cross-DEX reverts `AE_RouterSelectorNotApproved`)

```bash
# Per-router swap selector (e.g. UniV2 swapExactTokensForTokens):
SEL=$(cast sig "swapExactTokensForTokens(uint256,uint256,address[],address,uint256)")   # 0x38ed1739
# BOTH routers — the repro proved the BACKWARD leg needs its selector whitelisted too:
cast send $AE "setRouterSelectorApproval(address,bytes4,bool)" $RA $SEL true --rpc-url $SEPOLIA_RPC --private-key $DEPLOYER_PRIVATE_KEY
cast send $AE "setRouterSelectorApproval(address,bytes4,bool)" $RB $SEL true --rpc-url $SEPOLIA_RPC --private-key $DEPLOYER_PRIVATE_KEY
# (Adjust SEL per router if forward/backward use different DEX function signatures.)
```

---

## Section 5 — On-chain validation of grants/approvals (before simulating)

```bash
cast call $AE  "hasRole(bytes32,address)(bool)" $ROLE $SIGNER --rpc-url $SEPOLIA_RPC   # → true
cast call $FLE "hasRole(bytes32,address)(bool)" $ROLE $SIGNER --rpc-url $SEPOLIA_RPC   # → true
cast call $AE  "hasRole(bytes32,address)(bool)" $ROLE $FLE    --rpc-url $SEPOLIA_RPC   # → true (SC-13)
cast call $AE  "approvedTokens(address)(bool)"  $TOKENIN --rpc-url $SEPOLIA_RPC        # → true
cast call $AE  "approvedRouters(address)(bool)" $RA --rpc-url $SEPOLIA_RPC             # → true
cast call $AE  "approvedSelectors(address,bytes4)(bool)" $RA $SEL --rpc-url $SEPOLIA_RPC # → true
```

---

## Section 6 — Aave Sepolia liquidity (PENDING manual confirmation)

```
[ ] Confirm the Aave V3 Sepolia Pool has liquidity for TOKENIN (flashLoanSimple).
    ⚠️ Cannot be verified offline. Typical Aave V3 Sepolia test-market assets with faucet:
    WETH, USDC, DAI, LINK. Verify on-chain:
        cast call $AAVE_POOL_ADDRESS "getReserveData(address)" $TOKENIN --rpc-url $SEPOLIA_RPC
    and that the aToken holds enough underlying. If no liquidity → flash loan reverts
    (fail-closed) → choose another TOKENIN with Aave-Sepolia liquidity.
[ ] Choose a cross-DEX pair (forward_router ≠ backward_router) whose tokens have real reserves on
    Sepolia DEXes. If no real arb exists, the sim reverts ZeroGrossProfit (correct/honest) — to
    validate the PATH, use reserves that leave spread > premium.
```

---

## Section 7 — E2E: real `SIM_SUCCESS` on a fork

Harness: `backend/searcher-rs/tests/multistep_fork.rs` (`#[ignore]` by default).
⚠️ Hard-coded to chain 1 env names. Two options:

**Option A — point chain-1 env at the Sepolia fork:**
```bash
export RPC_HTTP_1="$SEPOLIA_RPC"  EXECUTOR_1=$AE  FLASHLOAN_EXECUTOR_1=$FLE
export SIM_ORCHESTRATOR_GAS_PRICE_WEI=<real Sepolia gwei in wei>
# (run in WSL2 with build-essential, or any linux toolchain)
cargo test -p searcher-rs --test multistep_fork -- --ignored --nocapture
```

**Option B —** parameterise the harness to `chain_id` (small follow-up PR).

**Success** = `SimulationOutcome.passed` with `retained_spread > 0`, `trace_hash != 0`, `gas_used > 0`, and the wrapped bytes equal `build_flash_funded_broadcast_calldata_with_intermediate(...)` (byte-parity). Repeat **≥10** simulations (program M5 criterion).

---

## Section 8 — ABORT criteria

```
[ ] Any grant/approval in Sections 2–4 reverts → ABORT; check the caller's ADMIN_ROLE.
[ ] Any hasRole/approved* in Section 5 returns false → ABORT (don't simulate with incomplete wiring).
[ ] flashLoanSimple reverts on liquidity → ABORT; change TOKENIN (Section 6).
[ ] e2e SIM_SUCCESS not reached in 10 attempts with a real arb → ABORT; do NOT declare M5 green.
[ ] Any real broadcast/signing appears → ABORT immediately (M5 is observer-only/sim).
```

---

## Section 9 — Rollback

UUPS contracts are not "deleted". Rollback = stop using the addresses + do NOT wire the searcher
(do not export `EXECUTOR_11155111` / `FLASHLOAN_EXECUTOR_11155111`) → the producer stays fail-closed
(no plan persisted, no broadcast). To revoke access: `revokeRole` the granted `EXECUTOR_ROLE`s and
`setTokenApproval`/`setRouterApproval(..., false)`. If ADMIN was already transferred to the timelock,
changes go through its 60s delay.

---

## Operator checklist (copy/paste — execute ONLY after GO + #224/#228 merged)

```
## M5 SEPOLIA — OPERATOR CHECKLIST

PRE
[ ] #224 merged · #228 merged · main CI green
[ ] AAVE_POOL_ADDRESS Sepolia CONFIRMED (.code.length>0)
[ ] DEPLOYER_PRIVATE_KEY (testnet) + SEPOLIA_RPC + ETHERSCAN_API_KEY exported
[ ] deployer + signer funded with Sepolia ETH

DEPLOY
[ ] forge script DeployTestnet --broadcast --verify --slow → record the 4 proxies

GRANTS (as deployer, BEFORE transferring ADMIN)
[ ] AE.grantRole(EXECUTOR_ROLE, SIGNER)
[ ] FLE.grantRole(EXECUTOR_ROLE, SIGNER)        ← caller→FLE role
[ ] AE.grantRole(EXECUTOR_ROLE, FLE_proxy)      ← SC-13 (manual on testnet)

APPROVALS
[ ] AE.setTokenApproval(tokenIn, true) + setTokenApproval(tokenOut, true)
[ ] AE.setRouterApproval(forward_router, true) + setRouterApproval(backward_router, true)
[ ] AE.setRouterSelectorApproval(forward_router, SEL, true)    ← GAP
[ ] AE.setRouterSelectorApproval(backward_router, SEL, true)   ← GAP

VERIFY ON-CHAIN
[ ] hasRole on AE/FLE for SIGNER and FLE → 3× true
[ ] approvedTokens / approvedRouters / approvedSelectors → true
[ ] Aave reserve liquidity for tokenIn → sufficient

E2E
[ ] export EXECUTOR_11155111=AE  FLASHLOAN_EXECUTOR_11155111=FLE  RPC_HTTP_11155111=SEPOLIA_RPC
[ ] run multistep_fork (pointed at Sepolia) → SIM_SUCCESS with spread>0, byte-parity
[ ] ≥10 real simulations OK

GATE
[ ] NO real broadcast/signing occurred (observer-only)
[ ] M5 declared green ONLY with evidence of the 10 SIM_SUCCESS
```

---

*Generated as a draft for operator review. The net-USD-of-gas pre-live gate (branch `omega/m2-net-usd-gate`) and the chain-1→chain_id harness generalisation are tracked follow-ups, not part of this runbook's execution.*
