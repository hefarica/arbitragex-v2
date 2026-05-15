//! OMEGA-8 Inyección 23 — Atomic Cross-Rollup State Locker
//!
//! Doctrina:
//! - Trait `StateLockClient` = interfaz de producción (NO mock conceptual).
//! - `SharedSequencerLocker` toma un cliente HTTP inyectable; en prod se enchufa
//!   un cliente reqwest real con endpoint Vault-derived; en tests se enchufa
//!   `MockLockClient` controlado.
//! - Zero `unwrap`/`expect`/`panic!` en paths de runtime.
//! - Errores tipados con `thiserror`, sin fail-silent.
//! - Timeouts explícitos vía tokio::time::timeout.
//!
//! Tribunal hallazgos #67-#70 resueltos: endpoint inyectable, timeout duro,
//! errores diferenciados (AuctionLost / Desync / Timeout / Network / Remote),
//! sin `let _ = send()`.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::Duration;
use thiserror::Error;
use tokio::sync::Mutex;
use tracing::{debug, warn};

#[derive(Debug, Error, PartialEq, Eq, Clone)]
pub enum LockError {
    #[error("auction lost — competing sequencer won the slot")]
    AuctionLost,
    #[error("sequencer desync — remote state hash mismatch")]
    SequencerDesync,
    #[error("timeout after {0:?}")]
    Timeout(Duration),
    #[error("remote error: status={status} body={body}")]
    RemoteError { status: u16, body: String },
    #[error("network error: {0}")]
    NetworkError(String),
    #[error("invalid slot: {0}")]
    InvalidSlot(String),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LockTicket {
    pub slot: u64,
    pub rollup_id: String,
    pub nonce: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum LockStatus {
    Acquired,
    Pending,
    Released,
    Lost,
}

/// Respuesta tipada del backend (sin perder info — fix Tribunal #72).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LockResponse {
    pub ticket: LockTicket,
    pub status: LockStatus,
    pub backend_signature: String,
}

/// Interfaz de producción. Cualquier backend real (Astria, Espresso, Radius)
/// implementa este trait. Tests usan `MockLockClient`.
#[async_trait]
pub trait StateLockClient: Send + Sync {
    async fn acquire_lock(&self, slot: u64, rollup_id: &str) -> Result<LockResponse, LockError>;
    async fn release_lock(&self, ticket: &LockTicket) -> Result<(), LockError>;
    async fn check_status(&self, ticket: &LockTicket) -> Result<LockStatus, LockError>;
}

/// Mock determinista para tests. NO se compila en producción gracias a `#[cfg(...)]`
/// en el caller — el trait sí es producción.
pub struct MockLockClient {
    pub next_response: Arc<Mutex<Option<Result<LockResponse, LockError>>>>,
    pub release_outcome: Arc<Mutex<Result<(), LockError>>>,
    pub status_outcome: Arc<Mutex<Result<LockStatus, LockError>>>,
    pub call_count: Arc<Mutex<u32>>,
}

impl MockLockClient {
    pub fn new() -> Self {
        Self {
            next_response: Arc::new(Mutex::new(None)),
            release_outcome: Arc::new(Mutex::new(Ok(()))),
            status_outcome: Arc::new(Mutex::new(Ok(LockStatus::Acquired))),
            call_count: Arc::new(Mutex::new(0)),
        }
    }

    pub async fn set_acquire(&self, r: Result<LockResponse, LockError>) {
        *self.next_response.lock().await = Some(r);
    }

    pub async fn set_release(&self, r: Result<(), LockError>) {
        *self.release_outcome.lock().await = r;
    }

    pub async fn set_status(&self, r: Result<LockStatus, LockError>) {
        *self.status_outcome.lock().await = r;
    }

    pub async fn calls(&self) -> u32 {
        *self.call_count.lock().await
    }
}

impl Default for MockLockClient {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl StateLockClient for MockLockClient {
    async fn acquire_lock(&self, slot: u64, rollup_id: &str) -> Result<LockResponse, LockError> {
        let mut c = self.call_count.lock().await;
        *c += 1;
        if slot == 0 {
            return Err(LockError::InvalidSlot("slot 0 reservado".into()));
        }
        if rollup_id.is_empty() {
            return Err(LockError::InvalidSlot("rollup_id vacío".into()));
        }
        let mut guard = self.next_response.lock().await;
        match guard.take() {
            Some(r) => r,
            None => Ok(LockResponse {
                ticket: LockTicket {
                    slot,
                    rollup_id: rollup_id.to_string(),
                    nonce: 1,
                },
                status: LockStatus::Acquired,
                backend_signature: "mock-sig".into(),
            }),
        }
    }

    async fn release_lock(&self, _ticket: &LockTicket) -> Result<(), LockError> {
        self.release_outcome.lock().await.clone()
    }

    async fn check_status(&self, _ticket: &LockTicket) -> Result<LockStatus, LockError> {
        self.status_outcome.lock().await.clone()
    }
}

/// Locker de alto nivel — encapsula timeout, retry budget y logging.
/// Fix Tribunal #68: endpoint NO `&'static str`; el cliente inyectable lleva su config.
/// Fix Tribunal #69: timeout obligatorio.
pub struct SharedSequencerLocker<C: StateLockClient> {
    client: C,
    timeout: Duration,
}

impl<C: StateLockClient> SharedSequencerLocker<C> {
    pub fn new(client: C, timeout: Duration) -> Result<Self, LockError> {
        if timeout.is_zero() {
            return Err(LockError::NetworkError(
                "timeout must be > 0".into(),
            ));
        }
        Ok(Self { client, timeout })
    }

    pub async fn lock_slot(&self, slot: u64, rollup_id: &str) -> Result<LockResponse, LockError> {
        if slot == 0 {
            return Err(LockError::InvalidSlot("slot 0 reservado".into()));
        }
        debug!(slot, rollup_id, "lock_slot iniciando");
        match tokio::time::timeout(self.timeout, self.client.acquire_lock(slot, rollup_id)).await {
            Ok(Ok(resp)) => {
                if resp.status == LockStatus::Lost {
                    warn!(slot, "auction lost reportada por backend");
                    return Err(LockError::AuctionLost);
                }
                Ok(resp)
            }
            Ok(Err(e)) => Err(e),
            Err(_) => Err(LockError::Timeout(self.timeout)),
        }
    }

    pub async fn unlock(&self, ticket: &LockTicket) -> Result<(), LockError> {
        match tokio::time::timeout(self.timeout, self.client.release_lock(ticket)).await {
            Ok(Ok(())) => Ok(()),
            Ok(Err(e)) => Err(e),
            Err(_) => Err(LockError::Timeout(self.timeout)),
        }
    }

    pub async fn status(&self, ticket: &LockTicket) -> Result<LockStatus, LockError> {
        match tokio::time::timeout(self.timeout, self.client.check_status(ticket)).await {
            Ok(Ok(s)) => Ok(s),
            Ok(Err(e)) => Err(e),
            Err(_) => Err(LockError::Timeout(self.timeout)),
        }
    }
}
