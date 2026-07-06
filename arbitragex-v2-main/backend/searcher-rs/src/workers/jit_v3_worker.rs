//! JitV3Worker — scaffold for Just-In-Time (JIT) liquidity MEV on Uniswap V3.
//!
//! ## Strategy shape (BE-3.3)
//!
//! JIT liquidity is a mempool-aware MEV strategy:
//!
//!   1. **Detect**: Watch the pending tx pool for large swaps targeting a V3 pool.
//!      "Large" is configurable via `JitConfig::min_swap_size_usd`.
//!   2. **Mint**: Immediately before the victim swap lands, mint a very tight LP
//!      position around the pool's current tick (range ±`tick_range_bps`).
//!      The narrow range means the JIT liquidity owns a disproportionate share
//!      of fees earned from the swap.
//!   3. **Capture**: The victim swap executes, trading through the JIT position
//!      and paying LP fees proportional to the JIT share of in-range liquidity.
//!   4. **Burn**: In the same bundle (same block, after the swap), burn the
//!      position and collect accumulated fees. Net profit = fees captured −
//!      gas(mint + burn) − impermanent loss (negligible for 1-block exposure).
//!
//! ## Adversarial landscape
//!
//! Multiple JIT bots compete for the same pending swap. The winner is whichever
//! bot mints with the highest in-range liquidity (largest capital deployed) AND
//! gets their mint tx ordered first (block builder cooperation / private relay).
//! Real JIT requires:
//!   - Sub-millisecond mempool observation (Bloxroute BDN or equivalent).
//!   - Private bundle submission (Flashbots / Titan) to prevent front-running.
//!   - Capital sized to the target pool depth (potentially millions of dollars
//!     of concentrated liquidity to dominate the fee capture).
//!
//! ## Scaffold state (BE-3.3 MVP)
//!
//! This worker establishes the structure and the core P&L mathematics.
//! Full implementation deferred to operator follow-up:
//!   - `hft_mempool_listener` integration for pending swap detection.
//!   - On-chain V3 slot0 / currentTick fetch via multicall.
//!   - Signed mint/burn tx construction and Flashbots bundle submission.
//!   - `Opportunity { strategy_kind: JitV3, ... }` emit to Redis stream + PG.
//!
//! ## Doctrine compliance
//!
//! - **RULE 00 zero mocks**: `run()` body is a no-op shell; no synthetic opps.
//! - **R8 fail-honest**: `compute_jit_pnl` returns a struct with `viable: false`
//!   when gas exceeds captured fees — never coerces to a positive number.
//! - **No `unwrap()` outside tests**, no `unimplemented!()`, no `todo!()`.

use anyhow::Result;
use redis::aio::ConnectionManager;
use shared_rs::rpc_failover::HttpRpcPool;
use sqlx::PgPool;
use std::sync::Arc;

/// Configuration knobs for the JIT V3 strategy.
///
/// All fields are runtime-configurable. Defaults ship with conservative values
/// that require a $100k+ swap to trigger consideration (mainnet gas makes
/// smaller swaps unprofitable for JIT).
#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct JitConfig {
    /// Minimum pending swap size in USD to trigger JIT consideration.
    /// Below this threshold the expected fee capture is too small to cover
    /// the gas cost of the mint + burn tx pair (~$50 on mainnet).
    pub min_swap_size_usd: f64,
    /// Uniswap V3 pool addresses to monitor for JIT opportunities.
    /// Operator-managed list; empty vec = worker is effectively idle.
    pub target_pools: Vec<String>,
    /// Tick range tightness expressed in basis points. A value of 10 means
    /// the JIT LP position spans ±0.1% around the pool's current tick.
    /// Tighter range → higher fee share per unit of capital deployed, but
    /// higher impermanent loss risk if the swap moves price outside the range.
    pub tick_range_bps: u32,
}

impl Default for JitConfig {
    fn default() -> Self {
        Self {
            min_swap_size_usd: 100_000.0,
            target_pools: vec![
                // USDC/WETH 0.05% — highest-volume V3 pool on Ethereum mainnet.
                // Operator should extend this list with pools matching their
                // capital profile and target chains.
                "0x88e6A0c2dDD26FEEb64F039a2c41296FcB3f5640".to_string(),
            ],
            tick_range_bps: 10,
        }
    }
}

/// Output of the JIT P&L computation.
#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct JitPnl {
    /// Gross profit in token-in units (= fees captured before gas).
    pub captured_fees_token_in: f64,
    /// Gross profit in USD (assumes token_in price = $1 for scaffold;
    /// real implementation must price via oracle — R8 fail-honest).
    pub gross_profit_usd: f64,
    /// Net profit after subtracting estimated gas cost.
    pub net_profit_usd: f64,
    /// True when `net_profit_usd > 0`.
    pub viable: bool,
}

/// JIT V3 liquidity worker (scaffold — mempool subscription deferred).
#[allow(dead_code)]
pub struct JitV3Worker {
    chain_id: u64,
    rpc_pool: Option<Arc<HttpRpcPool>>,
    /// Held for future use when the worker emits `Opportunity` rows to PG.
    _db: PgPool,
    /// Held for future use when the worker publishes to the Redis stream.
    _redis: ConnectionManager,
    config: JitConfig,
}

#[allow(dead_code)]
impl JitV3Worker {
    /// Construct the worker with production dependencies.
    ///
    /// `rpc_pool = None` is valid for offline/test environments; `run()` will
    /// log a warning and return immediately, matching the pattern of other
    /// scaffold workers (see `hft_mempool_listener` early-exit).
    pub fn new(
        chain_id: u64,
        rpc_pool: Option<Arc<HttpRpcPool>>,
        db: PgPool,
        redis: ConnectionManager,
    ) -> Self {
        Self {
            chain_id,
            rpc_pool,
            _db: db,
            _redis: redis,
            config: JitConfig::default(),
        }
    }

    /// Main worker loop.
    ///
    /// Currently a no-op shell. Full implementation (mempool subscription,
    /// slot0 fetch, bundle construction) is the BE-3.3 follow-up task.
    pub async fn run(self) -> Result<()> {
        tracing::info!(
            event = "jit_v3_worker.boot",
            chain_id = self.chain_id,
            min_swap_size_usd = self.config.min_swap_size_usd,
            target_pool_count = self.config.target_pools.len(),
            tick_range_bps = self.config.tick_range_bps,
        );

        if self.rpc_pool.is_none() {
            tracing::warn!(
                event = "jit_v3_worker.no_rpc",
                chain_id = self.chain_id,
                "JIT V3 worker idle — no RPC pool configured; \
                 mempool subscription requires live RPC (BE-3.3 follow-up)"
            );
            return Ok(());
        }

        // TODO(BE-3.3 follow-up): subscribe to mempool stream via hft_mempool_listener.
        // TODO(BE-3.3 follow-up): for each pending swap targeting a target_pool,
        //   call compute_jit_pnl() and filter on viable + min_swap_size_usd.
        // TODO(BE-3.3 follow-up): fetch V3 slot0 (currentTick, sqrtPriceX96) via multicall.
        // TODO(BE-3.3 follow-up): construct mint/burn tx pair and submit as Flashbots bundle.
        // TODO(BE-3.3 follow-up): emit Opportunity { strategy_kind: JitV3, ... } to Redis + PG.

        tracing::info!(
            event = "jit_v3_worker.scaffold_idle",
            chain_id = self.chain_id,
            "BE-3.3 scaffold: mempool subscription not yet implemented"
        );
        Ok(())
    }

    /// Compute expected JIT P&L for a single pending swap.
    ///
    /// This is a **pure function** — no I/O, no side effects, fully testable.
    ///
    /// # Arguments
    ///
    /// - `swap_size_token_in`: Volume of the pending swap in token-in units.
    ///   For scaffold purposes this is also treated as the USD value (price = $1).
    ///   Real implementation must apply an oracle price before comparing against
    ///   `JitConfig::min_swap_size_usd`.
    /// - `pool_fee_bps`: V3 pool fee tier in basis points (e.g. 5 = 0.05% tier,
    ///   30 = 0.3% tier, 100 = 1% tier).
    /// - `jit_share_pct`: Fraction of the pool's in-range liquidity the JIT
    ///   position represents after minting (0.0 to 1.0). A newly minted JIT
    ///   position that equals existing in-range liquidity has `jit_share_pct = 0.5`.
    ///   For P&L analysis 0.9 is the theoretical ceiling assuming the JIT bot
    ///   deploys capital equal to 9x existing in-range liquidity.
    /// - `gas_cost_usd`: Estimated USD cost of the mint + burn transaction pair.
    ///   On Ethereum mainnet ~$30-$100 depending on gas price and pool complexity.
    ///
    /// # Formula
    ///
    /// ```text
    /// swap_fees     = swap_size_token_in x (pool_fee_bps / 10_000)
    /// captured_fees = swap_fees x jit_share_pct
    /// gross_usd     = captured_fees          (scaffold: token_in price = $1)
    /// net_usd       = gross_usd - gas_cost_usd
    /// viable        = net_usd > 0
    /// ```
    pub fn compute_jit_pnl(
        swap_size_token_in: f64,
        pool_fee_bps: u32,
        jit_share_pct: f64,
        gas_cost_usd: f64,
    ) -> JitPnl {
        let swap_fees_token_in = swap_size_token_in * (pool_fee_bps as f64 / 10_000.0);
        let captured_fees_token_in = swap_fees_token_in * jit_share_pct;
        // Scaffold assumption: token_in price = $1 USD.
        // Real implementation: multiply by oracle price before comparing USD gates.
        let gross_profit_usd = captured_fees_token_in;
        let net_profit_usd = gross_profit_usd - gas_cost_usd;
        JitPnl {
            captured_fees_token_in,
            gross_profit_usd,
            net_profit_usd,
            viable: net_profit_usd > 0.0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A $1M swap in the 0.05% tier with 90% JIT share and $50 gas should
    /// yield $450 gross - $50 gas = $400 net (viable).
    ///
    /// Derivation:
    ///   swap_fees = 1_000_000 x (5 / 10_000) = 50   <- WRONG unit check below
    ///
    /// Re-check: pool_fee_bps = 5 means 0.05%.
    ///   swap_fees = 1_000_000 x 0.0005 = 500
    ///   captured  = 500 x 0.9          = 450
    ///   net       = 450 - 50           = 400  (verified)
    #[test]
    fn jit_pnl_positive_for_large_swap() {
        let pnl = JitV3Worker::compute_jit_pnl(
            1_000_000.0, // $1M swap
            5,           // 0.05% fee tier (pool_fee_bps = 5)
            0.9,         // 90% JIT share of in-range liquidity
            50.0,        // $50 gas cost for mint + burn pair
        );
        assert!(pnl.viable, "expected viable for $1M swap");
        assert!(
            pnl.net_profit_usd >= 400.0,
            "expected net_profit_usd >= 400, got {}",
            pnl.net_profit_usd
        );
        // Gross: 1_000_000 x 0.0005 x 0.9 = 450.0
        assert!(
            (pnl.gross_profit_usd - 450.0_f64).abs() < 1e-6,
            "gross_profit_usd mismatch: expected 450.0, got {}",
            pnl.gross_profit_usd
        );
    }

    /// A $10k swap with the same parameters produces $4.50 gross.
    /// After $50 gas the net is -$45.50 (not viable).
    ///
    /// Derivation:
    ///   swap_fees = 10_000 x 0.0005 = 5
    ///   captured  = 5 x 0.9         = 4.5
    ///   net       = 4.5 - 50        = -45.5  (verified)
    #[test]
    fn jit_pnl_negative_when_gas_exceeds_fees() {
        let pnl = JitV3Worker::compute_jit_pnl(
            10_000.0, // $10k swap — too small to cover gas
            5,        // 0.05% fee tier
            0.9,      // 90% JIT share
            50.0,     // $50 gas
        );
        assert!(!pnl.viable, "expected not-viable for $10k swap");
        assert!(
            pnl.net_profit_usd < 0.0,
            "expected negative net, got {}",
            pnl.net_profit_usd
        );
    }
}
