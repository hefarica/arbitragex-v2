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
//! ## Sub-modules (built up across Sprint 4 tasks)
//!
//! - [`bellman_ford`]: negative-cycle detection on the token-pool graph.
//!   Reference implementation: `.agents/skills/sop_atomic_route_construction/SKILL.md`.
//! - [`lazy_db`] (Task 4.2 — pending): `revm::Database` impl over an
//!   `ethers::Provider`, fetching state slots on-demand via
//!   `eth_getStorageAt` / `eth_getCode` with a `DashMap` dedup cache.
//! - [`revm_runner`] (Task 4.3 — pending): builds `revm::Evm` with the lazy
//!   DB, executes a candidate, returns `SimResult` with net profit + gas.
//!
//! Until 4.2/4.3 land, `SimulatorV2::simulate` deliberately returns
//! `unimplemented!()`. The dispatch flag in `searcher-rs/main.rs` logs a
//! warning and falls through to the v1 path so production behaviour is
//! unchanged. This is the no-damage contract honoured.

pub mod bellman_ford;

use serde::{Deserialize, Serialize};

#[derive(Debug, thiserror::Error)]
pub enum SimError {
    #[error("revm reverted: {0}")]
    Reverted(String),
    #[error("provider failure: {0}")]
    Provider(String),
    #[error("not implemented yet — Sprint 4 task 4.3 pending")]
    NotImplemented,
}

/// Output of a single-candidate simulation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SimResult {
    pub net_profit_wei: i128,
    pub gas_used: u64,
    pub trace_hash: [u8; 32],
}

/// Minimal candidate input. Keeping the struct decoupled from the prioritization
/// spine `OpportunityCandidate` lets us evolve the v2 contract without touching
/// the v1 path.
#[derive(Debug, Clone)]
pub struct CandidateInput {
    pub chain_id: u64,
    pub block_number: u64,
    pub from: [u8; 20],
    pub to: [u8; 20],
    pub calldata: Vec<u8>,
    pub value_wei: u128,
}

/// Trait the searcher consumes when ARBX_USE_SIMULATOR_V2 is on.
pub trait Simulator: Send + Sync {
    fn simulate(&self, candidate: &CandidateInput) -> Result<SimResult, SimError>;
}

/// V2 simulator. Concrete implementation arrives with Tasks 4.2 + 4.3; today
/// the type exists so the dispatch flag in searcher-rs compiles cleanly.
pub struct SimulatorV2 {
    /// Operator-supplied RPC endpoint for state queries. Filled at boot.
    pub rpc_url: String,
}

impl SimulatorV2 {
    pub fn new(rpc_url: impl Into<String>) -> Self {
        Self { rpc_url: rpc_url.into() }
    }
}

impl Simulator for SimulatorV2 {
    fn simulate(&self, _candidate: &CandidateInput) -> Result<SimResult, SimError> {
        // Pending Tasks 4.2 (lazy_db) + 4.3 (revm_runner). Until they land,
        // the dispatch flag in searcher-rs logs a warning and falls back to
        // the v1 simulator — no production candidate ever reaches this path.
        Err(SimError::NotImplemented)
    }
}
