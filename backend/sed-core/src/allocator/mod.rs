//! `allocator` — Dirac Manifold Allocator (spec §3.3, Phase 5).
//!
//! ## Architecture
//!
//! The allocator resolves the Optimal Control Problem (OCP) on liquidity
//! manifolds using the Pontryagin Maximum Principle with hyperbolic
//! constraint x·y = k.
//!
//! It consumes:
//! - `EquilibriumBoundary` from Phase 3 (eigenstate)
//! - `LiquidityManifold` from on-chain CPMM state
//! - `GateVerdict::Dispatch(bundle)` from Phase 4 (GateManager)
//!
//! It produces:
//! - `OptimalControlSolution` with the optimal control trajectory
//! - `DiracImpulse` at the topological injection point
//! - `BundlePosition<DiracImpulseOnly>` sealed for dispatch
//!
//! ## Feature Gate
//!
//! The full allocator math (Newton-Raphson solver, DiracImpulse,
//! LiquidityManifold) compiles behind `--features allocator`.
//! The `OptimalControlSolution` type is always available because
//! `bundle_position.rs` references it unconditionally for the
//! `DiracImpulseOnly` typestate constructor.

// ── Always-available types (no feature gate) ──────────────────────────
// OptimalControlSolution must be importable unconditionally because
// bundle_position.rs uses it.

#[cfg(not(feature = "allocator"))]
mod stub {
    /// Stub of the post-resolution proof type required by
    /// [`crate::types::bundle_position::BundlePosition::new_dirac_impulse_only`].
    #[derive(Debug, Clone)]
    pub struct OptimalControlSolution {
        /// `true` when the Dirac impulse preserves `x · y = k`.
        pub hyperbolic_constraint_satisfied: bool,
    }

    impl OptimalControlSolution {
        /// Test-only constructor.
        #[doc(hidden)]
        pub fn stub(hyperbolic_ok: bool) -> Self {
            Self {
                hyperbolic_constraint_satisfied: hyperbolic_ok,
            }
        }
    }
}

#[cfg(not(feature = "allocator"))]
pub use stub::OptimalControlSolution;

// ── Full Phase 5 submodules (feature-gated) ───────────────────────────

#[cfg(feature = "allocator")]
pub mod hyperbolic_constraint;

#[cfg(feature = "allocator")]
pub mod liquidity_manifold;

#[cfg(feature = "allocator")]
pub mod optimal_control;

#[cfg(feature = "allocator")]
pub mod dirac_manifold_allocator;

// ── Re-exports when allocator feature is active ───────────────────────

#[cfg(feature = "allocator")]
pub use hyperbolic_constraint::HyperbolicConstraint;

#[cfg(feature = "allocator")]
pub use liquidity_manifold::{LiquidityManifold, ManifoldError};

#[cfg(feature = "allocator")]
pub use optimal_control::{OptimalControlSolution, OcpError, PontryaginSolver};

#[cfg(feature = "allocator")]
pub use dirac_manifold_allocator::{
    AllocationError, AllocationResult, DiracImpulse, DiracManifoldAllocator,
};
