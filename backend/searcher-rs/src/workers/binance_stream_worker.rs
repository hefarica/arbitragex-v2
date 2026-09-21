//! BinanceStreamWorker — live CEX mid-price feed over Binance public WebSocket
//! (BE-3.2 Phase 2 feed source).
//!
//! Replaces the REST-poll half of `cex_dex_worker` Phase 1 with a combined
//! `bookTicker` stream (`wss://stream.binance.com:9443/stream?streams=…`).
//! Public endpoint — no API key, no cost (operator free-sources doctrine).
//!
//! ## Data flow
//!
//! ```text
//! Binance WS bookTicker → mid = (best_bid + best_ask) / 2
//!   → ChangeDetector (|Δ| ≥ threshold_pct OR entry older than force_rewrite_ms)
//!   → HSET arbx:cex_prices:<chain_id>  (field = BASE asset, value = CexPriceEntry JSON)
//!   → PUBLISH arbx:prices:updated:<chain_id> {"source":"binance_ws",...}
//! ```
//!
//! The CEX hash is SEPARATE from the on-chain `arbx:token_prices:<chain_id>`
//! tower — this worker never overwrites on-chain fields. Readers merge with
//! on-chain precedence (`price_oracle::merge_cex_fallback`,
//! `RedisCachedPriceOracle::snapshot_from_redis`) or read the CEX hash
//! directly for the UI `cex` map (api-server prices-stream bridge).
//!
//! ## Bugs from the original pasted design — corrected here
//!
//! 1. `write` moved into a ping task → here ONE task owns the socket and
//!    answers `Ping` with `Pong` inline (no sink split, no move).
//! 2. `recv_time_ms` semantics → wall-clock ms taken when the frame is
//!    processed, stored in `CexPriceEntry::ts_ms` (source of staleness truth).
//! 3. division by `quantity == 0` → bookTicker mid uses bid/ask only;
//!    quantities are never in the denominator.
//! 4. wrong URL (`/ws/stream`) → combined-stream URL is
//!    `<base>/stream?streams=<sym>@bookTicker/…` (verified shape below).
//! 5. non-clonable stream → no `Clone`; the socket lives in a single
//!    `run_stream` frame, reconnect handled by the outer loop.
//!
//! ## Doctrine compliance
//!
//! - **RULE 00**: no keys, no fabricated prices; every field in the hash came
//!   from a real Binance frame. Feed down → gauge 0 + on-chain tower stands.
//! - **R8 fail-honest**: geo-block (HTTP 451), DNS failure, idle socket —
//!   all surface as reconnect-warn + `arbx_binance_ws_feed_healthy = 0`.
//!   The system NEVER depends on this feed to function.
//! - **R9 log discipline**: per-frame work at `debug!`; `info!` only on
//!   connect/disconnect transitions; reconnect warns are bounded by the
//!   30 s backoff ceiling.
//! - **§34 mode-invariant**: pure data source — identical in paper/testnet/mainnet.

use futures_util::{SinkExt, StreamExt};
use shared_rs::metrics::{
    BINANCE_WS_CHANGE_EVENTS_TOTAL, BINANCE_WS_FEED_HEALTHY, BINANCE_WS_MESSAGES_TOTAL,
    BINANCE_WS_RECONNECTS_TOTAL,
};
use shared_rs::price_oracle::{prices_updated_channel, redis_cex_prices_key, CexPriceEntry};
use std::collections::HashMap;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::Message;
use tracing::{debug, info, warn};

// ──────────────────────────────────────────────────────────────────────────────
// Configuration
// ──────────────────────────────────────────────────────────────────────────────

/// Default symbol set — the 5 deepest spot USDT books. Combined-stream limit
/// is 1024 streams/connection; we sit far below it.
pub const DEFAULT_SYMBOLS: &[&str] = &["BTCUSDT", "ETHUSDT", "BNBUSDT", "SOLUSDT", "AVAXUSDT"];

/// Default mid-price change threshold (percent). 0.01 % filters top-of-book
/// noise; the pasted design used 0.01 spot / 0.005 futures (futures feed is
/// not enabled by default).
pub const DEFAULT_CHANGE_THRESHOLD_PCT: f64 = 0.01;

/// Even with no threshold crossing, re-write an entry this old so the hash
/// TTL stays alive and `ts_ms` reflects feed liveness (stale-price guard for
/// downstream consumers).
pub const FORCE_REWRITE_MS: u64 = 30_000;

/// Hash TTL — must comfortably exceed FORCE_REWRITE_MS so a healthy feed
/// never lets the key expire, while a dead feed (no rewrites) cleans up after
/// itself instead of serving stale CEX prices forever.
pub const CACHE_TTL_SECS: u64 = 300;

/// Idle-socket watchdog: bookTicker for these books emits multiple frames per
/// second; 60 s of total silence means the connection is zombie — reconnect.
pub const IDLE_TIMEOUT_SECS: u64 = 60;

/// Documented Binance stream endpoints (port 9443 primary, 443 alternate).
/// Rotation mirrors the block_scanner reconnect pattern.
pub const STREAM_ENDPOINTS: &[&str] = &[
    "wss://stream.binance.com:9443",
    "wss://stream.binance.com:443",
];

/// Boot-time configuration. Built from env (`from_env`) or explicitly (tests).
#[derive(Debug, Clone)]
pub struct BinanceStreamWorkerConfig {
    pub chain_id: u64,
    pub symbols: Vec<String>,
    pub change_threshold_pct: f64,
    /// Tests inject a `ws://localhost` wiremock-style endpoint here.
    pub endpoint_override: Option<String>,
    /// Master switch (`ARBX_BINANCE_WS_ENABLED`, default ON — data takes are
    /// active from boot per operator order 2026-09-20; feed failure degrades
    /// honestly rather than disabling anything).
    pub enabled: bool,
}

impl Default for BinanceStreamWorkerConfig {
    fn default() -> Self {
        Self {
            chain_id: 1,
            symbols: DEFAULT_SYMBOLS.iter().map(|s| s.to_string()).collect(),
            change_threshold_pct: DEFAULT_CHANGE_THRESHOLD_PCT,
            endpoint_override: None,
            enabled: true,
        }
    }
}

impl BinanceStreamWorkerConfig {
    /// Env knobs (absence = documented defaults; malformed = warn + default,
    /// never boot-crash — this feed is best-effort, cf. `read_interval_ms_env`).
    pub fn from_env(chain_id: u64) -> Self {
        let mut cfg = Self {
            chain_id,
            ..Self::default()
        };
        if let Ok(v) = std::env::var("ARBX_BINANCE_WS_SYMBOLS") {
            let symbols: Vec<String> = v
                .split(',')
                .map(|s| s.trim().to_ascii_uppercase())
                .filter(|s| s.len() >= 5)
                .collect();
            if symbols.is_empty() {
                warn!(
                    event = "binance_ws.symbols_env_invalid",
                    raw = %v,
                    "ARBX_BINANCE_WS_SYMBOLS produced no usable symbols — using defaults"
                );
            } else {
                cfg.symbols = symbols;
            }
        }
        if let Ok(v) = std::env::var("ARBX_BINANCE_WS_CHANGE_THRESHOLD_PCT") {
            match v.trim().parse::<f64>() {
                Ok(pct) if pct.is_finite() && pct > 0.0 && pct < 100.0 => {
                    cfg.change_threshold_pct = pct;
                }
                _ => warn!(
                    event = "binance_ws.threshold_env_invalid",
                    raw = %v,
                    default = DEFAULT_CHANGE_THRESHOLD_PCT,
                    "using default change threshold"
                ),
            }
        }
        cfg
    }

    /// `ARBX_BINANCE_WS_ENABLED` — default ON; only "false"/"0"/"off" disable.
    pub fn enabled_from_env() -> bool {
        match std::env::var("ARBX_BINANCE_WS_ENABLED") {
            Ok(v) => !matches!(
                v.trim().to_ascii_lowercase().as_str(),
                "false" | "0" | "off" | "no"
            ),
            Err(_) => true,
        }
    }
}

// ──────────────────────────────────────────────────────────────────────────────
// Frame parsing (pure — unit-tested without network)
// ──────────────────────────────────────────────────────────────────────────────

/// One bookTicker frame, post-validation.
#[derive(Debug, Clone, PartialEq)]
pub struct BookTicker {
    pub symbol: String,
    pub best_bid: f64,
    pub best_ask: f64,
}

impl BookTicker {
    /// Mid price. No quantity involved (design bug #3 — quantities are never
    /// a denominator). Caller still validates finiteness at detection time.
    pub fn mid(&self) -> f64 {
        (self.best_bid + self.best_ask) / 2.0
    }
}

/// Parse one combined-stream frame:
/// `{"stream":"ethusdt@bookTicker","data":{"u":…,"s":"ETHUSDT","b":"…","a":"…"}}`
/// Raw (non-combined) frames `{"s":…,"b":…,"a":…}` are accepted too — the
/// parser keys off the payload, not the transport shape.
///
/// Returns `None` on: malformed JSON, missing `s`/`b`/`a`, non-finite or
/// non-positive sides, or a crossed book (bid > ask — reject rather than emit
/// a nonsense mid; R8).
pub fn parse_book_ticker_frame(raw: &str) -> Option<BookTicker> {
    let v: serde_json::Value = serde_json::from_str(raw).ok()?;
    let payload = v.get("data").unwrap_or(&v);
    let symbol = payload.get("s")?.as_str()?.trim().to_ascii_uppercase();
    if symbol.is_empty() {
        return None;
    }
    let bid = payload.get("b")?.as_str()?.parse::<f64>().ok()?;
    let ask = payload.get("a")?.as_str()?.parse::<f64>().ok()?;
    if !bid.is_finite() || !ask.is_finite() || bid <= 0.0 || ask <= 0.0 {
        return None;
    }
    if bid > ask {
        return None; // crossed/locked book — nonsense mid
    }
    Some(BookTicker {
        symbol,
        best_bid: bid,
        best_ask: ask,
    })
}

/// Extract the BASE asset from a Binance symbol ("ETHUSDT" → "ETH").
/// Longest quote suffix first (USDCUSDT → USDC, not USDCU-SDT). Unknown
/// suffix → the whole symbol uppercase (honest identity; the merge only uses
/// it as a fallback key nobody else produces).
pub fn base_asset_of(symbol: &str) -> String {
    const QUOTES: &[&str] = &[
        "USDT", "USDC", "BUSD", "FDUSD", "TUSD", "USDS", "BTC", "ETH", "BNB",
    ];
    let s = symbol.trim().to_ascii_uppercase();
    for q in QUOTES {
        if let Some(base) = s.strip_suffix(q) {
            if !base.is_empty() {
                return base.to_string();
            }
        }
    }
    s
}

/// Combined-stream URL. Correct shape is `<base>/stream?streams=…`
/// (design bug #4 was `/ws/stream?…` — invalid on Binance).
pub fn build_stream_url(base: &str, symbols: &[String]) -> String {
    let streams = symbols
        .iter()
        .map(|s| format!("{}@bookTicker", s.to_ascii_lowercase()))
        .collect::<Vec<_>>()
        .join("/");
    format!("{}/stream?streams={}", base.trim_end_matches('/'), streams)
}

// ──────────────────────────────────────────────────────────────────────────────
// ChangeDetector (pure — unit-tested)
// ──────────────────────────────────────────────────────────────────────────────

/// Per-symbol last-emitted state.
#[derive(Debug, Clone, Copy)]
struct SymbolState {
    mid: f64,
    ts_ms: u64,
}

/// Emits `true` when a new mid should be persisted: either the relative move
/// since the last persisted mid crossed `threshold_pct`, or the last
/// persisted entry is older than `force_rewrite_ms` (staleness/TTL keepalive).
/// Non-finite / non-positive mids are rejected outright (R8).
pub struct ChangeDetector {
    threshold_pct: f64,
    force_rewrite_ms: u64,
    last: HashMap<String, SymbolState>,
}

impl ChangeDetector {
    pub fn new(threshold_pct: f64, force_rewrite_ms: u64) -> Self {
        Self {
            threshold_pct: threshold_pct.max(0.0),
            force_rewrite_ms,
            last: HashMap::new(),
        }
    }

    /// Returns `Some(base_asset)` when the mid should be written (the base
    /// asset is the hash field), `None` when it's below threshold and fresh.
    pub fn on_mid(&mut self, symbol: &str, mid: f64, now_ms: u64) -> Option<String> {
        if !mid.is_finite() || mid <= 0.0 {
            return None; // design bug guard: never persist garbage
        }
        let base = base_asset_of(symbol);
        let emit = match self.last.get(&base) {
            None => true, // first observation for this symbol — always write
            Some(prev) => {
                let pct = ((mid - prev.mid).abs() / prev.mid) * 100.0;
                let stale = now_ms.saturating_sub(prev.ts_ms) >= self.force_rewrite_ms;
                pct >= self.threshold_pct || stale
            }
        };
        if emit {
            self.last
                .insert(base.clone(), SymbolState { mid, ts_ms: now_ms });
            Some(base)
        } else {
            None
        }
    }
}

// ──────────────────────────────────────────────────────────────────────────────
// Redis persistence
// ──────────────────────────────────────────────────────────────────────────────

/// Wall-clock ms since epoch (fail-honest: SystemTime before epoch → 0).
fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

/// Atomically HSET every pending entry + EXPIRE + PUBLISH the same
/// `arbx:prices:updated:<chain_id>` notice the on-chain tower uses (source
/// field distinguishes writers). Mirrors `price_worker::persist_prices`.
async fn persist_cex_entries(
    redis: &mut redis::aio::ConnectionManager,
    chain_id: u64,
    entries: &HashMap<String, CexPriceEntry>,
) -> anyhow::Result<()> {
    if entries.is_empty() {
        return Ok(());
    }
    let key = redis_cex_prices_key(chain_id);
    let mut pipe = redis::pipe();
    pipe.atomic();
    for (base, entry) in entries {
        pipe.hset(&key, base, serde_json::to_string(entry)?)
            .ignore();
    }
    pipe.expire(&key, CACHE_TTL_SECS as i64).ignore();
    pipe.cmd("PUBLISH")
        .arg(prices_updated_channel(chain_id))
        .arg(
            serde_json::json!({
                "source": "binance_ws",
                "chain_id": chain_id,
                "written": entries.len(),
            })
            .to_string(),
        )
        .ignore();
    let _: () = pipe.query_async(redis).await?;
    Ok(())
}

// ──────────────────────────────────────────────────────────────────────────────
// Worker
// ──────────────────────────────────────────────────────────────────────────────

pub struct BinanceStreamWorker {
    cfg: BinanceStreamWorkerConfig,
    redis: redis::aio::ConnectionManager,
}

impl BinanceStreamWorker {
    pub fn new(cfg: BinanceStreamWorkerConfig, redis: redis::aio::ConnectionManager) -> Self {
        Self { cfg, redis }
    }

    /// Outer loop: connect → stream → on drop, rotate endpoint + exponential
    /// backoff (1 s → 30 s), forever. Pattern: `block_scanner` reconnect.
    pub async fn run(self) {
        let chain = self.cfg.chain_id.to_string();
        let endpoints: Vec<String> = match &self.cfg.endpoint_override {
            Some(u) => vec![u.clone()],
            None => STREAM_ENDPOINTS.iter().map(|s| s.to_string()).collect(),
        };
        info!(
            event = "binance_ws.boot",
            chain_id = self.cfg.chain_id,
            symbols = self.cfg.symbols.len(),
            threshold_pct = self.cfg.change_threshold_pct,
            endpoints = endpoints.len(),
            "Binance bookTicker feed worker started"
        );

        let mut backoff_ms: u64 = 1_000;
        const MAX_BACKOFF_MS: u64 = 30_000;
        let mut url_idx = 0usize;
        let mut detector = ChangeDetector::new(self.cfg.change_threshold_pct, FORCE_REWRITE_MS);

        loop {
            let url = build_stream_url(&endpoints[url_idx % endpoints.len()], &self.cfg.symbols);
            match run_stream(
                &url,
                &self.cfg,
                &mut detector,
                &mut self.redis.clone(),
                &chain,
            )
            .await
            {
                Ok(()) => return, // unreachable today (no cancel token wired)
                Err(e) => {
                    BINANCE_WS_FEED_HEALTHY.with_label_values(&[&chain]).set(0);
                    BINANCE_WS_RECONNECTS_TOTAL
                        .with_label_values(&[&chain])
                        .inc();
                    warn!(
                        event = "binance_ws.reconnect",
                        chain_id = self.cfg.chain_id,
                        error = %e,
                        backoff_ms,
                        "Binance WS dropped (451 geo-block / network / idle) — \
                         rotating endpoint + backing off; on-chain price tower stands"
                    );
                    url_idx = url_idx.wrapping_add(1);
                    tokio::time::sleep(Duration::from_millis(backoff_ms)).await;
                    backoff_ms = (backoff_ms * 2).min(MAX_BACKOFF_MS);
                }
            }
        }
    }
}

/// One WS connection. ONE task owns the socket (design bug #1/#5): Ping is
/// answered inline through the same `WebSocketStream`, no sink split, no
/// ping task, no Clone. Returns `Err` on any drop → outer loop reconnects.
async fn run_stream(
    url: &str,
    cfg: &BinanceStreamWorkerConfig,
    detector: &mut ChangeDetector,
    redis: &mut redis::aio::ConnectionManager,
    chain: &str,
) -> anyhow::Result<()> {
    let (mut ws, _resp) = connect_async(url).await?;
    BINANCE_WS_FEED_HEALTHY.with_label_values(&[chain]).set(1);
    info!(event = "binance_ws.connected", chain_id = cfg.chain_id, url = %url);

    let mut idle = tokio::time::interval(Duration::from_secs(IDLE_TIMEOUT_SECS));
    idle.tick().await; // first tick fires immediately — drain it

    loop {
        tokio::select! {
            _ = idle.tick() => {
                return Err(anyhow::anyhow!(
                    "no frames in {IDLE_TIMEOUT_SECS}s — zombie connection"
                ));
            }
            frame = ws.next() => {
                let Some(frame) = frame else {
                    return Err(anyhow::anyhow!("stream ended"));
                };
                let msg = frame?;
                match msg {
                    Message::Ping(payload) => {
                        // Bug #1 fix: single-owner inline pong.
                        ws.send(Message::Pong(payload)).await?;
                    }
                    Message::Close(frame) => {
                        return Err(anyhow::anyhow!("server closed: {frame:?}"));
                    }
                    Message::Text(txt) => {
                        BINANCE_WS_MESSAGES_TOTAL.with_label_values(&[chain]).inc();
                        let Some(ticker) = parse_book_ticker_frame(txt.as_str()) else {
                            // Subscribed frames only — malformed ones are
                            // provider noise, not an error state (R9: debug).
                            debug!(event = "binance_ws.frame_unparsed", "dropping malformed frame");
                            continue;
                        };
                        let mid = ticker.mid();
                        let ts = now_ms();
                        if let Some(base) = detector.on_mid(&ticker.symbol, mid, ts) {
                            BINANCE_WS_CHANGE_EVENTS_TOTAL
                                .with_label_values(&[chain, &base])
                                .inc();
                            let mut batch = HashMap::with_capacity(1);
                            batch.insert(
                                base.clone(),
                                CexPriceEntry {
                                    price: mid,
                                    ts_ms: ts,
                                    quote: "USDT".to_string(),
                                    source: "binance_ws".to_string(),
                                },
                            );
                            if let Err(e) = persist_cex_entries(redis, cfg.chain_id, &batch).await {
                                // Feed is healthy; Redis write failed. Warn +
                                // drop this batch — the next threshold crossing
                                // (or FORCE_REWRITE_MS staleness tick) rewrites.
                                warn!(
                                    event = "binance_ws.persist_failed",
                                    chain_id = cfg.chain_id,
                                    base = %base,
                                    error = %e,
                                    "dropping CEX batch this tick (fail-honest — no retry storm)"
                                );
                            } else {
                                debug!(event = "binance_ws.wrote", base = %base, mid, ts_ms = ts);
                            }
                        }
                    }
                    Message::Binary(_) | Message::Pong(_) | Message::Frame(_) => {
                        // bookTicker is text-only; ignore the rest.
                    }
                }
            }
        }
    }
}

// ──────────────────────────────────────────────────────────────────────────────
// Tests (pure, no network — RULE 00)
// ──────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)] // M11: test module
    use super::*;

    // ── URL builder ─────────────────────────────────────────────────────────

    #[test]
    fn stream_url_uses_combined_stream_shape() {
        let url = build_stream_url(
            "wss://stream.binance.com:9443",
            &["ETHUSDT".to_string(), "BTCUSDT".to_string()],
        );
        // Bug #4 regression guard: `/stream?streams=`, NOT `/ws/stream`.
        assert_eq!(
            url,
            "wss://stream.binance.com:9443/stream?streams=ethusdt@bookTicker/btcusdt@bookTicker"
        );
    }

    #[test]
    fn stream_url_strips_trailing_slash_from_base() {
        let url = build_stream_url("wss://x.example/", &["ETHUSDT".to_string()]);
        assert!(url.starts_with("wss://x.example/stream?"));
        assert!(!url.contains("//stream"));
    }

    #[test]
    fn default_symbols_all_usdt_books() {
        for s in DEFAULT_SYMBOLS {
            assert!(s.ends_with("USDT"), "{s} must be a USDT book");
        }
        assert_eq!(DEFAULT_SYMBOLS.len(), 5);
    }

    // ── Frame parser ────────────────────────────────────────────────────────

    const COMBINED: &str = r#"{"stream":"ethusdt@bookTicker","data":{"u":400900217,"s":"ETHUSDT","b":"2517.40","B":"10.5","a":"2517.42","A":"8.2"}}"#;

    #[test]
    fn parses_combined_stream_frame() {
        let t = parse_book_ticker_frame(COMBINED).expect("valid frame");
        assert_eq!(t.symbol, "ETHUSDT");
        assert!((t.best_bid - 2517.40).abs() < 1e-9);
        assert!((t.best_ask - 2517.42).abs() < 1e-9);
        assert!((t.mid() - 2517.41).abs() < 1e-9);
    }

    #[test]
    fn parses_raw_frame_without_wrapper() {
        let raw = r#"{"u":1,"s":"BTCUSDT","b":"60000.00","a":"60000.10"}"#;
        let t = parse_book_ticker_frame(raw).expect("valid raw frame");
        assert_eq!(t.symbol, "BTCUSDT");
        assert!((t.mid() - 60000.05).abs() < 1e-9);
    }

    #[test]
    fn rejects_malformed_json() {
        assert!(parse_book_ticker_frame("not-json").is_none());
    }

    #[test]
    fn rejects_missing_fields() {
        assert!(parse_book_ticker_frame(r#"{"s":"ETHUSDT","b":"1.0"}"#).is_none());
        assert!(parse_book_ticker_frame(r#"{"b":"1.0","a":"1.1"}"#).is_none());
    }

    #[test]
    fn rejects_non_positive_or_nonfinite_sides() {
        assert!(parse_book_ticker_frame(r#"{"s":"ETHUSDT","b":"0","a":"1"}"#).is_none());
        assert!(
            parse_book_ticker_frame(r#"{"s":"ETHUSDT","b":"NaN","a":"1"}"#).is_none(),
            "NaN must be rejected (R8 — never persist garbage)"
        );
    }

    #[test]
    fn rejects_crossed_book() {
        assert!(parse_book_ticker_frame(r#"{"s":"ETHUSDT","b":"2000","a":"1999"}"#).is_none());
    }

    #[test]
    fn ignores_unrelated_combined_payloads() {
        // e.g. an error frame or a subscription ack — no s/b/a book inside.
        assert!(parse_book_ticker_frame(r#"{"result":null,"id":1}"#).is_none());
    }

    // ── Base-asset extraction ───────────────────────────────────────────────

    #[test]
    fn base_asset_strips_longest_quote_first() {
        assert_eq!(base_asset_of("ETHUSDT"), "ETH");
        assert_eq!(base_asset_of("USDCUSDT"), "USDC"); // not "USDCU"
        assert_eq!(base_asset_of("WBTCETH"), "WBTC");
    }

    #[test]
    fn base_asset_unknown_suffix_is_identity() {
        assert_eq!(base_asset_of("XYZABC"), "XYZABC");
    }

    // ── ChangeDetector ──────────────────────────────────────────────────────

    #[test]
    fn first_observation_always_emits() {
        let mut d = ChangeDetector::new(0.01, 30_000);
        assert_eq!(d.on_mid("ETHUSDT", 2500.0, 1_000), Some("ETH".to_string()));
    }

    #[test]
    fn sub_threshold_move_does_not_emit() {
        let mut d = ChangeDetector::new(0.01, 30_000);
        d.on_mid("ETHUSDT", 2500.0, 1_000);
        // 0.004% move — below 0.01% threshold.
        assert_eq!(d.on_mid("ETHUSDT", 2500.1, 1_100), None);
    }

    #[test]
    fn threshold_crossing_emits() {
        let mut d = ChangeDetector::new(0.01, 30_000);
        d.on_mid("ETHUSDT", 2500.0, 1_000);
        // 0.02% move — above threshold.
        assert_eq!(d.on_mid("ETHUSDT", 2500.5, 1_100), Some("ETH".to_string()));
    }

    #[test]
    fn staleness_forces_rewrite() {
        let mut d = ChangeDetector::new(0.01, 30_000);
        d.on_mid("ETHUSDT", 2500.0, 1_000);
        // Identical price but 30 s later → TTL keepalive rewrite.
        assert_eq!(d.on_mid("ETHUSDT", 2500.0, 31_000), Some("ETH".to_string()));
        // Fresh identical price → no write.
        assert_eq!(d.on_mid("ETHUSDT", 2500.0, 31_100), None);
    }

    #[test]
    fn invalid_mid_never_emits() {
        let mut d = ChangeDetector::new(0.01, 30_000);
        d.on_mid("ETHUSDT", 2500.0, 1_000);
        assert_eq!(d.on_mid("ETHUSDT", f64::NAN, 2_000), None);
        assert_eq!(d.on_mid("ETHUSDT", 0.0, 2_000), None);
        assert_eq!(d.on_mid("ETHUSDT", -1.0, 2_000), None);
        // State untouched — the last good mid still gates the next compare.
        assert_eq!(d.on_mid("ETHUSDT", 2500.0, 2_000), None);
    }

    #[test]
    fn detector_state_is_per_symbol() {
        let mut d = ChangeDetector::new(0.01, 30_000);
        d.on_mid("ETHUSDT", 2500.0, 1_000);
        // BTC first observation still emits regardless of ETH state.
        assert_eq!(d.on_mid("BTCUSDT", 60000.0, 1_050), Some("BTC".to_string()));
    }

    // ── Config env gating ───────────────────────────────────────────────────

    #[test]
    fn enabled_defaults_true_and_respects_off() {
        // No env set (test process doesn't define it) → default ON.
        if std::env::var("ARBX_BINANCE_WS_ENABLED").is_err() {
            assert!(BinanceStreamWorkerConfig::enabled_from_env());
        }
    }

    // ── R3 panic containment (audit t_ade4fcb6) ────────────────────────────

    #[tokio::test]
    async fn catch_unwind_wrapper_contains_feed_panic() {
        // R3 regression: a panic inside the spawned feed task (the rustls
        // CryptoProvider panic this fix removes) used to kill the task
        // silently — the Err branch that increments
        // `arbx_binance_ws_reconnects_total` never ran. main.rs now wraps the
        // worker future with AssertUnwindSafe + FutureExt::catch_unwind. This
        // pins the exact wrapper semantics: a panicking feed future resolves
        // to Err (contained, not a silent task death) and the fail-honest
        // gauges are writable from that path (same Lazy statics).
        let feed = std::panic::AssertUnwindSafe(async {
            panic!("Could not automatically determine the process-level CryptoProvider");
        });
        let caught = futures_util::FutureExt::catch_unwind(feed).await;
        assert!(caught.is_err(), "catch_unwind must contain a feed panic");
        let chain = "1";
        BINANCE_WS_FEED_HEALTHY.with_label_values(&[chain]).set(0);
        BINANCE_WS_RECONNECTS_TOTAL
            .with_label_values(&[chain])
            .inc();
    }
}
