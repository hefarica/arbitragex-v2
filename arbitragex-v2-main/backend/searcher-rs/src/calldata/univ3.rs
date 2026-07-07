//! UniswapV3 SwapRouter/SwapRouter02 calldata decoders.
//!
//! Selectors:
//!   exactInputSingle  0x414bf389      (ExactInputSingleParams)
//!   exactInput        0xc04b8d59      (ExactInputParams)
//!   exactOutputSingle 0xdb3e2198
//!   exactOutput       0xf28c0498
//!   multicall         0xac9650d8      (NOT decoded in S2 — flagged unsupported)
//!
//! ## Phase 1 fields
//!
//! All V3 decoders now populate `path_tokens`, `path_fees_bps`, `exact_mode`,
//! and `protocol_type`.
//!
//! For `exactInputSingle` / `exactOutputSingle`:
//!   `path_tokens = [tokenIn, tokenOut]`, `path_fees_bps = [fee / 100]`.
//!
//! For `exactInput` / `exactOutput`:
//!   parsed from the packed path bytes using `calldata::parse_v3_path_bytes_with_fees`.
//!
//! V3 `exactOutput` paths are encoded **reversed** by Uniswap convention (the
//! router traverses them back-to-front internally). We reverse them here so that
//! `path_tokens` is always in execution order: [token_sent, ..., token_received].

use super::{
    parse_v3_path_bytes_with_fees, DecodeFailReason, DecodedSwap, ProtocolType, SwapExactMode,
};
use ethers::abi::{decode as abi_decode, ParamType};

pub fn decode(selector: [u8; 4], body: &[u8]) -> Result<DecodedSwap, DecodeFailReason> {
    match selector {
        [0x41, 0x4b, 0xf3, 0x89] => decode_exact_input_single(body, selector),
        [0xc0, 0x4b, 0x8d, 0x59] => decode_exact_input(body, selector),
        [0xdb, 0x3e, 0x21, 0x98] => decode_exact_output_single(body, selector),
        [0xf2, 0x8c, 0x04, 0x98] => decode_exact_output(body, selector),
        _ => Err(DecodeFailReason::UnsupportedSelector),
    }
}

fn selector_hex(s: [u8; 4]) -> String {
    format!("0x{:02x}{:02x}{:02x}{:02x}", s[0], s[1], s[2], s[3])
}

fn decode_exact_input_single(
    body: &[u8],
    selector: [u8; 4],
) -> Result<DecodedSwap, DecodeFailReason> {
    // struct ExactInputSingleParams {
    //   address tokenIn; address tokenOut; uint24 fee; address recipient;
    //   uint256 deadline; uint256 amountIn; uint256 amountOutMinimum; uint160 sqrtPriceLimitX96;
    // }
    let params = ParamType::Tuple(vec![
        ParamType::Address,
        ParamType::Address,
        ParamType::Uint(24),
        ParamType::Address,
        ParamType::Uint(256),
        ParamType::Uint(256),
        ParamType::Uint(256),
        ParamType::Uint(160),
    ]);
    let tokens = abi_decode(&[params], body).map_err(|_| DecodeFailReason::AbiDecodeError)?;
    let tuple = tokens
        .first()
        .and_then(|t| t.clone().into_tuple())
        .ok_or(DecodeFailReason::AbiDecodeError)?;
    if tuple.len() != 8 {
        return Err(DecodeFailReason::AbiDecodeError);
    }
    let token_in = tuple
        .first()
        .and_then(|t| t.clone().into_address())
        .ok_or(DecodeFailReason::AbiDecodeError)?;
    let token_out = tuple
        .get(1)
        .and_then(|t| t.clone().into_address())
        .ok_or(DecodeFailReason::AbiDecodeError)?;
    let fee_raw = tuple
        .get(2)
        .and_then(|t| t.clone().into_uint())
        .ok_or(DecodeFailReason::AbiDecodeError)?
        .low_u32();
    let recipient = tuple
        .get(3)
        .and_then(|t| t.clone().into_address())
        .ok_or(DecodeFailReason::AbiDecodeError)?;
    let deadline = tuple
        .get(4)
        .and_then(|t| t.clone().into_uint())
        .ok_or(DecodeFailReason::AbiDecodeError)?;
    let amount_in = tuple
        .get(5)
        .and_then(|t| t.clone().into_uint())
        .ok_or(DecodeFailReason::AbiDecodeError)?;
    let min_out = tuple
        .get(6)
        .and_then(|t| t.clone().into_uint())
        .ok_or(DecodeFailReason::AbiDecodeError)?;

    // fee_raw is in V3 units (e.g. 3000 = 0.3%) → convert to basis points (/100).
    let fee_bps = fee_raw / 100;

    Ok(DecodedSwap {
        router: "uniswap-v3",
        token_in,
        token_out,
        amount_in,
        min_amount_out: min_out,
        path_len: 2,
        deadline,
        recipient,
        selector_hex: selector_hex(selector),
        path_tokens: vec![token_in, token_out],
        path_fees_bps: vec![fee_bps],
        exact_mode: SwapExactMode::ExactIn,
        protocol_type: ProtocolType::V3,
    })
}

fn decode_exact_input(body: &[u8], selector: [u8; 4]) -> Result<DecodedSwap, DecodeFailReason> {
    // struct ExactInputParams { bytes path; address recipient; uint256 deadline;
    //                           uint256 amountIn; uint256 amountOutMinimum; }
    let params = ParamType::Tuple(vec![
        ParamType::Bytes,
        ParamType::Address,
        ParamType::Uint(256),
        ParamType::Uint(256),
        ParamType::Uint(256),
    ]);
    let tokens = abi_decode(&[params], body).map_err(|_| DecodeFailReason::AbiDecodeError)?;
    let tuple = tokens
        .first()
        .and_then(|t| t.clone().into_tuple())
        .ok_or(DecodeFailReason::AbiDecodeError)?;
    if tuple.len() != 5 {
        return Err(DecodeFailReason::AbiDecodeError);
    }
    let path_bytes = tuple
        .first()
        .and_then(|t| t.clone().into_bytes())
        .ok_or(DecodeFailReason::AbiDecodeError)?;

    // V3 path encoding: tokenIn(20) | fee(3) | tokenOut(20) | fee(3) | token(20) ...
    if path_bytes.len() < 43 {
        return Err(DecodeFailReason::AbiDecodeError);
    }

    // Parse full path + per-hop fees.
    let (path_tokens, path_fees_bps) =
        parse_v3_path_bytes_with_fees(&path_bytes).ok_or(DecodeFailReason::AbiDecodeError)?;

    let token_in_bytes: [u8; 20] = path_bytes[0..20]
        .try_into()
        .map_err(|_| DecodeFailReason::AbiDecodeError)?;
    let token_out_bytes: [u8; 20] = path_bytes[path_bytes.len() - 20..]
        .try_into()
        .map_err(|_| DecodeFailReason::AbiDecodeError)?;

    let recipient = tuple
        .get(1)
        .and_then(|t| t.clone().into_address())
        .ok_or(DecodeFailReason::AbiDecodeError)?;
    let deadline = tuple
        .get(2)
        .and_then(|t| t.clone().into_uint())
        .ok_or(DecodeFailReason::AbiDecodeError)?;
    let amount_in = tuple
        .get(3)
        .and_then(|t| t.clone().into_uint())
        .ok_or(DecodeFailReason::AbiDecodeError)?;
    let min_out = tuple
        .get(4)
        .and_then(|t| t.clone().into_uint())
        .ok_or(DecodeFailReason::AbiDecodeError)?;

    let hop_count = path_tokens.len() - 1;

    Ok(DecodedSwap {
        router: "uniswap-v3",
        token_in: token_in_bytes.into(),
        token_out: token_out_bytes.into(),
        amount_in,
        min_amount_out: min_out,
        path_len: (hop_count + 1) as u32,
        deadline,
        recipient,
        selector_hex: selector_hex(selector),
        path_tokens,
        path_fees_bps,
        exact_mode: SwapExactMode::ExactIn,
        protocol_type: ProtocolType::V3,
    })
}

fn decode_exact_output_single(
    body: &[u8],
    selector: [u8; 4],
) -> Result<DecodedSwap, DecodeFailReason> {
    // struct ExactOutputSingleParams {
    //   address tokenIn; address tokenOut; uint24 fee; address recipient;
    //   uint256 deadline; uint256 amountOut; uint256 amountInMaximum; uint160 sqrtPriceLimitX96;
    // }
    // Note: tokenIn/tokenOut in this struct are in execution-order (NOT reversed),
    // so no reversal needed for exactOutputSingle.
    let params = ParamType::Tuple(vec![
        ParamType::Address,
        ParamType::Address,
        ParamType::Uint(24),
        ParamType::Address,
        ParamType::Uint(256),
        ParamType::Uint(256),
        ParamType::Uint(256),
        ParamType::Uint(160),
    ]);
    let tokens = abi_decode(&[params], body).map_err(|_| DecodeFailReason::AbiDecodeError)?;
    let tuple = tokens
        .first()
        .and_then(|t| t.clone().into_tuple())
        .ok_or(DecodeFailReason::AbiDecodeError)?;
    if tuple.len() != 8 {
        return Err(DecodeFailReason::AbiDecodeError);
    }
    let token_in = tuple
        .first()
        .and_then(|t| t.clone().into_address())
        .ok_or(DecodeFailReason::AbiDecodeError)?;
    let token_out = tuple
        .get(1)
        .and_then(|t| t.clone().into_address())
        .ok_or(DecodeFailReason::AbiDecodeError)?;
    let fee_raw = tuple
        .get(2)
        .and_then(|t| t.clone().into_uint())
        .ok_or(DecodeFailReason::AbiDecodeError)?
        .low_u32();
    let recipient = tuple
        .get(3)
        .and_then(|t| t.clone().into_address())
        .ok_or(DecodeFailReason::AbiDecodeError)?;
    let deadline = tuple
        .get(4)
        .and_then(|t| t.clone().into_uint())
        .ok_or(DecodeFailReason::AbiDecodeError)?;
    // amountOut is the exact output (index 5); amountInMaximum is the bound (index 6).
    let amount_out = tuple
        .get(5)
        .and_then(|t| t.clone().into_uint())
        .ok_or(DecodeFailReason::AbiDecodeError)?;
    let amount_in_max = tuple
        .get(6)
        .and_then(|t| t.clone().into_uint())
        .ok_or(DecodeFailReason::AbiDecodeError)?;

    let fee_bps = fee_raw / 100;

    Ok(DecodedSwap {
        router: "uniswap-v3",
        token_in,
        token_out,
        amount_in: amount_in_max,
        min_amount_out: amount_out,
        path_len: 2,
        deadline,
        recipient,
        selector_hex: selector_hex(selector),
        path_tokens: vec![token_in, token_out],
        path_fees_bps: vec![fee_bps],
        exact_mode: SwapExactMode::ExactOut,
        protocol_type: ProtocolType::V3,
    })
}

fn decode_exact_output(body: &[u8], selector: [u8; 4]) -> Result<DecodedSwap, DecodeFailReason> {
    // struct ExactOutputParams { bytes path; address recipient; uint256 deadline;
    //                            uint256 amountOut; uint256 amountInMaximum; }
    //
    // IMPORTANT: V3 exactOutput encodes the path in REVERSED order (tokenOut first,
    // tokenIn last) because the router traverses it back-to-front internally.
    // We reverse the parsed path here so that `path_tokens` is always in
    // execution order: [token_sent, ..., token_received].
    // Correspondingly, the fees array is also reversed so index i corresponds
    // to the hop between path_tokens[i] and path_tokens[i+1].
    let params = ParamType::Tuple(vec![
        ParamType::Bytes,
        ParamType::Address,
        ParamType::Uint(256),
        ParamType::Uint(256),
        ParamType::Uint(256),
    ]);
    let tokens = abi_decode(&[params], body).map_err(|_| DecodeFailReason::AbiDecodeError)?;
    let tuple = tokens
        .first()
        .and_then(|t| t.clone().into_tuple())
        .ok_or(DecodeFailReason::AbiDecodeError)?;
    if tuple.len() != 5 {
        return Err(DecodeFailReason::AbiDecodeError);
    }
    let path_bytes = tuple
        .first()
        .and_then(|t| t.clone().into_bytes())
        .ok_or(DecodeFailReason::AbiDecodeError)?;

    if path_bytes.len() < 43 {
        return Err(DecodeFailReason::AbiDecodeError);
    }

    // Parse in encoded order (reversed), then reverse for execution order.
    let (mut path_tokens, mut path_fees_bps) =
        parse_v3_path_bytes_with_fees(&path_bytes).ok_or(DecodeFailReason::AbiDecodeError)?;

    // The encoded path is [tokenOut, ..., tokenIn] — reverse to execution order.
    path_tokens.reverse();
    path_fees_bps.reverse();

    let recipient = tuple
        .get(1)
        .and_then(|t| t.clone().into_address())
        .ok_or(DecodeFailReason::AbiDecodeError)?;
    let deadline = tuple
        .get(2)
        .and_then(|t| t.clone().into_uint())
        .ok_or(DecodeFailReason::AbiDecodeError)?;
    let amount_out = tuple
        .get(3)
        .and_then(|t| t.clone().into_uint())
        .ok_or(DecodeFailReason::AbiDecodeError)?;
    let amount_in_max = tuple
        .get(4)
        .and_then(|t| t.clone().into_uint())
        .ok_or(DecodeFailReason::AbiDecodeError)?;

    // After reversal: path_tokens[0] = tokenIn, path_tokens.last() = tokenOut.
    let token_in = *path_tokens
        .first()
        .ok_or(DecodeFailReason::AbiDecodeError)?;
    let token_out = *path_tokens.last().ok_or(DecodeFailReason::AbiDecodeError)?;
    let hop_count = path_tokens.len() - 1;

    Ok(DecodedSwap {
        router: "uniswap-v3",
        token_in,
        token_out,
        amount_in: amount_in_max,
        min_amount_out: amount_out,
        path_len: (hop_count + 1) as u32,
        deadline,
        recipient,
        selector_hex: selector_hex(selector),
        path_tokens,
        path_fees_bps,
        exact_mode: SwapExactMode::ExactOut,
        protocol_type: ProtocolType::V3,
    })
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use ethers::abi::{encode, Token};
    use ethers::types::{Address, U256};

    fn addr(n: u64) -> Address {
        Address::from_low_u64_be(n)
    }

    fn v3_exact_input_single_body(
        token_in: Address,
        token_out: Address,
        fee: u32,
        amount_in: u64,
    ) -> Vec<u8> {
        encode(&[Token::Tuple(vec![
            Token::Address(token_in),
            Token::Address(token_out),
            Token::Uint(U256::from(fee)),
            Token::Address(addr(0xDD)),
            Token::Uint(U256::from(0xdeadu32)),
            Token::Uint(U256::from(amount_in)),
            Token::Uint(U256::from(950u64)),
            Token::Uint(U256::zero()),
        ])])
    }

    fn build_v3_packed_path(tokens: &[Address], fees_raw: &[u32]) -> Vec<u8> {
        // fees_raw.len() == tokens.len() - 1
        let mut p = Vec::new();
        p.extend_from_slice(tokens[0].as_bytes());
        for (i, &fee) in fees_raw.iter().enumerate() {
            // fee as 3-byte big-endian uint24
            p.push((fee >> 16) as u8);
            p.push((fee >> 8) as u8);
            p.push(fee as u8);
            p.extend_from_slice(tokens[i + 1].as_bytes());
        }
        p
    }

    // ── exactInputSingle: fee tier preserved in bps ──────────────────────────

    #[test]
    fn exact_input_single_fee_500_gives_5_bps() {
        let tin = addr(0x1);
        let tout = addr(0x2);
        let body = v3_exact_input_single_body(tin, tout, 500, 1_000);
        let decoded = decode([0x41, 0x4b, 0xf3, 0x89], &body).unwrap();

        assert_eq!(decoded.path_tokens, vec![tin, tout]);
        assert_eq!(decoded.path_fees_bps, vec![5]); // 500 / 100 = 5 bps
        assert_eq!(decoded.exact_mode, SwapExactMode::ExactIn);
        assert_eq!(decoded.protocol_type, ProtocolType::V3);
    }

    #[test]
    fn exact_input_single_fee_3000_gives_30_bps() {
        let tin = addr(0x1);
        let tout = addr(0x2);
        let body = v3_exact_input_single_body(tin, tout, 3000, 2_000);
        let decoded = decode([0x41, 0x4b, 0xf3, 0x89], &body).unwrap();

        assert_eq!(decoded.path_fees_bps, vec![30]);
    }

    #[test]
    fn exact_input_single_fee_10000_gives_100_bps() {
        let tin = addr(0x1);
        let tout = addr(0x2);
        let body = v3_exact_input_single_body(tin, tout, 10000, 3_000);
        let decoded = decode([0x41, 0x4b, 0xf3, 0x89], &body).unwrap();

        assert_eq!(decoded.path_fees_bps, vec![100]);
    }

    // ── exactInput multi-hop: path_tokens + path_fees_bps in order ──────────

    #[test]
    fn exact_input_multi_hop_three_tokens_two_fees() {
        let ta = addr(0xAA);
        let tb = addr(0xBB);
        let tc = addr(0xCC);
        let path_bytes = build_v3_packed_path(&[ta, tb, tc], &[3000, 500]);
        let body = encode(&[Token::Tuple(vec![
            Token::Bytes(path_bytes),
            Token::Address(addr(0xEE)),
            Token::Uint(U256::from(0u32)),
            Token::Uint(U256::from(1_000u64)),
            Token::Uint(U256::from(950u64)),
        ])]);
        let decoded = decode([0xc0, 0x4b, 0x8d, 0x59], &body).unwrap();

        assert_eq!(decoded.path_tokens, vec![ta, tb, tc]);
        assert_eq!(decoded.path_fees_bps, vec![30, 5]); // 3000/100=30, 500/100=5
        assert_eq!(decoded.exact_mode, SwapExactMode::ExactIn);
        assert_eq!(decoded.protocol_type, ProtocolType::V3);
    }

    // ── exactOutputSingle: ExactOut mode, fee preserved ─────────────────────

    #[test]
    fn exact_output_single_fee_and_mode() {
        let tin = addr(0x1);
        let tout = addr(0x2);
        let body = encode(&[Token::Tuple(vec![
            Token::Address(tin),
            Token::Address(tout),
            Token::Uint(U256::from(3000u32)),
            Token::Address(addr(0xDD)),
            Token::Uint(U256::from(0xdeadu32)),
            Token::Uint(U256::from(1_000u64)), // amountOut
            Token::Uint(U256::from(1_100u64)), // amountInMaximum
            Token::Uint(U256::zero()),
        ])]);
        let decoded = decode([0xdb, 0x3e, 0x21, 0x98], &body).unwrap();

        assert_eq!(decoded.path_tokens, vec![tin, tout]);
        assert_eq!(decoded.path_fees_bps, vec![30]);
        assert_eq!(decoded.exact_mode, SwapExactMode::ExactOut);
        assert_eq!(decoded.protocol_type, ProtocolType::V3);
    }

    // ── exactOutput: path reversed to execution order ────────────────────────

    #[test]
    fn exact_output_path_reversed_to_execution_order() {
        // V3 exactOutput encodes path as [tokenOut, ..., tokenIn] (reversed).
        // We encode [C, B, A] (reversed), decoder should output [A, B, C] (execution).
        let ta = addr(0xAA);
        let tb = addr(0xBB);
        let tc = addr(0xCC);
        // Encoded path: [C → B → A] (reversed V3 convention)
        let path_bytes_reversed = build_v3_packed_path(&[tc, tb, ta], &[500, 3000]);
        let body = encode(&[Token::Tuple(vec![
            Token::Bytes(path_bytes_reversed),
            Token::Address(addr(0xEE)),
            Token::Uint(U256::from(0u32)),
            Token::Uint(U256::from(900u64)),   // amountOut
            Token::Uint(U256::from(1_000u64)), // amountInMaximum
        ])]);
        let decoded = decode([0xf2, 0x8c, 0x04, 0x98], &body).unwrap();

        // After reversal: execution order [A, B, C]
        assert_eq!(decoded.path_tokens, vec![ta, tb, tc]);
        // Fees also reversed: original [500, 3000] → reversed to hops A→B (3000) and B→C (500)
        assert_eq!(decoded.path_fees_bps, vec![30, 5]);
        assert_eq!(decoded.exact_mode, SwapExactMode::ExactOut);
        assert_eq!(decoded.token_in, ta);
        assert_eq!(decoded.token_out, tc);
    }

    // ── path_fees_bps.len() == path_tokens.len() - 1 for all selectors ───────

    #[test]
    fn fee_bps_length_invariant_all_selectors() {
        // exactInputSingle
        {
            let body = v3_exact_input_single_body(addr(0x1), addr(0x2), 3000, 1_000);
            let d = decode([0x41, 0x4b, 0xf3, 0x89], &body).unwrap();
            assert_eq!(d.path_fees_bps.len(), d.path_tokens.len() - 1);
        }
        // exactInput multi-hop
        {
            let path_bytes = build_v3_packed_path(&[addr(0xA), addr(0xB), addr(0xC)], &[3000, 500]);
            let body = encode(&[Token::Tuple(vec![
                Token::Bytes(path_bytes),
                Token::Address(addr(0xEE)),
                Token::Uint(U256::from(0u32)),
                Token::Uint(U256::from(1_000u64)),
                Token::Uint(U256::from(950u64)),
            ])]);
            let d = decode([0xc0, 0x4b, 0x8d, 0x59], &body).unwrap();
            assert_eq!(d.path_fees_bps.len(), d.path_tokens.len() - 1);
        }
    }

    // ── Incomplete calldata returns AbiDecodeError, never panics ─────────────

    #[test]
    fn incomplete_calldata_returns_abi_decode_error() {
        let truncated = vec![0x00u8; 10]; // far too short to be a valid tuple
        let result = decode([0x41, 0x4b, 0xf3, 0x89], &truncated);
        assert!(
            matches!(result, Err(DecodeFailReason::AbiDecodeError)),
            "truncated input must return AbiDecodeError"
        );
    }

    #[test]
    fn unsupported_selector_returns_error() {
        let result = decode([0xde, 0xad, 0xbe, 0xef], &[]);
        assert!(matches!(result, Err(DecodeFailReason::UnsupportedSelector)));
    }
}
