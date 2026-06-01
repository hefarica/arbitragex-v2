// M11 allow: the three `.try_into().unwrap()` calls convert
// `keccak256("fn()")[..4]` — a 4-byte slice — into `[u8; 4]`.
// This is unconditionally infallible (the slice is always 4 bytes).
// Follow-up: replace with selector consts (M11-TODO-selector-const).
#![allow(clippy::unwrap_used, clippy::expect_used)]
//! PoolSyncWorker — fetches V2 pool reserves via Multicall3, persists to
//! Postgres `pool_reserves` and Redis `arbx:pool_reserves:<chain>:<addr>`.
//! Also fetches V3 pool slot0 + liquidity via a separate Multicall3 batch and
//! writes `arbx:v3_slot0:<chain>:<addr>` so `prioritization-spine`'s
//! `ConfigAwareEvaluator::with_v3_slot0` can compute real V3 price impact
//! instead of falling back to the `max_slippage_pct` proxy (Sprint A H2).
//!
//! Boot sequence:
//!   1. Read V2+V3 pools+tokens+factories from Postgres (one query each).
//!   2. Populate Redis `arbx:tokens:*`, `arbx:pool_index:*`, `arbx:pool_index_v3:*`.
//!   3. Start polling loop: every `poll_interval`:
//!      a. 1 Multicall3 aggregate3 with N `getReserves()` calls for V2 pools.
//!      b. 1 Multicall3 aggregate3 with 2N calls for V3 pools (slot0 + liquidity).
//!      Both multicalls are separate RPC calls to avoid oversized calldata.
//!
//! Doctrine: log structured tracing JSON (no fake metrics), report measured
//! latency, fail-loud on RPC error (do not pretend success). Per R8 fail-honest:
//! if a V3 pool's slot0 call reverts, skip that pool and warn — do not write
//! a fake/zero value.

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
    set_pool_index, set_pool_index_v3, set_reserves, set_token_meta, set_v3_slot0, ReservesEntry,
    TokenMeta, V3PoolInfo, V3Slot0Entry,
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

    pub use IMulticall3::{aggregate3Call, Call3, Result};
}

// The getReserves / slot0 / liquidity keccak selectors are computed at runtime
// via ethers utils — no `abigen!` needed for single 4-byte constants.

const MULTICALL3_ADDR: &str = "0xcA11bde05977b3631167028862bE2a173976CA11";
const RESERVES_TTL_SECS: u64 = 30;
/// TTL for `arbx:v3_slot0` keys. Matches V2 reserves TTL (30s). PoolSyncWorker
/// ticks every ~5s so slots are refreshed 6x per TTL window.
const V3_SLOT0_TTL_SECS: u64 = 30;

/// Minimum returnData length for a valid slot0() response.
/// slot0 returns 7 ABI-words (uint160,int24,uint16,uint16,uint16,uint8,bool)
/// each padded to 32 bytes = 224 bytes.
const SLOT0_RETURN_LEN: usize = 7 * 32;
/// Minimum returnData length for a valid liquidity() response: 1 x uint128 = 32 bytes.
const LIQUIDITY_RETURN_LEN: usize = 32;

/// Default sub-calls per aggregate3. Public RPCs (publicnode/drpc/lava) hang or
/// reject oversized eth_call payloads, so we chunk. Override: `POOL_SYNC_BATCH_SIZE`.
const DEFAULT_MULTICALL_BATCH: usize = 25;
/// Hard per-multicall wall-clock timeout. `HttpRpcPool::with_retry` has NO timeout
/// of its own, so without this an unresponsive provider hangs the ENTIRE worker
/// (observed: a 60-call aggregate3 to a public RPC stalled the loop forever, leaving
/// `arbx:pool_reserves`/`arbx:v3_slot0` empty). Override: `POOL_SYNC_CALL_TIMEOUT_MS`.
const DEFAULT_MULTICALL_TIMEOUT_MS: u64 = 4_000;

/// Execute ONE aggregate3 over `chunk`, wrapped in a hard timeout + the pool's
/// failover/circuit-breaker. Returns the decoded per-call results (owned, aligned to
/// `chunk`) or an error (timeout / RPC / decode / short-result). Never hangs.
async fn exec_aggregate3(
    rpc_pool: &shared_rs::rpc_failover::HttpRpcPool,
    multicall_alloy: AlloyAddress,
    chunk: &[multicall_abi::Call3],
    call_timeout: Duration,
) -> anyhow::Result<Vec<multicall_abi::Result>> {
    let calldata = multicall_abi::aggregate3Call { calls: chunk.to_vec() }.abi_encode();
    let raw = match tokio::time::timeout(
        call_timeout,
        rpc_pool.with_retry(|provider| {
            let tx = TransactionRequest::default()
                .to(multicall_alloy)
                .input(TransactionInput::new(calldata.clone().into()));
            async move { provider.call(tx).await.map_err(|e| anyhow::anyhow!("{e}")) }
        }),
    )
    .await
    {
        Ok(Ok(b)) => b,
        Ok(Err(e)) => return Err(anyhow::anyhow!("rpc: {e}")),
        Err(_) => return Err(anyhow::anyhow!("timeout_{}ms", call_timeout.as_millis())),
    };
    let results = multicall_abi::aggregate3Call::abi_decode_returns(&raw)?;
    if results.len() != chunk.len() {
        return Err(anyhow::anyhow!(
            "short_result_{}_of_{}",
            results.len(),
            chunk.len()
        ));
    }
    Ok(results)
}

/// Resilient multicall: split `calls` into `batch_size` chunks, each timeout-bounded.
/// A chunk that fails/times out is retried CALL-BY-CALL (individual fallback) so one
/// unresponsive pool can't sink its whole batch and NOTHING can hang the worker.
/// Returns results aligned 1:1 to `calls` (`None` where the call ultimately failed),
/// so caller decode loops can index by position regardless of chunk boundaries.
async fn multicall_resilient(
    rpc_pool: &shared_rs::rpc_failover::HttpRpcPool,
    multicall_alloy: AlloyAddress,
    calls: &[multicall_abi::Call3],
    batch_size: usize,
    call_timeout: Duration,
    chain_id: u64,
    label: &str,
) -> Vec<Option<multicall_abi::Result>> {
    let mut out: Vec<Option<multicall_abi::Result>> = Vec::with_capacity(calls.len());
    let bs = batch_size.max(1);
    let mut start = 0usize;
    while start < calls.len() {
        let end = (start + bs).min(calls.len());
        let chunk = &calls[start..end];
        match exec_aggregate3(rpc_pool, multicall_alloy, chunk, call_timeout).await {
            Ok(results) => out.extend(results.into_iter().map(Some)),
            Err(e) => {
                // Degrade: the whole chunk failed/timed out — retry each call alone so a
                // single bad pool only loses itself, not the batch. Loud (R7 traceable).
                warn!(
                    event = "pool_sync.batch_fallback",
                    chain_id,
                    label,
                    batch = chunk.len(),
                    error = %e,
                    "multicall chunk failed; retrying call-by-call"
                );
                for call in chunk {
                    match exec_aggregate3(
                        rpc_pool,
                        multicall_alloy,
                        std::slice::from_ref(call),
                        call_timeout,
                    )
                    .await
                    {
                        Ok(mut r) => out.push(r.pop()),
                        Err(e2) => {
                            debug!(event = "pool_sync.single_call_failed", chain_id, label, error = %e2);
                            out.push(None);
                        }
                    }
                }
            }
        }
    }
    out
}

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

/// Minimal V3 pool descriptor for the slot0 polling loop. Only the pool address
/// is needed -- symbol and fee_tier are already in `arbx:pool_index_v3`.
struct V3PoolRow {
    address: H160,
    address_lower: String,
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
        info!(
            event = "pool_sync.boot",
            chain_id = self.chain_id,
            providers = rpc_pool.entries.len()
        );

        let multicall_addr = Address::from_str(MULTICALL3_ADDR)?;

        // Bootstrap: read pools + tokens from DB and populate Redis caches.
        // V2 pools enter the reserves polling loop (getReserves every tick).
        // V3 pools enter the slot0 polling loop (slot0+liquidity every tick)
        // to populate `arbx:v3_slot0` keys for the spine price-impact path.
        let pools = self.load_pools(&db).await?;
        info!(
            event = "pool_sync.pools_loaded",
            chain_id = self.chain_id,
            count = pools.len()
        );

        let v3_pools = self.load_v3_pools(&db).await?;

        self.bootstrap_token_cache(&db, &mut redis).await?;
        self.bootstrap_pool_index_cache(&pools, &mut redis).await?;
        let v3_count = self.bootstrap_v3_pool_index_cache(&db, &mut redis).await?;
        info!(
            event = "pool_sync.caches_bootstrapped",
            chain_id = self.chain_id,
            v2_pools = pools.len(),
            v3_pools = v3_count,
        );

        // Build static call data once per pool -- getReserves() has no args.
        let get_reserves_selector: [u8; 4] = ethers::utils::keccak256("getReserves()")[..4]
            .try_into()
            .unwrap();
        let get_reserves_calldata_bytes: Vec<u8> = get_reserves_selector.to_vec();

        // V3 selectors -- computed once at boot.
        let slot0_selector: [u8; 4] = ethers::utils::keccak256("slot0()")[..4].try_into().unwrap();
        let liquidity_selector: [u8; 4] = ethers::utils::keccak256("liquidity()")[..4]
            .try_into()
            .unwrap();
        let slot0_calldata: Vec<u8> = slot0_selector.to_vec();
        let liquidity_calldata: Vec<u8> = liquidity_selector.to_vec();

        // alloy address of the multicall contract -- computed once.
        let multicall_alloy = AlloyAddress::from_slice(multicall_addr.as_bytes());

        // Resilient-batching config: chunk size + hard per-call timeout. Both env-tunable
        // (no hardcoded productive values; defaults are conservative for public RPCs).
        let batch_size: usize = std::env::var("POOL_SYNC_BATCH_SIZE")
            .ok()
            .and_then(|v| v.parse().ok())
            .filter(|&n| n > 0)
            .unwrap_or(DEFAULT_MULTICALL_BATCH);
        let call_timeout = Duration::from_millis(
            std::env::var("POOL_SYNC_CALL_TIMEOUT_MS")
                .ok()
                .and_then(|v| v.parse().ok())
                .filter(|&n| n > 0)
                .unwrap_or(DEFAULT_MULTICALL_TIMEOUT_MS),
        );
        info!(
            event = "pool_sync.batch_config",
            chain_id = self.chain_id,
            batch_size,
            call_timeout_ms = call_timeout.as_millis() as u64,
            "resilient multicall batching active (chunk + timeout + call-by-call fallback)"
        );

        loop {
            let tick_start = Instant::now();

            // -- V2 reserves multicall -------------------------------------------
            let calls: Vec<multicall_abi::Call3> = pools
                .iter()
                .map(|p| multicall_abi::Call3 {
                    target: AlloyAddress::from_slice(p.address.as_bytes()),
                    allowFailure: true,
                    callData: get_reserves_calldata_bytes.clone().into(),
                })
                .collect();
            // Chunked, timeout-bounded, fallback-per-call. `results[i]` aligns to `pools[i]`;
            // `None` = that pool's getReserves ultimately failed. NEVER hangs the worker.
            let results = multicall_resilient(
                &rpc_pool,
                multicall_alloy,
                &calls,
                batch_size,
                call_timeout,
                self.chain_id,
                "v2_reserves",
            )
            .await;

            // Get current block once per tick — timeout-bounded so it can't hang either.
            let block_number: u64 = tokio::time::timeout(
                call_timeout,
                rpc_pool.with_retry(|provider| async move {
                    provider
                        .get_block_number()
                        .await
                        .map_err(|e| anyhow::anyhow!("{e}"))
                }),
            )
            .await
            .ok()
            .and_then(|r| r.ok())
            .unwrap_or(0);
            let now_ts = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0);

            let mut ok_count = 0usize;
            let mut fail_count = 0usize;

            // Persist each result. `results[i]` aligns 1:1 to `pools[i]`.
            for (pool, result) in pools.iter().zip(results.iter()) {
                // alloy sol! Result fields: `.success` (bool), `.returnData` (alloy Bytes).
                // None = the call (or its fallback) failed; Some-but-unsuccessful = on-chain revert.
                let result = match result {
                    Some(r) if r.success && r.returnData.len() >= 64 => r,
                    _ => {
                        fail_count += 1;
                        debug!(event = "pool_sync.pool_failed", pool = %pool.address_lower);
                        continue;
                    }
                };
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
                    warn!(
                        event = "pool_sync.redis_set_failed",
                        pool = %pool.address_lower,
                        error = %e
                    );
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
                    warn!(
                        event = "pool_sync.db_insert_failed",
                        pool = %pool.address_lower,
                        error = %e
                    );
                }

                ok_count += 1;
            }

            let elapsed_v2_ms = tick_start.elapsed().as_millis();
            info!(
                event = "pool_sync.tick",
                chain_id = self.chain_id,
                pools = pools.len(),
                ok = ok_count,
                failed = fail_count,
                block = block_number,
                latency_ms = elapsed_v2_ms as u64,
            );

            // -- V3 slot0 + liquidity multicall ----------------------------------
            // One aggregate3 call with 2 x N sub-calls: slot0() and liquidity()
            // interleaved per pool. Each pool occupies indices [2i, 2i+1].
            // If v3_pools is empty, skip entirely (no RPC call, no alloc).
            if !v3_pools.is_empty() {
                let v3_tick_start = Instant::now();
                let v3_calls: Vec<multicall_abi::Call3> = v3_pools
                    .iter()
                    .flat_map(|p| {
                        let target = AlloyAddress::from_slice(p.address.as_bytes());
                        [
                            multicall_abi::Call3 {
                                target,
                                allowFailure: true,
                                callData: slot0_calldata.clone().into(),
                            },
                            multicall_abi::Call3 {
                                target,
                                allowFailure: true,
                                callData: liquidity_calldata.clone().into(),
                            },
                        ]
                    })
                    .collect();

                // Chunked + timeout-bounded + per-call fallback. `v3_results[2i]` = slot0 and
                // `v3_results[2i+1]` = liquidity for `v3_pools[i]`; `None` = that call failed.
                // Chunk boundaries do NOT break this alignment (results stay positional).
                let v3_results = multicall_resilient(
                    &rpc_pool,
                    multicall_alloy,
                    &v3_calls,
                    batch_size,
                    call_timeout,
                    self.chain_id,
                    "v3_slot0",
                )
                .await;

                let mut v3_ok = 0usize;
                let mut v3_fail = 0usize;

                // Results are interleaved: index 2i = slot0, index 2i+1 = liquidity.
                for (i, pool) in v3_pools.iter().enumerate() {
                    let slot0_idx = 2 * i;
                    let liq_idx = 2 * i + 1;

                    // Bounds guard: aggregate3 may return fewer results on partial revert.
                    if liq_idx >= v3_results.len() {
                        warn!(
                            event = "pool_sync.v3_short_result",
                            pool = %pool.address_lower,
                            expected_idx = liq_idx,
                            got = v3_results.len(),
                        );
                        v3_fail += 1;
                        continue;
                    }

                    let slot0_res = match &v3_results[slot0_idx] {
                        Some(r) => r,
                        None => {
                            v3_fail += 1;
                            continue;
                        }
                    };
                    let liq_res = match &v3_results[liq_idx] {
                        Some(r) => r,
                        None => {
                            v3_fail += 1;
                            continue;
                        }
                    };

                    // R8 fail-honest: if either call failed or returned too few bytes,
                    // skip this pool. Log at warn so the gap is traceable (R7).
                    if !slot0_res.success || slot0_res.returnData.len() < SLOT0_RETURN_LEN {
                        warn!(
                            event = "pool_sync.v3_slot0_call_failed",
                            pool = %pool.address_lower,
                            success = slot0_res.success,
                            data_len = slot0_res.returnData.len(),
                        );
                        v3_fail += 1;
                        continue;
                    }
                    if !liq_res.success || liq_res.returnData.len() < LIQUIDITY_RETURN_LEN {
                        warn!(
                            event = "pool_sync.v3_liquidity_call_failed",
                            pool = %pool.address_lower,
                            success = liq_res.success,
                            data_len = liq_res.returnData.len(),
                        );
                        v3_fail += 1;
                        continue;
                    }

                    // Decode sqrtPriceX96 from slot0 returnData.
                    // ABI layout: uint160 is right-aligned in a 32-byte word.
                    // bytes [0..32] = sqrtPriceX96 (big-endian, 32-byte ABI word).
                    // uint160 fits in u128 for all realistic ETH prices:
                    //   max u128 ~ 3.4e38; sqrtPriceX96 at 1e12 price ~ 7.9e34 < 2^116.
                    let sqrt_raw = &slot0_res.returnData[0..32];
                    let sqrt_u256 = U256::from_big_endian(sqrt_raw);
                    // u128 saturation: if price ratio > 2^64 (impossible for live pools),
                    // skip rather than panic (defensive guard).
                    let sqrt_price_x96: u128 = if sqrt_u256 > U256::from(u128::MAX) {
                        warn!(
                            event = "pool_sync.v3_sqrt_overflow",
                            pool = %pool.address_lower,
                            value = %sqrt_u256,
                        );
                        v3_fail += 1;
                        continue;
                    } else {
                        sqrt_u256.low_u128()
                    };

                    // Decode liquidity from liquidity() returnData.
                    // uint128 is right-aligned in a 32-byte word.
                    let liq_raw = &liq_res.returnData[0..32];
                    let liq_u256 = U256::from_big_endian(liq_raw);
                    let liquidity: u128 = if liq_u256 > U256::from(u128::MAX) {
                        warn!(
                            event = "pool_sync.v3_liq_overflow",
                            pool = %pool.address_lower,
                            value = %liq_u256,
                        );
                        v3_fail += 1;
                        continue;
                    } else {
                        liq_u256.low_u128()
                    };

                    // Guard: uninitialized pool has sqrtPriceX96 = 0. Skip it --
                    // writing zero would give a 0-impact result in the spine.
                    if sqrt_price_x96 == 0 {
                        warn!(
                            event = "pool_sync.v3_uninitialized_pool",
                            pool = %pool.address_lower,
                        );
                        v3_fail += 1;
                        continue;
                    }

                    let slot0_entry = V3Slot0Entry {
                        sqrt_price_x96: sqrt_price_x96.to_string(),
                        liquidity: liquidity.to_string(),
                        ts: now_ts,
                    };

                    if let Err(e) = set_v3_slot0(
                        &mut redis,
                        self.chain_id,
                        &pool.address_lower,
                        &slot0_entry,
                        V3_SLOT0_TTL_SECS,
                    )
                    .await
                    {
                        warn!(
                            event = "pool_sync.v3_redis_set_failed",
                            pool = %pool.address_lower,
                            error = %e
                        );
                        // Redis errors are transient; don't count as v3_fail.
                    } else {
                        v3_ok += 1;
                    }
                }

                let v3_elapsed_ms = v3_tick_start.elapsed().as_millis();
                info!(
                    event = "pool_sync.v3_tick",
                    chain_id = self.chain_id,
                    pools = v3_pools.len(),
                    ok = v3_ok,
                    failed = v3_fail,
                    latency_ms = v3_elapsed_ms as u64,
                );
            }

            sleep(self.poll_interval).await;
        }
    }

    async fn load_pools(&self, db: &PgPool) -> anyhow::Result<Vec<PoolRow>> {
        // V2-only filter: V3 pools don't have getReserves(), so polling them
        // would cost an RPC call per pool per tick and always fail. V3 lives
        // in `load_v3_pools` + the slot0 polling path.
        // Selects t0.address so the scanner can resolve swap orientation
        // without computing both V2 directions (closes scanner.rs:350 TODO).
        // Nullable symbols: a V2 pool referencing a token with a NULL symbol must not crash
        // the worker (it just can't be pair-indexed) — skip it fail-honestly.
        let rows = sqlx::query_as::<_, (String, Option<String>, Option<String>, String)>(
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
                let (sym0, sym1) = match (sym0, sym1) {
                    (Some(a), Some(b)) if !a.is_empty() && !b.is_empty() => (a, b),
                    _ => return None, // null/empty symbol → not pair-indexable; skip
                };
                let lower = addr.to_lowercase();
                Address::from_str(&lower).ok().map(|h| PoolRow {
                    address: h,
                    address_lower: lower,
                    sym0,
                    sym1,
                    token0_address_lower: token0_addr.to_lowercase(),
                })
            })
            .collect())
    }

    /// Load V3 pool addresses for the per-tick slot0 polling loop.
    /// Only the address is needed -- symbol/fee_tier are in the pool_index_v3.
    async fn load_v3_pools(&self, db: &PgPool) -> anyhow::Result<Vec<V3PoolRow>> {
        let rows = sqlx::query_as::<_, (String,)>(
            r#"SELECT p.address
               FROM pools p
               JOIN factories f ON p.factory_id = f.id
               JOIN dexes d ON f.dex_id = d.id
               WHERE p.chain_id = $1
                 AND p.is_active = TRUE
                 AND d.protocol_type = 'UNISWAP_V3'"#,
        )
        .bind(self.chain_id as i64)
        .fetch_all(db)
        .await?;

        Ok(rows
            .into_iter()
            .filter_map(|(addr,)| {
                let lower = addr.to_lowercase();
                Address::from_str(&lower).ok().map(|h| V3PoolRow {
                    address: h,
                    address_lower: lower,
                })
            })
            .collect())
    }

    async fn bootstrap_token_cache(
        &self,
        db: &PgPool,
        redis: &mut redis::aio::ConnectionManager,
    ) -> anyhow::Result<()> {
        // Decode symbol/decimals as Option: a single token row with a NULL symbol or
        // decimals must NOT crash the whole worker at boot (that bug left every reserves/
        // slot0 cache empty). Fail-honest: skip the unusable token + count it, never
        // fabricate a symbol/decimals for a token that can't be priced or matched.
        let rows = sqlx::query_as::<_, (String, Option<String>, Option<i32>, Option<bool>)>(
            r#"SELECT address, symbol, decimals, is_stablecoin
               FROM tokens WHERE chain_id = $1 AND is_active = TRUE"#,
        )
        .bind(self.chain_id as i64)
        .fetch_all(db)
        .await?;

        let mut skipped = 0usize;
        for (addr, symbol, decimals, is_stable) in rows {
            let (symbol, decimals) = match (symbol, decimals) {
                (Some(s), Some(d)) if !s.is_empty() => (s, d),
                _ => {
                    skipped += 1;
                    debug!(event = "pool_sync.token_skipped_null", chain_id = self.chain_id, addr = %addr);
                    continue;
                }
            };
            let meta = TokenMeta {
                symbol,
                decimals: decimals as u8,
                // is_stablecoin is a hint, not a hard requirement — default missing to false.
                is_stablecoin: is_stable.unwrap_or(false),
            };
            if let Err(e) = set_token_meta(redis, self.chain_id, &addr.to_lowercase(), &meta).await
            {
                warn!(event = "pool_sync.token_cache_set_failed", error = %e);
            }
        }
        if skipped > 0 {
            warn!(
                event = "pool_sync.tokens_skipped_null",
                chain_id = self.chain_id,
                skipped,
                "tokens skipped — NULL/empty symbol or NULL decimals (data gap, not fatal)"
            );
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
            by_pair
                .entry((lo, hi))
                .or_default()
                .push(p.address_lower.clone());
        }
        for ((sym_a, sym_b), addrs) in by_pair {
            if let Err(e) = set_pool_index(redis, self.chain_id, &sym_a, &sym_b, &addrs).await {
                warn!(event = "pool_sync.pool_index_set_failed", error = %e);
            }
        }
        Ok(())
    }

    /// One-shot bootstrap of the V3 pool index. Reads V3 pools from PG (joined
    /// to factories->dexes for protocol_type filter), groups by sorted-symbol
    /// pair, and writes Vec<V3PoolInfo> per pair to Redis.
    ///
    /// V3 slot0 is populated per-tick in the main polling loop (not here).
    /// This index just lets the scanner discover which V3 pools cover a given
    /// pair so it can build a Multicall3-batched Quoter request.
    ///
    /// Returns the number of V3 pools indexed.
    async fn bootstrap_v3_pool_index_cache(
        &self,
        db: &PgPool,
        redis: &mut redis::aio::ConnectionManager,
    ) -> anyhow::Result<usize> {
        // Nullable symbol/fee_tier decode: one V3 pool whose token has a NULL symbol (or a
        // NULL fee_tier) must not crash the worker. Fail-honest: skip the unindexable pool.
        let rows = sqlx::query_as::<_, (String, Option<String>, Option<String>, Option<i32>)>(
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
        let mut skipped = 0usize;
        for (addr, sym0, sym1, fee_tier) in rows {
            let (sym0, sym1, fee_tier) = match (sym0, sym1, fee_tier) {
                (Some(a), Some(b), Some(f)) if !a.is_empty() && !b.is_empty() => (a, b, f),
                _ => {
                    skipped += 1;
                    debug!(event = "pool_sync.v3_pool_skipped_null", chain_id = self.chain_id, addr = %addr);
                    continue;
                }
            };
            let (lo, hi) = if sym0 <= sym1 {
                (sym0.clone(), sym1.clone())
            } else {
                (sym1.clone(), sym0.clone())
            };
            by_pair.entry((lo, hi)).or_default().push(V3PoolInfo {
                pool_addr: addr.to_lowercase(),
                fee_bps: fee_tier as u32,
            });
        }
        let total = by_pair.values().map(|v| v.len()).sum::<usize>();
        if skipped > 0 {
            warn!(
                event = "pool_sync.v3_pools_skipped_null",
                chain_id = self.chain_id,
                skipped,
                "V3 pools skipped from index — NULL symbol/fee_tier (data gap, not fatal)"
            );
        }

        for ((sym_a, sym_b), pools) in &by_pair {
            if let Err(e) = set_pool_index_v3(redis, self.chain_id, sym_a, sym_b, pools).await {
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
