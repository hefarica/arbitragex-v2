# REVM Real Implementation — Design + Sprint Roadmap

> **Date**: 2026-05-05
> **Scope**: Replace the simulator stub (`prioritization-spine::simulator`) with
> a fully functional REVM-based round-trip arbitrage validator.
> **Status**: Phase 1 (foundation utility) lands with this commit. Phases 2-5
> are independent sub-tasks targetable in separate sessions.
> **Tracked in**: `anti_reincidencia.md` Incidente #6 sub-tarea (a) — open
> since 2026-05-03.

---

## Problem statement

The current `EvmSimulator::simulate_candidate` (prioritization-spine/src/simulator.rs:24)
sets up an EVM environment with **empty calldata** targeting a dummy address
(`0x22...22`) and returns `"PASS"` whenever the empty transaction succeeds —
which is always, because executing 0 bytes of bytecode is a successful no-op.

```rust
// simulator.rs:38-44 — current state
evm.env.tx.caller = caller;             // 0x11...11 (dummy)
evm.env.tx.transact_to = TransactTo::Call(target);  // 0x22...22 (dummy)
evm.env.tx.value = U256::ZERO;
evm.env.tx.data = Bytes::new();          // ← EMPTY CALLDATA
let result = evm.transact();             // → always Success
```

**Consequence**: `expected_profit_usd` produced by the spine is the
**spread upper bound** between two pool quotes for the same forward swap —
NOT the round-trip arbitrage profit (forward leg + backward leg + gas + fees).
A real arbitrage requires:

1. EOA holds X token_in
2. Approve router for X token_in
3. Swap X token_in → Y token_out at pool A (best price for that direction)
4. Approve router for Y token_out
5. Swap Y token_out → X' token_in at pool B (best price for return)
6. Profit = X' - X (or negative if pools are aligned)

The spread `hi - lo` ignores steps 4-6 and conflates "forward output difference"
with "round-trip realisable profit". Real profit is always smaller (second leg
also has fees + slippage + gas).

---

## Existing assets (already shipped)

✅ **`lazy_db.rs` (138 LOC)**: A working `revm::Database` impl that lazily
fetches account state from RPC and caches in memory. Implements:
- `basic(address)` — balance, nonce, code via `getBalance` / `getTransactionCount` / `getCode`
- `storage(address, slot)` — via `getStorageAt`
- `code_by_hash(hash)` — cache lookup (warns if miss; relies on `basic` to populate)
- `block_hash(n)` — returns `B256::ZERO` (placeholder, not yet wired)

✅ **revm 3.5.0** in Cargo workspace
✅ **ethers-rs 2** for RPC (NOTE: doctrine §16 says migrate to alloy 0.9 — orthogonal task)

❌ **What's missing**: real swap calldata, EOA pre-funding via storage overrides,
post-execution balance reading, round-trip profit calc.

---

## Sprint roadmap (5 phases, ~1-2 weeks total)

### ✅ Phase 1 — Foundation utility (THIS COMMIT, ~2h)

**Deliverable**: `prioritization-spine/src/swap_encoder.rs`

A pure ABI-encoding utility that produces calldata for Uniswap V2 swap functions.
No RPC calls, no EVM execution, no async — fully testable with `ethers::abi`
in unit tests. Single source of truth for "what bytes do I send to the router
for this swap?".

Functions:

```rust
pub fn encode_v2_swap_exact_tokens_for_tokens(
    amount_in: U256,
    amount_out_min: U256,
    path: &[Address],
    to: Address,
    deadline: U256,
) -> Bytes;

pub fn encode_v2_swap_exact_eth_for_tokens(
    amount_out_min: U256,
    path: &[Address],
    to: Address,
    deadline: U256,
) -> Bytes;

pub fn encode_erc20_approve(spender: Address, amount: U256) -> Bytes;
pub fn encode_erc20_balance_of(account: Address) -> Bytes;
pub fn encode_erc20_transfer(to: Address, amount: U256) -> Bytes;
```

5+ TDD tests covering: standard cases, multi-hop paths, edge cases (zero amount,
empty path, max u256), function selectors verified against Etherscan.

### ⏸ Phase 2 — V3 swap encoder (~3-4h, separate session)

Same shape as Phase 1 but for `IUniswapV3SwapRouter::exactInputSingle`,
`exactInput`, `exactOutputSingle`, `exactOutput`. Path encoding for V3 is
non-trivial (alternating address+fee bytes); needs careful tests.

### ⏸ Phase 3 — Storage override helpers (~2-3h)

When simulating, the dummy EOA `caller` (currently `0x11...11`) has zero
token balance. To execute a swap, we need to make `IERC20.balanceOf(caller)`
return the trade size. Two approaches:

1. **Storage override**: compute the storage slot for `balances[caller]`
   in each ERC20 (slot 0/1/9 depending on contract — varies). Set the slot
   via `LazyRpcDatabase::storage` cache override BEFORE executing.

2. **Direct deposit transaction**: simulate calling `transfer` from a known
   whale (Binance hot wallet, USDC issuer). Avoids per-token slot research
   but requires a "whale registry".

Recommend approach 1 with a hardcoded slot table for top 20 tokens (extensible
via operator config), fallback to approach 2 for unknown tokens.

### ⏸ Phase 4 — Round-trip executor (~3-5h)

The orchestration layer. Given an `OpportunityCandidate`:

1. Pick best forward pool (existing scanner output)
2. Pick best backward pool (NEW: scan pool indexes for return-leg routes)
3. Encode forward swap calldata via Phase 1/2 utilities
4. Encode backward swap calldata
5. Pre-fund caller via Phase 3 storage override
6. Execute forward swap in REVM (using existing LazyRpcDatabase)
7. Read intermediate token balance
8. Execute backward swap with intermediate balance
9. Read final token balance
10. Compute `actual_profit_usd = (final - initial) × token_in_price`
11. Return `SimulationResult { passed, simulated_profit_usd, gas_used, ... }`

This replaces `simulate_candidate` returning "PASS" with a real verdict.

### ⏸ Phase 5 — Integration tests with mainnet fork (~2-3h)

`forge test --fork-url` style integration tests using a real Ethereum mainnet
RPC. Pin a known historic block where an arbitrage existed; run the simulator;
assert it produces realistic profit. Catches regressions where the encoder
produces incorrect calldata that nonetheless executes successfully.

Requires: an `RPC_HTTP_FORK` env var with a mainnet endpoint, dedicated
CI job (or local-only marker like `#[ignore]` until CI is wired).

---

## Architectural decisions

### Why keep the existing `lazy_db.rs` instead of rewriting?
It works. The TODO at scanner.rs:350 was about V2 orientation (now fixed in
commit 289d5ee), not about the database layer. `lazy_db.rs` correctly fetches
state via RPC and caches. The block_hash placeholder is fine for current
needs (REVM only calls `block_hash` when executing `BLOCKHASH` opcode, rare
in DEX swap calldata).

### Why split encoder from executor?
- **Testability**: pure encoding has no IO and trivial unit tests. Executor
  needs RPC and is harder to test.
- **Reusability**: scanner.rs may want to encode swap calldata for different
  purposes (e.g., bundle construction for relays-client) without going
  through the simulator.
- **Independent rollout**: encoder can ship with N tests and zero deploy
  risk; executor lands later in its own commit.

### Why ethers-rs 2 instead of alloy 0.9?
The existing stack (lazy_db, scanner, prioritization-spine) is on ethers-rs 2.
Migrating to alloy 0.9 is doctrine §16 but orthogonal to REVM real. Mixing
the two crates causes conversion overhead but works. Phase 1 uses ethers's
ABI encoder (well-tested, documented). When alloy migration lands, the
encoder migrates with it (~30 min of mechanical changes).

### Why approach 1 (storage override) for Phase 3?
- Faster execution (no extra transaction in simulation)
- More deterministic (no whale-balance-dependence)
- Cost: hardcoded slot table for ~20 tokens. Acceptable for the curated
  allowlist; extensible.

---

## Acceptance criteria for the FULL implementation (post-Phase 5)

The day all phases land:

1. `simulator.simulate_candidate(candidate)` returns
   `SimulationResult { passed: bool, simulated_profit_usd: f64, gas_used: u64 }`
   instead of `String` placeholder.

2. The reported `simulated_profit_usd` matches what would actually realise
   on-chain within ±5% (slippage + gas estimation tolerance). Validated
   via mainnet fork test.

3. `prioritization-spine::config_aware::evaluate` consumes the new
   `SimulationResult` and rejects candidates where the REVM simulation
   reverts OR returns simulated_profit_usd ≤ min_profit_usd.

4. Heartbeat counters add a new `simulator_passed` and `simulator_reverted`
   metric set, giving the operator a per-minute view of sim outcomes.

5. anti_reincidencia.md Incidente #6 sub-tarea (a) marked DONE with the
   commit hash.

---

## Risk assessment

| Risk | Likelihood | Mitigation |
|------|-----------|------------|
| Encoder bug → calldata revert | Medium | TDD coverage; Etherscan-verified selectors |
| Storage slot lookup wrong → balance fetch fails | Medium | Hardcoded table for top tokens with manual verification; whale fallback for unknowns |
| RPC throttling under sim load | High | LazyRpcDatabase caches aggressively; pin to a single block_id per simulation |
| revm 3.5.0 API drift (we're on older revm vs current 19.0) | Low | revm 3 is stable; upgrade to 19.0 is its own task (doctrine §16 alloy migration) |
| Round-trip executor produces unrealistic profits | High | Phase 5 mainnet fork tests gate-keep merge |

---

## Sub-task tracking for next sessions

After this commit lands, the operator can pick any of the 4 remaining phases
independently. Recommended order:

1. **Phase 2** (V3 encoder) — same shape as Phase 1, fast win
2. **Phase 3** (storage overrides) — required by Phase 4
3. **Phase 4** (round-trip executor) — the actual replacement of the stub
4. **Phase 5** (fork tests) — quality gate before declaring "done"

Each phase is self-contained; partial completion (e.g., Phase 2 done, Phase 3 not)
does NOT regress production — the simulator stub continues returning "PASS"
until Phase 4 lands and replaces it atomically.
