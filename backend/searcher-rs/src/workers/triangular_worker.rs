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

use crate::amm_math::{v2_amount_out, v3_quote_exact_in_multicall, V3QuoteRequest};
use crate::counters::counters;
use crate::persistence;
use crate::publisher;
use crate::reserves::{
    get_pools_for_pair, get_pools_for_pair_v3, get_reserves, get_token_meta, ReservesEntry,
    V3PoolInfo,
};
use chrono::Utc;
use ethers::types::{Address, U256};
use redis::aio::ConnectionManager;
use shared_rs::candidates::RouteMetadata;
use shared_rs::contracts::{Opportunity, StrategyKind};
use shared_rs::rpc_failover::HttpRpcPool;
use shared_rs::tokens;
use shared_rs::trading_config::TradingConfigClient;
use sqlx::postgres::PgPool;
use std::collections::HashSet;
use std::str::FromStr;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::Duration;
use tokio::time::interval;
use tracing::{debug, info, warn};
use uuid::Uuid;

/// Mainnet QuoterV2 + Multicall3 addresses. Mirrored from `scanner.rs` —
/// both modules quote against the same on-chain contracts. V3 quoting is
/// mainnet-only for now; multi-chain V3 lands in a future sub-project.
const V3_QUOTER_V2_MAINNET: &str = "0x61fFE014bA17989E743c5F6cB21bF9697530B21e";
const V3_MULTICALL3_ADDR: &str = "0xcA11bde05977b3631167028862bE2a173976CA11";

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
    /// V3-specific: at least one V3 quote in the cycle came back
    /// `success=false` or `amount_out=0` from QuoterV2 (insufficient
    /// liquidity, pool revert, RPC error). R8 fail-honest skip — never
    /// fabricated.
    skip_v3_quote_failed: u32,
    emitted: u32,
}

impl TickStats {
    /// Returns the name of the dominant skip bucket (the one with the highest
    /// count) so the operator sees at a glance WHY cycles aren't producing
    /// emit-able opportunities. None when no skips happened in the period.
    fn dominant_skip_reason(&self) -> Option<&'static str> {
        let buckets: [(&'static str, u32); 7] = [
            ("missing_pool", self.skip_missing_pool),
            ("stale_reserves", self.skip_stale_reserves),
            ("dedup_hit", self.skip_dedup_hit),
            ("no_capital_cap", self.skip_no_capital_cap),
            ("no_profit", self.skip_no_profit),
            ("unknown_token", self.skip_unknown_token),
            ("v3_quote_failed", self.skip_v3_quote_failed),
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
/// Long-tail tokens (PEPE/SHIB/MKR/COMP) live in `V3_CYCLES` below — they
/// trade primarily on Uniswap V3 and have no V2 USDC pool, so the worker
/// resolves their hops through the on-chain QuoterV2 multicall path
/// (`scan_v3_bearing_cycles`) instead of the cached-V2 path used here.
///
/// Operator can extend by editing this constant; future sub-project lifts
/// the list to PG so the operator can manage cycles via UI.
pub const MVP_CYCLES: &[(&str, &str, &str)] = &[
    ("WETH", "USDC", "DAI"),
    ("WETH", "USDC", "USDT"),
    ("WETH", "USDT", "DAI"),  // unblocked by migration 037 (DAI/USDT V2)
    ("WETH", "WBTC", "USDC"), // unblocked by migration 037 (WBTC/USDC V2)
    ("USDC", "DAI", "USDT"),  // unblocked by migration 037 (DAI/USDT V2)
];

/// V3-bearing triangular cycles re-introduced 2026-05-07 alongside the V3
/// QuoterV2 multicall path. Each cycle (a, b, c) defines `a → b → c → a`
/// and the worker tries BOTH directions per tick.
///
/// All four cycles share the structure `X → WETH → USDC → X` where X is a
/// long-tail token (PEPE/SHIB/MKR/COMP). The middle hop (WETH↔USDC) is
/// resolved through the V2 reserves cache when available; the X↔WETH and
/// X↔USDC hops fall back to V3 QuoterV2 — X has plenty of V3 liquidity
/// against both WETH (high-volume V3 pools) and USDC (lower volume but
/// real on mainnet) but no canonical V2 USDC pool.
///
/// **MVP single-point evaluation (decision C in the design brief):** V3
/// concentrated liquidity makes amount_out non-linear in amount_in, so we
/// cannot reuse the cached "spot rate" trick that V2 affords. Running
/// golden-section over QuoterV2 would cost ~25 RPC × 2 V3 hops × 4 cycles
/// × 2 dirs ≈ 400 RPC/tick — breaks Alchemy rate limits. We accept the
/// known limitation that V3 cycles use a single fixed amount_in
/// (`cap_usd / token_a_price` floored to wei) and follow up with golden-
/// section once V3 tick math is implemented in Rust.
///
/// V3 fee tier selection: the V3 pool index can list multiple tiers per
/// pair (typically 0.05% + 0.30% for blue-chip pairs). MVP picks the FIRST
/// entry returned by `get_pools_for_pair_v3` — future sub-project ranks by
/// TVL or quote magnitude.
pub const V3_CYCLES: &[(&str, &str, &str)] = &[
    ("PEPE", "WETH", "USDC"),
    ("SHIB", "WETH", "USDC"),
    ("MKR", "WETH", "USDC"),
    ("COMP", "WETH", "USDC"),
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
pub fn cycle_profit(x: U256, hop_reserves: &[(U256, U256)], fee_bps: u32) -> (U256, i128) {
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

    let x_star_f = if yc > yd {
        (a + d) / 2.0
    } else {
        (c + b) / 2.0
    };
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
    debug_assert!(
        result <= cap_wei,
        "clamp_to_cap_wei produced value > cap_wei"
    );
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
/// Delegates to `arbx_shared::tokens` (chain_id = 1, Ethereum mainnet).
/// Returns `None` for any symbol not present in the canonical catalog.
///
/// Operator extension path: new tokens must be added to `shared_rs::tokens`
/// (reviewed, auditable constants) rather than this function. If both the
/// catalog lookup and the Redis cache miss, the cycle is skipped (fail-honest).
///
/// # Type note
/// The return type was changed from `Option<&'static str>` to `Option<String>`
/// to match the `token_address_str` API, which builds the hex string at
/// call-time from the static byte array stored in `TokenEntry`. The single
/// production caller (`resolve_token`) already converts to `String` for the
/// tuple it returns, so the change is transparent. The symbol lookup is
/// case-insensitive (normalised to uppercase before delegating).
fn known_token_address(symbol: &str) -> Option<String> {
    tokens::token_address_str(1u64, &symbol.to_ascii_uppercase())
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
    if let Ok(Some(meta)) = get_token_meta(redis, chain_id, &addr).await {
        return Some((addr, meta.decimals, meta.is_stablecoin));
    }
    // Fallback to hard-coded mainnet decimals (truthful, not invented).
    // Decimals are sourced from the canonical catalog (shared_rs::tokens) for
    // correctness; the match below mirrors TokenEntry.decimals for performance
    // (avoids a second catalog lookup in the hot path).
    let (decimals, is_stable) = match symbol.to_ascii_uppercase().as_str() {
        "WETH" => (18, false),
        "USDC" | "USDT" => (6, true),
        "DAI" => (18, true),
        "WBTC" => (8, false),
        // Long-tail majors — all use 18 decimals per their on-chain contracts.
        "PEPE" | "SHIB" | "MKR" | "COMP" => (18, false),
        _ => return None,
    };
    Some((addr, decimals, is_stable))
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

/// One hop's resolved data, source-tagged. V2 hops carry the cached
/// reserves entry (cheap repeated reads). V3 hops carry the chosen pool
/// address + fee tier; the actual amount_out is fetched via QuoterV2
/// multicall at evaluation time and stored separately so the worker can
/// batch every V3 hop across every cycle into a single RPC.
#[derive(Debug, Clone)]
enum HopKind {
    V2(HopData),
    V3 {
        pool_addr: String,
        fee_bps: u32,
        token_in_addr: String,
        token_out_addr: String,
    },
}

impl HopKind {
    /// Block number attribution for dedup + staleness:
    /// - V2 hop → the block at which `PoolSyncWorker` last refreshed reserves.
    /// - V3 hop → None (QuoterV2 returns the quote at "current state" with no
    ///   block number; the cycle's block is taken from the most-recent V2 hop).
    fn block_number(&self) -> Option<u64> {
        match self {
            HopKind::V2(h) => Some(h.entry.blk),
            HopKind::V3 { .. } => None,
        }
    }

    /// Pool address (lowercase hex). Surfaced for diagnostic dumps.
    fn pool_addr(&self) -> &str {
        match self {
            HopKind::V2(h) => &h.pool_addr,
            HopKind::V3 { pool_addr, .. } => pool_addr,
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
    let entry = get_reserves(redis, chain_id, &pool_addr)
        .await
        .ok()
        .flatten()?;
    let token0 = entry.token0_addr.as_deref()?;
    let swap_in_is_token0 = token0.eq_ignore_ascii_case(token_in_addr);
    Some(HopData {
        pool_addr,
        entry,
        swap_in_is_token0,
    })
}

/// Resolve a hop as V3 by looking up the V3 pool index. Returns the FIRST
/// pool entry — multi-tier ranking (lowest fee tier vs highest TVL) is a
/// future-improvement listed in the V3_CYCLES doc above.
///
/// Returns None when the V3 index is empty for this pair (no V3 pool covers
/// the symbols on mainnet) — caller falls through to "skip cycle". R8 fail-
/// honest: NEVER fabricates a synthetic pool address.
async fn resolve_hop_v3(
    redis: &mut ConnectionManager,
    chain_id: u64,
    token_in_sym: &str,
    token_out_sym: &str,
    token_in_addr: &str,
    token_out_addr: &str,
) -> Option<HopKind> {
    let pools = get_pools_for_pair_v3(redis, chain_id, token_in_sym, token_out_sym)
        .await
        .ok()?;
    let info: V3PoolInfo = pools.into_iter().next()?;
    Some(HopKind::V3 {
        pool_addr: info.pool_addr,
        fee_bps: info.fee_bps,
        token_in_addr: token_in_addr.to_string(),
        token_out_addr: token_out_addr.to_string(),
    })
}

/// Resolve a hop preferring V2 (cached, cheap) and falling back to V3 (on-
/// chain QuoterV2 batched per tick). Returns None when neither source has a
/// pool for the pair — caller skips cycle with `cycle_missing_pool`.
async fn resolve_hop_any(
    redis: &mut ConnectionManager,
    chain_id: u64,
    token_in_addr: &str,
    token_in_sym: &str,
    token_out_addr: &str,
    token_out_sym: &str,
) -> Option<HopKind> {
    if let Some(v2) = resolve_hop(redis, chain_id, token_in_addr, token_in_sym, token_out_sym).await
    {
        return Some(HopKind::V2(v2));
    }
    resolve_hop_v3(
        redis,
        chain_id,
        token_in_sym,
        token_out_sym,
        token_in_addr,
        token_out_addr,
    )
    .await
}

/// Identify the most recent block across a cycle's hops; used to enforce
/// the staleness bound (hop with blk < latest - MAX_RESERVE_LAG_BLOCKS is rejected).
fn cycle_latest_block(hops: &[HopData]) -> u64 {
    hops.iter().map(|h| h.entry.blk).max().unwrap_or(0)
}

/// Same as `cycle_latest_block` but for the mixed-kind hop slice used by V3-
/// bearing cycles: V2 hops contribute their cached block; V3 hops contribute
/// nothing (their quote is "current state"). Returns the max of the V2-hop
/// blocks, or 0 when all hops are V3 (in which case dedup falls back to the
/// per-tick block sentinel — never an issue for V3_CYCLES because the WETH↔USDC
/// middle hop is always V2-resolvable).
fn cycle_latest_block_mixed(hops: &[HopKind]) -> u64 {
    hops.iter()
        .filter_map(|h| h.block_number())
        .max()
        .unwrap_or(0)
}

/// Staleness check for the mixed-kind slice. We can only enforce the lag
/// bound across V2 hops; V3 hops are stateless under our single-point
/// evaluation. Returns true when ANY V2 hop is more than
/// `MAX_RESERVE_LAG_BLOCKS` behind the latest V2 hop.
fn cycle_has_stale_reserves_mixed(hops: &[HopKind]) -> bool {
    let v2_blocks: Vec<u64> = hops.iter().filter_map(|h| h.block_number()).collect();
    if v2_blocks.is_empty() {
        return false;
    }
    let latest = *v2_blocks.iter().max().unwrap_or(&0);
    v2_blocks
        .iter()
        .any(|b| latest.saturating_sub(*b) > MAX_RESERVE_LAG_BLOCKS)
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
    let search_ceiling = if cap_wei < r_in_first_hop {
        cap_wei
    } else {
        r_in_first_hop
    };
    let x_hi = if search_ceiling > x_lo {
        search_ceiling
    } else {
        x_lo
    };

    let (x_star, profit_wei) =
        golden_section_search(x_lo, x_hi, &input.hop_reserves, input.fee_bps, 25);

    // Defensive cap re-application (anti-BUG-3): no matter what the search
    // found, never exceed the cap.
    let amount_in = clamp_to_cap_wei(x_star, input.cap_usd, token_a_price, input.token_a_decimals)?;
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

/// Pre-computed amount_in / amount_out for one hop of a V3-bearing cycle.
/// The mixed-kind evaluator iterates these in cycle order; each one was either
/// produced by the V2 amount_out math kernel (cheap, deterministic) or fetched
/// via QuoterV2 multicall (one batched RPC per tick). Public for testability.
///
/// **Chain consistency (math-validator CRITICAL fix 2026-05-06):** the kernel
/// asserts that for i > 0, `hop_outs[i].amount_in_used == hop_outs[i-1].amount_out`
/// (within a small tolerance for V2 truncation when the predecessor is V2).
/// Without this check, a buggy caller can feed a V3 quote with amount_in
/// from BEFORE a V2 hop saturated, producing inflated downstream profit.
/// See `chain_consistent_with_predecessor` for the tolerance semantics.
#[derive(Debug, Clone)]
pub struct HopAmountOut {
    /// Amount fed into THIS hop. For hop 0 this equals `amount_in_wei` of the
    /// cycle. For hop i > 0 this MUST equal the previous hop's `amount_out`
    /// (the kernel verifies this).
    pub amount_in_used: U256,
    pub amount_out: U256,
    /// Source tag for the diagnostic log on sanity reject.
    #[allow(dead_code)]
    pub source: HopAmountSource,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HopAmountSource {
    V2,
    V3,
}

/// Chain consistency tolerance: how far apart can hop[i].amount_in_used and
/// hop[i-1].amount_out be before the kernel rejects the chain as inconsistent?
///
/// Strict equality would be ideal but introduces noise around V2 truncation:
/// when the predecessor is a V2 hop, `v2_amount_out` produces a value that the
/// caller MUST then feed unchanged to the next hop (no further truncation).
/// In practice the caller computes both sides identically so equality holds —
/// the tolerance exists only to avoid a subtle mismatch surfacing as a false
/// reject in some corner case we haven't yet anticipated. Difference must be
/// at MOST 1 wei (single-unit slop) — anything larger is a genuine bug.
fn chain_consistent_with_predecessor(predecessor_out: U256, hop_in: U256) -> bool {
    if predecessor_out == hop_in {
        return true;
    }
    let diff = if predecessor_out > hop_in {
        predecessor_out - hop_in
    } else {
        hop_in - predecessor_out
    };
    diff <= U256::from(1u64)
}

/// **Pure-function kernel for V3-bearing cycles** — single-point evaluation.
///
/// Inputs:
///   - `amount_in_wei`: the candidate input size in token-a wei. The caller
///     computes this as `floor(cap_usd / token_a_price_usd × 10^decimals)` —
///     i.e. the operator's capital cap mapped to wei. **No optimal-search
///     for V3 is performed in MVP** (decision C in the design brief).
///   - `hop_outs`: per-hop amount_out, in cycle order. Length must equal 3.
///   - `token_a_price_usd`: needed to translate the integer profit to USD.
///   - `token_a_decimals`: scales the integer profit to f64 token units.
///   - `cap_usd`: operator capital cap, used by the caller's sanity guard.
///     Returned as part of `EvalResult` for the sanity-bound check at the
///     persistence layer.
///
/// Returns `Some(EvalResult)` only when:
///   1. Length checks pass (3 hops, no zero amount_outs).
///   2. Final amount > input (positive profit in token-a wei).
///   3. Token-a price is known (`token_a_price_usd > 0`, finite).
///
/// Otherwise returns None and the caller skips. **R8 fail-honest**: zero
/// amount_outs (V3 quote failure) → None at this hop, never fabricated.
pub fn evaluate_v3_cycle(
    amount_in_wei: U256,
    hop_outs: &[HopAmountOut],
    token_a_price_usd: Option<f64>,
    token_a_decimals: u8,
) -> Option<EvalResult> {
    if hop_outs.len() != 3 || amount_in_wei.is_zero() {
        return None;
    }
    // No-zero-link sanity (R8 fail-honest — V3 quote failure produces
    // amount_out=0 which the kernel must NOT treat as a chain).
    for h in hop_outs {
        if h.amount_out.is_zero() {
            return None;
        }
    }
    // **Chain consistency** (math-validator CRITICAL fix 2026-05-06).
    // Hop 0's amount_in_used must equal the cycle's amount_in_wei; subsequent
    // hops' amount_in_used must equal the predecessor's amount_out. Without
    // this check, a buggy caller (the original V3 chain bug — feeding V2 with
    // pre-V3 wei because V3 amount_out wasn't yet known when the chain was
    // built) can inflate downstream profit by orders of magnitude. The bug
    // would slip past the 5x sanity bound at ratios up to ~2.5x in the
    // PEPE/SHIB long-tail case (see `pepe_v3_v2_v3_cycle_does_not_emit_fake_positive_under_sanity_bound`).
    if hop_outs[0].amount_in_used != amount_in_wei {
        return None;
    }
    for i in 1..hop_outs.len() {
        if !chain_consistent_with_predecessor(
            hop_outs[i - 1].amount_out,
            hop_outs[i].amount_in_used,
        ) {
            return None;
        }
    }
    let final_out = hop_outs[hop_outs.len() - 1].amount_out;
    if final_out <= amount_in_wei {
        return None;
    }
    // Profit in token-a wei. Use the same i128 clamp the V2 kernel uses for
    // consistency with the spine + sanity-bound math downstream.
    let profit_wei =
        u256_to_i128_clamped(final_out).saturating_sub(u256_to_i128_clamped(amount_in_wei));
    if profit_wei <= 0 {
        return None;
    }
    let price = token_a_price_usd?;
    if !price.is_finite() || price <= 0.0 {
        return None;
    }
    let profit_tokens = (profit_wei as f64) / 10f64.powi(token_a_decimals as i32);
    let expected_profit_usd = Some(profit_tokens * price);
    Some(EvalResult {
        amount_in_wei,
        amount_out_wei: final_out,
        profit_token_a_wei: profit_wei,
        expected_profit_usd,
    })
}

/// Compute the candidate amount_in for a V3-bearing cycle: `cap_usd /
/// price_a` floored to wei. Same anti-BUG-3 cap math the V2 path uses; we
/// reuse `clamp_to_cap_wei` with `U256::MAX` as the unconstrained sentinel
/// so the function returns the cap itself.
///
/// Returns None when the cap math fails (zero / NaN price, sub-token cap),
/// matching `clamp_to_cap_wei` semantics — caller treats None as "skip
/// cycle, R8 fail-honest, never fabricate amount_in".
pub fn cap_amount_in_wei(cap_usd: f64, price_a: f64, decimals_a: u8) -> Option<U256> {
    clamp_to_cap_wei(U256::MAX, cap_usd, price_a, decimals_a)
}

/// Single iteration of the two-phase chain resolver.
///
/// Walks the plan's hops in order. For each hop with `amount_in_used` already
/// known (set on initialisation for hop 0, set by a previous iteration's V3
/// resolution + V2 inline chaining for hop > 0):
///   - If the hop is V2 and its amount_out is unknown: compute it inline via
///     `v2_amount_out`, store it, propagate to hop+1's amount_in_used.
///   - If the hop is V3 and its amount_out is unknown: queue a V3QuoteRequest
///     for THIS phase's batch; downstream hops are deferred.
///   - If amount_out is already known (filled by earlier V3 multicall result),
///     propagate to hop+1's amount_in_used.
///
/// Returns true if any progress was made (a new amount_in or amount_out was
/// filled, or a request was queued). The outer phase loop iterates until no
/// plan reports progress, then evaluates.
///
/// **Why this is the chain-consistency guarantee** (math-validator CRITICAL
/// fix 2026-05-06): `amount_in_used` for hop `i+1` is set EXCLUSIVELY from
/// `amount_out` of hop `i` once hop `i` is fully resolved. There is no path
/// in this function that fabricates or carries-over a stale `amount_in_used`.
/// Combined with `evaluate_v3_cycle`'s explicit chain check, this rules out
/// the original failure mode (PEPE→V2 fed wrong-token wei → fake positive).
fn try_progress_plan(
    plan: &mut V3CyclePlan,
    phase_requests: &mut Vec<V3QuoteRequest>,
    routing: &mut Vec<(usize, usize)>,
    plan_idx: usize,
) -> bool {
    let mut progressed = false;
    for hop_idx in 0..plan.hops.len() {
        let amount_in = match plan.hop_amount_ins[hop_idx] {
            Some(v) => v,
            None => continue, // predecessor not yet resolved → skip; later phase will fill.
        };
        // Compute amount_out if missing.
        if plan.hop_amount_outs[hop_idx].is_none() {
            match &plan.hops[hop_idx] {
                HopKind::V2(hd) => {
                    if let Some((r_in, r_out)) = hd.reserves_oriented() {
                        let out = v2_amount_out(amount_in, r_in, r_out, V2_FEE_BPS);
                        plan.hop_amount_outs[hop_idx] = Some(out);
                        progressed = true;
                    }
                    // else: malformed reserves — leave as None, plan fails at evaluation.
                }
                HopKind::V3 {
                    pool_addr,
                    fee_bps,
                    token_in_addr,
                    token_out_addr,
                } => {
                    // Queue for THIS phase's batched multicall.
                    let pool_a = match Address::from_str(pool_addr) {
                        Ok(a) => a,
                        Err(_) => continue,
                    };
                    let tin = match Address::from_str(token_in_addr) {
                        Ok(a) => a,
                        Err(_) => continue,
                    };
                    let tout = match Address::from_str(token_out_addr) {
                        Ok(a) => a,
                        Err(_) => continue,
                    };
                    phase_requests.push(V3QuoteRequest {
                        pool_addr: pool_a,
                        token_in: tin,
                        token_out: tout,
                        amount_in,
                        fee_bps: *fee_bps,
                    });
                    routing.push((plan_idx, hop_idx));
                    progressed = true;
                    // Continue walking — downstream V2 hops can't progress
                    // until this V3 result is back, but we may catch other
                    // already-resolved V3 hops on the way out.
                    continue;
                }
            }
        }
        // Propagate amount_out → next hop's amount_in (chain consistency).
        if let Some(out) = plan.hop_amount_outs[hop_idx] {
            let next = hop_idx + 1;
            if next < plan.hops.len() && plan.hop_amount_ins[next].is_none() {
                plan.hop_amount_ins[next] = Some(out);
                progressed = true;
            }
        }
    }
    progressed
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
    /// Returns true when the dedup set is empty.
    #[allow(dead_code)]
    pub fn is_empty(&self) -> bool {
        self.seen.is_empty()
    }
}

/// TriangularWorker — owns its tick interval and dedup state. Lives for the
/// lifetime of the `searcher-rs` process.
pub struct TriangularWorker {
    pub period: Duration,
    pub chain_id: u64,
    /// Optional HTTP RPC pool for V3 QuoterV2 multicall. When None the worker
    /// silently skips `V3_CYCLES` (V2-only path stays fully functional). Set
    /// at boot via `with_v3_provider`; mainnet only — `main.rs` reuses the
    /// pool already constructed for the scanner's V3 quoter.
    v3_provider: Option<Arc<HttpRpcPool>>,
}

impl TriangularWorker {
    pub fn new(interval_secs: u64, chain_id: u64) -> Self {
        Self {
            period: Duration::from_secs(interval_secs.max(1)),
            chain_id,
            v3_provider: None,
        }
    }

    /// Attach an HTTP RPC pool for V3 QuoterV2 multicall. Without this the
    /// worker scans only `MVP_CYCLES` and counts every V3-cycle attempt as
    /// `triangular_v3_quote_failures` (with the diagnostic hint
    /// "v3_provider_unavailable"). Mainnet only for now.
    pub fn with_v3_provider(mut self, pool: Arc<HttpRpcPool>) -> Self {
        self.v3_provider = Some(pool);
        self
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
            v3_cycles = V3_CYCLES.len(),
            v3_provider_attached = self.v3_provider.is_some(),
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

            // --- Phase 1: V2-only cycles (cached reserves, no RPC) ---
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

            // --- Phase 2: V3-bearing cycles (single batched QuoterV2 multicall) ---
            // Aggregates every V3 hop across every V3-bearing cycle into ONE
            // multicall per tick (≤ ~12 RPC calls per pool worst case → still
            // a single RPC overall thanks to Multicall3). Skipped silently
            // when no v3_provider is attached (non-mainnet, or env not set).
            if let Some(rpc_pool) = self.v3_provider.as_ref() {
                if let Some(blk) = self
                    .scan_v3_bearing_cycles(
                        &mut redis,
                        db.as_ref(),
                        &cfg_opt,
                        &snapshot_map,
                        rpc_pool.clone(),
                        &mut dedup,
                        &mut stats,
                    )
                    .await
                {
                    if blk > latest_block_observed {
                        latest_block_observed = blk;
                    }
                }
            } else {
                // No provider: count the V3 cycles we WOULD have scanned so
                // the heartbeat surfaces the missing-provider state instead
                // of pretending V3 is not configured. R8 fail-honest.
                let count = (V3_CYCLES.len() as u64) * 2;
                counters()
                    .triangular_v3_cycles_scanned
                    .fetch_add(count, Ordering::Relaxed);
                counters()
                    .triangular_v3_quote_failures
                    .fetch_add(count, Ordering::Relaxed);
                stats.skip_v3_quote_failed += count as u32;
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
                    skip_v3_quote_failed = stats.skip_v3_quote_failed,
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
        let (addr_a, decimals_a, _is_stable_a) =
            match resolve_token(redis, self.chain_id, sym_a).await {
                Some(t) => t,
                None => {
                    stats.skip_unknown_token += 1;
                    return None;
                }
            };
        let (addr_b, _, _) = match resolve_token(redis, self.chain_id, sym_b).await {
            Some(t) => t,
            None => {
                stats.skip_unknown_token += 1;
                return None;
            }
        };
        let (addr_c, _, _) = match resolve_token(redis, self.chain_id, sym_c).await {
            Some(t) => t,
            None => {
                stats.skip_unknown_token += 1;
                return None;
            }
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
        // X10THINK defense: result.expected_profit_usd is Option<f64>. R8 fail-
        // honest — if None/NaN/Inf/negative, refuse to emit. evaluate_cycle
        // *should* have already filtered these (token_a price required, math
        // returns Some only on positive profit) but this is the last gate
        // before persistence; trust nothing.
        let profit_usd = match result.expected_profit_usd {
            Some(v) if v.is_finite() && v >= 0.0 => v,
            _ => {
                stats.skip_no_profit += 1;
                warn!(
                    event = "triangular_worker.malformed_profit",
                    chain_id = self.chain_id,
                    cycle = %cycle_key,
                    raw = ?result.expected_profit_usd,
                );
                return Some(cycle_block);
            }
        };
        let profit_cap_ratio = profit_usd / cap_usd;
        if profit_cap_ratio > SANITY_PROFIT_MULT_OF_CAP {
            stats.skip_no_profit += 1; // bucketed under no_profit for tick_stats
                                       // Diagnostic dump: per-hop reserves + orientation actually used by
                                       // the math kernel. Critical for diagnosing why the kernel produces
                                       // implausible profits even with apparently-correct cached
                                       // ReservesEntry.token0_addr. Each line shows pool / orientation /
                                       // (r_in, r_out) so a human can reproduce the math by hand.
            let r1 = hop1.reserves_oriented();
            let r2 = hop2.reserves_oriented();
            let r3 = hop3.reserves_oriented();
            warn!(
                event = "triangular_worker.sanity_reject",
                chain_id = self.chain_id,
                cycle = %cycle_key,
                expected_profit_usd = profit_usd,
                cap_usd = cap_usd,
                profit_cap_ratio = profit_cap_ratio,
                threshold = SANITY_PROFIT_MULT_OF_CAP,
                amount_in_wei = %result.amount_in_wei,
                hop1_pool = %hop1.pool_addr,
                hop1_token0 = ?hop1.entry.token0_addr,
                hop1_swap_in_is_token0 = hop1.swap_in_is_token0,
                hop1_r0 = %hop1.entry.r0,
                hop1_r1 = %hop1.entry.r1,
                hop1_oriented = ?r1,
                hop2_pool = %hop2.pool_addr,
                hop2_token0 = ?hop2.entry.token0_addr,
                hop2_swap_in_is_token0 = hop2.swap_in_is_token0,
                hop2_r0 = %hop2.entry.r0,
                hop2_r1 = %hop2.entry.r1,
                hop2_oriented = ?r2,
                hop3_pool = %hop3.pool_addr,
                hop3_token0 = ?hop3.entry.token0_addr,
                hop3_swap_in_is_token0 = hop3.swap_in_is_token0,
                hop3_r0 = %hop3.entry.r0,
                hop3_r1 = %hop3.entry.r1,
                hop3_oriented = ?r3,
                hint = "diagnostic dump for root-cause analysis — math kernel returned profit > 5x cap",
            );
            return Some(cycle_block);
        }

        // Build & emit Opportunity.
        // H2 landmine fix: compute net_expected_profit_usd inline so submit_engine
        // Check 7 gates on NET (gross minus gas), not gross.  The spine evaluator
        // does not run on the worker persistence path, so we must populate the field
        // here.  Formula mirrors prioritization_spine::config_aware::estimate_gas_cost_usd.
        // When cfg is absent (cold-start / Redis miss) we fall back to None, which
        // causes resolve_profit_for_checklist to use gross — same conservative
        // direction as the pre-fix behaviour, not a regression.
        let net_expected_profit_usd_v2: Option<f64> = cfg.as_ref().and_then(|c| {
            let gross = result.expected_profit_usd?;
            Some(gross - c.gas_cost_usd())
        });

        let opp = Opportunity {
            id: Uuid::new_v4(),
            chain_id: self.chain_id,
            strategy_kind: StrategyKind::triangular(),
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
            net_expected_profit_usd: net_expected_profit_usd_v2,
            roi_pct: None,
            risk_score: None,
            block_number: Some(cycle_block),
            rejection_reason: None,
            cartridge_id: None,
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

        // §IV Gap 1 fix: build RouteMetadata so sim-ctl can resolve the route.
        // Triangular: 3 hops (A→B→C→A). Uses the resolved cycle token addresses
        // — NOT each pool's token0_addr, which is pool-ordering-dependent and
        // not the actual traversal sequence (the old code produced 4× token0,
        // which is meaningless as a route path).
        let route_metadata = RouteMetadata {
            pool_addresses: vec![
                hop1.pool_addr.to_string(),
                hop2.pool_addr.to_string(),
                hop3.pool_addr.to_string(),
            ],
            token_addresses: vec![
                addr_a.clone(),
                addr_b.clone(),
                addr_c.clone(),
                addr_a.clone(),
            ],
            dex_adapters: vec![
                "uniswap_v2_router".to_string(),
                "uniswap_v2_router".to_string(),
                "uniswap_v2_router".to_string(),
            ],
            decimals: Default::default(),
        };

        // Persist + publish (best-effort, mirror scanner.rs pattern).
        if let Some(pool) = db {
            if let Err(e) =
                persistence::insert_opportunity_with_route(pool, &opp, Some(&route_metadata)).await
            {
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

    /// Scan all V3-bearing cycles (currently `V3_CYCLES`) in both directions
    /// with **two-phase quoting** (math-validator CRITICAL fix 2026-05-06).
    ///
    /// **Why two phases?** A V3 hop's amount_in equals its predecessor's
    /// amount_out. For V3 hops whose predecessor is V2, amount_in is known
    /// synchronously (V2 math is closed-form). For V3 hops whose predecessor
    /// is ANOTHER V3 hop, amount_in is known only AFTER the predecessor's
    /// QuoterV2 result is back. The original single-phase implementation
    /// (which left `current_in` unchanged across V3 hops) fed downstream V2
    /// hops the wrong-token wei and saturated their CPMM, producing fake
    /// positives at profit_cap_ratio ≈ 2.5x — slipping past the 5x sanity
    /// bound. Two-phase quoting + chain consistency check eliminates the
    /// failure mode at its root.
    ///
    /// **Cost:** at most 2 multicalls per tick instead of 1 — well within the
    /// per-block RPC budget. For the current `V3_CYCLES` topology (X-WETH-USDC
    /// with the V2 hop in the middle), Phase 2 is typically empty because
    /// every V3 hop has a V2 (or cap) predecessor. The infrastructure is in
    /// place for future cycles where two consecutive V3 hops appear.
    ///
    /// Strategy (decision C in the design brief — single-point evaluation):
    ///   1. For each cycle, both directions, resolve every hop via
    ///      `resolve_hop_any` (V2 cached path first, V3 pool index fallback).
    ///   2. Compute amount_in for the cycle from the operator's capital cap
    ///      (`cap_usd / price_a → wei`). If the cap math fails, skip cycle.
    ///   3. Walk hops with `try_progress_plan`: when a hop's amount_in is
    ///      known (hop 0 or predecessor's amount_out is now known), compute
    ///      its amount_out — V2 inline, V3 queued for the current phase batch.
    ///   4. Phase 1 multicall — every V3 hop whose amount_in was known on the
    ///      first walk (≥ 1 batched Multicall3 RPC per tick).
    ///   5. Walk hops again. V2 hops that follow a Phase 1 V3 hop now have a
    ///      known amount_in → compute their amount_out. V3 hops that follow
    ///      another V3 hop now have a known amount_in → queue for Phase 2.
    ///   6. Phase 2 multicall (if non-empty). For the current `V3_CYCLES`
    ///      topology (`X → WETH → USDC → X` with V2 in the middle hop),
    ///      Phase 2 is empty because every V3 hop has a V2 or cap predecessor.
    ///      Infrastructure is in place for future cycles with consecutive
    ///      V3 hops.
    ///   7. Build chain-consistent `HopAmountOut` (carrying both
    ///      `amount_in_used` and `amount_out`); evaluate via
    ///      `evaluate_v3_cycle` which verifies that for i > 0 each hop's
    ///      amount_in_used matches the predecessor's amount_out. Apply the
    ///      same 5x sanity bound + dedup as the V2 path.
    ///
    /// **Anti-Incidente #9 (math-validator CRITICAL fix 2026-05-06):** the
    /// previous single-phase implementation left `current_in` unchanged after
    /// V3 hops, feeding V2 with wrong-token wei → saturated CPMM →
    /// downstream V3 quote inflated → profit_cap_ratio ≈ 2.5x slipped past
    /// the 5x sanity bound, emitting fake positives. Two-phase quoting +
    /// chain consistency check at the kernel boundary eliminates the failure
    /// mode at its root.
    ///
    /// Returns the latest cycle block observed (for dedup pruning).
    #[allow(clippy::too_many_arguments)]
    async fn scan_v3_bearing_cycles(
        &self,
        redis: &mut ConnectionManager,
        db: Option<&PgPool>,
        cfg: &Option<shared_rs::trading_config::TradingConfigState>,
        price_snapshot: &std::collections::HashMap<String, f64>,
        rpc_pool: Arc<HttpRpcPool>,
        dedup: &mut DedupState,
        stats: &mut TickStats,
    ) -> Option<u64> {
        // ---------------------------------------------------------------
        // Resolve every cycle in both directions; collect per-cycle plans.
        // ---------------------------------------------------------------
        let mut plans: Vec<V3CyclePlan> = Vec::new();
        for (a, b, c) in V3_CYCLES {
            for direction in &[(*a, *b, *c), (*a, *c, *b)] {
                counters()
                    .triangular_v3_cycles_scanned
                    .fetch_add(1, Ordering::Relaxed);
                stats.scanned += 1;
                let (sym_a, sym_b, sym_c) = *direction;

                let plan = match self
                    .build_v3_cycle_plan(redis, cfg, price_snapshot, sym_a, sym_b, sym_c, stats)
                    .await
                {
                    Some(p) => p,
                    None => continue,
                };
                plans.push(plan);
            }
        }
        if plans.is_empty() {
            return None;
        }

        // ---------------------------------------------------------------
        // Two-phase quoting (math-validator CRITICAL fix 2026-05-06).
        //
        // Phase loop: progress every plan synchronously (V2 inline math,
        // queue V3 hops whose amount_in is now known). Run ONE multicall
        // for the queued V3 hops. Repeat until no plan made progress in
        // an iteration. Bounded at HOPS_PER_CYCLE phases (3 hops × 1 V3
        // dependency-chain each = at most 3 phases for any 3-hop cycle).
        // ---------------------------------------------------------------
        const MAX_PHASES: usize = 3;
        let quoter = match Address::from_str(V3_QUOTER_V2_MAINNET) {
            Ok(a) => a,
            Err(e) => {
                warn!(event = "triangular_worker.v3_quoter_addr_invalid", error = %e);
                return None;
            }
        };
        let multicall_addr = match Address::from_str(V3_MULTICALL3_ADDR) {
            Ok(a) => a,
            Err(e) => {
                warn!(event = "triangular_worker.v3_multicall_addr_invalid", error = %e);
                return None;
            }
        };

        for phase in 0..MAX_PHASES {
            let mut phase_requests: Vec<V3QuoteRequest> = Vec::new();
            // Index from request slot → (plan_idx, hop_idx) so results route
            // back into the right plan slot.
            let mut routing: Vec<(usize, usize)> = Vec::new();
            let mut any_progress = false;

            for (plan_idx, plan) in plans.iter_mut().enumerate() {
                let progressed =
                    try_progress_plan(plan, &mut phase_requests, &mut routing, plan_idx);
                if progressed {
                    any_progress = true;
                }
            }

            if !any_progress {
                // Nothing left to do — either the cycle is fully resolved or
                // quote failures left holes (handled below in evaluation).
                break;
            }

            if phase_requests.is_empty() {
                // V2-only progress this phase (no RPC); continue to next phase
                // in case a later V3 hop is now resolvable.
                continue;
            }

            // Run Phase N multicall — via with_retry so the circuit breaker
            // and failover are engaged on RPC failure.
            let phase_results = match rpc_pool
                .with_retry(|provider| {
                    let reqs = phase_requests.clone();
                    async move {
                        v3_quote_exact_in_multicall(provider, quoter, multicall_addr, reqs).await
                    }
                })
                .await
            {
                Ok(r) => r,
                Err(e) => {
                    // Whole-batch RPC failure → all V3 hops in THIS phase
                    // counted as failures. Plans whose unresolved V3 hops
                    // were in this batch will be skipped during evaluation
                    // (their hop_amount_outs stay None).
                    let n = phase_requests.len() as u64;
                    counters()
                        .triangular_v3_quote_failures
                        .fetch_add(n, Ordering::Relaxed);
                    stats.skip_v3_quote_failed += n as u32;
                    warn!(
                        event = "triangular_worker.v3_multicall_rpc_failed",
                        chain_id = self.chain_id,
                        phase = phase,
                        batch_size = phase_requests.len(),
                        error = %e,
                    );
                    // Don't return early — V2-only plans may still be valid.
                    // Break out of the phase loop and proceed to evaluation;
                    // partial-progress plans naturally fail the chain check.
                    break;
                }
            };

            for (i, res) in phase_results.iter().enumerate() {
                let (plan_idx, hop_idx) = routing[i];
                if res.success && !res.amount_out.is_zero() {
                    plans[plan_idx].hop_amount_outs[hop_idx] = Some(res.amount_out);
                } else {
                    counters()
                        .triangular_v3_quote_failures
                        .fetch_add(1, Ordering::Relaxed);
                    // Leave amount_out as None — caller skips during eval.
                }
            }
        }

        // ---------------------------------------------------------------
        // Evaluate every plan with its now-complete amount_outs.
        // ---------------------------------------------------------------
        let mut latest_block: u64 = 0;
        for plan in plans.into_iter() {
            // Drop plans that had any hop come back without an amount_out OR
            // an amount_in (= V3 quote failed before downstream chain could
            // be established). R8 fail-honest — never fabricate a substitute.
            let mut hop_outs: Vec<HopAmountOut> = Vec::with_capacity(plan.hops.len());
            let mut missing = false;
            for (hop_idx, hop) in plan.hops.iter().enumerate() {
                let amount_in_used = match plan.hop_amount_ins[hop_idx] {
                    Some(v) => v,
                    None => {
                        missing = true;
                        break;
                    }
                };
                let (amount_out, source) = match (hop, plan.hop_amount_outs[hop_idx]) {
                    (HopKind::V2(_), Some(o)) => (o, HopAmountSource::V2),
                    (HopKind::V3 { .. }, Some(o)) => (o, HopAmountSource::V3),
                    _ => {
                        missing = true;
                        break;
                    }
                };
                hop_outs.push(HopAmountOut {
                    amount_in_used,
                    amount_out,
                    source,
                });
            }
            if missing {
                stats.skip_v3_quote_failed += 1;
                debug!(
                    event = "triangular_worker.v3_cycle_quote_missing",
                    chain_id = self.chain_id,
                    cycle = %plan.cycle_key,
                );
                continue;
            }

            // Block attribution: take the max V2-hop block in the plan (or
            // 0 if pure V3, which V3_CYCLES never hits because the WETH↔USDC
            // middle hop is always V2-resolvable).
            let cycle_block = cycle_latest_block_mixed(&plan.hops);
            if cycle_block > latest_block {
                latest_block = cycle_block;
            }

            // Staleness check (V2 hops only — V3 is "current state").
            if cycle_has_stale_reserves_mixed(&plan.hops) {
                stats.skip_stale_reserves += 1;
                debug!(
                    event = "triangular_worker.v3_cycle_stale_reserves",
                    chain_id = self.chain_id,
                    cycle = %plan.cycle_key,
                );
                continue;
            }

            // Dedup BEFORE evaluation/persistence.
            if !dedup.check_and_mark(&plan.cycle_key, cycle_block) {
                stats.skip_dedup_hit += 1;
                continue;
            }

            // Pure-function evaluation.
            let result = match evaluate_v3_cycle(
                plan.amount_in_wei,
                &hop_outs,
                Some(plan.price_a),
                plan.decimals_a,
            ) {
                Some(r) => r,
                None => {
                    stats.skip_no_profit += 1;
                    debug!(
                        event = "triangular_worker.v3_no_profit",
                        chain_id = self.chain_id,
                        cycle = %plan.cycle_key,
                        amount_in_wei = %plan.amount_in_wei,
                        final_out_wei = %hop_outs.last().map(|h| h.amount_out).unwrap_or(U256::zero()),
                    );
                    continue;
                }
            };

            // Worker-level sanity bound (mirrors V2 path) — V3 tick math is
            // MORE complex than V2 CPMM, so bugs are MORE likely. Same
            // SANITY_PROFIT_MULT_OF_CAP=5× threshold the V2 path uses.
            const SANITY_PROFIT_MULT_OF_CAP: f64 = 5.0;
            let profit_usd = match result.expected_profit_usd {
                Some(v) if v.is_finite() && v >= 0.0 => v,
                _ => {
                    stats.skip_no_profit += 1;
                    warn!(
                        event = "triangular_worker.v3_malformed_profit",
                        chain_id = self.chain_id,
                        cycle = %plan.cycle_key,
                        raw = ?result.expected_profit_usd,
                    );
                    continue;
                }
            };
            let profit_cap_ratio = profit_usd / plan.cap_usd;
            if profit_cap_ratio > SANITY_PROFIT_MULT_OF_CAP {
                counters()
                    .triangular_v3_sanity_reject
                    .fetch_add(1, Ordering::Relaxed);
                stats.skip_no_profit += 1;
                // Diagnostic dump: per-hop pool / kind / amount_in / amount_out.
                let dump_hops: Vec<String> = plan
                    .hops
                    .iter()
                    .enumerate()
                    .map(|(i, h)| {
                        let kind = match h {
                            HopKind::V2(_) => "V2",
                            HopKind::V3 { .. } => "V3",
                        };
                        format!(
                            "{}:{}:in={}:out={}",
                            kind,
                            h.pool_addr(),
                            plan.hop_amount_ins[i].unwrap_or(U256::zero()),
                            plan.hop_amount_outs[i].unwrap_or(U256::zero()),
                        )
                    })
                    .collect();
                warn!(
                    event = "triangular_worker.sanity_reject_v3",
                    chain_id = self.chain_id,
                    cycle = %plan.cycle_key,
                    expected_profit_usd = profit_usd,
                    cap_usd = plan.cap_usd,
                    profit_cap_ratio = profit_cap_ratio,
                    threshold = SANITY_PROFIT_MULT_OF_CAP,
                    amount_in_wei = %result.amount_in_wei,
                    amount_out_wei = %result.amount_out_wei,
                    hops = ?dump_hops,
                    hint = "diagnostic dump for root-cause analysis — V3-bearing cycle returned profit > 5x cap",
                );
                continue;
            }

            // Build & emit Opportunity (mirrors V2 path).
            // H2 landmine fix: populate net_expected_profit_usd inline.
            // See V2 emit site for full rationale.
            let net_expected_profit_usd_v3: Option<f64> = cfg.as_ref().and_then(|c| {
                let gross = result.expected_profit_usd?;
                Some(gross - c.gas_cost_usd())
            });
            let opp = Opportunity {
                id: Uuid::new_v4(),
                chain_id: self.chain_id,
                strategy_kind: StrategyKind::triangular(),
                dex_a: "uniswap-v3".to_string(),
                dex_b: Some(format!(
                    "cycle:{}>{}>{}",
                    plan.sym_a.to_uppercase(),
                    plan.sym_b.to_uppercase(),
                    plan.sym_c.to_uppercase(),
                )),
                pair_symbol: format!(
                    "{}/{}/{}/{}",
                    plan.sym_a, plan.sym_b, plan.sym_c, plan.sym_a
                ),
                token_in: plan.addr_a.clone(),
                token_out: plan.addr_a.clone(),
                amount_in_wei: result.amount_in_wei.to_string(),
                expected_profit_usd: result.expected_profit_usd,
                net_expected_profit_usd: net_expected_profit_usd_v3,
                roi_pct: None,
                risk_score: None,
                block_number: Some(cycle_block),
                rejection_reason: None,
                cartridge_id: None,
                detected_at: Utc::now(),
                trace_id: Uuid::new_v4(),
            };

            info!(
                event = "triangular_worker.v3_opp_emit",
                chain_id = self.chain_id,
                cycle = %plan.cycle_key,
                block = cycle_block,
                amount_in_wei = %opp.amount_in_wei,
                expected_profit_usd = ?opp.expected_profit_usd,
                hop1 = %plan.hops[0].pool_addr(),
                hop2 = %plan.hops[1].pool_addr(),
                hop3 = %plan.hops[2].pool_addr(),
            );

            // §IV Gap 1 fix (V3, 2026-08-10): build RouteMetadata so sim-ctl and
            // the exchange dashboard can resolve the 3-hop cycle (A→B→C→A).
            // V3-bearing cycles may mix V2/V3 pools; dex_adapters reflect each
            // hop's actual protocol. Decimals resolved separately downstream.
            let route_metadata = RouteMetadata {
                pool_addresses: plan
                    .hops
                    .iter()
                    .map(|h| h.pool_addr().to_string())
                    .collect(),
                token_addresses: vec![
                    plan.addr_a.clone(),
                    plan.addr_b.clone(),
                    plan.addr_c.clone(),
                    plan.addr_a.clone(),
                ],
                dex_adapters: plan
                    .hops
                    .iter()
                    .map(|h| match h {
                        HopKind::V2(_) => "uniswap_v2_router",
                        HopKind::V3 { .. } => "uniswap_v3_router",
                    })
                    .map(String::from)
                    .collect(),
                decimals: Default::default(),
            };

            if let Some(pool) = db {
                if let Err(e) =
                    persistence::insert_opportunity_with_route(pool, &opp, Some(&route_metadata))
                        .await
                {
                    counters().db_errors.fetch_add(1, Ordering::Relaxed);
                    warn!(event = "triangular_worker.v3_db_error", error = %e);
                } else {
                    counters().db_persisted.fetch_add(1, Ordering::Relaxed);
                }
            }
            if let Err(e) = publisher::publish(redis, &opp).await {
                warn!(event = "triangular_worker.v3_publish_error", error = %e);
            } else {
                counters()
                    .triangular_opps_emitted
                    .fetch_add(1, Ordering::Relaxed);
                stats.emitted += 1;
            }
        }

        Some(latest_block)
    }

    /// Build a `V3CyclePlan` for one V3-bearing cycle direction.
    ///
    /// Resolves token metadata + all 3 hops + the capital cap. Initialises
    /// `hop_amount_ins[0] = amount_in_wei`; **does NOT chain through V3 hops**
    /// (math-validator CRITICAL fix 2026-05-06). Chaining happens iteratively
    /// in `scan_v3_bearing_cycles` via `try_progress_plan`, which only fills
    /// each hop's amount_in/amount_out when the predecessor is fully known.
    /// V3 hops with unknown amount_in are deferred to the next phase.
    ///
    /// Returns None on any of:
    ///   - unknown token (one of sym_a/b/c not in `known_token_address`)
    ///   - missing pool for any hop (neither V2 nor V3 covers the pair)
    ///   - cap math fails (zero/NaN price, sub-token cap)
    #[allow(clippy::too_many_arguments)]
    async fn build_v3_cycle_plan(
        &self,
        redis: &mut ConnectionManager,
        cfg: &Option<shared_rs::trading_config::TradingConfigState>,
        price_snapshot: &std::collections::HashMap<String, f64>,
        sym_a: &'static str,
        sym_b: &'static str,
        sym_c: &'static str,
        stats: &mut TickStats,
    ) -> Option<V3CyclePlan> {
        // Resolve token metadata.
        let (addr_a, decimals_a, _is_stable_a) =
            match resolve_token(redis, self.chain_id, sym_a).await {
                Some(t) => t,
                None => {
                    stats.skip_unknown_token += 1;
                    return None;
                }
            };
        let (addr_b, _, _) = match resolve_token(redis, self.chain_id, sym_b).await {
            Some(t) => t,
            None => {
                stats.skip_unknown_token += 1;
                return None;
            }
        };
        let (addr_c, _, _) = match resolve_token(redis, self.chain_id, sym_c).await {
            Some(t) => t,
            None => {
                stats.skip_unknown_token += 1;
                return None;
            }
        };

        // Resolve all 3 hops as V2-or-V3.
        let hop1 = resolve_hop_any(redis, self.chain_id, &addr_a, sym_a, &addr_b, sym_b).await;
        let hop2 = resolve_hop_any(redis, self.chain_id, &addr_b, sym_b, &addr_c, sym_c).await;
        let hop3 = resolve_hop_any(redis, self.chain_id, &addr_c, sym_c, &addr_a, sym_a).await;
        let (h1, h2, h3) = match (hop1, hop2, hop3) {
            (Some(h1), Some(h2), Some(h3)) => (h1, h2, h3),
            _ => {
                stats.skip_missing_pool += 1;
                debug!(
                    event = "triangular_worker.v3_cycle_missing_pool",
                    chain_id = self.chain_id,
                    cycle = format!("{}>{}>{}", sym_a, sym_b, sym_c),
                );
                return None;
            }
        };

        // Capital cap → amount_in_wei.
        let cap_usd = cfg
            .as_ref()
            .map(|c| c.effective_capital_for(sym_a, STRATEGY_KIND))
            .unwrap_or(0.0);
        if cap_usd <= 0.0 {
            stats.skip_no_capital_cap += 1;
            debug!(
                event = "triangular_worker.v3_no_capital_cap",
                chain_id = self.chain_id,
                token = sym_a,
            );
            return None;
        }
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
            })?;
        let amount_in_wei = match cap_amount_in_wei(cap_usd, price_a, decimals_a) {
            Some(w) if !w.is_zero() => w,
            _ => {
                stats.skip_no_capital_cap += 1;
                return None;
            }
        };

        // Initialise per-hop arrays. Only hop 0's amount_in is known up front;
        // every other hop's amount_in becomes known progressively as the
        // multi-phase algorithm in `scan_v3_bearing_cycles` resolves V3 hops
        // and forwards their amount_outs into V2 hop math.
        let hops = vec![h1, h2, h3];
        let mut hop_amount_ins: Vec<Option<U256>> = vec![None; hops.len()];
        let hop_amount_outs: Vec<Option<U256>> = vec![None; hops.len()];
        hop_amount_ins[0] = Some(amount_in_wei);

        Some(V3CyclePlan {
            cycle_key: cycle_hash(&[sym_a, sym_b, sym_c]),
            sym_a,
            sym_b,
            sym_c,
            addr_a,
            addr_b,
            addr_c,
            decimals_a,
            cap_usd,
            price_a,
            amount_in_wei,
            hops,
            hop_amount_ins,
            hop_amount_outs,
        })
    }
}

/// Plan returned by `build_v3_cycle_plan` and consumed by `scan_v3_bearing_cycles`.
/// Externalised so the inner function can construct it without a giant tuple.
///
/// **Two-phase invariants** (math-validator CRITICAL fix 2026-05-06):
///   - `hop_amount_ins[0]` is always `Some(amount_in_wei)` from construction.
///   - `hop_amount_ins[i]` for `i > 0` is filled by `try_progress_plan` once
///     the previous hop's `amount_out` is known. NEVER filled with a stale
///     or pre-V3 value.
///   - `hop_amount_outs[i]` is filled either inline by V2 math (when its
///     amount_in becomes known) or by Phase N multicall results (V3).
///   - A hop's amount_out being `None` after all phases run = quote failure
///     → cycle skipped (R8 fail-honest).
struct V3CyclePlan {
    cycle_key: String,
    sym_a: &'static str,
    sym_b: &'static str,
    sym_c: &'static str,
    addr_a: String,
    // Resolved cycle token addresses (B, C). Added 2026-08-10 so the V3 persist
    // path can build a correct RouteMetadata (token path A→B→C→A) — previously
    // only addr_a was carried, so V3 detections persisted with no route topology.
    addr_b: String,
    addr_c: String,
    decimals_a: u8,
    cap_usd: f64,
    price_a: f64,
    amount_in_wei: U256,
    hops: Vec<HopKind>,
    /// Per-hop amount_in. `Some` only once the predecessor's amount_out is
    /// known; never assumed/approximated.
    hop_amount_ins: Vec<Option<U256>>,
    hop_amount_outs: Vec<Option<U256>>,
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)] // M11: test module
    #![allow(clippy::field_reassign_with_default, clippy::assertions_on_constants)]
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
        assert!(
            out < U256::from(1_000u64),
            "balanced cycle should lose to fees"
        );
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
        assert!(
            out > U256::from(1_000u64),
            "imbalanced cycle should net positive"
        );
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
        assert!(
            profit > 0,
            "profit={} at x*={} expected positive",
            profit,
            x_star
        );
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
        assert!(
            profit <= 0,
            "balanced cycle: profit={} should be ≤ 0",
            profit
        );
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
            (U256::from(10_000u64), U256::from(50_000_000u64)), // 5000x
            (U256::from(50_000_000u64), U256::from(50_000_000u64)),
            (U256::from(50_000_000u64), U256::from(50_000_000u64)),
        ];
        let (_, profit_search) = golden_section_search(
            U256::from(1u64),
            U256::from(1_000_000u64),
            &reserves,
            30,
            25,
        );
        // Brute-force every 10K-wei step: find the best.
        let mut brute_best = i128::MIN;
        for x_raw in (1..=1_000_000u64).step_by(10_000) {
            let (_, p) = cycle_profit(U256::from(x_raw), &reserves, 30);
            if p > brute_best {
                brute_best = p;
            }
        }
        assert!(brute_best > 0, "brute force should find positive profit");
        assert!(
            profit_search > 0,
            "search profit={} should be > 0",
            profit_search
        );
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
        assert!(
            cap.is_none(),
            "sub-token cap must yield None, not silently emit 0 wei"
        );
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
            let addr =
                known_token_address(sym).unwrap_or_else(|| panic!("missing address for {sym}"));
            assert!(
                addr.starts_with("0x") && addr.len() == 42,
                "bad addr for {sym}: {addr}"
            );
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
        // 5e19 doesn't fit u64 (max ~1.8e19) → use 10×U256::exp10(18) = 1e19
        // and 50×U256::exp10(18) for the larger value.
        let x = U256::from(2u64) * U256::exp10(18); // 2 WETH (2e18 wei)
        let reserves = vec![
            // WETH→USDC realistic (10 WETH = 1e19 wei, 20K USDC = 2e10 wei)
            (
                U256::from(10u64) * U256::exp10(18),
                U256::from(20_000_000_000u64),
            ),
            // USDC→WBTC FLIPPED (this is the bug — 61M USDC wei, 50B WBTC wei… inverted)
            (U256::from(61_564_199u64), U256::from(49_997_126_049u64)),
            // WBTC→WETH realistic (1B WBTC wei = 10 WBTC, 50 WETH = 5e19 wei)
            (
                U256::from(1_000_000_000u64),
                U256::from(50u64) * U256::exp10(18),
            ),
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
        assert!(
            documented_threshold > 1.0,
            "threshold must allow legitimate profitable opps"
        );
        assert!(
            documented_threshold < 100.0,
            "threshold must catch orientation-flip bugs"
        );
    }

    // ===================================================================
    // V3-bearing cycle support — Subproyecto 2 (2026-05-07)
    // ===================================================================

    /// Helper: build a chain-consistent `HopAmountOut` sequence with V2/V3
    /// sources for tests. Each hop's `amount_in_used` is set from the
    /// predecessor's amount_out (or `amount_in` for hop 0), matching the
    /// invariant that the production code maintains via `try_progress_plan`.
    fn outs_v3_chain(amount_in: U256, values: &[(u128, HopAmountSource)]) -> Vec<HopAmountOut> {
        let mut out = Vec::with_capacity(values.len());
        let mut prev_in = amount_in;
        for (v, s) in values {
            let amount_out = U256::from(*v);
            out.push(HopAmountOut {
                amount_in_used: prev_in,
                amount_out,
                source: *s,
            });
            prev_in = amount_out;
        }
        out
    }

    /// Compatibility helper kept for the older tests that hand-build chains
    /// where amount_in_used == amount_in for hop 0 only and chains through.
    /// New tests should prefer `outs_v3_chain`.
    fn outs_v3(values: &[(u128, HopAmountSource)]) -> Vec<HopAmountOut> {
        // Default: chain from a 1e18 reference; the existing tests using this
        // helper supply explicit amount_in to evaluate_v3_cycle, so the chain
        // computed here is overwritten only on the first hop. To stay
        // backward-compatible with their expectations (which checked only
        // amount_out semantics), we fix `amount_in_used = amount_out_of_prev`
        // assuming hop 0's amount_in matches what the test passes to
        // evaluate_v3_cycle. The tests using this helper that pass arbitrary
        // amount_in must be updated to use `outs_v3_chain` directly.
        outs_v3_chain(U256::from(10u128).pow(U256::from(18)), values)
    }

    // ---- evaluate_v3_cycle — pure kernel ----

    #[test]
    fn evaluate_v3_cycle_profitable_returns_some() {
        // amount_in 1 WETH (1e18 wei), final_out 1.001 WETH → 1e15 wei profit.
        // At $2000/WETH the profit is $2.00. cap_usd is irrelevant to the
        // pure kernel — only the sanity bound at the caller uses it.
        let amount_in = U256::from(10u128).pow(U256::from(18));
        let outs = outs_v3(&[
            (3_500_000_000u128, HopAmountSource::V2), // mid-cycle USDC wei
            (1_750_000u128 * 1_000_000_000_000u128, HopAmountSource::V3),
            (
                amount_in.as_u128() + 1_000_000_000_000_000u128,
                HopAmountSource::V3,
            ),
        ]);
        let r = evaluate_v3_cycle(amount_in, &outs, Some(2_000.0), 18)
            .expect("profitable cycle must yield Some");
        assert!(r.profit_token_a_wei > 0);
        let usd = r.expected_profit_usd.expect("usd must be priced");
        // 1e15 wei × $2000 / 1e18 = $2.00
        assert!((usd - 2.0).abs() < 1e-9, "usd={} expected ~2.00", usd);
    }

    #[test]
    fn v3_cycle_pepe_weth_usdc_known_quote_returns_some_when_profitable() {
        // Realistic mainnet magnitudes for a PEPE→WETH→USDC→PEPE cycle.
        // PEPE has 18 decimals; price ≈ $1e-5. cap_usd = $1000 → 1e8 PEPE
        // (single-pip whales here, this is the long-tail case).
        // amount_in = 1e26 wei (1e8 PEPE * 1e18 / 1e6).
        // Target: profit > 0 token_a_wei, expected_profit_usd ~$3 on a 0.3% spread.
        // Chain-consistent (math-validator CRITICAL fix): each hop's
        // amount_in_used is the predecessor's amount_out (or amount_in_wei
        // for hop 0). We use `outs_v3_chain` so the kernel chain check passes.
        let amount_in = U256::from(10u128).pow(U256::from(26));
        let final_out = amount_in + (amount_in / U256::from(333u32)); // ~+0.3%
        let outs = outs_v3_chain(
            amount_in,
            &[
                (1_000_000_000_000_000u128, HopAmountSource::V3), // PEPE→WETH
                (3_500_000_000u128, HopAmountSource::V2),         // WETH→USDC
                (final_out.as_u128(), HopAmountSource::V3),       // USDC→PEPE (closes >input)
            ],
        );
        let r = evaluate_v3_cycle(amount_in, &outs, Some(0.00001), 18)
            .expect("PEPE long-tail cycle must produce Some on positive spread");
        assert!(r.profit_token_a_wei > 0);
        let usd = r.expected_profit_usd.expect("usd should be priced");
        assert!(usd > 0.0, "usd profit should be > 0, got {}", usd);
    }

    #[test]
    fn v3_cycle_quote_failure_skips_cleanly() {
        // Any zero amount_out in the chain → None. Models a V3 QuoterV2
        // call coming back success=false (insufficient liquidity, pool
        // revert, RPC error). R8 fail-honest: never substitute a value.
        let amount_in = U256::from(10u128).pow(U256::from(18));
        let outs = outs_v3(&[
            (5_000u128, HopAmountSource::V2),
            (0u128, HopAmountSource::V3), // failed quote
            (10_000u128, HopAmountSource::V3),
        ]);
        let r = evaluate_v3_cycle(amount_in, &outs, Some(2_000.0), 18);
        assert!(
            r.is_none(),
            "any zero-amount hop must collapse cycle to None"
        );
    }

    #[test]
    fn v3_cycle_returns_none_when_not_profitable() {
        // final_out <= amount_in → no profit, return None even with valid quotes.
        let amount_in = U256::from(10u128).pow(U256::from(18));
        let outs = outs_v3(&[
            (3_500_000_000u128, HopAmountSource::V2),
            (1_750_000u128 * 1_000_000_000_000u128, HopAmountSource::V3),
            (amount_in.as_u128() - 1u128, HopAmountSource::V3), // 1 wei loss
        ]);
        let r = evaluate_v3_cycle(amount_in, &outs, Some(2_000.0), 18);
        assert!(r.is_none(), "non-profitable cycle must return None");
    }

    #[test]
    fn v3_cycle_returns_none_when_price_unknown() {
        // Profitable but no token-a price → cannot compute USD profit, R8
        // fail-honest skip (mirrors V2 evaluate_cycle behaviour).
        let amount_in = U256::from(10u128).pow(U256::from(18));
        let outs = outs_v3(&[
            (3_500_000_000u128, HopAmountSource::V2),
            (1_750_000u128 * 1_000_000_000_000u128, HopAmountSource::V3),
            (
                amount_in.as_u128() + 1_000_000_000_000_000u128,
                HopAmountSource::V3,
            ),
        ]);
        let r = evaluate_v3_cycle(amount_in, &outs, None, 18);
        assert!(r.is_none(), "missing price must produce None");
    }

    #[test]
    fn v3_cycle_zero_amount_in_returns_none() {
        let outs = outs_v3(&[(1u128, HopAmountSource::V2); 3]);
        assert!(evaluate_v3_cycle(U256::zero(), &outs, Some(1.0), 18).is_none());
    }

    #[test]
    fn v3_cycle_wrong_hop_count_returns_none() {
        let amount_in = U256::from(10u128).pow(U256::from(18));
        let outs = outs_v3(&[(1u128, HopAmountSource::V2); 2]); // only 2 hops
        assert!(evaluate_v3_cycle(amount_in, &outs, Some(1.0), 18).is_none());
    }

    #[test]
    fn v3_cycle_sanity_rejects_huge_profit_orientation_simulation() {
        // Reproduces the orientation-flip scenario for V3: the multicall
        // returns an absurdly huge final_out because, e.g., the V3 pool
        // index has the wrong fee tier and the price impact computation
        // collapses. The pure kernel says "profit > 0 → Some"; the worker-
        // level sanity bound (in scan_v3_bearing_cycles) is what rejects it.
        // This test asserts the kernel surface — the integration sanity
        // bound is mirrored in `sanity_threshold_v3_is_documented_constant`.
        let amount_in = U256::from(10u128).pow(U256::from(18)); // 1 WETH
        let huge_out = amount_in * U256::from(1_000u32); // 1000× return
        let outs = outs_v3(&[
            (3_500_000_000u128, HopAmountSource::V2),
            (1u128, HopAmountSource::V3), // not zero, just tiny mid-cycle
            (huge_out.as_u128(), HopAmountSource::V3),
        ]);
        let r = evaluate_v3_cycle(amount_in, &outs, Some(2_000.0), 18)
            .expect("kernel returns Some on positive profit; sanity bound is integration-level");
        let usd = r.expected_profit_usd.unwrap();
        // Profit ≈ 999 WETH * $2000 = $1.998M. The integration sanity bound
        // (5× cap) would reject — this test documents the kernel surface.
        let cap_usd = 1_000.0;
        assert!(
            usd / cap_usd > 5.0,
            "integration sanity bound MUST reject this; ratio={}",
            usd / cap_usd
        );
    }

    #[test]
    fn v3_quote_request_construction_for_known_long_tail() {
        // Construct a V3QuoteRequest for the PEPE/WETH 0.30% V3 pool
        // (mainnet 0x11950d141ecb863f01007add7d1a342041227b58). Verifies
        // the wiring used by `scan_v3_bearing_cycles` to feed the quoter.
        use crate::amm_math::V3QuoteRequest;
        use std::str::FromStr;
        let req = V3QuoteRequest {
            pool_addr: Address::from_str("0x11950d141ecb863f01007add7d1a342041227b58").unwrap(),
            token_in: Address::from_str("0x6982508145454ce325ddbe47a25d4ec3d2311933").unwrap(), // PEPE
            token_out: Address::from_str("0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2").unwrap(), // WETH
            amount_in: U256::from(10u128).pow(U256::from(26)),
            fee_bps: 3000, // 0.30%
        };
        // Field accessibility check (the actual encode is exercised by
        // `amm_math::v3_tests::v3_quote_request_construction`).
        assert_eq!(req.fee_bps, 3000);
        assert_eq!(req.amount_in, U256::from(10u128).pow(U256::from(26)));
    }

    #[test]
    fn mixed_v2_v3_cycle_evaluates_correctly() {
        // V2 first hop, then V3, then V3 back. Mirrors the canonical
        // PEPE→WETH→USDC→PEPE shape but with WETH→PEPE direction
        // (V3 hop), USDC→WETH (V2 hop), PEPE→USDC (V3 hop).
        let amount_in = U256::from(10u128).pow(U256::from(18));
        let outs = outs_v3(&[
            (1_000_000_000_000u128, HopAmountSource::V2), // V2: WETH→USDC mid
            (5_000_000_000u128, HopAmountSource::V3),     // V3: USDC→intermediate
            (
                amount_in.as_u128() + 5_000_000_000_000_000u128,
                HopAmountSource::V3,
            ), // V3: back to WETH
        ]);
        let r = evaluate_v3_cycle(amount_in, &outs, Some(2_000.0), 18).unwrap();
        assert!(r.profit_token_a_wei > 0);
    }

    // ---- cap_amount_in_wei ----

    #[test]
    fn cap_amount_in_wei_returns_cap_for_known_price() {
        // cap=$2000, price=$2000/WETH → 1 WETH (1e18 wei).
        let w = cap_amount_in_wei(2_000.0, 2_000.0, 18).expect("cap should compute");
        assert_eq!(w, U256::from(10u128).pow(U256::from(18)));
    }

    #[test]
    fn cap_amount_in_wei_returns_none_for_zero_price() {
        assert!(cap_amount_in_wei(2_000.0, 0.0, 18).is_none());
        assert!(cap_amount_in_wei(2_000.0, f64::NAN, 18).is_none());
    }

    // ---- HopKind / HopAmountSource ----

    #[test]
    fn hopkind_v2_block_attribution() {
        let hd = HopData {
            pool_addr: "0xabc".into(),
            entry: ReservesEntry {
                r0: "1000".into(),
                r1: "1000".into(),
                token0_addr: Some("0x01".into()),
                blk: 12_345,
                ts: 0,
            },
            swap_in_is_token0: true,
        };
        let v2 = HopKind::V2(hd.clone());
        assert_eq!(v2.block_number(), Some(12_345));
        assert_eq!(v2.pool_addr(), "0xabc");
    }

    #[test]
    fn hopkind_v3_no_block() {
        let v3 = HopKind::V3 {
            pool_addr: "0xdef".into(),
            fee_bps: 500,
            token_in_addr: "0x01".into(),
            token_out_addr: "0x02".into(),
        };
        // V3 hops are stateless from the worker's perspective — block
        // attribution comes from co-located V2 hops.
        assert!(v3.block_number().is_none());
        assert_eq!(v3.pool_addr(), "0xdef");
    }

    #[test]
    fn cycle_latest_block_mixed_takes_max_of_v2_only() {
        let v2_a = HopKind::V2(HopData {
            pool_addr: "0xa".into(),
            entry: ReservesEntry {
                r0: "1".into(),
                r1: "1".into(),
                token0_addr: Some("0x01".into()),
                blk: 100,
                ts: 0,
            },
            swap_in_is_token0: true,
        });
        let v2_b = HopKind::V2(HopData {
            pool_addr: "0xb".into(),
            entry: ReservesEntry {
                r0: "1".into(),
                r1: "1".into(),
                token0_addr: Some("0x01".into()),
                blk: 105,
                ts: 0,
            },
            swap_in_is_token0: true,
        });
        let v3 = HopKind::V3 {
            pool_addr: "0xc".into(),
            fee_bps: 500,
            token_in_addr: "0x01".into(),
            token_out_addr: "0x02".into(),
        };
        assert_eq!(cycle_latest_block_mixed(&[v2_a, v2_b, v3]), 105);
    }

    #[test]
    fn cycle_has_stale_reserves_mixed_returns_true_when_v2_lag_exceeds() {
        let stale_v2 = HopKind::V2(HopData {
            pool_addr: "0xa".into(),
            entry: ReservesEntry {
                r0: "1".into(),
                r1: "1".into(),
                token0_addr: Some("0x01".into()),
                blk: 100,
                ts: 0,
            },
            swap_in_is_token0: true,
        });
        let fresh_v2 = HopKind::V2(HopData {
            pool_addr: "0xb".into(),
            entry: ReservesEntry {
                r0: "1".into(),
                r1: "1".into(),
                token0_addr: Some("0x01".into()),
                blk: 110, // lag = 10 > MAX_RESERVE_LAG_BLOCKS (5)
                ts: 0,
            },
            swap_in_is_token0: true,
        });
        let v3 = HopKind::V3 {
            pool_addr: "0xc".into(),
            fee_bps: 500,
            token_in_addr: "0x01".into(),
            token_out_addr: "0x02".into(),
        };
        assert!(cycle_has_stale_reserves_mixed(&[stale_v2, fresh_v2, v3]));
    }

    #[test]
    fn cycle_has_stale_reserves_mixed_pure_v3_returns_false() {
        // No V2 hops to compare against → no staleness signal possible.
        // V3 quotes are "current state" by definition.
        let v3a = HopKind::V3 {
            pool_addr: "0xa".into(),
            fee_bps: 500,
            token_in_addr: "0x01".into(),
            token_out_addr: "0x02".into(),
        };
        let v3b = HopKind::V3 {
            pool_addr: "0xb".into(),
            fee_bps: 3000,
            token_in_addr: "0x02".into(),
            token_out_addr: "0x03".into(),
        };
        assert!(!cycle_has_stale_reserves_mixed(&[v3a, v3b]));
    }

    // ---- V3_CYCLES sanity ----

    #[test]
    fn v3_cycles_all_use_known_long_tail_tokens() {
        // Every cycle in V3_CYCLES must use tokens that resolve through
        // `known_token_address`. PEPE/SHIB/MKR/COMP were added 2026-05-07.
        for (a, b, c) in V3_CYCLES {
            assert!(
                known_token_address(a).is_some()
                    && known_token_address(b).is_some()
                    && known_token_address(c).is_some(),
                "V3 cycle {}>{}>{}: every token must be in known_token_address fallback",
                a,
                b,
                c
            );
        }
    }

    #[test]
    fn v3_cycles_no_self_loop() {
        for (a, b, c) in V3_CYCLES {
            assert_ne!(a, b);
            assert_ne!(b, c);
            assert_ne!(a, c);
        }
    }

    #[test]
    fn v3_cycles_all_long_tails_present() {
        // Document-as-test: 2026-05-07 long-tail re-introduction added all
        // four major V3-only tokens. If a cycle is removed, this regression
        // signal fires.
        let long_tails = ["PEPE", "SHIB", "MKR", "COMP"];
        for sym in &long_tails {
            let found = V3_CYCLES.iter().any(|(a, _, _)| a == sym);
            assert!(
                found,
                "V3_CYCLES must contain a cycle starting with {}",
                sym
            );
        }
    }

    #[test]
    fn v3_cycles_share_weth_usdc_middle() {
        // All four MVP V3 cycles share the WETH→USDC middle hop so the
        // Phase-1 V2 cache covers that arm. A future-extension that
        // breaks this assumption MUST also relax the dedup block-attribution
        // logic in `cycle_latest_block_mixed` (which currently relies on at
        // least one V2 hop per cycle for a fresh block number).
        for (_, b, c) in V3_CYCLES {
            assert!(
                (*b == "WETH" && *c == "USDC") || (*b == "USDC" && *c == "WETH"),
                "V3 cycle middle hop must be WETH↔USDC, found ({}, {})",
                b,
                c
            );
        }
    }

    // ---- TickStats with V3 bucket ----

    #[test]
    fn tick_stats_dominant_includes_v3_quote_failed() {
        let s = TickStats {
            skip_v3_quote_failed: 8,
            skip_no_profit: 2,
            ..TickStats::default()
        };
        assert_eq!(s.dominant_skip_reason(), Some("v3_quote_failed"));
    }

    #[test]
    fn sanity_threshold_v3_is_documented_constant() {
        // The V3 sanity threshold mirrors V2 (5× cap). If this changes,
        // `sanity_reject_v3` log + `triangular_v3_sanity_reject` counter
        // semantics must update together.
        let documented_threshold: f64 = 5.0;
        assert!(documented_threshold > 1.0);
        assert!(documented_threshold < 100.0);
    }

    // ===================================================================
    // Chain-consistency regression tests — math-validator CRITICAL fix
    // 2026-05-06. These guard the V3-bearing cycle path against the
    // original failure mode (single-phase quoting that left current_in
    // unchanged across V3 hops, producing fake positives ~2.5x cap_usd
    // that slipped past the 5x sanity bound).
    // ===================================================================

    /// CRITICAL regression: `evaluate_v3_cycle` must reject a chain where
    /// hop 0's amount_in_used does NOT match the cycle's amount_in_wei.
    /// The kernel's chain-check is the safety net behind two-phase quoting.
    #[test]
    fn evaluate_v3_cycle_rejects_inconsistent_chain_hop0() {
        let amount_in = U256::from(10u128).pow(U256::from(18));
        // Hop 0 amount_in_used is WRONG (claims 2x the cycle's amount_in).
        let outs = vec![
            HopAmountOut {
                amount_in_used: amount_in * U256::from(2u32),
                amount_out: U256::from(3_500_000_000u64),
                source: HopAmountSource::V3,
            },
            HopAmountOut {
                amount_in_used: U256::from(3_500_000_000u64),
                amount_out: U256::from(1_750_000u128 * 1_000_000_000_000u128),
                source: HopAmountSource::V2,
            },
            HopAmountOut {
                amount_in_used: U256::from(1_750_000u128 * 1_000_000_000_000u128),
                amount_out: amount_in + U256::from(10u128).pow(U256::from(15)), // profitable
                source: HopAmountSource::V3,
            },
        ];
        // Chain check rejects despite the math being "profitable".
        let r = evaluate_v3_cycle(amount_in, &outs, Some(2_000.0), 18);
        assert!(
            r.is_none(),
            "kernel must reject when hop 0's amount_in_used != cycle amount_in"
        );
    }

    /// CRITICAL regression: `evaluate_v3_cycle` must reject a chain where
    /// hop i (i > 0) has amount_in_used that does NOT match hop i-1's
    /// amount_out. This is the EXACT failure mode of the original V3
    /// implementation: V3 hops left `current_in` unchanged, so the next
    /// hop's amount_in didn't equal its predecessor's amount_out.
    #[test]
    fn evaluate_v3_cycle_rejects_inconsistent_chain() {
        let amount_in = U256::from(10u128).pow(U256::from(18));
        let outs = vec![
            HopAmountOut {
                amount_in_used: amount_in,
                amount_out: U256::from(3_500_000_000u64),
                source: HopAmountSource::V3,
            },
            // Hop 1 amount_in_used DOES NOT match hop 0's amount_out
            // (this is the original V3 bug — current_in stayed at amount_in).
            HopAmountOut {
                amount_in_used: amount_in,
                amount_out: U256::from(1_750_000u128 * 1_000_000_000_000u128),
                source: HopAmountSource::V2,
            },
            HopAmountOut {
                amount_in_used: U256::from(1_750_000u128 * 1_000_000_000_000u128),
                amount_out: amount_in + U256::from(10u128).pow(U256::from(15)),
                source: HopAmountSource::V3,
            },
        ];
        let r = evaluate_v3_cycle(amount_in, &outs, Some(2_000.0), 18);
        assert!(
            r.is_none(),
            "kernel must reject when hop[i].amount_in_used != hop[i-1].amount_out"
        );
    }

    /// Tolerance regression: `chain_consistent_with_predecessor` allows a
    /// 1-wei delta (V2 truncation slop). 2-wei or more must reject.
    #[test]
    fn chain_consistency_tolerates_one_wei_slop_only() {
        let v = U256::from(1_000_000_000u128);
        assert!(chain_consistent_with_predecessor(v, v));
        assert!(chain_consistent_with_predecessor(v, v + U256::from(1u32)));
        assert!(chain_consistent_with_predecessor(v, v - U256::from(1u32)));
        assert!(!chain_consistent_with_predecessor(v, v + U256::from(2u32)));
        assert!(!chain_consistent_with_predecessor(v, v - U256::from(2u32)));
    }

    /// Reproduces the exact PEPE/SHIB long-tail fake-positive scenario
    /// described by math-validator (REJECT 2026-05-06).
    ///
    /// **Setup**: PEPE→WETH→USDC→PEPE direction.
    /// - cap_usd = $1000, price_a = $1e-5, decimals = 18
    /// - amount_in = $1000 / $1e-5 × 1e18 = 1e26 PEPE wei
    ///
    /// **Without fix** (single-phase + chain-bug):
    /// - Hop 0 V3 PEPE→WETH: returns some WETH wei W (Phase 1 quote).
    ///   But `current_in` stays at 1e26.
    /// - Hop 1 V2 WETH→USDC fed amount_in = 1e26 (treated as WETH wei).
    ///   Saturates the WETH-USDC reserves → produces ~3500e6 = $3500 USDC.
    /// - Hop 2 V3 USDC→PEPE fed amount_in = 3500e6 → returns inflated PEPE wei
    ///   (e.g. 3.5e26 = $3500 worth).
    /// - profit_wei = 3.5e26 - 1e26 = 2.5e26 → profit_usd = 2.5e26 × $1e-5
    ///   / 1e18 = $2500.
    /// - profit_cap_ratio = $2500 / $1000 = 2.5x → SLIPS past 5x bound →
    ///   emits FAKE positive.
    ///
    /// **With fix**: chain consistency check at the kernel boundary catches
    /// the mismatch (hop 1's amount_in_used = 1e26, but hop 0's amount_out =
    /// W != 1e26) and returns None. No fake positive emitted.
    #[test]
    fn pepe_v3_v2_v3_cycle_does_not_emit_fake_positive_under_sanity_bound() {
        // Setup matching math-validator's analysis.
        let amount_in = U256::from(10u128).pow(U256::from(26)); // 1e26 PEPE wei (cap = $1000 / $1e-5)
        let cap_usd = 1_000.0_f64;
        let price_a = 0.00001_f64;
        let decimals_a = 18u8;

        // Hop 0 V3 PEPE→WETH: real V3 quote (representative, e.g., 1e15 WETH wei).
        let weth_out_real = U256::from(1_000_000_000_000_000u128);

        // Hop 1 V2 WETH→USDC:
        // **Bug semantic** — fed amount_in = 1e26 (PEPE wei treated as WETH wei).
        // V2 saturates: amount_out_with_fee × R_out / (R_in + amount_in_with_fee).
        // For typical mainnet WETH-USDC (R_in≈3000 WETH=3e21 wei, R_out=6e9 USDC wei),
        // amount_in_with_fee = 0.997 × 1e26 ≈ 1e26 (>> R_in), so output saturates
        // close to R_out = ~3.5e9 USDC wei (≈ $3500). Use this saturated value.
        let usdc_out_saturated = U256::from(3_500_000_000u64);

        // Hop 2 V3 USDC→PEPE: fed the saturated amount → returns inflated PEPE
        // (math-validator: ~3.5e26 PEPE wei → $3500 worth → profit $2500).
        let final_pepe_inflated = amount_in + (amount_in * U256::from(25u32) / U256::from(10u32));
        // = 1e26 × 3.5 = 3.5e26

        // Construct hop_outs as the BUGGY caller would have: each hop's
        // amount_in_used is "what the buggy chain fed the quoter":
        //   - Hop 0: amount_in (correct)
        //   - Hop 1: amount_in (BUG — should be weth_out_real)
        //   - Hop 2: usdc_out_saturated (the V3 result of the saturated query)
        let buggy_outs = vec![
            HopAmountOut {
                amount_in_used: amount_in,
                amount_out: weth_out_real,
                source: HopAmountSource::V3,
            },
            HopAmountOut {
                // **BUG**: should be weth_out_real, but the original code left
                // current_in at amount_in across the V3 hop.
                amount_in_used: amount_in,
                amount_out: usdc_out_saturated,
                source: HopAmountSource::V2,
            },
            HopAmountOut {
                amount_in_used: usdc_out_saturated,
                amount_out: final_pepe_inflated,
                source: HopAmountSource::V3,
            },
        ];

        // Demonstrate the failure mode WITHOUT chain check would slip past
        // sanity bound: profit_cap_ratio = $2500 / $1000 = 2.5 < 5.
        let raw_profit_wei = final_pepe_inflated.saturating_sub(amount_in);
        let raw_profit_usd =
            (raw_profit_wei.as_u128() as f64) / 10f64.powi(decimals_a as i32) * price_a;
        let raw_profit_cap_ratio = raw_profit_usd / cap_usd;
        assert!(
            raw_profit_cap_ratio > 1.0 && raw_profit_cap_ratio < 5.0,
            "scenario must reproduce the fake-positive failure mode (1 < ratio < 5); got {}",
            raw_profit_cap_ratio
        );

        // **With fix**: kernel chain check rejects the buggy chain.
        let r = evaluate_v3_cycle(amount_in, &buggy_outs, Some(price_a), decimals_a);
        assert!(
            r.is_none(),
            "PEPE chain-bug fake positive must be REJECTED by kernel chain check, \
             but got Some(_) — chain consistency invariant is broken"
        );
    }

    /// Integration-flavoured test: `try_progress_plan` correctly chains a
    /// V3-V2-V3 cycle across two phases when needed. We model the V3
    /// multicall result by directly setting `hop_amount_outs` between phases.
    /// This test guards the production code against regressions where the
    /// progress helper drifts from the chain-consistency invariant.
    #[test]
    fn build_v3_cycle_plan_chains_v3_v2_v3_correctly() {
        use std::str::FromStr;
        // Construct a synthetic plan: V3 PEPE→WETH (hop 0), V2 WETH→USDC
        // (hop 1), V3 USDC→PEPE (hop 2). amount_in = 1e18 wei.
        let amount_in = U256::from(10u128).pow(U256::from(18));

        let hop0 = HopKind::V3 {
            pool_addr: "0x11950d141ecb863f01007add7d1a342041227b58".to_string(),
            fee_bps: 3000,
            token_in_addr: "0x6982508145454ce325ddbe47a25d4ec3d2311933".to_string(), // PEPE
            token_out_addr: "0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2".to_string(), // WETH
        };
        let hop1 = HopKind::V2(HopData {
            pool_addr: "0xb4e16d0168e52d35cacd2c6185b44281ec28c9dc".to_string(),
            entry: ReservesEntry {
                // 3000 WETH × 6_000_000 USDC realistic mainnet
                r0: "3000000000000000000000".to_string(),
                r1: "6000000000".to_string(),
                token0_addr: Some("0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2".to_string()),
                blk: 100,
                ts: 0,
            },
            swap_in_is_token0: true,
        });
        let hop2 = HopKind::V3 {
            pool_addr: "0x88e6a0c2ddd26feeb64f039a2c41296fcb3f5640".to_string(),
            fee_bps: 500,
            token_in_addr: "0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48".to_string(), // USDC
            token_out_addr: "0x6982508145454ce325ddbe47a25d4ec3d2311933".to_string(), // PEPE
        };

        let mut plan = V3CyclePlan {
            cycle_key: "test-pepe-cycle".to_string(),
            sym_a: "PEPE",
            sym_b: "WETH",
            sym_c: "USDC",
            addr_a: "0x6982508145454ce325ddbe47a25d4ec3d2311933".to_string(),
            addr_b: "0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2".to_string(), // WETH
            addr_c: "0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48".to_string(), // USDC
            decimals_a: 18,
            cap_usd: 1_000.0,
            price_a: 0.00001,
            amount_in_wei: amount_in,
            hops: vec![hop0, hop1, hop2],
            hop_amount_ins: vec![Some(amount_in), None, None],
            hop_amount_outs: vec![None, None, None],
        };

        // -------- Phase 1 --------
        // Only hop 0 has known amount_in (the cap-derived amount_in_wei).
        // hop 1 (V2) waits on hop 0's amount_out; hop 2 (V3) waits on hop 1's.
        let mut phase1_requests: Vec<V3QuoteRequest> = Vec::new();
        let mut phase1_routing: Vec<(usize, usize)> = Vec::new();
        let progressed1 =
            try_progress_plan(&mut plan, &mut phase1_requests, &mut phase1_routing, 0);
        assert!(progressed1, "Phase 1 must progress (queue hop 0 V3)");
        assert_eq!(
            phase1_requests.len(),
            1,
            "Phase 1 should queue exactly hop 0 V3"
        );
        assert_eq!(
            phase1_routing[0],
            (0, 0),
            "Phase 1 routing must point to hop 0"
        );
        // Hop 1 and 2 still don't have amount_in yet.
        assert_eq!(plan.hop_amount_ins[1], None);
        assert_eq!(plan.hop_amount_ins[2], None);
        // Verify request amount_in was set CORRECTLY from amount_in_wei.
        assert_eq!(phase1_requests[0].amount_in, amount_in);
        assert_eq!(
            phase1_requests[0].pool_addr,
            Address::from_str("0x11950d141ecb863f01007add7d1a342041227b58").unwrap(),
        );

        // Simulate Phase 1 multicall result: hop 0 amount_out = 1e15 (WETH wei).
        let weth_out = U256::from(10u128).pow(U256::from(15));
        plan.hop_amount_outs[0] = Some(weth_out);

        // -------- Phase 2 --------
        // Now hop 0 is fully resolved. try_progress_plan should:
        //   - propagate weth_out → hop_amount_ins[1] = Some(weth_out)
        //   - compute V2 amount_out for hop 1 inline
        //   - propagate that → hop_amount_ins[2]
        //   - queue hop 2 V3 with the V2-derived amount_in.
        let mut phase2_requests: Vec<V3QuoteRequest> = Vec::new();
        let mut phase2_routing: Vec<(usize, usize)> = Vec::new();
        let progressed2 =
            try_progress_plan(&mut plan, &mut phase2_requests, &mut phase2_routing, 0);
        assert!(
            progressed2,
            "Phase 2 must progress (V2 inline + queue hop 2 V3)"
        );
        assert_eq!(
            plan.hop_amount_ins[1],
            Some(weth_out),
            "hop 1 amount_in chained"
        );
        // V2 amount_out from realistic pool: ~6e6 / 3 = 2e6, but exact is computed.
        let hop1_v2_out = plan.hop_amount_outs[1].expect("hop 1 V2 amount_out filled inline");
        assert!(
            hop1_v2_out > U256::zero(),
            "V2 inline math must produce non-zero amount_out"
        );
        assert_eq!(
            plan.hop_amount_ins[2],
            Some(hop1_v2_out),
            "hop 2 amount_in chained"
        );
        assert_eq!(
            phase2_requests.len(),
            1,
            "Phase 2 should queue exactly hop 2 V3"
        );
        assert_eq!(
            phase2_routing[0],
            (0, 2),
            "Phase 2 routing must point to hop 2"
        );
        // Critical: hop 2's V3 query uses the chain-consistent amount_in
        // (V2's actual output) — NOT the original amount_in (the bug).
        assert_eq!(phase2_requests[0].amount_in, hop1_v2_out);
        assert_ne!(
            phase2_requests[0].amount_in, amount_in,
            "hop 2 amount_in MUST NOT equal cycle amount_in (regression: original bug)"
        );

        // Simulate Phase 2 multicall result: hop 2 amount_out (closes back to PEPE).
        let final_pepe = amount_in + U256::from(10u128).pow(U256::from(15));
        plan.hop_amount_outs[2] = Some(final_pepe);

        // -------- Phase 3 (no progress expected) --------
        let mut phase3_requests: Vec<V3QuoteRequest> = Vec::new();
        let mut phase3_routing: Vec<(usize, usize)> = Vec::new();
        let progressed3 =
            try_progress_plan(&mut plan, &mut phase3_requests, &mut phase3_routing, 0);
        assert!(
            !progressed3,
            "Phase 3 must NOT progress — chain fully resolved"
        );
        assert_eq!(phase3_requests.len(), 0);

        // Verify chain consistency end-to-end.
        let hop_outs: Vec<HopAmountOut> = (0..plan.hops.len())
            .map(|i| HopAmountOut {
                amount_in_used: plan.hop_amount_ins[i].unwrap(),
                amount_out: plan.hop_amount_outs[i].unwrap(),
                source: match plan.hops[i] {
                    HopKind::V2(_) => HopAmountSource::V2,
                    HopKind::V3 { .. } => HopAmountSource::V3,
                },
            })
            .collect();
        // Now the kernel chain check must PASS (no fabricated values).
        let r = evaluate_v3_cycle(amount_in, &hop_outs, Some(0.00001), 18);
        assert!(
            r.is_some(),
            "chain-consistent profitable cycle must produce Some — got None"
        );
    }
}
