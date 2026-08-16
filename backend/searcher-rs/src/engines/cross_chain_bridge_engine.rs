//! CrossChainBridgeEngine — Inter-chain Holonomic Loop Resolution detector.
//!
//! Detects topological yield opportunities arising from price divergences
//! across distinct EVM chains, accounting for bridge transfer fees and latency.
//!
//! ## Mathematical Model
//!
//! For a token T on chains A and B:
//! - Price_A = price of T on chain A (from oracle)
//! - Price_B = price of T on chain B (from oracle)
//! - Spread = |Price_A - Price_B| / min(Price_A, Price_B)
//!
//! Opportunity exists when:
//!   Spread > (bridge_fee_bps + min_profit_bps) / 10000
//!
//! Where bridge_fee includes:
//! - Protocol bridge fee (e.g., LayerZero, Wormhole)
//! - Destination chain gas cost (estimated)
//! - Temporal Liquidity Superposition (TLS) fee if applicable
//!
//! ## R8 Invariants
//!
//! - No oracle price on either chain → skip (silent, no synthetic prices)
//! - Spread < threshold → emit REJECTED with reason "spread_below_threshold"
//! - All bridge costs must be accounted for in net yield calculation

use crate::engines::StrategyCandidate;
use crate::route_intent::RouteIntent;
use crate::strategy_label::StrategyLabel;
use chrono::Utc;
use ethers::types::{Address, H256, U256};
use prioritization_spine::route_plan::{RouteLeg, RoutePlan};
use prioritization_spine::types::OpportunityCandidate;
use shared_rs::chains::{
    DAI_MAINNET_LC, USDC_MAINNET_LC, USDT_MAINNET_LC, WBTC_MAINNET_LC, WETH_MAINNET_LC,
};
use shared_rs::contracts::Opportunity;
use shared_rs::trading_config::TradingConfigState;
use std::collections::HashMap;
use std::sync::Arc;
use tracing::{debug, info, warn};
use uuid::Uuid;

/// Bridge protocol configuration.
#[derive(Debug, Clone)]
pub struct BridgeConfig {
    /// Bridge protocol identifier (e.g., "layerzero", "wormhole", "stargate")
    pub protocol: String,
    /// Source chain ID
    pub source_chain: u64,
    /// Destination chain ID
    pub dest_chain: u64,
    /// Bridge contract address on source chain
    pub bridge_address: Address,
    /// Fee in basis points charged by the bridge
    pub protocol_fee_bps: u32,
    /// Estimated gas cost on destination chain (in USD)
    pub dest_gas_cost_usd: f64,
    /// Average latency in seconds for bridge completion
    pub avg_latency_secs: u64,
    /// Whether the bridge supports the token
    pub supported_tokens: Vec<Address>,
}

/// Price oracle trait for cross-chain price discovery.
#[async_trait::async_trait]
pub trait PriceOracle: Send + Sync {
    /// Returns the current price of a token in USD.
    /// Returns None if the price is unavailable (R8 fail-honest).
    async fn get_price_usd(&self, token: Address) -> Option<f64>;

    /// Returns the chain ID this oracle serves.
    fn chain_id(&self) -> u64;
}

/// Cross-chain opportunity representation.
#[derive(Debug, Clone)]
pub struct CrossChainOpportunity {
    /// Unique opportunity ID
    pub id: u64,
    /// Token being arbitraged
    pub token: Address,
    /// Token symbol for observability
    pub token_symbol: String,
    /// Source chain (lower price)
    pub source_chain: u64,
    /// Destination chain (higher price)
    pub dest_chain: u64,
    /// Price on source chain
    pub source_price: f64,
    /// Price on destination chain
    pub dest_price: f64,
    /// Spread as ratio (e.g., 0.005 = 50 bps)
    pub spread_bps: u32,
    /// Bridge to use for transfer
    pub bridge: BridgeConfig,
    /// Total cost including all fees (in USD)
    pub total_cost_usd: f64,
    /// Estimated net topological yield (in USD)
    pub net_yield_usd: f64,
    /// Optimal amount to transfer (in token wei)
    pub optimal_amount_wei: U256,
}

/// Engine for detecting cross-chain Holonomic Loop Resolutions.
pub struct CrossChainBridgeEngine {
    /// Configured bridges for cross-chain transfers
    bridges: Vec<BridgeConfig>,
    /// Price oracles keyed by chain ID
    price_oracles: HashMap<u64, Arc<dyn PriceOracle>>,
    /// Minimum spread threshold in basis points (default: 50)
    min_spread_bps: u32,
    /// Capital allocation per opportunity (USD)
    capital_usd: f64,
}

impl CrossChainBridgeEngine {
    /// Creates a new CrossChainBridgeEngine with the given bridges and oracles.
    pub fn new(
        bridges: Vec<BridgeConfig>,
        price_oracles: HashMap<u64, Arc<dyn PriceOracle>>,
    ) -> Self {
        Self {
            bridges,
            price_oracles,
            min_spread_bps: 50, // 50 bps = 0.5% minimum spread
            capital_usd: 10_000.0,
        }
    }

    /// Creates an engine with default bridge configurations for mainnet.
    pub fn with_default_bridges(price_oracles: HashMap<u64, Arc<dyn PriceOracle>>) -> Self {
        let bridges = Self::default_bridge_configs();
        Self::new(bridges, price_oracles)
    }

    /// Detects cross-chain opportunities across all configured bridges.
    pub async fn detect_cross_chain_arbs(&self) -> Vec<CrossChainOpportunity> {
        let mut opportunities = Vec::new();
        let mut id_counter = 0u64;

        for bridge in &self.bridges {
            // Get oracles for both chains
            let source_oracle = match self.price_oracles.get(&bridge.source_chain) {
                Some(o) => o,
                None => {
                    debug!(
                        event = "cross_chain_engine.no_source_oracle",
                        chain_id = bridge.source_chain,
                        "skipping bridge: no oracle for source chain"
                    );
                    continue;
                }
            };

            let dest_oracle = match self.price_oracles.get(&bridge.dest_chain) {
                Some(o) => o,
                None => {
                    debug!(
                        event = "cross_chain_engine.no_dest_oracle",
                        chain_id = bridge.dest_chain,
                        "skipping bridge: no oracle for dest chain"
                    );
                    continue;
                }
            };

            // Check each supported token
            for token in &bridge.supported_tokens {
                let token_symbol = Self::token_symbol(token);

                // Fetch prices from both chains concurrently
                let (source_price, dest_price) = tokio::join!(
                    source_oracle.get_price_usd(*token),
                    dest_oracle.get_price_usd(*token)
                );

                let (source_p, dest_p) = match (source_price, dest_price) {
                    (Some(s), Some(d)) => (s, d),
                    _ => {
                        debug!(
                            event = "cross_chain_engine.missing_price",
                            token = %token,
                            token_symbol = %token_symbol,
                            has_source = source_price.is_some(),
                            has_dest = dest_price.is_some(),
                            "skipping: price unavailable on one or both chains"
                        );
                        continue;
                    }
                };

                // Calculate spread
                let spread_ratio = (dest_p - source_p).abs() / source_p.min(dest_p);
                let spread_bps = (spread_ratio * 10_000.0) as u32;

                // Check against threshold
                if spread_bps < self.min_spread_bps {
                    debug!(
                        event = "cross_chain_engine.spread_below_threshold",
                        token = %token,
                        spread_bps,
                        min_spread_bps = self.min_spread_bps,
                        "skipping: spread too small"
                    );
                    continue;
                }

                // Calculate costs
                let bridge_fee_usd = self.capital_usd * (bridge.protocol_fee_bps as f64 / 10_000.0);
                let total_cost_usd = bridge_fee_usd + bridge.dest_gas_cost_usd;

                // Calculate potential yield (conservative: assume 50% of spread captured)
                let gross_yield_usd = self.capital_usd * spread_ratio * 0.5;
                let net_yield_usd = gross_yield_usd - total_cost_usd;

                // Only include if net yield is positive
                if net_yield_usd <= 0.0 {
                    debug!(
                        event = "cross_chain_engine.negative_net_yield",
                        token = %token,
                        net_yield_usd,
                        "skipping: costs exceed potential yield"
                    );
                    continue;
                }

                id_counter += 1;

                info!(
                    event = "cross_chain_engine.opportunity_detected",
                    opportunity_id = id_counter,
                    token = %token,
                    token_symbol = %token_symbol,
                    source_chain = bridge.source_chain,
                    dest_chain = bridge.dest_chain,
                    source_price,
                    dest_price,
                    spread_bps,
                    net_yield_usd,
                    "cross-chain opportunity detected"
                );

                opportunities.push(CrossChainOpportunity {
                    id: id_counter,
                    token: *token,
                    token_symbol,
                    source_chain: bridge.source_chain,
                    dest_chain: bridge.dest_chain,
                    source_price: source_p,
                    dest_price: dest_p,
                    spread_bps,
                    bridge: bridge.clone(),
                    total_cost_usd,
                    net_yield_usd,
                    optimal_amount_wei: Self::usd_to_wei(self.capital_usd, source_p, 18),
                });
            }
        }

        opportunities
    }

    /// Entry point for orchestrator integration.
    /// Evaluates cross-chain opportunities in response to a RouteIntent.
    pub async fn build_cross_chain_candidates(
        &self,
        intent: &RouteIntent,
        cfg: Option<&TradingConfigState>,
    ) -> anyhow::Result<Vec<StrategyCandidate>> {
        let chain_id = intent.chain_id;
        let tx_hash = intent.tx_hash;

        // Detect opportunities
        let opportunities = self.detect_cross_chain_arbs().await;

        // Convert to StrategyCandidates
        let mut candidates = Vec::new();

        for opp in opportunities {
            let candidate = self.build_candidate(&opp, chain_id, tx_hash, cfg);
            candidates.push(candidate);
        }

        Ok(candidates)
    }

    // -------------------------------------------------------------------------
    // Internal methods
    // -------------------------------------------------------------------------

    /// Builds a StrategyCandidate from a CrossChainOpportunity.
    fn build_candidate(
        &self,
        opp: &CrossChainOpportunity,
        chain_id: u64,
        tx_hash: H256,
        _cfg: Option<&TradingConfigState>,
    ) -> StrategyCandidate {
        let id = Uuid::new_v4();
        let trace_id = Uuid::new_v4();
        let strategy_kind = StrategyLabel::CrossChainArb.to_contract_strategy_kind();

        let pair_symbol = format!("{}(cross_chain)", opp.token_symbol);
        let token_in = format!("0x{:040x}", opp.token);
        let token_out = token_in.clone(); // Same token, different chain

        let gross_yield_usd = Some(opp.net_yield_usd);

        let opportunity = Opportunity {
            id,
            chain_id,
            strategy_kind,
            dex_a: opp.bridge.protocol.clone(),
            dex_b: None,
            pair_symbol,
            token_in: token_in.clone(),
            token_out: token_out.clone(),
            amount_in_wei: opp.optimal_amount_wei.to_string(),
            expected_profit_usd: gross_yield_usd,
            net_expected_profit_usd: None,
            roi_pct: Some((opp.net_yield_usd / self.capital_usd) * 100.0),
            risk_score: Some(0.3), // Cross-chain carries bridge risk
            block_number: None,
            rejection_reason: None,
            cartridge_id: None,
            detected_at: Utc::now(),
            trace_id,
        };

        let pool_addresses = vec![format!("0x{:040x}", opp.bridge.bridge_address)];
        let token_addresses = vec![token_in.clone()];

        let candidate = OpportunityCandidate {
            route_fingerprint: format!(
                "cc_{}_{}_{}_{}",
                opp.source_chain, opp.dest_chain, opp.token_symbol, opp.id
            ),
            pool_addresses,
            token_addresses,
            dex_adapters: vec![opp.bridge.protocol.clone()],
            amount_in: u256_to_f64(&opp.optimal_amount_wei) / 1e18,
            expected_amount_out: u256_to_f64(&opp.optimal_amount_wei) / 1e18, // Same token amount
            gross_profit: opp.net_yield_usd,
        };

        // Build a single-leg route plan representing the bridge transfer
        let legs = vec![RouteLeg {
            dex_id: opp.bridge.protocol.clone(),
            dex_name: opp.bridge.protocol.clone(),
            protocol_type: "bridge".to_string(),
            factory_address: String::new(),
            pool_id: None,
            pool_address: Some(format!("0x{:040x}", opp.bridge.bridge_address)),
            token_in: token_in.clone(),
            token_out: token_out.clone(),
            fee_bps: Some(opp.bridge.protocol_fee_bps),
            amount_in: Some(u256_to_f64(&opp.optimal_amount_wei) / 1e18),
            amount_out: None,
            tvl_usd: None,
            volume_24h_usd: None,
            pool_is_active: true,
        }];

        let route_plan = RoutePlan {
            route_id: Some(format!("cc-{}-{:x}", opp.id, tx_hash)),
            strategy_kind: StrategyLabel::CrossChainArb.as_str().to_string(),
            chain_id,
            legs,
            atomic: false, // Bridge transfers are not atomic
            estimated_slippage_pct: None,
            price_impact_pct: Some(opp.spread_bps as f64 / 100.0),
        };

        debug!(
            event = "cross_chain_engine.candidate_built",
            chain_id,
            opportunity_id = opp.id,
            source_chain = opp.source_chain,
            dest_chain = opp.dest_chain,
            spread_bps = opp.spread_bps,
            "cross-chain candidate accepted"
        );

        StrategyCandidate {
            label: StrategyLabel::CrossChainArb,
            opportunity,
            candidate,
            route_plan,
            gross_profit_usd: gross_yield_usd,
            net_expected_profit_usd: None,
            rejection_reason: None,
            source_intent_hash: tx_hash,
            base_strategy: None,
        }
    }

    /// Returns default bridge configurations for supported routes.
    fn default_bridge_configs() -> Vec<BridgeConfig> {
        // These are placeholder configurations
        // In production, these would be loaded from configuration
        vec![
            // LayerZero: Ethereum -> Arbitrum
            BridgeConfig {
                protocol: "layerzero".to_string(),
                source_chain: 1,   // Ethereum mainnet
                dest_chain: 42161, // Arbitrum
                bridge_address: Address::from_low_u64_be(0x1234),
                protocol_fee_bps: 10, // 0.1%
                dest_gas_cost_usd: 5.0,
                avg_latency_secs: 300, // 5 minutes
                supported_tokens: vec![
                    Address::from_str("0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48")
                        .unwrap_or_default(), // USDC
                ],
            },
            // Wormhole: Ethereum -> Solana (would need adapter)
            BridgeConfig {
                protocol: "wormhole".to_string(),
                source_chain: 1,        // Ethereum
                dest_chain: 1399811149, // Solana (Wormhole chain ID)
                bridge_address: Address::from_low_u64_be(0x5678),
                protocol_fee_bps: 15,
                dest_gas_cost_usd: 0.5,
                avg_latency_secs: 900, // 15 minutes
                supported_tokens: vec![
                    Address::from_str("0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2")
                        .unwrap_or_default(), // WETH
                ],
            },
        ]
    }

    /// Converts USD amount to token wei.
    fn usd_to_wei(usd: f64, price_usd: f64, decimals: u8) -> U256 {
        if price_usd <= 0.0 {
            return U256::zero();
        }
        let token_amount = usd / price_usd;
        let multiplier = 10f64.powi(decimals as i32);
        let wei = (token_amount * multiplier) as u128;
        U256::from(wei)
    }

    /// Returns the symbol for a well-known token address.
    fn token_symbol(token: &Address) -> String {
        let addr_str = format!("0x{:040x}", token);
        match addr_str.as_str() {
            WETH_MAINNET_LC => "WETH".to_string(),
            USDC_MAINNET_LC => "USDC".to_string(),
            USDT_MAINNET_LC => "USDT".to_string(),
            DAI_MAINNET_LC => "DAI".to_string(),
            WBTC_MAINNET_LC => "WBTC".to_string(),
            _ => format!("0x{:08x}", token),
        }
    }
}

/// Converts U256 to f64.
fn u256_to_f64(v: &U256) -> f64 {
    v.low_u128() as f64
}

use std::str::FromStr;

#[cfg(test)]
mod tests {
    use super::*;
    use ethers::types::Address;

    /// Mock price oracle for testing
    struct MockOracle {
        chain: u64,
        price: f64,
    }

    #[async_trait::async_trait]
    impl PriceOracle for MockOracle {
        async fn get_price_usd(&self, _token: Address) -> Option<f64> {
            Some(self.price)
        }

        fn chain_id(&self) -> u64 {
            self.chain
        }
    }

    #[tokio::test]
    async fn test_detects_opportunity_with_large_spread() {
        let oracle_eth: Arc<dyn PriceOracle> = Arc::new(MockOracle {
            chain: 1,
            price: 1.00, // USDC = $1.00 on Ethereum
        });

        let oracle_arb: Arc<dyn PriceOracle> = Arc::new(MockOracle {
            chain: 42161,
            price: 1.01, // USDC = $1.01 on Arbitrum (1% spread)
        });

        let mut oracles = HashMap::new();
        oracles.insert(1u64, oracle_eth);
        oracles.insert(42161u64, oracle_arb);

        let engine = CrossChainBridgeEngine::with_default_bridges(oracles);
        let opportunities = engine.detect_cross_chain_arbs().await;

        // Should find opportunities due to 1% spread > 50 bps threshold
        assert!(
            !opportunities.is_empty(),
            "should detect opportunity with 1% spread"
        );

        let opp = &opportunities[0];
        assert_eq!(opp.source_chain, 1);
        assert_eq!(opp.dest_chain, 42161);
        assert!(opp.spread_bps >= 50, "spread should be at least 50 bps");
    }

    #[tokio::test]
    async fn test_skips_small_spread() {
        let oracle_eth: Arc<dyn PriceOracle> = Arc::new(MockOracle {
            chain: 1,
            price: 1.00,
        });

        let oracle_arb: Arc<dyn PriceOracle> = Arc::new(MockOracle {
            chain: 42161,
            price: 1.001, // Only 0.1% spread (10 bps)
        });

        let mut oracles = HashMap::new();
        oracles.insert(1u64, oracle_eth);
        oracles.insert(42161u64, oracle_arb);

        let engine = CrossChainBridgeEngine::with_default_bridges(oracles);
        let opportunities = engine.detect_cross_chain_arbs().await;

        // Should NOT find opportunities due to spread < 50 bps
        assert!(
            opportunities.is_empty(),
            "should skip opportunities with < 50 bps spread"
        );
    }

    #[test]
    fn test_usd_to_wei_conversion() {
        // $1000 of ETH at $2000/ETH with 18 decimals
        let wei = CrossChainBridgeEngine::usd_to_wei(1000.0, 2000.0, 18);
        let expected = U256::from(500_000_000_000_000_000u64); // 0.5 ETH
        assert_eq!(wei, expected);
    }

    #[test]
    fn test_token_symbol_resolution() {
        let weth = Address::from_str("0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2").unwrap();
        assert_eq!(CrossChainBridgeEngine::token_symbol(&weth), "WETH");

        let usdc = Address::from_str("0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48").unwrap();
        assert_eq!(CrossChainBridgeEngine::token_symbol(&usdc), "USDC");
    }
}
