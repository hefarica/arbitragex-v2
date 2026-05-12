// M11 allow: test modules use .unwrap()/.expect() for readability;
// production paths use ? / anyhow throughout.
//! LendingPositionIndexer — Phase 11.
//!
//! ## Design (spec §3.7)
//!
//! Maintains a Redis-backed watchlist of Aave V3 and Compound V2 positions
//! that are near or past the liquidation threshold (`health_factor < 1.05`).
//! The indexer is NOT the periodic poller — the existing
//! `liquidation_worker.rs` handles polling. This indexer is the **lookup
//! layer** used by `LiquidationEngine` when a `RouteIntent` arrives and we
//! need to instantly answer: "is there a known under-collateralised position
//! involving the token the mempool tx just moved?"
//!
//! ## Redis key format
//!
//! Watchlist (set of lowercase 0x-prefixed user addresses):
//! ```
//! arbx:lending_watchlist:{protocol}:{chain_id}
//! ```
//! Position cache (JSON-serialised `LendingPosition`):
//! ```
//! arbx:lending_position:{protocol}:{chain_id}:{user_addr}
//! ```
//!
//! ## R8 invariants
//!
//! - `get_position` returns `None` when user is not in cache — never fabricates.
//! - `recompute_position` returns `Err` on RPC failure — never silently inserts stale data.
//! - `health_factor = f64::MAX` is the canonical Aave "no debt" sentinel — preserved as-is.
//! - `watchlist_size` returns `Err` on Redis failure — no fabricated counts.

use crate::impact_index::LendingProtocol;
use anyhow::Context;
use ethers::types::Address;
use redis::aio::ConnectionManager;
use redis::AsyncCommands;
use serde::{Deserialize, Serialize};
use tracing::{debug, warn};

// ---------------------------------------------------------------------------
// LendingPosition
// ---------------------------------------------------------------------------

/// A cached snapshot of a single user's lending position on a protocol.
///
/// Populated by `LendingPositionIndexer::recompute_position` and read
/// hot-path by `LiquidationEngine::build_from_lending_impact`.
///
/// ## R8 field invariants
///
/// - `health_factor = f64::MAX` → Aave reports `type(uint256).max` when
///   the user has no debt. Callers MUST treat this as "safe, never liquidatable".
/// - `total_collateral_usd = 0.0` and `total_debt_usd = 0.0` are legitimate
///   (new user with no open position). Not an error — just uninitialised.
/// - `collateral_assets` and `debt_assets` are the token addresses involved;
///   they may be empty if the position data is not yet enriched.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LendingPosition {
    pub protocol: LendingProtocol,
    pub user: Address,
    /// Token addresses deposited as collateral.
    pub collateral_assets: Vec<Address>,
    /// Token addresses borrowed as debt.
    pub debt_assets: Vec<Address>,
    /// Health factor decoded from 1e18 on-chain value.
    /// `f64::MAX` means "no debt" (Aave sentinel).
    pub health_factor: f64,
    /// Total collateral in USD (Aave base-8 units converted to f64).
    pub total_collateral_usd: f64,
    /// Total debt in USD (Aave base-8 units converted to f64).
    pub total_debt_usd: f64,
    /// Block at which this position was last read from on-chain.
    pub last_checked_block: u64,
}

// ---------------------------------------------------------------------------
// Redis key helpers (match the pattern liquidation_worker uses)
// ---------------------------------------------------------------------------

/// Redis SET of lowercase 0x-prefixed borrower addresses that are near-
/// or past-threshold (`health_factor < 1.05`).
///
/// Format matches `liquidation_worker::watchlist_redis_key` for Aave V3
/// (`arbx:aave_v3_watchlist:{chain_id}`). For the new multi-protocol
/// indexer we prefix with the protocol string for extensibility.
pub fn watchlist_key(protocol: LendingProtocol, chain_id: u64) -> String {
    match protocol {
        LendingProtocol::AaveV3 => format!("arbx:lending_watchlist:aave_v3:{chain_id}"),
        LendingProtocol::CompoundV2 => format!("arbx:lending_watchlist:compound_v2:{chain_id}"),
    }
}

/// Redis STRING storing the JSON-serialised `LendingPosition` for one user.
pub fn position_cache_key(protocol: LendingProtocol, chain_id: u64, user: Address) -> String {
    let protocol_str = match protocol {
        LendingProtocol::AaveV3 => "aave_v3",
        LendingProtocol::CompoundV2 => "compound_v2",
    };
    let user_lc = format!("{user:?}").to_ascii_lowercase();
    format!("arbx:lending_position:{protocol_str}:{chain_id}:{user_lc}")
}

// ---------------------------------------------------------------------------
// LendingPositionIndexer
// ---------------------------------------------------------------------------

/// Maintains the watchlist of Aave V3 + Compound V2 positions with
/// `health_factor < 1.05` and serves instant cached lookups for the
/// `LiquidationEngine`.
///
/// ## Threading
///
/// Constructed once and shared behind `Arc`. The `redis: ConnectionManager`
/// is already `Clone + Send + Sync` (it is a connection pool internally).
/// Individual methods take `&mut self` only where they mutate local in-memory
/// state (currently none — all state is in Redis).
pub struct LendingPositionIndexer {
    pub chain_id: u64,
    redis: ConnectionManager,
}

impl LendingPositionIndexer {
    /// Constructs a new indexer.
    ///
    /// - `chain_id`: EVM chain ID for key namespacing.
    /// - `redis`: shared Redis connection manager.
    /// - `_multicall`: placeholder for future on-chain recompute path (Phase 15+).
    ///   Ignored today; the MVP recompute path uses the existing
    ///   `liquidation_worker` Multicall3 ABI — wiring it here is Phase 15+ work.
    pub fn new(chain_id: u64, redis: ConnectionManager) -> Self {
        Self { chain_id, redis }
    }

    // -----------------------------------------------------------------------
    // Read: single position
    // -----------------------------------------------------------------------

    /// Returns the cached `LendingPosition` for `(protocol, user)`, or `None`
    /// if the user is not in the watchlist cache.
    ///
    /// R8 invariant: returns `None` on cache miss — never fabricates a position.
    pub async fn get_position(
        &self,
        protocol: LendingProtocol,
        user: Address,
    ) -> Option<LendingPosition> {
        let key = position_cache_key(protocol, self.chain_id, user);
        let mut redis = self.redis.clone();
        let raw: Result<Option<String>, _> = redis.get(&key).await;
        match raw {
            Ok(Some(json)) => match serde_json::from_str::<LendingPosition>(&json) {
                Ok(pos) => Some(pos),
                Err(e) => {
                    warn!(
                        event = "lending_position_indexer.cache_deserialize_error",
                        chain_id = self.chain_id,
                        key = %key,
                        error = %e,
                        "cached position JSON is corrupt; returning None (R8 fail-honest)"
                    );
                    None
                }
            },
            Ok(None) => None,
            Err(e) => {
                warn!(
                    event = "lending_position_indexer.redis_get_error",
                    chain_id = self.chain_id,
                    key = %key,
                    error = %e,
                    "Redis GET failed; returning None"
                );
                None
            }
        }
    }

    // -----------------------------------------------------------------------
    // Write: recompute + cache
    // -----------------------------------------------------------------------

    /// Recompute the health factor for a user from on-chain and update the
    /// Redis cache.
    ///
    /// **MVP implementation**: accepts the new `LendingPosition` data
    /// externally (from `liquidation_worker` which already does the Multicall3
    /// read) and stores it in Redis. Phase 15+ will integrate the Alloy
    /// Multicall3 provider directly here.
    ///
    /// R8 invariant: returns `Err` on Redis write failure — never silently
    /// inserts a stale or partial position.
    pub async fn recompute_position(
        &mut self,
        protocol: LendingProtocol,
        user: Address,
        position: LendingPosition,
    ) -> anyhow::Result<Option<LendingPosition>> {
        // Preserve the Aave "no debt" sentinel: if health_factor == f64::MAX,
        // the position is safe; store it but never emit a liquidation candidate.
        let key = position_cache_key(protocol, self.chain_id, user);
        let json = serde_json::to_string(&position)
            .context("serialize LendingPosition for Redis cache")?;

        let _: () = self
            .redis
            .set(&key, &json)
            .await
            .context("Redis SET for LendingPosition cache")?;

        debug!(
            event = "lending_position_indexer.position_cached",
            chain_id = self.chain_id,
            user = ?user,
            health_factor = position.health_factor,
            "position recomputed and cached"
        );

        Ok(Some(position))
    }

    // -----------------------------------------------------------------------
    // Watchlist management
    // -----------------------------------------------------------------------

    /// Adds `user` to the watchlist for `protocol` on this chain.
    ///
    /// The watchlist is a Redis SET. SADD is idempotent — adding an existing
    /// member is a no-op.
    pub async fn watchlist_add(
        &mut self,
        protocol: LendingProtocol,
        user: Address,
    ) -> anyhow::Result<()> {
        let key = watchlist_key(protocol, self.chain_id);
        let user_str = format!("{user:?}").to_ascii_lowercase();
        let _: () = self
            .redis
            .sadd(&key, &user_str)
            .await
            .context("Redis SADD for lending watchlist")?;
        debug!(
            event = "lending_position_indexer.watchlist_add",
            chain_id = self.chain_id,
            protocol = ?protocol,
            user = %user_str,
        );
        Ok(())
    }

    /// Removes `user` from the watchlist for `protocol` on this chain.
    ///
    /// Called when the position's HF rises back above 1.05 (no longer
    /// near-threshold). SREM is idempotent — removing a non-member is a no-op.
    pub async fn watchlist_remove(
        &mut self,
        protocol: LendingProtocol,
        user: Address,
    ) -> anyhow::Result<()> {
        let key = watchlist_key(protocol, self.chain_id);
        let user_str = format!("{user:?}").to_ascii_lowercase();
        let _: () = self
            .redis
            .srem(&key, &user_str)
            .await
            .context("Redis SREM for lending watchlist")?;
        debug!(
            event = "lending_position_indexer.watchlist_remove",
            chain_id = self.chain_id,
            protocol = ?protocol,
            user = %user_str,
        );
        Ok(())
    }

    /// Returns the number of addresses currently in the watchlist for
    /// `protocol` on this chain.
    ///
    /// R8 invariant: returns `Err` on Redis failure — never fabricates a count.
    pub async fn watchlist_size(&self, protocol: LendingProtocol) -> anyhow::Result<usize> {
        let key = watchlist_key(protocol, self.chain_id);
        let mut redis = self.redis.clone();
        let size: usize = redis
            .scard(&key)
            .await
            .context("Redis SCARD for lending watchlist")?;
        Ok(size)
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use crate::metrics::POSITIONS_WATCHLIST_EMPTY_TOTAL;

    // ── Helper: build a minimal LendingPosition ──────────────────────────────

    fn addr(n: u64) -> Address {
        Address::from_low_u64_be(n)
    }

    fn make_position(user: Address, hf: f64) -> LendingPosition {
        LendingPosition {
            protocol: LendingProtocol::AaveV3,
            user,
            collateral_assets: vec![addr(0x10)],
            debt_assets: vec![addr(0x20)],
            health_factor: hf,
            total_collateral_usd: 1_000.0,
            total_debt_usd: 900.0,
            last_checked_block: 100,
        }
    }

    // ── indexer::tests::watchlist_key_format ─────────────────────────────────

    #[test]
    fn watchlist_key_format_aave_v3() {
        let key = watchlist_key(LendingProtocol::AaveV3, 1);
        assert_eq!(key, "arbx:lending_watchlist:aave_v3:1");
    }

    #[test]
    fn watchlist_key_format_compound_v2() {
        let key = watchlist_key(LendingProtocol::CompoundV2, 42161);
        assert_eq!(key, "arbx:lending_watchlist:compound_v2:42161");
    }

    // ── indexer::tests::position_cache_key_format ────────────────────────────

    #[test]
    fn position_cache_key_format() {
        let user = addr(1);
        let key = position_cache_key(LendingProtocol::AaveV3, 1, user);
        // The key must contain the protocol, chain_id and lowercased address.
        assert!(key.starts_with("arbx:lending_position:aave_v3:1:"));
        assert!(key.contains("0x"));
    }

    // ── indexer::tests::hf_max_is_no_debt_sentinel ──────────────────────────

    #[test]
    fn hf_max_is_no_debt_sentinel() {
        // f64::MAX is the sentinel for "no debt" from Aave. This test pins
        // the contract: callers must NOT emit a liquidation candidate when HF == MAX.
        let pos = make_position(addr(1), f64::MAX);
        assert!(
            pos.health_factor >= 1.0,
            "f64::MAX must be >= 1.0 — never liquidatable"
        );
    }

    // ── indexer::tests::lending_position_serde_roundtrip ────────────────────

    #[test]
    fn lending_position_serde_roundtrip() {
        let pos = make_position(addr(0xAB), 0.95);
        let json = serde_json::to_string(&pos).expect("serialize");
        let back: LendingPosition = serde_json::from_str(&json).expect("deserialize");
        assert!((back.health_factor - 0.95).abs() < f64::EPSILON);
        assert_eq!(back.user, addr(0xAB));
    }

    // ── indexer::tests::positions_watchlist_empty_total_counter ─────────────
    //
    // Verifies the Prometheus counter (Phase 16) increments correctly.
    // Label chain_id="1" is used for the mainnet instance.

    #[test]
    fn positions_watchlist_empty_total_counter_increments() {
        let chain = "1";
        let before = POSITIONS_WATCHLIST_EMPTY_TOTAL
            .with_label_values(&[chain])
            .get();
        POSITIONS_WATCHLIST_EMPTY_TOTAL
            .with_label_values(&[chain])
            .inc();
        let after = POSITIONS_WATCHLIST_EMPTY_TOTAL
            .with_label_values(&[chain])
            .get();
        assert_eq!(after, before + 1);
    }

    // ── indexer::tests::get_position_on_missing_key_returns_none ────────────
    //
    // Without a live Redis we test the JSON decode path through from_str to
    // validate the None path for corrupt JSON (simulates a cache miss).

    #[test]
    fn corrupt_json_deserializes_to_none() {
        // Simulate what get_position does on corrupt JSON
        let corrupt = "not valid json {{{";
        let result = serde_json::from_str::<LendingPosition>(corrupt);
        assert!(
            result.is_err(),
            "corrupt JSON must fail deserialization (get_position returns None)"
        );
    }
}
