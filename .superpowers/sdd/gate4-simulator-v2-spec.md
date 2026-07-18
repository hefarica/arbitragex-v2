# Gate 4 — Completar Simulator-v2: Blueprint / Spec

> Mission: replace the stub simulator with a real, fork-backed, differential REVM simulator that validates calldata, sender, value, gas, balances, approvals, and state before any shadow or live broadcast.
>
> Identity: IA OMEGA — topological terminology is used throughout. `Holonomic Loop Resolution` = multi-pool atomic round-trip; `Temporal Liquidity Superposition (TLS)` = flash-funded capital; `Topological Yield` = gross retained spread; `Decoherencia de Estado` = slippage; `Variedad de Liquidez` = liquidity pool/DEX.
>
> Constraints: read-only / shadow / paper only. No live flips, no capital exposure, no broadcast. This document is the specification the implementation agents must follow.

---

## 1. Executive summary

`ARBX_SIMULATOR_V2_READY=false` is the current structural blocker. `sim-ctl` is healthy, but G-SIM-1 Layer 2 is red because the real simulator-v2 has not been fully wired and proven against a deployed fork. This blueprint defines the minimal, surgical set of changes required to turn G-SIM-1 green based on a real smoke test.

What already exists (do not rebuild):

- `simulator-v2` crate: `LazyDb` (RPC-backed `revm::Database` with cache), `revm_runner` (single-tx REVM executor), `sequence_runner` (multi-step persistent `CacheDB<LazyDb>` context), `SimulatorV2` wrapper with block memoization.
- `sim-core` crate: `sim_encoder` (OpportunityCandidate → RoundTripContext), `sim_multistep::execute_multistep_revm` (wrapped-flash REVM orchestrator), `sim_prefund` (AccessControl role override helpers).
- `sim-ctl` crate: `/simulate` handler, backend selection (`SIM_BACKEND=revm`), real-sim dispatcher in `sim_runner.rs`, `/fork-status` health.
- `relays-client`: `bundle_builder` broadcasts the exact `wrapped_calldata` from a `ValidatedPlan` (sim↔broadcast byte-parity already designed).
- `api-server`: G-SIM-1 verifier reads `ARBX_SIMULATOR_V2_READY` and Prometheus `arbx_simulation_total`.

What is missing (this blueprint covers):

1. A differential validation layer inside `simulator-v2` that compares local REVM output against an RPC `eth_call` on the same pinned block.
2. Explicit pre-flight state validation (gas limit, sender balance, nonce, token approvals, calldata prefix, value, block freshness).
3. A smoke-test harness that proves the wrapped-flash path against a real deployed contract on a fork (Sepolia or mainnet pinned block), producing `passed=true` with real gas and a non-zero retained spread.
4. Removal of the fail-closed `route_encoding_not_available` guard in `sim-ctl/src/revm_backend.rs` only after the real calldata path is proven; until then the guard stays.
5. The flip of `ARBX_SIMULATOR_V2_READY` happens only by the operator after the smoke test passes; no code change flips it.

---

## 2. Files audited

| File | Purpose | Current state |
|------|---------|---------------|
| `backend/sim-ctl/src/main.rs` | Axum server, backend selection, real-sim wiring | Healthy; backend selection OK; A3 PG lookup stub exists |
| `backend/sim-ctl/src/sim_runner.rs` | `run_real_simulation`: candidate → encoder → `execute_multistep_revm` | Implemented (PR #271); runs real REVM sim when env present |
| `backend/sim-ctl/src/revm_backend.rs` | Legacy `RevmBackend::simulate` wrapper; empty-calldata fail-closed guard | Guard still active (`route_encoding_not_available`) |
| `backend/sim-ctl/src/sim_engine.rs` | Anvil-backed `SimEngine::simulate` (legacy S4) | Preserved; not on critical path for v2 |
| `backend/sim-ctl/src/anvil_backend.rs` | Adapter to `SimEngine` | Preserved |
| `backend/sim-ctl/src/simulator_backend.rs` | Trait `SimulatorBackend` | Preserved |
| `backend/simulator-v2/src/lib.rs` | `SimulatorV2`, `Simulator` trait, `CandidateInput`, `SimResult`, `SimError` | Implemented; block memoization present |
| `backend/simulator-v2/src/lazy_db.rs` | RPC-backed `revm::Database` + `DatabaseRef` with cache | Implemented; good test coverage |
| `backend/simulator-v2/src/revm_runner.rs` | Single-tx REVM executor | Implemented; revert/halt/profit/gas OK |
| `backend/simulator-v2/src/sequence_runner.rs` | Multi-step `SequenceContext` over `CacheDB<LazyDb>` | Implemented; balance/amounts-out reads OK |
| `backend/simulator-v2/src/bellman_ford.rs` | Negative-cycle graph detector | Implemented; not on critical v2 path |
| `backend/sim-core/src/sim_multistep.rs` | Wrapped-flash multi-step orchestrator | Implemented; requires deployed fork validation |
| `backend/sim-core/src/sim_encoder.rs` | `OpportunityCandidate → RoundTripContext` | Implemented; V2-only in Phase A.3.a |
| `backend/prioritization-spine/src/round_trip_executor.rs` | `SimulationOutcome`, `RoundTripContext`, `compute_profit_usd`, `net_usd_viable` | Implemented; net-USD gate defined |
| `backend/relays-client/src/bundle_builder.rs` | Verbatim broadcast of `wrapped_calldata` | Implemented; selector guard present |
| `backend/relays-client/src/submit_engine.rs` | Pre-execute checklist + broadcast orchestration | Implemented; BE-05 callBundle re-sim present |
| `backend/api-server/src/readiness/verifiers/g-sim-1.ts` | G-SIM-1 readiness verifier | Red while `ARBX_SIMULATOR_V2_READY=false` |
| `docs/gsim1-simulator-v2-ready-flip-checklist.md` | Operator flip checklist | Current; must be followed |

---

## 3. Architecture: Simulator-v2

### 3.1 Fork mechanism

Two complementary modes:

1. **In-memory REVM fork (`LazyDb`)** — the hot path. `LazyDb` pins to a block number at construction time (`BlockId::Number`). All state reads go through `ethers::Provider<Http>` with the pinned block. `CacheDB<LazyDb>` persists mutations across sequence steps. Block number is resolved once and memoized in `SimulatorV2.block_number` (`OnceLock`).
2. **Anvil fork (existing `SimEngine`)** — the legacy S4 path. Kept for backward compatibility and independent smoke tests, but not required for v2.

For differential validation, the same pinned block is used for both the local REVM run and the RPC `eth_call` comparison. The block must be no older than `SIM_MAX_BLOCK_AGE_SECONDS` (env, default 60 s) at simulation start; otherwise the simulation returns `block_too_stale`.

### 3.2 Calldata path

```text
OpportunityCandidate (shared_rs)
        |
        v
 sim_core::sim_encoder::build_round_trip_context_from_candidate
        |
        v
 RoundTripContext (prioritization_spine)
        |
        v
 sim_core::sim_multistep::execute_multistep_revm
        |
        +--> SequenceContext::read_amounts_out  (forward quote, non-committing)
        +--> build_multistep_plan               (encode wrapped flash calldata)
        +--> apply_storage                      (caller → FLE EXECUTOR_ROLE grant)
        +--> read_balance                       (FLE token_in pre)
        +--> call                               (caller → FlashLoanExecutor.requestFlashLoan)
        +--> read_balance                       (FLE token_in post)
        |
        v
 SimulationOutcome { passed, gas_used_total, gas_price_wei,
                     simulated_profit_token_in, wrapped_calldata }
        |
        v
 sim-ctl /simulate response
        |
        v
 searcher-rs persists ValidatedPlan { ctx, route_hash, min_profit_wei,
                                          executor_address, wrapped_calldata }
        |
        v
 relays-client bundle_builder broadcasts wrapped_calldata VERBATIM
```

Key invariant: the bytes in `SimulationOutcome.wrapped_calldata` are the exact bytes that may later be broadcast. Any re-encode is forbidden (already enforced in `bundle_builder.rs`).

### 3.3 Differential validation: local REVM vs RPC `eth_call`

For the wrapped-flash call only, after local REVM succeeds, the simulator optionally runs the same calldata through RPC `eth_call` at the same pinned block and compares:

| Field | Local REVM | RPC `eth_call` | Tolerance / rule |
|-------|------------|----------------|------------------|
| Success/revert | `ExecutionResult::Success` | no revert | Must agree; any RPC revert → fail-closed |
| Return data bytes | `output.data()` | `eth_call` return bytes | Must be byte-identical up to first 4 KiB (large outputs truncated for comparison log only) |
| Gas used | `gas_used` local | `eth_estimateGas` (same block) | Two-sided relative tolerance: `|rpc_gas - local_gas| / local_gas ≤ 0.25`, and `rpc_gas ≤ SIM_GAS_LIMIT_PER_STEP`. Gas comparison arithmetic is overflow-safe because both values are bounded by `SIM_GAS_LIMIT_PER_STEP ≤ 30_000_000`. |

The differential check is **optional** in the hot path for latency, but **mandatory** for the smoke test that flips `ARBX_SIMULATOR_V2_READY`. Env knob: `SIM_DIFFERENTIAL_CHECK=true|false` (default `false` in hot path, `true` in smoke test).

A mismatch does NOT silently fall back to RPC; it returns `differential_mismatch:<reason>` and `passed=false`.

### 3.4 State validation checks (new module)

Create `backend/simulator-v2/src/preflight.rs` with a pure, env-free function:

```rust
pub fn preflight_check(
    candidate: &CandidateInput,
    db: &impl DatabaseRef,
    config: &PreflightConfig,
) -> Result<(), PreflightError>
```

Checks, in order, fail-closed:

1. `calldata` is non-empty and starts with a known selector (for the wrapped-flash path: `0x5107d61e` `requestFlashLoan`).
2. `from` is not zero address.
3. `to` is not zero address and resolves to a contract with non-empty code (real fork) or is explicitly allowed as an EOA-only target.
4. `value_wei` does not exceed `config.max_value_wei`.
5. Sender native balance ≥ `value_wei + gas_limit × gas_price_wei` (read from `db.basic(from).balance`).
6. Sender nonce matches `candidate.nonce` if `candidate.nonce` is `Some`.
7. For ERC-20 transfers inside the calldata: token `allowance(owner=from, spender=to)` ≥ amount. This is a best-effort static decode of the inner `executeArbitrageFlashFunded` params; if decode fails, return `approval_decode_failed` (fail-closed).
8. Block freshness (time-based, chain-aware): read the pinned block's `timestamp` and ensure `now - timestamp ≤ SIM_MAX_BLOCK_AGE_SECONDS`. The default is 60 s. For chains with different block times, the operator may override via env. Block-number drift (`max_block_drift`) is derived from this age bound and the configured chain block time; it is not an independent check.

`PreflightError` carries stable `reason_tag()` strings for metrics and logs.

#### Numerical safety properties (must be preserved by implementation)

- **U256 → i128 balance delta** (`revm_runner::balance_delta`): must saturate at `i128::MAX/MIN`; never panic or wrap.
- **Gas cost multiplication** (`round_trip_executor::compute_profit_usd`): must use `saturating_mul`; overflow yields `U256::MAX` → `f64::INFINITY` → `net_usd_viable=false` (fail-closed).
- **f64 precision**: `compute_profit_usd` is lossy past ~15 significant digits. For 18-decimal tokens this produces sub-cent absolute error; document this as an accepted precision bound.
- **Gas differential overflow**: precluded by the `SIM_GAS_LIMIT_PER_STEP` ceiling.

### 3.5 Revert reason capture

`revm_runner::decode_revert_reason` already handles standard `Error(string)` (selector `0x08c379a0`). Extend it to also decode:

- Panic codes (`0x4e487b71` + code).
- Custom errors with known 4-byte selectors from the ArbitrageExecutor / FlashLoanExecutor ABI (defined in a new `backend/simulator-v2/src/revert_dictionary.rs`).
- Raw hex fallback for everything else.

The revert reason must propagate through `SimulationOutcome.fail_reason` and `sim-ctl` JSON unchanged.

### 3.6 Fuzzing corpus (stress tests)

Create `backend/simulator-v2/tests/fuzz_corpus.rs` (integration tests, `#[ignore]` by default, run manually against a fork):

- Adversarial token: fee-on-transfer, rebasing, pausable, blacklisted sender.
- Adversarial DEX: V2 router with no liquidity, V3 with wrong fee tier, Curve pool with `A` ramp mid-change.
- State drift: run sim at block N, then at block N+1 where a large unrelated swap moved reserves; assert `trace_hash` differs.
- Gas extremes: base fee × 5, gas limit at ceiling.
- Revert classes: `INSUFFICIENT_OUTPUT_AMOUNT`, `FL_RepaymentShortfall`, `AccessControlUnauthorizedAccount`, custom panic.

Corpus is parameter-driven from JSON fixtures in `backend/simulator-v2/tests/fixtures/`; no hardcoded mainnet addresses.

### 3.7 Integration with `PostResolutionTopology`

The simulator-v2 output is not a `BundlePosition<T>` directly; it feeds the spine's `ValidatedPlan`. However, for doctrinal alignment the spec requires:

- 2-leg V2 round-trips map to `DiracImpulseOnly` topological intent as a **placeholder**. `DiracImpulseOnly` currently requires an `OptimalControlSolution` proof with a hyperbolic constraint; a 2-pool cycle is not literally a single CPMM impulse. This mapping is acceptable for Gate 4 because simulator-v2 does **not** construct `BundlePosition<T>`; it only produces the `ValidatedPlan`. Before the spine constructs a `BundlePosition`, either a new proof type must be added to `DiracImpulseOnly` or a dedicated 2-cycle topology must be proposed per `mev-ethics.md §Amendments`.
- 3+-leg triangular cycles map to `HolonomicLoopResolution` when the spine eventually supports them.
- Cross-venue hedge shapes map to `OrthogonalEquilibrium`.

For Gate 4 we only need to ensure the simulator-v2 output fields (`wrapped_calldata`, `route_hash`, `simulated_profit_token_in`, `gas_used_total`) are sufficient for the spine to construct the appropriate `BundlePosition<T>` later. No change to `sed-core` types is required now.

---

## 4. Files to modify

| File | Change | Rationale |
|------|--------|-----------|
| `backend/simulator-v2/src/lib.rs` | Add `pub mod preflight; pub mod revert_dictionary; pub mod differential;` | New modules |
| `backend/simulator-v2/src/revm_runner.rs` | Call `preflight_check` before `transact`; wire revert dictionary; include `output_bytes` in `trace_hash` (already done) | Validate input + richer revert reasons |
| `backend/simulator-v2/src/sequence_runner.rs` | Add `preflight_check` at `SequenceContext::call` entry (lightweight: nonce/balance/allowance); add `block_number()` accessor | Multi-step state validation |
| `backend/simulator-v2/src/lazy_db.rs` | Expose `latest_block_number()` and `provider()` read-only accessors for differential check | Differential RPC call needs provider |
| `backend/sim-core/src/sim_multistep.rs` | Add optional differential RPC check after local success; env-gated (`SIM_DIFFERENTIAL_CHECK`); log `differential_mismatch` fail-closed | Prove local REVM matches RPC |
| `backend/sim-core/src/sim_encoder.rs` | Add V3 fee-tier support behind a new `dex_adapters` label convention (`uniswap-v3:<fee>`) or reject with clear tag; out of Gate 4 scope unless trivial | Expand sim coverage later |
| `backend/sim-ctl/src/revm_backend.rs` | Keep empty-calldata guard until smoke test passes; then remove guard and wire real candidate calldata path | G-SIM-1 fail-closed discipline |
| `backend/sim-ctl/src/sim_runner.rs` | Add block-freshness read; pass `block_number` into `SimulatorV2::with_block`; log `preflight`/`differential` reason tags | Production hygiene |
| `backend/sim-ctl/src/main.rs` | Add env vars to startup log: `SIM_DIFFERENTIAL_CHECK`, `SIM_MAX_BLOCK_AGE_SECONDS`, chain block-time config | Observability |
| `backend/relays-client/src/bundle_builder.rs` | No functional change; add unit test proving `wrapped_calldata` from a passing sim equals broadcast bytes (already present) | Confirm sim↔broadcast parity |
| `backend/api-server/src/readiness/verifiers/g-sim-1.ts` | No code change; operator flips env flag after smoke test | Gate remains honest |

---

## 5. Files to create

| File | Purpose |
|------|---------|
| `backend/simulator-v2/src/preflight.rs` | `PreflightConfig`, `PreflightError`, `preflight_check` (pure, env-free) |
| `backend/simulator-v2/src/revert_dictionary.rs` | Known revert selectors for ArbitrageExecutor/FlashLoanExecutor; decode helper |
| `backend/simulator-v2/src/differential.rs` | `DifferentialCheck`, `eth_call` + `eth_estimateGas` comparison against local result |
| `backend/simulator-v2/tests/fork_smoke_test.rs` | Integration smoke test against real deployed contracts on a fork; `#[ignore]` default |
| `backend/simulator-v2/tests/fuzz_corpus.rs` | Adversarial corpus tests; `#[ignore]` default |
| `backend/simulator-v2/tests/fixtures/smoke_sepolia.json` | Fixture template for smoke test (addresses, selectors, amounts) |
| `backend/simulator-v2/tests/fixtures/fuzz_corpus.json` | Fuzz corpus parameter list |
| `docs/operations/SIMULATOR_V2_SMOKE_TEST.md` | Operator-run smoke-test runbook (follows `gsim1-simulator-v2-ready-flip-checklist.md`) |

---

## 6. TDD test plan

### 6.1 Tests to write before implementation

1. **Preflight unit tests** (`backend/simulator-v2/src/preflight.rs` under `#[cfg(test)]`):
   - `empty_calldata_rejected`
   - `zero_from_rejected`
   - `zero_to_rejected`
   - `insufficient_balance_rejected`
   - `nonce_mismatch_rejected`
   - `value_exceeds_cap_rejected`
   - `approval_insufficient_rejected`
   - `all_checks_pass`

2. **Revert dictionary unit tests**:
   - `decodes_error_string`
   - `decodes_panic_code`
   - `decodes_known_arbitrageexecutor_revert`
   - `raw_hex_fallback`

3. **Differential unit tests** (mock provider, no RPC):
   - `matching_success_returns_ok`
   - `rpc_revert_returns_mismatch`
   - `gas_estimate_too_high_returns_mismatch`
   - `return_data_diff_returns_mismatch`

4. **Sequence runner preflight tests**:
   - `call_with_zero_from_rejected_preflight`
   - `call_with_insufficient_allowance_rejected_preflight`

### 6.2 Tests to write during implementation

1. **Fork consistency** (`fork_smoke_test.rs`):
   - Start `anvil --fork-url <RPC> --fork-block-number <N>`.
   - Run `eth_blockNumber` and assert fork reports `N`.
   - Run `eth_getBalance` of a known address at `N` and assert it matches a reference value.
   - Run two identical simulations at `N` and assert same `trace_hash`.
   - Run at `N` and `N+1` and assert different `trace_hash`.

2. **Calldata encoding correctness**:
   - Build `RoundTripContext` from a fixture candidate.
   - Encode via `build_flash_funded_broadcast_calldata_with_intermediate`.
   - Assert outer selector = `0x5107d61e` and inner selector = `0xdde0bf51`.
   - Assert a re-encode with a different intermediate produces different bytes.

3. **Revert capture and propagation**:
   - Craft a candidate whose wrapped flash calldata triggers `FL_RepaymentShortfall` (or a known revert) on the fork.
   - Run sim; assert `passed=false` and `fail_reason` contains the human-readable revert text.

### 6.3 Tests to write after implementation

1. **End-to-end smoke test**:
   - Start sim-ctl with `SIM_BACKEND=revm`, `REVM_RPC_URL`, `ARBITRAGE_EXECUTOR`, `FLASHLOAN_EXECUTOR_1`.
   - POST `/simulate` with an enriched `OpportunityCandidate`.
   - Assert response `passed=true`, `gas_used_total > 0`, `wrapped_calldata` starts with `0x5107d61e`.
   - Assert Prometheus `arbx_simulation_total{simulator="revm",passed="true"}` incremented.

2. **Sim↔broadcast parity test**:
   - Take `wrapped_calldata` from smoke-test response.
   - Run `bundle_builder::verbatim_broadcast_calldata` with a `ValidatedPlan` carrying those bytes.
   - Assert returned bytes equal the sim bytes.

3. **G-SIM-1 verifier test**:
   - With `ARBX_SIMULATOR_V2_READY=true` and a recent `arbx_simulation_total` sample, assert `verifyGSIM1` returns green.
   - With the flag `false`, assert red reason mentions `ARBX_SIMULATOR_V2_READY=false`.

---

## 7. External dependencies and blockers

| Dependency | Risk | Mitigation |
|------------|------|------------|
| `revm` version (workspace pinned) | Low — already compiles and passes unit tests | Pin in workspace; upgrade only in a separate PR |
| Anvil fork stability | Medium — fork may drift or RPC may rate-limit | Use pinned block; `LazyDb` timeout 5 s; fallback to fail-closed |
| Contract ABI availability | Medium — need deployed ArbitrageExecutor + FlashLoanExecutor addresses and known revert selectors | Use Foundry build artifacts in `contracts/out/`; if ABIs missing, decode reverts via raw hex fallback |
| Real RPC endpoint (Sepolia/mainnet) | Medium — test relies on external RPC | Run smoke test on VPS where `REVM_RPC_URL` is configured; skip by default with `#[ignore]` |
| Token approvals on fork | Medium — FLE must have approved routers, or sim must apply allowance overrides | Current wrapped-flash path funds FLE via flash loan; no caller allowance needed. If AE approves routers internally, no override required. |
| `FLASHLOAN_EXECUTOR_1` env var | Low — already fail-closed in `resolve_flashloan_executor_address` | Document in `.env.example` |

---

## 8. Estimated implementation effort

| Work stream | Size | Notes |
|-------------|------|-------|
| `preflight.rs` + unit tests | S | Pure module, no external I/O |
| `revert_dictionary.rs` + tests | S | Selector table + decode wrapper |
| `differential.rs` + mock tests | M | Needs `ethers::Provider` mock or local RPC; careful gas comparison |
| Wire preflight into `revm_runner` and `sequence_runner` | S-M | Small, surgical |
| Wire differential into `sim_multistep` | M | Must preserve performance when disabled; fail-closed when enabled |
| Fork smoke test + fixtures | M-L | Requires real RPC and deployed contracts; most of the Gate 4 evidence lives here |
| Fuzz corpus + fixtures | M | Parameter-driven; can be deferred post-flip but should exist before flip for confidence |
| Operator runbook `SIMULATOR_V2_SMOKE_TEST.md` | S | Document exact commands and expected outputs |
| CI wiring (optional) | S | Mark fork tests `#[ignore]`; run in scheduled job with `FOR_RPC_URL` secret |
| **Total** | **M-L** | Roughly 2–3 focused implementation PRs + 1 smoke-test evidence PR |

---

## 9. Success criteria for Gate 4

1. `cargo test -p simulator-v2` and `cargo test -p sim-core` pass (local unit tests).
2. `cargo check -p sim-ctl --features default` passes.
3. A manually run smoke test against a real fork returns `passed=true` with real gas and a non-zero `wrapped_calldata` for at least one opportunity candidate.
4. Differential check (when enabled) agrees between local REVM and RPC `eth_call` for the smoke-test case.
5. `bundle_builder` unit test proves the broadcast bytes equal the sim bytes.
6. Operator updates VPS `.env` with `ARBX_SIMULATOR_V2_READY=true` only after (1)–(5) pass.
7. G-SIM-1 turns green in the readiness panel after the env flip and a recent `arbx_simulation_total` sample.
8. `submit_engine.rs` pre-execute checklist invokes `net_usd_viable` with the `SimulationOutcome` fields (`simulated_profit_token_in`, `gas_used_total`, `gas_price_wei`) before any broadcast path is reached.

---

## 10. Ordered next commits (suggested)

1. `feat(simulator-v2): preflight state validation module + tests`
2. `feat(simulator-v2): revert dictionary for executor/flash-loan errors`
3. `feat(sim-core): optional differential eth_call validation against pinned block`
4. `feat(sim-ctl): wire preflight/differential logs and block freshness check`
5. `test(simulator-v2): fork smoke test against deployed Sepolia contracts`
6. `docs(ops): SIMULATOR_V2_SMOKE_TEST runbook`
7. *(operator)* `env: ARBX_SIMULATOR_V2_READY=true` after evidence review.

---

*End of Gate 4 Simulator-v2 Blueprint. No implementation code is contained here; this is the specification for the implementation agents.*
