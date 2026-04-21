//! UniswapV3 SwapRouter/SwapRouter02 calldata decoders.
//!
//! Selectors:
//!   exactInputSingle  0x414bf389      (ExactInputSingleParams)
//!   exactInput        0xc04b8d59      (ExactInputParams)
//!   exactOutputSingle 0xdb3e2198
//!   exactOutput       0xf28c0498
//!   multicall         0xac9650d8      (NOT decoded in S2 — flagged unsupported)

use super::{DecodeFailReason, DecodedSwap};
use ethers::abi::{decode as abi_decode, ParamType};
use ethers::types::U256;

pub fn decode(selector: [u8; 4], body: &[u8]) -> Result<DecodedSwap, DecodeFailReason> {
    match selector {
        [0x41, 0x4b, 0xf3, 0x89] => decode_exact_input_single(body, selector),
        [0xc0, 0x4b, 0x8d, 0x59] => decode_exact_input(body, selector),
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
    let tuple = tokens.first().and_then(|t| t.clone().into_tuple()).ok_or(DecodeFailReason::AbiDecodeError)?;
    if tuple.len() != 8 {
        return Err(DecodeFailReason::AbiDecodeError);
    }
    let token_in = tuple.first().and_then(|t| t.clone().into_address()).ok_or(DecodeFailReason::AbiDecodeError)?;
    let token_out = tuple.get(1).and_then(|t| t.clone().into_address()).ok_or(DecodeFailReason::AbiDecodeError)?;
    let recipient = tuple.get(3).and_then(|t| t.clone().into_address()).ok_or(DecodeFailReason::AbiDecodeError)?;
    let deadline = tuple.get(4).and_then(|t| t.clone().into_uint()).ok_or(DecodeFailReason::AbiDecodeError)?;
    let amount_in = tuple.get(5).and_then(|t| t.clone().into_uint()).ok_or(DecodeFailReason::AbiDecodeError)?;
    let min_out   = tuple.get(6).and_then(|t| t.clone().into_uint()).ok_or(DecodeFailReason::AbiDecodeError)?;
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
    })
}

fn decode_exact_input(
    body: &[u8],
    selector: [u8; 4],
) -> Result<DecodedSwap, DecodeFailReason> {
    // struct ExactInputParams { bytes path; address recipient; uint256 deadline; uint256 amountIn; uint256 amountOutMinimum; }
    let params = ParamType::Tuple(vec![
        ParamType::Bytes,
        ParamType::Address,
        ParamType::Uint(256),
        ParamType::Uint(256),
        ParamType::Uint(256),
    ]);
    let tokens = abi_decode(&[params], body).map_err(|_| DecodeFailReason::AbiDecodeError)?;
    let tuple = tokens.first().and_then(|t| t.clone().into_tuple()).ok_or(DecodeFailReason::AbiDecodeError)?;
    if tuple.len() != 5 {
        return Err(DecodeFailReason::AbiDecodeError);
    }
    let path_bytes = tuple.first().and_then(|t| t.clone().into_bytes()).ok_or(DecodeFailReason::AbiDecodeError)?;
    // V3 path encoding: tokenIn(20) | fee(3) | tokenOut(20) | fee(3) | token(20) ...
    if path_bytes.len() < 43 {
        return Err(DecodeFailReason::AbiDecodeError);
    }
    let token_in_bytes: [u8; 20] = path_bytes[0..20].try_into().map_err(|_| DecodeFailReason::AbiDecodeError)?;
    let token_out_bytes: [u8; 20] = path_bytes[path_bytes.len() - 20..].try_into().map_err(|_| DecodeFailReason::AbiDecodeError)?;
    let hops = (path_bytes.len() - 20) / 23;

    let recipient = tuple.get(1).and_then(|t| t.clone().into_address()).ok_or(DecodeFailReason::AbiDecodeError)?;
    let deadline  = tuple.get(2).and_then(|t| t.clone().into_uint()).ok_or(DecodeFailReason::AbiDecodeError)?;
    let amount_in = tuple.get(3).and_then(|t| t.clone().into_uint()).ok_or(DecodeFailReason::AbiDecodeError)?;
    let min_out   = tuple.get(4).and_then(|t| t.clone().into_uint()).ok_or(DecodeFailReason::AbiDecodeError)?;

    Ok(DecodedSwap {
        router: "uniswap-v3",
        token_in: token_in_bytes.into(),
        token_out: token_out_bytes.into(),
        amount_in,
        min_amount_out: min_out,
        path_len: (hops + 1) as u32,
        deadline,
        recipient,
        selector_hex: selector_hex(selector),
    })
}

