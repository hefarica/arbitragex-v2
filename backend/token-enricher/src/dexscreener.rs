//! DexScreener price oracle — fills the `no_price_oracle` gap for long-tail tokens.
//!
//! ## Why this exists
//!
//! The scanner prices tokens via a cascade: `RedisCachedPriceOracle` (tier 1,
//! the `arbx:token_prices:<chain>` hash populated by `searcher-rs::price_worker`
//! from Chainlink / Alchemy / CoinGecko) → `ConfigPriceOracle` (tier 2) → `None`.
//! When BOTH tiers miss, `dex_engine` emits `rejection_reason = "no_price_oracle"`.
//! Long-tail tokens (anything Chainlink/Alchemy/CoinGecko don't cover) fall
//! through and starve candidate opportunities upstream of the net-profit gate.
//!
//! This oracle adds a NEW population path into the SAME tier-1 hash, keyed by the
//! SAME canonical symbol the scanner reads — so it requires **zero scanner
//! changes**. It queries DexScreener (a free public aggregator that covers the
//! long tail) for the active tokens and writes the best-liquidity USD price.
//!
//! ## Divergences from the original mission spec (documented per mandate)
//!
//! The mission assumed: (a) a TypeScript `token-enricher` with `dexscreener.ts`;
//! (b) a Redis price contract `arbx:prices:<chain>:<addr>` keyed by ADDRESS;
//! (c) a `searcher-rs` `ActiveTokensWriter` publishing active addresses to a
//! Redis set `arbx:active-token-addresses:<chain>` for this service to read.
//!
//! The REAL repo differs, so this implementation adapts (mission rule: "prioriza
//! el contrato real del repo, documenta la diferencia y adapta sin romper
//! compatibilidad"):
//!   1. `token-enricher` is **Rust** → this is a Rust module, not `dexscreener.ts`.
//!   2. The real price contract is **`arbx:token_prices:<chain>` HASH keyed by
//!      UPPERCASE SYMBOL** (`shared_rs::price_oracle::redis_token_prices_key`),
//!      value = decimal-string USD price. We write THAT, by symbol — never by
//!      address. Reusing the canonical key fn guarantees we never drift from the
//!      reader.
//!   3. **No `ActiveTokensWriter` in `searcher-rs`.** The enricher already holds
//!      `DATABASE_URL` and reads the PG `tokens` table, so it sources active
//!      `(address, symbol)` DIRECTLY from its own Postgres (same query
//!      `pool_sync_worker::bootstrap_token_cache` uses). This removes an entire
//!      cross-service Redis coupling AND a hot-path-service change, and — most
//!      importantly — guarantees the symbol we write under is identical to the
//!      one the scanner derives its price lookup from (both come from
//!      `tokens.symbol`). DexScreener's own `baseToken.symbol` is intentionally
//!      NOT used as the key, precisely to avoid that mismatch class.
//!
//! ## Doctrine
//!
//! Read-only / shadow. No signer, no private key, no broadcast, no live trading,
//! no contract calls. Default-OFF: does nothing unless `ARBX_DEXSCREENER_ORACLE`
//! is exactly `active`. Fail-honest: a DexScreener/Redis/liquidity failure
//! degrades silently (skip the cycle/chunk/token — never write a fabricated or
//! non-positive price, never clear existing prices). Idempotent: each cycle is a
//! pure `HSET` upsert by symbol. Rate-limited: ≤30 addresses/request, ~1.1s
//! between EVERY request (cycle-wide, across chunks AND chains), well within
//! DexScreener's /latest/dex/tokens 300 req/min (5/sec) tier.

use anyhow::{Context, Result};
use serde::Deserialize;
use sqlx::PgPool;
use std::collections::HashMap;
use std::time::Duration;
use tracing::{debug, info, warn};

use shared_rs::price_oracle::redis_token_prices_key;

/// DexScreener public token endpoint base. Final URL is `…/tokens/{a,b,c}`.
const DEXSCREENER_TOKENS_URL: &str = "https://api.dexscreener.com/latest/dex/tokens/";
/// DexScreener documents ~30 comma-separated addresses per token request.
const MAX_ADDRS_PER_REQUEST: usize = 30;
/// Delay inserted before every DexScreener request after the first in a cycle
/// (across chunks AND chains) — caps a single chain's chunk stream at <1 req/s,
/// comfortably inside DexScreener's 300 req/min (5/sec) /latest/dex/tokens tier.
const INTER_CHUNK_DELAY: Duration = Duration::from_millis(1100);
/// HTTP request timeout for a single DexScreener call.
const HTTP_TIMEOUT: Duration = Duration::from_secs(10);

// --- env keys ---
/// Off-switch. Oracle runs ONLY when this equals (case-insensitively) `active`.
const ENV_GATE: &str = "ARBX_DEXSCREENER_ORACLE";
/// Minimum pair liquidity (USD) to trust a DexScreener price. Default 50_000.
const ENV_MIN_LIQ: &str = "ARBX_MIN_PRICE_LIQUIDITY_USD";
/// Poll interval in ms. Default 15_000. Floored to 1_000 to avoid hammering.
const ENV_INTERVAL: &str = "DEXSCREENER_PRICE_INTERVAL_MS";

const DEFAULT_MIN_LIQ_USD: f64 = 50_000.0;
const DEFAULT_INTERVAL_MS: u64 = 15_000;
const MIN_INTERVAL_MS: u64 = 1_000;

/// Map a numeric chain id to DexScreener's string `chainId` slug.
///
/// This is DexScreener API reference metadata (a fixed property of their API,
/// like an endpoint path or an ABI), NOT operator/productive configuration, so
/// it is a const map rather than env-sourced. Unknown chains return `None` and
/// are skipped fail-honestly (we never guess a slug — a wrong slug would silently
/// price the wrong chain's token).
fn dexscreener_chain_slug(chain_id: u64) -> Option<&'static str> {
    Some(match chain_id {
        1 => "ethereum",
        10 => "optimism",
        56 => "bsc",
        137 => "polygon",
        250 => "fantom",
        8453 => "base",
        42161 => "arbitrum",
        43114 => "avalanche",
        59144 => "linea",
        534352 => "scroll",
        5000 => "mantle",
        _ => return None,
    })
}

// ---------------------------------------------------------------------------
// Config
// ---------------------------------------------------------------------------

#[derive(Clone, Debug)]
pub struct DexScreenerConfig {
    /// Chains to price (intersection of ENRICHER_CHAINS and the slug map).
    pub chains: Vec<u64>,
    /// Minimum pair liquidity (USD) for a price to be trusted.
    pub min_liquidity_usd: f64,
    /// Poll interval.
    pub interval: Duration,
}

impl DexScreenerConfig {
    /// Read config from env. Returns `Ok(None)` (default-safe) unless the gate
    /// is exactly `active`. Errors only when the gate IS active but no enabled
    /// chain maps to a DexScreener slug (a misconfiguration worth surfacing).
    pub fn from_env(enabled_chains: &[u64]) -> Result<Option<Self>> {
        let gate = std::env::var(ENV_GATE).unwrap_or_default();
        let min_liq = std::env::var(ENV_MIN_LIQ).ok();
        let interval = std::env::var(ENV_INTERVAL).ok();
        // Surface unsupported chains explicitly (the pure helper drops them
        // silently). The gate is loop-invariant — evaluate it once.
        if gate.trim().eq_ignore_ascii_case("active") {
            for &c in enabled_chains {
                if dexscreener_chain_slug(c).is_none() {
                    warn!(
                        event = "dexscreener.chain_unsupported",
                        chain_id = c,
                        "no DexScreener slug mapping for this chain — it will be skipped"
                    );
                }
            }
        }
        Self::from_values(
            &gate,
            min_liq.as_deref(),
            interval.as_deref(),
            enabled_chains,
        )
    }

    /// Pure config parse (no env access) — unit-testable without touching the
    /// process environment.
    fn from_values(
        gate: &str,
        min_liq: Option<&str>,
        interval: Option<&str>,
        enabled_chains: &[u64],
    ) -> Result<Option<Self>> {
        if !gate.trim().eq_ignore_ascii_case("active") {
            return Ok(None);
        }
        let min_liquidity_usd = min_liq
            .and_then(|v| v.trim().parse::<f64>().ok())
            .filter(|v| v.is_finite() && *v >= 0.0)
            .unwrap_or(DEFAULT_MIN_LIQ_USD);
        let interval_ms = interval
            .and_then(|v| v.trim().parse::<u64>().ok())
            .filter(|v| *v >= MIN_INTERVAL_MS)
            .unwrap_or(DEFAULT_INTERVAL_MS);
        let chains: Vec<u64> = enabled_chains
            .iter()
            .copied()
            .filter(|c| dexscreener_chain_slug(*c).is_some())
            .collect();
        if chains.is_empty() {
            anyhow::bail!(
                "ARBX_DEXSCREENER_ORACLE=active but none of the enabled chains \
                 map to a DexScreener slug (supported: 1,10,56,137,250,8453,42161,\
                 43114,59144,534352,5000)"
            );
        }
        Ok(Some(Self {
            chains,
            min_liquidity_usd,
            interval: Duration::from_millis(interval_ms),
        }))
    }
}

// ---------------------------------------------------------------------------
// DexScreener response shapes (only the fields we consume; serde ignores rest)
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
struct DexScreenerResponse {
    #[serde(default)]
    pairs: Vec<DexPair>,
}

#[derive(Debug, Deserialize)]
struct DexPair {
    #[serde(rename = "chainId", default)]
    chain_id: String,
    #[serde(rename = "baseToken", default)]
    base_token: DexToken,
    #[serde(rename = "priceUsd", default)]
    price_usd: Option<String>,
    #[serde(default)]
    liquidity: Option<DexLiquidity>,
}

#[derive(Debug, Default, Deserialize)]
struct DexToken {
    #[serde(default)]
    address: String,
    #[allow(dead_code)] // captured for completeness; we key by PG symbol, not this
    #[serde(default)]
    symbol: String,
}

#[derive(Debug, Deserialize)]
struct DexLiquidity {
    #[serde(default)]
    usd: Option<f64>,
}

// ---------------------------------------------------------------------------
// Oracle
// ---------------------------------------------------------------------------

#[derive(Default, Debug)]
struct RunStats {
    tokens_considered: usize,
    requests_ok: usize,
    requests_err: usize,
    prices_written: usize,
    write_errors: usize,
}

pub struct DexScreenerPriceOracle {
    cfg: DexScreenerConfig,
    db: PgPool,
    redis_url: String,
    http: reqwest::Client,
}

impl DexScreenerPriceOracle {
    pub fn new(cfg: DexScreenerConfig, db: PgPool, redis_url: String) -> Result<Self> {
        let http = reqwest::Client::builder()
            .user_agent("arbitragex-token-enricher/0.1 (dexscreener-price-oracle; shadow)")
            .timeout(HTTP_TIMEOUT)
            .build()
            .context("build reqwest client for DexScreener")?;
        Ok(Self {
            cfg,
            db,
            redis_url,
            http,
        })
    }

    /// Detached run loop. Ticks immediately, then every `cfg.interval`. Holds a
    /// Redis connection across cycles and reconnects after a write error (handles
    /// Redis restarts gracefully). Relies on process exit for shutdown — same
    /// detached-task idiom as `serve_metrics` and the RPC health loops.
    pub async fn run(self) {
        // Cross-writer TTL coupling guard: our EXPIRE governs the WHOLE shared
        // arbx:token_prices hash (Redis < 7.4 has no per-field TTL). A large
        // interval over-extends price_worker's fields too — warn the operator.
        let ttl_secs = self.cfg.interval.as_secs().saturating_mul(3).max(60);
        if ttl_secs > 180 {
            warn!(
                event = "dexscreener.large_ttl",
                ttl_secs,
                interval_ms = self.cfg.interval.as_millis() as u64,
                "DEXSCREENER_PRICE_INTERVAL_MS is large — it sets the TTL of the SHARED token-prices hash for ALL writers; keep it <= price_worker's period so price_worker stays the binding TTL floor"
            );
        }
        info!(
            event = "dexscreener.oracle_started",
            chains = ?self.cfg.chains,
            min_liquidity_usd = self.cfg.min_liquidity_usd,
            interval_ms = self.cfg.interval.as_millis() as u64,
        );
        let mut interval = tokio::time::interval(self.cfg.interval);
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        let mut conn: Option<redis::aio::MultiplexedConnection> = None;

        loop {
            interval.tick().await;
            if conn.is_none() {
                match self.connect_redis().await {
                    Ok(c) => conn = Some(c),
                    Err(e) => {
                        warn!(event = "dexscreener.redis_connect_err", err = %e, "skipping cycle");
                        continue;
                    }
                }
            }
            // Safe: just ensured Some above.
            let c = conn.as_mut().expect("redis connection present");
            let stats = self.run_once(c).await;
            info!(
                event = "dexscreener.cycle_done",
                tokens_considered = stats.tokens_considered,
                prices_written = stats.prices_written,
                requests_ok = stats.requests_ok,
                requests_err = stats.requests_err,
                write_errors = stats.write_errors,
            );
            // Force a fresh connection next cycle if a write failed.
            if stats.write_errors > 0 {
                conn = None;
            }
        }
    }

    async fn connect_redis(&self) -> Result<redis::aio::MultiplexedConnection> {
        let client = redis::Client::open(self.redis_url.as_str()).context("redis::Client::open")?;
        client
            .get_multiplexed_async_connection()
            .await
            .context("get_multiplexed_async_connection")
    }

    /// One full cycle across all configured chains. Never errors out — every
    /// failure is logged and the cycle continues fail-honestly.
    async fn run_once(&self, conn: &mut redis::aio::MultiplexedConnection) -> RunStats {
        let mut stats = RunStats::default();
        // Cycle-wide request throttle: ~1.1s between EVERY DexScreener request,
        // across chunks AND chains, so a multi-chain cycle can't emit a burst at
        // t=0. The very first request of the whole cycle fires immediately.
        let mut first_request = true;
        for &chain_id in &self.cfg.chains {
            let slug = match dexscreener_chain_slug(chain_id) {
                Some(s) => s,
                None => continue, // defensive — config already filtered these out
            };

            let tokens = match self.fetch_active_tokens(chain_id).await {
                Ok(t) => t,
                Err(e) => {
                    warn!(event = "dexscreener.fetch_tokens_err", chain_id, err = %e);
                    continue;
                }
            };
            if tokens.is_empty() {
                debug!(event = "dexscreener.no_active_tokens", chain_id);
                continue;
            }
            stats.tokens_considered += tokens.len();

            // address(lc) -> canonical UPPERCASE symbol (from PG `tokens.symbol`).
            let sym_by_addr: HashMap<&str, &str> = tokens
                .iter()
                .map(|(a, s)| (a.as_str(), s.as_str()))
                .collect();
            let addrs: Vec<&str> = tokens.iter().map(|(a, _)| a.as_str()).collect();

            // symbol -> (price, liquidity). Deduplicated by symbol keeping the
            // DEEPER-liquidity price (see `merge_priced`): the PG `tokens` table
            // is UNIQUE on (chain_id, address) NOT (chain_id, symbol), so two
            // distinct active addresses can legitimately share a symbol. This
            // mirrors price_worker's one-address-per-symbol invariant and keeps
            // the HSET deterministic instead of order-dependent last-writer-wins.
            let mut priced: HashMap<String, (f64, f64)> = HashMap::new();
            for chunk in addrs.chunks(MAX_ADDRS_PER_REQUEST) {
                if !first_request {
                    tokio::time::sleep(INTER_CHUNK_DELAY).await;
                }
                first_request = false;
                match self.query_chunk(chunk).await {
                    Ok(pairs) => {
                        stats.requests_ok += 1;
                        for &addr in chunk {
                            if let Some((price, liq)) =
                                select_best_pair(&pairs, addr, slug, self.cfg.min_liquidity_usd)
                            {
                                if let Some(&sym) = sym_by_addr.get(addr) {
                                    if !merge_priced(&mut priced, sym.to_string(), price, liq) {
                                        debug!(
                                            event = "dexscreener.symbol_collision",
                                            chain_id, symbol = %sym,
                                            "two active addresses map to one symbol — kept the deeper-liquidity price"
                                        );
                                    }
                                }
                            }
                        }
                    }
                    Err(e) => {
                        stats.requests_err += 1;
                        warn!(
                            event = "dexscreener.request_err",
                            chain_id, err = %e,
                            "skipping chunk fail-honestly (no fabricated prices)"
                        );
                    }
                }
            }

            if priced.is_empty() {
                debug!(
                    event = "dexscreener.no_prices_resolved",
                    chain_id,
                    considered = tokens.len()
                );
                continue;
            }
            let flat: Vec<(String, f64)> = priced.into_iter().map(|(s, (p, _))| (s, p)).collect();
            match self.write_prices(conn, chain_id, &flat).await {
                Ok(written) => {
                    stats.prices_written += written;
                    info!(
                        event = "dexscreener.chain_priced",
                        chain_id,
                        slug,
                        considered = tokens.len(),
                        priced = written,
                    );
                }
                Err(e) => {
                    stats.write_errors += 1;
                    warn!(event = "dexscreener.write_err", chain_id, err = %e);
                }
            }
        }
        stats
    }

    /// Active tokens for a chain, sourced from the enricher's own Postgres.
    /// Same predicate `pool_sync_worker::bootstrap_token_cache` uses, so the
    /// symbol we price under is identical to the scanner's meta symbol.
    /// Returns `(address_lowercase, symbol_UPPERCASE)`.
    async fn fetch_active_tokens(&self, chain_id: u64) -> Result<Vec<(String, String)>> {
        let rows = sqlx::query_as::<_, (String, Option<String>)>(
            r#"SELECT address, symbol
               FROM tokens
               WHERE chain_id = $1 AND is_active = TRUE
                 AND symbol IS NOT NULL AND symbol <> ''"#,
        )
        .bind(chain_id as i64)
        .fetch_all(&self.db)
        .await
        .context("query active tokens for DexScreener pricing")?;

        let out = rows
            .into_iter()
            .filter_map(|(addr, sym)| {
                let sym = sym?;
                let sym = sym.trim();
                let addr = addr.trim();
                if sym.is_empty() || addr.is_empty() {
                    return None;
                }
                Some((addr.to_ascii_lowercase(), sym.to_ascii_uppercase()))
            })
            .collect();
        Ok(out)
    }

    /// One DexScreener token request for up to `MAX_ADDRS_PER_REQUEST` addresses.
    async fn query_chunk(&self, addresses: &[&str]) -> Result<Vec<DexPair>> {
        let joined = addresses.join(",");
        let url = format!("{DEXSCREENER_TOKENS_URL}{joined}");
        let resp = self
            .http
            .get(&url)
            .send()
            .await
            .context("DexScreener GET")?;
        let status = resp.status();
        if !status.is_success() {
            anyhow::bail!("DexScreener HTTP {status}");
        }
        let parsed: DexScreenerResponse = resp.json().await.context("DexScreener JSON parse")?;
        Ok(parsed.pairs)
    }

    /// Upsert `(symbol -> price)` into `arbx:token_prices:<chain>` and refresh
    /// the key TTL — the exact contract `price_worker::persist_prices` uses, so
    /// our writes coexist with the existing Chainlink/Alchemy/CoinGecko writes
    /// in the same hash (last-writer-wins per symbol; DexScreener fills the gaps
    /// those sources don't cover).
    /// Returns the number of fields actually HSET (after the defensive
    /// finite/positive re-filter) so the cycle metric reflects writes, not
    /// submissions.
    async fn write_prices(
        &self,
        conn: &mut redis::aio::MultiplexedConnection,
        chain_id: u64,
        prices: &[(String, f64)],
    ) -> Result<usize> {
        if prices.is_empty() {
            return Ok(0);
        }
        let key = redis_token_prices_key(chain_id);
        // TTL = interval*3 floored at 60s, mirroring price_worker's (period*2).max(60).
        // NOTE (cross-writer TTL coupling): Redis < 7.4 has no per-field hash TTL,
        // so this EXPIRE governs the lifetime of EVERY field in the shared hash —
        // including price_worker's. Keep DEXSCREENER_PRICE_INTERVAL_MS small (the
        // default 15s → 60s TTL) so this never over-extends price_worker's fields;
        // run() warns at boot if it grows large.
        let ttl_secs: i64 = ((self.cfg.interval.as_secs() as i64).saturating_mul(3)).max(60);

        let mut pipe = redis::pipe();
        pipe.atomic();
        let mut written = 0usize;
        for (sym, price) in prices {
            // Defensive: never write garbage (the reader also drops these, but
            // honesty starts at the writer).
            if !(price.is_finite() && *price > 0.0) {
                continue;
            }
            pipe.hset(&key, sym, format!("{price}")).ignore();
            written += 1;
        }
        if written == 0 {
            // Nothing valid to write — don't touch the key (no EXPIRE, no clobber).
            return Ok(0);
        }
        pipe.expire(&key, ttl_secs).ignore();
        pipe.query_async::<_, ()>(conn)
            .await
            .context("redis HSET token prices pipeline")?;
        Ok(written)
    }
}

/// Merge a resolved `(symbol, price, liquidity)` into the per-symbol map, keeping
/// the DEEPER-liquidity price on collision. Returns `true` if it inserted or
/// replaced, `false` if an existing deeper-liquidity entry was kept.
///
/// The PG `tokens` table is UNIQUE on (chain_id, address), NOT (chain_id, symbol),
/// so two distinct active addresses can legitimately share a symbol (a bridged
/// variant, or a clone). Resolving deterministically to the deepest pool mirrors
/// `price_worker`'s one-address-per-symbol invariant and keeps the HSET output
/// independent of PG row order.
fn merge_priced(map: &mut HashMap<String, (f64, f64)>, sym: String, price: f64, liq: f64) -> bool {
    match map.get(&sym) {
        Some(&(_, cur_liq)) if cur_liq >= liq => false,
        _ => {
            map.insert(sym, (price, liq));
            true
        }
    }
}

/// Pick `(USD price, liquidity)` of the highest-liquidity qualifying pair for
/// `address_lc`.
///
/// Qualifying = pair on the right chain slug, base token == this address, pair
/// liquidity ≥ `min_liq`, and a finite positive `priceUsd`. Returns `None` when
/// nothing qualifies (fail-honest: caller writes no price for that token). The
/// liquidity is returned so the caller can deterministically resolve two active
/// addresses that share a symbol (deeper pool wins — see `merge_priced`).
fn select_best_pair(
    pairs: &[DexPair],
    address_lc: &str,
    slug: &str,
    min_liq: f64,
) -> Option<(f64, f64)> {
    let mut best: Option<(f64 /* liquidity */, f64 /* price */)> = None;
    for p in pairs {
        if p.chain_id != slug {
            continue;
        }
        if !p.base_token.address.trim().eq_ignore_ascii_case(address_lc) {
            continue;
        }
        let liq = p.liquidity.as_ref().and_then(|l| l.usd).unwrap_or(0.0);
        if !(liq.is_finite() && liq >= min_liq) {
            continue;
        }
        let price = match p
            .price_usd
            .as_deref()
            .and_then(|s| s.trim().parse::<f64>().ok())
        {
            Some(v) if v.is_finite() && v > 0.0 => v,
            _ => continue,
        };
        match best {
            Some((best_liq, _)) if best_liq >= liq => {}
            _ => best = Some((liq, price)),
        }
    }
    best.map(|(liq, price)| (price, liq))
}

// ---------------------------------------------------------------------------
// Unit tests (pure — no network, no DB, no env mutation)
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn pair(chain: &str, addr: &str, price: Option<&str>, liq: Option<f64>) -> DexPair {
        DexPair {
            chain_id: chain.to_string(),
            base_token: DexToken {
                address: addr.to_string(),
                symbol: "X".to_string(),
            },
            price_usd: price.map(|s| s.to_string()),
            liquidity: liq.map(|u| DexLiquidity { usd: Some(u) }),
        }
    }

    // --- dexscreener_chain_slug ---

    #[test]
    fn slug_known_chains() {
        assert_eq!(dexscreener_chain_slug(1), Some("ethereum"));
        assert_eq!(dexscreener_chain_slug(56), Some("bsc"));
        assert_eq!(dexscreener_chain_slug(42161), Some("arbitrum"));
        assert_eq!(dexscreener_chain_slug(8453), Some("base"));
    }

    #[test]
    fn slug_unknown_chain_is_none() {
        assert_eq!(dexscreener_chain_slug(999_999), None);
    }

    // --- DexScreenerConfig::from_values ---

    #[test]
    fn config_off_when_gate_absent_or_other() {
        assert!(DexScreenerConfig::from_values("", None, None, &[1])
            .unwrap()
            .is_none());
        assert!(DexScreenerConfig::from_values("off", None, None, &[1])
            .unwrap()
            .is_none());
        assert!(DexScreenerConfig::from_values("shadow", None, None, &[1])
            .unwrap()
            .is_none());
    }

    #[test]
    fn config_active_defaults() {
        let cfg = DexScreenerConfig::from_values("active", None, None, &[1, 137])
            .unwrap()
            .unwrap();
        assert_eq!(cfg.chains, vec![1, 137]);
        assert_eq!(cfg.min_liquidity_usd, DEFAULT_MIN_LIQ_USD);
        assert_eq!(cfg.interval, Duration::from_millis(DEFAULT_INTERVAL_MS));
    }

    #[test]
    fn config_active_case_insensitive_and_custom_values() {
        let cfg = DexScreenerConfig::from_values("ACTIVE", Some("25000"), Some("30000"), &[1])
            .unwrap()
            .unwrap();
        assert_eq!(cfg.min_liquidity_usd, 25_000.0);
        assert_eq!(cfg.interval, Duration::from_millis(30_000));
    }

    #[test]
    fn config_filters_unsupported_chains() {
        let cfg = DexScreenerConfig::from_values("active", None, None, &[1, 999_999, 137])
            .unwrap()
            .unwrap();
        assert_eq!(cfg.chains, vec![1, 137]); // 999999 dropped
    }

    #[test]
    fn config_errors_when_no_chain_supported() {
        assert!(DexScreenerConfig::from_values("active", None, None, &[999_999]).is_err());
    }

    #[test]
    fn config_interval_floor_and_bad_values_fall_back() {
        // Below floor -> default.
        let cfg = DexScreenerConfig::from_values("active", Some("abc"), Some("10"), &[1])
            .unwrap()
            .unwrap();
        assert_eq!(cfg.min_liquidity_usd, DEFAULT_MIN_LIQ_USD);
        assert_eq!(cfg.interval, Duration::from_millis(DEFAULT_INTERVAL_MS));
    }

    #[test]
    fn config_negative_min_liq_falls_back() {
        let cfg = DexScreenerConfig::from_values("active", Some("-5"), None, &[1])
            .unwrap()
            .unwrap();
        assert_eq!(cfg.min_liquidity_usd, DEFAULT_MIN_LIQ_USD);
    }

    // --- select_best_pair + merge_priced ---

    const ADDR: &str = shared_rs::chains::WETH_MAINNET_LC;

    /// Test helper: extract just the price (production also uses the liquidity
    /// for cross-address symbol dedup, exercised in merge_priced* tests).
    fn best_price(pairs: &[DexPair], addr: &str, slug: &str, min_liq: f64) -> Option<f64> {
        select_best_pair(pairs, addr, slug, min_liq).map(|(p, _)| p)
    }

    #[test]
    fn best_price_picks_highest_liquidity() {
        let pairs = vec![
            pair("ethereum", ADDR, Some("2500.0"), Some(100_000.0)),
            pair("ethereum", ADDR, Some("2510.0"), Some(900_000.0)), // deepest
            pair("ethereum", ADDR, Some("2490.0"), Some(60_000.0)),
        ];
        assert_eq!(best_price(&pairs, ADDR, "ethereum", 50_000.0), Some(2510.0));
    }

    #[test]
    fn select_best_pair_returns_price_and_liquidity() {
        let pairs = vec![
            pair("ethereum", ADDR, Some("2500.0"), Some(100_000.0)),
            pair("ethereum", ADDR, Some("2510.0"), Some(900_000.0)), // deepest
        ];
        // The deeper pool's price AND its liquidity are both returned so the
        // caller can dedup same-symbol addresses by depth.
        assert_eq!(
            select_best_pair(&pairs, ADDR, "ethereum", 50_000.0),
            Some((2510.0, 900_000.0))
        );
    }

    #[test]
    fn best_price_filters_below_min_liquidity() {
        let pairs = vec![pair("ethereum", ADDR, Some("2500.0"), Some(10_000.0))];
        assert_eq!(best_price(&pairs, ADDR, "ethereum", 50_000.0), None);
    }

    #[test]
    fn best_price_filters_wrong_chain() {
        let pairs = vec![pair("bsc", ADDR, Some("2500.0"), Some(900_000.0))];
        assert_eq!(best_price(&pairs, ADDR, "ethereum", 50_000.0), None);
    }

    #[test]
    fn best_price_filters_wrong_address() {
        let pairs = vec![pair("ethereum", "0xdead", Some("2500.0"), Some(900_000.0))];
        assert_eq!(best_price(&pairs, ADDR, "ethereum", 50_000.0), None);
    }

    #[test]
    fn best_price_address_match_is_case_insensitive() {
        let upper = ADDR.to_ascii_uppercase();
        let pairs = vec![pair("ethereum", &upper, Some("2500.0"), Some(900_000.0))];
        assert_eq!(best_price(&pairs, ADDR, "ethereum", 50_000.0), Some(2500.0));
    }

    #[test]
    fn best_price_rejects_non_positive_and_non_finite() {
        let pairs = vec![
            pair("ethereum", ADDR, Some("0"), Some(900_000.0)),
            pair("ethereum", ADDR, Some("-1.5"), Some(900_000.0)),
            pair("ethereum", ADDR, Some("not-a-number"), Some(900_000.0)),
            pair("ethereum", ADDR, None, Some(900_000.0)),
        ];
        assert_eq!(best_price(&pairs, ADDR, "ethereum", 50_000.0), None);
    }

    #[test]
    fn best_price_handles_missing_liquidity_as_zero() {
        let pairs = vec![pair("ethereum", ADDR, Some("2500.0"), None)];
        assert_eq!(best_price(&pairs, ADDR, "ethereum", 0.0), Some(2500.0));
        // With a positive floor, missing liquidity (=0) is filtered out.
        assert_eq!(best_price(&pairs, ADDR, "ethereum", 1.0), None);
    }

    #[test]
    fn best_price_empty_pairs_is_none() {
        assert_eq!(best_price(&[], ADDR, "ethereum", 50_000.0), None);
    }

    #[test]
    fn merge_priced_keeps_deeper_liquidity() {
        let mut m: HashMap<String, (f64, f64)> = HashMap::new();
        // First address for USDC.
        assert!(merge_priced(&mut m, "USDC".to_string(), 1.00, 100_000.0));
        // Second, SHALLOWER pool for the same symbol — rejected; deeper kept.
        assert!(!merge_priced(&mut m, "USDC".to_string(), 0.98, 50_000.0));
        assert_eq!(m.get("USDC"), Some(&(1.00, 100_000.0)));
        // Third, DEEPER pool for the same symbol — replaces deterministically.
        assert!(merge_priced(&mut m, "USDC".to_string(), 1.01, 900_000.0));
        assert_eq!(m.get("USDC"), Some(&(1.01, 900_000.0)));
        // Distinct symbol — inserts independently.
        assert!(merge_priced(&mut m, "WETH".to_string(), 2500.0, 1_000.0));
        assert_eq!(m.len(), 2);
    }

    // --- response deserialization (real-ish DexScreener shape) ---

    #[test]
    fn deserialize_dexscreener_response_tolerates_partial_pairs() {
        let raw = r#"{
            "schemaVersion": "1.0.0",
            "pairs": [
                {
                    "chainId": "ethereum",
                    "dexId": "uniswap",
                    "baseToken": { "address": "0xABC", "symbol": "ABC", "name": "Token" },
                    "quoteToken": { "address": "0xDEF", "symbol": "WETH" },
                    "priceUsd": "1.2345",
                    "liquidity": { "usd": 123456.7, "base": 1.0, "quote": 2.0 }
                },
                {
                    "chainId": "bsc",
                    "baseToken": { "address": "0x999", "symbol": "XYZ" },
                    "priceUsd": null
                }
            ]
        }"#;
        let parsed: DexScreenerResponse = serde_json::from_str(raw).unwrap();
        assert_eq!(parsed.pairs.len(), 2);
        assert_eq!(parsed.pairs[0].chain_id, "ethereum");
        assert_eq!(parsed.pairs[0].base_token.address, "0xABC");
        assert_eq!(parsed.pairs[0].price_usd.as_deref(), Some("1.2345"));
        assert_eq!(
            parsed.pairs[0].liquidity.as_ref().unwrap().usd,
            Some(123456.7)
        );
        assert!(parsed.pairs[1].price_usd.is_none());
        assert!(parsed.pairs[1].liquidity.is_none());
    }

    #[test]
    fn deserialize_empty_pairs() {
        let parsed: DexScreenerResponse = serde_json::from_str(r#"{"pairs":[]}"#).unwrap();
        assert!(parsed.pairs.is_empty());
        // Missing "pairs" entirely also yields empty (serde default).
        let parsed2: DexScreenerResponse = serde_json::from_str(r#"{}"#).unwrap();
        assert!(parsed2.pairs.is_empty());
    }
}
