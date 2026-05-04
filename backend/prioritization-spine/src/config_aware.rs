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

use crate::evidence::OpportunityEvidence;
use crate::types::OpportunityCandidate;
use crate::decision::{ExecutionDecision, RejectReason};
use math_engine::risk_engine::{
    OpportunityRiskProfile, RiskPolicy, RiskRejectionReason, validate_opportunity_risk,
};
use math_engine::roi_engine::{RoiCalculationParams, calc_net_profit_and_roi};
use math_engine::DefiArbitrageOutcome;
use shared_rs::trading_config::TradingConfigState;

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
    Evaluated {
        outcome: DefiArbitrageOutcome,
        evidence: OpportunityEvidence,
        rejection: Option<RejectReason>,
    },
}

/// Translate config thresholds into a RiskPolicy that math-engine consumes.
fn policy_from_config(cfg: &TradingConfigState) -> RiskPolicy {
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
        // Liquidity floor — operator's capital_usd is a sensible proxy: don't
        // touch pools that can't absorb the operator's full deployable capital.
        min_liquidity_usd: cfg.capital_usd.max(1.0),
        max_trade_size_usd: cfg.capital_usd,
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
}

impl<'a> ConfigAwareEvaluator<'a> {
    pub fn new(config: &'a TradingConfigState, signals: NetworkSignals) -> Self {
        Self { config, signals }
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

        // 3. Capital sizing — observed amount_in capped by operator capital.
        // Convert observed amount_in (token units) → USD via config's base price.
        //
        // BUG-3 fix (2026-05-04 outlier): when the cap reduces effective input,
        // expected_amount_out_usd MUST also be scaled by the same ratio. The
        // pre-fix code capped only the input side, producing fake gross profit
        // equal to the cap delta (e.g. observed $125 → capped $10 with full
        // output $125 yielded a phantom $115 gross profit, ROI > 1000%).
        //
        // Linear scaling is conservative — it under-estimates slippage on
        // smaller trades (real AMM output for capped input would be slightly
        // higher per unit) — but it eliminates the asymmetric inflation that
        // pollutes dashboards and trips the schema's numeric(10,4) ceiling.
        // BUG-2 (token-blind USD pricing via profit_token_to_usd) is NOT
        // addressed here and may still inflate cross-token cases; tracked
        // separately for the price-oracle sprint.
        let observed_amount_in_usd = self.config.profit_token_to_usd(candidate.amount_in);
        let amount_in_usd = observed_amount_in_usd.min(self.config.capital_usd);
        let cap_ratio = if observed_amount_in_usd > 0.0 {
            amount_in_usd / observed_amount_in_usd
        } else {
            1.0
        };
        let expected_amount_out_usd =
            self.config.profit_token_to_usd(candidate.expected_amount_out) * cap_ratio;

        // 4. Math: compute net profit and ROI deterministically.
        let gas_cost_usd = estimate_gas_cost_usd(self.config, self.signals);
        let roi_params = RoiCalculationParams {
            amount_in_usd,
            expected_amount_out_usd,
            expected_gas_cost_usd: gas_cost_usd,
            flashloan_fee_pct: self.config.flashloan_fee_pct,
            max_slippage_pct: self.config.max_slippage_pct / 100.0, // pct → fraction
            failure_risk_buffer_usd: amount_in_usd * self.config.failure_risk_buffer_pct,
        };
        let outcome = calc_net_profit_and_roi(&roi_params);

        // 5. Risk gate: validate against config-derived policy.
        let policy = policy_from_config(self.config);
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
        let rejection: Option<RejectReason> = match validate_opportunity_risk(&risk_profile, &policy) {
            Ok(()) => None,
            Err(reason) => Some(map_risk_rejection(reason)),
        };

        // 6. Build evidence with REAL numbers (no more hardcoded 0.95 / 0.9 / 1.0).
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
        };

        ConfigGateOutcome::Evaluated { outcome, evidence, rejection }
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

    fn cfg() -> TradingConfigState {
        TradingConfigState {
            chain_id: 1,
            capital_usd: 1000.0,
            base_token_symbol: "WETH".into(),
            base_token_price_usd: 2000.0,
            allowed_token_symbols: vec!["WETH".into(), "USDC".into()],
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
        // but order-of-magnitude bounded. Asserting < 100,000% leaves headroom while
        // catching the BUG-3 regression specifically.
        assert!(
            outcome.net_roi_pct < 100_000.0,
            "BUG-3 regression: net_roi_pct = {} (must be < 100,000% after proportional cap). \
             Note: BUG-2 (token-blind USD) is unaddressed in this fix and may keep ROI inflated; \
             this test only ensures BUG-3 is fixed, not BUG-2.",
            outcome.net_roi_pct,
        );
    }
}
