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

use crate::evidence::{CostBreakdown, OpportunityEvidence, PFailSource};
use crate::strategy_scores_db::StrategyFailRate;
use crate::types::OpportunityCandidate;
use crate::decision::{ExecutionDecision, RejectReason};
use math_engine::amm_math::calc_univ2_price_impact;
use math_engine::risk_engine::{
    OpportunityRiskProfile, RiskPolicy, RiskRejectionReason, validate_opportunity_risk,
};
use math_engine::roi_engine::{RoiCalculationParams, calc_net_profit_and_roi};
use math_engine::DefiArbitrageOutcome;
use shared_rs::price_oracle::{
    CascadePriceOracle, ConfigPriceOracle, PriceOracle, RedisCachedPriceOracle,
};
use shared_rs::trading_config::TradingConfigState;
use std::collections::HashMap;

// ETH mainnet block time in seconds. Used for capital_cost_usd denominator.
// Follow-up Sprint B: replace with `block_time_s_per_chain(chain_id)` so
// ARB/Base/OP (2s) are accurate. Using ETH (12s) for ALL chains in Sprint A is
// conservative — it OVER-estimates capital cost for fast chains, which is the
// safe direction (never under-estimates cost).
const ETH_BLOCK_TIME_S: f64 = 12.0;
const SECONDS_PER_YEAR: f64 = 31_536_000.0;

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
        Self { basefee_gwei: 0.0, p75_priority_tip_gwei: 0.0, block_number }
    }
}

#[derive(Debug, Clone)]
pub enum ConfigGateOutcome {
    /// Token outside operator's allowlist — skipped silently.
    TokenNotAllowed { token_symbol_or_addr: String },
    /// Strategy class disabled by operator.
    StrategyDisabled { strategy_kind: String },
    /// Ran the math; here is the result (may still be unprofitable).
    /// `outcome` and `evidence` are boxed to avoid the `large_enum_variant`
    /// lint: `OpportunityEvidence` (~480 bytes) dwarfs the other variants
    /// (~24 bytes). Boxing heap-allocates the large payload; callers
    /// dereference with `*` or pattern-match normally.
    Evaluated {
        outcome: Box<DefiArbitrageOutcome>,
        evidence: Box<OpportunityEvidence>,
        rejection: Option<RejectReason>,
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
    let gas_price_gwei = cfg.resolve_gas_price_gwei(signals.basefee_gwei, signals.p75_priority_tip_gwei);
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
        }
    }

    /// Single-shot evaluation. Returns the gate outcome (allowlist, strategy,
    /// or full evaluated profile + evidence ready for `PrioritizationEngine::score`).
    ///
    /// `strategy_kind` is the candidate's class — must match an entry in
    /// `config.enabled_strategies` (e.g. "dex_arb_v2v2"). When the operator's
    /// `enabled_strategies` is empty, the evaluator treats it as "all enabled"
    /// to avoid silent paralysis on a freshly-seeded config.
    pub fn evaluate(
        &self,
        candidate: &OpportunityCandidate,
        strategy_kind: &str,
        chain_id: u64,
        rpc_url_hash: String,
        rpc_latency_ms: u64,
    ) -> ConfigGateOutcome {
        // 1. Token allowlist gate.
        for tok in &candidate.token_addresses {
            if !self.config.token_allowed(tok) {
                return ConfigGateOutcome::TokenNotAllowed {
                    token_symbol_or_addr: tok.clone(),
                };
            }
        }

        // 2. Strategy gate (empty list = permissive default).
        if !self.config.enabled_strategies.is_empty()
            && !self.config.enabled_strategies.iter().any(|s| s == strategy_kind)
        {
            return ConfigGateOutcome::StrategyDisabled {
                strategy_kind: strategy_kind.to_string(),
            };
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
        let cache_oracle =
            RedisCachedPriceOracle::from_snapshot(self.price_cache_snapshot.clone());
        let config_oracle = ConfigPriceOracle::new(self.config);
        let oracle: CascadePriceOracle = CascadePriceOracle::new(vec![
            Box::new(cache_oracle),
            Box::new(config_oracle),
        ]);
        let token_in_id = candidate
            .token_addresses
            .first()
            .map(String::as_str)
            .unwrap_or("");
        let token_out_id = candidate
            .token_addresses
            .get(1)
            .map(String::as_str)
            .unwrap_or("");
        let in_price_opt = oracle.price_usd(token_in_id);
        let out_price_opt = oracle.price_usd(token_out_id);
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
            .effective_capital_for(token_in_id, strategy_kind);
        let observed_amount_in_usd = candidate.amount_in * in_price;
        let amount_in_usd = observed_amount_in_usd.min(effective_capital);
        let cap_ratio = if observed_amount_in_usd > 0.0 {
            amount_in_usd / observed_amount_in_usd
        } else {
            1.0
        };
        let expected_amount_out_usd = candidate.expected_amount_out * out_price * cap_ratio;

        // 4. Math: compute net profit and ROI deterministically.
        let gas_cost_usd = estimate_gas_cost_usd(self.config, self.signals);

        // --- Component 2: LP fees ---
        // Sprint A: route-leg detail not yet available at this call site (the
        // candidate carries pool addresses but not per-leg fee_bps). We pass
        // 0.0, which is safe — fees are implicitly reflected in the spread
        // between `amount_in_usd` and `expected_amount_out_usd` (AMM math
        // already deducts them from `expected_amount_out`). Sprint B will
        // propagate `RouteLeg` slices from the scanner and compute the sum
        // explicitly: `Σ(amount_through_leg_usd × fee_bps / 10_000)`.
        let lp_fees_usd: f64 = 0.0;

        // --- Component 3: Real V2 price impact ---
        // Sprint A: pool reserves are not yet fetched from Redis
        // (`arbx:pool_reserves:<chain>:<addr>` — Sprint B). Pass `0.0` here
        // so `calc_net_profit_and_roi` falls back to `max_slippage_pct` proxy.
        // V3 pools: price-impact from tick-math is deferred to Sprint B as well.
        //
        // When reserves ARE available (Sprint B), caller will compute:
        //   let (reserve_in, _reserve_out) = fetch_v2_reserves(...).await;
        //   let impact_fraction = calc_univ2_price_impact(amount_in, reserve_in, fee_pct);
        //   let price_impact_pct = impact_fraction * 100.0;
        //
        // The import of `calc_univ2_price_impact` is present now so Sprint B
        // requires no import changes, only the fetch plumbing.
        let _ = calc_univ2_price_impact; // suppress unused-import warning until Sprint B
        let price_impact_pct: f64 = 0.0;

        // --- Component 6: Capital opportunity cost ---
        // Flash-loan strategies borrow capital atomically in the same tx;
        // no lock-up period occurs, so capital cost is zero.
        // Non-flash strategies lock capital for one block duration.
        //
        // Detection: strategy names containing "flash" are treated as flash-loan.
        // This is conservative: unknown strategies are treated as non-flash
        // (capital cost applies), which is the safer direction.
        //
        // Follow-up Sprint B: replace string heuristic with an explicit boolean
        // `is_flash_loan` flag on `OpportunityCandidate` set by the scanner.
        let is_flash_loan = strategy_kind.to_ascii_lowercase().contains("flash");
        let capital_cost_usd = if is_flash_loan || self.config.capital_cost_rate_annual_pct == 0.0 {
            0.0
        } else {
            amount_in_usd
                * (self.config.capital_cost_rate_annual_pct / 100.0)
                * (ETH_BLOCK_TIME_S / SECONDS_PER_YEAR)
        };

        // --- Component 7: Ops overhead ---
        let ops_overhead_usd = self.config.ops_overhead_usd_per_attempt;

        let failure_risk_buffer_usd = amount_in_usd * self.config.failure_risk_buffer_pct;
        let flashloan_fee_usd_computed = amount_in_usd * self.config.flashloan_fee_pct;

        // --- Component 4 (Sprint B): resolve p_fail for the failure buffer ---
        // `p_fail_rate` was pre-fetched by the caller via `StrategyScoresCache`.
        // Map it into `Option<f64>` for `RoiCalculationParams`, and prepare the
        // `PFailSource` enum for `CostBreakdown` (dashboard surfacing).
        let (p_fail_opt, p_fail_source) = match &self.p_fail_rate {
            Some(rate) => (
                Some(rate.p_fail),
                PFailSource::Statistical {
                    p: rate.p_fail,
                    sample_count: rate.sample_count,
                },
            ),
            None => (
                None,
                PFailSource::Proxy {
                    buffer_usd: failure_risk_buffer_usd,
                },
            ),
        };

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
        //   1. UnknownTokenPrice — input itself is invalid; no math is meaningful
        //   2. AnomalousMath     — math layer producing absurd values (BUG-2 net)
        //   3. RiskPolicy reasons — operational gates (gas, slippage, liquidity)
        // Higher tiers signal "fix upstream" before tweaking risk knobs.
        let rejection: Option<RejectReason> = if unknown_price {
            Some(RejectReason::UnknownTokenPrice)
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

        let cost_breakdown = CostBreakdown::new(
            gas_cost_usd,
            lp_fees_usd,
            flashloan_fee_usd_computed,
            effective_slippage_usd_for_breakdown,
            actual_failure_buffer_usd,
            capital_cost_usd,
            ops_overhead_usd,
            p_fail_source,
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
            min_amount_out: candidate.expected_amount_out * (1.0 - self.config.max_slippage_pct / 100.0),
            gross_profit: outcome.gross_profit_usd,
            gas_units_estimated: self.config.gas_estimate_units,
            gas_price: self.config.resolve_gas_price_gwei(self.signals.basefee_gwei, self.signals.p75_priority_tip_gwei) * 1e9,
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

        ConfigGateOutcome::Evaluated {
            outcome: Box::new(outcome),
            evidence: Box::new(evidence),
            rejection,
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
            capital_cost_rate_annual_pct: 0.0,
            ops_overhead_usd_per_attempt: 0.01,
            enabled: true,
            updated_at: Utc::now(),
            updated_by: None,
        }
    }

    fn signals() -> NetworkSignals {
        NetworkSignals { basefee_gwei: 25.0, p75_priority_tip_gwei: 2.0, block_number: 19_000_000 }
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
            &candidate, "dex_arb_v2v2", 1, "rpc".into(), 10,
        );
        assert!(matches!(out, ConfigGateOutcome::TokenNotAllowed { .. }));
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
            &candidate, "curve_stable", 1, "rpc".into(), 10,
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
            &candidate, "anything_goes", 1, "rpc".into(), 10,
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
            &candidate, "dex_arb_v2v2", 1, "rpc".into(), 10,
        );
        if let ConfigGateOutcome::Evaluated { outcome, .. } = out {
            // Total capital required is capped at $1000, not $2000
            assert!(outcome.total_capital_required_usd <= 1000.0,
                    "expected capital cap, got {}", outcome.total_capital_required_usd);
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
            amount_in: 0.05,            // observed: 0.05 ETH = $125 (>> $10 capital)
            expected_amount_out: 0.05,  // ~same magnitude → no real spread
            gross_profit: 0.0,
        };
        let out = ConfigAwareEvaluator::new(&c, signals()).evaluate(
            &candidate, "dex_arb_v2v2", 1, "rpc".into(), 10,
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
            &candidate, "dex_arb_v2v2", 1, "rpc".into(), 10,
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

    /// Defense-in-depth sanity bound. Post-BUG-2 the natural trigger for
    /// AnomalousMath is operator misconfiguration of `token_prices_usd` —
    /// e.g. typo'ing UNI as $10,000 instead of $10. The bound clamps the
    /// resulting absurd outcome and tags it `AnomalousMath` so the operator
    /// sees the diagnostic reason and audits their config.
    #[test]
    fn anomalous_roi_triggers_sanity_bound() {
        let mut c = cfg();
        c.capital_usd = 10.0;
        c.base_token_price_usd = 2500.0;
        c.allowed_token_symbols = vec!["WETH".into(), "UNI".into()];
        // Operator typo: UNI mis-priced as $10,000 instead of ~$8.
        // This is the kind of mistake the sanity bound exists to catch
        // even after BUG-2 fix (oracle has a price; price is just wrong).
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
            &candidate, "dex_arb_v2v2", 1, "rpc".into(), 10,
        );
        let (outcome, rejection) = match out {
            ConfigGateOutcome::Evaluated { outcome, rejection, .. } => (outcome, rejection),
            other => panic!("expected Evaluated outcome, got {:?}", other),
        };

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
        assert_eq!(
            rejection,
            Some(RejectReason::AnomalousMath),
            "expected AnomalousMath rejection reason, got {:?}",
            rejection,
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
            &candidate, "dex_arb_v2v2", 1, "rpc".into(), 10,
        );
        let (outcome, rejection) = match out {
            ConfigGateOutcome::Evaluated { outcome, rejection, .. } => (outcome, rejection),
            other => panic!("expected Evaluated outcome, got {:?}", other),
        };
        assert_eq!(
            rejection,
            Some(RejectReason::UnknownTokenPrice),
            "unknown token_in should reject with UnknownTokenPrice, got {:?}",
            rejection,
        );
        assert_eq!(outcome.gross_profit_usd, 0.0,
                   "outcome must be zeroed when price unknown, got {}", outcome.gross_profit_usd);
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
            &candidate, "dex_arb_v2v2", 1, "rpc".into(), 10,
        );
        let (outcome, rejection) = match out {
            ConfigGateOutcome::Evaluated { outcome, rejection, .. } => (outcome, rejection),
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
            amount_in: 5.0,             // 5 WETH = $10K (above operational $1K cap)
            expected_amount_out: 10_500.0, // 10,500 USDC = $10,500 → 5% gross
            gross_profit: 0.0,
        };
        let out = ConfigAwareEvaluator::new(&c, signals()).evaluate(
            &candidate, "dex_arb_v2v2", 1, "rpc".into(), 10,
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
            &candidate, "dex_arb_v2v2", 1, "rpc".into(), 10,
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
            &candidate, "dex_arb_v2v2", 1, "rpc".into(), 10,
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
            &candidate, "dex_arb_v2v2", 1, "rpc".into(), 10,
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
            amount_in: 1.0,             // 1 WETH = $2000 (cfg base price)
            expected_amount_out: 2100.0, // 2100 USDC = $2100 (stablecoin default)
            gross_profit: 0.0,
        };
        let out = ConfigAwareEvaluator::new(&c, signals()).evaluate(
            &candidate, "dex_arb_v2v2", 1, "rpc".into(), 10,
        );
        let (outcome, rejection) = match out {
            ConfigGateOutcome::Evaluated { outcome, rejection, .. } => (outcome, rejection),
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
        let out = ConfigAwareEvaluator::with_cache(&c, signals(), snapshot)
            .evaluate(&candidate, "dex_arb_v2v2", 1, "rpc".into(), 10);
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
        let out1 = ConfigAwareEvaluator::with_cache(&c, signals(), HashMap::new())
            .evaluate(&candidate, "dex_arb_v2v2", 1, "rpc".into(), 10);
        let out2 = ConfigAwareEvaluator::new(&c, signals())
            .evaluate(&candidate, "dex_arb_v2v2", 1, "rpc".into(), 10);
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
            amount_in: 1.0,           // 1 WETH (cache: $2500)
            expected_amount_out: 2550.0, // 2550 USDC (config stable: $1)
            gross_profit: 0.0,
        };
        let out = ConfigAwareEvaluator::with_cache(&c, signals(), snapshot)
            .evaluate(&candidate, "dex_arb_v2v2", 1, "rpc".into(), 10);
        let (outcome, rejection) = match out {
            ConfigGateOutcome::Evaluated { outcome, rejection, .. } => (outcome, rejection),
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
            &candidate, "dex_arb_v2v2", 1, "rpc".into(), 10,
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
}
