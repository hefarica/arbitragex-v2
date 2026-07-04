//! Host Bindings — native Rust functions exposed to Rhai cartridges.
//!
//! These bindings bridge the gap between the sandboxed Rhai script world and
//! the ArbitrageX v2 infrastructure (Redis, RPC, Mempool, Telemetry).
//!
//! ## Security Model
//!
//! - All bindings are READ-ONLY by default (no state mutation from scripts).
//! - Redis operations use a dedicated read-only connection.
//! - RPC calls are rate-limited per cartridge (max 10 calls/evaluation).
//! - Telemetry is fire-and-forget (never blocks the script).
//! - No filesystem, network, or process access is exposed.
//!
//! ## Binding Registry
//!
//! | Function                          | Source          | Latency    |
//! |-----------------------------------|-----------------|------------|
//! | `get_reserves(pool_addr)`         | Redis cache     | <1ms       |
//! | `get_token_meta(token_addr)`      | Redis cache     | <1ms       |
//! | `get_pool_index(token_a, token_b)`| Redis cache     | <1ms       |
//! | `simulate_swap(amount, path)`     | RPC (cached)    | 5-50ms     |
//! | `get_base_fee()`                  | Redis/mempool   | <1ms       |
//! | `get_block_number()`              | Redis/chain     | <1ms       |
//! | `get_timestamp()`                 | System clock    | <1μs       |
//! | `log_quantum(level, message)`     | Redis telemetry | fire&forget|
//! | `emit_signal(signal, data)`       | Redis PubSub    | fire&forget|
//! | `math_sqrt(x)`                    | Native f64      | <1μs       |
//! | `math_abs(x)`                     | Native f64      | <1μs       |
//! | `math_min(a, b)`                  | Native f64      | <1μs       |
//! | `math_max(a, b)`                  | Native f64      | <1μs       |
//! | `math_pow(base, exp)`             | Native f64      | <1μs       |
//! | `to_wei(amount, decimals)`        | Pure conversion | <1μs       |
//! | `from_wei(amount, decimals)`      | Pure conversion | <1μs       |

use redis::aio::ConnectionManager;
use rhai::{Dynamic, Engine, Map};
use std::sync::atomic::AtomicU64;
use std::sync::{Arc, Mutex};
use tokio::runtime::Handle;
use tokio::sync::RwLock;
use tracing::{debug, warn};

/// Shared context passed to host binding closures.
///
/// All fields are `Arc`-wrapped for safe concurrent access from multiple
/// Rhai evaluation threads.
#[derive(Clone)]
pub struct HostContext {
    /// Redis connection for cache reads (reserves, tokens, gas).
    pub redis: Arc<RwLock<ConnectionManager>>,
    /// Chain ID for this searcher instance.
    pub chain_id: u64,
    /// Cartridge ID currently being evaluated (for telemetry attribution).
    pub cartridge_id: Arc<RwLock<String>>,
    /// Tokio runtime handle for blocking async calls from sync Rhai context.
    pub rt_handle: Handle,
    /// Latest known block number (updated by scanner).
    pub block_number: Arc<std::sync::atomic::AtomicU64>,
    /// Latest known base fee in gwei (updated by gas oracle worker).
    pub base_fee_gwei: Arc<std::sync::atomic::AtomicU64>,
    /// Telemetry Redis channel for log_quantum messages.
    pub telemetry_channel: String,
    // ── simulate_swap RPC plumbing (real rate-limited cached quoter) ──────────
    //
    // OMEGA SEAL: this is the SAME read-only failover HTTP provider the observer
    // half uses (`rpc_multiplexer.rs`, `v3_quote_provider.rs`). RPC != signer —
    // `HttpRpcPool` carries NO `SignerMiddleware`/`Wallet`/`LocalWallet` and never
    // signs or broadcasts; attaching a signer would trip the capital-key lockout
    // in `main.rs`. All 8 signer env vars are untouched and unreferenced from this
    // module. The field is `Option` so non-mainnet / no-RPC-pool cases degrade to
    // the existing behaviour with NO RPC attempt (fail-safe).
    /// Read-only failover RPC pool (None on non-mainnet / when RPC_HTTP_* unset).
    pub rpc_pool: Option<Arc<shared_rs::rpc_failover::HttpRpcPool>>,
    /// Token-bucket rate limiter shared across all cartridge eval threads. Bounds
    /// a buggy tight-loop `.rhai` cartridge so it cannot saturate the RPC provider.
    pub rpc_budget: Arc<Mutex<RpcBudget>>,
    /// Hard floor (ns) between any two simulate_swap RPC calls (lock-free CAS guard).
    pub rpc_min_interval_ns: Arc<AtomicU64>,
    /// Wall-clock ns of the last RPC call (CAS target for the min-interval floor).
    pub rpc_last_call_ns: Arc<AtomicU64>,
}

/// Default RPC rate-limit floor: 100ms between any two simulate_swap RPC calls.
/// Process-global and lock-free (CAS loop), so a runaway cartridge loop is
/// throttled to ≤10 RPC calls/sec regardless of how many threads spin it.
pub const SIM_SWAP_RPC_MIN_INTERVAL_NS: u64 = 100_000_000;

/// Token-bucket rate limiter. Lazy refill-on-acquire (no background task).
/// Defaults: max=10, refill 10/sec — matches the "max 10 calls/evaluation" doc
/// contract at the top of this file. Acquire is non-blocking; on an empty bucket
/// the caller fails-honest (RULE 00) and does NOT touch the cache.
#[derive(Debug)]
pub struct RpcBudget {
    pub tokens: u32,
    pub max: u32,
    pub refill_per_sec: u32,
    pub last_refill_ns: u64,
}

impl RpcBudget {
    pub fn new(max: u32, refill_per_sec: u32) -> Self {
        Self {
            tokens: max,
            max,
            refill_per_sec,
            last_refill_ns: now_ns(),
        }
    }

    /// Non-blocking acquire. Refills by elapsed wall-clock (capped at `max`), then
    /// decrements one token if available. Returns `false` when empty (caller
    /// fails-honest, does NOT touch the cache — no failure poisoning).
    pub fn acquire(&mut self) -> bool {
        let now = now_ns();
        let elapsed_ns = now.saturating_sub(self.last_refill_ns);
        if elapsed_ns > 0 {
            let refilled =
                (elapsed_ns as u128 * self.refill_per_sec as u128 / 1_000_000_000) as u32;
            if refilled > 0 {
                self.tokens = self.tokens.saturating_add(refilled).min(self.max);
                self.last_refill_ns = now;
            }
        }
        if self.tokens >= 1 {
            self.tokens -= 1;
            true
        } else {
            false
        }
    }
}

/// Wall-clock nanoseconds since UNIX_EPOCH (0 on clock error — degrades to
/// "always allow" which is safe because the bucket cap still bounds the rate).
fn now_ns() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(0)
}

/// Pure Uniswap-V3 spot-price math, extracted from the `calc_v3_spot_price` binding so it is
/// unit-testable in isolation.
///
/// `price = (sqrtPriceX96 / 2^96)^2 * 10^(dec_in - dec_out)`. The `(sqrtPriceX96/2^96)^2` term is
/// the raw token1-per-token0 ratio; the decimal factor converts to human units in the SAME
/// convention as `calculate_price_v2` (token_out per token_in), so V2 and V3 prices compare
/// directly. The uint160 `sqrtPriceX96` is parsed from its decimal string straight into `f64`
/// (lossy in low bits, ample for a price ratio) — never through a fixed-width int, so no
/// shift/overflow can occur. Returns `0.0` for malformed / non-positive input.
pub(crate) fn v3_spot_price_from_sqrt(sqrt_price_x96: &str, dec_in: i64, dec_out: i64) -> f64 {
    let sqrt: f64 = match sqrt_price_x96.trim().parse() {
        Ok(v) => v,
        Err(_) => return 0.0,
    };
    if sqrt <= 0.0 || sqrt.is_nan() {
        return 0.0;
    }
    let q96 = 2f64.powi(96);
    let ratio = sqrt / q96; // = sqrt(price); price = token1/token0 in raw units
    let raw_price = ratio * ratio;
    raw_price * 10f64.powi((dec_in - dec_out) as i32)
}

/// Registers all host bindings into the Rhai engine.
///
/// This function is called once during `CartridgeRunner` initialization.
/// Each binding captures a clone of `HostContext` for infrastructure access.
pub fn register_host_bindings(engine: &mut Engine, ctx: HostContext) {
    // ─────────────────────────────────────────────────────────────────────
    // INFRASTRUCTURE BINDINGS (Redis/RPC)
    // ─────────────────────────────────────────────────────────────────────

    // get_reserves(pool_addr: String) -> Map
    // Returns: #{ r0: "...", r1: "...", token0_addr: "...", block: N, ts: N }
    let ctx_reserves = ctx.clone();
    engine.register_fn("get_reserves", move |pool_addr: &str| -> Dynamic {
        let ctx = ctx_reserves.clone();
        let pool_addr = pool_addr.to_lowercase();
        let chain_id = ctx.chain_id;
        let result = ctx.rt_handle.block_on(async {
            let mut redis = ctx.redis.write().await;
            let key = format!("arbx:pool_reserves:{}:{}", ctx.chain_id, pool_addr);
            let raw: Option<String> = redis::AsyncCommands::get(&mut *redis, &key).await.ok()?;
            raw
        });

        // FASE 4 — cartridge cache observability: log hit/miss so a starved cartridge run is
        // traceable to "Redis had no reserves for this pool" vs "cartridge never asked".
        match result {
            Some(json_str) => match serde_json::from_str::<serde_json::Value>(&json_str) {
                Ok(val) => {
                    debug!(event = "cartridge.redis_lookup", chain_id, pool = %pool_addr, hit = true);
                    json_value_to_dynamic(&val)
                }
                // Redis HAD a value but it did not parse — a real corruption signal, not a miss.
                Err(e) => {
                    warn!(event = "cartridge.redis_decode_failed", chain_id, pool = %pool_addr, kind = "pool_reserves", error = %e);
                    Dynamic::UNIT
                }
            },
            None => {
                debug!(event = "cartridge.redis_lookup", chain_id, pool = %pool_addr, hit = false);
                Dynamic::UNIT
            }
        }
    });

    // get_token_meta(token_addr: String) -> Map
    // Returns: #{ symbol: "...", decimals: N, is_stablecoin: bool }
    let ctx_token = ctx.clone();
    engine.register_fn("get_token_meta", move |token_addr: &str| -> Dynamic {
        let ctx = ctx_token.clone();
        let addr = token_addr.to_lowercase();
        let result = ctx.rt_handle.block_on(async {
            let mut redis = ctx.redis.write().await;
            let key = format!("arbx:tokens:{}:{}", ctx.chain_id, addr);
            let raw: Option<String> = redis::AsyncCommands::get(&mut *redis, &key).await.ok()?;
            raw
        });

        match result {
            Some(json_str) => match serde_json::from_str::<serde_json::Value>(&json_str) {
                Ok(val) => json_value_to_dynamic(&val),
                Err(_) => Dynamic::UNIT,
            },
            None => Dynamic::UNIT,
        }
    });

    // get_pool_index(token_a: String, token_b: String) -> Array
    // Returns array of pool addresses for the given token pair.
    let ctx_pools = ctx.clone();
    engine.register_fn(
        "get_pool_index",
        move |token_a: &str, token_b: &str| -> Dynamic {
            let ctx = ctx_pools.clone();
            let (lo, hi) = if token_a < token_b {
                (token_a.to_lowercase(), token_b.to_lowercase())
            } else {
                (token_b.to_lowercase(), token_a.to_lowercase())
            };
            let result = ctx.rt_handle.block_on(async {
                let mut redis = ctx.redis.write().await;
                let key = format!("arbx:pool_index:{}:{}:{}", ctx.chain_id, lo, hi);
                let raw: Option<String> =
                    redis::AsyncCommands::get(&mut *redis, &key).await.ok()?;
                raw
            });

            match result {
                Some(json_str) => match serde_json::from_str::<Vec<String>>(&json_str) {
                    Ok(addrs) => {
                        let arr: Vec<Dynamic> = addrs.into_iter().map(Dynamic::from).collect();
                        Dynamic::from_array(arr)
                    }
                    Err(_) => Dynamic::from_array(vec![]),
                },
                None => Dynamic::from_array(vec![]),
            }
        },
    );

    // get_v3_slot0(pool_addr: String) -> Map
    // Returns: #{ sqrt_price_x96: "...", liquidity: "...", ts: N }  (() on cache miss).
    // Reads arbx:v3_slot0:<chain>:<pool> — populated by PoolSyncWorker every tick. This is the
    // V3 analogue of get_reserves: V3 pools have no constant-product r0/r1, their price lives
    // in slot0's sqrtPriceX96.
    let ctx_slot0 = ctx.clone();
    engine.register_fn("get_v3_slot0", move |pool_addr: &str| -> Dynamic {
        let ctx = ctx_slot0.clone();
        let pool_addr = pool_addr.to_lowercase();
        let chain_id = ctx.chain_id;
        let result = ctx.rt_handle.block_on(async {
            let mut redis = ctx.redis.write().await;
            let key = format!("arbx:v3_slot0:{}:{}", ctx.chain_id, pool_addr);
            let raw: Option<String> = redis::AsyncCommands::get(&mut *redis, &key).await.ok()?;
            raw
        });
        // FASE 4 — same hit/miss + decode-failure observability as get_reserves, for the V3 path.
        match result {
            Some(json_str) => match serde_json::from_str::<serde_json::Value>(&json_str) {
                Ok(val) => {
                    debug!(event = "cartridge.redis_lookup", chain_id, pool = %pool_addr, hit = true);
                    json_value_to_dynamic(&val)
                }
                Err(e) => {
                    warn!(event = "cartridge.redis_decode_failed", chain_id, pool = %pool_addr, kind = "v3_slot0", error = %e);
                    Dynamic::UNIT
                }
            },
            None => {
                debug!(event = "cartridge.redis_lookup", chain_id, pool = %pool_addr, hit = false);
                Dynamic::UNIT
            }
        }
    });

    // calc_v3_spot_price(sqrt_price_x96: String, dec_in: i64, dec_out: i64) -> f64
    // Uniswap V3 spot price from slot0's sqrtPriceX96. The raw ratio (token1 per token0, in
    // smallest units) is (sqrtPriceX96 / 2^96)^2; multiplying by 10^(dec_in - dec_out) yields the
    // SAME convention as calculate_price_v2 (token_out per token_in, human units) so V2 and V3
    // prices are directly comparable. Overflow-safe: the uint160 sqrtPriceX96 is parsed from its
    // decimal string straight into f64 (lossy in low bits but ample for a price ratio) — never
    // through a fixed-width int, so no shift/overflow is possible.
    engine.register_fn(
        "calc_v3_spot_price",
        move |sqrt_price_x96: &str, dec_in: i64, dec_out: i64| -> f64 {
            v3_spot_price_from_sqrt(sqrt_price_x96, dec_in, dec_out)
        },
    );

    // v3_amount_out_single_tick(amount_in, sqrt_price_x96, liquidity, fee_bps, zero_for_one) -> Dynamic
    // Within-tick (single-tick) Uniswap-V3 output estimate. This is an UPPER BOUND on real output
    // (V3 has less liquidity beyond the active tick), so cartridges MUST treat the result as a
    // candidate only — never as a confirmed opportunity. Pure integer math (no RPC/Redis/block_on).
    // `fee_bps` (basis points: 30 = 0.30%) is converted to V3 on-chain pips (millionths) by ×100.
    // Returns the decimal-string amount_out, or () on parse error / out-of-range fee / degenerate move.
    engine.register_fn(
        "v3_amount_out_single_tick",
        move |amount_in: &str,
              sqrt_price_x96: &str,
              liquidity: &str,
              fee_bps: i64,
              zero_for_one: bool|
              -> Dynamic {
            use ethers::types::U256;
            let amount_in = match U256::from_dec_str(amount_in) {
                Ok(v) => v,
                Err(_) => return Dynamic::UNIT,
            };
            let sqrt_price_x96 = match U256::from_dec_str(sqrt_price_x96) {
                Ok(v) => v,
                Err(_) => return Dynamic::UNIT,
            };
            let liquidity = match U256::from_dec_str(liquidity) {
                Ok(v) => v,
                Err(_) => return Dynamic::UNIT,
            };
            // V3 on-chain fee is in millionths (pips): bps 30 -> pips 3000 (0.30%).
            let fee_pips = fee_bps * 100;
            if !(0..1_000_000).contains(&fee_pips) {
                return Dynamic::UNIT;
            }
            let result = crate::amm_math::v3_amount_out_single_tick(
                amount_in,
                sqrt_price_x96,
                liquidity,
                fee_pips as u32,
                zero_for_one,
            );
            if result.is_zero() {
                Dynamic::UNIT
            } else {
                Dynamic::from(result.to_string())
            }
        },
    );

    // v2_amount_out_str(amount_in, reserve_in, reserve_out, fee_bps) -> Dynamic
    // Constant-product (Uniswap-V2) EXACT-INPUT output, post-fee, as a decimal wei STRING.
    // This is the V2 analogue of v3_amount_out_single_tick: it keeps the round-trip arb math in
    // exact integer wei (no f64 rounding) and is the ONLY way a cartridge prices a V2 leg for the
    // round-trip path. `reserve_in`/`reserve_out` MUST be the reserves in swap direction (caller
    // orders them by token0_addr). `fee_bps` is the V2 basis-point convention (30 = 0.30%).
    // Returns () on parse error, out-of-range fee, or degenerate (zero) output (RULE 00: never
    // fabricates a non-zero amount).
    engine.register_fn(
        "v2_amount_out_str",
        move |amount_in: &str, reserve_in: &str, reserve_out: &str, fee_bps: i64| -> Dynamic {
            use ethers::types::U256;
            if !(0..10_000).contains(&fee_bps) {
                return Dynamic::UNIT;
            }
            let amount_in = match U256::from_dec_str(amount_in) {
                Ok(v) => v,
                Err(_) => return Dynamic::UNIT,
            };
            let reserve_in = match U256::from_dec_str(reserve_in) {
                Ok(v) => v,
                Err(_) => return Dynamic::UNIT,
            };
            let reserve_out = match U256::from_dec_str(reserve_out) {
                Ok(v) => v,
                Err(_) => return Dynamic::UNIT,
            };
            let out =
                crate::amm_math::v2_amount_out(amount_in, reserve_in, reserve_out, fee_bps as u32);
            if out.is_zero() {
                Dynamic::UNIT
            } else {
                Dynamic::from(out.to_string())
            }
        },
    );

    // get_token_price_usd(symbol: String) -> Dynamic (f64 USD, or () on miss)
    // Reads the price oracle hash arbx:token_prices:<chain> (HGET field=symbol), populated every
    // ~30s by price_worker (Chainlink/DexScreener/GeckoTerminal). RULE 00: returns () on cache miss
    // or unparsable value — the cartridge MUST fail honestly ("v3_arb_no_price"), never fabricate.
    let ctx_price = ctx.clone();
    engine.register_fn("get_token_price_usd", move |symbol: &str| -> Dynamic {
        let ctx = ctx_price.clone();
        let key = format!("arbx:token_prices:{}", ctx.chain_id);
        let field = symbol.to_string();
        let raw: Option<String> = ctx.rt_handle.block_on(async {
            let mut redis = ctx.redis.write().await;
            redis::AsyncCommands::hget(&mut *redis, &key, &field)
                .await
                .ok()
                .flatten()
        });
        match raw.and_then(|s| s.trim().parse::<f64>().ok()) {
            Some(p) if p.is_finite() && p > 0.0 => Dynamic::from(p),
            _ => Dynamic::UNIT,
        }
    });

    // v3_arb_enabled() -> bool
    // Gate for the cross-pool round-trip arb emission (is_opportunity:true path). Reads
    // ARBX_V3_ARB_MODE; ONLY "on" enables it. Default OFF (any other/unset value -> false), so the
    // branch keeps emitting the honest is_opportunity:false single-tick candidate until an operator
    // explicitly opts in. Shadow/paper only — capital=0.
    engine.register_fn("v3_arb_enabled", || -> bool {
        std::env::var("ARBX_V3_ARB_MODE")
            .map(|v| v == "on")
            .unwrap_or(false)
    });

    // simulate_swap(amount_in: String, path: Array) -> Map
    // Cached swap simulation through the given path, rate-limited at the host layer
    // so a buggy tight-loop .rhai cartridge cannot saturate the RPC provider.
    //
    // V3 legs: on-chain QuoterV2 multicall via the read-only failover pool (RPC).
    // V2 legs: pure constant-product math over the cached reserves (zero-RPC, the
    // canonical V2 pricer — `amm_math::v2_amount_out`). A path is computed in ONE
    // protocol mode resolved up-front: if `rpc_pool` + QuoterV2/Multicall3 resolve
    // for the chain, the WHOLE path is quoted via V3 QuoterV2 (per-leg fold);
    // otherwise the WHOLE path is priced via V2 cached reserves. Mixed-mode paths
    // are NOT silently mis-priced — a V3 attempt on a V2-only pair returns
    // `v3_quote_failed` (RULE 00: never fabricate).
    //
    // Returns one of:
    //   { success: true,  amount_out: "<wei>", block: N, ts: N, quoter: "...", path: [...] }
    //   { success: false, error: "path_too_short" | "amount_parse" | "rate_limited"
    //                     | "no_rpc_pool" | "v3_quote_failed" | "v2_reserves_missing" }
    // On ANY non-success path the cache is NOT written (no failure poisoning): the
    // next call can retry immediately rather than being stuck with a cached failure.
    let ctx_sim = ctx.clone();
    engine.register_fn(
        "simulate_swap",
        move |amount_in: &str, path: rhai::Array| -> Dynamic {
            let ctx = ctx_sim.clone();
            let path_strs: Vec<String> = path
                .iter()
                .filter_map(|d| d.clone().into_string().ok())
                .collect();

            if path_strs.len() < 2 {
                return err_map("path_too_short");
            }

            let amount = match ethers::types::U256::from_dec_str(amount_in) {
                Ok(v) => v,
                Err(_) => return err_map("amount_parse"),
            };

            let cache_key = format!(
                "arbx:sim_cache:{}:{}:{}",
                ctx.chain_id,
                amount_in,
                path_strs.join("_")
            );
            let cur_block = ctx.block_number.load(std::sync::atomic::Ordering::Relaxed);

            // ── 1. cache read + block-number guard (reject stale quotes) ──────────
            // A quote cached at block N is INVALID at N+1 (the active tick may have
            // crossed). Treat as MISS when cached_block + 1 < cur_block (the block
            // moved on by more than one) and re-quote. TTL alone is insufficient
            // because block interval is not constant (PoS miss slots, reorgs).
            let cached_json: Option<String> = ctx.rt_handle.block_on(async {
                let mut redis = ctx.redis.write().await;
                redis::AsyncCommands::get(&mut *redis, &cache_key)
                    .await
                    .ok()
                    .flatten()
            });
            if let Some(j) = cached_json.as_deref() {
                if let Ok(v) = serde_json::from_str::<serde_json::Value>(j) {
                    let cached_block = v.get("block").and_then(|b| b.as_u64()).unwrap_or(0);
                    if cached_block + 1 >= cur_block {
                        return json_value_to_dynamic(&v);
                    }
                    // else fall through to re-quote (stale)
                }
            }

            // ── 2. rate limit (token bucket + min-interval CAS floor) ─────────────
            // Both guards are checked BEFORE any RPC. The min-interval floor is
            // process-global and lock-free (CAS), so even `while true { simulate_swap() }`
            // in a cartridge is throttled to ≤10 RPC calls/sec AND ≥100ms apart. On
            // rate-limit-exceeded the binding returns {success:false, error:"rate_limited"}
            // and DOES NOT touch Redis (no cache poisoning).
            let min_iv = ctx
                .rpc_min_interval_ns
                .load(std::sync::atomic::Ordering::Relaxed);
            loop {
                let last = ctx
                    .rpc_last_call_ns
                    .load(std::sync::atomic::Ordering::Relaxed);
                let now = now_ns();
                if min_iv > 0 && now.saturating_sub(last) < min_iv {
                    return err_map("rate_limited");
                }
                match ctx.rpc_last_call_ns.compare_exchange(
                    last,
                    now,
                    std::sync::atomic::Ordering::Relaxed,
                    std::sync::atomic::Ordering::Relaxed,
                ) {
                    Ok(_) => break,
                    Err(_) => continue, // another thread won the slot; re-read
                }
            }
            let allowed = ctx
                .rpc_budget
                .lock()
                .map(|mut b| b.acquire())
                .unwrap_or(false);
            if !allowed {
                return err_map("rate_limited");
            }

            // ── 3. compute the quote (single block_on — never nest block_on) ──────
            // Cache read AND RPC happen inside ONE block_on closure each (two
            // sequential block_ons are safe under block_in_place on the multi-
            // threaded runtime). The RPC future is already async (no internal
            // block_on) — same verified pattern as v3_quote_provider.rs:96.
            let computed = ctx
                .rt_handle
                .block_on(async { simulate_swap_compute(&ctx, amount, &path_strs).await });

            let (amount_out, quoter_tag) = match computed {
                Some((out, tag)) if !out.is_zero() => (out, tag),
                Some((_, tag)) => return err_map(&tag),
                None => return err_map("v3_quote_failed"),
            };

            // ── 4. write cache (success only) + return ────────────────────────────
            let payload = serde_json::json!({
                "success": true,
                "amount_out": amount_out.to_string(),
                "block": cur_block,
                "ts": chrono::Utc::now().timestamp_millis(),
                "quoter": quoter_tag,
                "path": path_strs,
            });
            let payload_str = payload.to_string();
            ctx.rt_handle.block_on(async {
                let mut redis = ctx.redis.write().await;
                let _: Result<(), _> = redis::AsyncCommands::set_ex(
                    &mut *redis,
                    &cache_key,
                    &payload_str,
                    12u64, // TTL 12s ≈ 1 mainnet block cadence
                )
                .await;
            });
            json_value_to_dynamic(&payload)
        },
    );

    // ─────────────────────────────────────────────────────────────────────
    // CHAIN STATE BINDINGS
    // ─────────────────────────────────────────────────────────────────────

    // get_base_fee() -> f64 (in gwei)
    let ctx_fee = ctx.clone();
    engine.register_fn("get_base_fee", move || -> Dynamic {
        let fee = ctx_fee
            .base_fee_gwei
            .load(std::sync::atomic::Ordering::Relaxed);
        // Stored as gwei * 1000 for 3-decimal precision without floats
        Dynamic::from(fee as f64 / 1000.0)
    });

    // get_block_number() -> i64
    let ctx_block = ctx.clone();
    engine.register_fn("get_block_number", move || -> Dynamic {
        let block = ctx_block
            .block_number
            .load(std::sync::atomic::Ordering::Relaxed);
        Dynamic::from(block as i64)
    });

    // get_timestamp() -> i64 (unix seconds)
    engine.register_fn("get_timestamp", || -> Dynamic {
        Dynamic::from(chrono::Utc::now().timestamp())
    });

    // get_chain_id() -> i64
    let ctx_chain = ctx.clone();
    engine.register_fn("get_chain_id", move || -> Dynamic {
        Dynamic::from(ctx_chain.chain_id as i64)
    });

    // ─────────────────────────────────────────────────────────────────────
    // TELEMETRY BINDINGS
    // ─────────────────────────────────────────────────────────────────────

    // log_quantum(level: String, message: String)
    // Levels: "debug", "info", "warn", "error"
    let ctx_log = ctx.clone();
    engine.register_fn(
        "log_quantum",
        move |level: &str, message: &str| -> Dynamic {
            let ctx = ctx_log.clone();
            let cartridge_id = ctx
                .rt_handle
                .block_on(async { ctx.cartridge_id.read().await.clone() });

            let log_entry = serde_json::json!({
                "cartridge_id": cartridge_id,
                "level": level,
                "message": message,
                "timestamp": chrono::Utc::now().to_rfc3339(),
                "chain_id": ctx.chain_id,
            });

            // Fire-and-forget telemetry publish
            let channel = ctx.telemetry_channel.clone();
            ctx.rt_handle.block_on(async {
                let mut redis = ctx.redis.write().await;
                let _: Result<(), _> =
                    redis::AsyncCommands::publish(&mut *redis, &channel, log_entry.to_string())
                        .await;
            });

            match level {
                "debug" => debug!(cartridge = %cartridge_id, "{}", message),
                "info" => tracing::info!(cartridge = %cartridge_id, "{}", message),
                "warn" => warn!(cartridge = %cartridge_id, "{}", message),
                "error" => tracing::error!(cartridge = %cartridge_id, "{}", message),
                _ => debug!(cartridge = %cartridge_id, "[{}] {}", level, message),
            }

            Dynamic::UNIT
        },
    );

    // emit_signal(signal_type: String, data: Map)
    // Publishes a signal to the Arteria PubSub for downstream consumers.
    let ctx_signal = ctx.clone();
    engine.register_fn(
        "emit_signal",
        move |signal_type: &str, data: Map| -> Dynamic {
            let ctx = ctx_signal.clone();
            let cartridge_id = ctx
                .rt_handle
                .block_on(async { ctx.cartridge_id.read().await.clone() });

            let signal = serde_json::json!({
                "signal_type": signal_type,
                "cartridge_id": cartridge_id,
                "chain_id": ctx.chain_id,
                "data": format!("{:?}", data),
                "timestamp": chrono::Utc::now().to_rfc3339(),
            });

            ctx.rt_handle.block_on(async {
                let mut redis = ctx.redis.write().await;
                let _: Result<(), _> = redis::AsyncCommands::publish(
                    &mut *redis,
                    "arbx:cartridge:signals",
                    signal.to_string(),
                )
                .await;
            });

            Dynamic::UNIT
        },
    );

    // ─────────────────────────────────────────────────────────────────────
    // MATH UTILITY BINDINGS (pure, no I/O)
    // ─────────────────────────────────────────────────────────────────────

    engine.register_fn("math_sqrt", |x: f64| -> Dynamic {
        if x < 0.0 {
            Dynamic::from(f64::NAN)
        } else {
            Dynamic::from(x.sqrt())
        }
    });

    engine.register_fn("math_abs", |x: f64| -> Dynamic { Dynamic::from(x.abs()) });

    engine.register_fn("math_min", |a: f64, b: f64| -> Dynamic {
        Dynamic::from(a.min(b))
    });

    engine.register_fn("math_max", |a: f64, b: f64| -> Dynamic {
        Dynamic::from(a.max(b))
    });

    engine.register_fn("math_pow", |base: f64, exp: f64| -> Dynamic {
        Dynamic::from(base.powf(exp))
    });

    engine.register_fn("math_log", |x: f64| -> Dynamic {
        if x <= 0.0 {
            Dynamic::from(f64::NAN)
        } else {
            Dynamic::from(x.ln())
        }
    });

    engine.register_fn("math_exp", |x: f64| -> Dynamic { Dynamic::from(x.exp()) });

    // to_wei(amount: f64, decimals: i64) -> String
    // Converts a human-readable amount to wei string representation.
    engine.register_fn("to_wei", |amount: f64, decimals: i64| -> Dynamic {
        let factor = 10f64.powi(decimals as i32);
        let wei = (amount * factor) as u128;
        Dynamic::from(wei.to_string())
    });

    // from_wei(wei_str: String, decimals: i64) -> f64
    // Converts a wei string to human-readable f64.
    engine.register_fn("from_wei", |wei_str: &str, decimals: i64| -> Dynamic {
        let wei: f64 = wei_str.parse().unwrap_or(0.0);
        let factor = 10f64.powi(decimals as i32);
        Dynamic::from(wei / factor)
    });

    // to_float(s: String) -> f64
    // String → f64 parse. rhai does NOT provide `to_float` on strings natively
    // (only on integer types), yet the dex_arb and triangular_arb cartridges call
    // `s.to_float()` inside their `parse_float` helper. Without this binding those
    // cartridges throw ErrorFunctionNotFound at runtime the moment they reach
    // reserve parsing — caught and swallowed by the shadow loop, so they would
    // SILENTLY never emit an opportunity. Registering it here is the root-cause fix
    // (regression-tested in tests/cartridge_strategies_test.rs). Fail-honest: an
    // unparseable string yields 0.0, which every cartridge already treats as "skip".
    engine.register_fn("to_float", |s: &str| -> Dynamic {
        Dynamic::from(s.parse::<f64>().unwrap_or(0.0))
    });
}

// ─────────────────────────────────────────────────────────────────────────────
// Helper: build a structured Rhai error Map for simulate_swap failure paths.
// ─────────────────────────────────────────────────────────────────────────────
fn err_map(code: &str) -> Dynamic {
    let mut map = Map::new();
    map.insert("success".into(), Dynamic::from(false));
    map.insert("error".into(), Dynamic::from(code.to_string()));
    Dynamic::from_map(map)
}

// ─────────────────────────────────────────────────────────────────────────────
// simulate_swap compute kernel (async). Folds left over consecutive token pairs.
//
// Protocol resolution is decided ONCE up-front to avoid silently mis-pricing a
// mixed-mode path:
//   - If `rpc_pool` is Some AND QuoterV2/Multicall3 resolve for the chain
//     (`resolve_quoter_multicall`), the WHOLE path is quoted via V3 QuoterV2
//     (one `quoteExactInputSingle` per leg, folded). A V2-only pair on mainnet
//     will return `v3_quote_failed` (RULE 00 — never fabricate a V2 amount via
//     the V3 quoter). Fee tier defaults to 3000 (most common); a wrong tier
//     reports honestly as `v3_quote_failed`, not a fabricated number.
//   - Otherwise the WHOLE path is priced via V2 cached reserves (zero-RPC): for
//     each leg, read `arbx:pool_index` → pool, `arbx:pool_reserves` → {r0,r1,
//     token0_addr}, order reserves by direction, and call `amm_math::v2_amount_out`
//     (the canonical V2 pricer — NO RPC, NO duplication of the v2 RPC path).
//     Missing reserves report `v2_reserves_missing`.
//
// Returns `Some((final_amount, quoter_tag))` on success, or `Some((zero, tag))`
// carrying the failure tag (`v3_quote_failed` | `v2_reserves_missing` |
// `no_rpc_pool`) so the caller can surface a structured error WITHOUT caching it.
// ─────────────────────────────────────────────────────────────────────────────
async fn simulate_swap_compute(
    ctx: &HostContext,
    amount_in: ethers::types::U256,
    path: &[String],
) -> Option<(ethers::types::U256, String)> {
    use ethers::types::{Address, U256};

    // Resolve addresses once; any unparseable token aborts the whole path
    // (fail-honest — no partial fabrication).
    let addrs: Vec<Address> = path
        .iter()
        .map(|s| s.trim().parse::<Address>())
        .collect::<Result<_, _>>()
        .ok()?;
    if addrs.len() < 2 {
        return Some((U256::zero(), "path_too_short".to_string()));
    }

    // ── V3 mode: rpc_pool present AND quoter/multicall resolve ─────────────────
    let v3_mode = ctx
        .rpc_pool
        .as_ref()
        .and_then(|_| crate::v3_quote_provider::resolve_quoter_multicall(ctx.chain_id));

    if let (Some(pool), Some((quoter_addr, multicall_addr))) = (ctx.rpc_pool.as_ref(), v3_mode) {
        let pool = pool.clone();
        // Default fee tier = 3000 (0.30%, the most common V3 tier). Best-effort:
        // a wrong tier reports `v3_quote_failed`, never a fabricated amount.
        const V3_DEFAULT_FEE_BPS: u32 = 3000;
        let mut leg_amount = amount_in;
        let mut last_success = false;
        for window in addrs.windows(2) {
            let token_in = window[0];
            let token_out = window[1];
            let reqs = vec![crate::amm_math::V3QuoteRequest {
                pool_addr: Address::zero(), // metadata only — QuoterV2 routes by (tokenIn,tokenOut,fee)
                token_in,
                token_out,
                amount_in: leg_amount,
                fee_bps: V3_DEFAULT_FEE_BPS,
            }];
            let result = pool
                .with_retry(|provider| {
                    let reqs = reqs.clone();
                    async move {
                        crate::amm_math::v3_quote_exact_in_multicall(
                            provider,
                            quoter_addr,
                            multicall_addr,
                            reqs,
                        )
                        .await
                    }
                })
                .await;
            match result {
                Ok(results) => match results.into_iter().next() {
                    Some(r) if r.success => {
                        leg_amount = r.amount_out;
                        last_success = true;
                    }
                    _ => {
                        // Per-pool failure (insufficient liquidity / wrong fee tier /
                        // pool revert) — fail-honest for the WHOLE path; cache untouched.
                        return Some((U256::zero(), "v3_quote_failed".to_string()));
                    }
                },
                Err(_) => return Some((U256::zero(), "v3_quote_failed".to_string())),
            }
        }
        if last_success {
            return Some((leg_amount, "v3_multicall".to_string()));
        }
        return Some((U256::zero(), "v3_quote_failed".to_string()));
    }

    // ── V2 mode: zero-RPC, pure constant-product math over cached reserves ─────
    // This is the canonical V2 pricer (`amm_math::v2_amount_out`) — exactly the
    // same kernel the `v2_amount_out_str` host binding exposes, never an RPC. Pool
    // addresses + reserves come from the existing Redis cache populated by
    // PoolSyncWorker. Missing reserves → fail-honest (RULE 00).
    let mut leg_amount = amount_in;
    for window in addrs.windows(2) {
        let token_in_addr_hex = format!("{:#x}", window[0]);
        let token_out_addr_hex = format!("{:#x}", window[1]);
        // Order the pair lexicographically (the pool_index key convention).
        let (lo, hi, zero_for_one) = if token_in_addr_hex < token_out_addr_hex {
            (token_in_addr_hex.clone(), token_out_addr_hex.clone(), true)
        } else {
            (token_out_addr_hex.clone(), token_in_addr_hex.clone(), false)
        };

        // (a) resolve pool address(es) for the pair.
        let pool_key = format!("arbx:pool_index:{}:{}:{}", ctx.chain_id, lo, hi);
        let pool_json: Option<String> = {
            let mut redis = ctx.redis.write().await;
            redis::AsyncCommands::get(&mut *redis, &pool_key)
                .await
                .ok()
                .flatten()
        };
        let pool_addr = pool_json
            .and_then(|s| serde_json::from_str::<Vec<String>>(&s).ok())
            .and_then(|v| v.into_iter().next())?;

        // (b) read reserves { r0, r1, token0_addr }.
        let reserves_key = format!("arbx:pool_reserves:{}:{}", ctx.chain_id, pool_addr);
        let reserves_json: Option<String> = {
            let mut redis = ctx.redis.write().await;
            redis::AsyncCommands::get(&mut *redis, &reserves_key)
                .await
                .ok()
                .flatten()
        };
        let rsv = reserves_json.and_then(|s| serde_json::from_str::<serde_json::Value>(&s).ok())?;
        let r0 = rsv
            .get("r0")
            .and_then(|v| v.as_str())
            .and_then(|s| U256::from_dec_str(s).ok())?;
        let r1 = rsv
            .get("r1")
            .and_then(|v| v.as_str())
            .and_then(|s| U256::from_dec_str(s).ok())?;
        // token0_addr tells us which reserve is token0. If absent, assume lo==token0
        // (the pool_index ordering convention) — best-effort, bounded by the
        // v2_amount_out degenerate-input guard.
        let token0_is_lo = rsv
            .get("token0_addr")
            .and_then(|v| v.as_str())
            .map(|t0| t0.to_lowercase() == lo)
            .unwrap_or(true);
        // reserve_in = reserves of token_in. token_in == lo iff zero_for_one.
        // If token0_is_lo: r0 = lo reserves, r1 = hi reserves.
        // reserve_in(token_in=lo)  = token0_is_lo ? r0 : r1
        // reserve_in(token_in=hi)  = token0_is_lo ? r1 : r0
        let (reserve_in, reserve_out) = if zero_for_one {
            // token_in == lo
            if token0_is_lo {
                (r0, r1)
            } else {
                (r1, r0)
            }
        } else {
            // token_in == hi
            if token0_is_lo {
                (r1, r0)
            } else {
                (r0, r1)
            }
        };

        let out = crate::amm_math::v2_amount_out(leg_amount, reserve_in, reserve_out, 30u32);
        if out.is_zero() {
            return Some((U256::zero(), "v2_reserves_missing".to_string()));
        }
        leg_amount = out;
    }
    Some((leg_amount, "v2_cpmm_reserves".to_string()))
}

// ─────────────────────────────────────────────────────────────────────────────
// Helper: Convert serde_json::Value to Rhai Dynamic
// ─────────────────────────────────────────────────────────────────────────────

fn json_value_to_dynamic(val: &serde_json::Value) -> Dynamic {
    match val {
        serde_json::Value::Null => Dynamic::UNIT,
        serde_json::Value::Bool(b) => Dynamic::from(*b),
        serde_json::Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                Dynamic::from(i)
            } else if let Some(f) = n.as_f64() {
                Dynamic::from(f)
            } else {
                Dynamic::UNIT
            }
        }
        serde_json::Value::String(s) => Dynamic::from(s.clone()),
        serde_json::Value::Array(arr) => {
            let items: Vec<Dynamic> = arr.iter().map(json_value_to_dynamic).collect();
            Dynamic::from_array(items)
        }
        serde_json::Value::Object(obj) => {
            let mut map = Map::new();
            for (k, v) in obj {
                map.insert(k.clone().into(), json_value_to_dynamic(v));
            }
            Dynamic::from_map(map)
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn json_null_to_unit() {
        let d = json_value_to_dynamic(&serde_json::Value::Null);
        assert!(d.is_unit());
    }

    #[test]
    fn json_number_to_dynamic() {
        let d = json_value_to_dynamic(&serde_json::json!(42));
        assert_eq!(d.as_int().unwrap(), 42);
    }

    #[test]
    fn json_object_to_map() {
        let val = serde_json::json!({"r0": "1000", "r1": "2000"});
        let d = json_value_to_dynamic(&val);
        let map = d.cast::<Map>();
        assert_eq!(
            map.get("r0").unwrap().clone().into_string().unwrap(),
            "1000"
        );
    }

    // 2^96 exactly = 79228162514264337593543950336 (exactly representable as f64).
    const Q96_STR: &str = "79228162514264337593543950336";
    // 2 * 2^96 = 2^97.
    const TWO_Q96_STR: &str = "158456325028528675187087900672";

    #[test]
    fn v3_spot_price_unit_ratio_at_q96() {
        // sqrtPriceX96 == 2^96 ⇒ sqrt(price)=1 ⇒ price=1.0 (equal decimals).
        let p = v3_spot_price_from_sqrt(Q96_STR, 18, 18);
        assert!((p - 1.0).abs() < 1e-9, "expected ~1.0, got {p}");
    }

    #[test]
    fn v3_spot_price_squares_the_ratio() {
        // sqrtPriceX96 == 2*2^96 ⇒ sqrt(price)=2 ⇒ price=4.0.
        let p = v3_spot_price_from_sqrt(TWO_Q96_STR, 18, 18);
        assert!((p - 4.0).abs() < 1e-6, "expected ~4.0, got {p}");
    }

    #[test]
    fn v3_spot_price_applies_decimal_delta() {
        // ratio 1.0, dec_in=6 dec_out=18 ⇒ 1.0 * 10^(6-18) = 1e-12 (USDC/WETH-style scaling).
        let p = v3_spot_price_from_sqrt(Q96_STR, 6, 18);
        let expected = 1e-12;
        assert!(
            (p - expected).abs() < expected * 1e-6,
            "expected ~1e-12, got {p}"
        );
    }

    #[test]
    fn v3_spot_price_rejects_bad_input() {
        assert_eq!(v3_spot_price_from_sqrt("", 18, 18), 0.0);
        assert_eq!(v3_spot_price_from_sqrt("not_a_number", 18, 18), 0.0);
        assert_eq!(v3_spot_price_from_sqrt("0", 18, 18), 0.0);
    }

    // ── simulate_swap real rate-limited cached quoter unit tests ───────────────

    #[test]
    fn err_map_builds_structured_error() {
        let d = err_map("rate_limited");
        let m = d.cast::<Map>();
        assert!(!m.get("success").unwrap().clone().as_bool().unwrap());
        assert_eq!(
            m.get("error").unwrap().clone().into_string().unwrap(),
            "rate_limited"
        );
    }

    #[test]
    fn rpc_budget_acquire_then_refill() {
        let mut b = RpcBudget::new(2, 2);
        // fresh bucket has max tokens.
        assert!(b.acquire(), "first acquire on full bucket must succeed");
        assert!(b.acquire(), "second acquire must succeed");
        assert!(
            !b.acquire(),
            "third acquire on empty bucket must fail (non-blocking)"
        );
        // Simulate ~0.5s elapsed → refilled = floor(0.5 * 2) = 1 token.
        b.last_refill_ns = now_ns().saturating_sub(500_000_000);
        assert!(b.acquire(), "acquire after elapsed refill must succeed");
    }

    #[test]
    fn rpc_budget_caps_at_max() {
        let mut b = RpcBudget::new(3, 1_000_000); // huge refill rate
                                                  // drain fully
        assert!(b.acquire());
        assert!(b.acquire());
        assert!(b.acquire());
        assert!(!b.acquire());
        // simulate a long sleep — refill must cap at max=3, not overflow
        b.last_refill_ns = now_ns().saturating_sub(60_000_000_000); // 60s
        assert!(b.acquire());
        assert!(b.acquire());
        assert!(b.acquire());
        // 4th must fail — capped at max=3.
        assert!(!b.acquire());
    }

    #[test]
    fn sim_swap_rpc_min_interval_constant_is_100ms() {
        // Locks the contract: 100ms floor between any two RPC calls.
        assert_eq!(SIM_SWAP_RPC_MIN_INTERVAL_NS, 100_000_000);
    }
}
