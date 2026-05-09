//! GasOracleWorker — periodic on-chain gas price fetch that stamps
//! `arbx:gas_price_ts:<chain_id>` in Redis so the Pre-Execute Checklist
//! Check 6 (`gas_price_fresh`) can confirm the oracle is alive.
//!
//! ## Purpose (liveness signal, not a data feed)
//!
//! Check 6 semantics are: "is the gas oracle still alive and writing?"
//! A fresh timestamp means the worker successfully called `eth_gasPrice`
//! against a live RPC node in the last `GAS_PRICE_MAX_AGE_SECS` (30s,
//! defined in `shared_rs::pre_execute_checklist`).
//!
//! The gas price value itself is currently NOT the authoritative input for
//! cost calculations (the spine uses `NetworkSignals` / operator config for
//! that). The worker exists exclusively so Check 6 stops blocking broadcasts
//! when `paper_mode=false`.
//!
//! ## Failure semantics (R8 fail-honest)
//!
//! - RPC error → log warn, do NOT write timestamp → key stays stale → Check 6
//!   blocks broadcast (fail-closed). Correct behaviour: stale oracle = unsafe.
//! - Redis SET error → log warn, silent drop — does not crash the worker.
//!   The key was not refreshed; on the next tick the worker will try again.
//! - No RPC pool → worker idles with a warn log; Check 6 blocks broadcast.
//!
//! ## TTL design
//!
//! TTL is `GAS_TS_TTL_SECS` (60s = 6× default 10s interval).
//! Check 6 blocks at >30s staleness. TTL=60s ensures:
//!   - If the worker misses ONE tick → key is still fresh at 10–20s.
//!   - If the worker DIES → key expires at 60s → Check 6 blocks at 30s.
//!     There is a 30s window where Check 6 is still passing after the worker
//!     dies, which is acceptable: the last known gas price was fetched <60s ago.
//!
//! ## Key name
//!
//! Written via `shared_rs::pre_execute_checklist::gas_price_ts_key(chain_id)`,
//! the SAME helper that Check 6 reads. Single source of truth for the key name.

use alloy::providers::Provider as AlloyProvider;
use redis::aio::ConnectionManager;
use redis::AsyncCommands;
use shared_rs::pre_execute_checklist::gas_price_ts_key;
use shared_rs::rpc_failover::HttpRpcPool;
use std::sync::Arc;
use std::time::Duration;
use tokio::time::interval;
use tracing::{info, warn};

/// Default fetch interval in milliseconds. At 10s we call `eth_gasPrice`
/// once per 10s (~8,640 calls/day) — well within any RPC tier's budget.
/// Override via `GAS_ORACLE_INTERVAL_MS` env var.
pub const DEFAULT_INTERVAL_MS: u64 = 10_000;

/// Redis TTL for the freshness key. Set to 2× the minimum Check 6 staleness
/// window (30s) so a single missed tick does NOT expire the key before the
/// next write. At the default 10s interval this equals ~6× the interval.
const GAS_TS_TTL_SECS: u64 = 60;

pub struct GasOracleWorker {
    interval_ms: u64,
    chain_id: u64,
}

impl GasOracleWorker {
    pub fn new(interval_ms: u64, chain_id: u64) -> Self {
        Self {
            interval_ms: interval_ms.max(1),
            chain_id,
        }
    }

    /// Run forever — never returns. Caller must `tokio::spawn` this.
    pub async fn run(self, rpc_pool: Arc<HttpRpcPool>, mut redis: ConnectionManager) {
        info!(
            event = "gas_oracle.boot",
            chain_id = self.chain_id,
            interval_ms = self.interval_ms,
            ttl_secs = GAS_TS_TTL_SECS,
            "gas oracle worker started (CODE-3 — Check 6 liveness writer)"
        );

        let mut ticker = interval(Duration::from_millis(self.interval_ms));
        // Drain the first immediate tick to align with the RPC-pool warm-up.
        ticker.tick().await;

        loop {
            ticker.tick().await;
            self.run_one_tick(&rpc_pool, &mut redis).await;
        }
    }

    /// One fetch + stamp cycle. Infallible: all errors are logged + swallowed
    /// so the worker loop never exits due to a transient RPC or Redis failure.
    async fn run_one_tick(&self, rpc_pool: &Arc<HttpRpcPool>, redis: &mut ConnectionManager) {
        // Pick the best available provider (Healthy > Degraded, round-robin
        // tie-break via the pool's own circuit-breaker machinery).
        let entry = match rpc_pool.pick() {
            Ok(e) => e,
            Err(e) => {
                warn!(
                    event = "gas_oracle.no_provider",
                    chain_id = self.chain_id,
                    error = %e,
                    "all RPC providers unhealthy — gas_price_ts NOT written (Check 6 will block)"
                );
                return;
            }
        };

        // eth_gasPrice — alloy Provider trait. Returns gas price in wei (u128).
        let gas_price_wei: u128 = match entry.provider.get_gas_price().await {
            Ok(p) => p,
            Err(e) => {
                warn!(
                    event = "gas_oracle.fetch_failed",
                    chain_id = self.chain_id,
                    rpc = %entry.name,
                    error = %e,
                    "eth_gasPrice failed — gas_price_ts NOT written (Check 6 will block)"
                );
                return;
            }
        };

        // Compute timestamp AFTER successful fetch — semantics are "oracle is
        // alive and produced a value at this moment", not "we attempted a call".
        let now_ts: u64 = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        let ts_key = gas_price_ts_key(self.chain_id);

        // Write the freshness timestamp. TTL 60s — covers ~6× the default fetch
        // interval so a single missed tick never causes an unexpected Check 6 block.
        let write_result: redis::RedisResult<()> = redis
            .set_ex(&ts_key, now_ts.to_string(), GAS_TS_TTL_SECS)
            .await;

        match write_result {
            Ok(()) => {
                info!(
                    event = "gas_oracle.tick_ok",
                    chain_id = self.chain_id,
                    gas_price_gwei = gas_price_wei as f64 / 1e9,
                    ts_key = %ts_key,
                    now_ts,
                    "gas_price_ts stamped (Check 6 will pass)"
                );
            }
            Err(e) => {
                // R8 fail-honest: Redis write error → silently swallow.
                // The key was NOT refreshed; Check 6 will see the previous
                // (still fresh if <30s old) or absent key → blocks conservatively.
                warn!(
                    event = "gas_oracle.redis_write_failed",
                    chain_id = self.chain_id,
                    error = %e,
                    "Redis SET failed — gas_price_ts NOT refreshed this tick"
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Pure unit tests — no Redis, no RPC. Verify constants and constructor.

    #[test]
    fn default_interval_ms_is_positive() {
        assert!(DEFAULT_INTERVAL_MS > 0);
    }

    #[test]
    fn ttl_is_at_least_twice_check6_max_age() {
        // Check 6 blocks at GAS_PRICE_MAX_AGE_SECS=30. TTL must be strictly
        // greater than that so a missed tick does not expire the key before
        // Check 6 would catch it. Current value is 60s ≥ 2×30 ✓.
        let check6_max_age_secs: u64 = 30;
        assert!(
            GAS_TS_TTL_SECS >= 2 * check6_max_age_secs,
            "TTL {GAS_TS_TTL_SECS}s must be ≥ 2×{check6_max_age_secs}s to safely cover a missed tick"
        );
    }

    #[test]
    fn constructor_clamps_zero_interval_to_one() {
        let w = GasOracleWorker::new(0, 1);
        assert_eq!(w.interval_ms, 1, "interval_ms must be clamped to ≥1 ms");
    }

    #[test]
    fn gas_price_ts_key_matches_check6_reader() {
        // The key written here MUST exactly match what check_gas_price_fresh reads.
        // gas_price_ts_key is imported from shared_rs — same function both sides,
        // so this test documents the contract.
        let key = gas_price_ts_key(1);
        assert_eq!(key, "arbx:gas_price_ts:1");

        let key_arb = gas_price_ts_key(42161);
        assert_eq!(key_arb, "arbx:gas_price_ts:42161");
    }
}
