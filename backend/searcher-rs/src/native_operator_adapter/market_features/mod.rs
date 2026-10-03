//! MARKET-FEATURES-01 — real producers for the `MarketState.features` keys the
//! native operators read but nothing wrote.
//!
//! # Why this module exists
//!
//! `MarketState.features` (`math-engine/src/operators/mod.rs:98`) is the only
//! channel through which the searcher hands non-price context to the operators
//! and to `RegimeRouter`. Five keys have readers
//! (`regime_router.rs:107,189,197,198,208`):
//!
//! `volatility` · `health_factor` · `oracle_price` · `onchain_price` · `parity_deviation`
//!
//! On this base only TWO of them are written anywhere, and neither by the
//! searcher's general feature path:
//!
//! * `parity_deviation` ← `math_evidence::regime_features_from_redis`
//!   (`math_evidence.rs:249-251`), from Redis `arbx:token_prices:<chain>`.
//! * `health_factor` ← the cached liquidation indexer, inserted at
//!   `orchestrator.rs:708-710`.
//!
//! So the keys genuinely missing a producer are exactly **three**:
//! `volatility`, `oracle_price`, `onchain_price`. This module produces those,
//! and deliberately produces nothing else —
//! `tests::already_produced_keys_are_never_emitted_here` fails if someone later
//! starts duplicating `parity_deviation` or `health_factor` here.
//!
//! `arbitrage_gap` is the fourth candidate in the original problem statement and
//! it is **not a feature at all**: FEATURES-01b made `RegimeRouter::analyze`
//! compute it internally from `pair_keys` (`regime_router.rs:153-186`), and no
//! `features.get("arbitrage_gap")` exists in the repo. Writing it would be dead
//! code. The reasoning is recorded as data in
//! [`contract::Owner::NoConsumer`] rather than in a comment nobody re-reads.
//!
//! # Correctness doctrine (R8 / RULE 00)
//!
//! * **Absence is absence.** A key whose datum is unavailable is **not
//!   inserted**. Never `0.0`, never a mean, never a proxy standing in for a
//!   measurement. `RegimeRouter` already handles `None` as "not computed".
//! * **`Some(0.0)` means computed and exactly zero.** A flat price series over a
//!   real span is zero volatility — that is a measurement and it IS inserted.
//!   Both directions are tested.
//! * **Pure entry point.** [`produce`] takes everything as arguments (including
//!   `now_ns`) and performs no I/O, so every branch is testable with fixtures.
//!   [`produce_from_global`] is the thin adapter over the process-wide
//!   observation store, which takes the `PriceBus` as a PARAMETER.
//!
//! # Integration status — PENDING, declared (see also `docs/verification/`)
//!
//! This module is **not yet wired** into the live path, because every call site
//! that could wire it is in the frozen hot zone. The exact connection is:
//!
//! ```text
//! backend/searcher-rs/src/orchestrator.rs:705
//!   AFTER:  let mut features =
//!               crate::math_evidence::regime_features_from_redis(&mut math_redis, chain_id).await;
//!   ADD:    features.extend(crate::native_operator_adapter::market_features::produce_from_global(
//!               crate::price_bus_global::get().as_deref(), &symbol, onchain_price_usd, now_ns));
//! ```
//!
//! (`symbol` = the route's base token; `onchain_price_usd` = the `price_matrix`
//! entry for that pair — both already computed in
//! `math_evidence::build_market_state`.) The equivalent addition can also be made
//! inside `math_evidence::regime_features_from_redis`, whose signature would then
//! need the symbol and the on-chain price. Neither file was touched: both are
//! hot-zone.
//!
//! Note the `crate::price_bus_global::get()` in that snippet: it is resolved at
//! the CALL SITE, and that is deliberate. `price_bus_global` is declared in
//! `main.rs:137`, so it exists only in the BIN target, while this module is
//! compiled into BOTH targets (`lib.rs:37` and `main.rs:200`). Referencing it
//! here breaks the lib build — measured, not assumed:
//! `cargo check -p searcher-rs --lib` →
//! `error[E0433]: failed to resolve: could not find price_bus_global in the crate
//! root` at `market_features/mod.rs:211`. The bus is therefore a PARAMETER: the
//! module stays target-agnostic and unit-testable without booting `main.rs`.

pub mod contract;
pub mod oracle_bias;
pub mod realized_volatility;

#[cfg(test)]
mod tests;

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

use shared_rs::price_bus::{PriceBus, PriceBusConfig, PriceSnapshot};

pub use contract::{contract_for, owned_contracts, FeatureContract, Owner, CONTRACTS, OWNED_KEYS};
pub use oracle_bias::{fresh_anchor_usd, oracle_bias_pair, ONCHAIN_PRICE_KEY, ORACLE_PRICE_KEY};
pub use realized_volatility::{
    realized_volatility, volatility_for, Sample, SeriesStore, VolatilityWindow, DEFAULT_CAPACITY,
    DEFAULT_MIN_SAMPLES, DEFAULT_MIN_SPAN_NS, DEFAULT_WINDOW_NS,
};

/// `MarketState.features` key for realized volatility.
pub const VOLATILITY_KEY: &str = "volatility";

/// Producer tunables. A struct so callers can shrink windows in tests without
/// touching globals.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct FeatureConfig {
    pub volatility: VolatilityWindow,
}

/// One production request — everything the producers need about the candidate.
#[derive(Debug, Clone, Copy)]
pub struct FeatureRequest<'a> {
    /// Token symbol of the pair being evaluated (case-insensitive).
    pub symbol: &'a str,
    /// DEX/on-chain price (USD per token) for that pair, from the real pool
    /// reserves already normalised in `build_market_state`. `None` when no leg
    /// was priced — which is what makes `oracle_price`/`onchain_price` absent.
    pub onchain_price_usd: Option<f64>,
    /// Fused live USD price of the same token, already resolved by the caller
    /// from the `PriceBus` (`PriceView::price_usd`). `None` = no live price this
    /// tick, so no observation is recorded (the series simply does not advance;
    /// it is never padded).
    pub live_price_usd: Option<f64>,
}

/// The oracle-side inputs, borrowed from the process `PriceBus` for the duration
/// of one `produce` call.
#[derive(Clone, Copy)]
pub struct OracleSources<'a> {
    pub snapshot: &'a PriceSnapshot,
    pub cfg: &'a PriceBusConfig,
}

/// **Pure producer.** Returns the feature map for one candidate.
///
/// `store` is the rolling observation accumulator (the only mutable state, and
/// it is passed in explicitly so tests own it). Returns an empty map when no key
/// is computable — which is the correct, honest answer, not an error.
pub fn produce(
    store: &mut SeriesStore,
    cfg: &FeatureConfig,
    req: &FeatureRequest<'_>,
    oracle: Option<OracleSources<'_>>,
    now_ns: u64,
) -> HashMap<String, f64> {
    let mut out = HashMap::new();
    let symbol = normalize_symbol(req.symbol);

    // ── 1) Self-sampling ───────────────────────────────────────────────────
    // Each call contributes ONE real observation of the fused live price. This
    // is what gives `volatility` a real time axis without a second wiring site:
    // the caller supplies the price it already has, we timestamp it at the
    // instant of the request. No live price -> no observation (never padded
    // with the on-chain price, which is a different quantity on a different
    // clock).
    if let Some(price) = req.live_price_usd {
        let _ = store.observe(&symbol, now_ns, price);
    }

    // ── 2) volatility ─────────────────────────────────────────────────────
    if let Some(v) = volatility_for(store, &symbol, cfg.volatility, now_ns) {
        out.insert(VOLATILITY_KEY.to_owned(), v);
    }

    // ── 3) oracle_price / onchain_price (both or neither) ─────────────────
    if let Some(src) = oracle {
        if let Some((oracle_price, onchain_price)) = oracle_bias_pair(
            src.snapshot,
            src.cfg,
            &symbol,
            req.onchain_price_usd,
            now_ns,
        ) {
            out.insert(ORACLE_PRICE_KEY.to_owned(), oracle_price);
            out.insert(ONCHAIN_PRICE_KEY.to_owned(), onchain_price);
        }
    }

    out
}

/// `trim().to_ascii_uppercase()` — one canonical symbol spelling so `"eth"` and
/// `"ETH"` accumulate into the SAME series instead of two half-empty ones.
fn normalize_symbol(symbol: &str) -> String {
    symbol.trim().to_ascii_uppercase()
}

/// Process-wide observation store. A singleton because the series must survive
/// across candidates — a per-call store could never span time.
static STORE: OnceLock<Mutex<SeriesStore>> = OnceLock::new();

fn global_store() -> &'static Mutex<SeriesStore> {
    STORE.get_or_init(|| Mutex::new(SeriesStore::default()))
}

/// Adapter over the process-wide observation store: resolves the fused live
/// price from the given `PriceBus`, reads its Chainlink anchor and produces the
/// feature map. **This is the entry point the pending integration calls.**
///
/// `bus` is `crate::price_bus_global::get().as_deref()` at a bin-target call
/// site. It is a parameter rather than a global lookup on purpose — see the
/// integration note in the module docs: `price_bus_global` does not exist in the
/// lib target, where this module is also compiled and tested.
///
/// Without a bus (e.g. a unit test that never boots `main.rs`) this does NOT
/// panic and does NOT fabricate: the oracle pair is simply absent and no
/// observation is recorded.
///
/// # Clock contract
///
/// `now_ns` must be the CURRENT instant on the same wall clock the `PriceBus`
/// uses (`SystemTime` since the Unix epoch). Two different readings of "now"
/// meet inside one call: `PriceView::price_usd` uses its own internal clock,
/// while anchor freshness here is judged against the injected `now_ns`. In
/// production these are the same instant and agree. Passing a historical
/// `now_ns` (as a test may, for determinism) can therefore yield a fresh anchor
/// with no fused live price — which is exactly what the
/// `produce_from_global_produces_the_pair_from_a_real_bus` test exercises, and
/// why that test asserts on the oracle pair rather than on `volatility`.
pub fn produce_from_global(
    bus: Option<&PriceBus>,
    symbol: &str,
    onchain_price_usd: Option<f64>,
    now_ns: u64,
) -> HashMap<String, f64> {
    produce_from_global_with(
        &FeatureConfig::default(),
        bus,
        symbol,
        onchain_price_usd,
        now_ns,
    )
}

/// [`produce_from_global`] with explicit tunables.
pub fn produce_from_global_with(
    cfg: &FeatureConfig,
    bus: Option<&PriceBus>,
    symbol: &str,
    onchain_price_usd: Option<f64>,
    now_ns: u64,
) -> HashMap<String, f64> {
    // Poisoning must not silently disable a producer: recover the inner guard
    // exactly as `PriceBus::sample_divergence` does (price_bus.rs:334-337).
    let mut guard = match global_store().lock() {
        Ok(g) => g,
        Err(poisoned) => poisoned.into_inner(),
    };
    match bus {
        Some(bus) => {
            let view = bus.view();
            let req = FeatureRequest {
                symbol,
                onchain_price_usd,
                live_price_usd: view.price_usd(symbol),
            };
            produce(
                &mut guard,
                cfg,
                &req,
                Some(OracleSources {
                    snapshot: view.snapshot(),
                    cfg: bus.config(),
                }),
                now_ns,
            )
        }
        None => {
            let req = FeatureRequest {
                symbol,
                onchain_price_usd,
                live_price_usd: None,
            };
            produce(&mut guard, cfg, &req, None, now_ns)
        }
    }
}
