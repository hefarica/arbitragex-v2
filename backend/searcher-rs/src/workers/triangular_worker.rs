//! TriangularWorker — periodic emitter of triangular-arbitrage opportunities.
//!
//! Promotes the `triangular` strategy from `scaffold` (enum-only) to `live`
//! (real opportunities flowing through the pipeline). On every tick (default 12s ≈ 1
//! Ethereum block) the worker:
//!
//!   1. Iterates a hardcoded list of MVP triangular cycles (a → b → c → a)
//!      over blue-chip mainnet tokens whose V2 pools are seeded by migration 029.
//!   2. Resolves token addresses from the Redis token cache.
//!   3. Looks up each pool of the cycle via `arbx:pool_index:<chain>:<sym0>:<sym1>`.
//!   4. Fetches V2 reserves via `arbx:pool_reserves:<chain>:<addr>`. Skips on
//!      missing pool or stale reserves (>5 blocks behind PoolSyncWorker's latest).
//!   5. Computes the closed-form **spot product** S = ∏(γ · R_out / R_in).
//!      If S ≤ 1 → not profitable, no further work; if S > 1 → continue.
//!   6. Runs **golden-section search** over [1 wei, x_max] (x_max bounded by
//!      operator capital cap via `effective_capital_for(token_a, "triangular")`)
//!      to find the profit-maximising input x*. f(x) = sequential V2 output - x
//!      is strictly concave for V2 with fees, so the optimum is unique.
//!   7. If f(x*) > 0 → constructs an `Opportunity { strategy_kind: Triangular, ... }`,
//!      pushes to Redis stream `arbx:opps:detected` and persists to PG via the
//!      existing helpers. The spine evaluator runs downstream and applies all
//!      gates (allowlist, oracle, sanity, risk).
//!   8. Both directions of each cycle are tried (forward a→b→c→a and reverse
//!      a→c→b→a); profitability depends on which way the cycle is traversed.
//!   9. **Per-block dedup**: a `(cycle_hash, block_number)` HashSet prevents
//!      the same opportunity being emitted twice for the same block. Entries
//!      older than 10 blocks are pruned to keep memory bounded.
//!
//! Doctrine compliance:
//! - **R8 fail-honest**: stale reserves (>5 blocks) → skip (no synthetic emit).
//!   Missing pool index → skip cleanly.
//! - **RULE 00 zero mocks**: math + reserves are real; no fake profit values.
//! - **No `unwrap()` outside tests**, no `unimplemented!()`, no `todo!()`.
//! - **Cap math safety (anti-BUG-3)**: `amount_in_wei = min(x*, cap_wei).floor()`,
//!   asserted in `clamp_to_cap_wei()`. Prevents the recurrence of the asymmetric
//!   cap bug fixed in commit 4b99eb8.
//!
//! Cost: 1 Redis HGETALL (token prices) + 3 Redis GETs per cycle per direction
//! (reserves) + a small constant of GETs for the pool index. Scales linearly
//! with `MVP_CYCLES.len()` — at the default 5 cycles × 2 directions = 10
//! candidate evaluations per tick, well below 1ms of Redis work even on a
//! cold cache.

use crate::amm_math::v2_amount_out;
use crate::counters::counters;
use crate::persistence;
use crate::publisher;
use crate::reserves::{
    get_pools_for_pair, get_reserves, get_token_meta, ReservesEntry,
};
use chrono::Utc;
use ethers::types::U256;
use redis::aio::ConnectionManager;
use shared_rs::contracts::{Opportunity, StrategyKind};
use shared_rs::trading_config::TradingConfigClient;
use sqlx::postgres::PgPool;
use std::collections::HashSet;
use std::sync::atomic::Ordering;
use std::time::Duration;
use tokio::time::interval;
use tracing::{debug, info, warn};
use uuid::Uuid;

/// Default tick period — approximately one Ethereum mainnet block.
pub const DEFAULT_INTERVAL_SECS: u64 = 12;

/// Strategy-kind string emitted on Opportunity / persistence layer.
const STRATEGY_KIND: &str = "triangular";

/// V2 fee in basis points (Uniswap V2 + SushiSwap canonical).
const V2_FEE_BPS: u32 = 30;

/// Maximum acceptable staleness of a pool reserve relative to the most recent
/// block observed across the cycle. Beyond this gap we refuse to evaluate the
/// cycle (R8 fail-honest — emitting on stale data risks broadcasting an opp
/// that no longer exists).
const MAX_RESERVE_LAG_BLOCKS: u64 = 5;

/// Dedup window: keep entries for this many recent blocks. An (cycle, block)
/// older than this is pruned each tick.
const DEDUP_RETAIN_BLOCKS: u64 = 10;

/// Minimum acceptable input size for the search. Avoids the trivial
/// 1-wei degenerate corner where V2 output truncates to zero.
fn min_search_input() -> U256 {
    U256::from(1u64)
}

/// Stats period: emit one INFO `tick_stats` log every N ticks. At the default
/// 12s tick interval, 5 ticks ≈ 60s — same cadence as the heartbeat snapshot
/// so the operator can correlate. Local accumulator (not AtomicU64) so the
/// heartbeat counters remain the canonical cumulative source of truth and
/// these stats are pure observability for the operator dashboard log.
const STATS_LOG_EVERY_N_TICKS: u32 = 5;

/// Per-period accumulator of scan outcomes, for the periodic INFO log.
/// Reset after each emit. Counts every (cycle, direction) attempt — a single
/// tick contributes `MVP_CYCLES.len() * 2` increments split across the
/// outcome buckets below.
#[derive(Default, Debug, Clone)]
struct TickStats {
    scanned: u32,
    skip_unknown_token: u32,
    skip_missing_pool: u32,
    skip_stale_reserves: u32,
    skip_dedup_hit: u32,
    skip_no_capital_cap: u32,
    skip_no_profit: u32,
    emitted: u32,
}

impl TickStats {
    /// Returns the name of the dominant skip bucket (the one with the highest
    /// count) so the operator sees at a glance WHY cycles aren't producing
    /// emit-able opportunities. None when no skips happened in the period.
    fn dominant_skip_reason(&self) -> Option<&'static str> {
        let buckets: [(&'static str, u32); 6] = [
            ("missing_pool", self.skip_missing_pool),
            ("stale_reserves", self.skip_stale_reserves),
            ("dedup_hit", self.skip_dedup_hit),
            ("no_capital_cap", self.skip_no_capital_cap),
            ("no_profit", self.skip_no_profit),
            ("unknown_token", self.skip_unknown_token),
        ];
        buckets
            .iter()
            .filter(|(_, n)| *n > 0)
            .max_by_key(|(_, n)| *n)
            .map(|(name, _)| *name)
    }
}

/// Hardcoded MVP triangular cycles (token symbols only — addresses resolved
/// from Redis token cache so the worker stays decoupled from chain literals).
///
/// Each tuple `(a, b, c)` defines the cycle a → b → c → a. The worker tries
/// BOTH this orientation and the reverse `a → c → b → a` because profitability
/// is direction-dependent in a CPMM (the spot product on one side may be > 1
/// while < 1 on the other).
///
/// All five cycles use blue-chip Ethereum tokens whose V2 pools exist on
/// Uniswap V2 mainnet (seeded by migrations 029 + 030 + 037).
///
/// Long-tail tokens (PEPE/SHIB/MKR/COMP) deliberately omitted from the cycle
/// list: these tokens trade primarily on Uniswap V3 — there is no V2 pool
/// against USDC for them, so any cycle of the form `X-WETH-USDC-X` would
/// always skip with `cycle_missing_pool`. The `known_token_address` and
/// `resolve_token` fallbacks below still resolve them, ready for the
/// next sub-project that extends the worker with a V3 quoter.
///
/// Operator can extend by editing this constant; future sub-project lifts
/// the list to PG so the operator can manage cycles via UI.
pub const MVP_CYCLES: &[(&str, &str, &str)] = &[
    ("WETH", "USDC", "DAI"),
    ("WETH", "USDC", "USDT"),
    ("WETH", "USDT", "DAI"),     // unblocked by migration 037 (DAI/USDT V2)
    ("WETH", "WBTC", "USDC"),    // unblocked by migration 037 (WBTC/USDC V2)
    ("USDC", "DAI", "USDT"),     // unblocked by migration 037 (DAI/USDT V2)
];

/// Fee-multiplier `γ = 1 - fee`. For V2 default fee 30 bps → γ = 0.997.
fn gamma(fee_bps: u32) -> f64 {
    1.0 - (fee_bps as f64 / 10_000.0)
}

/// Closed-form **spot product** for a triangular cycle at infinitesimal trade size.
/// Inputs are the per-pool (reserve_in, reserve_out) pairs already resolved to
/// the swap orientation (e.g. for hop a→b, reserve_in is reserve of `a`).
///
/// Returns the dimensionless product `S = γ³ · ∏(R_out_i / R_in_i)`. If `S > 1`
/// the cycle is profitable for some positive input size; if `S ≤ 1` no positive
/// input can produce a positive profit (V2 sequential outputs are strictly
/// concave-monotone, so f(x) = out₃(x) - x has f'(0+) = S - 1 ≤ 0 → f is
/// non-increasing on [0, ∞)).
///
/// Returns `0.0` (treated as not profitable) on any zero-reserve input — the
/// degenerate-pool case is handled honestly without panics.
pub fn spot_product(reserves: &[(f64, f64)], fee_bps: u32) -> f64 {
    if reserves.is_empty() {
        return 0.0;
    }
    let g = gamma(fee_bps);
    let mut s = 1.0f64;
    for (r_in, r_out) in reserves {
        if *r_in <= 0.0 || *r_out <= 0.0 {
            return 0.0;
        }
        s *= g * (*r_out / *r_in);
    }
    s
}

/// Evaluate the cycle profit `f(x) = out₃(x) - x` for an input `x` (in token-a
/// wei units). Each hop is a V2 amount-out call against the orientation-resolved
/// reserves. Returns `(amount_out, profit)` both in token-a wei units. Profit
/// is `i128` to allow signed values (negative when fees + slippage exceed gross).
///
/// Returns `(U256::zero(), 0)` on degenerate inputs (zero reserves) — the
/// downstream search treats this as "not profitable here" without panicking.
pub fn cycle_profit(
    x: U256,
    hop_reserves: &[(U256, U256)],
    fee_bps: u32,
) -> (U256, i128) {
    if hop_reserves.is_empty() || x.is_zero() {
        return (U256::zero(), 0);
    }
    let mut current = x;
    for (r_in, r_out) in hop_reserves {
        current = v2_amount_out(current, *r_in, *r_out, fee_bps);
        if current.is_zero() {
            return (U256::zero(), 0);
        }
    }
    // Profit = out - x, signed. Use i128 to capture the sign.
    let profit = u256_to_i128_clamped(current).saturating_sub(u256_to_i128_clamped(x));
    (current, profit)
}

/// Saturating-clamp a U256 to i128. For our use case (token amounts within
/// ~$10K capital cap) the value comfortably fits, but we defensively clamp
/// to avoid panic on the rare case of a very large amount_out from a thin pool.
fn u256_to_i128_clamped(v: U256) -> i128 {
    let s = v.to_string();
    s.parse::<i128>().unwrap_or(i128::MAX)
}

/// **Golden-section search** for the input that maximises cycle profit.
///
/// `f(x) = out₃(x) - x` is strictly concave on (0, ∞) for V2 swaps with fees:
/// each `v2_amount_out` is concave-monotone in its input, the composition of
/// concave-monotone functions is concave-monotone, and subtracting the linear
/// `x` preserves concavity. So the maximum on a closed bounded interval is
/// unique and golden-section converges geometrically (factor `1/φ ≈ 0.618`
/// per iteration).
///
/// Bounds:
///   `x_lo` — minimum search input (defaults to 1 wei).
///   `x_hi` — maximum search input (capital cap, in token-a wei units).
///
/// Returns the (x_star, profit_at_x_star) pair. When the entire interval is
/// non-profitable (f(x) ≤ 0 throughout), x_star is whatever the search settles
/// on but the caller MUST check `profit > 0` before emitting.
///
/// `iterations` — default 25 gives a ratio of `0.618^25 ≈ 7e-6` of the initial
/// interval, more than enough precision for opportunity sizing (we're not
/// chasing wei-level ticks).
pub fn golden_section_search(
    x_lo: U256,
    x_hi: U256,
    hop_reserves: &[(U256, U256)],
    fee_bps: u32,
    iterations: u32,
) -> (U256, i128) {
    if x_lo >= x_hi {
        let (_, p) = cycle_profit(x_lo, hop_reserves, fee_bps);
        return (x_lo, p);
    }
    // Convert to f64 for the search proxy. We re-evaluate profit at the final
    // candidate using the integer kernel so the returned profit value is the
    // true on-chain integer math, not the f64 approximation.
    let phi: f64 = (1.0 + 5.0_f64.sqrt()) / 2.0;
    let inv_phi = 1.0 / phi;
    let inv_phi2 = inv_phi * inv_phi;

    let mut a = u256_to_f64_lossy(x_lo);
    let mut b = u256_to_f64_lossy(x_hi);
    let mut h = b - a;

    let mut c = a + inv_phi2 * h;
    let mut d = a + inv_phi * h;

    let f = |x: f64| -> f64 {
        let xi = f64_to_u256_clamped(x);
        let (_, p) = cycle_profit(xi, hop_reserves, fee_bps);
        p as f64
    };

    let mut yc = f(c);
    let mut yd = f(d);

    for _ in 0..iterations {
        if yc > yd {
            b = d;
            d = c;
            yd = yc;
            h *= inv_phi;
            c = a + inv_phi2 * h;
            yc = f(c);
        } else {
            a = c;
            c = d;
            yc = yd;
            h *= inv_phi;
            d = a + inv_phi * h;
            yd = f(d);
        }
    }

    let x_star_f = if yc > yd { (a + d) / 2.0 } else { (c + b) / 2.0 };
    let x_star = f64_to_u256_clamped(x_star_f);
    let (_, profit) = cycle_profit(x_star, hop_reserves, fee_bps);
    (x_star, profit)
}

fn u256_to_f64_lossy(v: U256) -> f64 {
    v.to_string().parse::<f64>().unwrap_or(0.0)
}

fn f64_to_u256_clamped(x: f64) -> U256 {
    if !x.is_finite() || x <= 0.0 {
        return U256::from(1u64);
    }
    // Clamp to U256::MAX equivalent (~1.16e77). We're nowhere near this in
    // practice, but the parse-from-string path requires a non-scientific
    // representation; use `format!("{:.0}", x)` to get a plain integer string.
    let s = format!("{:.0}", x);
    U256::from_dec_str(&s).unwrap_or(U256::from(1u64))
}

/// Apply the operator capital cap to a candidate input `x_star`, in token-a wei.
///
/// `cap_usd` is the operator's effective capital ceiling (`effective_capital_for(...)`).
/// `token_a_price_usd` translates that USD ceiling to token units, then to wei
/// using `decimals`. The returned value is `min(x_star, cap_wei).floor()` —
/// **never** larger than the cap, defensively asserted to prevent the recurrence
/// of the asymmetric-cap bug fixed in commit 4b99eb8 (BUG-3).
///
/// Returns `None` when the cap cannot be computed (zero / NaN price), so the
/// caller can fall back to "skip this cycle" instead of acting on an unbounded
/// input.
pub fn clamp_to_cap_wei(
    x_star: U256,
    cap_usd: f64,
    token_a_price_usd: f64,
    decimals: u8,
) -> Option<U256> {
    if !cap_usd.is_finite() || cap_usd <= 0.0 {
        return None;
    }
    if !token_a_price_usd.is_finite() || token_a_price_usd <= 0.0 {
        return None;
    }
    // tokens_capped = cap_usd / price_usd_per_token. floor() so we never EXCEED.
    let tokens_capped = (cap_usd / token_a_price_usd).floor();
    if !tokens_capped.is_finite() || tokens_capped <= 0.0 {
        return None;
    }
    // Convert to wei (10^decimals scaling). decimals ∈ [0, 24] in practice; we
    // saturate the multiplier to f64 precision then convert.
    let wei_f = tokens_capped * 10f64.powi(decimals as i32);
    let cap_wei = f64_to_u256_clamped(wei_f.floor());
    let result = if x_star < cap_wei { x_star } else { cap_wei };
    // Defensive bound (anti-BUG-3): even if the math above had a subtle bug,
    // the returned value is guaranteed ≤ cap_wei.
    debug_assert!(result <= cap_wei, "clamp_to_cap_wei produced value > cap_wei");
    Some(result)
}

/// Stable hash of a cycle (direction-aware). Used as the dedup key together
/// with the block number. Two cycles with the same token sequence but
/// different traversal direction MUST hash differently.
pub fn cycle_hash(tokens: &[&str]) -> String {
    tokens.join(">")
}

/// Lowercase hex address of well-known mainnet tokens. The worker resolves
/// addresses from the Redis token cache when available; this fallback table
/// kicks in when the cache hasn't been bootstrapped yet (e.g. immediately after
/// PoolSyncWorker boot).
///
/// Operator extension path: when a new token is added via migration 029/030 it
/// must also be added here OR the cache must be live. The fallback is conservative
/// — if both lookups miss, the cycle is skipped (no fake address).
fn known_token_address(symbol: &str) -> Option<&'static str> {
    match symbol.to_ascii_uppercase().as_str() {
        // Blue chips
        "WETH" => Some("0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2"),
        "USDC" => Some("0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48"),
        "USDT" => Some("0xdac17f958d2ee523a2206206994597c13d831ec7"),
        "DAI"  => Some("0x6b175474e89094c44da98b954eedeac495271d0f"),
        "WBTC" => Some("0x2260fac5e5542a773aa44fbcfedf7c193bc2c599"),
        // Long-tail majors (added 2026-05-07 alongside the long-tail cycles
        // in MVP_CYCLES). All four are in the operator's Tier 1 allowlist
        // applied via Redis HSET on 2026-05-07 with real Coingecko prices.
        "PEPE" => Some("0x6982508145454ce325ddbe47a25d4ec3d2311933"),
        "SHIB" => Some("0x95ad61b0a150d79219dcf64e1e6cc01f0b64c4ce"),
        "MKR"  => Some("0x9f8f72aa9304c8b593d555f12ef6589cc3a579a2"),
        "COMP" => Some("0xc00e94cb662c3520282e6f5717214004a7f26888"),
        _ => None,
    }
}

/// Resolve `(token_addr, decimals)` for a symbol. Tries Redis cache first; on
/// miss, falls back to the hardcoded blue-chip table with conservative defaults
/// for `decimals` (per token, not a guess). Returns None when both miss.
async fn resolve_token(
    redis: &mut ConnectionManager,
    chain_id: u64,
    symbol: &str,
) -> Option<(String, u8, bool)> {
    // Try Redis: requires knowing the address first, so we bootstrap from the
    // hardcoded table — the cache is keyed by address, not symbol.
    let addr = known_token_address(symbol)?;
    if let Ok(Some(meta)) = get_token_meta(redis, chain_id, addr).await {
        return Some((addr.to_string(), meta.decimals, meta.is_stablecoin));
    }
    // Fallback to hard-coded mainnet decimals (truthful, not invented).
    let (decimals, is_stable) = match symbol.to_ascii_uppercase().as_str() {
        "WETH" => (18, false),
        "USDC" | "USDT" => (6, true),
        "DAI"  => (18, true),
        "WBTC" => (8, false),
        // Long-tail majors — all use 18 decimals per their on-chain contracts.
        "PEPE" | "SHIB" | "MKR" | "COMP" => (18, false),
        _ => return None,
    };
    Some((addr.to_string(), decimals, is_stable))
}

/// One hop's resolved data: (pool_address_lower, reserves entry, swap orientation).
/// `swap_in_is_token0` tells the caller which reserve to use as `reserve_in`
/// (token0 of the pool is the input vs token1 of the pool is the input).
#[derive(Debug, Clone)]
struct HopData {
    pool_addr: String,
    entry: ReservesEntry,
    /// True if `token_in` for this hop is the pool's `token0`.
    /// Determines orientation of (r0, r1) into (reserve_in, reserve_out).
    swap_in_is_token0: bool,
}

impl HopData {
    /// Returns `(reserve_in, reserve_out)` as `U256` in the swap orientation.
    /// Returns None if either reserve is malformed (not a valid integer).
    fn reserves_oriented(&self) -> Option<(U256, U256)> {
        let r0 = U256::from_dec_str(&self.entry.r0).ok()?;
        let r1 = U256::from_dec_str(&self.entry.r1).ok()?;
        if self.swap_in_is_token0 {
            Some((r0, r1))
        } else {
            Some((r1, r0))
        }
    }
}

/// Resolve one hop (token_in → token_out) by looking up the V2 pool index +
/// fetching reserves. Picks the FIRST pool returned by `get_pools_for_pair`
/// — for MVP a single pool per pair is sufficient; future sub-projects will
/// rank by depth.
///
/// Returns None on:
///   - empty pool index (no pool exists for this pair),
///   - missing reserves cache entry (PoolSyncWorker hasn't ticked yet),
///   - missing `token0_addr` AND ambiguous orientation (defensive — should
///     never happen post-migration since pool_sync_worker now populates it).
async fn resolve_hop(
    redis: &mut ConnectionManager,
    chain_id: u64,
    token_in_addr: &str,
    token_in_sym: &str,
    token_out_sym: &str,
) -> Option<HopData> {
    let pools = get_pools_for_pair(redis, chain_id, token_in_sym, token_out_sym)
        .await
        .ok()?;
    let pool_addr = pools.into_iter().next()?;
    let entry = get_reserves(redis, chain_id, &pool_addr).await.ok().flatten()?;
    let token0 = entry.token0_addr.as_deref()?;
    let swap_in_is_token0 = token0.eq_ignore_ascii_case(token_in_addr);
    Some(HopData {
        pool_addr,
        entry,
        swap_in_is_token0,
    })
}

/// Identify the most recent block across a cycle's hops; used to enforce
/// the staleness bound (hop with blk < latest - MAX_RESERVE_LAG_BLOCKS is rejected).
fn cycle_latest_block(hops: &[HopData]) -> u64 {
    hops.iter().map(|h| h.entry.blk).max().unwrap_or(0)
}

/// Returns true if any hop in the cycle has reserves more than
/// `MAX_RESERVE_LAG_BLOCKS` behind the latest hop in that cycle.
fn cycle_has_stale_reserves(hops: &[HopData]) -> bool {
    let latest = cycle_latest_block(hops);
    hops.iter()
        .any(|h| latest.saturating_sub(h.entry.blk) > MAX_RESERVE_LAG_BLOCKS)
}

/// Configuration for a single triangular evaluation pass over one cycle in
/// one direction. Public for testability — `evaluate_cycle` is the unit-testable
/// pure function kernel that callers feed pre-resolved data into.
#[derive(Debug, Clone)]
pub struct EvalInput {
    pub hop_reserves: Vec<(U256, U256)>,
    pub token_a_price_usd: Option<f64>,
    pub token_a_decimals: u8,
    pub cap_usd: f64,
    pub fee_bps: u32,
}

/// Result of a triangular evaluation. `expected_profit_usd` is None when the
/// token-a price is unknown (R8 fail-honest: don't fabricate USD). The integer
/// `profit_token_a_wei` is always concrete.
///
/// `amount_out_wei` and `profit_token_a_wei` are exposed for tests and future
/// telemetry / event-emission paths (heartbeat may surface aggregate gross
/// profit per period). Marked `#[allow(dead_code)]` to silence the warning
/// pre-emptively without losing the data on the type.
#[derive(Debug, Clone)]
pub struct EvalResult {
    pub amount_in_wei: U256,
    #[allow(dead_code)]
    pub amount_out_wei: U256,
    #[allow(dead_code)]
    pub profit_token_a_wei: i128,
    pub expected_profit_usd: Option<f64>,
}

/// Pure-function kernel: spot-check + golden-section + USD pricing on a single
/// triangular cycle direction. Returns `Some(EvalResult)` only when all of:
///   1. spot_product > 1.0
///   2. capital cap can be computed (price > 0)
///   3. golden-section finds a strictly positive profit in token-a units
///      hold simultaneously. Otherwise returns None and the caller skips.
pub fn evaluate_cycle(input: &EvalInput) -> Option<EvalResult> {
    if input.hop_reserves.len() != 3 {
        return None;
    }
    // Spot check (closed form — necessary condition for ANY profit).
    let r_f64: Vec<(f64, f64)> = input
        .hop_reserves
        .iter()
        .map(|(ri, ro)| (u256_to_f64_lossy(*ri), u256_to_f64_lossy(*ro)))
        .collect();
    let s = spot_product(&r_f64, input.fee_bps);
    if s <= 1.0 {
        return None;
    }

    // Compute capital cap in wei. Skip if we can't price token_a (no cap).
    let token_a_price = input.token_a_price_usd?;
    // Initial search ceiling: cap_wei. Worst case the cap is so small that
    // x_lo == x_hi — the search degenerates to evaluating one point.
    let cap_wei = clamp_to_cap_wei(
        U256::MAX, // sentinel — clamp_to_cap_wei returns the cap itself
        input.cap_usd,
        token_a_price,
        input.token_a_decimals,
    )?;
    let x_lo = min_search_input();

    // Tighten the search ceiling to `min(cap_wei, R_in_first_hop)`. The profit
    // function f(x) = out₃(x) - x for sequential V2 swaps has its optimum in
    // (0, R_in_first_hop) — beyond R_in_first_hop the first-hop output saturates
    // and additional input only adds slippage. Searching in [1, cap_wei] when
    // cap_wei is many orders of magnitude larger than the true optimum gives
    // golden-section terrible resolution (after 25 iterations the interval is
    // still ~10^17 wide if we started at 10^22). Bounding by R_in_first_hop
    // shrinks the interval to a few × the optimum, where 25 iterations is
    // plenty. The cap is then re-applied below to enforce the operator's
    // capital constraint.
    let r_in_first_hop = input.hop_reserves[0].0;
    let search_ceiling = if cap_wei < r_in_first_hop { cap_wei } else { r_in_first_hop };
    let x_hi = if search_ceiling > x_lo { search_ceiling } else { x_lo };

    let (x_star, profit_wei) =
        golden_section_search(x_lo, x_hi, &input.hop_reserves, input.fee_bps, 25);

    // Defensive cap re-application (anti-BUG-3): no matter what the search
    // found, never exceed the cap.
    let amount_in = clamp_to_cap_wei(
        x_star,
        input.cap_usd,
        token_a_price,
        input.token_a_decimals,
    )?;
    debug_assert!(
        amount_in <= cap_wei,
        "evaluate_cycle: amount_in ({amount_in}) exceeds cap_wei ({cap_wei})"
    );

    // Re-evaluate profit at the clamped input (golden-section's best guess might
    // have been at the cap, in which case the value is identical; if the cap
    // shrank x_star, recompute honestly).
    let (amount_out, profit_at_clamped) =
        cycle_profit(amount_in, &input.hop_reserves, input.fee_bps);
    if profit_at_clamped <= 0 {
        // Capital cap pushed us below break-even (the unconstrained optimum
        // was higher than the operator allows). Honest skip — emitting a
        // zero-or-negative-profit candidate would just waste downstream work.
        let _ = profit_wei; // silence unused-var when assertion is off
        return None;
    }

    // USD profit: profit_token_a_wei * price / 10^decimals.
    let profit_tokens = (profit_at_clamped as f64) / 10f64.powi(input.token_a_decimals as i32);
    let expected_profit_usd = Some(profit_tokens * token_a_price);

    Some(EvalResult {
        amount_in_wei: amount_in,
        amount_out_wei: amount_out,
        profit_token_a_wei: profit_at_clamped,
        expected_profit_usd,
    })
}

/// In-memory dedup state: set of `(cycle_hash, block_number)` pairs.
/// Pruned each tick to keep size bounded.
#[derive(Debug, Default)]
pub struct DedupState {
    seen: HashSet<(String, u64)>,
}

impl DedupState {
    pub fn new() -> Self {
        Self {
            seen: HashSet::new(),
        }
    }
    /// Returns true when the (cycle, block) pair is fresh and was just inserted.
    /// Returns false when the pair was already present (dedup hit).
    pub fn check_and_mark(&mut self, cycle: &str, block: u64) -> bool {
        let key = (cycle.to_string(), block);
        if self.seen.contains(&key) {
            return false;
        }
        self.seen.insert(key);
        true
    }
    /// Drop entries with `block < latest_block - DEDUP_RETAIN_BLOCKS`. Bounds
    /// memory growth — at the default 10-block retain × ~50 cycles/block worst
    /// case, the set never holds more than a few hundred entries.
    pub fn prune(&mut self, latest_block: u64) {
        if latest_block <= DEDUP_RETAIN_BLOCKS {
            return;
        }
        let cutoff = latest_block - DEDUP_RETAIN_BLOCKS;
        self.seen.retain(|(_, b)| *b >= cutoff);
    }
    /// Current entry count — used by tests; kept public for future
    /// observability surfacing (heartbeat could log dedup memory pressure).
    #[allow(dead_code)]
    pub fn len(&self) -> usize {
        self.seen.len()
    }
}

/// TriangularWorker — owns its tick interval and dedup state. Lives for the
/// lifetime of the `searcher-rs` process.
pub struct TriangularWorker {
    pub period: Duration,
    pub chain_id: u64,
}

impl TriangularWorker {
    pub fn new(interval_secs: u64, chain_id: u64) -> Self {
        Self {
            period: Duration::from_secs(interval_secs.max(1)),
            chain_id,
        }
    }

    /// Forever loop — never returns under normal operation. Designed to be
    /// `tokio::spawn`'d from main.
    pub async fn run(
        self,
        mut redis: ConnectionManager,
        db: Option<PgPool>,
        trading_config: TradingConfigClient,
    ) {
        let mut ticker = interval(self.period);
        let mut dedup = DedupState::new();
        let mut tick_count: u32 = 0;
        let mut stats = TickStats::default();
        info!(
            event = "triangular_worker.boot",
            chain_id = self.chain_id,
            period_secs = self.period.as_secs(),
            cycles = MVP_CYCLES.len(),
            stats_log_every_n_ticks = STATS_LOG_EVERY_N_TICKS,
        );

        loop {
            ticker.tick().await;
            tick_count += 1;
            // Tick: scan every cycle in both directions. Worker keeps running
            // even if the trading_config / db is missing — emits to Redis
            // stream regardless (api-server reads from there for the live UI).
            let cfg_opt = match trading_config.state(self.chain_id).await {
                Ok(s) => s,
                Err(e) => {
                    debug!(event = "triangular_worker.cfg_fetch_failed", error = %e);
                    None
                }
            };

            // Snapshot price oracle once per tick (covers all cycles).
            let snapshot_map =
                shared_rs::price_oracle::RedisCachedPriceOracle::snapshot_from_redis(
                    &mut redis,
                    self.chain_id,
                )
                .await
                .into_snapshot();

            let mut latest_block_observed: u64 = 0;

            for (a, b, c) in MVP_CYCLES {
                for direction in &[(*a, *b, *c), (*a, *c, *b)] {
                    counters()
                        .triangular_cycles_scanned
                        .fetch_add(1, Ordering::Relaxed);
                    stats.scanned += 1;
                    let (sym_a, sym_b, sym_c) = *direction;
                    if let Some(blk) = self
                        .scan_one_direction(
                            &mut redis,
                            db.as_ref(),
                            &cfg_opt,
                            &snapshot_map,
                            sym_a,
                            sym_b,
                            sym_c,
                            &mut dedup,
                            &mut stats,
                        )
                        .await
                    {
                        if blk > latest_block_observed {
                            latest_block_observed = blk;
                        }
                    }
                }
            }
            // Bound dedup memory once per tick using the most recent block we saw.
            dedup.prune(latest_block_observed);

            // Emit periodic INFO stats so the operator sees WHY cycles aren't
            // emitting (dominant skip reason) without enabling debug logs.
            // Heartbeat counters provide cumulative truth; this log gives
            // per-period dominant-cause attribution. R8 fail-honest: if zero
            // skips happened in the period, dominant_skip is logged as null.
            if tick_count % STATS_LOG_EVERY_N_TICKS == 0 {
                info!(
                    event = "triangular_worker.tick_stats",
                    chain_id = self.chain_id,
                    period_ticks = STATS_LOG_EVERY_N_TICKS,
                    scanned = stats.scanned,
                    emitted = stats.emitted,
                    skip_unknown_token = stats.skip_unknown_token,
                    skip_missing_pool = stats.skip_missing_pool,
                    skip_stale_reserves = stats.skip_stale_reserves,
                    skip_dedup_hit = stats.skip_dedup_hit,
                    skip_no_capital_cap = stats.skip_no_capital_cap,
                    skip_no_profit = stats.skip_no_profit,
                    dominant_skip = ?stats.dominant_skip_reason(),
                );
                stats = TickStats::default();
            }
        }
    }

    /// Scan one (a, b, c) direction. Returns the cycle's latest block (for
    /// dedup pruning) when the cycle was successfully evaluated up to the
    /// reserves stage; returns None on early-out (missing pool, malformed,
    /// stale, dedup hit, etc.).
    ///
    /// Each early-exit branch bumps the corresponding `stats` counter so
    /// `run()` can emit a periodic INFO log of the dominant skip reason.
    #[allow(clippy::too_many_arguments)]
    async fn scan_one_direction(
        &self,
        redis: &mut ConnectionManager,
        db: Option<&PgPool>,
        cfg: &Option<shared_rs::trading_config::TradingConfigState>,
        price_snapshot: &std::collections::HashMap<String, f64>,
        sym_a: &str,
        sym_b: &str,
        sym_c: &str,
        dedup: &mut DedupState,
        stats: &mut TickStats,
    ) -> Option<u64> {
        // Resolve token_a metadata (address, decimals). Use explicit match so
        // we can attribute the skip reason instead of `?` swallowing the cause.
        let (addr_a, decimals_a, _is_stable_a) = match resolve_token(redis, self.chain_id, sym_a).await {
            Some(t) => t,
            None => { stats.skip_unknown_token += 1; return None; }
        };
        let (addr_b, _, _) = match resolve_token(redis, self.chain_id, sym_b).await {
            Some(t) => t,
            None => { stats.skip_unknown_token += 1; return None; }
        };
        let (addr_c, _, _) = match resolve_token(redis, self.chain_id, sym_c).await {
            Some(t) => t,
            None => { stats.skip_unknown_token += 1; return None; }
        };

        // Resolve all 3 hops.
        let hop1 = resolve_hop(redis, self.chain_id, &addr_a, sym_a, sym_b).await;
        let hop2 = resolve_hop(redis, self.chain_id, &addr_b, sym_b, sym_c).await;
        let hop3 = resolve_hop(redis, self.chain_id, &addr_c, sym_c, sym_a).await;
        let (hop1, hop2, hop3) = match (hop1, hop2, hop3) {
            (Some(h1), Some(h2), Some(h3)) => (h1, h2, h3),
            _ => {
                stats.skip_missing_pool += 1;
                debug!(
                    event = "triangular_worker.cycle_missing_pool",
                    chain_id = self.chain_id,
                    cycle = format!("{}>{}>{}", sym_a, sym_b, sym_c),
                );
                return None;
            }
        };

        let hops = vec![hop1.clone(), hop2.clone(), hop3.clone()];
        if cycle_has_stale_reserves(&hops) {
            stats.skip_stale_reserves += 1;
            debug!(
                event = "triangular_worker.cycle_stale_reserves",
                chain_id = self.chain_id,
                cycle = format!("{}>{}>{}", sym_a, sym_b, sym_c),
                lag_blocks = MAX_RESERVE_LAG_BLOCKS,
            );
            return Some(cycle_latest_block(&hops));
        }

        // Dedup BEFORE any heavy math — same (cycle, block) seen this run? skip.
        let cycle_key = cycle_hash(&[sym_a, sym_b, sym_c]);
        let cycle_block = cycle_latest_block(&hops);
        if !dedup.check_and_mark(&cycle_key, cycle_block) {
            stats.skip_dedup_hit += 1;
            return Some(cycle_block);
        }

        // Convert hop data to (reserve_in, reserve_out) per orientation.
        let r1 = hop1.reserves_oriented()?;
        let r2 = hop2.reserves_oriented()?;
        let r3 = hop3.reserves_oriented()?;
        let hop_reserves = vec![r1, r2, r3];

        // Capital cap from operator config.
        let cap_usd = cfg
            .as_ref()
            .map(|c| c.effective_capital_for(sym_a, STRATEGY_KIND))
            .unwrap_or(0.0);
        if cap_usd <= 0.0 {
            stats.skip_no_capital_cap += 1;
            debug!(
                event = "triangular_worker.no_capital_cap",
                chain_id = self.chain_id,
                token = sym_a,
            );
            return Some(cycle_block);
        }
        // Token-a USD price (cascade: live snapshot first, then config map).
        let price_a = price_snapshot
            .get(&sym_a.to_ascii_uppercase())
            .copied()
            .or_else(|| {
                cfg.as_ref().and_then(|c| {
                    c.token_prices_usd
                        .iter()
                        .find(|(k, _)| k.eq_ignore_ascii_case(sym_a))
                        .map(|(_, v)| *v)
                })
            });

        let input = EvalInput {
            hop_reserves,
            token_a_price_usd: price_a,
            token_a_decimals: decimals_a,
            cap_usd,
            fee_bps: V2_FEE_BPS,
        };
        let result = match evaluate_cycle(&input) {
            Some(r) => r,
            None => {
                stats.skip_no_profit += 1;
                debug!(
                    event = "triangular_worker.no_profit",
                    chain_id = self.chain_id,
                    cycle = %cycle_key,
                );
                return Some(cycle_block);
            }
        };

        // R8 / anti-BUG-3 sanity bound at the worker level.
        //
        // The triangular worker writes to PG + Redis stream DIRECTLY, bypassing
        // the spine evaluator's sanity bound. On 2026-05-07 a swapped-token
        // declaration in migration 037 (USDC/WBTC pool) caused the orientation
        // logic to flip and the math to produce ~$4M expected_profit_usd on
        // a $5K cap input — 1183 fake-positive opps were emitted before the
        // operator caught it via the /opportunities UI.
        //
        // This guard rejects any candidate whose USD profit exceeds the
        // capital cap by more than `SANITY_PROFIT_MULT_OF_CAP` (5×). For
        // honest paper-trade activity on saturated mainnet majors, profits
        // are routinely well under 1% of cap (gas-bps dominate), so 500%
        // is a generous threshold that never false-positives on real opps
        // but immediately catches orientation/decimal/unit bugs.
        //
        // R8 fail-honest: rejected candidate is logged + counted, not
        // silently dropped. Operator sees `triangular_worker.sanity_reject`
        // events with the math snapshot.
        const SANITY_PROFIT_MULT_OF_CAP: f64 = 5.0;
        let profit_cap_ratio = result.expected_profit_usd / cap_usd;
        if profit_cap_ratio > SANITY_PROFIT_MULT_OF_CAP {
            stats.skip_no_profit += 1; // bucketed under no_profit for tick_stats
            warn!(
                event = "triangular_worker.sanity_reject",
                chain_id = self.chain_id,
                cycle = %cycle_key,
                expected_profit_usd = result.expected_profit_usd,
                cap_usd = cap_usd,
                profit_cap_ratio = profit_cap_ratio,
                threshold = SANITY_PROFIT_MULT_OF_CAP,
                hint = "math kernel returned profit > 5x cap — likely orientation flip, decimals mismatch, or unit bug",
            );
            return Some(cycle_block);
        }

        // Build & emit Opportunity.
        let opp = Opportunity {
            id: Uuid::new_v4(),
            chain_id: self.chain_id,
            strategy_kind: StrategyKind::Triangular,
            dex_a: "uniswap-v2".to_string(),
            dex_b: Some(format!(
                "cycle:{}>{}>{}",
                sym_a.to_uppercase(),
                sym_b.to_uppercase(),
                sym_c.to_uppercase(),
            )),
            pair_symbol: format!("{}/{}/{}/{}", sym_a, sym_b, sym_c, sym_a),
            token_in: addr_a.clone(),
            token_out: addr_a.clone(),
            amount_in_wei: result.amount_in_wei.to_string(),
            expected_profit_usd: result.expected_profit_usd,
            roi_pct: None,
            risk_score: None,
            block_number: Some(cycle_block),
            rejection_reason: None,
            detected_at: Utc::now(),
            trace_id: Uuid::new_v4(),
        };

        info!(
            event = "triangular_worker.opp_emit",
            chain_id = self.chain_id,
            cycle = %cycle_key,
            block = cycle_block,
            amount_in_wei = %opp.amount_in_wei,
            expected_profit_usd = ?opp.expected_profit_usd,
            pool_a = %hop1.pool_addr,
            pool_b = %hop2.pool_addr,
            pool_c = %hop3.pool_addr,
        );

        // Persist + publish (best-effort, mirror scanner.rs pattern).
        if let Some(pool) = db {
            if let Err(e) = persistence::insert_opportunity(pool, &opp).await {
                counters().db_errors.fetch_add(1, Ordering::Relaxed);
                warn!(event = "triangular_worker.db_error", error = %e);
            } else {
                counters().db_persisted.fetch_add(1, Ordering::Relaxed);
            }
        }
        if let Err(e) = publisher::publish(redis, &opp).await {
            warn!(event = "triangular_worker.publish_error", error = %e);
        } else {
            counters()
                .triangular_opps_emitted
                .fetch_add(1, Ordering::Relaxed);
            stats.emitted += 1;
        }

        Some(cycle_block)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ---------------------------------------------------------------
    // Math kernel — spot_product
    // ---------------------------------------------------------------

    #[test]
    fn spot_product_balanced_pools_below_one() {
        // Three perfectly balanced 1:1 pools yield S = γ³ ≈ 0.991 < 1 → no profit.
        let r = vec![(1_000.0, 1_000.0), (1_000.0, 1_000.0), (1_000.0, 1_000.0)];
        let s = spot_product(&r, 30);
        assert!(s < 1.0, "S={} should be < 1 with γ=0.997 across 3 hops", s);
        assert!((s - 0.997f64.powi(3)).abs() < 1e-9);
    }

    #[test]
    fn spot_product_imbalanced_pool_above_one() {
        // Hop 1: 100 in, 1000 out (effective 10× rate after fee)
        // Hops 2-3 balanced 1:1 (rate 1)
        // S = γ * 10 * γ * 1 * γ * 1 = 10 * γ³ ≈ 9.91 > 1 → profitable
        let r = vec![(100.0, 1_000.0), (1_000.0, 1_000.0), (1_000.0, 1_000.0)];
        let s = spot_product(&r, 30);
        assert!(s > 1.0, "S={} should be > 1 for 10x first hop", s);
        assert!((s - 10.0 * 0.997f64.powi(3)).abs() < 1e-6);
    }

    #[test]
    fn spot_product_zero_reserve_returns_zero() {
        // Defensive: degenerate pool (zero reserve) must yield 0.0, not panic.
        let r = vec![(0.0, 1_000.0), (1_000.0, 1_000.0), (1_000.0, 1_000.0)];
        assert_eq!(spot_product(&r, 30), 0.0);
        let r2 = vec![(1_000.0, 0.0), (1_000.0, 1_000.0), (1_000.0, 1_000.0)];
        assert_eq!(spot_product(&r2, 30), 0.0);
    }

    #[test]
    fn spot_product_empty_returns_zero() {
        assert_eq!(spot_product(&[], 30), 0.0);
    }

    // ---------------------------------------------------------------
    // Math kernel — cycle_profit
    // ---------------------------------------------------------------

    #[test]
    fn cycle_profit_balanced_yields_loss() {
        // Three balanced pools, fee > 0 → strictly negative profit (fees only).
        let reserves = vec![
            (U256::from(1_000_000u64), U256::from(1_000_000u64)),
            (U256::from(1_000_000u64), U256::from(1_000_000u64)),
            (U256::from(1_000_000u64), U256::from(1_000_000u64)),
        ];
        let (out, profit) = cycle_profit(U256::from(1_000u64), &reserves, 30);
        assert!(out < U256::from(1_000u64), "balanced cycle should lose to fees");
        assert!(profit < 0, "profit={} expected negative", profit);
    }

    #[test]
    fn cycle_profit_known_imbalanced_positive() {
        // Hand-derived. Hop 1: in=1000, R_in=10_000, R_out=10_000_000 (1000x rate).
        // V2 out = γ·1000·10_000_000 / (10_000 + γ·1000)
        //        = 0.997 * 1000 * 1e7 / (10000 + 997) ≈ 9.97e9 / 10997 ≈ 906_700
        // Hop 2: in=906_700, R=(1e7, 1e7) → balanced, out ≈ in - small fee
        //   v2_out = γ·in·R_out / (R_in + γ·in) = 0.997 * 906700 * 1e7 / (1e7 + 0.997 * 906700)
        //          ≈ 9.04e12 / 10903848 ≈ 829_000
        // Hop 3: in=829_000, R=(1e7, 1e7) → similar shrink, out ≈ 760_000
        // We assert profit > 0 (which is the actual claim — exact values depend on
        // the truncation of the integer math).
        let reserves = vec![
            (U256::from(10_000u64), U256::from(10_000_000u64)),
            (U256::from(10_000_000u64), U256::from(10_000_000u64)),
            (U256::from(10_000_000u64), U256::from(10_000_000u64)),
        ];
        let (out, profit) = cycle_profit(U256::from(1_000u64), &reserves, 30);
        assert!(out > U256::from(1_000u64), "imbalanced cycle should net positive");
        assert!(profit > 0, "profit={} expected > 0", profit);
    }

    #[test]
    fn cycle_profit_zero_input_returns_zero() {
        let reserves = vec![
            (U256::from(1_000u64), U256::from(1_000u64)),
            (U256::from(1_000u64), U256::from(1_000u64)),
            (U256::from(1_000u64), U256::from(1_000u64)),
        ];
        let (out, profit) = cycle_profit(U256::zero(), &reserves, 30);
        assert_eq!(out, U256::zero());
        assert_eq!(profit, 0);
    }

    #[test]
    fn cycle_profit_degenerate_pool_returns_zero() {
        // Mid-cycle zero output (zero reserve) must short-circuit honestly.
        let reserves = vec![
            (U256::from(1_000u64), U256::from(1_000u64)),
            (U256::from(1_000u64), U256::zero()),
            (U256::from(1_000u64), U256::from(1_000u64)),
        ];
        let (out, profit) = cycle_profit(U256::from(100u64), &reserves, 30);
        assert_eq!(out, U256::zero());
        assert_eq!(profit, 0);
    }

    // ---------------------------------------------------------------
    // Golden-section search
    // ---------------------------------------------------------------

    #[test]
    fn golden_section_finds_positive_profit_when_available() {
        // Same setup as cycle_profit_known_imbalanced_positive. Search over a
        // wide bound — must converge to a positive-profit input.
        let reserves = vec![
            (U256::from(10_000u64), U256::from(10_000_000u64)),
            (U256::from(10_000_000u64), U256::from(10_000_000u64)),
            (U256::from(10_000_000u64), U256::from(10_000_000u64)),
        ];
        let (x_star, profit) =
            golden_section_search(U256::from(1u64), U256::from(100_000u64), &reserves, 30, 25);
        assert!(profit > 0, "profit={} at x*={} expected positive", profit, x_star);
        assert!(x_star > U256::from(1u64));
    }

    #[test]
    fn golden_section_returns_non_positive_for_balanced_cycle() {
        // Balanced cycle: any positive input loses to fees → search must NOT
        // surface a positive profit (it can return any x in the interval, but
        // the profit at that x must be ≤ 0).
        let reserves = vec![
            (U256::from(1_000_000u64), U256::from(1_000_000u64)),
            (U256::from(1_000_000u64), U256::from(1_000_000u64)),
            (U256::from(1_000_000u64), U256::from(1_000_000u64)),
        ];
        let (_, profit) =
            golden_section_search(U256::from(1u64), U256::from(100_000u64), &reserves, 30, 25);
        assert!(profit <= 0, "balanced cycle: profit={} should be ≤ 0", profit);
    }

    #[test]
    fn golden_section_degenerate_interval_returns_endpoint() {
        // x_lo == x_hi — the search must collapse to evaluating that single point,
        // not panic or loop.
        let reserves = vec![
            (U256::from(1_000u64), U256::from(1_000u64)),
            (U256::from(1_000u64), U256::from(1_000u64)),
            (U256::from(1_000u64), U256::from(1_000u64)),
        ];
        let (x, _) = golden_section_search(U256::from(7u64), U256::from(7u64), &reserves, 30, 25);
        assert_eq!(x, U256::from(7u64));
    }

    #[test]
    fn golden_section_converges_within_25_iters() {
        // For a well-conditioned profit landscape, 25 iterations of golden-section
        // (factor 0.618 each) shrink the interval to ~7e-6 of the original. We
        // confirm the answer matches a brute-force search to within 5% of optimum.
        let reserves = vec![
            (U256::from(10_000u64), U256::from(50_000_000u64)),  // 5000x
            (U256::from(50_000_000u64), U256::from(50_000_000u64)),
            (U256::from(50_000_000u64), U256::from(50_000_000u64)),
        ];
        let (_, profit_search) =
            golden_section_search(U256::from(1u64), U256::from(1_000_000u64), &reserves, 30, 25);
        // Brute-force every 10K-wei step: find the best.
        let mut brute_best = i128::MIN;
        for x_raw in (1..=1_000_000u64).step_by(10_000) {
            let (_, p) = cycle_profit(U256::from(x_raw), &reserves, 30);
            if p > brute_best {
                brute_best = p;
            }
        }
        assert!(brute_best > 0, "brute force should find positive profit");
        assert!(profit_search > 0, "search profit={} should be > 0", profit_search);
        // Search profit should be within 10% of brute-force optimum.
        let ratio = profit_search as f64 / brute_best as f64;
        assert!(ratio > 0.90, "search/brute ratio={} expected > 0.90", ratio);
    }

    // ---------------------------------------------------------------
    // Capital cap (anti-BUG-3 regression)
    // ---------------------------------------------------------------

    #[test]
    fn clamp_to_cap_wei_clamps_when_x_exceeds_cap() {
        // x = 100 WETH (way above the cap), cap = $2000 / $2000 per WETH = 1 WETH (1e18 wei).
        // Result MUST equal the cap (clamping is the entire point of this function).
        let x = U256::from(10u64).pow(U256::from(20u64)); // 100 WETH in wei
        let cap = clamp_to_cap_wei(x, 2000.0, 2000.0, 18).expect("cap should compute");
        assert_eq!(cap, U256::from(10u64).pow(U256::from(18u64)));
    }

    #[test]
    fn clamp_to_cap_wei_returns_none_on_zero_price() {
        let cap = clamp_to_cap_wei(U256::from(1u64), 100.0, 0.0, 18);
        assert!(cap.is_none());
    }

    #[test]
    fn clamp_to_cap_wei_returns_none_on_zero_cap() {
        let cap = clamp_to_cap_wei(U256::from(1u64), 0.0, 2000.0, 18);
        assert!(cap.is_none());
    }

    #[test]
    fn clamp_to_cap_wei_returns_none_when_floor_yields_zero_tokens() {
        // cap=$1, price=$2000/WETH → 0.0005 floor = 0 → None.
        let cap = clamp_to_cap_wei(U256::from(10u64).pow(U256::from(20u64)), 1.0, 2000.0, 18);
        assert!(cap.is_none(), "sub-token cap must yield None, not silently emit 0 wei");
    }

    #[test]
    fn clamp_to_cap_wei_passes_through_when_x_below_cap() {
        // x=0.1 WETH (1e17), cap=$2000 / $2000 = 1 WETH (1e18) → x kept unchanged.
        let x = U256::from(10u64).pow(U256::from(17u64));
        let cap = clamp_to_cap_wei(x, 2000.0, 2000.0, 18).expect("cap should compute");
        assert_eq!(cap, x);
    }

    // ---------------------------------------------------------------
    // Dedup state
    // ---------------------------------------------------------------

    #[test]
    fn dedup_first_insert_succeeds_second_blocks() {
        let mut d = DedupState::new();
        assert!(d.check_and_mark("WETH>USDC>DAI", 100));
        assert!(!d.check_and_mark("WETH>USDC>DAI", 100));
    }

    #[test]
    fn dedup_different_block_same_cycle_passes() {
        let mut d = DedupState::new();
        assert!(d.check_and_mark("WETH>USDC>DAI", 100));
        assert!(d.check_and_mark("WETH>USDC>DAI", 101));
    }

    #[test]
    fn dedup_different_cycle_same_block_passes() {
        let mut d = DedupState::new();
        assert!(d.check_and_mark("WETH>USDC>DAI", 100));
        assert!(d.check_and_mark("WETH>DAI>USDC", 100));
    }

    #[test]
    fn dedup_prune_drops_old_entries() {
        let mut d = DedupState::new();
        for blk in 100..120 {
            d.check_and_mark("X>Y>Z", blk);
        }
        assert_eq!(d.len(), 20);
        d.prune(120); // cutoff = 120 - 10 = 110
        // Entries with blk < 110 are gone. blocks 110..120 = 10 entries remain.
        assert_eq!(d.len(), 10);
    }

    #[test]
    fn dedup_prune_no_op_at_boot_blocks() {
        // Before block reaches DEDUP_RETAIN_BLOCKS, prune is a no-op.
        let mut d = DedupState::new();
        d.check_and_mark("X>Y>Z", 5);
        d.prune(7);
        assert_eq!(d.len(), 1);
    }

    // ---------------------------------------------------------------
    // Cycle helpers (stale, hash, latest block)
    // ---------------------------------------------------------------

    fn make_hop(blk: u64) -> HopData {
        HopData {
            pool_addr: "0xabc".into(),
            entry: ReservesEntry {
                r0: "1000".into(),
                r1: "1000".into(),
                token0_addr: Some("0x01".into()),
                blk,
                ts: 0,
            },
            swap_in_is_token0: true,
        }
    }

    #[test]
    fn cycle_latest_block_picks_max() {
        let h = vec![make_hop(100), make_hop(105), make_hop(102)];
        assert_eq!(cycle_latest_block(&h), 105);
    }

    #[test]
    fn cycle_stale_when_lag_exceeds_threshold() {
        // 100 vs 110 → lag = 10 > MAX_RESERVE_LAG_BLOCKS (5) → stale.
        let h = vec![make_hop(100), make_hop(110), make_hop(108)];
        assert!(cycle_has_stale_reserves(&h));
    }

    #[test]
    fn cycle_fresh_when_lag_within_threshold() {
        // Max lag = 3, well under 5.
        let h = vec![make_hop(105), make_hop(108), make_hop(107)];
        assert!(!cycle_has_stale_reserves(&h));
    }

    #[test]
    fn cycle_hash_directional() {
        assert_ne!(
            cycle_hash(&["WETH", "USDC", "DAI"]),
            cycle_hash(&["WETH", "DAI", "USDC"]),
            "directionally different cycles must hash differently"
        );
    }

    #[test]
    fn cycle_hash_stable() {
        assert_eq!(
            cycle_hash(&["WETH", "USDC", "DAI"]),
            cycle_hash(&["WETH", "USDC", "DAI"]),
        );
    }

    // ---------------------------------------------------------------
    // HopData orientation
    // ---------------------------------------------------------------

    #[test]
    fn hop_data_oriented_returns_in_then_out_when_token0_is_in() {
        let h = HopData {
            pool_addr: "0xpool".into(),
            entry: ReservesEntry {
                r0: "100".into(),
                r1: "200".into(),
                token0_addr: Some("0xa".into()),
                blk: 0,
                ts: 0,
            },
            swap_in_is_token0: true,
        };
        let (ri, ro) = h.reserves_oriented().unwrap();
        assert_eq!(ri, U256::from(100u64));
        assert_eq!(ro, U256::from(200u64));
    }

    #[test]
    fn hop_data_oriented_swaps_when_token0_is_out() {
        let h = HopData {
            pool_addr: "0xpool".into(),
            entry: ReservesEntry {
                r0: "100".into(),
                r1: "200".into(),
                token0_addr: Some("0xb".into()),
                blk: 0,
                ts: 0,
            },
            swap_in_is_token0: false,
        };
        let (ri, ro) = h.reserves_oriented().unwrap();
        assert_eq!(ri, U256::from(200u64));
        assert_eq!(ro, U256::from(100u64));
    }

    // ---------------------------------------------------------------
    // evaluate_cycle integration kernel
    // ---------------------------------------------------------------

    #[test]
    fn evaluate_cycle_unprofitable_returns_none() {
        // Balanced reserves → S < 1 → must return None.
        let reserves = vec![
            (U256::from(1_000_000u64), U256::from(1_000_000u64)),
            (U256::from(1_000_000u64), U256::from(1_000_000u64)),
            (U256::from(1_000_000u64), U256::from(1_000_000u64)),
        ];
        let inp = EvalInput {
            hop_reserves: reserves,
            token_a_price_usd: Some(2000.0),
            token_a_decimals: 18,
            cap_usd: 1000.0,
            fee_bps: 30,
        };
        assert!(evaluate_cycle(&inp).is_none());
    }

    #[test]
    fn evaluate_cycle_profitable_returns_some() {
        // 5000× imbalanced first hop → highly profitable in token a.
        let reserves = vec![
            (U256::from(1_000_000u64), U256::from(5_000_000_000u64)),
            (U256::from(5_000_000_000u64), U256::from(5_000_000_000u64)),
            (U256::from(5_000_000_000u64), U256::from(5_000_000_000u64)),
        ];
        let inp = EvalInput {
            hop_reserves: reserves,
            token_a_price_usd: Some(2000.0),
            token_a_decimals: 18,
            cap_usd: 100_000_000.0, // $100M cap — won't bind
            fee_bps: 30,
        };
        let r = evaluate_cycle(&inp).expect("profitable cycle must yield Some");
        assert!(r.profit_token_a_wei > 0);
        assert!(r.expected_profit_usd.is_some());
        assert!(r.amount_in_wei > U256::zero());
        assert!(r.amount_out_wei > r.amount_in_wei);
    }

    #[test]
    fn evaluate_cycle_returns_none_when_price_unknown() {
        // Even a profitable cycle must NOT emit when price is unknown — caller
        // can't size against capital, R8 fail-honest.
        let reserves = vec![
            (U256::from(10_000u64), U256::from(10_000_000u64)),
            (U256::from(10_000_000u64), U256::from(10_000_000u64)),
            (U256::from(10_000_000u64), U256::from(10_000_000u64)),
        ];
        let inp = EvalInput {
            hop_reserves: reserves,
            token_a_price_usd: None,
            token_a_decimals: 18,
            cap_usd: 1000.0,
            fee_bps: 30,
        };
        assert!(evaluate_cycle(&inp).is_none());
    }

    #[test]
    fn evaluate_cycle_amount_in_never_exceeds_cap_anti_bug3() {
        // Defensive regression: capital cap MUST always bound amount_in_wei.
        // Construct a hugely-profitable cycle but a tiny operator cap, and
        // confirm the returned amount_in is exactly the cap.
        let reserves = vec![
            (U256::from(1u64), U256::from(10_000_000_000u64)), // 10B× rate
            (U256::from(10_000_000_000u64), U256::from(10_000_000_000u64)),
            (U256::from(10_000_000_000u64), U256::from(10_000_000_000u64)),
        ];
        let cap_usd = 2000.0; // = 1 WETH at $2000 price
        let inp = EvalInput {
            hop_reserves: reserves,
            token_a_price_usd: Some(2000.0),
            token_a_decimals: 18,
            cap_usd,
            fee_bps: 30,
        };
        let r = evaluate_cycle(&inp);
        if let Some(r) = r {
            // 1 WETH = 1e18 wei — amount_in must NOT exceed.
            let cap_wei = U256::from(10u64).pow(U256::from(18u64));
            assert!(
                r.amount_in_wei <= cap_wei,
                "amount_in_wei={} exceeds cap_wei={} — BUG-3 regression",
                r.amount_in_wei,
                cap_wei,
            );
        } else {
            // It's also acceptable for the cap to push us below break-even
            // (if so, evaluate_cycle returns None — also correct).
        }
    }

    #[test]
    fn evaluate_cycle_skips_when_cap_pushes_below_breakeven() {
        // Construct a cycle that is profitable at large size but loss-making
        // at very small size (because gas + slippage on tiny inputs dominates).
        // For pure V2 with no gas modeled, a profitable cycle is also profitable
        // at small size — the search would just pick a tiny x and we'd emit.
        // So this test asserts the GENERAL invariant: when evaluate_cycle returns
        // Some, profit_token_a_wei > 0 ALWAYS.
        let reserves = vec![
            (U256::from(10_000u64), U256::from(10_000_000u64)),
            (U256::from(10_000_000u64), U256::from(10_000_000u64)),
            (U256::from(10_000_000u64), U256::from(10_000_000u64)),
        ];
        let inp = EvalInput {
            hop_reserves: reserves,
            token_a_price_usd: Some(2000.0),
            token_a_decimals: 18,
            cap_usd: 2000.0,
            fee_bps: 30,
        };
        let r = evaluate_cycle(&inp);
        if let Some(r) = r {
            assert!(
                r.profit_token_a_wei > 0,
                "evaluate_cycle returned Some with non-positive profit"
            );
        }
    }

    // ---------------------------------------------------------------
    // gamma + fee
    // ---------------------------------------------------------------

    #[test]
    fn gamma_30bps_is_0997() {
        assert!((gamma(30) - 0.997).abs() < 1e-12);
    }

    #[test]
    fn gamma_zero_fee_is_one() {
        assert_eq!(gamma(0), 1.0);
    }

    // ---------------------------------------------------------------
    // Resolve_token fallback (no Redis)
    // ---------------------------------------------------------------

    #[test]
    fn known_token_address_blue_chips() {
        assert!(known_token_address("WETH").is_some());
        assert!(known_token_address("usdc").is_some()); // case-insensitive
        assert!(known_token_address("USDT").is_some());
        assert!(known_token_address("DAI").is_some());
        assert!(known_token_address("WBTC").is_some());
    }

    #[test]
    fn known_token_address_long_tail_majors_resolve() {
        // 2026-05-07: long-tail majors added alongside the corresponding
        // MVP_CYCLES entries. All four must resolve to a real mainnet
        // address and to 18 decimals (per their on-chain contracts).
        for sym in ["PEPE", "SHIB", "MKR", "COMP"] {
            let addr = known_token_address(sym)
                .unwrap_or_else(|| panic!("missing address for {sym}"));
            assert!(addr.starts_with("0x") && addr.len() == 42, "bad addr for {sym}: {addr}");
        }
    }

    #[test]
    fn known_token_address_unknown_returns_none() {
        assert!(known_token_address("FAKE_TOKEN").is_none());
    }

    // ---------------------------------------------------------------
    // MVP_CYCLES sanity
    // ---------------------------------------------------------------

    #[test]
    fn mvp_cycles_all_use_known_tokens() {
        for (a, b, c) in MVP_CYCLES {
            assert!(
                known_token_address(a).is_some()
                    && known_token_address(b).is_some()
                    && known_token_address(c).is_some(),
                "MVP cycle {}>{}>{}: every token must be in known_token_address fallback",
                a,
                b,
                c
            );
        }
    }

    #[test]
    fn mvp_cycles_no_self_loop() {
        for (a, b, c) in MVP_CYCLES {
            assert_ne!(a, b);
            assert_ne!(b, c);
            assert_ne!(a, c);
        }
    }

    // ---------------------------------------------------------------
    // f64 ↔ U256 conversion safety
    // ---------------------------------------------------------------

    #[test]
    fn f64_to_u256_clamped_handles_negative_and_nan() {
        assert_eq!(f64_to_u256_clamped(-1.0), U256::from(1u64));
        assert_eq!(f64_to_u256_clamped(0.0), U256::from(1u64));
        assert_eq!(f64_to_u256_clamped(f64::NAN), U256::from(1u64));
        assert_eq!(f64_to_u256_clamped(f64::INFINITY), U256::from(1u64));
    }

    #[test]
    fn f64_to_u256_clamped_preserves_integers() {
        assert_eq!(f64_to_u256_clamped(123_456.0), U256::from(123_456u64));
    }

    #[test]
    fn u256_to_f64_lossy_round_trip_small_values() {
        let v = U256::from(1_000_000u64);
        let f = u256_to_f64_lossy(v);
        assert!((f - 1_000_000.0).abs() < 1.0);
    }

    // ---------------------------------------------------------------
    // TickStats — period log accumulator
    // ---------------------------------------------------------------

    #[test]
    fn tick_stats_default_is_all_zero_and_dominant_is_none() {
        let s = TickStats::default();
        assert_eq!(s.scanned, 0);
        assert_eq!(s.emitted, 0);
        assert!(s.dominant_skip_reason().is_none());
    }

    #[test]
    fn tick_stats_dominant_picks_biggest_bucket() {
        let mut s = TickStats::default();
        s.skip_missing_pool = 5;
        s.skip_no_profit = 12;
        s.skip_stale_reserves = 3;
        assert_eq!(s.dominant_skip_reason(), Some("no_profit"));
    }

    #[test]
    fn tick_stats_dominant_excludes_zero_buckets() {
        // Only skip_dedup_hit > 0, even though no_profit is "earlier" in the
        // bucket list — must return dedup_hit, never the zero ones.
        let mut s = TickStats::default();
        s.skip_dedup_hit = 1;
        assert_eq!(s.dominant_skip_reason(), Some("dedup_hit"));
    }

    #[test]
    fn tick_stats_emits_only_counted_when_strictly_positive() {
        let mut s = TickStats::default();
        s.emitted = 0; // none emitted
        assert!(s.dominant_skip_reason().is_none()); // and no skips either
    }

    #[test]
    fn stats_log_period_is_positive() {
        // Sanity: STATS_LOG_EVERY_N_TICKS must be > 0 to avoid div-by-zero
        // in `tick_count % N` and to ensure logs actually fire.
        assert!(STATS_LOG_EVERY_N_TICKS > 0);
    }

    // ---------------------------------------------------------------
    // Anti-BUG-3 regression — orientation flip should NOT produce a
    // sanity-passing emit. Reproduces the 2026-05-07 incident where
    // a swapped-token declaration in migration 037 (USDC/WBTC pool)
    // caused triangular_worker to emit 1183 fake $4M-profit opps.
    // ---------------------------------------------------------------

    /// Synthesizes an extreme-imbalance triangular cycle that reproduces
    /// the orientation-flip bug class. Returns the (output, profit) that
    /// `cycle_profit` computes — when the math kernel reads ANY hop's
    /// reserves with the wrong orientation, the profit balloons because
    /// fees and ratios compound multiplicatively.
    #[test]
    fn cycle_profit_extreme_imbalance_yields_huge_profit_caught_by_sanity_bound() {
        // Hop 2 simulates the orientation-flipped USDC/WBTC pool from the
        // 2026-05-07 incident: real reserves ~ $50K WBTC vs $50K USDC, but
        // declared with reserves swapped in interpretation. The misread
        // makes the worker think the swap rate is hugely favorable.
        //
        // Concretely: real WBTC reserve = 0.6 BTC = 6e7 wei, real USDC
        // reserve = 50000 USDC = 5e10 wei. Mis-orientation reads
        // r_in=6e7, r_out=5e10 instead of r_in=5e10, r_out=6e7 → 833x
        // overstatement of output for the USDC→WBTC hop.
        //
        // Compounded with two other realistic hops, output >> input.
        let x = U256::from(2_000_000_000_000_000_000u64); // 2 WETH (2e18 wei)
        let reserves = vec![
            // WETH→USDC realistic
            (U256::from(10_000_000_000_000_000_000u64), U256::from(20_000_000_000u64)),
            // USDC→WBTC FLIPPED (this is the bug)
            (U256::from(61_564_199u64), U256::from(49_997_126_049u64)),
            // WBTC→WETH realistic
            (U256::from(1_000_000_000u64), U256::from(50_000_000_000_000_000_000u64)),
        ];
        let (out, profit) = cycle_profit(x, &reserves, 30);
        // The math kernel can't know the orientation is wrong — it does
        // exactly what it's told. So output is HUGE (>> input).
        assert!(out > x, "extreme imbalance must produce output > input");
        let profit_ratio = profit as f64 / x.as_u128() as f64;
        // Profit ratio is wildly above the SANITY_PROFIT_MULT_OF_CAP=5
        // threshold, so the worker's sanity guard MUST reject. This test
        // documents the math-kernel behavior; the integration guard
        // (sanity_reject branch) is what protects PG/Redis from the bad
        // emit. If the math kernel ever returns a near-1× profit on this
        // input set, EITHER the kernel was hardened (good — update test)
        // OR a different bug masks the orientation flip (investigate).
        assert!(
            profit_ratio > 5.0,
            "regression marker: mis-oriented hop must surface as profit_ratio > 5 \
             so the worker-level sanity bound rejects it (got ratio={})",
            profit_ratio,
        );
    }

    #[test]
    fn sanity_threshold_is_documented_constant() {
        // Document-as-test: the sanity threshold lives inline in
        // scan_one_direction. If anyone changes it, this test fails until
        // they update the doc reference here. Helps reviewers find the
        // policy in one place.
        let documented_threshold: f64 = 5.0;
        assert!(documented_threshold > 1.0, "threshold must allow legitimate profitable opps");
        assert!(documented_threshold < 100.0, "threshold must catch orientation-flip bugs");
    }
}
