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
//! - Alchemy 429/5xx → `ProviderBackoff` opens an exponential window
//!   (1s → 2s → 4s → … cap 60s) during which NO Alchemy request is sent —
//!   the tick's remaining chunks are skipped early (self-contamination
//!   guard: a 158-token cache-miss tick must not become 158 Alchemy calls).
//!   Single `price_worker.alchemy_backoff` INFO per transition in.
//! - Coingecko 429/5xx → same `ProviderBackoff` mechanism (supersedes the
//!   fixed CG429-01 300s window). Single `price_worker.coingecko_backoff`
//!   INFO per transition in.
//! - On window expiry exactly ONE probe request is allowed (half-open); if
//!   it fails backoff-worthy the window doubles, on success the breaker
//!   resets gradually (failure level halved per success).
//! - Non-rate-limit errors (parse, 4xx other than 429, timeouts) do NOT open
//!   a window — they keep the pre-existing warn + counter behaviour.
//! - Both down → cache not updated this tick; existing TTL-bounded entries
//!   continue serving (≤60s old). After TTL expiry, cache empties and the
//!   spine cascades to ConfigPriceOracle. NEVER fabricates a price.
//! - Allowlist empty → worker logs and skips its tick (no input to fetch for).
//!
//! ## Token discovery (PC2 — deterministic, route-graph complete)
//!
//! Fetches `(symbol, address)` tuples from two Redis sources:
//!   1. `allowed_token_symbols` from `arbx:trading_config:<chain_id>` —
//!      operator's universe (high-priority refresh).
//!   2. `arbx:tokens:<chain_id>:<address>` — every token the pool sync
//!      worker has discovered (broad coverage).
//!
//! and one route-graph source:
//!   3. `arbx:pool_index[:_v3]:<chain>:<sym0>:<sym1>` key names — the tokens
//!      that are actually in a detected pool, i.e. the ones the graph can
//!      route through and the cartridges therefore evaluate. Deduped against
//!      the allowlist, **sorted ascending by symbol**.
//!
//! The two classes answer to different provider contracts, so they are bound
//! differently:
//!   - **Alchemy** (symbol-keyed, `MAX_BATCH_SIZE=5`): allowlist always fully
//!     included, then a deterministic *rotating window* of the route graph up
//!     to `MAX_PRICED_TOKENS` (<= 60 calls/tick, unchanged).
//!   - **Coingecko** (by contract address, `MAX_COINGECKO_BATCH_SIZE=25`): the
//!     allowlist leftovers first — capped at `budget - 1` whenever a route
//!     graph exists — then a rotating window over the FULL route-graph set,
//!     under one shared `MAX_COINGECKO_CALLS_PER_TICK` budget (<= 5 calls/tick).
//!     This is what closes the coverage gap: the route graph
//!     is swept completely every `ceil(len / window)` ticks instead of being
//!     sampled by hash-set order, and the budget cannot be exceeded no matter
//!     how many pools discovery enumerates.

use crate::counters::counters;
use crate::reserves::TokenMeta;
use redis::aio::ConnectionManager;
use redis::AsyncCommands;
use serde::{Deserialize, Serialize};
use shared_rs::price_oracle::redis_token_prices_key;
use shared_rs::trading_config::{redis_key as trading_config_redis_key, TradingConfigState};
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};
use tokio::time::interval;
use tracing::{debug, info, warn};

/// Default refresh period. Operator overrides via `PRICE_WORKER_INTERVAL_SECS`.
pub const DEFAULT_PERIOD_SECS: u64 = 30;
/// Cache TTL on the populated Redis hash. Set to 2× the worker period so a
/// single missed tick doesn't empty the cache (gives the worker one chance to
/// recover before downstream `UnknownTokenPrice` rejections start firing).
pub const CACHE_TTL_MULTIPLIER: u64 = 2;
/// Provider request limit safety bound for **Alchemy**; chunk its candidate
/// list into this many tokens per HTTP call. Alchemy free tier rejects
/// batches above ~10 addresses with 400 Bad Request (observed 2026-08-15), so
/// this stays small — see `MAX_COINGECKO_BATCH_SIZE` for the Coingecko side,
/// whose contract-address endpoint takes far more per request and must NOT
/// inherit this Alchemy-driven limit.
pub const MAX_BATCH_SIZE: usize = 5;
/// Per-request HTTP timeout. Prices are best-effort; we never want a hung
/// upstream to delay the next worker tick.
pub const HTTP_TIMEOUT_SECS: u64 = 8;
/// PC2 (PRICE-COVERAGE-02) — addresses per Coingecko request, decoupled from
/// the Alchemy-driven `MAX_BATCH_SIZE`. `simple/token_price` accepts a
/// comma-separated `contract_addresses` list; charging it 5 addresses per call
/// is what turned a 300-token universe into 60 requests per tick, i.e. 120
/// requests/min against a keyless public tier whose published band is 5–15
/// requests/min — precisely the 429 the tier lives in (measured 2026-10-03:
/// 16 of 17 consecutive ticks logged `price_worker.coingecko_failed` 429).
/// 25 × 43 B ≈ 1.1 kB of query string: far below the public edge's request-line
/// limit, while cutting the request count for the same coverage 5×.
pub const MAX_COINGECKO_BATCH_SIZE: usize = 25;
/// PC2 — hard ceiling of Coingecko requests per refresh tick. BOTH call sites
/// (the allowlist/Chainlink/Alchemy leftovers and the pool-resident route-graph
/// window) draw from this single budget, so the tier can never exceed it no
/// matter how large the universe grows: 5 requests / 30 s = 10 requests/min,
/// inside the keyless public band (5–15/min; 30/min with a free Demo key) and a
/// 12× reduction from the 60 requests/tick the previous design targeted.
pub const MAX_COINGECKO_CALLS_PER_TICK: usize = 5;
/// PC2 — compile-time invariant: paying Alchemy's 5-address ceiling on Coingecko
/// is what made a full sweep a rate-limit storm, so the by-address batch must
/// stay strictly larger than `MAX_BATCH_SIZE`.
const _: () = assert!(MAX_COINGECKO_BATCH_SIZE > MAX_BATCH_SIZE);
/// Hard cap on the per-tick **Alchemy candidate list** (the symbol-keyed
/// snapshot). Allowlist tokens are always included first; pool-resident tokens
/// (recovered from `arbx:pool_index[:_v3]:<chain>:<sym0>:<sym1>` key names)
/// fill the remainder up to this bound through a deterministic rotating window.
/// Protects Alchemy's shared RPC-key budget: at `MAX_BATCH_SIZE=5` this is
/// <= 60 Alchemy calls per tick — unchanged by PC2.
///
/// This cap does **not** bound the route graph any more. Coverage of the tokens
/// carts actually evaluate comes from the deterministic Coingecko-by-address
/// pass, which sweeps the FULL pool-resident set under its own budget
/// (`MAX_COINGECKO_CALLS_PER_TICK`). Truncating this list no longer decides
/// which pool tokens ever get a price — it only decides which of them are
/// additionally offered to Alchemy.
pub const MAX_PRICED_TOKENS: usize = 300;
/// WO-PRICE-SOVEREIGN-01 f1 — per-provider exponential backoff base window
/// (1s). Each consecutive backoff-worthy failure (429/5xx) doubles it.
pub const PRICE_BACKOFF_BASE_MS: u64 = 1_000;
/// Hard cap of one backoff window: 1s → 2s → 4s → … → 60s, never beyond.
pub const PRICE_BACKOFF_CAP_MS: u64 = 60_000;

/// Current wall-clock epoch milliseconds. 0 on clock error — degrades to "not
/// in backoff" (same convention as the previous `unix_now_secs` helper).
fn unix_now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

/// Wall-clock epoch nanoseconds for bus `recv_ns` fields.
fn unix_now_ns() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(0)
}

/// Exponential backoff circuit breaker for ONE price provider
/// (WO-PRICE-SOVEREIGN-01 f1). Replaces the fixed CG429-01 Coingecko window
/// and extends the same protection to Alchemy, whose 429s were being
/// re-triggered ~158×/tick (self-contamination).
///
/// State machine (all transitions pure functions of `now_ms` — no clock and
/// no network inside, so every behaviour is unit-testable):
///
/// ```text
///   healthy --429/5xx--> window = base·2^(failures-1) [cap 60s]
///   in-window --now < deadline--> NO request (skip early)
///   in-window --now >= deadline--> exactly ONE probe request (half-open)
///        probe --429/5xx--> window doubles (failures += 1)
///        probe --success--> window cleared, failures /= 2 (gradual reset)
///   healthy --success--> failures /= 2 (floors at 0)
/// ```
///
/// "Gradual reset": a success HALVES the failure level instead of zeroing it,
/// so a flapping provider re-enters backoff one exponent below its previous
/// peak (peak 4s → one success → next window 2s) rather than restarting from
/// the base window every time; two consecutive successes fully heal.
#[derive(Debug)]
pub struct ProviderBackoff {
    consecutive_failures: u32,
    /// Epoch-ms deadline until which attempts are suppressed. 0 = healthy.
    backoff_until_ms: u64,
    /// Half-open guard: true while the single allowed probe request is in
    /// flight. Prevents a burst of probes the instant a window expires.
    probe_in_flight: bool,
}

impl Default for ProviderBackoff {
    fn default() -> Self {
        Self::new()
    }
}

impl ProviderBackoff {
    pub fn new() -> Self {
        Self {
            consecutive_failures: 0,
            backoff_until_ms: 0,
            probe_in_flight: false,
        }
    }

    /// May the caller send exactly one request right now? `Ok(())` = yes
    /// (this is the half-open probe when a window just expired); `Err(
    /// remaining_ms)` = no, suppress — skip early, do NOT retry.
    pub fn gate(&mut self, now_ms: u64) -> Result<(), u64> {
        if self.probe_in_flight {
            return Err(self.backoff_until_ms.saturating_sub(now_ms));
        }
        if now_ms < self.backoff_until_ms {
            return Err(self.backoff_until_ms - now_ms);
        }
        if self.backoff_until_ms > 0 {
            // Window just expired — this attempt is the single probe.
            self.probe_in_flight = true;
        }
        Ok(())
    }

    /// Register a successful request: clears any open window and the probe
    /// flag, halves the failure level (gradual reset — see struct docs).
    pub fn record_success(&mut self) {
        self.probe_in_flight = false;
        self.backoff_until_ms = 0;
        self.consecutive_failures /= 2;
    }

    /// Register a failed request. Caller decides (via
    /// `error_is_backoff_worthy`) whether the failure opens/doubles the
    /// window; this method always clears the probe flag so the breaker never
    /// wedges on a non-backoff error mid-probe. Returns the window opened
    /// (`Some(ms)`) when the failure was backoff-worthy, `None` otherwise.
    pub fn record_failure(&mut self, now_ms: u64, backoff_worthy: bool) -> Option<u64> {
        self.probe_in_flight = false;
        if !backoff_worthy {
            return None;
        }
        self.consecutive_failures = self.consecutive_failures.saturating_add(1);
        // failures=1 → base (2^0), 2 → 2·base, … capped. Shift width bounded
        // so it can never overflow u64 before the min() clamps to the cap.
        let exp = self.consecutive_failures.saturating_sub(1).min(32);
        let window = PRICE_BACKOFF_BASE_MS
            .saturating_mul(1u64 << exp)
            .min(PRICE_BACKOFF_CAP_MS);
        self.backoff_until_ms = now_ms.saturating_add(window);
        Some(window)
    }

    /// True while a backoff window is open (probe-in-flight included).
    pub fn is_active(&self, now_ms: u64) -> bool {
        self.probe_in_flight || now_ms < self.backoff_until_ms
    }

    /// Current consecutive-failure level (diagnostics + tests).
    pub fn consecutive_failures(&self) -> u32 {
        self.consecutive_failures
    }
}

/// Does this HTTP status open/double a backoff window? Only 429 (rate limit)
/// and 5xx (provider unhealthy) — a 400 bad-batch or 404 is a permanent
/// shape error that hammering-with-pauses will never fix.
fn status_is_backoff_worthy(status: Option<reqwest::StatusCode>) -> bool {
    match status {
        Some(s) => s.as_u16() == 429 || s.is_server_error(),
        None => false,
    }
}

/// `reqwest::error::Error` surfaced through `anyhow` (the `error_for_status`
/// path carries the HTTP status; transport errors carry none).
fn error_is_backoff_worthy(e: &anyhow::Error) -> bool {
    e.downcast_ref::<reqwest::Error>()
        .is_some_and(|re| status_is_backoff_worthy(re.status()))
}

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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TokenRef {
    /// Canonical uppercase symbol used as the Redis hash field key.
    pub symbol: String,
    /// Lowercase 0x-prefixed address used by both upstreams.
    pub address_lower: String,
}

/// PC2 — the tick's pricing universe, split along the two provider contracts.
///
/// - `allowlist`: the operator's `allowed_token_symbols`, in the order the
///   operator declared them, deduped and address-resolved. Never cut by any
///   cap — the curated set is the highest-priority class.
/// - `pool`: every symbol that appears in at least one detected pool
///   (`arbx:pool_index[:_v3]:<chain>:*`) AND has an `arbx:tokens` identity —
///   i.e. exactly the tokens the route graph can route through, which is what
///   the cartridges evaluate. **Sorted ascending by symbol**, deduped against
///   `allowlist`.
///
/// The sort is the determinism fix. The previous implementation collected these
/// symbols into a `HashSet` and consumed them in hash-iteration order, so which
/// of them survived `MAX_PRICED_TOKENS` was decided by a per-instance random
/// seed and re-drawn every tick. A total order derived from data already in
/// hand (the symbol) is stable across processes, restarts and SCAN order, and
/// needs no extra Redis read.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
struct PricingUniverse {
    allowlist: Vec<TokenRef>,
    pool: Vec<TokenRef>,
}

impl PricingUniverse {
    fn is_empty(&self) -> bool {
        self.allowlist.is_empty() && self.pool.is_empty()
    }
}

/// PC2 — deterministic window over a sorted slice: `want` consecutive items
/// starting at `cursor`, wrapping at the end. Pure: an identical
/// `(items, cursor, want)` always yields an identical window, whatever the
/// order the items were *inserted* in.
///
/// Rotation (rather than "take the first `want`") is what makes the budget
/// honest. A fixed prefix would exclude the lexicographic tail forever — the
/// same starvation defect as hash-order sampling, merely deterministic — while
/// rotation reaches every item within `ceil(len / want)` windows. The caller
/// advances `cursor` by the number of symbols it actually handed to a provider,
/// so a full sweep of the route graph stays bounded even when a provider only
/// serves part of a tick.
fn rotating_window(items: &[TokenRef], cursor: usize, want: usize) -> Vec<TokenRef> {
    if items.is_empty() || want == 0 {
        return Vec::new();
    }
    let want = want.min(items.len());
    let start = cursor % items.len();
    let mut out = Vec::with_capacity(want);
    for i in 0..want {
        out.push(items[(start + i) % items.len()].clone());
    }
    out
}

/// PC2 — collect symbols into the single canonical order used by the pool side
/// of the universe: UPPERCASE, deduped, ascending. Pure, so the same symbol set
/// yields the same order regardless of the order it arrived in (Redis `SCAN`
/// order is arbitrary and must not leak into selection).
fn sorted_unique_symbols<I: IntoIterator<Item = String>>(symbols: I) -> Vec<String> {
    symbols
        .into_iter()
        .filter(|s| !s.is_empty())
        .collect::<std::collections::BTreeSet<String>>()
        .into_iter()
        .collect()
}

/// PC2 — record `symbol → address` deterministically.
///
/// A symbol can name several contracts (measured on production 2026-10-03:
/// `FLUID` has 5 metas in `arbx:tokens:1:*`, `DMC`/`BTL`/`BAVAR`/`APPLE` 4
/// each), and the previous `entry().or_insert()` kept whichever the SCAN order
/// happened to yield first — a second, silent nondeterministic input into the
/// universe (the same symbol could resolve to a different contract after a
/// restart, and therefore be asked of the provider at a different address).
/// Keeping the lexicographically smallest address makes the mapping a pure
/// function of the meta set.
fn record_symbol_addr(
    sym_to_addr: &mut HashMap<String, String>,
    symbol: &str,
    address_lower: &str,
) {
    use std::collections::hash_map::Entry;
    let upper = symbol.to_ascii_uppercase();
    match sym_to_addr.entry(upper) {
        Entry::Occupied(mut e) => {
            if address_lower < e.get().as_str() {
                e.insert(address_lower.to_string());
            }
        }
        Entry::Vacant(e) => {
            e.insert(address_lower.to_string());
        }
    }
}

/// PC2 — how many of the shared per-tick Coingecko calls the allowlist pass may
/// use. The curated set keeps absolute priority over the leftovers the Alchemy
/// snapshot did not price (21 unique symbols on `arbx:1`), but it may not
/// consume the whole tier: when a route graph exists, one call is always held
/// back so the sweep advances every tick. Starving the allowlist would break the
/// "operator's set is always priced" invariant; starving the sweep is the exact
/// defect this PR removes, so neither is allowed to win outright.
fn allowlist_cg_call_budget(pool_len: usize) -> usize {
    if pool_len == 0 {
        MAX_COINGECKO_CALLS_PER_TICK
    } else {
        MAX_COINGECKO_CALLS_PER_TICK.saturating_sub(1)
    }
}

/// PRICE-COVERAGE-01 (PC3) — allowlist symbols that are GENUINELY missing a price
/// this tick: neither priced by a provider already (`tick_prices`) nor already
/// PUBLISHED (and therefore fresh) in `arbx:token_prices:<chain>`
/// (`published_upper`).
///
/// Why "published" counts as present: `RedisCachedPriceOracle` — the reader the
/// whole pipeline uses — reads exactly that hash, and its TTL is
/// `period × CACHE_TTL_MULTIPLIER` (≥60 s) precisely so a price survives a
/// missed tick. Re-asking a provider for a value that is ALREADY published buys
/// nothing and costs the shared per-tick call budget.
///
/// MEDIDO EN PRODUCCIÓN (chain 1, ventana de 24 ticks, 2026-10-04): el pase de
/// allowlist pedía a Coingecko 16 símbolos — COMP, MKR, UNI, SAND, ENS, SHIB,
/// RETH, APE, MATIC, SUSHI, LINK, LDO, MANA, PEPE, AAVE, CRV — de los cuales
/// **16/16 ya estaban publicados** en el hash (TTL 51 s). Esas peticiones
/// repetidas devolvían 429 (`price_worker.coingecko_failed`, 9 en la ventana),
/// abrían la ventana de backoff del proveedor y — como el chequeo del breaker
/// está DENTRO de los dos bucles — dejaban el sweep por address en
/// `pool_cg_attempted = 0` en **24/24 ticks**. Ese sweep es el ÚNICO productor
/// posible de los tokens long-tail ruteados (los 16 majors tienen Chainlink,
/// Binance y `trading_config.token_prices_usd`; BORIS/FUND5/💫MSG no tienen
/// ninguno), así que el presupuesto del proveedor debe gastarse en él.
///
/// Puro: mismo orden que la allowlist, sin I/O — la selección es testeable en
/// los dos sentidos.
fn allowlist_remaining(
    allowlist: &[TokenRef],
    tick_prices: &HashMap<String, f64>,
    published_upper: &std::collections::HashSet<String>,
) -> Vec<TokenRef> {
    allowlist
        .iter()
        .filter(|t| !tick_prices.contains_key(&t.symbol) && !published_upper.contains(&t.symbol))
        .cloned()
        .collect()
}

/// PC2 — assemble the universe deterministically.
///
/// `pool_symbols` is expected UPPERCASE, deduped and sorted (see
/// `scan_pool_resident_symbols`); `sym_to_addr` is only ever *looked up*, never
/// iterated, so its internal layout cannot influence the result. Symbols with
/// no `arbx:tokens` identity are dropped: an address we cannot name is not a
/// token we may ask a provider about, and its honest absence stays an absence
/// (R8 — no fabricated address, no invented price).
fn build_universe(
    allowlist: &[String],
    sym_to_addr: &HashMap<String, String>,
    pool_symbols: &[String],
) -> PricingUniverse {
    let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();
    let mut universe = PricingUniverse::default();
    for sym in allowlist {
        let upper = sym.to_ascii_uppercase();
        if !seen.insert(upper.clone()) {
            continue;
        }
        if let Some(addr) = sym_to_addr.get(&upper) {
            universe.allowlist.push(TokenRef {
                symbol: upper,
                address_lower: addr.clone(),
            });
        }
    }
    // `pool_symbols` is sorted, so the pushed order — and therefore the rotation
    // stride, the wire order and the log — is fully reproducible.
    for sym in pool_symbols {
        if !seen.insert(sym.clone()) {
            continue;
        }
        if let Some(addr) = sym_to_addr.get(sym) {
            universe.pool.push(TokenRef {
                symbol: sym.clone(),
                address_lower: addr.clone(),
            });
        }
    }
    universe
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
#[derive(Clone)]
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
    /// Optional Coingecko Demo/Pro API key. When set, `fetch_coingecko` attaches
    /// the `x-cg-demo-api-key` header so the call is authenticated (the free
    /// tier now 400s unauthenticated calls). `None` → current unauth behavior.
    pub coingecko_api_key: Option<String>,
    /// Tier-0 Chainlink source. `db_pool` reads operator-seeded `price_oracles`
    /// (kind='chainlink', enabled); `rpc_http_url` is the chain's HTTP RPC for
    /// `eth_call latestRoundData()`. Both must be present to enable the tier;
    /// otherwise it is skipped (Chainlink prices = none, cascade falls through).
    pub db_pool: Option<sqlx::postgres::PgPool>,
    pub rpc_http_url: Option<String>,
    /// WO-PC3/WO-PC7(b) — process price bus. When attached, `fetch_chainlink`
    /// publishes each anchor to the bus (fused with Binance bookTicker), and
    /// `run_one_tick` lets fused bus prices override the provider cascade
    /// before persisting. `None` in tests / when the bus is not booted.
    pub price_bus: Option<std::sync::Arc<shared_rs::price_bus::PriceBus>>,
}

impl PriceWorkerConfig {
    pub fn new(chain_id: u64, period_secs: u64, alchemy_api_key: Option<String>) -> Self {
        Self {
            chain_id,
            period: Duration::from_secs(period_secs.max(1)),
            alchemy_base_url_override: None,
            alchemy_api_key,
            coingecko_base_url_override: None,
            coingecko_api_key: None,
            db_pool: None,
            rpc_http_url: None,
            price_bus: None,
        }
    }

    /// Enable the Chainlink Tier-0 source (read-only on-chain price feeds).
    pub fn with_chainlink(mut self, db_pool: sqlx::postgres::PgPool, rpc_http_url: String) -> Self {
        self.db_pool = Some(db_pool);
        self.rpc_http_url = Some(rpc_http_url);
        self
    }

    /// WO-PC3 — attach the process price bus for anchor publication and
    /// fused-price precedence (WO-PC7(b)).
    pub fn with_price_bus(mut self, bus: std::sync::Arc<shared_rs::price_bus::PriceBus>) -> Self {
        self.price_bus = Some(bus);
        self
    }
}

impl std::fmt::Debug for PriceWorkerConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // PriceBus is not Debug (lock-free internals); print presence only.
        f.debug_struct("PriceWorkerConfig")
            .field("chain_id", &self.chain_id)
            .field("period", &self.period)
            .field(
                "alchemy_api_key",
                &self.alchemy_api_key.as_ref().map(|_| "<redacted>"),
            )
            .field(
                "coingecko_api_key",
                &self.coingecko_api_key.as_ref().map(|_| "<redacted>"),
            )
            .field("db_pool", &self.db_pool.as_ref().map(|_| "<set>"))
            .field("rpc_http_url", &self.rpc_http_url)
            .field("price_bus", &self.price_bus.as_ref().map(|_| "<attached>"))
            .finish()
    }
}

pub struct PriceWorker {
    cfg: PriceWorkerConfig,
    http: reqwest::Client,
    /// WO-PRICE-SOVEREIGN-01 f1 — per-provider circuit breakers. Mutex (never
    /// contended: one worker task per chain) so `run_one_tick(&self)` can
    /// gate + record without taking `&mut self`.
    alchemy_backoff: std::sync::Mutex<ProviderBackoff>,
    coingecko_backoff: std::sync::Mutex<ProviderBackoff>,
    /// PC2 — deterministic sweep cursors, in SYMBOL units, into
    /// `PricingUniverse::pool`. Each tick advances a cursor by the number of
    /// pool symbols actually handed to that provider, so route-graph coverage
    /// advances monotonically (a full sweep every `ceil(len / window)` ticks)
    /// instead of being re-sampled by hash order. Atomic for the same
    /// one-task-per-chain reason as the breakers.
    alchemy_cursor: AtomicU64,
    coingecko_pool_cursor: AtomicU64,
}

/// Which provider a fetch went to — selects the breaker instance, the
/// heartbeat counters and the log event names.
#[derive(Debug, Clone, Copy)]
enum PriceProvider {
    Alchemy,
    Coingecko,
}

impl PriceProvider {
    fn as_str(self) -> &'static str {
        match self {
            Self::Alchemy => "alchemy",
            Self::Coingecko => "coingecko",
        }
    }
}

impl PriceWorker {
    pub fn new(cfg: PriceWorkerConfig) -> anyhow::Result<Self> {
        let http = reqwest::Client::builder()
            .timeout(Duration::from_secs(HTTP_TIMEOUT_SECS))
            .user_agent("arbitragex-v2-price-worker/0.1")
            .build()?;
        Ok(Self {
            cfg,
            http,
            alchemy_backoff: std::sync::Mutex::new(ProviderBackoff::new()),
            coingecko_backoff: std::sync::Mutex::new(ProviderBackoff::new()),
            alchemy_cursor: AtomicU64::new(0),
            coingecko_pool_cursor: AtomicU64::new(0),
        })
    }

    fn backoff_for(&self, provider: PriceProvider) -> &std::sync::Mutex<ProviderBackoff> {
        match provider {
            PriceProvider::Alchemy => &self.alchemy_backoff,
            PriceProvider::Coingecko => &self.coingecko_backoff,
        }
    }

    /// May we send ONE request to this provider right now? False = backoff
    /// window open → the caller must skip early (whole remaining loop, not
    /// just this chunk). Poisoned lock = a panic while holding it; the state
    /// machine is still memory-valid, recover it (same stance as counters).
    fn gate_provider(&self, provider: PriceProvider, now_ms: u64) -> bool {
        let mut bo = self
            .backoff_for(provider)
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        bo.gate(now_ms).is_ok()
    }

    /// Register a successful provider request: breaker reset (gradual) +
    /// active-gauge to 0.
    fn record_provider_success(&self, provider: PriceProvider) {
        let mut bo = self
            .backoff_for(provider)
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        bo.record_success();
        let c = crate::counters::chain_counters(self.cfg.chain_id);
        match provider {
            PriceProvider::Alchemy => c.price_alchemy_backoff_active.store(0, Ordering::Relaxed),
            PriceProvider::Coingecko => {
                c.price_coingecko_backoff_active.store(0, Ordering::Relaxed)
            }
        }
    }

    /// Register a failed provider request. Returns `Some(window_ms)` when the
    /// failure was backoff-worthy (429/5xx) and opened/doubled a window —
    /// the caller logs the transition INFO. Bumps the per-provider
    /// `*_backoff_total` counter and sets the active gauge on entry.
    fn record_provider_failure(
        &self,
        provider: PriceProvider,
        e: &anyhow::Error,
        now_ms: u64,
    ) -> Option<u64> {
        let worthy = error_is_backoff_worthy(e);
        let mut bo = self
            .backoff_for(provider)
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let window = bo.record_failure(now_ms, worthy);
        drop(bo);
        if window.is_some() {
            let c = crate::counters::chain_counters(self.cfg.chain_id);
            match provider {
                PriceProvider::Alchemy => {
                    c.price_alchemy_backoff_total
                        .fetch_add(1, Ordering::Relaxed);
                    c.price_alchemy_backoff_active.store(1, Ordering::Relaxed);
                }
                PriceProvider::Coingecko => {
                    c.price_coingecko_backoff_total
                        .fetch_add(1, Ordering::Relaxed);
                    c.price_coingecko_backoff_active.store(1, Ordering::Relaxed);
                }
            }
        }
        window
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
                        bus_fused_hits = stats.bus_fused_hits,
                        cache_misses = stats.cache_misses,
                        attempted = stats.attempted,
                        // PC2 — route-graph coverage is observable: a full sweep
                        // of `pool_resident` tokens takes
                        // ceil(pool_resident / pool_cg_attempted) ticks.
                        pool_resident = stats.pool_resident,
                        pool_cg_attempted = stats.pool_cg_attempted,
                        allowlist_cached_skipped = stats.allowlist_cached_skipped,
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

        let universe = self.discover_tokens(redis).await?;
        if universe.is_empty() {
            // No allowlist tokens for the external APIs, but Chainlink prices (if
            // any) are still worth persisting so the cascade can serve them.
            if !prices.is_empty() {
                self.fuse_bus_prices(&mut prices);
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

        // PC2 — the tick's Alchemy candidate list. Same bound as before
        // (`MAX_PRICED_TOKENS`, `MAX_BATCH_SIZE=5` ⇒ <= 60 calls) but no longer
        // hash-ordered: the operator allowlist is always fully included, and the
        // pool-resident remainder is a deterministic rotating window, so every
        // route-graph token is offered to Alchemy within `ceil(pool / remainder)`
        // ticks instead of being re-sampled at random each tick.
        let allow_n = universe.allowlist.len().min(MAX_PRICED_TOKENS);
        let alchemy_budget = MAX_PRICED_TOKENS.saturating_sub(allow_n);
        let alchemy_cursor = self.alchemy_cursor.load(Ordering::Relaxed) as usize;
        let mut tokens: Vec<TokenRef> = universe.allowlist[..allow_n].to_vec();
        tokens.extend(rotating_window(
            &universe.pool,
            alchemy_cursor,
            alchemy_budget,
        ));

        // Tier 1: Alchemy batch (fills only what Chainlink did NOT already price).
        // Circuit breaker (WO-PRICE-SOVEREIGN-01 f1): one 429/5xx opens an
        // exponential window; while it is open the loop breaks immediately —
        // a 158-token cache-miss tick must NOT become 158 Alchemy calls.
        let mut alchemy_hits = 0usize;
        let mut alchemy_attempted = 0usize;
        if self.cfg.alchemy_api_key.is_some() {
            for chunk in tokens.chunks(MAX_BATCH_SIZE) {
                if !self.gate_provider(PriceProvider::Alchemy, unix_now_ms()) {
                    break; // backoff active — skip early, no request, no retry.
                }
                alchemy_attempted += chunk.len();
                match self.fetch_alchemy(chunk).await {
                    Ok(map) => {
                        self.record_provider_success(PriceProvider::Alchemy);
                        for (sym, price) in map {
                            if let std::collections::hash_map::Entry::Vacant(e) = prices.entry(sym)
                            {
                                e.insert(price);
                                alchemy_hits += 1;
                            }
                        }
                    }
                    Err(e) => {
                        let backoff_ms =
                            self.record_provider_failure(PriceProvider::Alchemy, &e, unix_now_ms());
                        counters()
                            .price_worker_errors
                            .fetch_add(1, Ordering::Relaxed);
                        if let Some(retry_in_ms) = backoff_ms {
                            info!(
                                event = "price_worker.alchemy_backoff",
                                chain_id = self.cfg.chain_id,
                                provider = PriceProvider::Alchemy.as_str(),
                                retry_in_ms,
                                "Alchemy entering exponential backoff; remaining tokens fall through to Coingecko/ConfigPriceOracle"
                            );
                        }
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
        // PC2 — advance the Alchemy sweep by the symbols actually offered to it
        // (a suppressed-backoff tick advances nothing, which is correct: no
        // request was made, so nothing was covered). Modulo keeps the cursor
        // inside the pool list even if the window covered all of it.
        if !universe.pool.is_empty() {
            let next = (alchemy_cursor + alchemy_attempted) % universe.pool.len();
            self.alchemy_cursor.store(next as u64, Ordering::Relaxed);
        }

        // Tier 2: Coingecko, by contract address, under ONE shared per-tick call
        // budget (PC2). Two passes draw from the same budget, in priority order:
        //
        //   1. the operator allowlist's leftovers — the curated set is the
        //      highest-priority class and may never be crowded out, but it also
        //      may not eat the whole tier: one call is always reserved for pass 2
        //      below whenever there is a route graph to sweep;
        //   2. a deterministic ROTATING WINDOW over the FULL pool-resident
        //      route-graph set, which `MAX_PRICED_TOKENS` does NOT bound.
        //
        // The capped snapshot's *pool* leftovers are deliberately NOT a third
        // pass: they are a subset of the route-graph set that pass 2 sweeps in
        // guaranteed order, so asking for them here would only burn the shared
        // budget (and, with 295 unpriced snapshot tokens — the production shape —
        // it would consume all 5 calls and starve the route graph entirely).
        //
        // Pass 2 is the coverage mechanism: it batches by contract address over
        // the set the carts actually route through, so the tokens the graph can
        // reach are swept completely every `ceil(pool / window)` ticks instead
        // of being sampled by hash order. Provider behaviour is unchanged — both
        // passes share the same breaker, counters and log events, and the budget
        // means the tier can never exceed 5 requests per tick whatever the
        // universe size.
        let allow_calls_cap = allowlist_cg_call_budget(universe.pool.len());
        // PRICE-COVERAGE-01 (PC3): el pase de allowlist NO vuelve a preguntar por
        // un símbolo que ya está publicado y fresco (`arbx:token_prices:<chain>`,
        // el MISMO hash que lee `RedisCachedPriceOracle`). Medido: 16/16 de los
        // símbolos que pedía ya estaban publicados; esas peticiones redundantes
        // devolvían 429, abrían el breaker compartido y dejaban el sweep por
        // address en `pool_cg_attempted = 0` durante 24/24 ticks — y ese sweep es
        // el único productor de los tokens long-tail ruteados.
        let published = self.published_symbols(redis).await;
        let allowlist_cached_skipped = universe
            .allowlist
            .iter()
            .filter(|t| !prices.contains_key(&t.symbol) && published.contains(&t.symbol))
            .count();
        let missing_allow = allowlist_remaining(&universe.allowlist, &prices, &published);
        let mut coingecko_hits = 0usize;
        let mut cg_calls = 0usize;
        for chunk in missing_allow.chunks(MAX_COINGECKO_BATCH_SIZE) {
            // Circuit breaker (WO-PRICE-SOVEREIGN-01 f1, supersedes the fixed
            // CG429-01 window): skip Coingecko entirely while the exponential
            // backoff window is active — no request, no WARN. The missing tokens
            // simply stay unpriced and cascade to ConfigPriceOracle (R8).
            if cg_calls >= allow_calls_cap
                || !self.gate_provider(PriceProvider::Coingecko, unix_now_ms())
            {
                break;
            }
            cg_calls += 1;
            coingecko_hits += self.coingecko_batch_into(chunk, &mut prices).await;
        }

        // Pass 2 — route-graph coverage, independent of the capped snapshot.
        let mut pool_cg_attempted = 0usize;
        if !universe.pool.is_empty() {
            let cg_calls_left = MAX_COINGECKO_CALLS_PER_TICK.saturating_sub(cg_calls);
            let pool_cursor = self.coingecko_pool_cursor.load(Ordering::Relaxed) as usize;
            let window = rotating_window(
                &universe.pool,
                pool_cursor,
                cg_calls_left * MAX_COINGECKO_BATCH_SIZE,
            );
            for chunk in window.chunks(MAX_COINGECKO_BATCH_SIZE) {
                if !self.gate_provider(PriceProvider::Coingecko, unix_now_ms()) {
                    break;
                }
                pool_cg_attempted += chunk.len();
                coingecko_hits += self.coingecko_batch_into(chunk, &mut prices).await;
            }
            // Advance only by what was actually queried: a tick that served one
            // chunk still moves the sweep one chunk forward, so a provider that
            // answers slowly still reaches every route-graph token — the sweep
            // can stretch, it cannot stall on a hash-ordered subset.
            if pool_cg_attempted > 0 {
                let next = (pool_cursor + pool_cg_attempted) % universe.pool.len();
                self.coingecko_pool_cursor
                    .store(next as u64, Ordering::Relaxed);
            }
        }
        counters()
            .price_coingecko_hits
            .fetch_add(coingecko_hits as u64, Ordering::Relaxed);

        // PC2 — count the snapshot tokens still without a price directly instead
        // of `tokens.len() - prices.len()`: `prices` now also carries the
        // route-graph hits from the by-address pass, which are not members of the
        // snapshot and would otherwise mask real misses in this gauge.
        let cache_misses = tokens
            .iter()
            .filter(|t| !prices.contains_key(&t.symbol))
            .count();
        counters()
            .price_cache_misses
            .fetch_add(cache_misses as u64, Ordering::Relaxed);

        // WO-PC7(b): fused bus prices (Binance + Chainlink) override the
        // provider cascade before persistence (R8 as in the empty-tokens path).
        let bus_fused_hits = self.fuse_bus_prices(&mut prices);

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
            bus_fused_hits,
            cache_misses,
            pool_resident: universe.pool.len(),
            pool_cg_attempted,
            allowlist_cached_skipped,
            elapsed_ms: started.elapsed().as_millis() as u64,
        })
    }

    /// PC2 — one Coingecko batch, with the shared breaker/counter/log handling
    /// and the merge rule in a single place so both passes (snapshot leftovers
    /// and the pool-resident window) behave identically. Returns how many NEW
    /// symbols this batch priced. A failing batch leaves `prices` untouched and
    /// returns 0: nothing is invented for a symbol whose provider said nothing
    /// (R8).
    async fn coingecko_batch_into(
        &self,
        chunk: &[TokenRef],
        prices: &mut HashMap<String, f64>,
    ) -> usize {
        match self.fetch_coingecko(chunk).await {
            Ok(map) => {
                self.record_provider_success(PriceProvider::Coingecko);
                let mut hits = 0usize;
                for (sym, price) in map {
                    // Don't overwrite an existing Chainlink/Alchemy hit. Use Entry
                    // API for clippy::map_entry compliance + clearer intent.
                    if let std::collections::hash_map::Entry::Vacant(e) = prices.entry(sym) {
                        e.insert(price);
                        hits += 1;
                    }
                }
                hits
            }
            Err(e) => {
                let backoff_ms =
                    self.record_provider_failure(PriceProvider::Coingecko, &e, unix_now_ms());
                counters()
                    .price_worker_errors
                    .fetch_add(1, Ordering::Relaxed);
                // INFO only when this failure actually opened/doubled a
                // window — ops sees the state once, not per tick.
                if let Some(retry_in_ms) = backoff_ms {
                    info!(
                        event = "price_worker.coingecko_backoff",
                        chain_id = self.cfg.chain_id,
                        provider = PriceProvider::Coingecko.as_str(),
                        retry_in_ms,
                        "Coingecko entering exponential backoff; affected tokens fall through to ConfigPriceOracle"
                    );
                }
                warn!(
                    event = "price_worker.coingecko_failed",
                    chain_id = self.cfg.chain_id,
                    chunk_size = chunk.len(),
                    error = %e,
                    "Coingecko batch failed; affected tokens will fall through to ConfigPriceOracle"
                );
                0
            }
        }
    }

    /// Reads the operator's allowlist from `trading_config` (provides symbol→
    /// preferred ordering) and joins against `arbx:tokens:<chain>:<addr>` to
    /// recover addresses. Tokens for which no meta entry exists are skipped
    /// silently — pool sync worker will eventually populate them, and on the
    /// next tick they'll be picked up.
    ///
    /// PC2 — returns the two classes separately (`PricingUniverse`) instead of
    /// one capped `Vec`, because they answer to different providers with
    /// different budgets. Nothing here is order-dependent: the allowlist keeps
    /// the operator's declared order, the pool side is sorted, and the address
    /// map is only ever looked up.
    async fn discover_tokens(
        &self,
        redis: &mut ConnectionManager,
    ) -> anyhow::Result<PricingUniverse> {
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
                    return Ok(PricingUniverse::default());
                }
            },
            None => {
                debug!(
                    event = "price_worker.no_trading_config",
                    chain_id = self.cfg.chain_id,
                    "no trading_config in Redis yet (operator hasn't seeded); skipping tick"
                );
                return Ok(PricingUniverse::default());
            }
        };
        if allowlist.is_empty() {
            return Ok(PricingUniverse::default());
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
                    // PC2 — deterministic when a symbol names several contracts:
                    // the lexicographically smallest address wins regardless of
                    // the SCAN order this process happened to see.
                    record_symbol_addr(&mut sym_to_addr, &meta.symbol, &addr_lower);
                }
            }
        }

        // Step 3: assemble the universe. Allowlist first (operator's curated
        // high-confidence set — always fully priced), THEN every symbol
        // recovered from `arbx:pool_index[:_v3]:<chain>:<sym0>:<sym1>` key
        // names that has an identity, deduped against the allowlist, in the
        // single canonical sorted order. Tokens with no `arbx:tokens` meta (no
        // resolvable address) are dropped fail-honestly (R8): we will not ask
        // a provider about an address we cannot name.
        //
        // Where the budget bites is now decided by `run_one_tick`, per provider
        // (rotating window for Alchemy, dedicated by-address pass for Coingecko)
        // — never by hash order, and never for the route graph as a whole.
        let pool_symbols = self.scan_pool_resident_symbols(redis).await;
        Ok(build_universe(&allowlist, &sym_to_addr, &pool_symbols))
    }

    /// PRICE-COVERAGE-01 (PC3) — símbolos YA publicados en
    /// `arbx:token_prices:<chain>` (normalizados a MAYÚSCULAS), para no volver a
    /// preguntar a un proveedor por un precio que el lector canónico
    /// (`RedisCachedPriceOracle`) ya puede servir.
    ///
    /// Mismo hash y misma clave que usa `persist_prices`, que ya lo lee con
    /// `hgetall` para su guarda de plausibilidad; aquí basta `HKEYS`. Un fallo de
    /// Redis devuelve el conjunto VACÍO (R8): sin evidencia de que el precio esté
    /// publicado, el símbolo se sigue pidiendo al proveedor — nunca se ASUME que
    /// un precio existe.
    async fn published_symbols(
        &self,
        redis: &mut ConnectionManager,
    ) -> std::collections::HashSet<String> {
        let key = redis_token_prices_key(self.cfg.chain_id);
        let fields: Vec<String> = redis.hkeys(&key).await.unwrap_or_default();
        fields.into_iter().map(|s| s.to_ascii_uppercase()).collect()
    }

    /// Scan `arbx:pool_index:<chain>:*` and `arbx:pool_index_v3:<chain>:*` key
    /// names and return the UPPERCASE token symbols that appear in at least one
    /// detected pool — **deduped and sorted ascending** (PC2). Key names encode
    /// both symbols (`arbx:pool_index:<chain>:<sym0>:<sym1>`); we take the last
    /// two colon-separated segments and uppercase them to match the
    /// `sym_to_addr` map (writers use mixed case: `pool_discovery` lowercases,
    /// `pool_sync_worker` boot preserves the PG symbol). Returns an empty list
    /// on SCAN failure — the pricing universe then falls back to allowlist only
    /// (R8 fail-honest: no fabricated symbols).
    ///
    /// The sorted order is the fix: the previous `HashSet` returned these in
    /// hash-iteration order, so with more pool-resident tokens than the cap
    /// allowed, *which* tokens got a price was re-drawn at random every tick —
    /// long-tail tokens like LAR (`arbx:pool_index:1:lar:weth`, identity
    /// present, price absent) were sampled probabilistically and effectively
    /// never reached `arbx:token_prices:1`.
    async fn scan_pool_resident_symbols(&self, redis: &mut ConnectionManager) -> Vec<String> {
        let mut out: Vec<String> = Vec::new();
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
                        out.push(upper);
                    }
                }
            }
            drop(iter);
        }
        sorted_unique_symbols(out)
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
        let mut request = self.http.get(&base).query(&[
            ("contract_addresses", addresses.join(",")),
            ("vs_currencies", "usd".to_string()),
        ]);
        // Coingecko's free tier now 400s unauthenticated calls. Attach the Demo
        // key header when configured (Plan A.2 code-gap). No key → unchanged.
        if let Some(key) = &self.cfg.coingecko_api_key {
            request = request.header("x-cg-demo-api-key", key);
        }
        let resp = request.send().await?.error_for_status()?;
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
            let (raw, updated_at) = match self.eth_call_latest_answer(rpc_url, &oracle_addr).await {
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
            // WO-PC3 — publish the anchor to the price bus so the fuser can
            // verify Binance bookTicker against it (freeze latch on divergence).
            if let Some(bus) = self.cfg.price_bus.as_ref() {
                bus.update_anchor(
                    &symbol,
                    shared_rs::price_bus::Anchor {
                        answer: price,
                        updated_at,
                        recv_ns: unix_now_ns(),
                    },
                );
            }
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

    /// WO-PC7(b) — let fused bus prices (Binance speed + Chainlink truth)
    /// take MAX precedence over the provider cascade before persistence.
    /// Verdict handling (R8 fail-honest):
    ///   - fused `Some(p)`  → overwrite the provider price with `p`
    ///   - `DivergenceFrozen` → the pair is frozen (band breach); WITHDRAW the
    ///     symbol from this tick's snapshot entirely — persisting an
    ///     unverified provider value while the fuser distrusts the market
    ///     would be silent fabrication, so the cascade must reject instead
    ///   - other `None` verdicts → bus has no view; keep the provider price
    ///
    /// Returns the number of symbols the bus overrode.
    fn fuse_bus_prices(&self, prices: &mut HashMap<String, f64>) -> usize {
        let Some(bus) = self.cfg.price_bus.as_ref() else {
            return 0;
        };
        let view = bus.view();
        let mut fused = 0usize;
        let symbols: Vec<String> = prices.keys().cloned().collect();
        for sym in symbols {
            match view.price_with_verdict(&sym) {
                (Some(p), _) => {
                    prices.insert(sym, p);
                    fused += 1;
                }
                (None, shared_rs::price_bus::Verdict::DivergenceFrozen) => {
                    prices.remove(&sym);
                    debug!(
                        event = "price_worker.bus_frozen",
                        chain_id = self.cfg.chain_id,
                        symbol = %sym,
                        "divergence band frozen — symbol withheld from snapshot this tick (R8)"
                    );
                }
                (None, _) => {}
            }
        }
        fused
    }

    /// `eth_call latestRoundData()` on a Chainlink aggregator; returns the raw
    /// `answer` (feed units, pre-decimals) and the round's `updated_at`
    /// (epoch seconds). Read-only JSON-RPC via the worker's reqwest client.
    /// `None` on any RPC / parse failure (fail-honest).
    async fn eth_call_latest_answer(&self, rpc_url: &str, oracle_addr: &str) -> Option<(f64, u64)> {
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
        // 5 × 32-byte words; `answer` = word[1] (hex chars 64..128), `updatedAt`
        // = word[3] (hex chars 192..256). The USD price fits within u128, so
        // parse the low 16 bytes of each word (chars 96..128 / 224..256).
        // Chainlink USD answers are positive and timestamps are unsigned, so
        // treating the words as unsigned is correct here.
        if hex.len() < 256 {
            return None;
        }
        let answer_low = &hex[96..128];
        let raw = u128::from_str_radix(answer_low, 16).ok()?;
        let updated_low = &hex[224..256];
        let updated_at = u128::from_str_radix(updated_low, 16).ok()? as u64;
        Some((raw as f64, updated_at))
    }

    async fn persist_prices(
        &self,
        redis: &mut ConnectionManager,
        prices: &HashMap<String, f64>,
    ) -> anyhow::Result<()> {
        let key = redis_token_prices_key(self.cfg.chain_id);
        // PRICE-PLAUSIBILITY-WIRE-01 (audit 2026-09-29): this writer used to HSET
        // every symbol with no sanity check — the ONLY one of the three writers of
        // this hash with no guard. The others consult B5d's decision ladder; this
        // one wrote unconditionally.
        //
        // Why it matters: this is the tier-0 writer (Chainlink anchors + free
        // sources) feeding the same `arbx:token_prices:<chain>` hash the detector
        // reads. A scale/decimal error in ANY source lands here unfiltered and
        // contaminates every gross computation downstream. It already happened
        // once: AAVE shipped as 161_339_420.31 against a real ~161 and SNX as
        // 272_885.72 against ~0.27 — both exactly x1e6. Those two values are
        // literally the fixtures of `is_plausible_price`'s own tests
        // (shared-rs/src/price_oracle.rs:561,563), i.e. the guard was written for
        // exactly this incident and then never wired into the writer that caused it.
        //
        // HONEST LIMIT (measured, not assumed): `is_plausible_price` needs a
        // `prev`, and the published hash has a ~50 s TTL that `persist_prices`
        // refreshes each cycle, so `prev` is normally present; after an expiry the
        // hash reads empty and the first write per symbol is a first-ever write
        // (accepted by design — see the guard's own doc, price_oracle.rs:394).
        // This closes the "poisoned value keeps being accepted" path, not the
        // "no reference exists yet" one. The latter needs B5d's long-lived
        // baseline (`decide_price_write` / `baseline_value`), which has zero
        // production callers today and is left as a separate, operator-visible
        // step because refusing writes changes what the detector can see.
        let published: HashMap<String, f64> = redis.hgetall(&key).await.unwrap_or_default();
        let mut refused: Vec<&str> = Vec::new();
        // Pipeline: HSET each field then EXPIRE on the hash key.
        let mut pipe = redis::pipe();
        pipe.atomic();
        for (sym, price) in prices {
            let prev = published.get(sym).copied();
            if !shared_rs::price_oracle::is_plausible_price(prev, *price) {
                refused.push(sym.as_str());
                warn!(
                    event = "price_worker.implausible_refused",
                    chain_id = self.cfg.chain_id,
                    symbol = %sym,
                    prev = ?prev,
                    candidate = %price,
                    "price outside the plausible tick ratio vs the published value; \
                     keeping the previous value (never persist a suspect price)"
                );
                continue;
            }
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
        // G-PRICE-1: notify subscribers (api-server prices-stream bridge) in the
        // same atomic pipeline — the receiver re-reads the hash for the data.
        pipe.cmd("PUBLISH")
            .arg(shared_rs::price_oracle::prices_updated_channel(
                self.cfg.chain_id,
            ))
            .arg(
                serde_json::json!({
                    "source": "price_worker",
                    "chain_id": self.cfg.chain_id,
                    "written": prices.len(),
                    // PRICE-PLAUSIBILITY-WIRE-01: a suppressed write is a real
                    // upstream event, so it is declared rather than silently
                    // dropped. Additive field — the existing consumer reads
                    // `written`/`chain_id` and re-reads the hash for data.
                    "refused": refused.len(),
                })
                .to_string(),
            )
            .ignore();
        let _: () = pipe.query_async(redis).await?;
        Ok(())
    }
}

#[derive(Debug, Default, Clone)]
pub struct TickStats {
    /// Size of this tick's Alchemy candidate list (allowlist + rotating pool
    /// window), i.e. the symbol-keyed snapshot.
    pub attempted: usize,
    pub chainlink_hits: usize,
    pub alchemy_hits: usize,
    pub coingecko_hits: usize,
    /// WO-PC7(b) — symbols whose persisted price came from the fused bus
    /// (Binance bookTicker verified against the Chainlink anchor).
    pub bus_fused_hits: usize,
    pub cache_misses: usize,
    /// PC2 — size of the pool-resident route-graph set (the tokens carts can
    /// route through), independent of `attempted`.
    pub pool_resident: usize,
    /// PC2 — route-graph symbols actually handed to Coingecko this tick. The
    /// sweep cursor advances by exactly this, so `pool_resident` symbols are
    /// covered every `ceil(pool_resident / pool_cg_attempted)` ticks.
    pub pool_cg_attempted: usize,
    /// PRICE-COVERAGE-01 (PC3) — allowlist symbols NOT re-asked to a provider
    /// because their price was already published and fresh. Observable so the
    /// effect is measurable in production: when this is 0 while the allowlist is
    /// fully priced, the worker is burning the shared tier on values it already
    /// has (the defect this counter was added to expose).
    pub allowlist_cached_skipped: usize,
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

    // --------------- ProviderBackoff state machine (WO-PRICE-SOVEREIGN-01 f1) ---------------
    //
    // Pure unit tests: the state machine takes `now_ms` as a parameter, so
    // every transition, the cap and the gradual reset are verified with NO
    // clock and NO network.

    #[test]
    fn backoff_doubles_per_consecutive_failure_and_caps_at_60s() {
        let mut bo = ProviderBackoff::new();
        let t0 = 1_000_000u64;
        // First backoff-worthy failure opens the base window.
        assert_eq!(bo.record_failure(t0, true), Some(PRICE_BACKOFF_BASE_MS));
        assert_eq!(bo.record_failure(t0, true), Some(2_000));
        assert_eq!(bo.record_failure(t0, true), Some(4_000));
        assert_eq!(bo.record_failure(t0, true), Some(8_000));
        // Keep failing: window must double but NEVER exceed the cap.
        let mut last = 0u64;
        for _ in 0..20 {
            if let Some(w) = bo.record_failure(t0, true) {
                assert!(w >= last, "window must be non-decreasing");
                assert!(w <= PRICE_BACKOFF_CAP_MS, "window {w} exceeds cap");
                last = w;
            }
        }
        assert_eq!(last, PRICE_BACKOFF_CAP_MS, "window must reach the cap");
    }

    #[test]
    fn backoff_non_worthy_failure_does_not_open_window() {
        let mut bo = ProviderBackoff::new();
        let t0 = 5_000u64;
        assert_eq!(bo.record_failure(t0, false), None);
        assert!(
            !bo.is_active(t0),
            "non-backoff failure must not open a window"
        );
        assert!(
            bo.gate(t0).is_ok(),
            "still healthy after non-backoff failure"
        );
        assert_eq!(bo.consecutive_failures(), 0);
    }

    #[test]
    fn backoff_blocks_during_window_then_allows_exactly_one_probe() {
        let mut bo = ProviderBackoff::new();
        let t0 = 5_000u64;
        assert!(bo.gate(t0).is_ok());
        assert_eq!(bo.record_failure(t0, true), Some(PRICE_BACKOFF_BASE_MS));
        // Inside the window: suppressed, and `gate` reports the remainder.
        assert_eq!(bo.gate(t0 + PRICE_BACKOFF_BASE_MS - 1), Err(1));
        assert!(bo.is_active(t0 + PRICE_BACKOFF_BASE_MS - 1));
        // Window expired: the FIRST attempt is the single half-open probe…
        assert!(bo.gate(t0 + PRICE_BACKOFF_BASE_MS).is_ok());
        // …and no second probe is allowed until the first one resolves.
        assert!(
            bo.gate(t0 + PRICE_BACKOFF_BASE_MS + 500).is_err(),
            "probe burst must be suppressed"
        );
        // Probe succeeded → healthy again immediately (gradual reset path).
        bo.record_success();
        assert!(bo.gate(t0 + PRICE_BACKOFF_BASE_MS + 600).is_ok());
        assert!(!bo.is_active(t0 + PRICE_BACKOFF_BASE_MS + 600));
    }

    #[test]
    fn backoff_failed_probe_doubles_the_window() {
        let mut bo = ProviderBackoff::new();
        let t0 = 10_000u64;
        bo.record_failure(t0, true); // 1s window → until 11_000
        let t1 = t0 + 1_000; // expiry → probe
        assert!(bo.gate(t1).is_ok());
        // Probe fails backoff-worthy → window doubles (2s), not restarts at 1s.
        assert_eq!(bo.record_failure(t1, true), Some(2_000));
        assert!(
            bo.gate(t1 + 1_999).is_err(),
            "doubled window must still block"
        );
    }

    #[test]
    fn backoff_resets_gradually_on_success() {
        let mut bo = ProviderBackoff::new();
        let t0 = 10_000u64;
        bo.record_failure(t0, true); // 1s, failures=1
        bo.record_failure(t0, true); // 2s, failures=2
        bo.record_failure(t0, true); // 4s, failures=3
        assert_eq!(bo.consecutive_failures(), 3);
        bo.record_success(); // window cleared, failures = 3/2 = 1 (halved)
        assert!(bo.gate(t0).is_ok());
        // Next failure re-enters one exponent BELOW the 4s peak.
        assert_eq!(bo.record_failure(t0, true), Some(2_000));
        // Two more successes shed the remaining levels → fully healed.
        bo.record_success();
        bo.record_success();
        assert_eq!(bo.consecutive_failures(), 0);
        assert_eq!(bo.record_failure(t0, true), Some(PRICE_BACKOFF_BASE_MS));
        // Successes while already healthy floor at zero (no underflow).
        bo.record_success();
        bo.record_success();
        assert_eq!(bo.consecutive_failures(), 0);
    }

    #[test]
    fn backoff_classifier_accepts_only_429_and_5xx() {
        use reqwest::StatusCode;
        assert!(status_is_backoff_worthy(Some(
            StatusCode::TOO_MANY_REQUESTS
        )));
        assert!(status_is_backoff_worthy(Some(
            StatusCode::INTERNAL_SERVER_ERROR
        )));
        assert!(status_is_backoff_worthy(Some(StatusCode::BAD_GATEWAY)));
        assert!(status_is_backoff_worthy(Some(
            StatusCode::SERVICE_UNAVAILABLE
        )));
        assert!(!status_is_backoff_worthy(Some(StatusCode::BAD_REQUEST)));
        assert!(!status_is_backoff_worthy(Some(StatusCode::UNAUTHORIZED)));
        assert!(!status_is_backoff_worthy(Some(StatusCode::NOT_FOUND)));
        assert!(!status_is_backoff_worthy(None));
        // Non-reqwest errors (and reqwest transport errors, which carry no
        // status) are NOT backoff-worthy per spec.
        let parse_err = anyhow::anyhow!("json parse blew up");
        assert!(!error_is_backoff_worthy(&parse_err));
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

    // --------------- PC2: deterministic, route-graph-complete universe ---------------
    //
    // Fixture shaped from the production measurements of `arbx:1` (2026-10-03):
    // 21 unique allowlist symbols, 478 pool-resident symbols with an identity
    // (1_622 unique pool-resident symbols in total), 300-token cap that bound
    // every single tick (`attempted=300`), and LAR
    // (`arbx:pool_index:1:lar:weth`, identity present, price absent) sitting
    // beyond the cap. Names are chosen so LAR sorts LAST, the worst case for
    // any prefix-style cut.

    const LAR_ADDR: &str = "0x6226caa1857afbc6dfb6ca66071eb241228031a1";

    fn allowlist_21() -> Vec<String> {
        [
            "WETH", "USDC", "USDT", "DAI", "WBTC", "COMP", "MKR", "UNI", "SAND", "ENS", "SHIB",
            "RETH", "APE", "MATIC", "SUSHI", "LINK", "LDO", "MANA", "PEPE", "AAVE", "CRV",
        ]
        .iter()
        .map(|s| (*s).to_string())
        .collect()
    }

    fn meta_map(entries: &[(&str, &str)]) -> HashMap<String, String> {
        let mut m = HashMap::new();
        for (sym, addr) in entries {
            record_symbol_addr(&mut m, sym, addr);
        }
        m
    }

    fn shaped_addr(i: usize) -> String {
        format!("0x{:040x}", i + 1)
    }

    /// 479 pool-resident symbols whose identities all resolve: `A0000..A0477`
    /// (so they sort BEFORE `LAR`) plus the real defect token.
    fn production_shaped_pool_source() -> Vec<String> {
        let mut v: Vec<String> = (0..478).map(|i| format!("A{i:04}")).collect();
        v.push("LAR".to_string());
        v
    }

    fn production_shaped_meta_pairs() -> Vec<(String, String)> {
        let mut pairs: Vec<(String, String)> = Vec::new();
        for (i, sym) in allowlist_21().iter().enumerate() {
            pairs.push((sym.clone(), shaped_addr(i)));
        }
        for (i, sym) in production_shaped_pool_source().iter().enumerate() {
            let addr = if sym == "LAR" {
                LAR_ADDR.to_string()
            } else {
                shaped_addr(1_000 + i)
            };
            pairs.push((sym.clone(), addr));
        }
        pairs
    }

    fn production_shaped_meta() -> HashMap<String, String> {
        let mut m: HashMap<String, String> = HashMap::new();
        for (sym, addr) in production_shaped_meta_pairs() {
            record_symbol_addr(&mut m, &sym, &addr);
        }
        m
    }

    fn production_shaped_universe() -> PricingUniverse {
        build_universe(
            &allowlist_21(),
            &production_shaped_meta(),
            &sorted_unique_symbols(production_shaped_pool_source()),
        )
    }

    /// Walk the Coingecko sweep the way `run_one_tick` does — advancing the
    /// cursor by the symbols actually handed to the provider — and return every
    /// symbol the sweep touches, tick by tick.
    fn sweep_windows(universe: &PricingUniverse, window: usize) -> Vec<Vec<String>> {
        let mut ticks: Vec<Vec<String>> = Vec::new();
        let mut cursor = 0usize;
        while ticks.len() < universe.pool.len() {
            let w = rotating_window(&universe.pool, cursor, window);
            if w.is_empty() {
                break;
            }
            cursor = (cursor + w.len()) % universe.pool.len();
            ticks.push(w.into_iter().map(|t| t.symbol).collect());
            if cursor == 0 {
                break;
            }
        }
        ticks
    }

    #[test]
    fn pc2_selection_is_deterministic_and_independent_of_insertion_order() {
        // Same logical inputs, two different arrival orders (Redis SCAN order is
        // arbitrary and must not leak into selection).
        let forward = production_shaped_pool_source();
        let mut reversed = forward.clone();
        reversed.reverse();
        let map_forward = production_shaped_meta();
        let mut reversed_pairs = production_shaped_meta_pairs();
        reversed_pairs.reverse();
        let mut map_reversed: HashMap<String, String> = HashMap::new();
        for (sym, addr) in &reversed_pairs {
            record_symbol_addr(&mut map_reversed, sym, addr);
        }

        let a = build_universe(
            &allowlist_21(),
            &map_forward,
            &sorted_unique_symbols(forward),
        );
        let b = build_universe(
            &allowlist_21(),
            &map_reversed,
            &sorted_unique_symbols(reversed),
        );
        assert_eq!(a, b, "universe must not depend on input/insertion order");
        // Repeated runs over the same input are identical too.
        assert_eq!(a, production_shaped_universe());
        assert_eq!(b, production_shaped_universe());

        // The pool side is in the single canonical (sorted) order.
        let symbols: Vec<&str> = a.pool.iter().map(|t| t.symbol.as_str()).collect();
        let mut sorted = symbols.clone();
        sorted.sort_unstable();
        assert_eq!(symbols, sorted, "pool side must be sorted ascending");
        assert_eq!(
            a.allowlist
                .iter()
                .map(|t| t.symbol.as_str())
                .collect::<Vec<_>>(),
            allowlist_21()
                .iter()
                .map(|s| s.as_str())
                .collect::<Vec<_>>(),
            "allowlist keeps the operator's declared order"
        );

        // The rotating window is a pure function of (items, cursor, want).
        let w1 = rotating_window(&a.pool, 137, 100);
        let w2 = rotating_window(&b.pool, 137, 100);
        assert_eq!(w1, w2);
        assert_eq!(w1.len(), 100);
        assert_eq!(
            rotating_window(&a.pool, a.pool.len() + 37, 5),
            rotating_window(&a.pool, 37, 5),
            "cursor wraps"
        );
        assert!(rotating_window(&a.pool, 0, 0).is_empty());
    }

    #[test]
    fn pc2_duplicate_symbols_resolve_to_the_smallest_address() {
        // Production has symbols naming several contracts (FLUID x5, DMC x4,
        // BTL x4, ...). The old `entry().or_insert()` kept whatever the SCAN
        // order yielded first, so the address asked of the provider could
        // change between restarts.
        let asc = meta_map(&[("FLUID", "0xaaa"), ("FLUID", "0xbbb")]);
        let desc = meta_map(&[("FLUID", "0xbbb"), ("FLUID", "0xaaa")]);
        assert_eq!(asc.get("FLUID"), Some(&"0xaaa".to_string()));
        assert_eq!(desc.get("FLUID"), Some(&"0xaaa".to_string()));
        assert_eq!(asc, desc);
        // Lowercase input is normalised to the uppercase field key.
        assert_eq!(
            meta_map(&[("lar", LAR_ADDR)]).get("LAR"),
            Some(&LAR_ADDR.to_string())
        );
    }

    #[test]
    fn pc2_pool_resident_token_beyond_the_old_cap_is_now_covered() {
        let universe = production_shaped_universe();
        assert_eq!(universe.allowlist.len(), 21);
        assert_eq!(universe.pool.len(), 479, "478 shaped + LAR");
        assert!(
            universe.pool.iter().any(|t| t.symbol == "LAR"),
            "LAR must be in the universe with its real address"
        );
        assert_eq!(
            universe
                .pool
                .iter()
                .find(|t| t.symbol == "LAR")
                .map(|t| t.address_lower.as_str()),
            Some(LAR_ADDR)
        );

        // Alchemy's cap still applies to its own candidate list (that budget is
        // real: 5-address batches on the shared RPC key)...
        let alchemy_budget = MAX_PRICED_TOKENS - universe.allowlist.len();
        assert_eq!(alchemy_budget, 279);
        let alchemy_window = rotating_window(&universe.pool, 0, alchemy_budget);
        assert_eq!(alchemy_window.len(), 279);
        let cut = &universe.pool[alchemy_budget].symbol;
        assert!(
            !alchemy_window.iter().any(|t| &t.symbol == cut),
            "{cut} is beyond the Alchemy window, as in production"
        );
        assert!(
            !alchemy_window.iter().any(|t| t.symbol == "LAR"),
            "LAR sorts last: no prefix-shaped cut can ever reach it"
        );

        // ...but the Coingecko by-address pass is NOT bound by that cap, and one
        // sweep of the route graph covers every single token, LAR included.
        let window = (MAX_COINGECKO_CALLS_PER_TICK - 1) * MAX_COINGECKO_BATCH_SIZE;
        let ticks = sweep_windows(&universe, window);
        let covered: std::collections::BTreeSet<&String> = ticks.iter().flatten().collect();
        assert_eq!(
            covered.len(),
            universe.pool.len(),
            "a full sweep must cover every route-graph token exactly"
        );
        let lar_tick = ticks
            .iter()
            .position(|w| w.iter().any(|s| s == "LAR"))
            .expect("LAR must be covered by the sweep");
        assert!(
            lar_tick < universe.pool.len() / window + 1,
            "LAR covered in tick {lar_tick}, sweep bound = ceil(479/{window})"
        );
        assert!(cut != "LAR");
        assert!(
            covered.contains(cut),
            "the token the old cap cut is covered too"
        );
        // Determinism holds across sweeps: the same starting cursor replays the
        // identical tick sequence.
        assert_eq!(ticks, sweep_windows(&universe, window));
    }

    #[test]
    fn pc2_unresolved_pool_symbol_is_never_fabricated() {
        // A pool-resident symbol with no `arbx:tokens` identity has no address
        // to ask about: it must stay out of the universe entirely, and the
        // sweep must never invent it.
        let mut map = production_shaped_meta();
        map.remove("LAR");
        let mut source = production_shaped_pool_source();
        source.push("GHOST".to_string());
        let universe = build_universe(&allowlist_21(), &map, &sorted_unique_symbols(source));
        assert!(
            !universe.pool.iter().any(|t| t.symbol == "LAR"),
            "identity-less token is dropped, not guessed"
        );
        assert!(!universe.pool.iter().any(|t| t.symbol == "GHOST"));
        assert_eq!(universe.pool.len(), 478);
        let covered: std::collections::BTreeSet<String> = sweep_windows(&universe, 100)
            .into_iter()
            .flatten()
            .collect();
        assert_eq!(covered.len(), 478);
        assert!(!covered.contains("LAR"));
        assert!(!covered.contains("GHOST"));

        // Every emitted TokenRef carries a real 0x address — nothing is
        // synthesised from a symbol.
        for t in universe.pool.iter().chain(universe.allowlist.iter()) {
            assert!(t.address_lower.starts_with("0x"), "{:?}", t);
        }
    }

    #[tokio::test]
    async fn pc2_coingecko_ignores_addresses_it_did_not_ask_for() {
        // Provider answers for an address outside the request (or answers
        // nothing at all): no entry may appear — honest absence, never a
        // fabricated price.
        let server = MockServer::start().await;
        let body = serde_json::json!({
            "0x6226caa1857afbc6dfb6ca66071eb241228031a1": {"usd": 6.969e-05},
            "0xdeadbeef00000000000000000000000000000000": {"usd": 123456.0},
        });
        Mock::given(method("GET"))
            .and(path("/"))
            .respond_with(ResponseTemplate::new(200).set_body_json(body))
            .mount(&server)
            .await;
        let worker = worker_pointing_at(None, Some(&format!("{}/", server.uri())));

        let asked = vec![token_ref("LAR", LAR_ADDR)];
        let parsed = worker.fetch_coingecko(&asked).await.expect("ok");
        assert_eq!(parsed.len(), 1, "only the requested address is priced");
        assert_eq!(parsed.get("LAR"), Some(&6.969e-05));

        // A provider that returns nothing for the token that has no source
        // leaves the price map untouched (R8).
        let empty_server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({})))
            .mount(&empty_server)
            .await;
        let silent = worker_pointing_at(None, Some(&format!("{}/", empty_server.uri())));
        let mut prices: HashMap<String, f64> = HashMap::new();
        let hits = silent.coingecko_batch_into(&asked, &mut prices).await;
        assert_eq!(hits, 0);
        assert!(
            !prices.contains_key("LAR"),
            "no price may be invented for a token with no source"
        );
    }

    #[tokio::test]
    async fn pc2_coingecko_budget_chunks_the_route_graph_not_the_symbol_snapshot() {
        // The by-address pass must ask in MAX_COINGECKO_BATCH_SIZE chunks (25),
        // not in Alchemy's 5-address chunks (invariant asserted at compile time
        // next to the constants), and must never exceed the shared per-tick call
        // budget.
        let universe = production_shaped_universe();
        let window = (MAX_COINGECKO_CALLS_PER_TICK - 1) * MAX_COINGECKO_BATCH_SIZE;
        let w = rotating_window(&universe.pool, 0, window);
        assert_eq!(w.len(), window);
        let chunks: Vec<&[TokenRef]> = w.chunks(MAX_COINGECKO_BATCH_SIZE).collect();
        assert_eq!(
            chunks.len(),
            MAX_COINGECKO_CALLS_PER_TICK - 1,
            "the window must decompose into at most the remaining call budget"
        );
        assert!(chunks.iter().all(|c| c.len() <= MAX_COINGECKO_BATCH_SIZE));
    }

    #[test]
    fn pc2_route_graph_sweep_is_never_starved_by_the_allowlist_pass() {
        // The production shape is 295 unpriced snapshot tokens OUT of a
        // 300-token snapshot. Feeding those to the allowlist pass would consume
        // all 5 shared calls (ceil(295/25) = 12 chunks, budget-capped at 5) and
        // leave ZERO for the route graph — the sweep would never advance, which
        // is the same "LAR never gets priced" outcome this PR removes. The
        // reserves one call: the allowlist pass draws only 4 of 5 when there is
        // a route graph, and the whole 5 when there is nothing to sweep.
        assert_eq!(allowlist_cg_call_budget(0), MAX_COINGECKO_CALLS_PER_TICK);
        assert_eq!(
            allowlist_cg_call_budget(479),
            MAX_COINGECKO_CALLS_PER_TICK - 1
        );
        let allow_cap = allowlist_cg_call_budget(479);
        assert!(
            MAX_COINGECKO_CALLS_PER_TICK - allow_cap >= 1,
            ">=1 sweep call"
        );

        // Worst case: the allowlist leftovers need more calls than the cap, so
        // the sweep still gets its reserved call.
        let leftovers: Vec<TokenRef> = (0..295)
            .map(|i| token_ref(&format!("A{i:04}"), &shaped_addr(i)))
            .collect();
        let allow_calls = leftovers
            .chunks(MAX_COINGECKO_BATCH_SIZE)
            .count()
            .min(allow_cap);
        assert_eq!(allow_calls, allow_cap);
        let sweep_calls = MAX_COINGECKO_CALLS_PER_TICK - allow_calls;
        assert_eq!(sweep_calls, 1);
        let universe = production_shaped_universe();
        let swept = rotating_window(&universe.pool, 0, sweep_calls * MAX_COINGECKO_BATCH_SIZE);
        assert_eq!(swept.len(), MAX_COINGECKO_BATCH_SIZE);
        assert!(
            !swept.is_empty(),
            "the route graph must advance every tick, whatever the allowlist needs"
        );
    }

    #[test]
    fn pc3_allowlist_pass_never_re_asks_for_an_already_published_price() {
        // PRICE-COVERAGE-01. Medido en producción (chain 1, 24 ticks): el pase de
        // allowlist pedía 16 símbolos a Coingecko — COMP, MKR, UNI, SAND, ENS,
        // SHIB, RETH, APE, MATIC, SUSHI, LINK, LDO, MANA, PEPE, AAVE, CRV — y los
        // 16/16 YA estaban publicados en `arbx:token_prices:1`. Esas peticiones
        // redundantes devolvían 429, abrían el breaker del proveedor y dejaban el
        // sweep por address en `pool_cg_attempted = 0` en 24/24 ticks; ese sweep
        // es el ÚNICO productor posible de los tokens long-tail ruteados.
        //
        // Este test fija las DOS direcciones: sin evidencia de precio publicado el
        // símbolo SÍ se pide (nada se asume), y con el precio publicado NO se
        // pide (el presupuesto queda para el sweep).
        let universe = production_shaped_universe();
        let allow: Vec<TokenRef> = universe.allowlist.clone();
        assert_eq!(allow.len(), 21, "allowlist de producción");

        // (a) Nada publicado ni resuelto en el tick: se piden TODOS (el
        //     comportamiento previo no se toca — sin evidencia no se asume nada).
        let nothing = std::collections::HashSet::new();
        let all = allowlist_remaining(&allow, &HashMap::new(), &nothing);
        assert_eq!(
            all.len(),
            allow.len(),
            "sin precios publicados no se omite nada"
        );

        // (b) El caso medido: los 16 leftovers ya publicados ⇒ el pase 1 queda
        //     vacío y NO gasta NINGUNA de las 5 llamadas compartidas.
        let published: std::collections::HashSet<String> = [
            "COMP", "MKR", "UNI", "SAND", "ENS", "SHIB", "RETH", "APE", "MATIC", "SUSHI", "LINK",
            "LDO", "MANA", "PEPE", "AAVE", "CRV",
        ]
        .iter()
        .map(|s| (*s).to_string())
        .collect();
        let remaining = allowlist_remaining(&allow, &HashMap::new(), &published);
        let remaining_syms: Vec<&str> = remaining.iter().map(|t| t.symbol.as_str()).collect();
        assert_eq!(
            remaining_syms,
            vec!["WETH", "USDC", "USDT", "DAI", "WBTC"],
            "sólo los 5 majors sin precio publicado siguen pidiéndose"
        );
        let allow_calls = remaining
            .chunks(MAX_COINGECKO_BATCH_SIZE)
            .count()
            .min(allowlist_cg_call_budget(universe.pool.len()));
        assert_eq!(allow_calls, 1, "un solo chunk para los 5 restantes");
        let sweep_calls = MAX_COINGECKO_CALLS_PER_TICK - allow_calls;
        assert_eq!(
            sweep_calls,
            MAX_COINGECKO_CALLS_PER_TICK - 1,
            "el sweep recupera 4 de las 5 llamadas (antes: 0 intentos por el breaker abierto)"
        );
        let swept = rotating_window(&universe.pool, 0, sweep_calls * MAX_COINGECKO_BATCH_SIZE);
        assert_eq!(swept.len(), sweep_calls * MAX_COINGECKO_BATCH_SIZE);

        // (c) Un símbolo ya resuelto POR EL TICK (Chainlink/Binance/Alchemy) no se
        //     vuelve a pedir aunque todavía no esté publicado.
        let mut tick = HashMap::new();
        tick.insert("WETH".to_string(), 2_700.0);
        let after_fusion = allowlist_remaining(&allow, &tick, &published);
        assert!(
            !after_fusion.iter().any(|t| t.symbol == "WETH"),
            "un precio ya resuelto en el tick no se re-pide"
        );

        // (d) Con TODA la allowlist publicada, el pase 1 usa 0 llamadas y el sweep
        //     dispone del presupuesto completo.
        let full: std::collections::HashSet<String> =
            allow.iter().map(|t| t.symbol.clone()).collect();
        let none_left = allowlist_remaining(&allow, &HashMap::new(), &full);
        assert!(
            none_left.is_empty(),
            "allowlist totalmente publicada ⇒ cero peticiones: {none_left:?}"
        );
        let swept_full = rotating_window(
            &universe.pool,
            0,
            MAX_COINGECKO_CALLS_PER_TICK * MAX_COINGECKO_BATCH_SIZE,
        );
        assert_eq!(
            swept_full.len(),
            MAX_COINGECKO_CALLS_PER_TICK * MAX_COINGECKO_BATCH_SIZE,
            "el sweep usa las 5 llamadas para los tokens sin otro productor"
        );
    }
}
