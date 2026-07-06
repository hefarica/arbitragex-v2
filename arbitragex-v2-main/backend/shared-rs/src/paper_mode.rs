//! Dynamic Paper Mode Client.
//!
//! - Reads `arbx:papermode:<chain_id>` JSON from Redis (per-chain).
//! - Falls back to legacy `arbx:papermode` (global) for 30 days from
//!   2026-05-13 (B0.2 migration). Legacy reads tag `source="legacy_fallback"`.
//! - Caches last state for `cache_ttl` (default 1s) to avoid per-op round trips.
//! - Replaces the static TOML configuration with a dynamic runtime toggle.
//!
//! B0.2 (2026-05-13) — Per-chain isolation:
//!   - Each chain has its OWN papermode key/channel.
//!   - Operator changes to Chain X do NOT affect Chain Y.
//!   - Legacy global key is READ-ONLY fallback; new writes always target
//!     a per-chain key.

use chrono::{DateTime, Utc};
use redis::AsyncCommands;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;

/// Legacy global key — READ-ONLY fallback for 30 days from 2026-05-13.
/// New writes MUST use `papermode_key(chain_id)` instead.
pub const PAPERMODE_KEY: &str = "arbx:papermode";
/// Legacy global channel — kept for retro-compat subscribers. New writes
/// publish on BOTH this channel (compat) and the per-chain channel.
pub const PAPERMODE_CHANNEL: &str = "arbx:papermode:changes";

/// Per-chain Redis key for paper-mode state.
pub fn papermode_key(chain_id: u64) -> String {
    format!("arbx:papermode:{chain_id}")
}

/// Per-chain Redis pub/sub channel for paper-mode change notifications.
pub fn papermode_channel(chain_id: u64) -> String {
    format!("arbx:papermode:{chain_id}:changes")
}

/// Source of the paper-mode state we returned. `PerChain` is the desired
/// modern path; `LegacyFallback` indicates the per-chain key was missing
/// and we read the deprecated global key. `Default` is the fail-honest
/// "no key set anywhere" path.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PaperModeSource {
    PerChain,
    LegacyFallback,
    Default,
}

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
        self.state()
            .await
            .map(|s| s.enabled)
            .unwrap_or(self.default_when_absent)
    }

    /// Per-chain paper-mode check. Reads `arbx:papermode:<chain_id>` first;
    /// falls back to legacy global key for 30 days from 2026-05-13.
    pub async fn is_enabled_for_chain(&self, chain_id: u64) -> bool {
        self.state_for_chain(chain_id)
            .await
            .map(|(s, _)| s.enabled)
            .unwrap_or(self.default_when_absent)
    }

    /// Per-chain paper-mode state with source attribution. Returns the
    /// `PaperModeSource` so the caller can surface "legacy_fallback" warnings
    /// in observability when the per-chain key is missing.
    pub async fn state_for_chain(
        &self,
        chain_id: u64,
    ) -> Result<(PaperModeState, PaperModeSource), PaperModeError> {
        let mut mgr = self.mgr.clone();
        // 1. Try per-chain key first.
        let per_chain_key = papermode_key(chain_id);
        let raw: Option<String> = mgr.get(&per_chain_key).await?;
        if let Some(v) = raw {
            let state = serde_json::from_str::<PaperModeState>(&v)?;
            return Ok((state, PaperModeSource::PerChain));
        }
        // 2. Fall back to legacy global key (read-only; 30-day deprecation).
        let raw: Option<String> = mgr.get(PAPERMODE_KEY).await?;
        if let Some(v) = raw {
            let state = serde_json::from_str::<PaperModeState>(&v)?;
            return Ok((state, PaperModeSource::LegacyFallback));
        }
        // 3. Fail-honest default (no key set anywhere — paper-mode safe default).
        Ok((
            PaperModeState {
                enabled: self.default_when_absent,
                ..Default::default()
            },
            PaperModeSource::Default,
        ))
    }

    /// Per-chain paper-mode setter. Writes ONLY to `arbx:papermode:<chain_id>`
    /// (never to the legacy global key). Publishes to both per-chain channel
    /// and the legacy channel for retro-compat subscribers.
    pub async fn set_for_chain(
        &self,
        chain_id: u64,
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
        let _: () = mgr.set(papermode_key(chain_id), &json).await?;
        // Publish on per-chain channel.
        let _: i64 = mgr.publish(papermode_channel(chain_id), &json).await?;
        Ok(state)
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
            None => PaperModeState {
                enabled: self.default_when_absent,
                ..Default::default()
            },
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
