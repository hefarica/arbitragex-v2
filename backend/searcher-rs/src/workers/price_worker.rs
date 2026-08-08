//! PriceWorker — periodic background fetch of live USD token prices.
//!
//! Replaces the operator-toil ConfigPriceOracle as the PRIMARY price source
//! by populating Redis hash `arbx:token_prices:<chain_id>` every
//! `PRICE_WORKER_INTERVAL_SECS` (default 30s). The spine evaluator's
//! `RedisCachedPriceOracle` reads that snapshot sync on the hot path; the
//! cascade falls through to ConfigPriceOracle when the cache misses.
//!
//! ## Cascade resolution (full picture)
//!
//! ```text
//!  candidate token → CascadePriceOracle::price_usd(symbol)
//!     ├─ tier 1: RedisCachedPriceOracle  (THIS worker populates it)
//!     │             ├─ Alchemy Token Prices API (primary)
//!     │             └─ Coingecko simple/price   (fallback)
//!     ├─ tier 2: ConfigPriceOracle       (operator manual + stables + base)
//!     └─ None    → RejectReason::UnknownTokenPrice  (R8 fail-honest)
//! ```
//!
//! ## Sources
//!
//! 1. **Alchemy Token Prices** — `POST /prices/v1/{API_KEY}/tokens/by-address`
//!    body `{"addresses":[{"network":"eth-mainnet","address":"0x..."}, ...]}`.
//!    Returns one entry per token with `prices: [{"currency":"usd","value":"..."}]`.
//!    Rate-limit + cost: counts against the same Alchemy key as the RPC pool;
//!    one batched call per refresh is well under any tier's budget.
//!
//! 2. **Coingecko** — `GET /simple/token_price/ethereum?contract_addresses=...&vs_currencies=usd`.
//!    Free tier (no key). Used ONLY for tokens Alchemy returned no price for.
//!    Cost: one batched call per period iff at least one Alchemy miss.
//!
//! ## Failure modes (R8 fail-honest)
//!
//! - Alchemy down → log `price_worker.alchemy_failed`, all tokens fall to Coingecko.
//! - Coingecko down → tokens missing from Alchemy stay missing in cache;
//!   spine cascade falls through to ConfigPriceOracle (operator overrides).
//! - Both down → cache not updated this tick; existing TTL-bounded entries
//!   continue serving (≤60s old). After TTL expiry, cache empties and the
//!   spine cascades to ConfigPriceOracle. NEVER fabricates a price.
//! - Allowlist empty → worker logs and skips its tick (no input to fetch for).
//!
//! ## Token discovery
//!
//! Fetches `(symbol, address)` tuples from two Redis sources:
//!   1. `allowed_token_symbols` from `arbx:trading_config:<chain_id>` —
//!      operator's universe (high-priority refresh).
//!   2. `arbx:tokens:<chain_id>:<address>` — every token the pool sync
//!      worker has discovered (broad coverage).
//!
//! The intersection is bounded; if the cache balloons, the worker batches
//! into chunks of `MAX_BATCH_SIZE` to stay under provider request limits.

use crate::counters::counters;
use crate::reserves::TokenMeta;
use redis::aio::ConnectionManager;
use redis::AsyncCommands;
use serde::{Deserialize, Serialize};
use shared_rs::price_oracle::redis_token_prices_key;
use shared_rs::trading_config::{redis_key as trading_config_redis_key, TradingConfigState};
use std::collections::HashMap;
use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};
use tokio::time::interval;
use tracing::{debug, info, warn};

/// Default refresh period. Operator overrides via `PRICE_WORKER_INTERVAL_SECS`.
pub const DEFAULT_PERIOD_SECS: u64 = 30;
/// Cache TTL on the populated Redis hash. Set to 2× the worker period so a
/// single missed tick doesn't empty the cache (gives the worker one chance to
/// recover before downstream `UnknownTokenPrice` rejections start firing).
pub const CACHE_TTL_MULTIPLIER: u64 = 2;
/// Provider request limit safety bound; chunk allowlist into this many
/// tokens per HTTP call. Both Alchemy and Coingecko accept comfortably more
/// but smaller batches recover faster from individual provider hiccups.
pub const MAX_BATCH_SIZE: usize = 30;
/// Per-request HTTP timeout. Prices are best-effort; we never want a hung
/// upstream to delay the next worker tick.
pub const HTTP_TIMEOUT_SECS: u64 = 8;
/// Hard cap on the total pricing universe per tick. Allowlist tokens are
/// always included first; pool-resident tokens (recovered from
/// `arbx:pool_index[:_v3]:<chain>:<sym0>:<sym1>` key names) fill the
/// remainder up to this bound. Protects Alchemy (shared RPC key budget) and
/// Coingecko (free-tier rate limit) from a blowout when pool discovery
/// enumerates a long tail. At MAX_BATCH_SIZE=30 this is <= 10 Alchemy +
/// <= 10 Coingecko batched calls per tick — within provider budgets. Lower
/// this if `price_worker.coingecko_failed` rises under sustained Alchemy
/// outage; the allowlist always survives the cut.
pub const MAX_PRICED_TOKENS: usize = 300;

// -------- Alchemy request/response types --------

#[derive(Serialize)]
struct AlchemyRequest {
    addresses: Vec<AlchemyAddressRef>,
}

#[derive(Serialize)]
struct AlchemyAddressRef {
    network: String,
    address: String,
}

#[derive(Deserialize)]
struct AlchemyResponse {
    data: Vec<AlchemyTokenPrice>,
}

#[derive(Deserialize)]
struct AlchemyTokenPrice {
    /// Lowercase 0x address as returned by the API. We re-key by symbol via
    /// the request's parallel array. `symbol` field IS present on Alchemy
    /// responses but is provider-derived and inconsistent across networks;
    /// we use OUR token meta's symbol as the canonical key.
    address: String,
    #[serde(default)]
    prices: Vec<AlchemyPriceQuote>,
    /// Some failure cases include an `error` key instead of `prices`.
    /// We just check `prices.is_empty()` and treat that as "no price".
    #[allow(dead_code)]
    #[serde(default)]
    error: Option<serde_json::Value>,
}

#[derive(Deserialize)]
struct AlchemyPriceQuote {
    currency: String,
    value: String,
}

// -------- Coingecko response (top-level is dynamic by address) --------
// `{"0xabc...": {"usd": 1234.5}, "0xdef...": {"usd": 0.001}}`

// -------- Worker --------

#[derive(Debug, Clone)]
pub struct TokenRef {
    /// Canonical uppercase symbol used as the Redis hash field key.
    pub symbol: String,
    /// Lowercase 0x-prefixed address used by both upstreams.
    pub address_lower: String,
}

/// Network identifier passed to Alchemy. Mainnet = "eth-mainnet". Other
/// chains have their own slugs; map here when we add multi-chain support.
fn alchemy_network_slug(chain_id: u64) -> Option<&'static str> {
    match chain_id {
        1 => Some("eth-mainnet"),
        137 => Some("polygon-mainnet"),
        42161 => Some("arb-mainnet"),
        10 => Some("opt-mainnet"),
        8453 => Some("base-mainnet"),
        _ => None,
    }
}

/// Coingecko platform identifier. Must match the slug used in the
/// `simple/token_price/{platform}` URL.
fn coingecko_platform_slug(chain_id: u64) -> Option<&'static str> {
    match chain_id {
        1 => Some("ethereum"),
        137 => Some("polygon-pos"),
        42161 => Some("arbitrum-one"),
        10 => Some("optimistic-ethereum"),
        8453 => Some("base"),
        _ => None,
    }
}

/// Parses the alchemy API key out of the `RPC_HTTP_<chain_id>` env value.
///
/// Looks for the first entry whose URL host contains "alchemy" and extracts
/// the trailing path segment after `/v2/` (Alchemy's standard URL shape:
/// `https://eth-mainnet.g.alchemy.com/v2/<KEY>`).
///
/// Returns `None` when:
///   - The env var is absent or empty.
///   - No entry has an alchemy host.
///   - The URL shape doesn't match the `/v2/<KEY>` pattern.
///
/// **Why parse it from the existing env**: matches no-hardcode discipline
/// (single source of truth for credentials) and avoids introducing a new
/// `ALCHEMY_PRICES_KEY_<chain>` env that the operator would have to maintain
/// in parallel.
pub fn extract_alchemy_key_from_rpc_env(rpc_http_csv: &str) -> Option<String> {
    for tok in rpc_http_csv.split(',') {
        let tok = tok.trim();
        if tok.is_empty() {
            continue;
        }
        let url = if let Some((_name, u)) = tok.split_once('=') {
            u.trim()
        } else {
            tok
        };
        if !url.to_ascii_lowercase().contains("alchemy") {
            continue;
        }
        // Match `/v2/<KEY>` — strip any query string first.
        let no_query = url.split('?').next().unwrap_or(url);
        if let Some(idx) = no_query.find("/v2/") {
            let key = &no_query[idx + "/v2/".len()..];
            // Strip any trailing slash + remove path segments after the key.
            let key = key.trim_end_matches('/');
            let key = key.split('/').next().unwrap_or(key);
            if !key.is_empty() {
                return Some(key.to_string());
            }
        }
    }
    None
}

/// Build the Alchemy Token Prices base URL from a key. Single helper so tests
/// can also point at wiremock by injecting a different base.
pub fn alchemy_prices_url(api_key: &str) -> String {
    format!(
        "https://api.g.alchemy.com/prices/v1/{}/tokens/by-address",
        api_key
    )
}

/// Coingecko free-tier base URL (no auth). Same helper rationale as above.
pub fn coingecko_prices_url(platform: &str) -> String {
    format!(
        "https://api.coingecko.com/api/v3/simple/token_price/{}",
        platform
    )
}

/// Chainlink `latestRoundData()` 4-byte selector = keccak256("latestRoundData()")[..4].
/// Returns (roundId, answer, startedAt, updatedAt, answeredInRound); `answer` is the
/// USD price in the feed's `decimals`.
const CHAINLINK_LATEST_ROUND_DATA_SELECTOR: &str = "0xfeaf968c";

/// Extract the first usable HTTP(S) RPC URL from a `RPC_HTTP_<chain>` CSV env value
/// (entries may be `name=url` or a bare `url`). Used as the `eth_call` target for the
/// Chainlink Tier-0 reads. Single source of truth for RPC endpoints (no-hardcode).
pub fn extract_first_rpc_http_url(rpc_http_csv: &str) -> Option<String> {
    for tok in rpc_http_csv.split(',') {
        let tok = tok.trim();
        if tok.is_empty() {
            continue;
        }
        let url = if let Some((_name, u)) = tok.split_once('=') {
            u.trim()
        } else {
            tok
        };
        if url.starts_with("http://") || url.starts_with("https://") {
            return Some(url.to_string());
        }
    }
    None
}

/// Resolve the chain's first HTTP RPC URL from `RPC_HTTP_<chain_id>` env.
pub fn rpc_http_url_from_env(chain_id: u64) -> Option<String> {
    let csv = std::env::var(format!("RPC_HTTP_{chain_id}")).unwrap_or_default();
    extract_first_rpc_http_url(&csv)
}

/// Configuration. Operator-tunable knobs at boot; nothing varies per tick.
#[derive(Debug, Clone)]
pub struct PriceWorkerConfig {
    pub chain_id: u64,
    pub period: Duration,
    /// Override for the Alchemy base URL (tests inject wiremock URL here).
    /// Production passes `None` and the worker computes the URL from the
    /// `alchemy_api_key` field.
    pub alchemy_base_url_override: Option<String>,
    pub alchemy_api_key: Option<String>,
    /// Override for the Coingecko base URL (tests inject wiremock URL here).
    pub coingecko_base_url_override: Option<String>,
    /// Tier-0 Chainlink source. `db_pool` reads operator-seeded `price_oracles`
    /// (kind='chainlink', enabled); `rpc_http_url` is the chain's HTTP RPC for
    /// `eth_call latestRoundData()`. Both must be present to enable the tier;
    /// otherwise it is skipped (Chainlink prices = none, cascade falls through).
    pub db_pool: Option<sqlx::postgres::PgPool>,
    pub rpc_http_url: Option<String>,
}

impl PriceWorkerConfig {
    pub fn new(chain_id: u64, period_secs: u64, alchemy_api_key: Option<String>) -> Self {
        Self {
            chain_id,
            period: Duration::from_secs(period_secs.max(1)),
            alchemy_base_url_override: None,
            alchemy_api_key,
            coingecko_base_url_override: None,
            db_pool: None,
            rpc_http_url: None,
        }
    }

    /// Enable the Chainlink Tier-0 source (read-only on-chain price feeds).
    pub fn with_chainlink(mut self, db_pool: sqlx::postgres::PgPool, rpc_http_url: String) -> Self {
        self.db_pool = Some(db_pool);
        self.rpc_http_url = Some(rpc_http_url);
        self
    }
}

pub struct PriceWorker {
    cfg: PriceWorkerConfig,
    http: reqwest::Client,
}

impl PriceWorker {
    pub fn new(cfg: PriceWorkerConfig) -> anyhow::Result<Self> {
        let http = reqwest::Client::builder()
            .timeout(Duration::from_secs(HTTP_TIMEOUT_SECS))
            .user_agent("arbitragex-v2-price-worker/0.1")
            .build()?;
        Ok(Self { cfg, http })
    }

    /// Run forever — never returns. Caller should `tokio::spawn` it.
    pub async fn run(self, mut redis: ConnectionManager) {
        if self.cfg.alchemy_api_key.is_none() {
            warn!(
                event = "price_worker.no_alchemy_key",
                chain_id = self.cfg.chain_id,
                "no Alchemy API key resolved from RPC_HTTP env — Alchemy fetches will be skipped, Coingecko fallback will carry the load (rate-limited)"
            );
        }
        if alchemy_network_slug(self.cfg.chain_id).is_none() {
            warn!(
                event = "price_worker.unsupported_chain",
                chain_id = self.cfg.chain_id,
                "chain_id has no Alchemy network slug mapping; worker exits — extend alchemy_network_slug() to add support"
            );
            return;
        }
        info!(
            event = "price_worker.boot",
            chain_id = self.cfg.chain_id,
            period_secs = self.cfg.period.as_secs(),
            "price worker started"
        );
        let mut ticker = interval(self.cfg.period);
        // Drain the first immediate tick — gives downstream services a
        // moment to populate the allowlist.
        ticker.tick().await;
        loop {
            ticker.tick().await;
            match self.run_one_tick(&mut redis).await {
                Ok(stats) => {
                    info!(
                        event = "price_worker.tick_done",
                        chain_id = self.cfg.chain_id,
                        chainlink_hits = stats.chainlink_hits,
                        alchemy_hits = stats.alchemy_hits,
                        coingecko_hits = stats.coingecko_hits,
                        cache_misses = stats.cache_misses,
                        attempted = stats.attempted,
                        elapsed_ms = stats.elapsed_ms,
                        "tick complete"
                    );
                }
                Err(e) => {
                    counters()
                        .price_worker_errors
                        .fetch_add(1, Ordering::Relaxed);
                    warn!(event = "price_worker.tick_failed", chain_id = self.cfg.chain_id, error = %e);
                }
            }
        }
    }

    /// One refresh cycle. Public for testability — tests call this directly
    /// with mocked HTTP servers and verify Redis state.
    pub async fn run_one_tick(&self, redis: &mut ConnectionManager) -> anyhow::Result<TickStats> {
        let started = Instant::now();

        let mut prices: HashMap<String, f64> = HashMap::new();

        // Tier 0: Chainlink on-chain feeds (authoritative; operator-seeded
        // price_oracles). Independent of the token allowlist — these are the core
        // priced anchors (WETH/WBTC/USDC/USDT/DAI). Direct insert: Chainlink wins.
        let chainlink = self.fetch_chainlink(redis).await;
        let chainlink_hits = chainlink.len();
        for (sym, price) in chainlink {
            prices.insert(sym, price);
        }

        let tokens = self.discover_tokens(redis).await?;
        if tokens.is_empty() {
            // No allowlist tokens for the external APIs, but Chainlink prices (if
            // any) are still worth persisting so the cascade can serve them.
            if !prices.is_empty() {
                self.persist_prices(redis, &prices).await?;
            } else {
                debug!(
                    event = "price_worker.no_tokens",
                    chain_id = self.cfg.chain_id,
                    "allowlist + meta cache produced zero tokens this tick"
                );
            }
            return Ok(TickStats {
                chainlink_hits,
                elapsed_ms: started.elapsed().as_millis() as u64,
                ..Default::default()
            });
        }

        // Tier 1: Alchemy batch (fills only what Chainlink did NOT already price).
        let mut alchemy_hits = 0usize;
        if self.cfg.alchemy_api_key.is_some() {
            for chunk in tokens.chunks(MAX_BATCH_SIZE) {
                match self.fetch_alchemy(chunk).await {
                    Ok(map) => {
                        for (sym, price) in map {
                            if let std::collections::hash_map::Entry::Vacant(e) = prices.entry(sym)
                            {
                                e.insert(price);
                                alchemy_hits += 1;
                            }
                        }
                    }
                    Err(e) => {
                        counters()
                            .price_worker_errors
                            .fetch_add(1, Ordering::Relaxed);
                        warn!(
                            event = "price_worker.alchemy_failed",
                            chain_id = self.cfg.chain_id,
                            chunk_size = chunk.len(),
                            error = %e,
                            "Alchemy batch failed; will try Coingecko fallback"
                        );
                    }
                }
            }
        }
        counters()
            .price_alchemy_hits
            .fetch_add(alchemy_hits as u64, Ordering::Relaxed);

        // Tier 2: Coingecko fallback for anything Alchemy missed.
        let missing: Vec<&TokenRef> = tokens
            .iter()
            .filter(|t| !prices.contains_key(&t.symbol))
            .collect();
        let mut coingecko_hits = 0usize;
        if !missing.is_empty() {
            for chunk in missing.chunks(MAX_BATCH_SIZE) {
                let chunk_owned: Vec<TokenRef> = chunk.iter().map(|t| (*t).clone()).collect();
                match self.fetch_coingecko(&chunk_owned).await {
                    Ok(map) => {
                        for (sym, price) in map {
                            // Don't overwrite an existing Alchemy hit. Use Entry
                            // API for clippy::map_entry compliance + clearer intent.
                            if let std::collections::hash_map::Entry::Vacant(e) = prices.entry(sym)
                            {
                                e.insert(price);
                                coingecko_hits += 1;
                            }
                        }
                    }
                    Err(e) => {
                        counters()
                            .price_worker_errors
                            .fetch_add(1, Ordering::Relaxed);
                        warn!(
                            event = "price_worker.coingecko_failed",
                            chain_id = self.cfg.chain_id,
                            chunk_size = chunk.len(),
                            error = %e,
                            "Coingecko batch failed; affected tokens will fall through to ConfigPriceOracle"
                        );
                    }
                }
            }
        }
        counters()
            .price_coingecko_hits
            .fetch_add(coingecko_hits as u64, Ordering::Relaxed);

        let cache_misses = tokens.len().saturating_sub(prices.len());
        counters()
            .price_cache_misses
            .fetch_add(cache_misses as u64, Ordering::Relaxed);

        // Persist whatever we have (R8: empty results are NOT written; tokens
        // missing from `prices` simply don't get a Redis entry, letting the
        // cascade fall through honestly).
        if !prices.is_empty() {
            self.persist_prices(redis, &prices).await?;
        }

        Ok(TickStats {
            attempted: tokens.len(),
            chainlink_hits,
            alchemy_hits,
            coingecko_hits,
            cache_misses,
            elapsed_ms: started.elapsed().as_millis() as u64,
        })
    }

    /// Reads the operator's allowlist from `trading_config` (provides symbol→
    /// preferred ordering) and joins against `arbx:tokens:<chain>:<addr>` to
    /// recover addresses. Tokens for which no meta entry exists are skipped
    /// silently — pool sync worker will eventually populate them, and on the
    /// next tick they'll be picked up.
    async fn discover_tokens(
        &self,
        redis: &mut ConnectionManager,
    ) -> anyhow::Result<Vec<TokenRef>> {
        // Step 1: load allowlist symbols.
        let cfg_key = trading_config_redis_key(self.cfg.chain_id);
        let raw_cfg: Option<String> = redis.get(&cfg_key).await?;
        let allowlist: Vec<String> = match raw_cfg {
            Some(s) => match serde_json::from_str::<TradingConfigState>(&s) {
                Ok(c) => c.allowed_token_symbols,
                Err(e) => {
                    warn!(
                        event = "price_worker.config_parse_failed",
                        chain_id = self.cfg.chain_id,
                        error = %e,
                        "trading_config JSON malformed; skipping tick"
                    );
                    return Ok(vec![]);
                }
            },
            None => {
                debug!(
                    event = "price_worker.no_trading_config",
                    chain_id = self.cfg.chain_id,
                    "no trading_config in Redis yet (operator hasn't seeded); skipping tick"
                );
                return Ok(vec![]);
            }
        };
        if allowlist.is_empty() {
            return Ok(vec![]);
        }

        // Step 2: scan all `arbx:tokens:<chain>:*` to build a symbol→address
        // map (lowercased addr). Chains rarely have >2K tokens so SCAN with
        // a generous COUNT hint is fine; we don't paginate further.
        let pattern = format!("arbx:tokens:{}:*", self.cfg.chain_id);
        let mut iter: redis::AsyncIter<String> = redis::cmd("SCAN")
            .cursor_arg(0)
            .arg("MATCH")
            .arg(&pattern)
            .arg("COUNT")
            .arg(500)
            .clone()
            .iter_async(redis)
            .await?;
        let mut keys: Vec<String> = Vec::new();
        while let Some(k) = iter.next_item().await {
            keys.push(k);
        }
        drop(iter);

        let mut sym_to_addr: HashMap<String, String> = HashMap::new();
        for k in keys {
            // Key format: arbx:tokens:<chain>:<addr_lower>
            let addr_lower = match k.rsplit(':').next() {
                Some(a) if a.starts_with("0x") => a.to_string(),
                _ => continue,
            };
            let raw: Option<String> = redis.get(&k).await.unwrap_or(None);
            if let Some(s) = raw {
                if let Ok(meta) = serde_json::from_str::<TokenMeta>(&s) {
                    let upper = meta.symbol.to_ascii_uppercase();
                    // Keep the FIRST address per symbol; allowlist is symbol-keyed
                    // so we can't disambiguate further without operator input.
                    sym_to_addr.entry(upper).or_insert(addr_lower);
                }
            }
        }

        // Step 3: build the pricing universe. Allowlist first (operator's
        // curated high-confidence set — always fully priced), THEN symbols
        // recovered from `arbx:pool_index[:_v3]:<chain>:<sym0>:<sym1>` key
        // names (tokens that appear in at least one detected pool), deduped
        // against the allowlist and capped to protect Alchemy/Coingecko rate
        // limits. Closes the `no_price_oracle` gap for pool-resident tokens
        // the allowlist doesn't name, without pricing every one of the 1,000+
        // discovered tokens indiscriminately. Tokens with no `arbx:tokens`
        // meta (no resolvable address) are skipped fail-honestly (R8).
        let mut out: Vec<TokenRef> = Vec::new();
        let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();
        for sym in &allowlist {
            if out.len() >= MAX_PRICED_TOKENS {
                break;
            }
            let upper = sym.to_ascii_uppercase();
            if seen.insert(upper.clone()) {
                if let Some(addr) = sym_to_addr.get(&upper) {
                    out.push(TokenRef {
                        symbol: upper,
                        address_lower: addr.clone(),
                    });
                }
            }
        }
        // Pool-resident tokens beyond the allowlist. `seen` dedupes against
        // allowlist symbols already added, so only NEW pool tokens are priced.
        if out.len() < MAX_PRICED_TOKENS {
            for sym in self.scan_pool_resident_symbols(redis).await {
                if out.len() >= MAX_PRICED_TOKENS {
                    break;
                }
                if seen.insert(sym.clone()) {
                    if let Some(addr) = sym_to_addr.get(&sym) {
                        out.push(TokenRef {
                            symbol: sym,
                            address_lower: addr.clone(),
                        });
                    }
                }
            }
        }
        Ok(out)
    }

    /// Scan `arbx:pool_index:<chain>:*` and `arbx:pool_index_v3:<chain>:*` key
    /// names and return the set of UPPERCASE token symbols that appear in at
    /// least one detected pool. Key names encode both symbols
    /// (`arbx:pool_index:<chain>:<sym0>:<sym1>`); we take the last two
    /// colon-separated segments and uppercase them to match the `sym_to_addr`
    /// map (writers use mixed case: `pool_discovery` lowercases,
    /// `pool_sync_worker` boot preserves the PG symbol). Returns an empty set
    /// on SCAN failure — the pricing universe then falls back to allowlist
    /// only (R8 fail-honest: no fabricated symbols).
    async fn scan_pool_resident_symbols(
        &self,
        redis: &mut ConnectionManager,
    ) -> std::collections::HashSet<String> {
        let mut out = std::collections::HashSet::new();
        for pattern in [
            format!("arbx:pool_index:{}:*", self.cfg.chain_id),
            format!("arbx:pool_index_v3:{}:*", self.cfg.chain_id),
        ] {
            let mut iter: redis::AsyncIter<String> = match redis::cmd("SCAN")
                .cursor_arg(0)
                .arg("MATCH")
                .arg(&pattern)
                .arg("COUNT")
                .arg(500)
                .clone()
                .iter_async(redis)
                .await
            {
                Ok(it) => it,
                Err(e) => {
                    warn!(
                        event = "price_worker.pool_index_scan_failed",
                        chain_id = self.cfg.chain_id,
                        pattern = %pattern,
                        error = %e,
                        "pool_index SCAN failed; universe falls back to allowlist"
                    );
                    continue;
                }
            };
            while let Some(k) = iter.next_item().await {
                // arbx:pool_index[:_v3]:<chain>:<sym0>:<sym1> — last two
                // colon-separated segments are the pair's symbols.
                let parts: Vec<&str> = k.rsplitn(3, ':').collect();
                if parts.len() < 2 {
                    continue;
                }
                for sym in &parts[0..2] {
                    let upper = sym.to_ascii_uppercase();
                    if !upper.is_empty() {
                        out.insert(upper);
                    }
                }
            }
            drop(iter);
        }
        out
    }

    /// POST one batch to Alchemy. Returns `symbol → price` for tokens that
    /// came back with a parseable USD quote. Tokens missing a quote are
    /// simply absent from the returned map (no fabricated entries).
    async fn fetch_alchemy(&self, tokens: &[TokenRef]) -> anyhow::Result<HashMap<String, f64>> {
        let api_key = self
            .cfg
            .alchemy_api_key
            .as_deref()
            .ok_or_else(|| anyhow::anyhow!("alchemy api key not configured"))?;
        let network = alchemy_network_slug(self.cfg.chain_id).ok_or_else(|| {
            anyhow::anyhow!(
                "alchemy network slug missing for chain {}",
                self.cfg.chain_id
            )
        })?;
        let url = match &self.cfg.alchemy_base_url_override {
            Some(u) => u.clone(),
            None => alchemy_prices_url(api_key),
        };

        let body = AlchemyRequest {
            addresses: tokens
                .iter()
                .map(|t| AlchemyAddressRef {
                    network: network.to_string(),
                    address: t.address_lower.clone(),
                })
                .collect(),
        };
        let resp = self
            .http
            .post(&url)
            .json(&body)
            .send()
            .await?
            .error_for_status()?;
        let parsed: AlchemyResponse = resp.json().await?;

        // Map address → symbol for re-keying (Alchemy returns addresses).
        let addr_to_sym: HashMap<String, String> = tokens
            .iter()
            .map(|t| (t.address_lower.to_ascii_lowercase(), t.symbol.clone()))
            .collect();

        let mut out: HashMap<String, f64> = HashMap::new();
        for entry in parsed.data {
            let addr_lower = entry.address.to_ascii_lowercase();
            let sym = match addr_to_sym.get(&addr_lower) {
                Some(s) => s.clone(),
                None => continue, // Address we didn't ask for; skip silently.
            };
            for q in &entry.prices {
                if !q.currency.eq_ignore_ascii_case("usd") {
                    continue;
                }
                if let Ok(p) = q.value.parse::<f64>() {
                    if p.is_finite() && p > 0.0 {
                        out.insert(sym, p);
                        break;
                    }
                }
            }
        }
        Ok(out)
    }

    /// GET one batch to Coingecko free tier. Same return semantics as Alchemy.
    async fn fetch_coingecko(&self, tokens: &[TokenRef]) -> anyhow::Result<HashMap<String, f64>> {
        let platform = coingecko_platform_slug(self.cfg.chain_id).ok_or_else(|| {
            anyhow::anyhow!(
                "coingecko platform slug missing for chain {}",
                self.cfg.chain_id
            )
        })?;
        let base = match &self.cfg.coingecko_base_url_override {
            Some(u) => u.clone(),
            None => coingecko_prices_url(platform),
        };
        let addresses: Vec<String> = tokens.iter().map(|t| t.address_lower.clone()).collect();
        let resp = self
            .http
            .get(&base)
            .query(&[
                ("contract_addresses", addresses.join(",")),
                ("vs_currencies", "usd".to_string()),
            ])
            .send()
            .await?
            .error_for_status()?;
        // Coingecko response: {"0xabc...": {"usd": 1234.5}, ...}
        let parsed: HashMap<String, HashMap<String, f64>> = resp.json().await?;

        let addr_to_sym: HashMap<String, String> = tokens
            .iter()
            .map(|t| (t.address_lower.to_ascii_lowercase(), t.symbol.clone()))
            .collect();

        let mut out: HashMap<String, f64> = HashMap::new();
        for (addr, currencies) in parsed {
            let addr_lower = addr.to_ascii_lowercase();
            let sym = match addr_to_sym.get(&addr_lower) {
                Some(s) => s.clone(),
                None => continue,
            };
            if let Some(p) = currencies.get("usd") {
                if p.is_finite() && *p > 0.0 {
                    out.insert(sym, *p);
                }
            }
        }
        Ok(out)
    }

    /// Tier 0 — Chainlink on-chain feeds. Reads operator-seeded `price_oracles`
    /// (kind='chainlink', enabled) for this chain, `eth_call`s `latestRoundData()`
    /// on each aggregator via JSON-RPC, and returns `symbol → USD price`. The
    /// symbol is resolved from the Redis token meta (`arbx:tokens:<chain>:<addr>`,
    /// same source as `discover_tokens`). Best-effort + R8 fail-honest: any feed
    /// that fails to read / parse / resolve is skipped (logged), never fabricated.
    /// Returns empty when the source is not configured so the cascade degrades to
    /// Alchemy/Coingecko/Config exactly as before.
    async fn fetch_chainlink(&self, redis: &mut ConnectionManager) -> HashMap<String, f64> {
        let mut out: HashMap<String, f64> = HashMap::new();
        let (db, rpc_url) = match (&self.cfg.db_pool, &self.cfg.rpc_http_url) {
            (Some(db), Some(u)) => (db, u),
            _ => return out, // Chainlink tier not configured
        };
        let rows = match sqlx::query_as::<_, (String, String, i32)>(
            "SELECT token_address, oracle_address, decimals FROM price_oracles \
             WHERE chain_id = $1 AND kind = 'chainlink' AND enabled = TRUE",
        )
        .bind(self.cfg.chain_id as i32)
        .fetch_all(db)
        .await
        {
            Ok(r) => r,
            Err(e) => {
                warn!(event = "price_worker.chainlink_db_failed", chain_id = self.cfg.chain_id, error = %e);
                return out;
            }
        };
        for (token_addr, oracle_addr, decimals) in rows {
            let raw = match self.eth_call_latest_answer(rpc_url, &oracle_addr).await {
                Some(v) => v,
                None => continue,
            };
            let price = raw / 10f64.powi(decimals.max(0));
            if !price.is_finite() || price <= 0.0 {
                continue;
            }
            let addr_lower = token_addr.to_ascii_lowercase();
            let meta_key = format!("arbx:tokens:{}:{}", self.cfg.chain_id, addr_lower);
            let meta_raw: Option<String> = redis.get(&meta_key).await.unwrap_or(None);
            let symbol = match meta_raw.and_then(|s| serde_json::from_str::<TokenMeta>(&s).ok()) {
                Some(m) => m.symbol.to_ascii_uppercase(),
                None => {
                    debug!(
                        event = "price_worker.chainlink_symbol_unresolved",
                        chain_id = self.cfg.chain_id,
                        token = %addr_lower,
                        "no token meta in Redis; skipping this Chainlink feed"
                    );
                    continue;
                }
            };
            out.insert(symbol, price);
        }
        if !out.is_empty() {
            info!(
                event = "price_worker.chainlink_priced",
                chain_id = self.cfg.chain_id,
                count = out.len(),
                "Chainlink Tier-0 prices resolved on-chain"
            );
        }
        out
    }

    /// `eth_call latestRoundData()` on a Chainlink aggregator; returns the raw
    /// `answer` (feed units, pre-decimals) as f64. Read-only JSON-RPC via the
    /// worker's reqwest client. `None` on any RPC / parse failure (fail-honest).
    async fn eth_call_latest_answer(&self, rpc_url: &str, oracle_addr: &str) -> Option<f64> {
        let body = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "eth_call",
            "params": [
                { "to": oracle_addr, "data": CHAINLINK_LATEST_ROUND_DATA_SELECTOR },
                "latest"
            ]
        });
        let resp = match self.http.post(rpc_url).json(&body).send().await {
            Ok(r) => r,
            Err(e) => {
                warn!(event = "price_worker.chainlink_rpc_failed", oracle = %oracle_addr, error = %e);
                return None;
            }
        };
        let parsed: serde_json::Value = match resp.json().await {
            Ok(v) => v,
            Err(e) => {
                warn!(event = "price_worker.chainlink_rpc_parse_failed", oracle = %oracle_addr, error = %e);
                return None;
            }
        };
        let result_hex = parsed.get("result")?.as_str()?;
        let hex = result_hex.strip_prefix("0x").unwrap_or(result_hex);
        // 5 × 32-byte words; `answer` = word[1] (hex chars 64..128). The USD price
        // fits within u128, so parse its low 16 bytes (chars 96..128). Chainlink USD
        // answers are positive, so treating the word as unsigned is correct here.
        if hex.len() < 128 {
            return None;
        }
        let answer_low = &hex[96..128];
        let raw = u128::from_str_radix(answer_low, 16).ok()?;
        Some(raw as f64)
    }

    async fn persist_prices(
        &self,
        redis: &mut ConnectionManager,
        prices: &HashMap<String, f64>,
    ) -> anyhow::Result<()> {
        let key = redis_token_prices_key(self.cfg.chain_id);
        // Pipeline: HSET each field then EXPIRE on the hash key.
        let mut pipe = redis::pipe();
        pipe.atomic();
        for (sym, price) in prices {
            // Serialise as decimal string — `RedisCachedPriceOracle` parses it
            // back to f64 with `f64::from_str`. Avoids any locale issues.
            pipe.hset(&key, sym, format!("{}", price)).ignore();
        }
        let ttl = self
            .cfg
            .period
            .as_secs()
            .saturating_mul(CACHE_TTL_MULTIPLIER)
            .max(60);
        pipe.expire(&key, ttl as i64).ignore();
        let _: () = pipe.query_async(redis).await?;
        Ok(())
    }
}

#[derive(Debug, Default, Clone)]
pub struct TickStats {
    pub attempted: usize,
    pub chainlink_hits: usize,
    pub alchemy_hits: usize,
    pub coingecko_hits: usize,
    pub cache_misses: usize,
    pub elapsed_ms: u64,
}

// Helper used by the worker startup path in main.rs (kept here so callers don't
// have to repeat the env-var parsing logic).
pub fn alchemy_key_from_env(chain_id: u64) -> Option<String> {
    let env_name = format!("RPC_HTTP_{chain_id}");
    let csv = std::env::var(&env_name).unwrap_or_default();
    extract_alchemy_key_from_rpc_env(&csv)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)] // M11: test module
    use super::*;

    // --------------- API key extraction ---------------

    #[test]
    fn extract_alchemy_key_from_named_csv_entry() {
        let csv = "alchemy=https://eth-mainnet.g.alchemy.com/v2/ABC123,infura=https://mainnet.infura.io/v3/XYZ";
        assert_eq!(
            extract_alchemy_key_from_rpc_env(csv),
            Some("ABC123".to_string())
        );
    }

    #[test]
    fn extract_alchemy_key_from_bare_url() {
        let csv = "https://eth-mainnet.g.alchemy.com/v2/SECRET_KEY_42";
        assert_eq!(
            extract_alchemy_key_from_rpc_env(csv),
            Some("SECRET_KEY_42".to_string())
        );
    }

    #[test]
    fn extract_alchemy_key_returns_none_when_no_alchemy_entry() {
        let csv = "infura=https://mainnet.infura.io/v3/abc,ankr=https://rpc.ankr.com/eth/abc";
        assert_eq!(extract_alchemy_key_from_rpc_env(csv), None);
    }

    #[test]
    fn extract_alchemy_key_returns_none_for_empty_input() {
        assert_eq!(extract_alchemy_key_from_rpc_env(""), None);
        assert_eq!(extract_alchemy_key_from_rpc_env("   "), None);
        assert_eq!(extract_alchemy_key_from_rpc_env(",,,"), None);
    }

    #[test]
    fn extract_alchemy_key_strips_query_string_and_trailing_slash() {
        let csv = "alchemy=https://eth-mainnet.g.alchemy.com/v2/KEY_WITH_QS/?ver=2";
        assert_eq!(
            extract_alchemy_key_from_rpc_env(csv),
            Some("KEY_WITH_QS".to_string())
        );
    }

    #[test]
    fn extract_alchemy_key_handles_mixed_case_host() {
        let csv = "primary=https://ETH-MAINNET.G.ALCHEMY.COM/v2/UPPERCASE_KEY";
        assert_eq!(
            extract_alchemy_key_from_rpc_env(csv),
            Some("UPPERCASE_KEY".to_string())
        );
    }

    // --------------- chain mapping ---------------

    #[test]
    fn chain_slug_mappings_cover_supported_set() {
        assert_eq!(alchemy_network_slug(1), Some("eth-mainnet"));
        assert_eq!(alchemy_network_slug(8453), Some("base-mainnet"));
        assert_eq!(alchemy_network_slug(42161), Some("arb-mainnet"));
        assert_eq!(alchemy_network_slug(99999), None);

        assert_eq!(coingecko_platform_slug(1), Some("ethereum"));
        assert_eq!(coingecko_platform_slug(137), Some("polygon-pos"));
        assert_eq!(coingecko_platform_slug(99999), None);
    }

    // --------------- HTTP path tests via wiremock ---------------
    //
    // These tests boot a localhost wiremock server and point the worker at it
    // via `*_base_url_override`. NO external network calls happen.

    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn token_ref(sym: &str, addr: &str) -> TokenRef {
        TokenRef {
            symbol: sym.to_ascii_uppercase(),
            address_lower: addr.to_ascii_lowercase(),
        }
    }

    fn worker_pointing_at(alchemy: Option<&str>, coingecko: Option<&str>) -> PriceWorker {
        let mut cfg = PriceWorkerConfig::new(1, 30, Some("test-key".into()));
        cfg.alchemy_base_url_override = alchemy.map(|s| s.to_string());
        cfg.coingecko_base_url_override = coingecko.map(|s| s.to_string());
        PriceWorker::new(cfg).expect("client builds")
    }

    #[tokio::test]
    async fn worker_parses_alchemy_response_and_keys_by_symbol() {
        let server = MockServer::start().await;
        // Alchemy responds with two tokens; we asked for two; both have USD prices.
        let body = serde_json::json!({
            "data": [
                {
                    "address": "0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2",
                    "prices": [{"currency": "usd", "value": "2517.42"}],
                },
                {
                    "address": "0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48",
                    "prices": [{"currency": "usd", "value": "1.0001"}],
                },
            ]
        });
        Mock::given(method("POST"))
            .and(path("/"))
            .respond_with(ResponseTemplate::new(200).set_body_json(body))
            .mount(&server)
            .await;
        let worker = worker_pointing_at(Some(&format!("{}/", server.uri())), None);
        let tokens = vec![
            token_ref("WETH", "0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2"),
            token_ref("USDC", "0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48"),
        ];
        let result = worker
            .fetch_alchemy(&tokens)
            .await
            .expect("alchemy call ok");
        assert_eq!(result.get("WETH"), Some(&2517.42));
        assert_eq!(result.get("USDC"), Some(&1.0001));
    }

    #[tokio::test]
    async fn worker_skips_alchemy_token_with_no_usd_quote() {
        let server = MockServer::start().await;
        // First token has no `prices` array (Alchemy returned an error
        // shape). Second has only EUR. Neither should produce an entry.
        let body = serde_json::json!({
            "data": [
                {"address": "0xaaaa", "error": {"message":"no_data"}},
                {"address": "0xbbbb", "prices": [{"currency":"eur","value":"100.0"}]},
                {"address": "0xcccc", "prices": [{"currency":"usd","value":"42.0"}]},
            ]
        });
        Mock::given(method("POST"))
            .and(path("/"))
            .respond_with(ResponseTemplate::new(200).set_body_json(body))
            .mount(&server)
            .await;
        let worker = worker_pointing_at(Some(&format!("{}/", server.uri())), None);
        let tokens = vec![
            token_ref("AAA", "0xaaaa"),
            token_ref("BBB", "0xbbbb"),
            token_ref("CCC", "0xcccc"),
        ];
        let result = worker.fetch_alchemy(&tokens).await.expect("ok");
        assert_eq!(result.len(), 1);
        assert_eq!(result.get("CCC"), Some(&42.0));
    }

    #[tokio::test]
    async fn worker_handles_alchemy_5xx_gracefully() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/"))
            .respond_with(ResponseTemplate::new(503))
            .mount(&server)
            .await;
        let worker = worker_pointing_at(Some(&format!("{}/", server.uri())), None);
        let tokens = vec![token_ref("WETH", "0xc02aaa39")];
        let result = worker.fetch_alchemy(&tokens).await;
        assert!(result.is_err(), "5xx must surface as Err, got {:?}", result);
    }

    #[tokio::test]
    async fn worker_alchemy_drops_non_finite_and_negative_prices() {
        let server = MockServer::start().await;
        let body = serde_json::json!({
            "data": [
                {"address":"0x01","prices":[{"currency":"usd","value":"NaN"}]},
                {"address":"0x02","prices":[{"currency":"usd","value":"Infinity"}]},
                {"address":"0x03","prices":[{"currency":"usd","value":"-1.5"}]},
                {"address":"0x04","prices":[{"currency":"usd","value":"100.0"}]},
            ]
        });
        Mock::given(method("POST"))
            .and(path("/"))
            .respond_with(ResponseTemplate::new(200).set_body_json(body))
            .mount(&server)
            .await;
        let worker = worker_pointing_at(Some(&format!("{}/", server.uri())), None);
        let tokens = vec![
            token_ref("A", "0x01"),
            token_ref("B", "0x02"),
            token_ref("C", "0x03"),
            token_ref("D", "0x04"),
        ];
        let result = worker.fetch_alchemy(&tokens).await.expect("ok");
        assert_eq!(result.len(), 1, "only valid finite positive price retained");
        assert_eq!(result.get("D"), Some(&100.0));
    }

    #[tokio::test]
    async fn worker_parses_coingecko_response() {
        let server = MockServer::start().await;
        let body = serde_json::json!({
            "0xc02aaa39": {"usd": 2500.5},
            "0xa0b86991": {"usd": 1.0},
        });
        // Coingecko URL is `<base>?contract_addresses=...&vs_currencies=usd`.
        // We mount a wildcard matcher on path "/" — wiremock matches the path
        // portion, not the query string, so this matches both GETs.
        Mock::given(method("GET"))
            .and(path("/"))
            .respond_with(ResponseTemplate::new(200).set_body_json(body))
            .mount(&server)
            .await;
        let worker = worker_pointing_at(None, Some(&format!("{}/", server.uri())));
        let tokens = vec![
            token_ref("WETH", "0xc02aaa39"),
            token_ref("USDC", "0xa0b86991"),
        ];
        let result = worker.fetch_coingecko(&tokens).await.expect("ok");
        assert_eq!(result.get("WETH"), Some(&2500.5));
        assert_eq!(result.get("USDC"), Some(&1.0));
    }

    #[tokio::test]
    async fn worker_coingecko_skips_non_finite() {
        let server = MockServer::start().await;
        let body = serde_json::json!({
            "0x01": {"usd": 0.0},          // zero — invalid
            "0x02": {"usd": -5.0},          // negative — invalid
            "0x03": {"usd": 12.5},          // valid
        });
        Mock::given(method("GET"))
            .and(path("/"))
            .respond_with(ResponseTemplate::new(200).set_body_json(body))
            .mount(&server)
            .await;
        let worker = worker_pointing_at(None, Some(&format!("{}/", server.uri())));
        let tokens = vec![
            token_ref("A", "0x01"),
            token_ref("B", "0x02"),
            token_ref("C", "0x03"),
        ];
        let result = worker.fetch_coingecko(&tokens).await.expect("ok");
        assert_eq!(result.len(), 1);
        assert_eq!(result.get("C"), Some(&12.5));
    }

    #[tokio::test]
    async fn worker_coingecko_5xx_is_error() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/"))
            .respond_with(ResponseTemplate::new(500))
            .mount(&server)
            .await;
        let worker = worker_pointing_at(None, Some(&format!("{}/", server.uri())));
        let tokens = vec![token_ref("WETH", "0x01")];
        let result = worker.fetch_coingecko(&tokens).await;
        assert!(result.is_err());
    }

    // --------------- url helpers ---------------

    #[test]
    fn alchemy_prices_url_uses_v1_path() {
        let url = alchemy_prices_url("MY_KEY");
        assert!(url.contains("/prices/v1/MY_KEY/tokens/by-address"));
    }

    #[test]
    fn coingecko_prices_url_uses_platform_slug() {
        let url = coingecko_prices_url("ethereum");
        assert!(url.contains("/simple/token_price/ethereum"));
    }
}
