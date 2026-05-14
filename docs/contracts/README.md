# ArbitrageX-V2 (OMEGA) Smart Contracts

> Core execution engine for flashloan-powered atomic arbitrage across Uniswap V2/V3, Curve, and Balancer.

## Architecture

```
  Backend Rust (searcher-rs)
           |
           v  +-- signed Resolution payload
  +--------------------+
  |    Executor.sol    |  <-- Entry point
  |  (core/Executor)   |
  +----+----+----+-----+
       |    |    |
  +----v+ +-v---+ +-v----+
  |Flash | | Sig | | DEX  |
  |Loan  | | Val | | Adap |
  +-------+ +-----+ +------+
       |              |
  +----v------+  +----v------+
  | Aave V3   |  | UniV2     |
  | Balancer  |  | UniV3     |
  | MakerDAO  |  | Curve     |
  +-----------+  +-----------+
```

## Quick Start

### Prerequisites
- Foundry (`forge`)
- Solidity 0.8.19+

### Install Dependencies

```bash
cd /mnt/agents/output/contracts

# Initialize Foundry project (if needed)
forge init --force

# Install OpenZeppelin Contracts v4
forge install OpenZeppelin/openzeppelin-contracts@v4.9.3 --no-commit

# Install Forge-STD (if not present)
forge install foundry-rs/forge-std --no-commit
```

### Compile

```bash
forge build --optimizer-runs 200
```

### Run Tests

```bash
# Run all tests
forge test -vv

# Run with gas report
forge test --gas-report

# Run invariant tests only
forge test --match-path "test/Executor.invariants.t.sol" -vvv

# Run with fuzzing (CI profile)
FOUNDRY_PROFILE=ci forge test
```

### Deploy (Mainnet)

```bash
# Copy environment
cp .env.example .env
# Edit .env with your values

# Dry run
forge script Deploy --rpc-url $RPC_URL

# Deploy with CREATE2
forge script Deploy --rpc-url $RPC_URL --broadcast --verify

# Deploy Executor only (upgrades)
forge script Deploy --sig "deployExecutor()" --rpc-url $RPC_URL --broadcast

# Verify deployed state
forge script Deploy --sig "verifyExecutor(address)" \
  --rpc-url $RPC_URL <EXECUTOR_ADDRESS>
```

## Contract Inventory

| Contract | File | Lines | Purpose |
|---|---|---|---|
| `IExecutor` | `src/interfaces/IExecutor.sol` | 197 | Interface + events + structs |
| `Executor` | `src/core/Executor.sol` | 677 | Core execution engine |
| `UniswapV2Adapter` | `src/adapters/UniswapV2Adapter.sol` | 516 | Uniswap V2 / SushiSwap |
| `UniswapV3Adapter` | `src/adapters/UniswapV3Adapter.sol` | 619 | Uniswap V3 with callback |
| `FlashloanProvider` | `src/utils/FlashloanProvider.sol` | 382 | Aave V3 / Balancer / Maker |
| `SignatureValidator` | `src/utils/SignatureValidator.sol` | 199 | ECDSA recovery via Yul |
| `ExecutorInvariantTest` | `test/Executor.invariants.t.sol` | 740 | Foundry invariant + fuzz |
| `Deploy` | `script/Deploy.s.sol` | 445 | CREATE2 deterministic deploy |

## Gas Costs (Documented in NatSpec)

| Function | Gas Cost | Notes |
|---|---|---|
| `execute()` | 180k-350k | Base + 45k/hop + flashloan overhead |
| `validate()` | ~25k | Read-only, no state changes |
| `emergencyWithdraw()` | ~35k | Owner only |
| `recoverSigner()` | ~3k | Yul-optimized ecrecover via precompile 0x01 |
| `swap()` (V2) | 95k-110k | Per swap, depends on hop count |
| `swap()` (V3) | 115k-135k | Per swap, fee-tier dependent |
| `getReserves()` | ~5k | Yul inline assembly |

## Security Invariants

1. **Balance Monotonicity**: Contract balance of any token never decreases post-execution
2. **Authorization**: Only `authorizedSigner` or `owner()` can call `execute()`
3. **Yield Distribution**: 100% of captured yield is transferred to `treasury`
4. **Nonce Uniqueness**: Each nonce can be consumed at most once (replay protection)
5. **Atomic Revert**: If topological yield <= `minYield`, entire transaction reverts atomically
6. **Deadline Enforcement**: Block timestamp > deadline causes immediate revert
7. **ECDSA Malleability**: Signatures with `s > n/2` are rejected (SECP256K1_HALF_N check)

## Design Patterns

- **Checks-Effects-Interactions (CEI)**: Every function follows this strictly
- **ReentrancyGuard**: All state-mutating functions use `nonReentrant`
- **Yul Inline Assembly**: Calldata decoding, signature recovery, reserve queries
- **CREATE2**: Deterministic deployment addresses across chains
- **Zero Hardcoded Addresses**: All addresses via constructor or governance

## License

MIT
