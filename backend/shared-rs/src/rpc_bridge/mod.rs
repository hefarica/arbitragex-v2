//! rpc_bridge — neutral RPC layer for the ethers↔alloy dual-track migration.
//!
//! Macro plan: `docs/plans/alloy-parallel-path-macro-plan.md` (§2-§3).
//!
//! Layout:
//! - [`traits`]          — backend-agnostic contracts (FASE 0). Services code
//!   against these; they mention neither ethers nor alloy.
//! - [`alloy_backend`]   — path B, alloy 1.x implementation (FASE 1).
//! - [`ethers_backend`]  — path A, wraps the production ethers stack (FASE 2).
//!
//! Selection is runtime-toggleable (`arbx:rpc_backend:<service>` in Redis →
//! [`BackendSelection`]), with ethers — the production path — as the fail-safe
//! default. FASE 3 (shadow comparison) and beyond build on this crate without
//! touching either backend's internals.
//!
//! Nothing in this module is wired into any service yet: adding it changed no
//! existing behavior (macro plan FASE 0/1/2 risk profile).

pub mod alloy_backend;
pub mod ethers_backend;
pub mod traits;

pub use traits::*;
