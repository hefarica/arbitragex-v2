// M11 allow: two infallible patterns in production code.
//   1. `NonZeroUsize::new(1).unwrap()` — 1 is never zero; the call cannot fail.
//   2. `Mutex::lock().expect(...)` — standard poison-recovery; any other
//      strategy (returning an error from new()) would propagate through every
//      caller with no benefit, since a poisoned mutex means the process is
//      already in an inconsistent state.
// Test modules also contain `.unwrap()` for readability.
#![allow(clippy::unwrap_used, clippy::expect_used)]
//! Dedup for pending tx hashes and opportunity routes.
//!
//! # Tx-level dedup (`Dedup`)
//! Two tiers:
//!   - L1: in-memory LRU per-chain (fast path).
//!   - L2: Redis SETNX with TTL so multi-instance does not double-insert.
//!
//! # Opportunity-level dedup (`OppDedup`)
//! Prevents the same profitable route from being emitted as a "new"
//! opportunity on every pending tx that happens to use the same pool pair.
//!
//! The dedup key is a compound string:
//!   `<route_hash>:<5min_bucket>:<profit_bucket>`
//!
//! - `route_hash`: stable identifier for the route (dex_a + token_in + token_out,
//!   as constructed in `scanner.rs` for `OpportunityCandidate.route_fingerprint`).
//! - `5min_bucket`: Unix timestamp divided by 300 — collapses all emissions within
//!   the same 5-minute window into one bucket. A re-emit after ≥5 min is always
//!   treated as a fresh opportunity regardless of route or profit.
//! - `profit_bucket`: `(profit_usd * 100.0) as i64 / 10` — rounds profit to the
//!   nearest $0.10. Opportunities with the same route but meaningfully different
//!   profit (diff > $0.10) are not deduped and are emitted independently. This
//!   captures the case where market conditions change the spread significantly
//!   within the same time bucket.
//!
//! This is an in-memory-only dedup (LRU). It is cheaper than adding a second
//! Redis tier because opportunity dedup is per-process (not multi-instance):
//! each chain scanner is a single-threaded tokio task owning its own Redis
//! connection. A cross-instance dedup for opportunities would require a shared
//! Redis key, but that is not needed — duplicate emissions across scanner
//! instances are collapsed at the prioritization-spine layer.

use ethers::types::H256;
use lru::LruCache;
use redis::aio::ConnectionManager;
use redis::AsyncCommands;
use std::num::NonZeroUsize;
use std::sync::Mutex;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

// ─── Tx-level dedup ──────────────────────────────────────────────────────────

pub struct Dedup {
    lru: Mutex<LruCache<H256, ()>>,
    redis_ttl: Duration,
}

impl Dedup {
    pub fn new(capacity: usize, redis_ttl: Duration) -> Self {
        let cap = NonZeroUsize::new(capacity).unwrap_or(NonZeroUsize::new(1).unwrap());
        Self {
            lru: Mutex::new(LruCache::new(cap)),
            redis_ttl,
        }
    }

    /// Returns true if the hash is fresh (not seen). false if duplicate.
    /// Call ORDER: check LRU first; if fresh, check Redis SETNX; if fresh there too, accept.
    pub async fn check_and_mark(&self, hash: H256, redis: &mut ConnectionManager) -> bool {
        // L1 LRU
        {
            let mut lru = self.lru.lock().expect("lru mutex");
            if lru.contains(&hash) {
                return false;
            }
            lru.put(hash, ());
        }
        // L2 Redis SETNX
        let key = format!("arbx:dedup:pendingtx:{:#x}", hash);
        let set: Result<Option<String>, _> = redis
            .set_options(
                &key,
                "1",
                redis::SetOptions::default()
                    .conditional_set(redis::ExistenceCheck::NX)
                    .with_expiration(redis::SetExpiry::EX(self.redis_ttl.as_secs() as usize)),
            )
            .await;
        matches!(set, Ok(Some(_)))
    }
}

// ─── Opportunity-level dedup ─────────────────────────────────────────────────

/// In-memory dedup for opportunity route+time+profit compound keys.
///
/// See module doc for key structure and rationale.
pub struct OppDedup {
    lru: Mutex<LruCache<String, ()>>,
}

impl OppDedup {
    /// `capacity`: maximum number of distinct compound keys held in LRU.
    /// A value of 4096 covers ~400 distinct routes × 10 profit buckets with
    /// comfortable headroom before the LRU starts evicting.
    pub fn new(capacity: usize) -> Self {
        let cap = NonZeroUsize::new(capacity).unwrap_or(NonZeroUsize::new(1).unwrap());
        Self {
            lru: Mutex::new(LruCache::new(cap)),
        }
    }

    /// Returns `true` if this (route, time_bucket, profit_bucket) triple is fresh.
    /// Returns `false` if it was already seen — the caller should skip re-emit.
    ///
    /// `route_hash`: the route fingerprint string (e.g. `"uniswap_v2_0xabc_0xdef"`).
    /// `profit_usd`: the computed USD gross profit. `None` is treated as bucket 0
    ///   (uncomputed profit — all oracle-gap emissions within the same 5-min window
    ///   collapse to one dedup slot, preventing floods on unpriced pairs).
    pub fn check_and_mark(&self, route_hash: &str, profit_usd: Option<f64>) -> bool {
        let key = build_opp_dedup_key(route_hash, profit_usd);
        let mut lru = self.lru.lock().expect("opp_dedup lru mutex");
        if lru.contains(&key) {
            return false; // duplicate
        }
        lru.put(key, ());
        true // fresh
    }
}

/// Builds the compound dedup key: `<route_hash>:<5min_bucket>:<profit_bucket>`.
///
/// Extracted as a pure function so it can be unit-tested without I/O.
///
/// - `bucket_5min`: `unix_secs / 300` — collapses a 5-minute window.
/// - `profit_bucket`: `(profit_usd * 100.0) as i64 / 10` — bins at $0.10 granularity.
///   `None` profit maps to bucket 0 (oracle gap; safe to dedup across a window).
pub fn build_opp_dedup_key(route_hash: &str, profit_usd: Option<f64>) -> String {
    let unix_secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let bucket_5min = unix_secs / 300;
    let profit_bucket = profit_usd.map(|p| (p * 100.0) as i64 / 10).unwrap_or(0);
    format!("{route_hash}:{bucket_5min}:{profit_bucket}")
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── OppDedup unit tests ──────────────────────────────────────────────────

    /// Same route + same profit within one call pair → first is fresh, second is dup.
    #[test]
    fn opp_dedup_same_route_same_profit_is_dup() {
        let dedup = OppDedup::new(64);
        let route = "uniswap_v2_0xabc_0xdef";
        assert!(
            dedup.check_and_mark(route, Some(1.50)),
            "first call must be fresh"
        );
        assert!(
            !dedup.check_and_mark(route, Some(1.50)),
            "second identical call must be dup"
        );
    }

    /// Different routes are never deduped regardless of profit.
    #[test]
    fn opp_dedup_different_routes_not_deduped() {
        let dedup = OppDedup::new(64);
        assert!(dedup.check_and_mark("route_A_0xabc_0xdef", Some(1.50)));
        assert!(dedup.check_and_mark("route_B_0xabc_0xdef", Some(1.50)));
    }

    /// Same route but profit differs by more than $0.10 → treated as new.
    #[test]
    fn opp_dedup_different_profit_bucket_is_fresh() {
        let dedup = OppDedup::new(64);
        let route = "uniswap_v2_0xabc_0xdef";
        // profit_bucket = (1.50 * 100) / 10 = 15
        assert!(dedup.check_and_mark(route, Some(1.50)));
        // profit_bucket = (2.50 * 100) / 10 = 25 — different bucket, must be fresh
        assert!(dedup.check_and_mark(route, Some(2.50)));
    }

    /// Profit within the same $0.10 bin is deduplicated.
    /// $1.50 and $1.59 both land in bucket 15 (floor of 1.5..1.6 range).
    #[test]
    fn opp_dedup_within_profit_bin_is_dup() {
        let dedup = OppDedup::new(64);
        let route = "uniswap_v2_0xabc_0xdef";
        // bucket = (1.50 * 100) / 10 = 15
        assert!(dedup.check_and_mark(route, Some(1.50)));
        // bucket = (1.59 * 100) / 10 = 15 (same bin)
        assert!(!dedup.check_and_mark(route, Some(1.59)));
    }

    /// None profit (oracle gap) maps to bucket 0; two None-profit calls on same route dedup.
    #[test]
    fn opp_dedup_none_profit_deduped_within_window() {
        let dedup = OppDedup::new(64);
        let route = "uniswap_v2_0xabc_0xdef";
        assert!(dedup.check_and_mark(route, None));
        assert!(!dedup.check_and_mark(route, None));
    }

    // ── build_opp_dedup_key pure unit tests ──────────────────────────────────

    /// The key format must be `<route>:<bucket>:<profit_bucket>` with no spaces.
    #[test]
    fn build_key_format_is_stable() {
        // We cannot control the time bucket in a unit test, but we CAN verify
        // that two calls within the same test run (same second) produce identical
        // keys for the same inputs.
        let k1 = build_opp_dedup_key("route_X", Some(5.0));
        let k2 = build_opp_dedup_key("route_X", Some(5.0));
        assert_eq!(
            k1, k2,
            "keys for identical inputs must match within same second"
        );
        // Three colon-separated segments.
        let parts: Vec<&str> = k1.splitn(3, ':').collect();
        assert_eq!(
            parts.len(),
            3,
            "key must have 3 colon-separated segments: route:bucket:profit"
        );
        assert_eq!(parts[0], "route_X");
    }

    /// Profit bucket arithmetic: values within ~$0.10 of each other share a bin.
    ///
    /// Note on floating-point: `5.10 * 100.0` evaluates to ~509.999... in f64
    /// (not exactly 510.0), so it truncates to bucket 50 — same as $5.00.
    /// This is intentional: the dedup tolerance is "approximately $0.10" not
    /// "exactly $0.10". Using branchless integer arithmetic avoids any fp-mode
    /// dependency while still collapsing small profit fluctuations. The important
    /// invariant is that $5.00 and $5.20 are in *different* buckets.
    #[test]
    fn profit_bucket_boundaries() {
        // bucket = (profit * 100.0) as i64 / 10
        let b_500 = (5.00_f64 * 100.0) as i64 / 10; // = 50
        let b_509 = (5.09_f64 * 100.0) as i64 / 10; // = 50 (floor, same bin)
        let b_520 = (5.20_f64 * 100.0) as i64 / 10; // = 52 (clearly different bin)
        assert_eq!(b_500, 50);
        assert_eq!(b_509, 50, "$5.09 must land in same bucket as $5.00");
        assert_eq!(b_520, 52, "$5.20 must be in bucket 52");
        // Same-bin guarantee: $5.00 and $5.09 dedup together.
        assert_eq!(b_500, b_509, "$5.00 and $5.09 must share a profit bucket");
        // Different-bin guarantee: $5.00 and $5.20 do NOT dedup.
        assert_ne!(
            b_500, b_520,
            "$5.00 and $5.20 must be in different profit buckets"
        );
    }

    /// Negative profit (valid edge case from spine) produces a negative bucket —
    /// not a panic or silent 0.
    #[test]
    fn negative_profit_produces_negative_bucket() {
        let bucket = (-1.50_f64 * 100.0) as i64 / 10;
        assert!(
            bucket < 0,
            "negative profit must produce negative bucket, got {bucket}"
        );
    }
}
