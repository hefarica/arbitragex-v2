//! UniswapV2 router calldata decoders.
//!
//! Selectors (from Uniswap V2 Router02 ABI):
//!   swapExactTokensForTokens              0x38ed1739
//!   swapTokensForExactTokens              0x8803dbee
//!   swapExactETHForTokens                 0x7ff36ab5
//!   swapExactTokensForETH                 0x18cbafe5
//!   swapTokensForExactETH                 0x4a25d94a
//!   swapETHForExactTokens                 0xfb3bdb41
//!   swapExactTokensForTokensSupportingFeeOnTransferTokens  0x5c11d795
//!   swapExactETHForTokensSupportingFeeOnTransferTokens     0xb6f9de95
//!   swapExactTokensForETHSupportingFeeOnTransferTokens     0x791ac947

use super::{DecodeFailReason, DecodedSwap};
use ethers::abi::{decode as abi_decode, ParamType};
use ethers::types::{Address, U256};

const WETH_MAINNET: [u8; 20] = [
    0xc0, 0x2a, 0xaa, 0x39, 0xb2, 0x23, 0xfe, 0x8d, 0x0a, 0x0e,
    0x5c, 0x4f, 0x27, 0xea, 0xd9, 0x08, 0x3c, 0x75, 0x6c, 0xc2,
];

pub fn decode(selector: [u8; 4], body: &[u8]) -> Result<DecodedSwap, DecodeFailReason> {
    match selector {
        // swapExactTokensForTokens(uint256,uint256,address[],address,uint256)
        [0x38, 0xed, 0x17, 0x39] => decode_exact_in_tokens_for_tokens(body, selector, false),
        // swapExactTokensForTokensSupportingFeeOnTransferTokens
        [0x5c, 0x11, 0xd7, 0x95] => decode_exact_in_tokens_for_tokens(body, selector, false),
        // swapTokensForExactTokens(uint256,uint256,address[],address,uint256)
        [0x88, 0x03, 0xdb, 0xee] => decode_exact_out_tokens_for_tokens(body, selector),
        // swapExactETHForTokens(uint256,address[],address,uint256)
        [0x7f, 0xf3, 0x6a, 0xb5] => decode_exact_in_eth_for_tokens(body, selector),
        // swapExactETHForTokensSupportingFeeOnTransferTokens
        [0xb6, 0xf9, 0xde, 0x95] => decode_exact_in_eth_for_tokens(body, selector),
        // swapExactTokensForETH(uint256,uint256,address[],address,uint256)
        [0x18, 0xcb, 0xaf, 0xe5] => decode_exact_in_tokens_for_tokens(body, selector, true),
        // swapExactTokensForETHSupportingFeeOnTransferTokens
        [0x79, 0x1a, 0xc9, 0x47] => decode_exact_in_tokens_for_tokens(body, selector, true),
        _ => Err(DecodeFailReason::UnsupportedSelector),
    }
}

fn selector_hex(s: [u8; 4]) -> String {
    format!("0x{:02x}{:02x}{:02x}{:02x}", s[0], s[1], s[2], s[3])
}

fn decode_exact_in_tokens_for_tokens(
    body: &[u8],
    selector: [u8; 4],
    _eth_out: bool,
) -> Result<DecodedSwap, DecodeFailReason> {
    // (uint256 amountIn, uint256 amountOutMin, address[] path, address to, uint256 deadline)
    let tokens = abi_decode(
        &[
            ParamType::Uint(256),
            ParamType::Uint(256),
            ParamType::Array(Box::new(ParamType::Address)),
            ParamType::Address,
            ParamType::Uint(256),
        ],
        body,
    )
    .map_err(|_| DecodeFailReason::AbiDecodeError)?;

    let amount_in = tokens.first().and_then(|t| t.clone().into_uint()).ok_or(DecodeFailReason::AbiDecodeError)?;
    let min_out   = tokens.get(1).and_then(|t| t.clone().into_uint()).ok_or(DecodeFailReason::AbiDecodeError)?;
    let path      = tokens.get(2).and_then(|t| t.clone().into_array()).ok_or(DecodeFailReason::AbiDecodeError)?;
    let to        = tokens.get(3).and_then(|t| t.clone().into_address()).ok_or(DecodeFailReason::AbiDecodeError)?;
    let deadline  = tokens.get(4).and_then(|t| t.clone().into_uint()).ok_or(DecodeFailReason::AbiDecodeError)?;

    if path.len() < 2 {
        return Err(DecodeFailReason::AbiDecodeError);
    }
    let token_in  = path.first().and_then(|t| t.clone().into_address()).ok_or(DecodeFailReason::AbiDecodeError)?;
    let token_out = path.last().and_then(|t| t.clone().into_address()).ok_or(DecodeFailReason::AbiDecodeError)?;

    Ok(DecodedSwap {
        router: "uniswap-v2",
        token_in,
        token_out,
        amount_in,
        min_amount_out: min_out,
        path_len: path.len() as u32,
        deadline,
        recipient: to,
        selector_hex: selector_hex(selector),
    })
}

fn decode_exact_out_tokens_for_tokens(
    body: &[u8],
    selector: [u8; 4],
) -> Result<DecodedSwap, DecodeFailReason> {
    // (uint256 amountOut, uint256 amountInMax, address[] path, address to, uint256 deadline)
    let tokens = abi_decode(
        &[
            ParamType::Uint(256),
            ParamType::Uint(256),
            ParamType::Array(Box::new(ParamType::Address)),
            ParamType::Address,
            ParamType::Uint(256),
        ],
        body,
    )
    .map_err(|_| DecodeFailReason::AbiDecodeError)?;
    let amount_out  = tokens.first().and_then(|t| t.clone().into_uint()).ok_or(DecodeFailReason::AbiDecodeError)?;
    let amount_in_max = tokens.get(1).and_then(|t| t.clone().into_uint()).ok_or(DecodeFailReason::AbiDecodeError)?;
    let path = tokens.get(2).and_then(|t| t.clone().into_array()).ok_or(DecodeFailReason::AbiDecodeError)?;
    let to   = tokens.get(3).and_then(|t| t.clone().into_address()).ok_or(DecodeFailReason::AbiDecodeError)?;
    let deadline = tokens.get(4).and_then(|t| t.clone().into_uint()).ok_or(DecodeFailReason::AbiDecodeError)?;

    if path.len() < 2 {
        return Err(DecodeFailReason::AbiDecodeError);
    }
    let token_in  = path.first().and_then(|t| t.clone().into_address()).ok_or(DecodeFailReason::AbiDecodeError)?;
    let token_out = path.last().and_then(|t| t.clone().into_address()).ok_or(DecodeFailReason::AbiDecodeError)?;

    Ok(DecodedSwap {
        router: "uniswap-v2",
        token_in,
        token_out,
        amount_in: amount_in_max,
        min_amount_out: amount_out,
        path_len: path.len() as u32,
        deadline,
        recipient: to,
        selector_hex: selector_hex(selector),
    })
}

fn decode_exact_in_eth_for_tokens(
    body: &[u8],
    selector: [u8; 4],
) -> Result<DecodedSwap, DecodeFailReason> {
    // (uint256 amountOutMin, address[] path, address to, uint256 deadline); amountIn = msg.value (from tx.value)
    let tokens = abi_decode(
        &[
            ParamType::Uint(256),
            ParamType::Array(Box::new(ParamType::Address)),
            ParamType::Address,
            ParamType::Uint(256),
        ],
        body,
    )
    .map_err(|_| DecodeFailReason::AbiDecodeError)?;
    let min_out   = tokens.first().and_then(|t| t.clone().into_uint()).ok_or(DecodeFailReason::AbiDecodeError)?;
    let path      = tokens.get(1).and_then(|t| t.clone().into_array()).ok_or(DecodeFailReason::AbiDecodeError)?;
    let to        = tokens.get(2).and_then(|t| t.clone().into_address()).ok_or(DecodeFailReason::AbiDecodeError)?;
    let deadline  = tokens.get(3).and_then(|t| t.clone().into_uint()).ok_or(DecodeFailReason::AbiDecodeError)?;
    if path.len() < 2 {
        return Err(DecodeFailReason::AbiDecodeError);
    }
    // amount_in comes from tx.value; caller fills this post-decode. For now we record 0 and note it.
    let token_in  = path.first().and_then(|t| t.clone().into_address()).ok_or(DecodeFailReason::AbiDecodeError)?;
    // Normally path[0] is WETH for these ETH→tokens functions; left as-is.
    let _weth = Address::from(WETH_MAINNET);
    let token_out = path.last().and_then(|t| t.clone().into_address()).ok_or(DecodeFailReason::AbiDecodeError)?;
    Ok(DecodedSwap {
        router: "uniswap-v2",
        token_in,
        token_out,
        amount_in: U256::zero(), // filled by caller from tx.value
        min_amount_out: min_out,
        path_len: path.len() as u32,
        deadline,
        recipient: to,
        selector_hex: selector_hex(selector),
    })
}
