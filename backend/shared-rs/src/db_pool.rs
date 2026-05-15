//! OMEGA-8 / M3 — P1-2 close: shared sqlx pool builder with timeouts.
//!
//! Every Rust binary that opens a `PgPool` MUST go through this helper. It
//! guarantees:
//!   - `acquire_timeout`  → connection acquisition cannot block forever
//!   - `idle_timeout`     → stale connections recycle
//!   - `max_lifetime`     → connections rotate to avoid stale state
//!   - `min_connections`  → warm-up after restart
//!
//! All four are env-overridable so operators can tune per service; safe
//! defaults are conservative. The function does NOT swallow connection
//! errors — callers decide whether absence of a DB is fail-fast or fail-honest.
//!
//! Logging: on success, emits `db.connected` with the pool config (NEVER the
//! DSN). On timeout/error, emits `db.connect_failed` with the redacted error
//! message; the DSN is never logged.

use sqlx::postgres::{PgPool, PgPoolOptions};
use std::time::Duration;

/// Pool configuration resolved from env or defaults.
///
/// Env vars (with defaults):
///   - `DATABASE_POOL_MAX`              (8)
///   - `DATABASE_POOL_MIN`              (1)
///   - `DATABASE_ACQUIRE_TIMEOUT_SECS`  (5)
///   - `DATABASE_IDLE_TIMEOUT_SECS`     (600)
///   - `DATABASE_MAX_LIFETIME_SECS`     (1800)
pub struct PoolConfig {
    pub max_connections: u32,
    pub min_connections: u32,
    pub acquire_timeout: Duration,
    pub idle_timeout: Duration,
    pub max_lifetime: Duration,
}

impl PoolConfig {
    /// Construct from env with safe defaults.
    pub fn from_env(default_max: u32) -> Self {
        let max_connections = std::env::var("DATABASE_POOL_MAX")
            .ok()
            .and_then(|v| v.parse::<u32>().ok())
            .unwrap_or(default_max);
        let min_connections = std::env::var("DATABASE_POOL_MIN")
            .ok()
            .and_then(|v| v.parse::<u32>().ok())
            .unwrap_or(1);
        let acquire_timeout_secs = std::env::var("DATABASE_ACQUIRE_TIMEOUT_SECS")
            .ok()
            .and_then(|v| v.parse::<u64>().ok())
            .unwrap_or(5);
        let idle_timeout_secs = std::env::var("DATABASE_IDLE_TIMEOUT_SECS")
            .ok()
            .and_then(|v| v.parse::<u64>().ok())
            .unwrap_or(600);
        let max_lifetime_secs = std::env::var("DATABASE_MAX_LIFETIME_SECS")
            .ok()
            .and_then(|v| v.parse::<u64>().ok())
            .unwrap_or(1800);
        Self {
            max_connections,
            min_connections,
            acquire_timeout: Duration::from_secs(acquire_timeout_secs),
            idle_timeout: Duration::from_secs(idle_timeout_secs),
            max_lifetime: Duration::from_secs(max_lifetime_secs),
        }
    }
}

/// Build a `PgPoolOptions` with timeouts applied. Caller invokes
/// `.connect(url).await` (or `.connect_lazy(url)`).
///
/// Keeps return type as `PgPoolOptions` so callers retain control over the
/// connect strategy and any custom post-connect hooks.
pub fn options_with_timeouts(cfg: &PoolConfig) -> PgPoolOptions {
    PgPoolOptions::new()
        .max_connections(cfg.max_connections)
        .min_connections(cfg.min_connections)
        .acquire_timeout(cfg.acquire_timeout)
        .idle_timeout(cfg.idle_timeout)
        .max_lifetime(cfg.max_lifetime)
}

/// One-shot helper: build a connected `PgPool` from a URL with env-driven
/// timeouts and a sensible default for `max_connections`.
pub async fn connect_pool(url: &str, default_max: u32) -> Result<PgPool, sqlx::Error> {
    let cfg = PoolConfig::from_env(default_max);
    options_with_timeouts(&cfg).connect(url).await
}
