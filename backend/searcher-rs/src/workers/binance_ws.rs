//! Binance combined-streams WebSocket client — PRECIO-CANONICO-CEX WO-PC2.
//!
//! ONE connection to `wss://stream.binance.com:9443/stream?streams=…`
//! carrying, per canonical pair:
//!   - `<pair>@bookTicker` — the canonical CEX price feed (always on; this IS
//!     the price source, not an optional extra);
//!   - `<pair>@depth5@100ms` — Fase-2 top-5 book for liquidity-aware sizing
//!     (WO-PC5). ACTIVO desde arranque por orden del operador 2026-09-20
//!     ("no te he dicho que la pongas en off"), con toggle runtime
//!     `arbx:controlboard:binance_depth5` (default ON, fail-safe ON) so the
//!     operator can switch the intake off from the frontend without a
//!     redeploy. The toggle gates only WHETHER depth frames reach the bus —
//!     toggling never tears the connection down (no reconnect churn).
//!
//! Resilience: exponential backoff 1s→60s cap (+jitter) on drops; proactive
//! reconnect at 23h (Binance rotates connections daily). Coste $0: public
//! stream endpoint, no key, no account.
//!
//! R9 logging: per-frame `debug!` only; one `info!` summary per minute with
//! counters. Fail-honest R8: connection errors are surfaced, never swallowed
//! into fake prices (the bus simply goes StaleBinance → anchor fallback).

use crate::runtime_knobs::RuntimeToggleClient;
use futures_util::{SinkExt, StreamExt};
use rand::Rng;
use shared_rs::price_bus::{BookTicker, Depth5, DepthLevel, PriceBus};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio_tungstenite::tungstenite::Message;
use tracing::{debug, info, warn};

/// Control-board module id for the depth5 intake toggle (WO-PC6, Rust side).
pub const DEPTH5_MODULE_ID: &str = "binance_depth5";

/// Canonical pair set (validated live 2026-09-20 — all have liquid spot
/// books; DAI has none and stays anchor-only in the bus).
pub const DEFAULT_PAIRS: &str = "ethusdc,btcusdt,ethusdt,usdcusdt,wbtcusdt";

const WS_BASE: &str = "wss://stream.binance.com:9443/stream?streams=";
const BACKOFF_START: Duration = Duration::from_secs(1);
const BACKOFF_CAP: Duration = Duration::from_secs(60);
/// Proactive reconnect horizon (Binance rotates server-side daily).
const RECONNECT_AFTER: Duration = Duration::from_secs(23 * 3600);
const SUMMARY_EVERY: Duration = Duration::from_secs(60);
const HEARTBEAT_EVERY: Duration = Duration::from_secs(10);
/// No-message watchdog: bookTicker + Binance pings are continuous, so silence
/// longer than this means a dead/half-open TCP — reconnect instead of waiting
/// for the 23h horizon with a blind writer (WO-V1 MAJOR).
const IDLE_RECONNECT: Duration = Duration::from_secs(30);

/// Pairs from `ARBX_BINANCE_PAIRS` (comma list, lowercase-insensitive),
/// defaulting to the canonical set. Empty/garbage entries drop honestly.
pub fn pairs_from_env() -> Vec<String> {
    let raw = std::env::var("ARBX_BINANCE_PAIRS").unwrap_or_else(|_| DEFAULT_PAIRS.to_string());
    let pairs: Vec<String> = raw
        .split(',')
        .map(|p| p.trim().to_ascii_lowercase())
        .filter(|p| !p.is_empty() && p.len() <= 20 && p.bytes().all(|b| b.is_ascii_alphanumeric()))
        .collect();
    if pairs.is_empty() {
        DEFAULT_PAIRS
            .split(',')
            .map(|p| p.to_string())
            .collect()
    } else {
        pairs
    }
}

/// Combined-streams URL. Depth streams are ALWAYS subscribed (the runtime
/// toggle gates frame application, not the subscription — zero reconnect
/// churn on toggle flips; frames are ~10/s per pair, trivially cheap).
pub fn build_stream_url(pairs: &[String]) -> String {
    let mut streams = Vec::with_capacity(pairs.len() * 2);
    for p in pairs {
        streams.push(format!("{p}@bookTicker"));
        streams.push(format!("{p}@depth5@100ms"));
    }
    format!("{WS_BASE}{}", streams.join("/"))
}

/// Parse a combined-stream frame into `(PAIR_UPPER, BookTicker)`.
/// Payload: `{"u":..,"s":"ETHUSDC","b":"2625.46","B":"..","a":"2625.47","A":".."}`
pub fn parse_book_ticker(data: &serde_json::Value) -> Option<(String, BookTicker)> {
    let sym = data.get("s")?.as_str()?;
    let bid: f64 = data.get("b")?.as_str()?.parse().ok()?;
    let ask: f64 = data.get("a")?.as_str()?.parse().ok()?;
    let event_ms = data.get("E").and_then(|v| v.as_u64()).unwrap_or(0);
    Some((
        sym.to_ascii_uppercase(),
        BookTicker {
            bid,
            ask,
            event_ms,
            recv_ns: now_ns(),
        },
    ))
}

/// Parse a depth5 partial-book frame into `(PAIR_UPPER, Depth5)`.
/// Real Binance payload carries NO symbol field:
/// `{"lastUpdateId":..,"bids":[["p","q"],..],"asks":[[..]]}` — the pair lives
/// only in the combined-stream name, so it is derived from `stream`'s prefix
/// before the first `@` (e.g. `btcusdt@depth5@100ms` → `BTCUSDT`).
pub fn parse_depth5(stream: &str, data: &serde_json::Value) -> Option<(String, Depth5)> {
    let sym = stream.split('@').next()?;
    if sym.is_empty() {
        return None;
    }
    let parse_levels = |key: &str| -> Option<Vec<DepthLevel>> {
        let arr = data.get(key)?.as_array()?;
        let mut out = Vec::with_capacity(arr.len().min(5));
        for lv in arr.iter().take(5) {
            let l = lv.as_array()?;
            let price: f64 = l.first()?.as_str()?.parse().ok()?;
            let qty: f64 = l.get(1)?.as_str()?.parse().ok()?;
            out.push(DepthLevel { price, qty });
        }
        Some(out)
    };
    let bids = parse_levels("bids")?;
    let asks = parse_levels("asks")?;
    Some((
        sym.to_ascii_uppercase(),
        Depth5 {
            bids,
            asks,
            event_ms: data.get("E").and_then(|v| v.as_u64()).unwrap_or(0),
        },
    ))
}

fn now_ns() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(0)
}

#[derive(Default, Debug)]
struct SessionStats {
    book_tickers: u64,
    depth5_frames: u64,
    depth5_gated: u64,
    parse_failures: u64,
    reconnects: u64,
}

/// Run the client forever (spawned from main.rs). Never returns; connection
/// errors loop through backoff. `redis` is the already-connected manager
/// used ONLY for the read-only toggle + heartbeat (INV-CB02-4).
pub async fn run(bus: Arc<PriceBus>, redis: redis::aio::ConnectionManager) {
    let pairs = pairs_from_env();
    let url = build_stream_url(&pairs);
    let toggle = RuntimeToggleClient::from_manager(redis.clone(), DEPTH5_MODULE_ID, true);

    info!(
        event = "binance_ws.start",
        url = %url,
        depth5_default_on = true,
        "Binance combined-streams client starting (canon price + depth5@100ms)"
    );

    let mut backoff = BACKOFF_START;
    let mut stats = SessionStats::default();
    loop {
        let session_started = Instant::now();
        let session = connect_and_stream(&url, &bus, &toggle, &redis, &mut stats).await;
        stats.reconnects += 1;
        match session {
            SessionEnd::NaturalReconnect => {
                info!(event = "binance_ws.reconnect_scheduled", "23h horizon reached — proactive reconnect");
                backoff = BACKOFF_START; // healthy connection, no penalty
            }
            SessionEnd::Error(e) => {
                // A session that stayed healthy for a while before dying
                // (e.g. server-side daily rotation closed with an error frame)
                // is not a pathological peer — drop the accumulated penalty.
                if session_started.elapsed() >= Duration::from_secs(60) {
                    backoff = BACKOFF_START;
                }
                warn!(event = "binance_ws.session_error", error = %e, "stream dropped — backing off");
                sleep_with_jitter(backoff).await;
                backoff = std::cmp::min(backoff * 2, BACKOFF_CAP);
            }
        }
    }
}

enum SessionEnd {
    NaturalReconnect,
    Error(anyhow::Error),
}

async fn connect_and_stream(
    url: &str,
    bus: &Arc<PriceBus>,
    toggle: &RuntimeToggleClient,
    redis: &redis::aio::ConnectionManager,
    stats: &mut SessionStats,
) -> SessionEnd {
    let (ws, _resp) = match tokio_tungstenite::connect_async(url).await {
        Ok(v) => v,
        Err(e) => return SessionEnd::Error(anyhow::anyhow!("connect: {e}")),
    };
    let (mut write, mut read) = ws.split();
    info!(event = "binance_ws.connected", url = %url);

    let started = Instant::now();
    let mut last_summary = Instant::now();
    let mut last_heartbeat = Instant::now() - HEARTBEAT_EVERY; // emit soon after connect
    let mut last_msg = Instant::now();

    loop {
        // Proactive reconnect at the 23h horizon.
        if started.elapsed() >= RECONNECT_AFTER {
            let _ = write.send(Message::Close(None)).await;
            return SessionEnd::NaturalReconnect;
        }
        let msg = tokio::select! {
            m = read.next() => match m {
                Some(Ok(m)) => m,
                Some(Err(e)) => return SessionEnd::Error(anyhow::anyhow!("read: {e}")),
                None => return SessionEnd::Error(anyhow::anyhow!("stream closed by peer")),
            },
            _ = tokio::time::sleep(Duration::from_secs(1)) => {
                // No-message watchdog: bookTicker + Binance pings are continuous,
                // so this branch firing with `last_msg` that old means the TCP
                // path is dead/half-open — reconnect instead of going blind.
                if last_msg.elapsed() >= IDLE_RECONNECT {
                    return SessionEnd::Error(anyhow::anyhow!(
                        "idle watchdog: no message in {}s",
                        last_msg.elapsed().as_secs()
                    ));
                }
                // Periodic duties piggyback on the 1s tick.
                if last_heartbeat.elapsed() >= HEARTBEAT_EVERY {
                    last_heartbeat = Instant::now();
                    let running = toggle.is_on().await;
                    let mut conn = redis.clone();
                    let _ = toggle
                        .emit_heartbeat(&mut conn, running, stats.book_tickers)
                        .await;
                }
                if last_summary.elapsed() >= SUMMARY_EVERY {
                    last_summary = Instant::now();
                    info!(
                        event = "binance_ws.summary",
                        book_tickers = stats.book_tickers,
                        depth5_frames = stats.depth5_frames,
                        depth5_gated = stats.depth5_gated,
                        parse_failures = stats.parse_failures,
                        reconnects = stats.reconnects,
                        uptime_secs = started.elapsed().as_secs(),
                        bus_book_tickers = bus.updates_binance.load(std::sync::atomic::Ordering::Relaxed),
                        bus_depth_books = bus.updates_depth.load(std::sync::atomic::Ordering::Relaxed),
                        bus_freezes = bus.freezes_total.load(std::sync::atomic::Ordering::Relaxed),
                        "R9 summary"
                    );
                }
                continue;
            }
        };
        last_msg = Instant::now();

        let text = match msg {
            Message::Text(t) => t,
            Message::Ping(p) => {
                if let Err(e) = write.send(Message::Pong(p)).await {
                    return SessionEnd::Error(anyhow::anyhow!("pong: {e}"));
                }
                continue;
            }
            Message::Close(c) => {
                return SessionEnd::Error(anyhow::anyhow!("close frame: {c:?}"));
            }
            _ => continue,
        };

        // Combined wrapper: {"stream":"ethusdc@bookTicker","data":{…}}.
        // Borrowed (no clone) — this runs for every frame (~50-100/s).
        let parsed: Option<serde_json::Value> = serde_json::from_str(&text).ok();
        let (stream, data) = match parsed.as_ref().and_then(|v| {
            let s = v.get("stream")?.as_str()?;
            let d = v.get("data")?;
            Some((s, d))
        }) {
            Some(v) => v,
            None => {
                stats.parse_failures += 1;
                debug!(event = "binance_ws.frame_unparseable", "dropping frame (R8)");
                continue;
            }
        };

        if stream.ends_with("@bookTicker") {
            match parse_book_ticker(data) {
                Some((pair, t)) => {
                    stats.book_tickers += 1;
                    debug!(event = "binance_ws.book_ticker", pair = %pair, bid = t.bid, ask = t.ask);
                    bus.update_binance(&pair, t);
                }
                None => stats.parse_failures += 1,
            }
        } else if stream.ends_with("@depth5@100ms") {
            // Operator toggle (WO-PC6): gates only application of frames.
            if !toggle.is_on().await {
                stats.depth5_gated += 1;
                continue;
            }
            match parse_depth5(stream, data) {
                Some((pair, d)) => {
                    stats.depth5_frames += 1;
                    debug!(event = "binance_ws.depth5", pair = %pair, bids = d.bids.len(), asks = d.asks.len());
                    bus.update_depth(&pair, d);
                }
                None => stats.parse_failures += 1,
            }
        }
    }
}

async fn sleep_with_jitter(base: Duration) {
    let jitter_ms = rand::thread_rng().gen_range(0..500);
    tokio::time::sleep(base + Duration::from_millis(jitter_ms)).await;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pairs_from_env_shape_defaults() {
        // No env set → canonical five (test hermeticity: clear if present).
        std::env::remove_var("ARBX_BINANCE_PAIRS");
        let pairs = pairs_from_env();
        assert_eq!(pairs, vec!["ethusdc", "btcusdt", "ethusdt", "usdcusdt", "wbtcusdt"]);
    }

    #[test]
    fn pairs_env_filters_garbage() {
        std::env::set_var("ARBX_BINANCE_PAIRS", " ETHusdc ,,x!,btcusdt");
        let pairs = pairs_from_env();
        assert_eq!(pairs, vec!["ethusdc", "btcusdt"]);
        std::env::remove_var("ARBX_BINANCE_PAIRS");
    }

    #[test]
    fn stream_url_contains_all_streams() {
        let url = build_stream_url(&["ethusdc".to_string()]);
        assert!(url.starts_with("wss://stream.binance.com:9443/stream?streams="));
        assert!(url.ends_with("ethusdc@bookTicker/ethusdc@depth5@100ms"));
    }

    #[test]
    fn parses_book_ticker_frame() {
        // Real Binance bookTicker payload: NO "E" field (staleness uses recv_ns).
        let data = serde_json::json!({
            "u": 400900217, "s": "ETHUSDC",
            "b": "2625.46", "B": "12.481", "a": "2625.47", "A": "3.200",
        });
        let (pair, t) = parse_book_ticker(&data).unwrap();
        assert_eq!(pair, "ETHUSDC");
        assert!((t.bid - 2625.46).abs() < 1e-9);
        assert!((t.ask - 2625.47).abs() < 1e-9);
        assert_eq!(t.event_ms, 0);
        assert!(t.recv_ns > 0);
    }

    #[test]
    fn book_ticker_missing_fields_is_none() {
        assert!(parse_book_ticker(&serde_json::json!({"s": "ETHUSDC"})).is_none());
        assert!(parse_book_ticker(&serde_json::json!({})).is_none());
    }

    #[test]
    fn parses_depth5_frame_and_caps_five_levels() {
        // Real Binance partial-depth payload: NO "s" field — the pair comes
        // from the stream name only (WO-V1 CRITICAL regression guard).
        let lv = |p: &str| serde_json::json!([p, "1.5"]);
        let data = serde_json::json!({
            "lastUpdateId": 42,
            "bids": [lv("80000.1"), lv("80000.0"), lv("79999.9"), lv("79999.8"), lv("79999.7"), lv("79999.6")],
            "asks": [lv("80000.2"), lv("80000.3")],
        });
        let (pair, d) = parse_depth5("btcusdt@depth5@100ms", &data).unwrap();
        assert_eq!(pair, "BTCUSDT");
        assert_eq!(d.bids.len(), 5, "6 levels capped to 5");
        assert_eq!(d.asks.len(), 2);
        assert!((d.bids[0].price - 80000.1).abs() < 1e-9);
        assert!((d.asks[1].qty - 1.5).abs() < 1e-9);
        assert_eq!(d.event_ms, 0);
    }

    #[test]
    fn depth5_missing_side_is_none() {
        let data = serde_json::json!({"bids": [["1", "1"]], "asks": "junk"});
        assert!(parse_depth5("btcusdt@depth5@100ms", &data).is_none());
    }

    #[test]
    fn depth5_stream_without_pair_prefix_is_none() {
        let data = serde_json::json!({"bids": [["1", "1"]], "asks": [["2", "1"]]});
        assert!(parse_depth5("@depth5@100ms", &data).is_none());
    }

    #[tokio::test]
    async fn parsed_frames_flow_into_bus() {
        let bus = shared_rs::price_bus::PriceBus::new(Default::default());
        let (pair, t) = parse_book_ticker(&serde_json::json!({
            "s": "ETHUSDC", "b": "2625.46", "a": "2625.47"
        }))
        .unwrap();
        bus.update_binance(&pair, t);
        assert!(bus.view().snapshot().binance.contains_key("ETHUSDC"));

        let (pair, d) = parse_depth5(
            "ethusdc@depth5@100ms",
            &serde_json::json!({"bids": [["2625.0", "2.0"]], "asks": [["2626.0", "2.0"]]}),
        )
        .unwrap();
        bus.update_depth(&pair, d);
        assert!(bus.view().snapshot().depth.contains_key("ETHUSDC"));
    }
}
