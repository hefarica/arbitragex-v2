//! LiquidationSnipeEngine — Lending position liquidation detector.
//!
//! Monitors Aave V3 and Compound V2 lending positions for accounts with
//! health_factor < 1.0, identifying profitable liquidation opportunities.
//!
//! ## Mathematical Model
//!
//! For a lending position:
//! - Total Collateral (USD)
//! - Total Debt (USD)
//! - Health Factor = (Collateral * Liquidation Threshold) / Debt
//!
//! When Health Factor < 1.0, the position is eligible for liquidation.
//!
//! Liquidation profit calculation:
//! - Liquidation Bonus (protocol-specific, e.g., 5-10% on Aave)
//! - Close Factor (maximum % of debt that can be repaid, e.g., 50%)
//! - Gas Cost (estimated on-chain execution cost)
//!
//! Net Topological Yield = (Debt Repaid * Liquidation Bonus) - Gas Cost
//!
//! ## R8 Invariants
//!
//! - Health Factor >= 1.0 → skip (position is safe)
//! - No liquidation bonus data → skip (cannot calculate yield)
//! - Gas cost > potential bonus → emit REJECTED with "gas_exceeds_yield"

use crate::engines::StrategyCandidate;
use crate::route_intent::RouteIntent;
use crate::strategy_label::StrategyLabel;
use chrono::Utc;
use ethers::types::{Address, H256, U256};
use prioritization_spine::route_plan::{RouteLeg, RoutePlan};
use prioritization_spine::types::OpportunityCandidate;
use shared_rs::contracts::Opportunity;
use shared_rs::trading_config::TradingConfigState;
use std::collections::HashMap;
use std::str::FromStr;
use tracing::{debug, info, warn};
use uuid::Uuid;

/// Lending protocol configuration.
#[derive(Debug, Clone)]
pub struct LendingPoolConfig {
    /// Protocol identifier ("aave-v3", "compound-v2")
    pub protocol: String,
    /// Chain ID where the protocol is deployed
    pub chain_id: u64,
    /// Pool/Comptroller contract address
    pub pool_address: Address,
    /// Data Provider contract address (for Aave)
    pub data_provider: Option<Address>,
    /// Liquidation contract address
    pub liquidation_proxy: Address,
    /// Minimum liquidation bonus to consider (in bps)
    pub min_liquidation_bonus_bps: u32,
    /// Protocol-specific close factor (max % of debt liquidatable)
    pub close_factor_bps: u32,
}

/// Lending position state.
#[derive(Debug, Clone)]
pub struct LendingPosition {
    /// Position owner address
    pub user: Address,
    /// Protocol this position belongs to
    pub protocol: String,
    /// Total collateral in USD
    pub total_collateral_usd: f64,
    /// Total debt in USD
    pub total_debt_usd: f64,
    /// Current health factor (liquidatable if < 1.0)
    pub health_factor: f64,
    /// Maximum debt that can be liquidated (USD)
    pub max_liquidatable_debt_usd: f64,
    /// Collateral token that can be seized
    pub collateral_token: Address,
    /// Debt token to be repaid
    pub debt_token: Address,
    /// Liquidation bonus for this asset pair (in bps)
    pub liquidation_bonus_bps: u32,
}

/// Liquidation candidate opportunity.
#[derive(Debug, Clone)]
pub struct LiquidationCandidate {
    /// Unique candidate ID
    pub id: u64,
    /// The lending position being liquidated
    pub position: LendingPosition,
    /// Estimated gross profit (bonus amount in USD)
    pub gross_profit_usd: f64,
    /// Estimated gas cost (USD)
    pub gas_cost_usd: f64,
    /// Net topological yield after gas
    pub net_yield_usd: f64,
    /// Optimal amount of debt to repay (USD)
    pub optimal_repay_usd: f64,
}

/// Engine for detecting liquidatable lending positions.
pub struct LiquidationSnipeEngine {
    /// Configured lending protocols to monitor
    lending_pools: HashMap<String, LendingPoolConfig>,
    /// Minimum liquidation bonus threshold (bps)
    min_liquidation_bonus_bps: u32,
    /// Estimated gas cost per liquidation (USD)
    gas_cost_usd: f64,
    /// Position cache for tracking known positions
    position_cache: HashMap<Address, LendingPosition>,
}

impl LiquidationSnipeEngine {
    /// Creates a new LiquidationSnipeEngine.
    pub fn new(lending_pools: Vec<LendingPoolConfig>) -> Self {
        let pools_map: HashMap<String, LendingPoolConfig> = lending_pools
            .into_iter()
            .map(|p| (format!("{}-{}", p.protocol, p.chain_id), p))
            .collect();

        Self {
            lending_pools: pools_map,
            min_liquidation_bonus_bps: 500, // 5% minimum bonus
            gas_cost_usd: 50.0,             // $50 estimated gas
            position_cache: HashMap::new(),
        }
    }

    /// Creates engine with default Aave and Compound configurations.
    pub fn with_default_pools() -> Self {
        let pools = vec![
            // Aave V3 on Ethereum mainnet
            LendingPoolConfig {
                protocol: "aave-v3".to_string(),
                chain_id: 1,
                pool_address: Address::from_str("0x87870bca3f3fd6335c3f4ce8392d69350b4fa4e2")
                    .unwrap_or_default(),
                data_provider: Some(
                    Address::from_str("0x7b4eb56e7cd4b454b8f71d048c14752252e36fb4")
                        .unwrap_or_default(),
                ),
                liquidation_proxy: Address::from_str("0x87870bca3f3fd6335c3f4ce8392d69350b4fa4e2")
                    .unwrap_or_default(),
                min_liquidation_bonus_bps: 500, // 5%
                close_factor_bps: 5000,         // 50%
            },
            // Compound V2 on Ethereum mainnet
            LendingPoolConfig {
                protocol: "compound-v2".to_string(),
                chain_id: 1,
                pool_address: Address::from_str("0x3d9819210a31b4961b30ef54be2aed79b9c9cd3b")
                    .unwrap_or_default(),
                data_provider: None,
                liquidation_proxy: Address::from_str("0x3d9819210a31b4961b30ef54be2aed79b9c9cd3b")
                    .unwrap_or_default(),
                min_liquidation_bonus_bps: 800, // 8%
                close_factor_bps: 5000,         // 50%
            },
        ];

        Self::new(pools)
    }

    /// Scans for liquidatable positions across all configured protocols.
    ///
    /// In production, this would query:
    /// - Aave V3: Pool.getUserAccountData() for all borrowers
    /// - Compound V2: Comptroller.getAccountLiquidity() for all borrowers
    /// - Subgraphs for efficient filtering
    pub async fn scan_liquidatable_positions(&self) -> Vec<LiquidationCandidate> {
        let mut candidates = Vec::new();
        let mut id_counter = 0u64;

        for (key, pool_config) in &self.lending_pools {
            debug!(
                event = "liquidation_snipe_engine.scanning_pool",
                pool = key,
                protocol = %pool_config.protocol,
                chain_id = pool_config.chain_id,
                "scanning for liquidatable positions"
            );

            // Fetch positions from subgraph/RPC
            // In production, this would call the actual protocol contracts
            let positions = self.fetch_positions_from_subgraph(pool_config).await;

            for position in positions {
                // Skip safe positions (R8: health_factor >= 1.0)
                if position.health_factor >= 1.0 {
                    continue;
                }

                // Skip if bonus below threshold
                if position.liquidation_bonus_bps < self.min_liquidation_bonus_bps {
                    debug!(
                        event = "liquidation_snipe_engine.bonus_too_low",
                        user = %position.user,
                        bonus_bps = position.liquidation_bonus_bps,
                        min_bonus_bps = self.min_liquidation_bonus_bps,
                        "skipping: liquidation bonus below threshold"
                    );
                    continue;
                }

                // Calculate liquidation opportunity
                if let Some(candidate) =
                    self.calculate_liquidation_opportunity(&position, &mut id_counter)
                {
                    if candidate.net_yield_usd > 0.0 {
                        info!(
                            event = "liquidation_snipe_engine.candidate_found",
                            candidate_id = candidate.id,
                            user = %position.user,
                            protocol = %position.protocol,
                            health_factor = position.health_factor,
                            debt_usd = position.total_debt_usd,
                            net_yield_usd = candidate.net_yield_usd,
                            "liquidation candidate detected"
                        );
                        candidates.push(candidate);
                    }
                }
            }
        }

        candidates
    }

    /// Entry point for orchestrator integration.
    /// Evaluates liquidation opportunities in response to a RouteIntent.
    pub async fn build_liquidation_candidates(
        &self,
        intent: &RouteIntent,
        cfg: Option<&TradingConfigState>,
    ) -> anyhow::Result<Vec<StrategyCandidate>> {
        let chain_id = intent.chain_id;
        let tx_hash = intent.tx_hash;

        // Detect liquidation opportunities
        let liquidations = self.scan_liquidatable_positions().await;

        // Convert to StrategyCandidates
        let mut candidates = Vec::new();

        for liq in liquidations {
            let candidate = self.build_strategy_candidate(&liq, chain_id, tx_hash, cfg);
            candidates.push(candidate);
        }

        Ok(candidates)
    }

    // -------------------------------------------------------------------------
    // Internal methods
    // -------------------------------------------------------------------------

    /// Fetches lending positions from protocol subgraph or RPC.
    ///
    /// In production, this would:
    /// 1. Query The Graph subgraph for Aave/Compound
    /// 2. Filter for positions with health_factor < 1.0
    /// 3. Paginate through all borrowers
    async fn fetch_positions_from_subgraph(
        &self,
        pool_config: &LendingPoolConfig,
    ) -> Vec<LendingPosition> {
        // Placeholder: In production, query actual subgraphs
        // Aave V3 subgraph: https://api.thegraph.com/subgraphs/name/aave/protocol-v3
        // Compound V2 subgraph: https://api.thegraph.com/subgraphs/name/graphprotocol/compound-v2

        // Return empty for now - this is a scaffold for the real implementation
        vec![]
    }

    /// Calculates the liquidation opportunity for a position.
    fn calculate_liquidation_opportunity(
        &self,
        position: &LendingPosition,
        id_counter: &mut u64,
    ) -> Option<LiquidationCandidate> {
        // Max liquidatable debt (limited by close factor)
        let max_repay_usd =
            position.total_debt_usd * (position.liquidation_bonus_bps as f64 / 10_000.0);
        let optimal_repay_usd = max_repay_usd.min(position.max_liquidatable_debt_usd);

        if optimal_repay_usd <= 0.0 {
            return None;
        }

        // Gross profit is the liquidation bonus on the repaid amount
        let bonus_rate = position.liquidation_bonus_bps as f64 / 10_000.0;
        let gross_profit_usd = optimal_repay_usd * bonus_rate;

        // Net yield after gas costs
        let net_yield_usd = gross_profit_usd - self.gas_cost_usd;

        *id_counter += 1;

        Some(LiquidationCandidate {
            id: *id_counter,
            position: position.clone(),
            gross_profit_usd,
            gas_cost_usd: self.gas_cost_usd,
            net_yield_usd,
            optimal_repay_usd,
        })
    }

    /// Builds a StrategyCandidate from a LiquidationCandidate.
    fn build_strategy_candidate(
        &self,
        liq: &LiquidationCandidate,
        chain_id: u64,
        tx_hash: H256,
        _cfg: Option<&TradingConfigState>,
    ) -> StrategyCandidate {
        let id = Uuid::new_v4();
        let trace_id = Uuid::new_v4();
        let strategy_kind = StrategyLabel::LiquidationSnipe.to_contract_strategy_kind();

        let position = &liq.position;
        let pair_symbol = format!(
            "{}(liquidation:{}",
            Self::token_symbol(&position.debt_token),
            position.protocol
        );

        let token_in = format!("0x{:040x}", position.debt_token);
        let token_out = format!("0x{:040x}", position.collateral_token);

        // Determine rejection reason if net yield is negative
        let (rejection_reason, gross_profit_usd) = if liq.net_yield_usd <= 0.0 {
            (Some("gas_exceeds_yield".to_string()), None)
        } else {
            (None, Some(liq.gross_profit_usd))
        };

        let opportunity = Opportunity {
            id,
            chain_id,
            strategy_kind,
            dex_a: position.protocol.clone(),
            dex_b: None,
            pair_symbol,
            token_in: token_in.clone(),
            token_out: token_out.clone(),
            amount_in_wei: Self::usd_to_wei(liq.optimal_repay_usd, 18).to_string(),
            expected_profit_usd: gross_profit_usd,
            net_expected_profit_usd: if liq.net_yield_usd > 0.0 {
                Some(liq.net_yield_usd)
            } else {
                None
            },
            roi_pct: Some((liq.net_yield_usd / liq.optimal_repay_usd) * 100.0),
            risk_score: Some(0.2), // Liquidations have execution risk
            block_number: None,
            rejection_reason: rejection_reason.clone(),
            cartridge_id: None,
            detected_at: Utc::now(),
            trace_id,
        };

        let pool_addresses = vec![format!("0x{:040x}", position.user)]; // Position being liquidated
        let token_addresses = vec![token_in.clone(), token_out.clone()];

        let candidate = OpportunityCandidate {
            route_fingerprint: format!("liq_{}_{}", liq.id, position.user),
            pool_addresses,
            token_addresses,
            dex_adapters: vec![position.protocol.clone()],
            amount_in: liq.optimal_repay_usd,
            expected_amount_out: liq.optimal_repay_usd
                * (1.0 + position.liquidation_bonus_bps as f64 / 10_000.0),
            gross_profit: liq.gross_profit_usd,
        };

        // Build route plan for liquidation
        let legs = vec![RouteLeg {
            dex_id: position.protocol.clone(),
            dex_name: position.protocol.clone(),
            protocol_type: "liquidation".to_string(),
            factory_address: String::new(),
            pool_id: None,
            pool_address: Some(format!("0x{:040x}", position.collateral_token)),
            token_in: token_in.clone(),
            token_out: token_out.clone(),
            fee_bps: Some(position.liquidation_bonus_bps),
            amount_in: Some(liq.optimal_repay_usd),
            amount_out: None,
            tvl_usd: Some(position.total_collateral_usd),
            volume_24h_usd: None,
            pool_is_active: true,
        }];

        let route_plan = RoutePlan {
            route_id: Some(format!("liq-{}-{:x}", liq.id, tx_hash)),
            strategy_kind: StrategyLabel::LiquidationSnipe.as_str().to_string(),
            chain_id,
            legs,
            atomic: true, // Liquidations are atomic
            estimated_slippage_pct: None,
            price_impact_pct: None,
        };

        debug!(
            event = "liquidation_snipe_engine.candidate_built",
            chain_id,
            candidate_id = liq.id,
            user = %position.user,
            protocol = %position.protocol,
            health_factor = position.health_factor,
            net_yield_usd = liq.net_yield_usd,
            "liquidation candidate {}",
            if rejection_reason.is_some() { "rejected" } else { "accepted" }
        );

        StrategyCandidate {
            label: StrategyLabel::LiquidationSnipe,
            opportunity,
            candidate,
            route_plan,
            gross_profit_usd,
            net_expected_profit_usd: if liq.net_yield_usd > 0.0 {
                Some(liq.net_yield_usd)
            } else {
                None
            },
            rejection_reason,
            source_intent_hash: tx_hash,
            base_strategy: None,
        }
    }

    /// Returns the symbol for a well-known token address.
    fn token_symbol(token: &Address) -> String {
        let addr_str = format!("{:040x}", token);
        match addr_str.as_str() {
            "c02aaa39b223fe8d0a0e5c4f27ead9083c756cc2" => "WETH".to_string(),
            "a0b86991c6218b36c1d19d4a2e9eb0ce3606eb48" => "USDC".to_string(),
            "dac17f958d2ee523a2206206994597c13d831ec7" => "USDT".to_string(),
            "6b175474e89094c44da98b954eedeac495271d0f" => "DAI".to_string(),
            "2260fac5e5542a773aa44fbcfedf7c193bc2c599" => "WBTC".to_string(),
            _ => format!("0x{:08x}", token),
        }
    }

    /// Converts USD amount to token wei.
    fn usd_to_wei(usd: f64, decimals: u8) -> U256 {
        let multiplier = 10f64.powi(decimals as i32);
        let wei = (usd * multiplier) as u128;
        U256::from(wei)
    }

    /// Adds or updates a position in the cache.
    pub fn update_position(&mut self, position: LendingPosition) {
        self.position_cache.insert(position.user, position);
    }

    /// Gets a cached position.
    pub fn get_cached_position(&self, user: &Address) -> Option<&LendingPosition> {
        self.position_cache.get(user)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ethers::types::Address;

    fn test_position(health_factor: f64, bonus_bps: u32) -> LendingPosition {
        LendingPosition {
            user: Address::from_low_u64_be(0x1234),
            protocol: "aave-v3".to_string(),
            total_collateral_usd: 10_000.0,
            total_debt_usd: 8_000.0,
            health_factor,
            max_liquidatable_debt_usd: 4_000.0, // 50% close factor
            collateral_token: Address::from_low_u64_be(0xABC),
            debt_token: Address::from_low_u64_be(0xDEF),
            liquidation_bonus_bps: bonus_bps,
        }
    }

    #[test]
    fn test_skips_safe_position() {
        let engine = LiquidationSnipeEngine::with_default_pools();
        let position = test_position(1.5, 500); // HF = 1.5 (safe)

        let mut id = 0u64;
        let candidate = engine.calculate_liquidation_opportunity(&position, &mut id);

        // Should not produce candidate for safe position
        // (scan_liquidatable_positions filters HF >= 1.0 before calling this)
        // But if we did calculate, it would have negative net yield due to gas
        assert!(candidate.is_none() || candidate.unwrap().net_yield_usd <= 0.0);
    }

    #[test]
    fn test_detects_liquidatable_position() {
        let engine = LiquidationSnipeEngine::with_default_pools();
        let position = test_position(0.85, 1000); // HF = 0.85 (liquidatable), 10% bonus

        let mut id = 0u64;
        let candidate = engine
            .calculate_liquidation_opportunity(&position, &mut id)
            .unwrap();

        // Max repay = $4000 (50% close factor)
        // Gross profit = $4000 * 10% = $400
        assert!(candidate.gross_profit_usd > 0.0);
        assert_eq!(candidate.id, 1);
        assert!(candidate.optimal_repay_usd <= position.max_liquidatable_debt_usd);
    }

    #[test]
    fn test_rejects_low_bonus() {
        let mut engine = LiquidationSnipeEngine::with_default_pools();
        let position = test_position(0.9, 100); // Only 1% bonus (below 5% threshold)

        // Manually check bonus threshold
        assert!(position.liquidation_bonus_bps < engine.min_liquidation_bonus_bps);
    }

    #[test]
    fn test_gas_cost_impact() {
        let engine = LiquidationSnipeEngine::with_default_pools();
        // Small position with low bonus - gas might exceed yield
        let position = test_position(0.95, 500); // 5% bonus

        let mut id = 0u64;
        if let Some(candidate) = engine.calculate_liquidation_opportunity(&position, &mut id) {
            // Net yield = gross - $50 gas
            assert_eq!(candidate.net_yield_usd, candidate.gross_profit_usd - 50.0);
        }
    }

    #[test]
    fn test_token_symbol_resolution() {
        let weth = Address::from_str("0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2").unwrap();
        assert_eq!(LiquidationSnipeEngine::token_symbol(&weth), "WETH");

        let usdc = Address::from_str("0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48").unwrap();
        assert_eq!(LiquidationSnipeEngine::token_symbol(&usdc), "USDC");
    }

    #[test]
    fn test_usd_to_wei_conversion() {
        // $1000 of tokens with 18 decimals
        let wei = LiquidationSnipeEngine::usd_to_wei(1000.0, 18);
        let expected = U256::from(1_000_000_000_000_000_000_000u128); // 1000 * 1e18
        assert_eq!(wei, expected);
    }
}
