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

    /// Output-contract lock for the M2 broadcast path: the encoder must emit a
    /// well-formed `executeArbitrage(bytes32,address,address,uint256,uint256,address[],bytes[])`
    /// call whose ABI-decoded fields match the inputs. This guards both the
    /// selector (0x76d81cdf — NOT the `executeArbitrageFlashFunded` 0xdde0bf51
    /// sibling) and the argument layout the deployed contract relies on.
    #[test]
    fn build_calldata_matches_execute_arbitrage_abi() {
        use ethers::abi::{decode, ParamType};
        use std::str::FromStr;

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
}
