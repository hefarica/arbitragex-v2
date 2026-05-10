use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ExecutionDecision {
    Execute,
    SimulateDeeper,
    Hold,
    Reject,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum RejectReason {
    NegativeNetProfit,
    LowLandingProbability,
    StaleState,
    LowLiquidity,
    ExcessiveSlippage,
    HighGasVolatility,
    HighTokenRisk,
    SimulationFailed,
    BundleSimulationFailed,
    MissingEvidence,
    RouteTooLong,
    PoolNotTrusted,
    RpcUnhealthy,
    OracleMismatch,
    ReorgRiskTooHigh,
    /// Defense-in-depth: math layer produced ROI > 999% or |gross_profit| > $1M.
    /// Real HFT MEV ROI distribution is 0.05% – 2%; outliers above 999% are
    /// always math bugs. Tracked in anti_reincidencia.md Incidente #7.
    AnomalousMath,
    /// BUG-2 fix: a token in the candidate (token_in or token_out) has no
    /// USD price resolvable via `shared_rs::price_oracle::ConfigPriceOracle`
    /// — i.e. it's not the operator's base token, not in the operator's
    /// `token_prices_usd` map, and not in the hardcoded stablecoin trust
    /// list. The spine refuses to invent a price (R8 Fail-Honest); operator
    /// closes the gap by populating `token_prices_usd` for that symbol.
    UnknownTokenPrice,
    /// Component 9 (Sprint C): AMM-quoted rate deviates from oracle reference
    /// rate by more than `spread_sanity_mult` (default 3×). Likely indicates
    /// stale AMM state or a math bug in the quoter — not a real opportunity.
    ///
    /// Surfaced with observed/reference rates for dashboard diagnostics.
    /// R8 fail-honest: ONLY fired when BOTH oracle prices are present.
    /// Missing oracle prices → check skipped, not rejected as ImplausibleSpread.
    ImplausibleSpread {
        observed_rate: f64,
        reference_rate: f64,
        threshold_mult: f64,
    },
    // ──── Migration 056 — StrategyConfigGate rejection reasons ────────────
    //
    // The new gate fires AFTER the chain-level token allowlist + strategy
    // gate (which produce TokenNotAllowed / StrategyDisabled outcomes from
    // ConfigGateOutcome) and BEFORE the math layer. These reasons surface in
    // the dashboard with enough context for the operator to fix the gap.
    //
    /// `strategy_configs[kind].enabled = false` (per-strategy explicit off).
    StrategyConfigDisabled { strategy_kind: String },
    /// Per-strategy `allowed_chain_ids` is set and excludes this chain.
    StrategyConfigChainBlocked { strategy_kind: String, chain_id: u64 },
    /// A leg's `dex_id` is not in the effective DEX allowlist (per-strategy
    /// or chain-level `enabled_dex_ids`). `dex_id` carries the offending UUID
    /// so the dashboard links straight to the DEX row.
    StrategyConfigDexBlocked { strategy_kind: String, dex_id: String },
    /// Audit 2026-05-10 follow-up: per-strategy `enabled_pool_ids` is set
    /// and a leg's `pool_id` is not in it. UUID surfaced for dashboard links.
    StrategyConfigPoolBlocked { strategy_kind: String, pool_id: String },
    /// A leg's `protocol_type` is not in the per-strategy
    /// `enabled_protocol_types` allowlist (when non-empty).
    StrategyConfigProtocolBlocked {
        strategy_kind: String,
        protocol_type: String,
    },
    /// Route fails the `route_constraints` shape gate (legs out of
    /// min/max range, atomicity required but missing, cross-chain not allowed).
    StrategyConfigRouteBlocked {
        strategy_kind: String,
        detail: String,
    },
    /// Per-leg pool TVL/volume is below the per-strategy floor.
    /// `metric` ∈ {"tvl_usd","volume_24h_usd"}. Carrying actual + threshold
    /// so the dashboard surfaces the gap directly.
    StrategyConfigPoolBelowFloor {
        strategy_kind: String,
        pool_address: String,
        metric: String,
        actual_usd: f64,
        threshold_usd: f64,
    },
    /// Net profit below the per-strategy `min_profit_usd` override.
    StrategyConfigBelowMinProfit {
        strategy_kind: String,
        net_profit_usd: f64,
        threshold_usd: f64,
    },
    /// ROI below the per-strategy `min_roi_pct` override.
    StrategyConfigBelowMinRoi {
        strategy_kind: String,
        roi_pct: f64,
        threshold_pct: f64,
    },
}

impl RejectReason {
    /// Stable short tag used in metrics + dashboards. Avoids leaking enum
    /// variant Debug formatting (which can change across Rust versions).
    pub fn tag(&self) -> &'static str {
        match self {
            RejectReason::NegativeNetProfit => "negative_net_profit",
            RejectReason::LowLandingProbability => "low_landing_probability",
            RejectReason::StaleState => "stale_state",
            RejectReason::LowLiquidity => "low_liquidity",
            RejectReason::ExcessiveSlippage => "excessive_slippage",
            RejectReason::HighGasVolatility => "high_gas_volatility",
            RejectReason::HighTokenRisk => "high_token_risk",
            RejectReason::SimulationFailed => "simulation_failed",
            RejectReason::BundleSimulationFailed => "bundle_simulation_failed",
            RejectReason::MissingEvidence => "missing_evidence",
            RejectReason::RouteTooLong => "route_too_long",
            RejectReason::PoolNotTrusted => "pool_not_trusted",
            RejectReason::RpcUnhealthy => "rpc_unhealthy",
            RejectReason::OracleMismatch => "oracle_mismatch",
            RejectReason::ReorgRiskTooHigh => "reorg_risk_too_high",
            RejectReason::AnomalousMath => "anomalous_math",
            RejectReason::UnknownTokenPrice => "unknown_token_price",
            RejectReason::ImplausibleSpread { .. } => "implausible_spread",
            RejectReason::StrategyConfigDisabled { .. } => "strategy_config_disabled",
            RejectReason::StrategyConfigChainBlocked { .. } => "strategy_config_chain_blocked",
            RejectReason::StrategyConfigDexBlocked { .. } => "strategy_config_dex_blocked",
            RejectReason::StrategyConfigPoolBlocked { .. } => "strategy_config_pool_blocked",
            RejectReason::StrategyConfigProtocolBlocked { .. } => {
                "strategy_config_protocol_blocked"
            }
            RejectReason::StrategyConfigRouteBlocked { .. } => "strategy_config_route_blocked",
            RejectReason::StrategyConfigPoolBelowFloor { .. } => {
                "strategy_config_pool_below_floor"
            }
            RejectReason::StrategyConfigBelowMinProfit { .. } => {
                "strategy_config_below_min_profit"
            }
            RejectReason::StrategyConfigBelowMinRoi { .. } => "strategy_config_below_min_roi",
        }
    }
}

pub trait ExecutionDecisionEngine {
    fn decide(&self, score: f64) -> ExecutionDecision;
}
