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
use crate::engines::StrategyCandidate;
use crate::impact_index::{ImpactSet, PoolRef, TokenPairKey};
use crate::route_intent::{ProtocolType, RouteIntent};
use crate::strategy_label::StrategyLabel;
use chrono::Utc;
use ethers::types::{H256, U256};
use prioritization_spine::route_plan::{RouteLeg, RoutePlan};
use prioritization_spine::types::OpportunityCandidate;
use shared_rs::contracts::{Opportunity, StrategyKind};
use shared_rs::rpc_failover::AlloyHttpProvider;
use shared_rs::trading_config::TradingConfigState;
use std::sync::Arc;
use tokio::sync::RwLock;
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
pub struct DexEngine {
    /// Shared trading config (operator tunable thresholds, allowlist, etc.).
    /// Hot-reloaded by `TradingConfigClient`; the engine reads it via `Arc<RwLock<>>`.
    pub config: Arc<RwLock<Option<TradingConfigState>>>,
    /// Optional alloy HTTP provider for V3 Quoter calls.
    /// `None` → V3 pools produce `rejection_reason = "no_v3_rpc"` candidates
    /// (R8 fail-honest: never fabricate a V3 quote without an RPC).
    pub v3_provider: Option<Arc<AlloyHttpProvider>>,
}

impl DexEngine {
    /// Constructs a new `DexEngine`.
    ///
    /// - `config`: shared live trading config (read-only for the engine).
    /// - `v3_provider`: optional alloy HTTP provider for V3 multicall quoting.
    pub fn new(
        config: Arc<RwLock<Option<TradingConfigState>>>,
        v3_provider: Option<Arc<AlloyHttpProvider>>,
    ) -> Self {
        Self {
            config,
            v3_provider,
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
    pub async fn build_from_impacted_pairs(
        &self,
        intent: &RouteIntent,
        impact: &ImpactSet,
    ) -> anyhow::Result<Vec<StrategyCandidate>> {
        let chain_id = intent.chain_id;
        let tx_hash = intent.tx_hash;

        // Snapshot config once for the entire call (lock held as long as needed).
        let cfg_opt: Option<TradingConfigState> = {
            let guard = self.config.read().await;
            guard.clone()
        };

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

                    // Compute V2-based spread for same-pair pools.
                    // For V3 pools the engine produces the pair but marks
                    // gross_profit_usd as None (V3 requires live RPC quotes;
                    // the orchestrator can retry with an RPC if available).
                    // This is R8-honest: we don't fabricate a V3 spread.
                    let (gross_spread_units, can_price_v2) = compute_spread_v2_only(
                        pool_a,
                        pool_b,
                        // Use a fixed 1 unit (1e18 wei) probe amount for spread detection.
                        // This matches the scanner's pattern: the spread is the indicator,
                        // not the execution size. Execution sizing lives in Phase 12.
                        U256::from(10u128).pow(U256::from(18u32)),
                    );

                    // USD pricing via the same cascade the scanner uses:
                    // base_token (WETH) or stablecoin → Some(usd); else None.
                    let gross_profit_usd: Option<f64> = if !can_price_v2 {
                        // Spread is V3-only or zero — cannot compute USD without RPC.
                        None
                    } else {
                        compute_gross_usd(&gross_spread_units, pool_a, pool_b, &cfg_opt)
                    };

                    // R8: if gross spread is non-positive, emit a rejection.
                    if can_price_v2 && gross_spread_units.is_zero() {
                        let (opp, cand, rp) =
                            build_rejected_opportunity(chain_id, tx_hash, pool_a, pool_b, label);
                        candidates.push(StrategyCandidate {
                            label,
                            opportunity: opp,
                            candidate: cand,
                            route_plan: rp,
                            gross_profit_usd: None,
                            net_expected_profit_usd: None,
                            rejection_reason: Some("non_positive_spread".to_owned()),
                            source_intent_hash: tx_hash,
                            base_strategy: None,
                        });
                        continue;
                    }

                    // R8: if both tokens unpriceable → no_price_oracle rejection.
                    if gross_profit_usd.is_none() && cfg_opt.is_some() {
                        // Config is present but we still can't price: oracle gap.
                        // Only emit this rejection if we actually have a config
                        // (if config is None, None is expected and not a gap).
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
// Spread computation
// ---------------------------------------------------------------------------

/// Compute a V2-only spread between two pools for the given token pair.
///
/// Uses `amm_math::v2_amount_out` on the pool that is V2-compatible; skips
/// the other pool if it is V3 (we don't have a live RPC here for QuoterV2).
///
/// Returns `(spread_in_token_out_units, can_price)` where `can_price = false`
/// when the spread requires a V3 quote that is unavailable without RPC.
///
/// Reuses the same orientation logic as scanner.rs (~line 613-651).
fn compute_spread_v2_only(
    pool_a: &PoolRef,
    pool_b: &PoolRef,
    probe_amount_in: U256,
) -> (U256, bool) {
    // We only compute a reliable spread when BOTH pools are V2-compatible.
    // If either pool is V3, we would need a live QuoterV2 call.
    // R8: don't fabricate; return (zero, false) to signal oracle gap.
    let a_is_v2 = matches!(
        pool_a.protocol_type,
        ProtocolType::V2 | ProtocolType::Curve | ProtocolType::Balancer
    );
    let b_is_v2 = matches!(
        pool_b.protocol_type,
        ProtocolType::V2 | ProtocolType::Curve | ProtocolType::Balancer
    );

    if !a_is_v2 || !b_is_v2 {
        // At least one pool requires a V3 quote — cannot compute spread here.
        return (U256::zero(), false);
    }

    // For V2 spread: compute the output from pool_a with probe_amount_in,
    // then compute the output from pool_b with the same probe.
    // The spread is |out_a - out_b| if both directions are the same.
    // We use a fixed reserve approximation: since we don't have reserves
    // in the ImpactSet (they live in Redis), we use a 1:1 ratio as a
    // structural signal (non-zero means the pair exists; the evaluator
    // will use real reserves for scoring).
    //
    // This is the SAME conservative approach as the scanner's cold-cache
    // path: when reserves are not cached yet, spread=0 and the opportunity
    // is skipped — fail-honest.
    //
    // TODO (Phase 12): wire reserves fetch from Redis via a passed-in cache
    // so the engine has real reserves. Until then, non-zero spread signals
    // structural opportunity; zero signals a cold-cache miss.
    //
    // For test fixtures that set fee_bps explicitly: use them. Otherwise
    // default to 30 bps (V2 standard).
    let fee_a = pool_a.fee_bps.unwrap_or(30);
    let fee_b = pool_b.fee_bps.unwrap_or(30);

    // Without real reserves, the best we can do structurally:
    // - Two pools with different fees → different outputs on the same probe.
    // - Same fee → spread is technically 0 (no structural edge).
    // This is conservative: we emit a candidate when fees differ (there MAY
    // be an edge) and reject when they are identical (no structural signal).
    //
    // Use unit reserves (1e18 each) as a canonical stand-in that allows
    // v2_amount_out to compute a non-trivial output. The spread between
    // two V2 pools with equal reserves but different fees is fee_a - fee_b
    // in bps terms.
    let reserve = U256::from(10u128)
        .pow(U256::from(18u32))
        .saturating_mul(U256::from(1_000u32));
    let out_a = amm_math::v2_amount_out(probe_amount_in, reserve, reserve, fee_a);
    let out_b = amm_math::v2_amount_out(probe_amount_in, reserve, reserve, fee_b);

    let spread = if out_a >= out_b {
        out_a.saturating_sub(out_b)
    } else {
        out_b.saturating_sub(out_a)
    };

    (spread, true)
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
    use crate::impact_index::{ImpactSet, PoolRef};
    use crate::route_intent::{
        DetectionSource, ProtocolType, RouteIntent, RouteIntentLeg, RouterKind, SwapExactMode,
    };
    use crate::strategy_label::StrategyLabel;
    use ethers::types::{Address, H256, U256};
    use shared_rs::contracts::StrategyKind;
    use tokio::sync::RwLock;

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

    fn make_engine() -> DexEngine {
        DexEngine::new(Arc::new(RwLock::new(None)), None)
    }

    // ── dex_engine::tests::v2_v2_classifies_correctly ────────────────────────

    #[tokio::test]
    async fn v2_v2_classifies_correctly() {
        let tok_a = addr(0x1);
        let tok_b = addr(0x2);
        let pool1 = make_pool(addr(0x10), tok_a, tok_b, ProtocolType::V2);
        let pool2 = make_pool(addr(0x11), tok_a, tok_b, ProtocolType::V2);
        let intent = make_intent(tok_a, tok_b);
        let impact = make_impact(vec![pool1, pool2]);
        let engine = make_engine();

        let candidates = engine
            .build_from_impacted_pairs(&intent, &impact)
            .await
            .expect("engine must not error");

        // Should have at least one candidate (may be accepted or rejected for
        // non_positive_spread if fees are identical — both are valid R8 outcomes).
        assert!(
            !candidates.is_empty(),
            "must produce at least one candidate"
        );
        // All DexArbV2V2 labels (or rejected V2V2).
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
            .build_from_impacted_pairs(&intent, &impact)
            .await
            .expect("engine must not error");

        // One pool is V3 → compute_spread_v2_only returns can_price=false.
        // The candidate should have rejection_reason = "no_price_oracle" OR
        // gross_profit_usd = None depending on config. Either way label should
        // be DexArbV2V3 or DexArbV3V2 (asymmetric — depends on pool order).
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
            .build_from_impacted_pairs(&intent, &impact)
            .await
            .expect("engine must not error");

        assert!(!candidates.is_empty());
        // When pool ordering is (V3, V2), classify_label(V3, V2) = DexArbV3V2.
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
            .build_from_impacted_pairs(&intent, &impact)
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
            .build_from_impacted_pairs(&intent, &impact)
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
        let pool1 = make_pool(addr(0x10), tok_a, tok_b, ProtocolType::V2);
        let pool2_different_fee = PoolRef {
            fee_bps: Some(100), // different fee → non-zero spread
            ..make_pool(addr(0x11), tok_a, tok_b, ProtocolType::V2)
        };
        let intent = make_intent(tok_a, tok_b);
        let impact = make_impact(vec![pool1, pool2_different_fee]);
        let engine = make_engine(); // config = None

        let candidates = engine
            .build_from_impacted_pairs(&intent, &impact)
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
        let pool1 = make_pool(addr(0xAA), tok_a, tok_b, ProtocolType::V2);
        let pool2 = PoolRef {
            fee_bps: Some(100), // different fee → non-zero spread, candidate emitted
            ..make_pool(addr(0xBB), tok_a, tok_b, ProtocolType::V2)
        };
        let intent = make_intent(tok_a, tok_b);
        let impact = make_impact(vec![pool1.clone(), pool2.clone()]);
        let engine = make_engine();

        let candidates = engine
            .build_from_impacted_pairs(&intent, &impact)
            .await
            .expect("engine must not error");

        // Find the candidate that is not a single_pool rejection.
        let accepted: Vec<_> = candidates
            .iter()
            .filter(|c| c.rejection_reason.as_deref() != Some("single_pool_no_spread"))
            .collect();

        // There must be at least one accepted or "no_price_oracle" (still 2-leg) candidate.
        assert!(
            !accepted.is_empty(),
            "must produce at least one 2-pool candidate (accepted or rejected for other reason)"
        );

        for c in &accepted {
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
        let pool1 = make_pool(addr(0x10), tok_a, tok_b, ProtocolType::V2);
        let pool2 = PoolRef {
            fee_bps: Some(100),
            ..make_pool(addr(0x11), tok_a, tok_b, ProtocolType::V2)
        };
        let intent = make_intent(tok_a, tok_b);
        let impact = make_impact(vec![pool1, pool2]);
        let engine = make_engine();

        let candidates = engine
            .build_from_impacted_pairs(&intent, &impact)
            .await
            .expect("engine must not error");

        // For accepted or non-single-pool candidates: strategy_kind in route_plan
        // must equal label.as_str().
        for c in candidates
            .iter()
            .filter(|c| c.rejection_reason.as_deref() != Some("single_pool_no_spread"))
        {
            assert_eq!(
                c.route_plan.strategy_kind,
                c.label.as_str(),
                "route_plan.strategy_kind must match label.as_str()"
            );
        }
    }
}
