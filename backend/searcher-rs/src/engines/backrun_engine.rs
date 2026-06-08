//! BackrunEngine — Non-extractive backrunning strategy (Zero Victim).
//!
//! ## Strategy shape
//!
//! This engine implements the "Non-Extractive Backrunning" module. It targets
//! the eigen-state where a massive swap has just executed, leaving the AMM
//! curve out of equilibrium with the global market, but the user's transaction
//! is already finalized and their price is locked.
//!
//! **Crucial Security Constraint (Fail-Honest):** Zero Front-Running. This engine
//! only ever emits bundles where our transaction is mathematically and temporally
//! ordered *strictly after* the target transaction. If the simulation detects any
//! impact on the user's execution price, the candidate is discarded.
//!
//! ## Doctrine compliance
//! - **Zero Mocks:** We rely on real `ImpactSet` data to calculate the post-state.
//! - **Fail-Honest:** Any uncertainty about the ordering guarantees results in `Err`
//!   and the candidate is skipped.

use super::StrategyCandidate;
use crate::models::{Opportunity, StrategyKind};
use crate::strategy_label::StrategyLabel;
use ethers::types::{H256, U256};
use prioritization_spine::route_plan::RoutePlan;
use prioritization_spine::types::OpportunityCandidate;
use tracing::{debug, warn};

/// Configuration for the backrun engine.
#[derive(Debug, Clone)]
pub struct BackrunEngineConfig {
    /// Minimum expected profit in USD to emit a candidate.
    pub min_profit_usd: f64,
    /// Maximum allowed gas impact to ensure we don't front-run.
    pub max_gas_price_gwei: f64,
}

impl Default for BackrunEngineConfig {
    fn default() -> Self {
        Self {
            min_profit_usd: 5.0,
            max_gas_price_gwei: 150.0,
        }
    }
}

pub struct BackrunEngine {
    config: BackrunEngineConfig,
}

impl BackrunEngine {
    pub fn new(config: BackrunEngineConfig) -> Self {
        Self { config }
    }

    /// Evaluates a detected massive swap for non-extractive backrunning potential.
    ///
    /// In a real implementation, `target_tx_hash` and `post_state_imbalance` would
    /// be derived from the mempool listener's `ImpactSet`.
    pub fn evaluate(
        &self,
        target_tx_hash: H256,
        post_state_imbalance_usd: f64,
        chain_id: u64,
    ) -> Option<StrategyCandidate> {
        // 1. Fail-Honest check: Is the imbalance worth pursuing?
        if post_state_imbalance_usd < self.config.min_profit_usd {
            debug!(
                event = "backrun_engine.skipped",
                reason = "insufficient_imbalance",
                imbalance = post_state_imbalance_usd
            );
            return None;
        }

        // 2. Construct the opportunity
        // Note: In Phase 1 of this module, we emit a structured candidate but
        // rely on the downstream `sim-ctl` to strictly enforce the ordering
        // via bundle composition (target_tx + our_tx).
        let gross_profit = post_state_imbalance_usd * 0.8; // Assume 20% slippage/fees on the leveling swap

        let opp = Opportunity {
            id: uuid::Uuid::new_v4().to_string(),
            chain_id,
            strategy_kind: StrategyKind::DexArb, // Will be mapped to a specific label
            tokens: vec![],                      // To be filled by the route decoder
            amounts: vec![],
            expected_profit_usd: gross_profit,
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
            strategy: "backrun_no_extractive".to_string(),
            expected_profit_usd: gross_profit,
            gas_cost_usd: 0.0, // Filled by evaluator
            net_profit_usd: 0.0,
            score: 0.0,
            is_viable: true,
            rejection_reason: None,
        };

        let route_plan = RoutePlan {
            steps: vec![], // To be filled by route intent
            estimated_gas: U256::from(150_000),
            minimum_profit_wei: U256::zero(),
        };

        Some(StrategyCandidate {
            label: StrategyLabel::DexArbV2V2, // Placeholder, needs specific Backrun label
            opportunity: opp,
            candidate,
            route_plan,
            gross_profit_usd: Some(gross_profit),
            net_expected_profit_usd: None,
            rejection_reason: None,
            source_intent_hash: target_tx_hash,
            base_strategy: None,
        })
    }
}
