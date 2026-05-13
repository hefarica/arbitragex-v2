//! `eigenstate` — eigenstate transition projector (spec §3.2).
//!
//! **Phase 1 status (V1.2 amended, 2026-05-13)**: scaffold + opaque
//! [`LiquidityManifold`] placeholder. The Lanczos solver, effective
//! Hamiltonian, transition projector, and equilibrium boundary land in
//! Phase 3 with their own `num-complex` dependency. `LiquidityManifold`
//! is introduced here ahead of schedule because the V1.2 holonomic
//! typestate ([`crate::types::holonomic::ClosedContourTrajectory`])
//! references it by composition — the closed-contour trajectory walks
//! over a sequence of manifolds.

use serde::{Deserialize, Serialize};

/// Opaque identifier for a liquidity manifold (a single CPMM / V3 pool /
/// stable pool / weighted pool surface).
///
/// **Phase 1 surface**: only the `id` is materialised. Downstream Phase 3
/// will attach the manifold's Riemannian metric tensor `g_ij`, the
/// curvature scalar `R`, and the geodesic transition operator.
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
