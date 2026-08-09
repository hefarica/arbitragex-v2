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
use ethers::types::{Address, U256};
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use tracing::debug;

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
    pub fee_bps: u32,
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
}

impl StateProjector {
    /// Constructs a `StateProjector`.
    ///
    /// - `reserves_cache`: shared pool reserves (populated by PoolSyncWorker).
    /// - `v3_provider`: optional V3 quoter impl. `None` → V3 projection returns `None`.
    pub fn new(
        reserves_cache: Arc<ReservesCache>,
        v3_provider: Option<Arc<dyn V3QuoteProvider>>,
    ) -> Self {
        Self {
            reserves_cache,
            v3_provider,
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

    /// Get a V3 quote at `amount_in` for the given pool.
    ///
    /// For Phase 12 this is a direct forward to the V3QuoteProvider. The
    /// quoter reads CURRENT on-chain state, which IS the post-tx state for
    /// mempool transactions (they haven't been included yet). Phase 15+ can
    /// add tick-level projection here.
    ///
    /// Returns `None` when:
    ///   - `v3_provider` is `None` (R8 honest: no fabrication without RPC).
    ///   - The provider returns an error (RPC failure or pool reverts).
    ///   - `amount_in.is_zero()`.
    pub async fn project_v3_quote(
        &self,
        pool: &PoolRef,
        amount_in: U256,
        zero_for_one: bool,
    ) -> Option<V3VirtualQuote> {
        let provider = self.v3_provider.as_ref()?;

        if amount_in.is_zero() {
            return None;
        }

        let fee_bps = pool.fee_bps.unwrap_or(500); // V3 default 0.05%

        // Orient token_in / token_out from zero_for_one flag.
        let (token_in, token_out) = if zero_for_one {
            (pool.token0, pool.token1)
        } else {
            (pool.token1, pool.token0)
        };

        match provider
            .quote_exact_input_single(pool.address, token_in, token_out, amount_in, fee_bps)
            .await
        {
            Ok(amount_out) => {
                debug!(
                    event = "state_projector.v3_quote",
                    pool = %pool.address,
                    amount_in = %amount_in,
                    amount_out = %amount_out,
                );
                Some(V3VirtualQuote {
                    pool: pool.address,
                    amount_out,
                    fee_bps,
                })
            }
            Err(e) => {
                debug!(
                    event = "state_projector.v3_quote_failed",
                    pool = %pool.address,
                    error = %e,
                );
                None
            }
        }
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
    /// A V3 leg could not be priced: provider absent, RPC failure, or pool
    /// revert. V2 legs never produce this (local arithmetic always succeeds).
    Unavailable,
}

/// Protocol-agnostic quoting for sizing. The SizeOptimizer consumes this; it
/// does NOT know V2 constant-product or V3 tick math. `quote_leg` prices one
/// leg; `quote_route` composes a route (leg[i].out → leg[i+1].in). Uses
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
                    LegQuote::Unavailable => return None,
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
                    match self.project_v3_quote(pool, amount_in, *zero_for_one).await {
                        Some(q) => LegQuote::Priced(q.amount_out),
                        None => LegQuote::Unavailable,
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
        StateProjector::new(cache, None)
    }

    fn make_projector_with_mock(cache: Arc<ReservesCache>, amount_out: U256) -> StateProjector {
        let provider = Arc::new(MockV3Provider {
            amount_out,
            always_err: false,
        });
        StateProjector::new(cache, Some(provider))
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
        let projector = make_projector_with_mock(cache, expected_out);

        let pool = make_pool(addr(0x99), addr(0x1), addr(0x2));
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
    }

    // ── state_projector::tests::v3_quote_no_provider_returns_none ─────────────

    #[tokio::test]
    async fn v3_quote_no_provider_returns_none() {
        let cache = Arc::new(ReservesCache::new());
        let projector = make_projector_no_v3(cache);

        let pool = make_pool(addr(0x99), addr(0x1), addr(0x2));
        let result = projector.project_v3_quote(&pool, unit(1), true).await;

        assert!(
            result.is_none(),
            "v3_provider = None must return None — R8 fail-honest"
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
