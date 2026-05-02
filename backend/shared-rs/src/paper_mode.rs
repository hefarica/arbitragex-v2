//! Dynamic Paper Mode Client.
//!
//! - Reads `arbx:papermode` JSON from Redis.
//! - Caches last state for `cache_ttl` (default 1s) to avoid per-op round trips.
//! - Replaces the static TOML configuration with a dynamic runtime toggle.

use chrono::{DateTime, Utc};
use redis::AsyncCommands;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;

pub const PAPERMODE_KEY: &str = "arbx:papermode";
pub const PAPERMODE_CHANNEL: &str = "arbx:papermode:changes";

#[derive(Debug, thiserror::Error)]
pub enum PaperModeError {
    #[error("redis error: {0}")]
    Redis(#[from] redis::RedisError),
    #[error("serialization error: {0}")]
    Serde(#[from] serde_json::Error),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaperModeState {
    pub enabled: bool,
    pub updated_at: DateTime<Utc>,
    pub updated_by: Option<String>,
}

impl Default for PaperModeState {
    fn default() -> Self {
        Self {
            enabled: true, // Safe default: paper mode ON
            updated_at: Utc::now(),
            updated_by: None,
        }
    }
}

#[derive(Clone)]
pub struct PaperModeClient {
    mgr: redis::aio::ConnectionManager,
    default_when_absent: bool,
    cache: Arc<RwLock<Option<(PaperModeState, Instant)>>>,
    cache_ttl: Duration,
}

impl PaperModeClient {
    pub async fn connect(url: &str, default_when_absent: bool) -> Result<Self, PaperModeError> {
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

    pub async fn is_enabled(&self) -> bool {
        self.state().await.map(|s| s.enabled).unwrap_or(self.default_when_absent)
    }

    pub async fn state(&self) -> Result<PaperModeState, PaperModeError> {
        {
            let g = self.cache.read().await;
            if let Some((s, at)) = &*g {
                if at.elapsed() < self.cache_ttl {
                    return Ok(s.clone());
                }
            }
        }
        let mut mgr = self.mgr.clone();
        let raw: Option<String> = mgr.get(PAPERMODE_KEY).await?;
        let state = match raw {
            Some(v) => serde_json::from_str::<PaperModeState>(&v)?,
            None => PaperModeState { enabled: self.default_when_absent, ..Default::default() },
        };
        let mut g = self.cache.write().await;
        *g = Some((state.clone(), Instant::now()));

        Ok(state)
    }

    pub async fn set(
        &self,
        enabled: bool,
        updated_by: Option<String>,
    ) -> Result<PaperModeState, PaperModeError> {
        let state = PaperModeState {
            enabled,
            updated_at: Utc::now(),
            updated_by,
        };
        let json = serde_json::to_string(&state)?;
        let mut mgr = self.mgr.clone();
        let _: () = mgr.set(PAPERMODE_KEY, &json).await?;
        let _: i64 = mgr.publish(PAPERMODE_CHANNEL, &json).await?;

        let mut g = self.cache.write().await;
        *g = Some((state.clone(), Instant::now()));

        Ok(state)
    }
}
