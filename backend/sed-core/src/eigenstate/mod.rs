//! `eigenstate` — eigenstate transition projector (spec §3.2, Phase 3).
//!
//! ## Phase 1 (V1.2 amended, 2026-05-13)
//!
//! Scaffold + opaque [`LiquidityManifold`] placeholder. Introduced
//! ahead of schedule because the V1.2 holonomic typestate
//! ([`crate::types::holonomic::ClosedContourTrajectory`]) references
//! it by composition — the closed-contour trajectory walks over a
//! sequence of manifolds.
//!
//! ## Phase 3 (2026-05-13) — Eigenstate Decomposition
//!
//! Three submodules graduate behind the `eigenstate` Cargo feature:
//!
//! - [`effective_hamiltonian`] — constructs the N×N symmetric Hamiltonian
//!   of the liquidity manifold network. Diagonal = self-energy (TVL × σ),
//!   off-diagonal = inter-manifold coupling. The CDC from Phase 2 enters
//!   as a diagonal perturbation.
//!
//! - [`lanczos_solver`] — eigendecomposition via nalgebra's `SymmetricEigen`
//!   (implicit QR, O(N³)). Returns sorted eigenvalues, eigenvectors,
//!   spectral gap, and ground-state participation ratio. Lanczos iterative
//!   path reserved for N > 50 manifolds (Phase 3.1).
//!
//! - [`transition_projector`] — computes ground-state transition
//!   probabilities under CDC perturbation. The `should_dispatch()` method
//!   is the O(1) gate that the GateManager (Phase 4) will call on every
//!   candidate bundle. When `--features "filtration eigenstate"` is active,
//!   the projector directly consumes `StateDivergenceCoefficient` from
//!   Phase 2's `CdcCalculator`.
//!
//! ## Coupling to Phase 2 (Filtration)
//!
//! The transition projector's `project_from_cdc()` method is conditionally
//! compiled under `#[cfg(feature = "filtration")]`. This means the full
//! Phase 2 → Phase 3 pipeline is available when both features are active:
//!
//! ```text
//! PdmpEstimator (Phase 2)
//!   → CdcCalculator::compute() → StateDivergenceCoefficient
//!     → TransitionProjector::project_from_cdc()
//!       → TransitionProjection::should_dispatch()  ← O(1) gate
//! ```

use serde::{Deserialize, Serialize};

/// Opaque identifier for a liquidity manifold (a single CPMM / V3 pool /
/// stable pool / weighted pool surface).
///
/// **Phase 1 surface**: only the `id` is materialised. Phase 3 attaches
/// the manifold's self-energy and coupling parameters via the
/// [`EffectiveHamiltonian`](effective_hamiltonian::EffectiveHamiltonian).
///
/// The struct is intentionally minimal so the V1.2 typestate amendment
/// can name the type without dragging in the eigenstate math kernel.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct LiquidityManifold {
    /// Stable identifier — typically the pool address as `0x…` lower-case
    /// hex (chain-prefixed if the bundle spans chains). Used as the
    /// composition key in [`ClosedContourTrajectory.manifolds`] and as
    /// the cardinality witness for the loop-size invariant.
    ///
    /// [`ClosedContourTrajectory.manifolds`]: crate::types::holonomic::ClosedContourTrajectory
    pub id: String,
}

impl LiquidityManifold {
    /// Constructor that normalises the id to lower-case so two callers
    /// that disagree on hex case (`0xAbCd` vs `0xabcd`) collapse to the
    /// same manifold. Pool addresses are case-insensitive on every EVM
    /// chain the platform targets; this avoids a class of "looks closed
    /// but isn't" bugs in the holonomic invariant check.
    pub fn new<S: Into<String>>(id: S) -> Self {
        Self {
            id: id.into().to_lowercase(),
        }
    }
}

// ── Phase 3 submodules (feature-gated) ────────────────────────────────

#[cfg(feature = "eigenstate")]
pub mod effective_hamiltonian;

#[cfg(feature = "eigenstate")]
pub mod lanczos_solver;

#[cfg(feature = "eigenstate")]
pub mod transition_projector;

// ── Phase 3 re-exports ────────────────────────────────────────────────

#[cfg(feature = "eigenstate")]
pub use effective_hamiltonian::{EffectiveHamiltonian, HamiltonianError};

#[cfg(feature = "eigenstate")]
pub use lanczos_solver::{EigenstateDecomposition, LanczosError, LANCZOS_THRESHOLD};

#[cfg(feature = "eigenstate")]
pub use transition_projector::{
    ProjectionError, TransitionProjection, TransitionProjector,
};

// ── Phase 5 bridge types (allocator consumes these from eigenstate) ───

use nalgebra::DVector;

/// Equilibrium boundary hypersurface from eigenstate decomposition.
///
/// The boundary represents the set of reserve configurations where the
/// network's ground-state eigenvalue crosses a critical threshold.
/// Phase 5's Pontryagin solver uses this as the target set ∂Ω_eq for
/// the optimal control problem.
#[derive(Debug, Clone, PartialEq)]
pub struct EquilibriumBoundary {
    /// Points sampling the boundary hypersurface in reserve-space ℝⁿ.
    pub boundary_hypersurface: Vec<DVector<f64>>,
    /// Critical probability threshold for the boundary (typically 0.95).
    pub critical_probability: f64,
    /// Convergence radius: maximum geodesic distance from the boundary
    /// within which the OCP solver will accept targets.
    pub convergence_radius: f64,
    /// Stability exponent: decay rate of perturbations near the boundary.
    pub stability_exponent: f64,
}

impl EquilibriumBoundary {
    /// Check if a point lies within the convergence radius of the boundary.
    pub fn contains(&self, point: &DVector<f64>) -> bool {
        if self.convergence_radius.is_infinite() {
            return true;
        }
        self.boundary_hypersurface.iter().any(|bp| {
            let diff = bp - point;
            diff.norm() <= self.convergence_radius
        })
    }
}

/// Eigenstate of the liquidity network Hamiltonian.
///
/// Carries the amplitude vector (complex-valued for generality),
/// energy eigenvalue, degeneracy, and quantum numbers.
/// Phase 5's allocator extracts the target reserve configuration
/// from the ground-state eigenvector.
#[cfg(feature = "allocator")]
#[derive(Debug, Clone, PartialEq)]
pub struct EigenState {
    /// Complex amplitude vector of the eigenstate.
    pub amplitude: DVector<num_complex::Complex64>,
    /// Energy eigenvalue.
    pub energy: f64,
    /// Degeneracy count (1 = non-degenerate).
    pub degeneracy: usize,
    /// Quantum numbers labeling this state.
    pub quantum_numbers: Vec<i64>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn manifold_id_is_normalised_to_lowercase() {
        let m = LiquidityManifold::new("0xAbCdEf0123456789aBcDeF0123456789aBcDeF01");
        assert_eq!(m.id, "0xabcdef0123456789abcdef0123456789abcdef01");
    }

    #[test]
    fn manifold_equality_is_case_insensitive_via_constructor() {
        let a = LiquidityManifold::new("0xAAA");
        let b = LiquidityManifold::new("0xaaa");
        assert_eq!(a, b);
    }

    #[test]
    fn equilibrium_boundary_contains_within_radius() {
        let boundary = EquilibriumBoundary {
            boundary_hypersurface: vec![DVector::from(vec![1000.0, 1000.0])],
            critical_probability: 0.95,
            convergence_radius: 100.0,
            stability_exponent: 0.0,
        };
        let close = DVector::from(vec![1050.0, 1050.0]);
        let far = DVector::from(vec![2000.0, 2000.0]);
        assert!(boundary.contains(&close));
        assert!(!boundary.contains(&far));
    }

    #[test]
    fn equilibrium_boundary_infinite_radius_contains_everything() {
        let boundary = EquilibriumBoundary {
            boundary_hypersurface: vec![DVector::from(vec![0.0, 0.0])],
            critical_probability: 0.95,
            convergence_radius: f64::INFINITY,
            stability_exponent: 0.0,
        };
        let far = DVector::from(vec![1e12, 1e12]);
        assert!(boundary.contains(&far));
    }
}

