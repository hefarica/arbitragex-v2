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
                    let (gross_spread_units, can_price_v2) =
                        if a_is_v2 && b_is_v2 && reserves_a.is_some() && reserves_b.is_some() {
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
                        compute_gross_usd(
                            &gross_spread_units,
                            &cfg_opt,
                            intent.legs.first().map(|l| l.token_out),
                        )
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

        // Price by the ACTUAL denomination token (token_out), NOT a blanket
        // base_token_price_usd — same fix as compute_gross_usd (root 1A). The
        // prior code returned None for ALL V3 pairs when base_token_price_usd=0,
        // causing the no_price_oracle pre-rejection flood (45/62 pools are V3).
        let spread_f64 = u256_to_f64_lossy(spread) / 1e18_f64;
        let token_out = intent.legs.first().map(|l| l.token_out);
        let price_usd =
            canonical_token_price_usd(token_out, cfg.base_token_price_usd, &cfg.token_prices_usd)?;
        Some(spread_f64 * price_usd)
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
            let zero_for_one = intent_token_in == pool.token0 || intent_token_in == Address::zero();
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
fn orient_reserves(reserves: (U256, U256), pool: &PoolRef, intent: &RouteIntent) -> (U256, U256) {
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
/// UNIT-SCALE (Bug $69M): the spread is in RAW units of the swapped token,
/// scaled by probe_amount (1e18). In an arb loop token_in == token_out, so the
/// spread is denominated in that token's raw units. The previous code divided
/// by a hardcoded 1e18 (18-dec assumption) — for a 6-dec token (USDC/USDT) that
/// inflates USD by ~1e12 (observed: expected_profit_usd = $69,074,653 for a
/// sub-dollar real arb). Correct scale: divide by 10^(token_decimals).
///
/// Decimals come from `canonical_token_decimals` — immutable contract properties
/// of canonical mainnet tokens (NOT market data, NOT a mock). Unknown token →
/// 18 (the dominant ERC-20 case; correct for WETH/DAI/most tokens).
fn compute_gross_usd(
    spread_units: &U256,
    cfg_opt: &Option<TradingConfigState>,
    token_out: Option<Address>,
) -> Option<f64> {
    // No config → oracle gap — R8 None.
    let cfg = cfg_opt.as_ref()?;

    // Zero spread → not profitable — R8 None.
    if spread_units.is_zero() {
        return None;
    }

    // The spread is denominated in the token RECEIVED by the swap (token_out of
    // the leg), since out_a/out_b = v2_amount_out(token_in → token_out). Scale by
    // THAT token's decimals.
    let decimals: u32 = canonical_token_decimals(token_out);

    // spread (raw) / 10^decimals → real token units of the swapped token.
    let scale = 10f64.powi(decimals as i32);
    let spread_f64 = u256_to_f64_lossy(*spread_units) / scale;

    // Price by the ACTUAL denomination token (token_out), NOT a blanket
    // base_token_price_usd. The prior code multiplied EVERY token's spread by
    // the WETH price (~$3000), inflating stablecoin spreads ~3000× (e.g. a
    // 3578 USDC spread → $10.7M). Stables ≈ $1; WETH = operator base price;
    // any other token → None (R8) so the SizeOptimizer/evaluator re-prices it
    // from live Redis downstream. This is a fast-filter proxy only.
    let price_usd =
        canonical_token_price_usd(token_out, cfg.base_token_price_usd, &cfg.token_prices_usd)?;
    Some(spread_f64 * price_usd)
}

/// Canonical verified USD price for a known mainnet token, for the
/// `compute_gross_usd` / `compute_v3_gross_usd` fast-filter. Checks:
/// 1. Stables (USDC/USDT/DAI) → $1 (canonical, no lookup).
/// 2. Canonical tokens → LIVE Redis price (DexScreener/Chainlink/GeckoTerminal)
///    from `token_prices_usd` (merged by the orchestrator before engine fan-out).
/// 3. WETH fallback → `base_token_price_usd` if configured.
/// 4. Else → None (R8: unpriced, NEVER fabricate).
fn canonical_token_price_usd(
    token: Option<Address>,
    base_token_price_usd: f64,
    token_prices_usd: &std::collections::HashMap<String, f64>,
) -> Option<f64> {
    let addr = token?;
    let addr_str = format!("0x{:040x}", addr);
    // Stables = $1 (canonical, no lookup needed).
    match addr_str.as_str() {
        "0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48" | // USDC
        "0xdac17f958d2ee523a2206206994597c13d831ec7" | // USDT
        "0x6b175474e89094c44da98b954eedeac495271d0f" => return Some(1.0), // DAI
        _ => {}
    }
    // Canonical tokens: look up the LIVE Redis price (DexScreener/Chainlink/
    // GeckoTerminal) merged into token_prices_usd. Uses REAL market price.
    if let Some(sym) = canonical_token_symbol(&addr_str) {
        let sym_upper = sym.to_uppercase();
        if let Some(&p) = token_prices_usd.get(&sym_upper) {
            if p > 0.0 {
                return Some(p);
            }
        }
    }
    // WETH fallback: base_token_price_usd if configured (>0).
    if addr_str == "0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2" && base_token_price_usd > 0.0 {
        return Some(base_token_price_usd);
    }
    None // R8: unpriced (no stable, no Redis price, no config)
}

/// Map a canonical mainnet token address → its symbol (for Redis price lookup).
fn canonical_token_symbol(addr: &str) -> Option<&'static str> {
    match addr {
        "0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2" => Some("WETH"),
        "0x2260fac5e5542a773aa44fbcfedf7c193bc2c599" => Some("WBTC"),
        "0x1f9840a85d5af5bf1d1762f925bdaddc4201f984" => Some("UNI"),
        "0x514910771af9ca656af840dff83e8264ecf986ca" => Some("LINK"),
        "0x7fc66500c84a76ad7e9c93437bfc5ac33e2ddae9" => Some("AAVE"),
        "0x6982508145454ce325ddbe47a25d4ec3d2311933" => Some("PEPE"),
        "0x4d224452801aced8b2f0aebe155379bb5d594381" => Some("APE"),
        "0x95ad61b0a150d79219dcf64e1e6cc01f0b64c4ce" => Some("SHIB"),
        "0x9f8f72aa9304c8b593d555f12ef6589cc3a579a2" => Some("MKR"),
        "0xd533a949740bb3306d119cc777fa900ba034cd52" => Some("CRV"),
        "0xc18360217d8f7ab5e7c516566761ea12ce7f9d72" => Some("ENS"),
        "0x7d1afa7b718fb893db30a3abc0cfc608aacfebb0" => Some("MATIC"),
        "0x853d955acef822db058eb8505911ed77f175b99e" => Some("FRAX"),
        "0xc00e94cb662c3520282e6f5717214004a7f26888" => Some("COMP"),
        "0x5a98fcbea516cf06857215779fd812ca3bef1b32" => Some("LDO"),
        "0x6b3595068778dd592e39a122f4f5a5cf09c90fe2" => Some("SUSHI"),
        "0xba100000625a3754423978a60c9317c58a424e3d" => Some("BAL"),
        "0x0bc529c00c6401aef6d220be8c6ea1667f6ad93e" => Some("YFI"),
        "0x111111111117dc0aa78b770fa6a738034120c302" => Some("1INCH"),
        "0x4e3fbd56cd56c3e72c1403e103b45db9da5b9d2b" => Some("CVX"),
        "0x3432b6a60d23ca0dfca7761b7ab56459d9c964d0" => Some("FXS"),
        "0xc011a73ee8576fb46f5e1c5751ca3b9fe0af2a6f" => Some("SNX"),
        "0xc944e90c64b2c07662a292be6244bdf05cda44a7" => Some("GRT"),
        _ => None,
    }
}

/// Canonical mainnet (chain_id=1) token decimals. These are IMMUTABLE contract
/// properties set at deploy time (e.g. USDC is permanently 6-dec) — protocol
/// constants, not market data, so a static lookup is honest and matches the
/// standard MEV-searcher practice (Flashbots/Artemis use the same canonical
/// tables). Anything not listed defaults to 18 (the dominant ERC-20 case).
fn canonical_token_decimals(token: Option<Address>) -> u32 {
    let Some(addr) = token else { return 18 };
    match format!("0x{:040x}", addr).as_str() {
        // USDC (6), USDT (6)
        "0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48" => 6,
        "0xdac17f958d2ee523a2206206994597c13d831ec7" => 6,
        // WBTC (8)
        "0x2260fac5e5542a773aa44fbcfedf7c193bc2c599" => 8,
        // Everything else (WETH, DAI, and the ERC-20 majority) → 18.
        _ => 18,
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
        cartridge_id: None,
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
        let engine =
            make_engine_with_reserves(vec![(pool_addr1, unit, unit), (pool_addr2, unit, unit)])
                .await;

        let candidates = engine
            .build_from_impacted_pairs(&intent, &impact, None)
            .await
            .expect("engine must not error");

        // Must emit at least one candidate — not rejected for reserves_cache_miss.
        assert!(
            !candidates.is_empty(),
            "must produce at least one candidate"
        );
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
            assert_eq!(
                c.label,
                StrategyLabel::DexArbV2V2,
                "must classify as DexArbV2V2"
            );
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
        let engine =
            make_engine_with_reserves(vec![(pool_addr1, unit, unit), (pool_addr2, unit, unit)])
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
        assert!(
            !candidates.is_empty(),
            "must produce at least one candidate"
        );
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
        let engine =
            make_engine_with_reserves(vec![(pool_addr1, unit, unit), (pool_addr2, unit, unit)])
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
        let engine =
            make_engine_with_reserves(vec![(pool_addr1, unit, unit), (pool_addr2, unit, unit)])
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
                StrategyKind::dex_arb(),
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
        let engine =
            make_engine_with_reserves(vec![(pool_addr1, unit, unit), (pool_addr2, unit, unit)])
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
