# Smart Contracts (`contracts/`)

This directory contains the Solidity smart contracts for the ArbitrageX V2
trading system. Contracts are deployed across supported EVM chains.

## Contracts

```
contracts/
  src/
    ArbitrageExecutor.sol   # Main execution entry point
    FlashLoanProvider.sol   # Flash loan adapter
    DEXRouter.sol           # DEX aggregator router
    OpportunityValidator.sol # Pre-flight opportunity checks
    libraries/
      SafeMath.sol          # Overflow-safe math helpers
      PathLib.sol           # DEX path encoding/decoding
  test/
    ArbitrageExecutor.t.sol # Foundry test suite
    FlashLoanProvider.t.sol
    DEXRouter.t.sol
  script/
    Deploy.s.sol            # Deployment script
  foundry.toml
```

## Supported Chains

| Chain | Chain ID | Deploy Status |
|-------|----------|---------------|
| Ethereum | 1 | Production |
| BSC | 56 | Production |
| Arbitrum | 42161 | Production |
| Base | 8453 | Production |

## Build & Test

```bash
# Install dependencies
forge install

# Compile
forge build

# Run tests
forge test

# Coverage
forge coverage

# Gas snapshot
forge snapshot
```

## Deployment

```bash
# Deploy to a specific chain
forge script script/Deploy.s.sol \
  --rpc-url $RPC_URL \
  --private-key $DEPLOYER_KEY \
  --broadcast \
  --verify
```

## Security

- All contracts are audited before mainnet deployment.
- Use [Kill-Switch runbook](../runbooks/kill-switch.md) in emergencies.
- Upgradeable contracts use OpenZeppelin UUPS proxy pattern.

## Conventions

- Solidity 0.8.20+
- Contracts use `I` prefix for interfaces.
- Events are emitted for all state changes.
- Custom errors replace `require` strings.