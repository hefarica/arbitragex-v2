//! MARKET-FEATURES-01 — `volatility`: realized volatility over a REAL time axis.
//!
//! ## Why this is not a proxy
//!
//! FEATURES-01c made `RegimeRouter::analyze` PREFER a caller-supplied
//! `features["volatility"]` over its own cross-venue proxy, and documented why
//! (`regime_router.rs:93-111`): the proxy treats the ROWS of `price_matrix` as a
//! series, but each row is a pool of the route and the hops of a route are
//! DIFFERENT pairs — log-ratios between them are cross-pair dispersion wearing
//! the label "volatility". The router cannot fix that because `MarketState`
//! carries no time axis. FEATURES-01c added the preference; **nothing added the
//! producer**, so in practice the preference never fires.
//!
//! This module is that producer, and it is honest about the only thing that
//! makes the number meaningful: **the time axis**. It accumulates genuine
//! `(timestamp, price)` observations of the fused PriceBus USD price and
//! computes the sample standard deviation of the log returns between
//! consecutive observations.
//!
//! ## The three guards that keep it from lying
//!
//! 1. **Invalid prices are not observations.** `NaN`, `±inf` and `<= 0` are
//!    dropped at `observe` time. A non-positive price is bad data, not a
//!    -100% return.
//! 2. **Monotone timestamps only.** A sample whose `ts_ns` is not strictly
//!    greater than the last one is dropped. Out-of-order arrivals would
//!    otherwise inject a negative time delta and a sign-flipped return.
//! 3. **A real span is required.** `min_samples` alone is not enough: 8
//!    observations inside 2 ms produce a number that is real arithmetic over a
//!    meaningless axis. `min_span_ns` (default 60 s) forces the window to
//!    actually span time, and below it the answer is `None` — absence, never a
//!    small fabricated number.
//!
//! ## `Some(0.0)` vs `None` (R8)
//!
//! If the window has enough samples over enough real span and the price did not
//! move, the correct answer is `Some(0.0)` — **computed, and exactly zero**.
//! Returning `None` there would be the mirror-image lie (hiding a measurement).
//! `None` is reserved for "not computable". Both directions are tested.

use std::collections::{HashMap, VecDeque};

/// Default rolling window: 15 minutes of observations.
pub const DEFAULT_WINDOW_NS: u64 = 900 * 1_000_000_000;
/// Default minimum observations (=> >= 7 log returns).
pub const DEFAULT_MIN_SAMPLES: usize = 8;
/// Default minimum real span between first and last observation.
pub const DEFAULT_MIN_SPAN_NS: u64 = 60 * 1_000_000_000;
/// Default per-symbol ring capacity. Bounded so a hot call site cannot grow
/// memory without bound; ~512 f64 + timestamps per symbol is negligible.
pub const DEFAULT_CAPACITY: usize = 512;

/// One real observation: the fused USD price of a token at a wall-clock instant.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Sample {
    /// Observation instant, nanoseconds since the Unix epoch (same base as
    /// `PriceBus`'s `recv_ns` / `Anchor.updated_at * 1e9`).
    pub ts_ns: u64,
    /// Fused USD price per token at that instant.
    pub price_usd: f64,
}

/// The estimator's window contract, declared explicitly so the value is never
/// read without its axis.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VolatilityWindow {
    /// Only observations younger than this (relative to `now_ns`) participate.
    pub window_ns: u64,
    /// Minimum observations required (>= 2 so at least one return exists; the
    /// default 8 is the one actually used).
    pub min_samples: usize,
    /// Minimum real span between the oldest and newest observation used.
    pub min_span_ns: u64,
}

impl Default for VolatilityWindow {
    fn default() -> Self {
        Self {
            window_ns: DEFAULT_WINDOW_NS,
            min_samples: DEFAULT_MIN_SAMPLES,
            min_span_ns: DEFAULT_MIN_SPAN_NS,
        }
    }
}

/// Per-symbol rolling store of real observations.
#[derive(Debug, Clone)]
pub struct SeriesStore {
    capacity: usize,
    series: HashMap<String, VecDeque<Sample>>,
}

impl Default for SeriesStore {
    fn default() -> Self {
        Self::new(DEFAULT_CAPACITY)
    }
}

impl SeriesStore {
    pub fn new(capacity: usize) -> Self {
        Self {
            capacity: capacity.max(2),
            series: HashMap::new(),
        }
    }

    pub fn capacity(&self) -> usize {
        self.capacity
    }

    /// Record one observation. Returns `true` iff it was accepted.
    ///
    /// Rejected (returns `false`, stores nothing): non-finite or `<= 0` price;
    /// a timestamp not strictly greater than the last accepted one for that
    /// symbol. Rejection is silent by design — the caller already decided the
    /// price was live, and a dropped junk frame must not become an observation.
    pub fn observe(&mut self, symbol: &str, ts_ns: u64, price_usd: f64) -> bool {
        if !(price_usd.is_finite() && price_usd > 0.0) {
            return false;
        }
        let ring = self.series.entry(symbol.to_owned()).or_default();
        if let Some(last) = ring.back() {
            if ts_ns <= last.ts_ns {
                return false;
            }
        }
        ring.push_back(Sample { ts_ns, price_usd });
        while ring.len() > self.capacity {
            ring.pop_front();
        }
        true
    }

    /// Number of stored observations for `symbol`.
    pub fn len(&self, symbol: &str) -> usize {
        self.series.get(symbol).map_or(0, |r| r.len())
    }

    pub fn is_empty(&self) -> bool {
        self.series.values().all(|r| r.is_empty())
    }

    /// The observations of `symbol` that fall inside `w.window_ns` of `now_ns`,
    /// oldest first. Stale entries are also pruned from the ring so the store
    /// does not accumulate history nobody will read.
    pub fn windowed(&mut self, symbol: &str, w: VolatilityWindow, now_ns: u64) -> Vec<Sample> {
        let Some(ring) = self.series.get_mut(symbol) else {
            return Vec::new();
        };
        while let Some(front) = ring.front() {
            // `saturating_sub`: a timestamp in the future (clock skew) is kept,
            // not silently reinterpreted as ancient history.
            if now_ns.saturating_sub(front.ts_ns) > w.window_ns {
                ring.pop_front();
            } else {
                break;
            }
        }
        ring.iter().copied().collect()
    }
}

/// Realized volatility of an ordered, already-windowed series.
///
/// Returns the sample standard deviation (`n-1`) of the log returns between
/// consecutive observations, or `None` when the data cannot support the
/// measurement: fewer than `min_samples` usable points, a real span below
/// `min_span_ns`, or any point that is not a finite positive price.
///
/// This function does NOT window by time — callers pass a series already
/// filtered to the window (see `SeriesStore::windowed`). It is pure: same input,
/// same output, no clock, no I/O.
pub fn realized_volatility(samples: &[Sample], w: VolatilityWindow) -> Option<f64> {
    if samples.len() < w.min_samples || samples.len() < 3 {
        return None;
    }
    // Every point must be usable; one bad price invalidates the series rather
    // than being skipped (skipping would silently measure a different series
    // than the caller handed us).
    if !samples
        .iter()
        .all(|s| s.price_usd.is_finite() && s.price_usd > 0.0)
    {
        return None;
    }
    let first = samples.first()?;
    let last = samples.last()?;
    if last.ts_ns.saturating_sub(first.ts_ns) < w.min_span_ns {
        return None; // a real axis is mandatory, not decorative
    }

    let rets: Vec<f64> = samples
        .windows(2)
        .map(|pair| (pair[1].price_usd / pair[0].price_usd).ln())
        .collect();
    if rets.len() < 2 || !rets.iter().all(|r| r.is_finite()) {
        return None;
    }
    let n = rets.len() as f64;
    let mean = rets.iter().sum::<f64>() / n;
    let var = rets.iter().map(|r| (r - mean).powi(2)).sum::<f64>() / (n - 1.0);
    let sd = var.sqrt();
    // A tiny negative variance from floating-point cancellation is clamped to
    // zero; anything else non-finite means the arithmetic itself failed.
    if !var.is_finite() || var < 0.0 {
        if var.is_finite() {
            return Some(0.0);
        }
        return None;
    }
    sd.is_finite().then_some(sd)
}

/// Convenience: window the store and estimate in one step.
pub fn volatility_for(
    store: &mut SeriesStore,
    symbol: &str,
    w: VolatilityWindow,
    now_ns: u64,
) -> Option<f64> {
    let series = store.windowed(symbol, w, now_ns);
    realized_volatility(&series, w)
}
