//! simulator-v2 — opt-in REVM-backed simulator with Bellman-Ford cycle detection.
//!
//! ## Activation
//!
//! Default = NOT used. The existing simulator stub in
//! `prioritization-spine/src/simulator.rs` keeps owning the live data path.
//! When the operator sets `ARBX_USE_SIMULATOR_V2=true` and rebuilds searcher-rs
//! with the `v2-simulator` cargo feature, `searcher-rs/src/main.rs` dispatches
//! candidates through `SimulatorV2::simulate()` instead.
//!
//! ## Sub-modules
//!
//! - [`bellman_ford`]: negative-cycle detection on the token-pool graph.
//!   Reference: `.agents/skills/sop_atomic_route_construction/SKILL.md`.
//! - [`lazy_db`] (Task 4.2): `revm::Database` over an `ethers::Provider<Http>`,
//!   fetching state on-demand with a `DashMap` dedup cache pinned to a block.
//! - [`revm_runner`] (Task 4.3): wires `LazyDb` into `revm::EVM`, executes a
//!   candidate, and returns a `SimResult` with net profit + gas used.

pub mod bellman_ford;
pub mod lazy_db;
pub mod revm_runner;

pub use lazy_db::LazyDb;
pub use lazy_db::LazyDbError;

use serde::{Deserialize, Serialize};
use tracing::warn;

// ---------------------------------------------------------------------------
// Error type
// ---------------------------------------------------------------------------

#[derive(Debug, thiserror::Error)]
pub enum SimError {
    #[error("revm reverted: {0}")]
    Reverted(String),
    #[error("provider failure: {0}")]
    Provider(String),
    /// Kept for callers that match on this variant by name; will never be
    /// constructed by `SimulatorV2::simulate()` after Tasks 4.2/4.3.
    #[error("not implemented")]
    NotImplemented,
}

// ---------------------------------------------------------------------------
// Output / Input types
// ---------------------------------------------------------------------------

/// Output of a single-candidate simulation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SimResult {
    /// Post-execution balance delta of the `from` address in wei.
    /// Positive = net profit; negative = net loss.
    pub net_profit_wei: i128,
    /// Gas consumed by the simulated transaction.
    pub gas_used: u64,
    /// SHA-256(calldata || output_bytes) — a deterministic fingerprint of
    /// this simulation's inputs and outputs.  Not a state trie hash.
    pub trace_hash: [u8; 32],
}

/// Minimal candidate input.  Decoupled from the prioritization-spine
/// `OpportunityCandidate` so the v2 contract can evolve independently.
#[derive(Debug, Clone)]
pub struct CandidateInput {
    pub chain_id: u64,
    pub block_number: u64,
    pub from: [u8; 20],
    pub to: [u8; 20],
    pub calldata: Vec<u8>,
    pub value_wei: u128,
}

// ---------------------------------------------------------------------------
// Simulator trait
// ---------------------------------------------------------------------------

/// Trait the searcher consumes when `ARBX_USE_SIMULATOR_V2=true`.
pub trait Simulator: Send + Sync {
    fn simulate(&self, candidate: &CandidateInput) -> Result<SimResult, SimError>;
}

// ---------------------------------------------------------------------------
// SimulatorV2 — concrete implementation
// ---------------------------------------------------------------------------

/// V2 simulator backed by revm + LazyDb.
///
/// ## Construction
/// ```ignore
/// // Pin to a specific block:
/// let sim = SimulatorV2::new("https://…rpc-url").with_block(21_000_000);
/// // Or let LazyDb resolve the latest block at first use:
/// let sim = SimulatorV2::new("https://…rpc-url");
/// ```
pub struct SimulatorV2 {
    /// Operator-supplied RPC endpoint for state queries.
    pub rpc_url: String,
    /// Block to pin the simulation to.  `None` = resolve latest at call time.
    pub block_number: Option<u64>,
}

impl SimulatorV2 {
    /// Create a `SimulatorV2` that resolves the latest block on first use.
    pub fn new(rpc_url: impl Into<String>) -> Self {
        Self {
            rpc_url: rpc_url.into(),
            block_number: None,
        }
    }

    /// Pin simulations to a specific block number (builder pattern).
    pub fn with_block(mut self, block: u64) -> Self {
        self.block_number = Some(block);
        self
    }
}

impl Simulator for SimulatorV2 {
    /// Simulate `candidate` using revm against a `LazyDb` backed by the
    /// configured RPC endpoint.
    ///
    /// The function:
    /// 1. Constructs a `LazyDb` pinned to `candidate.block_number` (or the
    ///    instance-level `block_number` if set, or latest if neither is set).
    /// 2. Pre-fetches the caller's pre-execution balance into the cache.
    /// 3. Delegates to `revm_runner::run()`.
    fn simulate(&self, candidate: &CandidateInput) -> Result<SimResult, SimError> {
        // Prefer the candidate's own block_number; fall back to the instance
        // pin; resolve latest inside LazyDb::new() if neither is set.
        let block = if candidate.block_number != 0 {
            Some(candidate.block_number)
        } else {
            self.block_number
        };

        let db = LazyDb::new(&self.rpc_url, block).map_err(|e| {
            warn!(
                event = "simulator_v2.lazy_db_create_failed",
                error = %e,
                "failed to create LazyDb"
            );
            SimError::Provider(format!("LazyDb::new: {e}"))
        })?;

        revm_runner::run(candidate, db)
    }
}
