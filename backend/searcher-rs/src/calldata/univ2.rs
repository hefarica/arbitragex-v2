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
//!
//! ## Phase 1 fields
//!
//! All V2 decoders now populate `path_tokens` (full address array from calldata),
//! `path_fees_bps` (30 bps per hop — Uniswap V2 protocol invariant, R8 exception),
//! `exact_mode` (ExactIn / ExactOut per selector), and `protocol_type` (V2).

use super::{DecodeFailReason, DecodedSwap, ProtocolType, SwapExactMode};
use ethers::abi::{decode as abi_decode, ParamType, Token};
use ethers::types::{Address, U256};

/// Uniswap V2 fixed LP fee in basis points (0.30%).
/// This is a protocol-level constant, not fabricated — R8 exception documented.
const V2_FEE_BPS: u32 = 30;

const WETH_MAINNET: [u8; 20] = [
    0xc0, 0x2a, 0xaa, 0x39, 0xb2, 0x23, 0xfe, 0x8d, 0x0a, 0x0e, 0x5c, 0x4f, 0x27, 0xea, 0xd9, 0x08,
    0x3c, 0x75, 0x6c, 0xc2,
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

/// Extract ordered `Vec<Address>` from an ABI-encoded `address[]` token.
fn extract_path(path_token: &Token) -> Option<Vec<Address>> {
    let arr = path_token.clone().into_array()?;
    arr.into_iter().map(|t| t.into_address()).collect()
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

    let amount_in = tokens
        .first()
        .and_then(|t| t.clone().into_uint())
        .ok_or(DecodeFailReason::AbiDecodeError)?;
    let min_out = tokens
        .get(1)
        .and_then(|t| t.clone().into_uint())
        .ok_or(DecodeFailReason::AbiDecodeError)?;
    let path_token = tokens.get(2).ok_or(DecodeFailReason::AbiDecodeError)?;
    let path = extract_path(path_token).ok_or(DecodeFailReason::AbiDecodeError)?;
    let to = tokens
        .get(3)
        .and_then(|t| t.clone().into_address())
        .ok_or(DecodeFailReason::AbiDecodeError)?;
    let deadline = tokens
        .get(4)
        .and_then(|t| t.clone().into_uint())
        .ok_or(DecodeFailReason::AbiDecodeError)?;

    if path.len() < 2 {
        return Err(DecodeFailReason::AbiDecodeError);
    }
    let token_in = *path.first().ok_or(DecodeFailReason::AbiDecodeError)?;
    let token_out = *path.last().ok_or(DecodeFailReason::AbiDecodeError)?;
    let hop_count = path.len() - 1;

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
        path_tokens: path,
        path_fees_bps: vec![V2_FEE_BPS; hop_count],
        exact_mode: SwapExactMode::ExactIn,
        protocol_type: ProtocolType::V2,
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
    let amount_out = tokens
        .first()
        .and_then(|t| t.clone().into_uint())
        .ok_or(DecodeFailReason::AbiDecodeError)?;
    let amount_in_max = tokens
        .get(1)
        .and_then(|t| t.clone().into_uint())
        .ok_or(DecodeFailReason::AbiDecodeError)?;
    let path_token = tokens.get(2).ok_or(DecodeFailReason::AbiDecodeError)?;
    let path = extract_path(path_token).ok_or(DecodeFailReason::AbiDecodeError)?;
    let to = tokens
        .get(3)
        .and_then(|t| t.clone().into_address())
        .ok_or(DecodeFailReason::AbiDecodeError)?;
    let deadline = tokens
        .get(4)
        .and_then(|t| t.clone().into_uint())
        .ok_or(DecodeFailReason::AbiDecodeError)?;

    if path.len() < 2 {
        return Err(DecodeFailReason::AbiDecodeError);
    }
    let token_in = *path.first().ok_or(DecodeFailReason::AbiDecodeError)?;
    let token_out = *path.last().ok_or(DecodeFailReason::AbiDecodeError)?;
    let hop_count = path.len() - 1;

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
        path_tokens: path,
        path_fees_bps: vec![V2_FEE_BPS; hop_count],
        exact_mode: SwapExactMode::ExactOut,
        protocol_type: ProtocolType::V2,
    })
}

fn decode_exact_in_eth_for_tokens(
    body: &[u8],
    selector: [u8; 4],
) -> Result<DecodedSwap, DecodeFailReason> {
    // (uint256 amountOutMin, address[] path, address to, uint256 deadline); amountIn = msg.value
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
    let min_out = tokens
        .first()
        .and_then(|t| t.clone().into_uint())
        .ok_or(DecodeFailReason::AbiDecodeError)?;
    let path_token = tokens.get(1).ok_or(DecodeFailReason::AbiDecodeError)?;
    let path = extract_path(path_token).ok_or(DecodeFailReason::AbiDecodeError)?;
    let to = tokens
        .get(2)
        .and_then(|t| t.clone().into_address())
        .ok_or(DecodeFailReason::AbiDecodeError)?;
    let deadline = tokens
        .get(3)
        .and_then(|t| t.clone().into_uint())
        .ok_or(DecodeFailReason::AbiDecodeError)?;
    if path.len() < 2 {
        return Err(DecodeFailReason::AbiDecodeError);
    }
    // path[0] is WETH for ETH→tokens swaps.
    let _weth = Address::from(WETH_MAINNET);
    let token_in = *path.first().ok_or(DecodeFailReason::AbiDecodeError)?;
    let token_out = *path.last().ok_or(DecodeFailReason::AbiDecodeError)?;
    let hop_count = path.len() - 1;

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
        path_tokens: path,
        path_fees_bps: vec![V2_FEE_BPS; hop_count],
        exact_mode: SwapExactMode::ExactIn,
        protocol_type: ProtocolType::V2,
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
    use ethers::types::U256;

    fn addr(n: u64) -> Address {
        Address::from_low_u64_be(n)
    }

    fn v2_exact_in_calldata(path: Vec<Address>, amount_in: u64, min_out: u64) -> Vec<u8> {
        encode(&[
            Token::Uint(U256::from(amount_in)),
            Token::Uint(U256::from(min_out)),
            Token::Array(path.iter().map(|a| Token::Address(*a)).collect()),
            Token::Address(addr(0xCC)),
            Token::Uint(U256::from(9999u32)),
        ])
    }

    fn v2_exact_out_calldata(path: Vec<Address>, amount_out: u64, amount_in_max: u64) -> Vec<u8> {
        encode(&[
            Token::Uint(U256::from(amount_out)),
            Token::Uint(U256::from(amount_in_max)),
            Token::Array(path.iter().map(|a| Token::Address(*a)).collect()),
            Token::Address(addr(0xCC)),
            Token::Uint(U256::from(9999u32)),
        ])
    }

    // ── V2 exact-in, 2-token path: path_tokens preserved, 1 fee entry = 30 bps ──

    #[test]
    fn v2_exact_in_two_tokens_path_and_fees() {
        let tin = addr(0xA);
        let tout = addr(0xB);
        let body = v2_exact_in_calldata(vec![tin, tout], 1_000, 900);
        let decoded = decode([0x38, 0xed, 0x17, 0x39], &body).unwrap();

        assert_eq!(decoded.path_tokens, vec![tin, tout]);
        assert_eq!(decoded.path_fees_bps, vec![30]);
        assert_eq!(decoded.exact_mode, SwapExactMode::ExactIn);
        assert_eq!(decoded.protocol_type, ProtocolType::V2);
    }

    // ── V2 exact-in, 3-token path: 2 fee entries ────────────────────────────

    #[test]
    fn v2_exact_in_three_tokens_two_fee_entries() {
        let tin = addr(0xA);
        let tmid = addr(0xB);
        let tout = addr(0xC);
        let body = v2_exact_in_calldata(vec![tin, tmid, tout], 5_000, 4_500);
        let decoded = decode([0x38, 0xed, 0x17, 0x39], &body).unwrap();

        assert_eq!(decoded.path_tokens, vec![tin, tmid, tout]);
        assert_eq!(
            decoded.path_fees_bps.len(),
            2,
            "3-token path → 2 fee entries"
        );
        assert_eq!(decoded.path_fees_bps, vec![30, 30]);
        assert_eq!(decoded.exact_mode, SwapExactMode::ExactIn);
    }

    // ── V2 exact-out: ExactOut mode, same fee logic ─────────────────────────

    #[test]
    fn v2_exact_out_path_and_fees() {
        let tin = addr(0xA);
        let tout = addr(0xB);
        let body = v2_exact_out_calldata(vec![tin, tout], 900, 1_000);
        let decoded = decode([0x88, 0x03, 0xdb, 0xee], &body).unwrap();

        assert_eq!(decoded.path_tokens, vec![tin, tout]);
        assert_eq!(decoded.path_fees_bps, vec![30]);
        assert_eq!(decoded.exact_mode, SwapExactMode::ExactOut);
        assert_eq!(decoded.protocol_type, ProtocolType::V2);
    }

    // ── V2 ETH-in: amount_in = 0 (caller fills from tx.value) ──────────────

    #[test]
    fn v2_eth_in_path_and_fees() {
        let weth = addr(0xEEEE);
        let usdc = addr(0xFFFF);
        let body = encode(&[
            Token::Uint(U256::from(500u64)),
            Token::Array(vec![Token::Address(weth), Token::Address(usdc)]),
            Token::Address(addr(0xABCD)),
            Token::Uint(U256::from(9999u32)),
        ]);
        let decoded = decode([0x7f, 0xf3, 0x6a, 0xb5], &body).unwrap();

        assert_eq!(decoded.path_tokens, vec![weth, usdc]);
        assert_eq!(decoded.path_fees_bps, vec![30]);
        assert_eq!(decoded.amount_in, U256::zero()); // caller fills from tx.value
        assert_eq!(decoded.exact_mode, SwapExactMode::ExactIn);
    }

    // ── path_fees_bps.len() == path_tokens.len() - 1 invariant ─────────────

    #[test]
    fn fee_bps_length_invariant() {
        for hop_count in 1usize..=4 {
            let path: Vec<Address> = (0..=hop_count as u64).map(addr).collect();
            let body = v2_exact_in_calldata(path.clone(), 1_000, 900);
            let decoded = decode([0x38, 0xed, 0x17, 0x39], &body).unwrap();
            assert_eq!(
                decoded.path_fees_bps.len(),
                decoded.path_tokens.len() - 1,
                "fee_bps.len() must equal path_tokens.len()-1 for hop_count={hop_count}"
            );
        }
    }
}
