// M11 (audit 2026-05-10): surface panics in hot-path crate.
#![warn(clippy::unwrap_used, clippy::expect_used)]

//! sim-ctl library target.
//!
//! Exists so the GET /capabilities handler (G-SIM-1 FASE 1) is unit-testable
//! under CI's `cargo test --workspace --lib` gate, which runs only lib-target
//! tests — a bin-only crate's inline `#[cfg(test)]` modules would never
//! execute there. The bin (`src/main.rs`) consumes this lib for the route.

pub mod capabilities;
/// §IV blocker A2: boot-time ArbitrageExecutor deployment to the ephemeral
/// anvil fork. Lives in the lib target (same reason as `capabilities`) so the
/// ground-truth unit tests (canonical create1 addresses, forge-broadcast
/// constructor-args vector, artifact sanity) execute under CI's
/// `cargo test --workspace --lib` gate.
pub mod executor_deploy;
