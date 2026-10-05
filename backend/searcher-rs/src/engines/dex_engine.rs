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
//! - `net_expected_profit_usd = None` at this phase (evaluator fills later) —
//!   EXCEPT on the measured-gross rejection path (ALWAYS-COMPUTE-03): when the
//!   gross IS measured the row carries the CLOSED `computed` economics
//!   (`amount_in`, gross, real costs, net, roi), so the net is the kernel-grade
//!   figure instead of a gap.
//! - `rejection_reason` is always `Some(...)` for rejected candidates.
//! - `pool_address` on `RouteLeg` is always `Some(...)` (we know the address
//!   from `PoolRef`).
//!
//! ## Rejection labels
//!
//! | reason                    | meaning                                     |
//! |---------------------------|---------------------------------------------|
//! | `single_pool_no_spread`   | only one pool in `ImpactSet` for this pair  |
//! | `no_price_oracle`         | token_out has NO canonical USD price (the   |
//! |                           | genuine price miss — G-ECON-1 narrowed it)  |
//! | `v3_quote_unavailable`    | V3 projector missing / quote failed on a    |
//! |                           | leg / both legs quoted zero (G-ECON-1)      |
//! | `v3_pool_not_catalogued`  | pool absent from the V3 fee catalog —       |
//! |                           | zero-RPC catalog gap (CATALOG-BACKFILL-01;  |
//! |                           | previously flattened into the line above)   |
//! | `v3_pair_no_pools`        | pair has no known V3 pools at all —         |
//! |                           | zero-RPC (CATALOG-BACKFILL-01; ditto)       |
//! | `spread_zero_equilibrium` | the CHAINED round trip returned EXACTLY the |
//! |                           | probe — a true equilibrium, not a data gap  |
//! |                           | (SPREAD-SIGNED-DELTA-01 corrected this text: |
//! |                           | it claimed "both legs quoted identical       |
//! |                           | amounts", which the code never evaluated)   |
//! | `spread_negative_round_trip` | the CHAINED round trip returned LESS than |
//! |                           | the probe — a MEASURED loss, published as a |
//! |                           | NEGATIVE gross. Before SPREAD-SIGNED-       |
//! |                           | DELTA-01 `saturating_sub` erased it into    |
//! |                           | the equilibrium zero above                  |
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
use shared_rs::chains::{DAI_MAINNET_LC, USDC_MAINNET_LC, USDT_MAINNET_LC};
use shared_rs::contracts::{Opportunity, StrategyKind};
use shared_rs::rpc_failover::AlloyHttpProvider;
use shared_rs::trading_config::TradingConfigState;
use std::sync::Arc;
use tracing::debug;
use uuid::Uuid;

// ---------------------------------------------------------------------------
// DexEngine
// ---------------------------------------------------------------------------

/// G-ECON-1 — classified outcome of the V3 gross-USD computation, replacing
/// the bare `Option<f64>` whose `None` collapsed three distinct failure modes
/// under the misleading `no_price_oracle` label.
#[derive(Debug, Clone, Copy)]
pub(crate) enum V3GrossOutcome {
    /// Computed gross spread in USD.
    Usd(f64),
    /// Projector missing, quote failed on either leg, or both legs quoted
    /// zero — the quote pipeline produced nothing usable.
    QuoteUnavailable,
    /// CATALOG-BACKFILL-01 (2026-09-17): a V3 leg was rejected by the
    /// projector with its PRECISE label (`ProjectV3Error::as_label()` —
    /// `v3_pool_not_catalogued` / `v3_pair_no_pools` / `v3_quote_unavailable`).
    /// Carried verbatim so the emitted `rejection_reason` never collapses a
    /// catalog gap into the old transport-looking `v3_quote_unavailable`
    /// (which hid not_catalogued=23% of resolutions from the operator).
    V3Labeled(&'static str),
    /// The CHAINED round trip returned EXACTLY the probe — the one market state
    /// that is a true equilibrium (no capturable spread at the probed size after
    /// both venues' fees and impact). The emitter publishes it as a MEASURED zero
    /// (`Some(0.0)`), which is different from "not computed" (`None`).
    ///
    /// SPREAD-SIGNED-DELTA-01 (2026-10-04): this variant's doc used to claim
    /// "both legs quoted IDENTICAL amounts out" — a condition this code never
    /// evaluated. `out_a`/`out_b` only pick the orientation of the chained round
    /// trip, and two DIFFERENT forward quotes can still return exactly the probe
    /// back. A round trip that returns LESS is `RoundTripLoss`, not this.
    SpreadZeroEquilibrium,
    /// SPREAD-SIGNED-DELTA-01 (2026-10-04): the CHAINED round trip returned LESS
    /// than the probe — a real, MEASURED loss, never an equilibrium.
    ///
    /// Carries the measured NEGATIVE gross (USD, priced at token_in's own live
    /// price) and the REAL amount the return leg delivered, so (a) a loss is
    /// never dressed as a measured zero (R8) and (b) the row can publish the
    /// cycle's real output instead of the derived `amount_in + gross` stand-in.
    ///
    /// This is the NORMAL state of a majors round trip at the probed size —
    /// measured by SPREAD-ZERO-01: 60 bps of round-trip cost for WETH/DAI
    /// (SushiSwap 30 bps + UniswapV3 3000 pips) against 1-10 bps venue gaps — and
    /// `saturating_sub` used to erase it into `SpreadZeroEquilibrium`.
    RoundTripLoss { gross_usd: f64, returned_wei: U256 },
    /// The token_out has no canonical USD price — the one case that truly
    /// deserves the `no_price_oracle` label.
    NoTokenPrice,
    /// No trading config snapshot (V2-only path never consults the projector).
    NoConfig,
    /// The pair was priceable via the V2 reserves path — projector not needed.
    Skipped,
}

/// CATALOG-BACKFILL-01 (2026-09-17): per-leg quote failure classification —
/// the V3 projector's precise label survives the leg boundary instead of being
/// flattened into a bare `Option::None`.
#[derive(Debug, Clone, Copy)]
enum V3QuoteLegError {
    /// V2 leg reserves cache miss — no distinct catalog label at this altitude.
    ReservesMiss,
    /// V3 projector rejection with its precise rejection label.
    V3(&'static str),
}

impl V3GrossOutcome {
    /// `Some(usd)` when computed, `None` otherwise (callers keep their
    /// Option-shaped gross plumbing).
    fn usd(&self) -> Option<f64> {
        match self {
            V3GrossOutcome::Usd(v) => Some(*v),
            // SPREAD-SIGNED-DELTA-01: `RoundTripLoss` is ALSO measured, but it is
            // not a gross PROFIT — staying `None` here is what keeps a losing
            // cycle on the rejection path instead of promoting it to an accepted
            // candidate. Its measured magnitude is published through the
            // emitter's `computed_gross` mapping, not through a "profit".
            _ => None,
        }
    }
}

/// SPREAD-SIGNED-DELTA-01: the REAL output of the chained cycle, in `token_in`
/// raw units, for the two verdicts that MEASURED it:
///   * the equilibrium — `returned == probe_amount` by the very equality the
///     verdict tests, so the output IS the probe;
///   * the loss — the verdict carries the amount the return leg delivered.
///
/// `None` for every other verdict: their cycle output was never exposed, and R8
/// forbids filling it with a derived stand-in. `economics.rs` derives
/// `amount_out_usd = amount_in_usd + gross` (economics.rs:211), which is exactly
/// equal to the input whenever the gross is zero — that stand-in is why the
/// channel could not tell a loss from an equilibrium.
fn measured_cycle_output(outcome: &V3GrossOutcome, probe_amount: U256) -> Option<U256> {
    match outcome {
        V3GrossOutcome::SpreadZeroEquilibrium => Some(probe_amount),
        V3GrossOutcome::RoundTripLoss { returned_wei, .. } => Some(*returned_wei),
        _ => None,
    }
}

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
    /// - `net_expected_profit_usd = None` (evaluator fills later) — except on the
    ///   measured-gross rejection path, which carries the closed economics
    ///   (ALWAYS-COMPUTE-03, see the module header).
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
                        intent.observed_block(),
                        intent,
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

            // B1 (V3-QUOTE-BATCH-20260919): prefetch every V3 pool of this
            // pair-group in batched aggregate3 multicalls BEFORE the probing
            // loop. The per-pair `get_pool_quote` below then answers from the
            // warm TTL cache — one eth_call per ~100 pools instead of one per
            // pool, which is what kept opening the public RPC circuit breakers
            // (78% of the funnel rejected as v3_quote_unavailable).
            if let Some(projector) = self.state_projector.as_ref() {
                // B1 FIX (math-audit AUDIT-MATH-OPPS-2026-09-26): the probe must be
                // ONE NATIVE UNIT of token_in (10^decimals), never a fixed 1e18.
                // For a 6-dec token (USDC/USDT) the old 1e18 probe meant 1e12
                // tokens — $999,935,091,316.80 at the live price — so every
                // gross/USD number for those pairs was computed at an operating
                // point that saturates any pool (and the card showed the $1T
                // notional). canonical_token_decimals is the same immutable
                // protocol table the USD conversion below already uses; unknown
                // tokens keep the 18-dec default (previous behaviour).
                let probe_amount = U256::from(10u128).pow(U256::from(canonical_token_decimals(
                    intent.legs.first().map(|l| l.token_in),
                )));
                let v3_pools: Vec<crate::state_projector::PoolRef> = pools
                    .iter()
                    .filter(|p| matches!(p.protocol_type, ProtocolType::V3))
                    .map(|p| crate::state_projector::PoolRef {
                        address: p.address,
                        token0: p.token0,
                        token1: p.token1,
                        fee_bps: p.fee_bps,
                    })
                    .collect();
                if !v3_pools.is_empty() {
                    let intent_token_in =
                        intent.legs.first().map(|l| l.token_in).unwrap_or_default();
                    projector
                        .prefetch_v3_quotes(&v3_pools, probe_amount, intent_token_in)
                        .await;
                }
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
                    // B1 FIX (math-audit 2026-09-26): ONE NATIVE UNIT of token_in
                    // (10^decimals) — see the prefetch site above for the full
                    // rationale; the fixed 1e18 was a $1T notional for 6-dec tokens.
                    //
                    // ECON-AMOUNT-DENOM-01 (2026-10-04): the denomination is the
                    // token this row PUBLISHES, resolved by the SAME rule
                    // `build_rejected_opportunity` / `build_accepted_opportunity` use
                    // for `token_in`/`token_out` (`economic_base_token`): the intent's
                    // entry token when it belongs to this pair, else the pair's
                    // canonical base. It denominates the probe principal below AND
                    // resolves the live USD price for the measured-gross economics.
                    //
                    // Why not the raw intent token (the previous code): the probe was
                    // therefore denominated in a token the pair may not even contain,
                    // while `route_metadata.economics_amount_in_wei` is written in the
                    // DECLARED base token's unit. MEASURED on live production
                    // (2026-10-04, `spread_zero_equilibrium`, 1 h): the two fields of
                    // the SAME row disagreed —
                    //   `economics.amount_in_wei`              = 1000000000000000000 (constant)
                    //   `route_metadata.economics_amount_in_wei` = 1000000
                    // — with ONE base token (USDC, 6 decimals) behind 10,167 rows, the
                    // probe identical (1e18) across all 13 cycle families, and the card
                    // showing $2,693.71 where the real probe is 1 USDC = $1.00 (the
                    // 2693.71 is the intent token's own price). `canonical_token_decimals`
                    // is decimals-aware; it was simply being asked about the wrong token.
                    let token_in_opt = Some(economic_base_token(pool_a, intent));
                    let probe_amount =
                        U256::from(10u128).pow(U256::from(canonical_token_decimals(token_in_opt)));

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
                        let (opp, cand, rp) = build_rejected_opportunity(
                            chain_id,
                            tx_hash,
                            pool_a,
                            pool_b,
                            label,
                            intent.observed_block(),
                            intent,
                        );
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
                            // B3 FIX (math-audit AUDIT-MATH-OPPS-2026-09-26): a real
                            // arb is a CHAINED cycle, not the difference of two
                            // independent probes. The old `|out_a − out_b|` ignored
                            // that the second swap receives the FIRST swap's output
                            // (paying both fees AND the pool's own price impact), and
                            // it reported a POSITIVE gross on routes the kernel sized
                            // as negative — a sign discrepancy against the router.
                            // The cycle: buy where token_in→token_out is better, sell
                            // the proceeds back into the other pool, and measure the
                            // RETURN in token_in units: profit = final − probe.
                            // B3 GATE: the chained math lives in a pure helper
                            // (v2_cycle_profit) so the phantom-positive case is
                            // unit-testable without an engine harness.
                            let profit = v2_cycle_profit(
                                probe_amount,
                                r_in_a,
                                r_out_a,
                                fee_a,
                                r_in_b,
                                r_out_b,
                                fee_b,
                            );
                            (profit, true)
                        } else {
                            // At least one pool is V3 — cannot compute spread here without projector.
                            (U256::zero(), false)
                        };

                    // For V3 paths: try to get a virtual quote via state_projector.
                    let v3_gross_usd = if !can_price_v2 {
                        self.compute_v3_gross_usd(pool_a, pool_b, probe_amount, &cfg_opt, intent)
                            .await
                    } else {
                        V3GrossOutcome::Skipped
                    };

                    // USD pricing: V2 cascade first, then V3 projector result.
                    let gross_profit_usd: Option<f64> = if can_price_v2 {
                        // B3 FIX (math-audit 2026-09-26): the V2/V2 value is now the
                        // CHAINED cycle's profit, denominated in token_in — the token
                        // the cycle returns to and the one the probe was measured in.
                        // Scale by token_in's decimals/price (passing token_out here
                        // would mis-scale by the wrong token's decimals).
                        compute_gross_usd(
                            &gross_spread_units,
                            &cfg_opt,
                            intent.legs.first().map(|l| l.token_in),
                        )
                    } else {
                        v3_gross_usd.usd()
                    };

                    // EMIT the candidate — the SizeOptimizer decides final profitability.
                    // We no longer pre-reject V2/V2 pairs with spread=0 at this point.
                    // A spread=0 with real reserves is an honest equilibrium market reading;
                    // the SizeOptimizer will reject with size_optimizer_no_profit if costs
                    // exceed gross profit. R8 fail-honest: emit honest data, don't reject
                    // prematurely based on a unit-reserves approximation.
                    //
                    // Exception: single-pool (handled above). V3 with no projector (below).

                    // R8 + G-ECON-1: reject with an HONEST, specific reason. The
                    // old blanket `no_price_oracle` label misclassified three
                    // different conditions (56% of ALL rejections in prod were
                    // WETH/USDT-class pairs that ARE priced — the actual causes
                    // were V3 quote failures and zero-spread equilibria).
                    if gross_profit_usd.is_none() && cfg_opt.is_some() && !can_price_v2 {
                        let reason = match v3_gross_usd {
                            V3GrossOutcome::QuoteUnavailable => "v3_quote_unavailable",
                            // CATALOG-BACKFILL-01: catalog gaps surface their
                            // OWN label — never the flattened transport string.
                            V3GrossOutcome::V3Labeled(label) => label,
                            V3GrossOutcome::SpreadZeroEquilibrium => "spread_zero_equilibrium",
                            // SPREAD-SIGNED-DELTA-01: a losing round trip is NOT
                            // an equilibrium. Its own label, so the channel stops
                            // reporting a measured loss as an "efficient market".
                            V3GrossOutcome::RoundTripLoss { .. } => "spread_negative_round_trip",
                            // The genuine token-price miss — keeps the original label.
                            V3GrossOutcome::NoTokenPrice => "no_price_oracle",
                            // Unreachable in this branch (Usd ⇒ gross Some; Skipped/NoConfig
                            // ⇒ !can_price_v2 false or cfg absent) — honest fallback.
                            _ => "no_price_oracle",
                        };
                        let (opp, cand, rp) = build_rejected_opportunity(
                            chain_id,
                            tx_hash,
                            pool_a,
                            pool_b,
                            label,
                            intent.observed_block(),
                            intent,
                        );
                        // ALWAYS-COMPUTE (orden del operador 2026-09-27: "todos
                        // sin excepcion deben tener sus calculos y el 100% de sus
                        // valores, independiente que den o no ganancia").
                        //
                        // Neither verdict is a missing datum: both are
                        // MEASUREMENTS on the chained round trip. The equilibrium
                        // is a computed-and-exactly-zero gross (`Some(0.0)`), and
                        // SPREAD-SIGNED-DELTA-01 adds the OTHER measured case — a
                        // loss — published as a NEGATIVE gross. Both differ from
                        // "not computed" (`None`), and both keep the row on the
                        // economics path so it shows its real arithmetic (measured
                        // costs, negative net) instead of a dash.
                        //
                        // A real loss is never rounded up to zero (R8): before this
                        // split `saturating_sub` made the two cases the SAME row,
                        // which is why the channel could not be audited.
                        let computed_gross: Option<f64> = match v3_gross_usd {
                            V3GrossOutcome::SpreadZeroEquilibrium => Some(0.0),
                            V3GrossOutcome::RoundTripLoss { gross_usd, .. } => Some(gross_usd),
                            _ => None,
                        };
                        let mut sc = StrategyCandidate {
                            label,
                            opportunity: opp,
                            candidate: cand,
                            route_plan: rp,
                            gross_profit_usd: computed_gross,
                            net_expected_profit_usd: None,
                            rejection_reason: Some(reason.to_owned()),
                            source_intent_hash: tx_hash,
                            base_strategy: None,
                        };
                        // ALWAYS-COMPUTE-03 (2026-09-27): publishing the measured
                        // gross was necessary but NOT sufficient — the emit
                        // boundary still stamped `economics_error(reason)` on this
                        // row (`computation_status: "error"`, all figures null)
                        // because the Opportunity carried no figures, so the card
                        // rendered a MEASURED row as "not computed". A row whose
                        // gross exists gets the CLOSED `computed` object here
                        // (amount_in, gross, REAL costs, net, roi), built by the
                        // shared economics builder from the same config oracle the
                        // sizing kernel reads.
                        //
                        // Trigger = the measured gross itself (`Some(..)`, even
                        // zero). Rows whose gross is `None` (no quote, catalog gap,
                        // unpriced token) keep their honest diagnostic untouched.
                        let measured_economics = computed_gross.and_then(|gross| {
                            let cfg = cfg_opt.as_ref()?;
                            let amount_in_usd =
                                probe_amount_in_usd(probe_amount, token_in_opt, cfg)?;
                            crate::economics::economics_from_measured_gross(
                                &sc,
                                probe_amount,
                                amount_in_usd,
                                gross,
                                cfg,
                            )
                        });
                        if let Some(econ) = measured_economics {
                            // The row's scalar fields are copied from the ONE
                            // closed object, so the card, PG and the payload
                            // cannot disagree; `amount_in_wei` records the
                            // principal the gross was actually measured at (the
                            // probe), replacing the 1e18 placeholder that
                            // mis-scales every non-18-dec entry token.
                            sc.opportunity.expected_profit_usd = econ.gross_profit_usd;
                            sc.opportunity.net_expected_profit_usd = econ.net_profit_usd;
                            sc.opportunity.amount_in_wei = probe_amount.to_string();
                            sc.opportunity.economics = Some(econ);
                        }
                        // SPREAD-SIGNED-DELTA-01 (D2): publish the cycle's REAL
                        // output. Until now it was never exposed — the row carried
                        // `amount_out_wei: null` with reason
                        // `cycle_output_not_exposed_by_kernel` while
                        // `economics.amount_out_usd` was DERIVED as
                        // `amount_in_usd + gross` (economics.rs:211), exactly equal
                        // to the input whenever the gross is zero. A derived figure
                        // that looks measured is how a loss passed as an
                        // equilibrium. The two verdicts that MEASURED the output
                        // publish it here, in `token_in` token units — the
                        // convention `OpportunityCandidate` documents; every other
                        // verdict leaves it `NaN` (not computed), never a stand-in.
                        if let Some(out_wei) = measured_cycle_output(&v3_gross_usd, probe_amount) {
                            let decimals = canonical_token_decimals(token_in_opt) as u8;
                            sc.candidate.expected_amount_out =
                                wei_to_token_units(out_wei, decimals);
                        }
                        candidates.push(sc);
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
                        intent.observed_block(),
                        intent,
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
    ///   - Convert to USD via the token_out canonical price.
    ///
    /// G-ECON-1: returns a classified outcome instead of a bare `Option` so the
    /// rejection reason is HONEST about which of the three distinct failure
    /// modes fired (the old blanket `no_price_oracle` mislabeled quote failures
    /// and zero-spread equilibria as missing prices — 56% of all prod
    /// rejections, dominated by WETH/USDT pairs that ARE priced).
    async fn compute_v3_gross_usd(
        &self,
        pool_a: &PoolRef,
        pool_b: &PoolRef,
        probe_amount: U256,
        cfg_opt: &Option<TradingConfigState>,
        intent: &RouteIntent,
    ) -> V3GrossOutcome {
        let Some(projector) = self.state_projector.as_ref() else {
            return V3GrossOutcome::QuoteUnavailable;
        };
        let Some(cfg) = cfg_opt.as_ref() else {
            return V3GrossOutcome::NoConfig;
        };

        // For each V3 pool, get a virtual quote using project_v3_quote.
        // V2 pools: use v2_amount_out with canonical unit reserves (same approximation
        // as compute_spread_v2_only — the real reserves are used by size_optimizer).
        // CATALOG-BACKFILL-01: leg errors carry the projector's precise label —
        // a catalog gap (PoolNotCatalogued / PairHasNoV3Pools) is surfaced as
        // V3Labeled(label), only a V2 reserves miss stays QuoteUnavailable.
        let out_a = match self
            .get_pool_quote(pool_a, probe_amount, projector, intent)
            .await
        {
            Ok(v) => v,
            Err(V3QuoteLegError::V3(label)) => return V3GrossOutcome::V3Labeled(label),
            Err(V3QuoteLegError::ReservesMiss) => return V3GrossOutcome::QuoteUnavailable,
        };
        let out_b = match self
            .get_pool_quote(pool_b, probe_amount, projector, intent)
            .await
        {
            Ok(v) => v,
            Err(V3QuoteLegError::V3(label)) => return V3GrossOutcome::V3Labeled(label),
            Err(V3QuoteLegError::ReservesMiss) => return V3GrossOutcome::QuoteUnavailable,
        };

        if out_a.is_zero() && out_b.is_zero() {
            // Projector answered but produced nothing usable on either leg
            // (e.g. zero-liquidity slot0) — same operator action as a miss.
            return V3GrossOutcome::QuoteUnavailable;
        }

        // ── V3-CYCLE-GROSS-01 (2026-09-27): the CHAINED cycle identity ───────
        //
        // MEASURED DEFECT this replaces. The previous code compared two
        // INDEPENDENT probes of the same size:
        //
        //     out_a = quote(pool_a, probe_amount)          // forward
        //     out_b = quote(pool_b, probe_amount)          // forward, SAME input
        //     spread = |out_a − out_b|                     // a VENUE PRICE GAP
        //     gross  = spread / 1e18 × price(token_out)    // hardcoded scale
        //
        // A venue price gap is not a round-trip return: it ignores the return
        // leg's price impact AND both fees, and `|out_a − out_b|` is largest
        // exactly where one venue is broken (an empty or near-empty pool), which
        // is why live rows carried `expected_profit_usd = 369_506_963_197.84`
        // and `710_273.79750992` on principals of $1000 and $1 — a "profit" of
        // 10^8 times the capital. The file's own note records the 6-decimals
        // half of it as the "UNIT-SCALE (Bug $69M)".
        //
        // The V2/V2 path was already corrected to the chained identity
        // (`v2_cycle_profit`, see its doc above); V3 kept the legacy form. This
        // applies the SAME identity to V3:
        //
        //   buy where the probe returns MORE, then SELL the proceeds back on
        //   the other venue, and measure the raw profit of that round trip in
        //   token_in units (token_in → … → token_in), scaled by token_in's real
        //   decimals and priced at token_in's live USD price.
        //
        // Cost: one extra quote (3 instead of 2). Only the orientation the
        // forward probes already favour is chained — the other is dominated by
        // construction, so no budget is spent on it.
        let token_in = intent.legs.first().map(|l| l.token_in);
        let token_out = intent.legs.first().map(|l| l.token_out);
        let (Some(token_in), Some(token_out)) = (token_in, token_out) else {
            // No intent direction → cannot chain a cycle (R8: refuse, never
            // fall back to the venue gap).
            return V3GrossOutcome::QuoteUnavailable;
        };

        // Buy on the venue the probe says is cheaper (more out for the same in).
        let (buy_out, sell_pool) = if out_a >= out_b {
            (out_a, pool_b)
        } else {
            (out_b, pool_a)
        };
        let returned = match self
            .get_pool_quote_dir(sell_pool, buy_out, projector, token_out)
            .await
        {
            Ok(v) => v,
            Err(V3QuoteLegError::V3(label)) => return V3GrossOutcome::V3Labeled(label),
            Err(V3QuoteLegError::ReservesMiss) => return V3GrossOutcome::QuoteUnavailable,
        };

        // SPREAD-SIGNED-DELTA-01 (2026-10-04): the SIGNED delta.
        //
        // `returned.saturating_sub(probe_amount)` used to collapse TWO different
        // market states into one verdict: a cycle that LOSES (`returned < probe`)
        // and a cycle that is EXACTLY flat (`returned == probe`) both produced
        // zero units, and both were reported as `spread_zero_equilibrium` whose
        // doc claimed "both legs quoted identical amounts" — a condition this
        // function never evaluates. MEASURED (SPREAD-ZERO-01, live PG): that
        // channel is ~26.9k emissions/h behind 2,553 real states, and at the
        // probed size a majors round trip LOSES by construction (60 bps of
        // round-trip cost for WETH/DAI vs 1-10 bps of venue gap), so the erased
        // branch was the majority: an unauditable loss wearing an equilibrium's
        // label. R8: a real loss stays a loss, a real equilibrium stays zero.
        if returned == probe_amount {
            // Exactly the probe back — the ONE true equilibrium. The emitter
            // publishes it as a MEASURED zero (`Some(0.0)`).
            return V3GrossOutcome::SpreadZeroEquilibrium;
        }
        if returned < probe_amount {
            // Short of the probe: a real loss. Measured shortfall in token_in raw
            // units, priced by the SAME live lookup the profit path uses (no
            // default, no filler: an unpriced token stays `NoTokenPrice`).
            let shortfall = probe_amount - returned;
            return match compute_gross_usd(&shortfall, cfg_opt, Some(token_in)) {
                Some(v) => V3GrossOutcome::RoundTripLoss {
                    gross_usd: -v,
                    returned_wei: returned,
                },
                None => V3GrossOutcome::NoTokenPrice,
            };
        }
        // More than the probe back — the profitable path (unchanged), now reached
        // only when the round trip really paid.
        let profit_units = returned - probe_amount;

        // Denomination: a cycle opens AND closes in token_in, so the profit is
        // token_in raw units. This replaces the hardcoded `/1e18` with the
        // token's real decimals (18 for WETH/DAI, 6 for USDC/USDT, 8 for WBTC)
        // and prices it at token_in's live USD price.
        match compute_gross_usd(&profit_units, cfg_opt, Some(token_in)) {
            Some(v) => V3GrossOutcome::Usd(v),
            None => V3GrossOutcome::NoTokenPrice,
        }
    }

    /// Get amount_out for `probe_amount` of token_in from a V3 pool using
    /// the state_projector's virtual quote capability.
    ///
    /// V2 pools are handled directly in `build_from_impacted_pairs` with
    /// real reserves from `ReservesCache`. This method is called only for V3.
    ///
    /// CATALOG-BACKFILL-01 (2026-09-17): returns `Err(V3QuoteLegError)`
    /// instead of `Option` so the projector's precise rejection label
    /// (`ProjectV3Error::as_label()`) survives the leg boundary — the prior
    /// `.ok()`-flattening was exactly where `v3_pool_not_catalogued` /
    /// `v3_pair_no_pools` collapsed into `v3_quote_unavailable`.
    async fn get_pool_quote(
        &self,
        pool: &PoolRef,
        probe_amount: U256,
        projector: &StateProjector,
        intent: &RouteIntent,
    ) -> Result<U256, V3QuoteLegError> {
        let token_in = intent.legs.first().map(|l| l.token_in).unwrap_or_default();
        self.get_pool_quote_dir(pool, probe_amount, projector, token_in)
            .await
    }

    /// V3-CYCLE-GROSS-01: quote `amount` through `pool` in the direction implied
    /// by an EXPLICIT `token_in`, not by the intent's first leg.
    ///
    /// The chained-cycle identity needs the RETURN leg (token_out → token_in),
    /// which is the opposite direction of the intent's first leg — deriving the
    /// direction from the intent (as the original helper did) can only ever
    /// quote the forward leg.
    async fn get_pool_quote_dir(
        &self,
        pool: &PoolRef,
        amount: U256,
        projector: &StateProjector,
        token_in: Address,
    ) -> Result<U256, V3QuoteLegError> {
        // V3 pools: virtual quote via state_projector (checked — label preserved).
        if matches!(pool.protocol_type, ProtocolType::V3) {
            let zero_for_one = token_in == pool.token0 || token_in == Address::zero();
            let sp_pool = crate::state_projector::PoolRef {
                address: pool.address,
                token0: pool.token0,
                token1: pool.token1,
                fee_bps: pool.fee_bps,
            };
            projector
                .project_v3_quote_checked(&sp_pool, amount, zero_for_one)
                .await
                .map(|q| q.amount_out)
                .map_err(|e| V3QuoteLegError::V3(e.as_label()))
        } else {
            // V2 / Curve / Balancer: quote via REAL reserves from the cache.
            // J-5 fix (2026-08-09): `compute_v3_gross_usd` calls this on BOTH
            // pools of a mixed V2-V3 pair. The prior `None` return for non-V3
            // pools made the `out_a?`/`out_b?` short-circuit reject EVERY
            // mixed V2-V3 pair as `no_price_oracle` (the V2 leg never quoted).
            // Reserves miss → Err (R8: no fabrication), same as the pure-V2
            // path which already emits `reserves_cache_miss` upstream.
            let Some((r0, r1)) = self.reserves_cache.get(&pool.address).await else {
                return Err(V3QuoteLegError::ReservesMiss);
            };
            // Orientation from the EXPLICIT token_in (same rule as
            // `orient_reserves`, kept local so both directions are expressible).
            let (r_in, r_out) = if token_in == pool.token1
                && token_in != Address::zero()
                && token_in != pool.token0
            {
                (r1, r0)
            } else {
                (r0, r1)
            };
            let fee = pool.fee_bps.unwrap_or(30);
            Ok(amm_math::v2_amount_out(amount, r_in, r_out, fee))
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
///
/// B3 (math-audit AUDIT-MATH-OPPS-2026-09-26): the V2/V2 value handed to this
/// function is the CHAINED cycle's profit in token_in raw units (see
/// `v2_cycle_profit`), so the denomination token at that call site is token_in.
fn v2_cycle_profit(
    probe_amount: U256,
    r_in_a: U256,
    r_out_a: U256,
    fee_a: u32,
    r_in_b: U256,
    r_out_b: U256,
    fee_b: u32,
) -> U256 {
    // A real arb is a CHAINED cycle: buy token_in→token_out where it is cheaper,
    // then sell the proceeds back into the other pool (REVERSE orientation).
    // The legacy `|out_a − out_b|` compared two independent probes of the same
    // size — ignoring both fees and the return leg's price impact — and reported
    // positive gross on routes the sizing kernel computed as negative.
    // U256 cannot go negative: a losing cycle returns zero (computed-and-zero,
    // which `compute_gross_usd` maps to None per R8).
    let cycle = |buy_in: U256,
                 buy_out: U256,
                 buy_fee: u32,
                 sell_in: U256,
                 sell_out: U256,
                 sell_fee: u32|
     -> U256 {
        let mid = amm_math::v2_amount_out(probe_amount, buy_in, buy_out, buy_fee);
        amm_math::v2_amount_out(mid, sell_in, sell_out, sell_fee)
    };
    // A→B: A buys (forward), B sells (reverse: token_out reserve first).
    let final_ab = cycle(r_in_a, r_out_a, fee_a, r_out_b, r_in_b, fee_b);
    // B→A: B buys, A sells.
    let final_ba = cycle(r_in_b, r_out_b, fee_b, r_out_a, r_in_a, fee_a);
    let best = if final_ab >= final_ba {
        final_ab
    } else {
        final_ba
    };
    best.saturating_sub(probe_amount)
}

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
    // 3578 USDC spread → $10.7M). Stables resolve through the same live price
    // lookup; WETH = operator base price; any unpriced token → None (R8) so
    // the SizeOptimizer/evaluator re-prices it from live Redis downstream.
    // This is a fast-filter proxy only.
    let price_usd =
        canonical_token_price_usd(token_out, cfg.base_token_price_usd, &cfg.token_prices_usd)?;
    Some(spread_f64 * price_usd)
}

/// ALWAYS-COMPUTE-03 (2026-09-27): USD value of the probe PRINCIPAL that a
/// measured engine gross belongs to.
///
/// Same two real sources `compute_gross_usd` already uses for the gross itself
/// — the canonical decimals table ([`canonical_token_decimals`], immutable
/// protocol constants) and the live price lookup
/// ([`canonical_token_price_usd`]: Redis-merged `token_prices_usd`, WETH
/// `base_token_price_usd` fallback). `None` when the entry token is not priced
/// (R8 — no fabricated capital) or the result is not a usable positive finite
/// figure; the caller then keeps the row's honest diagnostic instead of
/// claiming a closed arithmetic it cannot price.
fn probe_amount_in_usd(
    probe_amount: U256,
    token_in: Option<Address>,
    cfg: &TradingConfigState,
) -> Option<f64> {
    let price_usd =
        canonical_token_price_usd(token_in, cfg.base_token_price_usd, &cfg.token_prices_usd)?;
    let decimals = canonical_token_decimals(token_in);
    let units = u256_to_f64_lossy(probe_amount) / 10f64.powi(decimals as i32);
    let usd = units * price_usd;
    (usd.is_finite() && usd > 0.0).then_some(usd)
}

/// Canonical verified USD price for a known mainnet token, for the
/// `compute_gross_usd` / `compute_v3_gross_usd` fast-filter. Checks:
/// 1. Canonical tokens (stables included) → LIVE Redis price
///    (DexScreener/Chainlink/GeckoTerminal) from `token_prices_usd` (merged by
///    the orchestrator before engine fan-out). NO $1.00 stable shortcut
///    (WO-PC4) — an unpriced stable is a reject, not parity.
/// 2. WETH fallback → `base_token_price_usd` if configured.
/// 3. Else → None (R8: unpriced, NEVER fabricate).
fn canonical_token_price_usd(
    token: Option<Address>,
    base_token_price_usd: f64,
    token_prices_usd: &std::collections::HashMap<String, f64>,
) -> Option<f64> {
    let addr = token?;
    let addr_str = format!("0x{:040x}", addr);
    // WO-PC4: stables have NO $1.00 shortcut — they resolve through the SAME
    // live price lookup below (Chainlink anchors / bus-fused snapshot in
    // `token_prices_usd`). An unpriced stable → None → R8 reject, never parity.
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
        // Stables mapped for the LIVE price lookup (WO-PC4: no $1 shortcut).
        "0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48" => Some("USDC"),
        "0xdac17f958d2ee523a2206206994597c13d831ec7" => Some("USDT"),
        // DAI-SYMBOL-ADDR-01 (audit 2026-09-29): this arm used to read
        // "0x6b175474e8f94a44ad05d02b745dcc163a999080", which differs from the
        // canonical DAI address in 27 of its 42 characters. Consequence: the REAL
        // DAI address fell through to `None` (so DAI — present in
        // `allowed_token_symbols` — was silently unpriced, the `no_price_oracle`
        // family), while an address that is NOT DAI was labelled "DAI". The
        // canonical constant is already imported at the top of this file
        // (shared_rs::chains), so use it instead of a literal
        // (arbx-no-hardcode-doctrine: one source of truth).
        DAI_MAINNET_LC => Some("DAI"),
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
        USDC_MAINNET_LC => 6,
        USDT_MAINNET_LC => 6,
        // WBTC (8)
        "0x2260fac5e5542a773aa44fbcfedf7c193bc2c599" => 8,
        // Everything else (WETH, DAI, and the ERC-20 majority) → 18.
        _ => 18,
    }
}

/// PER-HOP (math-audit AUDIT-MATH-OPPS-2026-09-26): string-address wrapper for
/// callers that hold route tokens as lowercase `0x…` strings (RouteMetadata's
/// `token_addresses`). Unparseable/missing → 18, the same dominant-ERC-20
/// default the engine uses — never a fabricated per-token value.
pub(crate) fn canonical_token_decimals_str(token: &str) -> u8 {
    token
        .parse::<Address>()
        .map(|a| canonical_token_decimals(Some(a)) as u8)
        .unwrap_or(18)
}

// ---------------------------------------------------------------------------
// Opportunity constructors
// ---------------------------------------------------------------------------

/// ECONOMIC DENOMINATION (CANDIDATE-POSTSIZE-SYNC-01 (c)): the base token of the
/// CLOSED cycle this candidate trades — the token its path opens AND closes in
/// (`route.token[0] == route.token[N]`).
///
/// Source of truth is the `RouteIntent` the engine actually measured:
/// `probe_amount` is one native unit of `intent.legs[0].token_in`,
/// `orient_reserves` orients every pool by it and `compute_gross_usd` prices the
/// chained cycle profit with it — so the intent's entry token IS the token this
/// engine's numbers are denominated in. `pool_a.token0/token1` is only the pool's
/// on-chain ordering and says nothing about the direction the observed
/// transaction took.
///
/// Fallback (never a guess): an intent whose entry token is NOT one of this pair's
/// tokens leaves the pair's direction undetermined — `orient_reserves` then keeps
/// the pool's canonical token0→token1 order, so `pool_a.token0` is exactly what
/// the engine measured for that shape.
fn economic_base_token(pool_a: &PoolRef, intent: &RouteIntent) -> Address {
    match intent.legs.first().map(|l| l.token_in) {
        Some(t) if t == pool_a.token0 || t == pool_a.token1 => t,
        _ => pool_a.token0,
    }
}

/// Builds an `Opportunity`, `OpportunityCandidate`, and `RoutePlan` for
/// an accepted (engine-level) DEX arb candidate.
///
/// CANDIDATE-POSTSIZE-SYNC-01 (2026-09-27): the ECONOMIC token identity comes
/// from `intent` (the route the engine measured), never from `pool_a`'s on-chain
/// ordering. A closed cycle opens and closes in its base token, so
/// `Opportunity.token_in`/`token_out` — the P&L denomination — are BOTH that base
/// token, while the counter (intermediate) token stays on the route legs and the
/// persisted `route_metadata` for allowlist/pricing/audit. See the field comments
/// below for why `expected_amount_out` starts as `NaN` instead of a fabricated
/// 1:1.
#[allow(clippy::too_many_arguments)]
fn build_accepted_opportunity(
    chain_id: u64,
    tx_hash: H256,
    pool_a: &PoolRef,
    pool_b: &PoolRef,
    label: StrategyLabel,
    gross_profit_usd: Option<f64>,
    amount_in_wei: U256,
    block_number: Option<u64>,
    intent: &RouteIntent,
) -> (Opportunity, OpportunityCandidate, RoutePlan) {
    let strategy_kind: StrategyKind = label.to_contract_strategy_kind();
    let id = Uuid::new_v4();
    let trace_id = Uuid::new_v4();

    // Closed cycle: base = opens and closes the path; counter = the other token
    // of the traded pair (the intermediate hop token, NOT the P&L denomination).
    let base_token = economic_base_token(pool_a, intent);
    let counter_token = if base_token == pool_a.token0 {
        pool_a.token1
    } else {
        pool_a.token0
    };

    // BOTH economic sides are the base token: `Opportunity.token_in`/`token_out`
    // are the P&L denomination of a closed cycle, and the spine prices the input
    // with `candidate.token_addresses[0]` and the measured output with
    // `candidate.token_addresses[1]` (config_aware.rs). Denomination ≠ traded
    // pair: the pair stays visible in `pair_symbol` and on the route legs.
    let token_in_str = format!("0x{:040x}", base_token);
    let token_out_str = token_in_str.clone();
    let counter_token_str = format!("0x{:040x}", counter_token);
    let pair_symbol = format!("{}…/{}…", &token_in_str[2..8], &counter_token_str[2..8],);

    let amount_in_wei_str = amount_in_wei.to_string();
    // ECON-AMOUNT-DENOM-01 (2026-10-04): the wei→token-units conversion uses the
    // REAL decimals of the token this amount is denominated in — the same base
    // token `token_in`/`token_out` and the route legs are expressed in.
    //
    // The blanket `/ 1e18_f64` was not "wrong as a constant" — one whole 18-decimals
    // token IS 1e18 raw units — but it was serving a semantic it cannot serve: the
    // TOKEN's unit. For a 6-decimals base (USDC/USDT) it was off by 1e12 in
    // `candidate.amount_in` and in every route leg's `amount_in`, which is the same
    // class of error that produced the `fee_tier` ×100 episode. The literal is gone:
    // the unit comes from the same immutable protocol table the probe and the USD
    // pricing already use, so every field of the row speaks in the base token's unit.
    let amount_decimals = canonical_token_decimals(Some(base_token)) as u8;
    let amount_in_f64: f64 = wei_to_token_units(amount_in_wei, amount_decimals);

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
        // §30 contract: anchor the row to the intent's observed block so the
        // FE semantic gate never flags `missing_block` when the block IS known.
        block_number,
        rejection_reason: None,
        cartridge_id: None,
        // WO-CARDS-COMPLETE-01 (2026-09-17): detector identity at construction;
        // pipeline_latency_ms is stamped by the emitter at emit entry.
        detector_id: Some("dex_engine".to_string()),
        pipeline_latency_ms: None,
        detected_at: Utc::now(),
        trace_id,
        economics: None,
    };

    let pool_a_lower = format!("0x{:040x}", pool_a.address);
    let pool_b_lower = format!("0x{:040x}", pool_b.address);

    let candidate = OpportunityCandidate {
        // Fingerprint = venue + the pair's two tokens (direction-aware) so two
        // different pairs on the same venue and base token never collapse.
        route_fingerprint: format!("{}_{}_{}", pool_a.dex_name, token_in_str, counter_token_str),
        pool_addresses: vec![pool_a_lower.clone(), pool_b_lower.clone()],
        // SPINE CONTRACT (config_aware.rs §3): index 0 denominates `amount_in`,
        // index 1 denominates `expected_amount_out`. A closed cycle's output is
        // denominated in the base token it returns to, so both slots are the base
        // token — putting the counter token in slot 1 would price a
        // base-denominated output at the counter's price and trip the spread
        // sanity gate on a row the kernel computed correctly. The FULL traversal
        // path (base → counter → base) rides `route_plan.legs` and the persisted
        // `route_metadata` (`build_route_metadata_from_plan`), which is where
        // allowlist/pricing/audit read the intermediate from.
        token_addresses: vec![token_in_str.clone(), token_out_str.clone()],
        dex_adapters: vec![pool_a.dex_name.clone(), pool_b.dex_name.clone()],
        amount_in: amount_in_f64,
        // NOT MEASURED — deliberately `NaN`, never `amount_in` (R8).
        //
        // This field is a bare `f64`: R8's `None` ("not computed") has no
        // representation here, and `0.0` is reserved for "computed and exactly
        // zero" — writing it would ALSO hand `observed_rate = 0` to the spread
        // sanity gate (a false `ImplausibleSpread`) and `0` to the evidence's
        // `min_amount_out`. The previous `amount_in` claimed a 1:1 cycle nobody
        // measured. IEEE-754 `NaN` is the one honest marker available: it is this
        // crate's existing "not computable" value (`scoring.rs` → InvalidEvidence,
        // `sim_encoder::convert_amount_to_wei` refuses it), it cannot be read as a
        // rate, and it fails the second evaluation CLOSED (net = NaN ⇒ `is_viable`
        // false ⇒ the risk gate rejects) instead of certifying a fabricated
        // parity. The orchestrator's sized path overwrites it with the kernel's
        // measured final-hop output (`orchestrator::stamp_sized_figures`) whenever
        // that measurement exists.
        expected_amount_out: f64::NAN,
        gross_profit: gross_profit_usd.unwrap_or(0.0),
    };

    // Route direction = the cycle's own: base → counter on `pool_a`, counter →
    // base on `pool_b`. The kernel orients each leg by these tokens, so the
    // ledger and the plan agree on one direction.
    let leg_a = build_route_leg(pool_a, &token_in_str, &counter_token_str, amount_in_f64);
    let leg_b = build_route_leg(pool_b, &counter_token_str, &token_out_str, amount_in_f64);

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
    block_number: Option<u64>,
    intent: &RouteIntent,
) -> (Opportunity, OpportunityCandidate, RoutePlan) {
    build_accepted_opportunity(
        chain_id,
        tx_hash,
        pool_a,
        pool_b,
        label,
        None,                                      // R8: no profit for rejected candidates
        U256::from(10u128).pow(U256::from(18u32)), // unit probe
        block_number,
        intent,
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
///
/// `pub(crate)`: the discovery→emission bridge
/// (`route_discovery::hop_cycle_bridge`) labels a discovered leg with its own
/// protocol family through this same table — one mapping, never a second one.
pub(crate) fn protocol_type_to_str(pt: ProtocolType) -> String {
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
///
/// `pub(crate)`: CANDIDATE-POSTSIZE-SYNC-01's orchestrator-side sync applies the
/// SAME truncation when it has no decimals entry for a token (legacy `1e18` rule)
/// — one conversion primitive, never a second divergent one.
pub(crate) fn u256_to_f64_lossy(v: U256) -> f64 {
    // U256 → u128 (truncates top 128 bits — negligible for amounts
    // that fit in u128, which is all practical EVM balances).
    v.low_u128() as f64
}

/// CANDIDATE-POSTSIZE-SYNC-01: wei → token units (f64) with the token's REAL
/// decimals — the exact inverse of the `10^decimals` scaling
/// `canonical_token_decimals` already applies on the USD side.
///
/// Used by the orchestrator to express the sizing kernel's `optimal_amount_in`
/// and its measured final-hop output in the token-unit convention
/// `OpportunityCandidate` documents (`amount_in` / `expected_amount_out` as f64
/// token units), instead of the legacy blanket `1e18`.
pub(crate) fn wei_to_token_units(wei: U256, decimals: u8) -> f64 {
    u256_to_f64_lossy(wei) / 10f64.powi(i32::from(decimals))
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
    use crate::state_projector::{StateProjector, V3QuoteProvider};
    use crate::strategy_label::StrategyLabel;
    use crate::v3_fee_catalog::V3FeeCatalog;
    use ethers::types::{Address, H256, U256};
    use shared_rs::contracts::StrategyKind;
    use shared_rs::trading_config::{GasPriceStrategy, TradingConfigState};
    use std::collections::HashMap;
    use std::future::Future;
    use std::pin::Pin;

    /// G-ECON-1: the Option-shaped plumbing only surfaces Usd; every failure
    /// classification stays invisible to gross consumers by design.
    #[test]
    fn v3_gross_outcome_usd_only_for_computed() {
        assert_eq!(V3GrossOutcome::Usd(1.5).usd(), Some(1.5));
        assert_eq!(V3GrossOutcome::QuoteUnavailable.usd(), None);
        assert_eq!(
            V3GrossOutcome::V3Labeled("v3_pool_not_catalogued").usd(),
            None
        );
        assert_eq!(V3GrossOutcome::SpreadZeroEquilibrium.usd(), None);
        assert_eq!(V3GrossOutcome::NoTokenPrice.usd(), None);
        assert_eq!(V3GrossOutcome::NoConfig.usd(), None);
        assert_eq!(V3GrossOutcome::Skipped.usd(), None);
    }

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

    // ── dex_engine::tests::candidates_anchor_observed_block ─────────────────
    // §30 regression: an intent with a known observed block must propagate it
    // to EVERY emitted Opportunity (accepted AND rejected). Before the fix both
    // constructors hardcoded block_number: None → persisted NULL → the FE
    // semantic gate quarantined the row as `missing_block`.

    #[tokio::test]
    async fn candidates_anchor_observed_block() {
        let tok_a = addr(0x1);
        let tok_b = addr(0x2);
        let pool1 = make_pool(addr(0x10), tok_a, tok_b, ProtocolType::V2);
        let pool2 = make_pool(addr(0x11), tok_a, tok_b, ProtocolType::V2);
        let mut intent = make_intent(tok_a, tok_b);
        // §30: `observed_block()` only surfaces a height for NewBlock-sourced
        // intents (mempool intents honestly carry none) — mirror block_scanner.
        intent.source_event = crate::route_intent::DetectionSource::NewBlock;
        intent.observed_block_number = Some(12_345);
        let impact = make_impact(vec![pool1, pool2]);
        // Empty reserves cache → reserves_cache_miss rejection path.
        let engine = make_engine();

        let candidates = engine
            .build_from_impacted_pairs(&intent, &impact, None)
            .await
            .expect("engine must not error");

        assert!(!candidates.is_empty(), "must produce candidates");
        for c in &candidates {
            assert_eq!(
                c.opportunity.block_number,
                Some(12_345),
                "rejected candidate must anchor the intent's observed block (§30 missing_block regression)"
            );
        }

        // Accepted path: real reserves → candidate reaches the optimizer.
        let unit = U256::from(10u128).pow(U256::from(18u32)) * U256::from(1_000u32);
        let engine_ok =
            make_engine_with_reserves(vec![(addr(0x10), unit, unit), (addr(0x11), unit, unit)])
                .await;
        let candidates_ok = engine_ok
            .build_from_impacted_pairs(&intent, &impact, None)
            .await
            .expect("engine must not error");
        assert!(
            !candidates_ok.is_empty(),
            "must produce accepted candidates"
        );
        for c in &candidates_ok {
            assert_eq!(
                c.opportunity.block_number,
                Some(12_345),
                "accepted candidate must anchor the intent's observed block (§30 missing_block regression)"
            );
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

    // ── dex_engine::tests::closed_cycle_reports_base_token_economically ───────
    //
    // CANDIDATE-POSTSIZE-SYNC-01 (c)+(b): the economic token identity of a CLOSED
    // cycle comes from the RouteIntent direction, not from `pool_a`'s on-chain
    // ordering, and the candidate never claims a 1:1 output it did not measure.
    //
    // Fixture: pool_a lists the tokens as (counter, base) — the OLD code reported
    // `pool_a.token0` (= counter) as `token_in`, i.e. the wrong leg of the cycle.
    // The intent enters at the base token, which is the token every kernel number
    // is denominated in (`probe_amount`, `orient_reserves`, `compute_gross_usd`).

    #[tokio::test]
    async fn closed_cycle_reports_base_token_economically() {
        let base = addr(0x1); // cycle base: opens AND closes the path
        let counter = addr(0x2); // intermediate hop token
        let pool_addr1 = addr(0x10);
        let pool_addr2 = addr(0x11);
        // Deliberately inverted vs the pool's canonical ordering: the pool says
        // token0 = counter, the INTENT enters at base.
        let pool1 = make_pool(pool_addr1, counter, base, ProtocolType::V2);
        let pool2 = make_pool(pool_addr2, counter, base, ProtocolType::V2);
        let intent = make_intent(base, counter);
        let impact = make_impact(vec![pool1, pool2]);

        let unit = U256::from(10u128).pow(U256::from(18u32)) * U256::from(1_000u32);
        let engine =
            make_engine_with_reserves(vec![(pool_addr1, unit, unit), (pool_addr2, unit, unit)])
                .await;

        let candidates = engine
            .build_from_impacted_pairs(&intent, &impact, None)
            .await
            .expect("engine must not error");

        let base_str = format!("0x{:040x}", base);
        let counter_str = format!("0x{:040x}", counter);

        let accepted: Vec<_> = candidates
            .iter()
            .filter(|c| c.rejection_reason.is_none())
            .collect();
        assert!(
            !accepted.is_empty(),
            "fixture must reach the accepted path (real reserves on both pools)"
        );

        for c in accepted {
            // (c) ECONOMIC denomination: both sides of a closed cycle are the base
            // token — never `pool_a.token0/token1`.
            assert_eq!(
                c.opportunity.token_in, base_str,
                "closed cycle must open in its base token, not pool_a.token0"
            );
            assert_eq!(
                c.opportunity.token_out, base_str,
                "closed cycle must close in its base token, not pool_a.token1"
            );
            assert_ne!(
                c.opportunity.token_in, counter_str,
                "the counter token is NOT the P&L denomination"
            );

            // Spine pricing contract: slot 0 denominates `amount_in`, slot 1 the
            // expected output — for a closed cycle both are the base token.
            assert_eq!(
                c.candidate.token_addresses,
                vec![base_str.clone(), base_str.clone()],
                "the spine's two pricing slots must both be the base token"
            );

            // The intermediate token is NOT lost: it rides the traversal path.
            assert_eq!(
                c.route_plan.legs[0].token_in, base_str,
                "leg 0 enters at the cycle base"
            );
            assert_eq!(
                c.route_plan.legs[0].token_out, counter_str,
                "leg 0 exits into the counter (intermediate) token"
            );
            assert_eq!(
                c.route_plan.legs[1].token_out, base_str,
                "leg 1 closes the cycle back into the base token"
            );
            let metadata = crate::persistence::build_route_metadata_from_plan(&c.route_plan);
            assert_eq!(
                metadata.token_addresses,
                vec![base_str.clone(), counter_str.clone(), base_str.clone()],
                "persisted route_metadata must keep the full traversal path \
                 (base → counter → base) for allowlist/pricing/audit"
            );

            // (b) NOT MEASURED: construction time has no cycle output, so the
            // candidate must not claim a 1:1 — `NaN` is the honest marker.
            assert!(
                c.candidate.expected_amount_out.is_nan(),
                "unmeasured output must be NaN, got {}",
                c.candidate.expected_amount_out
            );
            assert_ne!(
                c.candidate.expected_amount_out, c.candidate.amount_in,
                "a 1:1 output would be a fabricated measurement"
            );
        }

        // The rejected shape (no reserves) carries the same economic identity.
        let no_reserves_engine = make_engine();
        let rejected = no_reserves_engine
            .build_from_impacted_pairs(&intent, &impact, None)
            .await
            .expect("engine must not error");
        assert!(!rejected.is_empty(), "must still emit rejection rows");
        for c in &rejected {
            assert_eq!(c.opportunity.token_in, base_str);
            assert_eq!(c.opportunity.token_out, base_str);
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

    // ── dex_engine::tests::perhop_decimals_str ───────────────────────────────

    /// PER-HOP gate (math-audit AUDIT-MATH-OPPS-2026-09-26): the string wrapper
    /// that populates RouteMetadata.decimals must agree with the canonical table
    /// the engine's USD conversion uses — otherwise per-hop wei would convert
    /// with the wrong scale (the exact bug class B1 fixed).
    #[test]
    fn perhop_decimals_str_matches_the_canonical_table() {
        assert_eq!(
            canonical_token_decimals_str("0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48"),
            6,
            "USDC"
        );
        assert_eq!(
            canonical_token_decimals_str("0xdac17f958d2ee523a2206206994597c13d831ec7"),
            6,
            "USDT"
        );
        assert_eq!(
            canonical_token_decimals_str("0x2260fac5e5542a773aa44fbcfedf7c193bc2c599"),
            8,
            "WBTC"
        );
        assert_eq!(
            canonical_token_decimals_str("0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2"),
            18,
            "WETH"
        );
        // Unknown-but-valid and unparseable input both take the dominant ERC-20
        // default (18) — never a fabricated per-token value.
        assert_eq!(
            canonical_token_decimals_str("0x0000000000000000000000000000000000000123"),
            18
        );
        assert_eq!(canonical_token_decimals_str("not-an-address"), 18);
        assert_eq!(canonical_token_decimals_str(""), 18);
    }

    // ── dex_engine::tests::dai_symbol_addr_01 ────────────────────────────────

    /// DAI-SYMBOL-ADDR-01 (audit 2026-09-29) — regression gate.
    ///
    /// The DAI arm of [`canonical_token_symbol`] used to be a literal that was
    /// NOT the DAI address (27 of 42 chars wrong). Two failures followed from it:
    /// the real DAI address resolved to `None` (no price lookup ⇒ DAI routes
    /// silently unpriced), and a non-DAI address was labelled "DAI" on the wire.
    /// This test fails if the typo ever comes back, and it also rejects any
    /// malformed address in the map (same error class: a hand-copied literal).
    #[test]
    fn dai_symbol_addr_01_canonical_address_resolves_and_map_is_well_formed() {
        assert_eq!(
            canonical_token_symbol(DAI_MAINNET_LC),
            Some("DAI"),
            "the canonical DAI address must resolve to its symbol"
        );
        let typo = concat!("0x6b175474e8f94a44ad05d02b745", "dcc163a999080");
        assert_eq!(
            canonical_token_symbol(typo),
            None,
            "the mistyped address is not DAI and must never resolve"
        );
        assert_ne!(typo, DAI_MAINNET_LC);
        for addr in [
            "0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2",
            "0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48",
            "0xdac17f958d2ee523a2206206994597c13d831ec7",
            "0x2260fac5e5542a773aa44fbcfedf7c193bc2c599",
            "0x1f9840a85d5af5bf1d1762f925bdaddc4201f984",
        ] {
            assert!(
                canonical_token_symbol(addr).is_some(),
                "canonical address must resolve: {addr}"
            );
            assert_eq!(addr.len(), 42, "an address is 0x + 40 hex chars: {addr}");
            assert!(
                addr.strip_prefix("0x").is_some_and(|h| h
                    .chars()
                    .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase())),
                "the map is keyed by lowercase 0x-prefixed hex: {addr}"
            );
        }
        assert_eq!(canonical_token_symbol("0xdeadbeef"), None);
        assert_eq!(canonical_token_symbol(""), None);
    }

    // ── dex_engine::tests::b1_probe_is_one_native_unit ───────────────────────

    /// B1 gate (math-audit AUDIT-MATH-OPPS-2026-09-26): the probe must be ONE
    /// NATIVE UNIT of token_in (10^decimals). The fixed 1e18 probe meant 1e12
    /// USDC = $999,935,091,316.80 at the live price, so every gross/USD number
    /// for 6-dec pairs was computed at an operating point that saturates any
    /// pool. This pins the immutable protocol table the probe derives from.
    #[test]
    fn b1_probe_is_one_native_unit_of_token_in() {
        use std::str::FromStr;
        let usdc = Address::from_str("0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48").unwrap();
        let usdt = Address::from_str("0xdac17f958d2ee523a2206206994597c13d831ec7").unwrap();
        let wbtc = Address::from_str("0x2260fac5e5542a773aa44fbcfedf7c193bc2c599").unwrap();
        let weth = Address::from_str("0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2").unwrap();
        assert_eq!(canonical_token_decimals(Some(usdc)), 6);
        assert_eq!(canonical_token_decimals(Some(usdt)), 6);
        assert_eq!(canonical_token_decimals(Some(wbtc)), 8);
        assert_eq!(canonical_token_decimals(Some(weth)), 18);
        // Unknown / absent → the dominant ERC-20 default (previous behaviour).
        assert_eq!(canonical_token_decimals(None), 18);

        // The probe derived from the table: 1 USDC = 1e6 raw, NOT 1e18.
        let probe_usdc = U256::from(10u128).pow(U256::from(canonical_token_decimals(Some(usdc))));
        assert_eq!(probe_usdc, U256::from(1_000_000u64));
        let probe_weth = U256::from(10u128).pow(U256::from(canonical_token_decimals(Some(weth))));
        assert_eq!(probe_weth, U256::from(10u128).pow(U256::from(18u32)));
    }

    // ── dex_engine::tests::b3_chained_cycle_kills_the_phantom_positive ───────

    /// B3 gate (math-audit AUDIT-MATH-OPPS-2026-09-26): on a route where the
    /// legacy `|out_a − out_b|` reported a strictly POSITIVE spread, the chained
    /// cycle must return ZERO — the second leg receives the first leg's output
    /// and pays its own fee/impact, so a phantom positive can never reach the
    /// emitter. A genuine dislocation must still produce profit.
    #[test]
    fn b3_chained_cycle_kills_the_phantom_positive() {
        let e18 = U256::from(10u128).pow(U256::from(18u32));
        // `probe` = one native unit in.
        let probe = e18;
        // Pool A balanced (1000/1000); pool B token_out-poor by 0.1% (1000/999).
        // The imbalance must be SMALLER than the round-trip fee drag (2 × 0.3%):
        // a bigger dislocation is a REAL arb and would (correctly) profit — that
        // is the second half of this gate.
        let a_in = e18 * U256::from(1000u32);
        let a_out = e18 * U256::from(1000u32);
        let b_in = e18 * U256::from(1000u32);
        let b_out = e18 * U256::from(999u32);

        // The legacy metric on this fixture was strictly positive…
        let out_a = amm_math::v2_amount_out(probe, a_in, a_out, 30);
        let out_b = amm_math::v2_amount_out(probe, b_in, b_out, 30);
        let legacy = if out_a >= out_b {
            out_a - out_b
        } else {
            out_b - out_a
        };
        assert!(
            legacy > U256::zero(),
            "fixture must reproduce the legacy phantom positive"
        );

        // …while the real chained cycle returns nothing (no arb exists).
        let profit = v2_cycle_profit(probe, a_in, a_out, 30, b_in, b_out, 30);
        assert_eq!(
            profit,
            U256::zero(),
            "chained cycle must not report the phantom positive"
        );

        // A genuine dislocation (B is token_out-rich) DOES yield chained profit.
        let b_rich = e18 * U256::from(1100u32);
        let profit_real = v2_cycle_profit(probe, a_in, a_out, 30, b_in, b_rich, 30);
        assert!(
            profit_real > U256::zero(),
            "a real dislocation must still produce profit"
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

    // ── CATALOG-BACKFILL-01 (2026-09-17): precise catalog-gap labels ──────────
    //
    // Regression anchor for the flattening bug: `get_pool_quote` used
    // `project_v3_quote(...).ok()` — Option::ok() discarded the checked
    // `ProjectV3Error`, so EVERY catalog gap surfaced downstream as
    // rejection_reason "v3_quote_unavailable" (evidence 2026-09-17:
    // not_catalogued=13,880 metric hits but ZERO v3_pool_not_catalogued /
    // v3_pair_no_pools strings in opportunities.rejection_reason for
    // dex_arb_v3v3).

    /// V3 provider mock that always answers — proves the rejection comes from
    /// the CATALOG path (zero-RPC), never from the provider.
    struct OkV3Mock;
    impl V3QuoteProvider for OkV3Mock {
        fn quote_exact_input_single(
            &self,
            _pool: Address,
            _token_in: Address,
            _token_out: Address,
            _amount_in: U256,
            _fee_bps: u32,
        ) -> Pin<Box<dyn Future<Output = anyhow::Result<U256>> + Send + '_>> {
            Box::pin(async move { Ok(U256::from(1_000u64)) })
        }
    }

    /// SPREAD-SIGNED-DELTA-01: an IDENTITY quote — returns exactly what it is
    /// asked to swap. With it the two forward probes agree AND the chained round
    /// trip returns EXACTLY the probe, which is what "equilibrium" means in this
    /// engine.
    ///
    /// The constant-quote `OkV3Mock` above does NOT produce that state: it
    /// answers 1000 wei to a 1e18 probe, i.e. a LOSING round trip that
    /// `saturating_sub` used to erase into the same zero. That is the defect this
    /// task fixes, visible in the old fixture itself.
    struct IdentityV3Mock;
    impl V3QuoteProvider for IdentityV3Mock {
        fn quote_exact_input_single(
            &self,
            _pool: Address,
            _token_in: Address,
            _token_out: Address,
            amount_in: U256,
            _fee_bps: u32,
        ) -> Pin<Box<dyn Future<Output = anyhow::Result<U256>> + Send + '_>> {
            Box::pin(async move { Ok(amount_in) })
        }
    }

    /// SPREAD-SIGNED-DELTA-01: a provider that keeps 30 bps on every leg — a real
    /// pool fee. The forward probes still agree, but the chained round trip now
    /// returns `0.997² × probe < probe`: a LOSING cycle, the other half of the
    /// partition, and the normal state of a majors round trip at the probed size.
    struct FeeTakingV3Mock;
    impl V3QuoteProvider for FeeTakingV3Mock {
        fn quote_exact_input_single(
            &self,
            _pool: Address,
            _token_in: Address,
            _token_out: Address,
            amount_in: U256,
            _fee_bps: u32,
        ) -> Pin<Box<dyn Future<Output = anyhow::Result<U256>> + Send + '_>> {
            Box::pin(async move { Ok(amount_in * U256::from(997u64) / U256::from(1_000u64)) })
        }
    }

    /// Minimal `TradingConfigState` (same shape as triangular_engine's helper)
    /// so the engine-level rejection branch (`cfg_opt.is_some()`) fires.
    fn make_cfg() -> TradingConfigState {
        TradingConfigState {
            chain_id: 1,
            capital_usd: 10_000.0,
            base_token_symbol: "WETH".into(),
            base_token_price_usd: 3_000.0,
            allowed_token_symbols: vec!["WETH".into(), "USDC".into()],
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
            enabled_strategies: vec!["dex_arb".into()],
            enabled_dex_ids: None,
            strategy_configs: HashMap::new(),
            capital_cost_rate_annual_pct: 0.0,
            ops_overhead_usd_per_attempt: 0.0,
            spread_sanity_mult: 3.0,
            p_copied_volume_threshold_usd: 1_000_000.0,
            p_copied_max: 0.5,
            lp_fee_default_pct: 0.003,
            kelly_multiplier: 0.5,
            kelly_max_per_trade_fraction: 1.0,
            kelly_gas_safety_multiplier: 1.0,
            enabled: true,
            updated_at: Utc::now(),
            updated_by: None,
        }
    }

    fn rejection_reasons(candidates: &[StrategyCandidate]) -> Vec<String> {
        candidates
            .iter()
            .filter_map(|c| c.rejection_reason.clone())
            .collect()
    }

    #[tokio::test]
    async fn catalog_backfill_not_catalogued_rejection_keeps_precise_label() {
        let tok_a = addr(0x1);
        let tok_b = addr(0x2);
        // Pool A catalogued (pair HAS tiers) at 3000; pool B absent from
        // by_pool → leg B resolves PoolNotCatalogued with ZERO RPC.
        let catalog = Arc::new(V3FeeCatalog::new());
        catalog.record_observed(addr(0x10), tok_a, tok_b, 3000);
        let projector = Arc::new(StateProjector::new(
            Arc::new(ReservesCache::new()),
            Some(Arc::new(OkV3Mock)),
            catalog,
        ));
        let engine = DexEngine::new(Arc::new(ReservesCache::new()), None, Some(projector));

        let pool_a = make_pool(addr(0x10), tok_a, tok_b, ProtocolType::V3);
        let pool_b = make_pool(addr(0x11), tok_a, tok_b, ProtocolType::V3);
        let intent = make_intent(tok_a, tok_b);
        let impact = make_impact(vec![pool_a, pool_b]);
        let cfg = make_cfg();

        let candidates = engine
            .build_from_impacted_pairs(&intent, &impact, Some(&cfg))
            .await
            .expect("engine must not error");

        let reasons = rejection_reasons(&candidates);
        assert!(
            reasons.iter().any(|r| r == "v3_pool_not_catalogued"),
            "uncatalogued pool must reject with ITS label, got {reasons:?}"
        );
        assert!(
            !reasons.iter().any(|r| r == "v3_quote_unavailable"),
            "a catalog gap must NOT be flattened to v3_quote_unavailable, got {reasons:?}"
        );
    }

    #[tokio::test]
    async fn catalog_backfill_pair_no_pools_rejection_keeps_precise_label() {
        let tok_a = addr(0x1);
        let tok_b = addr(0x2);
        // EMPTY catalog: the pair has no known V3 tiers at all → the leading
        // leg resolves PairHasNoV3Pools with ZERO RPC.
        let catalog = Arc::new(V3FeeCatalog::new());
        let projector = Arc::new(StateProjector::new(
            Arc::new(ReservesCache::new()),
            Some(Arc::new(OkV3Mock)),
            catalog,
        ));
        let engine = DexEngine::new(Arc::new(ReservesCache::new()), None, Some(projector));

        let pool_a = make_pool(addr(0x10), tok_a, tok_b, ProtocolType::V3);
        let pool_b = make_pool(addr(0x11), tok_a, tok_b, ProtocolType::V3);
        let intent = make_intent(tok_a, tok_b);
        let impact = make_impact(vec![pool_a, pool_b]);
        let cfg = make_cfg();

        let candidates = engine
            .build_from_impacted_pairs(&intent, &impact, Some(&cfg))
            .await
            .expect("engine must not error");

        let reasons = rejection_reasons(&candidates);
        assert!(
            reasons.iter().any(|r| r == "v3_pair_no_pools"),
            "unknown pair must reject with v3_pair_no_pools, got {reasons:?}"
        );
        assert!(
            !reasons.iter().any(|r| r == "v3_quote_unavailable"),
            "a pair gap must NOT be flattened to v3_quote_unavailable, got {reasons:?}"
        );
    }

    #[tokio::test]
    async fn catalog_backfill_provider_failure_still_uses_transport_label() {
        // Guard: a REAL provider failure keeps the transport label — the
        // precise labels are reserved for catalog gaps, not a blanket rename.
        let tok_a = addr(0x1);
        let tok_b = addr(0x2);
        struct ErrV3Mock;
        impl V3QuoteProvider for ErrV3Mock {
            fn quote_exact_input_single(
                &self,
                _pool: Address,
                _token_in: Address,
                _token_out: Address,
                _amount_in: U256,
                _fee_bps: u32,
            ) -> Pin<Box<dyn Future<Output = anyhow::Result<U256>> + Send + '_>> {
                Box::pin(async move { Err(anyhow::anyhow!("mock rpc error")) })
            }
        }
        // BOTH pools catalogued → no catalog gap; the provider error must
        // surface as the transport label v3_quote_unavailable.
        let catalog = Arc::new(V3FeeCatalog::new());
        catalog.record_observed(addr(0x10), tok_a, tok_b, 3000);
        catalog.record_observed(addr(0x11), tok_a, tok_b, 500);
        let projector = Arc::new(StateProjector::new(
            Arc::new(ReservesCache::new()),
            Some(Arc::new(ErrV3Mock)),
            catalog,
        ));
        let engine = DexEngine::new(Arc::new(ReservesCache::new()), None, Some(projector));

        let pool_a = make_pool(addr(0x10), tok_a, tok_b, ProtocolType::V3);
        let pool_b = make_pool(addr(0x11), tok_a, tok_b, ProtocolType::V3);
        let intent = make_intent(tok_a, tok_b);
        let impact = make_impact(vec![pool_a, pool_b]);
        let cfg = make_cfg();

        let candidates = engine
            .build_from_impacted_pairs(&intent, &impact, Some(&cfg))
            .await
            .expect("engine must not error");

        let reasons = rejection_reasons(&candidates);
        assert!(
            reasons.iter().any(|r| r == "v3_quote_unavailable"),
            "provider RPC failure must keep v3_quote_unavailable, got {reasons:?}"
        );
    }

    // ── V3-CYCLE-GROSS-01 ────────────────────────────────────────────────────

    /// The counterexample the operator's reference package encodes
    /// (`VALIDACION.md`: *"dos cotizaciones de ida difieren positivamente pero
    /// los dos ciclos de ida y vuelta pierden"*), as a regression on the identity
    /// the discovery fast-filter uses.
    ///
    /// Pool A is the deep venue; pool B is nearly empty on the return side.
    /// `|out_a − out_b|` — the identity V3 used until this change — is a large
    /// POSITIVE number (a venue price gap). The actual round trip (buy on A at
    /// `probe`, sell the proceeds back on B) returns LESS than `probe`, so the
    /// honest profit is zero. Publishing the gap is what put
    /// `expected_profit_usd = 369_506_963_197.84` on a $1000 principal.
    #[test]
    fn chained_cycle_identity_rejects_a_positive_venue_gap() {
        let probe = U256::from(1_000u64) * U256::from(10u64).pow(U256::from(18u32));
        let deep = probe * U256::from(10u64);
        // A: price 1:1.
        let (a_in, a_out) = (deep, deep);
        // B: price 1.004:1 — a 0.4% better forward quote.
        let (b_in, b_out) = (deep, deep + deep * U256::from(4u64) / U256::from(1_000u64));

        let out_a = amm_math::v2_amount_out(probe, a_in, a_out, 30);
        let out_b = amm_math::v2_amount_out(probe, b_in, b_out, 30);

        // The legacy identity: two INDEPENDENT forward probes, differenced. It is
        // positive whenever the venues disagree by ANY amount.
        let gap = if out_a >= out_b {
            out_a - out_b
        } else {
            out_b - out_a
        };
        assert!(
            !gap.is_zero(),
            "fixture must reproduce a positive venue gap (out_a={out_a} out_b={out_b})"
        );

        // The chained identity: buy where the probe returns more, then sell the
        // proceeds back on the other venue (REVERSE orientation: the return leg
        // consumes token_out, so the selling pool is oriented out→in).
        let (buy_out, sell_in, sell_out) = if out_a >= out_b {
            (out_a, b_out, b_in)
        } else {
            (out_b, a_out, a_in)
        };
        let returned = amm_math::v2_amount_out(buy_out, sell_in, sell_out, 30);
        let chained = returned.saturating_sub(probe);

        // 0.4% of venue edge does not cover the round trip's two 0.3% fees, so
        // the cycle LOSES even though the forward gap is positive.
        assert!(
            returned < probe,
            "fixture must make the round trip lose: returned {returned} vs probe {probe}"
        );
        assert!(
            chained.is_zero(),
            "a losing round trip has no POSITIVE gross profit — the chained \
             identity returned {chained}; the LOSS itself is measured separately \
             (SPREAD-SIGNED-DELTA-01: see \
             losing_round_trip_and_exact_equilibrium_get_different_verdicts)"
        );
        assert!(
            gap > chained,
            "the venue gap ({gap}) must never be published as the cycle's profit (chained {chained})"
        );
    }

    /// The other half of the measured absurdity: the removed code divided the
    /// profit by a HARDCODED `1e18` regardless of the swapped token's decimals
    /// (the file documents it as "UNIT-SCALE (Bug $69M)" — a 6-decimals token
    /// inflated by ~1e12).
    ///
    /// The cycle opens AND closes in `token_in`, so the profit is token_in raw
    /// units and must be scaled by token_in's canonical decimals.
    #[test]
    fn chained_profit_is_scaled_by_token_in_decimals_not_a_hardcoded_1e18() {
        let usdc: Address = "0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48"
            .parse()
            .expect("canonical USDC address");
        let mut cfg = make_cfg();
        cfg.token_prices_usd = HashMap::from([("USDC".to_string(), 1.0)]);

        // 1_000_000 raw USDC = 1.00 USDC, priced at the live $1.00 print.
        let usd = compute_gross_usd(&U256::from(1_000_000u64), &Some(cfg), Some(usdc))
            .expect("USDC resolves through the live price map");
        assert!(
            (usd - 1.0).abs() < 1e-9,
            "1e6 raw USDC must be $1.00 with 6 decimals, got {usd}"
        );
        // What the removed `/1e18` produced for the same input: ~1e-12.
        let legacy = 1_000_000f64 / 1e18 * 1.0;
        assert!(
            legacy < 1e-11 && usd > 1e-3,
            "the hardcoded-1e18 scale ({legacy}) must not be reachable any more (got {usd})"
        );
    }

    // ── ALWAYS-COMPUTE-03 (2026-09-27): MEASURED gross ⇒ CLOSED `computed` ────
    //
    // MEASURED DEFECT (production PG): the `spread_zero_equilibrium` early-exit
    // published a measured gross (`Some(0.0)` — both legs quoted the same
    // amount, so the cycle gross is exactly zero) while the emit boundary still
    // stamped `economics_error(reason)`: `computation_status: "error"`, every
    // figure null. A MEASURED row rendered as "not computed" is the inversion
    // the operator's mandate removes.

    /// Fixture: BOTH pools V3 and catalogued at the same tier, the provider
    /// answering an IDENTITY quote → the two forward probes agree, the chained
    /// round trip returns exactly the probe back, and the engine measures the
    /// honest `spread_zero_equilibrium` (cycle gross exactly zero).
    ///
    /// SPREAD-SIGNED-DELTA-01: this fixture used `OkV3Mock` (a constant 1000 wei
    /// against a 1e18 probe) and CLAIMED the round trip returned the probe back.
    /// It did not: it lost ~1e18 units every time, and only `saturating_sub` made
    /// that indistinguishable from the equilibrium this doc describes. The
    /// identity quote makes the fixture produce the state it claims.
    async fn spread_zero_candidate(
        token_in: Address,
        token_out: Address,
        cfg: &TradingConfigState,
    ) -> StrategyCandidate {
        let catalog = Arc::new(V3FeeCatalog::new());
        catalog.record_observed(addr(0x10), token_in, token_out, 500);
        catalog.record_observed(addr(0x11), token_in, token_out, 500);
        let projector = Arc::new(StateProjector::new(
            Arc::new(ReservesCache::new()),
            Some(Arc::new(IdentityV3Mock)),
            catalog,
        ));
        let engine = DexEngine::new(Arc::new(ReservesCache::new()), None, Some(projector));
        let pool_a = make_pool(addr(0x10), token_in, token_out, ProtocolType::V3);
        let pool_b = make_pool(addr(0x11), token_in, token_out, ProtocolType::V3);
        let intent = make_intent(token_in, token_out);
        let impact = make_impact(vec![pool_a, pool_b]);

        let candidates = engine
            .build_from_impacted_pairs(&intent, &impact, Some(cfg))
            .await
            .expect("engine must not error");

        candidates
            .into_iter()
            .find(|c| c.rejection_reason.as_deref() == Some("spread_zero_equilibrium"))
            .expect("an identity round trip must measure a zero cycle gross")
    }

    /// SPREAD-SIGNED-DELTA-01: the other half of the partition. Same shape as
    /// `spread_zero_candidate`, but the provider keeps 30 bps per leg, so the
    /// forward probes still agree while the chained round trip returns strictly
    /// less than the probe — a LOSING cycle.
    async fn losing_round_trip_candidate(
        token_in: Address,
        token_out: Address,
        cfg: &TradingConfigState,
    ) -> StrategyCandidate {
        let catalog = Arc::new(V3FeeCatalog::new());
        catalog.record_observed(addr(0x10), token_in, token_out, 500);
        catalog.record_observed(addr(0x11), token_in, token_out, 500);
        let projector = Arc::new(StateProjector::new(
            Arc::new(ReservesCache::new()),
            Some(Arc::new(FeeTakingV3Mock)),
            catalog,
        ));
        let engine = DexEngine::new(Arc::new(ReservesCache::new()), None, Some(projector));
        let pool_a = make_pool(addr(0x10), token_in, token_out, ProtocolType::V3);
        let pool_b = make_pool(addr(0x11), token_in, token_out, ProtocolType::V3);
        let intent = make_intent(token_in, token_out);
        let impact = make_impact(vec![pool_a, pool_b]);

        let candidates = engine
            .build_from_impacted_pairs(&intent, &impact, Some(cfg))
            .await
            .expect("engine must not error");

        candidates
            .into_iter()
            .find(|c| c.rejection_reason.as_deref() == Some("spread_negative_round_trip"))
            .expect("a round trip that returns less must carry the loss verdict")
    }

    /// SPREAD-SIGNED-DELTA-01 — THE BIDIRECTIONAL PARTITION. A cycle that returns
    /// EXACTLY the probe and one that returns LESS are different market states:
    /// they must produce DIFFERENT verdicts, different persisted labels, and gross
    /// figures with different SIGNS. Before this change `saturating_sub` gave both
    /// the same zero (`spread_zero_equilibrium`), which is why the biggest channel
    /// in the system could not be audited — its label claimed an equilibrium for
    /// what was, in the majority of ticks, a measured loss.
    #[tokio::test]
    async fn losing_round_trip_and_exact_equilibrium_get_different_verdicts() {
        let weth: Address = "0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2"
            .parse()
            .expect("canonical WETH address");
        let usdc: Address = "0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48"
            .parse()
            .expect("canonical USDC address");
        let mut cfg = make_cfg();
        cfg.token_prices_usd = HashMap::from([("USDC".to_string(), 1.0)]);

        let equilibrium = spread_zero_candidate(weth, usdc, &cfg).await;
        let loss = losing_round_trip_candidate(weth, usdc, &cfg).await;

        // 1) DIFFERENT VERDICTS — the persisted reason stops conflating them.
        assert_eq!(
            equilibrium.rejection_reason.as_deref(),
            Some("spread_zero_equilibrium")
        );
        assert_eq!(
            loss.rejection_reason.as_deref(),
            Some("spread_negative_round_trip")
        );
        assert_ne!(
            equilibrium.rejection_reason, loss.rejection_reason,
            "a loss and an equilibrium must not share a verdict"
        );

        // 2) DIFFERENT MEASURED SIGNS — exactly zero vs strictly negative. R8:
        //    neither is invented and neither is rounded into the other.
        assert_eq!(
            equilibrium.gross_profit_usd,
            Some(0.0),
            "the equilibrium is a MEASURED zero"
        );
        let loss_gross = loss
            .gross_profit_usd
            .expect("the loss is MEASURED, not withheld");
        assert!(
            loss_gross < 0.0,
            "a cycle that returns less than the probe is a loss, got {loss_gross}"
        );

        // 3) Both stay on the closed-economics path with their own figure.
        let eq_econ = equilibrium
            .opportunity
            .economics
            .as_ref()
            .expect("measured equilibrium ⇒ object");
        let loss_econ = loss
            .opportunity
            .economics
            .as_ref()
            .expect("measured loss ⇒ object");
        assert_eq!(eq_econ.computation_status, "computed");
        assert_eq!(loss_econ.computation_status, "computed");
        assert_eq!(eq_econ.gross_profit_usd, Some(0.0));
        assert_eq!(loss_econ.gross_profit_usd, Some(loss_gross));

        // 4) D2: the REAL cycle output is published (computed, not derived). The
        //    equilibrium returned exactly the 1 WETH probe; the loss returned the
        //    mock's measured 0.997² of it.
        const PROBE_UNITS: f64 = 1.0; // 10^18 wei of an 18-decimals token
        assert!(
            (equilibrium.candidate.expected_amount_out - PROBE_UNITS).abs() < 1e-9,
            "equilibrium output must BE the probe, got {}",
            equilibrium.candidate.expected_amount_out
        );
        assert!(
            !loss.candidate.expected_amount_out.is_nan(),
            "a MEASURED output must not stay NaN"
        );
        let measured_loss_out = 0.997_f64 * 0.997_f64;
        assert!(
            (loss.candidate.expected_amount_out - measured_loss_out).abs() < 1e-6,
            "loss output must be the measured round trip (0.997²), got {}",
            loss.candidate.expected_amount_out
        );
        assert!(
            loss.candidate.expected_amount_out < PROBE_UNITS,
            "a losing round trip publishes an output BELOW the probe, got {}",
            loss.candidate.expected_amount_out
        );
    }

    /// Regression on the production defect: a rejected row whose gross IS
    /// measured carries the CLOSED `computed` object — amount_in, gross, REAL
    /// costs, explicit negative net and the matching roi — and keeps it through
    /// the emit boundary.
    #[tokio::test]
    async fn measured_gross_rejection_carries_closed_computed_economics() {
        let weth: Address = "0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2"
            .parse()
            .expect("canonical WETH address");
        let usdc: Address = "0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48"
            .parse()
            .expect("canonical USDC address");
        let mut cfg = make_cfg();
        // Non-zero ops overhead so the sheet-07 OtherCost column is really
        // exercised (a 0.0 would hide a dropped component).
        cfg.ops_overhead_usd_per_attempt = 0.25;
        cfg.token_prices_usd = HashMap::from([("USDC".to_string(), 1.0)]);

        let c = spread_zero_candidate(weth, usdc, &cfg).await;
        assert_eq!(
            c.gross_profit_usd,
            Some(0.0),
            "fixture must MEASURE a zero cycle gross"
        );

        let e = c
            .opportunity
            .economics
            .as_ref()
            .expect("a MEASURED gross must carry the economics object");
        assert_eq!(
            e.computation_status, "computed",
            "measured gross ⇒ computed (never error/partial)"
        );
        assert_eq!(e.error_reason, None);
        assert_eq!(
            e.gross_profit_usd,
            Some(0.0),
            "the measured zero is published as computed-and-exactly-zero"
        );

        // Principal = ONE native unit of token_in (the probe the gross was
        // measured at), priced by the same live source the gross conversion uses.
        let amount_in_usd = e.amount_in_usd.expect("priced principal");
        assert!(
            (amount_in_usd - 3_000.0).abs() < 1e-9,
            "1 WETH at the configured base price must be $3000, got {amount_in_usd}"
        );
        assert_eq!(
            c.opportunity.amount_in_wei,
            U256::from(10u128).pow(U256::from(18u32)).to_string(),
            "the row records the principal the gross was measured at"
        );

        // REAL costs — the gas figure IS the config oracle's, no literal.
        let gas = e.gas_usd.expect("gas from the real oracle");
        assert!(
            (gas - cfg.gas_cost_usd()).abs() < 1e-9,
            "gas must be TradingConfigState::gas_cost_usd(), got {gas} vs {}",
            cfg.gas_cost_usd()
        );
        assert!(gas > 0.0, "200k gas × 20 gwei × $3000 must be positive");
        assert_eq!(
            e.other_costs_usd,
            Some(cfg.ops_overhead_usd_per_attempt),
            "OtherCost comes from the operator config, never a table"
        );
        assert_eq!(
            e.flash_fee_usd,
            Some(0.0),
            "no flash wrapper at engine altitude ⇒ own capital ⇒ zero fee (computed)"
        );
        assert_eq!(e.bribe_usd, Some(0.0), "no bid path ⇒ computed zero");
        let total = e.total_cost_usd.expect("total_cost must be present");
        assert!(total.is_finite() && total > 0.0, "total={total}");
        assert!(
            (total - (gas + 0.25)).abs() < 1e-9,
            "total must be the sum of the present components, got {total}"
        );

        // Rule 4: the loss is EXPLICIT — Some, finite and negative.
        let net = e
            .net_profit_usd
            .expect("net must be Some on a closed loss, never None");
        assert!(
            net.is_finite() && net < 0.0,
            "zero gross minus real costs must be negative, got {net}"
        );
        assert!((net - (0.0 - total)).abs() < 1e-9, "net vs gross − costs");

        // roi consistent with net / amount_in × 100.
        let roi = e.roi_pct.expect("roi of a priced principal");
        assert!(
            (roi - net / amount_in_usd * 100.0).abs() < 1e-12,
            "roi {roi} must equal net/amount_in×100"
        );

        // Row and payload are one closure (no drift between the two layers).
        assert_eq!(c.opportunity.expected_profit_usd, Some(0.0));
        assert_eq!(c.opportunity.net_expected_profit_usd, Some(net));

        // The emit boundary (opportunity_emitter::stamped_for_emit →
        // economics::stamp_on_emit) keeps the producer object verbatim.
        let mut wire = c.opportunity.clone();
        crate::economics::stamp_on_emit(&mut wire);
        let stamped = wire
            .economics
            .as_ref()
            .expect("the object survives the boundary");
        assert_eq!(stamped.computation_status, "computed");
        assert_eq!(stamped.net_profit_usd, Some(net));
        assert_eq!(stamped.total_cost_usd, Some(total));
    }

    /// The other half of the contract: a rejected row WITHOUT a measured gross
    /// keeps its diagnostic status and its verbatim reason — no figures are
    /// invented for it.
    #[tokio::test]
    async fn unmeasured_gross_rejection_keeps_its_diagnostic_status() {
        // PRICED entry token (WETH ⇒ base_token_price_usd) so the ONLY thing that
        // can withhold the closed object on this row is the missing GROSS — the
        // discrimination the test is about.
        let tok_a: Address = "0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2"
            .parse()
            .expect("canonical WETH address");
        let tok_b: Address = "0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48"
            .parse()
            .expect("canonical USDC address");
        // Pool B absent from the by_pool index → leg B resolves
        // `v3_pool_not_catalogued` with ZERO RPC: no quote, so no gross.
        let catalog = Arc::new(V3FeeCatalog::new());
        catalog.record_observed(addr(0x10), tok_a, tok_b, 3_000);
        let projector = Arc::new(StateProjector::new(
            Arc::new(ReservesCache::new()),
            Some(Arc::new(OkV3Mock)),
            catalog,
        ));
        let engine = DexEngine::new(Arc::new(ReservesCache::new()), None, Some(projector));
        let pool_a = make_pool(addr(0x10), tok_a, tok_b, ProtocolType::V3);
        let pool_b = make_pool(addr(0x11), tok_a, tok_b, ProtocolType::V3);
        let intent = make_intent(tok_a, tok_b);
        let impact = make_impact(vec![pool_a, pool_b]);
        let cfg = make_cfg();

        let candidates = engine
            .build_from_impacted_pairs(&intent, &impact, Some(&cfg))
            .await
            .expect("engine must not error");
        let c = candidates
            .iter()
            .find(|c| c.rejection_reason.as_deref() == Some("v3_pool_not_catalogued"))
            .expect("catalog gap must produce its precise-label candidate");

        assert!(
            c.gross_profit_usd.is_none(),
            "no quote ⇒ the gross is NOT measured"
        );
        assert!(
            c.opportunity.economics.is_none(),
            "the engine must not attach an economics object without a measured gross"
        );

        // Orchestrator engine-rejection arm (reason onto the row) + emit
        // boundary: the row keeps a DECLARED absence naming the reason.
        let mut row = c.opportunity.clone();
        row.rejection_reason = c.rejection_reason.clone();
        crate::economics::stamp_on_emit(&mut row);
        let e = row
            .economics
            .as_ref()
            .expect("the boundary stamps the honest object");
        assert_eq!(e.computation_status, "error");
        assert_eq!(e.error_reason.as_deref(), Some("v3_pool_not_catalogued"));
        assert!(
            e.amount_in_usd.is_none()
                && e.gross_profit_usd.is_none()
                && e.total_cost_usd.is_none()
                && e.net_profit_usd.is_none()
                && e.roi_pct.is_none(),
            "a not-computed row carries NO figure (R8)"
        );
        assert_eq!(row.expected_profit_usd, None);
        assert_eq!(row.net_expected_profit_usd, None);
    }

    /// Anti-literal guard: the closed object's gas figure TRACKS the operator's
    /// gas oracle — raise the configured gas price 3× and the row's gas, total
    /// cost and net follow. A hardcoded gas figure (or table) cannot pass this.
    #[tokio::test]
    async fn measured_gross_cost_tracks_the_real_gas_oracle() {
        let weth: Address = "0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2"
            .parse()
            .expect("canonical WETH address");
        let usdc: Address = "0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48"
            .parse()
            .expect("canonical USDC address");
        let mut cfg = make_cfg();
        cfg.ops_overhead_usd_per_attempt = 0.25;

        let base = spread_zero_candidate(weth, usdc, &cfg).await;
        let base_econ = base
            .opportunity
            .economics
            .as_ref()
            .expect("measured gross ⇒ object");
        let g1 = base_econ.gas_usd.expect("gas");
        let total1 = base_econ.total_cost_usd.expect("total");
        let net1 = base_econ.net_profit_usd.expect("net");

        // Same gas units, 3× the price the operator configured.
        cfg.fixed_gas_price_gwei = Some(60.0);
        let tripled = spread_zero_candidate(weth, usdc, &cfg).await;
        let tripled_econ = tripled
            .opportunity
            .economics
            .as_ref()
            .expect("measured gross ⇒ object");
        let g3 = tripled_econ.gas_usd.expect("gas");
        let total3 = tripled_econ.total_cost_usd.expect("total");
        let net3 = tripled_econ.net_profit_usd.expect("net");

        assert!(g1 > 0.0, "fixture gas must be priced, got {g1}");
        assert!(
            (g3 - 3.0 * g1).abs() < 1e-9,
            "gas must scale with the config oracle: {g1} → {g3}"
        );
        assert!((total1 - (g1 + 0.25)).abs() < 1e-9, "total1={total1}");
        assert!((total3 - (g3 + 0.25)).abs() < 1e-9, "total3={total3}");
        assert!(
            net3 < net1 && net3 < 0.0,
            "a 3× gas price must worsen the closed loss: {net1} → {net3}"
        );
    }

    /// Boundary, documented: a MEASURED gross whose entry token has no live USD
    /// price cannot be closed — the principal would have to be invented. The row
    /// then keeps its honest `error` + verbatim reason instead of a fabricated
    /// capital figure (R8 beats a cosmetic "computed").
    #[tokio::test]
    async fn measured_gross_with_unpriced_principal_never_invents_capital() {
        let tok_a = addr(0x1); // not in the canonical price table
        let tok_b = addr(0x2);
        let cfg = make_cfg();

        let c = spread_zero_candidate(tok_a, tok_b, &cfg).await;
        assert_eq!(
            c.gross_profit_usd,
            Some(0.0),
            "the cycle measurement happened..."
        );
        assert!(
            c.opportunity.economics.is_none(),
            "...but an unpriced principal has no closed arithmetic to claim (R8)"
        );
        assert!(
            c.opportunity.expected_profit_usd.is_none(),
            "no USD figure is invented for the row either"
        );

        let mut row = c.opportunity.clone();
        row.rejection_reason = c.rejection_reason.clone();
        crate::economics::stamp_on_emit(&mut row);
        let e = row.economics.as_ref().expect("object");
        assert_eq!(
            e.computation_status, "error",
            "no price ⇒ honest not-computed, never a fabricated $ principal"
        );
        assert_eq!(e.error_reason.as_deref(), Some("spread_zero_equilibrium"));
        assert!(e.amount_in_usd.is_none() && e.net_profit_usd.is_none());
    }

    /// ECON-AMOUNT-DENOM-01 — the principal is denominated in the ROW's own base
    /// token, and the partition holds for BOTH units.
    ///
    /// The live defect this pins (measured 2026-10-04 on production,
    /// `rejection_reason='spread_zero_equilibrium'`, 1 h window): in the SAME row
    ///   `economics.amount_in_wei`               = 1000000000000000000  (constant)
    ///   `route_metadata.economics_amount_in_wei` = 1000000
    /// with ONE base token behind the second group (USDC, 6 decimals; 10,167 rows),
    /// the probe identical (1e18) across all 13 cycle families, and the card
    /// showing $2,693.71 — the intent token's own price — where the real probe is
    /// 1 USDC = $1.00.
    ///
    /// The fixture forces exactly that divergence: the intent enters a token the
    /// evaluated pair does NOT contain, so the engine's own `economic_base_token`
    /// fallback (the pair's `token0`) is the only honest denomination. One
    /// 6-decimals pair and one 18-decimals pair prove the partition — a
    /// single-token test cannot.
    #[tokio::test]
    async fn measured_principal_is_denominated_in_the_rows_base_token_in_both_units() {
        let usdc: Address = "0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48"
            .parse()
            .expect("canonical USDC address");
        let usdt: Address = "0xdac17f958d2ee523a2206206994597c13d831ec7"
            .parse()
            .expect("canonical USDT address");
        let weth: Address = "0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2"
            .parse()
            .expect("canonical WETH address");
        let dai: Address = "0x6b175474e89094c44da98b954eedeac495271d0f"
            .parse()
            .expect("canonical DAI address");

        // The denomination RULE itself, independent of engine plumbing: the intent's
        // token is foreign to the pair ⇒ the pair's base denominates.
        let pool_usdc_usdt = make_pool(addr(0x10), usdc, usdt, ProtocolType::V3);
        let foreign_intent = make_intent(weth, usdt);
        assert_eq!(
            economic_base_token(&pool_usdc_usdt, &foreign_intent),
            usdc,
            "an intent entering a token the pair does not contain denominates in the pair's base"
        );
        assert_eq!(canonical_token_decimals(Some(usdc)), 6);
        assert_eq!(canonical_token_decimals(Some(weth)), 18);

        let mut cfg = make_cfg();
        cfg.token_prices_usd = HashMap::from([
            ("USDC".to_string(), 1.0),
            ("USDT".to_string(), 1.0),
            ("DAI".to_string(), 1.0),
        ]);

        // 6 decimals: 1 USDC = 1e6 raw, priced at its OWN live print.
        let six = measured_principal_for_pair(usdc, usdt, weth, &cfg).await;
        assert_eq!(
            six.opportunity.amount_in_wei, "1000000",
            "the principal must be 1 USDC = 1e6 raw units, never a blanket 1e18"
        );
        let six_usd = six
            .opportunity
            .economics
            .as_ref()
            .expect("measured gross ⇒ object")
            .amount_in_usd
            .expect("the base token is priced");
        assert!(
            (six_usd - 1.0).abs() < 1e-9,
            "1 USDC must be valued at its OWN price ($1.00), got {six_usd}"
        );

        // 18 decimals: 1 WETH = 1e18 raw, priced by the configured base price.
        let eighteen = measured_principal_for_pair(weth, dai, usdc, &cfg).await;
        assert_eq!(
            eighteen.opportunity.amount_in_wei, "1000000000000000000",
            "the principal must be 1 WETH = 1e18 raw units"
        );
        let eighteen_usd = eighteen
            .opportunity
            .economics
            .as_ref()
            .expect("measured gross ⇒ object")
            .amount_in_usd
            .expect("the base token is priced");
        assert!(
            (eighteen_usd - cfg.base_token_price_usd).abs() < 1e-9,
            "1 WETH must be valued at the configured base price, got {eighteen_usd}"
        );

        assert_ne!(
            six.opportunity.amount_in_wei, eighteen.opportunity.amount_in_wei,
            "the two units must not collapse into the same principal"
        );
    }

    /// Fixture for the denomination test: a V3 pair (both pools at the same
    /// catalogued tier, provider answering a constant quote) evaluated with an
    /// intent that enters `intent_token` — which the caller makes FOREIGN to the
    /// pair, the live shape behind D4.
    ///
    /// SPREAD-SIGNED-DELTA-01 reconciliation (2026-10-05): `OkV3Mock` answers
    /// 1000 wei to a 1e18 probe, so this fixture's chained round trip returns
    /// far LESS than the probe — a real, measured loss. Before #792 separated
    /// loss from equilibrium, `saturating_sub` erased exactly that into
    /// `spread_zero_equilibrium`; this selector used to ask for the erased
    /// label. It now names the verdict that actually measures the case.
    async fn measured_principal_for_pair(
        pair_token0: Address,
        pair_token1: Address,
        intent_token: Address,
        cfg: &TradingConfigState,
    ) -> StrategyCandidate {
        let catalog = Arc::new(V3FeeCatalog::new());
        catalog.record_observed(addr(0x10), pair_token0, pair_token1, 500);
        catalog.record_observed(addr(0x11), pair_token0, pair_token1, 500);
        let projector = Arc::new(StateProjector::new(
            Arc::new(ReservesCache::new()),
            Some(Arc::new(OkV3Mock)),
            catalog,
        ));
        let engine = DexEngine::new(Arc::new(ReservesCache::new()), None, Some(projector));
        let pool_a = make_pool(addr(0x10), pair_token0, pair_token1, ProtocolType::V3);
        let pool_b = make_pool(addr(0x11), pair_token0, pair_token1, ProtocolType::V3);
        let intent = make_intent(intent_token, pair_token1);
        let impact = make_impact(vec![pool_a, pool_b]);

        let candidates = engine
            .build_from_impacted_pairs(&intent, &impact, Some(cfg))
            .await
            .expect("engine must not error");

        candidates
            .into_iter()
            .find(|c| c.rejection_reason.as_deref() == Some("spread_negative_round_trip"))
            .expect("a constant quote MEASURES a loss, never an equilibrium")
    }
}
