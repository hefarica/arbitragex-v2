# M5 — Sepolia Deploy Runbook + Operator Checklist

> **Status:** READY for operator review. Execute ONLY after an explicit operator GO. Code preconditions are MET on `main` (CI green): **PR #224 merged** (executor cross-DEX `_runRoute` + carry-validated-calldata + the `ZeroIntermediate` test) and **PR #246 merged** (net-USD-of-gas pre-live gate). (PR #228 was **closed** — its rmcp allowlist already landed via #223.)
> **Mode:** observer-only validation. NO mainnet, NO real capital, NO broadcast enablement.
> **Chain:** Sepolia (`chain_id = 11155111`).
> **Plane (governance — CI keyless):** GitHub Actions (`m5-sepolia-validation.yml`) is **keyless** — it only **simulates** the deploy (no signing, no on-chain send) and runs the read-only A.4 fork validation. **The live deploy in Sections 1–5 below runs from the operator/KMS plane (your machine, local keystore/HSM), NEVER from CI.** No deployer signing key is ever a GitHub secret. *CI validates; the operator signs. CI never custodies or transmits keys.*

This runbook makes the M2 fix (executor cross-DEX `_runRoute` + carry-validated-calldata) **executable on a real testnet** so the wrapped-flash sim can reach a genuine `SIM_SUCCESS` against a deployed, wired `FlashLoanExecutor` + `ArbitrageExecutor`.

---

## ⚠️ Audit findings baked into this runbook

1. **`DeployTestnet.s.sol` performs NO in-script grants** — unlike `DeployMainnet.s.sol` (which does the SC-13 `FLE→AE` grant in-script). On Sepolia **every** role grant + approval is a **manual** post-deploy step (Sections 2–4).
2. **`setRouterSelectorApproval` is absent from all three deploy-script checklists** — but the red Foundry repro proved a cross-DEX 2-leg route reverts `AE_RouterSelectorNotApproved` unless **both** routers' swap selectors are whitelisted (Section 4).
3. **The e2e fork harness `multistep_fork.rs` is MAINNET-SPECIFIC — not merely chain-1-env** (verified 2026-06-30). Beyond the `RPC_HTTP_1` / `EXECUTOR_1` / `FLASHLOAN_EXECUTOR_1` env names, it hard-codes **mainnet token/router addresses** (WETH `0xc02a…`, USDC `0xa0b8…`, UniV2 `0x7a25…`, Sushi `0xd9e1…` — lines ~200-203), **mainnet ERC20 storage layouts** (lines ~76-90, keyed to chain 1) and `chain_id: 1` (line ~224). **Re-pointing the env at Sepolia is NOT sufficient** — the test would still drive mainnet addresses against Sepolia and fail. A.4 on Sepolia requires adding a **Sepolia fixture** (Sepolia token/router addresses + their on-chain storage slots), which can only be authored **after** the deploy. See Section 7.

---

## Section 0 — Preconditions (secrets / credentials / balances)

```
[x] PR #224 merged (DONE) + PR #246 net-USD merged (DONE) + main CI green   (#228 closed — allowlist already on main via #223)
[ ] DEPLOYER_PRIVATE_KEY  — testnet deployer key (NOT mainnet). Holds DEFAULT_ADMIN/ADMIN_ROLE initially.
[ ] SEPOLIA_RPC           — Sepolia RPC URL (Alchemy/Infura). chain_id 11155111.
[ ] ETHERSCAN_API_KEY     — for --verify on Sepolia Etherscan.
[x] AAVE_POOL_ADDRESS      = 0x6Ae43d3271ff6888e7Fc43Fd7321a503ff738951  (Aave V3 Sepolia Pool)
                            ✅ CONFIRMED on-chain 2026-07-01 (chainId 11155111): contract present
                            (4847 bytes), ADDRESSES_PROVIDER 0x012bAC54348C0E635dCAc9D5FB99f06F24136C9A,
                            getReservesList() = 9 reserves (see Section 6 for tokenIn candidates).
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

## Section 6 — Aave Sepolia liquidity (Pool ✅ CONFIRMED on-chain 2026-07-01)

Pool `0x6Ae43d3271ff6888e7Fc43Fd7321a503ff738951` is a live Aave V3 Pool on Sepolia
(`getReservesList()` verified). **9 reserves with faucet — `tokenIn` candidates for `flashLoanSimple`:**

```
WETH  0xC558DBdd856501FCd9aaF1E62eae57A9F0629a3c  (18)   <- liquid, recommended
USDC  0x94a9D9AC8a22534E3FaCa9F4e7F2E2cf85d5E4C8  (6)    <- liquid, recommended
DAI   0xFF34B3d4Aee8ddCd6F9AFFFB6Fe49bD371b8a357  (18)
USDT  0xaA8E23Fb1079EA71e0a56F48a2aA51851D8433D0  (6)
WBTC  0x29f2D40B0605204364af54EC677bD022dA425d03  (8)
LINK  0xf8Fb3713D459D7C1018BD0A49D19b4C44290EBE5  (18)
AAVE  0x88541670E55cC00bEEFD87eB59EDd1b7C511AC9a  (18)
EURS  0x6d906e526a4e2Ca02097BA9d0caA3c382F52278E  (2)
GHO   0xc4bF5CbDaBE595361438F8c6a187bDc330539c60  (18)
```

```
[ ] Confirm the chosen TOKENIN reserve holds enough underlying for the flash amount:
        cast call $AAVE_POOL_ADDRESS "getReserveData(address)" $TOKENIN --rpc-url $SEPOLIA_RPC
    If insufficient -> flash loan reverts (fail-closed) -> pick another (WETH/USDC are safest).
[ ] Choose a cross-DEX pair (forward_router != backward_router) whose tokens have real reserves on
    Sepolia DEXes. If no real arb exists, the sim reverts ZeroGrossProfit (correct/honest) — to
    validate the PATH, use reserves that leave spread > premium.
```

---

## Section 7 — E2E: real `SIM_SUCCESS` on a fork

Harness: `backend/searcher-rs/tests/multistep_fork.rs` (`#[ignore]` by default).

🔴 **The harness is MAINNET-SPECIFIC (verified) — re-pointing env is NOT enough.** It hard-codes
mainnet token/router addresses (lines ~200-203), mainnet ERC20 storage layouts (lines ~76-90),
and `chain_id: 1` (line ~224). A Sepolia run needs a **Sepolia fixture**: the Sepolia test-token
addresses (tokenIn/tokenOut), the Sepolia router addresses, and the **on-chain storage slots**
(`balanceOf` / `allowance` base slots) for each Sepolia token — all deployment-specific, so they
can only be filled in **after** Section 1 (read slots with `cast storage` on the live tokens).

```bash
# 1) After the deploy, set the chain-11155111 env the producer/searcher reads:
export RPC_HTTP_11155111="$SEPOLIA_RPC"  EXECUTOR_11155111=$AE  FLASHLOAN_EXECUTOR_11155111=$FLE
export SIM_ORCHESTRATOR_GAS_PRICE_WEI=<real Sepolia gwei in wei>  SIM_ORCHESTRATOR_MODE=<mode>
# 2) Add a Sepolia fixture to multistep_fork.rs (S3/S4 follow-up — NOT a chain-id one-liner):
#    - FixtureLayoutProvider: insert (11155111, <sepolia_token>) -> {balance_slot, allowance_slot}
#      discover slots: cast storage <token> <slotGuess> --rpc-url $SEPOLIA_RPC
#    - RoundTripContext: Sepolia tokenIn/tokenOut/forward_router/backward_router
#    - MultiStepExecutionConfig { chain_id: 11155111, ... }
# 3) Run (WSL2 with build-essential, or any linux toolchain):
cargo test -p searcher-rs --test multistep_fork -- --ignored --nocapture
```

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

## Operator checklist (copy/paste — execute ONLY after explicit GO; #224 + #246 already merged)

```
## M5 SEPOLIA — OPERATOR CHECKLIST

PRE
[x] #224 merged · #246 net-USD merged · main CI green   (#228 closed — allowlist via #223)
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
[ ] add Sepolia fixture to multistep_fork.rs (token addrs + storage slots, Section 7) → run → SIM_SUCCESS spread>0, byte-parity
[ ] ≥10 real simulations OK

GATE
[ ] NO real broadcast/signing occurred (observer-only)
[ ] M5 declared green ONLY with evidence of the 10 SIM_SUCCESS
```

---

*Finalized for operator review (2026-06-30). The net-USD-of-gas pre-live gate landed as **PR #246** (merged to `main`). The fork-harness Sepolia work (Section 7) is a deployment-dependent S3/S4 follow-up: it needs the live Sepolia token addresses + their storage slots, so it is authored **after** Section 1 — it is NOT a chain-id one-liner. This runbook stays observer-only: NO mainnet, NO real capital, NO broadcast/signer enablement; M5 executes only under an explicit operator GO with a protected (KMS/HSM/hardware) testnet key, never from CI.*
