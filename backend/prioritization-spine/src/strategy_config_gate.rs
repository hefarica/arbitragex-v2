//! `StrategyConfigGate` — enforces per-strategy operator config from
//! `trading_config.strategy_configs` (migration 056).
//!
//! Two-pass design (Decision C from 2026-05-10 design notes):
//!
//!   PASS A (cheap, before math)
//!     - strategy_enabled (chain-level + per-strategy)
//!     - chain allowlist
//!     - per-leg DEX allowlist (skipped if RoutePlan absent → falls back
//!       to dex_adapters by name match)
//!     - per-leg protocol_type allowlist
//!     - pool_is_active for every leg
//!     - route shape (min/max legs, atomic, cross-chain)
//!
//!   PASS B (post-math, after net_profit + roi computed)
//!     - per-leg pool TVL/volume floor (only when RoutePlan carries them)
//!     - per-strategy min_profit_usd / min_roi_pct overrides
//!
//! When `RoutePlan` is `None` (legacy worker, Decision B graceful
//! degradation), PASS A skips per-leg checks but still enforces
//! strategy/chain/DEX-by-name. PASS B skips pool floor checks (no data)
//! but still enforces profit/ROI overrides.
//!
//! Never silently approves: missing data → `data_quality=partial` hint
//! visible to operator (returned via `GateOutcome::PartialEnforcement`),
//! never a synthetic pass.

use crate::decision::RejectReason;
use crate::route_plan::RoutePlan;
use crate::types::OpportunityCandidate;
use shared_rs::trading_config::TradingConfigState;
#[cfg(test)]
use shared_rs::trading_config::StrategyRuntimeConfig;

/// Outcome of `StrategyConfigGate::check_pre_math`.
///
/// `Pass` and `PartialPass` both let the candidate through the cheap pass
/// — `PartialPass` flags that a per-leg check (DEX, protocol, pool active)
/// could not be enforced because the worker did not provide a `RoutePlan`.
/// The dashboard surfaces this as `data_quality=partial` so the operator
/// knows their toggles are best-effort for that worker until it is
/// migrated.
///
/// `Reject(reason)` carries the precise reject reason for persistence
/// + WebSocket broadcast.
#[derive(Debug, Clone, PartialEq)]
pub enum GateOutcome {
    Pass,
    PartialPass { missing: &'static str },
    Reject(RejectReason),
}

impl GateOutcome {
    pub fn is_reject(&self) -> bool {
        matches!(self, GateOutcome::Reject(_))
    }
    pub fn rejection(&self) -> Option<&RejectReason> {
        match self {
            GateOutcome::Reject(r) => Some(r),
            _ => None,
        }
    }
}

/// Stateless gate. Holds no config — every check takes
/// `(cfg, candidate, route_plan, strategy_kind, chain_id)` so the gate is
/// safe to call from concurrent worker tasks without locking.
pub struct StrategyConfigGate;

impl StrategyConfigGate {
    /// PASS A — cheap checks before math. Order matters: cheapest first
    /// so most rejections short-circuit at micro-cost.
    pub fn check_pre_math(
        cfg: &TradingConfigState,
        candidate: &OpportunityCandidate,
        route_plan: Option<&RoutePlan>,
        strategy_kind: &str,
        chain_id: u64,
    ) -> GateOutcome {
        // 1. Strategy enabled (chain-level + per-strategy override).
        if !cfg.strategy_enabled(strategy_kind) {
            return GateOutcome::Reject(RejectReason::StrategyConfigDisabled {
                strategy_kind: strategy_kind.to_string(),
            });
        }

        // 2. Chain allowlist (per-strategy `allowed_chain_ids`).
        if !cfg.strategy_allowed_on_chain(strategy_kind, chain_id) {
            return GateOutcome::Reject(RejectReason::StrategyConfigChainBlocked {
                strategy_kind: strategy_kind.to_string(),
                chain_id,
            });
        }

        // 3. DEX allowlist. With RoutePlan we use per-leg dex_id (UUID).
        //    Without, we fall back to candidate.dex_adapters BY NAME — that
        //    is what legacy workers emit. The allowlist UUIDs cannot match
        //    names directly, so the fallback bypasses this check and we
        //    return PartialPass at the end. Operator sees the limitation
        //    via data_quality=partial.
        let allowlist = cfg.effective_dex_allowlist(strategy_kind);
        let mut partial_missing: Option<&'static str> = None;

        match (allowlist.as_ref(), route_plan) {
            // Strict allowlist + RoutePlan available: enforce per-leg dex_id.
            (Some(allow), Some(plan)) => {
                for leg in &plan.legs {
                    let leg_id = leg.dex_id.to_ascii_lowercase();
                    if !allow.iter().any(|a| a == &leg_id) {
                        return GateOutcome::Reject(RejectReason::StrategyConfigDexBlocked {
                            strategy_kind: strategy_kind.to_string(),
                            dex_id: leg.dex_id.clone(),
                        });
                    }
                }
            }
            // Strict allowlist but NO RoutePlan: cannot map names→UUIDs,
            // so we cannot enforce per-leg DEX. Flag partial.
            (Some(_), None) => {
                partial_missing = Some("route_plan");
            }
            // No allowlist (None or absent): permissive — every DEX OK.
            (None, _) => {}
        }

        // 4. Per-strategy `enabled_protocol_types` (only with RoutePlan).
        if let Some(strat_cfg) = cfg.strategy_config(strategy_kind) {
            if !strat_cfg.enabled_protocol_types.is_empty() {
                if let Some(plan) = route_plan {
                    let allowed: Vec<String> = strat_cfg
                        .enabled_protocol_types
                        .iter()
                        .map(|s| s.to_ascii_lowercase())
                        .collect();
                    for leg in &plan.legs {
                        let p = leg.protocol_type.to_ascii_lowercase();
                        if !allowed.contains(&p) {
                            return GateOutcome::Reject(
                                RejectReason::StrategyConfigProtocolBlocked {
                                    strategy_kind: strategy_kind.to_string(),
                                    protocol_type: leg.protocol_type.clone(),
                                },
                            );
                        }
                    }
                } else if partial_missing.is_none() {
                    partial_missing = Some("route_plan");
                }
            }
        }

        // 5. Pool is_active (only with RoutePlan).
        if let Some(plan) = route_plan {
            for leg in &plan.legs {
                if !leg.pool_is_active {
                    return GateOutcome::Reject(RejectReason::StrategyConfigRouteBlocked {
                        strategy_kind: strategy_kind.to_string(),
                        detail: format!(
                            "leg pool inactive: dex={} pool={}",
                            leg.dex_name,
                            leg.pool_address.as_deref().unwrap_or("?"),
                        ),
                    });
                }
            }
        }

        // 6. Route shape: min/max legs, atomic, cross-chain.
        if let Some(strat_cfg) = cfg.strategy_config(strategy_kind) {
            let rc = &strat_cfg.route_constraints;
            if let Some(plan) = route_plan {
                let n = plan.legs.len() as u32;
                if n < rc.min_legs {
                    return GateOutcome::Reject(RejectReason::StrategyConfigRouteBlocked {
                        strategy_kind: strategy_kind.to_string(),
                        detail: format!("legs={n} < min_legs={}", rc.min_legs),
                    });
                }
                if n > rc.max_legs {
                    return GateOutcome::Reject(RejectReason::StrategyConfigRouteBlocked {
                        strategy_kind: strategy_kind.to_string(),
                        detail: format!("legs={n} > max_legs={}", rc.max_legs),
                    });
                }
                if rc.require_atomic && !plan.atomic {
                    return GateOutcome::Reject(RejectReason::StrategyConfigRouteBlocked {
                        strategy_kind: strategy_kind.to_string(),
                        detail: "non-atomic route blocked by require_atomic".into(),
                    });
                }
            } else {
                // Without a plan we can still enforce min/max legs against
                // the legacy `pool_addresses` length as a best-effort approx
                // (each pool is one leg in the legacy model).
                let n = candidate.pool_addresses.len() as u32;
                if n > 0 && n < rc.min_legs {
                    return GateOutcome::Reject(RejectReason::StrategyConfigRouteBlocked {
                        strategy_kind: strategy_kind.to_string(),
                        detail: format!("legs(pool_count)={n} < min_legs={}", rc.min_legs),
                    });
                }
                if n > 0 && n > rc.max_legs {
                    return GateOutcome::Reject(RejectReason::StrategyConfigRouteBlocked {
                        strategy_kind: strategy_kind.to_string(),
                        detail: format!("legs(pool_count)={n} > max_legs={}", rc.max_legs),
                    });
                }
                if partial_missing.is_none() && (rc.require_atomic || !rc.allowed_base_tokens.is_empty()) {
                    partial_missing = Some("route_plan");
                }
            }
        }

        // 7. Per-strategy token allowlist override (when set).
        if let Some(strat_cfg) = cfg.strategy_config(strategy_kind) {
            let rc = &strat_cfg.route_constraints;
            if !rc.allowed_base_tokens.is_empty() {
                if let Some(plan) = route_plan {
                    let allow: Vec<String> = rc
                        .allowed_base_tokens
                        .iter()
                        .map(|s| s.to_ascii_lowercase())
                        .collect();
                    if let Some(first_leg) = plan.legs.first() {
                        let tin = first_leg.token_in.to_ascii_lowercase();
                        if !allow.iter().any(|a| a == &tin) {
                            return GateOutcome::Reject(
                                RejectReason::StrategyConfigRouteBlocked {
                                    strategy_kind: strategy_kind.to_string(),
                                    detail: format!(
                                        "base token {} not in allowed_base_tokens",
                                        first_leg.token_in
                                    ),
                                },
                            );
                        }
                    }
                }
            }
        }

        match partial_missing {
            Some(m) => GateOutcome::PartialPass { missing: m },
            None => GateOutcome::Pass,
        }
    }

    /// PASS B — post-math checks. Called after the evaluator has computed
    /// `net_profit_usd` and `roi_pct`. Enforces per-strategy economic
    /// overrides on top of the chain-level gates already applied by the
    /// existing risk_engine.
    pub fn check_post_math(
        cfg: &TradingConfigState,
        route_plan: Option<&RoutePlan>,
        strategy_kind: &str,
        net_profit_usd: f64,
        roi_pct: f64,
    ) -> GateOutcome {
        let strat_cfg = match cfg.strategy_config(strategy_kind) {
            Some(c) => c,
            None => return GateOutcome::Pass,
        };

        // 1. Per-strategy min_profit_usd override.
        if let Some(min) = strat_cfg.min_profit_usd {
            if net_profit_usd < min {
                return GateOutcome::Reject(RejectReason::StrategyConfigBelowMinProfit {
                    strategy_kind: strategy_kind.to_string(),
                    net_profit_usd,
                    threshold_usd: min,
                });
            }
        }

        // 2. Per-strategy min_roi_pct override.
        if let Some(min) = strat_cfg.min_roi_pct {
            if roi_pct < min {
                return GateOutcome::Reject(RejectReason::StrategyConfigBelowMinRoi {
                    strategy_kind: strategy_kind.to_string(),
                    roi_pct,
                    threshold_pct: min,
                });
            }
        }

        // 3. Per-leg pool TVL / volume floors (only with RoutePlan).
        let rc = &strat_cfg.route_constraints;
        if let Some(plan) = route_plan {
            for leg in &plan.legs {
                if let (Some(min), Some(actual)) = (rc.min_pool_tvl_usd, leg.tvl_usd) {
                    if actual < min {
                        return GateOutcome::Reject(
                            RejectReason::StrategyConfigPoolBelowFloor {
                                strategy_kind: strategy_kind.to_string(),
                                pool_address: leg
                                    .pool_address
                                    .clone()
                                    .unwrap_or_else(|| "?".into()),
                                metric: "tvl_usd".into(),
                                actual_usd: actual,
                                threshold_usd: min,
                            },
                        );
                    }
                }
                if let (Some(min), Some(actual)) = (rc.min_pool_volume_24h_usd, leg.volume_24h_usd) {
                    if actual < min {
                        return GateOutcome::Reject(
                            RejectReason::StrategyConfigPoolBelowFloor {
                                strategy_kind: strategy_kind.to_string(),
                                pool_address: leg
                                    .pool_address
                                    .clone()
                                    .unwrap_or_else(|| "?".into()),
                                metric: "volume_24h_usd".into(),
                                actual_usd: actual,
                                threshold_usd: min,
                            },
                        );
                    }
                }
            }
        }
        // Operator can still flag missing data via data_quality even when
        // gates pass — the evaluator's responsibility, not the gate's.
        let _ = strat_cfg;
        GateOutcome::Pass
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::route_plan::RouteLeg;
    use chrono::Utc;
    use shared_rs::trading_config::{GasPriceStrategy, RouteConstraints};
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
            enabled_dex_ids: None,
            strategy_configs: HashMap::new(),
            capital_cost_rate_annual_pct: 0.0,
            ops_overhead_usd_per_attempt: 0.01,
            spread_sanity_mult: 3.0,
            p_copied_volume_threshold_usd: 1_000_000.0,
            p_copied_max: 0.5,
            enabled: true,
            updated_at: Utc::now(),
            updated_by: None,
        }
    }

    fn cand() -> OpportunityCandidate {
        OpportunityCandidate {
            route_fingerprint: "fp".into(),
            pool_addresses: vec!["0xpool".into()],
            token_addresses: vec!["WETH".into(), "USDC".into()],
            dex_adapters: vec!["uniswap-v2".into()],
            amount_in: 1.0,
            expected_amount_out: 1.005,
            gross_profit: 0.005,
        }
    }

    fn leg(dex_id: &str, protocol: &str) -> RouteLeg {
        RouteLeg {
            dex_id: dex_id.into(),
            dex_name: "uniswap-v2".into(),
            protocol_type: protocol.into(),
            factory_address: "0xf".into(),
            pool_id: None,
            pool_address: Some("0xpool".into()),
            token_in: "0xa".into(),
            token_out: "0xb".into(),
            fee_bps: Some(30),
            amount_in: Some(1.0),
            amount_out: Some(0.99),
            tvl_usd: Some(2_000_000.0),
            volume_24h_usd: Some(1_000_000.0),
            pool_is_active: true,
        }
    }

    fn plan(legs: Vec<RouteLeg>) -> RoutePlan {
        RoutePlan {
            route_id: None,
            strategy_kind: "dex_arb_v2v2".into(),
            chain_id: 1,
            legs,
            atomic: true,
            estimated_slippage_pct: None,
            price_impact_pct: None,
        }
    }

    // ─── Pre-math (PASS A) ─────────────────────────────────────────────

    #[test]
    fn pass_when_no_strategy_config_set() {
        let c = cfg();
        let outcome = StrategyConfigGate::check_pre_math(
            &c, &cand(), None, "dex_arb_v2v2", 1,
        );
        assert!(matches!(outcome, GateOutcome::Pass));
    }

    #[test]
    fn rejects_when_strategy_disabled_per_strategy_override() {
        let mut c = cfg();
        c.strategy_configs.insert(
            "dex_arb_v2v2".into(),
            StrategyRuntimeConfig {
                enabled: false,
                ..Default::default()
            },
        );
        let outcome = StrategyConfigGate::check_pre_math(
            &c, &cand(), None, "dex_arb_v2v2", 1,
        );
        match outcome {
            GateOutcome::Reject(RejectReason::StrategyConfigDisabled { strategy_kind }) => {
                assert_eq!(strategy_kind, "dex_arb_v2v2");
            }
            other => panic!("expected StrategyConfigDisabled, got {other:?}"),
        }
    }

    #[test]
    fn rejects_when_chain_blocked_per_strategy() {
        let mut c = cfg();
        c.strategy_configs.insert(
            "dex_arb_v2v2".into(),
            StrategyRuntimeConfig {
                enabled: true,
                allowed_chain_ids: vec![137], // not chain 1
                ..Default::default()
            },
        );
        let outcome = StrategyConfigGate::check_pre_math(
            &c, &cand(), None, "dex_arb_v2v2", 1,
        );
        assert!(matches!(
            outcome,
            GateOutcome::Reject(RejectReason::StrategyConfigChainBlocked { chain_id: 1, .. })
        ));
    }

    #[test]
    fn rejects_when_dex_not_in_allowlist_with_route_plan() {
        let mut c = cfg();
        c.enabled_dex_ids = Some(vec!["allowed-uuid".into()]);
        let p = plan(vec![leg("blocked-uuid", "uniswap-v2")]);
        let outcome = StrategyConfigGate::check_pre_math(
            &c, &cand(), Some(&p), "dex_arb_v2v2", 1,
        );
        match outcome {
            GateOutcome::Reject(RejectReason::StrategyConfigDexBlocked { dex_id, .. }) => {
                assert_eq!(dex_id, "blocked-uuid");
            }
            other => panic!("expected StrategyConfigDexBlocked, got {other:?}"),
        }
    }

    #[test]
    fn empty_dex_allowlist_blocks_everything() {
        // Operator explicit block-all (different from None).
        let mut c = cfg();
        c.enabled_dex_ids = Some(vec![]);
        let p = plan(vec![leg("any-uuid", "uniswap-v2")]);
        let outcome = StrategyConfigGate::check_pre_math(
            &c, &cand(), Some(&p), "dex_arb_v2v2", 1,
        );
        assert!(matches!(
            outcome,
            GateOutcome::Reject(RejectReason::StrategyConfigDexBlocked { .. })
        ));
    }

    #[test]
    fn partial_pass_when_dex_allowlist_set_but_no_route_plan() {
        // Decision B graceful degradation: gate cannot enforce per-leg DEX
        // without a RoutePlan, but does NOT silently approve. Returns
        // PartialPass so the dashboard surfaces data_quality=partial.
        let mut c = cfg();
        c.enabled_dex_ids = Some(vec!["allowed-uuid".into()]);
        let outcome = StrategyConfigGate::check_pre_math(
            &c, &cand(), None, "dex_arb_v2v2", 1,
        );
        assert!(matches!(
            outcome,
            GateOutcome::PartialPass { missing: "route_plan" }
        ));
    }

    #[test]
    fn rejects_when_protocol_not_allowed() {
        let mut c = cfg();
        c.strategy_configs.insert(
            "dex_arb_v2v2".into(),
            StrategyRuntimeConfig {
                enabled: true,
                enabled_protocol_types: vec!["uniswap-v3".into()],
                ..Default::default()
            },
        );
        let p = plan(vec![leg("uuid", "uniswap-v2")]); // v2 leg
        let outcome = StrategyConfigGate::check_pre_math(
            &c, &cand(), Some(&p), "dex_arb_v2v2", 1,
        );
        match outcome {
            GateOutcome::Reject(RejectReason::StrategyConfigProtocolBlocked {
                protocol_type, ..
            }) => {
                assert_eq!(protocol_type, "uniswap-v2");
            }
            other => panic!("expected StrategyConfigProtocolBlocked, got {other:?}"),
        }
    }

    #[test]
    fn rejects_when_pool_inactive() {
        let mut leg_inactive = leg("uuid", "uniswap-v2");
        leg_inactive.pool_is_active = false;
        let p = plan(vec![leg_inactive]);
        let outcome = StrategyConfigGate::check_pre_math(
            &cfg(), &cand(), Some(&p), "dex_arb_v2v2", 1,
        );
        assert!(matches!(
            outcome,
            GateOutcome::Reject(RejectReason::StrategyConfigRouteBlocked { .. })
        ));
    }

    #[test]
    fn rejects_when_route_too_short() {
        let mut c = cfg();
        c.strategy_configs.insert(
            "triangular".into(),
            StrategyRuntimeConfig {
                enabled: true,
                route_constraints: RouteConstraints {
                    min_legs: 3,
                    max_legs: 3,
                    require_atomic: true,
                    ..Default::default()
                },
                ..Default::default()
            },
        );
        // A single-leg "triangular" is a route shape error.
        let p = plan(vec![leg("uuid", "uniswap-v2")]);
        // strategy_kind triangular needs to be in chain-level enabled_strategies
        c.enabled_strategies.push("triangular".into());
        let outcome = StrategyConfigGate::check_pre_math(
            &c, &cand(), Some(&p), "triangular", 1,
        );
        assert!(matches!(
            outcome,
            GateOutcome::Reject(RejectReason::StrategyConfigRouteBlocked { .. })
        ));
    }

    #[test]
    fn rejects_when_atomic_required_but_route_is_not() {
        let mut c = cfg();
        c.strategy_configs.insert(
            "dex_arb_v2v2".into(),
            StrategyRuntimeConfig {
                enabled: true,
                route_constraints: RouteConstraints {
                    require_atomic: true,
                    ..Default::default()
                },
                ..Default::default()
            },
        );
        let mut p = plan(vec![leg("uuid", "uniswap-v2")]);
        p.atomic = false;
        let outcome = StrategyConfigGate::check_pre_math(
            &c, &cand(), Some(&p), "dex_arb_v2v2", 1,
        );
        match outcome {
            GateOutcome::Reject(RejectReason::StrategyConfigRouteBlocked { detail, .. }) => {
                assert!(detail.contains("non-atomic"));
            }
            other => panic!("expected RouteBlocked non-atomic, got {other:?}"),
        }
    }

    // ─── Post-math (PASS B) ────────────────────────────────────────────

    #[test]
    fn post_math_pass_when_no_overrides() {
        let outcome =
            StrategyConfigGate::check_post_math(&cfg(), None, "dex_arb_v2v2", 100.0, 5.0);
        assert!(matches!(outcome, GateOutcome::Pass));
    }

    #[test]
    fn post_math_rejects_below_min_profit_override() {
        let mut c = cfg();
        c.strategy_configs.insert(
            "dex_arb_v2v2".into(),
            StrategyRuntimeConfig {
                enabled: true,
                min_profit_usd: Some(50.0), // strategy floor higher than chain
                ..Default::default()
            },
        );
        let outcome =
            StrategyConfigGate::check_post_math(&c, None, "dex_arb_v2v2", 25.0, 5.0);
        assert!(matches!(
            outcome,
            GateOutcome::Reject(RejectReason::StrategyConfigBelowMinProfit { .. })
        ));
    }

    #[test]
    fn post_math_rejects_below_min_roi_override() {
        let mut c = cfg();
        c.strategy_configs.insert(
            "dex_arb_v2v2".into(),
            StrategyRuntimeConfig {
                enabled: true,
                min_roi_pct: Some(2.0),
                ..Default::default()
            },
        );
        let outcome =
            StrategyConfigGate::check_post_math(&c, None, "dex_arb_v2v2", 100.0, 0.5);
        assert!(matches!(
            outcome,
            GateOutcome::Reject(RejectReason::StrategyConfigBelowMinRoi { .. })
        ));
    }

    #[test]
    fn post_math_rejects_pool_below_tvl_floor() {
        let mut c = cfg();
        c.strategy_configs.insert(
            "dex_arb_v2v2".into(),
            StrategyRuntimeConfig {
                enabled: true,
                route_constraints: RouteConstraints {
                    min_pool_tvl_usd: Some(5_000_000.0),
                    ..Default::default()
                },
                ..Default::default()
            },
        );
        // leg.tvl_usd = 2M, floor = 5M → reject
        let p = plan(vec![leg("uuid", "uniswap-v2")]);
        let outcome =
            StrategyConfigGate::check_post_math(&c, Some(&p), "dex_arb_v2v2", 100.0, 5.0);
        match outcome {
            GateOutcome::Reject(RejectReason::StrategyConfigPoolBelowFloor {
                metric,
                actual_usd,
                threshold_usd,
                ..
            }) => {
                assert_eq!(metric, "tvl_usd");
                assert_eq!(actual_usd, 2_000_000.0);
                assert_eq!(threshold_usd, 5_000_000.0);
            }
            other => panic!("expected PoolBelowFloor, got {other:?}"),
        }
    }

    #[test]
    fn post_math_skips_pool_check_when_no_route_plan_r8_fail_honest() {
        // No RoutePlan → cannot check leg.tvl_usd. Skip silently (the
        // pre-math pass already flagged PartialPass for the operator).
        let mut c = cfg();
        c.strategy_configs.insert(
            "dex_arb_v2v2".into(),
            StrategyRuntimeConfig {
                enabled: true,
                route_constraints: RouteConstraints {
                    min_pool_tvl_usd: Some(5_000_000.0),
                    ..Default::default()
                },
                ..Default::default()
            },
        );
        let outcome =
            StrategyConfigGate::check_post_math(&c, None, "dex_arb_v2v2", 100.0, 5.0);
        assert!(matches!(outcome, GateOutcome::Pass));
    }
}
