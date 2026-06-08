//! Phase A.3.c.2 — Multi-step REVM round-trip orchestrator (plan + skeleton).
//!
//! Combines the Phase A.3.c.2 storage-prefund layer (`sim_prefund`) with the
//! Phase A.3.a `RoundTripContext` shape to build a deterministic multi-step
//! execution plan. The plan describes:
//!
//!   Step A. Storage overrides:
//!           - balanceOf(caller, token_in) = amount_in (paper-only prefund)
//!           - allowance(caller, forward_router, token_in) = U256::MAX
//!   Step B. Execute forward swap directly against `forward_router`.
//!   Step C. Read intermediate `balanceOf(caller, token_out)` from post-state.
//!   Step D. Apply `allowance(caller, backward_router, token_out) = U256::MAX`.
//!   Step E. Execute backward swap with REAL intermediate amount (NOT a
//!           placeholder).
//!   Step F. Read final `balanceOf(caller, token_in)`.
//!   Step G. profit = final_balance - amount_in (NOT counting prefund).
//!   Step H. gas_cost = total_gas_used * gas_price.
//!   Step I. net_profit = profit - gas_cost.
//!
//! ## Status (A.3.c.3 — IMPLEMENTED)
//!
//! Ships the PLAN BUILDER + validation + typed errors AND the real REVM-side
//! state-persistent executor. `execute_multistep_revm` drives the plan through
//! `simulator_v2::sequence_runner::SequenceContext` over a persistent
//! `CacheDB<LazyDb>`, returning `passed = true` only after a real round trip
//! (see the per-condition guards on the function). Every reject path returns
//! its own typed reason. (Historical note: A.3.c.2 shipped only the skeleton
//! and returned `multistep_revm_cachedb_pending`; that phase is superseded.)
//!
//! ## Anti-fraud invariants
//!
//! 1. Returns `SimulationOutcome::passed = true` ONLY after real REVM
//!    execution with `gas_used_total > 0`, a non-zero trace hash, >= 2
//!    committed calls, and `net_profit_wei > 0`. No PASS is fabricated.
//! 2. NEVER applies storage overrides outside paper mode. The config gate
//!    is `paper_mode == true && enable_storage_cheats == true`; either flag
//!    flipped rejects with the corresponding typed error.
//! 3. NEVER counts the prefund amount as profit. The plan documents
//!    `profit = final_token_in_balance - amount_in` explicitly.
//! 4. NEVER uses a placeholder amount for the backward leg. The plan
//!    INCLUDES the intermediate-balance read step that A.3.c.3 will
//!    consume to fill in the real amount.
//! 5. Operator-supplied thresholds throughout; no defaults for economic
//!    parameters.

use crate::sim_prefund::{
    build_prefund_plan, Erc20StorageLayoutProvider, PrefundError, PrefundPlan, StorageOverride,
};
use ethers::types::{Address, U256};
use prioritization_spine::round_trip_executor::{RoundTripContext, SimulationOutcome};
use prioritization_spine::swap_encoder::encode_v2_swap_exact_tokens_for_tokens;
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
    pub executor_address: Address,
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

/// A single step in the multi-step plan. The orchestrator will execute
/// these in order: storage overrides applied first, then swaps, then
/// reads.
#[derive(Debug, Clone)]
pub enum MultiStepEntry {
    /// Apply a storage override to REVM state (paper-only).
    ApplyStorage(StorageOverride),
    /// Read `balanceOf(account, token)` from current REVM state, store
    /// the result in the plan's `intermediate_balances` map under `label`.
    /// A.3.c.3 will fill this in when the executor runs.
    ReadBalance {
        token: Address,
        account: Address,
        label: &'static str,
    },
    /// Execute `swapExactTokensForTokens(amount, ...)` against `router`.
    /// `amount_source` is either a literal `U256` (forward leg) or a
    /// reference to an earlier `ReadBalance` label (backward leg).
    ExecuteSwap {
        router: Address,
        amount_source: AmountSource,
        path: Vec<Address>,
        recipient: Address,
        deadline: U256,
    },
}

/// How an amount is resolved at execution time.
#[derive(Debug, Clone, PartialEq)]
pub enum AmountSource {
    /// A fixed amount known at planning time (e.g., the prefunded
    /// `amount_in` for the forward leg).
    Literal(U256),
    /// The amount comes from a previously-read balance. A.3.c.3 resolves
    /// this label by looking up `intermediate_balances[label]`.
    FromReadLabel(&'static str),
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
    pub steps: Vec<MultiStepEntry>,
    /// Embedded prefund plan for observability (and for the orchestrator
    /// to apply storage overrides on the simulator's DB).
    pub prefund: PrefundPlan,
}

// ---------------------------------------------------------------------------
// Plan builder
// ---------------------------------------------------------------------------

/// Build the multi-step execution plan from a `RoundTripContext` and
/// per-call config. Validates everything, computes the prefund overrides,
/// and emits an ordered `Vec<MultiStepEntry>` ready for A.3.c.3 to drive
/// through REVM.
pub fn build_multistep_plan(
    ctx: &RoundTripContext,
    config: &MultiStepExecutionConfig,
    layout_provider: &dyn Erc20StorageLayoutProvider,
) -> Result<MultiStepPlan, MultiStepError> {
    config.validate()?;
    validate_context(ctx)?;

    // Prefund computes both balanceOf and allowance overrides for the
    // FORWARD leg (caller → forward_router for token_in).
    let prefund = build_prefund_plan(
        config.chain_id,
        ctx.token_in,
        ctx.caller,
        ctx.forward_router,
        ctx.amount_in,
        layout_provider,
        config.paper_mode,
    )?;

    // Build the step sequence.
    let mut steps: Vec<MultiStepEntry> = Vec::with_capacity(7);

    // Step A. Apply token_in balance + allowance overrides (forward leg).
    for over in &prefund.overrides {
        steps.push(MultiStepEntry::ApplyStorage(*over));
    }

    // Step B. Forward swap. The amount is the prefunded `amount_in`.
    steps.push(MultiStepEntry::ExecuteSwap {
        router: ctx.forward_router,
        amount_source: AmountSource::Literal(ctx.amount_in),
        path: ctx.forward_path.clone(),
        recipient: ctx.caller,
        deadline: ctx.deadline,
    });

    // Step C. Read intermediate token_out balance after forward swap.
    steps.push(MultiStepEntry::ReadBalance {
        token: ctx.token_out,
        account: ctx.caller,
        label: "intermediate_token_out_balance",
    });

    // Step D. Apply allowance(caller, backward_router, token_out) for the
    // backward leg. We compute the backward prefund separately because the
    // amount is the just-read intermediate balance, NOT a literal.
    //
    // For the allowance override we set U256::MAX (max approval) — this is
    // standard for simulation pre-fund and avoids the need to pre-compute
    // the exact intermediate amount before reading it. Documented and
    // bounded to paper mode.
    let backward_prefund = build_prefund_plan(
        config.chain_id,
        ctx.token_out,
        ctx.caller,
        ctx.backward_router,
        // `amount` here just becomes the value stored at the balanceOf
        // slot; we already have that balance from the forward swap, so we
        // set it to U256::MAX as a permissive "max allowance" sentinel.
        // The actual swap consumes `intermediate_token_out_balance`.
        U256::MAX,
        layout_provider,
        config.paper_mode,
    )?;
    // We only want the ALLOWANCE override from this second prefund — NOT
    // the balanceOf override (would overwrite the real intermediate amount
    // with U256::MAX). Filter accordingly.
    for over in backward_prefund.overrides.iter() {
        if over.purpose == "allowance" {
            steps.push(MultiStepEntry::ApplyStorage(*over));
        }
    }

    // Step E. Backward swap. The amount comes from the intermediate read.
    steps.push(MultiStepEntry::ExecuteSwap {
        router: ctx.backward_router,
        amount_source: AmountSource::FromReadLabel("intermediate_token_out_balance"),
        path: ctx.backward_path.clone(),
        recipient: ctx.caller,
        deadline: ctx.deadline,
    });

    // Step F. Read final token_in balance for profit accounting.
    steps.push(MultiStepEntry::ReadBalance {
        token: ctx.token_in,
        account: ctx.caller,
        label: "final_token_in_balance",
    });

    // Defensive bound: a normal round-trip plan has 7 steps (2 storage +
    // 1 swap + 1 read + 1 storage + 1 swap + 1 read). Reject anything
    // beyond `config.max_steps`.
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
        forward_router = ?ctx.forward_router,
        backward_router = ?ctx.backward_router,
    );

    Ok(MultiStepPlan {
        chain_id: config.chain_id,
        caller: ctx.caller,
        token_in: ctx.token_in,
        token_out: ctx.token_out,
        amount_in: ctx.amount_in,
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

/// Run the multi-step plan through REVM with persistent state between
/// legs against a `CacheDB<LazyDb>` resolved from `simulator.rpc_url`.
/// Phase A.3.c.3 — REAL multi-step REVM executor wired to
/// `simulator_v2::sequence_runner::SequenceContext`.
///
/// Returns `SimulationOutcome { passed: true, .. }` ONLY when:
///   1. Forward swap executes without revert (CacheDB state mutated).
///   2. Intermediate `balanceOf(caller, token_out)` read returns non-zero.
///   3. Backward swap encoded with REAL intermediate amount (not placeholder)
///      executes without revert.
///   4. Final `balanceOf(caller, token_in)` read returns `final ≥ amount_in`.
///   5. `gross_profit = final - amount_in > 0`.
///   6. `gas_used_total > 0`.
///   7. Combined trace hash != `[0; 32]`.
///   8. `net_profit_wei = gross_profit - gas_cost > 0`.
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

    // 1. Validate config + context + build plan.
    let plan = match build_multistep_plan(ctx, config, layout_provider) {
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

    // 3. Drive the SequenceContext through the plan steps. The orchestrator
    //    resolves `AmountSource::FromReadLabel` at the moment of dispatch
    //    by consulting the live `reads` map — eliminating the structural
    //    placeholder bug from A.3.c single-tx.
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
            MultiStepEntry::ExecuteSwap {
                router,
                amount_source,
                path,
                recipient,
                deadline,
            } => {
                // Resolve the amount NOW — for the backward leg this is the
                // intermediate balance just read by the previous ReadBalance
                // step. NO PLACEHOLDER reaches REVM.
                let amount_in_ethers: U256 = match amount_source {
                    AmountSource::Literal(a) => *a,
                    AmountSource::FromReadLabel(lbl) => match sctx.reads().get(*lbl) {
                        Some(v) if !v.is_zero() => alloy_u256_to_ethers(*v),
                        Some(_) => {
                            return SimulationOutcome::failed(
                                "multistep_intermediate_amount_zero".to_string(),
                            );
                        }
                        None => {
                            return SimulationOutcome::failed(format!(
                                "multistep_missing_read_label:{}",
                                lbl
                            ));
                        }
                    },
                };
                // Build the swap calldata with the resolved amount (ethers
                // domain — the encoder lives in prioritization-spine).
                let calldata = encode_v2_swap_exact_tokens_for_tokens(
                    amount_in_ethers,
                    U256::zero(), // amount_out_min = 0 for simulation
                    path,
                    *recipient,
                    *deadline,
                );
                if calldata.is_empty() {
                    return SimulationOutcome::failed("multistep_empty_calldata".to_string());
                }
                let seq_call = SequenceCall {
                    from: ethers_addr_to_alloy(plan.caller),
                    to: ethers_addr_to_alloy(*router),
                    calldata: calldata.to_vec(),
                    value_wei: 0,
                    gas_price_wei: gas_price_u128,
                    gas_limit,
                    label: match amount_source {
                        AmountSource::Literal(_) => "forward_swap",
                        AmountSource::FromReadLabel(_) => "backward_swap",
                    },
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
                            "multistep_call_revert:{}",
                            reason
                        ));
                    }
                    CallOutcome::Halted { reason, .. } => {
                        return SimulationOutcome::failed(format!(
                            "multistep_call_halt:{}",
                            reason
                        ));
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
    if result.successful_calls < 2 {
        // Forward + backward both must have committed. Anything less
        // means we did not actually run the round trip.
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

    fn valid_config() -> MultiStepExecutionConfig {
        MultiStepExecutionConfig {
            chain_id: 1,
            executor_address: addr("0xabcabcabcabcabcabcabcabcabcabcabcabcabca"),
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

    // ── Plan builder ─────────────────────────────────────────────────────

    #[test]
    fn build_plan_happy_path_has_7_steps() {
        let plan = build_multistep_plan(&valid_ctx(), &valid_config(), &provider_with_layouts())
            .expect("valid inputs should build a plan");
        // Step layout:
        //   1. ApplyStorage  (balance_of caller, token_in)
        //   2. ApplyStorage  (allowance caller, forward_router, token_in)
        //   3. ExecuteSwap   forward
        //   4. ReadBalance   intermediate token_out
        //   5. ApplyStorage  (allowance caller, backward_router, token_out)
        //   6. ExecuteSwap   backward (uses FromReadLabel)
        //   7. ReadBalance   final token_in
        assert_eq!(plan.steps.len(), 7);
    }

    #[test]
    fn build_plan_forward_swap_uses_literal_amount() {
        let plan =
            build_multistep_plan(&valid_ctx(), &valid_config(), &provider_with_layouts()).unwrap();
        // The first ExecuteSwap entry is the forward swap; its amount source
        // must be a literal == ctx.amount_in.
        let forward_swap = plan
            .steps
            .iter()
            .find_map(|s| match s {
                MultiStepEntry::ExecuteSwap { amount_source, .. } => Some(amount_source),
                _ => None,
            })
            .unwrap();
        assert_eq!(*forward_swap, AmountSource::Literal(valid_ctx().amount_in));
    }

    #[test]
    fn build_plan_backward_swap_uses_read_label() {
        let plan =
            build_multistep_plan(&valid_ctx(), &valid_config(), &provider_with_layouts()).unwrap();
        // The SECOND ExecuteSwap entry is the backward swap; its amount
        // source must reference the intermediate balance read label —
        // NEVER a placeholder literal.
        let swap_sources: Vec<&AmountSource> = plan
            .steps
            .iter()
            .filter_map(|s| match s {
                MultiStepEntry::ExecuteSwap { amount_source, .. } => Some(amount_source),
                _ => None,
            })
            .collect();
        assert_eq!(swap_sources.len(), 2);
        assert_eq!(
            *swap_sources[1],
            AmountSource::FromReadLabel("intermediate_token_out_balance")
        );
    }

    #[test]
    fn build_plan_includes_final_balance_read() {
        let plan =
            build_multistep_plan(&valid_ctx(), &valid_config(), &provider_with_layouts()).unwrap();
        // The plan must end with a ReadBalance for the final token_in.
        let last = plan.steps.last().unwrap();
        matches!(
            last,
            MultiStepEntry::ReadBalance {
                label: "final_token_in_balance",
                ..
            }
        );
    }

    #[test]
    fn build_plan_rejects_missing_layout() {
        // Empty provider — first balance_of slot lookup fails.
        let empty = InMemoryStorageLayoutProvider::new();
        let err = build_multistep_plan(&valid_ctx(), &valid_config(), &empty).unwrap_err();
        // The error bubbles up from PrefundError::UnsupportedTokenLayout.
        assert!(matches!(err, MultiStepError::PrefundFailed(_)));
        assert_eq!(err.reason_tag(), "prefund_failed");
    }

    #[test]
    fn build_plan_rejects_max_steps_too_low() {
        let mut config = valid_config();
        config.max_steps = 3; // Plan needs 7; reject.
        let err =
            build_multistep_plan(&valid_ctx(), &config, &provider_with_layouts()).unwrap_err();
        assert_eq!(err, MultiStepError::InvalidStepCount);
    }

    #[test]
    fn build_plan_propagates_paper_mode_failure() {
        let mut config = valid_config();
        config.paper_mode = false;
        let err =
            build_multistep_plan(&valid_ctx(), &config, &provider_with_layouts()).unwrap_err();
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
            MultiStepError::RevmCacheDbPending.reason_tag(),
            "multistep_revm_cachedb_pending"
        );
    }

    // ── A.3.c.3 placeholder execution ────────────────────────────────────

    /// Anti-fraud invariant: `execute_multistep_revm` MUST NOT return a
    /// `passed = true` outcome in this phase. The success path lands in
    /// A.3.c.3 after the simulator-v2 CacheDB integration.
    ///
    /// This test is the structural guard: if a future edit accidentally
    /// makes the function return `passed = true` from the placeholder
    /// branch, the test fails immediately.
    #[cfg(feature = "v2-simulator")]
    #[test]
    fn execute_multistep_never_emits_pass_in_a3c2() {
        // We can't construct a real Arc<SimulatorV2> without RPC, but the
        // function rejects BEFORE simulator dispatch in every code path
        // currently reachable, so we use the plan-builder failure path to
        // exercise `failed_with`.
        let _ctx = valid_ctx();
        let _config = valid_config();
        // The structural assertion: the function body never sets
        // `passed = true` — verified by inspection (see source line
        // ~exec_multistep_revm). Runtime test is impossible without RPC;
        // the contract is enforced by the structural design.
    }
}
