//! Route decoder — converts a decoded pending tx into one or more `RouteIntent`s.
//!
//! ## Design (Phase 1 — spec §3 TASK 3)
//!
//! `decode_to_route_intents` is the single entry point. It:
//! 1. Calls the existing `calldata::decode()` to validate the selector and extract
//!    all swap fields, including the new Phase 1 fields: `path_tokens`,
//!    `path_fees_bps`, `exact_mode`, and `protocol_type`.
//! 2. Constructs one `RouteIntentLeg` per hop directly from `decoded.path_tokens`
//!    and `decoded.path_fees_bps` — no calldata re-parsing in this module.
//! 3. Builds one `RouteIntent` per logical swap (multicall decomposition §3.3.1).
//!
//! ## Simplification vs previous implementation
//!
//! The previous version re-parsed the raw calldata body to reconstruct intermediate
//! token addresses (via `decode_v2_address_path` / `decode_v3_packed_path`). Those
//! helpers have been removed because `DecodedSwap.path_tokens` now carries the full
//! ordered path, populated by the sub-decoders in `calldata/univ2.rs` and
//! `calldata/univ3.rs`. This module is now a pure transformation function.
//!
//! ## R8 invariants
//!
//! - When the router is unknown → 0 intents returned (caller skips).
//! - When calldata fails decode → 0 intents returned, error logged but never
//!   propagated as a crash.
//! - All `RouteIntentLeg` optional fields (`pool_hint`, `dex_hint`, `fee_bps`)
//!   stay `None` when not deterministically extractable — no sentinel substitution.
//!   Exception: `fee_bps` is now populated from `decoded.path_fees_bps` when
//!   present (V2: 30 bps protocol constant; V3: from calldata struct).

use crate::calldata::{self, DecodeFailReason};
use crate::route_intent::{DetectionSource, RouteIntent, RouteIntentLeg, RouterKind};
use ethers::types::{Address, Transaction};
use shared_rs::chains::RouterEntry;
use tracing::{debug, warn};

/// Maximum number of intents produced from a single transaction (safety bound,
/// spec §7 risk register — guards against multicall fan-out explosion).
const MAX_INTENTS_PER_TX: usize = 16;

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Decode a pending transaction into zero or more `RouteIntent`s.
///
/// ### Parameters
/// - `tx`: the raw pending transaction.
/// - `router`: the matched router entry from the static catalog
///   (`shared_rs::chains::find_router`). The caller is responsible for the
///   catalog lookup.
/// - `chain_id`: chain this tx was observed on.
/// - `source`: which event source delivered this tx.
///
/// ### Returns
/// - `Ok(Vec<RouteIntent>)` — possibly empty when the selector is unknown.
///   A single-element vec is the common case.
/// - `Err(_)` — only for internal bugs (never for calldata parse failures, which
///   produce an empty vec and an info-level log). The caller should log errors
///   but never crash the scanning loop.
pub fn decode_to_route_intents(
    tx: &Transaction,
    router: &RouterEntry,
    chain_id: u64,
    source: DetectionSource,
) -> anyhow::Result<Vec<RouteIntent>> {
    let input = tx.input.as_ref();
    if input.len() < 4 {
        debug!(
            event = "route_decoder.short_input",
            tx_hash = %tx.hash,
            input_len = input.len(),
        );
        return Ok(vec![]);
    }

    // First-pass decode using the existing dispatcher.
    // The returned `DecodedSwap` now carries `path_tokens`, `path_fees_bps`,
    // `exact_mode`, and `protocol_type` — no further calldata parsing needed here.
    let router_catalog_kind = router.kind;
    let decoded = match calldata::decode(input, router_catalog_kind) {
        Ok(d) => d,
        Err(DecodeFailReason::UnknownRouter) => {
            debug!(
                event = "route_decoder.unknown_router",
                tx_hash = %tx.hash,
                router = %format!("0x{}", hex::encode(router.address)),
            );
            return Ok(vec![]);
        }
        Err(DecodeFailReason::UnsupportedSelector) => {
            debug!(
                event = "route_decoder.unsupported_selector",
                tx_hash = %tx.hash,
                selector = %hex::encode(&input[..4]),
            );
            return Ok(vec![]);
        }
        Err(DecodeFailReason::ShortInput) => {
            debug!(
                event = "route_decoder.short_body",
                tx_hash = %tx.hash,
            );
            return Ok(vec![]);
        }
        Err(DecodeFailReason::AbiDecodeError) => {
            warn!(
                event = "route_decoder.abi_decode_error",
                tx_hash = %tx.hash,
                selector = %hex::encode(&input[..4]),
            );
            return Ok(vec![]);
        }
    };

    // Convert shared catalog RouterKind → local RouterKind.
    let local_router_kind = RouterKind::from(router_catalog_kind);

    // DEX hint: the static router name (e.g. "uniswap-v2", "sushi").
    let dex_hint = Some(router.kind.as_str().to_owned());

    // Build legs directly from DecodedSwap.path_tokens + path_fees_bps.
    // This replaces the previous calldata re-parsing in build_legs().
    let legs = build_legs_from_decoded(&decoded, dex_hint)?;

    if legs.is_empty() {
        warn!(
            event = "route_decoder.zero_legs_after_build",
            tx_hash = %tx.hash,
        );
        return Ok(vec![]);
    }

    // amount_in: for ETH-in selectors the decoded amount_in is zero (caller
    // fills from tx.value). Substitute tx.value when the decoded value is zero
    // AND the selector indicates ETH-in.
    let selector = [input[0], input[1], input[2], input[3]];
    let amount_in = if decoded.amount_in.is_zero() && is_eth_in_selector(selector) {
        tx.value
    } else {
        decoded.amount_in
    };

    // min_amount_out: R8 — keep Some(value) from decoder; 0 from the decoder
    // is a real value (no slippage protection set by sender).
    let min_amount_out = Some(decoded.min_amount_out);

    let sender = tx.from;
    let router_address = Address::from(router.address);

    let intent = match RouteIntent::new(
        chain_id,
        tx.hash,
        router_address,
        local_router_kind,
        sender,
        legs,
        amount_in,
        min_amount_out,
        decoded.exact_mode,
        source,
    ) {
        Some(i) => i,
        None => {
            // Invariant violation: build_legs_from_decoded returned non-empty
            // but RouteIntent::new returned None — unreachable given the
            // legs.is_empty() check above.
            warn!(
                event = "route_decoder.invariant_violation",
                tx_hash = %tx.hash,
                "legs was non-empty but RouteIntent::new returned None"
            );
            return Ok(vec![]);
        }
    };

    Ok(vec![intent])
}

// ---------------------------------------------------------------------------
// Private helpers
// ---------------------------------------------------------------------------

/// Build the ordered list of `RouteIntentLeg`s from the already-decoded swap.
///
/// Uses `decoded.path_tokens` and `decoded.path_fees_bps` (populated by the
/// sub-decoders in Phase 1). This is now a pure transformation — no calldata
/// bytes are re-parsed here.
///
/// For a path of N tokens, emits N-1 legs (one per hop). The `MAX_INTENTS_PER_TX`
/// bound caps the number of legs to prevent fan-out explosion.
fn build_legs_from_decoded(
    decoded: &calldata::DecodedSwap,
    dex_hint: Option<String>,
) -> anyhow::Result<Vec<RouteIntentLeg>> {
    let path = &decoded.path_tokens;

    if path.len() < 2 {
        // Defensive: should not happen since decoders require path.len() >= 2,
        // but guard here for R8 fail-honest.
        warn!(
            event = "route_decoder.path_too_short",
            path_len = path.len(),
            "path_tokens has fewer than 2 entries; producing empty legs (R8)"
        );
        return Ok(vec![]);
    }

    let legs: Vec<RouteIntentLeg> = path
        .windows(2)
        .take(MAX_INTENTS_PER_TX)
        .enumerate()
        .map(|(i, w)| {
            // fee_bps: from decoded.path_fees_bps[i] when available.
            // R8: if the fees array is shorter than expected (e.g. Unknown protocol),
            // fall back to None rather than fabricating a value.
            let fee_bps = decoded.path_fees_bps.get(i).copied();
            RouteIntentLeg {
                token_in: w[0],
                token_out: w[1],
                pool_hint: None, // R8: never fabricate a pool address from calldata
                dex_hint: dex_hint.clone(),
                fee_bps,
                protocol_type: decoded.protocol_type,
            }
        })
        .collect();

    Ok(legs)
}

/// Returns `true` when the selector is an ETH-in variant (amount_in = tx.value).
fn is_eth_in_selector(selector: [u8; 4]) -> bool {
    matches!(
        selector,
        [0x7f, 0xf3, 0x6a, 0xb5]  // swapExactETHForTokens
        | [0xb6, 0xf9, 0xde, 0x95] // swapExactETHForTokensSupportingFOT
        | [0xfb, 0x3b, 0xdb, 0x41] // swapETHForExactTokens
    )
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use crate::route_intent::{ProtocolType, SwapExactMode};
    use ethers::abi::{encode, Token};
    use ethers::types::{Address, Bytes, H256, U256};
    use shared_rs::chains::{RouterEntry, RouterKind as SharedRK};

    // ── Helpers ──────────────────────────────────────────────────────────────

    fn router_entry(kind: SharedRK) -> RouterEntry {
        RouterEntry {
            chain_id: 1,
            name: "test",
            kind,
            address: [0xCAu8; 20],
        }
    }

    fn build_tx(input: Vec<u8>, value: U256, from: Address) -> Transaction {
        Transaction {
            hash: H256::from_low_u64_be(0xDEAD),
            from,
            input: Bytes::from(input),
            value,
            ..Default::default()
        }
    }

    fn pad_selector_body(selector: [u8; 4], tokens: &[Token]) -> Vec<u8> {
        let mut out = selector.to_vec();
        out.extend_from_slice(&encode(tokens));
        out
    }

    fn token(n: u64) -> Address {
        Address::from_low_u64_be(n)
    }

    fn v2_swap_exact_in(token_in: Address, token_out: Address, amount_in: u64) -> Vec<u8> {
        pad_selector_body(
            [0x38, 0xed, 0x17, 0x39],
            &[
                Token::Uint(U256::from(amount_in)),
                Token::Uint(U256::from(900u64)),
                Token::Array(vec![Token::Address(token_in), Token::Address(token_out)]),
                Token::Address(Address::from_low_u64_be(0xCC)),
                Token::Uint(U256::from(9999u32)),
            ],
        )
    }

    fn v2_swap_multi_hop(
        token_in: Address,
        token_mid: Address,
        token_out: Address,
        amount_in: u64,
    ) -> Vec<u8> {
        pad_selector_body(
            [0x38, 0xed, 0x17, 0x39],
            &[
                Token::Uint(U256::from(amount_in)),
                Token::Uint(U256::from(900u64)),
                Token::Array(vec![
                    Token::Address(token_in),
                    Token::Address(token_mid),
                    Token::Address(token_out),
                ]),
                Token::Address(Address::from_low_u64_be(0xCC)),
                Token::Uint(U256::from(9999u32)),
            ],
        )
    }

    fn v3_exact_input_single(
        token_in: Address,
        token_out: Address,
        fee: u32,
        amount_in: u64,
    ) -> Vec<u8> {
        pad_selector_body(
            [0x41, 0x4b, 0xf3, 0x89],
            &[Token::Tuple(vec![
                Token::Address(token_in),
                Token::Address(token_out),
                Token::Uint(U256::from(fee)),
                Token::Address(Address::from_low_u64_be(0xDD)),
                Token::Uint(U256::from(0xdeadu32)),
                Token::Uint(U256::from(amount_in)),
                Token::Uint(U256::from(950u64)),
                Token::Uint(U256::zero()),
            ])],
        )
    }

    // ── spec §8: route_decoder::tests::v2_simple_swap_one_leg ────────────────

    #[test]
    fn v2_simple_swap_one_leg() {
        let tin = token(0xA);
        let tout = token(0xB);
        let tx = build_tx(v2_swap_exact_in(tin, tout, 1_000), U256::zero(), token(0xF));
        let router = router_entry(SharedRK::UniswapV2);

        let intents =
            decode_to_route_intents(&tx, &router, 1, DetectionSource::PublicMempool).unwrap();

        assert_eq!(intents.len(), 1, "single swap produces exactly one intent");
        let intent = &intents[0];
        assert_eq!(intent.legs.len(), 1);
        assert_eq!(intent.legs[0].token_in, tin);
        assert_eq!(intent.legs[0].token_out, tout);
        assert_eq!(intent.legs[0].protocol_type, ProtocolType::V2);
        assert_eq!(intent.exact_mode, SwapExactMode::ExactIn);
        assert_eq!(intent.amount_in, U256::from(1_000u64));
        assert_eq!(intent.chain_id, 1);
        assert_eq!(intent.source_event, DetectionSource::PublicMempool);
        // R8: pool_hint must be None (not extractable from calldata).
        assert!(intent.legs[0].pool_hint.is_none());
        // V2 fee: 30 bps (protocol constant).
        assert_eq!(intent.legs[0].fee_bps, Some(30));
    }

    // ── V2 multi-hop → single intent with 2 legs (spec §3.3) ─────────────────

    #[test]
    fn v2_multi_hop_produces_one_intent_two_legs() {
        let tin = token(0xA);
        let tmid = token(0xB);
        let tout = token(0xC);
        let tx = build_tx(
            v2_swap_multi_hop(tin, tmid, tout, 5_000),
            U256::zero(),
            token(0xF),
        );
        let router = router_entry(SharedRK::UniswapV2);

        let intents =
            decode_to_route_intents(&tx, &router, 1, DetectionSource::PublicMempool).unwrap();

        assert_eq!(intents.len(), 1, "multi-hop produces ONE intent");
        let intent = &intents[0];
        assert_eq!(intent.legs.len(), 2, "path=[A,B,C] produces 2 legs");
        assert_eq!(intent.legs[0].token_in, tin);
        assert_eq!(intent.legs[0].token_out, tmid);
        assert_eq!(intent.legs[0].fee_bps, Some(30));
        assert_eq!(intent.legs[1].token_in, tmid);
        assert_eq!(intent.legs[1].token_out, tout);
        assert_eq!(intent.legs[1].fee_bps, Some(30));
    }

    // ── V2 path of 3 tokens generates 2 entries in path_fees_bps ────────────

    #[test]
    fn v2_three_tokens_two_fee_bps_entries() {
        let tin = token(0xA);
        let tmid = token(0xB);
        let tout = token(0xC);
        let tx = build_tx(
            v2_swap_multi_hop(tin, tmid, tout, 5_000),
            U256::zero(),
            token(0xF),
        );
        let router = router_entry(SharedRK::UniswapV2);
        let intents =
            decode_to_route_intents(&tx, &router, 1, DetectionSource::PublicMempool).unwrap();
        let intent = &intents[0];
        assert_eq!(
            intent.legs.len(),
            2,
            "path of 3 tokens → 2 legs → 2 fee_bps entries"
        );
        assert!(intent.legs.iter().all(|l| l.fee_bps == Some(30)));
    }

    // ── spec §8: route_decoder::tests::v3_packed_path_multi_leg ──────────────

    #[test]
    fn v3_packed_path_multi_leg() {
        // Build a 3-token packed path: A(20)|fee(3)|B(20)|fee(3)|C(20) = 66 bytes
        let mut path_bytes = Vec::new();
        let addr_a = [0xAAu8; 20];
        let addr_b = [0xBBu8; 20];
        let addr_c = [0xCCu8; 20];
        path_bytes.extend_from_slice(&addr_a);
        path_bytes.extend_from_slice(&[0x00, 0x0b, 0xb8]); // fee 3000
        path_bytes.extend_from_slice(&addr_b);
        path_bytes.extend_from_slice(&[0x00, 0x0b, 0xb8]);
        path_bytes.extend_from_slice(&addr_c);
        assert_eq!(path_bytes.len(), 66);

        let input = pad_selector_body(
            [0xc0, 0x4b, 0x8d, 0x59],
            &[Token::Tuple(vec![
                Token::Bytes(path_bytes.clone()),
                Token::Address(Address::from_low_u64_be(0xEE)),
                Token::Uint(U256::from(0u32)),
                Token::Uint(U256::from(2_000u64)),
                Token::Uint(U256::from(1_900u64)),
            ])],
        );
        let tx = build_tx(input, U256::zero(), token(0xF));
        let router = router_entry(SharedRK::UniswapV3);

        let intents =
            decode_to_route_intents(&tx, &router, 1, DetectionSource::PublicMempool).unwrap();

        assert_eq!(intents.len(), 1);
        let intent = &intents[0];
        assert_eq!(intent.legs.len(), 2, "3-token V3 path → 2 legs");
        assert_eq!(intent.legs[0].token_in, Address::from(addr_a));
        assert_eq!(intent.legs[0].token_out, Address::from(addr_b));
        assert_eq!(intent.legs[1].token_in, Address::from(addr_b));
        assert_eq!(intent.legs[1].token_out, Address::from(addr_c));
        assert_eq!(intent.legs[0].protocol_type, ProtocolType::V3);
        // V3 fee: 3000 / 100 = 30 bps per hop.
        assert_eq!(intent.legs[0].fee_bps, Some(30));
        assert_eq!(intent.legs[1].fee_bps, Some(30));
    }

    // ── V3 exactInputSingle preserves the real fee tier ──────────────────────

    #[test]
    fn v3_exact_input_single_preserves_fee_tier() {
        // 500 raw → 5 bps
        let tin = token(0x1);
        let tout = token(0x2);
        let tx = build_tx(
            v3_exact_input_single(tin, tout, 500, 2_000),
            U256::zero(),
            token(0xF),
        );
        let router = router_entry(SharedRK::UniswapV3);
        let intents =
            decode_to_route_intents(&tx, &router, 1, DetectionSource::PublicMempool).unwrap();
        let intent = &intents[0];
        assert_eq!(intent.legs[0].fee_bps, Some(5), "500 raw → 5 bps");

        // 3000 raw → 30 bps
        let tx2 = build_tx(
            v3_exact_input_single(tin, tout, 3000, 2_000),
            U256::zero(),
            token(0xF),
        );
        let intents2 =
            decode_to_route_intents(&tx2, &router, 1, DetectionSource::PublicMempool).unwrap();
        assert_eq!(intents2[0].legs[0].fee_bps, Some(30), "3000 raw → 30 bps");

        // 10000 raw → 100 bps
        let tx3 = build_tx(
            v3_exact_input_single(tin, tout, 10000, 2_000),
            U256::zero(),
            token(0xF),
        );
        let intents3 =
            decode_to_route_intents(&tx3, &router, 1, DetectionSource::PublicMempool).unwrap();
        assert_eq!(
            intents3[0].legs[0].fee_bps,
            Some(100),
            "10000 raw → 100 bps"
        );
    }

    // ── V3 exactInput multi-hop preserves all tokens AND fees in order ────────

    #[test]
    fn v3_exact_input_multi_hop_all_tokens_and_fees() {
        let addr_a = [0xAAu8; 20];
        let addr_b = [0xBBu8; 20];
        let addr_c = [0xCCu8; 20];
        let mut path_bytes = Vec::new();
        path_bytes.extend_from_slice(&addr_a);
        path_bytes.extend_from_slice(&[0x00, 0x0b, 0xb8]); // 3000 raw
        path_bytes.extend_from_slice(&addr_b);
        path_bytes.extend_from_slice(&[0x00, 0x01, 0xf4]); // 500 raw
        path_bytes.extend_from_slice(&addr_c);

        let input = pad_selector_body(
            [0xc0, 0x4b, 0x8d, 0x59],
            &[Token::Tuple(vec![
                Token::Bytes(path_bytes),
                Token::Address(Address::from_low_u64_be(0xEE)),
                Token::Uint(U256::zero()),
                Token::Uint(U256::from(1_000u64)),
                Token::Uint(U256::from(900u64)),
            ])],
        );
        let tx = build_tx(input, U256::zero(), token(0xF));
        let router = router_entry(SharedRK::UniswapV3);
        let intents =
            decode_to_route_intents(&tx, &router, 1, DetectionSource::PublicMempool).unwrap();
        let intent = &intents[0];
        assert_eq!(intent.legs.len(), 2);
        assert_eq!(intent.legs[0].token_in, Address::from(addr_a));
        assert_eq!(intent.legs[0].token_out, Address::from(addr_b));
        assert_eq!(intent.legs[0].fee_bps, Some(30)); // 3000/100
        assert_eq!(intent.legs[1].token_in, Address::from(addr_b));
        assert_eq!(intent.legs[1].token_out, Address::from(addr_c));
        assert_eq!(intent.legs[1].fee_bps, Some(5)); // 500/100
    }

    // ── spec §8: route_decoder::tests::unknown_router_zero_intents ───────────

    #[test]
    fn unknown_router_zero_intents() {
        let tx = build_tx(
            v2_swap_exact_in(token(0xA), token(0xB), 1_000),
            U256::zero(),
            token(0xF),
        );
        let router = router_entry(SharedRK::Unknown);

        let intents =
            decode_to_route_intents(&tx, &router, 1, DetectionSource::PublicMempool).unwrap();

        assert_eq!(
            intents.len(),
            0,
            "unknown router must produce zero intents (spec §3.3 behaviour matrix)"
        );
    }

    // ── V3 exactInputSingle → 1 leg, fee_bps extracted ───────────────────────

    #[test]
    fn v3_exact_input_single_one_leg_with_fee() {
        let tin = token(0x1);
        let tout = token(0x2);
        let tx = build_tx(
            v3_exact_input_single(tin, tout, 3000, 2_000),
            U256::zero(),
            token(0xF),
        );
        let router = router_entry(SharedRK::UniswapV3);

        let intents =
            decode_to_route_intents(&tx, &router, 1, DetectionSource::PublicMempool).unwrap();

        assert_eq!(intents.len(), 1);
        let intent = &intents[0];
        assert_eq!(intent.legs.len(), 1);
        assert_eq!(intent.legs[0].token_in, tin);
        assert_eq!(intent.legs[0].token_out, tout);
        assert_eq!(intent.legs[0].protocol_type, ProtocolType::V3);
        // fee 3000 ABI units / 100 = 30 bps.
        assert_eq!(intent.legs[0].fee_bps, Some(30));
        assert_eq!(intent.exact_mode, SwapExactMode::ExactIn);
    }

    // ── ETH-in swap: amount_in comes from tx.value ───────────────────────────

    #[test]
    fn eth_in_swap_uses_tx_value() {
        let weth = token(0xEEEE);
        let usdc = token(0xFFFF);
        let input = pad_selector_body(
            [0x7f, 0xf3, 0x6a, 0xb5], // swapExactETHForTokens
            &[
                Token::Uint(U256::from(500u64)), // amountOutMin
                Token::Array(vec![Token::Address(weth), Token::Address(usdc)]),
                Token::Address(Address::from_low_u64_be(0xABCD)),
                Token::Uint(U256::from(9999u32)),
            ],
        );
        let tx_value = U256::from(1_000_000u64);
        let tx = build_tx(input, tx_value, token(0xF));
        let router = router_entry(SharedRK::UniswapV2);

        let intents =
            decode_to_route_intents(&tx, &router, 1, DetectionSource::PublicMempool).unwrap();

        assert_eq!(intents.len(), 1);
        assert_eq!(
            intents[0].amount_in, tx_value,
            "ETH-in swap must use tx.value as amount_in"
        );
    }

    // ── Short input → 0 intents (no panic) ───────────────────────────────────

    #[test]
    fn short_input_zero_intents() {
        let tx = build_tx(vec![0x38, 0xed], U256::zero(), token(0xF));
        let router = router_entry(SharedRK::UniswapV2);

        let intents =
            decode_to_route_intents(&tx, &router, 1, DetectionSource::PublicMempool).unwrap();
        assert_eq!(intents.len(), 0);
    }

    // ── Unsupported selector → 0 intents ─────────────────────────────────────

    #[test]
    fn unsupported_selector_zero_intents() {
        let tx = build_tx(
            vec![0xde, 0xad, 0xbe, 0xef, 0, 0, 0, 0],
            U256::zero(),
            token(0xF),
        );
        let router = router_entry(SharedRK::UniswapV2);

        let intents =
            decode_to_route_intents(&tx, &router, 1, DetectionSource::PublicMempool).unwrap();
        assert_eq!(intents.len(), 0);
    }

    // ── R8: pool_hint is always None from decoder ─────────────────────────────

    #[test]
    fn pool_hint_always_none() {
        let tx = build_tx(
            v2_swap_exact_in(token(0xA), token(0xB), 1_000),
            U256::zero(),
            token(0xF),
        );
        let router = router_entry(SharedRK::UniswapV2);
        let intents =
            decode_to_route_intents(&tx, &router, 1, DetectionSource::PublicMempool).unwrap();
        for intent in &intents {
            for leg in &intent.legs {
                assert!(
                    leg.pool_hint.is_none(),
                    "pool_hint must always be None from the decoder (spec §3.2 R8)"
                );
            }
        }
    }

    // ── Incomplete calldata returns 0 intents, never panics ──────────────────

    #[test]
    fn incomplete_calldata_zero_intents_no_panic() {
        // Selector only, no body — ABI decode will fail gracefully.
        let tx = build_tx(
            vec![0x41, 0x4b, 0xf3, 0x89, 0x00, 0x00],
            U256::zero(),
            token(0xF),
        );
        let router = router_entry(SharedRK::UniswapV3);
        let intents =
            decode_to_route_intents(&tx, &router, 1, DetectionSource::PublicMempool).unwrap();
        assert_eq!(
            intents.len(),
            0,
            "truncated calldata must produce 0 intents"
        );
    }
}
