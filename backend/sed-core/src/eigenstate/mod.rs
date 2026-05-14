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
}
