//! `SimulatorBackend` — abstraction over Anvil-fork and REVM-in-memory simulators.
//!
//! New backends implement this trait. `sim-ctl/main.rs` selects the concrete
//! type at boot via `SIM_BACKEND` env var (default: `anvil`).
//!
//! The trait is `async_trait` because:
//!   - Anvil calls are async (HTTP RPC).
//!   - REVM calls are sync internally but wrapped in `spawn_blocking` to avoid
//!     blocking the tokio executor.
//!
//! R8 Fail-Honest: every backend MUST return a populated `SimulationResult` even
//! on error — `passed = false`, `fail_reason = Some(...)`. NEVER return Ok with
//! fabricated data.

use async_trait::async_trait;
use shared_rs::contracts::{Opportunity, SimulationResult};

/// Error type for the simulation boundary.  All internal errors are folded into
/// a `SimulationResult { passed: false }` by each backend; this enum is only
/// used for unrecoverable infrastructure failures (e.g., can't acquire a DB pool)
/// that should bubble up to the caller.
#[derive(Debug, thiserror::Error)]
pub enum SimulationError {
    #[error("infrastructure failure: {0}")]
    Infra(String),
}

/// Abstraction over simulation backends.
///
/// Implementations:
/// - [`crate::anvil_backend::AnvilBackend`] — delegates to the existing `SimEngine`
///   (Anvil fork via eth_call). Selected by `SIM_BACKEND=anvil` or by default.
/// - [`crate::revm_backend::RevmBackend`] — wraps `simulator_v2::SimulatorV2` for
///   in-memory REVM simulation. Selected by `SIM_BACKEND=revm`.
#[async_trait]
pub trait SimulatorBackend: Send + Sync {
    /// Simulate a candidate opportunity. Always returns a `SimulationResult`.
    /// Never fabricates a passing result when the underlying simulation failed.
    async fn simulate(&self, opp: &Opportunity) -> Result<SimulationResult, SimulationError>;

    /// Human-readable name for logs and `SimulationResult.simulator` tagging.
    fn name(&self) -> &str;
}
