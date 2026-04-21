//! Calldata decoder dispatcher.
//!
//! Given router kind + calldata bytes, returns a `DecodedSwap` or `None`.
//! Unknown routers / selectors map to `None` and the caller increments
//! `arbx_searcher_undecoded_total`.

use ethers::types::{Address, U256};
use serde::Serialize;
use shared_rs::chains::RouterKind;

pub mod univ2;
pub mod univ3;

#[derive(Debug, Clone, Serialize)]
pub struct DecodedSwap {
    pub router: &'static str,
    pub token_in: Address,
    pub token_out: Address,
    /// If the selector is `ExactIn*`, `amount_in` is exact; for `ExactOut*` it is the upper bound.
    pub amount_in: U256,
    /// If the selector is `ExactIn*`, `amount_out_minimum`; for `ExactOut*`, `amount_out` (exact).
    pub min_amount_out: U256,
    pub path_len: u32,
    pub deadline: U256,
    pub recipient: Address,
    /// The 4-byte selector observed.
    pub selector_hex: String,
}

#[derive(Debug, Clone, Copy)]
pub enum DecodeFailReason {
    UnknownRouter,
    UnsupportedSelector,
    AbiDecodeError,
    ShortInput,
}

impl DecodeFailReason {
    pub fn as_str(&self) -> &'static str {
        match self {
            DecodeFailReason::UnknownRouter        => "unknown_router",
            DecodeFailReason::UnsupportedSelector  => "unsupported_selector",
            DecodeFailReason::AbiDecodeError       => "abi_decode_error",
            DecodeFailReason::ShortInput           => "short_input",
        }
    }
}

pub fn decode(input: &[u8], router: RouterKind) -> Result<DecodedSwap, DecodeFailReason> {
    if input.len() < 4 {
        return Err(DecodeFailReason::ShortInput);
    }
    let selector = [input[0], input[1], input[2], input[3]];
    match router {
        RouterKind::UniswapV2 | RouterKind::Sushi => univ2::decode(selector, &input[4..]),
        RouterKind::UniswapV3                    => univ3::decode(selector, &input[4..]),
        _ => Err(DecodeFailReason::UnknownRouter),
    }
}
