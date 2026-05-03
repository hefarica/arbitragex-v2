//! Dynamic Trading Config Client.
//!
//! Operator-tunable strategy parameters with sub-second hot-reload.
//!
//! Architecture mirrors `paper_mode.rs`:
//!   - Source of truth: PostgreSQL `trading_config` table (one row per chain_id).
//!   - Hot cache: Redis key `arbx:trading_config:<chain_id>` (JSON serialised).
//!   - Change broadcast: Redis pub/sub channel `arbx:trading_config:changes`.
//!   - Local cache: 1s TTL RwLock to avoid Redis round-trips per opportunity.
//!
//! When the operator PUTs a new config via api-server, the api-server writes the
//! row to PG, refreshes the Redis key, and PUBLISHes to the channel. Searcher-rs
//! picks up changes in ≤1s via cache TTL expiry, so capital sizing, gas strategy,
//! and token allowlists react without restart.
//!
//! When a chain has no row, `state(chain_id)` returns `None` and the searcher
//! treats that chain as IDLE — explicit by the no-hardcode doctrine, never
//! invents a default that risks capital.

use chrono::{DateTime, Utc};
use redis::AsyncCommands;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;

pub const TRADING_CONFIG_CHANNEL: &str = "arbx:trading_config:changes";

pub fn redis_key(chain_id: u64) -> String {
    format!("arbx:trading_config:{chain_id}")
}

#[derive(Debug, thiserror::Error)]
pub enum TradingConfigError {
    #[error("redis error: {0}")]
    Redis(#[from] redis::RedisError),
    #[error("serialization error: {0}")]
    Serde(#[from] serde_json::Error),
}

/// Strategy for choosing gas price at scoring time.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum GasPriceStrategy {
    Fixed,
    DynamicBasefeePlusTip,
    Percentile75,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TradingConfigState {
    pub chain_id: u64,

    // Capital & token universe
    pub capital_usd: f64,
    pub base_token_symbol: String,
    pub base_token_price_usd: f64,
    pub allowed_token_symbols: Vec<String>,

    // Profit gate thresholds
    pub min_profit_usd: f64,
    pub min_roi_pct: f64,
    pub min_landing_probability: f64,
    pub min_liquidity_confidence: f64,
    pub max_token_risk_score: f64,

    // Gas & slippage strategy
    pub gas_price_strategy: GasPriceStrategy,
    pub fixed_gas_price_gwei: Option<f64>,
    pub gas_estimate_units: u64,
    pub max_slippage_pct: f64,
    pub failure_risk_buffer_pct: f64,
    pub flashloan_fee_pct: f64,

    // Strategy mix
    pub enabled_strategies: Vec<String>,

    pub enabled: bool,
    pub updated_at: DateTime<Utc>,
    pub updated_by: Option<String>,
}

impl TradingConfigState {
    /// Resolve the gas price (gwei) for this opportunity given live network signals.
    /// `live_basefee_gwei` and `live_p75_tip_gwei` come from the chain client; the
    /// operator's strategy choice picks which input drives the decision.
    pub fn resolve_gas_price_gwei(&self, live_basefee_gwei: f64, live_p75_tip_gwei: f64) -> f64 {
        match self.gas_price_strategy {
            GasPriceStrategy::Fixed => self.fixed_gas_price_gwei.unwrap_or(live_basefee_gwei),
            GasPriceStrategy::DynamicBasefeePlusTip => live_basefee_gwei + live_p75_tip_gwei.max(1.0),
            GasPriceStrategy::Percentile75 => live_p75_tip_gwei.max(live_basefee_gwei),
        }
    }

    /// True if `token` (case-insensitive symbol) is in the operator's allowlist.
    pub fn token_allowed(&self, symbol: &str) -> bool {
        let needle = symbol.to_ascii_uppercase();
        self.allowed_token_symbols
            .iter()
            .any(|s| s.to_ascii_uppercase() == needle)
    }

    /// Convert profit denominated in base token (e.g. WETH) to USD using the
    /// operator-supplied price. Caller is responsible for using a fresh price
    /// when feeding live opportunities (oracle integration is the next sprint).
    pub fn profit_token_to_usd(&self, profit_in_base_token: f64) -> f64 {
        profit_in_base_token * self.base_token_price_usd
    }
}

#[derive(Clone)]
pub struct TradingConfigClient {
    mgr: redis::aio::ConnectionManager,
    cache: Arc<RwLock<HashMap<u64, (TradingConfigState, Instant)>>>,
    cache_ttl: Duration,
}

impl TradingConfigClient {
    pub async fn connect(url: &str) -> Result<Self, TradingConfigError> {
        let client = redis::Client::open(url)?;
        let mgr = client.get_connection_manager().await?;
        Ok(Self::from_manager(mgr))
    }

    /// Reuse an existing `ConnectionManager` instead of opening a new one.
    /// Preferred from services that already hold a Redis connection (scanner,
    /// sim-ctl) — avoids doubling the open-fd count per pod.
    pub fn from_manager(mgr: redis::aio::ConnectionManager) -> Self {
        Self {
            mgr,
            cache: Arc::new(RwLock::new(HashMap::new())),
            cache_ttl: Duration::from_secs(1),
        }
    }

    pub fn with_cache_ttl(mut self, ttl: Duration) -> Self {
        self.cache_ttl = ttl;
        self
    }

    /// Returns `None` if no operator config exists for this chain (idle by design).
    pub async fn state(&self, chain_id: u64) -> Result<Option<TradingConfigState>, TradingConfigError> {
        {
            let g = self.cache.read().await;
            if let Some((s, at)) = g.get(&chain_id) {
                if at.elapsed() < self.cache_ttl {
                    return Ok(Some(s.clone()));
                }
            }
        }
        let mut mgr = self.mgr.clone();
        let raw: Option<String> = mgr.get(redis_key(chain_id)).await?;
        let state = match raw {
            Some(v) => Some(serde_json::from_str::<TradingConfigState>(&v)?),
            None => None,
        };
        if let Some(ref s) = state {
            let mut g = self.cache.write().await;
            g.insert(chain_id, (s.clone(), Instant::now()));
        }
        Ok(state)
    }

    /// Idempotent write — used by api-server when the operator submits new config.
    /// Sets the Redis key AND publishes to the change channel so subscribers
    /// (searcher-rs, sim-ctl) refresh their caches.
    pub async fn put(
        &self,
        state: &TradingConfigState,
    ) -> Result<(), TradingConfigError> {
        let json = serde_json::to_string(state)?;
        let mut mgr = self.mgr.clone();
        let _: () = mgr.set(redis_key(state.chain_id), &json).await?;
        let _: i64 = mgr.publish(TRADING_CONFIG_CHANNEL, &json).await?;
        let mut g = self.cache.write().await;
        g.insert(state.chain_id, (state.clone(), Instant::now()));
        Ok(())
    }

    /// Bust local cache for a chain — used by pub/sub subscribers when a change
    /// notification arrives, so the next `state()` re-fetches from Redis.
    pub async fn invalidate(&self, chain_id: u64) {
        let mut g = self.cache.write().await;
        g.remove(&chain_id);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_state() -> TradingConfigState {
        TradingConfigState {
            chain_id: 1,
            capital_usd: 1000.0,
            base_token_symbol: "WETH".into(),
            base_token_price_usd: 2000.0,
            allowed_token_symbols: vec!["WETH".into(), "USDC".into(), "USDT".into()],
            min_profit_usd: 2.0,
            min_roi_pct: 0.3,
            min_landing_probability: 0.5,
            min_liquidity_confidence: 0.7,
            max_token_risk_score: 1.0,
            gas_price_strategy: GasPriceStrategy::DynamicBasefeePlusTip,
            fixed_gas_price_gwei: None,
            gas_estimate_units: 250_000,
            max_slippage_pct: 0.5,
            failure_risk_buffer_pct: 0.001,
            flashloan_fee_pct: 0.0009,
            enabled_strategies: vec!["dex_arb_v2v2".into()],
            enabled: true,
            updated_at: Utc::now(),
            updated_by: Some("ops".into()),
        }
    }

    #[test]
    fn token_allowlist_is_case_insensitive() {
        let s = sample_state();
        assert!(s.token_allowed("usdc"));
        assert!(s.token_allowed("USDC"));
        assert!(!s.token_allowed("DAI"));
    }

    #[test]
    fn dynamic_gas_strategy_uses_basefee_plus_tip() {
        let s = sample_state();
        // basefee 30 + tip 2 = 32
        assert_eq!(s.resolve_gas_price_gwei(30.0, 2.0), 32.0);
    }

    #[test]
    fn fixed_gas_strategy_uses_operator_value() {
        let mut s = sample_state();
        s.gas_price_strategy = GasPriceStrategy::Fixed;
        s.fixed_gas_price_gwei = Some(15.0);
        // ignores live_basefee/tip
        assert_eq!(s.resolve_gas_price_gwei(99.0, 99.0), 15.0);
    }

    #[test]
    fn fixed_gas_falls_back_to_basefee_when_unset() {
        let mut s = sample_state();
        s.gas_price_strategy = GasPriceStrategy::Fixed;
        s.fixed_gas_price_gwei = None;
        assert_eq!(s.resolve_gas_price_gwei(42.0, 0.0), 42.0);
    }

    #[test]
    fn profit_conversion_uses_base_price() {
        let s = sample_state();
        assert_eq!(s.profit_token_to_usd(0.05), 100.0); // 0.05 ETH * 2000 USD/ETH
    }
}
