//! Tip/Bribe Optimizer — Adaptive γ for proposer compensation.
//!
//! Research finding (FASE C, MEV practice): production searchers pay 50-90%
//! of gross profit to proposers/builders via tips. The optimal γ (tip fraction)
//! depends on competition level: under heavy competition, higher γ increases
//! inclusion probability; in quiet periods, lower γ maximizes retained profit.
//!
//! ## Mathematical Model
//!
//! The searcher's expected profit is:
//!   E[profit] = P(inclusion | γ) × (gross_profit × (1 - γ) - gas)
//!
//! Where P(inclusion | γ) is the probability the bundle gets included given
//! the tip fraction γ. Under the Flashbots model, builders rank bundles by
//! effective gas price (tip / gas_used), so higher γ → higher ranking →
//! higher inclusion probability.
//!
//! ## Adaptive γ Algorithm
//!
//! Based on observed competition (from recent block data):
//!   - No competition (0 competitor bundles): γ_min = 0.10 (retain 90%)
//!   - Light competition (1-3 bundles): γ = 0.30 (retain 70%)
//!   - Medium competition (4-10 bundles): γ = 0.50 (retain 50%)
//!   - Heavy competition (>10 bundles): γ_max = 0.85 (retain 15%)
//!
//! The algorithm also considers the absolute profit: for very small profits,
//! even a small tip makes the opportunity net-negative, so γ is capped by
//! the break-even constraint: gross × (1 - γ) - gas > min_profit.
//!
//! ## Sources
//! - Flashbots simple-arbitrage default: 80% to proposer
//! - Flashbots simple-blind-arbitrage default: 50% to proposer
//! - ethresear.ch analysis: optimal γ under competition
//! - EigenPhi data: average 90%+ on competitive flow

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TipConfig {
    /// Minimum tip fraction (quiet periods). Default 0.10 (10%).
    pub gamma_min: f64,
    /// Maximum tip fraction (heavy competition). Default 0.85 (85%).
    pub gamma_max: f64,
    /// Minimum retained profit in USD — never tip below this. Default $1.
    pub min_retained_usd: f64,
    /// Gas cost estimate in USD for the transaction.
    pub gas_cost_usd: f64,
    /// Whether to use aggressive mode (for time-sensitive opportunities).
    pub aggressive: bool,
}

impl Default for TipConfig {
    fn default() -> Self {
        Self {
            gamma_min: 0.10,
            gamma_max: 0.85,
            min_retained_usd: 1.0,
            gas_cost_usd: 5.0,
            aggressive: false,
        }
    }
}

#[derive(Debug, Clone)]
pub struct TipRecommendation {
    /// Recommended tip fraction (γ ∈ [0, 1]).
    pub gamma: f64,
    /// Tip amount in USD.
    pub tip_usd: f64,
    /// Retained profit after tip and gas.
    pub retained_usd: f64,
    /// Competition level that drove this recommendation.
    pub competition_level: CompetitionLevel,
    /// Whether the opportunity is still profitable after tipping.
    pub viable: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CompetitionLevel {
    None,
    Light,
    Medium,
    Heavy,
}

/// Compute the optimal tip for an arbitrage opportunity.
///
/// # Arguments
/// * `gross_profit_usd` - The gross profit before any costs
/// * `competitor_bundles` - Number of competing bundles in recent blocks
/// * `config` - Tip configuration
///
/// # Returns
/// A recommendation with the optimal γ, tip amount, and retained profit.
pub fn optimize_tip(
    gross_profit_usd: f64,
    competitor_bundles: u32,
    config: &TipConfig,
) -> TipRecommendation {
    // Determine competition level
    let level = match competitor_bundles {
        0 => CompetitionLevel::None,
        1..=3 => CompetitionLevel::Light,
        4..=10 => CompetitionLevel::Medium,
        _ => CompetitionLevel::Heavy,
    };

    // Base γ from competition level
    let base_gamma = match level {
        CompetitionLevel::None => config.gamma_min,
        CompetitionLevel::Light => 0.30,
        CompetitionLevel::Medium => 0.50,
        CompetitionLevel::Heavy => config.gamma_max,
    };

    // Aggressive mode bumps γ by 10pp (for time-sensitive opportunities)
    let mut gamma = if config.aggressive {
        (base_gamma + 0.10).min(config.gamma_max)
    } else {
        base_gamma
    };

    // Break-even constraint: retained = gross × (1 - γ) - gas ≥ min_retained
    // → γ ≤ 1 - (min_retained + gas) / gross
    let max_gamma = 1.0 - (config.min_retained_usd + config.gas_cost_usd) / gross_profit_usd;

    if max_gamma < 0.0 {
        // Even γ=0 doesn't cover gas + min retained → not viable
        return TipRecommendation {
            gamma: 0.0,
            tip_usd: 0.0,
            retained_usd: gross_profit_usd - config.gas_cost_usd,
            competition_level: level,
            viable: false,
        };
    }

    gamma = gamma.min(max_gamma);

    let tip_usd = gross_profit_usd * gamma;
    let retained_usd = gross_profit_usd - tip_usd - config.gas_cost_usd;

    TipRecommendation {
        gamma,
        tip_usd,
        retained_usd,
        competition_level: level,
        viable: retained_usd >= config.min_retained_usd,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_competition_minimizes_tip() {
        let rec = optimize_tip(100.0, 0, &TipConfig::default());
        assert!((rec.gamma - 0.10).abs() < 1e-9);
        assert!(rec.viable);
        assert!(rec.retained_usd > 80.0); // Retain > 80%
    }

    #[test]
    fn heavy_competition_maximizes_tip() {
        let rec = optimize_tip(100.0, 20, &TipConfig::default());
        assert!((rec.gamma - 0.85).abs() < 1e-9);
        assert!(rec.viable);
        assert!(rec.retained_usd < 20.0); // Retain < 20%
    }

    #[test]
    fn small_profit_capped_by_break_even() {
        // $10 gross, $5 gas, $1 min → max γ = 1 - 6/10 = 0.4
        let rec = optimize_tip(10.0, 20, &TipConfig::default());
        assert!(rec.gamma <= 0.4);
        assert!(rec.retained_usd >= 0.99); // ≈ $1 min retained
    }

    #[test]
    fn unprofitable_after_gas_not_viable() {
        // $3 gross, $5 gas → even γ=0 loses money
        let rec = optimize_tip(3.0, 0, &TipConfig::default());
        assert!(!rec.viable);
    }

    #[test]
    fn aggressive_mode_bumps_gamma() {
        let config = TipConfig {
            aggressive: true,
            ..TipConfig::default()
        };
        let rec = optimize_tip(100.0, 0, &config);
        assert!(rec.gamma > 0.10); // Bumped from 0.10
        assert!(rec.gamma <= config.gamma_max);
    }

    #[test]
    fn medium_competition_half_tip() {
        let rec = optimize_tip(100.0, 5, &TipConfig::default());
        assert!((rec.gamma - 0.50).abs() < 1e-9);
        assert!((rec.tip_usd - 50.0).abs() < 1e-9);
    }
}
