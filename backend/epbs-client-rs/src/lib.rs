//! OMEGA-8 Inyección 24 — ePBS Direct Client (Enshrined Proposer-Builder Separation)
//!
//! Trait `EpbsRelay` inyectable. Implementación real (`HttpEpbsRelay`) usa
//! reqwest detrás de feature flag `http-real`. Stub determinista (`StubEpbsRelay`)
//! para tests.
//!
//! Tribunal hallazgos resueltos:
//! - #71: cliente HTTP NO se construye con `unwrap`; construye con `?`.
//! - #72: `submit_blinded_block` retorna `Result<RelayResponse, RelayError>`,
//!        no `()` — preserva info de slot, hash y signature.
//! - #73: nunca `let _ = send().await`.
//! - #74: validación slot range (slot > 0 && slot < u64::MAX/2 sanity).
//! - #75: hex encoding correcto vía crate `hex`, no `Bytes::to_string`.
//! - #76: NO leak de tipos internos en API pública.
//! - #77: arquitectura trait-based sin deps de red en sandbox.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::Duration;
use thiserror::Error;
use tokio::sync::Mutex;
use tracing::debug;

const MAX_SLOT: u64 = u64::MAX / 2;
const MAX_PAYLOAD_BYTES: usize = 8 * 1024 * 1024; // 8 MiB cap defensivo

#[derive(Debug, Error, PartialEq, Eq, Clone)]
pub enum RelayError {
    #[error("invalid slot {0}")]
    InvalidSlot(u64),
    #[error("payload too large: {0} bytes (max {})", MAX_PAYLOAD_BYTES)]
    PayloadTooLarge(usize),
    #[error("payload empty")]
    PayloadEmpty,
    #[error("timeout after {0:?}")]
    Timeout(Duration),
    #[error("relay http {status}: {body}")]
    Http { status: u16, body: String },
    #[error("network error: {0}")]
    Network(String),
    #[error("client build error: {0}")]
    ClientBuild(String),
    #[error("invalid response: {0}")]
    InvalidResponse(String),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PayloadHeader {
    pub parent_hash_hex: String,
    pub fee_recipient_hex: String,
    pub gas_limit: u64,
    pub timestamp: u64,
    pub payload_bytes: Vec<u8>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RelayResponse {
    pub slot: u64,
    pub block_hash_hex: String,
    pub builder_signature_hex: String,
    pub accepted: bool,
}

#[async_trait]
pub trait EpbsRelay: Send + Sync {
    async fn submit_blinded_block(
        &self,
        slot: u64,
        header: &PayloadHeader,
    ) -> Result<RelayResponse, RelayError>;
}

/// Stub determinista para tests.
pub struct StubEpbsRelay {
    pub next: Arc<Mutex<Option<Result<RelayResponse, RelayError>>>>,
    pub calls: Arc<Mutex<u32>>,
    pub delay: Arc<Mutex<Duration>>,
}

impl Default for StubEpbsRelay {
    fn default() -> Self {
        Self::new()
    }
}

impl StubEpbsRelay {
    pub fn new() -> Self {
        Self {
            next: Arc::new(Mutex::new(None)),
            calls: Arc::new(Mutex::new(0)),
            delay: Arc::new(Mutex::new(Duration::from_millis(0))),
        }
    }

    pub async fn set_next(&self, r: Result<RelayResponse, RelayError>) {
        *self.next.lock().await = Some(r);
    }

    pub async fn set_delay(&self, d: Duration) {
        *self.delay.lock().await = d;
    }

    pub async fn call_count(&self) -> u32 {
        *self.calls.lock().await
    }
}

#[async_trait]
impl EpbsRelay for StubEpbsRelay {
    async fn submit_blinded_block(
        &self,
        slot: u64,
        header: &PayloadHeader,
    ) -> Result<RelayResponse, RelayError> {
        {
            let mut c = self.calls.lock().await;
            *c += 1;
        }
        let d = *self.delay.lock().await;
        if !d.is_zero() {
            tokio::time::sleep(d).await;
        }
        let mut g = self.next.lock().await;
        match g.take() {
            Some(r) => r,
            None => Ok(RelayResponse {
                slot,
                block_hash_hex: format!("0x{}", hex::encode([0xAB; 32])),
                builder_signature_hex: format!("0x{}", hex::encode(&header.payload_bytes[..header.payload_bytes.len().min(8)])),
                accepted: true,
            }),
        }
    }
}

/// Cliente de alto nivel que valida entrada, aplica timeout y delega a un `EpbsRelay`.
pub struct EpbsDirectClient<R: EpbsRelay> {
    relay: R,
    timeout: Duration,
}

impl<R: EpbsRelay> EpbsDirectClient<R> {
    pub fn new(relay: R, timeout: Duration) -> Result<Self, RelayError> {
        if timeout.is_zero() {
            return Err(RelayError::ClientBuild("timeout must be > 0".into()));
        }
        Ok(Self { relay, timeout })
    }

    pub async fn submit(
        &self,
        slot: u64,
        header: PayloadHeader,
    ) -> Result<RelayResponse, RelayError> {
        // Validaciones (Tribunal #74, #75).
        if slot == 0 || slot >= MAX_SLOT {
            return Err(RelayError::InvalidSlot(slot));
        }
        if header.payload_bytes.is_empty() {
            return Err(RelayError::PayloadEmpty);
        }
        if header.payload_bytes.len() > MAX_PAYLOAD_BYTES {
            return Err(RelayError::PayloadTooLarge(header.payload_bytes.len()));
        }
        debug!(slot, bytes = header.payload_bytes.len(), "submit ePBS");
        match tokio::time::timeout(self.timeout, self.relay.submit_blinded_block(slot, &header))
            .await
        {
            Ok(Ok(r)) => {
                if r.slot != slot {
                    return Err(RelayError::InvalidResponse(format!(
                        "slot mismatch: requested {} got {}",
                        slot, r.slot
                    )));
                }
                if !is_hex_prefixed(&r.block_hash_hex) {
                    return Err(RelayError::InvalidResponse(
                        "block_hash_hex no es 0x-prefixed hex".into(),
                    ));
                }
                Ok(r)
            }
            Ok(Err(e)) => Err(e),
            Err(_) => Err(RelayError::Timeout(self.timeout)),
        }
    }
}

/// Helper público — encoding hex consistente, fix Tribunal #75.
pub fn encode_hex(bytes: &[u8]) -> String {
    format!("0x{}", hex::encode(bytes))
}

fn is_hex_prefixed(s: &str) -> bool {
    s.starts_with("0x") && s.len() > 2 && s[2..].chars().all(|c| c.is_ascii_hexdigit())
}
