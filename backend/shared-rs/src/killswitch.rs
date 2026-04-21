//! Kill-switch client.
//!
//! - Reads `arbx:killswitch` JSON from Redis.
//! - Caches last state for `cache_ttl` (default 1s) to avoid per-op round trips.
//! - Services call `client.is_enabled().await?` before any critical action.
//! - Updates propagate via pub/sub channel `arbx:killswitch:changes` (optional subscriber).

use chrono::{DateTime, Utc};
use redis::AsyncCommands;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;

pub const KILLSWITCH_KEY: &str = "arbx:killswitch";
pub const KILLSWITCH_CHANNEL: &str = "arbx:killswitch:changes";

#[derive(Debug, thiserror::Error)]
pub enum KillSwitchError {
    #[error("redis error: {0}")]
    Redis(#[from] redis::RedisError),
    #[error("serialization error: {0}")]
    Serde(#[from] serde_json::Error),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KillSwitchState {
    pub enabled: bool,
    pub reason: Option<String>,
    pub triggered_by: Option<String>,
    pub updated_at: DateTime<Utc>,
}

impl Default for KillSwitchState {
    fn default() -> Self {
        Self {
            enabled: false,
            reason: None,
            triggered_by: None,
            updated_at: Utc::now(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct KillSwitchClient {
    mgr: redis::aio::ConnectionManager,
    default_when_absent: bool,
    cache: Arc<RwLock<Option<(KillSwitchState, Instant)>>>,
    cache_ttl: Duration,
}

impl KillSwitchClient {
    pub async fn connect(url: &str, default_when_absent: bool) -> Result<Self, KillSwitchError> {
        let client = redis::Client::open(url)?;
        let mgr = client.get_connection_manager().await?;
        Ok(Self {
            mgr,
            default_when_absent,
            cache: Arc::new(RwLock::new(None)),
            cache_ttl: Duration::from_secs(1),
        })
    }

    pub fn with_cache_ttl(mut self, ttl: Duration) -> Self {
        self.cache_ttl = ttl;
        self
    }

    /// True if the kill-switch is currently armed. Safe-by-default: on any error
    /// returns `default_when_absent` (pass `true` in prod, `false` in dev).
    pub async fn is_enabled(&self) -> bool {
        self.state().await.map(|s| s.enabled).unwrap_or(self.default_when_absent)
    }

    pub async fn state(&self) -> Result<KillSwitchState, KillSwitchError> {
        {
            let g = self.cache.read().await;
            if let Some((s, at)) = &*g {
                if at.elapsed() < self.cache_ttl {
                    return Ok(s.clone());
                }
            }
        }
        let mut mgr = self.mgr.clone();
        let raw: Option<String> = mgr.get(KILLSWITCH_KEY).await?;
        let state = match raw {
            Some(v) => serde_json::from_str::<KillSwitchState>(&v)?,
            None => KillSwitchState { enabled: self.default_when_absent, ..Default::default() },
        };
        let mut g = self.cache.write().await;
        *g = Some((state.clone(), Instant::now()));

        // Publish metric gauge
        crate::metrics::KILLSWITCH_ENABLED.set(if state.enabled { 1 } else { 0 });

        Ok(state)
    }

    /// Admin API. Returns the new state. Writes to Redis + publishes.
    pub async fn set(
        &self,
        enabled: bool,
        reason: Option<String>,
        triggered_by: Option<String>,
    ) -> Result<KillSwitchState, KillSwitchError> {
        let state = KillSwitchState {
            enabled,
            reason,
            triggered_by,
            updated_at: Utc::now(),
        };
        let json = serde_json::to_string(&state)?;
        let mut mgr = self.mgr.clone();
        let _: () = mgr.set(KILLSWITCH_KEY, &json).await?;
        let _: i64 = mgr.publish(KILLSWITCH_CHANNEL, &json).await?;

        let mut g = self.cache.write().await;
        *g = Some((state.clone(), Instant::now()));

        crate::metrics::KILLSWITCH_ENABLED.set(if enabled { 1 } else { 0 });
        Ok(state)
    }
}
