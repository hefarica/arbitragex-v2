//! `hedger` — orthogonal variance hedger (spec §3.4).
//!
//! **Phase 1 status**: only the proof-bearing type referenced by
//! `bundle_position::OrthogonalEquilibrium` is stubbed. The full entangled
//! state synchroniser (covariance engine, orthogonal hyperplane builder)
//! lands in Phase 2 with `nalgebra` + `num-complex` dependencies.

/// Stub of the post-resolution proof type required by
/// [`crate::types::bundle_position::BundlePosition::new_orthogonal_equilibrium`].
///
/// In Phase 2 this gets a full hedged state, covariance matrix, residual
/// direction, orthogonality error, etc., per spec §3.4.3. For Phase 1 we
/// expose only the method the typestate constructor calls —
/// `verify_null_covariance(tolerance)` — so the type-system invariant
/// compiles end-to-end and tests assert the typestate behaviour without
/// a real covariance solver.
#[derive(Debug, Clone)]
pub struct OrthogonalHedgeResult {
    /// `true` when the covariance matrix's off-diagonal is below the
    /// caller-supplied tolerance and the residual direction is short
    /// enough that null-covariance is asserted.
    null_covariance: bool,
}

impl OrthogonalHedgeResult {
    /// Check the null-covariance invariant. Phase 1 stub: returns the
    /// stored `null_covariance` flag and ignores `_tolerance`. Phase 2
    /// will compute the actual covariance and compare off-diagonal terms
    /// against `_tolerance` per the spec.
    pub fn verify_null_covariance(&self, _tolerance: f64) -> bool {
        self.null_covariance
    }

    /// Test-only constructor. See `allocator::OptimalControlSolution::stub`
    /// for the rationale on exposing this unconditionally during Phase 1.
    #[doc(hidden)]
    pub fn stub(null_cov: bool) -> Self {
        Self {
            null_covariance: null_cov,
        }
    }
}
