//! Phase A.3.c — REVM execution orchestrator for `RoundTripContext`.
//!
//! Bridges the Phase A.3.a encoder output (`RoundTripContext`) to the
//! Phase A.2.5 per-chain `SimulatorV2` instance. The orchestrator builds
//! `ArbitrageExecutor.executeArbitrage(...)` calldata, dispatches a single
//! REVM transaction through `simulator_v2::SimulatorV2::simulate`, and
//! converts the result into a typed `SimulationOutcome` for downstream
//! gates.
//!
//! ## Scope and honest disclaimers
//!
//! This module ships the SINGLE-TX dispatch path. For most real candidates
//! the simulated `executeArbitrage` call will REVERT today because:
//!
//!   * The caller has no `EXECUTOR_ROLE` on the live deployed contract
//!     (the simulation does not modify AccessControl storage).
//!   * The caller has no `tokenIn` balance (Phase A.3.c.2 will implement
//!     the ERC-20 storage prefund cheat).
//!   * The `routers[]` are checked against `approvedRouters` storage;
//!     unapproved routers revert with `RouterNotApproved`.
//!   * For multi-leg arbitrage the backward payload requires the
//!     intermediate amount of `token_out` produced by the forward swap
//!     — unknown at encoding time. Phase A.3.c.2 introduces a multi-step
//!     orchestrator (forward swap → read intermediate balance → encode
//!     backward swap → backward swap) that solves this.
//!
//! That is the HONEST state of the wire. `SIM_REVERT` for the majority of
//! candidates is the expected and correct outcome — `gates.rs::
//! EvidenceGate` then keeps the candidate fail-closed. `SIM_SUCCESS` is
//! emitted only when REVM genuinely runs to completion AND
//! `SimResult.net_profit_wei > 0` AND `gas_used > 0` AND `trace_hash`
//! is present.
//!
//! ## Anti-fraud invariants
//!
//! 1. No PASS fabricated. Every `SimulationOutcome::passed=true` originates
//!    in a successful `SimulatorV2::simulate` return.
//! 2. No `gas_used = 0` accepted on the success path.
//! 3. No `trace_hash = [0u8; 32]` accepted on the success path (the
//!    `revm_runner::compute_trace_hash` SHA-256 makes a zero output
//!    cryptographically improbable, but the orchestrator rechecks).
//! 4. No `net_profit_wei <= 0` accepted on the success path.
//! 5. No fallback to a stub if REVM returns an error.
//! 6. No live execution. The orchestrator NEVER signs nor broadcasts.
//! 7. No private relay submit. Result lives in `SimulationOutcome`.
//!
//! ## Synchronous fn — call via `spawn_blocking`
//!
//! `simulator_v2::SimulatorV2::simulate` performs blocking RPC calls via
//! `LazyDb` (the alloy provider transport is async internally but the
//! revm `transact()` invocation is synchronous and blocks the calling
//! thread). The scanner hot path wraps this orchestrator in
//! `tokio::task::spawn_blocking` so the tokio worker thread is never
//! parked on a blocking call — cs-validator finding 2026-05-12 applied.

use ethers::abi::{encode, Token};
use ethers::types::{Address, U256};
use prioritization_spine::round_trip_executor::{RoundTripContext, SimulationOutcome};
use prioritization_spine::swap_encoder::encode_v2_swap_exact_tokens_for_tokens;
use thiserror::Error;
use tracing::{debug, warn};

#[cfg(feature = "v2-simulator")]
use std::sync::Arc;

// ---------------------------------------------------------------------------
// Errors
// ---------------------------------------------------------------------------

/// Typed orchestrator error. Maps 1:1 to a `reason_tag` for counter
/// labelling and `SimulationOutcome.fail_reason` strings.
#[derive(Debug, Error, PartialEq)]
pub enum RoundTripExecutionError {
    #[error("RoundTripContext has zero caller address")]
    InvalidCaller,
    #[error("RoundTripContext has zero token_in address")]
    InvalidTokenIn,
    #[error("RoundTripContext has zero token_out address")]
    InvalidTokenOut,
    #[error("RoundTripContext has zero amount_in")]
    InvalidAmountIn,
    #[error("RoundTripContext has zero forward_router address")]
    InvalidForwardRouter,
    #[error("RoundTripContext has zero backward_router address")]
    InvalidBackwardRouter,
    #[error("RoundTripContext.forward_path is empty")]
    EmptyForwardPath,
    #[error("RoundTripContext.backward_path is empty")]
    EmptyBackwardPath,
    #[error("forward calldata is empty after encoding")]
    EmptyForwardCalldata,
    #[error("config.gas_price_wei exceeds u128::MAX (overflow on simulator-v2 input)")]
    GasPriceOverflow,
    #[error("config.gas_limit is zero (REVM would refuse to execute)")]
    InvalidGasLimit,
    #[error("config.executor_address is the zero address")]
    InvalidExecutorAddress,
    #[error("paper_mode must be true; live mode is not supported by the simulator")]
    PaperModeRequired,
    #[error("REVM reverted: {reason}")]
    RevmReverted { reason: String },
    #[error("REVM infrastructure failure: {reason}")]
    RevmInfraError { reason: String },
    #[error("REVM returned success but gas_used = 0 — anti-fraud guard")]
    SuccessWithZeroGas,
    #[error("REVM returned success but trace_hash is all zeros — anti-fraud guard")]
    SuccessWithEmptyTraceHash,
    #[error("net_profit_wei = {net_profit_wei} is non-positive — candidate not viable")]
    NetProfitNonPositive { net_profit_wei: i128 },
}

impl RoundTripExecutionError {
    /// Short stable tag for counter labelling + `SimulationOutcome.fail_reason`
    /// suffixing. Adding a new variant requires extending the match to keep
    /// the operator-visible vocabulary controlled.
    pub fn reason_tag(&self) -> &'static str {
        match self {
            Self::InvalidCaller => "invalid_caller",
            Self::InvalidTokenIn => "invalid_token_in",
            Self::InvalidTokenOut => "invalid_token_out",
            Self::InvalidAmountIn => "invalid_amount_in",
            Self::InvalidForwardRouter => "invalid_forward_router",
            Self::InvalidBackwardRouter => "invalid_backward_router",
            Self::EmptyForwardPath => "empty_forward_path",
            Self::EmptyBackwardPath => "empty_backward_path",
            Self::EmptyForwardCalldata => "empty_forward_calldata",
            Self::GasPriceOverflow => "gas_price_overflow",
            Self::InvalidGasLimit => "invalid_gas_limit",
            Self::InvalidExecutorAddress => "invalid_executor_address",
            Self::PaperModeRequired => "paper_mode_required",
            Self::RevmReverted { .. } => "revm_reverted",
            Self::RevmInfraError { .. } => "revm_infra_error",
            Self::SuccessWithZeroGas => "success_zero_gas",
            Self::SuccessWithEmptyTraceHash => "success_empty_trace_hash",
            Self::NetProfitNonPositive { .. } => "net_profit_non_positive",
        }
    }
}

// ---------------------------------------------------------------------------
// Config
// ---------------------------------------------------------------------------

/// Per-call orchestrator configuration. Supplied by the scanner so the
/// orchestrator stays pure (no env reads, no system time). All fields are
/// MANDATORY and validated up front.
#[derive(Debug, Clone)]
pub struct RoundTripExecutionConfig {
    /// Chain ID for the `BlockEnv.chain_id` REVM environment.
    pub chain_id: u64,
    /// Deployed `ArbitrageExecutor` contract address for this chain.
    /// Read from `EXECUTOR_<chain_id>` by the scanner; passed in here.
    pub executor_address: Address,
    /// Gas limit for the simulated `executeArbitrage` transaction. Must
    /// be > 0. Typical mainnet value: 30_000_000 (block gas limit).
    pub gas_limit: u64,
    /// Effective gas price in wei used for `net_profit_wei` accounting.
    /// Caller (scanner) reads this from the gas oracle. Must fit in u128
    /// (REVM's `TxEnv.gas_price` is U256 but the simulator-v2 input
    /// adapter caps at u128).
    pub gas_price_wei: U256,
    /// Hash digest of the route plan used by the deployed contract for
    /// event indexing. The simulator does not enforce this; we pass the
    /// caller-supplied value through to the calldata so the encoding is
    /// byte-stable across runs of the same candidate.
    pub route_hash: [u8; 32],
    /// Minimum profit in `token_in` wei the simulated `executeArbitrage`
    /// must satisfy. The contract reverts with `InsufficientProfit` if
    /// `profit < min_profit_wei`. Scanner provides `U256::from(1)` as the
    /// bare contract minimum during A.3.c — A.3.c.2 derives this from a
    /// real economic floor.
    pub min_profit_wei: U256,
    /// Paper-mode flag. MUST be `true` for the orchestrator to proceed;
    /// `false` rejects with `PaperModeRequired`. This is the structural
    /// gate that prevents the orchestrator from ever participating in a
    /// live execution path (RULE 16 + RULE 17).
    pub paper_mode: bool,
}

impl RoundTripExecutionConfig {
    /// Validate every mandatory field. Caller invokes this before any work.
    pub fn validate(&self) -> Result<(), RoundTripExecutionError> {
        if !self.paper_mode {
            return Err(RoundTripExecutionError::PaperModeRequired);
        }
        if self.executor_address == Address::zero() {
            return Err(RoundTripExecutionError::InvalidExecutorAddress);
        }
        if self.gas_limit == 0 {
            return Err(RoundTripExecutionError::InvalidGasLimit);
        }
        if self.gas_price_wei > U256::from(u128::MAX) {
            return Err(RoundTripExecutionError::GasPriceOverflow);
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Validation
// ---------------------------------------------------------------------------

/// Validate a `RoundTripContext` produced by the A.3.a encoder. The encoder
/// already validates most invariants, but the orchestrator re-checks the
/// minimum set so an in-process refactor that bypasses the encoder still
/// cannot reach REVM with bad data.
pub fn validate_context(ctx: &RoundTripContext) -> Result<(), RoundTripExecutionError> {
    if ctx.caller == Address::zero() {
        return Err(RoundTripExecutionError::InvalidCaller);
    }
    if ctx.token_in == Address::zero() {
        return Err(RoundTripExecutionError::InvalidTokenIn);
    }
    if ctx.token_out == Address::zero() {
        return Err(RoundTripExecutionError::InvalidTokenOut);
    }
    if ctx.amount_in.is_zero() {
        return Err(RoundTripExecutionError::InvalidAmountIn);
    }
    if ctx.forward_router == Address::zero() {
        return Err(RoundTripExecutionError::InvalidForwardRouter);
    }
    if ctx.backward_router == Address::zero() {
        return Err(RoundTripExecutionError::InvalidBackwardRouter);
    }
    if ctx.forward_path.is_empty() {
        return Err(RoundTripExecutionError::EmptyForwardPath);
    }
    if ctx.backward_path.is_empty() {
        return Err(RoundTripExecutionError::EmptyBackwardPath);
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// executeArbitrage calldata builder
// ---------------------------------------------------------------------------

/// First 4 bytes of `keccak256("executeArbitrage(bytes32,address,address,uint256,uint256,address[],bytes[])")`.
///
/// Verified against `contracts/src/ArbitrageExecutor.sol:199-207`. The
/// selector is computed once at build time and validated in
/// `tests::execute_arbitrage_selector_matches_known_hash`.
pub const EXECUTE_ARBITRAGE_SELECTOR: [u8; 4] = [0x76, 0xd8, 0x1c, 0xdf];

/// Build the calldata for a single `executeArbitrage(...)` invocation
/// against the deployed `ArbitrageExecutor` contract.
///
/// The function pre-encodes BOTH leg payloads (forward + backward swap)
/// at the call site. For a real multi-hop arbitrage this is incorrect at
/// the byte level — the backward payload's `amountIn` slot is set to
/// `min_profit_wei` as a non-zero sentinel rather than the (unknown)
/// intermediate amount of `token_out`. This calldata is therefore
/// SIMULATION-ONLY: the contract's swap will revert if the router
/// receives an `amountIn` it cannot honour, and the orchestrator records
/// `SIM_REVERT` honestly.
///
/// The full multi-step orchestrator that reads the intermediate amount
/// between legs lands in Phase A.3.c.2.
pub fn build_execute_arbitrage_calldata(
    ctx: &RoundTripContext,
    config: &RoundTripExecutionConfig,
) -> Result<Vec<u8>, RoundTripExecutionError> {
    let forward_payload = encode_v2_swap_exact_tokens_for_tokens(
        ctx.amount_in,
        U256::zero(), // amountOutMin=0 for sim; downstream gates enforce slippage
        &ctx.forward_path,
        config.executor_address, // recipient = the executor contract holding funds
        ctx.deadline,
    );
    if forward_payload.is_empty() {
        return Err(RoundTripExecutionError::EmptyForwardCalldata);
    }
    // Backward payload — note the SIMULATION-ONLY caveat above: the
    // intermediate amount of token_out is unknown here. We use
    // `min_profit_wei` as a non-zero placeholder so the calldata is not
    // structurally empty. The contract's router call will almost
    // certainly revert; the orchestrator records that revert honestly.
    let backward_payload = encode_v2_swap_exact_tokens_for_tokens(
        config.min_profit_wei,
        U256::zero(),
        &ctx.backward_path,
        config.executor_address,
        ctx.deadline,
    );

    // ABI-encode the executeArbitrage(...) arguments.
    let routers = vec![
        Token::Address(ctx.forward_router),
        Token::Address(ctx.backward_router),
    ];
    let payloads = vec![
        Token::Bytes(forward_payload.to_vec()),
        Token::Bytes(backward_payload.to_vec()),
    ];
    let args = encode(&[
        Token::FixedBytes(config.route_hash.to_vec()),
        Token::Address(ctx.token_in),
        Token::Address(ctx.token_out),
        Token::Uint(ctx.amount_in),
        Token::Uint(config.min_profit_wei),
        Token::Array(routers),
        Token::Array(payloads),
    ]);

    let mut calldata = Vec::with_capacity(4 + args.len());
    calldata.extend_from_slice(&EXECUTE_ARBITRAGE_SELECTOR);
    calldata.extend_from_slice(&args);
    Ok(calldata)
}

// ---------------------------------------------------------------------------
// REVM orchestrator
// ---------------------------------------------------------------------------

/// Run `executeArbitrage` against the per-chain `SimulatorV2` and convert
/// the result into a `SimulationOutcome`. Synchronous — the scanner wraps
/// the call in `tokio::task::spawn_blocking` to keep the tokio runtime
/// healthy.
///
/// Returns `SimulationOutcome` with:
///   * `passed = true` ONLY when REVM ran successfully, gas_used > 0,
///     trace_hash is non-zero, AND `net_profit_wei > 0`.
///   * `passed = false` for every other path; `fail_reason` carries the
///     `RoundTripExecutionError::reason_tag()`.
#[cfg(feature = "v2-simulator")]
pub fn execute_round_trip_revm(
    ctx: &RoundTripContext,
    simulator: Arc<simulator_v2::SimulatorV2>,
    config: &RoundTripExecutionConfig,
) -> SimulationOutcome {
    use simulator_v2::{CandidateInput, SimError, Simulator};

    // Validate config + context. Any failure returns a typed reason — no
    // REVM call attempted.
    if let Err(e) = config.validate() {
        return failed_with(e);
    }
    if let Err(e) = validate_context(ctx) {
        return failed_with(e);
    }

    let calldata = match build_execute_arbitrage_calldata(ctx, config) {
        Ok(c) => c,
        Err(e) => return failed_with(e),
    };

    let gas_price_u128 = config.gas_price_wei.as_u128();
    // U256 → u128 narrowing already validated by config.validate(); double-check.
    if U256::from(gas_price_u128) != config.gas_price_wei {
        return failed_with(RoundTripExecutionError::GasPriceOverflow);
    }

    let candidate_input = CandidateInput {
        chain_id: config.chain_id,
        block_number: 0, // 0 lets SimulatorV2 use its memoised "latest" pin
        from: ctx.caller.0,
        to: config.executor_address.0,
        calldata,
        value_wei: 0,
        gas_price_wei: gas_price_u128,
    };

    debug!(
        event = "round_trip.execution_start",
        chain_id = config.chain_id,
        caller = ?ctx.caller,
        executor = ?config.executor_address,
        token_in = ?ctx.token_in,
        token_out = ?ctx.token_out,
        amount_in_wei = %ctx.amount_in,
        calldata_len = candidate_input.calldata.len(),
        gas_limit = config.gas_limit,
        paper_mode = config.paper_mode,
        "running executeArbitrage against SimulatorV2"
    );

    match simulator.simulate(&candidate_input) {
        Ok(result) => {
            // Anti-fraud guards — never accept a "success" with zeroed
            // observability fields (RULE 5 + RULE 6 from the directive).
            if result.gas_used == 0 {
                return failed_with(RoundTripExecutionError::SuccessWithZeroGas);
            }
            if result.trace_hash == [0u8; 32] {
                return failed_with(RoundTripExecutionError::SuccessWithEmptyTraceHash);
            }
            if result.net_profit_wei <= 0 {
                return failed_with(RoundTripExecutionError::NetProfitNonPositive {
                    net_profit_wei: result.net_profit_wei,
                });
            }
            // All anti-fraud guards passed. SIM_SUCCESS is REAL.
            let profit_u256 = if result.net_profit_wei > 0 {
                U256::from(result.net_profit_wei as u128)
            } else {
                U256::zero()
            };
            debug!(
                event = "round_trip.revm_result",
                status = "success",
                gas_used = result.gas_used,
                net_profit_wei = result.net_profit_wei,
                "executeArbitrage simulation succeeded"
            );
            SimulationOutcome {
                passed: true,
                simulated_profit_token_in: profit_u256,
                intermediate_amount_out: None,
                gas_used_total: result.gas_used,
                fail_reason: None,
            }
        }
        Err(SimError::Reverted(reason)) => {
            debug!(
                event = "round_trip.revm_result",
                status = "revert",
                reason = %reason,
                "executeArbitrage simulation reverted"
            );
            failed_with(RoundTripExecutionError::RevmReverted { reason })
        }
        Err(SimError::Provider(reason)) => {
            warn!(
                event = "round_trip.revm_result",
                status = "infra_error",
                reason = %reason,
                "REVM infrastructure failure"
            );
            failed_with(RoundTripExecutionError::RevmInfraError { reason })
        }
        Err(SimError::NotImplemented) => failed_with(RoundTripExecutionError::RevmInfraError {
            reason: "simulator returned NotImplemented".to_string(),
        }),
    }
}

/// Helper: build a `SimulationOutcome::failed(...)` with a tagged reason
/// suffix (`"<reason_tag>:<display>"`) so the operator dashboard can
/// group by tag while still surfacing the human-readable detail.
fn failed_with(e: RoundTripExecutionError) -> SimulationOutcome {
    SimulationOutcome::failed(format!("{}:{}", e.reason_tag(), e))
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use std::str::FromStr;

    fn addr(s: &str) -> Address {
        Address::from_str(s).unwrap()
    }

    fn valid_ctx() -> RoundTripContext {
        let weth = addr("0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2");
        let usdc = addr("0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48");
        let router_a = addr("0x7a250d5630b4cf539739df2c5dacb4c659f2488d");
        let router_b = addr("0xd9e1ce17f2641f24ae83637ab66a2cca9c378b9f");
        RoundTripContext {
            caller: addr("0x1234567890123456789012345678901234567890"),
            token_in: weth,
            token_out: usdc,
            amount_in: U256::from(10u64).pow(U256::from(18u64)),
            forward_router: router_a,
            forward_path: vec![weth, usdc],
            backward_router: router_b,
            backward_path: vec![usdc, weth],
            deadline: U256::from(1_700_000_000u64),
        }
    }

    fn valid_config() -> RoundTripExecutionConfig {
        RoundTripExecutionConfig {
            chain_id: 1,
            executor_address: addr("0xabcabcabcabcabcabcabcabcabcabcabcabcabca"),
            gas_limit: 30_000_000,
            gas_price_wei: U256::from(25_000_000_000u64),
            route_hash: [1u8; 32],
            min_profit_wei: U256::from(1u64),
            paper_mode: true,
        }
    }

    // ── validation tests ───────────────────────────────────────────────────

    #[test]
    fn invalid_context_zero_caller_rejected() {
        let mut ctx = valid_ctx();
        ctx.caller = Address::zero();
        assert_eq!(
            validate_context(&ctx).unwrap_err(),
            RoundTripExecutionError::InvalidCaller
        );
    }

    #[test]
    fn invalid_context_zero_token_in_rejected() {
        let mut ctx = valid_ctx();
        ctx.token_in = Address::zero();
        assert_eq!(
            validate_context(&ctx).unwrap_err(),
            RoundTripExecutionError::InvalidTokenIn
        );
    }

    #[test]
    fn invalid_context_zero_token_out_rejected() {
        let mut ctx = valid_ctx();
        ctx.token_out = Address::zero();
        assert_eq!(
            validate_context(&ctx).unwrap_err(),
            RoundTripExecutionError::InvalidTokenOut
        );
    }

    #[test]
    fn invalid_context_zero_amount_rejected() {
        let mut ctx = valid_ctx();
        ctx.amount_in = U256::zero();
        assert_eq!(
            validate_context(&ctx).unwrap_err(),
            RoundTripExecutionError::InvalidAmountIn
        );
    }

    #[test]
    fn invalid_context_zero_forward_router_rejected() {
        let mut ctx = valid_ctx();
        ctx.forward_router = Address::zero();
        assert_eq!(
            validate_context(&ctx).unwrap_err(),
            RoundTripExecutionError::InvalidForwardRouter
        );
    }

    #[test]
    fn invalid_context_zero_backward_router_rejected() {
        let mut ctx = valid_ctx();
        ctx.backward_router = Address::zero();
        assert_eq!(
            validate_context(&ctx).unwrap_err(),
            RoundTripExecutionError::InvalidBackwardRouter
        );
    }

    #[test]
    fn invalid_context_empty_forward_path_rejected() {
        let mut ctx = valid_ctx();
        ctx.forward_path = vec![];
        assert_eq!(
            validate_context(&ctx).unwrap_err(),
            RoundTripExecutionError::EmptyForwardPath
        );
    }

    #[test]
    fn invalid_context_empty_backward_path_rejected() {
        let mut ctx = valid_ctx();
        ctx.backward_path = vec![];
        assert_eq!(
            validate_context(&ctx).unwrap_err(),
            RoundTripExecutionError::EmptyBackwardPath
        );
    }

    #[test]
    fn valid_context_passes_validation() {
        assert!(validate_context(&valid_ctx()).is_ok());
    }

    #[test]
    fn config_paper_mode_required() {
        let mut cfg = valid_config();
        cfg.paper_mode = false;
        assert_eq!(
            cfg.validate().unwrap_err(),
            RoundTripExecutionError::PaperModeRequired
        );
    }

    #[test]
    fn config_zero_executor_rejected() {
        let mut cfg = valid_config();
        cfg.executor_address = Address::zero();
        assert_eq!(
            cfg.validate().unwrap_err(),
            RoundTripExecutionError::InvalidExecutorAddress
        );
    }

    #[test]
    fn config_zero_gas_limit_rejected() {
        let mut cfg = valid_config();
        cfg.gas_limit = 0;
        assert_eq!(
            cfg.validate().unwrap_err(),
            RoundTripExecutionError::InvalidGasLimit
        );
    }

    #[test]
    fn config_gas_price_overflow_rejected() {
        let mut cfg = valid_config();
        cfg.gas_price_wei = U256::from(u128::MAX) + U256::one();
        assert_eq!(
            cfg.validate().unwrap_err(),
            RoundTripExecutionError::GasPriceOverflow
        );
    }

    #[test]
    fn valid_config_passes_validation() {
        assert!(valid_config().validate().is_ok());
    }

    // ── calldata builder tests ─────────────────────────────────────────────

    #[test]
    fn execute_arbitrage_selector_matches_known_hash() {
        use ethers::utils::keccak256;
        let signature =
            b"executeArbitrage(bytes32,address,address,uint256,uint256,address[],bytes[])";
        let hash = keccak256(signature);
        assert_eq!(EXECUTE_ARBITRAGE_SELECTOR, hash[..4]);
    }

    #[test]
    fn calldata_starts_with_correct_selector() {
        let calldata = build_execute_arbitrage_calldata(&valid_ctx(), &valid_config()).unwrap();
        assert_eq!(&calldata[..4], &EXECUTE_ARBITRAGE_SELECTOR);
    }

    #[test]
    fn calldata_is_non_empty() {
        let calldata = build_execute_arbitrage_calldata(&valid_ctx(), &valid_config()).unwrap();
        // Selector (4) + 7 statics × 32 bytes (head) + at least the dynamic
        // arrays' offsets and lengths. Bound is loose.
        assert!(calldata.len() > 4 + 7 * 32);
    }

    #[test]
    fn calldata_encodes_route_hash_correctly() {
        let calldata = build_execute_arbitrage_calldata(&valid_ctx(), &valid_config()).unwrap();
        // First 32 bytes after the selector are the routeHash word.
        assert_eq!(&calldata[4..36], &[1u8; 32]);
    }

    // ── outcome helper tests ───────────────────────────────────────────────

    #[test]
    fn failed_with_carries_reason_tag_prefix() {
        let outcome = failed_with(RoundTripExecutionError::RevmReverted {
            reason: "InsufficientBalance".to_string(),
        });
        assert!(!outcome.passed);
        let reason = outcome.fail_reason.as_deref().unwrap();
        assert!(reason.starts_with("revm_reverted:"), "reason = {reason}");
    }

    #[test]
    fn net_profit_non_positive_error_has_tag() {
        let e = RoundTripExecutionError::NetProfitNonPositive {
            net_profit_wei: -100,
        };
        assert_eq!(e.reason_tag(), "net_profit_non_positive");
    }

    #[test]
    fn revm_reverted_error_has_tag() {
        let e = RoundTripExecutionError::RevmReverted {
            reason: "any".to_string(),
        };
        assert_eq!(e.reason_tag(), "revm_reverted");
    }

    #[test]
    fn success_zero_gas_error_has_tag() {
        let e = RoundTripExecutionError::SuccessWithZeroGas;
        assert_eq!(e.reason_tag(), "success_zero_gas");
    }
}
