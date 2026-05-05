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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::trading_config::{GasPriceStrategy, TradingConfigState};
    use chrono::Utc;
    use std::collections::HashMap;

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
}
