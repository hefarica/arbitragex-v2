//! `hedger` — Orthogonal Variance Hedger + Holonomic Loop Resolution (Phase 6).
//!
//! ## Architecture
//!
//! Phase 6 will implement two remaining topological resolutions:
//!
//! 1. **Orthogonal Variance Hedger**: Computes inverse compensation vectors
//!    in orthogonal venues to guarantee null aggregate covariance.
//!
//! 2. **Holonomic Loop Resolution**: Bellman-Ford graph search for closed
//!    contour trajectories across N ≥ 3 manifolds.
//!
//! ## Current status
//!
//! `OrthogonalHedgeResult` is available unconditionally for
//! `bundle_position.rs` backward compatibility. The full covariance engine
//! and graph searcher are pending Phase 6 implementation.

// ── Always-available types (no feature gate) ──────────────────────────
// OrthogonalHedgeResult must be importable unconditionally because
// bundle_position.rs uses it.

/// Post-resolution proof type required by
/// `BundlePosition::new_orthogonal_equilibrium`.
///
/// Phase 6 will replace this with a full covariance engine.
/// Currently provides the `verify_null_covariance` gate and
/// a test-only `stub()` constructor.
#[derive(Debug, Clone)]
pub struct OrthogonalHedgeResult {
    /// `true` when the covariance matrix's off-diagonal is below
    /// tolerance and the residual direction is short enough.
    null_covariance: bool,
}

impl OrthogonalHedgeResult {
    /// Check the null-covariance invariant.
    pub fn verify_null_covariance(&self, _tolerance: f64) -> bool {
        self.null_covariance
    }

    /// Test-only constructor.
    #[doc(hidden)]
    pub fn stub(null_cov: bool) -> Self {
        Self {
            null_covariance: null_cov,
        }
    }
}

// Phase 6 submodules will be added here when implementation begins:
// pub mod orthogonal_variance_hedger;
// pub mod holonomic_loop_resolution;
