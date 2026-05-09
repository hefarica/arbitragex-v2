//! PriceOracle — per-token USD valuation for the spine evaluator.
//!
//! Replaces the buggy `TradingConfigState::profit_token_to_usd(amount)` which
//! treated every token as base-token-priced (BUG-2, see anti_reincidencia.md
//! Incidente #7). The oracle resolves prices by token symbol with three
//! priority tiers, then fails honestly:
//!
//!   1. Match `config.base_token_symbol` (case-insensitive) → `base_token_price_usd`
//!   2. Lookup in `config.token_prices_usd` map (operator-managed override)
//!   3. Hardcoded consensus stablecoin list → $1.00
//!   4. Otherwise → `None` (caller MUST reject — no fabricated default)
//!
//! The trait abstraction lets future implementations swap to Chainlink,
//! TWAP, or external feeds without touching callers in the spine.
//!
//! Doctrine: R8 Fail-Honest. An unknown token returns `None` so the spine
//! rejects the opportunity with `RejectReason::UnknownTokenPrice` instead
//! of inventing a price. Operator sees the gap explicitly via dashboard /
//! heartbeat counters and populates `token_prices_usd` to close it.

use crate::trading_config::TradingConfigState;
use std::collections::HashMap;

/// Hard-coded set of widely-trusted stablecoins valued at $1.00 by default.
/// Symbols are matched case-insensitively at the call site; this list is
/// canonical uppercase.
///
/// Selection criteria (locked-in 2026-05-05 — operator review required to
/// add/remove):
///   - Backed by reserves OR overcollateralized (no algorithmic stables).
///   - >$50M circulating supply at time of inclusion.
///   - Documented redemption mechanism.
///
/// Excluded from defaults — operator must set explicit price if they want to
/// trade these:
///   - USDe (Ethena, algorithmic delta-neutral, depeg episodes)
///   - MIM (Magic Internet Money, exploit history)
///   - crvUSD, GHO (newer; operator should review trust manually)
pub fn is_known_stablecoin(symbol_upper: &str) -> bool {
    matches!(
        symbol_upper,
        "USDC"
            | "USDT"
            | "DAI"
            | "BUSD"
            | "FRAX"
            | "LUSD"
            | "USDP"
            | "TUSD"
            | "GUSD"
            | "USDD"
            | "PYUSD"
    )
}

/// Per-token USD price resolution. `None` means the token is outside the
/// operator's universe — callers MUST treat this as a hard reject reason
/// (see `RejectReason::UnknownTokenPrice`) rather than substituting a default.
pub trait PriceOracle {
    /// Returns USD price PER UNIT of the token (multiply by token amount to
    /// get total USD value). `token_id` may be a symbol or hex address;
    /// implementations should normalize internally.
    fn price_usd(&self, token_id: &str) -> Option<f64>;
}

/// Default oracle backed by `TradingConfigState`. Resolves base token, then
/// operator-supplied token prices, then hardcoded stablecoin defaults.
/// Returns `None` for any unknown symbol or hex address.
pub struct ConfigPriceOracle<'a> {
    config: &'a TradingConfigState,
}

impl<'a> ConfigPriceOracle<'a> {
    pub fn new(config: &'a TradingConfigState) -> Self {
        Self { config }
    }
}

impl<'a> PriceOracle for ConfigPriceOracle<'a> {
    fn price_usd(&self, token_id: &str) -> Option<f64> {
        let upper = token_id.trim().to_ascii_uppercase();
        if upper.is_empty() {
            return None;
        }

        // Tier 1 — base token symbol (operator's reference asset).
        if upper == self.config.base_token_symbol.to_ascii_uppercase() {
            return Some(self.config.base_token_price_usd);
        }

        // Tier 2 — operator-managed map. Case-insensitive lookup; the operator
        // may store keys in any case ("WBTC", "wbtc", "WbTc") — all match.
        for (sym, price) in self.config.token_prices_usd.iter() {
            if sym.to_ascii_uppercase() == upper {
                return Some(*price);
            }
        }

        // Tier 3 — hardcoded stablecoin defaults at $1.00. Operator can
        // override any of these by adding the symbol to `token_prices_usd`
        // (Tier 2 takes precedence — checked above).
        if is_known_stablecoin(&upper) {
            return Some(1.0);
        }

        // Tier 4 — fail-honest. No fabricated default.
        None
    }
}

/// Composes multiple `PriceOracle` implementations into a single cascade.
/// Tries each child oracle in declaration order; returns the first `Some(_)`
/// or `None` if every child misses. Empty / whitespace `token_id` short-
/// circuits to `None` so child oracles never see degenerate input.
///
/// Production layout (after Sprint X):
///   1. `RedisCachedPriceOracle` — sub-ms snapshot of `arbx:token_prices:1`,
///      populated every 30s by `searcher-rs::workers::price_worker` from
///      Alchemy Token Prices + Coingecko fallback.
///   2. `ConfigPriceOracle` — operator's manual overrides + base token +
///      hardcoded stablecoins. Last-resort fallback when no live source
///      has a price (e.g. brand-new token, both feeds down, boot before
///      first worker tick).
///   3. (no third tier) — falls through to `None` → `RejectReason::UnknownTokenPrice`.
///
/// **Doctrine**: cascading does NOT hide failure. If every tier returns
/// `None` the cascade returns `None` and the spine rejects fail-honestly.
/// We never substitute a "best guess" or stale value (R8).
///
/// The `'a` lifetime allows borrowed inner oracles (e.g. `ConfigPriceOracle<'a>`
/// borrowing `&'a TradingConfigState`). For owned-only cascades use `'static`.
pub struct CascadePriceOracle<'a> {
    oracles: Vec<Box<dyn PriceOracle + Send + Sync + 'a>>,
}

impl<'a> CascadePriceOracle<'a> {
    /// Build a cascade from oracles in priority order (highest first).
    /// Empty vec is permitted but produces an oracle that always returns
    /// `None` — useful as a degenerate placeholder during boot.
    pub fn new(oracles: Vec<Box<dyn PriceOracle + Send + Sync + 'a>>) -> Self {
        Self { oracles }
    }
}

impl<'a> PriceOracle for CascadePriceOracle<'a> {
    fn price_usd(&self, token_id: &str) -> Option<f64> {
        // Short-circuit on empty input. Matches `ConfigPriceOracle` semantics
        // and avoids inner oracles having to re-validate.
        if token_id.trim().is_empty() {
            return None;
        }
        for oracle in &self.oracles {
            if let Some(p) = oracle.price_usd(token_id) {
                return Some(p);
            }
        }
        None
    }
}

/// Sync `PriceOracle` over a pre-fetched `HashMap<String, f64>` snapshot.
///
/// **Why "snapshot" not "client"**: the spine evaluator is sync (per-opportunity
/// hot path). Making it `async` to round-trip Redis on every lookup would
/// cascade refactors through `evaluate()` → `score()` → caller chains. Instead,
/// the spine fetches the latest snapshot ONCE per evaluation tick (or on a TTL
/// timer) via `snapshot_from_redis()` and queries it sync at micro-second
/// latency. Staleness ≤ refresh period (30s by default) — acceptable for
/// USD valuation since intra-block price movement is bounded.
///
/// The cache is populated asynchronously by
/// `searcher-rs::workers::price_worker` from Alchemy Token Prices API +
/// Coingecko fallback. Redis hash key: `arbx:token_prices:1`. Field key
/// = uppercase token symbol. Value = USD price as `f64`.
///
/// **R8 enforcement**: snapshot construction filters out non-finite (NaN /
/// Inf) and non-positive prices so the spine never multiplies by garbage.
/// External feeds occasionally emit those (provider bugs, 0-divisions on
/// illiquid pools); silently dropping them is honest because the cache miss
/// then falls through to the next cascade tier or to `UnknownTokenPrice`.
pub struct RedisCachedPriceOracle {
    snapshot: HashMap<String, f64>,
}

impl RedisCachedPriceOracle {
    /// Build from an in-memory snapshot. Symbol keys are normalized to
    /// uppercase; non-finite / non-positive prices are dropped silently
    /// (caller's snapshot may include junk from upstream feeds; oracle
    /// will return `None` for those tokens, letting the cascade fall
    /// through honestly).
    pub fn from_snapshot(snapshot: HashMap<String, f64>) -> Self {
        let mut clean = HashMap::with_capacity(snapshot.len());
        for (k, v) in snapshot {
            if v.is_finite() && v > 0.0 {
                clean.insert(k.to_ascii_uppercase(), v);
            }
        }
        Self { snapshot: clean }
    }

    /// Fetch the entire `arbx:token_prices:<chain_id>` hash from Redis and
    /// return a fresh oracle wrapping the snapshot.
    ///
    /// Returns an oracle with an EMPTY snapshot (never an error) when:
    ///   - The Redis hash is absent (worker hasn't ticked yet at boot).
    ///   - Redis is unreachable.
    ///   - Any field fails to parse as `f64`.
    ///
    /// Rationale: the spine cascade always falls through to ConfigPriceOracle
    /// on cache miss, so an empty cache snapshot degrades gracefully (operator
    /// overrides + stablecoins still work). Surfacing the Redis error here
    /// would force every caller to handle it; instead, log + return empty,
    /// and let the caller's downstream `UnknownTokenPrice` rejections signal
    /// the problem to the operator dashboard.
    pub async fn snapshot_from_redis(
        redis: &mut redis::aio::ConnectionManager,
        chain_id: u64,
    ) -> Self {
        use redis::AsyncCommands;
        let key = redis_token_prices_key(chain_id);
        let raw: Result<HashMap<String, String>, redis::RedisError> = redis.hgetall(&key).await;
        let map = match raw {
            Ok(m) => m,
            Err(e) => {
                tracing::warn!(
                    event = "price_oracle.redis_hgetall_failed",
                    chain_id,
                    key = %key,
                    error = %e,
                    "returning empty snapshot — cascade falls through to ConfigPriceOracle"
                );
                HashMap::new()
            }
        };
        let mut parsed: HashMap<String, f64> = HashMap::with_capacity(map.len());
        for (sym, val_str) in map {
            match val_str.parse::<f64>() {
                Ok(v) => {
                    parsed.insert(sym, v);
                }
                Err(e) => {
                    tracing::debug!(
                        event = "price_oracle.parse_failed",
                        chain_id,
                        symbol = %sym,
                        raw = %val_str,
                        error = %e,
                        "dropping malformed cached price"
                    );
                }
            }
        }
        Self::from_snapshot(parsed)
    }

    /// Number of usable entries in the snapshot (post-validation). Useful
    /// for heartbeat / observability ("how many tokens are priced live?").
    pub fn len(&self) -> usize {
        self.snapshot.len()
    }

    /// True when the snapshot is empty (worker hasn't ticked OR Redis miss).
    pub fn is_empty(&self) -> bool {
        self.snapshot.is_empty()
    }

    /// Borrow the underlying validated snapshot. Used by callers that need
    /// to plug the same data into a different consumer (e.g. the spine
    /// evaluator's `with_cache` constructor) without re-fetching from Redis.
    pub fn snapshot(&self) -> &HashMap<String, f64> {
        &self.snapshot
    }

    /// Consume the oracle and return its validated snapshot. Same use case
    /// as `snapshot()` but transfers ownership when the caller doesn't need
    /// the oracle again.
    pub fn into_snapshot(self) -> HashMap<String, f64> {
        self.snapshot
    }
}

impl PriceOracle for RedisCachedPriceOracle {
    fn price_usd(&self, token_id: &str) -> Option<f64> {
        let upper = token_id.trim().to_ascii_uppercase();
        if upper.is_empty() {
            return None;
        }
        self.snapshot.get(&upper).copied()
    }
}

/// Canonical Redis key holding the live token-prices snapshot for a chain.
/// Hash schema: field = uppercase symbol, value = USD price as decimal string.
/// Populated by `searcher-rs::workers::price_worker`; consumed by
/// `RedisCachedPriceOracle::snapshot_from_redis`.
pub fn redis_token_prices_key(chain_id: u64) -> String {
    format!("arbx:token_prices:{chain_id}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::trading_config::{GasPriceStrategy, TradingConfigState};
    use chrono::Utc;

    fn cfg_with_prices(prices: HashMap<String, f64>) -> TradingConfigState {
        TradingConfigState {
            chain_id: 1,
            capital_usd: 1000.0,
            base_token_symbol: "WETH".into(),
            base_token_price_usd: 2500.0,
            allowed_token_symbols: vec![
                "WETH".into(),
                "USDC".into(),
                "WBTC".into(),
                "UNI".into(),
            ],
            token_prices_usd: prices,
            simulation_capital_usd: None,
            simulation_per_token_amounts_usd: HashMap::new(),
            simulation_per_strategy_caps_usd: HashMap::new(),
            simulation_target_profit_usd: None,
            simulation_target_roi_pct: None,
            min_profit_usd: 1.0,
            min_roi_pct: 0.1,
            min_landing_probability: 0.5,
            min_liquidity_confidence: 0.7,
            max_token_risk_score: 1.0,
            gas_price_strategy: GasPriceStrategy::Fixed,
            fixed_gas_price_gwei: Some(20.0),
            gas_estimate_units: 200_000,
            max_slippage_pct: 0.5,
            failure_risk_buffer_pct: 0.001,
            flashloan_fee_pct: 0.0009,
            enabled_strategies: vec!["dex_arb_v2v2".into()],
            capital_cost_rate_annual_pct: 0.0,
            ops_overhead_usd_per_attempt: 0.01,
            enabled: true,
            updated_at: Utc::now(),
            updated_by: None,
        }
    }

    #[test]
    fn resolves_base_token_symbol_case_insensitive() {
        let c = cfg_with_prices(HashMap::new());
        let oracle = ConfigPriceOracle::new(&c);
        assert_eq!(oracle.price_usd("WETH"), Some(2500.0));
        assert_eq!(oracle.price_usd("weth"), Some(2500.0));
        assert_eq!(oracle.price_usd("Weth"), Some(2500.0));
        assert_eq!(oracle.price_usd("  WETH  "), Some(2500.0)); // trim whitespace
    }

    #[test]
    fn resolves_operator_token_price_map() {
        let mut prices = HashMap::new();
        prices.insert("WBTC".into(), 95_000.0);
        prices.insert("UNI".into(), 8.5);
        let c = cfg_with_prices(prices);
        let oracle = ConfigPriceOracle::new(&c);
        assert_eq!(oracle.price_usd("WBTC"), Some(95_000.0));
        assert_eq!(oracle.price_usd("uni"), Some(8.5));
    }

    #[test]
    fn defaults_known_stablecoins_to_one_dollar() {
        let c = cfg_with_prices(HashMap::new());
        let oracle = ConfigPriceOracle::new(&c);
        assert_eq!(oracle.price_usd("USDC"), Some(1.0));
        assert_eq!(oracle.price_usd("usdt"), Some(1.0));
        assert_eq!(oracle.price_usd("DAI"), Some(1.0));
        assert_eq!(oracle.price_usd("FRAX"), Some(1.0));
        assert_eq!(oracle.price_usd("PYUSD"), Some(1.0));
        assert_eq!(oracle.price_usd("LUSD"), Some(1.0));
    }

    #[test]
    fn operator_override_takes_precedence_over_stablecoin_default() {
        // Defensive use case: operator wants to model a temporary depeg.
        // E.g. FRAX trading at $0.985 during stress — operator sets it
        // explicitly so opportunity sizing reflects reality, not the
        // optimistic $1 default.
        let mut prices = HashMap::new();
        prices.insert("FRAX".into(), 0.985);
        let c = cfg_with_prices(prices);
        let oracle = ConfigPriceOracle::new(&c);
        assert_eq!(oracle.price_usd("FRAX"), Some(0.985));
    }

    #[test]
    fn returns_none_for_unknown_token() {
        let c = cfg_with_prices(HashMap::new());
        let oracle = ConfigPriceOracle::new(&c);
        assert_eq!(oracle.price_usd("PEPE"), None);
        assert_eq!(oracle.price_usd("UNKNOWN"), None);
        // Hex addresses (caller passed when meta cache was empty) — also None
        // because oracle indexes by symbol. Caller treats this as REJECT.
        assert_eq!(oracle.price_usd("0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2"), None);
    }

    #[test]
    fn returns_none_for_empty_or_whitespace_input() {
        let c = cfg_with_prices(HashMap::new());
        let oracle = ConfigPriceOracle::new(&c);
        assert_eq!(oracle.price_usd(""), None);
        assert_eq!(oracle.price_usd("   "), None);
        assert_eq!(oracle.price_usd("\t\n"), None);
    }

    #[test]
    fn is_known_stablecoin_covers_locked_trust_list() {
        // The trust list is locked-in policy (2026-05-05). Adding/removing
        // requires explicit governance update with reasoning. This test
        // documents the canonical list AND the explicitly-excluded set.
        for s in [
            "USDC", "USDT", "DAI", "BUSD", "FRAX", "LUSD", "USDP", "TUSD",
            "GUSD", "USDD", "PYUSD",
        ] {
            assert!(is_known_stablecoin(s), "expected {s} in trust list");
        }
        // Excluded by policy — operator must set explicit price.
        for s in ["USDE", "MIM", "CRVUSD", "GHO", "USDX"] {
            assert!(
                !is_known_stablecoin(s),
                "{s} should NOT be in trust list (operator must set explicitly)",
            );
        }
    }

    // ----------------------------------------------------------------
    // CascadePriceOracle — wraps multiple oracles in priority order
    // ----------------------------------------------------------------

    /// Trivial test oracle that returns a fixed map. Lets the cascade tests
    /// assemble arbitrary tier orderings without depending on real cache /
    /// config state.
    struct StubOracle(HashMap<String, f64>);

    impl PriceOracle for StubOracle {
        fn price_usd(&self, token_id: &str) -> Option<f64> {
            let upper = token_id.trim().to_ascii_uppercase();
            if upper.is_empty() {
                return None;
            }
            self.0.get(&upper).copied()
        }
    }

    fn stub(prices: &[(&str, f64)]) -> StubOracle {
        let mut m = HashMap::new();
        for (k, v) in prices {
            m.insert((*k).to_string(), *v);
        }
        StubOracle(m)
    }

    #[test]
    fn cascade_returns_first_match() {
        // First oracle has WETH; cascade returns it without consulting later tiers.
        let cascade = CascadePriceOracle::new(vec![
            Box::new(stub(&[("WETH", 2500.0)])),
            Box::new(stub(&[("WETH", 9999.0)])), // would override if reached — must NOT
        ]);
        assert_eq!(cascade.price_usd("WETH"), Some(2500.0));
    }

    #[test]
    fn cascade_falls_through_to_later_oracle_on_miss() {
        // First oracle misses; cascade tries the second.
        let cascade = CascadePriceOracle::new(vec![
            Box::new(stub(&[("WETH", 2500.0)])),
            Box::new(stub(&[("PEPE", 0.0001)])),
        ]);
        assert_eq!(cascade.price_usd("PEPE"), Some(0.0001));
    }

    #[test]
    fn cascade_returns_none_if_all_miss() {
        let cascade = CascadePriceOracle::new(vec![
            Box::new(stub(&[("WETH", 2500.0)])),
            Box::new(stub(&[("USDC", 1.0)])),
        ]);
        assert_eq!(cascade.price_usd("UNKNOWN"), None);
    }

    #[test]
    fn cascade_with_no_oracles_returns_none() {
        // Empty cascade is a degenerate but legal config — must return None,
        // never panic. R8 fail-honest at the most extreme edge.
        let cascade = CascadePriceOracle::new(vec![]);
        assert_eq!(cascade.price_usd("WETH"), None);
    }

    #[test]
    fn cascade_propagates_empty_input_correctly() {
        // Empty / whitespace inputs must short-circuit to None without
        // consulting any oracle (matches ConfigPriceOracle semantics).
        let cascade = CascadePriceOracle::new(vec![Box::new(stub(&[("WETH", 2500.0)]))]);
        assert_eq!(cascade.price_usd(""), None);
        assert_eq!(cascade.price_usd("   "), None);
        assert_eq!(cascade.price_usd("\t\n"), None);
    }

    #[test]
    fn cascade_preserves_config_oracle_internal_priority() {
        // The cascade does NOT flatten internal priorities of its inner oracles.
        // ConfigPriceOracle's tier 2 (operator override) wins over tier 3
        // (stablecoin default) INSIDE that oracle — cascade just composes oracles.
        let mut prices = HashMap::new();
        prices.insert("FRAX".into(), 0.985); // operator depeg override
        let cfg = cfg_with_prices(prices);
        let cascade = CascadePriceOracle::new(vec![Box::new(ConfigPriceOracle::new(&cfg))]);
        // FRAX hits operator override INSIDE ConfigPriceOracle, not stablecoin default.
        assert_eq!(cascade.price_usd("FRAX"), Some(0.985));
    }

    #[test]
    fn cascade_with_realistic_two_tier_layout() {
        // Production layout: tier 1 = cache snapshot (background-fetched live
        // prices), tier 2 = ConfigPriceOracle (operator overrides + stablecoins
        // + base token). The cache wins when populated; falls through to
        // operator config when the cache misses.
        let mut cache = HashMap::new();
        cache.insert("WETH".to_string(), 2517.42); // live price beats config base price
        let cache_oracle = RedisCachedPriceOracle::from_snapshot(cache);

        let cfg = cfg_with_prices(HashMap::new());
        let cascade: CascadePriceOracle = CascadePriceOracle::new(vec![
            Box::new(cache_oracle),
            Box::new(ConfigPriceOracle::new(&cfg)),
        ]);

        // WETH: cache hits with the LIVE price, beating ConfigPriceOracle's $2500 base.
        assert_eq!(cascade.price_usd("WETH"), Some(2517.42));
        // USDC: cache miss → falls through to ConfigPriceOracle stablecoin default.
        assert_eq!(cascade.price_usd("USDC"), Some(1.0));
        // PEPE: cache miss + config miss → None (R8 fail-honest).
        assert_eq!(cascade.price_usd("PEPE"), None);
    }

    // ----------------------------------------------------------------
    // RedisCachedPriceOracle — sync snapshot of an async-fetched cache
    // ----------------------------------------------------------------

    #[test]
    fn redis_cached_oracle_returns_cached_price_case_insensitive() {
        let mut snap = HashMap::new();
        snap.insert("WETH".to_string(), 2500.0);
        snap.insert("USDC".to_string(), 1.0001); // live USDC slightly off-peg
        let oracle = RedisCachedPriceOracle::from_snapshot(snap);
        assert_eq!(oracle.price_usd("WETH"), Some(2500.0));
        assert_eq!(oracle.price_usd("weth"), Some(2500.0));
        assert_eq!(oracle.price_usd("Weth"), Some(2500.0));
        assert_eq!(oracle.price_usd("  USDC  "), Some(1.0001));
    }

    #[test]
    fn redis_cached_oracle_returns_none_on_cache_miss() {
        let mut snap = HashMap::new();
        snap.insert("WETH".to_string(), 2500.0);
        let oracle = RedisCachedPriceOracle::from_snapshot(snap);
        // PEPE is not in cache → None (R8: prefer None over fabricated default).
        assert_eq!(oracle.price_usd("PEPE"), None);
    }

    #[test]
    fn redis_cached_oracle_returns_none_for_empty_input() {
        let mut snap = HashMap::new();
        snap.insert("WETH".to_string(), 2500.0);
        let oracle = RedisCachedPriceOracle::from_snapshot(snap);
        assert_eq!(oracle.price_usd(""), None);
        assert_eq!(oracle.price_usd("   "), None);
    }

    #[test]
    fn redis_cached_oracle_empty_snapshot_always_returns_none() {
        // Boot scenario: cache not yet populated (worker hasn't ticked).
        // Oracle MUST return None for everything — let the cascade fall
        // through to ConfigPriceOracle (operator's seed values).
        let oracle = RedisCachedPriceOracle::from_snapshot(HashMap::new());
        assert_eq!(oracle.price_usd("WETH"), None);
        assert_eq!(oracle.price_usd("USDC"), None);
        assert_eq!(oracle.price_usd("ANYTHING"), None);
    }

    #[test]
    fn redis_cached_oracle_rejects_non_finite_prices_at_construction() {
        // Live external feeds occasionally return NaN / Infinity (provider bug,
        // 0-division on illiquid pool, etc.). The snapshot constructor MUST
        // filter those out so the spine never multiplies by NaN. The brief
        // says R8: prefer None over a fake value — non-finite IS a fake value.
        let mut snap = HashMap::new();
        snap.insert("WETH".to_string(), 2500.0);
        snap.insert("BAD1".to_string(), f64::NAN);
        snap.insert("BAD2".to_string(), f64::INFINITY);
        snap.insert("BAD3".to_string(), f64::NEG_INFINITY);
        snap.insert("BAD4".to_string(), -1.0); // negative price = nonsense
        let oracle = RedisCachedPriceOracle::from_snapshot(snap);
        assert_eq!(oracle.price_usd("WETH"), Some(2500.0));
        assert_eq!(oracle.price_usd("BAD1"), None);
        assert_eq!(oracle.price_usd("BAD2"), None);
        assert_eq!(oracle.price_usd("BAD3"), None);
        assert_eq!(oracle.price_usd("BAD4"), None);
    }
}
