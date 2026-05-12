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
