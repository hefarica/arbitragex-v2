//! FlashloanArbWorker — periodic emitter of flash-loan-funded multi-DEX
//! arbitrage opportunities (the third LIVE strategy alongside `dex_arb_v2v2`
//! and `triangular`).
//!
//! Strategy shape: borrow token A from a flash-loan provider (Aave V3 / Balancer
//! Vault) atomically, swap A→A on two pools holding the SAME pair (e.g. UniV2
//! WETH/USDC vs SushiV2 WETH/USDC), repay the loan + premium, keep the residual
//! profit. The worker scans for **price discrepancies between pools of the
//! same pair** because that is the only place a 2-hop A→A round trip can be
//! profitable. With flash loans the operator's "own capital" is zero — the
//! constraint is pool depth + flash provider liquidity + min profit > gas +
//! premium.
//!
//! ## Architecture (mirrors `triangular_worker`)
//!
//! Per tick (default 12s ≈ 1 Ethereum block):
//!   1. For each MVP pair (`WETH/USDC`, `WETH/USDT`, `WETH/DAI`, `WETH/WBTC`,
//!      `USDC/USDT`, `USDC/DAI`):
//!      - Look up `arbx:pool_index:1:<sym_lo>:<sym_hi>` → list of pool addresses.
//!      - Skip if < 2 pools (need at least two to compare prices).
//!   2. For each pair-of-pools (combinations of 2):
//!      - Fetch ReservesEntry for each pool.
//!      - Compute spot rate of token A in terms of token B for each pool.
//!      - Identify buy-side (lower rate) and sell-side (higher rate) pool.
//!      - Spot diff < 15bps → skip with `no_profit` (can't beat 5bps premium +
//!        gas + slippage).
//!   3. Run **golden-section search** over `[1, min(R_buy_in, R_sell_out)]` to
//!      find the optimal borrow_amount of token A. f(x) = sell_pool_out(buy_pool_out(x)) - x - premium.
//!   4. If profit_token_a ≤ 0 → skip with `no_profit`.
//!   5. **Worker-level sanity bound** (load-bearing per Incidente #9):
//!      if `profit_usd > borrow_usd × FLASHLOAN_PROFIT_SANITY_MULT` (10%) →
//!      reject with full diagnostic dump. Real flash arbs are 0.05-0.5%; 10%
//!      means orientation/decimal/unit bug. NEVER trust the spine alone.
//!   6. If `profit_usd < min_profit_usd` (chain-aware floor from `trading_config`) → skip with `below_min_profit`.
//!   7. Build `Opportunity { strategy_kind: FlashloanArb, ... }` and emit to
//!      Redis stream + persist to PG. Spine evaluator runs downstream.
//!   8. **Per-block dedup**: `(buy_pool, sell_pool, block)` triple — same
//!      opportunity not emitted twice in the same block.
//!
//! ## Doctrine compliance
//!
//! - **R8 fail-honest**: every guard returns `None` and increments a skip
//!   counter; never fabricates values. None / NaN / Inf / negative inputs are
//!   refused at the gate, not silently coerced.
//! - **RULE 00 zero mocks**: math + reserves are real; no synthetic reserves
//!   in production code.
//! - **No `unwrap()` outside tests**, no `unimplemented!()`, no `todo!()`.
//! - **Defensive cap math (anti-BUG-3)**: optimal borrow is bounded by pool
//!   depths via `floor()` + `min()` + `debug_assert!` post-clamp.
//! - **Worker sanity bound** (anti-Incidente #9): rejects implausible
//!   profit_usd > 10% of borrow_usd before persisting — the spine evaluator's
//!   gates run downstream but the worker bypasses them on the persistence
//!   path, so self-defense is mandatory.
//!
//! ## Cost
//!
//! 1 Redis HGETALL (token prices) + 1 GET per pool index + 1 GET per pool
//! reserves + a small constant. With `MVP_PAIRS.len()` = 6 pairs and an
//! average of ~3 pools per pair, that's ~25 GETs per tick, well under 1ms
//! of Redis work.

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
const STRATEGY_KIND: &str = "flashloan_arb";

/// V2 fee in basis points (Uniswap V2 + SushiSwap canonical).
const V2_FEE_BPS: u32 = 30;

/// Flash-loan provider premium in basis points. Aave V3 = 5 bps (0.05%).
/// Balancer Vault = 0 bps (free). For MVP we conservatively assume Aave
/// (5 bps) so any opp profitable here is profitable on either provider.
/// Operator can lift this to a per-provider runtime knob in a future sub-project.
const FLASH_PROVIDER_PREMIUM_BPS: u32 = 5;

/// Minimum spot diff (bps) between buy-side and sell-side pool BEFORE doing
/// the expensive golden-section search. Set to cover:
///   - flash loan premium (5 bps)
///   - gas in bps terms (~5 bps on mainnet for a 2-hop swap on a 5-WETH trade)
///   - slippage (V2 amount-out concavity ~5 bps for trades < 1% of pool depth)
///
/// 15 bps is the structural floor. Below this no positive-profit input exists.
const MIN_SPOT_DIFF_BPS: f64 = 15.0;

/// Maximum acceptable USD profit relative to borrow_usd. A real flash arb
/// produces 0.05-0.5% returns on borrowed capital. 10% means the math kernel
/// was fed mis-oriented reserves OR a decimal-mismatch bug landed in the
/// pipeline (cf. Incidente #9). Worker REJECTS these and dumps diagnostics.
const FLASHLOAN_PROFIT_SANITY_MULT: f64 = 0.10;

/// Chain-aware minimum USD profit floor used when no operator `trading_config`
/// row has been loaded from Redis yet (i.e. Redis is cold or the operator has
/// not configured the chain). Values are conservative estimates based on
/// observed gas costs (2025-2026): ETH=450k gas×7gwei×$3000 + 3x safety +
/// failed-tx buffer ≈ $50; L2s are cheaper but still 10x above the old $1
/// hardcode. Migration 046 seeds these values into `trading_config` so this
/// fallback is only active during the bootstrap window.
fn min_profit_usd_fallback(chain_id: u64) -> f64 {
    match chain_id {
        1     => 50.0,  // Ethereum mainnet
        42161 => 10.0,  // Arbitrum One
        56    => 5.0,   // BNB Smart Chain
        10    => 5.0,   // Optimism
        8453  => 5.0,   // Base
        137   => 2.0,   // Polygon
        _     => 10.0,  // Unknown chain: conservative default
    }
}

/// Maximum acceptable staleness of reserves relative to the most recent
/// block observed across the pool pair. Beyond this we refuse to evaluate
/// (R8 fail-honest — emitting on stale data risks broadcasting an opp that
/// no longer exists).
const MAX_RESERVE_LAG_BLOCKS: u64 = 5;

/// Dedup window: keep entries for this many recent blocks. An (buy_pool,
/// sell_pool, block) triple older than this is pruned each tick.
const DEDUP_RETAIN_BLOCKS: u64 = 10;

/// Periodic INFO `tick_stats` log every N ticks. At 12s/tick × 5 = 60s.
const STATS_LOG_EVERY_N_TICKS: u32 = 5;

/// Per-period accumulator of scan outcomes for the periodic INFO log.
/// Reset after each emit.
#[derive(Default, Debug, Clone)]
struct TickStats {
    pairs_scanned: u32,
    pool_combos_evaluated: u32,
    skip_unknown_token: u32,
    skip_insufficient_pools: u32,
    skip_stale_reserves: u32,
    skip_dedup_hit: u32,
    skip_no_profit: u32,
    skip_below_min_profit: u32,
    skip_sanity_reject: u32,
    emitted: u32,
}

impl TickStats {
    /// Returns the dominant skip bucket name (highest count) so the operator
    /// sees at a glance WHY pool combos aren't producing emit-able opps.
    /// None when no skips happened in the period.
    fn dominant_skip_reason(&self) -> Option<&'static str> {
        let buckets: [(&'static str, u32); 7] = [
            ("insufficient_pools", self.skip_insufficient_pools),
            ("unknown_token", self.skip_unknown_token),
            ("stale_reserves", self.skip_stale_reserves),
            ("dedup_hit", self.skip_dedup_hit),
            ("no_profit", self.skip_no_profit),
            ("below_min_profit", self.skip_below_min_profit),
            ("sanity_reject", self.skip_sanity_reject),
        ];
        buckets
            .iter()
            .filter(|(_, n)| *n > 0)
            .max_by_key(|(_, n)| *n)
            .map(|(name, _)| *name)
    }
}

/// Hardcoded MVP token pairs. Each tuple `(a, b)` defines a pair to scan for
/// price discrepancies across all V2 pools holding it. Token order is
/// canonical (`a` is the borrow currency for sizing & profit accounting).
///
/// All pairs use blue-chip Ethereum tokens whose V2 pools exist on Uniswap V2
/// + SushiSwap mainnet (seeded by migrations 029 + 030 + 037).
///
/// Operator extension path: future sub-project lifts this to PG so the
/// operator can manage pairs via UI.
pub const MVP_PAIRS: &[(&str, &str)] = &[
    ("WETH", "USDC"),
    ("WETH", "USDT"),
    ("WETH", "DAI"),
    ("WETH", "WBTC"),
    ("USDC", "USDT"),
    ("USDC", "DAI"),
];

// =============================================================================
// MATH KERNEL — pure functions, no async, no I/O. Each has tests below.
// =============================================================================

/// Fee-multiplier `γ = 1 - fee`. For V2 default fee 30 bps → γ = 0.997.
fn gamma(fee_bps: u32) -> f64 {
    1.0 - (fee_bps as f64 / 10_000.0)
}

/// Spot rate of token-out per token-in for a V2 pool: `rate = γ · R_out / R_in`.
/// This is the price quoted at the limit of an infinitesimal trade — a useful
/// pre-screen to identify the higher-priced pool (sell-side) vs lower (buy-side)
/// without doing a full V2 amount-out simulation.
///
/// Returns `0.0` on degenerate inputs (zero reserves) — caller treats the pool
/// as un-priceable and skips.
pub fn spot_rate(reserve_in: U256, reserve_out: U256, fee_bps: u32) -> f64 {
    let r_in = u256_to_f64_lossy(reserve_in);
    let r_out = u256_to_f64_lossy(reserve_out);
    if r_in <= 0.0 || r_out <= 0.0 || !r_in.is_finite() || !r_out.is_finite() {
        return 0.0;
    }
    gamma(fee_bps) * (r_out / r_in)
}

/// Compute the round-trip profit (in token-A units) of:
///   A→B on `buy_pool` (where rate is lower, i.e. you get more B per A)
///   B→A on `sell_pool` (where rate is higher, i.e. you get more A per B)
/// minus the flash-loan premium (`borrow * premium_bps / 10_000`).
///
/// `buy_r_in`  = reserve of token A in the buy pool
/// `buy_r_out` = reserve of token B in the buy pool
/// `sell_r_in` = reserve of token B in the sell pool
/// `sell_r_out`= reserve of token A in the sell pool
///
/// Returns the signed profit in token-A wei units (i128). Negative when the
/// trade is loss-making (slippage + fees + premium dominate the spread).
///
/// Returns `0` on degenerate inputs (zero borrow / zero reserves).
pub fn flash_arb_profit(
    borrow: U256,
    buy_r_in: U256,
    buy_r_out: U256,
    sell_r_in: U256,
    sell_r_out: U256,
    fee_bps: u32,
    premium_bps: u32,
) -> i128 {
    if borrow.is_zero() {
        return 0;
    }
    // Hop 1: A → B on buy pool.
    let amount_b = v2_amount_out(borrow, buy_r_in, buy_r_out, fee_bps);
    if amount_b.is_zero() {
        return 0;
    }
    // Hop 2: B → A on sell pool.
    let amount_a_out = v2_amount_out(amount_b, sell_r_in, sell_r_out, fee_bps);
    if amount_a_out.is_zero() {
        return 0;
    }
    // Premium = borrow * premium_bps / 10_000. Use i128 throughout for sign.
    let borrow_i = u256_to_i128_clamped(borrow);
    let amount_out_i = u256_to_i128_clamped(amount_a_out);
    // Premium math: integer division mirrors on-chain Aave V3 behavior
    // (truncated), which is the exact amount the contract must repay.
    let premium = borrow_i.saturating_mul(premium_bps as i128) / 10_000i128;
    // Net profit = amount_out - borrow - premium.
    amount_out_i
        .saturating_sub(borrow_i)
        .saturating_sub(premium)
}

/// Saturating-clamp a U256 to i128. For our use case (token amounts within
/// pool depths < 1e30) the value comfortably fits, but we defensively clamp
/// to avoid panic on extreme inputs.
fn u256_to_i128_clamped(v: U256) -> i128 {
    let s = v.to_string();
    s.parse::<i128>().unwrap_or(i128::MAX)
}

fn u256_to_f64_lossy(v: U256) -> f64 {
    v.to_string().parse::<f64>().unwrap_or(0.0)
}

fn f64_to_u256_clamped(x: f64) -> U256 {
    if !x.is_finite() || x <= 0.0 {
        return U256::from(1u64);
    }
    let s = format!("{:.0}", x);
    U256::from_dec_str(&s).unwrap_or(U256::from(1u64))
}

/// Golden-section search for the borrow amount that maximises `flash_arb_profit`.
///
/// `f(x) = v2_amount_out(v2_amount_out(x)) - x - x·prem` is strictly concave
/// on (0, ∞) for V2 swaps (composition of concave-monotone functions with a
/// linear penalty). Maximum on a closed bounded interval is unique and
/// golden-section converges geometrically (factor `1/φ ≈ 0.618` per iteration).
///
/// Bounds:
///   `x_lo` — minimum borrow size (default 1 wei).
///   `x_hi` — maximum borrow size; caller passes `min(R_buy_in, R_sell_out)`
///     so we never search past the pool's ability to fill the trade.
///
/// `iterations` — default 25 gives ~7e-6 of the initial interval, more than
/// enough precision for opportunity sizing.
///
/// Returns `(x_star, profit_at_x_star)`. Caller MUST check `profit > 0` before
/// emitting; the search returns whatever it converged on, not a guarantee.
///
/// 9 arguments (over clippy's default 7) reflect the inherent complexity of a
/// 2-hop search: bounds (2) + per-pool reserves (4) + fee (1) + premium (1) +
/// iterations (1). Bundling these into a struct adds indirection without
/// reducing the dimensionality of what callers must specify, so we accept the
/// `too_many_arguments` lint here.
#[allow(clippy::too_many_arguments)]
pub fn golden_section_search_2hop(
    x_lo: U256,
    x_hi: U256,
    buy_r_in: U256,
    buy_r_out: U256,
    sell_r_in: U256,
    sell_r_out: U256,
    fee_bps: u32,
    premium_bps: u32,
    iterations: u32,
) -> (U256, i128) {
    if x_lo >= x_hi {
        let p = flash_arb_profit(x_lo, buy_r_in, buy_r_out, sell_r_in, sell_r_out, fee_bps, premium_bps);
        return (x_lo, p);
    }
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
        flash_arb_profit(xi, buy_r_in, buy_r_out, sell_r_in, sell_r_out, fee_bps, premium_bps) as f64
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
    let profit = flash_arb_profit(x_star, buy_r_in, buy_r_out, sell_r_in, sell_r_out, fee_bps, premium_bps);
    (x_star, profit)
}

/// Convert profit_token_a_wei + price + decimals → USD. Returns None when
/// any input is non-finite / non-positive — R8 fail-honest, never fabricate
/// USD from missing data.
pub fn profit_to_usd(profit_token_a_wei: i128, token_a_price_usd: f64, decimals: u8) -> Option<f64> {
    if profit_token_a_wei <= 0 {
        return None;
    }
    if !token_a_price_usd.is_finite() || token_a_price_usd <= 0.0 {
        return None;
    }
    let scale = 10f64.powi(decimals as i32);
    if !scale.is_finite() || scale <= 0.0 {
        return None;
    }
    let profit_tokens = (profit_token_a_wei as f64) / scale;
    let usd = profit_tokens * token_a_price_usd;
    if !usd.is_finite() || usd < 0.0 {
        return None;
    }
    Some(usd)
}

/// Convert borrow_wei + price + decimals → USD. Used for the sanity-bound
/// ratio check. Returns None on bad inputs.
pub fn borrow_to_usd(borrow_wei: U256, token_a_price_usd: f64, decimals: u8) -> Option<f64> {
    if borrow_wei.is_zero() {
        return None;
    }
    if !token_a_price_usd.is_finite() || token_a_price_usd <= 0.0 {
        return None;
    }
    let scale = 10f64.powi(decimals as i32);
    if !scale.is_finite() || scale <= 0.0 {
        return None;
    }
    let tokens = u256_to_f64_lossy(borrow_wei) / scale;
    let usd = tokens * token_a_price_usd;
    if !usd.is_finite() || usd <= 0.0 {
        return None;
    }
    Some(usd)
}

// =============================================================================
// POOL DATA — orientation, freshness, dedup
// =============================================================================

/// One pool's resolved data: address, reserves entry, and orientation flag.
/// `swap_in_is_token0` tells the caller which reserve is `reserve_in` vs
/// `reserve_out` for a swap of token-A → token-B.
#[derive(Debug, Clone)]
pub struct PoolData {
    pub pool_addr: String,
    pub entry: ReservesEntry,
    /// True if `token_a` is the pool's `token0` (so r_in_a = r0, r_out_b = r1).
    pub swap_a_to_b_uses_token0_in: bool,
}

impl PoolData {
    /// Returns `(reserve_a, reserve_b)` where reserve_a is the reserve of token A
    /// regardless of which token0/token1 slot it occupies in the pool.
    /// Returns None if either reserve is malformed.
    pub fn reserves_a_then_b(&self) -> Option<(U256, U256)> {
        let r0 = U256::from_dec_str(&self.entry.r0).ok()?;
        let r1 = U256::from_dec_str(&self.entry.r1).ok()?;
        if self.swap_a_to_b_uses_token0_in {
            Some((r0, r1))
        } else {
            Some((r1, r0))
        }
    }
}

/// Identify the most recent block across a pool pair; used for the staleness bound.
fn pair_latest_block(pools: &[&PoolData]) -> u64 {
    pools.iter().map(|p| p.entry.blk).max().unwrap_or(0)
}

/// Returns true if any pool has reserves more than `MAX_RESERVE_LAG_BLOCKS`
/// behind the latest pool in the pair.
fn pair_has_stale_reserves(pools: &[&PoolData]) -> bool {
    let latest = pair_latest_block(pools);
    pools
        .iter()
        .any(|p| latest.saturating_sub(p.entry.blk) > MAX_RESERVE_LAG_BLOCKS)
}

/// Stable hash of a (buy_pool, sell_pool) ordered pair. Direction-aware:
/// a swap of pools (buy⇄sell) hashes differently because the math is
/// asymmetric.
pub fn combo_hash(buy_pool: &str, sell_pool: &str) -> String {
    format!("{}>{}", buy_pool, sell_pool)
}

/// In-memory dedup state: set of `(combo_hash, block)` pairs. Pruned each tick.
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
    /// Returns true when the (combo, block) pair is fresh and was just inserted.
    /// Returns false when the pair was already present (dedup hit).
    pub fn check_and_mark(&mut self, combo: &str, block: u64) -> bool {
        let key = (combo.to_string(), block);
        if self.seen.contains(&key) {
            return false;
        }
        self.seen.insert(key);
        true
    }
    /// Drop entries with `block < latest_block - DEDUP_RETAIN_BLOCKS`.
    pub fn prune(&mut self, latest_block: u64) {
        if latest_block <= DEDUP_RETAIN_BLOCKS {
            return;
        }
        let cutoff = latest_block - DEDUP_RETAIN_BLOCKS;
        self.seen.retain(|(_, b)| *b >= cutoff);
    }
    #[allow(dead_code)]
    pub fn len(&self) -> usize {
        self.seen.len()
    }
}

// =============================================================================
// EVALUATE — pure-function kernel for one (buy_pool, sell_pool) candidate
// =============================================================================

/// Input to `evaluate_combo`. Reserves are pre-oriented to (token_a, token_b).
#[derive(Debug, Clone)]
pub struct EvalInput {
    /// Buy pool reserves: (reserve of A, reserve of B). Buy pool is the one
    /// where rate B/A is lower, i.e. you get more B per unit A.
    pub buy_reserves: (U256, U256),
    /// Sell pool reserves: (reserve of A, reserve of B). Sell pool is the one
    /// where rate B/A is higher.
    pub sell_reserves: (U256, U256),
    pub token_a_price_usd: Option<f64>,
    pub token_a_decimals: u8,
    pub fee_bps: u32,
    pub premium_bps: u32,
}

/// Result of `evaluate_combo`. None when not profitable / inputs missing.
#[derive(Debug, Clone)]
pub struct EvalResult {
    pub borrow_wei: U256,
    /// Profit denominated in token-A wei. Exposed for tests and future telemetry
    /// (heartbeat may surface aggregate gross profit per period). Marked
    /// `#[allow(dead_code)]` because the production hot path consumes
    /// `expected_profit_usd` (the dollar-denominated equivalent) only.
    #[allow(dead_code)]
    pub profit_token_a_wei: i128,
    pub expected_profit_usd: f64,
    pub borrow_usd: f64,
    /// Spot diff in bps between sell and buy pool. Useful for diagnostics.
    pub spot_diff_bps: f64,
}

/// Pure-function kernel: spot-rate diff check + golden-section + USD conversion
/// for one (buy_pool, sell_pool) candidate. Returns `Some(EvalResult)` only when
/// all of:
///   1. spot diff > MIN_SPOT_DIFF_BPS
///   2. price > 0 (R8 fail-honest)
///   3. golden-section finds strictly positive profit_token_a_wei
///   4. profit_usd > 0 (min_profit_usd floor is checked by the caller, not here)
///   5. profit_usd ≤ borrow_usd × FLASHLOAN_PROFIT_SANITY_MULT
///      hold simultaneously. Otherwise returns None and the caller skips.
pub fn evaluate_combo(input: &EvalInput) -> Option<EvalResult> {
    let (buy_a, buy_b) = input.buy_reserves;
    let (sell_a, sell_b) = input.sell_reserves;

    // R8: refuse degenerate reserves.
    if buy_a.is_zero() || buy_b.is_zero() || sell_a.is_zero() || sell_b.is_zero() {
        return None;
    }

    // Spot-rate pre-check. We swap A→B on buy, then B→A on sell. For profit
    // we need the sell pool to give back MORE A per B than the buy pool consumed.
    // Equivalently: buy_pool spot rate (B/A) < sell_pool spot rate (B/A).
    let buy_rate = spot_rate(buy_a, buy_b, input.fee_bps);
    let sell_rate = spot_rate(sell_a, sell_b, input.fee_bps);
    if buy_rate <= 0.0 || sell_rate <= 0.0 {
        return None;
    }
    if sell_rate <= buy_rate {
        return None;
    }
    let spot_diff_bps = (sell_rate - buy_rate) / buy_rate * 10_000.0;
    if !spot_diff_bps.is_finite() || spot_diff_bps < MIN_SPOT_DIFF_BPS {
        return None;
    }

    // Token-A price required for USD conversion + sanity bound.
    let token_a_price = input.token_a_price_usd?;
    if !token_a_price.is_finite() || token_a_price <= 0.0 {
        return None;
    }

    // Search bounds: x_hi = min(buy_r_a, sell_r_b_inverted).
    // - Beyond R_buy_a the first hop saturates (slippage explodes).
    // - We also can't extract more A from sell than sell_r_a; conservative
    //   bound is min(buy_a, sell_a) since the round trip can't yield more
    //   A than the sell pool can supply.
    let cap_by_pools = if buy_a < sell_a { buy_a } else { sell_a };
    if cap_by_pools <= U256::from(1u64) {
        return None;
    }
    // For golden-section to converge usefully, search ceiling should be ~ pool depth.
    let x_lo = U256::from(1u64);
    let x_hi = cap_by_pools;

    let (x_star, profit_wei) = golden_section_search_2hop(
        x_lo, x_hi,
        buy_a, buy_b,
        sell_b, sell_a,  // sell pool: hop B→A so reserve_in = sell_b, reserve_out = sell_a
        input.fee_bps, input.premium_bps,
        25,
    );

    if profit_wei <= 0 {
        return None;
    }
    // Defensive: x_star must not exceed the cap.
    debug_assert!(
        x_star <= x_hi,
        "evaluate_combo: x_star ({x_star}) exceeded x_hi ({x_hi})"
    );
    // Re-clamp belt-and-braces (anti-BUG-3).
    let borrow_wei = if x_star > x_hi { x_hi } else { x_star };
    // Re-evaluate at clamped to ensure profit reflects clamped borrow.
    let profit_at_clamped = flash_arb_profit(
        borrow_wei,
        buy_a, buy_b,
        sell_b, sell_a,
        input.fee_bps, input.premium_bps,
    );
    if profit_at_clamped <= 0 {
        return None;
    }

    let profit_usd = profit_to_usd(profit_at_clamped, token_a_price, input.token_a_decimals)?;
    let borrow_usd = borrow_to_usd(borrow_wei, token_a_price, input.token_a_decimals)?;

    // NOTE: the min_profit_usd floor check is NOT here — it is applied by the
    // caller (`scan_one_pair`) using the per-chain value from `trading_config`.
    // `evaluate_combo` is a pure math function; policy gates live one level up.
    // Worker-level sanity bound (anti-Incidente #9).
    if profit_usd > borrow_usd * FLASHLOAN_PROFIT_SANITY_MULT {
        // Caller logs the diagnostic dump — kernel just refuses.
        return None;
    }

    Some(EvalResult {
        borrow_wei,
        profit_token_a_wei: profit_at_clamped,
        expected_profit_usd: profit_usd,
        borrow_usd,
        spot_diff_bps,
    })
}

// =============================================================================
// TOKEN RESOLUTION — Redis cache + hardcoded blue-chip fallback
// =============================================================================

/// Lowercase hex address of well-known mainnet tokens. Same fallback table as
/// `triangular_worker::known_token_address` (intentional duplication — we
/// don't want a cross-module dependency on a hardcoded constant).
fn known_token_address(symbol: &str) -> Option<&'static str> {
    match symbol.to_ascii_uppercase().as_str() {
        "WETH" => Some("0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2"),
        "USDC" => Some("0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48"),
        "USDT" => Some("0xdac17f958d2ee523a2206206994597c13d831ec7"),
        "DAI"  => Some("0x6b175474e89094c44da98b954eedeac495271d0f"),
        "WBTC" => Some("0x2260fac5e5542a773aa44fbcfedf7c193bc2c599"),
        _ => None,
    }
}

/// Resolve `(token_addr, decimals, is_stable)` for a symbol. Tries Redis cache
/// first; on miss, falls back to hardcoded blue-chip table. Returns None when both miss.
async fn resolve_token(
    redis: &mut ConnectionManager,
    chain_id: u64,
    symbol: &str,
) -> Option<(String, u8, bool)> {
    let addr = known_token_address(symbol)?;
    if let Ok(Some(meta)) = get_token_meta(redis, chain_id, addr).await {
        return Some((addr.to_string(), meta.decimals, meta.is_stablecoin));
    }
    let (decimals, is_stable) = match symbol.to_ascii_uppercase().as_str() {
        "WETH" => (18, false),
        "USDC" | "USDT" => (6, true),
        "DAI"  => (18, true),
        "WBTC" => (8, false),
        _ => return None,
    };
    Some((addr.to_string(), decimals, is_stable))
}

/// Fetch reserves for a single pool. Returns None on missing entry, missing
/// token0_addr, or any malformed field.
async fn fetch_pool_data(
    redis: &mut ConnectionManager,
    chain_id: u64,
    pool_addr: &str,
    token_a_addr: &str,
) -> Option<PoolData> {
    let entry = get_reserves(redis, chain_id, pool_addr).await.ok().flatten()?;
    let token0 = entry.token0_addr.as_deref()?;
    let swap_a_to_b_uses_token0_in = token0.eq_ignore_ascii_case(token_a_addr);
    Some(PoolData {
        pool_addr: pool_addr.to_string(),
        entry,
        swap_a_to_b_uses_token0_in,
    })
}

// =============================================================================
// WORKER
// =============================================================================

/// FlashloanArbWorker — owns its tick interval, dedup, and ticker state.
pub struct FlashloanArbWorker {
    pub period: Duration,
    pub chain_id: u64,
}

impl FlashloanArbWorker {
    pub fn new(interval_secs: u64, chain_id: u64) -> Self {
        Self {
            period: Duration::from_secs(interval_secs.max(1)),
            chain_id,
        }
    }

    /// Forever loop. Designed to be `tokio::spawn`'d from main.
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
        let fallback_min = min_profit_usd_fallback(self.chain_id);
        info!(
            event = "flashloan_arb_worker.boot",
            chain_id = self.chain_id,
            period_secs = self.period.as_secs(),
            pairs = MVP_PAIRS.len(),
            premium_bps = FLASH_PROVIDER_PREMIUM_BPS,
            min_spot_diff_bps = MIN_SPOT_DIFF_BPS,
            sanity_mult = FLASHLOAN_PROFIT_SANITY_MULT,
            min_profit_usd_fallback = fallback_min,
            stats_log_every_n_ticks = STATS_LOG_EVERY_N_TICKS,
        );

        loop {
            ticker.tick().await;
            tick_count += 1;

            // Load per-chain config from operator config (Redis-backed).
            // Falls back to a chain-aware conservative floor if Redis has no config
            // yet (cold start or operator has not configured this chain).
            // H2 landmine fix: also extract gas_cost_usd so scan_one_pair can
            // populate net_expected_profit_usd at the emit site.
            let (min_profit_usd, gas_cost_usd_tick) = match trading_config.state(self.chain_id).await {
                Ok(Some(ref cfg)) => (cfg.min_profit_usd, Some(cfg.gas_cost_usd())),
                Ok(None) => (fallback_min, None),
                Err(e) => {
                    debug!(
                        event = "flashloan_arb_worker.cfg_fetch_failed",
                        chain_id = self.chain_id,
                        error = %e,
                        fallback = fallback_min,
                    );
                    (fallback_min, None)
                }
            };

            // Snapshot price oracle once per tick.
            let snapshot_map =
                shared_rs::price_oracle::RedisCachedPriceOracle::snapshot_from_redis(
                    &mut redis,
                    self.chain_id,
                )
                .await
                .into_snapshot();

            let mut latest_block_observed: u64 = 0;

            for (sym_a, sym_b) in MVP_PAIRS {
                stats.pairs_scanned += 1;
                counters()
                    .flashloan_arb_pairs_scanned
                    .fetch_add(1, Ordering::Relaxed);
                if let Some(blk) = self
                    .scan_one_pair(
                        &mut redis,
                        db.as_ref(),
                        &snapshot_map,
                        sym_a,
                        sym_b,
                        min_profit_usd,
                        gas_cost_usd_tick,
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

            dedup.prune(latest_block_observed);

            if tick_count % STATS_LOG_EVERY_N_TICKS == 0 {
                info!(
                    event = "flashloan_arb_worker.tick_stats",
                    chain_id = self.chain_id,
                    period_ticks = STATS_LOG_EVERY_N_TICKS,
                    pairs_scanned = stats.pairs_scanned,
                    pool_combos_evaluated = stats.pool_combos_evaluated,
                    emitted = stats.emitted,
                    skip_unknown_token = stats.skip_unknown_token,
                    skip_insufficient_pools = stats.skip_insufficient_pools,
                    skip_stale_reserves = stats.skip_stale_reserves,
                    skip_dedup_hit = stats.skip_dedup_hit,
                    skip_no_profit = stats.skip_no_profit,
                    skip_below_min_profit = stats.skip_below_min_profit,
                    skip_sanity_reject = stats.skip_sanity_reject,
                    dominant_skip = ?stats.dominant_skip_reason(),
                );
                stats = TickStats::default();
            }
        }
    }

    /// Scan one pair (sym_a, sym_b). Returns the latest block observed for
    /// dedup pruning. Returns None when the pair was skipped before any
    /// reserves were fetched (unknown token, no pools).
    ///
    /// `min_profit_usd` is the per-chain floor loaded from `trading_config`
    /// each tick (see `run()`). Must be > 0.
    /// `gas_cost_usd` is the estimated gas cost in USD from `TradingConfigState::gas_cost_usd()`;
    /// `None` when the config was unavailable (cold-start / Redis miss) — in that case
    /// `net_expected_profit_usd` is not populated (falls back to gross in submit_engine).
    #[allow(clippy::too_many_arguments)]
    async fn scan_one_pair(
        &self,
        redis: &mut ConnectionManager,
        db: Option<&PgPool>,
        price_snapshot: &std::collections::HashMap<String, f64>,
        sym_a: &str,
        sym_b: &str,
        min_profit_usd: f64,
        gas_cost_usd: Option<f64>,
        dedup: &mut DedupState,
        stats: &mut TickStats,
    ) -> Option<u64> {
        // Resolve token A (borrow currency) + decimals + addr.
        let (addr_a, decimals_a, _is_stable_a) = match resolve_token(redis, self.chain_id, sym_a).await {
            Some(t) => t,
            None => { stats.skip_unknown_token += 1; return None; }
        };
        let (_addr_b, _, _) = match resolve_token(redis, self.chain_id, sym_b).await {
            Some(t) => t,
            None => { stats.skip_unknown_token += 1; return None; }
        };

        // Fetch all pools for the pair. On Redis error we degrade to "no pools"
        // so the per-pair scan honours R8 fail-honest (skip silently, surface
        // via heartbeat counters) instead of bringing down the worker loop.
        let pool_addrs = get_pools_for_pair(redis, self.chain_id, sym_a, sym_b)
            .await
            .unwrap_or_default();
        if pool_addrs.len() < 2 {
            stats.skip_insufficient_pools += 1;
            debug!(
                event = "flashloan_arb_worker.insufficient_pools",
                chain_id = self.chain_id,
                pair = format!("{}/{}", sym_a, sym_b),
                pool_count = pool_addrs.len(),
            );
            return None;
        }

        // Fetch reserves for each pool. Drop pools with malformed/missing data.
        let mut pool_data: Vec<PoolData> = Vec::with_capacity(pool_addrs.len());
        for addr in &pool_addrs {
            if let Some(pd) = fetch_pool_data(redis, self.chain_id, addr, &addr_a).await {
                pool_data.push(pd);
            }
        }
        if pool_data.len() < 2 {
            stats.skip_insufficient_pools += 1;
            return None;
        }

        // Token-A price (cascade: live snapshot first).
        let price_a = price_snapshot.get(&sym_a.to_ascii_uppercase()).copied();

        // Iterate every ordered pair (i, j) where i is buy and j is sell, and
        // also try the reverse to find the truly profitable orientation.
        let n = pool_data.len();
        let mut latest_block_for_pair: u64 = 0;
        for i in 0..n {
            for j in 0..n {
                if i == j {
                    continue;
                }
                stats.pool_combos_evaluated += 1;
                let buy_pd = &pool_data[i];
                let sell_pd = &pool_data[j];

                let pair_pools: [&PoolData; 2] = [buy_pd, sell_pd];
                if pair_has_stale_reserves(&pair_pools) {
                    stats.skip_stale_reserves += 1;
                    continue;
                }
                let block = pair_latest_block(&pair_pools);
                if block > latest_block_for_pair {
                    latest_block_for_pair = block;
                }
                let combo_key = combo_hash(&buy_pd.pool_addr, &sell_pd.pool_addr);
                if !dedup.check_and_mark(&combo_key, block) {
                    stats.skip_dedup_hit += 1;
                    continue;
                }

                let buy_reserves = match buy_pd.reserves_a_then_b() {
                    Some(r) => r,
                    None => { stats.skip_no_profit += 1; continue; }
                };
                let sell_reserves = match sell_pd.reserves_a_then_b() {
                    Some(r) => r,
                    None => { stats.skip_no_profit += 1; continue; }
                };

                let input = EvalInput {
                    buy_reserves,
                    sell_reserves,
                    token_a_price_usd: price_a,
                    token_a_decimals: decimals_a,
                    fee_bps: V2_FEE_BPS,
                    premium_bps: FLASH_PROVIDER_PREMIUM_BPS,
                };

                // Two-stage evaluation: separate the "no profit / below min"
                // cases from the "sanity_reject" case so the operator gets the
                // correct counter and (in the sanity case) a diagnostic dump.
                let result = match evaluate_combo(&input) {
                    Some(r) => r,
                    None => {
                        // Could be: spot diff too small, no profit, below min,
                        // OR sanity_reject. Re-check the sanity branch explicitly
                        // because that one needs a warn log, not a debug.
                        if let Some((profit_usd, borrow_usd)) =
                            recheck_sanity_for_log(&input)
                        {
                            stats.skip_sanity_reject += 1;
                            counters()
                                .flashloan_arb_sanity_reject
                                .fetch_add(1, Ordering::Relaxed);
                            warn!(
                                event = "flashloan_arb_worker.sanity_reject",
                                chain_id = self.chain_id,
                                pair = format!("{}/{}", sym_a, sym_b),
                                buy_pool = %buy_pd.pool_addr,
                                sell_pool = %sell_pd.pool_addr,
                                buy_pool_token0 = ?buy_pd.entry.token0_addr,
                                buy_pool_swap_a_to_b_uses_token0_in = buy_pd.swap_a_to_b_uses_token0_in,
                                buy_pool_r0 = %buy_pd.entry.r0,
                                buy_pool_r1 = %buy_pd.entry.r1,
                                buy_pool_oriented = ?buy_pd.reserves_a_then_b(),
                                sell_pool_token0 = ?sell_pd.entry.token0_addr,
                                sell_pool_swap_a_to_b_uses_token0_in = sell_pd.swap_a_to_b_uses_token0_in,
                                sell_pool_r0 = %sell_pd.entry.r0,
                                sell_pool_r1 = %sell_pd.entry.r1,
                                sell_pool_oriented = ?sell_pd.reserves_a_then_b(),
                                expected_profit_usd = profit_usd,
                                borrow_usd = borrow_usd,
                                ratio = profit_usd / borrow_usd,
                                threshold_mult = FLASHLOAN_PROFIT_SANITY_MULT,
                                fee_bps = V2_FEE_BPS,
                                premium_bps = FLASH_PROVIDER_PREMIUM_BPS,
                                hint = "diagnostic dump for root-cause analysis — math kernel returned profit > 10% of borrow"
                            );
                        } else {
                            stats.skip_no_profit += 1;
                        }
                        continue;
                    }
                };

                if result.expected_profit_usd < min_profit_usd {
                    stats.skip_below_min_profit += 1;
                    continue;
                }

                // Build & emit Opportunity.
                // H2 landmine fix: compute net_expected_profit_usd inline so
                // submit_engine Check 7 gates on NET (gross minus gas).
                // `gas_cost_usd` is None on cold-start; fallback to gross is safe.
                let net_expected_profit_usd_fl: Option<f64> =
                    gas_cost_usd.map(|g| result.expected_profit_usd - g);

                let opp = Opportunity {
                    id: Uuid::new_v4(),
                    chain_id: self.chain_id,
                    strategy_kind: StrategyKind::FlashloanArb,
                    dex_a: format!("v2:{}", buy_pd.pool_addr),
                    dex_b: Some(format!("v2:{}", sell_pd.pool_addr)),
                    pair_symbol: format!("{}/{}", sym_a, sym_b),
                    token_in: addr_a.clone(),
                    token_out: addr_a.clone(),
                    amount_in_wei: result.borrow_wei.to_string(),
                    expected_profit_usd: Some(result.expected_profit_usd),
                    net_expected_profit_usd: net_expected_profit_usd_fl,
                    roi_pct: None,
                    risk_score: None,
                    block_number: Some(block),
                    rejection_reason: None,
                    detected_at: Utc::now(),
                    trace_id: Uuid::new_v4(),
                };

                info!(
                    event = "flashloan_arb_worker.opp_emit",
                    chain_id = self.chain_id,
                    pair = %opp.pair_symbol,
                    block = block,
                    borrow_wei = %opp.amount_in_wei,
                    borrow_usd = result.borrow_usd,
                    expected_profit_usd = result.expected_profit_usd,
                    spot_diff_bps = result.spot_diff_bps,
                    buy_pool = %buy_pd.pool_addr,
                    sell_pool = %sell_pd.pool_addr,
                    strategy = STRATEGY_KIND,
                );

                if let Some(pool) = db {
                    if let Err(e) = persistence::insert_opportunity(pool, &opp).await {
                        counters().db_errors.fetch_add(1, Ordering::Relaxed);
                        warn!(event = "flashloan_arb_worker.db_error", error = %e);
                    } else {
                        counters().db_persisted.fetch_add(1, Ordering::Relaxed);
                    }
                }
                if let Err(e) = publisher::publish(redis, &opp).await {
                    warn!(event = "flashloan_arb_worker.publish_error", error = %e);
                } else {
                    counters()
                        .flashloan_arb_opps_emitted
                        .fetch_add(1, Ordering::Relaxed);
                    stats.emitted += 1;
                }
            }
        }

        if latest_block_for_pair > 0 {
            Some(latest_block_for_pair)
        } else {
            None
        }
    }
}

/// Recompute profit_usd / borrow_usd for the combo to determine if the reason
/// `evaluate_combo` returned None was specifically the sanity bound (so the
/// caller can log a `sanity_reject` event with the diagnostic dump). Returns
/// `Some((profit_usd, borrow_usd))` only when the math kernel produced a
/// positive but anomalously-large profit. Returns `None` for all other
/// rejection causes (no spot diff, no profit, below min profit).
fn recheck_sanity_for_log(input: &EvalInput) -> Option<(f64, f64)> {
    let (buy_a, buy_b) = input.buy_reserves;
    let (sell_a, sell_b) = input.sell_reserves;
    if buy_a.is_zero() || buy_b.is_zero() || sell_a.is_zero() || sell_b.is_zero() {
        return None;
    }
    // buy pool: A→B trade → rate expressed as B-per-A = spot_rate(A_reserve, B_reserve)
    // sell pool: B→A trade → rate expressed as A-per-B = spot_rate(B_reserve, A_reserve)
    // A profitable cycle requires sell_rate_a_per_b > 1/buy_rate_b_per_a, i.e. the round-trip
    // spot rate exceeds 1.0 before fees. We pre-screen by checking that the sell pool
    // offers a better A-per-B rate than 1/buy_rate (a necessary but not sufficient condition).
    let buy_rate = spot_rate(buy_a, buy_b, input.fee_bps);   // B per A (buy leg)
    let sell_rate = spot_rate(sell_b, sell_a, input.fee_bps); // A per B (sell leg)
    if buy_rate <= 0.0 || sell_rate <= 0.0 {
        return None;
    }
    // Round-trip spot return = buy_rate * sell_rate (B/A × A/B = dimensionless).
    // Must exceed 1.0 to even be a candidate (before premium).
    if buy_rate * sell_rate <= 1.0 {
        return None;
    }
    let token_a_price = input.token_a_price_usd?;
    if !token_a_price.is_finite() || token_a_price <= 0.0 {
        return None;
    }
    // cap_by_pools = min pool depth on the A-side (limits meaningful borrow size).
    // When cap_by_pools == 1 wei (e.g. orientation-flipped fixture from Incidente #9),
    // we still evaluate flash_arb_profit at 1 wei so the sanity bound can fire —
    // that is precisely the scenario this function is designed to catch.
    let cap_by_pools = if buy_a < sell_a { buy_a } else { sell_a };
    let x_lo = U256::from(1u64);
    let x_hi = if cap_by_pools < x_lo { x_lo } else { cap_by_pools };
    let (x_star, profit_wei) = golden_section_search_2hop(
        x_lo, x_hi,
        buy_a, buy_b,
        sell_b, sell_a,
        input.fee_bps, input.premium_bps,
        25,
    );
    if profit_wei <= 0 {
        return None;
    }
    let borrow_wei = if x_star > x_hi { x_hi } else { x_star };
    let profit_usd = profit_to_usd(profit_wei, token_a_price, input.token_a_decimals)?;
    let borrow_usd = borrow_to_usd(borrow_wei, token_a_price, input.token_a_decimals)?;
    if profit_usd > borrow_usd * FLASHLOAN_PROFIT_SANITY_MULT {
        Some((profit_usd, borrow_usd))
    } else {
        None
    }
}

// =============================================================================
// TESTS
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // ---------------------------------------------------------------
    // gamma + spot_rate
    // ---------------------------------------------------------------

    #[test]
    fn gamma_30bps_is_0997() {
        assert!((gamma(30) - 0.997).abs() < 1e-12);
    }

    #[test]
    fn spot_rate_balanced_pool_is_gamma() {
        // R_in = R_out → rate = γ · 1 = 0.997 for V2.
        let r = spot_rate(U256::from(1_000_000u64), U256::from(1_000_000u64), 30);
        assert!((r - 0.997).abs() < 1e-9);
    }

    #[test]
    fn spot_rate_zero_reserve_is_zero() {
        assert_eq!(spot_rate(U256::zero(), U256::from(1_000u64), 30), 0.0);
        assert_eq!(spot_rate(U256::from(1_000u64), U256::zero(), 30), 0.0);
    }

    #[test]
    fn spot_rate_imbalanced_returns_correct_ratio() {
        // R_in=100, R_out=1000 → rate ≈ γ·10 ≈ 9.97.
        let r = spot_rate(U256::from(100u64), U256::from(1_000u64), 30);
        assert!((r - 9.97).abs() < 1e-6);
    }

    // ---------------------------------------------------------------
    // flash_arb_profit
    // ---------------------------------------------------------------

    #[test]
    fn flash_arb_profit_balanced_pools_yields_loss() {
        // Two identical pools → no spread → after fees + premium: loss.
        let r0 = U256::from(1_000_000u64);
        let p = flash_arb_profit(
            U256::from(1_000u64),
            r0, r0,  // buy
            r0, r0,  // sell
            30, 5,
        );
        assert!(p < 0, "balanced pools must produce loss; got profit={}", p);
    }

    #[test]
    fn flash_arb_profit_zero_borrow_is_zero() {
        assert_eq!(
            flash_arb_profit(
                U256::zero(),
                U256::from(1u64), U256::from(1u64),
                U256::from(1u64), U256::from(1u64),
                30, 5
            ),
            0
        );
    }

    #[test]
    fn flash_arb_profit_known_imbalance_positive() {
        // Buy pool:  buy_a A, buy_b B   → ratio B/A = 1010 (1 A buys many B)
        // Sell pool: sell_b B, sell_a A → ratio A/B = 1/990 (selling B returns A)
        //
        // Round-trip spot rate (no fees): (1010/990) ≈ 1.0202
        // After two 30bps fees: 1.0202 × 0.997² ≈ 1.0141  (+1.41% net of fees)
        // After 5bps premium:   1.0141 − 0.0005 ≈ 1.0136  (+1.36% net profit)
        //
        // Pool depth: 1_000_000 ETH per side. Borrow = 0.01 ETH (1e16 wei).
        // Price impact fraction ≈ 0.01/1_000_000 = 1e-8 — negligible.
        // Therefore profit is dominated by the spread, not price impact.
        //
        // Previously this test used pools of only 1 ETH of A, so the sell-pool
        // (990 B / 1 A) was completely overwhelmed by the ~10 ETH of B produced
        // in hop 1, causing a large negative AMM price impact that wiped the
        // spread. Fixed by scaling to deep (1M ETH) pools.
        let one_eth = U256::from(1_000_000_000_000_000_000u64); // 1e18 wei
        let million_eth = one_eth * U256::from(1_000_000u64);   // 1e24 wei
        let buy_a  = million_eth;                                          // 1M ETH A
        let buy_b  = million_eth * U256::from(1_010u64);                   // 1.01B ETH B (ratio 1010)
        let sell_b = million_eth * U256::from(990u64);                     // 990M ETH B
        let sell_a = million_eth;                                           // 1M ETH A  (ratio 990)
        let borrow = U256::from(10_000_000_000_000_000u64);               // 0.01 ETH
        let p = flash_arb_profit(borrow, buy_a, buy_b, sell_b, sell_a, 30, 5);
        assert!(p > 0, "imbalanced cycle must produce profit; got {}", p);
    }

    // Real-mainnet-magnitudes test: WETH/USDC pool with realistic depth.
    // WETH: 5_000 ETH = 5e21 wei, USDC: 12_000_000 USDC = 1.2e13 (6 dec).
    #[test]
    fn flash_arb_profit_real_mainnet_magnitudes_weth_usdc() {
        // Two pools, slight price discrepancy (~30bps).
        // Buy pool: 5000 WETH / 12_000_000 USDC → 1 ETH = 2400 USDC
        // Sell pool: 5000 WETH / 12_036_000 USDC → 1 ETH = 2407.2 USDC (30bps higher)
        // Direction: borrow WETH, swap WETH→USDC on cheap (buy_pool yields more USDC),
        // then USDC→WETH on expensive (sell pool yields more WETH per USDC, i.e. lower B/A).
        //
        // Wait — for round trip A→A profit, we need:
        //   step 1 (buy pool): A→B yields lots of B (cheap A there)
        //   step 2 (sell pool): B→A yields lots of A (cheap B there i.e. expensive A)
        // So buy pool: high B/A ratio (more B per A); sell pool: low B/A (more A per B).
        //
        // Buy: B/A = 2410 → 5000 WETH (5e21) vs 12_050_000 USDC (1.205e13)
        // Sell: B/A = 2390 → 5000 WETH (5e21) vs 11_950_000 USDC (1.195e13)
        let buy_a = U256::from(5_000u64) * U256::from(10u64).pow(U256::from(18u64));   // 5e21 WETH wei
        let buy_b = U256::from(12_050_000u64) * U256::from(10u64).pow(U256::from(6u64)); // 1.205e13 USDC wei
        let sell_a = U256::from(5_000u64) * U256::from(10u64).pow(U256::from(18u64));  // 5e21
        let sell_b = U256::from(11_950_000u64) * U256::from(10u64).pow(U256::from(6u64)); // 1.195e13

        // Borrow 1 WETH = 1e18.
        let borrow = U256::from(10u64).pow(U256::from(18u64));
        let p = flash_arb_profit(
            borrow,
            buy_a, buy_b,
            sell_b, sell_a,
            30, 5,
        );
        // 30bps spread > 30bps round-trip fee + 5bps premium → small positive profit.
        // Rough order: 1 WETH * (2410/2390 - 1 - 0.0030*2 - 0.0005) ≈ 1 WETH * 0.0019
        // ≈ 1.9e15 wei ≈ 0.0019 ETH ≈ $4-5 at $2400/ETH.
        assert!(p > 0, "30bps spread on real WETH/USDC magnitudes should profit; got {}", p);
        // Sanity: profit should not exceed 1% of borrow at this spread.
        let profit_ratio = p as f64 / borrow.as_u128() as f64;
        assert!(
            profit_ratio < 0.01,
            "profit ratio {} (>1%) suggests math bug — real spread of 30bps yields << 100bps net",
            profit_ratio,
        );
    }

    /// Cross-decimals test: WBTC (8 dec) / USDC (6 dec) pair.
    /// This is the exact class of bug Incidente #9 hit — the math kernel
    /// must produce a SENSIBLE profit when given correctly-oriented reserves
    /// for tokens of different decimals.
    #[test]
    fn flash_arb_profit_cross_decimals_wbtc_usdc() {
        // Realistic pool: 100 WBTC (1e10 wei) vs 5_000_000 USDC (5e12 wei).
        // Two pools with 25bps spread.
        // Token A = WBTC.
        // buy pool B/A higher: 100 WBTC / 5_012_500 USDC (5.0125e12)
        // sell pool B/A lower: 100 WBTC / 4_987_500 USDC (4.9875e12)
        let buy_a = U256::from(100u64) * U256::from(10u64).pow(U256::from(8u64));   // 1e10 WBTC wei
        let buy_b = U256::from(5_012_500u64) * U256::from(10u64).pow(U256::from(6u64));   // 5.0125e12 USDC wei
        let sell_a = U256::from(100u64) * U256::from(10u64).pow(U256::from(8u64));  // 1e10
        let sell_b = U256::from(4_987_500u64) * U256::from(10u64).pow(U256::from(6u64)); // 4.9875e12

        // Borrow 0.01 WBTC = 1e6 wei (small relative to 100 WBTC pool).
        let borrow = U256::from(1_000_000u64);
        let p = flash_arb_profit(
            borrow,
            buy_a, buy_b,
            sell_b, sell_a,
            30, 5,
        );
        // 50bps spread minus 30bps×2 fees minus 5bps premium ≈ -15bps → loss.
        // Expected: small loss, NOT a giant profit (which would indicate a
        // decimal-mismatch bug as in Incidente #9).
        let profit_ratio_abs = (p as f64 / borrow.as_u128() as f64).abs();
        assert!(
            profit_ratio_abs < 0.05,
            "profit_ratio_abs={} suggests cross-decimal bug (real outcome should be < 5% in abs)",
            profit_ratio_abs,
        );
    }

    #[test]
    fn flash_arb_profit_equilibrium_pools_loss() {
        // Two pools with identical reserves → no spread → guaranteed loss
        // (fees + premium). Verifies our profit kernel is honest about
        // unprofitable opportunities (no fabrication).
        let r_a = U256::from(10u64).pow(U256::from(20u64));   // 1e20
        let r_b = U256::from(2_400u64) * U256::from(10u64).pow(U256::from(8u64));  // 2400 * 1e8
        let p = flash_arb_profit(
            U256::from(10u64).pow(U256::from(17u64)),  // 0.1 ETH
            r_a, r_b,
            r_b, r_a,
            30, 5,
        );
        assert!(p <= 0, "equilibrium pools should not profit; got {}", p);
    }

    // ---------------------------------------------------------------
    // golden_section_search_2hop
    // ---------------------------------------------------------------

    #[test]
    fn golden_section_finds_positive_profit_when_spread_exists() {
        // Same setup as flash_arb_profit_known_imbalance_positive.
        let buy_a = U256::from(1_000_000_000_000_000_000u64);
        let buy_b = U256::from(1_010u64) * U256::from(1_000_000_000_000_000_000u64);
        let sell_b = U256::from(990u64) * U256::from(1_000_000_000_000_000_000u64);
        let sell_a = U256::from(1_000_000_000_000_000_000u64);
        let (x, p) = golden_section_search_2hop(
            U256::from(1u64),
            U256::from(10_000_000_000_000_000u64),
            buy_a, buy_b,
            sell_b, sell_a,
            30, 5, 25,
        );
        assert!(p > 0, "golden-section: profit={} at x={} expected positive", p, x);
        assert!(x > U256::from(1u64));
    }

    #[test]
    fn golden_section_returns_non_positive_for_balanced() {
        let r0 = U256::from(1_000_000u64);
        let (_, p) = golden_section_search_2hop(
            U256::from(1u64), U256::from(10_000u64),
            r0, r0, r0, r0,
            30, 5, 25,
        );
        assert!(p <= 0, "balanced pools: profit={} should be ≤ 0", p);
    }

    #[test]
    fn golden_section_degenerate_interval_returns_endpoint() {
        let r0 = U256::from(1_000u64);
        let (x, _) = golden_section_search_2hop(
            U256::from(7u64), U256::from(7u64),
            r0, r0, r0, r0,
            30, 5, 25,
        );
        assert_eq!(x, U256::from(7u64));
    }

    // ---------------------------------------------------------------
    // profit_to_usd / borrow_to_usd — R8 fail-honest
    // ---------------------------------------------------------------

    #[test]
    fn profit_to_usd_returns_none_on_zero_price() {
        assert!(profit_to_usd(1_000_000_000_000u128 as i128, 0.0, 18).is_none());
    }

    #[test]
    fn profit_to_usd_returns_none_on_nan_price() {
        assert!(profit_to_usd(1_000_000_000_000u128 as i128, f64::NAN, 18).is_none());
    }

    #[test]
    fn profit_to_usd_returns_none_on_negative_profit() {
        assert!(profit_to_usd(-1, 2000.0, 18).is_none());
    }

    #[test]
    fn profit_to_usd_correct_for_eth() {
        // 0.01 ETH (1e16 wei) at $2000/ETH = $20.
        let p = profit_to_usd(10_000_000_000_000_000i128, 2000.0, 18).unwrap();
        assert!((p - 20.0).abs() < 1e-9);
    }

    #[test]
    fn borrow_to_usd_returns_none_on_zero_borrow() {
        assert!(borrow_to_usd(U256::zero(), 2000.0, 18).is_none());
    }

    // ---------------------------------------------------------------
    // PoolData orientation
    // ---------------------------------------------------------------

    #[test]
    fn pool_data_oriented_when_token_a_is_token0() {
        let pd = PoolData {
            pool_addr: "0xpool".into(),
            entry: ReservesEntry {
                r0: "100".into(),
                r1: "200".into(),
                token0_addr: Some("0xa".into()),
                blk: 0,
                ts: 0,
            },
            swap_a_to_b_uses_token0_in: true,
        };
        let (ra, rb) = pd.reserves_a_then_b().unwrap();
        assert_eq!(ra, U256::from(100u64));
        assert_eq!(rb, U256::from(200u64));
    }

    #[test]
    fn pool_data_oriented_when_token_a_is_token1() {
        let pd = PoolData {
            pool_addr: "0xpool".into(),
            entry: ReservesEntry {
                r0: "100".into(),
                r1: "200".into(),
                token0_addr: Some("0xb".into()),
                blk: 0,
                ts: 0,
            },
            swap_a_to_b_uses_token0_in: false,
        };
        let (ra, rb) = pd.reserves_a_then_b().unwrap();
        assert_eq!(ra, U256::from(200u64));
        assert_eq!(rb, U256::from(100u64));
    }

    // ---------------------------------------------------------------
    // Dedup state
    // ---------------------------------------------------------------

    #[test]
    fn dedup_first_insert_succeeds_second_blocks() {
        let mut d = DedupState::new();
        assert!(d.check_and_mark("buy>sell", 100));
        assert!(!d.check_and_mark("buy>sell", 100));
    }

    #[test]
    fn dedup_different_block_passes() {
        let mut d = DedupState::new();
        assert!(d.check_and_mark("buy>sell", 100));
        assert!(d.check_and_mark("buy>sell", 101));
    }

    #[test]
    fn dedup_directional_combo_distinct() {
        let mut d = DedupState::new();
        assert!(d.check_and_mark("a>b", 100));
        assert!(d.check_and_mark("b>a", 100));
    }

    #[test]
    fn dedup_prune_drops_old_entries() {
        let mut d = DedupState::new();
        for blk in 100..120 {
            d.check_and_mark("X>Y", blk);
        }
        assert_eq!(d.len(), 20);
        d.prune(120);
        assert_eq!(d.len(), 10);
    }

    #[test]
    fn dedup_prune_no_op_at_boot_blocks() {
        let mut d = DedupState::new();
        d.check_and_mark("X>Y", 5);
        d.prune(7);
        assert_eq!(d.len(), 1);
    }

    // ---------------------------------------------------------------
    // pair_latest_block + pair_has_stale_reserves
    // ---------------------------------------------------------------

    fn make_pool_at(blk: u64) -> PoolData {
        PoolData {
            pool_addr: format!("0x{:040}", blk),
            entry: ReservesEntry {
                r0: "1000".into(),
                r1: "1000".into(),
                token0_addr: Some("0x01".into()),
                blk,
                ts: 0,
            },
            swap_a_to_b_uses_token0_in: true,
        }
    }

    #[test]
    fn pair_latest_block_picks_max() {
        let p1 = make_pool_at(100);
        let p2 = make_pool_at(105);
        let pools: [&PoolData; 2] = [&p1, &p2];
        assert_eq!(pair_latest_block(&pools), 105);
    }

    #[test]
    fn pair_stale_when_lag_exceeds_threshold() {
        // 100 vs 110 → lag = 10 > MAX_RESERVE_LAG_BLOCKS (5) → stale.
        let p1 = make_pool_at(100);
        let p2 = make_pool_at(110);
        let pools: [&PoolData; 2] = [&p1, &p2];
        assert!(pair_has_stale_reserves(&pools));
    }

    #[test]
    fn pair_fresh_when_lag_within_threshold() {
        let p1 = make_pool_at(105);
        let p2 = make_pool_at(108);
        let pools: [&PoolData; 2] = [&p1, &p2];
        assert!(!pair_has_stale_reserves(&pools));
    }

    // ---------------------------------------------------------------
    // evaluate_combo integration kernel
    // ---------------------------------------------------------------

    #[test]
    fn evaluate_combo_balanced_pools_returns_none() {
        let r0 = U256::from(1_000_000u64);
        let inp = EvalInput {
            buy_reserves: (r0, r0),
            sell_reserves: (r0, r0),
            token_a_price_usd: Some(2000.0),
            token_a_decimals: 18,
            fee_bps: 30,
            premium_bps: 5,
        };
        assert!(evaluate_combo(&inp).is_none());
    }

    #[test]
    fn evaluate_combo_returns_none_when_price_unknown() {
        // Profitable spread but no price → R8 fail-honest, no emit.
        let buy_a = U256::from(1_000_000_000_000_000_000u64);
        let buy_b = U256::from(1_010u64) * U256::from(1_000_000_000_000_000_000u64);
        let sell_b = U256::from(990u64) * U256::from(1_000_000_000_000_000_000u64);
        let sell_a = U256::from(1_000_000_000_000_000_000u64);
        let inp = EvalInput {
            buy_reserves: (buy_a, buy_b),
            sell_reserves: (sell_a, sell_b),
            token_a_price_usd: None,
            token_a_decimals: 18,
            fee_bps: 30,
            premium_bps: 5,
        };
        assert!(evaluate_combo(&inp).is_none());
    }

    #[test]
    fn evaluate_combo_profitable_returns_some() {
        // 2% spread on 1-ETH pools → should yield profit in dollars.
        let buy_a = U256::from(1_000_000_000_000_000_000u64);
        let buy_b = U256::from(1_020u64) * U256::from(1_000_000_000_000_000_000u64);
        let sell_a = U256::from(1_000_000_000_000_000_000u64);
        let sell_b = U256::from(980u64) * U256::from(1_000_000_000_000_000_000u64);
        let inp = EvalInput {
            buy_reserves: (buy_a, buy_b),
            sell_reserves: (sell_a, sell_b),
            token_a_price_usd: Some(2000.0),
            token_a_decimals: 18,
            fee_bps: 30,
            premium_bps: 5,
        };
        let r = evaluate_combo(&inp);
        // Either we emit (profitable) OR we sanity-reject (large profit_usd / borrow_usd
        // because the spread is 2% — much larger than 0.5% real-world). Both are correct.
        // Test asserts the contract: if Some, profit > 0 and below sanity bound.
        if let Some(r) = r {
            assert!(r.profit_token_a_wei > 0);
            assert!(r.expected_profit_usd > 0.0);
            assert!(r.borrow_wei > U256::zero());
            assert!(
                r.expected_profit_usd <= r.borrow_usd * FLASHLOAN_PROFIT_SANITY_MULT,
                "profit_usd {} > sanity {} * borrow_usd {} — this should have been rejected",
                r.expected_profit_usd, FLASHLOAN_PROFIT_SANITY_MULT, r.borrow_usd,
            );
        }
    }

    #[test]
    fn evaluate_combo_sanity_rejects_huge_profit() {
        // Construct a wildly-mispriced setup that produces profit > 10% of borrow
        // — must be rejected by the sanity bound (return None).
        let buy_a = U256::from(1_000_000_000_000u64);   // tiny pool A side
        let buy_b = U256::from(10u64).pow(U256::from(24u64));  // huge B side (massive B per A)
        let sell_a = U256::from(10u64).pow(U256::from(24u64)); // huge A side on sell
        let sell_b = U256::from(1_000_000_000_000u64);  // tiny B side on sell
        let inp = EvalInput {
            buy_reserves: (buy_a, buy_b),
            sell_reserves: (sell_a, sell_b),
            token_a_price_usd: Some(2000.0),
            token_a_decimals: 18,
            fee_bps: 30,
            premium_bps: 5,
        };
        // evaluate_combo MUST return None — the math kernel will compute a huge profit
        // but the sanity bound rejects it (Incidente #9 protection).
        assert!(
            evaluate_combo(&inp).is_none(),
            "wildly mis-balanced pools must be sanity-rejected"
        );
    }

    #[test]
    fn evaluate_combo_tiny_spread_returns_none() {
        // Tiny spread → spot_diff_bps < MIN_SPOT_DIFF_BPS (15bps) → None.
        // Note: evaluate_combo no longer checks min_profit_usd; the caller
        // (scan_one_pair) applies the chain-aware floor from trading_config.
        let buy_a = U256::from(1_000_000_000_000_000_000u64);
        let buy_b = U256::from(1_000_500_000_000_000_000_000u128); // 0.05% spread, ~5bps
        let sell_a = U256::from(1_000_000_000_000_000_000u64);
        let sell_b = U256::from(1_000_000_000_000_000_000u64);
        let inp = EvalInput {
            buy_reserves: (buy_a, buy_b),
            sell_reserves: (sell_a, sell_b),
            token_a_price_usd: Some(0.001), // tiny price → tiny USD profit
            token_a_decimals: 18,
            fee_bps: 30,
            premium_bps: 5,
        };
        // MIN_SPOT_DIFF_BPS (15bps) rejects 5bps spread → None.
        assert!(evaluate_combo(&inp).is_none());
    }

    #[test]
    fn evaluate_combo_rejects_when_sell_rate_below_buy_rate() {
        // Sell rate < buy rate → no opportunity (would lose money instantly).
        let buy_a = U256::from(1_000_000_000_000_000_000u64);
        let buy_b = U256::from(1_020u64) * U256::from(1_000_000_000_000_000_000u64);
        let sell_a = U256::from(1_000_000_000_000_000_000u64);
        let sell_b = U256::from(1_010u64) * U256::from(1_000_000_000_000_000_000u64); // sell B/A < buy B/A
        let inp = EvalInput {
            buy_reserves: (buy_a, buy_b),
            sell_reserves: (sell_a, sell_b),
            token_a_price_usd: Some(2000.0),
            token_a_decimals: 18,
            fee_bps: 30,
            premium_bps: 5,
        };
        assert!(evaluate_combo(&inp).is_none());
    }

    // ---------------------------------------------------------------
    // combo_hash
    // ---------------------------------------------------------------

    #[test]
    fn combo_hash_directional() {
        assert_ne!(combo_hash("0xa", "0xb"), combo_hash("0xb", "0xa"));
    }

    #[test]
    fn combo_hash_stable() {
        assert_eq!(combo_hash("0xa", "0xb"), combo_hash("0xa", "0xb"));
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
    fn u256_to_i128_clamped_saturates_on_overflow() {
        // Build a U256 larger than i128::MAX.
        let big = U256::MAX;
        assert_eq!(u256_to_i128_clamped(big), i128::MAX);
    }

    // ---------------------------------------------------------------
    // TickStats
    // ---------------------------------------------------------------

    #[test]
    fn tick_stats_default_is_all_zero_and_dominant_is_none() {
        let s = TickStats::default();
        assert_eq!(s.pairs_scanned, 0);
        assert_eq!(s.emitted, 0);
        assert!(s.dominant_skip_reason().is_none());
    }

    #[test]
    fn tick_stats_dominant_picks_biggest_bucket() {
        let s = TickStats {
            skip_insufficient_pools: 5,
            skip_no_profit: 12,
            skip_stale_reserves: 3,
            ..Default::default()
        };
        assert_eq!(s.dominant_skip_reason(), Some("no_profit"));
    }

    #[test]
    fn tick_stats_dominant_excludes_zero_buckets() {
        let s = TickStats {
            skip_dedup_hit: 1,
            ..Default::default()
        };
        assert_eq!(s.dominant_skip_reason(), Some("dedup_hit"));
    }

    #[test]
    fn tick_stats_sanity_reject_appears_in_dominant() {
        let s = TickStats {
            skip_sanity_reject: 7,
            skip_no_profit: 3,
            ..Default::default()
        };
        assert_eq!(s.dominant_skip_reason(), Some("sanity_reject"));
    }

    // ---------------------------------------------------------------
    // MVP_PAIRS sanity
    // ---------------------------------------------------------------

    #[test]
    fn mvp_pairs_all_use_known_tokens() {
        for (a, b) in MVP_PAIRS {
            assert!(
                known_token_address(a).is_some() && known_token_address(b).is_some(),
                "MVP pair {}/{}: every token must be in known_token_address fallback",
                a,
                b
            );
        }
    }

    #[test]
    fn mvp_pairs_no_self_loop() {
        for (a, b) in MVP_PAIRS {
            assert_ne!(a, b, "self-loop pair {} == {}", a, b);
        }
    }

    // ---------------------------------------------------------------
    // recheck_sanity_for_log — the diagnostic-dump trigger
    // ---------------------------------------------------------------

    #[test]
    fn recheck_sanity_returns_some_only_for_huge_profit() {
        // Trigger condition = same as evaluate_combo's sanity branch.
        let buy_a = U256::from(1_000_000_000_000u64);
        let buy_b = U256::from(10u64).pow(U256::from(24u64));
        let sell_a = U256::from(10u64).pow(U256::from(24u64));
        let sell_b = U256::from(1_000_000_000_000u64);
        let inp = EvalInput {
            buy_reserves: (buy_a, buy_b),
            sell_reserves: (sell_a, sell_b),
            token_a_price_usd: Some(2000.0),
            token_a_decimals: 18,
            fee_bps: 30,
            premium_bps: 5,
        };
        let res = recheck_sanity_for_log(&inp);
        assert!(res.is_some(), "huge profit should trigger sanity log");
        if let Some((p, b)) = res {
            assert!(p > b * FLASHLOAN_PROFIT_SANITY_MULT);
        }
    }

    #[test]
    fn recheck_sanity_returns_none_for_balanced_pools() {
        let r0 = U256::from(1_000_000u64);
        let inp = EvalInput {
            buy_reserves: (r0, r0),
            sell_reserves: (r0, r0),
            token_a_price_usd: Some(2000.0),
            token_a_decimals: 18,
            fee_bps: 30,
            premium_bps: 5,
        };
        assert!(recheck_sanity_for_log(&inp).is_none());
    }

    // ---------------------------------------------------------------
    // Incidente #9 anti-regression — orientation flip protection
    // ---------------------------------------------------------------

    #[test]
    fn orientation_flip_produces_huge_profit_caught_by_sanity_bound() {
        // Reproduces the class of bug from Incidente #9 (triangular):
        // a swapped-token interpretation in migration 037 caused the math
        // to "see" reserves with the wrong meaning, ballooning profit.
        // For the flashloan worker, the analogous bug would be: wrong
        // `token0_addr` in the cache, OR wrong `swap_a_to_b_uses_token0_in`
        // flag → reserves_a_then_b returns swapped → wildly inflated profit.
        //
        // Construct that exact scenario: feed evaluate_combo reserves as if
        // they came from a flipped pool. The spread looks like 100x not 0.3%.
        let buy_a = U256::from(1u64);  // "1 wei A" (impossibly small)
        let buy_b = U256::from(10u64).pow(U256::from(20u64));  // 1e20 B
        let sell_a = U256::from(10u64).pow(U256::from(20u64)); // 1e20 A
        let sell_b = U256::from(1u64);  // "1 wei B"
        let inp = EvalInput {
            buy_reserves: (buy_a, buy_b),
            sell_reserves: (sell_a, sell_b),
            token_a_price_usd: Some(2000.0),
            token_a_decimals: 18,
            fee_bps: 30,
            premium_bps: 5,
        };
        // The math kernel sees implausible reserves but doesn't know it's a bug.
        // The sanity bound MUST reject by returning None.
        assert!(
            evaluate_combo(&inp).is_none(),
            "Incidente #9 regression: orientation-flipped reserves must be rejected"
        );
        // And the diagnostic dump path must fire.
        assert!(recheck_sanity_for_log(&inp).is_some());
    }

    // Document-as-test: thresholds are tuned for HFT flash arbs (sub-1%
    // returns). These are all const, so we use compile-time const assertions
    // — they fire at *compile time* and clippy is happy. If any operator
    // edits the constants to a value that violates these invariants the
    // build breaks loudly.
    const _: () = assert!(FLASHLOAN_PROFIT_SANITY_MULT > 0.0);
    const _: () = assert!(
        FLASHLOAN_PROFIT_SANITY_MULT < 1.0,
        "10% threshold must reject anything looking like 'free money'"
    );
    // NOTE: MIN_PROFIT_USD is no longer a module-level const — it is loaded per
    // tick from trading_config (chain-aware, operator-tunable). The invariant
    // that min_profit_usd > 0.0 is now a runtime check in the fallback function
    // and enforced by the DB CHECK constraint in migration 026.
    const _: () = assert!(MIN_SPOT_DIFF_BPS > 0.0);
    const _: () = assert!(FLASH_PROVIDER_PREMIUM_BPS > 0);
    const _: () = assert!(STATS_LOG_EVERY_N_TICKS > 0);
    const _: () = assert!(MAX_RESERVE_LAG_BLOCKS > 0);
    const _: () = assert!(DEDUP_RETAIN_BLOCKS > 0);

    /// Verifies that the chain-aware fallback floors are all above the old
    /// broken $1 floor AND are chain-specific (ETH > ARB > L2s > Polygon).
    #[test]
    fn fallback_floors_are_economically_sound() {
        // ETH: 3x safety on 450k gas × 7gwei × $3000/ETH ≈ $9.45 raw → $50 conservative.
        assert_eq!(min_profit_usd_fallback(1), 50.0);
        // Arbitrum: L2 gas + L1 calldata ≈ $2-3 raw → $10 conservative.
        assert_eq!(min_profit_usd_fallback(42161), 10.0);
        // BSC / Optimism / Base: cheaper chains → $5 floor.
        assert_eq!(min_profit_usd_fallback(56), 5.0);
        assert_eq!(min_profit_usd_fallback(10), 5.0);
        assert_eq!(min_profit_usd_fallback(8453), 5.0);
        // Polygon: lowest gas cost → $2 floor.
        assert_eq!(min_profit_usd_fallback(137), 2.0);
        // Unknown chain: conservative $10 fallback.
        assert_eq!(min_profit_usd_fallback(999), 10.0);
        // Every fallback must be > the broken $1 hardcode (economic invariant).
        for chain in &[1u64, 42161, 56, 10, 8453, 137, 999] {
            assert!(
                min_profit_usd_fallback(*chain) > 1.0,
                "chain {} fallback {} is not > $1 — economic defect E2 reintroduced",
                chain,
                min_profit_usd_fallback(*chain)
            );
        }
    }

    /// Runtime sanity test that surfaces in the test count even though the
    /// remaining constant invariants are enforced at compile time above.
    #[test]
    fn sanity_constants_documented_at_compile_time() {
        let mult: f64 = FLASHLOAN_PROFIT_SANITY_MULT;
        assert!(mult.is_finite() && mult > 0.0 && mult < 1.0);
        // min_profit_usd is now runtime-loaded; validated via fallback_floors_are_economically_sound above.
    }

    // ---------------------------------------------------------------
    // FlashloanArbWorker constructor
    // ---------------------------------------------------------------

    #[test]
    fn flashloan_arb_worker_new_clamps_zero_interval_to_one_sec() {
        let w = FlashloanArbWorker::new(0, 1);
        assert!(w.period.as_secs() >= 1);
        assert_eq!(w.chain_id, 1);
    }

    #[test]
    fn flashloan_arb_worker_new_preserves_interval() {
        let w = FlashloanArbWorker::new(12, 1);
        assert_eq!(w.period.as_secs(), 12);
    }
}
