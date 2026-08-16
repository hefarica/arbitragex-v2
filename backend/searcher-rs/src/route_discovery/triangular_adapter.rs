//! Triangular reserves adapter — FASE 2 of the OMEGA ACTIVACIÓN TRIANGULAR
//! directive (B3 / D-01/F2): the on-demand data bridge that
//! `needs_triangular_adapter` named.
//!
//! ## The problem it exists for
//! PoolSyncWorker only polls reserves for the pools it loaded from Postgres;
//! radar-discovered pools mostly are NOT in that set (measured coverage today:
//! 6/93 pools cached), so a triangular candidate built from the discovery
//! graph would ask the cache for legs that are simply absent — and every
//! cartridge would have to reject honestly forever. This adapter closes that
//! gap: before dispatch, each triangular cycle's legs are served from the
//! Redis cache when fresh, and cache misses / staleness are BACKFILLED via
//! ONE batched Multicall3 (V2 `getReserves` batch; V3 legs `slot0`+`liquidity`
//! — the exact call shapes `pool_sync_worker` proved), written back under the
//! canonical keys (`arbx:pool_reserves:*` / `arbx:v3_slot0:*`).
//!
//! ## Freshness contract (mirrors the consumer)
//! V2 reserves entries carry a block stamp; the TriangularWorker treats a hop
//! more than [`MAX_RESERVE_LAG_BLOCKS`] (5) behind as unusable — so the cache
//! hit requires `current_block - entry.blk <= 5`. V3 slot0 entries carry no
//! block (see `reserves.rs`), so their window is time-based at the 30s TTL
//! that PoolSyncWorker re-stamps every ~5s tick.
//!
//! ## Discipline
//! - **Bounded RPC**: at most [`MAX_BACKFILLS_PER_BLOCK`] (8) backfill
//!   multicalls per block (lock-free per-block budget). Exhaustion is a typed
//!   `Skipped::BudgetExhausted` — the caller drops that candidate for the tick
//!   and telemetries; it never degrades into an unbounded RPC storm.
//! - **R8 fail-honest**: any leg whose reserves cannot be obtained (cache
//!   miss, then a failed/reverted/short backfill) is a typed
//!   `Skipped::MissingReserves(pool)` — NEVER a synthesized value (RULE 00).
//! - **NO-ACTIVE invariant**: this module is read-only data plumbing — no
//!   emitter, no orchestrator, no write to `arbx:opps:detected`. Its strongest
//!   effect is a cache write-back of REAL on-chain readings.
//! - **REGLA 0f**: this PR merges BEFORE #350 (F1/F3) — the triangular
//!   dispatch that PR enables consumes this bridge.

use crate::reserves::{
    get_reserves, get_v3_slot0, set_reserves, set_v3_slot0, ReservesEntry, V3Slot0Entry,
};
use crate::route_discovery::types::RouteCandidate;
use crate::route_intent::{ProtocolType, RouteIntent};
use crate::workers::pool_sync_worker::{
    multicall_abi, LIQUIDITY_RETURN_LEN, MULTICALL3_ADDR, RESERVES_TTL_SECS, SLOT0_RETURN_LEN,
    V3_SLOT0_TTL_SECS,
};
use crate::workers::triangular_worker::MAX_RESERVE_LAG_BLOCKS;
use alloy::primitives::Address as AlloyAddress;
use alloy::providers::Provider as AlloyProvider;
use alloy::rpc::types::{TransactionInput, TransactionRequest};
use alloy::sol_types::SolCall;
use async_trait::async_trait;
use ethers::types::{Address, H256};
use redis::aio::ConnectionManager;
use shared_rs::rpc_failover::HttpRpcPool;
use std::str::FromStr;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tracing::{debug, warn};

/// Per-block backfill allowance. One backfill = ONE Multicall3 `aggregate3`
/// covering all of a cycle's missing legs (3-6 sub-calls), so 8 backfills per
/// 12s block bounds the adapter at ≤8 RPC requests/block worst-case — the same
/// order of magnitude as PoolSyncWorker's own per-tick multicall traffic.
/// Exhaustion is honest (`Skipped::BudgetExhausted`), never a silent drop.
pub const MAX_BACKFILLS_PER_BLOCK: u64 = 8;

/// Hard wall-clock timeout for the backfill multicall (and the per-tick block
/// fetch). Mirrors pool_sync_worker's `DEFAULT_MULTICALL_TIMEOUT_MS` rationale:
/// `HttpRpcPool::with_retry` has no timeout of its own, so without this a dead
/// provider would hang the discovery tick.
pub const ADAPTER_CALL_TIMEOUT_MS: u64 = 4_000;

/// V3 slot0 freshness window. Slot0 entries carry no block number, so the
/// bound is time-based; 30s aligns with the `arbx:v3_slot0` TTL contract in
/// `reserves.rs` (re-stamped every ~5s by PoolSyncWorker, TTL 30s).
const V3_SLOT0_FRESH_SECS: u64 = V3_SLOT0_TTL_SECS;

/// One leg of a triangular cycle, as the adapter needs it: which pool to
/// ensure, its protocol family, and the leg's token pair (the pair is enough
/// to derive V2 `token0` — V2 pools sort their tokens by address).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CycleLeg {
    pub pool: Address,
    pub token_in: Address,
    pub token_out: Address,
    pub protocol: ProtocolType,
}

/// Project a discovered [`RouteCandidate`] into the legs the adapter serves
/// (parallel-vector shape → per-hop legs, closing hop included via `% n`).
/// `None` on a shape mismatch (empty or ragged vectors) — the caller skips
/// honestly rather than guessing which pool belongs to which hop.
pub fn cycle_legs(c: &RouteCandidate) -> Option<Vec<CycleLeg>> {
    if c.pools.is_empty()
        || c.pools.len() != c.protocols.len()
        || c.protocols.len() != c.tokens.len()
    {
        return None;
    }
    let n = c.pools.len();
    Some(
        (0..n)
            .map(|i| CycleLeg {
                pool: c.pools[i],
                token_in: c.tokens[i],
                token_out: c.tokens[(i + 1) % n],
                protocol: c.protocols[i],
            })
            .collect(),
    )
}

/// Why a candidate was skipped. Both variants are honest stops (R8): the
/// candidate is dropped for this tick and the reason telemetered — never a
/// partial/synthetic evaluation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Skipped {
    /// The per-block backfill allowance ([`MAX_BACKFILLS_PER_BLOCK`]) is spent.
    BudgetExhausted,
    /// A leg's reserves/slot0 could not be obtained (cache miss + backfill
    /// failed, reverted, short, or unwritable). Carries the offending pool.
    MissingReserves(Address),
}

impl Skipped {
    /// Stable telemetry token (matches `route_discovery.adapter_skipped.reason`).
    pub fn as_str(self) -> &'static str {
        match self {
            Skipped::BudgetExhausted => "budget_exhausted",
            Skipped::MissingReserves(_) => "missing_reserves",
        }
    }
}

/// Outcome of [`ensure_reserves`] for one candidate.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AdapterOutcome {
    /// Every leg was already cache-fresh — zero RPC.
    AllFresh,
    /// Misses/stale legs were backfilled from chain and written back;
    /// `n` = legs written.
    Backfilled(usize),
    /// Candidate skipped, honestly (see [`Skipped`]).
    Skipped(Skipped),
}

/// Bounded per-block backfill allowance: an AtomicU64 counter + the block it
/// belongs to, reset whenever a new block is observed. Lock-free CAS claims, so
/// concurrent ticks can never both spend the same slot (and never overspend).
///
/// Note on `current_block == 0` (block fetch failed): the allowance then never
/// resets — after [`MAX_BACKFILLS_PER_BLOCK`] spends the adapter stays
/// exhausted until a real block number is seen again. That is deliberate
/// fail-honest degradation: a broken RPC path must not keep issuing backfills.
#[derive(Debug, Default)]
pub struct BackfillBudget {
    block: AtomicU64,
    used: AtomicU64,
}

impl BackfillBudget {
    pub fn new() -> Self {
        Self::default()
    }

    /// Claim one backfill slot for `current_block`. `true` = spend it.
    pub fn claim(&self, current_block: u64) -> bool {
        loop {
            let observed_block = self.block.load(Ordering::Acquire);
            if observed_block != current_block {
                // New block → reset the allowance, then fall through to claim.
                // A concurrent winner of this CAS performs the same reset.
                if self
                    .block
                    .compare_exchange(
                        observed_block,
                        current_block,
                        Ordering::AcqRel,
                        Ordering::Acquire,
                    )
                    .is_ok()
                {
                    self.used.store(0, Ordering::Release);
                }
                continue;
            }
            let used = self.used.load(Ordering::Acquire);
            if used >= MAX_BACKFILLS_PER_BLOCK {
                return false;
            }
            if self
                .used
                .compare_exchange(used, used + 1, Ordering::AcqRel, Ordering::Acquire)
                .is_ok()
            {
                return true;
            }
            // Lost the race against a concurrent claim → retry.
        }
    }
}

/// V2 reserves freshness: the entry is usable when it is at most
/// [`MAX_RESERVE_LAG_BLOCKS`] behind `current_block` — the same bound the
/// TriangularWorker enforces on its hops, so what the adapter calls "fresh" is
/// exactly what the consumer would accept. `current_block == 0` (tick block
/// fetch failed) ⇒ freshness is UNVERIFIABLE ⇒ treated as stale, forcing a
/// backfill rather than a blind cache hit (R8).
pub fn v2_reserves_fresh(entry: &ReservesEntry, current_block: u64) -> bool {
    current_block != 0 && current_block.saturating_sub(entry.blk) <= MAX_RESERVE_LAG_BLOCKS
}

/// V3 slot0 freshness: time-based window at the slot0 TTL (entries carry no
/// block — see `reserves.rs`). `now_ts == 0` (clock unavailable) ⇒
/// unverifiable ⇒ stale, same fail-honest direction as the V2 guard.
pub fn v3_slot0_fresh(entry: &V3Slot0Entry, now_ts: u64) -> bool {
    now_ts != 0 && now_ts.saturating_sub(entry.ts) <= V3_SLOT0_FRESH_SECS
}

/// Injectable data boundary so [`ensure_reserves`] is unit-testable without
/// Redis/RPC — the same seam philosophy as `graph_builder`'s pure/async split
/// and `pool_sync_worker`'s synthetic executor for `drive_resilient_batches`.
///
/// Semantics (production `RedisRpcBridge` and test fakes must both honor
/// them):
/// - reads return `Option` — `None` = absent OR unreadable (a Redis error is
///   an honest miss → the leg becomes a backfill candidate, matching
///   `graph_builder`'s `.ok().flatten()` tolerance);
/// - writes return `bool` — `false` = the entry did NOT land in the cache;
/// - `aggregate3` executes ONE Multicall3 over `calls` with results aligned
///   1:1; `Err` = the whole request failed (timeout / RPC / circuit-open).
#[async_trait]
pub trait ReservesBridge: Send + Sync {
    async fn read_v2(&self, chain_id: u64, pool_lower: &str) -> Option<ReservesEntry>;
    async fn read_v3(&self, chain_id: u64, pool_lower: &str) -> Option<V3Slot0Entry>;
    async fn write_v2(&self, chain_id: u64, pool_lower: &str, entry: &ReservesEntry) -> bool;
    async fn write_v3(&self, chain_id: u64, pool_lower: &str, entry: &V3Slot0Entry) -> bool;
    async fn aggregate3(
        &self,
        calls: Vec<multicall_abi::Call3>,
    ) -> anyhow::Result<Vec<multicall_abi::Result>>;
}

/// Production [`ReservesBridge`]: the Redis caches the worker already holds
/// (a `ConnectionManager` clone — same multiplexed connection) + the shared
/// read-only `HttpRpcPool`. `rpc_pool = None` (non-mainnet / RPC unset) keeps
/// the cache-read path alive and makes every backfill fail honestly with
/// `no_rpc_pool`.
pub struct RedisRpcBridge {
    pub redis: ConnectionManager,
    pub rpc_pool: Option<Arc<HttpRpcPool>>,
}

#[async_trait]
impl ReservesBridge for RedisRpcBridge {
    async fn read_v2(&self, chain_id: u64, pool_lower: &str) -> Option<ReservesEntry> {
        let mut redis = self.redis.clone();
        get_reserves(&mut redis, chain_id, pool_lower)
            .await
            .ok()
            .flatten()
    }

    async fn read_v3(&self, chain_id: u64, pool_lower: &str) -> Option<V3Slot0Entry> {
        let mut redis = self.redis.clone();
        get_v3_slot0(&mut redis, chain_id, pool_lower)
            .await
            .ok()
            .flatten()
    }

    async fn write_v2(&self, chain_id: u64, pool_lower: &str, entry: &ReservesEntry) -> bool {
        let mut redis = self.redis.clone();
        set_reserves(&mut redis, chain_id, pool_lower, entry, RESERVES_TTL_SECS)
            .await
            .is_ok()
    }

    async fn write_v3(&self, chain_id: u64, pool_lower: &str, entry: &V3Slot0Entry) -> bool {
        let mut redis = self.redis.clone();
        set_v3_slot0(&mut redis, chain_id, pool_lower, entry, V3_SLOT0_TTL_SECS)
            .await
            .is_ok()
    }

    async fn aggregate3(
        &self,
        calls: Vec<multicall_abi::Call3>,
    ) -> anyhow::Result<Vec<multicall_abi::Result>> {
        let rpc = self
            .rpc_pool
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("no_rpc_pool"))?;
        // ONE aggregate3 over every needed leg, through the pool's own
        // failover retry, wrapped in a hard timeout (pool_sync_worker shape).
        let count = calls.len();
        let calldata = multicall_abi::aggregate3Call { calls }.abi_encode();
        let multicall_alloy = AlloyAddress::from_str(MULTICALL3_ADDR)?;
        let raw = tokio::time::timeout(
            Duration::from_millis(ADAPTER_CALL_TIMEOUT_MS),
            rpc.with_retry(|provider| {
                let tx = TransactionRequest::default()
                    .to(multicall_alloy)
                    .input(TransactionInput::new(calldata.clone().into()));
                async move { provider.call(tx).await.map_err(|e| anyhow::anyhow!("{e}")) }
            }),
        )
        .await
        .map_err(|_| anyhow::anyhow!("timeout_{}ms", ADAPTER_CALL_TIMEOUT_MS))??;
        let results = multicall_abi::aggregate3Call::abi_decode_returns(&raw)?;
        anyhow::ensure!(
            results.len() == count,
            "short_result_{}_of_{}",
            results.len(),
            count
        );
        Ok(results)
    }
}

/// Function selector for a no-arg view call (`keccak256(sig)[..4]`).
fn selector(sig: &str) -> Vec<u8> {
    ethers::utils::keccak256(sig)[..4].to_vec()
}

/// Which leg a sub-call of the backfill multicall decodes into.
#[derive(Debug, Clone, Copy)]
enum SubCallSlot {
    /// `getReserves()` for leg `i`.
    V2Reserves(usize),
    /// `slot0()` for leg `i` (paired with `V3Liquidity(i)`).
    V3Slot0(usize),
    /// `liquidity()` for leg `i`.
    V3Liquidity(usize),
}

/// Ensure every leg of a triangular cycle has fresh reserves/slot0 data in the
/// cache, backfilling misses with ONE bounded multicall (see module doc).
///
/// Order of operations: (1) cache pass — fresh legs are free; (2) budget
/// claim — BEFORE any RPC; (3) ONE `aggregate3` covering all needed legs;
/// (4) per-leg ABI decode + canonical-key write-back. Legs that decoded fine
/// are written even when a sibling leg fails (partial REAL data is still real
/// data); the candidate outcome is then `Skipped::MissingReserves`.
pub async fn ensure_reserves<B: ReservesBridge + ?Sized>(
    bridge: &B,
    budget: &BackfillBudget,
    chain_id: u64,
    current_block: u64,
    now_ts: u64,
    legs: &[CycleLeg],
) -> AdapterOutcome {
    // ── 1) Cache pass ───────────────────────────────────────────────────────
    let mut need: Vec<CycleLeg> = Vec::with_capacity(legs.len());
    for leg in legs {
        let pool_lower = format!("{:#x}", leg.pool);
        let fresh = match leg.protocol {
            ProtocolType::V2 => bridge
                .read_v2(chain_id, &pool_lower)
                .await
                .is_some_and(|e| v2_reserves_fresh(&e, current_block)),
            ProtocolType::V3 => bridge
                .read_v3(chain_id, &pool_lower)
                .await
                .is_some_and(|e| v3_slot0_fresh(&e, now_ts)),
            // A triangular candidate is V2/V3-only by RouteKind::classify, but
            // defend anyway: neither cache can ever serve this leg.
            ProtocolType::Curve | ProtocolType::Balancer | ProtocolType::Unknown => {
                return AdapterOutcome::Skipped(Skipped::MissingReserves(leg.pool));
            }
        };
        if !fresh && !need.iter().any(|l| l.pool == leg.pool) {
            need.push(*leg);
        }
    }
    if need.is_empty() {
        return AdapterOutcome::AllFresh;
    }

    // ── 2) Budget gate (before any RPC) ────────────────────────────────────
    if !budget.claim(current_block) {
        return AdapterOutcome::Skipped(Skipped::BudgetExhausted);
    }

    // ── 3) ONE multicall for every needed leg ──────────────────────────────
    let get_reserves_calldata = selector("getReserves()");
    let slot0_calldata = selector("slot0()");
    let liquidity_calldata = selector("liquidity()");

    let mut calls: Vec<multicall_abi::Call3> = Vec::with_capacity(need.len() * 2);
    let mut slots: Vec<SubCallSlot> = Vec::with_capacity(need.len() * 2);
    for (i, leg) in need.iter().enumerate() {
        let target = AlloyAddress::from_slice(leg.pool.as_bytes());
        match leg.protocol {
            ProtocolType::V2 => {
                calls.push(multicall_abi::Call3 {
                    target,
                    allowFailure: true,
                    callData: get_reserves_calldata.clone().into(),
                });
                slots.push(SubCallSlot::V2Reserves(i));
            }
            ProtocolType::V3 => {
                calls.push(multicall_abi::Call3 {
                    target,
                    allowFailure: true,
                    callData: slot0_calldata.clone().into(),
                });
                slots.push(SubCallSlot::V3Slot0(i));
                calls.push(multicall_abi::Call3 {
                    target,
                    allowFailure: true,
                    callData: liquidity_calldata.clone().into(),
                });
                slots.push(SubCallSlot::V3Liquidity(i));
            }
            // Unreachable: `need` only collects V2/V3 legs (cache pass above
            // returns early for anything else). Defensive no-op.
            ProtocolType::Curve | ProtocolType::Balancer | ProtocolType::Unknown => {}
        }
    }

    let first_needed_pool = need[0].pool;
    let results = match bridge.aggregate3(calls).await {
        Ok(r) => r,
        Err(e) => {
            warn!(
                event = "route_discovery.adapter_backfill_failed",
                chain_id,
                pools = need.len(),
                error = %e,
                "backfill multicall failed — skipping candidate honestly (R8)"
            );
            return AdapterOutcome::Skipped(Skipped::MissingReserves(first_needed_pool));
        }
    };

    // ── 4) Decode + write back (partial-real-data policy) ──────────────────
    // slot0/liquidity per V3 leg are consumed pairwise, so buffer the decoded
    // sqrt until its liquidity sibling is decoded too.
    let mut written = 0usize;
    let mut failed: Option<Address> = None;
    let mut pending_sqrt: Vec<Option<u128>> = vec![None; need.len()];

    let leg_fail = |leg: &CycleLeg, why: &str, failed: &mut Option<Address>| {
        warn!(
            event = "route_discovery.adapter_leg_failed",
            chain_id,
            pool = %format!("{:#x}", leg.pool),
            reason = why
        );
        if failed.is_none() {
            *failed = Some(leg.pool);
        }
    };

    for (idx, slot) in slots.iter().enumerate() {
        let (leg, res_ok, return_data): (&CycleLeg, bool, &[u8]) = match results.get(idx) {
            Some(r) => (&need[slot.leg_index()], r.success, &r.returnData),
            None => (&need[slot.leg_index()], false, &[]),
        };
        let pool_lower = format!("{:#x}", leg.pool);
        match slot {
            SubCallSlot::V2Reserves(_) => {
                // ABI: uint112 reserve0, uint112 reserve1, uint32 blockTimestampLast
                // — each left-padded to one 32-byte word; ≥2 words required.
                if !res_ok || return_data.len() < 64 {
                    leg_fail(leg, "getreserves_failed_or_short", &mut failed);
                    continue;
                }
                let r0 = ethers::types::U256::from_big_endian(&return_data[0..32]).to_string();
                let r1 = ethers::types::U256::from_big_endian(&return_data[32..64]).to_string();
                // token0 is derived, not fabricated: V2 pools sort their token
                // pair by address, and the leg carries both tokens.
                let token0 = if leg.token_in < leg.token_out {
                    leg.token_in
                } else {
                    leg.token_out
                };
                let entry = ReservesEntry {
                    r0,
                    r1,
                    token0_addr: Some(format!("{token0:#x}")),
                    blk: current_block,
                    ts: now_ts,
                };
                if bridge.write_v2(chain_id, &pool_lower, &entry).await {
                    written += 1;
                } else {
                    leg_fail(leg, "redis_write_failed", &mut failed);
                }
            }
            SubCallSlot::V3Slot0(i) => {
                // ABI: 7 words (uint160 sqrtPriceX96 first). uint160 fits u128
                // for all live pools; overflow means corrupt data → skip leg.
                if !res_ok || return_data.len() < SLOT0_RETURN_LEN {
                    leg_fail(leg, "slot0_failed_or_short", &mut failed);
                    continue;
                }
                let sqrt = ethers::types::U256::from_big_endian(&return_data[0..32]);
                if sqrt > ethers::types::U256::from(u128::MAX) {
                    leg_fail(leg, "slot0_sqrt_overflow", &mut failed);
                    continue;
                }
                let sqrt = sqrt.low_u128();
                if sqrt == 0 {
                    // Uninitialized pool — writing zero would price the leg at 0.
                    leg_fail(leg, "slot0_uninitialized_pool", &mut failed);
                    continue;
                }
                pending_sqrt[*i] = Some(sqrt);
            }
            SubCallSlot::V3Liquidity(i) => {
                let Some(sqrt) = pending_sqrt[*i] else {
                    // Slot0 sibling already failed → this leg is already failed.
                    continue;
                };
                if !res_ok || return_data.len() < LIQUIDITY_RETURN_LEN {
                    leg_fail(leg, "liquidity_failed_or_short", &mut failed);
                    continue;
                }
                let liq = ethers::types::U256::from_big_endian(&return_data[0..32]);
                if liq > ethers::types::U256::from(u128::MAX) {
                    leg_fail(leg, "liquidity_overflow", &mut failed);
                    continue;
                }
                let entry = V3Slot0Entry {
                    sqrt_price_x96: sqrt.to_string(),
                    liquidity: liq.low_u128().to_string(),
                    ts: now_ts,
                };
                if bridge.write_v3(chain_id, &pool_lower, &entry).await {
                    written += 1;
                } else {
                    leg_fail(leg, "redis_write_failed", &mut failed);
                }
            }
        }
    }

    match failed {
        Some(pool) => AdapterOutcome::Skipped(Skipped::MissingReserves(pool)),
        None => {
            debug!(
                event = "route_discovery.adapter_backfilled",
                chain_id,
                legs_written = written
            );
            AdapterOutcome::Backfilled(written)
        }
    }
}

/// Dispatch gate for adapter-skipped routes: drop intents whose `tx_hash`
/// equals a skipped route's `route_hash` (`build_intent` stamps
/// `tx_hash = route_hash`, so the match is exact). Non-triangular intents
/// never collide (route hashes are keccak digests per route).
/// Returns the surviving intents + how many were dropped.
pub fn gate_dispatch_intents(
    intents: Vec<RouteIntent>,
    skipped_hashes: &[H256],
) -> (Vec<RouteIntent>, usize) {
    let mut kept = Vec::with_capacity(intents.len());
    let mut dropped = 0usize;
    for intent in intents {
        if skipped_hashes.contains(&intent.tx_hash) {
            dropped += 1;
        } else {
            kept.push(intent);
        }
    }
    (kept, dropped)
}

impl SubCallSlot {
    fn leg_index(self) -> usize {
        match self {
            SubCallSlot::V2Reserves(i) | SubCallSlot::V3Slot0(i) | SubCallSlot::V3Liquidity(i) => i,
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use crate::route_discovery::types::{RouteDirection, RouteKind};
    use crate::route_intent::{DetectionSource, RouteIntentLeg, RouterKind, SwapExactMode};
    use alloy::primitives::Bytes as AlloyBytes;
    use ethers::types::U256;
    use std::collections::{HashMap, VecDeque};
    use std::sync::Mutex;

    fn addr(n: u64) -> Address {
        Address::from_low_u64_be(n)
    }

    fn pool_lower(a: Address) -> String {
        format!("{a:#x}")
    }

    /// 32-byte big-endian ABI word.
    fn word(v: u64) -> Vec<u8> {
        let mut buf = [0u8; 32];
        U256::from(v).to_big_endian(&mut buf);
        buf.to_vec()
    }

    fn v2_entry(blk: u64) -> ReservesEntry {
        ReservesEntry {
            r0: "1000".into(),
            r1: "2000".into(),
            token0_addr: Some(pool_lower(addr(1))),
            blk,
            ts: 1_000,
        }
    }

    fn v3_entry(ts: u64) -> V3Slot0Entry {
        V3Slot0Entry {
            sqrt_price_x96: "1682363847015700853685856186817536".into(),
            liquidity: "12345678901234567890".into(),
            ts,
        }
    }

    fn leg(pool: u64, token_in: u64, token_out: u64, proto: ProtocolType) -> CycleLeg {
        CycleLeg {
            pool: addr(pool),
            token_in: addr(token_in),
            token_out: addr(token_out),
            protocol: proto,
        }
    }

    /// Fake bridge: canned caches + a queue of per-call aggregate3 responses.
    /// Records writes and call counts so tests can assert ONE-call batching.
    /// State lives behind a `Mutex` (interior mutability through `&self`,
    /// required by the `Send + Sync` trait bound); guards are never held
    /// across an await, so the `async_trait` boxed futures stay `Send`.
    struct FakeBridge(Mutex<FakeState>);

    struct FakeState {
        v2: HashMap<String, ReservesEntry>,
        v3: HashMap<String, V3Slot0Entry>,
        writes_v2: Vec<(String, ReservesEntry)>,
        writes_v3: Vec<(String, V3Slot0Entry)>,
        aggregate3_calls: usize,
        subcall_counts: Vec<usize>,
        responses: VecDeque<anyhow::Result<Vec<multicall_abi::Result>>>,
    }

    impl FakeBridge {
        fn new() -> Self {
            Self(Mutex::new(FakeState {
                v2: HashMap::new(),
                v3: HashMap::new(),
                writes_v2: Vec::new(),
                writes_v3: Vec::new(),
                aggregate3_calls: 0,
                subcall_counts: Vec::new(),
                responses: VecDeque::new(),
            }))
        }

        fn seed_v2(&self, pool: Address, entry: ReservesEntry) {
            self.0.lock().unwrap().v2.insert(pool_lower(pool), entry);
        }

        /// Queue one successful aggregate3 whose per-sub-call `returnData` is
        /// `datas[i]` (`None` ⇒ that sub-call failed).
        fn push_ok(&self, datas: Vec<Option<Vec<u8>>>) {
            let results = datas
                .into_iter()
                .map(|d| match d {
                    Some(bytes) => multicall_abi::Result {
                        success: true,
                        returnData: AlloyBytes::from(bytes),
                    },
                    None => multicall_abi::Result {
                        success: false,
                        returnData: AlloyBytes::new(),
                    },
                })
                .collect();
            self.0.lock().unwrap().responses.push_back(Ok(results));
        }

        fn push_err(&self, msg: &str) {
            self.0
                .lock()
                .unwrap()
                .responses
                .push_back(Err(anyhow::anyhow!(msg.to_string())));
        }

        fn calls(&self) -> usize {
            self.0.lock().unwrap().aggregate3_calls
        }

        fn subcall_counts(&self) -> Vec<usize> {
            self.0.lock().unwrap().subcall_counts.clone()
        }

        fn writes_v2(&self) -> Vec<(String, ReservesEntry)> {
            self.0.lock().unwrap().writes_v2.clone()
        }

        fn writes_v3(&self) -> Vec<(String, V3Slot0Entry)> {
            self.0.lock().unwrap().writes_v3.clone()
        }
    }

    #[async_trait]
    impl ReservesBridge for FakeBridge {
        async fn read_v2(&self, _chain_id: u64, pool_lower: &str) -> Option<ReservesEntry> {
            self.0.lock().unwrap().v2.get(pool_lower).cloned()
        }

        async fn read_v3(&self, _chain_id: u64, pool_lower: &str) -> Option<V3Slot0Entry> {
            self.0.lock().unwrap().v3.get(pool_lower).cloned()
        }

        async fn write_v2(&self, _chain_id: u64, pool_lower: &str, entry: &ReservesEntry) -> bool {
            self.0
                .lock()
                .unwrap()
                .writes_v2
                .push((pool_lower.to_string(), entry.clone()));
            true
        }

        async fn write_v3(&self, _chain_id: u64, pool_lower: &str, entry: &V3Slot0Entry) -> bool {
            self.0
                .lock()
                .unwrap()
                .writes_v3
                .push((pool_lower.to_string(), entry.clone()));
            true
        }

        async fn aggregate3(
            &self,
            calls: Vec<multicall_abi::Call3>,
        ) -> anyhow::Result<Vec<multicall_abi::Result>> {
            assert!(
                !calls.is_empty(),
                "adapter must never issue an empty multicall"
            );
            let mut state = self.0.lock().unwrap();
            state.aggregate3_calls += 1;
            state.subcall_counts.push(calls.len());
            state
                .responses
                .pop_front()
                .expect("test setup: one queued response per expected call")
        }
    }

    const NOW: u64 = 10_000;

    #[tokio::test]
    async fn fresh_cache_returns_all_fresh_without_rpc() {
        let bridge = FakeBridge::new();
        for p in [0x10u64, 0x20, 0x30] {
            bridge.seed_v2(addr(p), v2_entry(100));
        }
        let legs = vec![
            leg(0x10, 1, 2, ProtocolType::V2),
            leg(0x20, 2, 3, ProtocolType::V2),
            leg(0x30, 3, 1, ProtocolType::V2),
        ];
        let budget = BackfillBudget::new();
        let out = ensure_reserves(&bridge, &budget, 1, 103, NOW, &legs).await;
        assert_eq!(out, AdapterOutcome::AllFresh);
        assert_eq!(bridge.calls(), 0, "fresh cache must issue no RPC");
    }

    #[tokio::test]
    async fn stale_legs_backfill_in_one_multicall() {
        let bridge = FakeBridge::new();
        // Leg 0 is 6 blocks stale (> MAX_RESERVE_LAG_BLOCKS); legs 1-2 fresh.
        bridge.seed_v2(addr(0x10), v2_entry(97));
        bridge.seed_v2(addr(0x20), v2_entry(103));
        bridge.seed_v2(addr(0x30), v2_entry(103));
        // getReserves() → 3 ABI words (r0, r1, blockTimestampLast).
        bridge.push_ok(vec![Some(
            word(111)
                .into_iter()
                .chain(word(222))
                .chain(word(0))
                .collect(),
        )]);
        let legs = vec![
            leg(0x10, 1, 2, ProtocolType::V2),
            leg(0x20, 2, 3, ProtocolType::V2),
            leg(0x30, 3, 1, ProtocolType::V2),
        ];
        let budget = BackfillBudget::new();
        let out = ensure_reserves(&bridge, &budget, 1, 103, NOW, &legs).await;
        assert_eq!(out, AdapterOutcome::Backfilled(1));
        // ONE aggregate3 for the single stale leg (3 legs batched ⇒ still 1 call).
        assert_eq!(bridge.calls(), 1);
        assert_eq!(
            bridge.subcall_counts()[0],
            1,
            "only the stale leg is fetched"
        );
        // Written back under the canonical key shape with the current stamp.
        assert_eq!(bridge.writes_v2().len(), 1);
        let (key, entry) = bridge.writes_v2()[0].clone();
        assert_eq!(key, pool_lower(addr(0x10)));
        assert_eq!(entry.r0, "111");
        assert_eq!(entry.r1, "222");
        assert_eq!(entry.blk, 103);
        assert!(entry.token0_addr.is_some());
    }

    #[tokio::test]
    async fn all_missing_legs_batched_into_one_call() {
        let bridge = FakeBridge::new();
        bridge.push_ok(vec![
            Some(word(1).into_iter().chain(word(2)).chain(word(0)).collect()),
            Some(word(3).into_iter().chain(word(4)).chain(word(0)).collect()),
            Some(word(5).into_iter().chain(word(6)).chain(word(0)).collect()),
        ]);
        let legs = vec![
            leg(0x10, 1, 2, ProtocolType::V2),
            leg(0x20, 2, 3, ProtocolType::V2),
            leg(0x30, 3, 1, ProtocolType::V2),
        ];
        let out = ensure_reserves(&bridge, &BackfillBudget::new(), 1, 103, NOW, &legs).await;
        assert_eq!(out, AdapterOutcome::Backfilled(3));
        assert_eq!(bridge.calls(), 1, "3 legs MUST be one multicall");
        assert_eq!(bridge.subcall_counts()[0], 3);
    }

    #[tokio::test]
    async fn mixed_v2_v3_legs_one_call_with_slot0_pairs() {
        let bridge = FakeBridge::new();
        // V2 leg fresh; V2 leg missing; V3 leg missing.
        bridge.seed_v2(addr(0x10), v2_entry(103));
        // Sub-calls in order: [V2 getReserves(0x20), V3 slot0(0x30), V3 liquidity(0x30)]
        let slot0_data: Vec<u8> = word(42)
            .into_iter()
            .chain(word(0))
            .chain(word(0))
            .chain(word(0))
            .chain(word(0))
            .chain(word(0))
            .chain(word(0))
            .collect();
        bridge.push_ok(vec![
            Some(word(7).into_iter().chain(word(8)).chain(word(0)).collect()),
            Some(slot0_data),
            Some(word(99)),
        ]);
        let legs = vec![
            leg(0x10, 1, 2, ProtocolType::V2),
            leg(0x20, 2, 3, ProtocolType::V2),
            leg(0x30, 3, 1, ProtocolType::V3),
        ];
        let out = ensure_reserves(&bridge, &BackfillBudget::new(), 1, 103, NOW, &legs).await;
        assert_eq!(out, AdapterOutcome::Backfilled(2));
        assert_eq!(bridge.calls(), 1, "mixed legs MUST share one multicall");
        assert_eq!(
            bridge.subcall_counts()[0],
            3,
            "V2→1 sub-call, V3→slot0+liquidity"
        );
        assert_eq!(bridge.writes_v3().len(), 1);
        assert_eq!(bridge.writes_v3()[0].0, pool_lower(addr(0x30)));
        assert_eq!(bridge.writes_v3()[0].1.sqrt_price_x96, "42");
        assert_eq!(bridge.writes_v3()[0].1.liquidity, "99");
        assert_eq!(bridge.writes_v3()[0].1.ts, NOW);
    }

    #[tokio::test]
    async fn unobtainable_leg_skips_with_missing_reserves() {
        let bridge = FakeBridge::new();
        // Leg 0 decodes fine; leg 1's sub-call reverted (success=false).
        bridge.push_ok(vec![
            Some(word(1).into_iter().chain(word(2)).chain(word(0)).collect()),
            None,
            Some(word(3).into_iter().chain(word(4)).chain(word(0)).collect()),
        ]);
        let legs = vec![
            leg(0x10, 1, 2, ProtocolType::V2),
            leg(0x20, 2, 3, ProtocolType::V2),
            leg(0x30, 3, 1, ProtocolType::V2),
        ];
        let out = ensure_reserves(&bridge, &BackfillBudget::new(), 1, 103, NOW, &legs).await;
        assert_eq!(
            out,
            AdapterOutcome::Skipped(Skipped::MissingReserves(addr(0x20))),
            "the reverted leg's pool is named"
        );
        // Partial real data is still written for the healthy legs.
        assert_eq!(bridge.writes_v2().len(), 2);
    }

    #[tokio::test]
    async fn whole_multicall_failure_skips_missing_reserves() {
        let bridge = FakeBridge::new();
        bridge.push_err("timeout_4000ms");
        let legs = vec![
            leg(0x10, 1, 2, ProtocolType::V2),
            leg(0x20, 2, 3, ProtocolType::V2),
            leg(0x30, 3, 1, ProtocolType::V2),
        ];
        let out = ensure_reserves(&bridge, &BackfillBudget::new(), 1, 103, NOW, &legs).await;
        assert_eq!(
            out,
            AdapterOutcome::Skipped(Skipped::MissingReserves(addr(0x10)))
        );
        assert!(
            bridge.writes_v2().is_empty(),
            "nothing written on total failure"
        );
    }

    #[tokio::test]
    async fn budget_exhausted_skips_without_rpc() {
        let bridge = FakeBridge::new();
        let legs = vec![
            leg(0x10, 1, 2, ProtocolType::V2),
            leg(0x20, 2, 3, ProtocolType::V2),
            leg(0x30, 3, 1, ProtocolType::V2),
        ];
        let budget = BackfillBudget::new();
        for _ in 0..MAX_BACKFILLS_PER_BLOCK {
            assert!(budget.claim(103));
        }
        let out = ensure_reserves(&bridge, &budget, 1, 103, NOW, &legs).await;
        assert_eq!(out, AdapterOutcome::Skipped(Skipped::BudgetExhausted));
        assert_eq!(bridge.calls(), 0, "no RPC past the allowance");
        assert_eq!(Skipped::BudgetExhausted.as_str(), "budget_exhausted");
    }

    #[tokio::test]
    async fn budget_resets_on_new_block() {
        let budget = BackfillBudget::new();
        for _ in 0..MAX_BACKFILLS_PER_BLOCK {
            assert!(budget.claim(100));
        }
        assert!(!budget.claim(100), "exhausted within the block");
        assert!(budget.claim(101), "next block resets the allowance");
    }

    #[test]
    fn freshness_bounds() {
        // V2: lag 5 fresh, lag 6 stale, unknown current block stale.
        assert!(v2_reserves_fresh(&v2_entry(98), 103));
        assert!(!v2_reserves_fresh(&v2_entry(97), 103));
        assert!(!v2_reserves_fresh(&v2_entry(103), 0));
        assert!(v2_reserves_fresh(&v2_entry(103), 103));
        // V3: age 30s fresh (TTL window), 31s stale, unknown clock stale.
        assert!(v3_slot0_fresh(&v3_entry(NOW - 30), NOW));
        assert!(!v3_slot0_fresh(&v3_entry(NOW - 31), NOW));
        assert!(!v3_slot0_fresh(&v3_entry(NOW), 0));
    }

    #[tokio::test]
    async fn unsupported_protocol_leg_skips_honestly() {
        let bridge = FakeBridge::new();
        let legs = vec![leg(0x10, 1, 2, ProtocolType::Curve)];
        let out = ensure_reserves(&bridge, &BackfillBudget::new(), 1, 103, NOW, &legs).await;
        assert_eq!(
            out,
            AdapterOutcome::Skipped(Skipped::MissingReserves(addr(0x10)))
        );
        assert_eq!(bridge.calls(), 0);
    }

    #[test]
    fn cycle_legs_projects_candidate_and_closes_cycle() {
        let c = RouteCandidate {
            chain_id: 1,
            route_hash: format!("0x{}", "ab".repeat(32)),
            route_kind: RouteKind::Triangular,
            tokens: vec![addr(1), addr(2), addr(3)],
            pools: vec![addr(0x10), addr(0x20), addr(0x30)],
            protocols: vec![ProtocolType::V2, ProtocolType::V3, ProtocolType::V2],
            fee_tiers: vec![Some(30), Some(500), Some(30)],
            directions: vec![
                RouteDirection::ZeroForOne,
                RouteDirection::ZeroForOne,
                RouteDirection::OneForZero,
            ],
            hops: 3,
            applicable_strategies: vec![],
            rejected_strategies: vec![],
            mode: "shadow".into(),
        };
        let legs = cycle_legs(&c).unwrap();
        assert_eq!(legs.len(), 3);
        assert_eq!(legs[0], leg(0x10, 1, 2, ProtocolType::V2));
        assert_eq!(legs[1], leg(0x20, 2, 3, ProtocolType::V3));
        // Closing leg returns to tokens[0].
        assert_eq!(legs[2], leg(0x30, 3, 1, ProtocolType::V2));

        // Ragged vectors → None (caller skips honestly).
        let mut broken = c.clone();
        broken.protocols.pop();
        assert!(cycle_legs(&broken).is_none());
    }

    #[test]
    fn gate_drops_only_matching_intents() {
        let h_skipped = H256::from_low_u64_be(1);
        let h_kept = H256::from_low_u64_be(2);
        let mk = |h: H256| {
            RouteIntent::new(
                1,
                h,
                Address::zero(),
                RouterKind::Unknown,
                Address::zero(),
                vec![RouteIntentLeg {
                    token_in: addr(1),
                    token_out: addr(2),
                    pool_hint: Some(addr(0x10)),
                    dex_hint: None,
                    fee_bps: None,
                    protocol_type: ProtocolType::V2,
                }],
                U256::zero(),
                None,
                SwapExactMode::ExactIn,
                DetectionSource::NewBlock,
            )
            .unwrap()
        };
        let (kept, dropped) = gate_dispatch_intents(vec![mk(h_skipped), mk(h_kept)], &[h_skipped]);
        assert_eq!(dropped, 1);
        assert_eq!(kept.len(), 1);
        assert_eq!(kept[0].tx_hash, h_kept);

        let (kept_none, dropped_none) = gate_dispatch_intents(vec![mk(h_kept)], &[]);
        assert_eq!(dropped_none, 0);
        assert_eq!(kept_none.len(), 1);
    }
}
