//! OMEGA-8 Inyección 25 — GPU EVM Coprocessor (batch route simulation)
//!
//! Doctrina:
//! - Trait `GpuBackend` = interfaz de producción. Backends reales (cuEVM, sputnikvm-gpu,
//!   revm wasm-bindgen-cuda) implementan este trait detrás de feature flag.
//! - `StubGpuBackend` calcula topological yield determinístico desde input
//!   (no mágico, no hardcoded). En tests verificamos invariantes matemáticos
//!   sobre el cálculo, no sobre números literales.
//! - Tribunal #78: NO `panic!` — devuelve `Err(GpuError::ContextLost)`.
//! - Tribunal #91: cap MAX_BATCH = 100_000.
//! - Tribunal #90: timeout configurable.
//! - Tribunal #83: aritmética entera (u128); ratios en bps.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::Duration;
use thiserror::Error;
use tokio::sync::Mutex;
use tracing::{debug, info};

pub const MAX_BATCH: usize = 100_000;
pub const ADDRESS_BYTES: usize = 20;

#[derive(Debug, Error, PartialEq, Eq, Clone)]
pub enum GpuError {
    #[error("gpu context lost or uninitialized")]
    ContextLost,
    #[error("batch demasiado grande: {0} (max {})", MAX_BATCH)]
    BatchTooLarge(usize),
    #[error("batch vacío")]
    BatchEmpty,
    #[error("ruta inválida en idx {idx}: {reason}")]
    InvalidRoute { idx: usize, reason: String },
    #[error("timeout {0:?}")]
    Timeout(Duration),
    #[error("backend error: {0}")]
    Backend(String),
}

/// Dirección EVM compactada (20 bytes) — newtype opaco para no leakear tipos externos.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct GpuAddress(pub [u8; ADDRESS_BYTES]);

impl GpuAddress {
    pub fn from_bytes(b: [u8; ADDRESS_BYTES]) -> Self {
        Self(b)
    }
    pub fn as_bytes(&self) -> &[u8; ADDRESS_BYTES] {
        &self.0
    }
}

/// Ruta candidata: secuencia de hops (mínimo 2, máximo 6 para circular arb).
pub type Route = Vec<GpuAddress>;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SimResult {
    pub route_idx: usize,
    pub topological_yield_wei: u128,
    pub gas_estimate: u64,
}

#[async_trait]
pub trait GpuBackend: Send + Sync {
    async fn batch_simulate(&self, routes: &[Route]) -> Result<Vec<SimResult>, GpuError>;
    fn is_ready(&self) -> bool;
}

/// Backend stub determinista. Calcula yield desde hash de la ruta — NO mágico,
/// reproducible, suficiente para validar el pipeline.
pub struct StubGpuBackend {
    ready: bool,
    delay: Arc<Mutex<Duration>>,
    forced_err: Arc<Mutex<Option<GpuError>>>,
}

impl Default for StubGpuBackend {
    fn default() -> Self {
        Self::new(true)
    }
}

impl StubGpuBackend {
    pub fn new(ready: bool) -> Self {
        Self {
            ready,
            delay: Arc::new(Mutex::new(Duration::from_millis(0))),
            forced_err: Arc::new(Mutex::new(None)),
        }
    }
    pub async fn set_delay(&self, d: Duration) {
        *self.delay.lock().await = d;
    }
    pub async fn force_error(&self, e: GpuError) {
        *self.forced_err.lock().await = Some(e);
    }
}

#[async_trait]
impl GpuBackend for StubGpuBackend {
    fn is_ready(&self) -> bool {
        self.ready
    }

    async fn batch_simulate(&self, routes: &[Route]) -> Result<Vec<SimResult>, GpuError> {
        if let Some(e) = self.forced_err.lock().await.take() {
            return Err(e);
        }
        if !self.ready {
            return Err(GpuError::ContextLost);
        }
        let d = *self.delay.lock().await;
        if !d.is_zero() {
            tokio::time::sleep(d).await;
        }
        let mut out = Vec::with_capacity(routes.len());
        for (idx, r) in routes.iter().enumerate() {
            // Hash determinístico sumando bytes — yield positivo solo si suma par.
            let mut acc: u128 = 0;
            for hop in r {
                for b in hop.as_bytes() {
                    acc = acc.wrapping_add(*b as u128);
                }
            }
            // Yield = acc * 10^15 si acc par; gas = len*21000.
            if acc % 2 == 0 {
                out.push(SimResult {
                    route_idx: idx,
                    topological_yield_wei: acc.saturating_mul(1_000_000_000_000_000u128),
                    gas_estimate: (r.len() as u64).saturating_mul(21_000),
                });
            }
        }
        Ok(out)
    }
}

/// Coprocesador de alto nivel — valida entrada, aplica timeout, delega al backend.
pub struct GpuEvmCoprocessor<B: GpuBackend> {
    backend: B,
    timeout: Duration,
}

impl<B: GpuBackend> GpuEvmCoprocessor<B> {
    pub fn new(backend: B, timeout: Duration) -> Result<Self, GpuError> {
        if timeout.is_zero() {
            return Err(GpuError::Backend("timeout debe ser > 0".into()));
        }
        if !backend.is_ready() {
            return Err(GpuError::ContextLost);
        }
        info!("🜂 GPU EVM coprocessor inicializado");
        Ok(Self { backend, timeout })
    }

    pub async fn simulate_batch(&self, routes: Vec<Route>) -> Result<Vec<SimResult>, GpuError> {
        if routes.is_empty() {
            return Err(GpuError::BatchEmpty);
        }
        if routes.len() > MAX_BATCH {
            return Err(GpuError::BatchTooLarge(routes.len()));
        }
        for (idx, r) in routes.iter().enumerate() {
            if r.len() < 2 {
                return Err(GpuError::InvalidRoute {
                    idx,
                    reason: format!("ruta con {} hops (mínimo 2)", r.len()),
                });
            }
            if r.len() > 6 {
                return Err(GpuError::InvalidRoute {
                    idx,
                    reason: format!("ruta con {} hops (máximo 6)", r.len()),
                });
            }
        }
        debug!(n = routes.len(), "GPU batch dispatch");
        match tokio::time::timeout(self.timeout, self.backend.batch_simulate(&routes)).await {
            Ok(Ok(r)) => Ok(r),
            Ok(Err(e)) => Err(e),
            Err(_) => Err(GpuError::Timeout(self.timeout)),
        }
    }
}
