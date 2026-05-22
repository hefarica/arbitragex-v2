//! CexDexEngine — Hybrid CEX-DEX Convergence.
//!
//! ## Strategy shape
//!
//! This engine implements the "Hybrid CEX-DEX Convergence" module.
//! It targets the eigen-state where the off-chain price (CEX like Binance/Bybit)
//! diverges from the on-chain price (DEX) by an exploitable magnitude.
//!
//! **Goal:** Arbitrage the macroeconomic differential using proprietary infrastructure.
//! The system acts as an organic Market Maker, affecting no retail users on-chain.
//!
//! ## Doctrine compliance
//! - **Self-Rebalancing:** Requires use of proprietary inventory on exchanges.
//! - **Real-time PnL:** Must maintain real-time PnL reconciliation with strict
//!   circuit breakers for API drops or RPC latency.

use super::StrategyCandidate;
use crate::models::{Opportunity, StrategyKind};
use crate::strategy_label::StrategyLabel;
use ethers::types::{H256, U256};
use prioritization_spine::route_plan::RoutePlan;
use prioritization_spine::types::OpportunityCandidate;
use tracing::{debug, warn};

/// Configuration for the CEX-DEX engine.
#[derive(Debug, Clone)]
pub struct CexDexEngineConfig {
    /// Minimum expected profit in USD to emit a candidate.
    pub min_profit_usd: f64,
    /// Minimum spread in basis points to trigger execution.
    pub min_spread_bps: u32,
}

impl Default for CexDexEngineConfig {
    fn default() -> Self {
        Self {
            min_profit_usd: 15.0,
            min_spread_bps: 20, // 0.20%
        }
    }
}

pub struct CexDexEngine {
    config: CexDexEngineConfig,
}

impl CexDexEngine {
    pub fn new(config: CexDexEngineConfig) -> Self {
        Self { config }
    }

    /// Evaluates a CEX-DEX spread.
    pub fn evaluate(
        &self,
        cex_price: f64,
        dex_price: f64,
        chain_id: u64,
    ) -> Option<StrategyCandidate> {
        let diff = (cex_price - dex_price).abs();
        let min_price = cex_price.min(dex_price);
        let spread_bps = ((diff / min_price) * 10000.0) as u32;

        if spread_bps < self.config.min_spread_bps {
            debug!(
                event = "cex_dex_engine.skipped",
                reason = "insufficient_spread",
                spread_bps
            );
            return None;
        }

        // 2. Construct the opportunity
        // In Phase 1, we assume a static inventory size and calculate theoretical profit.
        let theoretical_profit = (diff * 5.0) - 5.0; // Assume 5 units traded, minus 5.0 USD gas/fees/hedging costs

        if theoretical_profit < self.config.min_profit_usd {
            return None;
        }

        let opp = Opportunity {
            id: uuid::Uuid::new_v4().to_string(),
            chain_id,
            strategy_kind: StrategyKind::DexArb, // Or a specific CexDex kind if added
            tokens: vec![], // To be filled
            amounts: vec![],
            expected_profit_usd: theoretical_profit,
            detected_at: chrono::Utc::now(),
            status: "detected".to_string(),
            tx_hash: None,
            gas_used: None,
            gas_price: None,
            block_number: None,
            error_reason: None,
        };

        let candidate = OpportunityCandidate {
            id: opp.id.clone(),
            chain_id,
            strategy: "hybrid_cex_dex".to_string(),
            expected_profit_usd: theoretical_profit,
            gas_cost_usd: 0.0,
            net_profit_usd: 0.0,
            score: 0.0,
            is_viable: true,
            rejection_reason: None,
        };

        let route_plan = RoutePlan {
            steps: vec![],
            estimated_gas: U256::from(120_000), // Standard swap gas
            minimum_profit_wei: U256::zero(),
        };

        Some(StrategyCandidate {
            label: StrategyLabel::DexArbV3V3, // Placeholder
            opportunity: opp,
            candidate,
            route_plan,
            gross_profit_usd: Some(theoretical_profit),
            net_expected_profit_usd: None,
            rejection_reason: None,
            source_intent_hash: H256::zero(),
            base_strategy: None,
        })
    }
}
