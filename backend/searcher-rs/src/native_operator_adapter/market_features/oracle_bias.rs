//! MARKET-FEATURES-01 — `oracle_price` / `onchain_price`: the oracle-bias pair.
//!
//! ## What the only consumer actually needs
//!
//! `RegimeRouter::analyze` (`regime_router.rs:195-203`) reads BOTH keys and
//! derives one scalar:
//!
//! ```text
//! oracle_bias = |oracle_price − onchain_price| / onchain_price
//! ```
//!
//! and `Regime::OracleBias` fires when that exceeds `oracle_bias` (0.2%).
//! `math_evidence.rs:229-233` recorded the gap precisely: the Chainlink anchors
//! live in the **in-process `PriceBus`**, not in Redis — `arbx:quote:anchor:1`
//! was inspected and is graph health (`cross_dex`/`liquidity`/`stability`/
//! `venues`), not oracle prices. "Requiere productor propio." This is it.
//!
//! ## What each side IS (no invention)
//!
//! * `oracle_price` — the Chainlink anchor's own `answer`, already
//!   decimal-adjusted to USD per token by the writer
//!   (`workers/price_worker.rs::fetch_chainlink`, from `latestRoundData()` on the
//!   PG-configured aggregators; `shared-rs/src/price_bus.rs:47-55`). This is the
//!   oracle's published value, read as published.
//! * `onchain_price` — the DEX/on-chain price of the same pair, derived from the
//!   REAL pool reserves normalised by both tokens' decimals in
//!   `math_evidence::build_market_state` — i.e. the value already carried in
//!   `price_matrix`. The repo names this counter-side `dex_price` in
//!   `cartridges/omega_strategy_pack.rhai:518` ("oracle vs dex"), which is the
//!   same comparison under the router's name `onchain_price`.
//!
//! ## Deliberate boundaries
//!
//! * **Both or neither.** The pair is returned as an `Option<(f64, f64)>` and
//!   the caller inserts both keys or none. A lone `oracle_price` computes
//!   nothing (the router needs both) while looking like coverage; a lone
//!   `onchain_price` is already available as `price_matrix`, so re-publishing it
//!   alone would be noise.
//! * **Reading an anchor is not bypassing the freeze.** The bus's divergence
//!   band is a TRADING gate: `PriceView::price_usd` returns `None` when a pair is
//!   `DivergenceFrozen` so nothing trades a depegged feed
//!   (`price_bus.rs:446-458`). This module reads `Anchor.answer` directly, which
//!   is exactly the quantity `oracle_bias` is DEFINED against — the freeze stays
//!   in force for every trading decision, and the frozen flag is itself the
//!   signal this metric surfaces.
//! * **Staleness is per asset class**, mirroring `PriceView::anchor_is_fresh`
//!   (`price_bus.rs:406-413`): Chainlink's heartbeat is ~1 h for volatile assets
//!   and ~24 h for stables (observed USDC at 13.7 h healthy). Using one number
//!   for both would either discard healthy stable feeds or accept dead ones.

use shared_rs::price_bus::{Anchor, PriceBusConfig, PriceSnapshot};

/// `MarketState.features` key for the oracle side.
pub const ORACLE_PRICE_KEY: &str = "oracle_price";
/// `MarketState.features` key for the on-chain DEX side.
pub const ONCHAIN_PRICE_KEY: &str = "onchain_price";

/// Fresh Chainlink anchor price (USD per token) for `symbol`, or `None`.
///
/// Pure: no clock, no I/O — `now_ns` is injected. Rejects an anchor whose
/// `answer` is not finite and positive (a zero oracle answer is a broken feed,
/// not a free asset) and one older than its asset class's heartbeat.
pub fn fresh_anchor_usd(
    snapshot: &PriceSnapshot,
    cfg: &PriceBusConfig,
    symbol: &str,
    now_ns: u64,
) -> Option<f64> {
    let sym = symbol.trim().to_ascii_uppercase();
    if sym.is_empty() {
        return None;
    }
    let anchor: Anchor = *snapshot.chainlink.get(&sym)?;
    if !(anchor.answer.is_finite() && anchor.answer > 0.0) {
        return None;
    }
    let now_secs = now_ns / 1_000_000_000;
    let max_age = if shared_rs::chains::is_stablecoin_symbol(&sym) {
        cfg.anchor_stale_secs_stable
    } else {
        cfg.anchor_stale_secs_volatile
    };
    // `saturating_sub`: an anchor stamped in the future (clock skew between the
    // aggregator and this host) must not wrap into an enormous age.
    if now_secs.saturating_sub(anchor.updated_at) > max_age {
        return None;
    }
    Some(anchor.answer)
}

/// The `(oracle_price, onchain_price)` pair, or `None` when either side is
/// unavailable or unusable. See the module docs for why it is atomic.
pub fn oracle_bias_pair(
    snapshot: &PriceSnapshot,
    cfg: &PriceBusConfig,
    symbol: &str,
    onchain_price_usd: Option<f64>,
    now_ns: u64,
) -> Option<(f64, f64)> {
    let oracle = fresh_anchor_usd(snapshot, cfg, symbol, now_ns)?;
    let onchain = onchain_price_usd?;
    if !(onchain.is_finite() && onchain > 0.0) {
        return None;
    }
    Some((oracle, onchain))
}
