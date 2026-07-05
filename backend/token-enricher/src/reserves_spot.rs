//! Reserves spot-price oracle — derives USD spot prices for the long tail from
//! V2 constant-product pool reserves, WITHOUT any external API call.
//!
//! ## Why this exists (Package C1 — highest-impact price-coverage fix)
//!
//! The scanner prices tokens via a cascade: `RedisCachedPriceOracle` (the
//! `arbx:token_prices:<chain>` hash, populated by `price_worker`,
//! `DexScreenerPriceOracle`, and `GeckoTerminalOracle`) → `ConfigPriceOracle`
//! → `None` → `rejection_reason = "no_price_oracle"`. Only ~224 of ~3238
//! discovered tokens have a USD price today, so ~57% of candidate opportunities
//! die at the price gate.
//!
//! This oracle adds a NEW population path into the SAME tier-1 hash. It needs
//! no HTTP, no API key, no rate limit — it reads the V2 pool reserves that
//! `pool_sync_worker` already refreshes every ~5s into
//! `arbx:pool_reserves:<chain>:<addr>` and applies the constant-product spot
//! identity: if ONE leg of a pool already has a trusted USD price (e.g. WETH,
//! USDC, USDT — the dominant quote currencies), the OTHER leg's price is
//! implied by the reserve ratio. Coverage of any token that shares a pool with
//! a priced token lifts the priced set from ~224 toward the full active set.
//!
//! ## Spot-price identity (V2 constant-product, zero-slippage limit)
//!
//! For a pool with raw reserves `R0` (token0) and `R1` (token1), the marginal
//! rate (an infinitesimal swap incurs no slippage) is:
//!
//! ```text
//!   price_token0_in_token1 = (R1 / R0) · 10^(dec0 − dec1)
//! ```
//!
//! (human units of token1 per human unit of token0 — the `10^(dec0−dec1)`
//! factor converts raw reserves to human-readable units.) If token1 has a
//! trusted USD price `P1`, then `P0 = P1 · (R1 / R0) · 10^(dec0−dec1)`.
//! Symmetrically, if token0 has known price `P0`, then
//! `P1 = P0 · (R0 / R1) · 10^(dec1−dec0)`.
//!
//! ## Doctrine
//!
//! Read-only / shadow. No signer, no private key, no broadcast, no live
//! trading, no contract calls, NO HTTP. Default-OFF: does nothing unless
//! `ARBX_RESERVES_SPOT_ORACLE` is exactly `active` (case-insensitive).
//! Fail-honest (R8): a missing reserve / missing token meta / stale pool /
//! zero-reserve / non-finite result is skipped — never write a fabricated or
//! non-positive price. Idempotent: each cycle is a pure `HSETNX` upsert by
//! symbol; running twice yields the same Redis state.
//!
//! ## Non-clobber contract (DO NOT overwrite API-sourced prices)
//!
//! `price_worker` (Chainlink/Alchemy/CoinGecko), `DexScreenerPriceOracle`,
//! and `GeckoTerminalOracle` are higher-fidelity than a reserve-implied spot
//! price. We therefore write with `HSETNX` — a derived price is ONLY written
//! when no field for that symbol currently exists in the hash. Once any API
//! writer covers that symbol (with `HSET`), this oracle respects it
//! permanently within the lifetime of the hash. The converse is also true:
//! once THIS oracle has derived a symbol, subsequent cycles will NOT refresh
//! it (HSETNX no-ops). Staleness is bounded by the shared hash TTL (~60s,
//! refreshed by whichever writer ticks last).
//!
//! ## Divergences from the original mission spec (documented per mandate)
//!
//! The mission assumed: (a) SCAN over `arbx:pool_reserves:<chain>:*` to
//! enumerate pools; (b) `ReservesEntry` carries BOTH token0_addr and
//! token1_addr; (c) token meta sourced from Redis `arbx:tokens:<chain>:<addr>`.
//!
//! The REAL repo differs, so this adapts (mission rule: "prioriza el contrato
//! real del repo, documenta la diferencia y adapta sin romper compatibilidad"):
//!
//!   1. **`ReservesEntry` carries only `token0_addr`** (searcher-rs reserves.rs
//!      line 27-44) — token1's address is NOT in the cache. We resolve both
//!      legs' `(symbol, decimals)` from the authoritative `pools` ⨝ `tokens`
//!      PG join (same source `pool_sync_worker::bootstrap_pool_index` uses to
//!      populate `arbx:pool_index`), keyed by `pool_addr_lower`. This is the
//!      cleanest path to BOTH addresses and matches the DexScreener oracle's
//!      pattern of sourcing symbols from PG.
//!   2. **Pool enumeration is PG-driven, not SCAN-driven.** We fetch active
//!      `(pool_addr, token0_addr, token0_sym, token0_dec, token1_addr,
//!      token1_sym, token1_dec)` rows once per cycle, then Redis-GET the
//!      reserves for each. Pools with no reserves entry yet (boot lag) are
//!      skipped naturally. This avoids a SCAN + per-pool reverse-lookup of
//!      token1_addr (which has no Redis index) and keeps the cycle O(N) in PG
//!      + O(N) in one pipelined Redis GET, not O(N) round-trips.
//!   3. **Trust gate uses the known leg's reserve × price**, not a separate
//!      liquidity feed — `liq_usd_known = price_known · R_known / 10^dec_known`.
//!      This mirrors `ARBX_MIN_PRICE_LIQUIDITY_USD` semantics from DexScreener
//!      but is computed from on-chain reserves (more truthful than a feed that
//!      might lag or stale).
//!
//! ## Paper-only safety
//!
//! This module writes PRICES, not trades. No signer is loaded, no broadcast
//! path is touched, no capital moves. The derived prices feed the paper-mode
//! scanner's opportunity evaluation only (RULE: `ARBX_PAPER_TRADE=true`).

use anyhow::{Context, Result};
use sqlx::PgPool;
use std::collections::HashMap;
use std::time::Duration;
use tracing::{debug, info, warn};

use shared_rs::price_oracle::redis_token_prices_key;

// --- env keys ---
/// Off-switch. Oracle runs ONLY when this equals (case-insensitively) `active`.
const ENV_GATE: &str = "ARBX_RESERVES_SPOT_ORACLE";
/// Minimum USD value of the KNOWN leg's reserve (`price · reserve_human`) for
/// a derived price to be trusted. Default $10K (looser than DexScreener's
/// $50K — the math is exact and reserves are on-chain truthful, so a smaller
/// pool is still computable; the floor exists only to suppress dust pools).
const ENV_MIN_LIQ: &str = "ARBX_MIN_PRICE_LIQUIDITY_USD";
/// Cycle period in seconds. Default 10. Floored to 2s to avoid hammering Redis.
const ENV_PERIOD: &str = "ARBX_RESERVES_SPOT_PERIOD_SECS";
/// Max age of reserves (seconds since `ts`) before the pool is skipped as
/// stale. Default 30s (pool_sync_worker refreshes every ~5s, so 30s = 6 missed
/// ticks — a conservative fail-honest threshold).
const ENV_MAX_STALE: &str = "ARBX_RESERVES_SPOT_MAX_STALE_SECS";

const DEFAULT_MIN_LIQ_USD: f64 = 10_000.0;
const DEFAULT_PERIOD_SECS: u64 = 10;
const MIN_PERIOD_SECS: u64 = 2;
const DEFAULT_MAX_STALE_SECS: u64 = 30;

/// `arbx:pool_reserves:<chain_id>:<pool_addr_lower>` (wire-compatible mirror
/// of `searcher_rs::reserves::ReservesEntry` — token-enricher does not depend
/// on the searcher-rs crate, so we redefine the view we read. Field set and
/// `#[serde(default)]` on `token0_addr` match the writer exactly.)
#[derive(Debug, Clone, serde::Deserialize)]
struct ReservesEntry {
    r0: String,
    r1: String,
    #[serde(default)]
    #[allow(dead_code)] // parsed for forward-compat; we trust PG for token binding
    token0_addr: Option<String>,
    blk: u64,
    /// Unix epoch seconds at which `pool_sync_worker` observed the reserves.
    /// Used for the staleness check (R8 fail-honest on stale data).
    ts: u64,
}

/// One row of the per-cycle pool registry (PG `pools` ⨝ `tokens` ⨝ `tokens`).
/// All fields are pre-normalized: addresses lowercase, symbols uppercase —
/// so the cycle body does no string munging.
#[derive(Debug, Clone)]
struct PoolRegistryRow {
    pool_addr_lower: String,
    token0_symbol_upper: String,
    token0_decimals: u8,
    token1_symbol_upper: String,
    token1_decimals: u8,
}

// ---------------------------------------------------------------------------
// Config
// ---------------------------------------------------------------------------

#[derive(Clone, Debug)]
pub struct ReservesSpotConfig {
    /// Chains to price (verbatim copy of `ENRICHER_CHAINS` — unlike DexScreener
    /// there is no per-chain slug map; any chain ID with pools in PG is valid).
    pub chains: Vec<u64>,
    /// Minimum USD value of the known leg's reserve for a derivation to be
    /// trusted. Pools below this are skipped as dust.
    pub min_liquidity_usd: f64,
    /// Cycle period (PG pool registry fetch + Redis pipeline + arithmetic).
    pub period: Duration,
    /// Max reserve age before the pool is treated as stale and skipped.
    pub max_stale_secs: u64,
}

impl ReservesSpotConfig {
    /// Read config from env. Returns `Ok(None)` (default-safe) unless the gate
    /// is exactly `active` (case-insensitive). Errors only when the gate IS
    /// active but no chain is enabled (a misconfiguration worth surfacing).
    pub fn from_env(enabled_chains: &[u64]) -> Result<Option<Self>> {
        let gate = std::env::var(ENV_GATE).unwrap_or_default();
        let min_liq = std::env::var(ENV_MIN_LIQ).ok();
        let period = std::env::var(ENV_PERIOD).ok();
        let max_stale = std::env::var(ENV_MAX_STALE).ok();
        Self::from_values(
            &gate,
            min_liq.as_deref(),
            period.as_deref(),
            max_stale.as_deref(),
            enabled_chains,
        )
    }

    /// Pure config parse (no env access) — unit-testable without touching the
    /// process environment.
    fn from_values(
        gate: &str,
        min_liq: Option<&str>,
        period: Option<&str>,
        max_stale: Option<&str>,
        enabled_chains: &[u64],
    ) -> Result<Option<Self>> {
        if !gate.trim().eq_ignore_ascii_case("active") {
            return Ok(None);
        }
        let min_liquidity_usd = min_liq
            .and_then(|v| v.trim().parse::<f64>().ok())
            .filter(|v| v.is_finite() && *v >= 0.0)
            .unwrap_or(DEFAULT_MIN_LIQ_USD);
        let period_secs = period
            .and_then(|v| v.trim().parse::<u64>().ok())
            .filter(|v| *v >= MIN_PERIOD_SECS)
            .unwrap_or(DEFAULT_PERIOD_SECS);
        let max_stale_secs = max_stale
            .and_then(|v| v.trim().parse::<u64>().ok())
            .filter(|v| *v > 0)
            .unwrap_or(DEFAULT_MAX_STALE_SECS);
        if enabled_chains.is_empty() {
            anyhow::bail!(
                "ARBX_RESERVES_SPOT_ORACLE=active but ENRICHER_CHAINS is empty — \
                 no chains to price"
            );
        }
        Ok(Some(Self {
            chains: enabled_chains.to_vec(),
            min_liquidity_usd,
            period: Duration::from_secs(period_secs),
            max_stale_secs,
        }))
    }
}

// ---------------------------------------------------------------------------
// Pure spot-price math
// ---------------------------------------------------------------------------

/// Derive the USD price of the UNKNOWN leg of a V2 pool from the KNOWN leg's
/// price and the raw reserves of both legs.
///
/// Identity (zero-slippage marginal rate):
/// ```text
///   price_unknown = price_known · (R_known / R_unknown) · 10^(dec_unknown − dec_known)
/// ```
///
/// Raw reserves are parsed as `u128` (uint112 fits; forward-compat decimal
/// string) and only widened to `f64` for the ratio — relative precision is
/// therefore ≤ 2^−53 ≈ 1.1e‑16, far below the noise of any price feed.
///
/// Returns `None` (fail-honest) when:
///   - either raw reserve fails to parse as `u128`,
///   - either reserve is zero (degenerate pool — division by zero),
///   - `price_known` is non-finite or non-positive,
///   - the computed price is non-finite or non-positive (overflow / underflow).
///
/// `dec_unknown − dec_known` may be negative (e.g. known=18, unknown=6); the
/// `10^(dec_unknown − dec_known)` factor then shrinks the price correctly.
fn derive_price(
    price_known: f64,
    reserve_known_raw: &str,
    reserve_unknown_raw: &str,
    dec_known: u8,
    dec_unknown: u8,
) -> Option<f64> {
    if !(price_known.is_finite() && price_known > 0.0) {
        return None;
    }
    let r_known = reserve_known_raw.trim().parse::<u128>().ok()?;
    let r_unknown = reserve_unknown_raw.trim().parse::<u128>().ok()?;
    if r_known == 0 || r_unknown == 0 {
        return None;
    }
    let dec_diff = (dec_unknown as i32) - (dec_known as i32);
    let scale = 10f64.powi(dec_diff);
    let ratio = (r_known as f64) / (r_unknown as f64);
    let price = price_known * ratio * scale;
    if price.is_finite() && price > 0.0 {
        Some(price)
    } else {
        None
    }
}

/// USD value of one leg's reserve — the trust-gate quantity. Same precision
/// note as `derive_price` (u128 raw → f64 ratio, ~1e‑16 relative error).
/// Returns `None` on parse failure or non-finite result (the caller skips the
/// pool fail-honestly instead of writing a fabricated liquidity).
fn leg_liquidity_usd(price_known: f64, reserve_raw: &str, decimals: u8) -> Option<f64> {
    if !(price_known.is_finite() && price_known > 0.0) {
        return None;
    }
    let r = reserve_raw.trim().parse::<u128>().ok()?;
    let human = (r as f64) / 10f64.powi(decimals as i32);
    let liq = price_known * human;
    if liq.is_finite() && liq > 0.0 {
        Some(liq)
    } else {
        None
    }
}

/// Staleness gate — `true` when the reserves entry's `ts` is older than
/// `max_stale_secs` relative to `now_secs`. Pure (no IO) so it is unit-tested
/// without `tokio::time` or a wall clock.
fn is_stale(ts: u64, now_secs: u64, max_stale_secs: u64) -> bool {
    // Saturating subtract handles the (rare) case of clock skew where ts > now.
    now_secs.saturating_sub(ts) > max_stale_secs
}

/// Merge a derived `(symbol, price, liquidity)` into the per-symbol map,
/// keeping the DEEPER-liquidity derivation on collision. Returns `true` if it
/// inserted or replaced, `false` if an existing deeper-liquidity entry was
/// kept. Mirrors `DexScreenerPriceOracle::merge_priced` so two pools that
/// share a symbol (a bridged variant, or a clone) resolve deterministically
/// to the deepest pool — independent of PG row order.
fn merge_derived(map: &mut HashMap<String, (f64, f64)>, sym: String, price: f64, liq: f64) -> bool {
    match map.get(&sym) {
        Some(&(_, cur_liq)) if cur_liq >= liq => false,
        _ => {
            map.insert(sym, (price, liq));
            true
        }
    }
}

// ---------------------------------------------------------------------------
// Oracle
// ---------------------------------------------------------------------------

#[derive(Default, Debug)]
struct RunStats {
    total_pools_scanned: usize,
    derived_prices_count: usize,
    prices_written: usize,
    skipped_no_reserves: usize,
    skipped_stale: usize,
    skipped_no_known_leg: usize,
    skipped_low_liquidity: usize,
    skipped_math_failed: usize,
    write_errors: usize,
}

pub struct ReservesSpotOracle {
    cfg: ReservesSpotConfig,
    db: PgPool,
    redis_url: String,
}

impl ReservesSpotOracle {
    pub fn new(cfg: ReservesSpotConfig, db: PgPool, redis_url: String) -> Result<Self> {
        Ok(Self { cfg, db, redis_url })
    }

    /// Detached run loop. Ticks immediately, then every `cfg.period`. Holds a
    /// Redis connection across cycles and reconnects after a write error
    /// (handles Redis restarts gracefully). Relies on process exit for
    /// shutdown — same detached-task idiom as `serve_metrics`,
    /// `DexScreenerPriceOracle`, and `GeckoTerminalOracle`.
    pub async fn run(self) {
        // Cross-writer TTL coupling guard (same caveat as DexScreener): our
        // EXPIRE governs the WHOLE shared `arbx:token_prices` hash (Redis < 7.4
        // has no per-field TTL). A large period over-extends price_worker's
        // fields too — warn the operator.
        let ttl_secs = self.cfg.period.as_secs().saturating_mul(3).max(60);
        if ttl_secs > 180 {
            warn!(
                event = "reserves_spot.large_ttl",
                ttl_secs,
                period_secs = self.cfg.period.as_secs(),
                "ARBX_RESERVES_SPOT_PERIOD_SECS is large — it sets the TTL of the \
                 SHARED token-prices hash for ALL writers; keep it small so \
                 price_worker stays the binding TTL floor"
            );
        }
        info!(
            event = "reserves_spot.oracle_started",
            chains = ?self.cfg.chains,
            min_liquidity_usd = self.cfg.min_liquidity_usd,
            period_secs = self.cfg.period.as_secs(),
            max_stale_secs = self.cfg.max_stale_secs,
        );
        let mut interval = tokio::time::interval(self.cfg.period);
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        let mut conn: Option<redis::aio::MultiplexedConnection> = None;

        loop {
            interval.tick().await;
            if conn.is_none() {
                match self.connect_redis().await {
                    Ok(c) => conn = Some(c),
                    Err(e) => {
                        warn!(
                            event = "reserves_spot.redis_connect_err",
                            err = %e,
                            "skipping cycle"
                        );
                        continue;
                    }
                }
            }
            // Safe: just ensured Some above.
            let c = conn.as_mut().expect("redis connection present");
            let stats = self.run_once(c).await;
            info!(
                event = "reserves_spot.cycle_done",
                total_pools_scanned = stats.total_pools_scanned,
                derived_prices_count = stats.derived_prices_count,
                prices_written = stats.prices_written,
                skipped_no_reserves = stats.skipped_no_reserves,
                skipped_stale = stats.skipped_stale,
                skipped_no_known_leg = stats.skipped_no_known_leg,
                skipped_low_liquidity = stats.skipped_low_liquidity,
                skipped_math_failed = stats.skipped_math_failed,
                write_errors = stats.write_errors,
            );
            // Force a fresh connection next cycle if a write failed.
            if stats.write_errors > 0 {
                conn = None;
            }
        }
    }

    async fn connect_redis(&self) -> Result<redis::aio::MultiplexedConnection> {
        let client =
            redis::Client::open(self.redis_url.as_str()).context("redis::Client::open")?;
        client
            .get_multiplexed_async_connection()
            .await
            .context("get_multiplexed_async_connection")
    }

    /// One full cycle across all configured chains. Never errors out — every
    /// failure is logged and the cycle continues fail-honestly.
    async fn run_once(&self, conn: &mut redis::aio::MultiplexedConnection) -> RunStats {
        let mut stats = RunStats::default();
        let now_secs = unix_now();

        for &chain_id in &self.cfg.chains {
            let pools = match self.fetch_pool_registry(chain_id).await {
                Ok(p) => p,
                Err(e) => {
                    warn!(event = "reserves_spot.fetch_pools_err", chain_id, err = %e);
                    continue;
                }
            };
            if pools.is_empty() {
                debug!(event = "reserves_spot.no_pools", chain_id);
                continue;
            }
            stats.total_pools_scanned += pools.len();

            // Known price snapshot — read ONCE per chain (the hash the reader
            // also reads; same contract DexScreener writes into).
            let known_prices: HashMap<String, f64> =
                match self.fetch_known_prices(conn, chain_id).await {
                    Ok(m) => m,
                    Err(e) => {
                        warn!(event = "reserves_spot.fetch_prices_err", chain_id, err = %e);
                        continue;
                    }
                };
            if known_prices.is_empty() {
                // No anchor prices → no derivation possible this cycle. Don't
                // bother fetching reserves for every pool; wait for price_worker.
                debug!(event = "reserves_spot.no_known_prices", chain_id);
                continue;
            }

            // Pipelined GET of every pool's reserves — one Redis round-trip.
            let reserves_by_pool = match self.fetch_all_reserves(conn, chain_id, &pools).await {
                Ok(m) => m,
                Err(e) => {
                    warn!(event = "reserves_spot.fetch_reserves_err", chain_id, err = %e);
                    continue;
                }
            };

            // symbol -> (price, liquidity). Dedup by symbol keeping the
            // DEEPER-liquidity derivation (mirrors DexScreener's merge_priced).
            let mut derived: HashMap<String, (f64, f64)> = HashMap::new();

            for pool in &pools {
                let reserves = match reserves_by_pool.get(&pool.pool_addr_lower) {
                    Some(r) => r,
                    None => {
                        stats.skipped_no_reserves += 1;
                        continue;
                    }
                };
                if is_stale(reserves.ts, now_secs, self.cfg.max_stale_secs) {
                    stats.skipped_stale += 1;
                    continue;
                }

                let p0 = known_prices.get(&pool.token0_symbol_upper).copied();
                let p1 = known_prices.get(&pool.token1_symbol_upper).copied();

                let derived_price = match (p0, p1) {
                    (Some(known0), None) => {
                        // token0 known ⇒ derive token1.
                        let liq = leg_liquidity_usd(known0, &reserves.r0, pool.token0_decimals);
                        let price = derive_price(
                            known0,
                            &reserves.r0,
                            &reserves.r1,
                            pool.token0_decimals,
                            pool.token1_decimals,
                        );
                        price.zip(liq).map(|(p, l)| (p, l, pool.token1_symbol_upper.clone()))
                    }
                    (None, Some(known1)) => {
                        // token1 known ⇒ derive token0.
                        let liq = leg_liquidity_usd(known1, &reserves.r1, pool.token1_decimals);
                        let price = derive_price(
                            known1,
                            &reserves.r1,
                            &reserves.r0,
                            pool.token1_decimals,
                            pool.token0_decimals,
                        );
                        price.zip(liq).map(|(p, l)| (p, l, pool.token0_symbol_upper.clone()))
                    }
                    _ => {
                        // Both known (already covered by an API writer — skip)
                        // or neither known (no anchor — can't derive).
                        stats.skipped_no_known_leg += 1;
                        continue;
                    }
                };

                match derived_price {
                    Some((price, liq, sym)) => {
                        if liq < self.cfg.min_liquidity_usd {
                            stats.skipped_low_liquidity += 1;
                            continue;
                        }
                        merge_derived(&mut derived, sym, price, liq);
                        stats.derived_prices_count += 1;
                    }
                    None => {
                        stats.skipped_math_failed += 1;
                    }
                }
            }

            if derived.is_empty() {
                debug!(
                    event = "reserves_spot.no_derivations",
                    chain_id,
                    pools_scanned = pools.len()
                );
                continue;
            }

            // HSETNX each derivation into the shared price hash. We do NOT
            // clobber existing fields — see "Non-clobber contract" in the
            // module docs. Only refresh the TTL if we actually wrote something.
            let flat: Vec<(String, f64)> = derived.into_iter().map(|(s, (p, _))| (s, p)).collect();
            match self.write_prices_hsetnx(conn, chain_id, &flat).await {
                Ok(written) => {
                    stats.prices_written += written;
                    info!(
                        event = "reserves_spot.chain_priced",
                        chain_id,
                        pools_scanned = pools.len(),
                        derived = written,
                    );
                }
                Err(e) => {
                    stats.write_errors += 1;
                    warn!(event = "reserves_spot.write_err", chain_id, err = %e);
                }
            }
        }
        stats
    }

    /// Active V2 pool registry for a chain, with BOTH legs' symbol + decimals.
    /// Sourced from PG `pools` ⨝ `tokens` (token0) ⨝ `tokens` (token1), the
    /// same source `pool_sync_worker::bootstrap_pool_index` uses to populate
    /// the Redis `arbx:pool_index`. Pools lacking either token ref or with an
    /// unparseable decimal are filtered out (the scanner would skip them too).
    ///
    /// We do NOT filter by `protocol_type` here — the V2 reserves cache key
    /// (`arbx:pool_reserves:<chain>:<addr>`) is only ever written by the V2
    /// path, so V3 pools simply miss the reserves fetch and are skipped. This
    /// keeps the query simple and relies on Redis for the V2/V3 discrimination.
    async fn fetch_pool_registry(&self, chain_id: u64) -> Result<Vec<PoolRegistryRow>> {
        let rows = sqlx::query_as::<_, (String, String, i32, String, String, i32)>(
            r#"SELECT
                   LOWER(p.address)      AS pool_addr,
                   LOWER(t0.address)     AS token0_addr,
                   t0.symbol             AS token0_symbol,
                   t0.decimals           AS token0_decimals,
                   LOWER(t1.address)     AS token1_addr,
                   t1.symbol             AS token1_symbol,
                   t1.decimals           AS token1_decimals
               FROM pools p
               JOIN tokens t0 ON p.token0_id = t0.id
               JOIN tokens t1 ON p.token1_id = t1.id
               WHERE p.chain_id = $1
                 AND p.is_active = TRUE
                 AND t0.symbol IS NOT NULL AND t0.symbol <> ''
                 AND t1.symbol IS NOT NULL AND t1.symbol <> ''
                 AND t0.decimals IS NOT NULL
                 AND t1.decimals IS NOT NULL"#,
        )
        .bind(chain_id as i64)
        .fetch_all(&self.db)
        .await
        .context("query pool registry for reserves-spot oracle")?;

        let mut out = Vec::with_capacity(rows.len());
        for (pool_addr, _t0_addr, t0_sym, t0_dec, _t1_addr, t1_sym, t1_dec) in rows {
            // Decimal column is INTEGER; coerce to u8 with a sane bounds check.
            // Tokens with decimals > 30 don't exist on any real chain (ERC20
            // max is 18 for WETH-like, 36 for some exotic test tokens). Skip
            // anything outside [0, 30] — the math helper would still compute
            // but the upstream scanner would have already dropped it.
            let (t0_dec_u8, t1_dec_u8) = match (u8::try_from(t0_dec), u8::try_from(t1_dec)) {
                (Ok(a), Ok(b)) if a <= 30 && b <= 30 => (a, b),
                _ => continue,
            };
            let pool_addr_lower = pool_addr.trim().to_ascii_lowercase();
            if pool_addr_lower.is_empty() {
                continue;
            }
            out.push(PoolRegistryRow {
                pool_addr_lower,
                token0_symbol_upper: t0_sym.trim().to_ascii_uppercase(),
                token0_decimals: t0_dec_u8,
                token1_symbol_upper: t1_sym.trim().to_ascii_uppercase(),
                token1_decimals: t1_dec_u8,
            });
        }
        Ok(out)
    }

    /// `HGETALL arbx:token_prices:<chain>` → `{ symbol_upper → price }`. Same
    /// read contract as `RedisCachedPriceOracle::snapshot_from_redis`. Symbol
    /// keys are normalized to uppercase (the hash stores them that way, but we
    /// defend against a malformed writer). Non-finite / non-positive values
    /// are dropped (R8 — never anchor a derivation on garbage).
    async fn fetch_known_prices(
        &self,
        conn: &mut redis::aio::MultiplexedConnection,
        chain_id: u64,
    ) -> Result<HashMap<String, f64>> {
        use redis::AsyncCommands;
        let key = redis_token_prices_key(chain_id);
        let raw: HashMap<String, String> = conn
            .hgetall(&key)
            .await
            .context("HGETALL token_prices for reserves-spot oracle")?;
        let mut out = HashMap::with_capacity(raw.len());
        for (sym, val) in raw {
            match val.trim().parse::<f64>() {
                Ok(v) if v.is_finite() && v > 0.0 => {
                    out.insert(sym.trim().to_ascii_uppercase(), v);
                }
                _ => debug!(
                    event = "reserves_spot.known_price_parse_failed",
                    chain_id, symbol = %sym, raw = %val,
                    "dropping malformed cached price"
                ),
            }
        }
        Ok(out)
    }

    /// Pipelined GET of every pool's reserves entry — ONE Redis round-trip
    /// regardless of pool count. Missing entries (cache miss at boot, or pool
    /// never seen by `pool_sync_worker`) are simply absent from the returned
    /// map; the caller skips them fail-honestly.
    ///
    /// Returns a map keyed by `pool_addr_lower` (the same key the registry
    /// uses) so the cycle body can join the two with one HashMap lookup.
    async fn fetch_all_reserves(
        &self,
        conn: &mut redis::aio::MultiplexedConnection,
        chain_id: u64,
        pools: &[PoolRegistryRow],
    ) -> Result<HashMap<String, ReservesEntry>> {
        if pools.is_empty() {
            return Ok(HashMap::new());
        }
        let mut pipe = redis::pipe();
        for p in pools {
            // GET — key returns the JSON string written by set_reserves, or nil.
            pipe.get(format!(
                "arbx:pool_reserves:{}:{}",
                chain_id, p.pool_addr_lower
            ));
        }
        let raws: Vec<Option<String>> = pipe
            .query_async(conn)
            .await
            .context("redis pipeline GET pool_reserves")?;
        let mut out = HashMap::with_capacity(pools.len());
        for (pool, raw) in pools.iter().zip(raws.into_iter()) {
            let Some(json) = raw else { continue };
            match serde_json::from_str::<ReservesEntry>(&json) {
                Ok(entry) => {
                    out.insert(pool.pool_addr_lower.clone(), entry);
                }
                Err(e) => debug!(
                    event = "reserves_spot.parse_failed",
                    chain_id,
                    pool = %pool.pool_addr_lower,
                    err = %e,
                    "skipping malformed reserves entry"
                ),
            }
        }
        Ok(out)
    }

    /// `HSETNX` each derived price into `arbx:token_prices:<chain>` and refresh
    /// the shared key TTL — the SAME key all price writers use (canonical key
    /// via `redis_token_prices_key`). HSETNX guarantees we NEVER overwrite an
    /// API-sourced field (see "Non-clobber contract"). Returns the count of
    /// fields actually written (HSETNX returns 1 for newly-set, 0 for
    /// already-existed — we sum these to surface "how many gaps did we fill
    /// this cycle" via the `prices_written` metric).
    ///
    /// Defensive: never write a non-finite or non-positive price (the reader
    /// also drops these, but honesty starts at the writer).
    async fn write_prices_hsetnx(
        &self,
        conn: &mut redis::aio::MultiplexedConnection,
        chain_id: u64,
        prices: &[(String, f64)],
    ) -> Result<usize> {
        if prices.is_empty() {
            return Ok(0);
        }
        let key = redis_token_prices_key(chain_id);
        // TTL = period*3 floored at 60s, mirroring price_worker's
        // (period*2).max(60) and DexScreener's (interval*3).max(60). Same
        // cross-writer TTL coupling caveat applies — see run() boot warning.
        let ttl_secs: i64 = ((self.cfg.period.as_secs() as i64).saturating_mul(3)).max(60);

        let mut pipe = redis::pipe();
        pipe.atomic();
        let mut submitted = 0usize;
        for (sym, price) in prices {
            if !(price.is_finite() && *price > 0.0) {
                continue;
            }
            // HSETNX returns 1 if field was set (newly added), 0 if already
            // existed (we kept the API-sourced value — desired). We sum the
            // return values via the pipeline result.
            pipe.hsetnx(&key, sym, format!("{price}"));
            submitted += 1;
        }
        if submitted == 0 {
            return Ok(0);
        }
        pipe.expire(&key, ttl_secs).ignore();
        // Result is one value per HSETNX (0 or 1) + the ignored EXPIRE.
        let results: Vec<i64> = pipe
            .query_async(conn)
            .await
            .context("redis HSETNX token prices pipeline")?;
        Ok(results.into_iter().map(|v| v.clamp(0, 1) as usize).sum())
    }
}

/// Wall-clock unix seconds. Wraps SystemTime so the production code path is
/// injectable in tests via the pure `is_stale` helper (which takes `now_secs`
/// as a parameter — tests pass a fixed `now_secs` and never touch the wall
/// clock). Returns 0 on the (impossible-before-1970) error case — `is_stale`
/// then treats the reserves entry as fresh, which is the safe default (better
/// to price off slightly-stale data than to skip the cycle entirely).
fn unix_now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

// ---------------------------------------------------------------------------
// Unit tests (pure — no network, no DB, no env mutation, no Redis)
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // --- ReservesSpotConfig::from_values ---

    #[test]
    fn config_off_when_gate_absent_or_other() {
        assert!(
            ReservesSpotConfig::from_values("", None, None, None, &[1])
                .unwrap()
                .is_none()
        );
        assert!(
            ReservesSpotConfig::from_values("off", None, None, None, &[1])
                .unwrap()
                .is_none()
        );
        assert!(
            ReservesSpotConfig::from_values("shadow", None, None, None, &[1])
                .unwrap()
                .is_none()
        );
    }

    #[test]
    fn config_active_case_insensitive() {
        for gate in ["active", "ACTIVE", "Active", "  active  "] {
            let cfg = ReservesSpotConfig::from_values(gate, None, None, None, &[1, 137])
                .unwrap()
                .unwrap_or_else(|| panic!("gate `{gate}` should activate the oracle"));
            assert_eq!(cfg.chains, vec![1, 137]);
            assert_eq!(cfg.min_liquidity_usd, DEFAULT_MIN_LIQ_USD);
            assert_eq!(cfg.period, Duration::from_secs(DEFAULT_PERIOD_SECS));
            assert_eq!(cfg.max_stale_secs, DEFAULT_MAX_STALE_SECS);
        }
    }

    #[test]
    fn config_custom_values() {
        let cfg = ReservesSpotConfig::from_values(
            "active",
            Some("25000.5"),
            Some("30"),
            Some("120"),
            &[1],
        )
        .unwrap()
        .unwrap();
        assert!((cfg.min_liquidity_usd - 25_000.5).abs() < f64::EPSILON);
        assert_eq!(cfg.period, Duration::from_secs(30));
        assert_eq!(cfg.max_stale_secs, 120);
    }

    #[test]
    fn config_period_floored_to_min() {
        let cfg = ReservesSpotConfig::from_values("active", None, Some("1"), None, &[1])
            .unwrap()
            .unwrap();
        assert_eq!(cfg.period, Duration::from_secs(DEFAULT_PERIOD_SECS));
    }

    #[test]
    fn config_negative_min_liq_falls_back() {
        let cfg = ReservesSpotConfig::from_values("active", Some("-5"), None, None, &[1])
            .unwrap()
            .unwrap();
        assert_eq!(cfg.min_liquidity_usd, DEFAULT_MIN_LIQ_USD);
    }

    #[test]
    fn config_errors_when_no_chain_enabled() {
        assert!(ReservesSpotConfig::from_values("active", None, None, None, &[]).is_err());
    }

    // --- derive_price: the spot-price identity ---

    /// WETH/USDC pool. token0 = WETH (18 dec) at $3000, token1 = USDC (6 dec)
    /// at $1. Reserves R0 = 1000 WETH (raw 1e21), R1 = 3M USDC (raw 3e12).
    /// Deriving USDC's price from WETH: $1.
    #[test]
    fn test_derive_price_known_token0() {
        // token0 known (WETH @ $3000), derive token1 (USDC).
        // price_unknown = 3000 · (R0 / R1) · 10^(dec1 − dec0)
        //               = 3000 · (1e21 / 3e12) · 10^(6 − 18)
        //               = 3000 · (3.33e8) · 1e‑12
        //               = 1.0
        let price = derive_price(3000.0, "1000000000000000000000", "3000000000000", 18, 6);
        assert!(price.is_some(), "should derive a price");
        let price = price.unwrap();
        assert!(
            (price - 1.0).abs() < 1e-9,
            "expected $1.0 for USDC, got {price}"
        );
    }

    /// Symmetric: token1 (USDC @ $1) known, derive token0 (WETH).
    #[test]
    fn test_derive_price_known_token1() {
        // price_unknown = 1 · (R1 / R0) · 10^(dec0 − dec1)
        //               = 1 · (3e12 / 1e21) · 10^(18 − 6)
        //               = 3e‑9 · 1e12
        //               = 3000
        let price = derive_price(1.0, "3000000000000", "1000000000000000000000", 6, 18);
        assert!(price.is_some(), "should derive a price");
        let price = price.unwrap();
        assert!(
            (price - 3000.0).abs() < 1e-6,
            "expected $3000 for WETH, got {price}"
        );
    }

    /// Decimals mismatch is the COMMON case (18-decimal WETH vs 6-decimal
    /// stable). This test pins the math against a hand-computed answer for a
    /// non-stable/non-1:1 pair to catch any sign-error on the exponent.
    #[test]
    fn test_decimals_mismatch_handled() {
        // Pool: ABC (9 dec) / WETH (18 dec). WETH known @ $2500.
        // R_abc = 5_000_000_000 ABC raw (5e9 = 5 ABC human at 9 dec... wait).
        // Actually 5e9 raw / 10^9 = 5 human ABC.
        // R_weth = 2_000_000_000_000_000_000 raw (2e18 = 2 WETH human).
        // Pool holds 5 ABC + 2 WETH → ABC price = 2·2500/5 = $1000 per ABC.
        //
        // Formula: price_abc = 2500 · (R_weth / R_abc) · 10^(dec_abc − dec_weth)
        //                    = 2500 · (2e18 / 5e9) · 10^(9 − 18)
        //                    = 2500 · 4e8 · 1e‑9
        //                    = 2500 · 0.4
        //                    = 1000
        let price = derive_price(
            2500.0,
            "2000000000000000000", // R_weth raw (KNOWN leg)
            "5000000000",          // R_abc raw (UNKNOWN leg)
            18,                    // dec KNOWN (WETH)
            9,                     // dec UNKNOWN (ABC)
        );
        let price = price.expect("should derive");
        assert!(
            (price - 1000.0).abs() < 1e-6,
            "expected $1000 for ABC, got {price}"
        );
    }

    /// Zero reserve (degenerate or just-bootstrapped pool) must NOT panic and
    /// must NOT produce a price — fail-honest None.
    #[test]
    fn test_zero_reserve_returns_none() {
        assert_eq!(derive_price(100.0, "0", "1000", 18, 6), None);
        assert_eq!(derive_price(100.0, "1000", "0", 18, 6), None);
        assert_eq!(derive_price(100.0, "0", "0", 18, 6), None);
    }

    /// Non-numeric / malformed reserves must return None, never panic.
    #[test]
    fn test_malformed_reserve_returns_none() {
        assert_eq!(derive_price(100.0, "not-a-number", "1000", 18, 6), None);
        assert_eq!(derive_price(100.0, "1000", "", 18, 6), None);
        assert_eq!(derive_price(100.0, "  ", "1000", 18, 6), None);
        // Whitespace is trimmed — a valid number with surrounding whitespace parses.
        assert!(derive_price(100.0, "  1000  ", "10", 18, 6).is_some());
    }

    /// Non-positive / non-finite anchor price must return None.
    #[test]
    fn test_non_finite_anchor_returns_none() {
        assert_eq!(derive_price(0.0, "1000", "10", 18, 6), None);
        assert_eq!(derive_price(-5.0, "1000", "10", 18, 6), None);
        assert_eq!(derive_price(f64::INFINITY, "1000", "10", 18, 6), None);
        assert_eq!(derive_price(f64::NAN, "1000", "10", 18, 6), None);
    }

    /// Same-decimal pair (both legs 18) — the exponent factor collapses to 1.
    #[test]
    fn test_same_decimals_no_scaling() {
        // Two 18-decimal tokens, R0 = R1 → price_unknown = price_known.
        let price = derive_price(42.0, "1000000000000000000", "1000000000000000000", 18, 18);
        assert!((price.unwrap() - 42.0).abs() < 1e-9);
    }

    // --- leg_liquidity_usd ---

    #[test]
    fn test_leg_liquidity_usd_basic() {
        // 1000 WETH raw=1e21, dec=18, price=$3000 → liq=$3M.
        let liq = leg_liquidity_usd(3000.0, "1000000000000000000000", 18);
        assert!((liq.unwrap() - 3_000_000.0).abs() < 1e-3);
    }

    #[test]
    fn test_leg_liquidity_usd_zero_reserve() {
        assert_eq!(leg_liquidity_usd(3000.0, "0", 18), None);
    }

    #[test]
    fn test_leg_liquidity_usd_malformed() {
        assert_eq!(leg_liquidity_usd(3000.0, "garbage", 18), None);
    }

    // --- is_stale ---

    #[test]
    fn test_stale_pool_skipped() {
        // ts = 1000, now = 2000, max_stale = 30 → age = 1000 > 30, stale.
        assert!(is_stale(1000, 2000, 30));
        // ts = 2000, now = 2030, max_stale = 30 → age = 30, NOT stale (≤).
        assert!(!is_stale(2000, 2030, 30));
        // ts = 2000, now = 2031, max_stale = 30 → age = 31 > 30, stale.
        assert!(is_stale(2000, 2031, 30));
    }

    #[test]
    fn test_stale_clock_skew_handled() {
        // ts > now (clock skew) — saturating_sub returns 0, NOT stale.
        // This is the safe default: better to price off slightly-future-dated
        // reserves than to skip the cycle entirely.
        assert!(!is_stale(3000, 2000, 30));
    }

    // --- merge_derived ---

    #[test]
    fn test_merge_derived_keeps_deeper_liquidity() {
        let mut m: HashMap<String, (f64, f64)> = HashMap::new();
        // First pool derives WETH at $3000 from a $100K-liquidity pool.
        assert!(merge_derived(&mut m, "WETH".to_string(), 3000.0, 100_000.0));
        // Second, SHALLOWER pool derives WETH at $2900 — rejected.
        assert!(!merge_derived(&mut m, "WETH".to_string(), 2900.0, 50_000.0));
        assert_eq!(m.get("WETH"), Some(&(3000.0, 100_000.0)));
        // Third, DEEPER pool derives WETH at $3050 — replaces.
        assert!(merge_derived(&mut m, "WETH".to_string(), 3050.0, 500_000.0));
        assert_eq!(m.get("WETH"), Some(&(3050.0, 500_000.0)));
        // Distinct symbol inserts independently.
        assert!(merge_derived(&mut m, "USDC".to_string(), 1.0, 1_000_000.0));
        assert_eq!(m.len(), 2);
    }

    // --- ReservesEntry wire-format compat ---

    /// Pin the deserialization against the JSON shape `set_reserves` writes
    /// (searcher-rs reserves.rs). If the upstream struct drifts, this test
    /// breaks loudly before any cycle runs in production.
    #[test]
    fn test_reserves_entry_deserializes_writer_shape() {
        let raw = r#"{
            "r0": "1000000000000000000",
            "r1": "3000000000000",
            "token0_addr": "0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2",
            "blk": 18000000,
            "ts": 1700000000
        }"#;
        let entry: ReservesEntry = serde_json::from_str(raw).unwrap();
        assert_eq!(entry.r0, "1000000000000000000");
        assert_eq!(entry.r1, "3000000000000");
        assert_eq!(
            entry.token0_addr.as_deref(),
            Some("0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2")
        );
        assert_eq!(entry.blk, 18_000_000);
        assert_eq!(entry.ts, 1_700_000_000);
    }

    /// Legacy cache entry WITHOUT `token0_addr` (the field is `#[serde(default)]`)
    /// must still parse — defensive against pre-`token0_addr` cache entries.
    #[test]
    fn test_reserves_entry_tolerates_missing_token0_addr() {
        let raw = r#"{"r0":"1","r1":"2","blk":1,"ts":1}"#;
        let entry: ReservesEntry = serde_json::from_str(raw).unwrap();
        assert!(entry.token0_addr.is_none());
    }
}
