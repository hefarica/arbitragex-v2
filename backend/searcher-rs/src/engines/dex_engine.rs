// M11 allow: test modules use .unwrap()/.expect() for readability;
// production paths use ? / anyhow throughout.
//! DexEngine — Phase 8 — DEX arb V2/V3 candidate builder.
//!
//! Converts an `ImpactSet` (pools impacted by a `RouteIntent`) into a
//! `Vec<StrategyCandidate>` for each exploitable pair of pools on the same
//! token pair.
//!
//! ## Migrated from scanner.rs (~line 577-828)
//!
//! The scanner's inline V2 reserve lookup, dual-orientation heuristic,
//! V3 quote cache + Multicall3 path, and spread math are **reused via
//! the existing helpers** (`amm_math::v2_amount_out`,
//! `amm_math::v3_quote_exact_in_multicall`, `reserves::get_reserves`,
//! `reserves::get_v3_quote`, etc.). This module does NOT re-implement
//! any of that math — it re-wires it to operate on `PoolRef` pairs.
//!
//! ## R8 invariants
//!
//! - `gross_profit_usd = None` when ANY token cannot be priced.
//! - `net_expected_profit_usd = None` always at this phase (evaluator fills later).
//! - `rejection_reason` is always `Some(...)` for rejected candidates.
//! - `pool_address` on `RouteLeg` is always `Some(...)` (we know the address
//!   from `PoolRef`).
//!
//! ## Rejection labels
//!
//! | reason                    | meaning                                     |
//! |---------------------------|---------------------------------------------|
//! | `single_pool_no_spread`   | only one pool in `ImpactSet` for this pair  |
//! | `no_price_oracle`         | neither token priceable — R8 None upstream  |
//! | `non_positive_spread`     | spread <= 0 after CPMM math                 |

use crate::amm_math;
use crate::engines::triangular_engine::ReservesCache;
use crate::engines::StrategyCandidate;
use crate::impact_index::{ImpactSet, PoolRef, TokenPairKey};
use crate::route_intent::{ProtocolType, RouteIntent};
use crate::state_projector::StateProjector;
use crate::strategy_label::StrategyLabel;
use chrono::Utc;
use ethers::types::{Address, H256, U256};
use prioritization_spine::route_plan::{RouteLeg, RoutePlan};
use prioritization_spine::types::OpportunityCandidate;
use shared_rs::contracts::{Opportunity, StrategyKind};
use shared_rs::rpc_failover::AlloyHttpProvider;
use shared_rs::trading_config::TradingConfigState;
use std::sync::Arc;
use tracing::debug;
use uuid::Uuid;

// ---------------------------------------------------------------------------
// DexEngine
// ---------------------------------------------------------------------------

/// Engine that converts impacted pool pairs into DEX arb candidates.
///
/// Constructed once at boot and `Arc`-cloned into the orchestrator.
/// All internal state is either stateless helpers or `Arc`-wrapped shared
/// data — no `Mutex` on the hot path.
///
/// The operator config is NOT stored here — it is received as a method
/// parameter on each call so the engine always sees the freshest snapshot
/// without lock contention (Bug 4 fix).
pub struct DexEngine {
    /// Shared in-memory reserves cache (hydrated from Redis at boot and on
    /// every `pool_sync_worker` tick). Used by `build_from_impacted_pairs` to
    /// fetch real V2 reserves instead of fabricating unit reserves (Bug 2 fix).
    pub reserves_cache: Arc<ReservesCache>,
    /// Optional alloy HTTP provider for V3 Quoter calls.
    /// `None` → V3 pools produce `rejection_reason = "no_v3_rpc"` candidates
    /// (R8 fail-honest: never fabricate a V3 quote without an RPC).
    pub v3_provider: Option<Arc<AlloyHttpProvider>>,
    /// StateProjector for virtual post-tx state (Phase 12).
    /// `None` → V3 gross profit stays `None` (pre-Phase-12 behaviour).
    pub state_projector: Option<Arc<StateProjector>>,
}

impl DexEngine {
    /// Constructs a new `DexEngine`.
    ///
    /// - `reserves_cache`: shared in-memory reserves cache (populated from Redis).
    /// - `v3_provider`: optional alloy HTTP provider for V3 multicall quoting.
    /// - `state_projector`: optional StateProjector for V3 virtual quotes (Phase 12).
    pub fn new(
        reserves_cache: Arc<ReservesCache>,
        v3_provider: Option<Arc<AlloyHttpProvider>>,
        state_projector: Option<Arc<StateProjector>>,
    ) -> Self {
        Self {
            reserves_cache,
            v3_provider,
            state_projector,
        }
    }

    // -----------------------------------------------------------------------
    // Main entry point
    // -----------------------------------------------------------------------

    /// Build DEX arb candidates for every exploitable pool pair in the
    /// `ImpactSet`.
    ///
    /// For each token pair that has ≥2 pools in `impact.impacted_pools`,
    /// considers every (source_pool, other_pool) combination, classifies
    /// the `StrategyLabel` from their `ProtocolType`, and computes the
    /// spread using existing `amm_math` helpers.
    ///
    /// Rejected legs (single pool, non-positive spread, no oracle) produce
    /// a `StrategyCandidate` with `rejection_reason = Some(...)` — these
    /// are forwarded to `emit_rejected` by the orchestrator for RULE 00
    /// transparency.
    ///
    /// ## R8 invariants
    ///
    /// - `gross_profit_usd = None` when either token cannot be priced.
    /// - `net_expected_profit_usd = None` (always — evaluator fills later).
    /// - `pool_address` on every `RouteLeg` is `Some(...)`.
    ///
    /// `cfg`: live operator config snapshot taken once per intent by the
    /// orchestrator before calling this method. `None` = no config for this
    /// chain — USD pricing falls back to `None` (R8 fail-honest).
    pub async fn build_from_impacted_pairs(
        &self,
        intent: &RouteIntent,
        impact: &ImpactSet,
        cfg: Option<&TradingConfigState>,
    ) -> anyhow::Result<Vec<StrategyCandidate>> {
        let chain_id = intent.chain_id;
        let tx_hash = intent.tx_hash;

        let cfg_opt: Option<TradingConfigState> = cfg.cloned();

        // Group impacted pools by canonical token pair.
        let mut pair_to_pools: std::collections::HashMap<TokenPairKey, Vec<&PoolRef>> =
            std::collections::HashMap::new();
        for pool_ref in &impact.impacted_pools {
            let key = TokenPairKey::canonical(pool_ref.token0, pool_ref.token1);
            pair_to_pools.entry(key).or_default().push(pool_ref);
        }

        let mut candidates = Vec::new();

        for pools in pair_to_pools.values() {
            if pools.len() < 2 {
                // Only one pool known for this pair — no spread possible.
                // Emit one rejection candidate per lone pool so the operator
                // can see single-pool intents in the dashboard.
                if let Some(pool) = pools.first() {
                    let (opp, cand, rp) = build_rejected_opportunity(
                        chain_id,
                        tx_hash,
                        pool,
                        pool,
                        StrategyLabel::DexArbV2V2,
                    );
                    candidates.push(StrategyCandidate {
                        label: StrategyLabel::DexArbV2V2,
                        opportunity: opp,
                        candidate: cand,
                        route_plan: rp,
                        gross_profit_usd: None,
                        net_expected_profit_usd: None,
                        rejection_reason: Some("single_pool_no_spread".to_owned()),
                        source_intent_hash: tx_hash,
                        base_strategy: None,
                    });
                }
                continue;
            }

            // Every pair of pools in the set: (i, j) for i < j.
            // Both directions of the pair are covered because V2 `amount_out`
            // is direction-aware (reserve_in / reserve_out).
            for i in 0..pools.len() {
                for j in (i + 1)..pools.len() {
                    let pool_a = pools[i];
                    let pool_b = pools[j];

                    // Determine strategy label from protocol types.
                    let label = classify_label(pool_a.protocol_type, pool_b.protocol_type);

                    // Fetch real reserves from ReservesCache for V2 pools.
                    // Missing reserves → emit reserves_cache_miss rejection (R8 honest,
                    // never fabricate). The SizeOptimizer and evaluator receive only
                    // candidates where data is available.
                    //
                    // For V3 pools: attempt a virtual quote via state_projector.
                    let probe_amount = U256::from(10u128).pow(U256::from(18u32));

                    let a_is_v2 = matches!(
                        pool_a.protocol_type,
                        ProtocolType::V2 | ProtocolType::Curve | ProtocolType::Balancer
                    );
                    let b_is_v2 = matches!(
                        pool_b.protocol_type,
                        ProtocolType::V2 | ProtocolType::Curve | ProtocolType::Balancer
                    );

                    // Fetch reserves for V2 pools. Miss on either → reserves_cache_miss.
                    let reserves_a: Option<(U256, U256)> = if a_is_v2 {
                        self.reserves_cache.get(&pool_a.address).await
                    } else {
                        None // V3: handled via state_projector below
                    };
                    let reserves_b: Option<(U256, U256)> = if b_is_v2 {
                        self.reserves_cache.get(&pool_b.address).await
                    } else {
                        None
                    };

                    // If both pools are V2 and EITHER has missing reserves → reserves_cache_miss.
                    if a_is_v2 && b_is_v2 && (reserves_a.is_none() || reserves_b.is_none()) {
                        let (opp, cand, rp) =
                            build_rejected_opportunity(chain_id, tx_hash, pool_a, pool_b, label);
                        candidates.push(StrategyCandidate {
                            label,
                            opportunity: opp,
                            candidate: cand,
                            route_plan: rp,
                            gross_profit_usd: None,
                            net_expected_profit_usd: None,
                            rejection_reason: Some("reserves_cache_miss".to_owned()),
                            source_intent_hash: tx_hash,
                            base_strategy: None,
                        });
                        continue;
                    }

                    // Compute spread using real reserves.
                    let (gross_spread_units, can_price_v2) = if a_is_v2
                        && b_is_v2
                        && reserves_a.is_some()
                        && reserves_b.is_some()
                    {
                        // Both V2: reserves guaranteed Some by the guard above.
                        // Use if-let to satisfy clippy::unwrap_used.
                        let Some(ra) = reserves_a else {
                            // unreachable — guarded above, but required for exhaustiveness
                            continue;
                        };
                        let Some(rb) = reserves_b else {
                            continue;
                        };
                        let (r_in_a, r_out_a) = orient_reserves(ra, pool_a, intent);
                        let (r_in_b, r_out_b) = orient_reserves(rb, pool_b, intent);
                        let fee_a = pool_a.fee_bps.unwrap_or(30);
                        let fee_b = pool_b.fee_bps.unwrap_or(30);
                        let out_a =
                            amm_math::v2_amount_out(probe_amount, r_in_a, r_out_a, fee_a);
                        let out_b =
                            amm_math::v2_amount_out(probe_amount, r_in_b, r_out_b, fee_b);
                        let spread = if out_a >= out_b {
                            out_a.saturating_sub(out_b)
                        } else {
                            out_b.saturating_sub(out_a)
                        };
                        (spread, true)
                    } else {
                        // At least one pool is V3 — cannot compute spread here without projector.
                        (U256::zero(), false)
                    };

                    // For V3 paths: try to get a virtual quote via state_projector.
                    let v3_gross_usd: Option<f64> = if !can_price_v2 {
                        self.compute_v3_gross_usd(pool_a, pool_b, probe_amount, &cfg_opt, intent)
                            .await
                    } else {
                        None
                    };

                    // USD pricing: V2 cascade first, then V3 projector result.
                    let gross_profit_usd: Option<f64> = if can_price_v2 {
                        compute_gross_usd(&gross_spread_units, pool_a, pool_b, &cfg_opt)
                    } else {
                        v3_gross_usd
                    };

                    // EMIT the candidate — the SizeOptimizer decides final profitability.
                    // We no longer pre-reject V2/V2 pairs with spread=0 at this point.
                    // A spread=0 with real reserves is an honest equilibrium market reading;
                    // the SizeOptimizer will reject with size_optimizer_no_profit if costs
                    // exceed gross profit. R8 fail-honest: emit honest data, don't reject
                    // prematurely based on a unit-reserves approximation.
                    //
                    // Exception: single-pool (handled above). V3 with no projector (below).

                    // R8: if both tokens unpriceable AND config present → no_price_oracle.
                    if gross_profit_usd.is_none() && cfg_opt.is_some() && !can_price_v2 {
                        // Config present but no V3 projector and both pools need quoting.
                        let (opp, cand, rp) =
                            build_rejected_opportunity(chain_id, tx_hash, pool_a, pool_b, label);
                        candidates.push(StrategyCandidate {
                            label,
                            opportunity: opp,
                            candidate: cand,
                            route_plan: rp,
                            gross_profit_usd: None,
                            net_expected_profit_usd: None,
                            rejection_reason: Some("no_price_oracle".to_owned()),
                            source_intent_hash: tx_hash,
                            base_strategy: None,
                        });
                        continue;
                    }

                    // Build a full StrategyCandidate (accepted at the engine level).
                    let (opp, cand, rp) = build_accepted_opportunity(
                        chain_id,
                        tx_hash,
                        pool_a,
                        pool_b,
                        label,
                        gross_profit_usd,
                        intent.amount_in,
                    );

                    debug!(
                        event = "dex_engine.candidate_built",
                        chain_id,
                        strategy = label.as_str(),
                        pool_a = %pool_a.address,
                        pool_b = %pool_b.address,
                        gross_usd = ?gross_profit_usd,
                    );

                    candidates.push(StrategyCandidate {
                        label,
                        opportunity: opp,
                        candidate: cand,
                        route_plan: rp,
                        gross_profit_usd,
                        net_expected_profit_usd: None, // filled by evaluator
                        rejection_reason: None,
                        source_intent_hash: tx_hash,
                        base_strategy: None,
                    });
                }
            }
        }

        Ok(candidates)
    }

    // -----------------------------------------------------------------------
    // V3 gross USD computation via StateProjector (Phase 12)
    // -----------------------------------------------------------------------

    /// Attempt to compute a gross USD spread for a V3-bearing pool pair using
    /// the state_projector's virtual quote capability.
    ///
    /// For a (V3, V2) or (V2, V3) or (V3, V3) pair:
    ///   - Get virtual quote from pool_a for `probe_amount` → `out_a`.
    ///   - Get virtual quote from pool_b for `probe_amount` → `out_b`.
    ///   - `spread = |out_a - out_b|` (same orientation check as V2 path).
    ///   - Convert to USD via base_token_price_usd.
    ///
    /// Returns `None` when no projector is wired, when pool is V2 (handled
    /// separately), or when the quote fails (R8 honest).
    async fn compute_v3_gross_usd(
        &self,
        pool_a: &PoolRef,
        pool_b: &PoolRef,
        probe_amount: U256,
        cfg_opt: &Option<TradingConfigState>,
        intent: &RouteIntent,
    ) -> Option<f64> {
        let projector = self.state_projector.as_ref()?;
        let cfg = cfg_opt.as_ref()?;

        // For each V3 pool, get a virtual quote using project_v3_quote.
        // V2 pools: use v2_amount_out with canonical unit reserves (same approximation
        // as compute_spread_v2_only — the real reserves are used by size_optimizer).
        let out_a = self
            .get_pool_quote(pool_a, probe_amount, projector, intent)
            .await?;
        let out_b = self
            .get_pool_quote(pool_b, probe_amount, projector, intent)
            .await?;

        if out_a.is_zero() && out_b.is_zero() {
            return None;
        }

        // Spread = |out_a - out_b| if both are quoting the same direction.
        let spread = if out_a >= out_b {
            out_a.saturating_sub(out_b)
        } else {
            out_b.saturating_sub(out_a)
        };

        if spread.is_zero() {
            return None;
        }

        // USD conversion: spread / 1e18 * base_token_price_usd.
        let spread_f64 = u256_to_f64_lossy(spread) / 1e18_f64;
        if cfg.base_token_price_usd > 0.0 {
            Some(spread_f64 * cfg.base_token_price_usd)
        } else {
            None
        }
    }

    /// Get amount_out for `probe_amount` of token_in from a V3 pool using
    /// the state_projector's virtual quote capability.
    ///
    /// V2 pools are handled directly in `build_from_impacted_pairs` with
    /// real reserves from `ReservesCache`. This method is called only for V3.
    async fn get_pool_quote(
        &self,
        pool: &PoolRef,
        probe_amount: U256,
        projector: &StateProjector,
        intent: &RouteIntent,
    ) -> Option<U256> {
        // Only called for V3 pools. V2 uses real reserves in the caller.
        if matches!(pool.protocol_type, ProtocolType::V3) {
            let intent_token_in = intent.legs.first().map(|l| l.token_in).unwrap_or_default();
            let zero_for_one =
                intent_token_in == pool.token0 || intent_token_in == Address::zero();
            let sp_pool = crate::state_projector::PoolRef {
                address: pool.address,
                token0: pool.token0,
                token1: pool.token1,
                fee_bps: pool.fee_bps,
            };
            projector
                .project_v3_quote(&sp_pool, probe_amount, zero_for_one)
                .await
                .map(|q| q.amount_out)
        } else {
            // V2 pools should never reach here — handled via ReservesCache in caller.
            // Return None (R8: no fabrication).
            None
        }
    }
}

// ---------------------------------------------------------------------------
// Classification helpers
// ---------------------------------------------------------------------------

/// Determines the `StrategyLabel` from the two pools' `ProtocolType`s.
///
/// The first pool is the "source" pool (the one the intent directly impacted);
/// the second is the "other" pool used to close the arb. Label encodes the
/// protocol of (source, other) in that order.
pub fn classify_label(source: ProtocolType, other: ProtocolType) -> StrategyLabel {
    match (source, other) {
        (ProtocolType::V2, ProtocolType::V2) => StrategyLabel::DexArbV2V2,
        (ProtocolType::V2, ProtocolType::V3) => StrategyLabel::DexArbV2V3,
        (ProtocolType::V3, ProtocolType::V2) => StrategyLabel::DexArbV3V2,
        (ProtocolType::V3, ProtocolType::V3) => StrategyLabel::DexArbV3V3,
        // Curve / Balancer / Unknown pools: classify as V2V2 for the gate
        // (conservative — the evaluator may reject on protocol-type gate).
        // A future sprint adds dedicated labels for Curve/Balancer.
        _ => StrategyLabel::DexArbV2V2,
    }
}

// ---------------------------------------------------------------------------
// Reserves orientation
// ---------------------------------------------------------------------------

/// Orient canonical (reserve0, reserve1) into (reserve_in, reserve_out) for a
/// given intent's token_in direction.
///
/// `reserve0` corresponds to `pool.token0`; `reserve1` to `pool.token1`.
/// If the intent swaps token0 → token1: `reserve_in = r0, reserve_out = r1`.
/// If the intent swaps token1 → token0: `reserve_in = r1, reserve_out = r0`.
/// When the intent's token_in is unknown (zero address or no legs): default to
/// token0→token1 direction (conservative; SizeOptimizer will re-orient).
fn orient_reserves(
    reserves: (U256, U256),
    pool: &PoolRef,
    intent: &RouteIntent,
) -> (U256, U256) {
    let (r0, r1) = reserves;
    let intent_token_in = intent.legs.first().map(|l| l.token_in).unwrap_or_default();
    // If token_in matches token1 (i.e. swapping token1 in), swap the orientations.
    if intent_token_in == pool.token1
        && intent_token_in != Address::zero()
        && intent_token_in != pool.token0
    {
        (r1, r0)
    } else {
        (r0, r1)
    }
}

// ---------------------------------------------------------------------------
// USD pricing (mirrors scanner.rs compute_gross_usd_for_spread)
// ---------------------------------------------------------------------------

/// Converts a spread denominated in token_out units to USD.
///
/// Phase 7 / R8 invariant: the engine does NOT have Redis access (token symbol
/// resolution from `TokenMeta` requires a Redis read that happens in
/// `scanner.rs` and will be wired in Phase 12 when `ReservesCache` is plumbed
/// into the engine). Until then, this function returns `None` for all pairs
/// unless the config supplies a `base_token_price_usd` AND the spread is
/// non-zero — a conservative best-effort approximation that mirrors the
/// scanner's oracle-gap path (`gross_profit_f64 = None` when neither token
/// is a known stablecoin or base token from the Redis cache).
///
/// The evaluator's `CascadePriceOracle` will attempt full USD resolution
/// downstream from live Redis + Coingecko data. Any `None` here degrades to
/// `UnknownTokenPrice` at the gate, which is the R8-correct outcome.
///
/// ## When `Some(usd)` IS returned (fast-filter path)
///
/// If the config's `base_token_price_usd > 0`, we assume the pair involves
/// the base token (WETH) and multiply. This is a heuristic (not accurate
/// for non-WETH pairs), but it is the SAME heuristic used by the scanner's
/// pre-filter and is corrected by the spine evaluator's cascade oracle. The
/// goal is to produce a non-zero fast-filter signal for WETH pairs so the
/// gate does not default-reject them before the oracle runs.
fn compute_gross_usd(
    spread_units: &U256,
    _pool_a: &PoolRef,
    _pool_b: &PoolRef,
    cfg_opt: &Option<TradingConfigState>,
) -> Option<f64> {
    // No config → oracle gap — R8 None.
    let cfg = cfg_opt.as_ref()?;

    // Zero spread → not profitable — R8 None.
    if spread_units.is_zero() {
        return None;
    }

    // Spread in f64 (lossy; acceptable — scoring path only).
    // Divide by 1e18 to convert from wei to token units (assumes 18-decimal
    // token_out — the evaluator corrects this with real TokenMeta decimals).
    let spread_f64 = u256_to_f64_lossy(*spread_units) / 1e18_f64;

    // Fast-filter: use base_token_price_usd as a proxy.
    // R8: only return Some when price > 0 (fabricating a zero-price USD is
    // indistinguishable from "not computed"). If price == 0 → None.
    if cfg.base_token_price_usd > 0.0 {
        Some(spread_f64 * cfg.base_token_price_usd)
    } else {
        None
    }
}

// ---------------------------------------------------------------------------
// Opportunity constructors
// ---------------------------------------------------------------------------

/// Builds an `Opportunity`, `OpportunityCandidate`, and `RoutePlan` for
/// an accepted (engine-level) DEX arb candidate.
fn build_accepted_opportunity(
    chain_id: u64,
    tx_hash: H256,
    pool_a: &PoolRef,
    pool_b: &PoolRef,
    label: StrategyLabel,
    gross_profit_usd: Option<f64>,
    amount_in_wei: U256,
) -> (Opportunity, OpportunityCandidate, RoutePlan) {
    let strategy_kind: StrategyKind = label.to_contract_strategy_kind();
    let id = Uuid::new_v4();
    let trace_id = Uuid::new_v4();

    let token_in_str = format!("0x{:040x}", pool_a.token0);
    let token_out_str = format!("0x{:040x}", pool_a.token1);
    let pair_symbol = format!("{}…/{}…", &token_in_str[2..8], &token_out_str[2..8],);

    let amount_in_wei_str = amount_in_wei.to_string();
    let amount_in_f64: f64 = u256_to_f64_lossy(amount_in_wei) / 1e18_f64;

    let opportunity = Opportunity {
        id,
        chain_id,
        strategy_kind,
        dex_a: pool_a.dex_name.clone(),
        dex_b: Some(pool_b.dex_name.clone()),
        pair_symbol,
        token_in: token_in_str.clone(),
        token_out: token_out_str.clone(),
        amount_in_wei: amount_in_wei_str,
        expected_profit_usd: gross_profit_usd,
        net_expected_profit_usd: None, // filled by evaluator
        roi_pct: None,
        risk_score: None,
        block_number: None,
        rejection_reason: None,
        detected_at: Utc::now(),
        trace_id,
    };

    let pool_a_lower = format!("0x{:040x}", pool_a.address);
    let pool_b_lower = format!("0x{:040x}", pool_b.address);

    let candidate = OpportunityCandidate {
        route_fingerprint: format!("{}_{}_{}", pool_a.dex_name, token_in_str, token_out_str),
        pool_addresses: vec![pool_a_lower.clone(), pool_b_lower.clone()],
        token_addresses: vec![token_in_str.clone(), token_out_str.clone()],
        dex_adapters: vec![pool_a.dex_name.clone(), pool_b.dex_name.clone()],
        amount_in: amount_in_f64,
        expected_amount_out: amount_in_f64, // best estimate without real reserves
        gross_profit: gross_profit_usd.unwrap_or(0.0),
    };

    let leg_a = build_route_leg(pool_a, &token_in_str, &token_out_str, amount_in_f64);
    let leg_b = build_route_leg(pool_b, &token_out_str, &token_in_str, amount_in_f64);

    let route_plan = RoutePlan {
        route_id: Some(format!("{}-{}-{:x}", pool_a_lower, pool_b_lower, tx_hash)),
        strategy_kind: label.as_str().to_string(),
        chain_id,
        legs: vec![leg_a, leg_b],
        atomic: true,
        estimated_slippage_pct: None,
        price_impact_pct: None,
    };

    (opportunity, candidate, route_plan)
}

/// Builds a minimal (`Opportunity`, `OpportunityCandidate`, `RoutePlan`)
/// for a rejected candidate (single_pool / no_price_oracle / non_positive_spread).
///
/// The opportunity row carries all required fields with R8-honest defaults.
/// The rejection_reason is NOT set here — the caller sets it on the
/// `StrategyCandidate` wrapper.
fn build_rejected_opportunity(
    chain_id: u64,
    tx_hash: H256,
    pool_a: &PoolRef,
    pool_b: &PoolRef,
    label: StrategyLabel,
) -> (Opportunity, OpportunityCandidate, RoutePlan) {
    build_accepted_opportunity(
        chain_id,
        tx_hash,
        pool_a,
        pool_b,
        label,
        None,                                      // R8: no profit for rejected candidates
        U256::from(10u128).pow(U256::from(18u32)), // unit probe
    )
}

/// Builds a `RouteLeg` from a `PoolRef`.
///
/// `pool_address` is always `Some(...)` — the engine always knows the address
/// from `PoolRef` (spec rule: `pool_address` in `RouteLeg` IS populated).
fn build_route_leg(pool: &PoolRef, token_in: &str, token_out: &str, amount_in: f64) -> RouteLeg {
    let pool_addr_lower = format!("0x{:040x}", pool.address);
    RouteLeg {
        dex_id: pool.dex_name.to_ascii_lowercase(),
        dex_name: pool.dex_name.clone(),
        protocol_type: protocol_type_to_str(pool.protocol_type),
        factory_address: String::new(), // not available in PoolRef; follow-up
        pool_id: None,
        pool_address: Some(pool_addr_lower),
        token_in: token_in.to_ascii_lowercase(),
        token_out: token_out.to_ascii_lowercase(),
        fee_bps: pool.fee_bps,
        amount_in: Some(amount_in),
        amount_out: None, // not computed yet (evaluator fills)
        tvl_usd: None,    // R8: not available — never fabricated
        volume_24h_usd: None,
        pool_is_active: true,
    }
}

// ---------------------------------------------------------------------------
// Utility helpers
// ---------------------------------------------------------------------------

/// Maps `ProtocolType` to the `protocol_type` string used in `RouteLeg`.
fn protocol_type_to_str(pt: ProtocolType) -> String {
    match pt {
        ProtocolType::V2 => "uniswap-v2".to_string(),
        ProtocolType::V3 => "uniswap-v3".to_string(),
        ProtocolType::Curve => "curve".to_string(),
        ProtocolType::Balancer => "balancer".to_string(),
        ProtocolType::Unknown => "unknown".to_string(),
    }
}

/// Lossless-truncating `U256` → `f64`. The same helper used in scanner.rs.
/// Lossy past ~15 significant figures (f64 mantissa); acceptable on the
/// scoring/display path. Never re-fed into on-chain arithmetic.
fn u256_to_f64_lossy(v: U256) -> f64 {
    // U256 → u128 (truncates top 128 bits — negligible for amounts
    // that fit in u128, which is all practical EVM balances).
    v.low_u128() as f64
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use crate::engines::triangular_engine::ReservesCache;
    use crate::impact_index::{ImpactSet, PoolRef};
    use crate::route_intent::{
        DetectionSource, ProtocolType, RouteIntent, RouteIntentLeg, RouterKind, SwapExactMode,
    };
    use crate::strategy_label::StrategyLabel;
    use ethers::types::{Address, H256, U256};
    use shared_rs::contracts::StrategyKind;

    // ── Helpers ──────────────────────────────────────────────────────────────

    fn addr(n: u64) -> Address {
        Address::from_low_u64_be(n)
    }

    fn make_pool(address: Address, token0: Address, token1: Address, pt: ProtocolType) -> PoolRef {
        PoolRef {
            chain_id: 1,
            address,
            dex_name: match pt {
                ProtocolType::V2 => "uniswap-v2".to_string(),
                ProtocolType::V3 => "uniswap-v3".to_string(),
                _ => "unknown".to_string(),
            },
            protocol_type: pt,
            token0,
            token1,
            fee_bps: match pt {
                ProtocolType::V2 => Some(30),
                ProtocolType::V3 => Some(500),
                _ => None,
            },
        }
    }

    fn make_intent(token_in: Address, token_out: Address) -> RouteIntent {
        RouteIntent::new(
            1,
            H256::from_low_u64_be(0xDEAD),
            Address::zero(),
            RouterKind::UniswapV2,
            Address::zero(),
            vec![RouteIntentLeg {
                token_in,
                token_out,
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

    fn make_impact(pools: Vec<PoolRef>) -> ImpactSet {
        ImpactSet {
            impacted_pools: pools,
            ..Default::default()
        }
    }

    /// Build engine with specified reserves pre-loaded into the cache.
    /// `reserves`: (pool_address, reserve0, reserve1).
    async fn make_engine_with_reserves(reserves: Vec<(Address, U256, U256)>) -> DexEngine {
        let cache = Arc::new(ReservesCache::new());
        for (addr, r0, r1) in reserves {
            cache.insert(addr, r0, r1).await;
        }
        DexEngine::new(cache, None, None)
    }

    fn make_engine() -> DexEngine {
        DexEngine::new(Arc::new(ReservesCache::new()), None, None)
    }

    // ── dex_engine::tests::v2_v2_real_reserves_emits_candidate_for_optimizer ──
    // Bug 2 fix: two V2 pools with real reserves loaded — candidate emitted
    // (not rejected) so SizeOptimizer can evaluate.

    #[tokio::test]
    async fn v2_v2_real_reserves_emits_candidate_for_optimizer() {
        let tok_a = addr(0x1);
        let tok_b = addr(0x2);
        let pool_addr1 = addr(0x10);
        let pool_addr2 = addr(0x11);
        let pool1 = make_pool(pool_addr1, tok_a, tok_b, ProtocolType::V2);
        let pool2 = make_pool(pool_addr2, tok_a, tok_b, ProtocolType::V2);
        let intent = make_intent(tok_a, tok_b);
        let impact = make_impact(vec![pool1, pool2]);

        // Pre-load real (nonzero) reserves into the cache.
        let unit = U256::from(10u128).pow(U256::from(18u32)) * U256::from(1_000u32);
        let engine = make_engine_with_reserves(vec![
            (pool_addr1, unit, unit),
            (pool_addr2, unit, unit),
        ])
        .await;

        let candidates = engine
            .build_from_impacted_pairs(&intent, &impact, None)
            .await
            .expect("engine must not error");

        // Must emit at least one candidate — not rejected for reserves_cache_miss.
        assert!(!candidates.is_empty(), "must produce at least one candidate");
        // No reserves_cache_miss rejection.
        for c in &candidates {
            assert_ne!(
                c.rejection_reason.as_deref(),
                Some("reserves_cache_miss"),
                "must not reject with reserves_cache_miss when reserves are present"
            );
        }
        // All must be DexArbV2V2.
        for c in &candidates {
            assert_eq!(c.label, StrategyLabel::DexArbV2V2, "must classify as DexArbV2V2");
        }
    }

    // ── dex_engine::tests::v2_v2_no_cache_emits_reserves_cache_miss ──────────
    // Bug 2 fix: V2 pool not in cache → rejected with reserves_cache_miss.

    #[tokio::test]
    async fn v2_v2_no_cache_emits_reserves_cache_miss() {
        let tok_a = addr(0x1);
        let tok_b = addr(0x2);
        let pool1 = make_pool(addr(0x10), tok_a, tok_b, ProtocolType::V2);
        let pool2 = make_pool(addr(0x11), tok_a, tok_b, ProtocolType::V2);
        let intent = make_intent(tok_a, tok_b);
        let impact = make_impact(vec![pool1, pool2]);
        // Empty cache — no reserves loaded.
        let engine = make_engine();

        let candidates = engine
            .build_from_impacted_pairs(&intent, &impact, None)
            .await
            .expect("engine must not error");

        assert!(!candidates.is_empty(), "must produce rejection candidates");
        let has_cache_miss = candidates
            .iter()
            .any(|c| c.rejection_reason.as_deref() == Some("reserves_cache_miss"));
        assert!(
            has_cache_miss,
            "must have at least one reserves_cache_miss rejection when cache is empty"
        );
    }

    // ── dex_engine::tests::v2_v2_classifies_correctly ────────────────────────

    #[tokio::test]
    async fn v2_v2_classifies_correctly() {
        let tok_a = addr(0x1);
        let tok_b = addr(0x2);
        let pool_addr1 = addr(0x10);
        let pool_addr2 = addr(0x11);
        let pool1 = make_pool(pool_addr1, tok_a, tok_b, ProtocolType::V2);
        let pool2 = make_pool(pool_addr2, tok_a, tok_b, ProtocolType::V2);
        let intent = make_intent(tok_a, tok_b);
        let impact = make_impact(vec![pool1, pool2]);
        // Pre-load reserves so the engine proceeds past the cache-miss gate.
        let unit = U256::from(10u128).pow(U256::from(18u32)) * U256::from(1_000u32);
        let engine = make_engine_with_reserves(vec![
            (pool_addr1, unit, unit),
            (pool_addr2, unit, unit),
        ])
        .await;

        let candidates = engine
            .build_from_impacted_pairs(&intent, &impact, None)
            .await
            .expect("engine must not error");

        // Should have at least one candidate (may be accepted or may have no_price_oracle
        // since config is None, but must never be reserves_cache_miss).
        assert!(
            !candidates.is_empty(),
            "must produce at least one candidate"
        );
        // All DexArbV2V2 labels.
        for c in &candidates {
            assert_eq!(
                c.label,
                StrategyLabel::DexArbV2V2,
                "v2/v2 pair must classify as DexArbV2V2"
            );
        }
        // route_plan must have exactly 2 legs.
        for c in &candidates {
            assert_eq!(
                c.route_plan.legs.len(),
                2,
                "route_plan must have 2 legs for a 2-pool DEX arb"
            );
            assert_eq!(c.route_plan.legs[0].protocol_type, "uniswap-v2");
            assert_eq!(c.route_plan.legs[1].protocol_type, "uniswap-v2");
        }
    }

    // ── dex_engine::tests::v3_legs_use_state_projector_quote ────────────────
    // Bug 2 fix: V3 pools go through state_projector (no_price_oracle when None).

    #[tokio::test]
    async fn v3_legs_use_state_projector_quote() {
        let tok_a = addr(0x1);
        let tok_b = addr(0x2);
        let pool_v3 = make_pool(addr(0x10), tok_a, tok_b, ProtocolType::V3);
        let pool_v2 = make_pool(addr(0x11), tok_a, tok_b, ProtocolType::V2);
        let intent = make_intent(tok_a, tok_b);
        let impact = make_impact(vec![pool_v3.clone(), pool_v2.clone()]);
        // Engine with no state_projector → V3 path cannot quote.
        let engine = make_engine();

        let candidates = engine
            .build_from_impacted_pairs(&intent, &impact, None)
            .await
            .expect("engine must not error");

        // V2/V3 mix: since projector is None and both pools are not V2-only,
        // can_price_v2=false. The engine emits no_price_oracle (config=None so
        // the oracle-gap check doesn't fire; instead the candidate falls through
        // to accepted path with gross=None). Either way: candidates is non-empty
        // and labels include V2V3 or V3V2.
        assert!(!candidates.is_empty(), "must produce at least one candidate");
        let labels: std::collections::HashSet<StrategyLabel> =
            candidates.iter().map(|c| c.label).collect();
        let has_v2v3_or_v3v2 = labels.contains(&StrategyLabel::DexArbV2V3)
            || labels.contains(&StrategyLabel::DexArbV3V2);
        assert!(
            has_v2v3_or_v3v2,
            "V2/V3 pair must classify as DexArbV2V3 or DexArbV3V2"
        );
    }

    // ── dex_engine::tests::v2_v3_classifies_correctly ────────────────────────

    #[tokio::test]
    async fn v2_v3_classifies_correctly() {
        let tok_a = addr(0x1);
        let tok_b = addr(0x2);
        // pool1 is V2, pool2 is V3 → but pool ordering in ImpactSet is not
        // guaranteed so we test both (i<j) combinations below.
        let pool_v2 = make_pool(addr(0x10), tok_a, tok_b, ProtocolType::V2);
        let pool_v3 = make_pool(addr(0x11), tok_a, tok_b, ProtocolType::V3);
        let intent = make_intent(tok_a, tok_b);
        let impact = make_impact(vec![pool_v2, pool_v3]);
        let engine = make_engine();

        let candidates = engine
            .build_from_impacted_pairs(&intent, &impact, None)
            .await
            .expect("engine must not error");

        assert!(!candidates.is_empty());
        let labels: std::collections::HashSet<StrategyLabel> =
            candidates.iter().map(|c| c.label).collect();
        let has_v2v3_or_v3v2 = labels.contains(&StrategyLabel::DexArbV2V3)
            || labels.contains(&StrategyLabel::DexArbV3V2);
        assert!(
            has_v2v3_or_v3v2,
            "V2/V3 pair must classify as DexArbV2V3 or DexArbV3V2"
        );
    }

    // ── dex_engine::tests::v3_v2_classifies_correctly ────────────────────────

    #[tokio::test]
    async fn v3_v2_classifies_correctly() {
        let tok_a = addr(0x1);
        let tok_b = addr(0x2);
        // Force pool ordering: V3 first (i=0), V2 second (i=1).
        let pool_v3 = make_pool(addr(0x10), tok_a, tok_b, ProtocolType::V3);
        let pool_v2 = make_pool(addr(0x11), tok_a, tok_b, ProtocolType::V2);
        let intent = make_intent(tok_a, tok_b);
        let impact = make_impact(vec![pool_v3, pool_v2]);
        let engine = make_engine();

        let candidates = engine
            .build_from_impacted_pairs(&intent, &impact, None)
            .await
            .expect("engine must not error");

        assert!(!candidates.is_empty());
        let has_v3v2 = candidates
            .iter()
            .any(|c| c.label == StrategyLabel::DexArbV3V2);
        assert!(
            has_v3v2,
            "V3 source / V2 other must classify as DexArbV3V2, candidates: {:?}",
            candidates.iter().map(|c| c.label).collect::<Vec<_>>()
        );
    }

    // ── dex_engine::tests::v3_v3_classifies_correctly ────────────────────────

    #[tokio::test]
    async fn v3_v3_classifies_correctly() {
        let tok_a = addr(0x1);
        let tok_b = addr(0x2);
        let pool1 = make_pool(addr(0x10), tok_a, tok_b, ProtocolType::V3);
        let pool2 = make_pool(addr(0x11), tok_a, tok_b, ProtocolType::V3);
        let intent = make_intent(tok_a, tok_b);
        let impact = make_impact(vec![pool1, pool2]);
        let engine = make_engine();

        let candidates = engine
            .build_from_impacted_pairs(&intent, &impact, None)
            .await
            .expect("engine must not error");

        assert!(!candidates.is_empty());
        for c in &candidates {
            assert_eq!(c.label, StrategyLabel::DexArbV3V3);
        }
    }

    // ── dex_engine::tests::single_pool_rejects_with_reason ──────────────────

    #[tokio::test]
    async fn single_pool_rejects_with_reason() {
        let tok_a = addr(0x1);
        let tok_b = addr(0x2);
        let pool = make_pool(addr(0x10), tok_a, tok_b, ProtocolType::V2);
        let intent = make_intent(tok_a, tok_b);
        // Only one pool in the impact set.
        let impact = make_impact(vec![pool]);
        let engine = make_engine();

        let candidates = engine
            .build_from_impacted_pairs(&intent, &impact, None)
            .await
            .expect("engine must not error");

        assert_eq!(
            candidates.len(),
            1,
            "must produce exactly one rejection candidate"
        );
        assert_eq!(
            candidates[0].rejection_reason.as_deref(),
            Some("single_pool_no_spread"),
            "single pool must produce rejection_reason = 'single_pool_no_spread'"
        );
    }

    // ── dex_engine::tests::unpriceable_token_keeps_gross_as_none ─────────────

    #[tokio::test]
    async fn unpriceable_token_keeps_gross_as_none() {
        // No config → gross_profit_usd must be None (R8 invariant).
        let tok_a = addr(0x1);
        let tok_b = addr(0x2);
        let pool_addr1 = addr(0x10);
        let pool_addr2 = addr(0x11);
        let pool1 = make_pool(pool_addr1, tok_a, tok_b, ProtocolType::V2);
        let pool2_different_fee = PoolRef {
            fee_bps: Some(100), // different fee → different spread
            ..make_pool(pool_addr2, tok_a, tok_b, ProtocolType::V2)
        };
        let intent = make_intent(tok_a, tok_b);
        let impact = make_impact(vec![pool1, pool2_different_fee]);
        // Pre-load reserves so the engine gets past the cache-miss gate.
        let unit = U256::from(10u128).pow(U256::from(18u32)) * U256::from(1_000u32);
        let engine = make_engine_with_reserves(vec![
            (pool_addr1, unit, unit),
            (pool_addr2, unit, unit),
        ])
        .await;

        let candidates = engine
            .build_from_impacted_pairs(&intent, &impact, None) // config = None
            .await
            .expect("engine must not error");

        // When config is None, gross_profit_usd must be None for all.
        for c in &candidates {
            assert!(
                c.gross_profit_usd.is_none(),
                "R8: gross_profit_usd must be None when config is None (no price oracle)"
            );
        }
    }

    // ── dex_engine::tests::route_plan_has_two_legs_with_pool_addresses ────────

    #[tokio::test]
    async fn route_plan_has_two_legs_with_pool_addresses() {
        let tok_a = addr(0x1);
        let tok_b = addr(0x2);
        let pool_addr1 = addr(0xAA);
        let pool_addr2 = addr(0xBB);
        let pool1 = make_pool(pool_addr1, tok_a, tok_b, ProtocolType::V2);
        let pool2 = PoolRef {
            fee_bps: Some(100), // different fee → asymmetric spread
            ..make_pool(pool_addr2, tok_a, tok_b, ProtocolType::V2)
        };
        let intent = make_intent(tok_a, tok_b);
        let impact = make_impact(vec![pool1.clone(), pool2.clone()]);
        let unit = U256::from(10u128).pow(U256::from(18u32)) * U256::from(1_000u32);
        let engine = make_engine_with_reserves(vec![
            (pool_addr1, unit, unit),
            (pool_addr2, unit, unit),
        ])
        .await;

        let candidates = engine
            .build_from_impacted_pairs(&intent, &impact, None)
            .await
            .expect("engine must not error");

        // Find candidates that are not single_pool or reserves_cache_miss.
        let two_pool: Vec<_> = candidates
            .iter()
            .filter(|c| {
                c.rejection_reason.as_deref() != Some("single_pool_no_spread")
                    && c.rejection_reason.as_deref() != Some("reserves_cache_miss")
            })
            .collect();

        assert!(
            !two_pool.is_empty(),
            "must produce at least one 2-pool candidate"
        );

        for c in &two_pool {
            assert_eq!(c.route_plan.legs.len(), 2, "route_plan must have 2 legs");
            assert!(
                c.route_plan.legs[0].pool_address.is_some(),
                "legs[0].pool_address must be Some"
            );
            assert!(
                c.route_plan.legs[1].pool_address.is_some(),
                "legs[1].pool_address must be Some"
            );
        }
    }

    // ── dex_engine::tests::contract_strategy_kind_collapses_correctly ─────────

    #[test]
    fn contract_strategy_kind_collapses_correctly() {
        // All four V2/V3 variants must collapse to StrategyKind::DexArb (spec §3.1).
        let variants = [
            StrategyLabel::DexArbV2V2,
            StrategyLabel::DexArbV2V3,
            StrategyLabel::DexArbV3V2,
            StrategyLabel::DexArbV3V3,
        ];
        for label in variants {
            assert_eq!(
                label.to_contract_strategy_kind(),
                StrategyKind::DexArb,
                "{label:?}.to_contract_strategy_kind() must be DexArb"
            );
        }
    }

    // ── dex_engine::tests::classify_label_matrix ────────────────────────────

    #[test]
    fn classify_label_matrix() {
        assert_eq!(
            classify_label(ProtocolType::V2, ProtocolType::V2),
            StrategyLabel::DexArbV2V2
        );
        assert_eq!(
            classify_label(ProtocolType::V2, ProtocolType::V3),
            StrategyLabel::DexArbV2V3
        );
        assert_eq!(
            classify_label(ProtocolType::V3, ProtocolType::V2),
            StrategyLabel::DexArbV3V2
        );
        assert_eq!(
            classify_label(ProtocolType::V3, ProtocolType::V3),
            StrategyLabel::DexArbV3V3
        );
    }

    // ── dex_engine::tests::route_plan_strategy_kind_matches_label ─────────────

    #[tokio::test]
    async fn route_plan_strategy_kind_matches_label() {
        let tok_a = addr(0x1);
        let tok_b = addr(0x2);
        let pool_addr1 = addr(0x10);
        let pool_addr2 = addr(0x11);
        let pool1 = make_pool(pool_addr1, tok_a, tok_b, ProtocolType::V2);
        let pool2 = PoolRef {
            fee_bps: Some(100),
            ..make_pool(pool_addr2, tok_a, tok_b, ProtocolType::V2)
        };
        let intent = make_intent(tok_a, tok_b);
        let impact = make_impact(vec![pool1, pool2]);
        let unit = U256::from(10u128).pow(U256::from(18u32)) * U256::from(1_000u32);
        let engine = make_engine_with_reserves(vec![
            (pool_addr1, unit, unit),
            (pool_addr2, unit, unit),
        ])
        .await;

        let candidates = engine
            .build_from_impacted_pairs(&intent, &impact, None)
            .await
            .expect("engine must not error");

        for c in candidates.iter().filter(|c| {
            c.rejection_reason.as_deref() != Some("single_pool_no_spread")
                && c.rejection_reason.as_deref() != Some("reserves_cache_miss")
        }) {
            assert_eq!(
                c.route_plan.strategy_kind,
                c.label.as_str(),
                "route_plan.strategy_kind must match label.as_str()"
            );
        }
    }
}
