//! V2 CPMM math + V3 batch quoting via Multicall3.
//!
//! References:
//! - UniswapV2Library.getAmountOut:
//!   https://github.com/Uniswap/v2-periphery/blob/master/contracts/libraries/UniswapV2Library.sol#L43-L50
//! - Uniswap V3 QuoterV2 (mainnet 0x61fFE014bA17989E743c5F6cB21bF9697530B21e):
//!   https://docs.uniswap.org/contracts/v3/reference/periphery/lens/QuoterV2
//! - Multicall3 (mainnet 0xcA11bde05977b3631167028862bE2a173976CA11):
//!   https://github.com/mds1/multicall
//!
//! Doctrine: math is parametrised by `fee_bps` (basis points of 10_000). Default 30 = 0.30%
//! used by both UniswapV2 and SushiSwap; V3 fee tiers are 100/500/3000/10000 (uint24).

use alloy::primitives::Address as AlloyAddress;
use alloy::providers::Provider as AlloyProvider;
use alloy::rpc::types::{TransactionInput, TransactionRequest};
use alloy::sol_types::SolCall;
use ethers::abi::{Function, Param, ParamType, StateMutability, Token};
use ethers::types::{Address, Bytes, U256, U512};
use shared_rs::rpc_failover::AlloyHttpProvider;
use std::sync::Arc;
use std::time::Duration;

/// Hard upper bound on a single multicall RPC. A stalled RPC (e.g. provider
/// hung on a TCP retransmit, dead VPN tunnel, blackhole) WITHOUT this guard
/// would freeze the entire worker tick indefinitely (cs-validator MAJOR fix
/// 2026-05-06). On timeout the multicall returns Err, caller treats it as a
/// whole-batch RPC failure, increments the per-pool failure counter, and
/// proceeds to the next tick. Five seconds is generous (Alchemy p99 ≈ 200ms
/// even for 50-call multicalls) but avoids tripping on ordinary congestion.
const V3_QUOTE_MULTICALL_TIMEOUT: Duration = Duration::from_secs(5);

/// Convert a wei-denominated decimal string to f64 token units using the
/// token's actual decimal precision.
///
/// `wei_str` — decimal integer string from EVM calldata (e.g. "49589584000000000").
/// `decimals` — token's decimal precision: 18 for WETH/most ERC-20s, 6 for USDC/USDT,
///              8 for WBTC, etc.
///
/// Returns 0.0 if the string fails to parse. Lossy past ~15 significant figures
/// (f64 mantissa). Never re-fed into on-chain arithmetic — display/scoring path only.
///
/// **Why this exists (BUG-1, 2026-05-04):** scanner.rs previously divided by 1e18
/// unconditionally, ignoring `TokenMeta.decimals`. For 6-decimal tokens (USDT, USDC)
/// this collapsed amount_in to ~0, making BUG-3's capital cap a no-op and producing
/// downstream ROI in the billions of percent. Always pass the token's true decimals.
pub fn wei_str_to_token_units(wei_str: &str, decimals: u8) -> f64 {
    // M11 fix (audit 2026-05-10): explicit warn on parse failure instead of
    // silent 0.0.  R8 fail-honest: a bad wei string is a data quality event
    // that must be visible in logs so the upstream calldata decoder can be
    // corrected.  Value is still clamped to 0.0 because callers are in the
    // display/scoring path and cannot propagate a Result.
    let raw: f64 = match wei_str.parse::<f64>() {
        Ok(v) => v,
        Err(e) => {
            tracing::warn!(
                wei_str,
                decimals,
                error = %e,
                "amm_math::wei_str_to_token_units: parse failed — returning 0.0 (M11)"
            );
            0.0
        }
    };
    raw / 10f64.powi(decimals as i32)
}

/// V2 constant-product market maker output amount, post-fee.
///
/// Formula (UniswapV2Library.getAmountOut):
///     amount_in_with_fee = amount_in * (10_000 - fee_bps)
///     numerator          = amount_in_with_fee * reserve_out
///     denominator        = reserve_in * 10_000 + amount_in_with_fee
///     amount_out         = numerator / denominator
///
/// Returns U256::zero() on degenerate inputs (zero reserves or zero amount_in).
pub fn v2_amount_out(amount_in: U256, reserve_in: U256, reserve_out: U256, fee_bps: u32) -> U256 {
    if amount_in.is_zero() || reserve_in.is_zero() || reserve_out.is_zero() {
        return U256::zero();
    }
    let fee_factor = U256::from(10_000u32 - fee_bps);
    let amount_in_with_fee = amount_in.saturating_mul(fee_factor);
    let numerator = amount_in_with_fee.saturating_mul(reserve_out);
    let denominator = reserve_in
        .saturating_mul(U256::from(10_000u32))
        .saturating_add(amount_in_with_fee);
    if denominator.is_zero() {
        return U256::zero();
    }
    numerator / denominator
}

/// Full-precision `a * b / denom` with no intermediate overflow.
///
/// V3 single-tick math multiplies `liquidity` (uint128) by `sqrtPriceX96`
/// (uint160) and `Q96` (2^96), whose products reach ~2^288–2^384 — far past
/// U256 (2^256). We widen to U512 via `U256::full_mul`, divide, then narrow
/// back (the quotient is a token amount that fits in U256). Returns
/// `U256::zero()` on a zero denominator (caller treats as degenerate).
#[allow(dead_code)]
pub fn mul_div(a: U256, b: U256, denom: U256) -> U256 {
    if denom.is_zero() {
        return U256::zero();
    }
    let prod: U512 = a.full_mul(b); // U256 × U256 → U512, lossless
                                    // Widen denom to U512 via big-endian bytes (low 32 of the 64-byte buffer).
    let mut db = [0u8; 64];
    denom.to_big_endian(&mut db[32..64]);
    let denom512 = U512::from_big_endian(&db);
    let q: U512 = prod / denom512;
    // Narrow back to U256 (low 256 bits; quotient fits for token-amount formulas).
    let mut qb = [0u8; 64];
    q.to_big_endian(&mut qb);
    U256::from_big_endian(&qb[32..64])
}

/// Uniswap V3 **single-active-tick** exact-input output amount, post-fee.
///
/// Canonical concentrated-liquidity swap math (UniswapV3 SwapMath / whitepaper
/// §6.2), evaluated WITHIN the current tick — i.e. assuming `liquidity` stays
/// constant over the price move. Exact when the swap does not cross a tick
/// boundary; for larger swaps it OVERESTIMATES output (real V3 has less
/// liquidity beyond the active tick). Per-tick liquidity is not cached
/// (`V3Slot0Entry` holds only sqrtPriceX96 + liquidity), so cross-tick cannot
/// be computed locally — callers cap `amount_in` by the capital gate (small vs
/// deep-pool liquidity ⇒ within-tick is the common case) and MAY confirm the
/// chosen size on-chain via `v3_quote_exact_in_multicall` (QuoterV2).
///
/// `fee_pips` is the V3 fee tier in millionths (100/500/3000/10000 = 0.01/0.05/
/// 0.30/1.00 %) — NOT the V2 basis-point convention. All arithmetic is integer
/// (U256/U512); no f64, no fabricated output (RULE 00). Returns `U256::zero()`
/// on degenerate input or a non-physical price move.
#[allow(dead_code)]
pub fn v3_amount_out_single_tick(
    amount_in: U256,
    sqrt_price_x96: U256,
    liquidity: U256,
    fee_pips: u32,
    zero_for_one: bool,
) -> U256 {
    if amount_in.is_zero()
        || sqrt_price_x96.is_zero()
        || liquidity.is_zero()
        || fee_pips >= 1_000_000
    {
        return U256::zero();
    }
    let q96 = U256::one() << 96u32; // Q96 = 2^96
    let l = liquidity;
    let sp = sqrt_price_x96;

    // amount net of fee: amount_in * (1e6 - fee_pips) / 1e6
    let amount_in_less_fee = mul_div(
        amount_in,
        U256::from(1_000_000u32 - fee_pips),
        U256::from(1_000_000u32),
    );
    if amount_in_less_fee.is_zero() {
        return U256::zero();
    }

    if zero_for_one {
        // token0 → token1; price (√P) falls.
        // √P_next = (L·√P) / (L + Δx·√P/Q96)
        let denom = l.saturating_add(mul_div(amount_in_less_fee, sp, q96));
        if denom.is_zero() {
            return U256::zero();
        }
        let sp_next = mul_div(l, sp, denom);
        if sp_next.is_zero() || sp_next >= sp {
            return U256::zero(); // price must fall; guard non-physical result
        }
        // amount_out (token1) = L·(√P − √P_next)/Q96
        mul_div(l, sp - sp_next, q96)
    } else {
        // token1 → token0; price (√P) rises.
        // √P_next = √P + Δy·Q96/L
        let sp_next = sp.saturating_add(mul_div(amount_in_less_fee, q96, l));
        if sp_next <= sp {
            return U256::zero();
        }
        // amount_out (token0) = L·Q96/√P − L·Q96/√P_next  (avoids √P_next·√P overflow)
        let inv_sp = mul_div(l, q96, sp);
        let inv_sp_next = mul_div(l, q96, sp_next);
        inv_sp.saturating_sub(inv_sp_next)
    }
}

// ============================================================================
// Uniswap V3 marginal (spot) pricing from slot0 — RU-2
// ============================================================================

/// 2^96 as f64 — the fixed-point denominator of `sqrtPriceX96`. Written as the
/// exact integer 79228162514264337593543950336; a power of two is exactly
/// representable in f64 (no rounding on conversion).
const Q96_F64: f64 = 79_228_162_514_264_337_593_543_950_336.0;

/// Marginal V3 pricing snapshot derived from `slot0.sqrtPriceX96` + `liquidity()`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct V3SpotSnapshot {
    /// Marginal (spot) rate in human units — token1 per token0:
    /// `(sqrtPriceX96 / 2^96)² · 10^(dec0 − dec1)`. This is the price at zero
    /// trade size (the price AT the active tick), the correct magnitude for an
    /// MMBF edge weight; how fast it degrades with size (depth across ticks) is
    /// the QuoterV2 / `v3_amount_out_single_tick` path, not this one.
    pub rate_01: f64,
    /// TVL proxy unit-consistent with the V2 graph-builder hint (`r0 + r1`
    /// normalized by decimals): the virtual reserves of the constant-product
    /// pool that reproduces this tick's local behaviour —
    /// `L/√P · 10^−dec0 + L·√P · 10^−dec1`. Without this, ranking parallel
    /// V2/V3 pools of the same pair would compare token units against raw
    /// uint128 liquidity (~17 orders of magnitude apart).
    pub virtual_reserves_hint: f64,
}

/// Compute the V3 marginal spot rate + virtual-reserves hint from the slot0
/// snapshot (RU-2: real `log_weight` for V3 graph edges).
///
/// `sqrt_price_x96` / `liquidity` are the raw on-chain uint160/uint128 values
/// (`V3Slot0Entry` stores them as decimal strings; `pool_sync_worker` refuses
/// to cache sqrtPrice > u128::MAX, so u128 covers the cached domain).
/// `tickSpacing` is deliberately NOT an input: it parameterizes tick
/// granularity — how liquidity distributes ACROSS ticks — not the spot price
/// AT the active tick, and it is not part of the cached snapshot.
///
/// R8 fail-honest: returns `None` — never a synthetic value — when either
/// input is zero (uninitialized pool / empty range ⇒ no price exists) or any
/// derived magnitude is non-finite or non-positive.
///
/// f64 precision: converting u128 inputs rounds to a 53-bit mantissa
/// (~1e-16 relative) — ample for a ranking weight whose fee component is
/// 1e-4..1e-2. The executable path keeps integer math
/// (`v3_amount_out_single_tick` / QuoterV2); this is scoring only (RULE 00).
pub fn v3_spot_snapshot(
    sqrt_price_x96: u128,
    liquidity: u128,
    dec0: u8,
    dec1: u8,
) -> Option<V3SpotSnapshot> {
    if sqrt_price_x96 == 0 || liquidity == 0 {
        return None; // uninitialized pool or empty active range — no price exists
    }
    let sp = sqrt_price_x96 as f64;
    let l = liquidity as f64;
    let sqrt_p = sp / Q96_F64; // √P in raw token1-wei per token0-wei
    let raw_price = sqrt_p * sqrt_p;
    if !(raw_price.is_finite() && raw_price > 0.0) {
        return None;
    }
    let rate_01 = raw_price * 10f64.powi(dec0 as i32 - dec1 as i32);
    if !(rate_01.is_finite() && rate_01 > 0.0) {
        return None;
    }
    // Virtual reserves (raw wei): x = L/√P, y = L·√P. Both stay finite for the
    // full u128 input domain (products ≤ ~1e77 << f64::MAX).
    let x_v = l / sqrt_p;
    let y_v = l * sqrt_p;
    if !(x_v.is_finite() && x_v > 0.0 && y_v.is_finite() && y_v > 0.0) {
        return None;
    }
    let hint = x_v / 10f64.powi(dec0 as i32) + y_v / 10f64.powi(dec1 as i32);
    if hint.is_finite() && hint > 0.0 {
        Some(V3SpotSnapshot {
            rate_01,
            virtual_reserves_hint: hint,
        })
    } else {
        None
    }
}

// ============================================================================
// Uniswap V3 batch quoting via Multicall3
// ============================================================================

/// Multicall3 ABI definitions via alloy `sol!`. Kept in a private sub-module
/// to avoid `Call3` / `aggregate3Call` name collisions with
/// `workers::pool_sync_worker` (which defines its own identical types).
///
/// Alloy 1.0 migration: replaced ethers `abigen!` with `sol!` so the
/// `aggregate3` calldata is encoded/decoded via `alloy_sol_types::SolCall`,
/// and the RPC call goes through the alloy `Provider` trait (`provider.call`).
mod multicall3 {
    use alloy::sol_types::sol;

    sol! {
        interface IMulticall3 {
            struct Call3 {
                address target;
                bool allowFailure;
                bytes callData;
            }
            struct Result {
                bool success;
                bytes returnData;
            }
            function aggregate3(Call3[] calldata calls) external payable returns (Result[] memory returnData);
        }
    }

    pub use IMulticall3::{aggregate3Call, Call3};
}

/// One V3 quote request (tied to a specific pool / fee tier / direction).
#[derive(Clone, Debug)]
pub struct V3QuoteRequest {
    pub pool_addr: Address,
    pub token_in: Address,
    pub token_out: Address,
    pub amount_in: U256,
    /// V3 fee tier in basis points (uint24): 100, 500, 3000, 10000.
    pub fee_bps: u32,
}

/// Per-pool V3 quote result. On per-pool failure (e.g. insufficient liquidity,
/// wrong fee tier, pool revert), `success=false` and `amount_out=U256::zero()`.
#[derive(Clone, Debug)]
pub struct V3QuoteResult {
    pub pool_addr: Address,
    pub amount_out: U256,
    pub success: bool,
}

/// Build the ABI descriptor for `quoteExactInputSingle((address,address,uint256,uint24,uint160))`.
///
/// We encode calldata manually rather than via `abigen!` because the inline
/// struct-arg form has historically been finicky with the macro's parser
/// (the same reason `pool_sync_worker.rs` uses inline JSON ABI). Manual
/// encoding via `ethers::abi::Function::encode_input` is fully type-safe.
fn quoter_v2_function() -> Function {
    #[allow(deprecated)]
    Function {
        name: "quoteExactInputSingle".to_string(),
        inputs: vec![Param {
            name: "params".to_string(),
            kind: ParamType::Tuple(vec![
                ParamType::Address,   // tokenIn
                ParamType::Address,   // tokenOut
                ParamType::Uint(256), // amountIn
                ParamType::Uint(24),  // fee
                ParamType::Uint(160), // sqrtPriceLimitX96
            ]),
            internal_type: None,
        }],
        outputs: vec![
            Param {
                name: "amountOut".to_string(),
                kind: ParamType::Uint(256),
                internal_type: None,
            },
            Param {
                name: "sqrtPriceX96After".to_string(),
                kind: ParamType::Uint(160),
                internal_type: None,
            },
            Param {
                name: "initializedTicksCrossed".to_string(),
                kind: ParamType::Uint(32),
                internal_type: None,
            },
            Param {
                name: "gasEstimate".to_string(),
                kind: ParamType::Uint(256),
                internal_type: None,
            },
        ],
        constant: None,
        state_mutability: StateMutability::NonPayable,
    }
}

/// Encode a single `quoteExactInputSingle` call as calldata bytes.
fn encode_quote_calldata(req: &V3QuoteRequest) -> anyhow::Result<Bytes> {
    let f = quoter_v2_function();
    let tuple = Token::Tuple(vec![
        Token::Address(req.token_in),
        Token::Address(req.token_out),
        Token::Uint(req.amount_in),
        Token::Uint(U256::from(req.fee_bps)),
        Token::Uint(U256::zero()), // sqrtPriceLimitX96 = 0 → no limit
    ]);
    let encoded = f.encode_input(&[tuple])?;
    Ok(Bytes::from(encoded))
}

/// Batch-quote multiple V3 pools in a single Multicall3 RPC.
///
/// On RPC failure: returns Err.
/// On per-pool failure (e.g. insufficient liquidity, pool reverts): the
/// corresponding `V3QuoteResult` has `success=false` and `amount_out=U256::zero()`.
/// Empty input short-circuits and returns `Ok(vec![])` without an RPC call.
///
/// The returned vector has the same length and order as `quotes`.
///
/// Alloy 1.0 migration: provider type changed from ethers `Arc<Provider<Http>>`
/// to `Arc<AlloyHttpProvider>`. The ABI encoding of `quoteExactInputSingle`
/// calldata (per-pool, ethers `abi::encode`) is unchanged — only the outer
/// `aggregate3` envelope and the `eth_call` go through alloy.
pub async fn v3_quote_exact_in_multicall(
    provider: Arc<AlloyHttpProvider>,
    quoter_addr: Address,
    multicall_addr: Address,
    quotes: Vec<V3QuoteRequest>,
) -> anyhow::Result<Vec<V3QuoteResult>> {
    if quotes.is_empty() {
        return Ok(vec![]);
    }

    // Build per-pool calls targeting the QuoterV2 (NOT the individual pool address).
    // Each call encodes the pool's quoteExactInputSingle params; the target is
    // always the quoter contract (quoter_addr). The multicall target contract
    // (multicall_addr) then batches all these calls into a single eth_call.
    let alloy_quoter = AlloyAddress::from_slice(quoter_addr.as_bytes());
    let calls: Vec<multicall3::Call3> = quotes
        .iter()
        .map(|q| {
            let calldata = encode_quote_calldata(q)?;
            Ok::<_, anyhow::Error>(multicall3::Call3 {
                target: alloy_quoter,
                allowFailure: true,
                callData: calldata.to_vec().into(),
            })
        })
        .collect::<anyhow::Result<Vec<_>>>()?;

    // Encode the aggregate3(Call3[]) calldata via alloy sol! ABI encoder.
    let calldata = multicall3::aggregate3Call { calls }.abi_encode();

    // Build alloy TransactionRequest for eth_call.
    let multicall_alloy = AlloyAddress::from_slice(multicall_addr.as_bytes());
    let tx = TransactionRequest::default()
        .to(multicall_alloy)
        .input(TransactionInput::new(calldata.into()));

    // Hard timeout (cs-validator MAJOR fix 2026-05-06). On timeout the caller
    // treats the whole batch as failed and counts each per-pool quote as a
    // failure (existing whole-batch failure counter at triangular_worker.rs
    // ~line 1493). Without this, a stalled provider would freeze the worker
    // tick indefinitely.
    let raw_bytes = match tokio::time::timeout(V3_QUOTE_MULTICALL_TIMEOUT, provider.call(tx)).await
    {
        Ok(Ok(b)) => b,
        Ok(Err(e)) => return Err(e.into()),
        Err(_elapsed) => {
            return Err(anyhow::anyhow!(
                "multicall timeout after {}s",
                V3_QUOTE_MULTICALL_TIMEOUT.as_secs()
            ));
        }
    };

    // Decode the aggregate3 return: Result[] (struct { bool success; bytes returnData; }[]).
    // alloy-sol-types 1.0: for a single return value, `abi_decode_returns` returns
    // the inner type directly — `Vec<Result>` in this case.
    let results = multicall3::aggregate3Call::abi_decode_returns(&raw_bytes)
        .map_err(|e| anyhow::anyhow!("aggregate3 decode failed: {e}"))?;

    // Decode each result. amountOut is the first 32 bytes of QuoterV2 return data
    // (uint256, big-endian, left-padded). We don't need the other return values.
    let mut out = Vec::with_capacity(quotes.len());
    for (req, res) in quotes.iter().zip(results.iter()) {
        if res.success && res.returnData.len() >= 32 {
            let amount_out = U256::from_big_endian(&res.returnData[0..32]);
            out.push(V3QuoteResult {
                pool_addr: req.pool_addr,
                amount_out,
                success: true,
            });
        } else {
            out.push(V3QuoteResult {
                pool_addr: req.pool_addr,
                amount_out: U256::zero(),
                success: false,
            });
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)] // M11: test module — panics are acceptable
    use super::*;

    /// ARBX-R-0003 cross-ABI pin: `rpc_failover`'s load-probe calldata (the
    /// half-open eth_call that reopens rate-limited breakers) must be byte-equal
    /// to THIS crate's `IMulticall3.aggregate3` encoding of an empty call list.
    /// If either side drifts, the load probe would send calldata no contract
    /// understands and every reopen decision silently changes meaning.
    #[test]
    fn r0003_load_probe_calldata_pins_multicall3_aggregate3_empty() {
        use alloy::sol_types::SolCall;
        let encoded = multicall3::aggregate3Call { calls: vec![] }.abi_encode();
        assert_eq!(
            encoded.as_slice(),
            shared_rs::rpc_failover::LOAD_PROBE_CALLDATA,
            "LOAD_PROBE_CALLDATA drifted from the amm_math multicall3 ABI — regenerate it from \
             aggregate3Call {{ calls: vec![] }}.abi_encode()"
        );
    }

    /// UniswapV2Library reference: amount_in=1e18 (1 WETH), reserves=(3000e18 WETH, 6_000_000e6 USDC).
    /// Expected: roughly 1995 USDC (less than 2000 due to fee + slippage on 1/3000th of pool).
    /// Hand-computed: amount_in_with_fee = 9.97e21
    ///                numerator   = 9.97e21 * 6e12 = 5.982e34
    ///                denominator = 3e25 + 9.97e21 ≈ 3.00997e25
    ///                out         = 5.982e34 / 3.00997e25 ≈ 1.987e9 → 1987 USDC (6 decimals)
    /// We assert within 5% of 1987e6.
    #[test]
    fn weth_to_usdc_realistic_pool() {
        let amount_in = U256::from(10u128).pow(18.into()); // 1 WETH
        let reserve_in = U256::from(3000u128) * U256::from(10u128).pow(18.into()); // 3000 WETH
        let reserve_out = U256::from(6_000_000u128) * U256::from(10u128).pow(6.into()); // 6M USDC
        let out = v2_amount_out(amount_in, reserve_in, reserve_out, 30);
        let expected = U256::from(1987u128) * U256::from(10u128).pow(6.into());
        // ±5%
        let lo = expected * U256::from(95) / U256::from(100);
        let hi = expected * U256::from(105) / U256::from(100);
        assert!(out >= lo && out <= hi, "got {} expected ~{}", out, expected);
    }

    #[test]
    fn fee_zero_matches_pure_xy() {
        // With fee_bps=0, amount_out = amount_in * reserve_out / (reserve_in + amount_in)
        let amount_in = U256::from(100u128);
        let reserve_in = U256::from(1_000u128);
        let reserve_out = U256::from(2_000u128);
        let out = v2_amount_out(amount_in, reserve_in, reserve_out, 0);
        // Manual: 100 * 2000 / (1000 + 100) = 200_000 / 1100 = 181 (truncated)
        assert_eq!(out, U256::from(181u128));
    }

    #[test]
    fn zero_amount_in_returns_zero() {
        let out = v2_amount_out(
            U256::zero(),
            U256::from(1_000u128),
            U256::from(2_000u128),
            30,
        );
        assert_eq!(out, U256::zero());
    }

    #[test]
    fn zero_reserve_in_returns_zero() {
        let out = v2_amount_out(U256::from(100u128), U256::zero(), U256::from(2_000u128), 30);
        assert_eq!(out, U256::zero());
    }

    #[test]
    fn zero_reserve_out_returns_zero() {
        let out = v2_amount_out(U256::from(100u128), U256::from(1_000u128), U256::zero(), 30);
        assert_eq!(out, U256::zero());
    }

    #[test]
    fn fee_30_bps_reduces_output_vs_zero_fee() {
        let amount_in = U256::from(1_000_000u128);
        let reserve_in = U256::from(10_000_000u128);
        let reserve_out = U256::from(20_000_000u128);
        let no_fee = v2_amount_out(amount_in, reserve_in, reserve_out, 0);
        let with_fee = v2_amount_out(amount_in, reserve_in, reserve_out, 30);
        assert!(
            with_fee < no_fee,
            "with_fee={} should be < no_fee={}",
            with_fee,
            no_fee
        );
    }

    // ------------------------------------------------------------------
    // wei_str_to_token_units — decimal-aware conversion (BUG-1 prevention)
    // ------------------------------------------------------------------

    #[test]
    fn wei_str_weth_18_decimals() {
        // 1 WETH = 1e18 wei → 1.0 token units
        assert_eq!(wei_str_to_token_units("1000000000000000000", 18), 1.0);
    }

    #[test]
    fn wei_str_usdt_6_decimals() {
        // 1 USDT raw (6 decimals) = "1000000" → 1.0 token units.
        // Pre-fix scanner code (always /1e18) returned 1e-12 here — the
        // root cause of the 42-billion-% ROI in production.
        assert_eq!(wei_str_to_token_units("1000000", 6), 1.0);
    }

    #[test]
    fn wei_str_wbtc_8_decimals() {
        // 1 WBTC raw (8 decimals) = "100000000" → 1.0 token units.
        assert_eq!(wei_str_to_token_units("100000000", 8), 1.0);
    }

    #[test]
    fn wei_str_zero_amount() {
        assert_eq!(wei_str_to_token_units("0", 18), 0.0);
        assert_eq!(wei_str_to_token_units("0", 6), 0.0);
    }

    #[test]
    fn wei_str_invalid_returns_zero() {
        // Preserves prior fallback behaviour (silent zero on parse failure)
        // so callers don't need to distinguish "unknown" from "zero" — both
        // safely flow into the gate as "no opportunity to act on".
        assert_eq!(wei_str_to_token_units("not_a_number", 18), 0.0);
        assert_eq!(wei_str_to_token_units("", 6), 0.0);
    }

    #[test]
    fn wei_str_bug1_regression_usdt_input() {
        // Reproduces the production live event 2026-05-04 11:01:14 (USDT-WETH):
        //   amount_in_wei = "10000000000" (= 10,000 USDT raw with 6 decimals)
        //   pre-fix:  10000000000 / 1e18 = 1e-8           → amount_in_usd ≈ $0
        //   post-fix: 10000000000 / 1e6  = 10000          → amount_in_usd ≈ $10K
        //
        // With the right value the BUG-3 capital cap correctly limits exposure
        // to operator capital (≤$10) instead of letting the input collapse to
        // zero and the cap become a no-op.
        let result = wei_str_to_token_units("10000000000", 6);
        assert!(
            (result - 10_000.0).abs() < 1e-6,
            "expected 10000 USDT, got {}",
            result
        );

        // Sanity: the old buggy semantic produced a ~1e-8 value, confirming
        // the scenario this fix prevents.
        let old_buggy = "10000000000".parse::<f64>().unwrap_or(0.0) / 1e18;
        assert!(
            old_buggy < 1e-7,
            "old buggy value was {}, confirming the bug semantic",
            old_buggy
        );
    }
}

#[cfg(test)]
mod v3_tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)] // M11: test module — panics are acceptable
    use super::*;
    use alloy::providers::ProviderBuilder;
    use ethers::types::{Address, U256};
    use std::str::FromStr;

    #[test]
    fn v3_quote_request_construction() {
        let req = V3QuoteRequest {
            pool_addr: Address::from_str("0x88e6A0c2dDD26FEEb64F039a2c41296FcB3f5640").unwrap(),
            token_in: Address::from_str("0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2").unwrap(),
            token_out: Address::from_str("0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48").unwrap(),
            amount_in: U256::from(10).pow(U256::from(18)),
            fee_bps: 500,
        };
        // Field accessibility check.
        assert_eq!(req.fee_bps, 500);
    }

    #[test]
    fn fee_bps_value_is_passed_as_uint24() {
        // Sanity: fee_bps is u32 in the struct but represents uint24 on chain.
        // 100/500/3000/10000 should all fit in u24 (max 16_777_215).
        for fee in [100u32, 500, 3000, 10000] {
            assert!(fee < 1 << 24, "fee {} doesn't fit in uint24", fee);
        }
    }

    #[test]
    fn v3_quote_result_failure_zero_amount_out() {
        // When the V3 quote fails (e.g., no liquidity), V3QuoteResult.success=false
        // and amount_out=0. Verify the type contract.
        let r = V3QuoteResult {
            pool_addr: Address::zero(),
            amount_out: U256::zero(),
            success: false,
        };
        assert!(!r.success);
        assert_eq!(r.amount_out, U256::zero());
    }

    #[test]
    fn empty_quotes_returns_empty_vec() {
        // Calling v3_quote_exact_in_multicall with empty input must short-circuit
        // and return Ok(vec![]) — no RPC call made.
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        let result = rt.block_on(async {
            let provider = std::sync::Arc::new(
                ProviderBuilder::new()
                    .disable_recommended_fillers()
                    .connect_http("http://invalid:0".parse().unwrap()),
            );
            v3_quote_exact_in_multicall(
                provider,
                Address::from_str("0x61fFE014bA17989E743c5F6cB21bF9697530B21e").unwrap(),
                Address::from_str("0xcA11bde05977b3631167028862bE2a173976CA11").unwrap(),
                vec![],
            )
            .await
        });
        assert!(result.is_ok());
        assert_eq!(result.unwrap().len(), 0);
    }

    /// cs-validator MAJOR fix 2026-05-06 — `v3_quote_exact_in_multicall` must
    /// NOT freeze indefinitely if the RPC stalls.
    ///
    /// We stand up a local TCP listener that accepts the connection but never
    /// responds (simulates a stalled provider). The wrapper enforces a 5s
    /// timeout via `tokio::time::timeout`, with `tokio::time::pause()` enabled
    /// virtual time advances instantly so the test completes in milliseconds
    /// rather than burning a real 5s of wall clock.
    ///
    /// Asserts: the call returns Err whose message contains "multicall timeout".
    #[test]
    fn v3_quote_multicall_times_out_after_5s() {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .start_paused(true) // virtual clock — `tokio::time::timeout` advances instantly
            .build()
            .unwrap();
        let result: anyhow::Result<Vec<V3QuoteResult>> = rt.block_on(async {
            // Bind a TCP listener that accepts then ignores forever.
            let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
            let addr = listener.local_addr().unwrap();
            tokio::spawn(async move {
                loop {
                    // Accept and hold — never write a response.
                    let (mut stream, _) = match listener.accept().await {
                        Ok(p) => p,
                        Err(_) => return,
                    };
                    tokio::spawn(async move {
                        // Hold the socket open forever (but not blocking the test).
                        let mut buf = [0u8; 1024];
                        let _ = tokio::io::AsyncReadExt::read(&mut stream, &mut buf).await;
                        std::future::pending::<()>().await;
                    });
                }
            });

            let url = format!("http://{}", addr);
            let provider = Arc::new(
                ProviderBuilder::new()
                    .disable_recommended_fillers()
                    .connect_http(url.parse().unwrap()),
            );
            let req = V3QuoteRequest {
                pool_addr: Address::from_str("0x88e6A0c2dDD26FEEb64F039a2c41296FcB3f5640").unwrap(),
                token_in: Address::from_str("0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2").unwrap(),
                token_out: Address::from_str("0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48").unwrap(),
                amount_in: U256::from(10).pow(U256::from(18)),
                fee_bps: 500,
            };
            v3_quote_exact_in_multicall(
                provider,
                Address::from_str("0x61fFE014bA17989E743c5F6cB21bF9697530B21e").unwrap(),
                Address::from_str("0xcA11bde05977b3631167028862bE2a173976CA11").unwrap(),
                vec![req],
            )
            .await
        });

        // Result must be Err — a stalled RPC must NOT freeze the worker.
        let err = result.expect_err("stalled RPC must surface as Err, not hang");
        let msg = format!("{}", err);
        assert!(
            msg.to_lowercase().contains("timeout"),
            "expected timeout error, got: {}",
            msg
        );
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod v3_single_tick_tests {
    use super::*;

    fn p10(n: u32) -> U256 {
        U256::from(10u64).pow(U256::from(n))
    }
    fn q96() -> U256 {
        U256::one() << 96u32
    }

    // ---- mul_div ----------------------------------------------------------
    #[test]
    fn mul_div_small_exact() {
        assert_eq!(
            mul_div(U256::from(6u64), U256::from(7u64), U256::from(2u64)),
            U256::from(21u64)
        );
        assert_eq!(
            mul_div(U256::from(100u64), U256::from(3u64), U256::from(7u64)),
            U256::from(42u64)
        ); // floor(300/7)
    }

    #[test]
    fn mul_div_overflows_u256_intermediate_but_quotient_fits() {
        // a*b = 2^300 (exceeds U256 max 2^256-1); / 2^150 = 2^150 (fits). Proves the U512 path.
        let a = U256::one() << 200u32;
        let b = U256::one() << 100u32;
        let denom = U256::one() << 150u32;
        assert_eq!(mul_div(a, b, denom), U256::one() << 150u32);
    }

    #[test]
    fn mul_div_zero_denom_is_zero() {
        assert_eq!(
            mul_div(U256::from(5u64), U256::from(5u64), U256::zero()),
            U256::zero()
        );
    }

    // ---- v3_amount_out_single_tick ----------------------------------------
    #[test]
    fn v3_degenerate_inputs_return_zero() {
        let sp = q96();
        let l = p10(18);
        assert_eq!(
            v3_amount_out_single_tick(U256::zero(), sp, l, 3000, true),
            U256::zero()
        );
        assert_eq!(
            v3_amount_out_single_tick(p10(12), U256::zero(), l, 3000, true),
            U256::zero()
        );
        assert_eq!(
            v3_amount_out_single_tick(p10(12), sp, U256::zero(), 3000, true),
            U256::zero()
        );
        assert_eq!(
            v3_amount_out_single_tick(p10(12), sp, l, 1_000_000, true),
            U256::zero()
        ); // fee >= 100%
    }

    #[test]
    fn v3_at_1to1_price_zero_fee_equals_cpmm_virtual_reserves() {
        // At sqrtPriceX96 = 2^96 (price 1:1) the V3 single-tick swap reduces EXACTLY to a
        // constant-product pool with virtual reserves (L, L). Cross-check against the proven
        // v2_amount_out — this pins the canonical formula end-to-end.
        let sp = q96();
        let l = p10(18);
        let amt = p10(12);
        let v3_z4o = v3_amount_out_single_tick(amt, sp, l, 0, true);
        let v3_o4z = v3_amount_out_single_tick(amt, sp, l, 0, false);
        let v2 = v2_amount_out(amt, l, l, 0);
        // zero_for_one reduces to the integer CPMM L·Δx/(L+Δx) bit-for-bit.
        assert_eq!(
            v3_z4o, v2,
            "zero_for_one single-tick must equal CPMM(L,L) at 1:1"
        );
        // Integer V3 single-tick math is NOT bit-symmetric across direction: the two
        // directions use different floor-division paths (zero_for_one: L·Δ√P/Q96;
        // one_for_zero: differences of L·Q96/√P), so they may disagree by ±1 wei — the
        // known per-direction rounding of Uniswap V3 SwapMath (always toward the pool).
        // Negligible for the bracket role; the profit number comes from QuoterV2 (Step 2b).
        let diff = if v3_o4z > v2 {
            v3_o4z - v2
        } else {
            v2 - v3_o4z
        };
        assert!(
            diff <= U256::one(),
            "one_for_zero symmetric at 1:1 within 1 wei (got {v3_o4z} vs {v2})"
        );
        assert!(
            v3_z4o > U256::zero() && v3_z4o < amt,
            "output positive and < input (slippage)"
        );
    }

    #[test]
    fn v3_fee_reduces_output() {
        let sp = q96();
        let l = p10(18);
        let amt = p10(12);
        let no_fee = v3_amount_out_single_tick(amt, sp, l, 0, true);
        let with_fee = v3_amount_out_single_tick(amt, sp, l, 3000, true); // 0.30%
        assert!(with_fee < no_fee, "fee must reduce output");
        assert!(with_fee > U256::zero());
    }

    #[test]
    fn v3_output_monotonic_in_input() {
        let sp = q96();
        let l = p10(20);
        let small = v3_amount_out_single_tick(p10(12), sp, l, 500, true);
        let big = v3_amount_out_single_tick(p10(12) * U256::from(2u64), sp, l, 500, true);
        assert!(big > small, "more input must yield more output");
    }

    #[test]
    fn v3_deeper_liquidity_less_slippage() {
        // Same trade in a deeper pool keeps more of the input (output closer to amount_in).
        let sp = q96();
        let amt = p10(15);
        let shallow = v3_amount_out_single_tick(amt, sp, p10(18), 0, true);
        let deep = v3_amount_out_single_tick(amt, sp, p10(24), 0, true);
        assert!(
            deep > shallow,
            "deeper liquidity = less slippage = more output"
        );
        assert!(
            deep < amt,
            "still bounded by input (zero-fee, positive slippage)"
        );
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod v3_spot_snapshot_tests {
    use super::*;

    fn q96_u128() -> u128 {
        1u128 << 96
    }

    #[test]
    fn unit_price_same_decimals_is_one_and_hint_is_two_virtual_reserves() {
        // sqrtPriceX96 = 2^96 → raw price exactly 1.0. With dec0 == dec1 the
        // human rate is 1.0, and the virtual reserves are (L, L) — so the hint
        // is 2L normalized (mirrors the CPMM(L,L) reduction pinned by
        // `v3_at_1to1_price_zero_fee_equals_cpmm_virtual_reserves`).
        let l = 10u128.pow(18);
        let snap = v3_spot_snapshot(q96_u128(), l, 18, 18).unwrap();
        assert!((snap.rate_01 - 1.0).abs() < 1e-12, "rate={}", snap.rate_01);
        assert!(
            (snap.virtual_reserves_hint - 2.0).abs() < 1e-9,
            "hint={}",
            snap.virtual_reserves_hint
        );
    }

    #[test]
    fn usdc_weth_mainnet_vector() {
        // Real USDC/WETH 0.05% pool shape: token0=USDC (6 dec), token1=WETH
        // (18 dec). sqrtPriceX96 ≈ 1.7727e33 → √P ≈ 22372.8 → raw price
        // ≈ 5.0054e8 wei-WETH per wei-USDC → human ≈ 5.0054e-4 WETH per USDC
        // (i.e. ETH ≈ $2000 — sanity-checks the decimal adjustment sign).
        let sp = 1_772_712_074_874_819_459_120_282_715_246_463u128;
        let l = 548_640_024_015_773_269u128;
        let snap = v3_spot_snapshot(sp, l, 6, 18).unwrap();
        let expected = 5.0054e-4;
        assert!(
            ((snap.rate_01 - expected) / expected).abs() < 1e-3,
            "rate={} expected ~{}",
            snap.rate_01,
            expected
        );
        // Virtual reserves: x = L/√P ≈ 2.45e7 USDC, y = L·√P ≈ 1.2e4 WETH.
        assert!(snap.virtual_reserves_hint.is_finite());
        assert!(snap.virtual_reserves_hint > 1.0e6);
        assert!(snap.virtual_reserves_hint < 1.0e9);
    }

    #[test]
    fn decimal_adjustment_shifts_rate_by_ten_pow_dec_delta() {
        // Same sqrtPrice with swapped decimals models two orientations of the
        // same raw price: the human rate shifts by exactly 10^(Δdec) —
        // rate(18,6)/rate(6,18) = 10^24 here. (A real WETH-first pool of the
        // same pair would carry the RECIPROCAL sqrtPrice; the cycle-level
        // symmetry that matters — lw01+lw10 = −2·ln(1−fee), rate cancels — is
        // pinned in graph_builder.)
        let sp = 1_772_712_074_874_819_459_120_282_715_246_463u128;
        let l = 548_640_024_015_773_269u128;
        let a = v3_spot_snapshot(sp, l, 6, 18).unwrap().rate_01;
        let b = v3_spot_snapshot(sp, l, 18, 6).unwrap().rate_01;
        let ratio = b / a;
        assert!(
            (ratio / 1e24 - 1.0).abs() < 1e-9,
            "ratio={ratio:e} expected 1e24"
        );
    }

    #[test]
    fn zero_sqrt_price_or_liquidity_is_none() {
        // Uninitialized pool (sqrt=0) or empty active range (L=0): no price
        // exists — None, never a synthetic rate (R8).
        let l = 10u128.pow(18);
        assert_eq!(v3_spot_snapshot(0, l, 18, 18), None);
        assert_eq!(v3_spot_snapshot(q96_u128(), 0, 18, 18), None);
    }

    #[test]
    fn extreme_u128_inputs_stay_finite() {
        // Full u128 domain must not panic or produce ±∞/NaN — the graph edge
        // built from it must never carry a non-finite weight.
        let snap = v3_spot_snapshot(u128::MAX, u128::MAX, 18, 0).unwrap();
        assert!(snap.rate_01.is_finite() && snap.rate_01 > 0.0);
        assert!(snap.virtual_reserves_hint.is_finite() && snap.virtual_reserves_hint > 0.0);
    }
}
