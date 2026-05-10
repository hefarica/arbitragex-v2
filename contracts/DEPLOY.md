# ArbitrageX v2 — Mainnet Deployment Runbook

This document covers the full procedure for deploying to Ethereum mainnet.
For testnet (Sepolia / Holesky), use `script/DeployTestnet.s.sol`.

## Prerequisites

| Item | Requirement |
|------|-------------|
| Foundry | `forge --version` >= nightly-2024-01 |
| Deployer wallet | EOA with >= 0.5 ETH for gas |
| Etherscan API key | For automatic contract verification |
| Mainnet RPC URL | Dedicated Alchemy or QuickNode endpoint |

## Environment variables

```bash
export DEPLOYER_PRIVATE_KEY=0x<hot-key>     # NOT the multisig key
export CONFIRM_MAINNET_DEPLOY=true           # Explicit opt-in safety gate
export MAINNET_RPC_URL=https://...           # Dedicated RPC endpoint
export ETHERSCAN_API_KEY=<key>               # For --verify
```

## Dry run (simulation only, no broadcast)

```bash
cd contracts
forge script script/DeployMainnet.s.sol \
  --rpc-url $MAINNET_RPC_URL \
  -vvvv
```

Inspect the output. Verify:
- Chain ID printed as `1`
- Deployer balance shown as sufficient
- No simulation errors

## Live deploy

```bash
forge script script/DeployMainnet.s.sol \
  --rpc-url $MAINNET_RPC_URL \
  --broadcast \
  --verify \
  -vvvv
```

Foundry writes the broadcast artifacts to `broadcast/DeployMainnet.s.sol/1/`.

## Post-deploy wiring (mandatory)

Run these transactions from the deployer address (or multisig if already transferred).

### 1. Wire AllowanceManager to ArbitrageExecutor

```bash
cast send <ArbitrageExecutor_proxy> \
  "setAllowanceManager(address)" <AllowanceManager_proxy> \
  --rpc-url $MAINNET_RPC_URL \
  --private-key $DEPLOYER_PRIVATE_KEY
```

### 2. Grant EXECUTOR_ROLE on ArbitrageExecutor

```bash
EXECUTOR_ROLE=$(cast call <ArbitrageExecutor_proxy> "EXECUTOR_ROLE()(bytes32)")
cast send <ArbitrageExecutor_proxy> \
  "grantRole(bytes32,address)" $EXECUTOR_ROLE <off_chain_signer> \
  --rpc-url $MAINNET_RPC_URL \
  --private-key $DEPLOYER_PRIVATE_KEY
```

### 3. Grant EXECUTOR_ROLE on FlashLoanExecutor

```bash
cast send <FlashLoanExecutor_proxy> \
  "grantRole(bytes32,address)" $EXECUTOR_ROLE <off_chain_signer> \
  --rpc-url $MAINNET_RPC_URL \
  --private-key $DEPLOYER_PRIVATE_KEY
```

### 4. Approve tokenIn tokens on ArbitrageExecutor

```bash
# WETH
cast send <ArbitrageExecutor_proxy> \
  "setTokenApproval(address,bool)" 0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2 true \
  --rpc-url $MAINNET_RPC_URL --private-key $DEPLOYER_PRIVATE_KEY

# USDC
cast send <ArbitrageExecutor_proxy> \
  "setTokenApproval(address,bool)" 0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48 true \
  --rpc-url $MAINNET_RPC_URL --private-key $DEPLOYER_PRIVATE_KEY
```

### 5. Approve routers on ArbitrageExecutor

```bash
# Example: Uniswap V3 SwapRouter
cast send <ArbitrageExecutor_proxy> \
  "setRouterApproval(address,bool)" 0xE592427A0AEce92De3Edee1F18E0157C05861564 true \
  --rpc-url $MAINNET_RPC_URL --private-key $DEPLOYER_PRIVATE_KEY
```

### 6. Batch-grant allowances in AllowanceManager

```bash
# Grant AllowanceManager allowance so UniV3 router can pull WETH from it.
cast send <AllowanceManager_proxy> \
  "grantAllowance(address,address,uint256)" \
  0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2 \
  0xE592427A0AEce92De3Edee1F18E0157C05861564 \
  1000000000000000000000 \
  --rpc-url $MAINNET_RPC_URL --private-key $DEPLOYER_PRIVATE_KEY
```

### 7. (Optional) Set Aave referral code

```bash
cast send <FlashLoanExecutor_proxy> \
  "setReferralCode(uint16)" <code> \
  --rpc-url $MAINNET_RPC_URL --private-key $DEPLOYER_PRIVATE_KEY
```

### 8. Transfer admin to multisig

```bash
ADMIN_ROLE=0x0000000000000000000000000000000000000000000000000000000000000000
# For each contract:
cast send <proxy> "grantRole(bytes32,address)" $ADMIN_ROLE <multisig> \
  --rpc-url $MAINNET_RPC_URL --private-key $DEPLOYER_PRIVATE_KEY
cast send <proxy> "revokeRole(bytes32,address)" $ADMIN_ROLE <deployer> \
  --rpc-url $MAINNET_RPC_URL --private-key $DEPLOYER_PRIVATE_KEY
```

## Verification (post-broadcast)

If `--verify` missed any contract, verify manually:

```bash
forge verify-contract <impl_address> src/ArbitrageExecutor.sol:ArbitrageExecutor \
  --chain mainnet \
  --etherscan-api-key $ETHERSCAN_API_KEY
```

## Emergency procedures

### Pause ArbitrageExecutor (kills executeArbitrage)

```bash
cast send <ArbitrageExecutor_proxy> "pause()" \
  --rpc-url $MAINNET_RPC_URL --private-key $DEPLOYER_PRIVATE_KEY
```

### Pause AllowanceManager (kills grant/revoke operations)

```bash
cast send <AllowanceManager_proxy> "pause()" \
  --rpc-url $MAINNET_RPC_URL --private-key $DEPLOYER_PRIVATE_KEY
```

### Emergency withdraw stranded tokens

```bash
cast send <ArbitrageExecutor_proxy> \
  "emergencyWithdraw(address)" <token_address> \
  --rpc-url $MAINNET_RPC_URL --private-key $DEPLOYER_PRIVATE_KEY
```

### 9. (SC-10) Transfer ADMIN_ROLE to AdminTimelock

After steps 1-8, transfer DEFAULT_ADMIN_ROLE on all three contracts from the
deployer EOA to the AdminTimelock proxy.  From this point forward, every admin
operation must be scheduled through the timelock and waits 24h before it can
execute.

```bash
ADMIN_ROLE=0x0000000000000000000000000000000000000000000000000000000000000000
TIMELOCK=<AdminTimelock_proxy>   # printed by deploy script

# ArbitrageExecutor
cast send <ArbitrageExecutor_proxy> \
  "grantRole(bytes32,address)" $ADMIN_ROLE $TIMELOCK \
  --rpc-url $MAINNET_RPC_URL --private-key $DEPLOYER_PRIVATE_KEY
cast send <ArbitrageExecutor_proxy> \
  "revokeRole(bytes32,address)" $ADMIN_ROLE <deployer_eoa> \
  --rpc-url $MAINNET_RPC_URL --private-key $DEPLOYER_PRIVATE_KEY

# AllowanceManager (same two calls)
cast send <AllowanceManager_proxy> \
  "grantRole(bytes32,address)" $ADMIN_ROLE $TIMELOCK \
  --rpc-url $MAINNET_RPC_URL --private-key $DEPLOYER_PRIVATE_KEY
cast send <AllowanceManager_proxy> \
  "revokeRole(bytes32,address)" $ADMIN_ROLE <deployer_eoa> \
  --rpc-url $MAINNET_RPC_URL --private-key $DEPLOYER_PRIVATE_KEY

# FlashLoanExecutor (same two calls)
cast send <FlashLoanExecutor_proxy> \
  "grantRole(bytes32,address)" $ADMIN_ROLE $TIMELOCK \
  --rpc-url $MAINNET_RPC_URL --private-key $DEPLOYER_PRIVATE_KEY
cast send <FlashLoanExecutor_proxy> \
  "revokeRole(bytes32,address)" $ADMIN_ROLE <deployer_eoa> \
  --rpc-url $MAINNET_RPC_URL --private-key $DEPLOYER_PRIVATE_KEY
```

After this step, the deployer EOA can no longer call any admin function
directly.  To make any future admin change:

1. Schedule the operation via the timelock:

   ```bash
   # Example: re-approve a new router
   cast send $TIMELOCK \
     "schedule(address,uint256,bytes,bytes32,bytes32,uint256)" \
     <ArbitrageExecutor_proxy> 0 \
     $(cast calldata "setRouterApproval(address,bool)" <new_router> true) \
     0x0 0x0 86400 \
     --rpc-url $MAINNET_RPC_URL --private-key $PROPOSER_PRIVATE_KEY
   ```

2. Wait 24h (or the configured minDelay).

3. Execute after the delay has elapsed:

   ```bash
   cast send $TIMELOCK \
     "execute(address,uint256,bytes,bytes32,bytes32)" \
     <ArbitrageExecutor_proxy> 0 \
     $(cast calldata "setRouterApproval(address,bool)" <new_router> true) \
     0x0 0x0 \
     --rpc-url $MAINNET_RPC_URL --private-key $EXECUTOR_PRIVATE_KEY
   ```

To cancel a scheduled operation before it executes (e.g. if the key is compromised):

```bash
cast send $TIMELOCK \
  "cancel(bytes32)" <operation_id> \
  --rpc-url $MAINNET_RPC_URL --private-key $PROPOSER_PRIVATE_KEY
```

## Contract addresses reference

Fill in after deployment:

| Contract | Proxy | Implementation |
|----------|-------|----------------|
| ArbitrageExecutor | TBD | TBD |
| AllowanceManager | TBD | TBD |
| FlashLoanExecutor | TBD | TBD |
| AdminTimelock | TBD | TBD |
| Aave V3 Pool | `0x87870Bca3F3fD6335C3F4ce8392D69350B4fA4E2` | — |
