//! `allocator` — Dirac manifold allocator (spec §3.3).
//!
//! **Phase 1 status**: only the proof-bearing types referenced by
//! `bundle_position::DiracImpulseOnly` are stubbed. The full optimal-control
//! solver (Pontryagin Hamiltonian, hyperbolic constraint resolver,
//! pseudospectral collocation) lands in Phase 2 with its own `argmin`
//! dependency.

/// Stub of the post-resolution proof type required by
/// [`crate::types::bundle_position::BundlePosition::new_dirac_impulse_only`].
///
/// In Phase 2 this gets a full `DVector` `optimal_control`, costate
/// trajectory, `DiracImpulse`, etc., per spec §3.3.3. For Phase 1 we expose
/// only the field the typestate constructor reads — `hyperbolic_constraint_satisfied` —
/// so the type-system invariant compiles end-to-end and tests can assert
/// the typestate behaviour without a real OCP solver.
#[derive(Debug, Clone)]
pub struct OptimalControlSolution {
    /// `true` when the Dirac impulse preserves `x · y = k`.
    pub hyperbolic_constraint_satisfied: bool,
}

impl OptimalControlSolution {
    /// Test-only constructor that produces a solution with the
    /// hyperbolic flag set to the requested value. Marked `#[cfg(test)]`
    /// in dependent crates would be preferable, but Phase 1's
    /// `bundle_position` unit tests live in this same crate so we expose
    /// it unconditionally and rely on `clippy::dead_code` to surface it
    /// when Phase 2's real constructor lands.
    #[doc(hidden)]
    pub fn stub(hyperbolic_ok: bool) -> Self {
        Self {
            hyperbolic_constraint_satisfied: hyperbolic_ok,
        }
    }
}
