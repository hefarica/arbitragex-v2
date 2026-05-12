//! Calldata decoder dispatcher.
//!
//! Given router kind + calldata bytes, returns a `DecodedSwap` or `None`.
//! Unknown routers / selectors map to `None` and the caller increments
//! `arbx_searcher_undecoded_total`.
//!
//! ## New fields in `DecodedSwap` (Phase 1 — spec §3 TASK 2)
//!
//! `path_tokens`, `path_fees_bps`, `exact_mode`, and `protocol_type` are now
//! populated by the sub-decoders so that `route_decoder::build_legs` is a pure
//! transformation over already-extracted data rather than re-parsing calldata.
//!
//! ## Type reuse
//!
//! `SwapExactMode` and `ProtocolType` are defined in `route_intent` and
//! re-exported here via `pub use` to avoid duplication. `calldata/univ2.rs` and
//! `calldata/univ3.rs` import them from `super` (i.e. this module).

use ethers::types::{Address, U256};
use serde::Serialize;
use shared_rs::chains::RouterKind;

pub mod univ2;
pub mod univ3;

// Re-export the protocol enums from route_intent so sub-decoders and callers
// can use them without importing from two separate modules.
pub use crate::route_intent::{ProtocolType, SwapExactMode};

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

    // ── Phase 1 additions (spec §3 TASK 2) ─────────────────────────────────
    /// Full token path extracted from calldata.
    ///
    /// - V2 `swapExactTokensForTokens(path=[A,B,C])`: `[A, B, C]`.
    /// - V3 `exactInputSingle`: `[tokenIn, tokenOut]`.
    /// - V3 `exactInput(packed path)`: parsed from the packed bytes,
    ///   e.g. `[A, B, C]` for a 2-hop path.
    /// - Unknown router / decode failure: `[token_in, token_out]` (fallback,
    ///   same as before — no synthetic intermediates introduced).
    pub path_tokens: Vec<Address>,

    /// LP fee per hop in basis points, length = `path_tokens.len() - 1`.
    ///
    /// - V2 swaps: `vec![30; path.len() - 1]` — the Uniswap V2 protocol-level
    ///   0.30% fee is a compile-time constant (R8 exception: this is a protocol
    ///   invariant, not fabricated data).
    /// - V3 `exactInputSingle`: `vec![fee_bps]` extracted from the struct.
    /// - V3 `exactInput` multi-hop: one entry per hop extracted from the packed
    ///   3-byte fee fields.
    /// - Unknown protocol or decode failure: `vec![]` (empty — R8 fail-honest).
    pub path_fees_bps: Vec<u32>,

    /// Whether the swap is exact-input or exact-output.
    pub exact_mode: SwapExactMode,

    /// V2 / V3 / Curve / Balancer / Unknown protocol family.
    pub protocol_type: ProtocolType,
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
            DecodeFailReason::UnknownRouter => "unknown_router",
            DecodeFailReason::UnsupportedSelector => "unsupported_selector",
            DecodeFailReason::AbiDecodeError => "abi_decode_error",
            DecodeFailReason::ShortInput => "short_input",
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
        RouterKind::UniswapV3 => univ3::decode(selector, &input[4..]),
        _ => Err(DecodeFailReason::UnknownRouter),
    }
}

// ---------------------------------------------------------------------------
// Shared path-parsing utilities (used by univ3 and route_decoder)
// ---------------------------------------------------------------------------

/// Parse a V3 packed path byte string into an ordered list of token addresses
/// and the per-hop fees in basis points.
///
/// Layout: `token(20) [fee(3) token(20)]*`
/// Minimum length for one hop: 20 + 3 + 20 = 43 bytes.
///
/// Returns `(tokens, fees_bps)` where `fees_bps.len() == tokens.len() - 1`.
/// Returns `None` when the byte string is too short or misaligned.
pub(crate) fn parse_v3_path_bytes_with_fees(path: &[u8]) -> Option<(Vec<Address>, Vec<u32>)> {
    if path.len() < 43 {
        return None;
    }
    // Validate layout: (len - 20) must be divisible by 23 (fee(3) + token(20)).
    if (path.len() - 20) % 23 != 0 {
        return None;
    }

    let mut tokens: Vec<Address> = Vec::new();
    let mut fees: Vec<u32> = Vec::new();
    let mut offset = 0usize;

    // First token.
    let first: [u8; 20] = path[offset..offset + 20].try_into().ok()?;
    tokens.push(Address::from(first));
    offset += 20;

    // Each subsequent hop: fee(3) + token(20).
    while offset + 23 <= path.len() {
        // fee is a 3-byte big-endian uint24 (Uniswap V3 convention).
        let fee_raw = u32::from_be_bytes([0, path[offset], path[offset + 1], path[offset + 2]]);
        // V3 fee in ABI units (e.g. 3000 = 0.3%) — convert to basis points (/100).
        fees.push(fee_raw / 100);
        offset += 3;

        let t: [u8; 20] = path[offset..offset + 20].try_into().ok()?;
        tokens.push(Address::from(t));
        offset += 20;
    }

    if tokens.len() < 2 {
        return None;
    }
    Some((tokens, fees))
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn v3_path_bytes_two_tokens() {
        let mut p = Vec::new();
        p.extend_from_slice(&[0xAAu8; 20]);
        p.extend_from_slice(&[0x00, 0x0b, 0xb8]); // 3000 raw / 100 = 30 bps
        p.extend_from_slice(&[0xBBu8; 20]);
        let (addrs, fees) = parse_v3_path_bytes_with_fees(&p).unwrap();
        assert_eq!(addrs.len(), 2);
        assert_eq!(fees.len(), 1);
        assert_eq!(fees[0], 30);
        assert_eq!(addrs[0], Address::from([0xAAu8; 20]));
        assert_eq!(addrs[1], Address::from([0xBBu8; 20]));
    }

    #[test]
    fn v3_path_bytes_three_tokens_two_fees() {
        let mut p = Vec::new();
        p.extend_from_slice(&[0x11u8; 20]);
        p.extend_from_slice(&[0x00, 0x0b, 0xb8]); // 3000 → 30 bps
        p.extend_from_slice(&[0x22u8; 20]);
        p.extend_from_slice(&[0x00, 0x01, 0xf4]); // 500 → 5 bps
        p.extend_from_slice(&[0x33u8; 20]);
        let (addrs, fees) = parse_v3_path_bytes_with_fees(&p).unwrap();
        assert_eq!(addrs.len(), 3);
        assert_eq!(fees.len(), 2);
        assert_eq!(fees[0], 30);
        assert_eq!(fees[1], 5);
    }

    #[test]
    fn v3_path_bytes_too_short_returns_none() {
        let p = vec![0u8; 42]; // one byte short of minimum 43
        assert!(parse_v3_path_bytes_with_fees(&p).is_none());
    }

    #[test]
    fn v3_path_bytes_invalid_alignment_returns_none() {
        // 44 bytes: (44 - 20) = 24, 24 % 23 != 0 → None
        let p = vec![0u8; 44];
        assert!(parse_v3_path_bytes_with_fees(&p).is_none());
    }
}
