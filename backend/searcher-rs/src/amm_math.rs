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
use ethers::types::{Address, Bytes, U256};
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
    let denominator = reserve_in.saturating_mul(U256::from(10_000u32)).saturating_add(amount_in_with_fee);
    if denominator.is_zero() {
        return U256::zero();
    }
    numerator / denominator
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
                ParamType::Address,        // tokenIn
                ParamType::Address,        // tokenOut
                ParamType::Uint(256),      // amountIn
                ParamType::Uint(24),       // fee
                ParamType::Uint(160),      // sqrtPriceLimitX96
            ]),
            internal_type: None,
        }],
        outputs: vec![
            Param { name: "amountOut".to_string(), kind: ParamType::Uint(256), internal_type: None },
            Param { name: "sqrtPriceX96After".to_string(), kind: ParamType::Uint(160), internal_type: None },
            Param { name: "initializedTicksCrossed".to_string(), kind: ParamType::Uint(32), internal_type: None },
            Param { name: "gasEstimate".to_string(), kind: ParamType::Uint(256), internal_type: None },
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
    let raw_bytes = match tokio::time::timeout(
        V3_QUOTE_MULTICALL_TIMEOUT,
        provider.call(tx),
    )
    .await
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
    use super::*;

    /// UniswapV2Library reference: amount_in=1e18 (1 WETH), reserves=(3000e18 WETH, 6_000_000e6 USDC).
    /// Expected: roughly 1995 USDC (less than 2000 due to fee + slippage on 1/3000th of pool).
    /// Hand-computed: amount_in_with_fee = 9.97e21
    ///                numerator   = 9.97e21 * 6e12 = 5.982e34
    ///                denominator = 3e25 + 9.97e21 ≈ 3.00997e25
    ///                out         = 5.982e34 / 3.00997e25 ≈ 1.987e9 → 1987 USDC (6 decimals)
    /// We assert within 5% of 1987e6.
    #[test]
    fn weth_to_usdc_realistic_pool() {
        let amount_in = U256::from(10u128).pow(18.into());                  // 1 WETH
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
        let out = v2_amount_out(U256::zero(), U256::from(1_000u128), U256::from(2_000u128), 30);
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
        assert!(with_fee < no_fee, "with_fee={} should be < no_fee={}", with_fee, no_fee);
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
        let rt = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
        let result = rt.block_on(async {
            let provider = std::sync::Arc::new(
                ProviderBuilder::new()
                    .disable_recommended_fillers()
                    .connect_http("http://invalid:0".parse().unwrap())
            );
            v3_quote_exact_in_multicall(
                provider,
                Address::from_str("0x61fFE014bA17989E743c5F6cB21bF9697530B21e").unwrap(),
                Address::from_str("0xcA11bde05977b3631167028862bE2a173976CA11").unwrap(),
                vec![],
            ).await
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
                pool_addr: Address::from_str("0x88e6A0c2dDD26FEEb64F039a2c41296FcB3f5640")
                    .unwrap(),
                token_in: Address::from_str("0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2")
                    .unwrap(),
                token_out: Address::from_str("0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48")
                    .unwrap(),
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
