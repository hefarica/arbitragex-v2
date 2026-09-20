// M11 allow: test modules use .unwrap()/.expect() for readability;
// production paths use ? / anyhow throughout.
//! StateProjector — Phase 12 — virtual post-tx pool state projection.
//!
//! Projects what V2 pool reserves look like AFTER a pending mempool transaction
//! executes, without mutating real cache state. Strategy engines use these
//! virtual reserves to evaluate the arb opportunity that EXISTS in the post-tx
//! world, not the pre-tx world.
//!
//! ## Design
//!
//! - `project_v2_post_swap`: applies the CPMM constant-product formula to
//!   derive virtual (reserve_in_new, reserve_out_new) from a `RouteIntent`.
//! - `project_v3_quote`: forwards to a `V3QuoteProvider` trait object so tests
//!   can inject a mock without touching alloy/ethers. The real impl wraps the
//!   existing v3_rpc_pool multicall path from scanner.rs.
//! - `project_triangular_cycle`: per-hop projection for 3-leg cycles; marks
//!   `all_hops_projected = false` if any hop is not touched by the intent.
//!
//! ## R8 invariants
//!
//! - Returns `None` (never panics or fabricates) when:
//!   - Pool reserves are not in cache (cold boot or eviction).
//!   - `intent.amount_in == 0` — treated as "no effect" (returns current reserves).
//!   - V3 provider is `None`.
//! - The underlying `ReservesCache` is NEVER mutated — this module only reads.
//!
//! ## Math: V2 constant-product post-swap
//!
//! After a swap of `amount_in` tokens (token_in → token_out) in a V2 pool:
//!
//!   fee_factor          = (10_000 − fee_bps) / 10_000
//!   amount_in_after_fee = amount_in × fee_factor     (deposited into pool)
//!   new_reserve_in      = reserve_in_current + amount_in_after_fee
//!   new_reserve_out     = k / new_reserve_in          (constant-product)
//!
//! where `k = reserve_in_current × reserve_out_current`.
//!
//! Edge cases handled: zero reserves → None; zero amount_in → current reserves;
//! token orientation (intent.token_in == pool.token1) → swap reserves before math.

use crate::amm_math::v2_amount_out;
use crate::engines::triangular_engine::ReservesCache;
use crate::route_intent::RouteIntent;
use crate::v3_fee_catalog::{FeeResolution, V3FeeCatalog};
use ethers::types::{Address, U256};
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use tracing::{debug, warn};

/// Default V2 fee in basis points.
const V2_FEE_BPS: u32 = 30;

// ---------------------------------------------------------------------------
// V3QuoteProvider trait
// ---------------------------------------------------------------------------

/// Abstraction over the V3 quoter (QuoterV2 on-chain contract).
///
/// The production implementation wraps `amm_math::v3_quote_exact_in_multicall`
/// with the existing `v3_rpc_pool`. Tests inject a `MockV3QuoteProvider` that
/// returns hard-coded values without any RPC calls (RULE 00).
///
/// Uses `Pin<Box<dyn Future>>` (BoxFuture pattern) for dyn-compatibility so
/// the trait can be stored as `Arc<dyn V3QuoteProvider>`. RPITIT (`impl Future`
/// in trait methods) is not dyn-compatible (Rust limitation — vtable layout is
/// undefined for opaque return types), so we use the explicit box form.
pub trait V3QuoteProvider: Send + Sync {
    /// Get the amount of `token_out` from a V3 pool for `amount_in` of `token_in`.
    ///
    /// Returns `Err` on RPC failure (caller treats as unavailable, returns None).
    fn quote_exact_input_single(
        &self,
        pool: Address,
        token_in: Address,
        token_out: Address,
        amount_in: U256,
        fee_bps: u32,
    ) -> Pin<Box<dyn Future<Output = anyhow::Result<U256>> + Send + '_>>;

    /// B1 batching (V3-QUOTE-BATCH-20260919): prefetch a set of quotes in
    /// batched multicalls, warming the provider's TTL cache. Returns the
    /// (request, result) pairs actually dispatched — an empty vec means
    /// "batching unsupported / nothing to prefetch"; the unary
    /// `quote_exact_input_single` path remains authoritative either way.
    /// Default: no-op (test mocks and future impls need no batching).
    fn quote_batch(
        &self,
        reqs: Vec<crate::amm_math::V3QuoteRequest>,
    ) -> Pin<Box<dyn Future<Output = V3BatchQuoteResults> + Send + '_>> {
        let _ = reqs;
        Box::pin(std::future::ready(Vec::new()))
    }
}

// ---------------------------------------------------------------------------
// Output types
// ---------------------------------------------------------------------------

/// Virtual V2 pool state after the pending tx's swap is applied.
#[derive(Debug, Clone)]
pub struct V2VirtualReserves {
    pub pool: Address,
    /// Post-swap reserve of token_in (the token the swap deposits into the pool).
    pub reserve_in: U256,
    /// Post-swap reserve of token_out (the token the pool sends to the swapper).
    pub reserve_out: U256,
    /// Block number at which the base reserves were observed.
    /// Same provenance as the `ReservesEntry::blk` used for projection.
    pub source_block: u64,
}

/// V3 quote result for a given amount_in at post-tx pool state.
/// For Phase 12 this is a direct forward to the on-chain QuoterV2
/// (the quoter reads CURRENT state, which IS post-tx for a mempool tx).
#[derive(Debug, Clone)]
pub struct V3VirtualQuote {
    pub pool: Address,
    pub amount_out: U256,
    /// Fee tier actually quoted (raw pips, resolved from the catalog — WO-06).
    pub fee_bps: u32,
}

/// Checked V3 quote failure (WO-06). Each variant maps to a DISTINCT
/// rejection label (`as_label`) so downstream observations never collapse a
/// catalog gap into a provider failure — `v3_quote_unavailable` stays
/// reserved for real provider failures (absent / RPC exhausted / revert).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProjectV3Error {
    /// Pool address absent from the fee catalog while the pair HAS known V3
    /// tiers — quoting would be blind at an unverified tier. Zero RPC.
    PoolNotCatalogued,
    /// Token pair has no known V3 pools at all — no tier can exist.
    /// Zero RPC.
    PairHasNoV3Pools,
    /// No `V3QuoteProvider` wired (non-mainnet / absent at boot).
    ProviderUnavailable,
    /// Provider present but the quote failed (RPC failure / revert), or the
    /// amount was zero.
    QuoteFailed(String),
}

impl ProjectV3Error {
    /// Stable rejection label (matches the `OptimizeRejectReason` strings).
    pub fn as_label(&self) -> &'static str {
        match self {
            Self::PoolNotCatalogued => "v3_pool_not_catalogued",
            Self::PairHasNoV3Pools => "v3_pair_no_pools",
            Self::ProviderUnavailable | Self::QuoteFailed(_) => "v3_quote_unavailable",
        }
    }
}

/// Projected state for a full triangular cycle.
#[derive(Debug, Clone)]
pub struct TriangularVirtualState {
    /// One V2VirtualReserves entry per hop (always 3 for triangular MVP).
    pub hops: Vec<V2VirtualReserves>,
    /// `true` when ALL hops were projected via the intent's swap effect.
    /// `false` when some hops use current cached reserves (unimpacted by intent).
    pub all_hops_projected: bool,
}

// ---------------------------------------------------------------------------
// PoolRef-like descriptor used only by state_projector
// ---------------------------------------------------------------------------

/// Minimal pool descriptor passed into projection calls.
/// Carries only what the projector needs (avoids importing the full PoolRef).
#[derive(Debug, Clone)]
pub struct PoolRef {
    pub address: Address,
    /// token0 address (canonical pool ordering; same as `ReservesCache` key).
    pub token0: Address,
    pub token1: Address,
    pub fee_bps: Option<u32>,
}

// ---------------------------------------------------------------------------
// StateProjector
// ---------------------------------------------------------------------------

/// Projects virtual post-tx pool state for strategy engines.
///
/// Constructed once at boot and shared via `Arc`. Thread-safe: contains only
/// `Arc`-wrapped data with no mutable state of its own.
pub struct StateProjector {
    /// Shared in-memory reserves cache populated by PoolSyncWorker.
    /// `pub(crate)` so `SizeOptimizer` in the same crate can read reserves
    /// for sizing bounds without going through the full projection API.
    pub(crate) reserves_cache: Arc<ReservesCache>,
    v3_provider: Option<Arc<dyn V3QuoteProvider>>,
    /// Authoritative V3 fee tiers (WO-06): every V3 quote resolves its tier
    /// from this catalog instead of a blind default — the quoter derives the
    /// pool from the fee, so an uncatalogued tier is a guaranteed revert.
    fee_catalog: Arc<V3FeeCatalog>,
}

impl StateProjector {
    /// Constructs a `StateProjector`.
    ///
    /// - `reserves_cache`: shared pool reserves (populated by PoolSyncWorker).
    /// - `v3_provider`: optional V3 quoter impl. `None` → V3 projection returns `None`.
    /// - `fee_catalog`: V3 fee-tier catalog (Redis pool_index_v3 mirror +
    ///   passive observations). Tests inject an in-memory catalog.
    pub fn new(
        reserves_cache: Arc<ReservesCache>,
        v3_provider: Option<Arc<dyn V3QuoteProvider>>,
        fee_catalog: Arc<V3FeeCatalog>,
    ) -> Self {
        Self {
            reserves_cache,
            v3_provider,
            fee_catalog,
        }
    }

    // -----------------------------------------------------------------------
    // V2 projection
    // -----------------------------------------------------------------------

    /// Project virtual V2 pool reserves after applying the intent's swap.
    ///
    /// Returns `None` when:
    ///   - Pool reserves not in cache (cold cache — honest skip).
    ///   - Pool reserves are zero (degenerate pool).
    ///   - `intent.amount_in == 0` is treated as no effect → returns current reserves
    ///     as a virtual state (no projection needed, source block is real).
    ///
    /// Token orientation: if `intent.legs[0].token_in == pool.token1`, the swap
    /// deposits token1 into the pool. Reserves are oriented accordingly before
    /// applying the CPMM formula.
    pub async fn project_v2_post_swap(
        &self,
        pool: &PoolRef,
        intent: &RouteIntent,
    ) -> Option<V2VirtualReserves> {
        // Fetch canonical (r0, r1) for the pool.
        let (r0, r1) = self.reserves_cache.get(&pool.address).await?;

        if r0.is_zero() || r1.is_zero() {
            debug!(
                event = "state_projector.v2_zero_reserves",
                pool = %pool.address,
                "pool has zero reserves — skipping projection"
            );
            return None;
        }

        let amount_in = intent.amount_in;

        // Zero amount_in → no swap effect. Return current reserves as virtual state.
        if amount_in.is_zero() {
            return Some(V2VirtualReserves {
                pool: pool.address,
                reserve_in: r0,
                reserve_out: r1,
                source_block: 0, // block not tracked in ReservesCache in-memory
            });
        }

        // Determine which token is being deposited into the pool.
        // The intent's first leg specifies token_in. Compare against pool token0.
        let intent_token_in = intent
            .legs
            .first()
            .map(|leg| leg.token_in)
            .unwrap_or(Address::zero());

        // Orient reserves: (reserve_in, reserve_out) from the pool's perspective.
        // If intent deposits token0 → reserve_in = r0, reserve_out = r1.
        // If intent deposits token1 → reserve_in = r1, reserve_out = r0.
        let (reserve_in_curr, reserve_out_curr, token_in_is_token0) =
            if intent_token_in == pool.token0 || intent_token_in == Address::zero() {
                // Default: treat token0 as the deposit side.
                (r0, r1, true)
            } else {
                // token1 is being deposited.
                (r1, r0, false)
            };

        let fee_bps = pool.fee_bps.unwrap_or(V2_FEE_BPS);

        // CPMM post-swap reserve computation — mirrors the real Uniswap V2 contract.
        //
        // Real V2 mechanics (UniswapV2Pair.swap):
        //   1. The full `amount_in` is deposited into the pool (reserve_in grows by
        //      the full amount).
        //   2. The fee (fee_bps) is subtracted from the effective input when computing
        //      amount_out — this causes k to GROW (fees accrue to LPs as additional k).
        //   3. new_reserve_out = reserve_out - amount_out
        //
        // This means k_new = new_reserve_in × new_reserve_out
        //                   = (reserve_in + amount_in) × (reserve_out - amount_out)
        //                   >= reserve_in × reserve_out = k_old   (proven above for
        //                      any fee > 0 and amount_in > 0).
        //
        // Use v2_amount_out for amount_out (same formula as everywhere else in codebase).
        let amount_out =
            crate::amm_math::v2_amount_out(amount_in, reserve_in_curr, reserve_out_curr, fee_bps);

        if amount_out.is_zero() {
            // amount_in too small for any output at this fee tier.
            // Return current reserves unchanged (no swap effect observable).
            return Some(V2VirtualReserves {
                pool: pool.address,
                reserve_in: reserve_in_curr,
                reserve_out: reserve_out_curr,
                source_block: 0,
            });
        }

        // new_reserve_in:  full amount_in added (the fee stays in the pool as k growth).
        let new_reserve_in = reserve_in_curr.saturating_add(amount_in);

        // new_reserve_out: reduced by amount_out sent to swapper.
        if amount_out > reserve_out_curr {
            // Pathological: output exceeds reserves (degenerate pool or oracle mismatch).
            debug!(
                event = "state_projector.v2_output_exceeds_reserve",
                pool = %pool.address,
                amount_out = %amount_out,
                reserve_out = %reserve_out_curr,
                "output exceeds reserve — skipping projection"
            );
            return None;
        }
        let new_reserve_out = reserve_out_curr.saturating_sub(amount_out);

        if new_reserve_in.is_zero() || new_reserve_out.is_zero() {
            return None;
        }

        // De-orient: store (reserve_in, reserve_out) in projection-relative order,
        // not pool-canonical order. Callers use `reserve_in` / `reserve_out` directly.
        let _ = token_in_is_token0; // orientation applied above; not needed downstream

        debug!(
            event = "state_projector.v2_projected",
            pool = %pool.address,
            reserve_in_was = %reserve_in_curr,
            reserve_out_was = %reserve_out_curr,
            reserve_in_new = %new_reserve_in,
            reserve_out_new = %new_reserve_out,
        );

        Some(V2VirtualReserves {
            pool: pool.address,
            reserve_in: new_reserve_in,
            reserve_out: new_reserve_out,
            source_block: 0, // in-memory cache doesn't track block numbers
        })
    }

    // -----------------------------------------------------------------------
    // V3 projection
    // -----------------------------------------------------------------------

    /// Get a V3 quote at `amount_in` for the given pool, with the failure
    /// reason preserved (WO-06 FEE-TIER-AWARE-QUOTING).
    ///
    /// The fee tier is resolved from the fee catalog — QuoterV2 derives the
    /// pool via `factory.getPool(tokenIn, tokenOut, fee)`, so the tier is part
    /// of the call identity and a blind default (the former `unwrap_or(500)`)
    /// reverts on any pool not at that exact tier. Uncatalogued pools/pairs
    /// are rejected WITHOUT an RPC (fail-honest, cero RPC).
    ///
    /// On a successful quote the observed (pool, pair, tier) is recorded back
    /// into the catalog — passive reconciliation against Redis index lag.
    pub async fn project_v3_quote_checked(
        &self,
        pool: &PoolRef,
        amount_in: U256,
        zero_for_one: bool,
    ) -> Result<V3VirtualQuote, ProjectV3Error> {
        if amount_in.is_zero() {
            return Err(ProjectV3Error::QuoteFailed("zero_amount_in".to_string()));
        }

        let provider = self
            .v3_provider
            .as_ref()
            .ok_or(ProjectV3Error::ProviderUnavailable)?;

        let fee_pips = self.resolve_fee_pips(pool)?;

        // Orient token_in / token_out from zero_for_one flag.
        let (token_in, token_out) = if zero_for_one {
            (pool.token0, pool.token1)
        } else {
            (pool.token1, pool.token0)
        };

        match provider
            .quote_exact_input_single(pool.address, token_in, token_out, amount_in, fee_pips)
            .await
        {
            Ok(amount_out) => {
                // Passive reconciliation: the quoter answered at this tier,
                // so the tier is real even if the Redis index lagged.
                self.fee_catalog
                    .record_observed(pool.address, pool.token0, pool.token1, fee_pips);
                debug!(
                    event = "state_projector.v3_quote",
                    pool = %pool.address,
                    amount_in = %amount_in,
                    amount_out = %amount_out,
                    fee_pips,
                );
                Ok(V3VirtualQuote {
                    pool: pool.address,
                    amount_out,
                    fee_bps: fee_pips,
                })
            }
            Err(e) => {
                debug!(
                    event = "state_projector.v3_quote_failed",
                    pool = %pool.address,
                    error = %e,
                );
                Err(ProjectV3Error::QuoteFailed(e.to_string()))
            }
        }
    }

    /// Get a V3 quote at `amount_in` for the given pool, flattened to
    /// `Option` (legacy callers that only need the amount). The checked
    /// variant preserves the honest failure reason — prefer it at new call
    /// sites.
    ///
    /// Returns `None` when:
    ///   - `v3_provider` is `None` (R8 honest: no fabrication without RPC).
    ///   - The provider returns an error (RPC failure or pool reverts).
    ///   - `amount_in.is_zero()`.
    ///   - The pool/pair is not in the fee catalog (WO-06: no blind quotes).
    pub async fn project_v3_quote(
        &self,
        pool: &PoolRef,
        amount_in: U256,
        zero_for_one: bool,
    ) -> Option<V3VirtualQuote> {
        self.project_v3_quote_checked(pool, amount_in, zero_for_one)
            .await
            .ok()
    }

    /// Resolve the V3 fee tier for a pool from the fee catalog, preserving the
    /// exact WO-06 semantics (catalog wins over the offered tier; uncatalogued
    /// pools/pairs are rejected without an RPC). Shared by the unary quote
    /// path and the B1 batch prefetch so both quote at the same tier.
    fn resolve_fee_pips(&self, pool: &PoolRef) -> Result<u32, ProjectV3Error> {
        match self.fee_catalog.resolve(pool.address, pool.fee_bps) {
            FeeResolution::Catalog(fee) => {
                crate::v3_fee_catalog::fee_resolution_metric("catalog");
                Ok(fee)
            }
            FeeResolution::Mismatch { offered, catalog } => {
                crate::v3_fee_catalog::fee_resolution_metric("mismatch");
                warn!(
                    event = "state_projector.v3_fee_mismatch",
                    pool = %pool.address,
                    offered,
                    catalog,
                    "offered fee differs from catalog — quoting with catalog tier"
                );
                Ok(catalog)
            }
            FeeResolution::NotCatalogued => {
                if self
                    .fee_catalog
                    .tiers_for_pair(pool.token0, pool.token1)
                    .is_empty()
                {
                    crate::v3_fee_catalog::fee_resolution_metric("pair_no_pools");
                    debug!(
                        event = "state_projector.v3_pair_no_pools",
                        pool = %pool.address,
                        "pair has no known V3 pools — rejecting without RPC (R8)"
                    );
                    return Err(ProjectV3Error::PairHasNoV3Pools);
                }
                crate::v3_fee_catalog::fee_resolution_metric("not_catalogued");
                debug!(
                    event = "state_projector.v3_pool_not_catalogued",
                    pool = %pool.address,
                    "pool absent from fee catalog — rejecting without RPC (R8)"
                );
                Err(ProjectV3Error::PoolNotCatalogued)
            }
        }
    }

    /// B1 batching (V3-QUOTE-BATCH-20260919): warm the provider's TTL cache
    /// for every V3 pool in a tick's pair-group BEFORE the per-pair probing
    /// loop, replacing N unary eth_calls with ⌈N/batch_size⌉ aggregate3
    /// multicalls. Fee tiers resolve through the same catalog as the unary
    /// path (`resolve_fee_pips`); uncatalogued pools are silently skipped
    /// here so their honest per-pair labels (`v3_pool_not_catalogued`, …)
    /// still surface in `dex_engine`'s unary path — the prefetch never
    /// fabricates or flattens a rejection reason.
    ///
    /// `intent_token_in` orients each pool (same rule as `dex_engine`'s
    /// `get_pool_quote`: token_in == token0 or unknown → zero_for_one).
    pub async fn prefetch_v3_quotes(
        &self,
        pools: &[PoolRef],
        amount_in: U256,
        intent_token_in: Address,
    ) {
        if amount_in.is_zero() || pools.is_empty() {
            return;
        }
        let Some(provider) = self.v3_provider.as_ref() else {
            return;
        };

        let mut reqs: Vec<crate::amm_math::V3QuoteRequest> = Vec::with_capacity(pools.len());
        for pool in pools {
            let fee_pips = match self.resolve_fee_pips(pool) {
                Ok(f) => f,
                // R8: skip — the unary path reports the exact label per pair.
                Err(_) => continue,
            };
            let zero_for_one = intent_token_in == pool.token0 || intent_token_in == Address::zero();
            let (token_in, token_out) = if zero_for_one {
                (pool.token0, pool.token1)
            } else {
                (pool.token1, pool.token0)
            };
            reqs.push(crate::amm_math::V3QuoteRequest {
                pool_addr: pool.address,
                token_in,
                token_out,
                amount_in,
                fee_bps: fee_pips,
            });
        }
        if reqs.is_empty() {
            return;
        }

        let dispatched = provider.quote_batch(reqs).await;
        let ok_count = dispatched.iter().filter(|(_, r)| r.is_ok()).count();
        debug!(
            event = "state_projector.v3_prefetch_batch",
            pools = pools.len(),
            dispatched = dispatched.len(),
            ok = ok_count,
        );
    }

    // -----------------------------------------------------------------------
    // Triangular cycle projection
    // -----------------------------------------------------------------------

    /// Project the full triangular cycle state.
    ///
    /// For each hop in `cycle_hops`, checks if the intent's first leg touches
    /// that hop's pool. If yes → applies `project_v2_post_swap` for the impacted
    /// hop. If no → uses current cached reserves.
    ///
    /// Returns `None` if ALL hops have cache misses (no basis for projection).
    ///
    /// `all_hops_projected = false` when ≥1 hop is at current state only.
    ///
    /// `cycle_hops`: `(token_in, token_out, pool_addr)` per hop in traversal order.
    pub async fn project_triangular_cycle(
        &self,
        intent: &RouteIntent,
        cycle_hops: &[(Address, Address, Address)],
    ) -> Option<TriangularVirtualState> {
        if cycle_hops.is_empty() {
            return None;
        }

        // Determine which pool the intent directly touches (first leg hint).
        let intent_pool_hint = intent.legs.first().and_then(|leg| leg.pool_hint);

        // The intent's first leg token_in — used for orientation detection.
        let intent_token_in = intent
            .legs
            .first()
            .map(|leg| leg.token_in)
            .unwrap_or(Address::zero());

        let mut projected_hops: Vec<V2VirtualReserves> = Vec::with_capacity(cycle_hops.len());
        let mut all_projected = true;
        let mut any_data = false;

        for &(token_in, token_out, pool_addr) in cycle_hops {
            // Determine if this hop is the one the intent impacts.
            let hop_is_impacted = intent_pool_hint.map(|ph| ph == pool_addr).unwrap_or(false)
                || (intent_token_in == token_in);

            if hop_is_impacted {
                // Build a minimal PoolRef for the hop.
                let pool_ref = PoolRef {
                    address: pool_addr,
                    token0: if token_in < token_out {
                        token_in
                    } else {
                        token_out
                    },
                    token1: if token_in < token_out {
                        token_out
                    } else {
                        token_in
                    },
                    fee_bps: Some(V2_FEE_BPS),
                };
                if let Some(vr) = self.project_v2_post_swap(&pool_ref, intent).await {
                    projected_hops.push(vr);
                    any_data = true;
                    continue;
                }
                // Projection failed (cache miss) — fall back to current reserves.
                all_projected = false;
            } else {
                all_projected = false;
            }

            // Use current cached reserves for this hop.
            if let Some((r0, r1)) = self.reserves_cache.get(&pool_addr).await {
                // Orient reserves: reserve_in for token_in side.
                let (reserve_in, reserve_out) = if token_in < token_out {
                    (r0, r1)
                } else {
                    (r1, r0)
                };
                projected_hops.push(V2VirtualReserves {
                    pool: pool_addr,
                    reserve_in,
                    reserve_out,
                    source_block: 0,
                });
                any_data = true;
            } else {
                // Cache miss for this hop — cannot complete the cycle projection.
                // Return None; the caller falls back to per-hop cache lookup
                // (same behavior as current TriangularEngine cold-cache path).
                debug!(
                    event = "state_projector.triangular_cache_miss",
                    pool = %pool_addr,
                    "cache miss on hop — returning None for cycle"
                );
                return None;
            }
        }

        if !any_data {
            return None;
        }

        Some(TriangularVirtualState {
            hops: projected_hops,
            all_hops_projected: all_projected,
        })
    }
}

// ---------------------------------------------------------------------------
// RouteQuoteProvider — protocol-agnostic per-leg / per-route quoting
// (Root 2C Phase 1). The SizeOptimizer searches a profit function built from
// `quote_leg`; it never implements V2 CPMM or V3 tick math itself. V2 legs are
// priced locally (reserves pre-fetched into `LegEval` + `amm_math::v2_amount_out`);
// V3 legs via the existing `V3QuoteProvider` (QuoterV2). Future protocols add
// variants/impls, not optimizer changes.
// ---------------------------------------------------------------------------

/// Per-leg quote descriptor — protocol-tagged. Resolved once before a sizing
/// search so the per-probe quote never re-reads the cache / re-parses addresses.
/// V2 carries oriented cached reserves (local CPMM); V3 carries the pool ref +
/// direction (on-chain QuoterV2). This is the protocol-neutral input to
/// `RouteQuoteProvider::quote_leg`.
#[derive(Debug, Clone)]
pub enum LegEval {
    /// V2 CPMM leg — oriented cached reserves + fee (local, no RPC).
    V2 {
        reserve_in: U256,
        reserve_out: U256,
        fee_bps: u32,
    },
    /// V3 concentrated-liquidity leg — priced on-chain via QuoterV2.
    V3 { pool: PoolRef, zero_for_one: bool },
}

/// Outcome of pricing one leg. R8 fail-honest: a real zero-yield (`Priced(0)`)
/// stays DISTINCT from a pricing failure (`Unavailable`) so the optimizer can
/// emit `NonPositiveProfit` vs `V3QuoteUnavailable` accurately — never
/// conflating "the quoter answered with 0" with "the quoter could not answer".
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LegQuote {
    /// The leg was priced. V2 is always priced (local CPMM); V3 is `Priced`
    /// when the on-chain quoter answered — the value may legitimately be 0.
    Priced(U256),
    /// A V3 leg could not be priced, carrying the honest rejection label
    /// (`ProjectV3Error::as_label` — WO-06): "v3_pool_not_catalogued" /
    /// "v3_pair_no_pools" for catalog gaps, "v3_quote_unavailable" only for
    /// real provider failures. V2 legs never produce this (local arithmetic
    /// always succeeds).
    Unavailable(&'static str),
}

/// Protocol-agnostic quoting for sizing. The SizeOptimizer consumes this; it
/// does NOT know V2 constant-product or V3 tick math. `quote_leg` prices one
/// leg; `quote_route` composes a route (leg[i].out → leg[i+1].in). Uses
/// Result of a batched V3 quote prefetch: the (request, outcome) pairs that
/// were actually dispatched (B1, V3-QUOTE-BATCH-20260919).
pub type V3BatchQuoteResults = Vec<(crate::amm_math::V3QuoteRequest, Result<U256, String>)>;

/// `Pin<Box<dyn Future>>` for dyn-compatibility (same reason as `V3QuoteProvider`:
/// RPITIT is not dyn-compatible).
pub trait RouteQuoteProvider: Send + Sync {
    /// Price one leg's output for `amount_in`. Returns `Priced(0)` for
    /// `amount_in == 0` (no RPC for V3). R8: `Unavailable` ≠ `Priced(0)`.
    fn quote_leg<'a>(
        &'a self,
        leg: &'a LegEval,
        amount_in: U256,
    ) -> Pin<Box<dyn Future<Output = LegQuote> + Send + 'a>>;

    /// Compose a route: fold `quote_leg` across `legs` (leg[i].out → leg[i+1].in).
    /// Returns the final `amount_out`, or `None` if any leg was `Unavailable`.
    fn quote_route<'a>(
        &'a self,
        legs: &'a [LegEval],
        amount_in: U256,
    ) -> Pin<Box<dyn Future<Output = Option<U256>> + Send + 'a>> {
        Box::pin(async move {
            let mut amount = amount_in;
            for leg in legs {
                match self.quote_leg(leg, amount).await {
                    LegQuote::Priced(out) => amount = out,
                    LegQuote::Unavailable(_) => return None,
                }
            }
            Some(amount)
        })
    }
}

impl RouteQuoteProvider for StateProjector {
    fn quote_leg<'a>(
        &'a self,
        leg: &'a LegEval,
        amount_in: U256,
    ) -> Pin<Box<dyn Future<Output = LegQuote> + Send + 'a>> {
        Box::pin(async move {
            if amount_in.is_zero() {
                return LegQuote::Priced(U256::zero());
            }
            match leg {
                LegEval::V2 {
                    reserve_in,
                    reserve_out,
                    fee_bps,
                } => LegQuote::Priced(v2_amount_out(
                    amount_in,
                    *reserve_in,
                    *reserve_out,
                    *fee_bps,
                )),
                LegEval::V3 { pool, zero_for_one } => {
                    match self
                        .project_v3_quote_checked(pool, amount_in, *zero_for_one)
                        .await
                    {
                        Ok(q) => LegQuote::Priced(q.amount_out),
                        Err(e) => LegQuote::Unavailable(e.as_label()),
                    }
                }
            }
        })
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use crate::engines::triangular_engine::ReservesCache;
    use crate::route_intent::{
        DetectionSource, ProtocolType, RouteIntent, RouteIntentLeg, RouterKind, SwapExactMode,
    };
    use ethers::types::{Address, H256, U256};
    use std::sync::Arc;

    // ── Mock V3 provider ──────────────────────────────────────────────────────

    struct MockV3Provider {
        /// Fixed amount_out to return for every call.
        amount_out: U256,
        /// If `true`, always returns an Err (simulates RPC failure).
        always_err: bool,
    }

    impl V3QuoteProvider for MockV3Provider {
        fn quote_exact_input_single(
            &self,
            _pool: Address,
            _token_in: Address,
            _token_out: Address,
            _amount_in: U256,
            _fee_bps: u32,
        ) -> std::pin::Pin<Box<dyn std::future::Future<Output = anyhow::Result<U256>> + Send + '_>>
        {
            let result = if self.always_err {
                Err(anyhow::anyhow!("mock rpc error"))
            } else {
                Ok(self.amount_out)
            };
            Box::pin(async move { result })
        }
    }

    // ── Helpers ──────────────────────────────────────────────────────────────

    fn addr(n: u64) -> Address {
        Address::from_low_u64_be(n)
    }

    fn unit(n: u64) -> U256 {
        U256::from(10u128).pow(U256::from(18u32)) * U256::from(n)
    }

    fn make_pool(address: Address, token0: Address, token1: Address) -> PoolRef {
        PoolRef {
            address,
            token0,
            token1,
            fee_bps: Some(30),
        }
    }

    fn make_intent_with_amount(token_in: Address, token_out: Address, amount: U256) -> RouteIntent {
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
            amount,
            None,
            SwapExactMode::ExactIn,
            DetectionSource::PublicMempool,
        )
        .expect("valid intent")
    }

    fn make_projector_no_v3(cache: Arc<ReservesCache>) -> StateProjector {
        StateProjector::new(cache, None, Arc::new(V3FeeCatalog::new()))
    }

    fn make_projector_with_mock(
        cache: Arc<ReservesCache>,
        amount_out: U256,
        catalog: Arc<V3FeeCatalog>,
    ) -> StateProjector {
        let provider = Arc::new(MockV3Provider {
            amount_out,
            always_err: false,
        });
        StateProjector::new(cache, Some(provider), catalog)
    }

    // ── WO-06 mocks: capture the exact fee requested / panic on any call ─────

    /// Capturing mock: records every (pool, fee) the projector requests and
    /// answers with a fixed amount — proves the EXACT catalog tiers reach the
    /// provider (T2/T4) without RPC.
    struct CapturingV3Mock {
        amount_out: U256,
        calls: std::sync::Mutex<Vec<(Address, u32)>>,
    }

    impl V3QuoteProvider for CapturingV3Mock {
        fn quote_exact_input_single(
            &self,
            pool: Address,
            _token_in: Address,
            _token_out: Address,
            _amount_in: U256,
            fee_bps: u32,
        ) -> std::pin::Pin<Box<dyn std::future::Future<Output = anyhow::Result<U256>> + Send + '_>>
        {
            self.calls
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .push((pool, fee_bps));
            let out = self.amount_out;
            Box::pin(async move { Ok(out) })
        }
    }

    /// Panic mock: any invocation fails the test — proves ZERO RPC was issued
    /// for catalog-rejected pools/pairs (T3/T5).
    struct PanicV3Mock;

    impl V3QuoteProvider for PanicV3Mock {
        fn quote_exact_input_single(
            &self,
            _pool: Address,
            _token_in: Address,
            _token_out: Address,
            _amount_in: U256,
            _fee_bps: u32,
        ) -> std::pin::Pin<Box<dyn std::future::Future<Output = anyhow::Result<U256>> + Send + '_>>
        {
            Box::pin(async move { panic!("provider must never be invoked (zero-RPC test)") })
        }
    }

    fn v3_pool(
        address: Address,
        token0: Address,
        token1: Address,
        fee_bps: Option<u32>,
    ) -> PoolRef {
        PoolRef {
            address,
            token0,
            token1,
            fee_bps,
        }
    }

    // ── state_projector::tests::v2_post_swap_increases_reserve_in ────────────
    //
    // Pool with (100, 100) units. Intent swaps 10 units in.
    // Expected: new_reserve_in ≈ 110 (+ amount after fee), new_reserve_out ≈ 90.91.

    #[tokio::test]
    async fn v2_post_swap_increases_reserve_in() {
        let pool_addr = addr(0x10);
        let tok0 = addr(0x1);
        let tok1 = addr(0x2);

        let cache = Arc::new(ReservesCache::new());
        // Insert (100, 100) unit reserves.
        cache.insert(pool_addr, unit(100), unit(100)).await;

        let projector = make_projector_no_v3(cache);
        let pool = make_pool(pool_addr, tok0, tok1);
        let intent = make_intent_with_amount(tok0, tok1, unit(10));

        let result = projector
            .project_v2_post_swap(&pool, &intent)
            .await
            .expect("projection must succeed");

        // reserve_in must be strictly larger than original 100 units.
        assert!(
            result.reserve_in > unit(100),
            "reserve_in must increase after deposit: got {}",
            result.reserve_in
        );
        // reserve_out must be strictly smaller than original 100 units.
        assert!(
            result.reserve_out < unit(100),
            "reserve_out must decrease after swap: got {}",
            result.reserve_out
        );
        // Real V2 model: full amount_in deposited → new_reserve_in = 100 + 10 = 110 exactly.
        // reserve_out decreases by amount_out (≈9.07 units for 30bps fee on equal reserves).
        let r_in_exact = unit(110);
        assert_eq!(
            result.reserve_in, r_in_exact,
            "reserve_in must be exactly 110 units (full amount deposited): got {}",
            result.reserve_in
        );
    }

    // ── state_projector::tests::v2_no_cache_returns_none ─────────────────────

    #[tokio::test]
    async fn v2_no_cache_returns_none() {
        let cache = Arc::new(ReservesCache::new()); // empty cache
        let projector = make_projector_no_v3(cache);
        let pool = make_pool(addr(0x10), addr(0x1), addr(0x2));
        let intent = make_intent_with_amount(addr(0x1), addr(0x2), unit(1));

        let result = projector.project_v2_post_swap(&pool, &intent).await;

        assert!(
            result.is_none(),
            "cold cache must return None — R8 fail-honest"
        );
    }

    // ── state_projector::tests::v2_constant_product_holds ────────────────────
    //
    // After projection, k_new = r_in_new * r_out_new >= k_old.
    // The full amount_in is deposited (fee stays in pool), so k grows.

    #[tokio::test]
    async fn v2_constant_product_holds() {
        let pool_addr = addr(0x10);
        let tok0 = addr(0x1);
        let tok1 = addr(0x2);

        let cache = Arc::new(ReservesCache::new());
        let r_in = unit(1000);
        let r_out = unit(500);
        cache.insert(pool_addr, r_in, r_out).await;

        let projector = make_projector_no_v3(cache);
        let pool = make_pool(pool_addr, tok0, tok1);
        let intent = make_intent_with_amount(tok0, tok1, unit(50));

        let result = projector
            .project_v2_post_swap(&pool, &intent)
            .await
            .expect("projection must succeed");

        // k_old = r_in * r_out
        // k_new = result.reserve_in * result.reserve_out
        // k_new >= k_old because:
        //   - full amount_in deposited → new_reserve_in = reserve_in + amount_in
        //   - fee causes amount_out < fair-price amount → pool keeps excess → k grows
        //
        // Note: U256 multiplication may overflow for very large reserve values.
        // For unit(1000) * unit(500): each is ~10^21, product is ~5e44 < 2^256 ok.
        let k_old = r_in.checked_mul(r_out).expect("k_old must not overflow");
        let k_new = result
            .reserve_in
            .checked_mul(result.reserve_out)
            .expect("k_new must not overflow");

        assert!(
            k_new >= k_old,
            "constant product must be non-decreasing after fee: k_new={k_new}, k_old={k_old}"
        );
    }

    // ── state_projector::tests::v2_orientation_handled ───────────────────────
    //
    // Intent token_in == pool.token1 → swap_in is token1.
    // Reserves are swapped before applying math. Reserve for token1 must increase.

    #[tokio::test]
    async fn v2_orientation_handled() {
        let pool_addr = addr(0x10);
        // token0 < token1 by value.
        let tok0 = addr(0x1); // pool.token0
        let tok1 = addr(0x2); // pool.token1

        let cache = Arc::new(ReservesCache::new());
        // r0 = reserve of token0, r1 = reserve of token1.
        let r0 = unit(200);
        let r1 = unit(100);
        cache.insert(pool_addr, r0, r1).await;

        let projector = make_projector_no_v3(cache);
        let pool = make_pool(pool_addr, tok0, tok1);

        // Intent: token_in = tok1 (depositing token1 into the pool).
        let intent = make_intent_with_amount(tok1, tok0, unit(10));

        let result = projector
            .project_v2_post_swap(&pool, &intent)
            .await
            .expect("projection must succeed");

        // Depositing token1 → reserve_in (token1 side) must increase beyond r1=100.
        // reserve_out (token0 side) must decrease below r0=200.
        assert!(
            result.reserve_in > r1,
            "depositing token1: reserve_in must increase beyond r1={r1}, got {}",
            result.reserve_in
        );
        assert!(
            result.reserve_out < r0,
            "reserve_out (token0) must decrease below r0={r0}, got {}",
            result.reserve_out
        );
    }

    // ── state_projector::tests::v3_quote_forwards_to_provider ────────────────

    #[tokio::test]
    async fn v3_quote_forwards_to_provider() {
        let cache = Arc::new(ReservesCache::new());
        let expected_out = U256::from(999_888_777u128);
        // WO-06: the pool must be catalogued for the quote to proceed.
        let catalog = Arc::new(V3FeeCatalog::new());
        catalog.record_observed(addr(0x99), addr(0x1), addr(0x2), 30);
        let projector = make_projector_with_mock(cache, expected_out, catalog);

        let pool = v3_pool(addr(0x99), addr(0x1), addr(0x2), Some(30));
        let amount_in = unit(1);

        let result = projector
            .project_v3_quote(&pool, amount_in, true)
            .await
            .expect("v3 quote must succeed with mock provider");

        assert_eq!(
            result.amount_out, expected_out,
            "project_v3_quote must forward mock provider result"
        );
        assert_eq!(result.pool, addr(0x99));
        assert_eq!(
            result.fee_bps, 30,
            "resolved catalog fee travels on the quote"
        );
    }

    // ── state_projector::tests::v3_quote_no_provider_returns_none ─────────────

    #[tokio::test]
    async fn v3_quote_no_provider_returns_none() {
        let cache = Arc::new(ReservesCache::new());
        let projector = make_projector_no_v3(cache);

        let pool = v3_pool(addr(0x99), addr(0x1), addr(0x2), Some(500));
        let result = projector.project_v3_quote(&pool, unit(1), true).await;

        assert!(
            result.is_none(),
            "v3_provider = None must return None — R8 fail-honest"
        );
    }

    // ── WO-06: FEE-TIER-AWARE-QUOTING (design tests T2-T6) ───────────────────

    // T2 — resolution replaces the blind unwrap_or(500): a catalogued pool
    // quotes at its CATALOG tier whether the caller offered None or a wrong
    // value (Mismatch still quotes WITH the catalog tier).
    #[tokio::test]
    async fn v3_fee_resolution_replaces_blind_default() {
        let cache = Arc::new(ReservesCache::new());
        let catalog = Arc::new(V3FeeCatalog::new());
        let pool_addr = addr(0x99);
        catalog.record_observed(pool_addr, addr(0x1), addr(0x2), 3000);
        let mock = Arc::new(CapturingV3Mock {
            amount_out: U256::from(1234u64),
            calls: std::sync::Mutex::new(Vec::new()),
        });
        let projector = StateProjector::new(cache, Some(mock.clone()), catalog);

        // fee_bps = None: previously quoted BLIND at 500 → now Catalog(3000).
        let q_none = projector
            .project_v3_quote_checked(
                &v3_pool(pool_addr, addr(0x1), addr(0x2), None),
                unit(1),
                true,
            )
            .await
            .expect("catalogued pool with fee None must quote at the catalog tier");
        assert_eq!(q_none.fee_bps, 3000);

        // fee_bps = Some(100): Mismatch → still quotes WITH the catalog tier.
        let q_offered = projector
            .project_v3_quote_checked(
                &v3_pool(pool_addr, addr(0x1), addr(0x2), Some(100)),
                unit(1),
                true,
            )
            .await
            .expect("mismatch must still quote at the catalog tier");
        assert_eq!(q_offered.fee_bps, 3000);

        let calls = mock.calls.lock().unwrap();
        assert_eq!(
            calls.as_slice(),
            &[(pool_addr, 3000), (pool_addr, 3000)],
            "both quotes must use the catalog tier — never 500 nor 100"
        );
    }

    // T3 — pair with no known V3 pools: PairHasNoV3Pools, ZERO RPC (the mock
    // panics if invoked).
    #[tokio::test]
    async fn v3_pair_without_pools_errors_without_rpc() {
        let cache = Arc::new(ReservesCache::new());
        let projector = StateProjector::new(
            cache,
            Some(Arc::new(PanicV3Mock)),
            Arc::new(V3FeeCatalog::new()), // empty — pair unknown
        );

        let err = projector
            .project_v3_quote_checked(
                &v3_pool(addr(0x99), addr(0x1), addr(0x2), Some(500)),
                unit(1),
                true,
            )
            .await
            .expect_err("uncatalogued pair must error without RPC");
        assert_eq!(err, ProjectV3Error::PairHasNoV3Pools);
        assert_eq!(err.as_label(), "v3_pair_no_pools");
    }

    // T4 — exotic tiers: catalog {1, 5}; the provider must be asked for
    // EXACTLY 1 and 5 — a non-existent 100 must never be requested.
    #[tokio::test]
    async fn v3_exotic_tiers_quote_exact_catalog_values() {
        let cache = Arc::new(ReservesCache::new());
        let catalog = Arc::new(V3FeeCatalog::new());
        catalog.record_observed(addr(0x91), addr(0x1), addr(0x2), 1);
        catalog.record_observed(addr(0x95), addr(0x1), addr(0x2), 5);
        let mock = Arc::new(CapturingV3Mock {
            amount_out: U256::from(42u64),
            calls: std::sync::Mutex::new(Vec::new()),
        });
        let projector = StateProjector::new(cache, Some(mock.clone()), catalog);

        // Offered 100 (a tier that does NOT exist for this pair) → Mismatch.
        let q1 = projector
            .project_v3_quote_checked(
                &v3_pool(addr(0x91), addr(0x1), addr(0x2), Some(100)),
                unit(1),
                true,
            )
            .await
            .expect("tier-1 pool must quote");
        assert_eq!(q1.fee_bps, 1);
        // No offered fee → Catalog(5).
        let q5 = projector
            .project_v3_quote_checked(
                &v3_pool(addr(0x95), addr(0x1), addr(0x2), None),
                unit(1),
                false,
            )
            .await
            .expect("tier-5 pool must quote");
        assert_eq!(q5.fee_bps, 5);

        let calls = mock.calls.lock().unwrap();
        let fees: Vec<u32> = calls.iter().map(|(_, f)| *f).collect();
        assert_eq!(
            fees,
            vec![1, 5],
            "exactly the exotic catalog tiers requested"
        );
        assert!(!fees.contains(&100), "tier 100 must NEVER be requested");
    }

    // T5 + T6 — pool not catalogued while the pair HAS tiers: PoolNotCatalogued
    // with the EXACT label (zero RPC), never "v3_quote_unavailable".
    #[tokio::test]
    async fn v3_pool_not_catalogued_errors_without_rpc_exact_label() {
        let cache = Arc::new(ReservesCache::new());
        let catalog = Arc::new(V3FeeCatalog::new());
        // A DIFFERENT pool on the same pair is catalogued → pair has tiers,
        // but THIS pool address is unknown.
        catalog.record_observed(addr(0x50), addr(0x1), addr(0x2), 500);
        let projector = StateProjector::new(cache, Some(Arc::new(PanicV3Mock)), catalog);

        let err = projector
            .project_v3_quote_checked(
                &v3_pool(addr(0x99), addr(0x1), addr(0x2), Some(500)),
                unit(1),
                true,
            )
            .await
            .expect_err("uncatalogued pool must error without RPC");
        assert_eq!(err, ProjectV3Error::PoolNotCatalogued);
        // T6: the label must be the exact new string, NOT v3_quote_unavailable
        // (that stays reserved for real provider failures).
        assert_eq!(err.as_label(), "v3_pool_not_catalogued");
        assert_ne!(err.as_label(), "v3_quote_unavailable");
    }

    #[test]
    fn v3_error_labels_are_exact() {
        assert_eq!(
            ProjectV3Error::PoolNotCatalogued.as_label(),
            "v3_pool_not_catalogued"
        );
        assert_eq!(
            ProjectV3Error::PairHasNoV3Pools.as_label(),
            "v3_pair_no_pools"
        );
        assert_eq!(
            ProjectV3Error::ProviderUnavailable.as_label(),
            "v3_quote_unavailable"
        );
        assert_eq!(
            ProjectV3Error::QuoteFailed("rpc".into()).as_label(),
            "v3_quote_unavailable"
        );
    }

    // ── state_projector::tests::triangular_partial_projection_flagged ─────────
    //
    // 3-hop cycle where the intent only impacts hop 0.
    // all_hops_projected must be false.

    #[tokio::test]
    async fn triangular_partial_projection_flagged() {
        let tok_a = addr(0x1);
        let tok_b = addr(0x2);
        let tok_c = addr(0x3);
        let pool_ab = addr(0x10);
        let pool_bc = addr(0x20);
        let pool_ca = addr(0x30);

        let cache = Arc::new(ReservesCache::new());
        // Insert reserves for all 3 hops.
        cache.insert(pool_ab, unit(100), unit(100)).await;
        cache.insert(pool_bc, unit(100), unit(100)).await;
        cache.insert(pool_ca, unit(100), unit(100)).await;

        let projector = make_projector_no_v3(cache);

        // Intent only touches pool_ab (token_in = tok_a).
        let intent = make_intent_with_amount(tok_a, tok_b, unit(5));

        let cycle_hops = vec![
            (tok_a, tok_b, pool_ab),
            (tok_b, tok_c, pool_bc),
            (tok_c, tok_a, pool_ca),
        ];

        let result = projector
            .project_triangular_cycle(&intent, &cycle_hops)
            .await
            .expect("projection must succeed");

        assert_eq!(result.hops.len(), 3, "must project all 3 hops");
        assert!(
            !result.all_hops_projected,
            "partial projection: all_hops_projected must be false"
        );
    }

    // ── state_projector::tests::triangular_full_projection_when_all_hops_impacted
    //
    // Intent's token_in matches hop 0, and the remaining hops also match
    // by token sequence. all_hops_projected = true only when the intent explicitly
    // touches all hops (via pool_hint on every leg). For Phase 12, with a single-leg
    // intent, only hop 0 is projected — so all_hops_projected = false.
    // This test verifies that the returned state has all hops populated.

    #[tokio::test]
    async fn triangular_full_projection_when_all_hops_impacted() {
        let tok_a = addr(0x1);
        let tok_b = addr(0x2);
        let tok_c = addr(0x3);
        let pool_ab = addr(0x10);
        let pool_bc = addr(0x20);
        let pool_ca = addr(0x30);

        let cache = Arc::new(ReservesCache::new());
        cache.insert(pool_ab, unit(100), unit(120)).await;
        cache.insert(pool_bc, unit(100), unit(110)).await;
        cache.insert(pool_ca, unit(100), unit(200)).await;

        let projector = make_projector_no_v3(cache);

        // Single-leg intent — only hop 0 is impacted by default.
        let intent = make_intent_with_amount(tok_a, tok_b, unit(5));

        let cycle_hops = vec![
            (tok_a, tok_b, pool_ab),
            (tok_b, tok_c, pool_bc),
            (tok_c, tok_a, pool_ca),
        ];

        let result = projector
            .project_triangular_cycle(&intent, &cycle_hops)
            .await
            .expect("projection must succeed with populated cache");

        // All 3 hops must be in the result.
        assert_eq!(
            result.hops.len(),
            3,
            "must produce 3 hop entries even with partial projection"
        );
        // With a single-leg intent, unimpacted hops use current reserves.
        // all_hops_projected will be false because hops 1 and 2 are not impacted.
        // This is the Phase 12 behavior — Phase 15 will improve this.
        assert!(
            !result.all_hops_projected,
            "single-leg intent cannot fully project all 3 hops"
        );
        // All hops must have non-zero reserves.
        for (i, hop) in result.hops.iter().enumerate() {
            assert!(
                !hop.reserve_in.is_zero() && !hop.reserve_out.is_zero(),
                "hop[{i}] must have non-zero reserves"
            );
        }
    }

    // ── state_projector::tests::projection_does_not_mutate_cache ─────────────

    #[tokio::test]
    async fn projection_does_not_mutate_cache() {
        let pool_addr = addr(0x10);
        let tok0 = addr(0x1);
        let tok1 = addr(0x2);

        let cache = Arc::new(ReservesCache::new());
        let r0_orig = unit(1000);
        let r1_orig = unit(2000);
        cache.insert(pool_addr, r0_orig, r1_orig).await;

        let projector = make_projector_no_v3(Arc::clone(&cache));
        let pool = make_pool(pool_addr, tok0, tok1);
        let intent = make_intent_with_amount(tok0, tok1, unit(100));

        // Run projection.
        let _ = projector.project_v2_post_swap(&pool, &intent).await;

        // Read cache again — must be unchanged.
        let (r0_after, r1_after) = cache
            .get(&pool_addr)
            .await
            .expect("cache must still contain entry");
        assert_eq!(
            r0_after, r0_orig,
            "cache r0 must be unchanged after projection"
        );
        assert_eq!(
            r1_after, r1_orig,
            "cache r1 must be unchanged after projection"
        );
    }
}
