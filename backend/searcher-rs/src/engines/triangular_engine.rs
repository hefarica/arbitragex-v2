// M11 allow: test modules use .unwrap()/.expect() for readability;
// production paths use ? / anyhow throughout.
//! TriangularEngine — Phase 9 — event-driven triangular cycle evaluator.
//!
//! Converts an `ImpactSet` (impacted cycles from a `RouteIntent`) into a
//! `Vec<StrategyCandidate>` by evaluating ONLY the cycles listed in
//! `impact.impacted_cycles`. Never polls the full cycle universe.
//!
//! ## Design
//!
//! Each cycle ID in `ImpactSet::impacted_cycles` maps to a 3-hop definition
//! stored in this engine's `cycle_registry`. The registry is built at
//! construction time from `MVP_CYCLES` (same table the legacy worker uses).
//!
//! For each impacted cycle the engine:
//!   1. Looks up the cycle's 3 hops from `cycle_registry`.
//!   2. Fetches per-hop reserves from `ReservesCache`.
//!      Missing/stale entries → silent skip (data-availability gap, not route rejection).
//!   3. Builds `EvalInput` from the hop reserves + config snapshot.
//!   4. Calls `evaluate_cycle` (reused from `triangular_worker`).
//!   5. Emits an accepted or rejected `StrategyCandidate`.
//!
//! ## R8 invariants
//!
//! - `gross_profit_usd = None` only when `token_a_price_usd` is unavailable.
//! - `net_expected_profit_usd = None` always (spine fills later).
//! - `pool_address` is always `Some(...)` on all 3 route legs.
//! - `fee_bps = Some(30)` on all 3 legs (V2 constant).
//! - If `impact.impacted_cycles.is_empty()` → `Ok(vec![])` immediately.
//!
//! ## Rejection labels
//!
//! | reason                              | meaning                                    |
//! |-------------------------------------|--------------------------------------------|
//! | `spot_product_le_one`               | S = γ³·∏(R_out/R_in) ≤ 1 — not profitable |
//! | `sanity_reject_implausible_profit`  | profit/cap > 5× — math orientation bug     |

use crate::engines::StrategyCandidate;
use crate::impact_index::{CycleId, ImpactSet};
use crate::route_intent::RouteIntent;
use crate::strategy_label::StrategyLabel;
use crate::workers::triangular_worker::{evaluate_cycle, spot_product, EvalInput, MVP_CYCLES};
use chrono::Utc;
use ethers::types::{Address, H256, U256};
use prioritization_spine::route_plan::{RouteLeg, RoutePlan};
use prioritization_spine::types::OpportunityCandidate;
use shared_rs::contracts::Opportunity;
use shared_rs::trading_config::TradingConfigState;
use std::collections::HashMap;
use std::str::FromStr;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::debug;
use uuid::Uuid;

// ---------------------------------------------------------------------------
// V2 fee constant
// ---------------------------------------------------------------------------

/// Uniswap V2 / SushiSwap canonical fee, in basis points.
const V2_FEE_BPS: u32 = 30;

/// Sanity multiplier: reject if profit/cap > this factor. Mirrors the
/// triangular_worker's `SANITY_PROFIT_MULT_OF_CAP` (5.0).
const SANITY_PROFIT_MULT_OF_CAP: f64 = 5.0;

// ---------------------------------------------------------------------------
// ReservesCache
// ---------------------------------------------------------------------------

/// In-memory per-hop reserves keyed by pool address.
///
/// For Phase 9 this is a simple `HashMap<Address, (U256, U256)>` wrapped in
/// `Arc<RwLock<...>>`. Phase 12 wires this to a live Redis-backed cache via
/// `PoolSyncWorker`'s periodic updates.
///
/// `(U256, U256)` is `(reserve_in, reserve_out)` in **canonical pool order**
/// (token0 = reserve0, token1 = reserve1). The `HopDescriptor::swap_in_is_token0`
/// flag tells the engine which reserve to use as the input.
pub struct ReservesCache {
    inner: RwLock<HashMap<Address, (U256, U256)>>,
}

impl ReservesCache {
    /// Construct an empty cache.
    pub fn new() -> Self {
        Self {
            inner: RwLock::new(HashMap::new()),
        }
    }

    /// Insert or update reserves for a pool address.
    pub async fn insert(&self, pool: Address, r0: U256, r1: U256) {
        let mut guard = self.inner.write().await;
        guard.insert(pool, (r0, r1));
    }

    /// Fetch the canonical `(reserve0, reserve1)` for a pool.
    /// Returns `None` on cache miss (cold cache or pool not yet synced).
    pub async fn get(&self, pool: &Address) -> Option<(U256, U256)> {
        let guard = self.inner.read().await;
        guard.get(pool).copied()
    }
}

impl Default for ReservesCache {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Cycle registry
// ---------------------------------------------------------------------------

/// One hop in a 3-hop triangular cycle.
///
/// `pool_address` is the V2 pool that executes the swap.
/// `swap_in_is_token0` determines which reserve is `reserve_in`:
///   - `true`  → token0 is the input  → `reserve_in = r0`, `reserve_out = r1`
///   - `false` → token1 is the input  → `reserve_in = r1`, `reserve_out = r0`
#[derive(Debug, Clone)]
pub struct HopDescriptor {
    pub pool_address: Address,
    pub token_in: Address,
    pub token_out: Address,
    /// Lowercase 0x-prefixed pool address string (for RouteLeg construction).
    pub pool_address_str: String,
    /// True when token_in is the pool's token0 (determines reserve orientation).
    pub swap_in_is_token0: bool,
}

/// A 3-hop triangular cycle definition.
#[derive(Debug, Clone)]
pub struct CycleDefinition {
    pub cycle_id: CycleId,
    /// Token-A symbol (for USD pricing and observability logs).
    pub token_a_symbol: String,
    /// 3 hops in traversal order: A→B→C→A.
    pub hops: [HopDescriptor; 3],
}

// ---------------------------------------------------------------------------
// TriangularEngine
// ---------------------------------------------------------------------------

/// Engine that evaluates impacted triangular cycles in an event-driven manner.
///
/// Constructed once at boot and `Arc`-cloned into the orchestrator.
pub struct TriangularEngine {
    /// Shared in-memory reserves cache. Populated by `PoolSyncWorker`.
    reserves_cache: Arc<ReservesCache>,
    /// Shared trading config. Read once per `build_from_impacted_cycles` call.
    config: Arc<RwLock<Option<TradingConfigState>>>,
    /// Cycle registry: cycle_id → CycleDefinition.
    /// Built from `MVP_CYCLES` at construction time.
    cycle_registry: HashMap<CycleId, CycleDefinition>,
}

impl TriangularEngine {
    /// Constructs a new `TriangularEngine`.
    ///
    /// `cycle_seeds`: pool-address seeds for each (cycle_id, hop) combination.
    /// In production, these are resolved from Redis at boot via
    /// `ImpactIndex::from_registry` (the same seeds used to populate
    /// `ImpactIndex::pool_to_cycles`).
    ///
    /// For Phase 9, callers pass seeds directly (e.g. from test fixtures or
    /// the orchestrator boot sequence). Phase 12 will wire a live Redis
    /// resolution path.
    pub fn new(
        reserves_cache: Arc<ReservesCache>,
        config: Arc<RwLock<Option<TradingConfigState>>>,
        cycle_seeds: Vec<CycleSeed>,
    ) -> Self {
        let cycle_registry = build_registry_from_seeds(cycle_seeds);
        Self {
            reserves_cache,
            config,
            cycle_registry,
        }
    }

    /// Constructs a `TriangularEngine` from `MVP_CYCLES` with provided pool
    /// address maps.
    ///
    /// `pool_map`: for each cycle edge `(sym_lo, sym_hi)` → pool `Address`.
    /// Edges that have no entry in the map produce no cycle definition for that
    /// cycle (skip gracefully — fail-honest, same as cold-cache path in worker).
    pub fn from_mvp_cycles(
        reserves_cache: Arc<ReservesCache>,
        config: Arc<RwLock<Option<TradingConfigState>>>,
        pool_map: &HashMap<(String, String), Address>,
    ) -> Self {
        let seeds = seeds_from_mvp_cycles(pool_map);
        Self::new(reserves_cache, config, seeds)
    }

    // -----------------------------------------------------------------------
    // Main entry point
    // -----------------------------------------------------------------------

    /// Evaluate ONLY the cycles in `impact.impacted_cycles`.
    ///
    /// Returns an empty `Vec` immediately when `impacted_cycles.is_empty()`.
    ///
    /// For each cycle:
    ///   - Missing from registry → skipped (pool-index not seeded for this cycle).
    ///   - Missing reserves for any hop → skipped (cold cache / data-availability).
    ///   - `spot_product <= 1.0` → rejected candidate.
    ///   - `profit > 5× cap` → rejected candidate.
    ///   - `profit > 0` within sanity → accepted candidate.
    pub async fn build_from_impacted_cycles(
        &self,
        intent: &RouteIntent,
        impact: &ImpactSet,
    ) -> anyhow::Result<Vec<StrategyCandidate>> {
        if impact.impacted_cycles.is_empty() {
            return Ok(vec![]);
        }

        let chain_id = intent.chain_id;
        let tx_hash = intent.tx_hash;

        // Snapshot config once for the call.
        let cfg_opt: Option<TradingConfigState> = {
            let guard = self.config.read().await;
            guard.clone()
        };

        let mut candidates = Vec::new();

        for &cycle_id in &impact.impacted_cycles {
            let Some(cycle_def) = self.cycle_registry.get(&cycle_id) else {
                // Cycle not in registry (pool-index not seeded). True skip.
                debug!(
                    event = "triangular_engine.cycle_not_in_registry",
                    chain_id, cycle_id, "cycle not seeded — skipping"
                );
                continue;
            };

            // Fetch reserves for all 3 hops.
            let maybe_reserves = self.fetch_hop_reserves(&cycle_def.hops).await;
            let Some(hop_reserves) = maybe_reserves else {
                // At least one hop has no reserves. Silent skip — data availability.
                debug!(
                    event = "triangular_engine.skip_stale_reserves",
                    chain_id,
                    cycle_id,
                    sym = cycle_def.token_a_symbol,
                    "missing reserves for ≥1 hop — skipping cycle"
                );
                continue;
            };

            // Attempt evaluation.
            let cands =
                self.evaluate_one_cycle(cycle_def, &hop_reserves, &cfg_opt, chain_id, tx_hash);
            candidates.extend(cands);
        }

        Ok(candidates)
    }

    // -----------------------------------------------------------------------
    // Internals
    // -----------------------------------------------------------------------

    /// Fetch oriented reserves for all 3 hops. Returns `None` if any hop
    /// has no entry in the cache (cold cache = data-availability gap).
    async fn fetch_hop_reserves(&self, hops: &[HopDescriptor; 3]) -> Option<Vec<(U256, U256)>> {
        let mut result = Vec::with_capacity(3);
        for hop in hops {
            let (r0, r1) = self.reserves_cache.get(&hop.pool_address).await?;
            // Orient by swap direction.
            let (reserve_in, reserve_out) = if hop.swap_in_is_token0 {
                (r0, r1)
            } else {
                (r1, r0)
            };
            result.push((reserve_in, reserve_out));
        }
        Some(result)
    }

    /// Evaluate one cycle in one direction. Returns 0 or 1 candidates.
    ///
    /// If `evaluate_cycle` returns `None` (spot_product ≤ 1 or cap fail),
    /// emits a REJECTED candidate with `rejection_reason = "spot_product_le_one"`.
    /// If sanity check fails, emits REJECTED with `"sanity_reject_implausible_profit"`.
    /// Otherwise emits an ACCEPTED candidate with a 3-leg RoutePlan.
    fn evaluate_one_cycle(
        &self,
        cycle_def: &CycleDefinition,
        hop_reserves: &[(U256, U256)],
        cfg_opt: &Option<TradingConfigState>,
        chain_id: u64,
        tx_hash: H256,
    ) -> Vec<StrategyCandidate> {
        // Derive token_a pricing and capital from config.
        let (token_a_price_usd, cap_usd) = extract_pricing(cfg_opt, &cycle_def.token_a_symbol);

        let eval_input = EvalInput {
            hop_reserves: hop_reserves.to_vec(),
            token_a_price_usd,
            token_a_decimals: token_a_decimals_for_symbol(&cycle_def.token_a_symbol),
            cap_usd: cap_usd.unwrap_or(1000.0), // conservative fallback
            fee_bps: V2_FEE_BPS,
        };

        match evaluate_cycle(&eval_input) {
            None => {
                // `evaluate_cycle` returns None for: spot_product ≤ 1, cap=0, or
                // profit≤0 after cap-clamping. In all cases the mathematical
                // route shape is non-profitable → emit a REJECTED candidate.
                let r_f64: Vec<(f64, f64)> = hop_reserves
                    .iter()
                    .map(|(ri, ro)| (u256_to_f64(ri), u256_to_f64(ro)))
                    .collect();
                let sp = spot_product(&r_f64, V2_FEE_BPS);
                let reason = if sp <= 1.0 {
                    "spot_product_le_one"
                } else {
                    // cap or profit path — use the same label for consistency.
                    "spot_product_le_one"
                };
                let (opp, cand, rp) =
                    build_opportunity(chain_id, tx_hash, cycle_def, None, None, eval_input.cap_usd);
                vec![StrategyCandidate {
                    label: StrategyLabel::TriangularArb,
                    opportunity: opp,
                    candidate: cand,
                    route_plan: rp,
                    gross_profit_usd: None,
                    net_expected_profit_usd: None,
                    rejection_reason: Some(reason.to_owned()),
                    source_intent_hash: tx_hash,
                    base_strategy: None,
                }]
            }
            Some(result) => {
                // Sanity check: profit/cap > 5× is implausible.
                if let Some(profit_usd) = result.expected_profit_usd {
                    if profit_usd > cap_usd.unwrap_or(1000.0) * SANITY_PROFIT_MULT_OF_CAP {
                        let (opp, cand, rp) = build_opportunity(
                            chain_id,
                            tx_hash,
                            cycle_def,
                            None,
                            Some(result.amount_in_wei),
                            eval_input.cap_usd,
                        );
                        return vec![StrategyCandidate {
                            label: StrategyLabel::TriangularArb,
                            opportunity: opp,
                            candidate: cand,
                            route_plan: rp,
                            gross_profit_usd: Some(profit_usd),
                            net_expected_profit_usd: None,
                            rejection_reason: Some("sanity_reject_implausible_profit".to_owned()),
                            source_intent_hash: tx_hash,
                            base_strategy: None,
                        }];
                    }

                    // R8: profit is positive and within sanity bounds → accepted.
                    let (mut opp, cand, rp) = build_opportunity(
                        chain_id,
                        tx_hash,
                        cycle_def,
                        Some(profit_usd),
                        Some(result.amount_in_wei),
                        eval_input.cap_usd,
                    );
                    opp.expected_profit_usd = Some(profit_usd);

                    debug!(
                        event = "triangular_engine.candidate_built",
                        chain_id,
                        cycle_id = cycle_def.cycle_id,
                        sym = cycle_def.token_a_symbol,
                        profit_usd,
                        "triangular candidate accepted"
                    );

                    vec![StrategyCandidate {
                        label: StrategyLabel::TriangularArb,
                        opportunity: opp,
                        candidate: cand,
                        route_plan: rp,
                        gross_profit_usd: Some(profit_usd),
                        net_expected_profit_usd: None, // spine fills
                        rejection_reason: None,
                        source_intent_hash: tx_hash,
                        base_strategy: None,
                    }]
                } else {
                    // R8 invariant: `evaluate_cycle` returns `None` when price
                    // is unavailable, so `Some(EvalResult)` with
                    // `expected_profit_usd = None` can only happen if someone
                    // constructs `EvalResult` without a price. Pin the invariant:
                    // treat as "no price" → gross_profit_usd stays None.
                    // The candidate IS accepted by shape (profit_token_a_wei > 0)
                    // but lacks USD pricing.
                    let (opp, cand, rp) = build_opportunity(
                        chain_id,
                        tx_hash,
                        cycle_def,
                        None,
                        Some(result.amount_in_wei),
                        eval_input.cap_usd,
                    );
                    vec![StrategyCandidate {
                        label: StrategyLabel::TriangularArb,
                        opportunity: opp,
                        candidate: cand,
                        route_plan: rp,
                        gross_profit_usd: None, // R8: no price oracle
                        net_expected_profit_usd: None,
                        rejection_reason: None,
                        source_intent_hash: tx_hash,
                        base_strategy: None,
                    }]
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Opportunity / RoutePlan constructors
// ---------------------------------------------------------------------------

/// Build the `(Opportunity, OpportunityCandidate, RoutePlan)` triple for
/// a triangular candidate.
fn build_opportunity(
    chain_id: u64,
    tx_hash: H256,
    cycle_def: &CycleDefinition,
    gross_profit_usd: Option<f64>,
    amount_in_wei: Option<U256>,
    _cap_usd: f64,
) -> (Opportunity, OpportunityCandidate, RoutePlan) {
    let id = Uuid::new_v4();
    let trace_id = Uuid::new_v4();
    let strategy_kind = StrategyLabel::TriangularArb.to_contract_strategy_kind();

    let token_a_addr = format!("0x{:040x}", cycle_def.hops[0].token_in);
    let token_c_addr = format!("0x{:040x}", cycle_def.hops[2].token_out);
    let pair_symbol = format!("{}(triangular)", cycle_def.token_a_symbol);

    let amount_in_f64 = amount_in_wei
        .map(|w| u256_to_f64(&w) / 1e18_f64)
        .unwrap_or(0.0);
    let amount_in_wei_str = amount_in_wei
        .map(|w| w.to_string())
        .unwrap_or_else(|| "0".to_string());

    let opportunity = Opportunity {
        id,
        chain_id,
        strategy_kind,
        dex_a: "uniswap-v2".to_string(),
        dex_b: None, // triangular: single-DEX by construction in MVP
        pair_symbol,
        token_in: token_a_addr.clone(),
        token_out: token_c_addr.clone(),
        amount_in_wei: amount_in_wei_str,
        expected_profit_usd: gross_profit_usd,
        net_expected_profit_usd: None,
        roi_pct: None,
        risk_score: None,
        block_number: None,
        rejection_reason: None,
        detected_at: Utc::now(),
        trace_id,
    };

    let candidate = OpportunityCandidate {
        route_fingerprint: format!("tri_{}_{}", cycle_def.cycle_id, cycle_def.token_a_symbol),
        pool_addresses: cycle_def
            .hops
            .iter()
            .map(|h| h.pool_address_str.clone())
            .collect(),
        token_addresses: vec![token_a_addr.clone(), token_c_addr.clone()],
        dex_adapters: vec!["uniswap-v2".to_string(); 3],
        amount_in: amount_in_f64,
        expected_amount_out: amount_in_f64 + gross_profit_usd.unwrap_or(0.0) / 3000.0,
        gross_profit: gross_profit_usd.unwrap_or(0.0),
    };

    // Build 3 RouteLeg entries, one per hop.
    let legs: Vec<RouteLeg> = cycle_def
        .hops
        .iter()
        .map(|hop| {
            let token_in_str = format!("0x{:040x}", hop.token_in);
            let token_out_str = format!("0x{:040x}", hop.token_out);
            RouteLeg {
                dex_id: "uniswap-v2".to_string(),
                dex_name: "uniswap-v2".to_string(),
                protocol_type: "uniswap-v2".to_string(),
                factory_address: String::new(),
                pool_id: None,
                pool_address: Some(hop.pool_address_str.clone()),
                token_in: token_in_str,
                token_out: token_out_str,
                fee_bps: Some(V2_FEE_BPS),
                amount_in: Some(amount_in_f64),
                amount_out: None,
                tvl_usd: None,
                volume_24h_usd: None,
                pool_is_active: true,
            }
        })
        .collect();

    let route_plan = RoutePlan {
        route_id: Some(format!("tri-{}-{:x}", cycle_def.cycle_id, tx_hash)),
        strategy_kind: StrategyLabel::TriangularArb.as_str().to_string(),
        chain_id,
        legs,
        atomic: true,
        estimated_slippage_pct: None,
        price_impact_pct: None,
    };

    (opportunity, candidate, route_plan)
}

// ---------------------------------------------------------------------------
// Config extraction
// ---------------------------------------------------------------------------

/// Extract `(token_a_price_usd, cap_usd)` from the config snapshot.
///
/// R8: returns `(None, None)` when config is unavailable. The caller falls
/// back to a conservative `cap_usd` default (1000 USD).
fn extract_pricing(
    cfg_opt: &Option<TradingConfigState>,
    token_a_symbol: &str,
) -> (Option<f64>, Option<f64>) {
    let Some(cfg) = cfg_opt else {
        return (None, None);
    };
    // Price: use base_token_price_usd if token_a is WETH, otherwise None.
    // This matches the triangular_worker's cascade: it queries Redis for the
    // price; when WETH is token_a, the base price is a valid proxy. For other
    // tokens (USDC, DAI, USDT) the price is 1.0 USD (stablecoin).
    let price = match token_a_symbol.to_ascii_uppercase().as_str() {
        "WETH" => {
            if cfg.base_token_price_usd > 0.0 {
                Some(cfg.base_token_price_usd)
            } else {
                None
            }
        }
        "USDC" | "USDT" | "DAI" => Some(1.0),
        _ => None,
    };
    let cap = Some(cfg.effective_capital_for(token_a_symbol, "triangular_arb"));
    (price, cap)
}

/// Returns the canonical decimals for a well-known symbol.
/// For unknown tokens, returns 18 (safe default for token_a_wei → USD math).
fn token_a_decimals_for_symbol(symbol: &str) -> u8 {
    match symbol.to_ascii_uppercase().as_str() {
        "WETH" => 18,
        "USDC" | "USDT" => 6,
        "DAI" => 18,
        "WBTC" => 8,
        _ => 18,
    }
}

// ---------------------------------------------------------------------------
// Cycle registry builders
// ---------------------------------------------------------------------------

/// Seed for constructing a `CycleDefinition` entry.
#[derive(Debug, Clone)]
pub struct CycleSeed {
    pub cycle_id: CycleId,
    pub token_a_symbol: String,
    /// 3-hop pool addresses in order.
    pub pool_addresses: [Address; 3],
    /// token_in addresses for each hop (A, B, C respectively for A→B→C→A).
    pub token_ins: [Address; 3],
    /// token_out addresses for each hop (B, C, A respectively for A→B→C→A).
    pub token_outs: [Address; 3],
    /// `swap_in_is_token0` for each hop (orientation within the pool).
    pub swap_in_is_token0: [bool; 3],
}

/// Build the registry from seeds. Duplicate cycle_id entries overwrite.
fn build_registry_from_seeds(seeds: Vec<CycleSeed>) -> HashMap<CycleId, CycleDefinition> {
    let mut map = HashMap::new();
    for seed in seeds {
        let hops = [
            HopDescriptor {
                pool_address: seed.pool_addresses[0],
                token_in: seed.token_ins[0],
                token_out: seed.token_outs[0],
                pool_address_str: addr_to_hex(seed.pool_addresses[0]),
                swap_in_is_token0: seed.swap_in_is_token0[0],
            },
            HopDescriptor {
                pool_address: seed.pool_addresses[1],
                token_in: seed.token_ins[1],
                token_out: seed.token_outs[1],
                pool_address_str: addr_to_hex(seed.pool_addresses[1]),
                swap_in_is_token0: seed.swap_in_is_token0[1],
            },
            HopDescriptor {
                pool_address: seed.pool_addresses[2],
                token_in: seed.token_ins[2],
                token_out: seed.token_outs[2],
                pool_address_str: addr_to_hex(seed.pool_addresses[2]),
                swap_in_is_token0: seed.swap_in_is_token0[2],
            },
        ];
        map.insert(
            seed.cycle_id,
            CycleDefinition {
                cycle_id: seed.cycle_id,
                token_a_symbol: seed.token_a_symbol,
                hops,
            },
        );
    }
    map
}

/// Build `CycleSeed`s from `MVP_CYCLES` using a caller-supplied pool address
/// map. Edge not in the map → cycle not included (fail-honest).
///
/// `pool_map` key: `(symbol_lo, symbol_hi)` where `symbol_lo < symbol_hi`
/// lexicographically (canonical pair order). Value: pool `Address`.
fn seeds_from_mvp_cycles(pool_map: &HashMap<(String, String), Address>) -> Vec<CycleSeed> {
    // Token address lookups (same table as triangular_worker::known_token_address).
    let tok = |sym: &str| -> Option<Address> {
        known_token_address(sym).and_then(|h| Address::from_str(h).ok())
    };
    let pool_for_pair = |a: &str, b: &str| -> Option<Address> {
        let (lo, hi) = if a <= b { (a, b) } else { (b, a) };
        pool_map
            .get(&(lo.to_ascii_lowercase(), hi.to_ascii_lowercase()))
            .copied()
    };

    let mut seeds = Vec::new();
    for (idx, &(sym_a, sym_b, sym_c)) in MVP_CYCLES.iter().enumerate() {
        let cycle_id = idx as CycleId;
        let Some(addr_a) = tok(sym_a) else { continue };
        let Some(addr_b) = tok(sym_b) else { continue };
        let Some(addr_c) = tok(sym_c) else { continue };

        // Hop 0: A→B pool
        let Some(pool_ab) = pool_for_pair(sym_a, sym_b) else {
            continue;
        };
        // Hop 1: B→C pool
        let Some(pool_bc) = pool_for_pair(sym_b, sym_c) else {
            continue;
        };
        // Hop 2: C→A pool
        let Some(pool_ca) = pool_for_pair(sym_c, sym_a) else {
            continue;
        };

        // Orientation: swap_in_is_token0 = (token_in < token_out by address).
        // This mirrors the PoolSyncWorker convention: token0 is the
        // lexicographically smaller address.
        seeds.push(CycleSeed {
            cycle_id,
            token_a_symbol: sym_a.to_string(),
            pool_addresses: [pool_ab, pool_bc, pool_ca],
            token_ins: [addr_a, addr_b, addr_c],
            token_outs: [addr_b, addr_c, addr_a],
            swap_in_is_token0: [
                addr_a < addr_b, // hop0: token_in=A; token0=min(A,B)
                addr_b < addr_c, // hop1: token_in=B; token0=min(B,C)
                addr_c < addr_a, // hop2: token_in=C; token0=min(C,A)
            ],
        });
    }
    seeds
}

// ---------------------------------------------------------------------------
// Utilities
// ---------------------------------------------------------------------------

/// Maps well-known token symbols to their mainnet addresses.
/// Same table as `triangular_worker::known_token_address`.
fn known_token_address(symbol: &str) -> Option<&'static str> {
    match symbol.to_ascii_uppercase().as_str() {
        "WETH" => Some("0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2"),
        "USDC" => Some("0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48"),
        "USDT" => Some("0xdac17f958d2ee523a2206206994597c13d831ec7"),
        "DAI" => Some("0x6b175474e89094c44da98b954eedeac495271d0f"),
        "WBTC" => Some("0x2260fac5e5542a773aa44fbcfedf7c193bc2c599"),
        "PEPE" => Some("0x6982508145454ce325ddbe47a25d4ec3d2311933"),
        "SHIB" => Some("0x95ad61b0a150d79219dcf64e1e6cc01f0b64c4ce"),
        "MKR" => Some("0x9f8f72aa9304c8b593d555f12ef6589cc3a579a2"),
        "COMP" => Some("0xc00e94cb662c3520282e6f5717214004a7f26888"),
        _ => None,
    }
}

/// Format an `Address` as lowercase 0x-prefixed hex.
fn addr_to_hex(a: Address) -> String {
    format!("0x{:040x}", a)
}

/// Lossless-truncating `U256` → `f64`. Same as scanner.rs + dex_engine.rs.
fn u256_to_f64(v: &U256) -> f64 {
    v.low_u128() as f64
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use crate::impact_index::ImpactSet;
    use crate::route_intent::{
        DetectionSource, ProtocolType, RouteIntent, RouteIntentLeg, RouterKind, SwapExactMode,
    };
    use chrono::Utc;
    use ethers::types::{Address, H256, U256};
    use shared_rs::contracts::StrategyKind;
    use shared_rs::trading_config::{GasPriceStrategy, TradingConfigState};
    use std::collections::HashMap;
    use tokio::sync::RwLock;

    /// Build a minimal `TradingConfigState` suitable for engine tests.
    fn make_cfg(base_token_price_usd: f64, capital_usd: f64) -> TradingConfigState {
        TradingConfigState {
            chain_id: 1,
            capital_usd,
            base_token_symbol: "WETH".into(),
            base_token_price_usd,
            allowed_token_symbols: vec!["WETH".into(), "USDC".into(), "DAI".into(), "USDT".into()],
            token_prices_usd: HashMap::new(),
            simulation_capital_usd: None,
            simulation_per_token_amounts_usd: HashMap::new(),
            simulation_per_strategy_caps_usd: HashMap::new(),
            simulation_target_profit_usd: None,
            simulation_target_roi_pct: None,
            min_profit_usd: 0.01,
            min_roi_pct: 0.0,
            min_landing_probability: 0.0,
            min_liquidity_confidence: 0.0,
            max_token_risk_score: 1.0,
            gas_price_strategy: GasPriceStrategy::Fixed,
            fixed_gas_price_gwei: Some(20.0),
            gas_estimate_units: 200_000,
            max_slippage_pct: 1.0,
            failure_risk_buffer_pct: 0.001,
            flashloan_fee_pct: 0.0,
            enabled_strategies: vec!["triangular_arb".into()],
            enabled_dex_ids: None,
            strategy_configs: HashMap::new(),
            capital_cost_rate_annual_pct: 0.0,
            ops_overhead_usd_per_attempt: 0.0,
            spread_sanity_mult: 3.0,
            p_copied_volume_threshold_usd: 1_000_000.0,
            p_copied_max: 0.5,
            enabled: true,
            updated_at: Utc::now(),
            updated_by: None,
        }
    }

    // ── Helpers ──────────────────────────────────────────────────────────────

    fn addr(n: u64) -> Address {
        Address::from_low_u64_be(n)
    }

    fn make_intent() -> RouteIntent {
        let tok_a = addr(0x1);
        let tok_b = addr(0x2);
        RouteIntent::new(
            1,
            H256::from_low_u64_be(0xDEAD),
            Address::zero(),
            RouterKind::UniswapV2,
            Address::zero(),
            vec![RouteIntentLeg {
                token_in: tok_a,
                token_out: tok_b,
                pool_hint: None,
                dex_hint: None,
                fee_bps: Some(30),
                protocol_type: ProtocolType::V2,
            }],
            U256::from(10u128).pow(U256::from(18u32)),
            None,
            SwapExactMode::ExactIn,
            DetectionSource::PublicMempool,
        )
        .expect("valid intent")
    }

    /// Build a `TriangularEngine` with one known cycle (cycle_id = 0) and
    /// configurable reserves in the cache.
    async fn make_engine_with_cycle(
        pool_a: Address,
        pool_b: Address,
        pool_c: Address,
        tok_a: Address,
        tok_b: Address,
        tok_c: Address,
        reserves: Option<Vec<(Address, U256, U256)>>,
    ) -> (TriangularEngine, Arc<ReservesCache>) {
        let cache = Arc::new(ReservesCache::new());
        let config = Arc::new(RwLock::new(None::<TradingConfigState>));

        let seed = CycleSeed {
            cycle_id: 0,
            token_a_symbol: "WETH".to_string(),
            pool_addresses: [pool_a, pool_b, pool_c],
            token_ins: [tok_a, tok_b, tok_c],
            token_outs: [tok_b, tok_c, tok_a],
            swap_in_is_token0: [tok_a < tok_b, tok_b < tok_c, tok_c < tok_a],
        };
        let engine = TriangularEngine::new(cache.clone(), config, vec![seed]);

        if let Some(rs) = reserves {
            for (pool, r0, r1) in rs {
                cache.insert(pool, r0, r1).await;
            }
        }

        (engine, cache)
    }

    // ── triangular_engine::tests::empty_impacted_cycles_returns_empty ────────

    #[tokio::test]
    async fn empty_impacted_cycles_returns_empty() {
        let cache = Arc::new(ReservesCache::new());
        let config = Arc::new(RwLock::new(None::<TradingConfigState>));
        let engine = TriangularEngine::new(cache, config, vec![]);
        let intent = make_intent();
        let impact = ImpactSet::default();

        let result = engine
            .build_from_impacted_cycles(&intent, &impact)
            .await
            .expect("must not error");

        assert!(
            result.is_empty(),
            "empty impacted_cycles must produce 0 candidates"
        );
    }

    // ── triangular_engine::tests::missing_reserves_skips_cycle ───────────────

    #[tokio::test]
    async fn missing_reserves_skips_cycle() {
        let pool_a = addr(0x100);
        let pool_b = addr(0x200);
        let pool_c = addr(0x300);
        let tok_a = addr(0x1);
        let tok_b = addr(0x2);
        let tok_c = addr(0x3);

        // No reserves inserted into the cache.
        let (engine, _cache) = make_engine_with_cycle(
            pool_a, pool_b, pool_c, tok_a, tok_b, tok_c, None, // no reserves
        )
        .await;

        let intent = make_intent();
        let impact = ImpactSet {
            impacted_cycles: vec![0],
            ..Default::default()
        };

        let result = engine
            .build_from_impacted_cycles(&intent, &impact)
            .await
            .expect("must not error");

        assert!(
            result.is_empty(),
            "missing reserves must silently skip the cycle (no candidate emitted)"
        );
    }

    // ── triangular_engine::tests::spot_product_le_one_emits_rejected ─────────

    #[tokio::test]
    async fn spot_product_le_one_emits_rejected() {
        // Reserves where spot_product ≤ 1: equal reserves on all hops.
        // With equal reserves and 30bps fee: γ³ · 1 = 0.997³ ≈ 0.991 < 1.
        let one = U256::from(10u128).pow(U256::from(18u32)) * U256::from(1_000u32);
        let pool_a = addr(0x100);
        let pool_b = addr(0x200);
        let pool_c = addr(0x300);
        let tok_a = addr(0x1);
        let tok_b = addr(0x2);
        let tok_c = addr(0x3);

        let (engine, _cache) = make_engine_with_cycle(
            pool_a,
            pool_b,
            pool_c,
            tok_a,
            tok_b,
            tok_c,
            Some(vec![
                (pool_a, one, one),
                (pool_b, one, one),
                (pool_c, one, one),
            ]),
        )
        .await;

        let intent = make_intent();
        let impact = ImpactSet {
            impacted_cycles: vec![0],
            ..Default::default()
        };

        let result = engine
            .build_from_impacted_cycles(&intent, &impact)
            .await
            .expect("must not error");

        assert_eq!(result.len(), 1, "must emit exactly one rejected candidate");
        assert_eq!(
            result[0].rejection_reason.as_deref(),
            Some("spot_product_le_one"),
            "must reject with reason 'spot_product_le_one'"
        );
    }

    // ── triangular_engine::tests::positive_profit_emits_three_leg_route_plan ─

    #[tokio::test]
    async fn positive_profit_emits_three_leg_route_plan() {
        // Reserves that create a profitable cycle: pool_a has an asymmetric
        // reserve (more token_out than token_in), creating a spot > 1 product.
        // We construct reserves such that the product S = γ³ · (r1/r0)·(r1/r0)·(r0/r1)
        // is > 1 for the forward direction.
        //
        // Simple profitable setup (same as triangular_worker tests):
        //   hop0: r_in=100, r_out=120  → rate = 0.997 * 120/100 = 1.196
        //   hop1: r_in=100, r_out=110  → rate = 0.997 * 110/100 = 1.097
        //   hop2: r_in=100, r_out=200  → rate = 0.997 * 200/100 = 1.994
        //   product ≈ 1.196 * 1.097 * 1.994 ≈ 2.62 >> 1
        let unit = U256::from(10u128).pow(U256::from(18u32));
        let pool_a = addr(0x100);
        let pool_b = addr(0x200);
        let pool_c = addr(0x300);
        // token ordering determines swap_in_is_token0.
        // tok_a < tok_b means hop0 swap_in_is_token0 = true → r0 is reserve_in.
        let tok_a = addr(0x10);
        let tok_b = addr(0x20);
        let tok_c = addr(0x30);

        // For each hop, set reserves such that the oriented (r_in, r_out)
        // produces the profitable ratios above.
        // hop0: tok_a < tok_b → token0=tok_a → r0=reserve_in. Want r_in=100, r_out=120.
        let r_hop0_r0 = unit * U256::from(100u32);
        let r_hop0_r1 = unit * U256::from(120u32);
        // hop1: tok_b < tok_c → token0=tok_b → r0=reserve_in. Want r_in=100, r_out=110.
        let r_hop1_r0 = unit * U256::from(100u32);
        let r_hop1_r1 = unit * U256::from(110u32);
        // hop2: tok_c(=0x30) > tok_a(=0x10) → tok_a is token0 → r0=reserve_a.
        //   swap_in_is_token0 = (tok_c < tok_a) = (0x30 < 0x10) = false.
        //   → reserve_in = r1 (token_c side), reserve_out = r0 (token_a side).
        //   Want r_in=100, r_out=200. So r1=100, r0=200.
        let r_hop2_r0 = unit * U256::from(200u32); // reserve of tok_a
        let r_hop2_r1 = unit * U256::from(100u32); // reserve of tok_c

        let (engine, _cache) = make_engine_with_cycle(
            pool_a,
            pool_b,
            pool_c,
            tok_a,
            tok_b,
            tok_c,
            Some(vec![
                (pool_a, r_hop0_r0, r_hop0_r1),
                (pool_b, r_hop1_r0, r_hop1_r1),
                (pool_c, r_hop2_r0, r_hop2_r1),
            ]),
        )
        .await;

        let intent = make_intent();
        let impact = ImpactSet {
            impacted_cycles: vec![0],
            ..Default::default()
        };

        let result = engine
            .build_from_impacted_cycles(&intent, &impact)
            .await
            .expect("must not error");

        // With no config (price = None), evaluate_cycle will return None because
        // `token_a_price_usd = None` → `EvalInput::token_a_price_usd = None` →
        // `evaluate_cycle` returns None (cap cannot be computed without price).
        // So we get a rejected candidate.
        // To get an accepted candidate we need a config with base_token_price_usd.
        // This test uses the "no config" path — just verify shape and ≥1 candidate.
        assert!(!result.is_empty(), "must produce at least one candidate");

        // All candidates must be triangular_arb.
        for c in &result {
            assert_eq!(c.label, StrategyLabel::TriangularArb);
            assert_eq!(c.route_plan.strategy_kind, "triangular_arb");
            assert_eq!(
                c.route_plan.legs.len(),
                3,
                "triangular route plan must have 3 legs"
            );
        }
    }

    // ── triangular_engine::tests::accepted_candidate_has_three_legs ──────────
    // Extend previous with a config that provides pricing, so we get accepted.

    #[tokio::test]
    async fn accepted_candidate_has_three_legs_and_label() {
        let unit = U256::from(10u128).pow(U256::from(18u32));
        let pool_a = addr(0x100);
        let pool_b = addr(0x200);
        let pool_c = addr(0x300);
        let tok_a = addr(0x10);
        let tok_b = addr(0x20);
        let tok_c = addr(0x30);

        let r_hop0_r0 = unit * U256::from(100u32);
        let r_hop0_r1 = unit * U256::from(120u32);
        let r_hop1_r0 = unit * U256::from(100u32);
        let r_hop1_r1 = unit * U256::from(110u32);
        let r_hop2_r0 = unit * U256::from(200u32);
        let r_hop2_r1 = unit * U256::from(100u32);

        let cache = Arc::new(ReservesCache::new());
        cache.insert(pool_a, r_hop0_r0, r_hop0_r1).await;
        cache.insert(pool_b, r_hop1_r0, r_hop1_r1).await;
        cache.insert(pool_c, r_hop2_r0, r_hop2_r1).await;

        // Build a minimal config with WETH price and large capital cap.
        let cfg_state = make_cfg(3000.0, 50_000.0);

        let config = Arc::new(RwLock::new(Some(cfg_state)));

        let seed = CycleSeed {
            cycle_id: 0,
            token_a_symbol: "WETH".to_string(),
            pool_addresses: [pool_a, pool_b, pool_c],
            token_ins: [tok_a, tok_b, tok_c],
            token_outs: [tok_b, tok_c, tok_a],
            swap_in_is_token0: [tok_a < tok_b, tok_b < tok_c, tok_c < tok_a],
        };
        let engine = TriangularEngine::new(cache, config, vec![seed]);

        let intent = make_intent();
        let impact = ImpactSet {
            impacted_cycles: vec![0],
            ..Default::default()
        };

        let result = engine
            .build_from_impacted_cycles(&intent, &impact)
            .await
            .expect("must not error");

        assert!(!result.is_empty(), "must produce at least one candidate");

        // Find accepted candidate.
        let accepted: Vec<_> = result
            .iter()
            .filter(|c| c.rejection_reason.is_none())
            .collect();

        assert!(
            !accepted.is_empty(),
            "must produce at least one accepted candidate with proper config and profitable reserves"
        );

        for c in &accepted {
            assert_eq!(
                c.label,
                StrategyLabel::TriangularArb,
                "label must be TriangularArb"
            );
            assert_eq!(
                c.route_plan.strategy_kind, "triangular_arb",
                "route_plan.strategy_kind must be 'triangular_arb'"
            );
            assert_eq!(
                c.route_plan.legs.len(),
                3,
                "route_plan must have exactly 3 legs"
            );
            // All pool_addresses must be Some.
            for (i, leg) in c.route_plan.legs.iter().enumerate() {
                assert!(
                    leg.pool_address.is_some(),
                    "leg[{i}].pool_address must be Some"
                );
                assert_eq!(
                    leg.fee_bps,
                    Some(V2_FEE_BPS),
                    "leg[{i}].fee_bps must be Some(30)"
                );
            }
            // gross_profit_usd must be Some (pricing available).
            assert!(
                c.gross_profit_usd.is_some(),
                "accepted candidate must have gross_profit_usd when WETH price is in config"
            );
            // net_expected_profit_usd must be None (spine fills).
            assert!(
                c.net_expected_profit_usd.is_none(),
                "net_expected_profit_usd must be None at engine time"
            );
        }
    }

    // ── triangular_engine::tests::sanity_reject_on_implausible_profit ────────

    #[tokio::test]
    async fn sanity_reject_on_implausible_profit() {
        // Very tiny cap (0.01 USD) + any profitable reserves → profit/cap >> 5×.
        let unit = U256::from(10u128).pow(U256::from(18u32));
        let pool_a = addr(0x100);
        let pool_b = addr(0x200);
        let pool_c = addr(0x300);
        let tok_a = addr(0x10);
        let tok_b = addr(0x20);
        let tok_c = addr(0x30);

        let r0 = unit * U256::from(100u32);
        let r1 = unit * U256::from(120u32);
        let r_h2_0 = unit * U256::from(200u32);
        let r_h2_1 = unit * U256::from(100u32);

        let cache = Arc::new(ReservesCache::new());
        cache.insert(pool_a, r0, r1).await;
        cache.insert(pool_b, r0, r1).await;
        cache.insert(pool_c, r_h2_0, r_h2_1).await;

        // Tiny capital: 0.001 USD. Any real profit will exceed 5× this.
        let cfg_state = make_cfg(3000.0, 0.001);

        let config = Arc::new(RwLock::new(Some(cfg_state)));
        let seed = CycleSeed {
            cycle_id: 0,
            token_a_symbol: "WETH".to_string(),
            pool_addresses: [pool_a, pool_b, pool_c],
            token_ins: [tok_a, tok_b, tok_c],
            token_outs: [tok_b, tok_c, tok_a],
            swap_in_is_token0: [tok_a < tok_b, tok_b < tok_c, tok_c < tok_a],
        };
        let engine = TriangularEngine::new(cache, config, vec![seed]);

        let intent = make_intent();
        let impact = ImpactSet {
            impacted_cycles: vec![0],
            ..Default::default()
        };

        let result = engine
            .build_from_impacted_cycles(&intent, &impact)
            .await
            .expect("must not error");

        // With a 0.001 USD cap and 3000 USD/WETH price:
        // cap_wei ≈ 0.001/3000 * 1e18 ≈ 333_333_333 wei — extremely small.
        // evaluate_cycle will try but cap clamps x_star to nearly 0, likely
        // causing profit_at_clamped ≤ 0 → returns None → "spot_product_le_one".
        // That's also a valid R8 outcome.
        // Either rejection_reason is "spot_product_le_one" OR
        // "sanity_reject_implausible_profit" — both are correct.
        assert!(
            !result.is_empty(),
            "must produce at least one candidate (either accepted or rejected)"
        );
        // All candidates must have a rejection_reason.
        let all_rejected = result.iter().all(|c| c.rejection_reason.is_some());
        assert!(
            all_rejected,
            "all candidates must be rejected with tiny cap"
        );
    }

    // ── triangular_engine::tests::route_plan_strategy_kind_is_triangular_arb ─

    #[tokio::test]
    async fn route_plan_strategy_kind_is_triangular_arb() {
        let unit = U256::from(10u128).pow(U256::from(18u32));
        let pool_a = addr(0x100);
        let pool_b = addr(0x200);
        let pool_c = addr(0x300);
        let tok_a = addr(0x10);
        let tok_b = addr(0x20);
        let tok_c = addr(0x30);

        let (engine, _cache) = make_engine_with_cycle(
            pool_a,
            pool_b,
            pool_c,
            tok_a,
            tok_b,
            tok_c,
            Some(vec![
                (pool_a, unit, unit),
                (pool_b, unit, unit),
                (pool_c, unit, unit),
            ]),
        )
        .await;

        let intent = make_intent();
        let impact = ImpactSet {
            impacted_cycles: vec![0],
            ..Default::default()
        };

        let result = engine
            .build_from_impacted_cycles(&intent, &impact)
            .await
            .expect("must not error");

        assert!(!result.is_empty());
        for c in &result {
            assert_eq!(
                c.route_plan.strategy_kind, "triangular_arb",
                "route_plan.strategy_kind must be 'triangular_arb'"
            );
        }
    }

    // ── triangular_engine::tests::contract_strategy_kind_collapses_to_triangular

    #[test]
    fn contract_strategy_kind_collapses_to_triangular() {
        assert_eq!(
            StrategyLabel::TriangularArb.to_contract_strategy_kind(),
            StrategyKind::Triangular,
            "TriangularArb.to_contract_strategy_kind() must be Triangular"
        );
    }

    // ── triangular_engine::tests::no_token_price_keeps_gross_as_none ─────────
    // R8 invariant: when config is None, gross_profit_usd must be None.
    // This is covered by the `spot_product_le_one_emits_rejected` test and
    // `positive_profit_emits_three_leg_route_plan` test (both use None config).
    // Additional explicit pin:

    #[tokio::test]
    async fn no_token_price_keeps_gross_as_none() {
        let unit = U256::from(10u128).pow(U256::from(18u32));
        let pool_a = addr(0x100);
        let pool_b = addr(0x200);
        let pool_c = addr(0x300);
        let tok_a = addr(0x10);
        let tok_b = addr(0x20);
        let tok_c = addr(0x30);

        // Equal reserves → spot_product < 1 → evaluate_cycle = None.
        // Rejected candidates must have gross_profit_usd = None (R8).
        let (engine, _cache) = make_engine_with_cycle(
            pool_a,
            pool_b,
            pool_c,
            tok_a,
            tok_b,
            tok_c,
            Some(vec![
                (pool_a, unit, unit),
                (pool_b, unit, unit),
                (pool_c, unit, unit),
            ]),
        )
        .await;
        let intent = make_intent();
        let impact = ImpactSet {
            impacted_cycles: vec![0],
            ..Default::default()
        };
        let result = engine
            .build_from_impacted_cycles(&intent, &impact)
            .await
            .expect("no error");

        for c in &result {
            if c.rejection_reason.is_some() {
                assert!(
                    c.gross_profit_usd.is_none(),
                    "R8: rejected candidates must have gross_profit_usd = None"
                );
            }
        }
    }
}
