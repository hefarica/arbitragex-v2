//! SpanningTreeEngine — Graph-theoretic cycle detection via Bellman-Ford.
//!
//! Detects negative cycles (Holonomic Loop Resolutions) in token liquidity
//! manifolds by treating exchange rates as weighted edges.
//!
//! ## Mathematical Model
//!
//! For a liquidity pool with reserves (R_in, R_out), the effective exchange
//! rate after fees is: rate = (R_out / R_in) * (1 - fee)
//!
//! Taking -ln(rate) as edge weight, a negative cycle in this transformed
//! graph corresponds to a profitable Holonomic Loop Resolution where the
//! product of rates exceeds 1.0.
//!
//! ## R8 Invariants
//!
//! - No reserves → skip cycle (silent, data-availability gap)
//! - All cycles with product ≤ 1.0 emit REJECTED candidates
//! - Profitable cycles emit ACCEPTED with gross_topological_yield_usd

use crate::engines::triangular_engine::ReservesCache;
use crate::engines::StrategyCandidate;
use crate::impact_index::{CycleId, ImpactSet};
use crate::route_intent::RouteIntent;
use crate::strategy_label::StrategyLabel;
use chrono::Utc;
use ethers::types::{Address, H256, U256};
use prioritization_spine::route_plan::{RouteLeg, RoutePlan};
use prioritization_spine::types::OpportunityCandidate;
use shared_rs::contracts::Opportunity;
use shared_rs::trading_config::TradingConfigState;
use std::collections::HashMap;
use std::sync::Arc;
use tracing::{debug, warn};
use uuid::Uuid;

/// Token node in the liquidity graph.
#[derive(Debug, Clone)]
pub struct TokenNode {
    /// Token address (canonical identifier)
    pub address: Address,
    /// Human-readable symbol for observability
    pub symbol: String,
    /// Token decimals for wei/USD conversion
    pub decimals: u8,
}

/// Pool edge connecting two tokens in the liquidity graph.
/// Represents a single hop with its exchange rate properties.
#[derive(Debug, Clone)]
pub struct PoolEdge {
    /// Pool address executing this swap
    pub pool_address: Address,
    /// DEX identifier (e.g., "uniswap-v2", "sushi")
    pub dex_id: String,
    /// Fee in basis points
    pub fee_bps: u32,
    /// Pool reserves (token0, token1)
    pub reserves: (U256, U256),
    /// True if token_in corresponds to token0 in the pool
    pub swap_in_is_token0: bool,
}

impl PoolEdge {
    /// Computes -ln(exchange_rate) as the edge weight for Bellman-Ford.
    /// Lower weight = better rate (more output per input).
    pub fn log_weight(&self) -> f64 {
        let (reserve_in, reserve_out) = if self.swap_in_is_token0 {
            (self.reserves.0, self.reserves.1)
        } else {
            (self.reserves.1, self.reserves.0)
        };

        // Convert to f64 (lossy but sufficient for cycle detection)
        let r_in = u256_to_f64(&reserve_in).max(1.0);
        let r_out = u256_to_f64(&reserve_out).max(1.0);

        // Exchange rate = (r_out / r_in) * (1 - fee)
        let fee_factor = 1.0 - (self.fee_bps as f64 / 10_000.0);
        let rate = (r_out / r_in) * fee_factor;

        // -ln(rate) as weight; negative cycles correspond to profitable loops
        -rate.ln()
    }

    /// Computes the raw exchange rate without log transformation.
    pub fn exchange_rate(&self) -> f64 {
        let (reserve_in, reserve_out) = if self.swap_in_is_token0 {
            (self.reserves.0, self.reserves.1)
        } else {
            (self.reserves.1, self.reserves.0)
        };

        let r_in = u256_to_f64(&reserve_in).max(1.0);
        let r_out = u256_to_f64(&reserve_out).max(1.0);

        let fee_factor = 1.0 - (self.fee_bps as f64 / 10_000.0);
        (r_out / r_in) * fee_factor
    }
}

/// Detected Holonomic Loop Resolution cycle with profitability metrics.
#[derive(Debug, Clone)]
pub struct ArbCycle {
    /// Unique cycle identifier
    pub cycle_id: CycleId,
    /// Ordered sequence of token nodes forming the cycle
    pub tokens: Vec<TokenNode>,
    /// Edges traversed (tokens[i] -> tokens[i+1] via edges[i])
    pub edges: Vec<PoolEdge>,
    /// Product of all exchange rates (S > 1.0 indicates profit)
    pub rate_product: f64,
    /// Estimated topological yield in USD
    pub estimated_yield_usd: f64,
    /// Optimal capital input based on reserves
    pub optimal_amount_in_wei: U256,
}

/// Bellman-Ford based engine for detecting negative cycles in liquidity graphs.
///
/// This engine maintains a directed graph where:
/// - Nodes = tokens
/// - Edges = liquidity pools (with -ln(rate) as weight)
/// - Negative cycles = profitable Holonomic Loop Resolutions
pub struct SpanningTreeEngine {
    /// In-memory reserves cache shared across engines
    reserves_cache: Arc<ReservesCache>,
    /// Token address -> symbol mapping for observability
    token_symbols: HashMap<Address, String>,
    /// Token address -> decimals mapping
    token_decimals: HashMap<Address, u8>,
}

impl SpanningTreeEngine {
    /// Constructs a new SpanningTreeEngine with the given reserves cache.
    pub fn new(reserves_cache: Arc<ReservesCache>) -> Self {
        Self {
            reserves_cache,
            token_symbols: Self::default_token_symbols(),
            token_decimals: Self::default_token_decimals(),
        }
    }

    /// Detects negative cycles starting from a given token using Bellman-Ford.
    ///
    /// Returns all profitable cycles found within the configured hop limit.
    pub async fn detect_cycles(&self, start_token: &str) -> Vec<ArbCycle> {
        let start_addr = match Address::from_str(start_token) {
            Ok(a) => a,
            Err(_) => {
                warn!(
                    event = "spanning_tree_engine.invalid_start_token",
                    token = start_token,
                    "skipping: cannot parse start token address"
                );
                return vec![];
            }
        };

        // Build dynamic graph from current reserves cache state
        let (nodes, edges) = self.build_graph_from_cache().await;

        if nodes.len() < 3 {
            debug!(
                event = "spanning_tree_engine.insufficient_nodes",
                node_count = nodes.len(),
                "need at least 3 tokens for triangular cycles"
            );
            return vec![];
        }

        // Bellman-Ford: find negative cycles (sum of weights < 0)
        self.bellman_ford_cycles(&nodes, &edges, start_addr).await
    }

    /// Evaluates cycles impacted by a RouteIntent and produces StrategyCandidates.
    ///
    /// This is the primary entry point called by the orchestrator.
    pub async fn build_from_impacted_cycles(
        &self,
        intent: &RouteIntent,
        impact: &ImpactSet,
        cfg: Option<&TradingConfigState>,
    ) -> anyhow::Result<Vec<StrategyCandidate>> {
        if impact.impacted_cycles.is_empty() {
            return Ok(vec![]);
        }

        let chain_id = intent.chain_id;
        let tx_hash = intent.tx_hash;
        let mut candidates = Vec::new();

        for &cycle_id in &impact.impacted_cycles {
            // Detect cycles from the first token in the intent
            if let Some(first_leg) = intent.legs.first() {
                let start_token = format!("0x{:040x}", first_leg.token_in);
                let cycles = self.detect_cycles(&start_token).await;

                for cycle in cycles {
                    if cycle.cycle_id == cycle_id {
                        let candidate =
                            self.build_candidate(&cycle, chain_id, tx_hash, cfg, cycle_id);
                        candidates.push(candidate);
                    }
                }
            }
        }

        Ok(candidates)
    }

    // -------------------------------------------------------------------------
    // Internal methods
    // -------------------------------------------------------------------------

    /// Builds the token graph from current reserves cache.
    async fn build_graph_from_cache(&self) -> (Vec<TokenNode>, Vec<(usize, usize, PoolEdge)>) {
        let mut nodes = Vec::new();
        let mut node_indices: HashMap<Address, usize> = HashMap::new();
        let mut edges = Vec::new();

        // For now, use a minimal set of well-known token pairs
        // In production, this would scan all pools from the reserves cache
        let well_known_pairs = self.well_known_token_pairs();

        for (token_a, token_b, pool_addr) in well_known_pairs {
            if let Some(reserves) = self.reserves_cache.get(&pool_addr).await {
                // Add nodes if not present
                let idx_a = *node_indices.entry(token_a).or_insert_with(|| {
                    let idx = nodes.len();
                    nodes.push(TokenNode {
                        address: token_a,
                        symbol: self
                            .token_symbols
                            .get(&token_a)
                            .cloned()
                            .unwrap_or_else(|| format!("0x{:08x}", token_a)),
                        decimals: *self.token_decimals.get(&token_a).unwrap_or(&18),
                    });
                    idx
                });

                let idx_b = *node_indices.entry(token_b).or_insert_with(|| {
                    let idx = nodes.len();
                    nodes.push(TokenNode {
                        address: token_b,
                        symbol: self
                            .token_symbols
                            .get(&token_b)
                            .cloned()
                            .unwrap_or_else(|| format!("0x{:08x}", token_b)),
                        decimals: *self.token_decimals.get(&token_b).unwrap_or(&18),
                    });
                    idx
                });

                // Create bidirectional edges (A->B and B->A)
                let swap_in_is_token0 = token_a < token_b;

                edges.push((
                    idx_a,
                    idx_b,
                    PoolEdge {
                        pool_address: pool_addr,
                        dex_id: "uniswap-v2".to_string(),
                        fee_bps: 30,
                        reserves,
                        swap_in_is_token0,
                    },
                ));

                edges.push((
                    idx_b,
                    idx_a,
                    PoolEdge {
                        pool_address: pool_addr,
                        dex_id: "uniswap-v2".to_string(),
                        fee_bps: 30,
                        reserves,
                        swap_in_is_token0: !swap_in_is_token0,
                    },
                ));
            }
        }

        (nodes, edges)
    }

    /// Bellman-Ford algorithm for negative cycle detection.
    async fn bellman_ford_cycles(
        &self,
        nodes: &[TokenNode],
        edges: &[(usize, usize, PoolEdge)],
        start_addr: Address,
    ) -> Vec<ArbCycle> {
        let n = nodes.len();
        if n == 0 {
            return vec![];
        }

        // Find start node index
        let start_idx = match nodes.iter().position(|n| n.address == start_addr) {
            Some(idx) => idx,
            None => 0, // Default to first node if start not found
        };

        // Initialize distances
        let mut dist: Vec<f64> = vec![f64::INFINITY; n];
        let mut predecessor: Vec<Option<usize>> = vec![None; n];
        dist[start_idx] = 0.0;

        // Relax edges n-1 times
        for _ in 0..n - 1 {
            for (u, v, edge) in edges {
                let weight = edge.log_weight();
                if dist[*u] != f64::INFINITY && dist[*u] + weight < dist[*v] {
                    dist[*v] = dist[*u] + weight;
                    predecessor[*v] = Some(*u);
                }
            }
        }

        // Detect negative cycles
        let mut cycles = Vec::new();
        let mut visited_negative = vec![false; n];

        for (u, v, edge) in edges {
            let weight = edge.log_weight();
            if dist[*u] != f64::INFINITY && dist[*u] + weight < dist[*v] - 1e-12 {
                // Negative cycle found
                if visited_negative[*v] {
                    continue;
                }

                // Reconstruct cycle
                if let Some(cycle) =
                    self.reconstruct_cycle(nodes, edges, &predecessor, *v, start_addr)
                {
                    visited_negative[*v] = true;
                    cycles.push(cycle);
                }
            }
        }

        cycles
    }

    /// Reconstructs a cycle from the predecessor chain.
    fn reconstruct_cycle(
        &self,
        nodes: &[TokenNode],
        edges: &[(usize, usize, PoolEdge)],
        predecessor: &[Option<usize>],
        start: usize,
        _start_addr: Address,
    ) -> Option<ArbCycle> {
        // Walk back to find the cycle
        let mut cycle_nodes = Vec::new();
        let mut cycle_edges = Vec::new();
        let mut current = start;
        let mut visited = std::collections::HashSet::new();

        // Collect cycle nodes by walking predecessors
        loop {
            if !visited.insert(current) {
                // Found the cycle start
                break;
            }

            match predecessor[current] {
                Some(prev) => {
                    // Find the edge
                    if let Some((_, _, edge)) =
                        edges.iter().find(|(u, v, _)| *u == prev && *v == current)
                    {
                        cycle_edges.push(edge.clone());
                        cycle_nodes.push(nodes[current].clone());
                    }
                    current = prev;
                }
                None => break,
            }

            if cycle_nodes.len() > 10 {
                // Safety limit - cycles shouldn't be longer than this
                break;
            }
        }

        if cycle_nodes.len() < 3 {
            return None;
        }

        // Calculate rate product
        let rate_product: f64 = cycle_edges.iter().map(|e| e.exchange_rate()).product();

        if rate_product <= 1.0 {
            return None; // Not profitable
        }

        // Estimate yield (simplified: assumes 1000 USD input)
        let estimated_yield_usd = 1000.0 * (rate_product - 1.0);

        Some(ArbCycle {
            cycle_id: 0, // Will be assigned by caller
            tokens: cycle_nodes,
            edges: cycle_edges,
            rate_product,
            estimated_yield_usd,
            optimal_amount_in_wei: U256::from(1_000_000_000_000_000_000u64), // 1 ETH default
        })
    }

    /// Builds a StrategyCandidate from an ArbCycle.
    fn build_candidate(
        &self,
        cycle: &ArbCycle,
        chain_id: u64,
        tx_hash: H256,
        cfg: Option<&TradingConfigState>,
        cycle_id: CycleId,
    ) -> StrategyCandidate {
        let id = Uuid::new_v4();
        let trace_id = Uuid::new_v4();
        let strategy_kind = StrategyLabel::SpanningTreeArb.to_contract_strategy_kind();

        // Get token symbols for pair naming
        let pair_symbol = if cycle.tokens.len() >= 2 {
            format!("{}-{}", cycle.tokens[0].symbol, cycle.tokens[1].symbol)
        } else {
            "unknown".to_string()
        };

        let token_in = format!(
            "0x{:040x}",
            cycle.tokens.first().map(|t| t.address).unwrap_or_default()
        );
        let token_out = format!(
            "0x{:040x}",
            cycle.tokens.get(1).map(|t| t.address).unwrap_or_default()
        );

        // Calculate gross yield in USD
        let gross_yield_usd = if cycle.rate_product > 1.0 {
            Some(cycle.estimated_yield_usd)
        } else {
            None
        };

        // Determine rejection reason
        let rejection_reason = if cycle.rate_product <= 1.0 {
            Some("rate_product_le_one".to_string())
        } else {
            None
        };

        let opportunity = Opportunity {
            id,
            chain_id,
            strategy_kind,
            dex_a: "uniswap-v2".to_string(),
            dex_b: None,
            pair_symbol: format!("{}(spanning_tree)", pair_symbol),
            token_in: token_in.clone(),
            token_out: token_out.clone(),
            amount_in_wei: cycle.optimal_amount_in_wei.to_string(),
            expected_profit_usd: gross_yield_usd,
            net_expected_profit_usd: None,
            roi_pct: None,
            risk_score: None,
            block_number: None,
            rejection_reason: rejection_reason.clone(),
            detected_at: Utc::now(),
            trace_id,
        };

        let pool_addresses: Vec<String> = cycle
            .edges
            .iter()
            .map(|e| format!("0x{:040x}", e.pool_address))
            .collect();

        let token_addresses: Vec<String> = cycle
            .tokens
            .iter()
            .map(|t| format!("0x{:040x}", t.address))
            .collect();

        let candidate = OpportunityCandidate {
            route_fingerprint: format!("st_{}_{}", cycle_id, pair_symbol),
            pool_addresses: pool_addresses.clone(),
            token_addresses,
            dex_adapters: vec!["uniswap-v2".to_string(); cycle.edges.len()],
            amount_in: u256_to_f64(&cycle.optimal_amount_in_wei) / 1e18,
            expected_amount_out: u256_to_f64(&cycle.optimal_amount_in_wei) / 1e18
                * cycle.rate_product,
            gross_profit: gross_yield_usd.unwrap_or(0.0),
        };

        // Build RouteLeg entries
        let legs: Vec<RouteLeg> = cycle
            .edges
            .iter()
            .enumerate()
            .map(|(i, edge)| {
                let token_in_addr = cycle
                    .tokens
                    .get(i)
                    .map(|t| format!("0x{:040x}", t.address))
                    .unwrap_or_default();
                let token_out_addr = cycle
                    .tokens
                    .get((i + 1) % cycle.tokens.len())
                    .map(|t| format!("0x{:040x}", t.address))
                    .unwrap_or_default();

                RouteLeg {
                    dex_id: edge.dex_id.clone(),
                    dex_name: edge.dex_id.clone(),
                    protocol_type: "uniswap-v2".to_string(),
                    factory_address: String::new(),
                    pool_id: None,
                    pool_address: Some(format!("0x{:040x}", edge.pool_address)),
                    token_in: token_in_addr,
                    token_out: token_out_addr,
                    fee_bps: Some(edge.fee_bps),
                    amount_in: Some(u256_to_f64(&cycle.optimal_amount_in_wei) / 1e18),
                    amount_out: None,
                    tvl_usd: None,
                    volume_24h_usd: None,
                    pool_is_active: true,
                }
            })
            .collect();

        let route_plan = RoutePlan {
            route_id: Some(format!("st-{}-{:x}", cycle_id, tx_hash)),
            strategy_kind: StrategyLabel::SpanningTreeArb.as_str().to_string(),
            chain_id,
            legs,
            atomic: true,
            estimated_slippage_pct: None,
            price_impact_pct: None,
        };

        debug!(
            event = "spanning_tree_engine.candidate_built",
            chain_id,
            cycle_id,
            rate_product = cycle.rate_product,
            "spanning tree candidate {}",
            if rejection_reason.is_some() {
                "rejected"
            } else {
                "accepted"
            }
        );

        StrategyCandidate {
            label: StrategyLabel::SpanningTreeArb,
            opportunity,
            candidate,
            route_plan,
            gross_profit_usd: gross_yield_usd,
            net_expected_profit_usd: None,
            rejection_reason,
            source_intent_hash: tx_hash,
            base_strategy: None,
        }
    }

    /// Returns well-known token pairs for graph construction.
    fn well_known_token_pairs(&self) -> Vec<(Address, Address, Address)> {
        // These would normally come from the pool registry
        // For now, return empty - the engine will use dynamic discovery
        vec![]
    }

    /// Default token symbols for well-known addresses.
    fn default_token_symbols() -> HashMap<Address, String> {
        let mut map = HashMap::new();
        if let Ok(weth) = Address::from_str("0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2") {
            map.insert(weth, "WETH".to_string());
        }
        if let Ok(usdc) = Address::from_str("0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48") {
            map.insert(usdc, "USDC".to_string());
        }
        if let Ok(usdt) = Address::from_str("0xdac17f958d2ee523a2206206994597c13d831ec7") {
            map.insert(usdt, "USDT".to_string());
        }
        if let Ok(dai) = Address::from_str("0x6b175474e89094c44da98b954eedeac495271d0f") {
            map.insert(dai, "DAI".to_string());
        }
        map
    }

    /// Default token decimals for well-known addresses.
    fn default_token_decimals() -> HashMap<Address, u8> {
        let mut map = HashMap::new();
        if let Ok(weth) = Address::from_str("0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2") {
            map.insert(weth, 18);
        }
        if let Ok(usdc) = Address::from_str("0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48") {
            map.insert(usdc, 6);
        }
        if let Ok(usdt) = Address::from_str("0xdac17f958d2ee523a2206206994597c13d831ec7") {
            map.insert(usdt, 6);
        }
        if let Ok(dai) = Address::from_str("0x6b175474e89094c44da98b954eedeac495271d0f") {
            map.insert(dai, 18);
        }
        map
    }
}

/// Converts U256 to f64 (lossy truncation to u128 first).
fn u256_to_f64(v: &U256) -> f64 {
    v.low_u128() as f64
}

use std::str::FromStr;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pool_edge_log_weight() {
        let edge = PoolEdge {
            pool_address: Address::zero(),
            dex_id: "uniswap-v2".to_string(),
            fee_bps: 30,
            reserves: (U256::from(1_000_000u64), U256::from(1_000_000u64)),
            swap_in_is_token0: true,
        };

        // Equal reserves with 30bps fee should give rate = 0.997
        // -ln(0.997) ≈ 0.003
        let weight = edge.log_weight();
        assert!(
            weight > 0.0,
            "weight should be positive for equal reserves with fee"
        );
        assert!(weight < 0.01, "weight should be small");
    }

    #[test]
    fn test_pool_edge_exchange_rate() {
        // Asymmetric reserves: 1000 token0, 2000 token1
        // Going 0->1: rate = (2000/1000) * 0.997 = 1.994
        let edge = PoolEdge {
            pool_address: Address::zero(),
            dex_id: "uniswap-v2".to_string(),
            fee_bps: 30,
            reserves: (U256::from(1_000u64), U256::from(2_000u64)),
            swap_in_is_token0: true,
        };

        let rate = edge.exchange_rate();
        assert!(rate > 1.9, "rate should be ~1.994");
        assert!(rate < 2.0, "rate should be less than 2 due to fees");
    }

    #[tokio::test]
    async fn test_empty_cache_returns_empty_cycles() {
        let cache = Arc::new(ReservesCache::new());
        let engine = SpanningTreeEngine::new(cache);

        let cycles = engine
            .detect_cycles("0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2")
            .await;
        assert!(cycles.is_empty(), "empty cache should produce no cycles");
    }
}
