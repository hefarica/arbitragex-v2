//! CexDexWorker — scaffold for CEX-DEX spread detection (BE-3.2).
//!
//! ## Strategy shape
//!
//! CEX-DEX arbitrage exploits the lag between centralised-exchange (CEX) order
//! books and on-chain DEX pricing. When the CEX mid price diverges from the
//! DEX quoted price by more than `min_spread_bps` the difference may be
//! capturable (subject to execution latency, withdrawal limits, gas, and
//! inventory constraints — all operator concerns beyond this scaffold).
//!
//! ## Scaffold scope (BE-3.2 Phase 1)
//!
//! This file delivers the **worker shell** only:
//!
//!   1. `CexDexPair` — one (CEX symbol, DEX pool) configuration entry.
//!   2. `CexDexWorkerConfig` — boot-time knobs (tick interval, HTTP timeout,
//!      custom base-URL overrides for tests).
//!   3. `CexDexWorker::run` — tokio task that polls CEX REST + DEX quoter
//!      every `tick_ms` and logs when spread ≥ `min_spread_bps`.
//!   4. `fetch_cex_price` — HTTP REST call to Binance or OKX ticker endpoint.
//!      Full WebSocket subscription is the follow-up (BE-3.2 Phase 2).
//!   5. `fetch_dex_price` — returns `Err` in Phase 1 (not yet wired).
//!      V3 QuoterV2 `eth_call` via `HttpRpcPool` lands in Phase 2.
//!
//! **Opportunity construction and pipeline emission are explicitly deferred.**
//! The `check_pair` method logs every detected spread but does NOT construct an
//! `Opportunity`, write to Redis, or insert into PostgreSQL. Reasons:
//!   - CEX-DEX requires capital on both sides (withdrawal rate limits, hedging
//!     latency) — operator decisions outside this scaffold.
//!   - Emitting unverified candidates into the spine risks false-positive
//!     execution in paper-trade mode.
//!   - BE-3.2 Phase 2 will wire the real quoter + add full `Opportunity`
//!     emission once the math is validated end-to-end.
//!
//! ## Doctrine compliance
//!
//! - **RULE 00 zero mocks**: `fetch_dex_price` returns `Err` when not wired —
//!   the worker skips the pair, logs at DEBUG, and increments
//!   `cex_dex_fetch_errors`. NEVER fabricates a synthetic price.
//! - **R8 fail-honest**: spread logged only when both prices are finite > 0.
//!   NaN / Inf / zero / negative → `Err` + skip.
//! - **No `unwrap()` outside tests**, no `unimplemented!()`, no `todo!()`.
//! - **Graceful absent-RPC boot**: `rpc_pool = None` → logs and continues;
//!   DEX fetch returns `Err`, pair is skipped, counter ticks.
//!
//! ## Cost
//!
//! At the default 5 000 ms tick with the MVP pair: 1 Binance REST call per
//! tick. DEX call returns `Err` immediately in Phase 1, so no RPC cost.
//! Both paths are best-effort; a failed call increments `cex_dex_fetch_errors`
//! and skips the pair.
//!
//! ## Phase 2 extension points
//!
//! 1. Replace REST polling with Binance/OKX WebSocket `bookTicker` subscription.
//! 2. Wire `fetch_dex_price` to `amm_math::v3_quote_exact_in_multicall` via
//!    `HttpRpcPool::with_retry`.
//! 3. Add `StrategyKind::CexDex` to `shared_rs::contracts` + extend
//!    `persistence::strategy_kind_str` (two lines).
//! 4. Construct `Opportunity` and call `publisher::publish` +
//!    `persistence::insert_opportunity_with_route` with a complete
//!    `RouteMetadata` (pool_addresses / token_addresses / dex_adapters from
//!    `CexDexPair.dex_pool_addr` + the quoted pair) so sim-ctl can resolve the
//!    route (§IV blocker A1: the no-route legacy insert is forbidden).
//! 5. Add heartbeat counters `cex_dex_pairs_scanned` + `cex_dex_opps_emitted`
//!    to `counters::ScannerCounters`.

use crate::counters::counters;
use shared_rs::rpc_failover::HttpRpcPool;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::Duration;
use tokio::time::interval;
use tracing::{debug, info, warn};

// ──────────────────────────────────────────────────────────────────────────────
// Configuration
// ──────────────────────────────────────────────────────────────────────────────

/// Default tick interval. REST polling at 5 s is conservative; Phase 2 WS
/// subscriptions will give <100 ms latency. Operator can tune via
/// `CEX_DEX_WORKER_INTERVAL_MS`.
#[allow(dead_code)]
pub const DEFAULT_TICK_MS: u64 = 5_000;

/// Per-request HTTP timeout for CEX REST calls. Matches `price_worker`
/// convention — price feeds are best-effort.
pub const HTTP_TIMEOUT_SECS: u64 = 8;

/// Default minimum spread threshold. Conservative: covers Binance taker fee
/// (7.5 bps) + UniV3 0.05% pool fee (5 bps) + gas + slippage buffer.
/// Operator can override per pair via `CexDexPair::min_spread_bps`.
pub const DEFAULT_MIN_SPREAD_BPS: u32 = 30;

/// One (CEX symbol, DEX pool) configuration entry.
///
/// Fields:
/// - `symbol`: CEX trading symbol, e.g. "ETHUSDT" (Binance) or "ETH-USDT" (OKX).
/// - `cex_exchange`: "binance" | "okx" — controls the REST endpoint used.
/// - `dex_pool_addr`: checksummed mainnet UniswapV3 pool address. Used in Phase 2
///   for the on-chain QuoterV2 call.
/// - `dex_token_in_decimals` / `dex_token_out_decimals`: raw ERC-20 decimals for
///   QuoterV2 integer → float conversion. Prevents silent decimal-mismatch bugs
///   (cf. Incidente #9).
/// - `min_spread_bps`: per-pair threshold. Overrides `DEFAULT_MIN_SPREAD_BPS`.
#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct CexDexPair {
    pub symbol: String,
    pub cex_exchange: String,
    pub dex_pool_addr: String,
    pub dex_token_in_decimals: u8,
    pub dex_token_out_decimals: u8,
    pub min_spread_bps: u32,
}

impl CexDexPair {
    /// MVP pair: ETH/USDT on Binance (REST) vs WETH/USDC 0.05% UniV3 pool
    /// (highest TVL mainnet pool, ~$500 M depth as of 2026).
    pub fn default_pairs() -> Vec<Self> {
        vec![CexDexPair {
            symbol: "ETHUSDT".to_string(),
            cex_exchange: "binance".to_string(),
            // WETH/USDC 0.05% pool (highest TVL single-tick mainnet pool).
            dex_pool_addr: "0x88e6A0c2dDD26FEEb64F039a2c41296FcB3f5640".to_string(),
            dex_token_in_decimals: 18,
            dex_token_out_decimals: 6,
            min_spread_bps: DEFAULT_MIN_SPREAD_BPS,
        }]
    }
}

/// Boot-time configuration for `CexDexWorker`.
#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct CexDexWorkerConfig {
    pub chain_id: u64,
    pub tick_ms: u64,
    /// Override for Binance base URL (tests inject a wiremock URL here).
    pub binance_base_url: Option<String>,
    /// Override for OKX base URL (tests inject a wiremock URL here).
    pub okx_base_url: Option<String>,
}

#[allow(dead_code)]
impl CexDexWorkerConfig {
    pub fn new(chain_id: u64, tick_ms: u64) -> Self {
        Self {
            chain_id,
            tick_ms,
            binance_base_url: None,
            okx_base_url: None,
        }
    }
}

// ──────────────────────────────────────────────────────────────────────────────
// Worker
// ──────────────────────────────────────────────────────────────────────────────

/// CEX-DEX spread detection worker.
///
/// Spawned via `tokio::spawn` in `main.rs`. Runs until process exit.
/// Does NOT emit `Opportunity` records in this scaffold — see module doc for
/// the Phase 2 extension plan.
#[allow(dead_code)]
pub struct CexDexWorker {
    cfg: CexDexWorkerConfig,
    /// Retained for Phase 2 DEX quoter path. Optional — absence is logged at boot.
    rpc_pool: Option<Arc<HttpRpcPool>>,
    pairs: Vec<CexDexPair>,
    http: reqwest::Client,
}

#[allow(dead_code)]
impl CexDexWorker {
    /// Construct a worker.
    ///
    /// `rpc_pool = None` → worker boots, CEX REST paths attempt normally,
    /// DEX quotes return `Err` (same as Phase 1 baseline). Logs at boot.
    pub fn new(
        cfg: CexDexWorkerConfig,
        rpc_pool: Option<Arc<HttpRpcPool>>,
    ) -> anyhow::Result<Self> {
        let http = reqwest::Client::builder()
            .timeout(Duration::from_secs(HTTP_TIMEOUT_SECS))
            .user_agent("arbitragex-v2-cex-dex-worker/0.1")
            .build()?;
        Ok(Self {
            cfg,
            rpc_pool,
            pairs: CexDexPair::default_pairs(),
            http,
        })
    }

    /// Override the pair set. Exposed for tests and future operator configuration.
    #[allow(dead_code)]
    pub fn with_pairs(mut self, pairs: Vec<CexDexPair>) -> Self {
        self.pairs = pairs;
        self
    }

    /// Main loop — runs forever. Caller must `tokio::spawn` this.
    pub async fn run(self) {
        info!(
            event = "cex_dex_worker.boot",
            chain_id = self.cfg.chain_id,
            pairs = self.pairs.len(),
            tick_ms = self.cfg.tick_ms,
            rpc_attached = self.rpc_pool.is_some(),
            "CEX-DEX worker started (Phase 1 scaffold — spread detection only)"
        );

        if self.rpc_pool.is_none() {
            warn!(
                event = "cex_dex_worker.no_rpc",
                chain_id = self.cfg.chain_id,
                "no RPC pool attached — DEX price fetch will always return Err \
                 (Phase 1 expected); pairs skipped each tick, cex_dex_fetch_errors \
                 will increment. Wire rpc_pool in Phase 2 to enable DEX quoting."
            );
        }

        let tick_duration = Duration::from_millis(self.cfg.tick_ms.max(100));
        let mut ticker = interval(tick_duration);
        // Drain the first immediate tick — lets price_worker + pool_sync_worker
        // warm up their caches first. Mirrors price_worker.rs convention.
        ticker.tick().await;

        loop {
            ticker.tick().await;
            for pair in &self.pairs {
                match self.check_pair(pair).await {
                    Ok(()) => {}
                    Err(e) => {
                        counters()
                            .cex_dex_fetch_errors
                            .fetch_add(1, Ordering::Relaxed);
                        debug!(
                            event = "cex_dex.pair_skip",
                            pair = %pair.symbol,
                            exchange = %pair.cex_exchange,
                            error = %e,
                        );
                    }
                }
            }
        }
    }

    /// One pair evaluation: fetch CEX price + DEX price, compute spread, log
    /// if threshold is met. No side effects (no Redis, no PG) in Phase 1.
    ///
    /// Returns `Ok(())` whether or not a spread was detected.
    /// Returns `Err` when a price source fails completely.
    pub async fn check_pair(&self, pair: &CexDexPair) -> anyhow::Result<()> {
        // 1. CEX mid price (Binance or OKX REST ticker).
        let cex_price = self.fetch_cex_price(pair).await?;

        // 2. DEX quoted price (V3 QuoterV2 eth_call — Phase 2 wiring).
        let dex_price = self.fetch_dex_price(pair).await?;

        // 3. Validate both prices are finite and positive before any arithmetic.
        //    R8 fail-honest: refuse non-sensical inputs rather than propagating
        //    garbage into a spread computation.
        if !cex_price.is_finite() || cex_price <= 0.0 {
            return Err(anyhow::anyhow!(
                "cex price invalid (non-finite or ≤ 0): {cex_price}"
            ));
        }
        if !dex_price.is_finite() || dex_price <= 0.0 {
            return Err(anyhow::anyhow!(
                "dex price invalid (non-finite or ≤ 0): {dex_price}"
            ));
        }

        // 4. Absolute spread in basis points: |dex - cex| / cex × 10 000.
        let spread_abs = (dex_price - cex_price).abs();
        let spread_bps = ((spread_abs / cex_price) * 10_000.0) as u32;

        if spread_bps >= pair.min_spread_bps {
            let direction = if dex_price > cex_price {
                "buy_cex_sell_dex"
            } else {
                "buy_dex_sell_cex"
            };
            info!(
                event = "cex_dex.spread_detected",
                pair = %pair.symbol,
                exchange = %pair.cex_exchange,
                cex_price,
                dex_price,
                spread_bps,
                direction,
                pool = %pair.dex_pool_addr,
                // Phase 2: construct Opportunity + persist here via
                // insert_opportunity_with_route (route_metadata from
                // dex_pool_addr + quoted pair — §IV blocker A1).
                note = "scaffold_only — opportunity emission deferred to BE-3.2 Phase 2"
            );
        } else {
            debug!(
                event = "cex_dex.spread_below_threshold",
                pair = %pair.symbol,
                spread_bps,
                threshold_bps = pair.min_spread_bps,
            );
        }

        Ok(())
    }

    /// Dispatch to the correct CEX REST handler.
    async fn fetch_cex_price(&self, pair: &CexDexPair) -> anyhow::Result<f64> {
        match pair.cex_exchange.as_str() {
            "binance" => self.fetch_binance_price(pair).await,
            "okx" => self.fetch_okx_price(pair).await,
            other => Err(anyhow::anyhow!("unsupported CEX exchange: {other}")),
        }
    }

    async fn fetch_binance_price(&self, pair: &CexDexPair) -> anyhow::Result<f64> {
        let base = self
            .cfg
            .binance_base_url
            .as_deref()
            .unwrap_or("https://api.binance.com");
        let url = format!("{}/api/v3/ticker/price?symbol={}", base, pair.symbol);
        let resp = self.http.get(&url).send().await?.error_for_status()?;
        let parsed: BinanceTickerPrice = resp.json().await?;
        parsed
            .price
            .parse::<f64>()
            .map_err(|e| anyhow::anyhow!("binance price parse '{}': {e}", parsed.price))
    }

    async fn fetch_okx_price(&self, pair: &CexDexPair) -> anyhow::Result<f64> {
        // OKX uses "ETH-USDT"; Binance uses "ETHUSDT". Normalise.
        let inst_id = to_okx_inst_id(&pair.symbol);
        let base = self
            .cfg
            .okx_base_url
            .as_deref()
            .unwrap_or("https://www.okx.com");
        let url = format!("{}/api/v5/market/ticker?instId={}", base, inst_id);
        let resp = self.http.get(&url).send().await?.error_for_status()?;
        let parsed: OkxTickerResponse = resp.json().await?;
        let entry = parsed
            .data
            .into_iter()
            .next()
            .ok_or_else(|| anyhow::anyhow!("okx returned empty data for {inst_id}"))?;
        entry
            .last
            .parse::<f64>()
            .map_err(|e| anyhow::anyhow!("okx price parse '{}': {e}", entry.last))
    }

    /// DEX quoted price via V3 QuoterV2.
    ///
    /// Phase 1: returns `Err` — integration deferred to BE-3.2 Phase 2.
    /// RULE 00: NEVER return a synthetic price here.
    async fn fetch_dex_price(&self, pair: &CexDexPair) -> anyhow::Result<f64> {
        // Suppress "unused variable" warnings while the fields are reserved for Phase 2.
        let _ = &self.rpc_pool;
        let _ = pair;
        Err(anyhow::anyhow!(
            "DEX quoter not yet wired (BE-3.2 Phase 2) — \
             pool {} skipped; set rpc_pool and implement V3 QuoterV2 call",
            pair.dex_pool_addr
        ))
    }
}

// ──────────────────────────────────────────────────────────────────────────────
// CEX response types
// ──────────────────────────────────────────────────────────────────────────────

/// Binance `GET /api/v3/ticker/price` shape.
/// `{"symbol":"ETHUSDT","price":"2517.42000000"}`
#[derive(serde::Deserialize)]
struct BinanceTickerPrice {
    price: String,
}

/// OKX `GET /api/v5/market/ticker` shape (abbreviated).
/// `{"code":"0","data":[{"instId":"ETH-USDT","last":"2517.42",...}]}`
#[derive(serde::Deserialize)]
struct OkxTickerResponse {
    data: Vec<OkxTickerEntry>,
}

#[derive(serde::Deserialize)]
struct OkxTickerEntry {
    last: String,
}

// ──────────────────────────────────────────────────────────────────────────────
// Helpers
// ──────────────────────────────────────────────────────────────────────────────

/// Convert a Binance-convention symbol ("ETHUSDT") to OKX instId ("ETH-USDT").
///
/// Tries known 4-char quote suffixes first, then 3-char. Falls back to the
/// symbol unchanged — OKX will return a 400, which surfaces as `Err` from
/// `fetch_okx_price`. This is correct fail-honest behaviour: don't guess.
pub fn to_okx_inst_id(binance_symbol: &str) -> String {
    // Already in OKX format.
    if binance_symbol.contains('-') {
        return binance_symbol.to_string();
    }
    // Longest suffix first to avoid mismatches (USDCUSDT → USDC-USDT, not USDCU-SDT).
    const QUOTE_4: &[&str] = &["USDT", "USDC", "BUSD", "WBTC"];
    const QUOTE_3: &[&str] = &["BTC", "ETH", "BNB"];
    for q in QUOTE_4 {
        if let Some(base) = binance_symbol.strip_suffix(q) {
            if !base.is_empty() {
                return format!("{}-{}", base, q);
            }
        }
    }
    for q in QUOTE_3 {
        if let Some(base) = binance_symbol.strip_suffix(q) {
            if !base.is_empty() {
                return format!("{}-{}", base, q);
            }
        }
    }
    binance_symbol.to_string()
}

// ──────────────────────────────────────────────────────────────────────────────
// Tests
// ──────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)] // M11: test module
    use super::*;
    use reqwest::Client;
    use serde::Deserialize;
    use std::time::Duration;
    use wiremock::matchers::{method, path, query_param};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    // ── Pair configuration ──────────────────────────────────────────────────

    #[test]
    fn default_pairs_includes_ethusdt_binance() {
        let pairs = CexDexPair::default_pairs();
        assert!(
            pairs
                .iter()
                .any(|p| p.symbol == "ETHUSDT" && p.cex_exchange == "binance"),
            "default pairs must include ETHUSDT on binance"
        );
    }

    #[test]
    fn default_pairs_min_spread_bps_in_sane_range() {
        for p in CexDexPair::default_pairs() {
            assert!(
                p.min_spread_bps >= 10 && p.min_spread_bps <= 500,
                "pair {} min_spread_bps={} must be 10-500 bps (0.1%-5%)",
                p.symbol,
                p.min_spread_bps
            );
        }
    }

    #[test]
    fn default_pairs_token_decimals_nonzero() {
        for p in CexDexPair::default_pairs() {
            assert!(p.dex_token_in_decimals > 0, "token_in_decimals must be > 0");
            assert!(
                p.dex_token_out_decimals > 0,
                "token_out_decimals must be > 0"
            );
        }
    }

    // ── OKX symbol conversion ───────────────────────────────────────────────

    #[test]
    fn to_okx_converts_ethusdt() {
        assert_eq!(to_okx_inst_id("ETHUSDT"), "ETH-USDT");
    }

    #[test]
    fn to_okx_converts_btcusdt() {
        assert_eq!(to_okx_inst_id("BTCUSDT"), "BTC-USDT");
    }

    #[test]
    fn to_okx_converts_bnbusdt() {
        assert_eq!(to_okx_inst_id("BNBUSDT"), "BNB-USDT");
    }

    #[test]
    fn to_okx_passthrough_when_already_dashed() {
        assert_eq!(to_okx_inst_id("ETH-USDT"), "ETH-USDT");
    }

    #[test]
    fn to_okx_unknown_symbol_passthrough() {
        // Unknown → returned unchanged; OKX will 400, which surfaces as Err.
        assert_eq!(to_okx_inst_id("XYZABC"), "XYZABC");
    }

    #[test]
    fn to_okx_converts_weth_wbtc() {
        assert_eq!(to_okx_inst_id("WETHWBTC"), "WETH-WBTC");
    }

    // ── Lightweight HTTP test helpers ───────────────────────────────────────
    //
    // Tests parse real serde_json via a thin helper that mirrors the worker's
    // HTTP logic. Avoids async-Redis setup complexity for unit tests.

    fn http_client() -> Client {
        Client::builder()
            .timeout(Duration::from_secs(HTTP_TIMEOUT_SECS))
            .user_agent("test")
            .build()
            .unwrap()
    }

    async fn get_binance_price(client: &Client, base: &str, symbol: &str) -> anyhow::Result<f64> {
        let url = format!("{}/api/v3/ticker/price?symbol={}", base, symbol);
        let resp = client.get(&url).send().await?.error_for_status()?;
        #[derive(Deserialize)]
        struct R {
            price: String,
        }
        let r: R = resp.json().await?;
        r.price.parse::<f64>().map_err(|e| anyhow::anyhow!("{e}"))
    }

    async fn get_okx_price(client: &Client, base: &str, inst_id: &str) -> anyhow::Result<f64> {
        let url = format!("{}/api/v5/market/ticker?instId={}", base, inst_id);
        let resp = client.get(&url).send().await?.error_for_status()?;
        #[derive(Deserialize)]
        struct Entry {
            last: String,
        }
        #[derive(Deserialize)]
        struct R {
            data: Vec<Entry>,
        }
        let r: R = resp.json().await?;
        let entry = r
            .data
            .into_iter()
            .next()
            .ok_or_else(|| anyhow::anyhow!("empty"))?;
        entry
            .last
            .parse::<f64>()
            .map_err(|e| anyhow::anyhow!("{e}"))
    }

    // ── Binance HTTP tests ──────────────────────────────────────────────────

    #[tokio::test]
    async fn fetch_binance_price_parses_response() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/v3/ticker/price"))
            .and(query_param("symbol", "ETHUSDT"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "symbol": "ETHUSDT",
                "price": "2517.42000000"
            })))
            .mount(&server)
            .await;
        let price = get_binance_price(&http_client(), &server.uri(), "ETHUSDT")
            .await
            .expect("parse ok");
        assert!((price - 2517.42).abs() < 1e-4, "price mismatch: {price}");
    }

    #[tokio::test]
    async fn fetch_binance_price_5xx_is_error() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/v3/ticker/price"))
            .respond_with(ResponseTemplate::new(503))
            .mount(&server)
            .await;
        let result = get_binance_price(&http_client(), &server.uri(), "ETHUSDT").await;
        assert!(result.is_err(), "5xx must surface as Err");
    }

    #[tokio::test]
    async fn fetch_binance_price_invalid_json_is_error() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/v3/ticker/price"))
            .respond_with(ResponseTemplate::new(200).set_body_string("not-json"))
            .mount(&server)
            .await;
        let result = get_binance_price(&http_client(), &server.uri(), "ETHUSDT").await;
        assert!(result.is_err(), "invalid JSON must be Err");
    }

    // ── OKX HTTP tests ──────────────────────────────────────────────────────

    #[tokio::test]
    async fn fetch_okx_price_parses_response() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/v5/market/ticker"))
            .and(query_param("instId", "ETH-USDT"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "code": "0",
                "data": [{"instId": "ETH-USDT", "last": "2516.00", "vol24h": "12345"}]
            })))
            .mount(&server)
            .await;
        let price = get_okx_price(&http_client(), &server.uri(), "ETH-USDT")
            .await
            .expect("parse ok");
        assert!((price - 2516.0).abs() < 1e-4, "price mismatch: {price}");
    }

    #[tokio::test]
    async fn fetch_okx_empty_data_is_error() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/v5/market/ticker"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "code": "0",
                "data": []
            })))
            .mount(&server)
            .await;
        let result = get_okx_price(&http_client(), &server.uri(), "ETH-USDT").await;
        assert!(result.is_err(), "empty data must be Err");
    }

    #[tokio::test]
    async fn fetch_okx_5xx_is_error() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/v5/market/ticker"))
            .respond_with(ResponseTemplate::new(500))
            .mount(&server)
            .await;
        let result = get_okx_price(&http_client(), &server.uri(), "ETH-USDT").await;
        assert!(result.is_err(), "5xx must be Err");
    }

    // ── Spread arithmetic ───────────────────────────────────────────────────

    #[test]
    fn spread_bps_arithmetic_is_correct() {
        // cex=2000, dex=2006 → spread=6/2000 × 10_000 = 30 bps.
        let cex = 2000.0_f64;
        let dex = 2006.0_f64;
        let spread_bps = (((dex - cex).abs() / cex) * 10_000.0) as u32;
        assert_eq!(spread_bps, 30);
    }

    #[test]
    fn spread_bps_below_threshold_is_not_detected() {
        // cex=2000, dex=2001 → 5 bps, below DEFAULT_MIN_SPREAD_BPS=30.
        let cex = 2000.0_f64;
        let dex = 2001.0_f64;
        let spread_bps = (((dex - cex).abs() / cex) * 10_000.0) as u32;
        assert!(spread_bps < DEFAULT_MIN_SPREAD_BPS);
    }
}
