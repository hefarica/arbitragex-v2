//! Strategy-scores database query layer (Sprint B — Component 4).
//!
//! Fetches the most-recent `strategy_scores` row per `(strategy_kind, chain_id)`
//! and surfaces the `revert_rate` column as `p_fail` for the failure-risk buffer.
//!
//! ## Caching
//! Results are cached in an [`lru::LruCache`] keyed by `(strategy_kind, chain_id)`
//! with a per-entry 60-second TTL.  The cache is accessed behind a `tokio::sync::Mutex`
//! so callers can share a single `StrategyScoresCache` instance across async tasks.
//!
//! ## R8 (fail-honest) contract
//! - Returns `None` when no row exists for the requested key.
//! - Returns `None` when the most-recent row has `sample_count < 10`
//!   (insufficient statistical confidence).
//! - Never invents or interpolates a value.

use lru::LruCache;
use sqlx::PgPool;
use std::num::NonZeroUsize;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::Mutex;
use tracing::{debug, warn};

/// Minimum number of samples required before trusting `revert_rate`.
/// Below this threshold we have insufficient data; fall back to proxy.
const MIN_SAMPLE_COUNT: i64 = 10;

/// How long a cached row stays valid before a fresh query is made.
const CACHE_TTL: Duration = Duration::from_secs(60);

/// Maximum number of `(strategy_kind, chain_id)` keys to keep in the LRU cache.
/// 256 covers all realistic strategy × chain combinations with comfortable headroom.
const CACHE_CAPACITY: usize = 256;

/// The statistical information returned for a strategy window.
///
/// `p_fail` is the `revert_rate` from `strategy_scores` expressed as a fraction
/// in `[0.0, 1.0]`.  The `sample_count` is surfaced so callers can include it in
/// `PFailSource::Statistical { sample_count }`.
#[derive(Debug, Clone)]
pub struct StrategyFailRate {
    /// Revert probability from `strategy_scores.revert_rate` (0.0–1.0 scale).
    /// The column stores NUMERIC(10,6) — fits losslessly in f64.
    pub p_fail: f64,
    /// Window sample count — used by dashboard to display confidence.
    pub sample_count: i64,
}

/// Cache entry wrapping a query result with its fetch timestamp.
#[derive(Debug, Clone)]
struct CacheEntry {
    value: Option<StrategyFailRate>,
    fetched_at: Instant,
}

impl CacheEntry {
    fn is_fresh(&self) -> bool {
        self.fetched_at.elapsed() < CACHE_TTL
    }
}

/// Thread-safe, TTL-evicting LRU cache for `strategy_scores` lookups.
///
/// Intended usage: create one instance at startup and share via `Arc<StrategyScoresCache>`.
/// The inner `Mutex` is a `tokio::sync::Mutex` so it integrates naturally with
/// async callers without spawning a blocking thread.
pub struct StrategyScoresCache {
    inner: Mutex<LruCache<(String, i32), CacheEntry>>,
}

impl StrategyScoresCache {
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            inner: Mutex::new(LruCache::new(
                NonZeroUsize::new(CACHE_CAPACITY).expect("CACHE_CAPACITY > 0"),
            )),
        })
    }

    /// Return the latest statistical failure rate for `(strategy_kind, chain_id)`.
    ///
    /// - Serves from cache when the cached entry is `< 60s` old.
    /// - Falls through to PostgreSQL on cache miss or TTL expiry.
    /// - Returns `None` (R8) when:
    ///   - No row exists in `strategy_scores` for the requested key.
    ///   - The most recent row has `sample_count < 10`.
    ///   - The database query fails (warns via tracing, never panics).
    pub async fn get(
        &self,
        pool: &PgPool,
        strategy_kind: &str,
        chain_id: i32,
    ) -> Option<StrategyFailRate> {
        let key = (strategy_kind.to_string(), chain_id);

        // --- Cache read ---
        {
            let mut guard = self.inner.lock().await;
            if let Some(entry) = guard.get(&key) {
                if entry.is_fresh() {
                    debug!(
                        event = "strategy_scores_cache.hit",
                        strategy_kind, chain_id, "strategy_scores cache hit"
                    );
                    return entry.value.clone();
                }
            }
        }

        // --- Cache miss / TTL expired: query PostgreSQL ---
        let result = query_latest_revert_rate(pool, strategy_kind, chain_id).await;

        // Store in cache regardless of hit/miss (None = "no data" is cacheable too,
        // avoids hammering PG on every tick for a new strategy).
        {
            let mut guard = self.inner.lock().await;
            guard.put(
                key,
                CacheEntry {
                    value: result.clone(),
                    fetched_at: Instant::now(),
                },
            );
        }

        result
    }
}

/// Raw row returned by the strategy_scores query.
/// We use `query_as` (runtime query) rather than `query!` (compile-time)
/// because there is no `DATABASE_URL` available at build time in this
/// workspace — the connection string is injected at runtime via env/config.
#[derive(sqlx::FromRow)]
struct StrategyScoresRow {
    /// `revert_rate::DOUBLE PRECISION` — NUMERIC(10,6) cast to f64 in SQL.
    revert_rate: Option<f64>,
    /// `sample_count` INTEGER cast to i64 for `PFailSource` reporting.
    sample_count: i64,
}

/// Execute the SQL query and return the validated fail rate, or `None`.
///
/// SQL contract:
/// ```sql
/// SELECT revert_rate::DOUBLE PRECISION AS revert_rate, sample_count
/// FROM strategy_scores
/// WHERE strategy_kind = $1
///   AND chain_id      = $2
///   AND sample_count  >= 10
/// ORDER BY window_end DESC
/// LIMIT 1
/// ```
///
/// `sample_count >= 10` is enforced in the WHERE clause so the DB uses the
/// existing index `idx_strat_recent (chain_id, strategy_kind, window_end DESC)`.
async fn query_latest_revert_rate(
    pool: &PgPool,
    strategy_kind: &str,
    chain_id: i32,
) -> Option<StrategyFailRate> {
    // NUMERIC(10,6) is cast to DOUBLE PRECISION in SQL for a plain f64.
    // 6 decimal places fit in f64 with no precision loss.
    let result: Result<Option<StrategyScoresRow>, sqlx::Error> = sqlx::query_as(
        r#"
        SELECT
            revert_rate::DOUBLE PRECISION AS revert_rate,
            sample_count
        FROM strategy_scores
        WHERE strategy_kind = $1
          AND chain_id      = $2
          AND sample_count  >= $3
        ORDER BY window_end DESC
        LIMIT 1
        "#,
    )
    .bind(strategy_kind)
    .bind(chain_id)
    .bind(MIN_SAMPLE_COUNT)
    .fetch_optional(pool)
    .await;

    match result {
        Ok(Some(r)) => {
            let p = match r.revert_rate {
                Some(v) => v,
                None => {
                    // NULL revert_rate in a row that passed sample_count >= 10 —
                    // unusual but possible if the learning loop hasn't written it yet.
                    debug!(
                        event = "strategy_scores_db.null_revert_rate",
                        strategy_kind, chain_id, "revert_rate is NULL — proxy fallback"
                    );
                    return None;
                }
            };
            // Guard: revert_rate must be a valid probability [0.0, 1.0].
            // The DB schema allows up to NUMERIC(10,6) — reject out-of-range values
            // rather than letting them corrupt the failure buffer calculation.
            if !(0.0..=1.0).contains(&p) {
                warn!(
                    event = "strategy_scores_db.invalid_revert_rate",
                    strategy_kind,
                    chain_id,
                    revert_rate = p,
                    "revert_rate out of [0,1] — discarding, using proxy fallback"
                );
                return None;
            }
            debug!(
                event = "strategy_scores_db.fetched",
                strategy_kind,
                chain_id,
                p_fail = p,
                sample_count = r.sample_count,
                "fetched revert_rate from strategy_scores"
            );
            Some(StrategyFailRate {
                p_fail: p,
                sample_count: r.sample_count,
            })
        }
        Ok(None) => {
            debug!(
                event = "strategy_scores_db.no_row",
                strategy_kind,
                chain_id,
                min_sample_count = MIN_SAMPLE_COUNT,
                "no qualifying strategy_scores row — proxy fallback"
            );
            None
        }
        Err(e) => {
            warn!(
                event = "strategy_scores_db.query_error",
                strategy_kind,
                chain_id,
                error = %e,
                "strategy_scores query failed — proxy fallback"
            );
            None
        }
    }
}
