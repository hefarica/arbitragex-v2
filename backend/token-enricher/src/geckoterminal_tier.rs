//! GeckoTerminal price + icon tier — a peer of `dexscreener.rs` that fills the
//! `no_price_oracle` gap AND produces token icons in one pass.
//!
//! ## Why this exists
//!
//! GeckoTerminal's `tokens/multi` endpoint returns, for up to 30 token addresses
//! per call, BOTH `price_usd` and `image_url` (plus `total_reserve_in_usd`, a
//! cross-pool liquidity proxy). So one HTTP call feeds two consumers with zero
//! extra cost:
//!   - PRICES → the SAME `arbx:token_prices:<chain>` hash the scanner reads
//!     (keyed by canonical uppercase symbol from the PG `tokens` table) — exactly
//!     like [`crate::dexscreener`], so it needs ZERO scanner changes and coexists
//!     with the DexScreener tier and `price_worker` (peer writers, real prices).
//!   - ICONS → `arbx:token-icons:{chain}:{addr}` via SETEX (raw image URL) — the
//!     EXACT contract the api-server reader (`routes/token-icon.ts`) names as its
//!     Redis hot-path tier. When this populates, that cascade's Redis tier lights
//!     up with zero frontend change.
//!
//! ## Relationship to the DexScreener tier
//!
//! Both are independent, env-gated WRITERS into `arbx:token_prices` (last-writer-
//! wins per symbol; both write REAL prices). They are NOT a reader cascade. To
//! prefer one source, the operator runs that gate and leaves the other off; to
//! widen coverage, run both. GeckoTerminal additionally produces icons, which
//! DexScreener (in this service) does not.
//!
//! ## Verified API contract (live, 2026-06-05)
//!
//! `GET https://api.geckoterminal.com/api/v2/networks/{slug}/tokens/multi/{a,b,..}`
//! → `{ "data": [ { "attributes": { address, symbol, decimals, image_url,
//! price_usd (decimal string), total_reserve_in_usd (decimal string) } } ] }`.
//! Free tier ≈ 30 req/min → ≥2.0s between requests.
//!
//! ## Doctrine
//!
//! Read-only / shadow. No signer, no private key, no broadcast, no live trading,
//! no contract calls. DEFAULT-OFF: does nothing unless `ARBX_GECKOTERMINAL_ORACLE`
//! is exactly `active`. Fail-honest: any HTTP/parse/Redis/coverage failure
//! degrades silently (skip token/chunk/cycle) — never writes a fabricated or
//! non-positive price, never writes a non-http icon URL, never clears existing
//! data. Idempotent: prices are an HSET upsert by symbol; icons are SETEX upserts
//! by address. Rate-limited: ≤30 addresses/request, ~2.1s between EVERY request
//! (cycle-wide), comfortably inside the ~30 req/min free-tier ceiling.

use anyhow::{Context, Result};
use serde::Deserialize;
use sqlx::PgPool;
use std::collections::HashMap;
use std::time::Duration;
use tracing::{debug, info, warn};

use shared_rs::price_oracle::redis_token_prices_key;

/// GeckoTerminal API base; final URL is `{base}{slug}/tokens/multi/{a,b,c}`.
const GT_BASE_URL: &str = "https://api.geckoterminal.com/api/v2/networks/";
/// GeckoTerminal `tokens/multi` accepts up to 30 comma-separated addresses.
const MAX_ADDRS_PER_REQUEST: usize = 30;
/// Delay before every request after the first in a cycle. Free tier ≈ 30 req/min
/// (1 per 2s); 2.1s keeps a comfortable margin across chunks AND chains.
const INTER_REQUEST_DELAY: Duration = Duration::from_millis(2100);
/// HTTP request timeout for a single GeckoTerminal call.
const HTTP_TIMEOUT: Duration = Duration::from_secs(12);

// --- env keys ---
/// Off-switch. Tier runs ONLY when this equals (case-insensitively) `active`.
const ENV_GATE: &str = "ARBX_GECKOTERMINAL_ORACLE";
/// Minimum `total_reserve_in_usd` to trust a price. Reuses the DexScreener tier's
/// knob, but NOTE the gated quantities differ: here it is GeckoTerminal's
/// CROSS-POOL aggregate reserve (summed over all of the token's pools), which is
/// structurally LOOSER than DexScreener's single-pair liquidity at the same
/// numeric threshold. Default 50_000.
const ENV_MIN_LIQ: &str = "ARBX_MIN_PRICE_LIQUIDITY_USD";
/// Poll interval in ms. Default 60_000 (GeckoTerminal is more rate-limited than
/// DexScreener). Floored to 2_000 to honour the free-tier ceiling.
const ENV_INTERVAL: &str = "GECKOTERMINAL_PRICE_INTERVAL_MS";
/// Icon TTL in seconds for the SETEX. Default 3_600 (matches the reader's
/// documented `ARBX_TOKEN_ICON_TTL_SECS` contract). Floored to 60.
const ENV_ICON_TTL: &str = "ARBX_GECKOTERMINAL_ICON_TTL_SECS";

const DEFAULT_MIN_LIQ_USD: f64 = 50_000.0;
const DEFAULT_INTERVAL_MS: u64 = 60_000;
const MIN_INTERVAL_MS: u64 = 2_000;
const DEFAULT_ICON_TTL_SECS: i64 = 3_600;
const MIN_ICON_TTL_SECS: i64 = 60;

/// Map a numeric chain id to GeckoTerminal's network slug.
///
/// GeckoTerminal API reference metadata (a fixed property of their API, like an
/// endpoint path), NOT operator/productive config — so a const map, not env.
/// These slugs DIFFER from DexScreener's (e.g. 137 = "polygon_pos" here vs
/// "polygon" there). Unknown chains return `None` and are skipped fail-honestly.
fn gt_network_slug(chain_id: u64) -> Option<&'static str> {
    Some(match chain_id {
        1 => "eth",
        10 => "optimism",
        56 => "bsc",
        137 => "polygon_pos",
        250 => "ftm",
        8453 => "base",
        42161 => "arbitrum",
        43114 => "avax",
        59144 => "linea",
        534352 => "scroll",
        5000 => "mantle",
        _ => return None,
    })
}

/// Producer side of the icon contract read by api-server `routes/token-icon.ts`:
/// `arbx:token-icons:{chainId}:{address}` with `address` LOWERCASE.
fn token_icon_key(chain_id: u64, address_lc: &str) -> String {
    format!("arbx:token-icons:{chain_id}:{address_lc}")
}

/// True for `http://`/`https://` URLs only — mirrors the reader's `isHttpUrl`
/// gate so we never write a non-URL (e.g. GeckoTerminal's "missing.png" sentinel)
/// into the icon cache.
fn is_http_url(s: &str) -> bool {
    let lower = s.trim().to_ascii_lowercase();
    lower.starts_with("https://") || lower.starts_with("http://")
}

/// Parse a GeckoTerminal decimal-string field into a finite positive f64, or
/// `None` (fail-honest: a missing/zero/negative/garbage value yields no price).
fn parse_pos_f64(opt: &Option<String>) -> Option<f64> {
    opt.as_deref()
        .and_then(|s| s.trim().parse::<f64>().ok())
        .filter(|v| v.is_finite() && *v > 0.0)
}

/// Shared-hash price TTL (seconds). Redis < 7.4 has no per-field hash TTL, so this
/// governs EVERY field in arbx:token_prices (price_worker's + the DexScreener
/// tier's too). interval*2 gives ~2 GeckoTerminal cycles of survival while staying
/// modest over price_worker's 60s floor; floored at 60s. Used by BOTH run()'s
/// boot guard and write_prices so the warn threshold can never drift from the
/// value actually written.
fn price_ttl_secs(interval: Duration) -> i64 {
    ((interval.as_secs() as i64).saturating_mul(2)).max(60)
}

// ---------------------------------------------------------------------------
// Config
// ---------------------------------------------------------------------------

#[derive(Clone, Debug)]
pub struct GeckoTerminalConfig {
    pub chains: Vec<u64>,
    pub min_liquidity_usd: f64,
    pub interval: Duration,
    pub icon_ttl_secs: i64,
}

impl GeckoTerminalConfig {
    /// Read config from env. `Ok(None)` (default-safe) unless the gate is exactly
    /// `active`. Errors only when active but no enabled chain maps to a slug.
    pub fn from_env(enabled_chains: &[u64]) -> Result<Option<Self>> {
        let gate = std::env::var(ENV_GATE).unwrap_or_default();
        let min_liq = std::env::var(ENV_MIN_LIQ).ok();
        let interval = std::env::var(ENV_INTERVAL).ok();
        let icon_ttl = std::env::var(ENV_ICON_TTL).ok();
        // Surface unsupported chains once (the pure helper drops them silently).
        if gate.trim().eq_ignore_ascii_case("active") {
            for &c in enabled_chains {
                if gt_network_slug(c).is_none() {
                    warn!(
                        event = "geckoterminal.chain_unsupported",
                        chain_id = c,
                        "no GeckoTerminal network slug for this chain — it will be skipped"
                    );
                }
            }
        }
        Self::from_values(
            &gate,
            min_liq.as_deref(),
            interval.as_deref(),
            icon_ttl.as_deref(),
            enabled_chains,
        )
    }

    /// Pure config parse (no env access) — unit-testable.
    fn from_values(
        gate: &str,
        min_liq: Option<&str>,
        interval: Option<&str>,
        icon_ttl: Option<&str>,
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
        let icon_ttl_secs = icon_ttl
            .and_then(|v| v.trim().parse::<i64>().ok())
            .filter(|v| *v >= MIN_ICON_TTL_SECS)
            .unwrap_or(DEFAULT_ICON_TTL_SECS);
        let chains: Vec<u64> = enabled_chains
            .iter()
            .copied()
            .filter(|c| gt_network_slug(*c).is_some())
            .collect();
        if chains.is_empty() {
            anyhow::bail!(
                "ARBX_GECKOTERMINAL_ORACLE=active but none of the enabled chains \
                 map to a GeckoTerminal network slug (supported: 1,10,56,137,250,\
                 8453,42161,43114,59144,534352,5000)"
            );
        }
        Ok(Some(Self {
            chains,
            min_liquidity_usd,
            interval: Duration::from_millis(interval_ms),
            icon_ttl_secs,
        }))
    }
}

// ---------------------------------------------------------------------------
// GeckoTerminal response shapes (JSON:API; only the fields we consume)
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
struct GtResponse {
    #[serde(default)]
    data: Vec<GtToken>,
}

#[derive(Debug, Deserialize)]
struct GtToken {
    #[serde(default)]
    attributes: GtAttributes,
}

#[derive(Debug, Default, Deserialize)]
struct GtAttributes {
    #[serde(default)]
    address: String,
    #[allow(dead_code)] // captured for completeness; prices key by PG symbol
    #[serde(default)]
    symbol: String,
    #[serde(default)]
    image_url: Option<String>,
    #[serde(default)]
    price_usd: Option<String>,
    #[serde(default)]
    total_reserve_in_usd: Option<String>,
}

/// Merge a resolved `(symbol, price, liquidity)` into the per-symbol map, keeping
/// the DEEPER-reserve price on collision (PG `tokens` is unique on address, not
/// symbol — two active addresses can share a symbol). Returns `true` if it
/// inserted/replaced. Mirrors `dexscreener::merge_priced`.
fn merge_priced(map: &mut HashMap<String, (f64, f64)>, sym: String, price: f64, liq: f64) -> bool {
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
    tokens_considered: usize,
    requests_ok: usize,
    requests_err: usize,
    prices_written: usize,
    icons_written: usize,
    write_errors: usize,
}

pub struct GeckoTerminalOracle {
    cfg: GeckoTerminalConfig,
    db: PgPool,
    redis_url: String,
    http: reqwest::Client,
    /// G-PRICE-5 circuit breaker (CG429-01 mirror): 3 consecutive 429s →
    /// open for 300s. Stops the log-flood AND the wasted request cycles.
    consecutive_429s: std::sync::atomic::AtomicU32,
    breaker_until_ms: std::sync::atomic::AtomicI64,
}

impl GeckoTerminalOracle {
    pub fn new(cfg: GeckoTerminalConfig, db: PgPool, redis_url: String) -> Result<Self> {
        let http = reqwest::Client::builder()
            .user_agent("arbitragex-token-enricher/0.1 (geckoterminal-price-icon-tier; shadow)")
            .timeout(HTTP_TIMEOUT)
            .build()
            .context("build reqwest client for GeckoTerminal")?;
        Ok(Self {
            cfg,
            db,
            redis_url,
            http,
            consecutive_429s: std::sync::atomic::AtomicU32::new(0),
            breaker_until_ms: std::sync::atomic::AtomicI64::new(0),
        })
    }

    /// Detached run loop. Ticks immediately, then every `cfg.interval`. Holds a
    /// Redis connection across cycles, reconnecting after a write error. Relies on
    /// process exit for shutdown (same detached idiom as `serve_metrics`).
    pub async fn run(self) {
        // Cross-writer TTL coupling guard: price_ttl_secs governs the WHOLE shared
        // arbx:token_prices hash (Redis <7.4 has no per-field TTL — price_worker's
        // fields too). Warn if a large interval pushes it well past the 60s floor.
        let ttl_secs = price_ttl_secs(self.cfg.interval);
        if ttl_secs > 180 {
            warn!(
                event = "geckoterminal.large_ttl",
                ttl_secs,
                interval_ms = self.cfg.interval.as_millis() as u64,
                "GECKOTERMINAL_PRICE_INTERVAL_MS is large — its TTL governs the SHARED token-prices hash for ALL writers; keep it modest so price_worker stays the binding TTL floor"
            );
        }
        info!(
            event = "geckoterminal.tier_started",
            chains = ?self.cfg.chains,
            min_liquidity_usd = self.cfg.min_liquidity_usd,
            interval_ms = self.cfg.interval.as_millis() as u64,
            icon_ttl_secs = self.cfg.icon_ttl_secs,
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
                        warn!(event = "geckoterminal.redis_connect_err", err = %e, "skipping cycle");
                        continue;
                    }
                }
            }
            let c = conn.as_mut().expect("redis connection present");
            let stats = self.run_once(c).await;
            info!(
                event = "geckoterminal.cycle_done",
                tokens_considered = stats.tokens_considered,
                prices_written = stats.prices_written,
                icons_written = stats.icons_written,
                requests_ok = stats.requests_ok,
                requests_err = stats.requests_err,
                write_errors = stats.write_errors,
            );
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
        // Cycle-wide request throttle: ~2.1s between EVERY request, across chunks
        // AND chains. The very first request of the cycle fires immediately.
        let mut first_request = true;
        for &chain_id in &self.cfg.chains {
            let slug = match gt_network_slug(chain_id) {
                Some(s) => s,
                None => continue, // defensive — config already filtered these out
            };

            let tokens = match self.fetch_active_tokens(chain_id).await {
                Ok(t) => t,
                Err(e) => {
                    warn!(event = "geckoterminal.fetch_tokens_err", chain_id, err = %e);
                    continue;
                }
            };
            if tokens.is_empty() {
                debug!(event = "geckoterminal.no_active_tokens", chain_id);
                continue;
            }
            stats.tokens_considered += tokens.len();

            // address(lc) -> canonical UPPERCASE symbol (from PG `tokens.symbol`).
            let sym_by_addr: HashMap<&str, &str> = tokens
                .iter()
                .map(|(a, s)| (a.as_str(), s.as_str()))
                .collect();
            let addrs: Vec<&str> = tokens.iter().map(|(a, _)| a.as_str()).collect();

            // symbol -> (price, reserve); deduped by deepest reserve (see merge_priced).
            let mut priced: HashMap<String, (f64, f64)> = HashMap::new();
            // address_lc -> image_url for valid http icons, deduped by address so
            // the SETEX count equals the distinct-address count (icons key by
            // address, which is already unique — no symbol-collision concern).
            let mut icons: HashMap<String, String> = HashMap::new();

            for chunk in addrs.chunks(MAX_ADDRS_PER_REQUEST) {
                if !first_request {
                    tokio::time::sleep(INTER_REQUEST_DELAY).await;
                }
                first_request = false;
                let gt_tokens = match self.query_chunk(slug, chunk).await {
                    Ok(d) => {
                        stats.requests_ok += 1;
                        d
                    }
                    Err(e) => {
                        stats.requests_err += 1;
                        warn!(
                            event = "geckoterminal.request_err",
                            chain_id, err = %e,
                            "skipping chunk fail-honestly (no fabricated data)"
                        );
                        continue;
                    }
                };

                for tok in &gt_tokens {
                    let addr = tok.attributes.address.trim().to_ascii_lowercase();
                    // Only our active tokens (GT echoes what we queried, but guard).
                    let sym = match sym_by_addr.get(addr.as_str()) {
                        Some(&s) => s,
                        None => continue,
                    };

                    // --- price (requires finite>0 price AND reserve >= min_liq) ---
                    if let Some(price) = parse_pos_f64(&tok.attributes.price_usd) {
                        let reserve =
                            parse_pos_f64(&tok.attributes.total_reserve_in_usd).unwrap_or(0.0);
                        if reserve >= self.cfg.min_liquidity_usd
                            && !merge_priced(&mut priced, sym.to_string(), price, reserve)
                        {
                            debug!(
                                event = "geckoterminal.symbol_collision",
                                chain_id, symbol = %sym,
                                "two active addresses map to one symbol — kept the deeper-reserve price"
                            );
                        }
                    }

                    // --- icon (only a valid http(s) URL; no liquidity gate) ---
                    if let Some(img) = tok.attributes.image_url.as_deref() {
                        if is_http_url(img) {
                            icons.insert(addr, img.trim().to_string());
                        }
                    }
                }
            }

            let priced_n = priced.len();
            let icons_n = icons.len();

            if priced_n > 0 {
                let flat: Vec<(String, f64)> =
                    priced.into_iter().map(|(s, (p, _))| (s, p)).collect();
                match self.write_prices(conn, chain_id, &flat).await {
                    Ok(w) => stats.prices_written += w,
                    Err(e) => {
                        stats.write_errors += 1;
                        warn!(event = "geckoterminal.price_write_err", chain_id, err = %e);
                    }
                }
            }
            if icons_n > 0 {
                match self.write_icons(conn, chain_id, &icons).await {
                    Ok(w) => stats.icons_written += w,
                    Err(e) => {
                        stats.write_errors += 1;
                        warn!(event = "geckoterminal.icon_write_err", chain_id, err = %e);
                    }
                }
            }
            if priced_n == 0 && icons_n == 0 {
                debug!(
                    event = "geckoterminal.nothing_resolved",
                    chain_id,
                    considered = tokens.len()
                );
            } else {
                info!(
                    event = "geckoterminal.chain_done",
                    chain_id,
                    slug,
                    considered = tokens.len(),
                    priced = priced_n,
                    icons = icons_n,
                );
            }
        }
        stats
    }

    /// Active tokens for a chain from the enricher's own Postgres — same predicate
    /// as `pool_sync_worker::bootstrap_token_cache`, so the symbol we price under
    /// is identical to the scanner's meta symbol. Returns `(address_lc, SYM_UPPER)`.
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
        .context("query active tokens for GeckoTerminal pricing")?;

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

    /// One GeckoTerminal `tokens/multi` request for up to `MAX_ADDRS_PER_REQUEST`.
    /// G-PRICE-5: circuit breaker (CG429-01 mirror) — 3 consecutive 429s →
    /// skip ALL requests for 300s. One log line on open, one on close.
    async fn query_chunk(&self, slug: &str, addresses: &[&str]) -> Result<Vec<GtToken>> {
        use std::sync::atomic::Ordering;

        // Circuit breaker check: if open, skip the request entirely.
        let until = self.breaker_until_ms.load(Ordering::Relaxed);
        if until > 0 {
            let now_ms = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_millis() as i64)
                .unwrap_or(0);
            if now_ms < until {
                anyhow::bail!("geckoterminal circuit breaker open (G-PRICE-5) — request skipped");
            }
            // Breaker expired — reset and log one line.
            self.breaker_until_ms.store(0, Ordering::Relaxed);
            self.consecutive_429s.store(0, Ordering::Relaxed);
            tracing::info!(
                event = "geckoterminal.breaker_closed",
                "GeckoTerminal circuit breaker CLOSED — resuming requests after cooldown"
            );
        }

        let joined = addresses.join(",");
        let url = format!("{GT_BASE_URL}{slug}/tokens/multi/{joined}");
        let resp = self
            .http
            .get(&url)
            .header("accept", "application/json")
            .send()
            .await
            .context("GeckoTerminal GET")?;
        let status = resp.status();
        if status.as_u16() == 429 {
            let count = self.consecutive_429s.fetch_add(1, Ordering::Relaxed) + 1;
            if count >= 3 {
                // Open the breaker for 300s (CG429-01 mirror).
                let now_ms = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|d| d.as_millis() as i64)
                    .unwrap_or(0);
                self.breaker_until_ms
                    .store(now_ms + 300_000, Ordering::Relaxed);
                tracing::warn!(
                    event = "geckoterminal.breaker_open",
                    consecutive_429s = count,
                    cooldown_secs = 300,
                    "GeckoTerminal circuit breaker OPEN — 429 flood detected (G-PRICE-5, CG429-01 mirror)"
                );
            }
            warn!(
                event = "geckoterminal.rate_limited",
                consecutive = count,
                "GeckoTerminal 429 — chunk skipped fail-honestly"
            );
        } else if status.is_success() {
            // Successful request resets the counter.
            self.consecutive_429s.store(0, Ordering::Relaxed);
        }
        if !status.is_success() {
            anyhow::bail!("GeckoTerminal HTTP {status}");
        }
        let parsed: GtResponse = resp.json().await.context("GeckoTerminal JSON parse")?;
        Ok(parsed.data)
    }

    /// Upsert `(symbol -> price)` into `arbx:token_prices:<chain>` + refresh TTL —
    /// the exact contract `price_worker::persist_prices` and the DexScreener tier
    /// use. Returns the number of fields actually HSET.
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
        let ttl_secs: i64 = price_ttl_secs(self.cfg.interval);

        let mut pipe = redis::pipe();
        pipe.atomic();
        let mut written = 0usize;
        for (sym, price) in prices {
            if !(price.is_finite() && *price > 0.0) {
                continue;
            }
            pipe.hset(&key, sym, format!("{price}")).ignore();
            written += 1;
        }
        if written == 0 {
            return Ok(0);
        }
        pipe.expire(&key, ttl_secs).ignore();
        // G-PRICE-1: notify subscribers (api-server prices-stream bridge) in the
        // same atomic pipeline — the receiver re-reads the hash for the data.
        pipe.cmd("PUBLISH")
            .arg(shared_rs::price_oracle::prices_updated_channel(chain_id))
            .arg(
                serde_json::json!({
                    "source": "geckoterminal",
                    "chain_id": chain_id,
                    "written": written,
                })
                .to_string(),
            )
            .ignore();
        pipe.query_async::<_, ()>(conn)
            .await
            .context("redis HSET token prices pipeline")?;
        Ok(written)
    }

    /// SETEX each `arbx:token-icons:<chain>:<addr>` to the raw image URL — the
    /// producer side of the api-server `token-icon.ts` reader contract. Returns
    /// the number of keys written.
    async fn write_icons(
        &self,
        conn: &mut redis::aio::MultiplexedConnection,
        chain_id: u64,
        icons: &HashMap<String, String>,
    ) -> Result<usize> {
        if icons.is_empty() {
            return Ok(0);
        }
        let mut pipe = redis::pipe();
        let mut written = 0usize;
        for (addr, url) in icons {
            // Defensive: only ever cache a real http(s) URL (honesty at the writer).
            if !is_http_url(url) {
                continue;
            }
            let key = token_icon_key(chain_id, addr);
            pipe.cmd("SETEX")
                .arg(&key)
                .arg(self.cfg.icon_ttl_secs)
                .arg(url)
                .ignore();
            written += 1;
        }
        if written == 0 {
            return Ok(0);
        }
        pipe.query_async::<_, ()>(conn)
            .await
            .context("redis SETEX token icons pipeline")?;
        Ok(written)
    }
}

// ---------------------------------------------------------------------------
// Unit tests (pure — no network, no DB, no env mutation)
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use shared_rs::chains::WETH_MAINNET_LC;

    // --- gt_network_slug ---

    #[test]
    fn slug_known_chains() {
        assert_eq!(gt_network_slug(1), Some("eth"));
        assert_eq!(gt_network_slug(56), Some("bsc"));
        assert_eq!(gt_network_slug(137), Some("polygon_pos")); // NOTE: differs from DexScreener
        assert_eq!(gt_network_slug(42161), Some("arbitrum"));
        assert_eq!(gt_network_slug(8453), Some("base"));
    }

    #[test]
    fn slug_unknown_chain_is_none() {
        assert_eq!(gt_network_slug(999_999), None);
    }

    // --- token_icon_key (MUST match api-server token-icon.ts) ---

    #[test]
    fn icon_key_matches_reader_contract() {
        assert_eq!(
            token_icon_key(1, WETH_MAINNET_LC),
            format!("arbx:token-icons:1:{}", WETH_MAINNET_LC)
        );
        assert_eq!(token_icon_key(137, "0xabc"), "arbx:token-icons:137:0xabc");
    }

    // --- is_http_url (mirrors reader isHttpUrl) ---

    #[test]
    fn is_http_url_accepts_http_and_https() {
        assert!(is_http_url("https://coin-images.coingecko.com/x.png"));
        assert!(is_http_url("http://example.com/a.png"));
        assert!(is_http_url("  HTTPS://x.io/y.png  "));
    }

    #[test]
    fn is_http_url_rejects_non_urls() {
        assert!(!is_http_url("missing.png")); // GeckoTerminal's no-image sentinel
        assert!(!is_http_url(""));
        assert!(!is_http_url("/relative/path.png"));
        assert!(!is_http_url("ftp://x/y.png"));
        assert!(!is_http_url("data:image/png;base64,AAAA"));
    }

    // --- parse_pos_f64 ---

    #[test]
    fn parse_pos_f64_valid_and_invalid() {
        assert_eq!(
            parse_pos_f64(&Some("1593.0455682772".into())),
            Some(1593.0455682772)
        );
        assert_eq!(parse_pos_f64(&Some(" 2.5 ".into())), Some(2.5));
        assert_eq!(parse_pos_f64(&Some("0".into())), None);
        assert_eq!(parse_pos_f64(&Some("-1.0".into())), None);
        assert_eq!(parse_pos_f64(&Some("not-a-number".into())), None);
        assert_eq!(parse_pos_f64(&None), None);
    }

    // --- GeckoTerminalConfig::from_values ---

    #[test]
    fn config_off_when_gate_absent_or_other() {
        assert!(GeckoTerminalConfig::from_values("", None, None, None, &[1])
            .unwrap()
            .is_none());
        assert!(
            GeckoTerminalConfig::from_values("off", None, None, None, &[1])
                .unwrap()
                .is_none()
        );
    }

    #[test]
    fn config_active_defaults() {
        let cfg = GeckoTerminalConfig::from_values("active", None, None, None, &[1, 137])
            .unwrap()
            .unwrap();
        assert_eq!(cfg.chains, vec![1, 137]);
        assert_eq!(cfg.min_liquidity_usd, DEFAULT_MIN_LIQ_USD);
        assert_eq!(cfg.interval, Duration::from_millis(DEFAULT_INTERVAL_MS));
        assert_eq!(cfg.icon_ttl_secs, DEFAULT_ICON_TTL_SECS);
    }

    #[test]
    fn config_active_custom_values_case_insensitive() {
        let cfg = GeckoTerminalConfig::from_values(
            "ACTIVE",
            Some("25000"),
            Some("90000"),
            Some("7200"),
            &[1],
        )
        .unwrap()
        .unwrap();
        assert_eq!(cfg.min_liquidity_usd, 25_000.0);
        assert_eq!(cfg.interval, Duration::from_millis(90_000));
        assert_eq!(cfg.icon_ttl_secs, 7_200);
    }

    #[test]
    fn config_filters_unsupported_chains() {
        let cfg = GeckoTerminalConfig::from_values("active", None, None, None, &[1, 999_999, 137])
            .unwrap()
            .unwrap();
        assert_eq!(cfg.chains, vec![1, 137]);
    }

    #[test]
    fn config_errors_when_no_chain_supported() {
        assert!(GeckoTerminalConfig::from_values("active", None, None, None, &[999_999]).is_err());
    }

    #[test]
    fn config_floors_interval_and_icon_ttl() {
        // interval below 2000 floor -> default; icon ttl below 60 floor -> default.
        let cfg = GeckoTerminalConfig::from_values("active", None, Some("500"), Some("5"), &[1])
            .unwrap()
            .unwrap();
        assert_eq!(cfg.interval, Duration::from_millis(DEFAULT_INTERVAL_MS));
        assert_eq!(cfg.icon_ttl_secs, DEFAULT_ICON_TTL_SECS);
    }

    // --- merge_priced ---

    #[test]
    fn merge_priced_keeps_deeper_reserve() {
        let mut m: HashMap<String, (f64, f64)> = HashMap::new();
        assert!(merge_priced(&mut m, "USDC".to_string(), 1.00, 100_000.0));
        assert!(!merge_priced(&mut m, "USDC".to_string(), 0.98, 50_000.0)); // shallower kept out
        assert_eq!(m.get("USDC"), Some(&(1.00, 100_000.0)));
        assert!(merge_priced(&mut m, "USDC".to_string(), 1.01, 900_000.0)); // deeper replaces
        assert_eq!(m.get("USDC"), Some(&(1.01, 900_000.0)));
        assert!(merge_priced(&mut m, "WETH".to_string(), 2500.0, 1_000.0));
        assert_eq!(m.len(), 2);
    }

    // --- response deserialization (real GeckoTerminal shape, live-verified) ---

    #[test]
    fn deserialize_gt_response_real_shape() {
        let raw = r#"{
            "data": [
                {
                    "id": "eth_0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2",
                    "type": "token",
                    "attributes": {
                        "address": "0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2",
                        "name": "Wrapped Ether",
                        "symbol": "WETH",
                        "decimals": 18,
                        "image_url": "https://coin-images.coingecko.com/coins/images/2518/large/weth.png?1696503332",
                        "price_usd": "1593.0455682772",
                        "total_reserve_in_usd": "818020838.81"
                    }
                },
                {
                    "type": "token",
                    "attributes": {
                        "address": "0xNoImage",
                        "symbol": "NOIMG",
                        "image_url": "missing.png",
                        "price_usd": null,
                        "total_reserve_in_usd": null
                    }
                }
            ]
        }"#;
        let parsed: GtResponse = serde_json::from_str(raw).unwrap();
        assert_eq!(parsed.data.len(), 2);
        let a0 = &parsed.data[0].attributes;
        assert_eq!(a0.address, WETH_MAINNET_LC);
        assert_eq!(a0.symbol, "WETH");
        assert_eq!(parse_pos_f64(&a0.price_usd), Some(1593.0455682772));
        assert_eq!(parse_pos_f64(&a0.total_reserve_in_usd), Some(818020838.81));
        assert!(is_http_url(a0.image_url.as_deref().unwrap()));
        // second token: no usable price, and "missing.png" must be rejected as an icon
        let a1 = &parsed.data[1].attributes;
        assert_eq!(parse_pos_f64(&a1.price_usd), None);
        assert!(!is_http_url(a1.image_url.as_deref().unwrap()));
    }

    #[test]
    fn deserialize_empty_or_missing_data() {
        let p: GtResponse = serde_json::from_str(r#"{"data":[]}"#).unwrap();
        assert!(p.data.is_empty());
        let p2: GtResponse = serde_json::from_str(r#"{}"#).unwrap();
        assert!(p2.data.is_empty());
    }
}
