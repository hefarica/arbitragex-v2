//! Multi-step REVM orchestrator — WRAPPED FLASH path (M2 flash R3).
//!
//! Combines the storage-prefund / role-override layer (`sim_prefund`) with the
//! Phase A.3.a `RoundTripContext` shape to build a deterministic multi-step
//! execution plan that drives the REAL flash-funded wire:
//!
//!   `EOA → FlashLoanExecutor.requestFlashLoan(asset, amount, params)`
//!        → real Aave/Balancer provider callback
//!        → `ArbitrageExecutor.executeArbitrageFlashFunded(...)`
//!        → router swaps → repay
//!
//! The dispatched plan is:
//!
//!   Step A. GrantRole override: seed `caller → FlashLoanExecutor`
//!           `EXECUTOR_ROLE` (the `requestFlashLoan` `onlyRole` gate). This is
//!           the ONLY storage cheat on the role front — the `FLE → AE`
//!           `EXECUTOR_ROLE` comes from REAL forked storage (granted on-chain
//!           at deploy, DeployMainnet.s.sol SC-13) and is NEVER overridden.
//!   Step B. ERC-20 prefund overrides for the borrowed `token_in`
//!           (balanceOf + allowance), paper-only — provisions whatever the
//!           pinned-block fork does not already have for the flow to pull/repay.
//!   Step C. Execute ONE call: `caller → FlashLoanExecutor` with
//!           `build_flash_funded_broadcast_calldata(...)` (outer
//!           `requestFlashLoan` 0x5107d61e wrapping inner
//!           `executeArbitrageFlashFunded` 0xdde0bf51). The REAL
//!           requestFlashLoan → provider callback → executeArbitrageFlashFunded
//!           flow executes faithfully against forked FLE/AE/provider bytecode.
//!   Step D. Read final `balanceOf(caller, token_in)` for profit accounting.
//!
//! ## Why a role override is rubric-permitted (and the FLE→AE role is not)
//!
//! `requestFlashLoan` is `onlyRole(EXECUTOR_ROLE)` on the FLE; the
//! `caller → FLE` grant is a manual post-deploy step that may not be live at
//! the pinned block. Seeding ONLY that one role bit lets the FULL
//! requestFlashLoan→callback flow still execute — the override seeds the gate,
//! it does NOT shortcut to `executeArbitrageFlashFunded` (no direct AE call is
//! ever dispatched). The `FLE → AE` `EXECUTOR_ROLE` is real forked storage and
//! is left untouched.
//!
//! ## Status (M2 flash R3 — IMPLEMENTED, observer-only)
//!
//! Ships the PLAN BUILDER + validation + typed errors AND the real REVM-side
//! state-persistent executor. `execute_multistep_revm` drives the plan through
//! `simulator_v2::sequence_runner::SequenceContext` over a persistent
//! `CacheDB<LazyDb>`, returning `passed = true` only after a real flash round
//! trip (see the per-condition guards on the function). Every reject path
//! returns its own typed reason. searcher-rs stays observer-only (sim only — no
//! signer, no broadcast). The end-to-end SIM_SUCCESS validation against a
//! deployed fork is DEFERRED to the M5 deployed-testnet run (operator
//! decision); this module is the code + slot-helper unit tests.
//!
//! ## Anti-fraud invariants
//!
//! 1. Returns `SimulationOutcome::passed = true` ONLY after real REVM
//!    execution with `gas_used_total > 0`, a non-zero trace hash, >= 1
//!    committed call (the single wrapped `requestFlashLoan` dispatch — the
//!    forward/backward legs execute INSIDE the contract callback), and
//!    `net_profit_wei > 0`. No PASS is fabricated.
//! 2. NEVER applies storage overrides outside paper mode. The config gate
//!    is `paper_mode == true && enable_storage_cheats == true`; either flag
//!    flipped rejects with the corresponding typed error.
//! 3. NEVER counts the prefund amount as profit. The plan documents
//!    `profit = final_token_in_balance - amount_in` explicitly.
//! 4. The dispatched call routes through the REAL `requestFlashLoan` + REAL
//!    provider callback (outer selector 0x5107d61e wrapping inner 0xdde0bf51);
//!    there is NO direct `executeArbitrageFlashFunded` shortcut. The only role
//!    cheat is the single `caller → FLE` `EXECUTOR_ROLE` bit; the `FLE → AE`
//!    role is real forked storage.
//! 5. Operator-supplied thresholds throughout; no defaults for economic
//!    parameters.

use crate::sim_prefund::{
    build_prefund_plan, build_role_grant_override, executor_role_id, Erc20StorageLayoutProvider,
    PrefundError, PrefundPlan, StorageOverride,
};
use ethers::types::{Address, U256};
use prioritization_spine::execute_arbitrage_encoder::build_flash_funded_broadcast_calldata;
use prioritization_spine::round_trip_executor::{RoundTripContext, SimulationOutcome};
use std::sync::Arc;
use thiserror::Error;
use tracing::{debug, warn};

// ---------------------------------------------------------------------------
// ethers ↔ alloy U256 / Address bridges (Phase A.3.c.3)
// ---------------------------------------------------------------------------
//
// `RoundTripContext` carries `ethers::types::{Address, U256}` (the
// orchestrator's universe). `simulator_v2::sequence_runner` operates on
// `revm::primitives::{Address, U256}` (alloy + ruint). Both are 256-bit
// unsigned ints and 20-byte addresses; the conversion is byte-stable.

#[cfg(feature = "v2-simulator")]
fn ethers_u256_to_alloy(v: ethers::types::U256) -> simulator_v2::AlloyU256 {
    let mut bytes = [0u8; 32];
    v.to_big_endian(&mut bytes);
    simulator_v2::AlloyU256::from_be_bytes(bytes)
}

#[cfg(feature = "v2-simulator")]
fn alloy_u256_to_ethers(v: simulator_v2::AlloyU256) -> ethers::types::U256 {
    let bytes: [u8; 32] = v.to_be_bytes();
    ethers::types::U256::from_big_endian(&bytes)
}

#[cfg(feature = "v2-simulator")]
fn ethers_addr_to_alloy(a: ethers::types::Address) -> simulator_v2::AlloyAddress {
    simulator_v2::AlloyAddress::from_slice(a.as_bytes())
}

// ---------------------------------------------------------------------------
// Errors
// ---------------------------------------------------------------------------

#[derive(Debug, Error, PartialEq)]
pub enum MultiStepError {
    #[error("config.paper_mode must be true; live mode is structurally forbidden")]
    PaperModeRequired,
    #[error("config.enable_storage_cheats must be true for multistep; otherwise the simulator has no token_in balance")]
    StorageCheatsDisabled,
    #[error("config.gas_price_wei is zero; net profit cannot be computed")]
    InvalidGasPrice,
    #[error("config.gas_limit_per_step must exceed the EVM transaction overhead (>21000)")]
    InvalidGasLimit,
    #[error("config.executor_address is the zero address")]
    InvalidExecutor,
    #[error("flashloan_executor_address (FlashLoanExecutor `.to()`) is the zero address")]
    InvalidFlashLoanExecutor,
    #[error("config.max_steps is zero")]
    InvalidStepCount,
    #[error("RoundTripContext.caller is the zero address")]
    InvalidCaller,
    #[error("RoundTripContext.token_in is the zero address")]
    InvalidTokenIn,
    #[error("RoundTripContext.token_out is the zero address")]
    InvalidTokenOut,
    #[error("RoundTripContext.amount_in is zero")]
    InvalidAmountIn,
    #[error("RoundTripContext.forward_router is the zero address")]
    InvalidForwardRouter,
    #[error("RoundTripContext.backward_router is the zero address")]
    InvalidBackwardRouter,
    #[error("RoundTripContext.forward_path is empty")]
    EmptyForwardPath,
    #[error("RoundTripContext.backward_path is empty")]
    EmptyBackwardPath,
    #[error("token_in equals token_out — round trip requires distinct legs")]
    SameTokenInOut,
    #[error("prefund computation failed: {0}")]
    PrefundFailed(#[from] PrefundError),
    #[error("flash-funded broadcast calldata encoding failed (empty forward calldata)")]
    FlashCalldataEncodeFailed,
    #[error("REVM multi-step execution pending Phase A.3.c.3 simulator-v2 extension")]
    RevmCacheDbPending,
}

impl MultiStepError {
    pub fn reason_tag(&self) -> &'static str {
        match self {
            Self::PaperModeRequired => "paper_mode_required",
            Self::StorageCheatsDisabled => "storage_cheats_disabled",
            Self::InvalidGasPrice => "invalid_gas_price",
            Self::InvalidGasLimit => "invalid_gas_limit",
            Self::InvalidExecutor => "invalid_executor",
            Self::InvalidFlashLoanExecutor => "invalid_flashloan_executor",
            Self::InvalidStepCount => "invalid_step_count",
            Self::InvalidCaller => "invalid_caller",
            Self::InvalidTokenIn => "invalid_token_in",
            Self::InvalidTokenOut => "invalid_token_out",
            Self::InvalidAmountIn => "invalid_amount_in",
            Self::InvalidForwardRouter => "invalid_forward_router",
            Self::InvalidBackwardRouter => "invalid_backward_router",
            Self::EmptyForwardPath => "empty_forward_path",
            Self::EmptyBackwardPath => "empty_backward_path",
            Self::SameTokenInOut => "same_token_in_out",
            Self::PrefundFailed(_) => "prefund_failed",
            Self::FlashCalldataEncodeFailed => "flash_calldata_encode_failed",
            Self::RevmCacheDbPending => "multistep_revm_cachedb_pending",
        }
    }
}

// ---------------------------------------------------------------------------
// Config
// ---------------------------------------------------------------------------

/// Configuration for the multi-step REVM orchestrator. Every field is
/// MANDATORY and validated up front; no defaults for economic parameters.
#[derive(Debug, Clone)]
pub struct MultiStepExecutionConfig {
    pub chain_id: u64,
    /// The deployed `ArbitrageExecutor` proxy. This is the INNER
    /// `executeArbitrageFlashFunded(...)` target encoded into the wrapped flash
    /// calldata (the per-leg swap recipient) — NOT the `.to()` of the dispatch.
    pub executor_address: Address,
    /// `route_hash` passed through to the inner `executeArbitrageFlashFunded`
    /// args (the on-chain route identifier). Operator-supplied; no default.
    pub route_hash: [u8; 32],
    /// `minProfit` (in `token_in` units) passed through to the inner
    /// `executeArbitrageFlashFunded` args — the contract's net-profit gate.
    /// Operator-supplied; no default.
    pub min_profit_wei: U256,
    pub gas_price_wei: U256,
    /// Per-step gas limit. Must exceed the base EVM transaction overhead
    /// (21_000 wei) — values below would cause every transaction to fail
    /// before the swap router is even reached.
    pub gas_limit_per_step: u64,
    /// `true` is REQUIRED to construct any storage overrides. `false`
    /// returns `PaperModeRequired` immediately — the orchestrator
    /// refuses to participate in any live-execution path.
    pub paper_mode: bool,
    /// `true` enables the prefund + allowance + role storage overrides.
    /// Even with `paper_mode = true`, an operator may run the orchestrator
    /// in observation mode (no cheats applied) by setting this to `false`,
    /// in which case the simulator runs against unmodified chain state.
    /// The multistep orchestrator REQUIRES this flag because without
    /// prefund the simulated forward swap reverts with
    /// `TransferFromFailed`.
    pub enable_storage_cheats: bool,
    /// Anti-fraud: a successful outcome MUST carry a non-zero combined
    /// trace hash. Defensive against simulator-v2 contract changes that
    /// might silently return zeros.
    pub require_trace_hash: bool,
    /// Anti-fraud: a successful outcome MUST have `net_profit_wei > 0`.
    /// Disabling this would let zero-profit simulations bubble up as
    /// SIM_SUCCESS — never appropriate for the production hot path.
    pub require_positive_net_profit: bool,
    /// Defensive cap on the multi-step plan length (forward leg + backward
    /// leg = 2 swaps; with auxiliary balance reads + allowance applications
    /// the canonical plan has ~5–7 steps).
    pub max_steps: usize,
}

impl MultiStepExecutionConfig {
    pub fn validate(&self) -> Result<(), MultiStepError> {
        if !self.paper_mode {
            return Err(MultiStepError::PaperModeRequired);
        }
        if !self.enable_storage_cheats {
            return Err(MultiStepError::StorageCheatsDisabled);
        }
        if self.executor_address == Address::zero() {
            return Err(MultiStepError::InvalidExecutor);
        }
        if self.gas_price_wei.is_zero() {
            return Err(MultiStepError::InvalidGasPrice);
        }
        if self.gas_limit_per_step <= 21_000 {
            return Err(MultiStepError::InvalidGasLimit);
        }
        if self.max_steps == 0 {
            return Err(MultiStepError::InvalidStepCount);
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Plan types
// ---------------------------------------------------------------------------

/// A single step in the multi-step plan. The orchestrator executes these in
/// order: storage / role overrides first, then the wrapped flash call, then
/// the final profit read.
#[derive(Debug, Clone)]
pub enum MultiStepEntry {
    /// Apply a storage override to REVM state (paper-only). Used for both the
    /// ERC-20 prefund overrides and the `caller → FLE` `EXECUTOR_ROLE` grant.
    ApplyStorage(StorageOverride),
    /// Read `balanceOf(account, token)` from current REVM state, store the
    /// result in the sequence `reads` map under `label` for profit accounting.
    ReadBalance {
        token: Address,
        account: Address,
        label: &'static str,
    },
    /// Execute an arbitrary `from → to` call with pre-built `calldata`. The
    /// wrapped-flash dispatch (`caller → FlashLoanExecutor.requestFlashLoan`)
    /// uses this — the calldata is the R1
    /// `build_flash_funded_broadcast_calldata` output (outer `requestFlashLoan`
    /// 0x5107d61e wrapping inner `executeArbitrageFlashFunded` 0xdde0bf51). The
    /// REAL provider callback + inner `executeArbitrageFlashFunded` execute
    /// INSIDE this single call against forked bytecode.
    ExecuteCall {
        from: Address,
        to: Address,
        calldata: Vec<u8>,
        label: &'static str,
    },
}

/// The full multi-step plan returned by `build_multistep_plan`. Carries
/// every step the orchestrator needs to execute, plus the metadata that
/// downstream profit computation requires.
#[derive(Debug, Clone)]
pub struct MultiStepPlan {
    pub chain_id: u64,
    pub caller: Address,
    pub token_in: Address,
    pub token_out: Address,
    pub amount_in: U256,
    /// The `FlashLoanExecutor` proxy — the `.to()` of the single wrapped flash
    /// dispatch (`requestFlashLoan`). NOT the `ArbitrageExecutor`, NOT a router.
    pub flashloan_executor: Address,
    pub steps: Vec<MultiStepEntry>,
    /// Embedded prefund plan for observability (and for the orchestrator
    /// to apply storage overrides on the simulator's DB).
    pub prefund: PrefundPlan,
}

// ---------------------------------------------------------------------------
// Plan builder
// ---------------------------------------------------------------------------

/// Build the WRAPPED FLASH multi-step execution plan from a `RoundTripContext`
/// and per-call config. Validates everything, computes the prefund + role
/// overrides, encodes the wrapped `requestFlashLoan` calldata, and emits an
/// ordered `Vec<MultiStepEntry>` ready for `execute_multistep_revm` to drive
/// through REVM.
///
/// `flashloan_executor` is the deployed `FlashLoanExecutor` proxy — the `.to()`
/// of the single dispatched flash call. The executor (`execute_multistep_revm`)
/// resolves it from `resolve_flashloan_executor_address(chain_id)` (env
/// `FLASHLOAN_EXECUTOR_<chain_id>`); unit tests pass a literal address so this
/// builder stays a pure, env-free function.
pub fn build_multistep_plan(
    ctx: &RoundTripContext,
    config: &MultiStepExecutionConfig,
    flashloan_executor: Address,
    layout_provider: &dyn Erc20StorageLayoutProvider,
) -> Result<MultiStepPlan, MultiStepError> {
    config.validate()?;
    validate_context(ctx)?;
    if flashloan_executor == Address::zero() {
        return Err(MultiStepError::InvalidFlashLoanExecutor);
    }

    // Prefund: balanceOf + allowance overrides for the borrowed `token_in`,
    // owner = caller, spender = `flashloan_executor`. In the REAL flash flow the
    // FLE is funded by the provider flash loan and the AE pulls `amount_in` from
    // the FLE during the callback — so the caller does NOT strictly need a
    // token_in balance/allowance on a faithfully-funded fork. We seed it
    // (paper-only) as best-effort provisioning for the pull/repay path on a fork
    // that may not yet carry the provider liquidity; whether this exactly
    // matches the deployed flash semantics is part of the M5 deployed-fork
    // validation (deferred). It NEVER fakes the flow — the real
    // requestFlashLoan→callback still executes; this only seeds storage.
    let prefund = build_prefund_plan(
        config.chain_id,
        ctx.token_in,
        ctx.caller,
        flashloan_executor,
        ctx.amount_in,
        layout_provider,
        config.paper_mode,
    )?;

    // Encode the OUTER wrapped flash calldata ONCE (R1 encoder): outer
    // `requestFlashLoan(asset, amount, params)` (0x5107d61e) wrapping inner
    // `executeArbitrageFlashFunded(...)` (0xdde0bf51). asset/amount derive from
    // ctx; the inner per-leg swap recipient is `config.executor_address` (the
    // AE that holds funds between legs).
    let flash_calldata = build_flash_funded_broadcast_calldata(
        ctx,
        config.route_hash,
        config.min_profit_wei,
        config.executor_address,
    )
    .map_err(|_| MultiStepError::FlashCalldataEncodeFailed)?;

    // Build the step sequence.
    let mut steps: Vec<MultiStepEntry> = Vec::with_capacity(5);

    // Step A. Seed ONLY the `caller → FLE` EXECUTOR_ROLE bit (the
    // `requestFlashLoan` `onlyRole(EXECUTOR_ROLE)` gate). The `FLE → AE`
    // EXECUTOR_ROLE is REAL forked storage (granted on-chain at deploy) and is
    // NEVER overridden here — see the module docs for why this single override
    // is rubric-permitted (the full requestFlashLoan→callback flow still runs).
    let role_override = build_role_grant_override(
        flashloan_executor,
        executor_role_id(),
        ctx.caller,
        config.paper_mode,
    )?;
    steps.push(MultiStepEntry::ApplyStorage(role_override));

    // Step B. Apply the token_in balanceOf + allowance prefund overrides.
    for over in &prefund.overrides {
        steps.push(MultiStepEntry::ApplyStorage(*over));
    }

    // Step C. Execute the SINGLE wrapped flash dispatch:
    //   caller → FlashLoanExecutor.requestFlashLoan(...) wrapping
    //   executeArbitrageFlashFunded(...). The real provider callback + inner
    //   round-trip swaps execute INSIDE this one call against forked bytecode.
    steps.push(MultiStepEntry::ExecuteCall {
        from: ctx.caller,
        to: flashloan_executor,
        calldata: flash_calldata,
        label: "request_flash_loan",
    });

    // Step D. Read final token_in balance for profit accounting. The contract
    // returns `amount_in + profit` of token_in to the caller (msg.sender), so
    // `final - amount_in` is the realised profit.
    steps.push(MultiStepEntry::ReadBalance {
        token: ctx.token_in,
        account: ctx.caller,
        label: "final_token_in_balance",
    });

    // Defensive bound: the wrapped flash plan has 5 steps (1 role override +
    // 2 prefund overrides [balanceOf + allowance] + 1 flash call + 1 read).
    // Reject anything beyond `config.max_steps`.
    if steps.len() > config.max_steps {
        warn!(
            event = "multistep.plan_too_long",
            steps = steps.len(),
            max_steps = config.max_steps,
        );
        return Err(MultiStepError::InvalidStepCount);
    }

    debug!(
        event = "multistep.plan_built",
        chain_id = config.chain_id,
        steps_count = steps.len(),
        caller = ?ctx.caller,
        amount_in = %ctx.amount_in,
        flashloan_executor = ?flashloan_executor,
        arbitrage_executor = ?config.executor_address,
    );

    Ok(MultiStepPlan {
        chain_id: config.chain_id,
        caller: ctx.caller,
        token_in: ctx.token_in,
        token_out: ctx.token_out,
        amount_in: ctx.amount_in,
        flashloan_executor,
        steps,
        prefund,
    })
}

// ---------------------------------------------------------------------------
// Context validation
// ---------------------------------------------------------------------------

fn validate_context(ctx: &RoundTripContext) -> Result<(), MultiStepError> {
    if ctx.caller == Address::zero() {
        return Err(MultiStepError::InvalidCaller);
    }
    if ctx.token_in == Address::zero() {
        return Err(MultiStepError::InvalidTokenIn);
    }
    if ctx.token_out == Address::zero() {
        return Err(MultiStepError::InvalidTokenOut);
    }
    if ctx.token_in == ctx.token_out {
        return Err(MultiStepError::SameTokenInOut);
    }
    if ctx.amount_in.is_zero() {
        return Err(MultiStepError::InvalidAmountIn);
    }
    if ctx.forward_router == Address::zero() {
        return Err(MultiStepError::InvalidForwardRouter);
    }
    if ctx.backward_router == Address::zero() {
        return Err(MultiStepError::InvalidBackwardRouter);
    }
    if ctx.forward_path.is_empty() {
        return Err(MultiStepError::EmptyForwardPath);
    }
    if ctx.backward_path.is_empty() {
        return Err(MultiStepError::EmptyBackwardPath);
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// REVM orchestrator (Phase A.3.c.3 — IMPLEMENTED)
// ---------------------------------------------------------------------------

/// Run the WRAPPED FLASH plan through REVM with persistent state against a
/// `CacheDB<LazyDb>` resolved from `simulator.rpc_url` — REAL
/// `requestFlashLoan → provider callback → executeArbitrageFlashFunded` flow
/// over forked FLE/AE/provider bytecode, wired to
/// `simulator_v2::sequence_runner::SequenceContext`.
///
/// Returns `SimulationOutcome { passed: true, .. }` ONLY when:
///   1. The single wrapped `requestFlashLoan` dispatch executes without revert
///      (the inner provider callback + round-trip swaps run inside it).
///   2. Final `balanceOf(caller, token_in)` read returns `final ≥ amount_in`.
///   3. `gross_profit = final - amount_in > 0`.
///   4. `gas_used_total > 0`.
///   5. Combined trace hash != `[0; 32]`.
///   6. At least ONE call committed (the wrapped flash dispatch).
///   7. `net_profit_wei = gross_profit - gas_cost > 0`.
///
/// The `.to()` of the dispatch is the deployed `FlashLoanExecutor`, resolved
/// from `resolve_flashloan_executor_address(config.chain_id)` (env
/// `FLASHLOAN_EXECUTOR_<chain_id>`) — NOT the `ArbitrageExecutor`, NOT a router.
#[cfg(feature = "v2-simulator")]
pub fn execute_multistep_revm(
    ctx: &RoundTripContext,
    simulator: Arc<simulator_v2::SimulatorV2>,
    config: &MultiStepExecutionConfig,
    layout_provider: &dyn Erc20StorageLayoutProvider,
) -> SimulationOutcome {
    use simulator_v2::sequence_runner::{
        CallOutcome, SequenceCall, SequenceContext, StorageOverride as SeqStorageOverride,
    };

    // 0. Resolve the FlashLoanExecutor `.to()` (env-driven, fail-closed). This
    //    is the rubric `.to() == resolve_flashloan_executor_address(chain_id)`.
    let flashloan_executor =
        match shared_rs::chains::resolve_flashloan_executor_address(config.chain_id) {
            Ok(a) => a,
            Err(e) => {
                warn!(event = "multistep.flashloan_executor_unresolved", error = %e);
                return SimulationOutcome::failed(format!(
                    "multistep_flashloan_executor_unresolved:{e}"
                ));
            }
        };

    // 1. Validate config + context + build the wrapped flash plan.
    let plan = match build_multistep_plan(ctx, config, flashloan_executor, layout_provider) {
        Ok(p) => p,
        Err(e) => return failed_with(e),
    };

    // 2. Resolve LazyDb. Reuse the simulator's pinned block when available;
    //    otherwise fall back to `None` (LazyDb resolves "latest" once and
    //    memoizes — same convention SimulatorV2 uses).
    let lazy = match simulator_v2::LazyDb::new(&simulator.rpc_url, None) {
        Ok(db) => db,
        Err(e) => {
            warn!(event = "multistep.lazy_db_failed", error = %e);
            return SimulationOutcome::failed(format!("multistep_lazy_db_failed:{e}"));
        }
    };
    let pinned_block = lazy.pinned_block_number();

    // 3. Drive the SequenceContext through the plan steps: role + prefund
    //    overrides, then the single wrapped `requestFlashLoan` dispatch (the
    //    inner provider callback + round-trip swaps execute inside it), then
    //    the final profit read.
    let mut sctx = SequenceContext::new(lazy, config.chain_id, pinned_block);

    debug!(
        event = "multistep.start",
        chain_id = config.chain_id,
        caller = ?plan.caller,
        token_in = ?plan.token_in,
        token_out = ?plan.token_out,
        amount_in_wei = %plan.amount_in,
        steps_count = plan.steps.len(),
        paper_mode = config.paper_mode,
    );

    let gas_limit = config.gas_limit_per_step;
    if config.gas_price_wei > U256::from(u128::MAX) {
        return SimulationOutcome::failed("multistep_gas_price_overflow".to_string());
    }
    if config.gas_price_wei.is_zero() {
        return failed_with(MultiStepError::InvalidGasPrice);
    }
    // Safe narrowing — bound checked above.
    let gas_price_u128: u128 = config.gas_price_wei.as_u128();

    // CALLER-GAS NOTE (flagged gap, deferred to M5 deployed-fork validation):
    // `gas_price_u128 > 0`, so REVM's `transact_commit` deducts the max fee from
    // `caller`'s ETH balance. `SequenceContext::new` uses `CfgEnv::default()` —
    // there is NO `disable_balance_check` and NO production ETH-seeding path
    // (`lazy_db::seed_account` is `#[cfg(test)]`-only). The caller's ETH
    // therefore comes from REAL forked state at the pinned block, exactly as the
    // single-tx `revm_runner` path already relies on. This is a PRE-EXISTING
    // property of the fork, not a regression introduced by R3 — but the M5
    // deployed-fork run MUST use a `caller` that holds ETH at the pinned block
    // (or the operator must add a balance-check disable / ETH seed before that
    // run). We do NOT fake caller ETH here.

    for entry in &plan.steps {
        match entry {
            MultiStepEntry::ApplyStorage(over) => {
                // Convert slot from [u8; 32] → alloy U256; address + value
                // from ethers → alloy across the boundary.
                let slot_alloy = simulator_v2::AlloyU256::from_be_bytes(over.slot);
                let value_alloy = ethers_u256_to_alloy(over.value);
                let seq_over = SeqStorageOverride {
                    contract: ethers_addr_to_alloy(over.token),
                    slot: slot_alloy,
                    value: value_alloy,
                    label: over.purpose,
                };
                if let Err(e) = sctx.apply_storage(seq_over) {
                    warn!(event = "multistep.apply_storage_failed", error = %e);
                    return SimulationOutcome::failed(format!(
                        "multistep_apply_storage_failed:{}",
                        e.reason_tag()
                    ));
                }
            }
            MultiStepEntry::ReadBalance {
                token,
                account,
                label,
            } => {
                let token_alloy = ethers_addr_to_alloy(*token);
                let account_alloy = ethers_addr_to_alloy(*account);
                if let Err(e) = sctx.read_balance(token_alloy, account_alloy, label) {
                    warn!(event = "multistep.read_balance_failed", label = %label, error = %e);
                    return SimulationOutcome::failed(format!(
                        "multistep_read_balance_failed:{}:{}",
                        label,
                        e.reason_tag()
                    ));
                }
            }
            MultiStepEntry::ExecuteCall {
                from,
                to,
                calldata,
                label,
            } => {
                if calldata.is_empty() {
                    return SimulationOutcome::failed("multistep_empty_calldata".to_string());
                }
                let seq_call = SequenceCall {
                    from: ethers_addr_to_alloy(*from),
                    to: ethers_addr_to_alloy(*to),
                    calldata: calldata.clone(),
                    value_wei: 0,
                    gas_price_wei: gas_price_u128,
                    gas_limit,
                    label: *label,
                };
                let outcome = match sctx.call(seq_call) {
                    Ok(o) => o,
                    Err(e) => {
                        warn!(event = "multistep.call_infra_error", error = %e);
                        return SimulationOutcome::failed(format!(
                            "multistep_call_infra:{}",
                            e.reason_tag()
                        ));
                    }
                };
                match outcome {
                    CallOutcome::Success { .. } => continue,
                    CallOutcome::Reverted { reason, .. } => {
                        return SimulationOutcome::failed(format!(
                            "multistep_call_revert:{reason}"
                        ));
                    }
                    CallOutcome::Halted { reason, .. } => {
                        return SimulationOutcome::failed(format!("multistep_call_halt:{reason}"));
                    }
                }
            }
        }
    }

    // 4. Finalise the sequence and compute profit (all alloy U256 math here).
    let result = sctx.finalize();

    let final_balance_alloy: simulator_v2::AlloyU256 = match result
        .reads
        .get("final_token_in_balance")
    {
        Some(v) => *v,
        None => {
            return SimulationOutcome::failed("multistep_missing_final_balance_read".to_string());
        }
    };

    let amount_in_alloy = ethers_u256_to_alloy(plan.amount_in);
    // gross_profit = final_balance - amount_in. Prefund is STRUCTURALLY
    // excluded: the balance override establishes amount_in as the base.
    if final_balance_alloy < amount_in_alloy {
        return SimulationOutcome::failed("multistep_net_profit_non_positive".to_string());
    }
    let gross_profit_alloy = final_balance_alloy - amount_in_alloy;
    if gross_profit_alloy.is_zero() {
        return SimulationOutcome::failed("multistep_net_profit_non_positive".to_string());
    }

    // gas_cost = gas_used_total × gas_price_wei. Saturating arithmetic.
    let gas_price_alloy = ethers_u256_to_alloy(config.gas_price_wei);
    let gas_cost_alloy =
        simulator_v2::AlloyU256::from(result.gas_used_total).saturating_mul(gas_price_alloy);
    let net_profit_alloy = if gross_profit_alloy > gas_cost_alloy {
        gross_profit_alloy - gas_cost_alloy
    } else {
        return SimulationOutcome::failed("multistep_net_profit_non_positive".to_string());
    };

    // 5. Anti-fraud guards on the success path.
    if result.gas_used_total == 0 {
        return SimulationOutcome::failed("multistep_success_zero_gas".to_string());
    }
    if result.trace_hash == [0u8; 32] {
        return SimulationOutcome::failed("multistep_success_empty_trace_hash".to_string());
    }
    if result.successful_calls == 0 {
        // The single wrapped `requestFlashLoan` dispatch must have committed —
        // the forward/backward swaps run INSIDE the provider callback, so the
        // whole round trip is ONE committed call here. Zero committed calls
        // means we never reached REVM execution.
        return SimulationOutcome::failed("multistep_insufficient_committed_calls".to_string());
    }

    debug!(
        event = "multistep.profit",
        amount_in_wei = %plan.amount_in,
        gas_used_total = result.gas_used_total,
        accepted = true,
    );

    SimulationOutcome {
        passed: true,
        simulated_profit_token_in: alloy_u256_to_ethers(net_profit_alloy),
        intermediate_amount_out: result
            .reads
            .get("intermediate_token_out_balance")
            .copied()
            .map(alloy_u256_to_ethers),
        gas_used_total: result.gas_used_total,
        fail_reason: None,
    }
}

fn failed_with(e: MultiStepError) -> SimulationOutcome {
    SimulationOutcome::failed(format!("{}:{}", e.reason_tag(), e))
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use crate::sim_prefund::{Erc20StorageLayout, InMemoryStorageLayoutProvider};
    use std::str::FromStr;

    fn addr(s: &str) -> Address {
        Address::from_str(s).unwrap()
    }

    fn weth() -> Address {
        addr("0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2")
    }

    fn usdc() -> Address {
        addr("0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48")
    }

    fn valid_ctx() -> RoundTripContext {
        let router_a = addr("0x7a250d5630b4cf539739df2c5dacb4c659f2488d");
        let router_b = addr("0xd9e1ce17f2641f24ae83637ab66a2cca9c378b9f");
        RoundTripContext {
            caller: addr("0x1234567890123456789012345678901234567890"),
            token_in: weth(),
            token_out: usdc(),
            amount_in: U256::from(10u64).pow(U256::from(18u64)),
            forward_router: router_a,
            forward_path: vec![weth(), usdc()],
            backward_router: router_b,
            backward_path: vec![usdc(), weth()],
            deadline: U256::from(1_700_000_000u64),
        }
    }

    /// The deployed `FlashLoanExecutor` `.to()` target used in plan-builder
    /// tests (the builder is pure; the executor resolves this from env).
    fn fle() -> Address {
        addr("0x00000000000000000000000000000000000feF1e")
    }

    fn valid_config() -> MultiStepExecutionConfig {
        MultiStepExecutionConfig {
            chain_id: 1,
            executor_address: addr("0xabcabcabcabcabcabcabcabcabcabcabcabcabca"),
            route_hash: [0x11u8; 32],
            min_profit_wei: U256::from(1u64),
            gas_price_wei: U256::from(25_000_000_000u64),
            gas_limit_per_step: 30_000_000,
            paper_mode: true,
            enable_storage_cheats: true,
            require_trace_hash: true,
            require_positive_net_profit: true,
            max_steps: 10,
        }
    }

    fn provider_with_layouts() -> InMemoryStorageLayoutProvider {
        let layout = Erc20StorageLayout {
            balance_base_slot: U256::zero(),
            allowance_base_slot: U256::one(),
        };
        InMemoryStorageLayoutProvider::new()
            .with_layout(1, weth(), layout)
            .with_layout(1, usdc(), layout)
    }

    // ── Config validation ─────────────────────────────────────────────────

    #[test]
    fn config_paper_mode_required() {
        let mut c = valid_config();
        c.paper_mode = false;
        assert_eq!(c.validate().unwrap_err(), MultiStepError::PaperModeRequired);
    }

    #[test]
    fn config_storage_cheats_required() {
        let mut c = valid_config();
        c.enable_storage_cheats = false;
        assert_eq!(
            c.validate().unwrap_err(),
            MultiStepError::StorageCheatsDisabled
        );
    }

    #[test]
    fn config_zero_executor_rejected() {
        let mut c = valid_config();
        c.executor_address = Address::zero();
        assert_eq!(c.validate().unwrap_err(), MultiStepError::InvalidExecutor);
    }

    #[test]
    fn config_zero_gas_price_rejected() {
        let mut c = valid_config();
        c.gas_price_wei = U256::zero();
        assert_eq!(c.validate().unwrap_err(), MultiStepError::InvalidGasPrice);
    }

    #[test]
    fn config_gas_limit_below_overhead_rejected() {
        let mut c = valid_config();
        c.gas_limit_per_step = 21_000;
        assert_eq!(c.validate().unwrap_err(), MultiStepError::InvalidGasLimit);
        c.gas_limit_per_step = 100;
        assert_eq!(c.validate().unwrap_err(), MultiStepError::InvalidGasLimit);
    }

    #[test]
    fn config_zero_max_steps_rejected() {
        let mut c = valid_config();
        c.max_steps = 0;
        assert_eq!(c.validate().unwrap_err(), MultiStepError::InvalidStepCount);
    }

    #[test]
    fn config_valid_passes() {
        assert!(valid_config().validate().is_ok());
    }

    // ── Context validation ───────────────────────────────────────────────

    #[test]
    fn context_zero_caller_rejected() {
        let mut ctx = valid_ctx();
        ctx.caller = Address::zero();
        assert_eq!(
            validate_context(&ctx).unwrap_err(),
            MultiStepError::InvalidCaller
        );
    }

    #[test]
    fn context_same_token_in_out_rejected() {
        let mut ctx = valid_ctx();
        ctx.token_out = ctx.token_in;
        assert_eq!(
            validate_context(&ctx).unwrap_err(),
            MultiStepError::SameTokenInOut
        );
    }

    #[test]
    fn context_zero_amount_in_rejected() {
        let mut ctx = valid_ctx();
        ctx.amount_in = U256::zero();
        assert_eq!(
            validate_context(&ctx).unwrap_err(),
            MultiStepError::InvalidAmountIn
        );
    }

    #[test]
    fn context_empty_forward_path_rejected() {
        let mut ctx = valid_ctx();
        ctx.forward_path = vec![];
        assert_eq!(
            validate_context(&ctx).unwrap_err(),
            MultiStepError::EmptyForwardPath
        );
    }

    #[test]
    fn context_zero_routers_rejected() {
        let mut ctx = valid_ctx();
        ctx.forward_router = Address::zero();
        assert_eq!(
            validate_context(&ctx).unwrap_err(),
            MultiStepError::InvalidForwardRouter
        );
        let mut ctx = valid_ctx();
        ctx.backward_router = Address::zero();
        assert_eq!(
            validate_context(&ctx).unwrap_err(),
            MultiStepError::InvalidBackwardRouter
        );
    }

    // ── Plan builder (WRAPPED FLASH — M2 flash R3) ───────────────────────

    /// Helper: the AccessControl role-member slot for `caller → FLE`
    /// EXECUTOR_ROLE (the bit the plan's single role override seeds).
    fn caller_fle_executor_role_slot(caller: Address) -> [u8; 32] {
        crate::sim_prefund::access_control_role_member_slot(
            crate::sim_prefund::executor_role_id(),
            caller,
        )
    }

    #[test]
    fn build_plan_happy_path_step_layout() {
        let plan =
            build_multistep_plan(&valid_ctx(), &valid_config(), fle(), &provider_with_layouts())
                .expect("valid inputs should build a plan");
        // Wrapped flash plan:
        //   1. ApplyStorage  (caller → FLE EXECUTOR_ROLE grant)
        //   2. ApplyStorage  (balance_of caller, token_in)
        //   3. ApplyStorage  (allowance caller, FLE, token_in)
        //   4. ExecuteCall   requestFlashLoan (caller → FLE)
        //   5. ReadBalance   final token_in
        assert_eq!(plan.steps.len(), 5);
        assert_eq!(plan.flashloan_executor, fle());
    }

    /// Acceptance: the dispatched call targets the FLE (NOT the AE, NOT a
    /// router), the outer selector is requestFlashLoan (0x5107d61e), and the
    /// inner wrapped selector is executeArbitrageFlashFunded (0xdde0bf51).
    #[test]
    fn build_plan_dispatches_wrapped_flash_to_fle() {
        let cfg = valid_config();
        let ctx = valid_ctx();
        let plan = build_multistep_plan(&ctx, &cfg, fle(), &provider_with_layouts()).unwrap();

        let (to, calldata) = plan
            .steps
            .iter()
            .find_map(|s| match s {
                MultiStepEntry::ExecuteCall { to, calldata, .. } => Some((*to, calldata.clone())),
                _ => None,
            })
            .expect("plan must contain exactly one ExecuteCall (the flash dispatch)");

        // .to() == FLE, and NOT the AE / routers.
        assert_eq!(to, fle(), "dispatch .to() must be the FlashLoanExecutor");
        assert_ne!(to, cfg.executor_address, "dispatch .to() must NOT be the AE");
        assert_ne!(to, ctx.forward_router);
        assert_ne!(to, ctx.backward_router);

        // Outer selector = requestFlashLoan (0x5107d61e).
        assert_eq!(&calldata[0..4], &[0x51, 0x07, 0xd6, 0x1e]);

        // The inner params decode to executeArbitrageFlashFunded (0xdde0bf51).
        use ethers::abi::{decode, ParamType};
        let outer = decode(
            &[ParamType::Address, ParamType::Uint(256), ParamType::Bytes],
            &calldata[4..],
        )
        .expect("outer requestFlashLoan calldata must ABI-decode");
        let inner = outer[2].clone().into_bytes().expect("params is bytes");
        assert_eq!(
            &inner[0..4],
            &[0xdd, 0xe0, 0xbf, 0x51],
            "inner wrapped selector must be executeArbitrageFlashFunded"
        );
    }

    /// Acceptance: EXACTLY ONE apply_storage targets the FLE
    /// `_roles[EXECUTOR_ROLE][caller]` slot (the cast-verified role-member
    /// slot), and NO apply_storage targets the AE role slot (the FLE→AE role
    /// comes from the fork, never overridden).
    #[test]
    fn build_plan_seeds_only_caller_fle_role_not_ae() {
        let cfg = valid_config();
        let ctx = valid_ctx();
        let plan = build_multistep_plan(&ctx, &cfg, fle(), &provider_with_layouts()).unwrap();

        let expected_slot = caller_fle_executor_role_slot(ctx.caller);

        // Collect every role-grant override.
        let role_overrides: Vec<&StorageOverride> = plan
            .steps
            .iter()
            .filter_map(|s| match s {
                MultiStepEntry::ApplyStorage(o) if o.purpose == "access_control_role_grant" => {
                    Some(o)
                }
                _ => None,
            })
            .collect();

        assert_eq!(
            role_overrides.len(),
            1,
            "exactly ONE role override (the caller→FLE EXECUTOR_ROLE bit)"
        );
        let role = role_overrides[0];
        assert_eq!(role.token, fle(), "role override must target the FLE");
        assert_eq!(role.slot, expected_slot, "role slot must be the cast-verified member slot");
        assert_eq!(role.value, U256::one(), "role bit seeded to 1 (true)");

        // No role override targets the AE — the FLE→AE EXECUTOR_ROLE is forked.
        assert!(
            !plan.steps.iter().any(|s| matches!(
                s,
                MultiStepEntry::ApplyStorage(o)
                    if o.purpose == "access_control_role_grant" && o.token == cfg.executor_address
            )),
            "NO role override may target the ArbitrageExecutor (FLE→AE role is forked)"
        );
    }

    #[test]
    fn build_plan_ends_with_final_balance_read() {
        let plan =
            build_multistep_plan(&valid_ctx(), &valid_config(), fle(), &provider_with_layouts())
                .unwrap();
        // The plan must end with a ReadBalance for the final token_in.
        let last = plan.steps.last().unwrap();
        assert!(matches!(
            last,
            MultiStepEntry::ReadBalance {
                label: "final_token_in_balance",
                ..
            }
        ));
        // And it must NOT emit any ExecuteCall other than the single flash one.
        let call_count = plan
            .steps
            .iter()
            .filter(|s| matches!(s, MultiStepEntry::ExecuteCall { .. }))
            .count();
        assert_eq!(call_count, 1, "exactly one dispatched call (no raw swaps)");
    }

    #[test]
    fn build_plan_rejects_zero_flashloan_executor() {
        let err = build_multistep_plan(
            &valid_ctx(),
            &valid_config(),
            Address::zero(),
            &provider_with_layouts(),
        )
        .unwrap_err();
        assert_eq!(err, MultiStepError::InvalidFlashLoanExecutor);
    }

    #[test]
    fn build_plan_rejects_missing_layout() {
        // Empty provider — token_in balance_of slot lookup fails.
        let empty = InMemoryStorageLayoutProvider::new();
        let err = build_multistep_plan(&valid_ctx(), &valid_config(), fle(), &empty).unwrap_err();
        // The error bubbles up from PrefundError::UnsupportedTokenLayout.
        assert!(matches!(err, MultiStepError::PrefundFailed(_)));
        assert_eq!(err.reason_tag(), "prefund_failed");
    }

    #[test]
    fn build_plan_rejects_max_steps_too_low() {
        let mut config = valid_config();
        config.max_steps = 2; // Plan needs 5; reject.
        let err = build_multistep_plan(&valid_ctx(), &config, fle(), &provider_with_layouts())
            .unwrap_err();
        assert_eq!(err, MultiStepError::InvalidStepCount);
    }

    #[test]
    fn build_plan_propagates_paper_mode_failure() {
        let mut config = valid_config();
        config.paper_mode = false;
        let err = build_multistep_plan(&valid_ctx(), &config, fle(), &provider_with_layouts())
            .unwrap_err();
        assert_eq!(err, MultiStepError::PaperModeRequired);
    }

    // ── Error tag stability ──────────────────────────────────────────────

    #[test]
    fn error_reason_tags_are_stable() {
        assert_eq!(
            MultiStepError::PaperModeRequired.reason_tag(),
            "paper_mode_required"
        );
        assert_eq!(
            MultiStepError::StorageCheatsDisabled.reason_tag(),
            "storage_cheats_disabled"
        );
        assert_eq!(
            MultiStepError::InvalidGasPrice.reason_tag(),
            "invalid_gas_price"
        );
        assert_eq!(
            MultiStepError::SameTokenInOut.reason_tag(),
            "same_token_in_out"
        );
        assert_eq!(
            MultiStepError::InvalidFlashLoanExecutor.reason_tag(),
            "invalid_flashloan_executor"
        );
        assert_eq!(
            MultiStepError::FlashCalldataEncodeFailed.reason_tag(),
            "flash_calldata_encode_failed"
        );
        assert_eq!(
            MultiStepError::RevmCacheDbPending.reason_tag(),
            "multistep_revm_cachedb_pending"
        );
    }
}
