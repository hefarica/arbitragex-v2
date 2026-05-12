//! searcher-rs library facade.
//!
//! Exposes internal modules for integration tests and future orchestrator crates.
//! The binary entry point is `main.rs`.

// Suppress the same lints as main.rs for consistency. Individual modules
// carry their own allows where the pattern is demonstrably safe.
#![deny(clippy::unwrap_used, clippy::expect_used)]

// Phase 1-3 modules re-exported so the library target compiles standalone.
pub mod amm_math;
pub mod calldata;
pub mod counters;
pub mod dedup;
// Phase 16: per-strategy Prometheus metrics for the orchestrator.
pub mod metrics;
pub mod impact_index;
pub mod opportunity_emitter;
pub mod patterns;
pub mod persistence;
pub mod publisher;
pub mod reserves;
pub mod route_decoder;
pub mod route_intent;
pub mod strategy_label;
// Phase 7-8: orchestrator + engines exposed for integration tests.
pub mod engines;
pub mod orchestrator;
// Phase 9: workers module exposed so engines can reuse pure math kernels.
// Workers themselves contain I/O-heavy code that is not called from tests;
// the lib target only needs the pure-function submodules (triangular_worker,
// flashloan_arb_worker) which have no async I/O in their math kernels.
#[allow(dead_code)]
pub mod workers;
// Phase 12: StateProjector — virtual post-tx pool state projection.
pub mod state_projector;
// Phase 13: SizeOptimizer — optimal amount_in sizing for arb candidates.
pub mod size_optimizer;
// Phase 11: LendingPositionIndexer — Redis-backed watchlist + cache for
// Aave V3 / Compound V2 positions, consumed by LiquidationEngine.
pub mod lending_position_indexer;
