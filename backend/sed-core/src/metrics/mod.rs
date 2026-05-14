//! `metrics` — SED Pipeline observability metrics.
//!
//! Feature-gated under `metrics`. Provides a lightweight metrics
//! recording API that can be backed by any metrics sink (Prometheus,
//! StatsD, in-memory for tests).
//!
//! ## Design Decision
//!
//! We use a trait-based approach rather than the `metrics` crate directly
//! so that sed-core remains independent of any specific metrics runtime.
//! The api-server or orchestrator layer provides the concrete implementation.

#[cfg(feature = "metrics")]
pub mod sed_metrics;

#[cfg(feature = "metrics")]
pub use sed_metrics::*;
