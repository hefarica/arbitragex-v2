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

    /// Per-token USD prices, keyed by symbol (case-insensitive at lookup).
    /// Operator-managed via Redis hot-reload; consumed by `price_oracle::ConfigPriceOracle`.
    /// `serde(default)` keeps backward compatibility with existing Redis configs
    /// that pre-date this field — they deserialise as empty map and the oracle
    /// falls back to base token + hardcoded stablecoin defaults. Tokens outside
    /// all three tiers are rejected with `RejectReason::UnknownTokenPrice` (R8
    /// Fail-Honest — no fabricated prices). See anti_reincidencia.md Incidente #7.
    #[serde(default)]
    pub token_prices_usd: HashMap<String, f64>,

    /// **Simulation knob — global cap** (decoupled from operational `capital_usd`).
    ///
    /// When `Some(amount)`, the spine evaluator uses `amount` as the effective
    /// capital cap for ALL math + risk policy decisions, INSTEAD of `capital_usd`.
    /// Lets the operator preview "what would the system see if I had $X capital?"
    /// without changing operational sizing — paper-trade mode (default) ensures
    /// zero on-chain execution regardless.
    ///
    /// When `None` (default), the evaluator falls back to `capital_usd` — pure
    /// backward-compatible behaviour.
    #[serde(default)]
    pub simulation_capital_usd: Option<f64>,

    /// **Simulation per-token amount caps** (USD), keyed by token symbol.
    /// When set for a given token, the evaluator caps the input side of any
    /// candidate where `token_in == this_symbol` at the corresponding amount.
    /// Lets the operator model "use no more than $5K when WETH is the input"
    /// independently from other tokens. Lookup is case-insensitive.
    ///
    /// Combined with `simulation_capital_usd` and `simulation_per_strategy_caps_usd`
    /// via `effective_capital_for()` — the MIN of all applicable caps wins.
    /// Empty map (default) → no per-token override.
    #[serde(default)]
    pub simulation_per_token_amounts_usd: HashMap<String, f64>,

    /// **Simulation per-strategy caps** (USD), keyed by strategy_kind string
    /// (e.g. "dex_arb_v2v2", "triangular", "cex_dex"). When set, the evaluator
    /// caps any candidate of that strategy at the corresponding amount.
    /// Lets the operator model strategy-specific risk envelopes without
    /// touching operational `capital_usd`. Lookup is case-insensitive.
    ///
    /// Combined with the global + per-token caps via `effective_capital_for()`
    /// — the MIN of all applicable caps wins. Empty map (default) → no
    /// per-strategy override.
    #[serde(default)]
    pub simulation_per_strategy_caps_usd: HashMap<String, f64>,

    /// **Simulation UI filter — minimum profit** (USD) the operator wants to
    /// see surfaced in the dashboard. Backend STORES this value but does NOT
    /// gate on it (R8 fail-honest: persist all real candidates, let the UI
    /// filter what to display). Frontend reads this to suppress sub-target
    /// rows from the live opportunities view.
    ///
    /// `None` (default) → UI shows all opportunities regardless of profit.
    #[serde(default)]
    pub simulation_target_profit_usd: Option<f64>,

    /// **Simulation UI filter — minimum ROI percent** the operator wants to
    /// see surfaced in the dashboard. Same semantics as
    /// `simulation_target_profit_usd` — UI hint only, backend persists honestly.
    ///
    /// `None` (default) → UI shows all opportunities regardless of ROI.
    #[serde(default)]
    pub simulation_target_roi_pct: Option<f64>,

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

    // --- Sprint A: Component 6 & 7 config scalars ---

    /// Annual opportunity-cost rate for capital locked during execution.
    ///
    /// Formula applied by `config_aware.rs`:
    ///   `capital_cost_usd = amount_in_usd × (rate / 100.0) × (block_time_s / 31_536_000.0)`
    ///
    /// Flash-loan strategies bypass this entirely (capital not locked).
    /// Default `0.0` (free capital assumption); operator sets to e.g. `5.0` for
    /// a 5% APR cost model. Backed by migration 047 column
    /// `trading_config.capital_cost_rate_annual_pct` (CHECK: 0–50).
    #[serde(default)]
    pub capital_cost_rate_annual_pct: f64,

    /// Amortised per-attempt infra/server cost in USD.
    ///
    /// Covers hosting, RPC credits, monitoring pro-rated over expected daily
    /// attempt volume. Default `$0.01`. Backed by migration 047 column
    /// `trading_config.ops_overhead_usd_per_attempt` (CHECK: 0–100).
    #[serde(default = "default_ops_overhead")]
    pub ops_overhead_usd_per_attempt: f64,

    // --- Sprint C: Component 9 (spread sanity) + Component 5 (p_copied) ---

    /// Multiplier for the AMM-vs-oracle spread sanity check (Component 9).
    ///
    /// The evaluator computes `spread_ratio = observed_rate / reference_rate`.
    /// If `spread_ratio < 1/mult` OR `spread_ratio > mult`, the opportunity
    /// is rejected with `RejectReason::ImplausibleSpread`.
    ///
    /// Default `3.0` (observed must be within 3× of oracle reference).
    /// Backed by migration 048 column `trading_config.spread_sanity_mult`
    /// (CHECK: 1.0 ≤ value ≤ 100.0).
    /// `serde(default)` keeps backward compatibility with pre-Sprint-C configs.
    #[serde(default = "default_spread_sanity_mult")]
    pub spread_sanity_mult: f64,

    /// Baseline 24h volume (USD) for the p_copied heuristic (Component 5).
    ///
    /// Formula: `p_copied = min(p_copied_max, log10(vol / threshold) × 0.1).max(0.0)`.
    /// At `vol == threshold` → p_copied = 0 (no copy-trade cost).
    /// At `vol == 10× threshold` → p_copied = 10%.
    ///
    /// Default `$1_000_000` ($1M/24h). Backed by migration 048 column
    /// `trading_config.p_copied_volume_threshold_usd`.
    #[serde(default = "default_p_copied_volume_threshold_usd")]
    pub p_copied_volume_threshold_usd: f64,

    /// Cap on the p_copied heuristic probability (Component 5).
    ///
    /// `p_copied` is clamped to `[0.0, p_copied_max]` before computing the
    /// buffer. Default `0.5` (50% maximum copy-trade probability).
    /// Backed by migration 048 column `trading_config.p_copied_max`
    /// (CHECK: 0.0 ≤ value ≤ 1.0).
    #[serde(default = "default_p_copied_max")]
    pub p_copied_max: f64,

    pub enabled: bool,
    pub updated_at: DateTime<Utc>,
    pub updated_by: Option<String>,
}

/// Provides the serde default for `ops_overhead_usd_per_attempt` ($0.01).
fn default_ops_overhead() -> f64 {
    0.01
}

/// Provides the serde default for `spread_sanity_mult` (3.0×).
fn default_spread_sanity_mult() -> f64 {
    3.0
}

/// Provides the serde default for `p_copied_volume_threshold_usd` ($1M/24h).
fn default_p_copied_volume_threshold_usd() -> f64 {
    1_000_000.0
}

/// Provides the serde default for `p_copied_max` (50%).
fn default_p_copied_max() -> f64 {
    0.5
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

    /// Returns the effective capital used for math sizing + risk policy bounds
    /// when neither token nor strategy context is available. Honours the global
    /// `simulation_capital_usd` knob when set, otherwise falls back to the
    /// operational `capital_usd`.
    ///
    /// **Prefer `effective_capital_for(token, strategy)` whenever the spine
    /// has the candidate's symbols** — that one also honours per-token and
    /// per-strategy caps, taking the MIN of all applicable ceilings.
    pub fn effective_capital_usd(&self) -> f64 {
        self.simulation_capital_usd.unwrap_or(self.capital_usd)
    }

    /// Resolves the effective capital cap for a specific (token_in, strategy)
    /// pair, honouring all three simulation knobs:
    ///
    ///   1. `simulation_per_token_amounts_usd[token_in]` — case-insensitive
    ///   2. `simulation_per_strategy_caps_usd[strategy_kind]` — case-insensitive
    ///   3. `simulation_capital_usd` — global sim cap
    ///   4. Operational `capital_usd` — fallback when no sim caps apply
    ///
    /// When MULTIPLE caps apply, the MINIMUM wins (most conservative). When
    /// none of the simulation knobs apply, the operator's operational
    /// `capital_usd` is returned untouched.
    ///
    /// Token symbols and strategy_kind are matched case-insensitively to
    /// shield against operator typos in the Redis config.
    pub fn effective_capital_for(&self, token_in_symbol: &str, strategy_kind: &str) -> f64 {
        let token_cap = self.lookup_per_token_cap(token_in_symbol);
        let strategy_cap = self.lookup_per_strategy_cap(strategy_kind);
        let global_cap = self.simulation_capital_usd;

        // Collect the sim caps that are actually set for this candidate.
        let applicable: Vec<f64> = [global_cap, token_cap, strategy_cap]
            .iter()
            .filter_map(|c| *c)
            .collect();

        if applicable.is_empty() {
            // No simulation overrides apply → operational capital governs.
            self.capital_usd
        } else {
            // Most-conservative: MIN of all applicable caps.
            applicable.into_iter().fold(f64::INFINITY, f64::min)
        }
    }

    /// Case-insensitive lookup helper for `simulation_per_token_amounts_usd`.
    fn lookup_per_token_cap(&self, symbol: &str) -> Option<f64> {
        if symbol.is_empty() {
            return None;
        }
        let upper = symbol.to_ascii_uppercase();
        self.simulation_per_token_amounts_usd
            .iter()
            .find(|(k, _)| k.to_ascii_uppercase() == upper)
            .map(|(_, v)| *v)
    }

    /// Case-insensitive lookup helper for `simulation_per_strategy_caps_usd`.
    fn lookup_per_strategy_cap(&self, strategy_kind: &str) -> Option<f64> {
        if strategy_kind.is_empty() {
            return None;
        }
        let upper = strategy_kind.to_ascii_uppercase();
        self.simulation_per_strategy_caps_usd
            .iter()
            .find(|(k, _)| k.to_ascii_uppercase() == upper)
            .map(|(_, v)| *v)
    }

    /// True if `token` (case-insensitive symbol) is in the operator's allowlist.
    pub fn token_allowed(&self, symbol: &str) -> bool {
        let needle = symbol.to_ascii_uppercase();
        self.allowed_token_symbols
            .iter()
            .any(|s| s.to_ascii_uppercase() == needle)
    }

    /// **DEPRECATED (BUG-2, 2026-05-05)**: this method assumes every token is
    /// the base token, producing nonsense USD valuations for any non-base
    /// non-stablecoin token. Use `shared_rs::price_oracle::ConfigPriceOracle`
    /// instead, which resolves prices per-token via three tiers (base, operator
    /// map, hardcoded stablecoins) with fail-honest `None` for unknown tokens.
    ///
    /// Kept for backward compatibility with any external caller; will be
    /// removed in a future sprint once all internal call sites are migrated
    /// (the spine evaluator was migrated in commit landing this PR).
    #[deprecated(
        since = "0.1.0",
        note = "use shared_rs::price_oracle::ConfigPriceOracle::price_usd; this method ignores token identity (BUG-2)"
    )]
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
            token_prices_usd: HashMap::new(),
            simulation_capital_usd: None,
            simulation_per_token_amounts_usd: HashMap::new(),
            simulation_per_strategy_caps_usd: HashMap::new(),
            simulation_target_profit_usd: None,
            simulation_target_roi_pct: None,
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
            capital_cost_rate_annual_pct: 0.0,
            ops_overhead_usd_per_attempt: 0.01,
            spread_sanity_mult: 3.0,
            p_copied_volume_threshold_usd: 1_000_000.0,
            p_copied_max: 0.5,
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
    #[allow(deprecated)] // exercises the legacy method intentionally — see #[deprecated] note
    fn profit_conversion_uses_base_price() {
        let s = sample_state();
        assert_eq!(s.profit_token_to_usd(0.05), 100.0); // 0.05 ETH * 2000 USD/ETH
    }

    #[test]
    fn effective_capital_falls_back_to_operational_capital_when_unset() {
        let s = sample_state(); // simulation_capital_usd = None
        assert_eq!(s.effective_capital_usd(), 1000.0);
    }

    #[test]
    fn effective_capital_uses_simulation_value_when_set() {
        let mut s = sample_state();
        s.simulation_capital_usd = Some(50_000.0);
        // operational capital is still 1000 but simulation overrides it
        assert_eq!(s.effective_capital_usd(), 50_000.0);
        assert_eq!(s.capital_usd, 1000.0); // operational unchanged
    }

    #[test]
    fn simulation_capital_can_be_smaller_than_operational() {
        // Operator may want to TEST conservative sizing without lowering prod.
        let mut s = sample_state();
        s.simulation_capital_usd = Some(100.0);
        assert_eq!(s.effective_capital_usd(), 100.0);
    }

    // ----------------------------------------------------------------
    // effective_capital_for(token, strategy) — multi-knob resolution
    // ----------------------------------------------------------------

    #[test]
    fn effective_capital_for_falls_back_to_operational_when_no_sim_caps() {
        let s = sample_state(); // capital_usd=1000, all sim fields empty/None
        assert_eq!(s.effective_capital_for("WETH", "dex_arb_v2v2"), 1000.0);
    }

    #[test]
    fn effective_capital_for_uses_global_sim_when_only_global_set() {
        let mut s = sample_state();
        s.simulation_capital_usd = Some(50_000.0);
        // Global applies regardless of token/strategy
        assert_eq!(s.effective_capital_for("WETH", "dex_arb_v2v2"), 50_000.0);
        assert_eq!(s.effective_capital_for("USDC", "triangular"), 50_000.0);
    }

    #[test]
    fn effective_capital_for_per_token_cap_applies_when_token_matches() {
        let mut s = sample_state();
        s.simulation_capital_usd = Some(50_000.0);
        s.simulation_per_token_amounts_usd.insert("WETH".into(), 5_000.0);
        // WETH input: per-token $5K wins over global $50K (MIN)
        assert_eq!(s.effective_capital_for("WETH", "dex_arb_v2v2"), 5_000.0);
        // USDC input: per-token doesn't apply, global $50K used
        assert_eq!(s.effective_capital_for("USDC", "dex_arb_v2v2"), 50_000.0);
    }

    #[test]
    fn effective_capital_for_per_strategy_cap_applies_when_strategy_matches() {
        let mut s = sample_state();
        s.simulation_capital_usd = Some(50_000.0);
        s.simulation_per_strategy_caps_usd.insert("triangular".into(), 2_000.0);
        // triangular: per-strategy $2K wins over global $50K
        assert_eq!(s.effective_capital_for("WETH", "triangular"), 2_000.0);
        // dex_arb: per-strategy doesn't apply, global $50K used
        assert_eq!(s.effective_capital_for("WETH", "dex_arb_v2v2"), 50_000.0);
    }

    #[test]
    fn effective_capital_for_uses_min_when_multiple_caps_apply() {
        let mut s = sample_state();
        s.simulation_capital_usd = Some(50_000.0);
        s.simulation_per_token_amounts_usd.insert("WETH".into(), 5_000.0);
        s.simulation_per_strategy_caps_usd.insert("triangular".into(), 2_000.0);
        // WETH + triangular: per-strategy $2K is MIN (wins over $5K and $50K)
        assert_eq!(s.effective_capital_for("WETH", "triangular"), 2_000.0);
        // WETH + dex_arb: per-token $5K wins (only token cap applies)
        assert_eq!(s.effective_capital_for("WETH", "dex_arb_v2v2"), 5_000.0);
        // USDC + triangular: per-strategy $2K wins
        assert_eq!(s.effective_capital_for("USDC", "triangular"), 2_000.0);
        // USDC + dex_arb: only global $50K
        assert_eq!(s.effective_capital_for("USDC", "dex_arb_v2v2"), 50_000.0);
    }

    #[test]
    fn effective_capital_for_lookups_are_case_insensitive() {
        let mut s = sample_state();
        s.simulation_per_token_amounts_usd.insert("WETH".into(), 5_000.0);
        s.simulation_per_strategy_caps_usd.insert("dex_arb_v2v2".into(), 3_000.0);
        // Lower-case input should still match upper-case keys
        assert_eq!(s.effective_capital_for("weth", "dex_arb_v2v2"), 3_000.0);
        assert_eq!(s.effective_capital_for("WeTh", "DEX_ARB_V2V2"), 3_000.0);
    }

    #[test]
    fn effective_capital_for_per_token_alone_overrides_operational() {
        // If operator only sets per-token caps (no global, no per-strategy),
        // the per-token cap takes precedence over operational capital_usd.
        let mut s = sample_state(); // capital_usd = 1000
        s.simulation_per_token_amounts_usd.insert("WETH".into(), 250.0);
        assert_eq!(s.effective_capital_for("WETH", "dex_arb_v2v2"), 250.0);
        // Non-matching token: no sim cap → operational capital $1000
        assert_eq!(s.effective_capital_for("USDC", "dex_arb_v2v2"), 1000.0);
    }
}
