//! `executeArbitrage` / `executeArbitrageFlashFunded` calldata encoders —
//! single source of truth.
//!
//! Promoted out of `searcher-rs/src/sim_orchestrator.rs` (M2 T1) so that both
//! the simulation path (searcher-rs) and, later, the broadcast path
//! (relays-client) encode the deployed `ArbitrageExecutor` calldata from ONE
//! implementation. The emitted bytes are identical to the previous
//! searcher-rs-local encoder.
//!
//! ## Self-funded vs flash-funded (M2 flash R1)
//!
//! The operator's live execution path is FLASH-FUNDED: the broadcast tx is
//! `EOA → FlashLoanExecutor.requestFlashLoan(asset, amount, params)` where
//! `params = executeArbitrageFlashFunded(...)`. The inner
//! `executeArbitrageFlashFunded(bytes32,address,address,uint256,uint256,address[],bytes[])`
//! (selector `0xdde0bf51`) takes the IDENTICAL 7 args as the existing
//! self-funded `executeArbitrage(...)` (selector `0x76d81cdf`), and the per-leg
//! swap payloads keep `recipient = the executor` (the executor holds funds
//! between legs in both modes). The inner flash calldata is therefore
//! byte-identical to the self-funded calldata EXCEPT the 4-byte selector — so
//! both selectors reuse ONE encoding body via [`encode_execute_arbitrage_body`].
//!
//! The function is intentionally decoupled from searcher-rs's
//! `RoundTripExecutionConfig`: it takes the three config fields it actually
//! reads (`route_hash`, `min_profit_wei`, `executor_address`) as primitives,
//! and returns a small spine-local error rather than searcher-rs's
//! `RoundTripExecutionError`. The sim call site maps the error back to its own
//! type so its behaviour is unchanged.

use ethers::abi::{encode, Token};
use ethers::types::{Address, U256};
use thiserror::Error;

use crate::round_trip_executor::RoundTripContext;
use crate::swap_encoder::encode_v2_swap_exact_tokens_for_tokens;

/// Encoder error for the `executeArbitrage*` builders.
///
/// Mirrors the single failure mode the encoder can hit on its own; callers map
/// it onto their domain error type (the sim path maps it to
/// `RoundTripExecutionError::EmptyForwardCalldata`).
#[derive(Debug, Error, PartialEq)]
pub enum ExecuteArbitrageEncodeError {
    #[error("forward calldata is empty after encoding")]
    EmptyForwardCalldata,
}

/// First 4 bytes of `keccak256("executeArbitrage(bytes32,address,address,uint256,uint256,address[],bytes[])")`.
///
/// Verified against `contracts/src/ArbitrageExecutor.sol:199-207`. The
/// selector is computed once at build time and validated in
/// `tests::execute_arbitrage_selector_matches_known_hash`.
pub const EXECUTE_ARBITRAGE_SELECTOR: [u8; 4] = [0x76, 0xd8, 0x1c, 0xdf];

/// First 4 bytes of `keccak256("executeArbitrageFlashFunded(bytes32,address,address,uint256,uint256,address[],bytes[])")`.
///
/// Verified against `contracts/src/ArbitrageExecutor.sol:279-287`. Identical
/// 7-arg layout to [`EXECUTE_ARBITRAGE_SELECTOR`]; only the selector differs.
/// Validated in `tests::execute_arbitrage_flash_funded_selector_matches_known_hash`.
pub const EXECUTE_ARBITRAGE_FLASH_FUNDED_SELECTOR: [u8; 4] = [0xdd, 0xe0, 0xbf, 0x51];

/// First 4 bytes of `keccak256("requestFlashLoan(address,uint256,bytes)")`.
///
/// Verified against `contracts/src/FlashLoanExecutor.sol:270`. The flash
/// provider is internal to the FlashLoanExecutor (no provider arg). Validated
/// in `tests::request_flash_loan_selector_matches_known_hash`.
pub const REQUEST_FLASH_LOAN_SELECTOR: [u8; 4] = [0x51, 0x07, 0xd6, 0x1e];

/// Shared inner body for both `executeArbitrage` (self-funded) and
/// `executeArbitrageFlashFunded` (flash-funded).
///
/// Both deployed entrypoints take the IDENTICAL 7 args
/// `(bytes32 routeHash, address tokenIn, address tokenOut, uint256 amountIn,
/// uint256 minProfit, address[] routers, bytes[] payload)` and both keep the
/// per-leg swap `recipient = executor_address` (the executor holds funds
/// between legs). The ONLY difference is the 4-byte selector — so this helper
/// builds the full `selector ++ abi(args)` calldata and the public builders
/// just pass their selector. This keeps the payload-building + ABI-encoding
/// logic in ONE place (DRY); see the module docs.
///
/// The function pre-encodes BOTH leg payloads (forward + backward swap)
/// at the call site. The backward payload's `amountIn` slot is the explicit
/// `backward_amount_in` parameter: callers that know the real intermediate
/// amount of `token_out` (the leg-1 input produced by the forward swap) pass
/// it; the SIMULATION-ONLY callers pass `min_profit_wei` as a non-zero
/// sentinel instead (the intermediate is unknown at encode time, so that
/// calldata's router call will almost certainly revert and the orchestrator
/// records `SIM_REVERT` honestly). The full multi-step orchestrator that reads
/// the intermediate amount between legs lands in Phase A.3.c.2.
fn encode_execute_arbitrage_body(
    selector: [u8; 4],
    ctx: &RoundTripContext,
    backward_amount_in: U256,
    route_hash: [u8; 32],
    min_profit_wei: U256,
    executor_address: Address,
) -> Result<Vec<u8>, ExecuteArbitrageEncodeError> {
    let forward_payload = encode_v2_swap_exact_tokens_for_tokens(
        ctx.amount_in,
        U256::zero(), // amountOutMin=0 for sim; downstream gates enforce slippage
        &ctx.forward_path,
        executor_address, // recipient = the executor contract holding funds
        ctx.deadline,
    );
    if forward_payload.is_empty() {
        return Err(ExecuteArbitrageEncodeError::EmptyForwardCalldata);
    }
    // Backward payload — `backward_amount_in` is the leg-1 swap amountIn. The
    // intermediate-aware caller passes the real `token_out` amount produced by
    // the forward swap; the SIMULATION-ONLY callers pass `min_profit_wei` as a
    // non-zero sentinel (the intermediate is unknown at encode time, so the
    // contract's router call will almost certainly revert and the orchestrator
    // records that revert honestly).
    let backward_payload = encode_v2_swap_exact_tokens_for_tokens(
        backward_amount_in,
        U256::zero(),
        &ctx.backward_path,
        executor_address,
        ctx.deadline,
    );

    // ABI-encode the executeArbitrage(...) / executeArbitrageFlashFunded(...)
    // arguments (identical 7-arg layout for both entrypoints).
    let routers = vec![
        Token::Address(ctx.forward_router),
        Token::Address(ctx.backward_router),
    ];
    let payloads = vec![
        Token::Bytes(forward_payload.to_vec()),
        Token::Bytes(backward_payload.to_vec()),
    ];
    let args = encode(&[
        Token::FixedBytes(route_hash.to_vec()),
        Token::Address(ctx.token_in),
        Token::Address(ctx.token_out),
        Token::Uint(ctx.amount_in),
        Token::Uint(min_profit_wei),
        Token::Array(routers),
        Token::Array(payloads),
    ]);

    let mut calldata = Vec::with_capacity(4 + args.len());
    calldata.extend_from_slice(&selector);
    calldata.extend_from_slice(&args);
    Ok(calldata)
}

/// Build the calldata for a single self-funded `executeArbitrage(...)`
/// invocation against the deployed `ArbitrageExecutor` contract
/// (selector `0x76d81cdf`).
///
/// Thin wrapper over [`encode_execute_arbitrage_body`] with the self-funded
/// selector. See that helper for the SIMULATION-ONLY caveat on the backward
/// leg.
pub fn build_execute_arbitrage_calldata(
    ctx: &RoundTripContext,
    route_hash: [u8; 32],
    min_profit_wei: U256,
    executor_address: Address,
) -> Result<Vec<u8>, ExecuteArbitrageEncodeError> {
    encode_execute_arbitrage_body(
        EXECUTE_ARBITRAGE_SELECTOR,
        ctx,
        // SENTINEL backward amount — preserves the self-funded sim path bytes.
        min_profit_wei,
        route_hash,
        min_profit_wei,
        executor_address,
    )
}

/// Build the INNER calldata for a single flash-funded
/// `executeArbitrageFlashFunded(...)` invocation against the deployed
/// `ArbitrageExecutor` contract (selector `0xdde0bf51`).
///
/// Same inputs and body as [`build_execute_arbitrage_calldata`] — the emitted
/// bytes are byte-identical EXCEPT the 4-byte selector (proved by
/// `tests::flash_funded_inner_equals_self_funded_except_selector`). This is the
/// `params` that gets wrapped by [`build_request_flash_loan_calldata`] for the
/// outer `requestFlashLoan` broadcast; callers that want the full wrapped
/// broadcast calldata should use [`build_flash_funded_broadcast_calldata`].
pub fn build_execute_arbitrage_flash_funded_calldata(
    ctx: &RoundTripContext,
    route_hash: [u8; 32],
    min_profit_wei: U256,
    executor_address: Address,
) -> Result<Vec<u8>, ExecuteArbitrageEncodeError> {
    encode_execute_arbitrage_body(
        EXECUTE_ARBITRAGE_FLASH_FUNDED_SELECTOR,
        ctx,
        // SENTINEL backward amount — preserves the flash-funded sim path bytes.
        min_profit_wei,
        route_hash,
        min_profit_wei,
        executor_address,
    )
}

/// Build the OUTER `requestFlashLoan(address asset, uint256 amount, bytes params)`
/// calldata against the deployed `FlashLoanExecutor` contract
/// (selector `0x5107d61e`).
///
/// The flash provider (Balancer adapter / Aave) is internal to the
/// FlashLoanExecutor, so there is NO provider arg here. `inner_params` is the
/// inner `executeArbitrageFlashFunded(...)` calldata that the executor forwards
/// to the `ArbitrageExecutor` inside the flash callback. This is the calldata
/// broadcast with `.to() = FlashLoanExecutor`.
pub fn build_request_flash_loan_calldata(
    asset: Address,
    amount: U256,
    inner_params: &[u8],
) -> Vec<u8> {
    let args = encode(&[
        Token::Address(asset),
        Token::Uint(amount),
        Token::Bytes(inner_params.to_vec()),
    ]);
    let mut calldata = Vec::with_capacity(4 + args.len());
    calldata.extend_from_slice(&REQUEST_FLASH_LOAN_SELECTOR);
    calldata.extend_from_slice(&args);
    calldata
}

/// Build the full FLASH-FUNDED broadcast calldata — the single entry the sim
/// and the relays-client will both call (later increments).
///
/// Composes the two layers:
///   1. `inner  = executeArbitrageFlashFunded(...)` (selector `0xdde0bf51`) via
///      [`build_execute_arbitrage_flash_funded_calldata`].
///   2. `outer  = requestFlashLoan(asset, amount, inner)` (selector
///      `0x5107d61e`) via [`build_request_flash_loan_calldata`].
///
/// The borrowed `asset` is the round trip's `token_in` (the leg-0 input that is
/// also the leg-1 output — i.e. the token the flash loan funds and repays), and
/// `amount` is `ctx.amount_in`. Both are derived from `ctx`; no extra
/// `ValidatedPlan` field is required. Returns the OUTER `requestFlashLoan`
/// calldata to broadcast with `.to() = FlashLoanExecutor`.
pub fn build_flash_funded_broadcast_calldata(
    ctx: &RoundTripContext,
    route_hash: [u8; 32],
    min_profit_wei: U256,
    executor_address: Address,
) -> Result<Vec<u8>, ExecuteArbitrageEncodeError> {
    let inner = build_execute_arbitrage_flash_funded_calldata(
        ctx,
        route_hash,
        min_profit_wei,
        executor_address,
    )?;
    // asset = the borrowed token = token_in; amount = amountIn (both from ctx).
    Ok(build_request_flash_loan_calldata(
        ctx.token_in,
        ctx.amount_in,
        &inner,
    ))
}

/// Build the full FLASH-FUNDED broadcast calldata with an EXPLICIT backward-leg
/// amount — the intermediate-aware sibling of
/// [`build_flash_funded_broadcast_calldata`].
///
/// Identical wrapping (outer `requestFlashLoan` `0x5107d61e` around inner
/// `executeArbitrageFlashFunded` `0xdde0bf51`, asset/amount derived from ctx,
/// `minProfit = min_profit_wei`) EXCEPT the inner backward-leg swap `amountIn`
/// is the supplied `backward_amount_in` — the REAL intermediate `token_out`
/// produced by the forward swap — instead of the `min_profit_wei` sentinel. The
/// forward leg is unchanged (`ctx.amount_in`).
///
/// Passing `backward_amount_in = min_profit_wei` reproduces
/// [`build_flash_funded_broadcast_calldata`] byte-for-byte (proved by
/// `tests::with_intermediate_sentinel_equals_legacy_broadcast`). This is the
/// foundation for carry-validated-calldata (O1): the off-chain encodes leg-1
/// with the on-chain intermediate delta that the reworked contract approves.
pub fn build_flash_funded_broadcast_calldata_with_intermediate(
    ctx: &RoundTripContext,
    backward_amount_in: U256,
    route_hash: [u8; 32],
    min_profit_wei: U256,
    executor_address: Address,
) -> Result<Vec<u8>, ExecuteArbitrageEncodeError> {
    let inner = encode_execute_arbitrage_body(
        EXECUTE_ARBITRAGE_FLASH_FUNDED_SELECTOR,
        ctx,
        backward_amount_in,
        route_hash,
        min_profit_wei,
        executor_address,
    )?;
    // asset = the borrowed token = token_in; amount = amountIn (both from ctx).
    Ok(build_request_flash_loan_calldata(
        ctx.token_in,
        ctx.amount_in,
        &inner,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use ethers::abi::{decode, ParamType};
    use std::str::FromStr;

    /// Build a deterministic 2-token round trip context shared across tests.
    fn fixture() -> (RoundTripContext, [u8; 32], U256, Address) {
        let token_in = Address::from_str("0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2")
            .expect("valid token_in address");
        let token_out = Address::from_str("0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48")
            .expect("valid token_out address");
        let forward_router = Address::from_str("0x7a250d5630b4cf539739df2c5dacb4c659f2488d")
            .expect("valid forward router");
        let backward_router = Address::from_str("0x68b3465833fb72a70ecdf485e0e4c7bd8665fc45")
            .expect("valid backward router");
        let caller = Address::from_str("0x1111111111111111111111111111111111111111")
            .expect("valid caller address");

        let ctx = RoundTripContext {
            caller,
            token_in,
            token_out,
            amount_in: U256::from(10_u64).pow(U256::from(18)),
            forward_router,
            forward_path: vec![token_in, token_out],
            backward_router,
            backward_path: vec![token_out, token_in],
            deadline: U256::from(1_700_000_000u64),
        };

        let route_hash = [0x11u8; 32];
        let min_profit_wei = U256::from(1);
        let executor_address = Address::from_str("0x2222222222222222222222222222222222222222")
            .expect("valid executor address");

        (ctx, route_hash, min_profit_wei, executor_address)
    }

    #[test]
    fn execute_arbitrage_selector_matches_known_hash() {
        use ethers::utils::keccak256;
        let signature =
            b"executeArbitrage(bytes32,address,address,uint256,uint256,address[],bytes[])";
        let hash = keccak256(signature);
        assert_eq!(EXECUTE_ARBITRAGE_SELECTOR, hash[..4]);
    }

    #[test]
    fn execute_arbitrage_flash_funded_selector_matches_known_hash() {
        use ethers::utils::keccak256;
        let signature =
            b"executeArbitrageFlashFunded(bytes32,address,address,uint256,uint256,address[],bytes[])";
        let hash = keccak256(signature);
        assert_eq!(EXECUTE_ARBITRAGE_FLASH_FUNDED_SELECTOR, hash[..4]);
    }

    #[test]
    fn request_flash_loan_selector_matches_known_hash() {
        use ethers::utils::keccak256;
        let signature = b"requestFlashLoan(address,uint256,bytes)";
        let hash = keccak256(signature);
        assert_eq!(REQUEST_FLASH_LOAN_SELECTOR, hash[..4]);
    }

    /// Output-contract lock for the M2 broadcast path: the encoder must emit a
    /// well-formed `executeArbitrage(bytes32,address,address,uint256,uint256,address[],bytes[])`
    /// call whose ABI-decoded fields match the inputs. This guards both the
    /// selector (0x76d81cdf — NOT the `executeArbitrageFlashFunded` 0xdde0bf51
    /// sibling) and the argument layout the deployed contract relies on.
    #[test]
    fn build_calldata_matches_execute_arbitrage_abi() {
        let (ctx, route_hash, min_profit_wei, executor_address) = fixture();

        let calldata =
            build_execute_arbitrage_calldata(&ctx, route_hash, min_profit_wei, executor_address)
                .expect("encoder should produce calldata for a 2-token round trip");

        // Selector locks 0x76d81cdf and guards against the
        // executeArbitrageFlashFunded (0xdde0bf51) confusion.
        assert_eq!(EXECUTE_ARBITRAGE_SELECTOR, calldata[0..4]);

        // ABI-decode the argument tail against the 7 declared param types.
        let param_types = [
            ParamType::FixedBytes(32),
            ParamType::Address,
            ParamType::Address,
            ParamType::Uint(256),
            ParamType::Uint(256),
            ParamType::Array(Box::new(ParamType::Address)),
            ParamType::Array(Box::new(ParamType::Bytes)),
        ];
        let decoded = decode(&param_types, &calldata[4..]).expect("calldata must ABI-decode");
        assert_eq!(decoded.len(), 7, "expected 7 top-level args");

        // routeHash
        let decoded_route_hash = decoded[0].clone().into_fixed_bytes().expect("arg 0 is bytes32");
        assert_eq!(decoded_route_hash.as_slice(), &route_hash[..]);

        // tokenIn / tokenOut
        let decoded_token_in = decoded[1].clone().into_address().expect("arg 1 is address");
        assert_eq!(decoded_token_in, ctx.token_in);
        let decoded_token_out = decoded[2].clone().into_address().expect("arg 2 is address");
        assert_eq!(decoded_token_out, ctx.token_out);

        // amountIn / minProfit
        let decoded_amount_in = decoded[3].clone().into_uint().expect("arg 3 is uint256");
        assert_eq!(decoded_amount_in, ctx.amount_in);
        let decoded_min_profit = decoded[4].clone().into_uint().expect("arg 4 is uint256");
        assert_eq!(decoded_min_profit, min_profit_wei);

        // routers == [forward_router, backward_router]
        let decoded_routers = decoded[5].clone().into_array().expect("arg 5 is address[]");
        assert_eq!(decoded_routers.len(), 2, "expected 2 routers (forward, backward)");
        let router_0 = decoded_routers[0].clone().into_address().expect("router 0 is address");
        let router_1 = decoded_routers[1].clone().into_address().expect("router 1 is address");
        assert_eq!(router_0, ctx.forward_router);
        assert_eq!(router_1, ctx.backward_router);

        // payload == 2 legs, each non-empty (forward + backward swap calldata)
        let decoded_payloads = decoded[6].clone().into_array().expect("arg 6 is bytes[]");
        assert_eq!(decoded_payloads.len(), 2, "expected 2 payload legs");
        for (i, leg) in decoded_payloads.iter().enumerate() {
            let leg_bytes = leg.clone().into_bytes().expect("payload leg is bytes");
            assert!(!leg_bytes.is_empty(), "payload leg {i} must be non-empty");
        }
    }

    /// The shared body proof: the flash-funded inner calldata is byte-identical
    /// to the self-funded calldata EXCEPT the leading 4-byte selector. Locks the
    /// DRY refactor — both selectors run through `encode_execute_arbitrage_body`.
    #[test]
    fn flash_funded_inner_equals_self_funded_except_selector() {
        let (ctx, route_hash, min_profit_wei, executor_address) = fixture();

        let self_funded =
            build_execute_arbitrage_calldata(&ctx, route_hash, min_profit_wei, executor_address)
                .expect("self-funded encoder should produce calldata");
        let flash_funded = build_execute_arbitrage_flash_funded_calldata(
            &ctx,
            route_hash,
            min_profit_wei,
            executor_address,
        )
        .expect("flash-funded encoder should produce calldata");

        // The two selectors differ.
        assert_ne!(
            EXECUTE_ARBITRAGE_SELECTOR, EXECUTE_ARBITRAGE_FLASH_FUNDED_SELECTOR,
            "self-funded and flash-funded selectors must differ"
        );
        assert_eq!(self_funded[0..4], EXECUTE_ARBITRAGE_SELECTOR);
        assert_eq!(flash_funded[0..4], EXECUTE_ARBITRAGE_FLASH_FUNDED_SELECTOR);

        // ...but the encoded argument tails are byte-identical (shared body).
        assert_eq!(
            self_funded[4..],
            flash_funded[4..],
            "inner flash calldata must match self-funded calldata except the selector"
        );
    }

    /// Full wrapped-contract lock: `build_flash_funded_broadcast_calldata`
    /// returns the OUTER `requestFlashLoan(address,uint256,bytes)` calldata, with
    /// asset/amount derived from ctx, wrapping the inner
    /// `executeArbitrageFlashFunded(...)` whose 7 args ABI-decode correctly.
    #[test]
    fn flash_funded_broadcast_wraps_request_flash_loan() {
        let (ctx, route_hash, min_profit_wei, executor_address) = fixture();

        let outer = build_flash_funded_broadcast_calldata(
            &ctx,
            route_hash,
            min_profit_wei,
            executor_address,
        )
        .expect("broadcast encoder should produce calldata");

        // Outer selector is requestFlashLoan (0x5107d61e).
        assert_eq!(REQUEST_FLASH_LOAN_SELECTOR, outer[0..4]);

        // Decode the outer requestFlashLoan(address asset, uint256 amount, bytes params).
        let outer_types = [ParamType::Address, ParamType::Uint(256), ParamType::Bytes];
        let outer_decoded =
            decode(&outer_types, &outer[4..]).expect("outer calldata must ABI-decode");
        assert_eq!(outer_decoded.len(), 3, "requestFlashLoan has 3 args");

        // asset == ctx.token_in (the borrowed/repaid token), amount == ctx.amount_in.
        let asset = outer_decoded[0].clone().into_address().expect("asset is address");
        assert_eq!(asset, ctx.token_in, "borrowed asset must be token_in");
        let amount = outer_decoded[1].clone().into_uint().expect("amount is uint256");
        assert_eq!(amount, ctx.amount_in, "borrowed amount must be amount_in");

        // The inner params are the executeArbitrageFlashFunded calldata.
        let inner = outer_decoded[2].clone().into_bytes().expect("params is bytes");
        assert_eq!(
            EXECUTE_ARBITRAGE_FLASH_FUNDED_SELECTOR,
            inner[0..4],
            "inner params must be executeArbitrageFlashFunded (0xdde0bf51)"
        );

        // Decode the inner tail against the same 7-arg executeArbitrage layout.
        let inner_types = [
            ParamType::FixedBytes(32),
            ParamType::Address,
            ParamType::Address,
            ParamType::Uint(256),
            ParamType::Uint(256),
            ParamType::Array(Box::new(ParamType::Address)),
            ParamType::Array(Box::new(ParamType::Bytes)),
        ];
        let inner_decoded =
            decode(&inner_types, &inner[4..]).expect("inner calldata must ABI-decode");
        assert_eq!(inner_decoded.len(), 7, "inner has 7 executeArbitrage args");

        let inner_route_hash =
            inner_decoded[0].clone().into_fixed_bytes().expect("inner arg 0 is bytes32");
        assert_eq!(inner_route_hash.as_slice(), &route_hash[..]);
        let inner_token_in = inner_decoded[1].clone().into_address().expect("inner arg 1 is address");
        assert_eq!(inner_token_in, ctx.token_in);
        let inner_token_out =
            inner_decoded[2].clone().into_address().expect("inner arg 2 is address");
        assert_eq!(inner_token_out, ctx.token_out);
        let inner_amount_in = inner_decoded[3].clone().into_uint().expect("inner arg 3 is uint256");
        assert_eq!(inner_amount_in, ctx.amount_in);
        let inner_min_profit = inner_decoded[4].clone().into_uint().expect("inner arg 4 is uint256");
        assert_eq!(inner_min_profit, min_profit_wei);
        let inner_routers = inner_decoded[5].clone().into_array().expect("inner arg 5 is address[]");
        assert_eq!(inner_routers.len(), 2, "expected 2 routers");
        let inner_payloads = inner_decoded[6].clone().into_array().expect("inner arg 6 is bytes[]");
        assert_eq!(inner_payloads.len(), 2, "expected 2 payload legs");
    }

    /// Decode the inner backward-leg swap payload's `amountIn` (the first arg of
    /// `swapExactTokensForTokens`) out of the wrapped broadcast calldata.
    ///
    /// Layers: outer `requestFlashLoan(address,uint256,bytes)` → inner
    /// `executeArbitrageFlashFunded(...)` arg 6 `bytes[]` → leg-1 (index 1)
    /// `swapExactTokensForTokens(uint256 amountIn, uint256, address[], address, uint256)`.
    fn decode_backward_leg_amount_in(outer: &[u8]) -> U256 {
        let outer_types = [ParamType::Address, ParamType::Uint(256), ParamType::Bytes];
        let outer_decoded = decode(&outer_types, &outer[4..]).expect("outer must ABI-decode");
        let inner = outer_decoded[2].clone().into_bytes().expect("params is bytes");

        let inner_types = [
            ParamType::FixedBytes(32),
            ParamType::Address,
            ParamType::Address,
            ParamType::Uint(256),
            ParamType::Uint(256),
            ParamType::Array(Box::new(ParamType::Address)),
            ParamType::Array(Box::new(ParamType::Bytes)),
        ];
        let inner_decoded = decode(&inner_types, &inner[4..]).expect("inner must ABI-decode");
        let payloads = inner_decoded[6].clone().into_array().expect("inner arg 6 is bytes[]");
        assert_eq!(payloads.len(), 2, "expected forward + backward legs");
        let backward_leg = payloads[1].clone().into_bytes().expect("backward leg is bytes");

        // swapExactTokensForTokens args after its 4-byte selector: arg 0 = amountIn.
        let swap_types = [
            ParamType::Uint(256),
            ParamType::Uint(256),
            ParamType::Array(Box::new(ParamType::Address)),
            ParamType::Address,
            ParamType::Uint(256),
        ];
        let swap_decoded =
            decode(&swap_types, &backward_leg[4..]).expect("backward swap must ABI-decode");
        swap_decoded[0].clone().into_uint().expect("swap arg 0 is amountIn")
    }

    /// The intermediate-aware broadcast encoder puts the REAL backward amount in
    /// the leg-1 swap `amountIn` (NOT the `min_profit_wei` sentinel), while
    /// preserving the outer `0x5107d61e` and inner `0xdde0bf51` selectors.
    #[test]
    fn with_intermediate_encodes_real_backward_amount() {
        let (ctx, route_hash, min_profit_wei, executor_address) = fixture();
        // A distinct intermediate amount (≠ min_profit_wei, ≠ amount_in).
        let backward_amount_in = U256::from(123_456_789_u64);
        assert_ne!(
            backward_amount_in, min_profit_wei,
            "test requires intermediate ≠ sentinel to be meaningful"
        );

        let outer = build_flash_funded_broadcast_calldata_with_intermediate(
            &ctx,
            backward_amount_in,
            route_hash,
            min_profit_wei,
            executor_address,
        )
        .expect("intermediate-aware broadcast encoder should produce calldata");

        // Outer + inner selectors unchanged.
        assert_eq!(REQUEST_FLASH_LOAN_SELECTOR, outer[0..4], "outer selector");
        let outer_types = [ParamType::Address, ParamType::Uint(256), ParamType::Bytes];
        let outer_decoded = decode(&outer_types, &outer[4..]).expect("outer must ABI-decode");
        let inner = outer_decoded[2].clone().into_bytes().expect("params is bytes");
        assert_eq!(
            EXECUTE_ARBITRAGE_FLASH_FUNDED_SELECTOR,
            inner[0..4],
            "inner selector"
        );

        // The backward leg carries the REAL intermediate amount, not the sentinel.
        let encoded_backward = decode_backward_leg_amount_in(&outer);
        assert_eq!(
            encoded_backward, backward_amount_in,
            "backward leg amountIn must equal the supplied intermediate"
        );
        assert_ne!(
            encoded_backward, min_profit_wei,
            "backward leg amountIn must NOT be the min_profit_wei sentinel"
        );
    }

    /// Refactor-preservation lock: feeding the sentinel (`min_profit_wei`) as the
    /// intermediate makes the new encoder BYTE-IDENTICAL to the legacy
    /// `build_flash_funded_broadcast_calldata` — proving the `backward_amount_in`
    /// threading did not change the existing sentinel path.
    #[test]
    fn with_intermediate_sentinel_equals_legacy_broadcast() {
        let (ctx, route_hash, min_profit_wei, executor_address) = fixture();

        let legacy = build_flash_funded_broadcast_calldata(
            &ctx,
            route_hash,
            min_profit_wei,
            executor_address,
        )
        .expect("legacy broadcast encoder should produce calldata");

        let via_intermediate = build_flash_funded_broadcast_calldata_with_intermediate(
            &ctx,
            min_profit_wei, // pass the sentinel as the intermediate
            route_hash,
            min_profit_wei,
            executor_address,
        )
        .expect("intermediate-aware encoder should produce calldata");

        assert_eq!(
            legacy, via_intermediate,
            "passing the sentinel as the intermediate must reproduce the legacy bytes exactly"
        );
    }
}
