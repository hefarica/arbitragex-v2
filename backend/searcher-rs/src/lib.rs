//! searcher-rs library facade.
//!
//! Exposes internal modules for integration tests and future orchestrator crates.
//! The binary entry point is `main.rs`.

// Suppress the same lints as main.rs for consistency. Individual modules
// carry their own allows where the pattern is demonstrably safe.
#![deny(clippy::unwrap_used, clippy::expect_used)]

pub mod calldata;
pub mod route_decoder;
pub mod route_intent;
pub mod strategy_label;
