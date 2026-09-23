//! PriceBus — atomic fused-price snapshot (PRECIO-CANONICO-CEX WO-PC1).
//!
//! Fuses two live sources under ONE lock-free read:
//!   - **Binance WS bookTicker** — the canonical CEX price (speed). Executable
//!     bid/ask, never mid; valuation uses the CONSERVATIVE side (bid — what
//!     you can actually sell into right now).
//!   - **Chainlink on-chain anchors** — the truth reference. A per-token
//!     Welford band over the live Binance↔Chainlink relative divergence
//!     decides when a pair is FROZEN (divergence beyond 2σ, floor 1%) instead
//!     of averaged — a depeg or bad feed must stop trading, not blend in.
//!
//! Read path (`PriceBus::view()`): a single `ArcSwap::load()` guard yields a
//! consistent, already-fused snapshot — "same millisecond" verification by
//! construction. Writers (`update_binance` / `update_anchor` / `update_depth`)
//! merge via `rcu`; each merge clones a small map (≤ ~10 entries), which is
//! negligible at bookTicker rates.
//!
//! Doctrine: R8 fail-honest. `NoSource` and `DivergenceFrozen` return `None`
//! — callers MUST reject with an explicit reason
//! (`no_live_price` / `price_divergence_binance_chainlink`), never fabricate.
//! Stables are NEVER assumed $1 — USDC/USDT/DAI resolve through the same
//! bus (Binance USDCUSDT book × Chainlink USDT anchor, or their own anchor).
//!
//! Fase 2 (WO-PC5): top-5 depth books (100ms partial streams) live here too;
//! `vwap_for_quote` walks the book so liquidity-aware sizing can price a
//! fill of a given size instead of assuming the touch price.

use crate::price_oracle::PriceOracle;
use arc_swap::ArcSwap;
use serde_json::{Map as JsonMap, Value};
use sha2::{Digest as Sha256Digest, Sha256};
use std::collections::{BTreeSet, HashMap};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

/// A Binance bookTicker update (combined stream `@bookTicker`). Prices are
/// per-unit in the PAIR's quote currency; `event_ms` is Binance's event time,
/// `recv_ns` our local receive monotonic-ish epoch (SystemTime ns — wall clock
/// is fine here: staleness compares it to later wall-clock reads).
/// `update_id` is Binance's order-book updateId (`u`) — source provenance
/// (WO-FE3); 0 = feed did not provide one.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BookTicker {
    pub bid: f64,
    pub ask: f64,
    pub event_ms: u64,
    pub recv_ns: u64,
    pub update_id: u64,
}

/// A Chainlink anchor sample from `latestRoundData()`. `answer` is already
/// decimal-adjusted (USD per unit); `updated_at` is the aggregator's round
/// timestamp (epoch SECONDS) used for staleness. `round_id` is the
/// aggregator's roundId (uint80) — source provenance (WO-FE3); 0 = feed did
/// not provide one.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Anchor {
    pub round_id: u64,
    pub answer: f64,
    pub updated_at: u64,
    pub recv_ns: u64,
}

/// One depth level (price/qty in pair units), sorted best-first by the feed.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DepthLevel {
    pub price: f64,
    pub qty: f64,
}

/// Top-5 partial book depth (`@depth5@100ms`). Bids descend from best,
/// asks ascend from best (Binance order). `last_update_id` is the book's
/// `lastUpdateId` — source provenance (WO-FE3); 0 = not provided.
#[derive(Debug, Clone, PartialEq)]
pub struct Depth5 {
    pub bids: Vec<DepthLevel>,
    pub asks: Vec<DepthLevel>,
    pub event_ms: u64,
    pub last_update_id: u64,
}

/// Immutable published snapshot. Symbol/pair keys are UPPERCASE.
#[derive(Debug, Clone, Default)]
pub struct PriceSnapshot {
    /// Key: uppercase Binance pair, e.g. "ETHUSDC".
    pub binance: HashMap<String, BookTicker>,
    /// Key: uppercase token symbol, e.g. "ETH", "USDT".
    pub chainlink: HashMap<String, Anchor>,
    /// Key: uppercase Binance pair.
    pub depth: HashMap<String, Depth5>,
    /// Tokens whose divergence band is FROZEN. Published in the snapshot so
    /// the read path stays lock-free (WO-V1 MAJOR: the band `Mutex` must not
    /// sit on the hot read path — only writers touch it now).
    pub frozen: std::collections::HashSet<String>,
}

/// Read-side verdict for a token price resolution.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Verdict {
    /// Binance fresh + anchor fresh + divergence inside the band.
    Ok,
    /// Binance book older than `binance_stale_ms` (or absent) → anchor price.
    StaleBinance,
    /// Anchor stale/absent → Binance price served, flagged (no verification).
    StaleAnchor,
    /// |binance − anchor| / anchor beyond the band → pair FROZEN (None).
    DivergenceFrozen,
    /// Neither source has usable data → None (R8).
    NoSource,
}

impl Verdict {
    pub fn as_str(self) -> &'static str {
        match self {
            Verdict::Ok => "ok",
            Verdict::StaleBinance => "stale_binance",
            Verdict::StaleAnchor => "stale_anchor",
            Verdict::DivergenceFrozen => "price_divergence_binance_chainlink",
            Verdict::NoSource => "no_live_price",
        }
    }
}

/// ─── WO-FE1: `arbx.pricebus.export.v1` canonical export ────────────────
///
/// Strict-consumer contract (tools/forensic_integrity/real_cards.py
/// `CanonicalPriceView`): the snapshot is a canonical-JSON object whose
/// `snapshot_hash` is the ARBX-CJSON-1 digest of the body WITHOUT the
/// `snapshot_hash` member. Decimal money values are exact decimal strings;
/// integers stay ≤ 2^53; nanosecond stamps are strings (the profile rejects
/// large integers as numbers). Verdicts that carry no verified price
/// (frozen / no source) are exported as EXPLICIT non-price records — a
/// stale_anchor or frozen pair is NEVER promoted to a verified verdict.
pub const EXPORT_SCHEMA: &str = "arbx.pricebus.export.v1";
pub const EXPORT_SOURCE: &str = "canonical_pricebus";

/// Render a double as an exact fixed-precision decimal string. Both Rust
/// `{:.` and Python `{:.` format the exact binary value with correct
/// rounding, so independent implementations produce identical bytes.
fn fmt_decimal(p: f64) -> String {
    format!("{p:.10}")
}

/// ARBX-CJSON-1 digest: serde_json with default features sorts object keys
/// and emits compact separators — byte-identical to the reference
/// `canonical_bytes` + sha256 in tools/forensic_integrity/integrity.py for
/// this profile (ASCII keys, no floats, ints ≤ 2^53).
fn canonical_digest(v: &Value) -> String {
    let bytes = serde_json::to_vec(v).expect("export profile is canonical-safe");
    let mut h = Sha256::new();
    h.update(&bytes);
    format!("{:x}", h.finalize())
}

/// Raw sources behind one fused resolution. Shared by the hot read path and
/// the export so the export can never re-derive (and diverge from) fusion.
struct ResolvedSources {
    price: Option<f64>,
    verdict: Verdict,
    pair: Option<(&'static str, &'static str)>,
    ticker: Option<BookTicker>,
    quote_anchor: Option<Anchor>,
    own_anchor: Option<Anchor>,
}

impl Default for ResolvedSources {
    fn default() -> Self {
        Self {
            price: None,
            verdict: Verdict::NoSource,
            pair: None,
            ticker: None,
            quote_anchor: None,
            own_anchor: None,
        }
    }
}

fn anchor_input_hash(symbol: &str, a: Anchor) -> String {
    // round_id as a decimal STRING: Chainlink roundId is uint80 (> 2^53) and
    // ARBX-CJSON-1 requires large ints as strings.
    canonical_digest(&serde_json::json!({
        "kind": "chainlink.latestRoundData",
        "symbol": symbol,
        "answer": fmt_decimal(a.answer),
        "round_id": a.round_id.to_string(),
        "updated_at": a.updated_at,
        "recv_ns": a.recv_ns.to_string(),
    }))
}

/// Source observation time (ms): the SOURCE's own timestamp when it has one
/// (Binance event time, Chainlink round time), else our local receive time.
fn observed_at_ms(r: &ResolvedSources) -> u64 {
    let mut obs: u64 = 0;
    if let Some(t) = r.ticker {
        obs = obs.max(if t.event_ms > 0 {
            t.event_ms
        } else {
            t.recv_ns / 1_000_000
        });
    }
    if let Some(a) = r.quote_anchor {
        obs = obs.max(anchor_observed_ms(a));
    }
    if let Some(a) = r.own_anchor {
        obs = obs.max(anchor_observed_ms(a));
    }
    obs
}

fn anchor_observed_ms(a: Anchor) -> u64 {
    if a.updated_at > 0 {
        a.updated_at * 1000
    } else {
        a.recv_ns / 1_000_000
    }
}

/// One `prices[key]` record. `key` is the exact map key the record is stored
/// under (consumer rejects any `asset_key` mismatch). Verified verdicts
/// (ok / stale_binance) carry full provenance; unverified ones (stale_anchor /
/// frozen / no_live_price) keep their honest verdict — the strict consumer
/// reads the verdict BEFORE the price, so their zeroed money fields are never
/// consumed as data.
fn export_record(key: &str, sym: &str, r: &ResolvedSources, now_ms: u64, bus: &PriceBus) -> Value {
    let price_str = match r.price {
        Some(p) if p.is_finite() && p > 0.0 => fmt_decimal(p),
        _ => "0".to_string(),
    };
    let valid_until_ms = if r.price.is_some() {
        match r.verdict {
            // Anchor-priced record: the anchor window bounds validity.
            Verdict::StaleBinance => {
                let secs = if crate::chains::is_stablecoin_symbol(sym) {
                    bus.config().anchor_stale_secs_stable
                } else {
                    bus.config().anchor_stale_secs_volatile
                };
                now_ms + secs * 1000
            }
            _ => now_ms + bus.config().binance_stale_ns / 1_000_000,
        }
    } else {
        0
    };
    let mut refs: BTreeSet<String> = BTreeSet::new();
    let mut input_hashes: Vec<String> = Vec::new();
    if let Some((pair, quote)) = r.pair {
        refs.insert(format!("binance_ws:{pair}@bookTicker"));
        refs.insert(format!("chainlink:{quote}/latestRoundData"));
        if let Some(a) = r.quote_anchor {
            input_hashes.push(anchor_input_hash(quote, a));
        }
        if let Some(t) = r.ticker {
            // update_id as a decimal STRING (ARBX-CJSON-1: u64 updateId can
            // exceed 2^53).
            input_hashes.push(canonical_digest(&serde_json::json!({
                "kind": "binance.bookTicker",
                "pair": pair,
                "bid": fmt_decimal(t.bid),
                "ask": fmt_decimal(t.ask),
                "event_ms": t.event_ms,
                "update_id": t.update_id.to_string(),
                "recv_ns": t.recv_ns.to_string(),
            })));
        }
    }
    if let Some(a) = r.own_anchor {
        refs.insert(format!("chainlink:{sym}/latestRoundData"));
        input_hashes.push(anchor_input_hash(sym, a));
    }
    serde_json::json!({
        "asset_key": key,
        "price_usd": price_str,
        "currency": "USD",
        "purpose": "valuation",
        "verdict": r.verdict.as_str(),
        "observed_at_ms": observed_at_ms(r),
        "valid_until_ms": valid_until_ms,
        "source_references": refs.into_iter().collect::<Vec<_>>(),
        "input_hashes": input_hashes,
    })
}

/// Tunables. Defaults follow the frozen charter decisions; every field is
/// overridable so tests (and only tests / explicit operator env wiring)
/// can shrink windows.
#[derive(Debug, Clone)]
pub struct PriceBusConfig {
    /// bookTicker older than this (ns since recv) counts as stale. Default 5s
    /// — bookTicker pushes on every book change; quiet ≥5s on ETH/BTC means
    /// the feed is gone, not the market.
    pub binance_stale_ns: u64,
    /// Anchor staleness for volatile assets (ETH/BTC/WBTC). Default 3900s —
    /// just above Chainlink's 1h heartbeat, so a quiet-but-healthy feed is
    /// not flagged.
    pub anchor_stale_secs_volatile: u64,
    /// Anchor staleness for stables (USDC/USDT/DAI). Default 90 000s — USDC's
    /// heartbeat is 24h (observed 2026-09-20: healthy feed, 13.7h age).
    pub anchor_stale_secs_stable: u64,
    /// Divergence band floor: never tighter than 1% (observed healthy
    /// divergence ETH ≈ +0.23% on 2026-09-20 raw sample).
    pub band_min: f64,
    /// Band multiplier over the Welford σ of live divergence.
    pub band_k: f64,
    /// Samples before the band is trusted (below → floor only).
    pub band_warmup: u32,
}

impl Default for PriceBusConfig {
    fn default() -> Self {
        Self {
            binance_stale_ns: 5_000_000_000,
            anchor_stale_secs_volatile: 3_900,
            anchor_stale_secs_stable: 90_000,
            band_min: 0.01,
            band_k: 2.0,
            band_warmup: 30,
        }
    }
}

/// Welford running mean/variance of the relative divergence samples.
#[derive(Debug, Clone, Default)]
struct Welford {
    n: u64,
    mean: f64,
    m2: f64,
}

impl Welford {
    fn push(&mut self, x: f64) {
        self.n += 1;
        let d = x - self.mean;
        self.mean += d / self.n as f64;
        self.m2 += d * (x - self.mean);
    }
    fn sigma(&self) -> f64 {
        if self.n < 2 {
            return 0.0;
        }
        (self.m2 / (self.n - 1) as f64).sqrt()
    }
}

/// Per-token divergence band state. `frozen` latches when divergence exceeds
/// the band; it re-heats (stats reset — "recalentable") once divergence
/// returns inside the FLOOR, so a genuine regime shift re-learns instead of
/// staying frozen on a stale σ.
#[derive(Debug, Clone, Default)]
struct BandState {
    welford: Welford,
    frozen: bool,
}

/// Canonical token→Binance-pair map (frozen charter set, validated against
/// the live exchange 2026-09-20: WBTCUSDC/DAIUSDC do NOT exist on spot).
/// Returns `(PAIR, QUOTE_SYMBOL)`.
pub fn canonical_pair_for_token(sym_upper: &str) -> Option<(&'static str, &'static str)> {
    match sym_upper {
        "WETH" | "ETH" => Some(("ETHUSDC", "USDC")),
        "WBTC" | "BTC" => Some(("WBTCUSDT", "USDT")),
        "USDC" => Some(("USDCUSDT", "USDT")),
        // USDT / DAI: no liquid Binance spot book — anchor-only resolution.
        _ => None,
    }
}

fn now_ns() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(0)
}

#[cfg(test)]
fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// The bus. Clone the `Arc` produced by [`PriceBus::new`] to share across
/// tasks. One writer task per source is the intended topology; the internal
/// band `Mutex` serializes just the statistics update, not the snapshot.
pub struct PriceBus {
    snapshot: ArcSwap<PriceSnapshot>,
    bands: Mutex<HashMap<String, BandState>>,
    cfg: PriceBusConfig,
    /// Monotone counters for observability (logged by owners, not scraped
    /// here — keeps shared-rs free of searcher's registry).
    pub updates_binance: AtomicU64,
    pub updates_anchor: AtomicU64,
    pub updates_depth: AtomicU64,
    pub freezes_total: AtomicU64,
}

impl PriceBus {
    pub fn new(cfg: PriceBusConfig) -> Arc<Self> {
        Arc::new(Self {
            snapshot: ArcSwap::from_pointee(PriceSnapshot::default()),
            bands: Mutex::new(HashMap::new()),
            cfg,
            updates_binance: AtomicU64::new(0),
            updates_anchor: AtomicU64::new(0),
            updates_depth: AtomicU64::new(0),
            freezes_total: AtomicU64::new(0),
        })
    }

    pub fn config(&self) -> &PriceBusConfig {
        &self.cfg
    }

    /// Digest of the export-relevant policy (WO-FE1). Floats render as exact
    /// decimal strings so the digest is byte-stable across languages.
    pub fn export_policy_hash(&self) -> String {
        canonical_digest(&serde_json::json!({
            "schema": EXPORT_SCHEMA,
            "anchor_stale_secs_stable": self.cfg.anchor_stale_secs_stable.to_string(),
            "anchor_stale_secs_volatile": self.cfg.anchor_stale_secs_volatile.to_string(),
            "band_k": fmt_decimal(self.cfg.band_k),
            "band_min": fmt_decimal(self.cfg.band_min),
            "band_warmup": self.cfg.band_warmup,
            "binance_stale_ns": self.cfg.binance_stale_ns.to_string(),
        }))
    }

    /// Canonical `arbx.pricebus.export.v1` snapshot (WO-FE1). `now_ms` is
    /// injected (deterministic test vector). `asset_keys` maps an UPPERCASE
    /// symbol (e.g. "ETH") to the consumer's asset id (e.g. "1:0xabc…");
    /// symbols without a mapping export under their own symbol.
    ///
    /// Every symbol present in EITHER source gets a record — including
    /// frozen / no-source symbols as explicit non-price records (R8: an absent
    /// key would be indistinguishable from "not computed").
    pub fn export_v1(&self, now_ms: u64, asset_keys: &HashMap<String, String>) -> Value {
        let view = self.view();
        let now_ns_val = (now_ms as u128 * 1_000_000u128) as u64;
        let mut symbols: BTreeSet<&str> = view.snap.chainlink.keys().map(|s| s.as_str()).collect();
        for pair in view.snap.binance.keys() {
            if let Some(token) = token_for_pair(pair) {
                symbols.insert(token);
            }
        }

        let mut prices = JsonMap::new();
        for sym in symbols {
            let r = view.resolve_fused(sym, now_ns_val);
            let key = asset_keys
                .get(sym)
                .cloned()
                .unwrap_or_else(|| sym.to_string());
            prices.insert(key.clone(), export_record(&key, sym, &r, now_ms, self));
        }

        let mut body = serde_json::json!({
            "schema": EXPORT_SCHEMA,
            "source": EXPORT_SOURCE,
            "generated_at_ms": now_ms,
            "policy_hash": self.export_policy_hash(),
            "prices": Value::Object(prices),
        });
        let snapshot_hash = canonical_digest(&body);
        if let Value::Object(m) = &mut body {
            m.insert("snapshot_hash".to_string(), Value::String(snapshot_hash));
        }
        body
    }

    /// Load a fused read view (single atomic load — consistent snapshot).
    pub fn view(&self) -> PriceView<'_> {
        PriceView {
            bus: self,
            snap: self.snapshot.load(),
        }
    }

    /// Merge one bookTicker update (pair key must be UPPERCASE, e.g.
    /// "ETHUSDC"). Also feeds the divergence band when a fresh anchor for
    /// the mapped token exists.
    pub fn update_binance(&self, pair_upper: &str, t: BookTicker) {
        if !(t.bid.is_finite() && t.ask.is_finite() && t.bid > 0.0 && t.ask >= t.bid) {
            return; // junk frame — drop honestly, never poison the book
        }
        self.updates_binance.fetch_add(1, Ordering::Relaxed);
        self.snapshot.rcu(|s| {
            let mut next = (**s).clone();
            next.binance.insert(pair_upper.to_string(), t);
            Arc::new(next)
        });
        if let Some(token) = token_for_pair(pair_upper) {
            self.sample_divergence(token);
        }
    }

    /// Merge one Chainlink anchor (symbol key UPPERCASE, decimals already
    /// applied by the caller).
    pub fn update_anchor(&self, symbol_upper: &str, a: Anchor) {
        if !(a.answer.is_finite() && a.answer > 0.0) {
            return;
        }
        self.updates_anchor.fetch_add(1, Ordering::Relaxed);
        self.snapshot.rcu(|s| {
            let mut next = (**s).clone();
            next.chainlink.insert(symbol_upper.to_string(), a);
            Arc::new(next)
        });
        self.sample_divergence(symbol_upper);
    }

    /// Merge one depth5 partial book (pair key UPPERCASE). Keeps at most the
    /// top 5 levels per side; levels failing finite/>0 validation drop the
    /// whole frame (a half-parsed book is worse than the previous one only
    /// if trusted — R8: prefer the last good frame).
    pub fn update_depth(&self, pair_upper: &str, d: Depth5) {
        let ok = |l: &DepthLevel| {
            l.price.is_finite() && l.price > 0.0 && l.qty.is_finite() && l.qty >= 0.0
        };
        if d.bids.len() > 5 || d.asks.len() > 5 || !d.bids.iter().all(ok) || !d.asks.iter().all(ok)
        {
            return;
        }
        self.updates_depth.fetch_add(1, Ordering::Relaxed);
        self.snapshot.rcu(|s| {
            let mut next = (**s).clone();
            next.depth.insert(pair_upper.to_string(), d.clone());
            Arc::new(next)
        });
    }

    /// Divergence statistics update for `symbol`: needs a fresh Binance
    /// executable price (via its canonical pair + fresh quote anchor) AND a
    /// fresh own anchor. Freezes / re-heats the band latch.
    fn sample_divergence(&self, symbol: &str) {
        let view = self.view();
        let (Some((pair, quote)), Some(ticker)) = (
            canonical_pair_for_token(symbol),
            canonical_pair_for_token(symbol).and_then(|(p, _)| view.snap.binance.get(p).copied()),
        ) else {
            return;
        };
        let now = now_ns();
        let Some(quote_usd) = view.fresh_anchor_price(quote, now) else {
            return;
        };
        let Some(anchor) = view.snap.chainlink.get(symbol).copied() else {
            return;
        };
        if !view.anchor_is_fresh(symbol, anchor, now) {
            return;
        }
        let binance_usd = ticker.bid * quote_usd;
        if binance_usd <= 0.0 || anchor.answer <= 0.0 {
            return;
        }
        let div = ((binance_usd - anchor.answer) / anchor.answer).abs();
        let _ = pair;
        let mut bands = match self.bands.lock() {
            Ok(b) => b,
            Err(poisoned) => poisoned.into_inner(),
        };
        let state = bands.entry(symbol.to_string()).or_default();
        let warm = state.welford.n >= self.cfg.band_warmup as u64;
        let threshold = if warm {
            (self.cfg.band_k * state.welford.sigma()).max(self.cfg.band_min)
        } else {
            self.cfg.band_min
        };
        if state.frozen {
            // Re-heat: divergence back inside the FLOOR resets stats so the
            // band re-learns the new regime instead of unfreezing onto a
            // σ computed from the old one.
            if div <= self.cfg.band_min {
                *state = BandState::default();
                state.welford.push(div);
                self.snapshot.rcu(|s| {
                    let mut next = (**s).clone();
                    next.frozen.remove(symbol);
                    Arc::new(next)
                });
            }
            return;
        }
        if warm && div > threshold {
            state.frozen = true;
            self.freezes_total.fetch_add(1, Ordering::Relaxed);
            self.snapshot.rcu(|s| {
                let mut next = (**s).clone();
                next.frozen.insert(symbol.to_string());
                Arc::new(next)
            });
            return;
        }
        state.welford.push(div);
    }
}

/// Reverse of [`canonical_pair_for_token`] for the tokens we map (used to
/// attribute a bookTicker to a token for band sampling).
fn token_for_pair(pair_upper: &str) -> Option<&'static str> {
    match pair_upper {
        "ETHUSDC" => Some("ETH"),
        "WBTCUSDT" => Some("WBTC"),
        "USDCUSDT" => Some("USDC"),
        _ => None,
    }
}

/// Fused read view over one consistent snapshot.
pub struct PriceView<'a> {
    bus: &'a PriceBus,
    snap: arc_swap::Guard<Arc<PriceSnapshot>>,
}

/// Which side of the book a fill consumes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Side {
    /// Buying base → walk ASKS ascending (unfavorable side).
    Buy,
    /// Selling base → walk BIDS descending (unfavorable side).
    Sell,
}

impl PriceView<'_> {
    /// Raw snapshot access (observability / tests).
    pub fn snapshot(&self) -> &PriceSnapshot {
        &self.snap
    }

    fn anchor_is_fresh(&self, symbol: &str, a: Anchor, now_ns_val: u64) -> bool {
        let max_age = if crate::chains::is_stablecoin_symbol(symbol) {
            self.bus.cfg.anchor_stale_secs_stable
        } else {
            self.bus.cfg.anchor_stale_secs_volatile
        };
        (now_ns_val / 1_000_000_000).saturating_sub(a.updated_at) <= max_age
    }

    fn fresh_anchor(&self, symbol: &str, now_ns_val: u64) -> Option<Anchor> {
        let a = self.snap.chainlink.get(symbol).copied()?;
        self.anchor_is_fresh(symbol, a, now_ns_val).then_some(a)
    }

    fn fresh_anchor_price(&self, symbol: &str, now_ns_val: u64) -> Option<f64> {
        self.fresh_anchor(symbol, now_ns_val).map(|a| a.answer)
    }

    /// USD price of the pair's quote token, from a FRESH Chainlink anchor.
    /// Quotes are stables (USDC/USDT) → their long heartbeats are honored.
    fn quote_usd(&self, quote: &str) -> Option<f64> {
        self.fresh_anchor_price(quote, now_ns())
    }

    /// Resolve `symbol`'s fused USD price + verdict. `None` results MUST be
    /// treated as explicit rejections (R8) with the verdict as the reason.
    pub fn price_with_verdict(&self, symbol: &str) -> (Option<f64>, Verdict) {
        let sym = symbol.trim().to_ascii_uppercase();
        if sym.is_empty() {
            return (None, Verdict::NoSource);
        }
        let r = self.resolve_fused(&sym, now_ns());
        (r.price, r.verdict)
    }

    /// Single source of truth for fusion: the hot read path AND the export
    /// both consume this. `now_ns_val` is injected so the export can pin a
    /// deterministic instant (WO-FE1 test vector).
    fn resolve_fused(&self, sym_upper: &str, now_ns_val: u64) -> ResolvedSources {
        let mut r = ResolvedSources {
            verdict: Verdict::NoSource,
            ..ResolvedSources::default()
        };
        let ticker_usable: Option<f64> = if let Some((pair, quote)) =
            canonical_pair_for_token(sym_upper)
        {
            match self.snap.binance.get(pair).copied() {
                Some(t)
                    if now_ns_val.saturating_sub(t.recv_ns) <= self.bus.cfg.binance_stale_ns
                        && t.bid > 0.0 =>
                {
                    match self.fresh_anchor(quote, now_ns_val) {
                        Some(qa) => {
                            r.pair = Some((pair, quote));
                            r.ticker = Some(t);
                            r.quote_anchor = Some(qa);
                            Some(t.bid * qa.answer)
                        }
                        None => None,
                    }
                }
                _ => None,
            }
        } else {
            None
        };
        let own_anchor = self.fresh_anchor(sym_upper, now_ns_val);

        match (ticker_usable, own_anchor) {
            (Some(binance_usd), Some(a)) => {
                // Frozen latch, read lock-free from the published snapshot
                // (WO-V1 MAJOR: no band Mutex on the hot read path).
                r.own_anchor = Some(a);
                if self.snap.frozen.contains(sym_upper) {
                    r.verdict = Verdict::DivergenceFrozen;
                } else {
                    r.price = Some(binance_usd);
                    r.verdict = Verdict::Ok;
                }
            }
            (Some(binance_usd), None) => {
                r.price = Some(binance_usd);
                r.verdict = Verdict::StaleAnchor;
            }
            (None, Some(a)) => {
                r.own_anchor = Some(a);
                r.price = Some(a.answer);
                r.verdict = Verdict::StaleBinance;
            }
            (None, None) => {}
        }
        r
    }

    /// Fused price; `None` on frozen/no-source (caller rejects fail-honest).
    pub fn price_usd(&self, symbol: &str) -> Option<f64> {
        self.price_with_verdict(symbol).0
    }

    /// Top-5 book for a token's canonical pair (WO-PC5 doble vía — liquidity
    /// consultable por las estrategias que aplican sizing consciente).
    pub fn depth_for_token(&self, symbol: &str) -> Option<&Depth5> {
        let sym = symbol.trim().to_ascii_uppercase();
        let (pair, _) = canonical_pair_for_token(&sym)?;
        self.snap.depth.get(pair)
    }

    /// VWAP price for a USD-sized fill on `symbol`'s canonical pair, walking
    /// the unfavorable side of the top-5 book (WO-PC5). Returns the average
    /// fill price; `None` when there is no book / insufficient liquidity /
    /// no fresh quote anchor (fail-honest — callers cap size or reject).
    pub fn vwap_usd(&self, symbol: &str, side: Side, size_usd: f64) -> Option<f64> {
        if !(size_usd.is_finite() && size_usd > 0.0) {
            return None;
        }
        let sym = symbol.trim().to_ascii_uppercase();
        let (pair, quote) = canonical_pair_for_token(&sym)?;
        let depth = self.snap.depth.get(pair)?;
        let quote_usd = self.quote_usd(quote)?;
        let size_quote = size_usd / quote_usd;
        // vwap_for_quote returns QUOTE/base, not USD/base. Never assume
        // USDC/USDT equals one dollar; use the same fresh quote anchor.
        let price_quote = vwap_for_quote(depth, side, size_quote)?;
        let price_usd = price_quote * quote_usd;
        (price_usd.is_finite() && price_usd > 0.0).then_some(price_usd)
    }
}

/// Walk a top-5 book to fill `size_quote` (pair-quote units, e.g. USDT for
/// BTCUSDT) on the unfavorable side; returns the volume-weighted average
/// price. Pure function — unit-testable without a bus.
pub fn vwap_for_quote(depth: &Depth5, side: Side, size_quote: f64) -> Option<f64> {
    let levels = match side {
        Side::Buy => &depth.asks,
        Side::Sell => &depth.bids,
    };
    if levels.is_empty() || !(size_quote.is_finite() && size_quote > 0.0) {
        return None;
    }
    let mut remaining = size_quote;
    let mut cost = 0.0;
    let mut base = 0.0;
    for l in levels {
        if remaining <= 0.0 {
            break;
        }
        let level_quote = l.price * l.qty;
        if level_quote <= 0.0 {
            continue;
        }
        let take = level_quote.min(remaining);
        let take_base = take / l.price;
        cost += take;
        base += take_base;
        remaining -= take;
    }
    if remaining > size_quote * 1e-9 || base <= 0.0 {
        // Relative epsilon: an "exact" fill on non-dyadic prices can leave a
        // ~1e-13 FP residue (WO-V1 MINOR) — only a real shortfall rejects.
        return None; // book too thin for this size — honest failure
    }
    Some(cost / base)
}

/// `PriceOracle` adapter over the bus — slots into `CascadePriceOracle` as
/// the top tier wherever the searcher has the in-process bus available.
pub struct BusPriceOracle(pub Arc<PriceBus>);

impl PriceOracle for BusPriceOracle {
    fn price_usd(&self, token_id: &str) -> Option<f64> {
        self.0.view().price_usd(token_id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bus() -> Arc<PriceBus> {
        PriceBus::new(PriceBusConfig::default())
    }

    fn anchor(answer: f64) -> Anchor {
        Anchor {
            round_id: 4_200_000_000,
            answer,
            updated_at: now_secs() - 10,
            recv_ns: now_ns(),
        }
    }

    fn ticker(bid: f64, ask: f64) -> BookTicker {
        BookTicker {
            bid,
            ask,
            event_ms: 0,
            recv_ns: now_ns(),
            update_id: 987_654_321,
        }
    }

    #[test]
    fn vwap_usd_applies_the_quote_currency_anchor() {
        let b = bus();
        // Synthetic unit vector: 10 quote/base, quote worth $0.80.
        // A $80 buy spends 100 quote for 10 base; true VWAP = $8/base.
        b.update_anchor("USDC", anchor(0.8));
        b.update_depth(
            "ETHUSDC",
            Depth5 {
                bids: vec![DepthLevel {
                    price: 9.0,
                    qty: 100.0,
                }],
                asks: vec![DepthLevel {
                    price: 10.0,
                    qty: 100.0,
                }],
                event_ms: now_ns() / 1_000_000,
                last_update_id: 0,
            },
        );
        let result = b.view().vwap_usd("ETH", Side::Buy, 80.0);
        assert!(matches!(result, Some(p) if (p - 8.0).abs() < 1e-10));
    }

    #[test]
    fn fused_ok_price_uses_conservative_bid_times_quote_anchor() {
        let b = bus();
        // USDT anchor 0.9998, ETHUSDC bid 2625.46 → ETH = 2625.46 × 0.9998…
        b.update_anchor("USDT", anchor(0.9998));
        b.update_anchor("USDC", anchor(0.99984));
        b.update_anchor("ETH", anchor(2619.59));
        b.update_binance("ETHUSDC", ticker(2625.46, 2625.47));
        let v = b.view();
        let (p, verdict) = v.price_with_verdict("ETH");
        assert_eq!(verdict, Verdict::Ok);
        let expected = 2625.46f64 * 0.99984;
        assert!((p.unwrap() - expected).abs() < 1e-6, "got {p:?}");
        // WETH and ETH resolve the same canonical pair.
        assert_eq!(v.price_with_verdict("WETH").0, Some(expected));
    }

    #[test]
    fn stale_binance_falls_back_to_anchor() {
        let b = bus();
        b.update_anchor("ETH", anchor(2619.59));
        // Ancient ticker → stale.
        b.update_binance(
            "ETHUSDC",
            BookTicker {
                bid: 2625.0,
                ask: 2625.1,
                event_ms: 0,
                recv_ns: now_ns().saturating_sub(60_000_000_000),
                update_id: 0,
            },
        );
        let v = b.view();
        let (p, verdict) = v.price_with_verdict("ETH");
        assert_eq!(verdict, Verdict::StaleBinance);
        assert_eq!(p, Some(2619.59));
    }

    #[test]
    fn stale_anchor_serves_binance_flagged() {
        let b = bus();
        b.update_anchor("USDC", anchor(0.99984));
        b.update_anchor(
            "ETH",
            Anchor {
                round_id: 0,
                answer: 2619.59,
                updated_at: now_secs() - 10_000, // > 3900s volatile limit
                recv_ns: now_ns(),
            },
        );
        b.update_binance("ETHUSDC", ticker(2625.46, 2625.47));
        let (p, verdict) = b.view().price_with_verdict("ETH");
        assert_eq!(verdict, Verdict::StaleAnchor);
        let expected = 2625.46f64 * 0.99984;
        assert!((p.unwrap() - expected).abs() < 1e-6);
    }

    #[test]
    fn no_source_is_none_fail_honest() {
        let b = bus();
        let (p, verdict) = b.view().price_with_verdict("ETH");
        assert_eq!(verdict, Verdict::NoSource);
        assert!(p.is_none());
        // Empty symbol → same.
        assert_eq!(b.view().price_with_verdict("  ").1, Verdict::NoSource);
    }

    #[test]
    fn stable_anchor_heartbeat_is_honored_not_flagged() {
        // USDC anchor 13.7h old is HEALTHY (24h heartbeat) — observed live
        // 2026-09-20. It must price USDC (via own anchor) without StaleAnchor.
        let b = bus();
        b.update_anchor(
            "USDC",
            Anchor {
                round_id: 0,
                answer: 0.99984488,
                updated_at: now_secs() - 49_297,
                recv_ns: now_ns(),
            },
        );
        let (p, verdict) = b.view().price_with_verdict("USDC");
        // No Binance book in this scenario → StaleBinance verdict, but the
        // ANCHOR side is fresh (13.7h < 90 000s stable limit) so the price
        // is served. If the stable heartbeat were mis-thresholded (volatile
        // 3900s), `p` would be None.
        assert_eq!(verdict, Verdict::StaleBinance);
        assert_eq!(p, Some(0.99984488));
    }

    #[test]
    fn stables_never_one_dollar_exact() {
        // Regression lock for the operator doctrine: USDC resolves to its
        // LIVE anchor (0.99984…), never a fabricated 1.0.
        let b = bus();
        b.update_anchor("USDC", anchor(0.99984488));
        let p = b.view().price_usd("USDC").unwrap();
        assert!((p - 1.0).abs() > 1e-9, "USDC must not be hardcode $1");
    }

    #[test]
    fn divergence_freeze_latches_and_reheats() {
        let b = bus();
        b.update_anchor("USDT", anchor(0.9998));
        b.update_anchor("USDC", anchor(1.0));
        b.update_anchor("ETH", anchor(2600.0));
        // Warm the band with tight samples (below warmup → floor 1% band).
        for i in 0..30 {
            b.update_binance("ETHUSDC", ticker(2600.0 + (i % 3) as f64 * 0.5, 2601.0));
        }
        let (p, v) = b.view().price_with_verdict("ETH");
        assert_eq!(v, Verdict::Ok);
        assert!(p.is_some());
        // Jump binance 3% away (sigma small after tight warmup → > 2σ band)…
        b.update_binance("ETHUSDC", ticker(2680.0, 2680.5));
        let (p, v) = b.view().price_with_verdict("ETH");
        assert_eq!(v, Verdict::DivergenceFrozen, "expected freeze, got {p:?}");
        assert!(p.is_none(), "frozen pair serves NO price");
        assert_eq!(b.freezes_total.load(Ordering::Relaxed), 1);
        // The latch is PUBLISHED in the snapshot (lock-free read path).
        assert!(b.view().snapshot().frozen.contains("ETH"));
        // …divergence back inside the floor (≤1%) → re-heat, price returns.
        b.update_binance("ETHUSDC", ticker(2605.0, 2605.5)); // ~0.19% off
        let (p, v) = b.view().price_with_verdict("ETH");
        assert_eq!(v, Verdict::Ok);
        assert!(p.is_some());
    }

    #[test]
    fn junk_frames_are_dropped() {
        let b = bus();
        b.update_binance(
            "ETHUSDC",
            BookTicker {
                bid: 0.0,
                ask: 1.0,
                event_ms: 0,
                recv_ns: 1,
                update_id: 0,
            },
        );
        b.update_binance(
            "ETHUSDC",
            BookTicker {
                bid: 2.0,
                ask: 1.0,
                event_ms: 0,
                recv_ns: 1,
                update_id: 0,
            },
        ); // crossed
        b.update_binance(
            "ETHUSDC",
            BookTicker {
                bid: f64::NAN,
                ask: 1.0,
                event_ms: 0,
                recv_ns: 1,
                update_id: 0,
            },
        );
        assert!(b.view().snapshot().binance.is_empty());
        assert_eq!(b.updates_binance.load(Ordering::Relaxed), 0);
        b.update_anchor(
            "ETH",
            Anchor {
                round_id: 0,
                answer: -5.0,
                updated_at: now_secs(),
                recv_ns: 1,
            },
        );
        assert!(b.view().snapshot().chainlink.is_empty());
    }

    #[test]
    fn bus_price_oracle_slots_into_cascade_contract() {
        let b = bus();
        b.update_anchor("USDC", anchor(0.99984));
        b.update_anchor("ETH", anchor(2619.59));
        let o = BusPriceOracle(b);
        assert_eq!(o.price_usd("ETH"), Some(2619.59)); // anchor-only until binance lands
        assert_eq!(o.price_usd("PEPE"), None);
    }

    // ─── WO-PC5: VWAP walking the top-5 book ─────────────────────────────

    fn depth() -> Depth5 {
        Depth5 {
            bids: vec![
                DepthLevel {
                    price: 100.0,
                    qty: 1.0,
                },
                DepthLevel {
                    price: 99.0,
                    qty: 2.0,
                },
                DepthLevel {
                    price: 98.0,
                    qty: 3.0,
                },
            ],
            asks: vec![
                DepthLevel {
                    price: 101.0,
                    qty: 1.0,
                },
                DepthLevel {
                    price: 102.0,
                    qty: 2.0,
                },
                DepthLevel {
                    price: 103.0,
                    qty: 3.0,
                },
            ],
            event_ms: 0,
            last_update_id: 0,
        }
    }

    #[test]
    fn vwap_buy_walks_asks_weighted() {
        // Fill 303 quote on asks: 101×1 (=101) + 102×2 (=204) → under ask3.
        let d = depth();
        let v = vwap_for_quote(&d, Side::Buy, 101.0).unwrap();
        assert!((v - 101.0).abs() < 1e-9);
        let v = vwap_for_quote(&d, Side::Buy, 305.0).unwrap();
        // (101·1 + 102·2 + 103·(0/…)) — 305 leaves 0 after two levels exactly? 101+204=305 → exact.
        let expected = (101.0 * 1.0 + 102.0 * 2.0) / 3.0;
        assert!((v - expected).abs() < 1e-9);
    }

    #[test]
    fn vwap_insufficient_liquidity_is_none() {
        let d = depth();
        // Max ask liquidity = 101 + 204 + 309 = 614 quote; ask 700 → None.
        assert!(vwap_for_quote(&d, Side::Buy, 700.0).is_none());
        assert!(vwap_for_quote(&d, Side::Sell, 700.0).is_none());
        assert!(vwap_for_quote(&d, Side::Buy, 0.0).is_none());
    }

    #[test]
    fn vwap_sell_walks_bids_and_spreads_levels() {
        let d = depth();
        // 200 quote on bids: 100×1 (=100) + 99×(100/99≈1.0101…) → ~200.
        let v = vwap_for_quote(&d, Side::Sell, 200.0).unwrap();
        let base = 1.0 + 100.0 / 99.0;
        let expected = 200.0 / base;
        assert!((v - expected).abs() < 1e-9);
        assert!(v < 100.0 && v > 99.0, "sell VWAP must sit between levels");
    }

    #[test]
    fn vwap_usd_via_bus_uses_quote_anchor() {
        let b = bus();
        b.update_anchor("USDC", anchor(1.0));
        b.update_depth(
            "ETHUSDC",
            Depth5 {
                bids: vec![DepthLevel {
                    price: 2600.0,
                    qty: 2.0,
                }],
                asks: vec![DepthLevel {
                    price: 2601.0,
                    qty: 2.0,
                }],
                event_ms: 0,
                last_update_id: 0,
            },
        );
        let v = b.view();
        // $5202 buy on asks: 2601×2 exactly.
        let p = v.vwap_usd("ETH", Side::Buy, 5202.0).unwrap();
        assert!((p - 2601.0).abs() < 1e-9);
        // Thin book for $1M → None (honest).
        assert!(v.vwap_usd("ETH", Side::Buy, 1_000_000.0).is_none());
        // Token without a pair (DAI) → None.
        assert!(v.vwap_usd("DAI", Side::Buy, 100.0).is_none());
    }

    #[test]
    fn depth_frames_validate() {
        let b = bus();
        b.update_depth(
            "ETHUSDC",
            Depth5 {
                bids: vec![DepthLevel {
                    price: -1.0,
                    qty: 1.0,
                }],
                asks: vec![],
                event_ms: 0,
                last_update_id: 0,
            },
        );
        assert!(b.view().snapshot().depth.is_empty());
        // >5 levels rejected.
        let six = Depth5 {
            bids: (0..6)
                .map(|i| DepthLevel {
                    price: 100.0 - i as f64,
                    qty: 1.0,
                })
                .collect(),
            asks: vec![],
            event_ms: 0,
            last_update_id: 0,
        };
        b.update_depth("ETHUSDC", six);
        assert!(b.view().snapshot().depth.is_empty());
    }

    // ─── WO-FE1: canonical export `arbx.pricebus.export.v1` ────────────────

    fn export_bus() -> Arc<PriceBus> {
        let b = bus();
        b.update_anchor("USDC", anchor(0.99984));
        b.update_anchor("ETH", anchor(2619.59));
        b.update_binance("ETHUSDC", ticker(2625.46, 2625.47));
        b
    }

    #[test]
    fn export_v1_hashes_maps_assets_and_provenance() {
        let b = export_bus();
        let now_ms = now_ns() / 1_000_000;
        let mut asset_keys = HashMap::new();
        asset_keys.insert("ETH".to_string(), "1:0xeth".to_string());
        let snap = b.export_v1(now_ms, &asset_keys);

        assert_eq!(snap["schema"], EXPORT_SCHEMA);
        assert_eq!(snap["source"], EXPORT_SOURCE);
        assert_eq!(snap["generated_at_ms"], now_ms);
        assert!(b.export_policy_hash().len() == 64);
        assert_eq!(snap["policy_hash"], b.export_policy_hash().as_str());

        let rec = &snap["prices"]["1:0xeth"];
        assert_eq!(rec["asset_key"], "1:0xeth");
        assert_eq!(rec["verdict"], "ok");
        assert_eq!(rec["currency"], "USD");
        assert_eq!(rec["purpose"], "valuation");
        let expected = format!("{:.10}", 2625.46f64 * 0.99984);
        assert_eq!(rec["price_usd"], expected.as_str());
        assert!(rec["observed_at_ms"].as_u64().unwrap() > 0);
        assert!(rec["valid_until_ms"].as_u64().unwrap() >= now_ms);
        assert!(!rec["source_references"].as_array().unwrap().is_empty());
        assert!(!rec["input_hashes"].as_array().unwrap().is_empty());
        // USDC has no asset_keys mapping → exported under its own symbol.
        assert!(snap["prices"]["USDC"]["verdict"] == "stale_binance");

        // snapshot_hash = digest(body without snapshot_hash) — structural
        // self-consistency (cross-language bytes are proven by the external
        // vector test, never re-computed here).
        let mut body = snap.clone();
        let obj = body.as_object_mut().unwrap();
        let stored = obj.remove("snapshot_hash").unwrap();
        assert_eq!(stored, canonical_digest(&body).as_str());
        // Tamper detection: mutate one price, re-digest → mismatch.
        let mut tampered = body.clone();
        tampered["prices"]["1:0xeth"]["price_usd"] = "9999.0".into();
        assert_ne!(stored, canonical_digest(&tampered).as_str());
    }

    #[test]
    fn export_v1_stale_anchor_is_never_verified() {
        // Binance fresh, own ETH anchor STALE → verdict must stay
        // "stale_anchor" (the strict consumer rejects it as unverified).
        let b = bus();
        b.update_anchor("USDC", anchor(0.99984));
        b.update_anchor(
            "ETH",
            Anchor {
                round_id: 0,
                answer: 2619.59,
                updated_at: now_secs() - 10_000,
                recv_ns: now_ns(),
            },
        );
        b.update_binance("ETHUSDC", ticker(2625.46, 2625.47));
        let snap = b.export_v1(now_ns() / 1_000_000, &HashMap::new());
        assert_eq!(snap["prices"]["ETH"]["verdict"], "stale_anchor");
    }

    #[test]
    fn export_v1_frozen_and_no_source_are_explicit_non_price_records() {
        // Stale-but-present anchor → symbol exported with no_live_price,
        // price "0", no expiry.
        let b = bus();
        b.update_anchor(
            "ETH",
            Anchor {
                round_id: 0,
                answer: 2619.59,
                updated_at: now_secs() - 10_000,
                recv_ns: now_ns(),
            },
        );
        let snap = b.export_v1(now_ns() / 1_000_000, &HashMap::new());
        let rec = &snap["prices"]["ETH"];
        assert_eq!(rec["verdict"], "no_live_price");
        assert_eq!(rec["price_usd"], "0");
        assert_eq!(rec["valid_until_ms"], 0);

        // Frozen pair (warm band + 3% jump, as in the freeze test).
        let b = bus();
        b.update_anchor("USDT", anchor(0.9998));
        b.update_anchor("USDC", anchor(1.0));
        b.update_anchor("ETH", anchor(2600.0));
        for i in 0..30 {
            b.update_binance("ETHUSDC", ticker(2600.0 + (i % 3) as f64 * 0.5, 2601.0));
        }
        b.update_binance("ETHUSDC", ticker(2680.0, 2680.5));
        let snap = b.export_v1(now_ns() / 1_000_000, &HashMap::new());
        let rec = &snap["prices"]["ETH"];
        assert_eq!(rec["verdict"], "price_divergence_binance_chainlink");
        assert_eq!(rec["price_usd"], "0");
        assert_eq!(rec["valid_until_ms"], 0);
    }
}
