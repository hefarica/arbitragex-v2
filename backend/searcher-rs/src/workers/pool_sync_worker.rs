//! PoolSyncWorker — fetches V2 pool reserves via Multicall3, persists to
//! Postgres `pool_reserves` and Redis `arbx:pool_reserves:<chain>:<addr>`.
//!
//! Boot sequence:
//!   1. Read pools+tokens+factories from Postgres (one query each, cached in struct).
//!   2. Populate Redis `arbx:tokens:*` and `arbx:pool_index:*` from DB rows.
//!   3. Start polling loop: every `poll_interval`, do 1 Multicall3 aggregate3 with
//!      N `getReserves()` calls, decode results, batch INSERT into pool_reserves,
//!      individual SET into Redis with 30s TTL.
//!
//! Doctrine: log structured tracing JSON (no fake metrics), report measured
//! latency, fail-loud on RPC error (do not pretend success).

use alloy::primitives::Address as AlloyAddress;
use alloy::providers::Provider as AlloyProvider;
use alloy::rpc::types::{TransactionInput, TransactionRequest};
use alloy::sol_types::SolCall;
use ethers::types::{Address, H160, U256};
use sqlx::PgPool;
use std::str::FromStr;
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tokio::time::sleep;
use tracing::{debug, info, warn};

use crate::reserves::{
    set_pool_index, set_pool_index_v3, set_reserves, set_token_meta, ReservesEntry, TokenMeta,
    V3PoolInfo,
};

/// Alloy 1.0 migration: replaced ethers `abigen!` for Multicall3 with `sol!`.
/// The `aggregate3` calldata is encoded via `SolCall::abi_encode()` and the
/// return bytes decoded via `SolCall::abi_decode_returns()`. The RPC call goes
/// through `AlloyProvider::call` (alloy `eth_call`). No live contract binding
/// object is created — we only need the ABI codec.
mod multicall_abi {
    use alloy::sol_types::sol;

    sol! {
        interface IMulticall3 {
            struct Call3 {
                address target;
                bool allowFailure;
                bytes callData;
            }
            struct Result {
                bool success;
                bytes returnData;
            }
            function aggregate3(Call3[] calldata calls) external payable returns (Result[] memory returnData);
        }
    }

    pub use IMulticall3::{aggregate3Call, Call3};
}

// The getReserves keccak selector is computed at runtime (ethers utils) — no
// `abigen!(IUniswapV2Pair, ...)` needed. This avoids pulling the macro for a
// single 4-byte constant and keeps the ethers abigen surface minimal.

const MULTICALL3_ADDR: &str = "0xcA11bde05977b3631167028862bE2a173976CA11";
const RESERVES_TTL_SECS: u64 = 30;

struct PoolRow {
    address: H160,
    address_lower: String,
    sym0: String,
    sym1: String,
    /// Lowercase 0x-prefixed address of token0. Plumbed into `ReservesEntry`
    /// so the scanner knows the swap orientation directly (which reserve is
    /// `in` vs `out`) without computing both directions and applying the
    /// dual-orientation magnitude heuristic. Closes the TODO at scanner.rs:350.
    token0_address_lower: String,
}

pub struct PoolSyncWorker {
    pub poll_interval: Duration,
    pub chain_id: u64,
}

impl PoolSyncWorker {
    pub fn new(poll_interval_ms: u64, chain_id: u64) -> Self {
        Self {
            poll_interval: Duration::from_millis(poll_interval_ms),
            chain_id,
        }
    }

    /// Bootstrap caches from DB then enter polling loop. Designed to run forever;
    /// returns only on unrecoverable errors.
    pub async fn run(
        self,
        rpc_pool: Arc<shared_rs::rpc_failover::HttpRpcPool>,
        db: PgPool,
        mut redis: redis::aio::ConnectionManager,
    ) -> anyhow::Result<()> {
        info!(event = "pool_sync.boot", chain_id = self.chain_id, providers = rpc_pool.entries.len());

        let multicall_addr = Address::from_str(MULTICALL3_ADDR)?;

        // Bootstrap: read pools + tokens from DB and populate Redis caches.
        // V2 pools enter the polling loop (getReserves every tick).
        // V3 pools are bootstrapped into a separate index but NOT polled —
        // V3 quoting goes through QuoterV2 at scan time (handled by scanner.rs),
        // since V3's slot0/liquidity state alone doesn't determine swap output
        // (tick crossing requires the on-chain math which Quoter encapsulates).
        let pools = self.load_pools(&db).await?;
        info!(event = "pool_sync.pools_loaded", chain_id = self.chain_id, count = pools.len());

        self.bootstrap_token_cache(&db, &mut redis).await?;
        self.bootstrap_pool_index_cache(&pools, &mut redis).await?;
        let v3_count = self.bootstrap_v3_pool_index_cache(&db, &mut redis).await?;
        info!(
            event = "pool_sync.caches_bootstrapped",
            chain_id = self.chain_id,
            v2_pools = pools.len(),
            v3_pools = v3_count,
        );

        // Build static call data once per pool — getReserves() has no args, just the selector.
        let get_reserves_selector: [u8; 4] = ethers::utils::keccak256("getReserves()")[..4]
            .try_into()
            .unwrap();
        let get_reserves_calldata_bytes: Vec<u8> = get_reserves_selector.to_vec();

        // alloy address of the multicall contract — computed once.
        let multicall_alloy = AlloyAddress::from_slice(multicall_addr.as_bytes());

        loop {
            let tick_start = Instant::now();
            // Build the aggregate3 calldata for ALL pools in one batch.
            // alloy sol! Call3 uses AlloyAddress and alloy Bytes (alloy_primitives::Bytes).
            let calls: Vec<multicall_abi::Call3> = pools
                .iter()
                .map(|p| multicall_abi::Call3 {
                    target: AlloyAddress::from_slice(p.address.as_bytes()),
                    allowFailure: true,
                    callData: get_reserves_calldata_bytes.clone().into(),
                })
                .collect();
            let calldata = multicall_abi::aggregate3Call { calls }.abi_encode();

            // Per-tick multicall via with_retry — circuit breaker + failover fire on RPC error.
            // Alloy 1.0: provider.call(TransactionRequest) → Bytes (raw return data).
            let raw_bytes = match rpc_pool.with_retry(|provider| {
                let tx = TransactionRequest::default()
                    .to(multicall_alloy)
                    .input(TransactionInput::new(calldata.clone().into()));
                async move {
                    provider.call(tx).await.map_err(|e| anyhow::anyhow!("{e}"))
                }
            }).await {
                Ok(b) => b,
                Err(e) => {
                    warn!(event = "pool_sync.multicall_failed", error = %e);
                    sleep(self.poll_interval).await;
                    continue;
                }
            };

            // Decode the aggregate3 return bytes.
            // alloy-sol-types 1.0: single return value → Vec<Result> returned directly.
            let results = match multicall_abi::aggregate3Call::abi_decode_returns(&raw_bytes) {
                Ok(r) => r,
                Err(e) => {
                    warn!(event = "pool_sync.multicall_decode_failed", error = %e);
                    sleep(self.poll_interval).await;
                    continue;
                }
            };

            // Get current block once per tick via with_retry.
            // alloy 1.0: get_block_number() returns u64 directly (no .as_u64()).
            let block_number: u64 = rpc_pool.with_retry(|provider| async move {
                provider.get_block_number().await.map_err(|e| anyhow::anyhow!("{e}"))
            }).await.unwrap_or(0);
            let now_ts = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0);

            let mut ok_count = 0usize;
            let mut fail_count = 0usize;

            // Persist each result.
            for (pool, result) in pools.iter().zip(results.iter()) {
                // alloy sol! Result fields: `.success` (bool), `.returnData` (alloy Bytes).
                if !result.success || result.returnData.len() < 64 {
                    fail_count += 1;
                    debug!(event = "pool_sync.pool_failed", pool = %pool.address_lower);
                    continue;
                }
                // ABI-decode (uint112 reserve0, uint112 reserve1, uint32 blockTimestampLast)
                // Each value is left-padded to 32 bytes in returndata.
                let bytes = &result.returnData;
                let r0 = U256::from_big_endian(&bytes[0..32]);
                let r1 = U256::from_big_endian(&bytes[32..64]);

                let entry = ReservesEntry {
                    r0: r0.to_string(),
                    r1: r1.to_string(),
                    // Plumb token0 from PG metadata so the scanner resolves
                    // swap orientation directly (no dual-direction heuristic).
                    token0_addr: Some(pool.token0_address_lower.clone()),
                    blk: block_number,
                    ts: now_ts,
                };

                // Redis SET with TTL.
                if let Err(e) = set_reserves(
                    &mut redis,
                    self.chain_id,
                    &pool.address_lower,
                    &entry,
                    RESERVES_TTL_SECS,
                )
                .await
                {
                    warn!(event = "pool_sync.redis_set_failed", pool = %pool.address_lower, error = %e);
                }

                // Postgres INSERT (best-effort; failures don't kill the loop).
                if let Err(e) = sqlx::query(
                    r#"INSERT INTO pool_reserves (pool_id, block_number, reserve0, reserve1, timestamp)
                       SELECT id, $1, $2::numeric, $3::numeric, NOW()
                       FROM pools WHERE chain_id=$4 AND address=$5"#,
                )
                .bind(block_number as i64)
                .bind(&entry.r0)
                .bind(&entry.r1)
                .bind(self.chain_id as i64)
                .bind(&pool.address_lower)
                .execute(&db)
                .await
                {
                    warn!(event = "pool_sync.db_insert_failed", pool = %pool.address_lower, error = %e);
                }

                ok_count += 1;
            }

            let elapsed_ms = tick_start.elapsed().as_millis();
            info!(
                event = "pool_sync.tick",
                chain_id = self.chain_id,
                pools = pools.len(),
                ok = ok_count,
                failed = fail_count,
                block = block_number,
                latency_ms = elapsed_ms as u64,
            );

            sleep(self.poll_interval).await;
        }
    }

    async fn load_pools(&self, db: &PgPool) -> anyhow::Result<Vec<PoolRow>> {
        // V2-only filter: V3 pools don't have getReserves(), so polling them
        // would cost an RPC call per pool per tick and always fail. V3 lives
        // in `bootstrap_v3_pool_index_cache` (one-shot index, no per-tick poll).
        // Selects t0.address so the scanner can resolve swap orientation
        // without computing both V2 directions (closes scanner.rs:350 TODO).
        let rows = sqlx::query_as::<_, (String, String, String, String)>(
            r#"SELECT p.address, t0.symbol, t1.symbol, t0.address
               FROM pools p
               JOIN tokens t0 ON p.token0_id = t0.id
               JOIN tokens t1 ON p.token1_id = t1.id
               JOIN factories f ON p.factory_id = f.id
               JOIN dexes d ON f.dex_id = d.id
               WHERE p.chain_id = $1
                 AND p.is_active = TRUE
                 AND d.protocol_type = 'UNISWAP_V2'"#,
        )
        .bind(self.chain_id as i64)
        .fetch_all(db)
        .await?;

        Ok(rows
            .into_iter()
            .filter_map(|(addr, sym0, sym1, token0_addr)| {
                let lower = addr.to_lowercase();
                Address::from_str(&lower)
                    .ok()
                    .map(|h| PoolRow {
                        address: h,
                        address_lower: lower,
                        sym0,
                        sym1,
                        token0_address_lower: token0_addr.to_lowercase(),
                    })
            })
            .collect())
    }

    async fn bootstrap_token_cache(
        &self,
        db: &PgPool,
        redis: &mut redis::aio::ConnectionManager,
    ) -> anyhow::Result<()> {
        let rows = sqlx::query_as::<_, (String, String, i32, bool)>(
            r#"SELECT address, symbol, decimals, is_stablecoin
               FROM tokens WHERE chain_id = $1 AND is_active = TRUE"#,
        )
        .bind(self.chain_id as i64)
        .fetch_all(db)
        .await?;

        for (addr, symbol, decimals, is_stable) in rows {
            let meta = TokenMeta {
                symbol,
                decimals: decimals as u8,
                is_stablecoin: is_stable,
            };
            if let Err(e) =
                set_token_meta(redis, self.chain_id, &addr.to_lowercase(), &meta).await
            {
                warn!(event = "pool_sync.token_cache_set_failed", error = %e);
            }
        }
        Ok(())
    }

    async fn bootstrap_pool_index_cache(
        &self,
        pools: &[PoolRow],
        redis: &mut redis::aio::ConnectionManager,
    ) -> anyhow::Result<()> {
        // Group V2 pool addresses by sorted-symbol pair.
        use std::collections::HashMap;
        let mut by_pair: HashMap<(String, String), Vec<String>> = HashMap::new();
        for p in pools {
            let (lo, hi) = if p.sym0 <= p.sym1 {
                (p.sym0.clone(), p.sym1.clone())
            } else {
                (p.sym1.clone(), p.sym0.clone())
            };
            by_pair.entry((lo, hi)).or_default().push(p.address_lower.clone());
        }
        for ((sym_a, sym_b), addrs) in by_pair {
            if let Err(e) = set_pool_index(redis, self.chain_id, &sym_a, &sym_b, &addrs).await {
                warn!(event = "pool_sync.pool_index_set_failed", error = %e);
            }
        }
        Ok(())
    }

    /// One-shot bootstrap of the V3 pool index. Reads V3 pools from PG (joined
    /// to factories→dexes for protocol_type filter), groups by sorted-symbol
    /// pair, and writes Vec<V3PoolInfo> per pair to Redis.
    ///
    /// V3 pools are NOT polled — QuoterV2 at scan time produces fresh quotes
    /// directly. This index just lets the scanner discover which V3 pools cover
    /// a given pair so it can build a Multicall3-batched Quoter request.
    ///
    /// Returns the number of V3 pools indexed.
    async fn bootstrap_v3_pool_index_cache(
        &self,
        db: &PgPool,
        redis: &mut redis::aio::ConnectionManager,
    ) -> anyhow::Result<usize> {
        let rows = sqlx::query_as::<_, (String, String, String, i32)>(
            r#"SELECT p.address, t0.symbol, t1.symbol, p.fee_tier
               FROM pools p
               JOIN tokens t0 ON p.token0_id = t0.id
               JOIN tokens t1 ON p.token1_id = t1.id
               JOIN factories f ON p.factory_id = f.id
               JOIN dexes d ON f.dex_id = d.id
               WHERE p.chain_id = $1
                 AND p.is_active = TRUE
                 AND d.protocol_type = 'UNISWAP_V3'"#,
        )
        .bind(self.chain_id as i64)
        .fetch_all(db)
        .await?;

        // Group by sorted-symbol pair.
        use std::collections::HashMap;
        let mut by_pair: HashMap<(String, String), Vec<V3PoolInfo>> = HashMap::new();
        let total = rows.len();
        for (addr, sym0, sym1, fee_tier) in rows {
            let (lo, hi) = if sym0 <= sym1 {
                (sym0.clone(), sym1.clone())
            } else {
                (sym1.clone(), sym0.clone())
            };
            by_pair
                .entry((lo, hi))
                .or_default()
                .push(V3PoolInfo {
                    pool_addr: addr.to_lowercase(),
                    fee_bps: fee_tier as u32,
                });
        }

        for ((sym_a, sym_b), pools) in &by_pair {
            if let Err(e) =
                set_pool_index_v3(redis, self.chain_id, sym_a, sym_b, pools).await
            {
                warn!(event = "pool_sync.v3_pool_index_set_failed", error = %e);
            }
        }
        info!(
            event = "pool_sync.v3_index_bootstrapped",
            chain_id = self.chain_id,
            pool_count = total,
            pair_count = by_pair.len(),
        );
        Ok(total)
    }
}

