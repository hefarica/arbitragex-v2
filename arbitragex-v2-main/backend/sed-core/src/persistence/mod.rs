//! `persistence` — Data Access Objects for SED pipeline telemetry.
//!
//! Feature-gated under `persistence`. Maps Rust types from Phases 1-6
//! into PostgreSQL tables `sed_filtrations`, `sed_eigenstates`,
//! `sed_allocations`, `sed_hedges` (migration 062).
//!
//! ## R8 Fail-Honest
//!
//! - All DAO methods return `Result`. No silent fallbacks.
//! - `status` defaults to `'PENDIENTE'`; callers update to `'OK'` or
//!   `'BLOQUEADO'` after GateManager evaluation.
//! - None/null means "not computed", never replaced with 0 or default.

#[cfg(feature = "persistence")]
pub mod dao;

#[cfg(feature = "persistence")]
pub use dao::*;

// Stub when persistence feature is off — module exists but is empty.
