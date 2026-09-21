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
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

/// A Binance bookTicker update (combined stream `@bookTicker`). Prices are
/// per-unit in the PAIR's quote currency; `event_ms` is Binance's event time,
/// `recv_ns` our local receive monotonic-ish epoch (SystemTime ns — wall clock
/// is fine here: staleness compares it to later wall-clock reads).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BookTicker {
    pub bid: f64,
    pub ask: f64,
    pub event_ms: u64,
    pub recv_ns: u64,
}

/// A Chainlink anchor sample from `latestRoundData()`. `answer` is already
/// decimal-adjusted (USD per unit); `updated_at` is the aggregator's round
/// timestamp (epoch SECONDS) used for staleness.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Anchor {
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
/// asks ascend from best (Binance order).
#[derive(Debug, Clone, PartialEq)]
pub struct Depth5 {
    pub bids: Vec<DepthLevel>,
    pub asks: Vec<DepthLevel>,
    pub event_ms: u64,
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

    fn fresh_anchor_price(&self, symbol: &str, now_ns_val: u64) -> Option<f64> {
        let a = self.snap.chainlink.get(symbol).copied()?;
        if self.anchor_is_fresh(symbol, a, now_ns_val) {
            Some(a.answer)
        } else {
            None
        }
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
        let now = now_ns();
        let ticker_usable = canonical_pair_for_token(&sym).and_then(|(pair, quote)| {
            let t = self.snap.binance.get(pair).copied()?;
            let fresh = now.saturating_sub(t.recv_ns) <= self.bus.cfg.binance_stale_ns;
            let q = self.quote_usd(quote)?;
            (fresh && t.bid > 0.0).then_some(t.bid * q)
        });
        let anchor = self.fresh_anchor_price(&sym, now);

        match (ticker_usable, anchor) {
            (Some(binance_usd), Some(_anchor_usd)) => {
                // Frozen latch, read lock-free from the published snapshot
                // (WO-V1 MAJOR: no band Mutex on the hot read path).
                if self.snap.frozen.contains(&sym) {
                    return (None, Verdict::DivergenceFrozen);
                }
                (Some(binance_usd), Verdict::Ok)
            }
            (Some(binance_usd), None) => (Some(binance_usd), Verdict::StaleAnchor),
            (None, Some(anchor_usd)) => (Some(anchor_usd), Verdict::StaleBinance),
            (None, None) => (None, Verdict::NoSource),
        }
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
        vwap_for_quote(depth, side, size_quote)
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
        }
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
            },
        );
        b.update_binance(
            "ETHUSDC",
            BookTicker {
                bid: 2.0,
                ask: 1.0,
                event_ms: 0,
                recv_ns: 1,
            },
        ); // crossed
        b.update_binance(
            "ETHUSDC",
            BookTicker {
                bid: f64::NAN,
                ask: 1.0,
                event_ms: 0,
                recv_ns: 1,
            },
        );
        assert!(b.view().snapshot().binance.is_empty());
        assert_eq!(b.updates_binance.load(Ordering::Relaxed), 0);
        b.update_anchor(
            "ETH",
            Anchor {
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
        };
        b.update_depth("ETHUSDC", six);
        assert!(b.view().snapshot().depth.is_empty());
    }
}
