//! CrossChainBridgeEngine — inter-venue / inter-chain Holonomic Loop Resolution detector.
//!
//! Detects topological yield opportunities arising from price divergences
//! between two REAL price venues for the same token, accounting for transfer
//! fees and latency.
//!
//! ## Route model (STEP 2.5d, 2026-09-25)
//!
//! A `BridgeConfig` describes ONE price-divergence route between two venues:
//!
//! - **Venue-pair route (default)** — chain 1 → chain 1: the same token is
//!   priced on two independent, live sources that legitimately diverge:
//!   `binance_spot` (fused Binance WS bookTicker + Chainlink anchors, served
//!   by the process `PriceBus`, see `crate::price_bus_global`) vs
//!   `onchain_aggregated` (the `arbx:token_prices:1` Redis snapshot populated
//!   every 30s by `price_worker` from Alchemy Token Prices → Coingecko
//!   fallback). This is the classic CEX/DEX spread with same-chain
//!   settlement. It removes the multi-chain oracle dependency that made the
//!   old defaults (Arbitrum 42161 / Solana 1399811149) dead routes: there is
//!   NO price producer for chain 42161 today, so every bridge silently
//!   `continue`d on `no_dest_oracle`.
//! - **True bridge route (future)** — chain A → chain B with a real bridge
//!   contract, once a second chain has a live price producer.
//!
//! ## Mathematical Model
//!
//! For a token T on venues A and B:
//! - Price_A = price of T on venue A (from oracle)
//! - Price_B = price of T on venue B (from oracle)
//! - Spread = |Price_A - Price_B| / min(Price_A, Price_B)
//!
//! Opportunity exists when:
//!   Spread > (route_fee_bps + min_profit_bps) / 10000
//!
//! Where route fees include:
//! - Venue execution fees (DEX swap fee tiers, CEX taker fees)
//! - Destination gas cost (estimated)
//! - Temporal Liquidity Superposition (TLS) fee if applicable
//!
//! ## R8 Invariants
//!
//! - No oracle price on either venue → skip with debug reason, counted in the
//!   per-scan summary (never a synthetic price)
//! - Spread < threshold → skip, reason "spread_below_threshold"
//! - Negative net yield → skip, reason "negative_net_yield"
//! - Every scan emits ONE `cross_chain_engine.scan_complete` info log with a
//!   reasons histogram (R9: per-item detail at debug, summary at info)
//! - `bridge_address: None` on venue-pair routes — no fake bridge contract
//!   addresses are ever emitted (RULE 00: zero sentinel addresses)

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
use std::str::FromStr;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tracing::{debug, info};
use uuid::Uuid;

/// Oracle venue key: fused Binance WS spot price (process `PriceBus`).
pub const VENUE_BINANCE_SPOT: &str = "binance_spot";

/// Oracle venue key: on-chain aggregated snapshot (`arbx:token_prices:<chain>`,
/// populated by `price_worker`).
pub const VENUE_ONCHAIN_AGGREGATED: &str = "onchain_aggregated";

/// Oracle map keyed by VENUE id (not chain id): a venue-pair route prices the
/// SAME chain-1 token from two DIFFERENT live sources that can legitimately
/// diverge. Keying by chain id would collapse both legs onto one oracle and
/// force spread = 0 forever.
pub type PriceOracleMap = HashMap<String, Arc<dyn PriceOracle>>;

/// Bridge / venue-pair route configuration.
#[derive(Debug, Clone)]
pub struct BridgeConfig {
    /// Route identifier (e.g. "cex_dex_weth" for a venue-pair route,
    /// "layerzero" for a true bridge route).
    pub protocol: String,
    /// Source chain ID (reporting; venue-pair routes are 1 → 1).
    pub source_chain: u64,
    /// Destination chain ID (reporting; venue-pair routes are 1 → 1).
    pub dest_chain: u64,
    /// Oracle venue key for the source leg (see `PriceOracleMap`).
    pub source_venue: String,
    /// Oracle venue key for the destination leg.
    pub dest_venue: String,
    /// Bridge contract address on the source chain. `None` for venue-pair
    /// routes — there is no bridge contract and NO sentinel is fabricated
    /// (RULE 00).
    pub bridge_address: Option<Address>,
    /// Fee in basis points charged along the route (swap tiers / taker fees).
    pub protocol_fee_bps: u32,
    /// Estimated gas cost on destination chain (in USD).
    pub dest_gas_cost_usd: f64,
    /// Average latency in seconds for route completion.
    pub avg_latency_secs: u64,
    /// Whether the route settles atomically. Venue-pair routes on one chain
    /// are bundleable (`true`); true bridge transfers are not (`false`).
    pub atomic: bool,
    /// Tokens the route supports.
    pub supported_tokens: Vec<Address>,
}

/// Price oracle trait for per-venue price discovery.
#[async_trait::async_trait]
pub trait PriceOracle: Send + Sync {
    /// Returns the current price of a token in USD.
    /// Returns None if the price is unavailable (R8 fail-honest).
    async fn get_price_usd(&self, token: Address) -> Option<f64>;

    /// Returns the chain ID this oracle serves.
    fn chain_id(&self) -> u64;
}

/// `PriceOracle` adapter over the fused price bus (venue `binance_spot`):
/// Binance WS bookTicker verified against Chainlink anchors. The bus handle
/// is injected by the bin-side wiring (`crate::price_bus_global::get()` is a
/// main.rs-only module, not visible from the lib where engines compile).
/// Fail-honest: `None` when no bus was injected or the pair has no live
/// quote/anchor — never a fabricated price.
pub struct PriceBusSpotOracle {
    bus: Option<Arc<shared_rs::price_bus::PriceBus>>,
    chain_id: u64,
}

impl PriceBusSpotOracle {
    pub fn new(bus: Option<Arc<shared_rs::price_bus::PriceBus>>, chain_id: u64) -> Self {
        Self { bus, chain_id }
    }
}

#[async_trait::async_trait]
impl PriceOracle for PriceBusSpotOracle {
    async fn get_price_usd(&self, token: Address) -> Option<f64> {
        let bus = self.bus.as_ref()?;
        let symbol = CrossChainBridgeEngine::token_symbol(&token);
        // `BusPriceOracle` normalizes WETH→ETH canonical pairs internally
        // (price_bus.rs tests pin that behavior). `as _` binds the shared-rs
        // sync trait's METHODS without importing its name (this file defines
        // its own async `PriceOracle` trait).
        use shared_rs::price_oracle::PriceOracle as _;
        shared_rs::price_bus::BusPriceOracle(Arc::clone(bus)).price_usd(&symbol)
    }

    fn chain_id(&self) -> u64 {
        self.chain_id
    }
}

/// Snapshot refresh TTL — bounds Redis round-trips to one HGETALL per venue
/// per few seconds while keeping staleness well inside `price_worker`'s 30s
/// refresh window (the detector is NOT the sync hot path).
const SNAPSHOT_TTL: Duration = Duration::from_secs(5);

/// `PriceOracle` adapter over the `arbx:token_prices:<chain_id>` Redis hash
/// (venue `onchain_aggregated`), populated every 30s by `price_worker`
/// (Alchemy Token Prices → Coingecko fallback). Fail-honest: `None` on Redis
/// miss/error → the engine skips that leg with an explicit reason.
pub struct RedisAggregatedOracle {
    redis: redis::aio::ConnectionManager,
    chain_id: u64,
    snapshot: tokio::sync::Mutex<Option<(Instant, HashMap<String, f64>)>>,
}

impl RedisAggregatedOracle {
    pub fn new(redis: redis::aio::ConnectionManager, chain_id: u64) -> Self {
        Self {
            redis,
            chain_id,
            snapshot: tokio::sync::Mutex::new(None),
        }
    }

    /// Returns the validated, uppercased symbol→price snapshot, refreshing
    /// from Redis at most once per `SNAPSHOT_TTL`.
    async fn live_snapshot(&self) -> HashMap<String, f64> {
        let mut guard = self.snapshot.lock().await;
        if let Some((at, map)) = guard.as_ref() {
            if at.elapsed() < SNAPSHOT_TTL {
                return map.clone();
            }
        }
        let fresh = shared_rs::price_oracle::RedisCachedPriceOracle::snapshot_from_redis(
            &mut self.redis.clone(),
            self.chain_id,
        )
        .await;
        // `snapshot()` is already validated (finite, > 0, uppercased keys).
        let map = fresh.snapshot().clone();
        *guard = Some((Instant::now(), map.clone()));
        map
    }
}

#[async_trait::async_trait]
impl PriceOracle for RedisAggregatedOracle {
    async fn get_price_usd(&self, token: Address) -> Option<f64> {
        let symbol = CrossChainBridgeEngine::token_symbol(&token).to_ascii_uppercase();
        let map = self.live_snapshot().await;
        map.get(&symbol)
            .copied()
            .filter(|p| p.is_finite() && *p > 0.0)
    }

    fn chain_id(&self) -> u64 {
        self.chain_id
    }
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
    /// Price on source venue
    pub source_price: f64,
    /// Price on destination venue
    pub dest_price: f64,
    /// Spread as ratio (e.g., 0.005 = 50 bps)
    pub spread_bps: u32,
    /// Route to use for transfer
    pub bridge: BridgeConfig,
    /// Total cost including all fees (in USD)
    pub total_cost_usd: f64,
    /// Estimated net topological yield (in USD)
    pub net_yield_usd: f64,
    /// Optimal amount to transfer (in token wei)
    pub optimal_amount_wei: U256,
}

/// Engine for detecting cross-venue / cross-chain Holonomic Loop Resolutions.
pub struct CrossChainBridgeEngine {
    /// Configured routes for price-divergence transfers
    bridges: Vec<BridgeConfig>,
    /// Price oracles keyed by venue ID
    price_oracles: PriceOracleMap,
    /// Minimum spread threshold in basis points (default: 50)
    min_spread_bps: u32,
    /// Capital allocation per opportunity (USD)
    capital_usd: f64,
}

impl CrossChainBridgeEngine {
    /// Creates a new CrossChainBridgeEngine with the given routes and oracles.
    pub fn new(bridges: Vec<BridgeConfig>, price_oracles: PriceOracleMap) -> Self {
        Self {
            bridges,
            price_oracles,
            min_spread_bps: 50, // 50 bps = 0.5% minimum spread
            capital_usd: 10_000.0,
        }
    }

    /// Creates an engine with default venue-pair routes for mainnet.
    pub fn with_default_bridges(price_oracles: PriceOracleMap) -> Self {
        let bridges = Self::default_bridge_configs();
        Self::new(bridges, price_oracles)
    }

    /// Builds the default venue-keyed oracle map from LIVE handles:
    /// `binance_spot` (fused price bus — the bin wiring passes
    /// `crate::price_bus_global::get()`) + `onchain_aggregated` (Redis
    /// snapshot). This is the one-call wiring input for
    /// `with_default_bridges`.
    pub fn default_oracles(
        chain_id: u64,
        bus: Option<Arc<shared_rs::price_bus::PriceBus>>,
        redis: redis::aio::ConnectionManager,
    ) -> PriceOracleMap {
        let mut oracles: PriceOracleMap = HashMap::new();
        oracles.insert(
            VENUE_BINANCE_SPOT.to_string(),
            Arc::new(PriceBusSpotOracle::new(bus, chain_id)),
        );
        oracles.insert(
            VENUE_ONCHAIN_AGGREGATED.to_string(),
            Arc::new(RedisAggregatedOracle::new(redis, chain_id)),
        );
        oracles
    }

    /// Detects cross-venue opportunities across all configured routes.
    pub async fn detect_cross_chain_arbs(&self) -> Vec<CrossChainOpportunity> {
        let mut opportunities = Vec::new();
        let mut id_counter = 0u64;
        let mut skip_reasons: HashMap<&str, u32> = HashMap::new();

        for bridge in &self.bridges {
            // Get oracles for both legs (keyed by VENUE, not chain)
            let source_oracle = match self.price_oracles.get(&bridge.source_venue) {
                Some(o) => o,
                None => {
                    *skip_reasons.entry("no_source_oracle").or_insert(0) += 1;
                    debug!(
                        event = "cross_chain_engine.no_source_oracle",
                        venue = %bridge.source_venue,
                        route = %bridge.protocol,
                        "skipping route: no oracle for source venue"
                    );
                    continue;
                }
            };

            let dest_oracle = match self.price_oracles.get(&bridge.dest_venue) {
                Some(o) => o,
                None => {
                    *skip_reasons.entry("no_dest_oracle").or_insert(0) += 1;
                    debug!(
                        event = "cross_chain_engine.no_dest_oracle",
                        venue = %bridge.dest_venue,
                        route = %bridge.protocol,
                        "skipping route: no oracle for dest venue"
                    );
                    continue;
                }
            };

            // Check each supported token
            for token in &bridge.supported_tokens {
                let token_symbol = Self::token_symbol(token);

                // Fetch prices from both venues concurrently
                let (source_price, dest_price) = tokio::join!(
                    source_oracle.get_price_usd(*token),
                    dest_oracle.get_price_usd(*token)
                );

                let (source_p, dest_p) = match (source_price, dest_price) {
                    (Some(s), Some(d)) => (s, d),
                    _ => {
                        *skip_reasons.entry("missing_price").or_insert(0) += 1;
                        debug!(
                            event = "cross_chain_engine.missing_price",
                            token = %token,
                            token_symbol = %token_symbol,
                            has_source = source_price.is_some(),
                            has_dest = dest_price.is_some(),
                            "skipping: price unavailable on one or both venues"
                        );
                        continue;
                    }
                };

                // Calculate spread
                let spread_ratio = (dest_p - source_p).abs() / source_p.min(dest_p);
                let spread_bps = (spread_ratio * 10_000.0) as u32;

                // Check against threshold
                if spread_bps < self.min_spread_bps {
                    *skip_reasons.entry("spread_below_threshold").or_insert(0) += 1;
                    debug!(
                        event = "cross_chain_engine.spread_below_threshold",
                        token = %token,
                        token_symbol = %token_symbol,
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
                    *skip_reasons.entry("negative_net_yield").or_insert(0) += 1;
                    debug!(
                        event = "cross_chain_engine.negative_net_yield",
                        token = %token,
                        token_symbol = %token_symbol,
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
                    source_venue = %bridge.source_venue,
                    dest_venue = %bridge.dest_venue,
                    source_price,
                    dest_price,
                    spread_bps,
                    net_yield_usd,
                    "cross-venue opportunity detected"
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

        // R9: ONE summary per scan — explicit reasons even when zero candidates
        // (this is the "razón explícita" the live criterion asks for).
        info!(
            event = "cross_chain_engine.scan_complete",
            routes_scanned = self.bridges.len(),
            candidates_found = opportunities.len(),
            reasons = ?skip_reasons,
            "cross-venue scan complete (R8: every skip counted, none silent)"
        );

        opportunities
    }

    /// Entry point for orchestrator integration.
    /// Evaluates cross-venue opportunities in response to a RouteIntent.
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
        let token_out = token_in.clone(); // Same token, different venue

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
            risk_score: Some(0.3), // Cross-venue carries venue/bridge risk
            block_number: None,
            rejection_reason: None,
            cartridge_id: None,
            // WO-CARDS-COMPLETE-01 (2026-09-17): detector identity at construction.
            detector_id: Some("cross_chain_bridge_engine".to_string()),
            pipeline_latency_ms: None,
            // ALWAYS-COMPUTE mandate (operator 2026-09-27): the economics
            // object is built downstream in the spine (post-sizing), not at
            // the detector — same posture as every other engine initializer.
            economics: None,
            detected_at: Utc::now(),
            trace_id,
        };

        // Venue-pair routes have NO bridge contract — an empty pool list is the
        // honest representation (RULE 00: never a sentinel address).
        let pool_addresses = opp
            .bridge
            .bridge_address
            .map(|a| vec![format!("0x{:040x}", a)])
            .unwrap_or_default();
        let token_addresses = vec![token_in.clone()];

        let candidate = OpportunityCandidate {
            route_fingerprint: format!(
                "cc_{}_{}_{}_{}",
                opp.source_chain, opp.dest_chain, opp.token_symbol, opp.id
            ),
            pool_addresses,
            token_addresses,
            dex_adapters: vec![
                opp.bridge.source_venue.clone(),
                opp.bridge.dest_venue.clone(),
            ],
            amount_in: u256_to_f64(&opp.optimal_amount_wei) / 1e18,
            expected_amount_out: u256_to_f64(&opp.optimal_amount_wei) / 1e18, // Same token amount
            gross_profit: opp.net_yield_usd,
        };

        // Build a single-leg route plan representing the venue transfer
        let legs = vec![RouteLeg {
            dex_id: opp.bridge.protocol.clone(),
            dex_name: opp.bridge.protocol.clone(),
            protocol_type: if opp.bridge.atomic {
                "venue_pair".to_string()
            } else {
                "bridge".to_string()
            },
            factory_address: String::new(),
            pool_id: None,
            pool_address: opp.bridge.bridge_address.map(|a| format!("0x{:040x}", a)),
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
            // Venue-pair routes settle same-chain (bundleable in one tx);
            // true bridge transfers are not atomic.
            atomic: opp.bridge.atomic,
            estimated_slippage_pct: None,
            price_impact_pct: Some(opp.spread_bps as f64 / 100.0),
        };

        debug!(
            event = "cross_chain_engine.candidate_built",
            chain_id,
            opportunity_id = opp.id,
            source_chain = opp.source_chain,
            dest_chain = opp.dest_chain,
            source_venue = %opp.bridge.source_venue,
            dest_venue = %opp.bridge.dest_venue,
            spread_bps = opp.spread_bps,
            "cross-venue candidate accepted"
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

    /// Returns default route configurations using ONLY venues that have a live
    /// price producer today (STEP 2.5d).
    ///
    /// The old defaults pointed at Arbitrum (42161) and Solana
    /// (1399811149) — chains with NO price producer in this process — so
    /// every route died on `no_dest_oracle` and the engine emitted nothing.
    /// The new defaults are chain 1 → 1 venue-pair routes (CEX/DEX spread)
    /// backed by the two real, live chain-1 price sources.
    fn default_bridge_configs() -> Vec<BridgeConfig> {
        vec![
            // CEX/DEX venue pair: Binance spot vs on-chain aggregated — WETH.
            BridgeConfig {
                protocol: "cex_dex_weth".to_string(),
                source_chain: 1, // Ethereum mainnet
                dest_chain: 1,   // same-chain settlement (venue pair)
                source_venue: VENUE_BINANCE_SPOT.to_string(),
                dest_venue: VENUE_ONCHAIN_AGGREGATED.to_string(),
                bridge_address: None, // venue-pair route: no bridge contract
                // Two Uniswap V3 0.05% (fee-tier 500) hops for WETH/USDC.
                protocol_fee_bps: 10,
                // ≈350k gas × ~5 gwei × ETH≈$3.4k (two swaps + approve).
                dest_gas_cost_usd: 6.0,
                // ~2 mainnet blocks from detection to settlement.
                avg_latency_secs: 24,
                atomic: true, // both legs settle on chain 1
                supported_tokens: vec![canonical_mainnet_address(WETH_MAINNET_LC)],
            },
            // CEX/DEX venue pair: Binance spot vs on-chain aggregated — USDC.
            BridgeConfig {
                protocol: "cex_dex_usdc".to_string(),
                source_chain: 1,
                dest_chain: 1,
                source_venue: VENUE_BINANCE_SPOT.to_string(),
                dest_venue: VENUE_ONCHAIN_AGGREGATED.to_string(),
                bridge_address: None,
                // Stable-tier hops (V3 0.01%/0.05% fee tiers) + buffer.
                protocol_fee_bps: 5,
                dest_gas_cost_usd: 6.0,
                avg_latency_secs: 24,
                atomic: true,
                supported_tokens: vec![canonical_mainnet_address(USDC_MAINNET_LC)],
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
    pub fn token_symbol(token: &Address) -> String {
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

/// Parses a canonical mainnet constant from `shared_rs::chains`. Fail-fast on
/// a malformed constant (they are round-trip tested in shared-rs) — NEVER a
/// silent 0x0 sentinel (RULE 00).
fn canonical_mainnet_address(lc_const: &'static str) -> Address {
    Address::from_str(lc_const)
        .unwrap_or_else(|e| panic!("canonical mainnet constant {lc_const} must parse: {e}"))
}

/// Converts U256 to f64.
fn u256_to_f64(v: &U256) -> f64 {
    v.low_u128() as f64
}

#[cfg(test)]
mod tests {
    use super::*;
    use ethers::types::Address;

    /// Mock price oracle for testing (single venue, fixed price).
    struct MockOracle {
        price: f64,
    }

    #[async_trait::async_trait]
    impl PriceOracle for MockOracle {
        async fn get_price_usd(&self, _token: Address) -> Option<f64> {
            Some(self.price)
        }

        fn chain_id(&self) -> u64 {
            1
        }
    }

    /// Builds a venue map with fixed mock prices for both default venues.
    fn mock_venue_map(binance_price: f64, aggregated_price: f64) -> PriceOracleMap {
        let mut oracles: PriceOracleMap = HashMap::new();
        oracles.insert(
            VENUE_BINANCE_SPOT.to_string(),
            Arc::new(MockOracle {
                price: binance_price,
            }),
        );
        oracles.insert(
            VENUE_ONCHAIN_AGGREGATED.to_string(),
            Arc::new(MockOracle {
                price: aggregated_price,
            }),
        );
        oracles
    }

    #[tokio::test]
    async fn test_detects_opportunity_with_large_spread() {
        // USDC = $1.00 on Binance spot, $1.01 on-chain aggregated (1% spread)
        let engine = CrossChainBridgeEngine::with_default_bridges(mock_venue_map(1.00, 1.01));
        let opportunities = engine.detect_cross_chain_arbs().await;

        // Should find opportunities due to 1% spread > 50 bps threshold
        assert!(
            !opportunities.is_empty(),
            "should detect opportunity with 1% venue spread"
        );

        let opp = &opportunities[0];
        assert_eq!(opp.source_chain, 1);
        assert_eq!(opp.dest_chain, 1);
        assert!(opp.spread_bps >= 50, "spread should be at least 50 bps");
        assert!(opp.net_yield_usd > 0.0, "net yield must be positive");
    }

    #[tokio::test]
    async fn test_skips_small_spread() {
        // Only 0.1% spread (10 bps) — below the 50 bps threshold
        let engine = CrossChainBridgeEngine::with_default_bridges(mock_venue_map(1.00, 1.001));
        let opportunities = engine.detect_cross_chain_arbs().await;

        assert!(
            opportunities.is_empty(),
            "should skip opportunities with < 50 bps spread"
        );
    }

    #[tokio::test]
    async fn test_missing_venue_skips_without_panic() {
        // Dest venue absent → every default route must skip fail-honest
        // (R8), never fabricate a price and never panic.
        let mut oracles: PriceOracleMap = HashMap::new();
        oracles.insert(
            VENUE_BINANCE_SPOT.to_string(),
            Arc::new(MockOracle { price: 1.00 }),
        );
        let engine = CrossChainBridgeEngine::with_default_bridges(oracles);
        let opportunities = engine.detect_cross_chain_arbs().await;
        assert!(opportunities.is_empty());
    }

    #[test]
    fn test_default_bridges_use_available_chain1_venues_only() {
        // STEP 2.5d regression guard: the old defaults pointed at chains
        // 42161 / 1399811149 with NO price producer — dead silent-continue
        // routes. New defaults must stay on chain 1 with two DIFFERENT real
        // venues per route.
        for b in CrossChainBridgeEngine::default_bridge_configs() {
            assert_eq!(
                b.source_chain, 1,
                "route {} must source on chain 1",
                b.protocol
            );
            assert_eq!(b.dest_chain, 1, "route {} must dest on chain 1", b.protocol);
            assert!(
                !b.source_venue.is_empty() && !b.dest_venue.is_empty(),
                "route {} must name both venues",
                b.protocol
            );
            assert_ne!(
                b.source_venue, b.dest_venue,
                "route {} needs two DIFFERENT real price venues",
                b.protocol
            );
            assert!(
                b.bridge_address.is_none(),
                "venue-pair route {} must not fabricate a bridge address",
                b.protocol
            );
            assert!(b.atomic, "same-chain venue-pair routes are bundleable");
            assert!(
                !b.supported_tokens.is_empty(),
                "route {} must support at least one token",
                b.protocol
            );
        }
    }

    #[tokio::test]
    async fn test_price_bus_oracle_without_bus_fails_honest() {
        // No bus injected → None (R8), never a fabricated price.
        let oracle = PriceBusSpotOracle::new(None, 1);
        let weth = Address::from_str("0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2").unwrap();
        assert!(oracle.get_price_usd(weth).await.is_none());
        assert_eq!(oracle.chain_id(), 1);
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
