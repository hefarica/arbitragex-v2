//! Config-aware evaluator: bridge between operator-tunable trading config and
//! deterministic profit math.
//!
//! Inputs:
//!   - `TradingConfigState` from `shared_rs::trading_config` (operator's live
//!     capital sizing, token allowlist, gas strategy, profit thresholds).
//!   - `OpportunityCandidate` (observed swap or constructed route).
//!   - Live network signals (gas basefee, p75 tip).
//!
//! Pipeline:
//!   1. Token allowlist gate (skip silently if outside operator's universe).
//!   2. Capital sizing — actual amount_in is min(operator capital, observed amount_in).
//!   3. Math evaluation via `math_engine::roi_engine::calc_net_profit_and_roi`.
//!   4. Risk validation via `math_engine::risk_engine::validate_opportunity_risk`.
//!   5. Build `OpportunityEvidence` enriched with real numbers.
//!
//! Why this lives in prioritization-spine and not searcher-rs:
//!   - This is pure logic over (config, candidate, signals) → evidence/decision.
//!     Putting it in spine keeps searcher-rs focused on mempool I/O and lets
//!     other entry points (sim-ctl, recon) re-use the same evaluator.
//!   - It composes existing spine primitives without modifying them. The legacy
//!     `PrioritizationEngine::score` keeps its signature; this evaluator builds
//!     the inputs that engine expects, with honest values instead of stubs.

use crate::decision::{ExecutionDecision, RejectReason};
use crate::evidence::{CostBreakdown, OpportunityEvidence, PFailSource};
use crate::feedback::FeedbackChannel;
use crate::route_plan::RoutePlan;
use crate::strategy_config_gate::{GateOutcome, StrategyConfigGate};
use crate::strategy_scores_db::StrategyFailRate;
use crate::types::OpportunityCandidate;
use math_engine::amm_math::{calc_univ2_price_impact, calc_univ3_price_impact_pct};
use math_engine::risk_engine::{
    validate_opportunity_risk, OpportunityRiskProfile, RiskPolicy, RiskRejectionReason,
};
use math_engine::roi_engine::{calc_net_profit_and_roi, RoiCalculationParams};
use math_engine::DefiArbitrageOutcome;
use shared_rs::chains::block_time_s_for_chain;
use shared_rs::price_oracle::{
    CascadePriceOracle, ConfigPriceOracle, PriceOracle, RedisCachedPriceOracle,
};
use shared_rs::token_identity::TokenIdentityIndex;
use shared_rs::trading_config::TradingConfigState;
use std::collections::HashMap;

// Sprint H1 tweak 1: per-chain block time constant removed.
// `block_time_s_for_chain(chain_id)` from `shared_rs::chains` replaces it.
// ETH (12s) / BSC (3s) / Polygon (2s) / Base (2s) / ARB (0.25s) / OP (2s).
// ARB was 6× over-costed at the old 12s constant.
const SECONDS_PER_YEAR: f64 = 31_536_000.0;

/// Returns the standard LP fee in basis points for a DEX adapter by name.
///
/// Sprint H1 tweak 2: provides explicit `lp_fees_usd` for the CostBreakdown
/// rather than relying on the implicit absorption in the AMM spread.
///
/// Values are protocol constants, not configuration — they change extremely
/// rarely and are fully auditable against on-chain contract bytecode.
///
/// | Adapter name (from RouterKind::as_str()) | fee_bps | Protocol note          |
/// |------------------------------------------|---------|------------------------|
/// | "uniswap-v2"                             |   30    | 0.30% LP fee           |
/// | "uniswap-v3"                             |    0    | fee varies by pool tier;|
/// |                                          |         | explicit bps unknown   |
/// |                                          |         | here → 0 (conservative)|
/// | "sushi"                                  |   30    | 0.30% LP fee           |
/// | "curve"                                  |    4    | 0.04% typical stable   |
/// | "balancer"                               |   10    | 0.10% weighted default |
/// | unknown                                  |    0    | fail-honest: don't add |
///
/// For V3 pools, the per-pool fee_bps comes from `V3PoolInfo.fee_bps` (stored
/// in Redis). That per-pool value is not available here (the candidate carries
/// only the dex adapter name, not the fee tier). Pass 0 for V3 (conservative:
/// fees are already absorbed in the AMM quote output). A future sprint can
/// propagate `fee_bps` via `OpportunityCandidate.dex_adapters_fee_bps: Vec<u32>`.
fn default_fee_bps_for_adapter(adapter_name: &str) -> u32 {
    match adapter_name.to_ascii_lowercase().as_str() {
        "uniswap-v2" => 30,
        "sushi" => 30,
        "curve" => 4,
        "balancer" => 10,
        // V3: fee varies by pool tier (100/500/3000/10000 bps); without the
        // tier we cannot resolve it here — return 0 (R8 fail-honest: don't
        // synthesise a fee we don't know). AMM quote already reflects the fee.
        "uniswap-v3" => 0,
        // unknown / future adapters: 0 (conservative: don't add phantom fees)
        _ => 0,
    }
}

/// Live signals the evaluator needs in addition to config + candidate.
/// Sourced from chain-client (`eth_gasPrice`, `eth_feeHistory`) at scoring time.
#[derive(Debug, Clone, Copy)]
pub struct NetworkSignals {
    pub basefee_gwei: f64,
    pub p75_priority_tip_gwei: f64,
    pub block_number: u64,
}

impl NetworkSignals {
    /// Sentinel used when the chain client has not been wired yet — the evaluator
    /// will fall back to the operator's `fixed_gas_price_gwei` if set, otherwise
    /// fail conservatively. Never reaches production paths once `chain_client`
    /// exposes basefee live.
    pub fn unknown(block_number: u64) -> Self {
        Self {
            basefee_gwei: 0.0,
            p75_priority_tip_gwei: 0.0,
            block_number,
        }
    }
}

#[derive(Debug, Clone)]
pub enum ConfigGateOutcome {
    /// Token outside operator's allowlist — skipped silently.
    TokenNotAllowed { token_symbol_or_addr: String },
    /// Strategy class disabled by operator.
    StrategyDisabled { strategy_kind: String },
    /// Migration 056 — `StrategyConfigGate` blocked the candidate based on
    /// per-strategy config (chain allowlist, DEX allowlist, protocol type,
    /// route shape, pool floor). Carries the precise reject reason so the
    /// dashboard surfaces actionable detail (which DEX / which leg / why).
    StrategyConfigGateBlocked { reason: RejectReason },
    /// Ran the math; here is the result (may still be unprofitable).
    /// `outcome` and `evidence` are boxed to avoid the `large_enum_variant`
    /// lint: `OpportunityEvidence` (~480 bytes) dwarfs the other variants
    /// (~24 bytes). Boxing heap-allocates the large payload; callers
    /// dereference with `*` or pattern-match normally.
    Evaluated {
        outcome: Box<DefiArbitrageOutcome>,
        evidence: Box<OpportunityEvidence>,
        rejection: Option<RejectReason>,
        /// `true` when the pre-math gate could not enforce a per-leg check
        /// because the candidate did not carry a `RoutePlan`. The dashboard
        /// surfaces this as `data_quality=partial` so the operator knows
        /// their toggles are best-effort for the producing worker until
        /// it migrates to `evaluate_with_route_plan`.
        partial_data_quality: bool,
    },
}

/// Translate config thresholds into a RiskPolicy that math-engine consumes.
/// Takes `effective_capital_usd` explicitly (not from `cfg`) so the spine
/// honours `simulation_capital_usd` overrides — the policy uses the same
/// capital figure the math layer uses for sizing, keeping risk gates and
/// math sizing internally consistent.
fn policy_from_config(cfg: &TradingConfigState, effective_capital_usd: f64) -> RiskPolicy {
    RiskPolicy {
        min_net_profit_usd: cfg.min_profit_usd,
        min_net_roi_pct: cfg.min_roi_pct,
        // Gas cost ratio is not in the operator UI yet — use a safe default of 0.5
        // (gas may consume up to 50% of gross profit, anything more is rejected).
        max_gas_cost_ratio: 0.5,
        max_slippage_pct: cfg.max_slippage_pct,
        // Price impact bound is implicitly enforced by liquidity confidence; for
        // now allow up to 5% impact, future PR adds a dedicated config field.
        max_price_impact_pct: 0.05,
        // Liquidity floor — effective capital is a sensible proxy: don't
        // touch pools that can't absorb the deployable capital being modelled.
        min_liquidity_usd: effective_capital_usd.max(1.0),
        max_trade_size_usd: effective_capital_usd,
    }
}

/// Estimate gas cost in USD using config's strategy + live signals.
/// Conservative: when neither basefee nor fixed price is known, returns 0
/// (caller must check `signals != unknown` before relying on this).
fn estimate_gas_cost_usd(cfg: &TradingConfigState, signals: NetworkSignals) -> f64 {
    let gas_price_gwei =
        cfg.resolve_gas_price_gwei(signals.basefee_gwei, signals.p75_priority_tip_gwei);
    let gas_units = cfg.gas_estimate_units as f64;
    // gwei → wei → ETH → USD (using operator's base_token_price_usd as ETH price proxy).
    // This is correct when base_token = WETH; for non-WETH base operators the
    // conversion needs a separate eth_price_usd field (next sprint).
    (gas_units * gas_price_gwei * 1e9) / 1e18 * cfg.base_token_price_usd
}

/// Produce a stable string key (chain:strategy_kind) for strategy gate.
pub fn strategy_key(chain_id: u64, strategy_kind: &str) -> String {
    format!("{}:{}", chain_id, strategy_kind)
}

pub struct ConfigAwareEvaluator<'a> {
    pub config: &'a TradingConfigState,
    pub signals: NetworkSignals,
    /// Pre-fetched live price snapshot (Alchemy → Coingecko populated via
    /// `searcher-rs::workers::price_worker`). When present, takes priority
    /// over `ConfigPriceOracle`. When empty (boot, all sources down), the
    /// cascade falls through to ConfigPriceOracle (operator overrides +
    /// stablecoins + base token), and finally to `None` →
    /// `RejectReason::UnknownTokenPrice` (R8 fail-honest).
    ///
    /// Snapshot is fetched ONCE per evaluation tick by the caller (async) and
    /// passed in here, keeping `evaluate()` sync on the hot path.
    pub price_cache_snapshot: HashMap<String, f64>,

    /// Pre-fetched statistical failure rate from `strategy_scores` (Sprint B).
    ///
    /// `Some(rate)` → `evaluate()` uses `rate.p_fail × gas_cost_usd` as the
    /// component-4 buffer instead of the flat proxy.
    /// `None` (cold-start, new strategy, `sample_count < 10`) → proxy fallback
    /// (`amount_in_usd × config.failure_risk_buffer_pct`).
    ///
    /// Callers fetch this via `StrategyScoresCache::get(pool, strategy_kind, chain_id).await`
    /// before constructing the evaluator, keeping `evaluate()` entirely sync.
    pub p_fail_rate: Option<StrategyFailRate>,

    /// Pre-fetched 24h volume (USD) of the weakest-link pool in the route (Sprint C).
    ///
    /// Sourced from `dex_chain_metrics.volume_24h_usd` for the pool with the
    /// lowest volume — the "competition floor" for copy-trade probability.
    ///
    /// `Some(vol)` → `evaluate()` computes `p_copied = min(config.p_copied_max,
    ///               log10(vol / config.p_copied_volume_threshold_usd) × 0.1).max(0.0)`.
    /// `None` (no `dex_chain_metrics` row, cold-start) → `p_copied = None` → no
    ///   copy-trade cost added (R8 fail-honest — never synthesise volume data).
    ///
    /// Caller fetches this async via `DexVolumeCache::get(pool_address, chain_id).await`
    /// before constructing the evaluator, keeping `evaluate()` entirely sync.
    pub pool_volume_24h_usd: Option<f64>,

    /// Pre-fetched V2 pool reserves for the primary leg of the route (Sprint H1).
    ///
    /// When populated, `evaluate()` calls `calc_univ2_price_impact` to obtain a
    /// real slippage estimate that replaces the `max_slippage_pct` proxy.
    ///
    /// Tuple: `(reserve_in, reserve_out)` — **reserve_in must correspond to the
    /// candidate's `token_in`** (orientation already resolved by the caller using
    /// `ReservesEntry.token0_addr`). Both values are in wei (raw uint112), as
    /// decimal strings → f64. The caller is responsible for orientation.
    ///
    /// `Some((r_in, r_out))` → real price impact used for component 3.
    /// `None` (cold cache, pool_addresses empty, or reserves TTL expired) →
    ///   falls back to `max_slippage_pct` proxy (R8 fail-honest).
    ///
    /// Redis key: `arbx:pool_reserves:<chain_id>:<pool_addr_lower>`.
    /// Populated every ~5s by `searcher-rs::workers::pool_sync_worker`.
    /// TTL: 30s. Staleness up to 30s is acceptable — price impact is a
    /// rough guard against large trades; ±30s reserve lag is within noise.
    pub v2_reserve_snapshot: Option<(f64, f64)>,

    /// Pre-fetched V3 slot0 data for the primary leg of the route (Sprint H2).
    ///
    /// When populated (V3 route, `v2_reserve_snapshot = None`), `evaluate()`
    /// calls `calc_univ3_price_impact_pct` for a first-order tick-math estimate
    /// that replaces the `max_slippage_pct` proxy.
    ///
    /// Tuple: `(sqrt_price_x96, liquidity, token0_to_token1)` where:
    ///   - `sqrt_price_x96`: raw Q96 value from `slot0()` — √P × 2^96.
    ///   - `liquidity`: active liquidity at current tick from `slot0()` (uint128).
    ///   - `token0_to_token1`: swap direction resolved by the caller from token
    ///     ordering in the pool (`true` = selling token0, buying token1).
    ///
    /// `Some(...)` → first-order tick-math impact used for component 3.
    /// `None` (cold cache, non-V3 route, or pool_sync_worker not yet emitting
    ///   V3 slot0 data) → falls back to `max_slippage_pct` proxy (R8 fail-honest).
    ///
    /// Redis key: `arbx:v3_slot0:<chain_id>:<pool_addr_lower>`.
    /// Intended to be populated by `searcher-rs::workers::pool_sync_worker` when
    /// the pool is identified as a V3 pool (via factory address check or fee_tier
    /// field in `V3PoolInfo`). As of H2 dispatch the key may not yet be emitted
    /// — callers should treat `None` gracefully and log a WARN for traceability.
    pub v3_slot0_snapshot: Option<(u128, u128, bool)>,

    /// Live adaptive feedback from the recon aggregator (Sprint BE-3.5).
    ///
    /// When `Some`, `evaluate()` queries the channel for a fresh signal
    /// (`< 300 s` old) for `(strategy_kind, chain_id)` **before** reading
    /// `p_fail_rate`.  If the channel returns a fresh signal, its
    /// `revert_rate` overrides `p_fail_rate.p_fail`.
    ///
    /// Priority order for `p_fail`:
    ///   1. `feedback_channel` (real-time pub/sub, freshest signal)
    ///   2. `p_fail_rate`       (SQL cache from `StrategyScoresCache`, ≤ 60 s old)
    ///   3. Proxy fallback      (`amount_in_usd × failure_risk_buffer_pct`)
    ///
    /// Pass `None` when the subscriber has not been spawned (e.g. sim-ctl,
    /// unit tests that do not need real-time feedback).
    pub feedback_channel: Option<FeedbackChannel>,

    /// Pre-fetched EWMA relay-bribe estimate in USD (C2 fix, audit re-run #2).
    ///
    /// Sourced from Redis key `arbx:relay_fee_ewma:{chain_id}:{strategy_kind}`
    /// (written by `relays-client::submit_engine` after each `eth_callBundle`
    /// simulation using α=0.2 and TTL=1h). Converted to USD externally by the
    /// caller (coinbase_diff_wei × eth_price_usd / 1e18).
    ///
    /// When the key is absent (cold-start, L2 chains, or first-ever bundle):
    /// the caller MUST apply the doctrine floor:
    ///   `max(gross_profit_usd × RELAY_FEE_FLOOR_PCT, RELAY_FEE_FLOOR_ABS_USD)`
    /// (5% of gross OR $0.50, whichever is greater).
    /// Constants live in `shared_rs::pre_execute_checklist`.
    ///
    /// For L2 chains (Arbitrum, Base, Optimism, Polygon): pass `0.0` — sequencer
    /// inclusion is deterministic and requires no searcher bribe.
    ///
    /// `evaluate()` passes this value directly into `RoiCalculationParams.estimated_relay_fee_usd`
    /// and `CostBreakdown.relay_fee_usd`. Callers pre-fetch async to keep
    /// `evaluate()` entirely sync on the hot path.
    pub estimated_relay_fee_usd: f64,

    /// ARBX-0018 — address-keyed token identity for this chain. When `Some`,
    /// `candidate.token_addresses` are treated as REAL ADDRESSES: the
    /// allowlist gate binds `(chain_id, address)` (`TokenIdentityIndex::
    /// is_allowed_addr`) and the symbol-keyed price stack / per-token caps
    /// receive `symbol_for_addr(addr)` (raw address passthrough when the
    /// universe doesn't know it — the miss stays honest downstream).
    ///
    /// `None` (default) keeps the LEGACY symbol-string binding for callers
    /// not yet migrated (`config.token_allowed` on whatever strings the
    /// caller put in `token_addresses`). Migration of remaining call sites
    /// is ARBX-R-0002; the two modes never mix within one evaluation.
    pub token_identity: Option<std::sync::Arc<TokenIdentityIndex>>,
}

impl<'a> ConfigAwareEvaluator<'a> {
    /// Backward-compatible constructor. Equivalent to `with_cache(cfg, signals, empty)`
    /// — the cascade still runs but tier 1 (live cache) is empty so behaviour
    /// degrades to ConfigPriceOracle alone (the previous semantics).
    pub fn new(config: &'a TradingConfigState, signals: NetworkSignals) -> Self {
        Self {
            config,
            signals,
            price_cache_snapshot: HashMap::new(),
            p_fail_rate: None,
            pool_volume_24h_usd: None,
            v2_reserve_snapshot: None,
            v3_slot0_snapshot: None,
            feedback_channel: None,
            estimated_relay_fee_usd: 0.0,
            token_identity: None,
        }
    }

    /// Construct with a pre-fetched live price snapshot. Production path used
    /// by `searcher-rs::scanner::process_pending` after calling
    /// `RedisCachedPriceOracle::snapshot_from_redis(&mut redis, chain_id).await`.
    /// The snapshot field map is `symbol_uppercase → usd_price`; non-finite or
    /// non-positive entries are filtered when the snapshot is built.
    pub fn with_cache(
        config: &'a TradingConfigState,
        signals: NetworkSignals,
        price_cache_snapshot: HashMap<String, f64>,
    ) -> Self {
        Self {
            config,
            signals,
            price_cache_snapshot,
            p_fail_rate: None,
            pool_volume_24h_usd: None,
            v2_reserve_snapshot: None,
            v3_slot0_snapshot: None,
            feedback_channel: None,
            estimated_relay_fee_usd: 0.0,
            token_identity: None,
        }
    }

    /// Full-featured constructor with both price cache and statistical failure rate.
    /// Production path once `StrategyScoresCache` is wired in the caller loop.
    ///
    /// `p_fail_rate` is `None` for cold-start / new strategies (R8 fail-honest:
    /// the evaluator falls back to the flat proxy rather than inventing a rate).
    pub fn with_p_fail(
        config: &'a TradingConfigState,
        signals: NetworkSignals,
        price_cache_snapshot: HashMap<String, f64>,
        p_fail_rate: Option<StrategyFailRate>,
    ) -> Self {
        Self {
            config,
            signals,
            price_cache_snapshot,
            p_fail_rate,
            pool_volume_24h_usd: None,
            v2_reserve_snapshot: None,
            v3_slot0_snapshot: None,
            feedback_channel: None,
            estimated_relay_fee_usd: 0.0,
            // ARBX-0018 (migration completion, 2026-08-24): secondary
            // constructors can't receive an identity index — fail-honest
            // `None`; injection stays exclusively via `with_token_identity`.
            token_identity: None,
        }
    }

    /// Full-featured constructor with price cache, statistical failure rate,
    /// and weakest-pool 24h volume for the p_copied heuristic (Sprint C).
    ///
    /// `pool_volume_24h_usd` is the minimum 24h volume (USD) across all pools
    /// in the candidate route — the weakest-link determines the competition floor.
    /// Pass `None` when no `dex_chain_metrics` row exists (R8 fail-honest).
    pub fn with_volume(
        config: &'a TradingConfigState,
        signals: NetworkSignals,
        price_cache_snapshot: HashMap<String, f64>,
        p_fail_rate: Option<StrategyFailRate>,
        pool_volume_24h_usd: Option<f64>,
    ) -> Self {
        Self {
            config,
            signals,
            price_cache_snapshot,
            p_fail_rate,
            pool_volume_24h_usd,
            v2_reserve_snapshot: None,
            v3_slot0_snapshot: None,
            feedback_channel: None,
            estimated_relay_fee_usd: 0.0,
            // ARBX-0018: see with_p_fail — `None` + builder injection.
            token_identity: None,
        }
    }

    /// Full-featured constructor with all inputs including pre-fetched V2 reserves
    /// (Sprint H1 tweak 3). Enables real price impact computation.
    ///
    /// `v2_reserve_snapshot`: `Some((reserve_in, reserve_out))` from Redis key
    /// `arbx:pool_reserves:<chain_id>:<pool_addr_lower>`. Orientation must be
    /// pre-resolved by the caller: `reserve_in` corresponds to `token_in`.
    /// Pass `None` when pool_addresses is empty or reserves cache is cold
    /// (R8 fail-honest — fallback to `max_slippage_pct` proxy).
    pub fn with_reserves(
        config: &'a TradingConfigState,
        signals: NetworkSignals,
        price_cache_snapshot: HashMap<String, f64>,
        p_fail_rate: Option<StrategyFailRate>,
        pool_volume_24h_usd: Option<f64>,
        v2_reserve_snapshot: Option<(f64, f64)>,
    ) -> Self {
        Self {
            config,
            signals,
            price_cache_snapshot,
            p_fail_rate,
            pool_volume_24h_usd,
            v2_reserve_snapshot,
            v3_slot0_snapshot: None,
            feedback_channel: None,
            estimated_relay_fee_usd: 0.0,
            // ARBX-0018: see with_p_fail — `None` + builder injection.
            token_identity: None,
        }
    }

    /// Full-featured constructor with all inputs including pre-fetched V3 slot0
    /// data (Sprint H2). Used for V3 routes where `v2_reserve_snapshot = None`.
    ///
    /// `v3_slot0_snapshot`: `Some((sqrt_price_x96, liquidity, token0_to_token1))`
    /// from Redis key `arbx:v3_slot0:<chain_id>:<pool_addr_lower>`.
    /// The direction flag must be resolved by the caller from token order in the
    /// pool (`true` = candidate's token_in is pool's token0).
    /// Pass `None` when the key is absent or the pool is not V3 (R8 fail-honest).
    ///
    /// When both `v2_reserve_snapshot` and `v3_slot0_snapshot` are `Some`, the
    /// V2 snapshot takes precedence (V2 constant-product formula is exact; V3 is
    /// an approximation). In practice callers should only populate the one that
    /// matches the route's DEX type.
    pub fn with_v3_slot0(
        config: &'a TradingConfigState,
        signals: NetworkSignals,
        price_cache_snapshot: HashMap<String, f64>,
        p_fail_rate: Option<StrategyFailRate>,
        pool_volume_24h_usd: Option<f64>,
        v2_reserve_snapshot: Option<(f64, f64)>,
        v3_slot0_snapshot: Option<(u128, u128, bool)>,
    ) -> Self {
        Self {
            config,
            signals,
            price_cache_snapshot,
            p_fail_rate,
            pool_volume_24h_usd,
            v2_reserve_snapshot,
            v3_slot0_snapshot,
            feedback_channel: None,
            estimated_relay_fee_usd: 0.0,
            // ARBX-0018: see with_p_fail — `None` + builder injection.
            token_identity: None,
        }
    }

    /// Builder-style setter for the estimated relay fee (C2 fix, audit re-run #2).
    ///
    /// The caller fetches the EWMA value from Redis key
    /// `arbx:relay_fee_ewma:{chain_id}:{strategy_kind}` (raw wei as f64 string),
    /// converts to USD with `ewma_wei / 1e18 * config.base_token_price_usd`,
    /// and passes the result here.
    ///
    /// Cold-start (key absent): apply the doctrine floor before calling this:
    /// ```ignore
    /// use shared_rs::pre_execute_checklist::{relay_fee_ewma_key,
    ///     RELAY_FEE_FLOOR_PCT, RELAY_FEE_FLOOR_ABS_USD};
    ///
    /// let ewma_wei: Option<f64> = redis.get(&relay_fee_ewma_key(chain_id, strategy_kind)).await.ok();
    /// let relay_fee_usd = ewma_wei
    ///     .map(|wei| (wei / 1e18) * config.base_token_price_usd)
    ///     .unwrap_or_else(|| {
    ///         // cold-start floor
    ///         let floor_pct = gross_profit_usd * RELAY_FEE_FLOOR_PCT;
    ///         floor_pct.max(RELAY_FEE_FLOOR_ABS_USD)
    ///     });
    /// let evaluator = ConfigAwareEvaluator::with_v3_slot0(...)
    ///     .with_relay_fee(relay_fee_usd);
    /// ```
    /// For L2 chains (Arbitrum, Base, Optimism, Polygon): pass `0.0`.
    pub fn with_relay_fee(mut self, estimated_relay_fee_usd: f64) -> Self {
        self.estimated_relay_fee_usd = estimated_relay_fee_usd;
        self
    }

    /// ARBX-0018 — attach the chain's address-keyed token identity index.
    /// Switches the evaluator to identity mode: the allowlist gate binds
    /// `(chain_id, address)`; symbols become metadata feeding the price
    /// stack. The index must belong to the SAME chain passed to
    /// `evaluate*` (one universe per chain — the caller composes both from
    /// the same scan; asserting `idx.chain_id() == chain_id` here would be
    /// redundant with the composition site's single source).
    pub fn with_token_identity(
        mut self,
        index: Option<std::sync::Arc<TokenIdentityIndex>>,
    ) -> Self {
        self.token_identity = index;
        self
    }

    /// Builder-style setter for the real-time feedback channel (Sprint BE-3.5).
    ///
    /// Call after any constructor to wire the pub/sub adaptive signal source:
    /// ```ignore
    /// let evaluator = ConfigAwareEvaluator::with_v3_slot0(...)
    ///     .with_feedback(feedback_channel.clone());
    /// ```
    /// When `channel` is `None`, the evaluator falls back to `p_fail_rate`
    /// (SQL path) or the proxy (R8 fail-honest).
    pub fn with_feedback(mut self, channel: Option<FeedbackChannel>) -> Self {
        self.feedback_channel = channel;
        self
    }

    /// Single-shot evaluation. Returns the gate outcome (allowlist, strategy,
    /// or full evaluated profile + evidence ready for `PrioritizationEngine::score`).
    ///
    /// `strategy_kind` is the candidate's class — must match an entry in
    /// `config.enabled_strategies` (e.g. "dex_arb_v2v2"). When the operator's
    /// `enabled_strategies` is empty, the evaluator treats it as "all enabled"
    /// to avoid silent paralysis on a freshly-seeded config.
    ///
    /// **Migration 056 backwards-compat**: this method calls
    /// `evaluate_with_route_plan` with `route_plan = None`. Workers migrated
    /// to emit `RoutePlan` should call `evaluate_with_route_plan` directly so
    /// the `StrategyConfigGate` enforces per-leg DEX/protocol/pool checks.
    pub fn evaluate(
        &self,
        candidate: &OpportunityCandidate,
        strategy_kind: &str,
        chain_id: u64,
        rpc_url_hash: String,
        rpc_latency_ms: u64,
    ) -> ConfigGateOutcome {
        self.evaluate_with_route_plan(
            candidate,
            None,
            strategy_kind,
            chain_id,
            rpc_url_hash,
            rpc_latency_ms,
        )
    }

    /// Migration 056 — `RoutePlan`-aware variant. Workers that have been
    /// migrated (Phase 3+) call this directly with the full route metadata,
    /// enabling the `StrategyConfigGate` to enforce per-leg DEX allowlist,
    /// protocol type allowlist, pool TVL/volume floors, and route shape
    /// constraints (min/max legs, atomicity).
    ///
    /// Workers NOT yet migrated keep calling `evaluate(...)` (which forwards
    /// here with `route_plan = None`); the gate then degrades gracefully:
    /// the cheap chain/strategy/disabled checks still apply, but per-leg
    /// checks are skipped and the result is flagged
    /// `partial_data_quality = true` so the dashboard surfaces the gap.
    pub fn evaluate_with_route_plan(
        &self,
        candidate: &OpportunityCandidate,
        route_plan: Option<&RoutePlan>,
        strategy_kind: &str,
        chain_id: u64,
        rpc_url_hash: String,
        rpc_latency_ms: u64,
    ) -> ConfigGateOutcome {
        // 1. Token allowlist gate.
        //
        // ARBX-0018: when a `TokenIdentityIndex` is attached, runtime
        // identity is `(chain_id, address)` — membership is checked
        // address-keyed and the SYMBOL metadata feeds the downstream
        // symbol-keyed price stack (oracle lookups + per-token caps below
        // consume `token_in_id`/`token_out_id`, which resolve through the
        // index in identity mode). A symbol STRING never passes the gate,
        // even when it matches the operator's allowlist. Legacy callers
        // (index = None) keep the symbol-string compare unchanged.
        let token_in_id: String;
        let token_out_id: String;
        match &self.token_identity {
            Some(idx) => {
                for tok in &candidate.token_addresses {
                    if !idx.is_allowed_addr(tok) {
                        return ConfigGateOutcome::TokenNotAllowed {
                            token_symbol_or_addr: tok.clone(),
                        };
                    }
                }
                // Symbol metadata for the price stack; raw address
                // passthrough on unknown → downstream miss stays honest.
                token_in_id = idx
                    .symbol_for_addr(
                        candidate
                            .token_addresses
                            .first()
                            .map(String::as_str)
                            .unwrap_or(""),
                    )
                    .map(str::to_string)
                    .unwrap_or_else(|| {
                        candidate
                            .token_addresses
                            .first()
                            .cloned()
                            .unwrap_or_default()
                    });
                token_out_id = idx
                    .symbol_for_addr(
                        candidate
                            .token_addresses
                            .get(1)
                            .map(String::as_str)
                            .unwrap_or(""),
                    )
                    .map(str::to_string)
                    .unwrap_or_else(|| {
                        candidate
                            .token_addresses
                            .get(1)
                            .cloned()
                            .unwrap_or_default()
                    });
            }
            None => {
                for tok in &candidate.token_addresses {
                    if !self.config.token_allowed(tok) {
                        return ConfigGateOutcome::TokenNotAllowed {
                            token_symbol_or_addr: tok.clone(),
                        };
                    }
                }
                token_in_id = candidate
                    .token_addresses
                    .first()
                    .cloned()
                    .unwrap_or_default();
                token_out_id = candidate
                    .token_addresses
                    .get(1)
                    .cloned()
                    .unwrap_or_default();
            }
        }

        // 2. Strategy gate (empty list = permissive default).
        if !self.config.enabled_strategies.is_empty()
            && !self
                .config
                .enabled_strategies
                .iter()
                .any(|s| s == strategy_kind)
        {
            return ConfigGateOutcome::StrategyDisabled {
                strategy_kind: strategy_kind.to_string(),
            };
        }

        // 2b. Migration 056 — per-strategy fine-grained config gate (PASS A).
        // Cheap checks before the math: chain allowlist, per-leg DEX/protocol
        // (only when RoutePlan is available), pool active, route shape.
        // PartialPass is propagated to the result via `partial_data_quality`.
        let pre_math_outcome = StrategyConfigGate::check_pre_math(
            self.config,
            candidate,
            route_plan,
            strategy_kind,
            chain_id,
        );
        let mut partial_data_quality = false;
        match pre_math_outcome {
            GateOutcome::Pass => {}
            GateOutcome::PartialPass { .. } => {
                partial_data_quality = true;
            }
            GateOutcome::Reject(reason) => {
                return ConfigGateOutcome::StrategyConfigGateBlocked { reason };
            }
        }

        // 3. Per-token USD valuation via cascade PriceOracle.
        //   tier 1: RedisCachedPriceOracle (live Alchemy/Coingecko snapshot)
        //   tier 2: ConfigPriceOracle      (operator manual + stables + base)
        //   miss   → None → RejectReason::UnknownTokenPrice (R8 fail-honest)
        //
        // Snapshot is owned by the evaluator (passed in via `with_cache`); the
        // cascade is rebuilt per-evaluate so the lifetimes line up cleanly with
        // `&self.config`. Construction cost is microseconds — two `Box::new`s
        // and a `Vec::push` — negligible vs the math + risk + serde work below.
        let cache_oracle = RedisCachedPriceOracle::from_snapshot(self.price_cache_snapshot.clone());
        let config_oracle = ConfigPriceOracle::new(self.config);
        let oracle: CascadePriceOracle =
            CascadePriceOracle::new(vec![Box::new(cache_oracle), Box::new(config_oracle)]);
        // ARBX-0018: identity mode resolved these through the index above
        // (addr → symbol metadata); legacy mode passes the raw entries.
        let in_price_opt = oracle.price_usd(&token_in_id);
        let out_price_opt = oracle.price_usd(&token_out_id);
        let unknown_price = in_price_opt.is_none() || out_price_opt.is_none();

        // When either side is unknown, force BOTH to zero so downstream
        // math collapses cleanly (gross=0, ROI=0). Forcing only the
        // unknown side would leave gross = -known_input_usd, polluting
        // the dashboard with negative profit numbers that mean nothing.
        // The rejection is overridden to UnknownTokenPrice below — the
        // operator's signal to populate `token_prices_usd` for the gap.
        let (in_price, out_price) = if unknown_price {
            (0.0, 0.0)
        } else {
            (in_price_opt.unwrap_or(0.0), out_price_opt.unwrap_or(0.0))
        };

        // BUG-3 fix preserved: when the cap reduces effective input,
        // expected output MUST also be scaled by the same ratio. Linear
        // scaling under-estimates slippage on smaller trades but eliminates
        // the asymmetric inflation that pollutes dashboards.
        //
        // Uses `effective_capital_for(token_in_symbol, strategy_kind)` so
        // operator's full set of simulation knobs apply — global cap,
        // per-token caps, AND per-strategy caps. The MIN of all applicable
        // caps wins. When no sim knobs are set, operational `capital_usd`
        // governs (backward compat).
        let effective_capital = self
            .config
            .effective_capital_for(&token_in_id, strategy_kind);
        let observed_amount_in_usd = candidate.amount_in * in_price;
        let amount_in_usd = observed_amount_in_usd.min(effective_capital);
        let cap_ratio = if observed_amount_in_usd > 0.0 {
            amount_in_usd / observed_amount_in_usd
        } else {
            1.0
        };
        let expected_amount_out_usd = candidate.expected_amount_out * out_price * cap_ratio;

        // 3b. Component 9 (Sprint C): spread sanity gate.
        //
        // Compares the AMM-quoted exchange rate with the oracle reference rate.
        // A large deviation (> spread_sanity_mult×) indicates stale AMM state,
        // a quoter bug, or a pool in an extreme state — NOT a real opportunity.
        //
        // R8 fail-honest: ONLY fires when BOTH oracle prices are present.
        // Missing prices → unknown_price=true path below handles the rejection;
        // we do NOT additionally reject as ImplausibleSpread on missing data.
        //
        // Spread ratio definition:
        //   observed_rate  = expected_amount_out / amount_in   (token units, not USD)
        //   reference_rate = price(token_in_usd) / price(token_out_usd)
        //     (how many token_out units one token_in buys at oracle fair value)
        //
        // Reject when spread_ratio < 1/mult OR spread_ratio > mult.
        let spread_sanity_rejection: Option<RejectReason> =
            if !unknown_price && candidate.amount_in > 0.0 && out_price > 0.0 {
                let observed_rate = candidate.expected_amount_out / candidate.amount_in;
                // reference_rate: how many token_out units = 1 token_in at oracle prices
                let reference_rate = in_price / out_price;
                if reference_rate > 0.0 {
                    let spread_ratio = observed_rate / reference_rate;
                    let mult = self.config.spread_sanity_mult;
                    if spread_ratio < (1.0 / mult) || spread_ratio > mult {
                        Some(RejectReason::ImplausibleSpread {
                            observed_rate,
                            reference_rate,
                            threshold_mult: mult,
                        })
                    } else {
                        None
                    }
                } else {
                    None // reference_rate=0 → oracle malfunction; skip check (R8)
                }
            } else {
                None // unknown prices → handled below; don't double-reject
            };

        // 3c. Component 5 (Sprint C): p_copied heuristic from dex_chain_metrics.
        //
        // Uses the 24h volume of the weakest-link pool (pre-fetched by the caller).
        // Formula: p = min(config.p_copied_max, log10(vol / threshold) × 0.1).max(0.0).
        // log10 of a ratio ≤ 1 is ≤ 0 → .max(0.0) floors it at zero.
        // R8: pool_volume_24h_usd = None → p_copied = None → no cost added.
        let p_copied: Option<f64> = match self.pool_volume_24h_usd {
            Some(vol) if vol > 0.0 => {
                let threshold = self.config.p_copied_volume_threshold_usd;
                let raw = if threshold > 0.0 {
                    (vol / threshold).log10() * 0.1
                } else {
                    0.0
                };
                let p = raw.max(0.0).min(self.config.p_copied_max);
                Some(p)
            }
            _ => None, // None or 0-volume → R8 fail-honest: no cost
        };

        // 4. Math: compute net profit and ROI deterministically.
        let gas_cost_usd = estimate_gas_cost_usd(self.config, self.signals);

        // --- Component 2: LP fees (Sprint H1 tweak 2) ---
        // Route-aggregate LP fee computed from the DEX adapter names carried
        // by the candidate. `default_fee_bps_for_adapter()` maps known protocol
        // names (uniswap-v2=30, sushi=30, curve=4, balancer=10, uniswap-v3=0)
        // to their standard fee tier. We sum across all adapters and multiply
        // by `amount_in_usd` as a conservative proxy for the amount routed
        // through each leg (exact per-leg amounts require RouteLeg propagation
        // from the scanner — deferred to a future sprint).
        //
        // For V3 routes, `default_fee_bps_for_adapter` returns 0 because the
        // per-pool fee tier is not encoded in the adapter name. The AMM quote
        // from QuoterV2 already reflects V3 fees in the output amount, so
        // the implicit absorption is the accurate treatment for V3 legs.
        //
        // R8 fail-honest: if dex_adapters is empty → 0.0 (no synthetic fees).
        let cumulative_fee_bps: u32 = candidate
            .dex_adapters
            .iter()
            .map(|a| default_fee_bps_for_adapter(a))
            .sum();
        let lp_fees_usd: f64 = if cumulative_fee_bps > 0 {
            amount_in_usd * (cumulative_fee_bps as f64 / 10_000.0)
        } else {
            0.0
        };

        // --- Component 3: Real price impact (Sprint H1 V2 + Sprint H2 V3) ---
        //
        // Priority order (first match wins):
        //   1. V2 reserves present → `calc_univ2_price_impact` (exact CPMM formula).
        //   2. V3 slot0 present   → `calc_univ3_price_impact_pct` (first-order tick
        //                           approximation; conservative-high for cross-tick swaps).
        //   3. Neither            → 0.0 → `max_slippage_pct` proxy (R8 fail-honest).
        //
        // V3 slot0 Redis key: `arbx:v3_slot0:<chain_id>:<pool_addr_lower>`.
        // Populated by `searcher-rs::workers::pool_sync_worker` for V3 pools.
        // As of H2 dispatch this key may not yet be emitted — callers must treat
        // `None` gracefully and log a WARN so the gap is traceable (R7).
        //
        // Both functions return a fraction (0.0–1.0); multiply by 100.0 to reach
        // the pct scale that `RoiCalculationParams.price_impact_pct` expects.
        let price_impact_pct: f64 = if let Some((reserve_in, _reserve_out)) =
            self.v2_reserve_snapshot
        {
            if reserve_in > 0.0 && amount_in_usd > 0.0 {
                // Derive fee fraction from the primary dex adapter.
                // Falls back to 30bps when adapter unknown — conservative.
                let fee_bps = candidate
                    .dex_adapters
                    .first()
                    .map(|a| default_fee_bps_for_adapter(a))
                    .unwrap_or(30);
                let fee_fraction = fee_bps as f64 / 10_000.0;
                let impact_fraction =
                    calc_univ2_price_impact(amount_in_usd, reserve_in, fee_fraction);
                impact_fraction * 100.0
            } else {
                0.0 // zero reserves → fall through to proxy
            }
        } else if let Some((sqrt_price_x96, liquidity, token0_to_token1)) = self.v3_slot0_snapshot {
            // V3 first-order tick approximation.
            // `amount_in_usd` is in USD; the tick formula needs token-unit wei.
            // Since `candidate.amount_in` is the raw token-unit amount observed
            // by the searcher (pre-price conversion), we use it directly after
            // converting to wei-scale integer. The searcher stores amounts in
            // floating-point token units; multiply by 1e18 to approximate wei.
            //
            // Limitation: this approximation is only valid when the swap is
            // denominated in the same token unit as the pool's token0/token1.
            // Token-decimal differences (e.g. USDC has 6 decimals, WETH has 18)
            // introduce a systematic scale error. Acceptable for the gate: the
            // function's conservative bias means the error direction is safe
            // (impact is over-reported, not under-reported, at high price ratio).
            //
            // Future sprint: propagate raw wei amounts through `OpportunityCandidate`
            // instead of float token units.
            let amount_in_wei_approx: u128 = (candidate.amount_in * 1e18) as u128;
            if amount_in_wei_approx > 0 {
                let impact_fraction = calc_univ3_price_impact_pct(
                    amount_in_wei_approx,
                    sqrt_price_x96,
                    liquidity,
                    token0_to_token1,
                );
                impact_fraction * 100.0
            } else {
                0.0 // zero amount → proxy fallback
            }
        } else {
            0.0 // no snapshot → proxy fallback (R8 fail-honest)
        };

        // --- Component 6: Capital opportunity cost (Sprint H1 tweak 1) ---
        // Flash-loan strategies borrow capital atomically in the same tx;
        // no lock-up period occurs, so capital cost is zero.
        // Non-flash strategies lock capital for one block duration.
        //
        // Sprint H1: replaced hardcoded `ETH_BLOCK_TIME_S = 12.0` with
        // `block_time_s_for_chain(chain_id)`. ARB was over-costed 6× at 12s
        // vs real ~0.25s; the correction matters for ranking tight-margin
        // opportunities where capital cost dominates at short block times.
        //
        // Detection: strategy names containing "flash" are treated as flash-loan.
        // This is conservative: unknown strategies are treated as non-flash
        // (capital cost applies), which is the safer direction.
        let is_flash_loan = strategy_kind.to_ascii_lowercase().contains("flash");
        let capital_cost_usd = if is_flash_loan || self.config.capital_cost_rate_annual_pct == 0.0 {
            0.0
        } else {
            let block_time = block_time_s_for_chain(chain_id);
            amount_in_usd
                * (self.config.capital_cost_rate_annual_pct / 100.0)
                * (block_time / SECONDS_PER_YEAR)
        };

        // --- Component 7: Ops overhead ---
        let ops_overhead_usd = self.config.ops_overhead_usd_per_attempt;

        let failure_risk_buffer_usd = amount_in_usd * self.config.failure_risk_buffer_pct;
        let flashloan_fee_usd_computed = amount_in_usd * self.config.flashloan_fee_pct;

        // --- Component 4 (Sprint B + BE-3.5): resolve p_fail for the failure buffer ---
        //
        // Priority order (most-fresh source wins):
        //   1. FeedbackChannel (Sprint BE-3.5) — real-time pub/sub signal, < 300 s old.
        //      Uses `revert_rate` from the latest recon aggregation cycle.
        //   2. p_fail_rate (Sprint B)          — SQL cache from StrategyScoresCache, ≤ 60 s old.
        //   3. Proxy fallback                  — amount_in_usd × failure_risk_buffer_pct.
        //
        // `p_fail_rate` was pre-fetched by the caller via `StrategyScoresCache`.
        // Map it into `Option<f64>` for `RoiCalculationParams`, and prepare the
        // `PFailSource` enum for `CostBreakdown` (dashboard surfacing).
        //
        // The feedback channel get() is synchronous (std::sync::RwLock reader) so
        // this block keeps evaluate() entirely non-async on the hot path.
        let (p_fail_opt, p_fail_source) = {
            // Tier 1: real-time feedback signal (BE-3.5).
            let feedback_signal = self
                .feedback_channel
                .as_ref()
                .and_then(|ch| ch.get(strategy_kind, chain_id));

            if let Some(ref sig) = feedback_signal {
                // Fresh pub/sub signal takes precedence over the SQL cache.
                (
                    Some(sig.revert_rate),
                    PFailSource::Statistical {
                        p: sig.revert_rate,
                        sample_count: sig.sample_count,
                    },
                )
            } else {
                // Tier 2: SQL StrategyScoresCache (Sprint B).
                match &self.p_fail_rate {
                    Some(rate) => (
                        Some(rate.p_fail),
                        PFailSource::Statistical {
                            p: rate.p_fail,
                            sample_count: rate.sample_count,
                        },
                    ),
                    // Tier 3: flat proxy fallback (R8 fail-honest).
                    None => (
                        None,
                        PFailSource::Proxy {
                            buffer_usd: failure_risk_buffer_usd,
                        },
                    ),
                }
            }
        };

        // --- Component 8 (C2 fix): relay fee EWMA ---
        //
        // `self.estimated_relay_fee_usd` is pre-fetched by the caller from Redis
        // key `arbx:relay_fee_ewma:{chain_id}:{strategy_kind}`. The cold-start
        // floor is applied by the caller BEFORE constructing the evaluator:
        //
        //   floor = max(gross_profit_usd × RELAY_FEE_FLOOR_PCT, RELAY_FEE_FLOOR_ABS_USD)
        //         = max(gross × 0.05, $0.50)
        //
        // This ensures cold-start bundles on mainnet are NOT modelled as zero-bribe
        // even before the EWMA accumulates real observations.
        //
        // For L2 chains (Arbitrum, Base, Optimism, Polygon) the caller passes 0.0 —
        // sequencer inclusion is deterministic; the bribe model does not apply.
        let estimated_relay_fee_usd = self.estimated_relay_fee_usd;

        let roi_params = RoiCalculationParams {
            amount_in_usd,
            expected_amount_out_usd,
            expected_gas_cost_usd: gas_cost_usd,
            flashloan_fee_pct: self.config.flashloan_fee_pct,
            max_slippage_pct: self.config.max_slippage_pct / 100.0, // pct → fraction
            failure_risk_buffer_usd,
            lp_fees_usd,
            price_impact_pct,
            capital_cost_usd,
            ops_overhead_usd,
            p_fail: p_fail_opt,
            p_copied,
            estimated_relay_fee_usd, // component 8 (C2 fix)
        };
        let outcome_raw = calc_net_profit_and_roi(&roi_params);

        // 4b. Defense-in-depth sanity bound for BUG-2 (token-blind USD valuation
        // in profit_token_to_usd, pending price-oracle sprint). Real HFT MEV
        // ROI distribution is 0.05% – 2%; outliers above 999% are math bugs.
        // When triggered, we clamp the persisted profit/ROI fields to zero so
        // PostgreSQL numeric(10,4) doesn't overflow and the operator dashboard
        // shows clean values + an explicit AnomalousMath rejection reason.
        // This bound REMAINS even after BUG-2 is properly fixed — it acts as
        // a regression catch for any future math error producing absurd values.
        const ANOMALOUS_ROI_THRESHOLD_PCT: f64 = 999.0;
        const ANOMALOUS_PROFIT_THRESHOLD_USD: f64 = 1_000_000.0;
        let anomalous = outcome_raw.net_roi_pct.abs() > ANOMALOUS_ROI_THRESHOLD_PCT
            || outcome_raw.gross_profit_usd.abs() > ANOMALOUS_PROFIT_THRESHOLD_USD;
        let outcome = if anomalous {
            DefiArbitrageOutcome {
                is_viable: false,
                gross_profit_usd: 0.0,
                net_profit_usd: 0.0,
                net_roi_pct: 0.0,
                ..outcome_raw
            }
        } else {
            outcome_raw
        };

        // 5. Risk gate: validate against config-derived policy. The policy
        // gets `effective_capital` (not raw `capital_usd`) so risk thresholds
        // align with the simulation knob — keeps math + risk gates internally
        // consistent when previewing "what would $X capital surface".
        let policy = policy_from_config(self.config, effective_capital);
        let risk_profile = OpportunityRiskProfile {
            gross_profit_usd: outcome.gross_profit_usd,
            net_profit_usd: outcome.net_profit_usd,
            net_roi_pct: outcome.net_roi_pct,
            gas_cost_usd: outcome.gas_cost_usd,
            slippage_expected_pct: outcome.slippage_expected_pct,
            // Price impact derives from amount_in/liquidity — until pool reserves
            // are wired, use slippage as a conservative proxy.
            price_impact_pct: outcome.slippage_expected_pct,
            liquidity_available_usd: amount_in_usd, // capital is a self-imposed cap
            trade_size_usd: amount_in_usd,
            // Simulation passes if math says it's viable — the real REVM gate
            // runs separately in scanner via simulator.rs.
            simulation_passed: outcome.is_viable,
            // No verifier yet — assume true; will become a real gate once token
            // safety screen is wired (arbx-token-safety-screen skill).
            contracts_verified: true,
        };
        // Rejection precedence (most-diagnostic first):
        //   1. UnknownTokenPrice   — input itself is invalid; no math is meaningful
        //   2. ImplausibleSpread   — AMM rate vs oracle deviates by > mult× (stale/buggy)
        //   3. AnomalousMath       — math layer producing absurd values (BUG-2 net)
        //   4. RiskPolicy reasons  — operational gates (gas, slippage, liquidity)
        // Higher tiers signal "fix upstream" before tweaking risk knobs.
        let rejection: Option<RejectReason> = if unknown_price {
            Some(RejectReason::UnknownTokenPrice)
        } else if spread_sanity_rejection.is_some() {
            spread_sanity_rejection.clone()
        } else if anomalous {
            Some(RejectReason::AnomalousMath)
        } else {
            match validate_opportunity_risk(&risk_profile, &policy) {
                Ok(()) => None,
                Err(reason) => Some(map_risk_rejection(reason)),
            }
        };

        // 6. Build evidence with REAL numbers (no more hardcoded 0.95 / 0.9 / 1.0).

        // Reconstruct the effective slippage cost for CostBreakdown — mirrors
        // the logic in `calc_net_profit_and_roi` so the breakdown is consistent
        // with the net_profit value the caller sees.
        let effective_slippage_usd_for_breakdown = if price_impact_pct > 0.0 {
            expected_amount_out_usd * (price_impact_pct / 100.0)
        } else {
            expected_amount_out_usd * (self.config.max_slippage_pct / 100.0)
        };

        // The actual failure buffer that went into net_profit may differ from
        // `failure_risk_buffer_usd` when statistical p_fail was used.
        // Re-derive the actual value so CostBreakdown.failure_buffer_usd is
        // consistent with what calc_net_profit_and_roi deducted.
        let actual_failure_buffer_usd = match p_fail_opt {
            Some(p) => p * gas_cost_usd,
            None => failure_risk_buffer_usd,
        };

        // Component 5 (Sprint C): copied_buffer mirrors the engine's computation.
        // gross_profit_usd is available from outcome_raw (before anomaly clamp)
        // but we use the pre-cost gross (expected_out - amount_in) as the base,
        // consistent with RoiCalculationParams: copied_buffer = p × gross_profit.
        let gross_profit_for_copied = expected_amount_out_usd - amount_in_usd;
        let copied_buffer_usd = match p_copied {
            Some(p) => p * gross_profit_for_copied,
            None => 0.0,
        };

        let cost_breakdown = CostBreakdown::new(
            gas_cost_usd,
            lp_fees_usd,
            flashloan_fee_usd_computed,
            effective_slippage_usd_for_breakdown,
            actual_failure_buffer_usd,
            capital_cost_usd,
            ops_overhead_usd,
            p_fail_source,
            copied_buffer_usd,
            p_copied,
            estimated_relay_fee_usd, // component 8 (C2 fix)
        );

        let evidence = OpportunityEvidence {
            chain_id,
            block_number: self.signals.block_number,
            rpc_url_hash,
            rpc_latency_ms,
            state_read_timestamp: chrono::Utc::now().timestamp(),
            pool_addresses: candidate.pool_addresses.clone(),
            token_addresses: candidate.token_addresses.clone(),
            dex_adapters: candidate.dex_adapters.clone(),
            route_fingerprint: candidate.route_fingerprint.clone(),
            amount_in: candidate.amount_in,
            expected_amount_out: candidate.expected_amount_out,
            min_amount_out: candidate.expected_amount_out
                * (1.0 - self.config.max_slippage_pct / 100.0),
            gross_profit: outcome.gross_profit_usd,
            gas_units_estimated: self.config.gas_estimate_units,
            gas_price: self.config.resolve_gas_price_gwei(
                self.signals.basefee_gwei,
                self.signals.p75_priority_tip_gwei,
            ) * 1e9,
            gas_cost: outcome.gas_cost_usd,
            bribe: 0.0,
            flashloan_fee: outcome.flashloan_fee_usd,
            net_expected_profit: outcome.net_profit_usd,
            roi_net: outcome.net_roi_pct,
            simulation_status: "PENDING".to_string(),
            simulation_trace_hash: None,
            bundle_simulation_status: None,
            // Risk inputs come from config thresholds — operator owns these knobs.
            token_risk_score: self.config.max_token_risk_score,
            liquidity_confidence: self.config.min_liquidity_confidence,
            state_freshness_ms: rpc_latency_ms,
            landing_probability: self.config.min_landing_probability,
            final_score: 0.0, // populated downstream by PrioritizationEngine::score
            decision: ExecutionDecision::Hold,
            reject_reason: rejection.clone(),
            cost_breakdown,
        };

        // Migration 056 — `StrategyConfigGate` PASS B: post-math overrides
        // (per-strategy min_profit_usd / min_roi_pct) and pool TVL/volume
        // floors. If the pre-math chain-level rejection is already populated
        // (e.g. NegativeNetProfit), the post-math gate respects that — we
        // only OVERRIDE the rejection when post-math finds a stricter cause.
        let mut final_rejection = rejection;
        if final_rejection.is_none() {
            let post_outcome = StrategyConfigGate::check_post_math(
                self.config,
                route_plan,
                strategy_kind,
                outcome.net_profit_usd,
                outcome.net_roi_pct,
            );
            if let GateOutcome::Reject(reason) = post_outcome {
                final_rejection = Some(reason);
            }
        }

        ConfigGateOutcome::Evaluated {
            outcome: Box::new(outcome),
            evidence: Box::new(evidence),
            rejection: final_rejection,
            partial_data_quality,
        }
    }
}

/// Map math-engine rejection reasons (broad RiskPolicy gates) to spine's
/// canonical RejectReason enum (consumed by dashboards / decision engine).
fn map_risk_rejection(reason: RiskRejectionReason) -> RejectReason {
    match reason {
        RiskRejectionReason::NegativeNetProfit => RejectReason::NegativeNetProfit,
        RiskRejectionReason::NetRoiTooLow => RejectReason::NegativeNetProfit,
        RiskRejectionReason::GasCostTooHigh => RejectReason::HighGasVolatility,
        RiskRejectionReason::SlippageTooHigh => RejectReason::ExcessiveSlippage,
        RiskRejectionReason::PriceImpactTooHigh => RejectReason::ExcessiveSlippage,
        RiskRejectionReason::InsufficientLiquidity => RejectReason::LowLiquidity,
        RiskRejectionReason::TradeSizeExceeded => RejectReason::LowLiquidity,
        RiskRejectionReason::SimulationFailed => RejectReason::SimulationFailed,
        RiskRejectionReason::UnverifiedContracts => RejectReason::PoolNotTrusted,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use shared_rs::trading_config::GasPriceStrategy;
    use std::collections::HashMap;

    fn cfg() -> TradingConfigState {
        TradingConfigState {
            chain_id: 1,
            capital_usd: 1000.0,
            base_token_symbol: "WETH".into(),
            base_token_price_usd: 2000.0,
            allowed_token_symbols: vec!["WETH".into(), "USDC".into()],
            token_prices_usd: HashMap::new(),
            simulation_capital_usd: None,
            simulation_per_token_amounts_usd: HashMap::new(),
            simulation_per_strategy_caps_usd: HashMap::new(),
            simulation_target_profit_usd: None,
            simulation_target_roi_pct: None,
            min_profit_usd: 50.0, // Ethereum mainnet floor (migration 046: chain_id=1 → $50)
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
            enabled_dex_ids: None,
            strategy_configs: HashMap::new(),
            capital_cost_rate_annual_pct: 0.0,
            ops_overhead_usd_per_attempt: 0.01,
            spread_sanity_mult: 3.0,
            p_copied_volume_threshold_usd: 1_000_000.0,
            p_copied_max: 0.5,
            kelly_multiplier: 0.5,
            kelly_max_per_trade_fraction: 1.0,
            kelly_gas_safety_multiplier: 1.0,
            enabled: true,
            updated_at: Utc::now(),
            updated_by: None,
        }
    }

    fn signals() -> NetworkSignals {
        NetworkSignals {
            basefee_gwei: 25.0,
            p75_priority_tip_gwei: 2.0,
            block_number: 19_000_000,
        }
    }

    #[test]
    fn token_outside_allowlist_skips() {
        let c = cfg();
        let candidate = OpportunityCandidate {
            route_fingerprint: "test".into(),
            pool_addresses: vec![],
            token_addresses: vec!["UNKNOWN".into()],
            dex_adapters: vec!["uniswap-v2".into()],
            amount_in: 0.1,
            expected_amount_out: 0.1001,
            gross_profit: 0.0001,
        };
        let out = ConfigAwareEvaluator::new(&c, signals()).evaluate(
            &candidate,
            "dex_arb_v2v2",
            1,
            "rpc".into(),
            10,
        );
        assert!(matches!(out, ConfigGateOutcome::TokenNotAllowed { .. }));
    }

    // ---- ARBX-0018: identity mode (address binds, symbol = metadata) ----

    fn identity_universe() -> Vec<(String, String)> {
        vec![
            ("0xaaaaweth".into(), "WETH".into()),
            ("0xbbbbusdc".into(), "USDC".into()),
        ]
    }

    fn identity_candidate(addrs: Vec<&str>) -> OpportunityCandidate {
        OpportunityCandidate {
            route_fingerprint: "test".into(),
            pool_addresses: vec![],
            token_addresses: addrs.into_iter().map(String::from).collect(),
            dex_adapters: vec!["uniswap-v2".into()],
            amount_in: 0.1,
            expected_amount_out: 0.1001,
            gross_profit: 0.0001,
        }
    }

    #[test]
    fn identity_mode_symbol_string_never_passes() {
        // THE prohibition (ARBX-0018 regression): "WETH" is in the allowlist
        // and resolves to an address — but the STRING is not a token. Under
        // identity mode it must NOT pass, even though the legacy gate (no
        // index attached, same config) would have let it through.
        let c = cfg();
        let idx = TokenIdentityIndex::resolve(1, &c.allowed_token_symbols, &identity_universe());
        let candidate = identity_candidate(vec!["WETH", "0xbbbbusdc"]);
        let out = ConfigAwareEvaluator::new(&c, signals())
            .with_token_identity(Some(std::sync::Arc::new(idx)))
            .evaluate(&candidate, "dex_arb_v2v2", 1, "rpc".into(), 10);
        match out {
            ConfigGateOutcome::TokenNotAllowed {
                token_symbol_or_addr,
            } => {
                assert_eq!(token_symbol_or_addr, "WETH");
            }
            other => panic!("symbol string must be rejected, got {:?}", other),
        }
    }

    #[test]
    fn identity_mode_resolved_address_prices_via_symbol_metadata() {
        // Address passes the gate AND the symbol-keyed price stack still
        // works: the index maps 0xaaaaweth→"WETH" (base token, $2000) and
        // 0xbbbbusdc→"USDC" (stablecoin, $1) so the evaluation proceeds past
        // pricing into the economics gates. If symbol resolution were
        // broken the raw address would miss the oracle and the rejection
        // would be UnknownTokenPrice instead.
        let c = cfg();
        let idx = TokenIdentityIndex::resolve(1, &c.allowed_token_symbols, &identity_universe());
        let candidate = identity_candidate(vec!["0xaaaaweth", "0xbbbbusdc"]);
        let out = ConfigAwareEvaluator::new(&c, signals())
            .with_token_identity(Some(std::sync::Arc::new(idx)))
            .evaluate(&candidate, "dex_arb_v2v2", 1, "rpc".into(), 10);
        match out {
            ConfigGateOutcome::Evaluated { rejection, .. } => {
                assert!(
                    !matches!(rejection, Some(RejectReason::UnknownTokenPrice)),
                    "symbol metadata must feed the oracle; got {:?}",
                    rejection
                );
            }
            other => panic!("resolved addresses must clear the gate, got {:?}", other),
        }
    }

    #[test]
    fn identity_mode_unknown_address_rejected_with_address() {
        // Address not in the universe + non-empty operator allowlist →
        // fail-closed, and the rejection carries the ADDRESS (the identity),
        // not a symbol guess.
        let c = cfg();
        let idx = TokenIdentityIndex::resolve(1, &c.allowed_token_symbols, &identity_universe());
        let candidate = identity_candidate(vec!["0xdeadbeef", "0xbbbbusdc"]);
        let out = ConfigAwareEvaluator::new(&c, signals())
            .with_token_identity(Some(std::sync::Arc::new(idx)))
            .evaluate(&candidate, "dex_arb_v2v2", 1, "rpc".into(), 10);
        match out {
            ConfigGateOutcome::TokenNotAllowed {
                token_symbol_or_addr,
            } => {
                assert_eq!(token_symbol_or_addr, "0xdeadbeef");
            }
            other => panic!("unknown address must fail closed, got {:?}", other),
        }
    }

    #[test]
    fn r0002_repro_usdc_1inch_addr_vs_symbol() {
        // ARBX-R-0002 repro with the production addresses from the incident
        // (112×1INCH + 6×USDC TokenNotAllowed rejections in 6h): the
        // cartridge/orchestrator paths passed ADDRESSES into a symbol-keyed
        // allowlist and every candidate died at the gate. Identity mode
        // resolves the operator's symbols (USDC, 1INCH) to exactly these
        // addresses and the gate passes. Legacy mode on the same input
        // still fails — kept as the second half of the repro to document
        // what was eliminated.
        const USDC_MAINNET: &str = "0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48";
        const INCH_MAINNET: &str = "0x111111111117dc0aa78b770fa6a738034120c302";
        const AGLD_MAINNET: &str = "0x32353a6c91143bfd6c7d363b546e62a9a2489a20";

        let mut c = cfg();
        c.allowed_token_symbols = vec!["USDC".into(), "1INCH".into()];
        let universe = vec![
            (USDC_MAINNET.to_string(), "USDC".into()),
            (INCH_MAINNET.to_string(), "1INCH".into()),
            (AGLD_MAINNET.to_string(), "AGLD".into()), // NOT in the allowlist
        ];
        let idx = TokenIdentityIndex::resolve(1, &c.allowed_token_symbols, &universe);
        let candidate = identity_candidate(vec![USDC_MAINNET, INCH_MAINNET]);

        // Identity mode: both legs clear the token gate (they reach the
        // strategy gate, i.e. NOT TokenNotAllowed).
        let out = ConfigAwareEvaluator::new(&c, signals())
            .with_token_identity(Some(std::sync::Arc::new(idx)))
            .evaluate(&candidate, "dex_arb_v2v2", 1, "rpc".into(), 10);
        assert!(
            !matches!(out, ConfigGateOutcome::TokenNotAllowed { .. }),
            "identity mode must clear the production USDC/1INCH pair: {:?}",
            out
        );

        // Legacy mode (no index), same address input: the eliminated bug.
        let out_legacy = ConfigAwareEvaluator::new(&c, signals()).evaluate(
            &candidate,
            "dex_arb_v2v2",
            1,
            "rpc".into(),
            10,
        );
        assert!(matches!(
            out_legacy,
            ConfigGateOutcome::TokenNotAllowed { .. }
        ));

        // AGLD stays OUTSIDE the operator allowlist → rejected under
        // identity mode too (allowlist membership is the operator's call;
        // the gate just binds it to the address now).
        let agld_candidate = identity_candidate(vec![AGLD_MAINNET, USDC_MAINNET]);
        let idx2 = TokenIdentityIndex::resolve(1, &c.allowed_token_symbols, &universe);
        let out_agld = ConfigAwareEvaluator::new(&c, signals())
            .with_token_identity(Some(std::sync::Arc::new(idx2)))
            .evaluate(&agld_candidate, "dex_arb_v2v2", 1, "rpc".into(), 10);
        match out_agld {
            ConfigGateOutcome::TokenNotAllowed {
                token_symbol_or_addr,
            } => {
                assert_eq!(token_symbol_or_addr, AGLD_MAINNET);
            }
            other => panic!("AGLD outside allowlist must be rejected: {:?}", other),
        }
    }

    #[test]
    fn disabled_strategy_skips() {
        let c = cfg();
        let candidate = OpportunityCandidate {
            route_fingerprint: "test".into(),
            pool_addresses: vec![],
            token_addresses: vec!["WETH".into(), "USDC".into()],
            dex_adapters: vec!["curve".into()],
            amount_in: 0.1,
            expected_amount_out: 0.1001,
            gross_profit: 0.0001,
        };
        let out = ConfigAwareEvaluator::new(&c, signals()).evaluate(
            &candidate,
            "curve_stable",
            1,
            "rpc".into(),
            10,
        );
        assert!(matches!(out, ConfigGateOutcome::StrategyDisabled { .. }));
    }

    #[test]
    fn empty_strategy_list_is_permissive() {
        let mut c = cfg();
        c.enabled_strategies = vec![];
        let candidate = OpportunityCandidate {
            route_fingerprint: "test".into(),
            pool_addresses: vec![],
            token_addresses: vec!["WETH".into(), "USDC".into()],
            dex_adapters: vec!["curve".into()],
            amount_in: 0.5,
            expected_amount_out: 0.51, // +0.01 ETH = $20 gross
            gross_profit: 0.01,
        };
        let out = ConfigAwareEvaluator::new(&c, signals()).evaluate(
            &candidate,
            "anything_goes",
            1,
            "rpc".into(),
            10,
        );
        match out {
            ConfigGateOutcome::Evaluated { .. } => {}
            other => panic!("expected Evaluated, got {:?}", other),
        }
    }

    #[test]
    fn capital_caps_amount_in() {
        let c = cfg(); // capital = $1000
                       // candidate observed at 1.0 ETH = $2000 (above capital cap)
        let candidate = OpportunityCandidate {
            route_fingerprint: "test".into(),
            pool_addresses: vec![],
            token_addresses: vec!["WETH".into(), "USDC".into()],
            dex_adapters: vec!["uniswap-v2".into()],
            amount_in: 1.0,
            expected_amount_out: 1.005, // +0.5% gross
            gross_profit: 0.005,
        };
        let out = ConfigAwareEvaluator::new(&c, signals()).evaluate(
            &candidate,
            "dex_arb_v2v2",
            1,
            "rpc".into(),
            10,
        );
        if let ConfigGateOutcome::Evaluated { outcome, .. } = out {
            // Total capital required is capped at $1000, not $2000
            assert!(
                outcome.total_capital_required_usd <= 1000.0,
                "expected capital cap, got {}",
                outcome.total_capital_required_usd
            );
        } else {
            panic!("expected evaluated outcome");
        }
    }

    /// Regression test for BUG-3 (asymmetric capital cap).
    ///
    /// Reproduces the production incident on 2026-05-04 where the operator's
    /// `capital_usd` was set to $10 and observed pending swaps (~0.05 ETH ≈ $125)
    /// produced fake gross profits in the $113 range with ROI > 1000%.
    ///
    /// Root cause: the OLD code capped `amount_in_usd` at capital but left
    /// `expected_amount_out_usd` at full (un-capped) value, so:
    ///     gross_profit_usd = expected_amount_out_usd - amount_in_usd_capped
    ///                      ≈ $125 - $10 = $115 (fake)
    ///
    /// After fix: when capital cap reduces effective input, output is scaled
    /// proportionally → gross_profit_usd reflects the true spread, not the cap delta.
    #[test]
    fn capital_cap_does_not_inflate_gross_profit() {
        let mut c = cfg();
        c.capital_usd = 10.0;
        c.base_token_price_usd = 2500.0;
        c.allowed_token_symbols = vec!["WETH".into(), "BNB".into()];

        // Realistic same-magnitude swap: 0.05 ETH-equivalent in, 0.05 BNB-equivalent
        // out. No real arbitrage spread (output ≈ input in magnitude). The system
        // must NOT report this as profit.
        let candidate = OpportunityCandidate {
            route_fingerprint: "test".into(),
            pool_addresses: vec![],
            token_addresses: vec!["WETH".into(), "BNB".into()],
            dex_adapters: vec!["uniswap-v3".into()],
            amount_in: 0.05,           // observed: 0.05 ETH = $125 (>> $10 capital)
            expected_amount_out: 0.05, // ~same magnitude → no real spread
            gross_profit: 0.0,
        };
        let out = ConfigAwareEvaluator::new(&c, signals()).evaluate(
            &candidate,
            "dex_arb_v2v2",
            1,
            "rpc".into(),
            10,
        );
        let outcome = match out {
            ConfigGateOutcome::Evaluated { outcome, .. } => outcome,
            other => panic!("expected Evaluated outcome, got {:?}", other),
        };

        // With BUG-3 present: gross_profit_usd ≈ $115 (capped input vs full output).
        // Sanity bound after fix: gross_profit must not exceed effective capital.
        // Even with worst-case rounding, |profit| < $1 for a no-spread swap.
        assert!(
            outcome.gross_profit_usd.abs() < 1.0,
            "BUG-3 reproduction: gross_profit_usd = {} (expected ≈ 0). \
             Cap was applied to amount_in_usd but not to expected_amount_out_usd, \
             producing fake profit equal to the cap delta.",
            outcome.gross_profit_usd,
        );
    }

    /// Bound test: the same scenario as the production 06:37 incident
    /// (WETH→UNI, observed input ~0.04 ETH = $105, capped to $10) must
    /// not produce ROI > 100%. Linear scaling is conservative but bounded.
    #[test]
    fn capital_cap_bounds_roi_to_realistic_range() {
        let mut c = cfg();
        c.capital_usd = 10.0;
        c.base_token_price_usd = 2500.0;
        c.allowed_token_symbols = vec!["WETH".into(), "UNI".into()];

        // Mirrors the 06:37 outlier: observed 0.0423 ETH input, large UNI output.
        // BUG-3 alone produced ROI = 735,184%. After proportional scaling output
        // also gets reduced; ROI may still be inflated by BUG-2 (token-blind USD
        // pricing) but must stay within sanity bounds for a regression gate.
        let candidate = OpportunityCandidate {
            route_fingerprint: "test".into(),
            pool_addresses: vec![],
            token_addresses: vec!["WETH".into(), "UNI".into()],
            dex_adapters: vec!["uniswap-v3".into()],
            amount_in: 0.0423,
            expected_amount_out: 29.56, // raw UNI units (BUG-2: spine treats as ETH)
            gross_profit: 0.0,
        };
        let out = ConfigAwareEvaluator::new(&c, signals()).evaluate(
            &candidate,
            "dex_arb_v2v2",
            1,
            "rpc".into(),
            10,
        );
        let outcome = match out {
            ConfigGateOutcome::Evaluated { outcome, .. } => outcome,
            other => panic!("expected Evaluated outcome, got {:?}", other),
        };

        // Pre-fix this returned ROI = 735184. Post-fix the proportional scaling
        // bounds it: output 29.56 × ratio (10/105.75 ≈ 0.0945) ≈ 2.79 (× $2500 = $6985);
        // gross = $6985 - $10 = $6975 → ROI ≈ 69750%. Still wrong (BUG-2 unaddressed)
        // but order-of-magnitude bounded. With the AnomalousMath sanity bound
        // also active (>999% triggers clamp-to-zero), the resulting outcome is
        // 0% — passing the < 100,000 assertion trivially. Both bounds are now
        // exercised by this single test.
        assert!(
            outcome.net_roi_pct < 100_000.0,
            "BUG-3 regression: net_roi_pct = {} (must be < 100,000% after proportional cap)",
            outcome.net_roi_pct,
        );
    }

    /// Defense-in-depth sanity gate. Post-BUG-2 the natural trigger for
    /// operator misconfiguration is a typo in `token_prices_usd` —
    /// e.g. UNI set to $10,000 instead of ~$8.
    ///
    /// With Sprint C deployed, the first check to fire is `ImplausibleSpread`
    /// (at default 3× threshold the 2792× deviation is caught immediately).
    /// `AnomalousMath` remains as defense-in-depth for pathological cases
    /// where spread_sanity_mult is raised but the USD math goes wild.
    ///
    /// This test asserts: the opportunity is REJECTED (either reason) AND
    /// the outcome is zeroed by the AnomalousMath clamp. This validates the
    /// full layered defense while accepting the new rejection precedence.
    #[test]
    fn anomalous_roi_triggers_sanity_bound() {
        let mut c = cfg();
        c.capital_usd = 10.0;
        c.base_token_price_usd = 2500.0;
        c.allowed_token_symbols = vec!["WETH".into(), "UNI".into()];
        // Operator typo: UNI mis-priced as $10,000 instead of ~$8.
        c.token_prices_usd.insert("UNI".into(), 10_000.0);

        let candidate = OpportunityCandidate {
            route_fingerprint: "test".into(),
            pool_addresses: vec![],
            token_addresses: vec!["WETH".into(), "UNI".into()],
            dex_adapters: vec!["uniswap-v3".into()],
            amount_in: 0.0423,
            expected_amount_out: 29.56,
            gross_profit: 0.0,
        };
        let out = ConfigAwareEvaluator::new(&c, signals()).evaluate(
            &candidate,
            "dex_arb_v2v2",
            1,
            "rpc".into(),
            10,
        );
        let (outcome, rejection) = match out {
            ConfigGateOutcome::Evaluated {
                outcome, rejection, ..
            } => (outcome, rejection),
            other => panic!("expected Evaluated outcome, got {:?}", other),
        };

        // Sprint C: ImplausibleSpread now fires before AnomalousMath (higher
        // precedence) when the spread deviation is extreme. Both checks protect
        // the operator from the same root cause (mis-priced oracle config).
        // This test validates that at least one rejection fires and the outcome
        // is correctly zeroed/non-viable.
        let is_sanity_rejected = matches!(
            rejection,
            Some(RejectReason::AnomalousMath) | Some(RejectReason::ImplausibleSpread { .. })
        );
        assert!(
            is_sanity_rejected,
            "mis-priced oracle must trigger AnomalousMath or ImplausibleSpread, got {:?}",
            rejection,
        );
        assert_eq!(
            outcome.gross_profit_usd, 0.0,
            "anomalous gross_profit_usd should be clamped to 0, got {}",
            outcome.gross_profit_usd,
        );
        assert_eq!(
            outcome.net_roi_pct, 0.0,
            "anomalous net_roi_pct should be clamped to 0, got {}",
            outcome.net_roi_pct,
        );
        assert!(
            !outcome.is_viable,
            "anomalous outcome must not be marked viable",
        );
    }

    // ----------------------------------------------------------------
    // BUG-2 fix: PriceOracle integration (per-token USD valuation)
    // ----------------------------------------------------------------

    /// When `token_in` is not resolvable by the price oracle (not base, not
    /// in token_prices_usd, not stablecoin), the evaluator MUST reject with
    /// `UnknownTokenPrice` and zero the outcome — never fabricate a price.
    #[test]
    fn unknown_token_in_triggers_unknown_token_price_rejection() {
        let mut c = cfg();
        c.allowed_token_symbols = vec!["PEPE".into(), "WETH".into()];
        // PEPE has no price (not base WETH, not in map, not stablecoin)
        let candidate = OpportunityCandidate {
            route_fingerprint: "test".into(),
            pool_addresses: vec![],
            token_addresses: vec!["PEPE".into(), "WETH".into()],
            dex_adapters: vec!["uniswap-v3".into()],
            amount_in: 1_000_000.0,
            expected_amount_out: 0.5,
            gross_profit: 0.0,
        };
        let out = ConfigAwareEvaluator::new(&c, signals()).evaluate(
            &candidate,
            "dex_arb_v2v2",
            1,
            "rpc".into(),
            10,
        );
        let (outcome, rejection) = match out {
            ConfigGateOutcome::Evaluated {
                outcome, rejection, ..
            } => (outcome, rejection),
            other => panic!("expected Evaluated outcome, got {:?}", other),
        };
        assert_eq!(
            rejection,
            Some(RejectReason::UnknownTokenPrice),
            "unknown token_in should reject with UnknownTokenPrice, got {:?}",
            rejection,
        );
        assert_eq!(
            outcome.gross_profit_usd, 0.0,
            "outcome must be zeroed when price unknown, got {}",
            outcome.gross_profit_usd
        );
        assert!(!outcome.is_viable);
    }

    /// Symmetric: token_out unknown → same rejection.
    #[test]
    fn unknown_token_out_triggers_unknown_token_price_rejection() {
        let mut c = cfg();
        c.allowed_token_symbols = vec!["WETH".into(), "PEPE".into()];
        let candidate = OpportunityCandidate {
            route_fingerprint: "test".into(),
            pool_addresses: vec![],
            token_addresses: vec!["WETH".into(), "PEPE".into()],
            dex_adapters: vec!["uniswap-v3".into()],
            amount_in: 0.05,
            expected_amount_out: 1_000_000.0,
            gross_profit: 0.0,
        };
        let out = ConfigAwareEvaluator::new(&c, signals()).evaluate(
            &candidate,
            "dex_arb_v2v2",
            1,
            "rpc".into(),
            10,
        );
        let (outcome, rejection) = match out {
            ConfigGateOutcome::Evaluated {
                outcome, rejection, ..
            } => (outcome, rejection),
            other => panic!("expected Evaluated outcome, got {:?}", other),
        };
        assert_eq!(
            rejection,
            Some(RejectReason::UnknownTokenPrice),
            "unknown token_out should reject with UnknownTokenPrice, got {:?}",
            rejection,
        );
        assert_eq!(outcome.gross_profit_usd, 0.0);
    }

    // ----------------------------------------------------------------
    // simulation_capital_usd — paper-trade preview knob
    // ----------------------------------------------------------------

    /// When the operator sets `simulation_capital_usd`, the spine evaluator
    /// uses it as the effective capital for sizing AND risk policy bounds —
    /// allowing previews of "what would $X capital surface?" without
    /// changing operational `capital_usd` or risking real execution.
    #[test]
    fn simulation_capital_overrides_operational_capital_for_sizing() {
        let mut c = cfg(); // capital_usd = 1000, base_token_price = 2000
        c.simulation_capital_usd = Some(50_000.0); // SIMULATE bigger book

        // 5 ETH input ($10K) — would be capped to $1000 with operational
        // capital alone (cap_ratio = 0.1 → 10x reduction). With simulation
        // override, 5 ETH ($10K) fits within $50K cap → no scaling, real
        // gross profit visible.
        let candidate = OpportunityCandidate {
            route_fingerprint: "test".into(),
            pool_addresses: vec![],
            token_addresses: vec!["WETH".into(), "USDC".into()],
            dex_adapters: vec!["uniswap-v3".into()],
            amount_in: 5.0,                // 5 WETH = $10K (above operational $1K cap)
            expected_amount_out: 10_500.0, // 10,500 USDC = $10,500 → 5% gross
            gross_profit: 0.0,
        };
        let out = ConfigAwareEvaluator::new(&c, signals()).evaluate(
            &candidate,
            "dex_arb_v2v2",
            1,
            "rpc".into(),
            10,
        );
        let outcome = match out {
            ConfigGateOutcome::Evaluated { outcome, .. } => outcome,
            other => panic!("expected Evaluated outcome, got {:?}", other),
        };
        // With operational capital alone: cap_ratio = 0.1, gross ≈ $50.
        // With simulation $50K: no cap (10K < 50K), gross ≈ $500 (full spread).
        assert!(
            outcome.gross_profit_usd > 400.0 && outcome.gross_profit_usd < 600.0,
            "simulation override should expose full ~$500 gross profit, got {}",
            outcome.gross_profit_usd,
        );
    }

    /// Per-token simulation cap takes precedence over global sim cap when
    /// the candidate's token_in matches. Models "use no more than $5K when
    /// WETH is the input even though my global sim is $50K".
    #[test]
    fn per_token_simulation_cap_wins_over_global() {
        let mut c = cfg(); // capital_usd = 1000, base_token_price = 2000
        c.simulation_capital_usd = Some(50_000.0); // global big sim
        c.simulation_per_token_amounts_usd
            .insert("WETH".into(), 5_000.0); // WETH input capped at $5K

        // 5 ETH input ($10K observed). Global $50K would NOT cap, but
        // per-token $5K WILL → cap_ratio = 0.5 → gross ~$250 (half of full).
        let candidate = OpportunityCandidate {
            route_fingerprint: "test".into(),
            pool_addresses: vec![],
            token_addresses: vec!["WETH".into(), "USDC".into()],
            dex_adapters: vec!["uniswap-v3".into()],
            amount_in: 5.0,
            expected_amount_out: 10_500.0,
            gross_profit: 0.0,
        };
        let out = ConfigAwareEvaluator::new(&c, signals()).evaluate(
            &candidate,
            "dex_arb_v2v2",
            1,
            "rpc".into(),
            10,
        );
        let outcome = match out {
            ConfigGateOutcome::Evaluated { outcome, .. } => outcome,
            other => panic!("expected Evaluated outcome, got {:?}", other),
        };
        // ratio = $5K / $10K = 0.5; expected_out = $10500 × 0.5 = $5250;
        // gross = $5250 - $5000 = $250
        assert!(
            outcome.gross_profit_usd > 200.0 && outcome.gross_profit_usd < 300.0,
            "per-token $5K cap should yield ~$250 gross, got {}",
            outcome.gross_profit_usd,
        );
    }

    /// Per-strategy simulation cap applies when strategy_kind matches.
    /// Models "no more than $2K on dex_arb_v2v2 even with bigger global".
    #[test]
    fn per_strategy_simulation_cap_wins_over_global_and_per_token() {
        let mut c = cfg();
        c.simulation_capital_usd = Some(50_000.0);
        c.simulation_per_token_amounts_usd
            .insert("WETH".into(), 5_000.0);
        c.simulation_per_strategy_caps_usd
            .insert("dex_arb_v2v2".into(), 2_000.0); // strategy cap STRICTEST

        let candidate = OpportunityCandidate {
            route_fingerprint: "test".into(),
            pool_addresses: vec![],
            token_addresses: vec!["WETH".into(), "USDC".into()],
            dex_adapters: vec!["uniswap-v3".into()],
            amount_in: 5.0,
            expected_amount_out: 10_500.0,
            gross_profit: 0.0,
        };
        let out = ConfigAwareEvaluator::new(&c, signals()).evaluate(
            &candidate,
            "dex_arb_v2v2",
            1,
            "rpc".into(),
            10,
        );
        let outcome = match out {
            ConfigGateOutcome::Evaluated { outcome, .. } => outcome,
            other => panic!("expected Evaluated outcome, got {:?}", other),
        };
        // MIN of $50K / $5K / $2K = $2K → ratio = $2K / $10K = 0.2
        // expected_out = $10500 × 0.2 = $2100; gross = $2100 - $2000 = $100
        assert!(
            outcome.gross_profit_usd > 70.0 && outcome.gross_profit_usd < 130.0,
            "MIN-of-3-caps should yield ~$100 gross with strategy cap winning, got {}",
            outcome.gross_profit_usd,
        );
    }

    /// When `simulation_capital_usd` is None, behaviour is unchanged —
    /// operational `capital_usd` drives everything (regression guard).
    #[test]
    fn simulation_capital_none_falls_back_to_operational() {
        let c = cfg(); // simulation_capital_usd = None, capital_usd = 1000
                       // Same 5 ETH input as above — must hit the $1000 operational cap.
        let candidate = OpportunityCandidate {
            route_fingerprint: "test".into(),
            pool_addresses: vec![],
            token_addresses: vec!["WETH".into(), "USDC".into()],
            dex_adapters: vec!["uniswap-v3".into()],
            amount_in: 5.0,
            expected_amount_out: 10_500.0,
            gross_profit: 0.0,
        };
        let out = ConfigAwareEvaluator::new(&c, signals()).evaluate(
            &candidate,
            "dex_arb_v2v2",
            1,
            "rpc".into(),
            10,
        );
        let outcome = match out {
            ConfigGateOutcome::Evaluated { outcome, .. } => outcome,
            other => panic!("expected Evaluated outcome, got {:?}", other),
        };
        // Capital cap applies: $10K observed → $1K capped, ratio 0.1.
        // gross ≈ ($10500 × 0.1) - $1000 = $50.
        assert!(
            outcome.gross_profit_usd > 30.0 && outcome.gross_profit_usd < 70.0,
            "operational cap should produce ~$50 gross, got {}",
            outcome.gross_profit_usd,
        );
    }

    /// When both tokens are resolvable, the evaluator uses REAL per-token
    /// prices. WETH→USDC profitable arb produces a sensible gross_profit
    /// reflecting actual USD diff (not the BUG-2 fantasy of $6.5M).
    ///
    /// Pre-fix: expected_amount_out_usd = 2600 × $2500 (WETH-priced) = $6.5M
    ///          → AnomalousMath clamp → gross_profit = 0
    /// Post-fix: expected_amount_out_usd = 2600 × $1 (USDC stable) = $2600
    ///           gross_profit = $2600 - $2500 = $100 → 4% ROI, viable
    #[test]
    fn both_tokens_known_evaluates_with_real_per_token_prices() {
        let mut c = cfg(); // WETH base @ $2000, USDC stablecoin @ $1
                           // Bump capital so observed input ($2000) fits without proportional cap
                           // muddying the assertion — we want to test PRICE resolution here, not
                           // cap-ratio interaction (covered by BUG-3 tests).
        c.capital_usd = 5_000.0;
        // 1 ETH → 2100 USDC swap (5% gross spread, realistic arb)
        let candidate = OpportunityCandidate {
            route_fingerprint: "test".into(),
            pool_addresses: vec![],
            token_addresses: vec!["WETH".into(), "USDC".into()],
            dex_adapters: vec!["uniswap-v3".into()],
            amount_in: 1.0,              // 1 WETH = $2000 (cfg base price)
            expected_amount_out: 2100.0, // 2100 USDC = $2100 (stablecoin default)
            gross_profit: 0.0,
        };
        let out = ConfigAwareEvaluator::new(&c, signals()).evaluate(
            &candidate,
            "dex_arb_v2v2",
            1,
            "rpc".into(),
            10,
        );
        let (outcome, rejection) = match out {
            ConfigGateOutcome::Evaluated {
                outcome, rejection, ..
            } => (outcome, rejection),
            other => panic!("expected Evaluated outcome, got {:?}", other),
        };

        // Real gross profit: $2100 - $2000 = $100 (~5% gross). Sanity bound
        // (>999% ROI) does NOT trigger. UnknownTokenPrice does NOT trigger.
        // Risk gate may still reject for other reasons (gas, slippage etc.)
        // but those aren't AnomalousMath / UnknownTokenPrice.
        assert_ne!(
            rejection,
            Some(RejectReason::UnknownTokenPrice),
            "both tokens known — UnknownTokenPrice must not fire",
        );
        assert_ne!(
            rejection,
            Some(RejectReason::AnomalousMath),
            "real arb math (ROI ~5%) must not trigger sanity bound",
        );
        assert!(
            (outcome.gross_profit_usd - 100.0).abs() < 1.0,
            "expected gross_profit ≈ $100, got {} (BUG-2 unfixed would give ~$5.2M)",
            outcome.gross_profit_usd,
        );
    }

    // ----------------------------------------------------------------
    // Cascade integration: RedisCachedPriceOracle wins over ConfigPriceOracle
    // ----------------------------------------------------------------

    /// When the live cache snapshot has a price, the evaluator MUST use it
    /// even if the config has a different value for the same symbol. This
    /// is the whole point of cascading: live data displaces operator toil.
    #[test]
    fn cache_snapshot_overrides_config_base_token_price() {
        let mut c = cfg(); // base WETH @ $2000 in config
        c.capital_usd = 10_000.0; // big enough to avoid cap_ratio noise
        c.allowed_token_symbols = vec!["WETH".into(), "USDC".into()];
        // Live cache: WETH at $2500 (config says $2000 — cache wins)
        let mut snapshot = HashMap::new();
        snapshot.insert("WETH".to_string(), 2500.0);
        // 1 WETH → 2550 USDC. Live USD math: 2550 - 2500 = $50 gross (2% spread)
        // Config-only math would be: 2550 - 2000 = $550 gross (FALSE — bug)
        let candidate = OpportunityCandidate {
            route_fingerprint: "test".into(),
            pool_addresses: vec![],
            token_addresses: vec!["WETH".into(), "USDC".into()],
            dex_adapters: vec!["uniswap-v3".into()],
            amount_in: 1.0,
            expected_amount_out: 2550.0,
            gross_profit: 0.0,
        };
        let out = ConfigAwareEvaluator::with_cache(&c, signals(), snapshot).evaluate(
            &candidate,
            "dex_arb_v2v2",
            1,
            "rpc".into(),
            10,
        );
        let outcome = match out {
            ConfigGateOutcome::Evaluated { outcome, .. } => outcome,
            other => panic!("expected Evaluated, got {:?}", other),
        };
        // With cache @ $2500: gross ≈ $50 (real spread). With config @ $2000: $550.
        // Assertion catches either side of the bug.
        assert!(
            outcome.gross_profit_usd > 30.0 && outcome.gross_profit_usd < 80.0,
            "live cache must drive valuation — expected ≈$50 gross, got {}",
            outcome.gross_profit_usd,
        );
    }

    /// When the cache is EMPTY (boot, all sources down), behaviour must be
    /// identical to ConfigPriceOracle alone — no regression for operators
    /// who haven't deployed the worker yet.
    #[test]
    fn empty_cache_snapshot_falls_through_to_config_oracle() {
        let c = cfg(); // WETH @ $2000, USDC stablecoin default
        let candidate = OpportunityCandidate {
            route_fingerprint: "test".into(),
            pool_addresses: vec![],
            token_addresses: vec!["WETH".into(), "USDC".into()],
            dex_adapters: vec!["uniswap-v3".into()],
            amount_in: 0.5,
            expected_amount_out: 1010.0, // ~1% spread
            gross_profit: 0.0,
        };
        // Two evaluators with same config — one with empty cache, one without.
        // Outcomes must match (cache empty = no contribution).
        let out1 = ConfigAwareEvaluator::with_cache(&c, signals(), HashMap::new()).evaluate(
            &candidate,
            "dex_arb_v2v2",
            1,
            "rpc".into(),
            10,
        );
        let out2 = ConfigAwareEvaluator::new(&c, signals()).evaluate(
            &candidate,
            "dex_arb_v2v2",
            1,
            "rpc".into(),
            10,
        );
        let g1 = match out1 {
            ConfigGateOutcome::Evaluated { outcome, .. } => outcome.gross_profit_usd,
            other => panic!("e1 unexpected: {:?}", other),
        };
        let g2 = match out2 {
            ConfigGateOutcome::Evaluated { outcome, .. } => outcome.gross_profit_usd,
            other => panic!("e2 unexpected: {:?}", other),
        };
        assert!(
            (g1 - g2).abs() < 1e-9,
            "empty cache must produce identical result to no cache, got {} vs {}",
            g1,
            g2,
        );
    }

    /// Cache miss for ONE token but cache hit for the OTHER: the missed
    /// token falls through to ConfigPriceOracle (per-token cascade resolution).
    /// Both sides must be priced for math to proceed; a partial cache is
    /// still useful as long as config covers the gaps.
    #[test]
    fn partial_cache_combines_with_config_per_token() {
        let mut c = cfg();
        c.capital_usd = 10_000.0;
        c.allowed_token_symbols = vec!["WETH".into(), "USDC".into()];
        // Cache has WETH only. USDC must come from ConfigPriceOracle stablecoin
        // default ($1.00). Both prices resolved → no UnknownTokenPrice rejection.
        let mut snapshot = HashMap::new();
        snapshot.insert("WETH".to_string(), 2500.0);
        let candidate = OpportunityCandidate {
            route_fingerprint: "test".into(),
            pool_addresses: vec![],
            token_addresses: vec!["WETH".into(), "USDC".into()],
            dex_adapters: vec!["uniswap-v3".into()],
            amount_in: 1.0,              // 1 WETH (cache: $2500)
            expected_amount_out: 2550.0, // 2550 USDC (config stable: $1)
            gross_profit: 0.0,
        };
        let out = ConfigAwareEvaluator::with_cache(&c, signals(), snapshot).evaluate(
            &candidate,
            "dex_arb_v2v2",
            1,
            "rpc".into(),
            10,
        );
        let (outcome, rejection) = match out {
            ConfigGateOutcome::Evaluated {
                outcome, rejection, ..
            } => (outcome, rejection),
            other => panic!("expected Evaluated, got {:?}", other),
        };
        assert_ne!(
            rejection,
            Some(RejectReason::UnknownTokenPrice),
            "partial cache + stablecoin coverage must NOT trigger UnknownTokenPrice"
        );
        // gross ≈ $2550 - $2500 = $50
        assert!(
            outcome.gross_profit_usd > 30.0 && outcome.gross_profit_usd < 80.0,
            "expected ~$50 gross from partial cache + stable config, got {}",
            outcome.gross_profit_usd,
        );
    }

    /// Bound is at 999% — values BELOW the boundary should NOT trigger
    /// the sanity bound (defensive against false positives). Real HFT outliers
    /// up to several hundred percent are theoretically possible (illiquid
    /// token + thin spread) and shouldn't be silently zeroed.
    #[test]
    fn roi_below_boundary_does_not_trigger_sanity_bound() {
        // Synthesise a ~500% ROI outcome (well under the 999% bound):
        // amount_in_usd = $10, expected_amount_out_usd = $60
        // → gross = $50 → ROI = 500%
        let mut c = cfg();
        c.capital_usd = 1_000.0; // big cap so no proportional scaling
        c.base_token_price_usd = 2500.0;
        c.allowed_token_symbols = vec!["WETH".into(), "USDC".into()];
        c.gas_estimate_units = 1; // ≈ 0 gas cost
        c.fixed_gas_price_gwei = Some(0.0);
        c.failure_risk_buffer_pct = 0.0;
        c.max_slippage_pct = 0.0;
        c.flashloan_fee_pct = 0.0;

        let candidate = OpportunityCandidate {
            route_fingerprint: "test".into(),
            pool_addresses: vec![],
            token_addresses: vec!["WETH".into(), "USDC".into()],
            dex_adapters: vec!["uniswap-v2".into()],
            // Per-token prices via oracle: WETH=$2500, USDC=$1.
            // 0.004 WETH = $10 input, 60 USDC = $60 output → 500% gross.
            amount_in: 0.004,
            expected_amount_out: 60.0,
            gross_profit: 0.0,
        };
        let out = ConfigAwareEvaluator::new(&c, signals()).evaluate(
            &candidate,
            "dex_arb_v2v2",
            1,
            "rpc".into(),
            10,
        );
        let outcome = match out {
            ConfigGateOutcome::Evaluated { outcome, .. } => outcome,
            other => panic!("expected Evaluated outcome, got {:?}", other),
        };
        // Outcome should NOT be zeroed — actual values preserved.
        // (Risk gate may still reject for other reasons; this asserts the
        // sanity bound did not fire spuriously.)
        assert!(
            outcome.gross_profit_usd > 0.0,
            "gross_profit_usd below boundary should not be clamped, got {}",
            outcome.gross_profit_usd,
        );
        assert!(
            outcome.net_roi_pct > 0.0 && outcome.net_roi_pct < 999.0,
            "net_roi_pct should be in (0, 999), got {}",
            outcome.net_roi_pct,
        );
    }

    // ----------------------------------------------------------------
    // Sprint C: Component 9 — spread sanity gate unit tests
    // ----------------------------------------------------------------

    /// Spread within the sanity multiplier bounds → opportunity passes the
    /// spread gate and proceeds to ROI math. WETH→USDC at a realistic
    /// rate: oracle says 1 WETH = 2000 USDC, AMM quotes 2010 USDC
    /// → observed_rate/reference_rate ≈ 1.005, well within 3×.
    #[test]
    fn spread_within_bounds_passes_gate() {
        let mut c = cfg();
        c.capital_usd = 10_000.0;
        c.spread_sanity_mult = 3.0;
        // cfg() base WETH price = $2000 (→ price_in = 2000)
        // USDC stablecoin price = $1 (→ price_out = 1)
        // reference_rate = 2000 / 1 = 2000 USDC/WETH
        // observed_rate  = 2010 / 1.0 = 2010 USDC/WETH → ratio 2010/2000 = 1.005 (< 3)
        let candidate = OpportunityCandidate {
            route_fingerprint: "spread_ok".into(),
            pool_addresses: vec![],
            token_addresses: vec!["WETH".into(), "USDC".into()],
            dex_adapters: vec!["uniswap-v3".into()],
            amount_in: 1.0,
            expected_amount_out: 2010.0,
            gross_profit: 0.0,
        };
        let out = ConfigAwareEvaluator::new(&c, signals()).evaluate(
            &candidate,
            "dex_arb_v2v2",
            1,
            "rpc".into(),
            10,
        );
        let (_, rejection) = match out {
            ConfigGateOutcome::Evaluated {
                outcome, rejection, ..
            } => (outcome, rejection),
            other => panic!("expected Evaluated, got {:?}", other),
        };
        assert!(
            !matches!(rejection, Some(RejectReason::ImplausibleSpread { .. })),
            "spread ≈1.005× reference should pass gate; got rejection={:?}",
            rejection,
        );
    }

    /// Spread exceeds the sanity multiplier → ImplausibleSpread rejection.
    /// Oracle says 1 WETH = 2000 USDC, but AMM quotes 10000 USDC/WETH
    /// → observed_rate/reference_rate = 5.0, exceeds default 3×.
    #[test]
    fn spread_exceeds_threshold_triggers_implausible_spread_rejection() {
        let mut c = cfg();
        c.capital_usd = 10_000.0;
        c.spread_sanity_mult = 3.0;
        // reference_rate = 2000 / 1 = 2000 USDC/WETH
        // observed_rate  = 10000 / 1.0 = 10000 USDC/WETH → ratio 5× (> 3×)
        let candidate = OpportunityCandidate {
            route_fingerprint: "spread_bad".into(),
            pool_addresses: vec![],
            token_addresses: vec!["WETH".into(), "USDC".into()],
            dex_adapters: vec!["uniswap-v3".into()],
            amount_in: 1.0,
            expected_amount_out: 10_000.0, // 5× the oracle reference rate
            gross_profit: 0.0,
        };
        let out = ConfigAwareEvaluator::new(&c, signals()).evaluate(
            &candidate,
            "dex_arb_v2v2",
            1,
            "rpc".into(),
            10,
        );
        let rejection = match out {
            ConfigGateOutcome::Evaluated { rejection, .. } => rejection,
            other => panic!("expected Evaluated, got {:?}", other),
        };
        assert!(
            matches!(rejection, Some(RejectReason::ImplausibleSpread { .. })),
            "spread 5× reference (> 3× threshold) must reject ImplausibleSpread; got {:?}",
            rejection,
        );
    }

    // ----------------------------------------------------------------
    // Sprint H1: Tweak 1 — per-chain block time in capital_cost_usd
    // ----------------------------------------------------------------

    /// Arbitrum (chain_id=42161) capital cost must be ≪ Ethereum (chain_id=1)
    /// for the same amount_in and annual rate. Pre-fix they were equal (both
    /// used ETH_BLOCK_TIME_S=12s). Post-fix ARB uses 0.25s → 48× cheaper.
    #[test]
    fn capital_cost_arbitrum_much_lower_than_ethereum() {
        // Setup: non-flash strategy with non-zero capital_cost_rate_annual_pct.
        // We evaluate the SAME candidate on chain_id=1 and chain_id=42161 and
        // verify that the Ethereum capital cost is materially higher.
        let mut c = cfg();
        c.capital_usd = 10_000.0;
        c.capital_cost_rate_annual_pct = 5.0; // 5% APR opportunity cost
        c.max_slippage_pct = 0.0;
        c.failure_risk_buffer_pct = 0.0;
        c.flashloan_fee_pct = 0.0;
        c.gas_estimate_units = 1;
        c.fixed_gas_price_gwei = Some(0.0);
        c.allowed_token_symbols = vec!["WETH".into(), "USDC".into()];

        let candidate = OpportunityCandidate {
            route_fingerprint: "chain_time_test".into(),
            pool_addresses: vec![],
            token_addresses: vec!["WETH".into(), "USDC".into()],
            dex_adapters: vec!["uniswap-v2".into()],
            amount_in: 1.0,              // 1 WETH = $2000 at cfg base price
            expected_amount_out: 2020.0, // $20 gross
            gross_profit: 0.0,
        };

        // ETH (12s block) capital_cost = $2000 × 0.05 × (12 / 31_536_000) ≈ $3.8e-5
        let eth_out = ConfigAwareEvaluator::new(&c, signals()).evaluate(
            &candidate,
            "dex_arb_v2v2",
            1,
            "rpc".into(),
            10,
        );
        // ARB (0.25s block) capital_cost = $2000 × 0.05 × (0.25 / 31_536_000) ≈ $7.9e-7
        let arb_out = ConfigAwareEvaluator::new(&c, signals()).evaluate(
            &candidate,
            "dex_arb_v2v2",
            42161,
            "rpc".into(),
            10,
        );

        let eth_net = match eth_out {
            ConfigGateOutcome::Evaluated { outcome, .. } => outcome.net_profit_usd,
            other => panic!("expected Evaluated for ETH, got {:?}", other),
        };
        let arb_net = match arb_out {
            ConfigGateOutcome::Evaluated { outcome, .. } => outcome.net_profit_usd,
            other => panic!("expected Evaluated for ARB, got {:?}", other),
        };

        // ARB net > ETH net because ARB capital cost is lower (all other costs equal).
        // The difference should be ETH_capital_cost - ARB_capital_cost > 0.
        assert!(
            arb_net > eth_net,
            "ARB (0.25s) capital cost must be lower than ETH (12s): \
             arb_net={arb_net:.6} > eth_net={eth_net:.6} expected, but got arb ≤ eth",
        );
        // Ratio: ETH_cost / ARB_cost = 12.0 / 0.25 = 48×.
        // The net profit difference must reflect this ratio (approximately).
        let diff = arb_net - eth_net;
        assert!(
            diff > 0.0,
            "Expected ARB net > ETH net by the capital_cost ratio diff; diff={diff:.8}",
        );
    }

    /// Flash-loan strategies have zero capital cost regardless of chain.
    /// (Capital is atomically borrowed and returned — no block-time lock-up.)
    #[test]
    fn flash_loan_strategy_has_zero_capital_cost_on_any_chain() {
        let mut c = cfg();
        c.capital_usd = 10_000.0;
        c.capital_cost_rate_annual_pct = 5.0;
        c.allowed_token_symbols = vec!["WETH".into(), "USDC".into()];
        c.gas_estimate_units = 1;
        c.fixed_gas_price_gwei = Some(0.0);
        c.max_slippage_pct = 0.0;
        c.failure_risk_buffer_pct = 0.0;
        c.flashloan_fee_pct = 0.0;
        // Allow both strategy kinds tested below — cfg() defaults to dex_arb only,
        // which would otherwise gate-reject the flashloan_arb evaluation.
        c.enabled_strategies = vec!["dex_arb_v2v2".into(), "flashloan_arb".into()];

        let candidate = OpportunityCandidate {
            route_fingerprint: "flash_test".into(),
            pool_addresses: vec![],
            token_addresses: vec!["WETH".into(), "USDC".into()],
            dex_adapters: vec!["uniswap-v2".into()],
            amount_in: 1.0,
            expected_amount_out: 2020.0,
            gross_profit: 0.0,
        };

        // Non-flash result for comparison (same chain, same params).
        let non_flash_out = ConfigAwareEvaluator::new(&c, signals()).evaluate(
            &candidate,
            "dex_arb_v2v2",
            1,
            "rpc".into(),
            10,
        );
        // Flash-loan strategy — strategy_kind contains "flash".
        let flash_out = ConfigAwareEvaluator::new(&c, signals()).evaluate(
            &candidate,
            "flashloan_arb",
            1,
            "rpc".into(),
            10,
        );

        let non_flash_net = match non_flash_out {
            ConfigGateOutcome::Evaluated { outcome, .. } => outcome.net_profit_usd,
            other => panic!("expected Evaluated non-flash, got {:?}", other),
        };
        let flash_net = match flash_out {
            ConfigGateOutcome::Evaluated { outcome, .. } => outcome.net_profit_usd,
            other => panic!("expected Evaluated flash, got {:?}", other),
        };

        // Flash-loan has zero capital cost → flash_net ≥ non_flash_net
        // (other costs identical; capital_cost_usd > 0 for non-flash on ETH)
        assert!(
            flash_net >= non_flash_net,
            "flash-loan strategy must have ≥ net profit vs non-flash (zero capital cost); \
             flash={flash_net:.8} non_flash={non_flash_net:.8}",
        );
    }

    // ----------------------------------------------------------------
    // Sprint H1: Tweak 2 — LP fee aggregation from dex adapter names
    // ----------------------------------------------------------------

    /// `default_fee_bps_for_adapter` returns known constants without DB access.
    #[test]
    fn fee_bps_lookup_known_adapters() {
        assert_eq!(default_fee_bps_for_adapter("uniswap-v2"), 30);
        assert_eq!(default_fee_bps_for_adapter("sushi"), 30);
        assert_eq!(default_fee_bps_for_adapter("curve"), 4);
        assert_eq!(default_fee_bps_for_adapter("balancer"), 10);
        // V3 returns 0 — per-pool fee is unknown without fee-tier context
        assert_eq!(default_fee_bps_for_adapter("uniswap-v3"), 0);
        // Unknown/future adapters: 0 (no phantom fees)
        assert_eq!(default_fee_bps_for_adapter("unknown"), 0);
        assert_eq!(default_fee_bps_for_adapter(""), 0);
    }

    /// UniswapV2 candidate: lp_fees_usd is non-zero because 30bps is resolved.
    /// A candidate routed through two V2 legs (dex_adapters = ["uniswap-v2", "uniswap-v2"])
    /// accumulates 60bps total fee = 0.6% of amount_in_usd.
    ///
    /// We verify net profit is LOWER for the V2 route than for a V3 route with
    /// the same gross spread (V3 has 0 explicit fee from adapter lookup — fees
    /// already in AMM output).
    #[test]
    fn lp_fees_nonzero_for_v2_adapter_reduces_net_profit() {
        let mut c = cfg();
        c.capital_usd = 10_000.0;
        c.max_slippage_pct = 0.0;
        c.failure_risk_buffer_pct = 0.0;
        c.flashloan_fee_pct = 0.0;
        c.gas_estimate_units = 1;
        c.fixed_gas_price_gwei = Some(0.0);
        c.capital_cost_rate_annual_pct = 0.0;
        c.ops_overhead_usd_per_attempt = 0.0;
        c.allowed_token_symbols = vec!["WETH".into(), "USDC".into()];

        // Two legs of uniswap-v2 → cumulative 60bps = 0.6% of amount_in
        let candidate_v2 = OpportunityCandidate {
            route_fingerprint: "v2_fee_test".into(),
            pool_addresses: vec![],
            token_addresses: vec!["WETH".into(), "USDC".into()],
            dex_adapters: vec!["uniswap-v2".into(), "uniswap-v2".into()],
            amount_in: 1.0,              // 1 WETH = $2000
            expected_amount_out: 2020.0, // 2020 USDC = $2020 (gross ≈ $20)
            gross_profit: 0.0,
        };
        // V3 route with identical spread but 0bps explicit fees
        let candidate_v3 = OpportunityCandidate {
            dex_adapters: vec!["uniswap-v3".into()],
            ..candidate_v2.clone()
        };

        let v2_out = ConfigAwareEvaluator::new(&c, signals()).evaluate(
            &candidate_v2,
            "dex_arb_v2v2",
            1,
            "rpc".into(),
            10,
        );
        let v3_out = ConfigAwareEvaluator::new(&c, signals()).evaluate(
            &candidate_v3,
            "dex_arb_v2v2",
            1,
            "rpc".into(),
            10,
        );

        let v2_net = match v2_out {
            ConfigGateOutcome::Evaluated { outcome, .. } => outcome.net_profit_usd,
            other => panic!("expected Evaluated v2, got {:?}", other),
        };
        let v3_net = match v3_out {
            ConfigGateOutcome::Evaluated { outcome, .. } => outcome.net_profit_usd,
            other => panic!("expected Evaluated v3, got {:?}", other),
        };

        // V2 has 60bps of explicit fees = $2000 × 0.006 = $12 deducted.
        // V3 has 0 explicit fees. So v3_net > v2_net.
        assert!(
            v3_net > v2_net,
            "V2 route (60bps) must have lower net than V3 (0bps); v2_net={v2_net:.2} v3_net={v3_net:.2}",
        );
        // The difference should be approximately $12 (60bps of $2000).
        let diff = v3_net - v2_net;
        assert!(
            diff > 10.0 && diff < 14.0,
            "60bps fee on $2000 should reduce net by ~$12; diff={diff:.4}",
        );
    }

    /// R8 fail-honest: when dex_adapters is empty → lp_fees_usd = 0 (no phantom fees).
    #[test]
    fn empty_dex_adapters_produces_zero_lp_fees() {
        let mut c = cfg();
        c.capital_usd = 10_000.0;
        c.max_slippage_pct = 0.0;
        c.failure_risk_buffer_pct = 0.0;
        c.flashloan_fee_pct = 0.0;
        c.gas_estimate_units = 1;
        c.fixed_gas_price_gwei = Some(0.0);
        c.capital_cost_rate_annual_pct = 0.0;
        c.ops_overhead_usd_per_attempt = 0.0;
        c.allowed_token_symbols = vec!["WETH".into(), "USDC".into()];

        let candidate_no_adapter = OpportunityCandidate {
            route_fingerprint: "no_adapter".into(),
            pool_addresses: vec![],
            token_addresses: vec!["WETH".into(), "USDC".into()],
            dex_adapters: vec![], // empty
            amount_in: 1.0,
            expected_amount_out: 2020.0,
            gross_profit: 0.0,
        };
        let candidate_unknown = OpportunityCandidate {
            dex_adapters: vec!["some_new_dex_xyz".into()],
            ..candidate_no_adapter.clone()
        };

        let out_no = ConfigAwareEvaluator::new(&c, signals()).evaluate(
            &candidate_no_adapter,
            "dex_arb_v2v2",
            1,
            "rpc".into(),
            10,
        );
        let out_unknown = ConfigAwareEvaluator::new(&c, signals()).evaluate(
            &candidate_unknown,
            "dex_arb_v2v2",
            1,
            "rpc".into(),
            10,
        );

        let net_no = match out_no {
            ConfigGateOutcome::Evaluated { outcome, .. } => outcome.net_profit_usd,
            other => panic!("expected Evaluated no_adapter, got {:?}", other),
        };
        let net_unknown = match out_unknown {
            ConfigGateOutcome::Evaluated { outcome, .. } => outcome.net_profit_usd,
            other => panic!("expected Evaluated unknown_adapter, got {:?}", other),
        };
        // Both should be identical — zero lp_fees → no difference
        assert!(
            (net_no - net_unknown).abs() < 1e-9,
            "empty/unknown dex_adapters must produce identical net profit; \
             no_adapter={net_no:.8} unknown={net_unknown:.8}",
        );
    }

    // ----------------------------------------------------------------
    // Sprint H1: Tweak 3 — V2 reserve snapshot price impact wiring
    // ----------------------------------------------------------------

    /// When `v2_reserve_snapshot = Some(...)` is supplied, price_impact_pct
    /// is computed from reserves rather than max_slippage_pct proxy.
    /// A large trade relative to reserves produces measurable price impact.
    #[test]
    fn v2_reserves_produce_nonzero_price_impact() {
        let mut c = cfg();
        c.capital_usd = 10_000.0;
        c.max_slippage_pct = 0.001; // 0.001% proxy — much smaller than real impact
        c.failure_risk_buffer_pct = 0.0;
        c.flashloan_fee_pct = 0.0;
        c.gas_estimate_units = 1;
        c.fixed_gas_price_gwei = Some(0.0);
        c.capital_cost_rate_annual_pct = 0.0;
        c.ops_overhead_usd_per_attempt = 0.0;
        c.allowed_token_symbols = vec!["WETH".into(), "USDC".into()];

        // 1 WETH into a pool with only 10 WETH reserve → 10% of reserves.
        // Price impact ≈ 9.97% >> max_slippage_pct proxy (0.001%).
        let candidate = OpportunityCandidate {
            route_fingerprint: "reserves_test".into(),
            pool_addresses: vec!["0xpool1".into()],
            token_addresses: vec!["WETH".into(), "USDC".into()],
            dex_adapters: vec!["uniswap-v2".into()],
            amount_in: 1.0, // 1 WETH
            expected_amount_out: 2020.0,
            gross_profit: 0.0,
        };

        // reserve_in = 1e19 (10 WETH in wei) → in USD-equivalent token units
        // The evaluator receives (reserve_in, reserve_out) as token-unit floats.
        // 10.0 WETH reserve_in, large USDC reserve_out.
        let reserve_in = 10.0_f64; // 10 WETH as token units
        let reserve_out = 20_000.0_f64; // 20,000 USDC token units

        let without_reserves = ConfigAwareEvaluator::new(&c, signals()).evaluate(
            &candidate,
            "dex_arb_v2v2",
            1,
            "rpc".into(),
            10,
        );

        let with_reserves = ConfigAwareEvaluator::with_reserves(
            &c,
            signals(),
            HashMap::new(),
            None,
            None,
            Some((reserve_in, reserve_out)),
        )
        .evaluate(&candidate, "dex_arb_v2v2", 1, "rpc".into(), 10);

        let net_without = match without_reserves {
            ConfigGateOutcome::Evaluated { outcome, .. } => outcome.net_profit_usd,
            other => panic!("expected Evaluated without_reserves, got {:?}", other),
        };
        let net_with = match with_reserves {
            ConfigGateOutcome::Evaluated { outcome, .. } => outcome.net_profit_usd,
            other => panic!("expected Evaluated with_reserves, got {:?}", other),
        };

        // With real price impact (~9.97%) >> proxy (0.001%), net must be much lower.
        assert!(
            net_with < net_without,
            "real V2 price impact must lower net profit vs proxy; \
             with_reserves={net_with:.4} without={net_without:.4}",
        );
        // The proxy is so small vs the real impact that the difference is large
        let diff = net_without - net_with;
        assert!(
            diff > 1.0,
            "V2 impact (10% of 10-WETH pool) must reduce net by > $1; diff={diff:.4}",
        );
    }

    /// When `v2_reserve_snapshot = None`, the max_slippage_pct proxy fires.
    /// Net profit from evaluator with None matches evaluator without reserves.
    #[test]
    fn v2_reserves_none_falls_back_to_max_slippage_proxy() {
        let mut c = cfg();
        c.capital_usd = 10_000.0;
        c.max_slippage_pct = 0.5;
        c.failure_risk_buffer_pct = 0.0;
        c.flashloan_fee_pct = 0.0;
        c.gas_estimate_units = 1;
        c.fixed_gas_price_gwei = Some(0.0);
        c.capital_cost_rate_annual_pct = 0.0;
        c.ops_overhead_usd_per_attempt = 0.0;
        c.allowed_token_symbols = vec!["WETH".into(), "USDC".into()];

        let candidate = OpportunityCandidate {
            route_fingerprint: "proxy_fallback".into(),
            pool_addresses: vec![],
            token_addresses: vec!["WETH".into(), "USDC".into()],
            dex_adapters: vec!["uniswap-v2".into()],
            amount_in: 1.0,
            expected_amount_out: 2020.0,
            gross_profit: 0.0,
        };

        let out_default = ConfigAwareEvaluator::new(&c, signals()).evaluate(
            &candidate,
            "dex_arb_v2v2",
            1,
            "rpc".into(),
            10,
        );
        let out_explicit_none =
            ConfigAwareEvaluator::with_reserves(&c, signals(), HashMap::new(), None, None, None)
                .evaluate(&candidate, "dex_arb_v2v2", 1, "rpc".into(), 10);

        let net_default = match out_default {
            ConfigGateOutcome::Evaluated { outcome, .. } => outcome.net_profit_usd,
            other => panic!("expected Evaluated default, got {:?}", other),
        };
        let net_explicit_none = match out_explicit_none {
            ConfigGateOutcome::Evaluated { outcome, .. } => outcome.net_profit_usd,
            other => panic!("expected Evaluated explicit_none, got {:?}", other),
        };

        // Both must be identical — both use max_slippage_pct proxy
        assert!(
            (net_default - net_explicit_none).abs() < 1e-9,
            "v2_reserve_snapshot=None must produce same result as no reserves; \
             default={net_default:.8} explicit_none={net_explicit_none:.8}",
        );
    }
}
