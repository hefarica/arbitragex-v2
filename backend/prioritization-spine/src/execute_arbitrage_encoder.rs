//! `executeArbitrage` calldata encoder — single source of truth.
//!
//! Promoted out of `searcher-rs/src/sim_orchestrator.rs` (M2 T1) so that both
//! the simulation path (searcher-rs) and, later, the broadcast path
//! (relays-client) encode the deployed `ArbitrageExecutor.executeArbitrage(...)`
//! calldata from ONE implementation. The emitted bytes are identical to the
//! previous searcher-rs-local encoder.
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

/// Encoder error for [`build_execute_arbitrage_calldata`].
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
    // Backward payload — note the SIMULATION-ONLY caveat above: the
    // intermediate amount of token_out is unknown here. We use
    // `min_profit_wei` as a non-zero placeholder so the calldata is not
    // structurally empty. The contract's router call will almost
    // certainly revert; the orchestrator records that revert honestly.
    let backward_payload = encode_v2_swap_exact_tokens_for_tokens(
        min_profit_wei,
        U256::zero(),
        &ctx.backward_path,
        executor_address,
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
        Token::FixedBytes(route_hash.to_vec()),
        Token::Address(ctx.token_in),
        Token::Address(ctx.token_out),
        Token::Uint(ctx.amount_in),
        Token::Uint(min_profit_wei),
        Token::Array(routers),
        Token::Array(payloads),
    ]);

    let mut calldata = Vec::with_capacity(4 + args.len());
    calldata.extend_from_slice(&EXECUTE_ARBITRAGE_SELECTOR);
    calldata.extend_from_slice(&args);
    Ok(calldata)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn execute_arbitrage_selector_matches_known_hash() {
        use ethers::utils::keccak256;
        let signature =
            b"executeArbitrage(bytes32,address,address,uint256,uint256,address[],bytes[])";
        let hash = keccak256(signature);
        assert_eq!(EXECUTE_ARBITRAGE_SELECTOR, hash[..4]);
    }
}
