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
// Phase A.3.c.3: multi-step REVM sequence runner with persistent CacheDB
// for round-trip simulations (forward swap → read intermediate → backward).
// Unblocked by A.3.c.4's `DatabaseRef for LazyDb` foundation.
pub mod sequence_runner;

// ---------------------------------------------------------------------------
// Compile-time capability truth (G-SIM-1 FASE 1)
// ---------------------------------------------------------------------------

/// Identity tag this crate reports for `simulator_backend` in sim-ctl's
/// GET /capabilities. Declared HERE — in the crate that owns the modules —
/// so downstream consumers relay it instead of hand-writing it (RULE 00).
pub const BACKEND_TAG: &str = "v2";

/// Compile-time capability snapshot of this crate.
///
/// RULE 00 Zero-Mocks: consumers (sim-ctl GET /capabilities) must serialize
/// THIS, never their own hand-written module list. Linkage is the proof —
/// the call only compiles when simulator-v2 is actually a dependency of the
/// binary, so a build that dropped simulator-v2 fails to compile instead of
/// reporting stale truth.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Capabilities {
    /// Pub modules compiled into this crate. Mirrors the `pub mod` items at
    /// the top of this file — kept side by side so drift is caught in review
    /// of THIS file, not in a consumer.
    pub modules: &'static [&'static str],
    /// Cargo features actually enabled at compile time. Derived from `cfg!`
    /// so the list can never claim a feature the build did not enable.
    /// simulator-v2 declares no `[features]` table today → empty by
    /// construction; adding a feature requires adding a `cfg!` line in
    /// [`compiled_features`].
    pub features: Vec<&'static str>,
}

/// Report the compile-time capabilities of this exact build of simulator-v2.
pub fn capabilities() -> Capabilities {
    Capabilities {
        modules: &["bellman_ford", "lazy_db", "revm_runner", "sequence_runner"],
        features: compiled_features(),
    }
}

/// cfg!-derived feature list. Every entry must be gated by a `cfg!` check on
/// the same name — no bare strings — so the array cannot diverge from the
/// feature set the compiler actually saw.
fn compiled_features() -> Vec<&'static str> {
    // simulator-v2 defines no [features] table today. When one is added,
    // switch to `let mut features = Vec::new();` and gate each entry, e.g.:
    // if cfg!(feature = "some-flag") { features.push("some-flag"); }
    Vec::new()
}

pub use lazy_db::LazyDb;
pub use lazy_db::LazyDbError;
// Phase A.3.c.3 — re-export revm primitives so downstream crates
// (searcher-rs) can construct `StorageOverride`, `SequenceCall`, etc.
// without depending on `revm` directly.
pub use revm::primitives::{Address as AlloyAddress, U256 as AlloyU256};

use serde::{Deserialize, Serialize};
use std::sync::OnceLock;
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
    /// Gas price in wei used to compute net-of-gas profit (CRITICAL #2 fix).
    ///
    /// revm deducts `gas_used × gas_price_wei` from the caller's balance, so
    /// `SimResult.net_profit_wei` is the TRUE net P&L after gas costs.
    /// Set to `0` only in tests that intentionally measure token-only delta.
    pub gas_price_wei: u128,
}

// ---------------------------------------------------------------------------
// Simulator trait
// ---------------------------------------------------------------------------

/// Trait the searcher consumes when `ARBX_USE_SIMULATOR_V2=true`.
pub trait Simulator: Send + Sync {
    /// Simulate `candidate` and return the result.
    ///
    /// Returned `net_profit_wei` is net of gas at `candidate.gas_price_wei`
    /// (CRITICAL #2 fix: G-NET-1 compliance).
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
/// // Or let LazyDb resolve the latest block at first use (memoized):
/// let sim = SimulatorV2::new("https://…rpc-url");
/// ```
pub struct SimulatorV2 {
    /// Operator-supplied RPC endpoint for state queries.
    pub rpc_url: String,
    /// Memoized block number (MAJOR #3 fix: linearizability).
    ///
    /// - Populated by `with_block()` before any `simulate()` call.
    /// - Populated lazily on the first `simulate()` call that resolves "latest".
    /// - All subsequent calls reuse this value → all calls see the same block.
    block_number: OnceLock<u64>,
}

impl SimulatorV2 {
    /// Create a `SimulatorV2` that resolves the latest block on first use.
    pub fn new(rpc_url: impl Into<String>) -> Self {
        Self {
            rpc_url: rpc_url.into(),
            block_number: OnceLock::new(),
        }
    }

    /// Pin simulations to a specific block number (builder pattern).
    ///
    /// Pre-populates the `OnceLock` so `simulate()` never queries the chain
    /// for the block number.
    pub fn with_block(self, block: u64) -> Self {
        // OnceLock::set returns Err if already set; that is fine — the first
        // call wins and a double-set is a no-op with the same intent.
        let _ = self.block_number.set(block);
        self
    }

    /// The memoized/pinned block, when one is already resolved.
    ///
    /// `Some(b)` after `with_block(b)` or a first `simulate()`; `None` when the
    /// instance has not resolved a block yet (lazy "latest" mode). Used by
    /// `sim_core::sim_multistep::execute_multistep_revm` so the multi-step REVM
    /// path replays at the SAME pinned block the `SimulatorV2` was configured
    /// with (variance-benchmark determinism) instead of always resolving
    /// "latest".
    pub fn pinned_block(&self) -> Option<u64> {
        self.block_number.get().copied()
    }
}

impl Simulator for SimulatorV2 {
    /// Simulate `candidate` using revm against a `LazyDb` backed by the
    /// configured RPC endpoint.
    ///
    /// Returned `net_profit_wei` is net of gas at `candidate.gas_price_wei`
    /// (CRITICAL #2 fix: G-NET-1 compliance).
    ///
    /// ## Block memoization (MAJOR #3 fix)
    /// The first call that must resolve "latest" queries the chain and stores
    /// the result in `self.block_number`.  All subsequent calls use the stored
    /// value, guaranteeing that every candidate sees the same block.
    ///
    /// ## BlockEnv consistency (MAJOR #6 fix)
    /// `effective_block` is passed to both `LazyDb::new()` (DB pin) and
    /// `revm_runner::run()` (BlockEnv.number).  They always agree.
    fn simulate(&self, candidate: &CandidateInput) -> Result<SimResult, SimError> {
        // Determine the effective block, resolving "latest" at most once.
        // Priority: candidate's explicit block > memoized instance block > RPC.
        let explicit = if candidate.block_number != 0 {
            Some(candidate.block_number)
        } else {
            self.block_number.get().copied()
        };

        // If we still have no block, create a LazyDb with None so it resolves
        // "latest" and then memoize the resolved block number.
        let effective_block = match explicit {
            Some(b) => b,
            None => {
                // Construct a temporary LazyDb purely to resolve the latest block.
                let probe = LazyDb::new(&self.rpc_url, None).map_err(|e| {
                    warn!(
                        event = "simulator_v2.block_resolve_failed",
                        error = %e,
                        "failed to resolve latest block"
                    );
                    SimError::Provider(format!("LazyDb::new (block resolve): {e}"))
                })?;
                let resolved = probe.pinned_block_number();
                // Store once; concurrent callers racing here all compute the
                // same block (within a single slot window), so last-writer-wins
                // is acceptable.
                let _ = self.block_number.set(resolved);
                resolved
            }
        };

        let db = LazyDb::new(&self.rpc_url, Some(effective_block)).map_err(|e| {
            warn!(
                event = "simulator_v2.lazy_db_create_failed",
                error = %e,
                "failed to create LazyDb"
            );
            SimError::Provider(format!("LazyDb::new: {e}"))
        })?;

        revm_runner::run(candidate, db, effective_block)
    }
}
