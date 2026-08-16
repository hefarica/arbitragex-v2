//! Uniswap Universal Router calldata decoder.
//!
//! The Universal Router is a command-dispatcher contract: a single
//! `execute()` call carries a byte string of commands and one ABI-encoded
//! input blob per command. One transaction can therefore contain several
//! swaps (plus permits, wraps, transfers, NFT actions, ...).
//!
//! Selectors:
//!   execute(bytes commands, bytes[] inputs)                     0x3593564c
//!   execute(bytes commands, bytes[] inputs, uint256 deadline)   0x24856bc3
//!
//! Command byte layout: bit 7 (0x80) is ALLOW_REVERT (the command may fail
//! on-chain without reverting the tx); the low bits carry the command id,
//! masked version-dependently on-chain (0x1f v1.2, 0x3f 2.0.0, 0x7f 2.1.1+).
//! The decoder strips 0x80 and dispatches ids <= 0x3f only.
//!
//! Decoded swap commands (all others are skipped — fail-honest, R8):
//!   0x00 V3_SWAP_EXACT_IN   (address recipient, uint256 amountIn,
//!                            uint256 amountOutMin, bytes path, bool payerIsUser)
//!   0x01 V3_SWAP_EXACT_OUT  (address recipient, uint256 amountOut,
//!                            uint256 amountInMax, bytes path, bool payerIsUser)
//!                            — V3 exact-out paths are encoded REVERSED
//!                            (tokenOut first); they are flipped back to
//!                            execution order before decoding.
//!   0x08 V2_SWAP_EXACT_IN   (address recipient, uint256 amountIn,
//!                            uint256 amountOutMin, address[] path, bool payerIsUser)
//!   0x09 V2_SWAP_EXACT_OUT  (address recipient, uint256 amountOut,
//!                            uint256 amountInMax, address[] path, bool payerIsUser)
//!
//! Sentinel values are real on-chain conventions, not decoder errors (R8):
//!   - recipient 0x0000...0001 = MSG_SENDER, 0x0000...0002 = ADDRESS_THIS —
//!     passed through raw.
//!   - V2 amountIn == 0 = ALREADY_PAID: the pair was pre-funded by a prior
//!     command, so the real amount is NOT derivable from calldata (and is NOT
//!     tx.value) — it stays 0 downstream.
//!   - V3 exact-in amountIn >= 1<<255 = CONTRACT_BALANCE (v1.x routers): the
//!     spend is the router's token balance — not determinable offline, the
//!     swap is skipped (see `decode_v3_swap`).

use super::{
    parse_v3_path_bytes_with_fees, DecodeFailReason, DecodedSwap, ProtocolType, SwapExactMode,
};
use ethers::abi::{decode as abi_decode, ParamType};
use ethers::types::{Address, U256};

/// execute(bytes commands, bytes[] inputs)
const SEL_EXECUTE: [u8; 4] = [0x35, 0x93, 0x56, 0x4c];
/// execute(bytes commands, bytes[] inputs, uint256 deadline)
const SEL_EXECUTE_WITH_DEADLINE: [u8; 4] = [0x24, 0x85, 0x6b, 0xc3];

const CMD_V3_SWAP_EXACT_IN: u8 = 0x00;
const CMD_V3_SWAP_EXACT_OUT: u8 = 0x01;
const CMD_V2_SWAP_EXACT_IN: u8 = 0x08;
const CMD_V2_SWAP_EXACT_OUT: u8 = 0x09;

/// ALLOW_REVERT flag of the command byte (bit 7, 0x80): the command may fail
/// on-chain without reverting the whole tx. Stripped before id dispatch.
/// (COMMAND_TYPE_MASK itself is version-dependent on-chain — 0x1f v1.2,
/// 0x3f 2.0.0, 0x7f 2.1.1+ — so the accepted id range is enforced at the
/// dispatch site instead of a single mask const.)
const FLAG_ALLOW_REVERT: u8 = 0x80;

/// Cap on decoded swaps per execute() batch — mirrors `route_decoder`'s
/// MAX_INTENTS_PER_TX bound against command fan-out explosion.
const MAX_SWAPS_PER_TX: usize = 16;

/// Uniswap V2 fixed LP fee in basis points (0.30%) — protocol-level constant
/// (same documented R8 exception as `calldata/univ2.rs`).
const V2_FEE_BPS: u32 = 30;

fn selector_hex(s: [u8; 4]) -> String {
    format!("0x{:02x}{:02x}{:02x}{:02x}", s[0], s[1], s[2], s[3])
}

/// Dispatcher-compatible single-swap decode: returns the FIRST swap command
/// of the execute() batch. Returns `UnsupportedSelector` when the selector is
/// unknown or the batch carries no decodable swap (legacy single-intent
/// callers then count the tx as undecoded — fail-honest).
pub fn decode(selector: [u8; 4], body: &[u8]) -> Result<DecodedSwap, DecodeFailReason> {
    match decode_all(selector, body)?.into_iter().next() {
        Some(s) => Ok(s),
        None => Err(DecodeFailReason::UnsupportedSelector),
    }
}

/// Decode every swap command of an execute() batch, in command order.
///
/// Non-swap commands and individually malformed swap inputs are skipped
/// (fail-honest): a tx with zero decodable swaps yields an empty vec, never
/// an error. Errors are reserved for tx-level failures: unknown selector,
/// broken ABI encoding, or commands/inputs length mismatch (the router itself
/// reverts on-chain with `LengthMismatch` in that case).
pub fn decode_all(selector: [u8; 4], body: &[u8]) -> Result<Vec<DecodedSwap>, DecodeFailReason> {
    let (commands, inputs, deadline) = parse_execute(selector, body)?;

    let mut swaps = Vec::new();
    for (raw_cmd, input) in commands.iter().zip(inputs.iter()) {
        // Strip ALLOW_REVERT (bit 7), then dispatch ids <= 0x3f only: ids
        // 0x40-0x7f are undefined on every deployed version, and decoding
        // them would invent phantom swaps (e.g. 0x48 reverts on 2.1.1 yet
        // would otherwise land here as V2 exact-in). Tradeoff: rare v1.2
        // encodings that set the don't-care bit 0x20 are skipped fail-honest
        // instead of decoded.
        let cmd = raw_cmd & !FLAG_ALLOW_REVERT;
        if cmd > 0x3f {
            continue;
        }
        let decoded = match cmd {
            CMD_V3_SWAP_EXACT_IN => decode_v3_swap(selector, deadline, input, true),
            CMD_V3_SWAP_EXACT_OUT => decode_v3_swap(selector, deadline, input, false),
            CMD_V2_SWAP_EXACT_IN => decode_v2_swap(selector, deadline, input, true),
            CMD_V2_SWAP_EXACT_OUT => decode_v2_swap(selector, deadline, input, false),
            // PERMIT2_*, WRAP_ETH, UNWRAP_WETH, TRANSFER, SWEEP, NFT markets,
            // V4, ... — no decodable V2/V3 path: skipped fail-honest.
            _ => None,
        };
        if let Some(s) = decoded {
            swaps.push(s);
            if swaps.len() >= MAX_SWAPS_PER_TX {
                break;
            }
        }
    }
    Ok(swaps)
}

/// Parsed `execute()` arguments: (commands, inputs, deadline).
type ExecuteArgs = (Vec<u8>, Vec<Vec<u8>>, U256);

/// Parse the execute() arguments into (commands, inputs, deadline).
/// `deadline` is zero for the no-deadline selector overload.
///
/// The envelope is hand-parsed instead of going through ethabi: ethabi 18's
/// Array(Bytes) decode tolerates aliased element offsets, so a hostile
/// ~128KB body could fan out into ~128MB of copied bytes (429-rate DoS
/// amplification). Every read below is bounds-checked, element offsets must
/// be strictly increasing (aliasing impossible), and the declared inputs
/// count must equal commands.len() — the router itself reverts
/// LengthMismatch otherwise.
fn parse_execute(selector: [u8; 4], body: &[u8]) -> Result<ExecuteArgs, DecodeFailReason> {
    let with_deadline = match selector {
        SEL_EXECUTE => false,
        SEL_EXECUTE_WITH_DEADLINE => true,
        _ => return Err(DecodeFailReason::UnsupportedSelector),
    };
    match parse_execute_args(body, with_deadline) {
        Some(args) => Ok(args),
        None => Err(DecodeFailReason::AbiDecodeError),
    }
}

/// Bounds-checked hand parser for the `(bytes commands, bytes[] inputs[, uint256 deadline])`
/// head/tail layout. Returns `None` on any violation.
fn parse_execute_args(body: &[u8], with_deadline: bool) -> Option<ExecuteArgs> {
    let head_words = if with_deadline { 3 } else { 2 };
    if body.len() < head_words * 32 {
        return None;
    }

    // (a) head: offset words for the two dynamic args (+ inline deadline).
    // NOTE: word_at_as_bound takes BYTE positions — word 1 lives at byte 32.
    let commands_off = word_at_as_bound(body, 0)?;
    let inputs_off = word_at_as_bound(body, 32)?;
    let deadline = if with_deadline {
        U256::from_big_endian(body.get(64..96)?)
    } else {
        U256::zero()
    };

    // (b) commands: offset -> len -> bytes.
    let commands = read_dyn_bytes(body, commands_off)?;

    // (c) inputs array head, peek-only: the declared count must match
    // commands.len() (semantic kept from the router's LengthMismatch revert).
    let count = word_at_as_bound(body, inputs_off)?;
    if count != commands.len() {
        return None;
    }
    let arr_head_end = inputs_off
        .checked_add(32)?
        .checked_add(count.checked_mul(32)?)?;
    if arr_head_end > body.len() {
        return None;
    }

    // (d) walk the element offsets. Per the ABI spec (and ethabi's decoder,
    // decoder.rs:163-166), element offsets are relative to the position AFTER
    // the array length word — `inputs_off + 32` — not to the length word
    // itself. Offsets must be strictly increasing — equal or decreasing
    // offsets would alias an earlier element, the amplification vector.
    let mut inputs = Vec::with_capacity(count);
    let elems_base = inputs_off.checked_add(32)?;
    let mut head_pos = elems_base;
    let mut prev_rel: Option<usize> = None;
    for _ in 0..count {
        let rel = word_at_as_bound(body, head_pos)?;
        if prev_rel.is_some_and(|p| rel <= p) {
            return None;
        }
        prev_rel = Some(rel);
        inputs.push(read_dyn_bytes(body, elems_base.checked_add(rel)?)?);
        head_pos = head_pos.checked_add(32)?;
    }

    Some((commands, inputs, deadline))
}

/// Read the 32-byte word at `pos` as a bounded offset/length. Rejects words
/// that cannot address `body` (non-zero high bytes or value > body.len()),
/// which also keeps every later sum far from usize overflow.
fn word_at_as_bound(body: &[u8], pos: usize) -> Option<usize> {
    let w = body.get(pos..pos.checked_add(32)?)?;
    if w[..24].iter().any(|&b| b != 0) {
        return None;
    }
    let v = u64::from_be_bytes(w[24..32].try_into().ok()?);
    if v > body.len() as u64 {
        return None;
    }
    Some(v as usize)
}

/// Read an ABI `bytes` value located at absolute offset `abs` (length word +
/// payload), bounds-checked, returning an owned copy.
fn read_dyn_bytes(body: &[u8], abs: usize) -> Option<Vec<u8>> {
    let len = word_at_as_bound(body, abs)?;
    let start = abs.checked_add(32)?;
    let end = start.checked_add(len)?;
    if end > body.len() {
        return None;
    }
    Some(body[start..end].to_vec())
}

/// V3 exact-in CONTRACT_BALANCE sentinel (v1.x routers): amountIn with the
/// top bit set (>= 1<<255) tells the router to spend its entire token
/// balance — the real amount is not determinable offline.
fn is_v3_contract_balance(amount: U256) -> bool {
    amount >= (U256::one() << 255)
}

/// Decode a V3_SWAP_EXACT_IN / V3_SWAP_EXACT_OUT command input.
///
/// ABI: (address recipient, uint256 amount0, uint256 amount1, bytes path,
/// bool payerIsUser). For exact-in, amount0 = amountIn / amount1 =
/// amountOutMin; for exact-out, amount0 = amountOut / amount1 = amountInMax
/// and the packed path is REVERSED (tokenOut first).
fn decode_v3_swap(
    selector: [u8; 4],
    deadline: U256,
    input: &[u8],
    exact_in: bool,
) -> Option<DecodedSwap> {
    let mut params = vec![
        ParamType::Address,
        ParamType::Uint(256),
        ParamType::Uint(256),
        ParamType::Bytes,
        ParamType::Bool,
    ];
    // UR 2.1.1 (0x4c82...) appends a 6th field, uint256[] minHopPriceX36, to
    // the swap inputs (5-field layout on 1.2.2 / 2.0.0); ethabi strict decode
    // rejects the trailing data, so retry with the array appended and ignore
    // its value.
    let tokens = abi_decode(&params, input)
        .or_else(|_| {
            params.push(ParamType::Array(Box::new(ParamType::Uint(256))));
            abi_decode(&params, input)
        })
        .ok()?;

    let recipient = tokens.first().and_then(|t| t.clone().into_address())?;
    let amount0 = tokens.get(1).and_then(|t| t.clone().into_uint())?;
    let amount1 = tokens.get(2).and_then(|t| t.clone().into_uint())?;
    let path_bytes = tokens.get(3).and_then(|t| t.clone().into_bytes())?;
    // tokens[4] (payerIsUser) selects the funds source; it does not affect
    // the route — ignored. Same for the 2.1.1 minHopPriceX36 tail (tokens[5]).

    // Exact-in CONTRACT_BALANCE sentinel: the spend is the router's balance,
    // unknowable offline — skip rather than let the sentinel flow downstream
    // as a literal amount (R8; it would also blow up U256::as_u128() sizing).
    if exact_in && is_v3_contract_balance(amount0) {
        return None;
    }

    let (mut path_tokens, mut fees) = parse_v3_path_bytes_with_fees(&path_bytes)?;
    if !exact_in {
        // Exact-out paths are encoded reversed — flip tokens and per-hop fees
        // back to execution order (token_in ... token_out).
        path_tokens.reverse();
        fees.reverse();
    }
    let token_in = *path_tokens.first()?;
    let token_out = *path_tokens.last()?;

    let (amount_in, min_amount_out, exact_mode) = if exact_in {
        (amount0, amount1, SwapExactMode::ExactIn)
    } else {
        // amount_in carries the upper bound (amountInMax); min_amount_out
        // carries the exact output — same convention as univ2/univ3 decoders.
        (amount1, amount0, SwapExactMode::ExactOut)
    };

    Some(DecodedSwap {
        router: "universal-router",
        token_in,
        token_out,
        amount_in,
        min_amount_out,
        path_len: path_tokens.len() as u32,
        deadline,
        recipient,
        selector_hex: selector_hex(selector),
        path_tokens,
        path_fees_bps: fees,
        exact_mode,
        protocol_type: ProtocolType::V3,
    })
}

/// Decode a V2_SWAP_EXACT_IN / V2_SWAP_EXACT_OUT command input.
///
/// ABI: (address recipient, uint256 amount0, uint256 amount1, address[] path,
/// bool payerIsUser). Path is in normal execution order for both variants.
fn decode_v2_swap(
    selector: [u8; 4],
    deadline: U256,
    input: &[u8],
    exact_in: bool,
) -> Option<DecodedSwap> {
    let mut params = vec![
        ParamType::Address,
        ParamType::Uint(256),
        ParamType::Uint(256),
        ParamType::Array(Box::new(ParamType::Address)),
        ParamType::Bool,
    ];
    // UR 2.1.1 (0x4c82...) appends a 6th field, uint256[] minHopPriceX36, to
    // the swap inputs (5-field layout on 1.2.2 / 2.0.0); ethabi strict decode
    // rejects the trailing data, so retry with the array appended and ignore
    // its value.
    let tokens = abi_decode(&params, input)
        .or_else(|_| {
            params.push(ParamType::Array(Box::new(ParamType::Uint(256))));
            abi_decode(&params, input)
        })
        .ok()?;

    let recipient = tokens.first().and_then(|t| t.clone().into_address())?;
    let amount0 = tokens.get(1).and_then(|t| t.clone().into_uint())?;
    let amount1 = tokens.get(2).and_then(|t| t.clone().into_uint())?;
    let path = tokens
        .get(3)
        .and_then(|t| t.clone().into_array())?
        .into_iter()
        .map(|t| t.into_address())
        .collect::<Option<Vec<Address>>>()?;
    if path.len() < 2 {
        return None;
    }
    let token_in = *path.first()?;
    let token_out = *path.last()?;
    let hop_count = path.len() - 1;

    let (amount_in, min_amount_out, exact_mode) = if exact_in {
        (amount0, amount1, SwapExactMode::ExactIn)
    } else {
        (amount1, amount0, SwapExactMode::ExactOut)
    };

    Some(DecodedSwap {
        router: "universal-router",
        token_in,
        token_out,
        amount_in,
        min_amount_out,
        path_len: path.len() as u32,
        deadline,
        recipient,
        selector_hex: selector_hex(selector),
        path_tokens: path,
        path_fees_bps: vec![V2_FEE_BPS; hop_count],
        exact_mode,
        protocol_type: ProtocolType::V2,
    })
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use ethers::abi::{encode, Token};

    fn token(n: u64) -> Address {
        Address::from_low_u64_be(n)
    }

    /// Pack a V3 path: token(20) [fee(3) token(20)]* — fees in hundredths of
    /// a bip as stored on-chain (e.g. 3000 = 0.30%).
    fn v3_path(tokens: &[Address], fees: &[u32]) -> Vec<u8> {
        assert_eq!(tokens.len(), fees.len() + 1);
        let mut out = tokens[0].as_bytes().to_vec();
        for (i, fee) in fees.iter().enumerate() {
            out.extend_from_slice(&fee.to_be_bytes()[1..]); // uint24 BE
            out.extend_from_slice(tokens[i + 1].as_bytes());
        }
        out
    }

    fn v3_input(recipient: Address, a: u64, b: u64, path: Vec<u8>) -> Vec<u8> {
        encode(&[
            Token::Address(recipient),
            Token::Uint(U256::from(a)),
            Token::Uint(U256::from(b)),
            Token::Bytes(path),
            Token::Bool(true),
        ])
    }

    /// `v3_input` with a raw U256 first amount (sentinel tests).
    fn v3_input_raw(recipient: Address, a: U256, b: u64, path: Vec<u8>) -> Vec<u8> {
        encode(&[
            Token::Address(recipient),
            Token::Uint(a),
            Token::Uint(U256::from(b)),
            Token::Bytes(path),
            Token::Bool(true),
        ])
    }

    fn v2_input(recipient: Address, a: u64, b: u64, path: Vec<Address>) -> Vec<u8> {
        encode(&[
            Token::Address(recipient),
            Token::Uint(U256::from(a)),
            Token::Uint(U256::from(b)),
            Token::Array(path.into_iter().map(Token::Address).collect()),
            Token::Bool(true),
        ])
    }

    fn execute_body(
        selector: [u8; 4],
        commands: Vec<u8>,
        inputs: Vec<Vec<u8>>,
        deadline: Option<u64>,
    ) -> Vec<u8> {
        let mut tokens = vec![
            Token::Bytes(commands),
            Token::Array(inputs.into_iter().map(Token::Bytes).collect()),
        ];
        if let Some(d) = deadline {
            tokens.push(Token::Uint(U256::from(d)));
        }
        let _ = selector;
        encode(&tokens)
    }

    #[test]
    fn v3_exact_in_decodes_path_and_fees() {
        let path = v3_path(&[token(0xA), token(0xB)], &[3000]);
        let body = execute_body(
            SEL_EXECUTE,
            vec![0x00],
            vec![v3_input(token(0xCC), 1_000, 900, path)],
            None,
        );
        let swaps = decode_all(SEL_EXECUTE, &body).unwrap();
        assert_eq!(swaps.len(), 1);
        let s = &swaps[0];
        assert_eq!(s.router, "universal-router");
        assert_eq!(s.token_in, token(0xA));
        assert_eq!(s.token_out, token(0xB));
        assert_eq!(s.amount_in, U256::from(1_000u64));
        assert_eq!(s.min_amount_out, U256::from(900u64));
        assert_eq!(s.path_tokens, vec![token(0xA), token(0xB)]);
        assert_eq!(s.path_fees_bps, vec![30]);
        assert_eq!(s.exact_mode, SwapExactMode::ExactIn);
        assert_eq!(s.protocol_type, ProtocolType::V3);
        assert_eq!(s.deadline, U256::zero());
        assert_eq!(s.recipient, token(0xCC));
    }

    #[test]
    fn v3_exact_out_reversed_path_is_flipped() {
        // Encoded path is REVERSED: tokenOut (B) first, then fee, then tokenIn (A).
        let path = v3_path(&[token(0xB), token(0xA)], &[500]);
        let body = execute_body(
            SEL_EXECUTE,
            vec![0x01],
            vec![v3_input(token(0xCC), 950, 1_100, path)],
            None,
        );
        let swaps = decode_all(SEL_EXECUTE, &body).unwrap();
        assert_eq!(swaps.len(), 1);
        let s = &swaps[0];
        assert_eq!(s.token_in, token(0xA));
        assert_eq!(s.token_out, token(0xB));
        assert_eq!(s.path_tokens, vec![token(0xA), token(0xB)]);
        assert_eq!(s.path_fees_bps, vec![5]);
        assert_eq!(s.amount_in, U256::from(1_100u64)); // amountInMax (upper bound)
        assert_eq!(s.min_amount_out, U256::from(950u64)); // amountOut (exact)
        assert_eq!(s.exact_mode, SwapExactMode::ExactOut);
    }

    #[test]
    fn v3_exact_out_multi_hop_reverses_tokens_and_fees() {
        // Encoded exact-out path is REVERSED: tokenOut (C) first, fees in
        // encoded order [500, 3000]. Flipped back to execution order:
        // tokens [A, B, C] with per-hop fees [30, 5] bps. Reversing the
        // tokens WITHOUT the fees (the regression this pins against) would
        // yield [5, 30].
        let path = v3_path(&[token(0xC), token(0xB), token(0xA)], &[500, 3000]);
        let body = execute_body(
            SEL_EXECUTE,
            vec![0x01],
            vec![v3_input(token(0xCC), 950, 1_100, path)],
            None,
        );
        let swaps = decode_all(SEL_EXECUTE, &body).unwrap();
        assert_eq!(swaps.len(), 1);
        let s = &swaps[0];
        assert_eq!(s.path_tokens, vec![token(0xA), token(0xB), token(0xC)]);
        assert_eq!(s.path_fees_bps, vec![30, 5]);
        assert_eq!(s.token_in, token(0xA));
        assert_eq!(s.token_out, token(0xC));
    }

    #[test]
    fn v2_exact_out_command_0x09_amounts_remapped() {
        // 0x09 V2_SWAP_EXACT_OUT: amount0 = amountOut (exact), amount1 =
        // amountInMax (upper bound); the path stays in execution order
        // (only V3 exact-out paths are encoded reversed).
        let body = execute_body(
            SEL_EXECUTE,
            vec![0x09],
            vec![v2_input(
                token(0xCC),
                950,
                1_100,
                vec![token(0xA), token(0xB)],
            )],
            None,
        );
        let swaps = decode_all(SEL_EXECUTE, &body).unwrap();
        assert_eq!(swaps.len(), 1);
        let s = &swaps[0];
        assert_eq!(s.token_in, token(0xA));
        assert_eq!(s.token_out, token(0xB));
        assert_eq!(s.path_tokens, vec![token(0xA), token(0xB)]);
        assert_eq!(s.amount_in, U256::from(1_100u64)); // amountInMax
        assert_eq!(s.min_amount_out, U256::from(950u64)); // amountOut (exact)
        assert_eq!(s.exact_mode, SwapExactMode::ExactOut);
        assert_eq!(s.protocol_type, ProtocolType::V2);
    }

    #[test]
    fn v3_exact_in_contract_balance_sentinel_skipped() {
        // amountIn = 1<<255 (and anything >= it) is the v1.x CONTRACT_BALANCE
        // sentinel: the real spend is the router's balance, unknowable
        // offline. The swap must be skipped (R8), never decoded with the
        // sentinel as a literal amount (downstream U256::as_u128() sizing
        // would panic on it).
        let path = v3_path(&[token(0xA), token(0xB)], &[3000]);
        let sentinel = U256::one() << 255;
        for amount in [sentinel, sentinel + U256::one()] {
            let body = execute_body(
                SEL_EXECUTE,
                vec![0x00],
                vec![v3_input_raw(token(0xCC), amount, 900, path.clone())],
                None,
            );
            let swaps = decode_all(SEL_EXECUTE, &body).unwrap();
            assert!(swaps.is_empty(), "sentinel amount must be skipped");
        }
    }

    #[test]
    fn decode_all_caps_at_16_swaps() {
        // 20 swap commands: the loop must early-exit at MAX_SWAPS_PER_TX
        // (mirrors route_decoder's MAX_INTENTS_PER_TX bound).
        let n = 20;
        let inputs: Vec<Vec<u8>> = (0..n)
            .map(|_| v2_input(token(0xCC), 1_000, 900, vec![token(0xA), token(0xB)]))
            .collect();
        let body = execute_body(SEL_EXECUTE, vec![0x08; n], inputs, None);
        let swaps = decode_all(SEL_EXECUTE, &body).unwrap();
        assert_eq!(swaps.len(), 16);
    }

    #[test]
    fn v3_swap_input_six_field_2_1_1_layout_decodes() {
        // UR 2.1.1 appends uint256[] minHopPriceX36 to the V3 swap input —
        // both layouts must decode identical swap values.
        let path = v3_path(&[token(0xA), token(0xB)], &[3000]);
        let five = v3_input(token(0xCC), 1_000, 900, path.clone());
        let six = encode(&[
            Token::Address(token(0xCC)),
            Token::Uint(U256::from(1_000u64)),
            Token::Uint(U256::from(900u64)),
            Token::Bytes(path),
            Token::Bool(true),
            Token::Array(vec![
                Token::Uint(U256::from(123u64)),
                Token::Uint(U256::from(456u64)),
            ]),
        ]);
        for input in [&five[..], &six[..]] {
            let body = execute_body(SEL_EXECUTE, vec![0x00], vec![input.to_vec()], None);
            let swaps = decode_all(SEL_EXECUTE, &body).unwrap();
            assert_eq!(swaps.len(), 1);
            let s = &swaps[0];
            assert_eq!(s.token_in, token(0xA));
            assert_eq!(s.token_out, token(0xB));
            assert_eq!(s.amount_in, U256::from(1_000u64));
            assert_eq!(s.min_amount_out, U256::from(900u64));
            assert_eq!(s.path_fees_bps, vec![30]);
        }
    }

    #[test]
    fn v2_swap_input_six_field_2_1_1_layout_decodes() {
        // UR 2.1.1 appends uint256[] minHopPriceX36 to the V2 swap input —
        // both layouts must decode identical swap values.
        let five = v2_input(
            token(0xCC),
            2_000,
            1_800,
            vec![token(0xA), token(0xB), token(0xC)],
        );
        let six = encode(&[
            Token::Address(token(0xCC)),
            Token::Uint(U256::from(2_000u64)),
            Token::Uint(U256::from(1_800u64)),
            Token::Array(vec![
                Token::Address(token(0xA)),
                Token::Address(token(0xB)),
                Token::Address(token(0xC)),
            ]),
            Token::Bool(true),
            Token::Array(vec![Token::Uint(U256::from(7u64))]),
        ]);
        for input in [&five[..], &six[..]] {
            let body = execute_body(SEL_EXECUTE, vec![0x08], vec![input.to_vec()], None);
            let swaps = decode_all(SEL_EXECUTE, &body).unwrap();
            assert_eq!(swaps.len(), 1);
            let s = &swaps[0];
            assert_eq!(s.path_tokens, vec![token(0xA), token(0xB), token(0xC)]);
            assert_eq!(s.amount_in, U256::from(2_000u64));
            assert_eq!(s.min_amount_out, U256::from(1_800u64));
            assert_eq!(s.protocol_type, ProtocolType::V2);
        }
    }

    #[test]
    fn v2_exact_in_multi_hop() {
        let body = execute_body(
            SEL_EXECUTE,
            vec![0x08],
            vec![v2_input(
                token(0xCC),
                2_000,
                1_800,
                vec![token(0xA), token(0xB), token(0xC)],
            )],
            None,
        );
        let swaps = decode_all(SEL_EXECUTE, &body).unwrap();
        assert_eq!(swaps.len(), 1);
        let s = &swaps[0];
        assert_eq!(s.path_tokens, vec![token(0xA), token(0xB), token(0xC)]);
        assert_eq!(s.path_fees_bps, vec![30, 30]);
        assert_eq!(s.protocol_type, ProtocolType::V2);
        assert_eq!(s.path_len, 3);
        // Amounts match the encoded input exactly.
        assert_eq!(s.amount_in, U256::from(2_000u64));
        assert_eq!(s.min_amount_out, U256::from(1_800u64));
        assert_eq!(s.exact_mode, SwapExactMode::ExactIn);
    }

    #[test]
    fn multi_command_batch_yields_one_swap_per_command() {
        let v3 = v3_input(
            token(0xCC),
            1_000,
            900,
            v3_path(&[token(0xA), token(0xB)], &[3000]),
        );
        let v2 = v2_input(token(0xCC), 2_000, 1_800, vec![token(0xC), token(0xD)]);
        let body = execute_body(SEL_EXECUTE, vec![0x00, 0x08], vec![v3, v2], None);
        let swaps = decode_all(SEL_EXECUTE, &body).unwrap();
        assert_eq!(swaps.len(), 2);
        assert_eq!(swaps[0].protocol_type, ProtocolType::V3);
        assert_eq!(swaps[1].protocol_type, ProtocolType::V2);
    }

    #[test]
    fn allow_revert_flag_0x80_is_stripped() {
        // ALLOW_REVERT is bit 7 (0x80): 0x80|0x08 = 0x88 still decodes the
        // V2 exact-in swap underneath the flag.
        let v2 = v2_input(token(0xCC), 1_000, 900, vec![token(0xA), token(0xB)]);
        let body = execute_body(SEL_EXECUTE, vec![0x80 | 0x08], vec![v2], None);
        let swaps = decode_all(SEL_EXECUTE, &body).unwrap();
        assert_eq!(
            swaps.len(),
            1,
            "0x80 ALLOW_REVERT flag must be stripped off the command id"
        );
    }

    #[test]
    fn command_id_above_0x3f_is_skipped() {
        // 0x48: after stripping 0x80 the id is 0x48 > 0x3f — undefined on
        // every deployed router version (on 2.1.1 it reverts). Decoding it
        // as V2 exact-in would fabricate a phantom swap: must be skipped.
        let v2 = v2_input(token(0xCC), 1_000, 900, vec![token(0xA), token(0xB)]);
        let body = execute_body(SEL_EXECUTE, vec![0x48], vec![v2], None);
        let swaps = decode_all(SEL_EXECUTE, &body).unwrap();
        assert!(swaps.is_empty());
    }

    #[test]
    fn non_swap_commands_are_skipped() {
        // 0x0b = WRAP_ETH — input layout irrelevant, never parsed.
        let body = execute_body(SEL_EXECUTE, vec![0x0b], vec![vec![0xde, 0xad]], None);
        let swaps = decode_all(SEL_EXECUTE, &body).unwrap();
        assert!(swaps.is_empty());
        // Dispatcher-compat single decode maps "no swap" to UnsupportedSelector.
        assert!(matches!(
            decode(SEL_EXECUTE, &body),
            Err(DecodeFailReason::UnsupportedSelector)
        ));
    }

    #[test]
    fn mixed_batch_skips_non_swaps_keeps_swaps() {
        let v2 = v2_input(token(0xCC), 1_000, 900, vec![token(0xA), token(0xB)]);
        let body = execute_body(
            SEL_EXECUTE,
            vec![0x0b, 0x08, 0x0c],
            vec![vec![], v2, vec![]],
            None,
        );
        let swaps = decode_all(SEL_EXECUTE, &body).unwrap();
        assert_eq!(swaps.len(), 1);
        assert_eq!(swaps[0].protocol_type, ProtocolType::V2);
    }

    #[test]
    fn deadline_overload_propagates_deadline() {
        let v2 = v2_input(token(0xCC), 1_000, 900, vec![token(0xA), token(0xB)]);
        let body = execute_body(SEL_EXECUTE_WITH_DEADLINE, vec![0x08], vec![v2], Some(9_999));
        let swaps = decode_all(SEL_EXECUTE_WITH_DEADLINE, &body).unwrap();
        assert_eq!(swaps[0].deadline, U256::from(9_999u64));
    }

    #[test]
    fn length_mismatch_is_an_error() {
        let body = execute_body(SEL_EXECUTE, vec![0x08, 0x08], vec![vec![]], None);
        assert!(matches!(
            decode_all(SEL_EXECUTE, &body),
            Err(DecodeFailReason::AbiDecodeError)
        ));
    }

    #[test]
    fn unknown_selector_is_unsupported() {
        assert!(matches!(
            decode_all([0xde, 0xad, 0xbe, 0xef], &[]),
            Err(DecodeFailReason::UnsupportedSelector)
        ));
    }

    #[test]
    fn malformed_body_does_not_panic() {
        assert!(matches!(
            decode_all(SEL_EXECUTE, &[0x00, 0x01, 0x02]),
            Err(DecodeFailReason::AbiDecodeError)
        ));
    }

    #[test]
    fn malformed_swap_input_is_skipped_not_fatal() {
        // One good V2 swap + one truncated V3 swap input: the good one survives.
        let v2 = v2_input(token(0xCC), 1_000, 900, vec![token(0xA), token(0xB)]);
        let body = execute_body(
            SEL_EXECUTE,
            vec![0x00, 0x08],
            vec![vec![0x01, 0x02], v2],
            None,
        );
        let swaps = decode_all(SEL_EXECUTE, &body).unwrap();
        assert_eq!(swaps.len(), 1);
        assert_eq!(swaps[0].protocol_type, ProtocolType::V2);
    }
}
