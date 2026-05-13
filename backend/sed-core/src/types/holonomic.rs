//! Closed-contour trajectory + topological yield types (V1.2 amendment,
//! ANEXOS_V1.2.md §4.1.3–§4.1.4).
//!
//! These two types are the mathematical evidence that the
//! `BundlePosition<HolonomicLoopResolution>` constructor consumes:
//!
//! - [`ClosedContourTrajectory`] — γ: [0,1] → ℳ with γ(0) = γ(1).
//!   Models a multi-pool atomic arbitrage cycle (ℒ₁ → ℒ₂ → … → ℒₙ → ℒ₁).
//! - [`TopologicalYield`] — `Y_topo = ∮_γ (dp/p) − F_net`. The raw
//!   holonomy minus the total network friction. Must be strictly
//!   positive for the cycle to be economically viable.
//!
//! Neither type is sealed — downstream crates can construct them
//! freely. The sealing happens one level up at the
//! [`BundlePosition<HolonomicLoopResolution>`] constructor, which
//! receives both as references and enforces the 6 cross-cutting
//! invariants (closed, ≥3 manifolds, non-trivial holonomy,
//! contour↔yield consistency, viable net yield, friction consistency).
//!
//! [`BundlePosition<HolonomicLoopResolution>`]: crate::types::bundle_position::BundlePosition

use nalgebra::DVector;

use crate::eigenstate::LiquidityManifold;

/// Closed-contour trajectory over N ≥ 3 liquidity manifolds.
///
/// **Topological constraint**: γ(0) = γ(1) — the initial and final
/// transition points coincide in ℝᵈ within tolerance `1e-9` per
/// [`Self::is_closed`].
///
/// **Cardinality constraint**: `loop_cardinality ≥ MIN_LOOP_SIZE` (= 3).
/// A two-pool round trip is a degenerate case handled by the
/// `DiracImpulseOnly` typestate, not the holonomic one.
///
/// **Field invariants**:
/// - `manifolds.len() == loop_cardinality`.
/// - `transition_points.len() == loop_cardinality + 1` (start point
///   appears twice: at index 0 and index `loop_cardinality`). The
///   `is_closed` check compares the first and last entries.
/// - All `transition_points[i]` share the same dimension (the manifold
///   embedding dimension `d`). Mismatched dims cause `(f - l).norm()` to
///   panic on subtraction — callers should enforce dim consistency up
///   front; the runtime panic is intentional fail-fast.
/// - `relative_prices.len() == loop_cardinality`. Each entry is the
///   ratio `pᵢ₊₁ / pᵢ` at the i-th transition (pre-friction).
pub struct ClosedContourTrajectory {
    /// Variedades de liquidez en orden de recorrido del ciclo.
    pub manifolds: Vec<LiquidityManifold>,

    /// Puntos de transición entre variedades (puntos de contacto). The
    /// last entry equals the first when [`Self::is_closed`] is true.
    pub transition_points: Vec<DVector<f64>>,

    /// Precios relativos en cada arista del ciclo: `pᵢ₊₁ / pᵢ`.
    /// `contour_integral` computes `Σᵢ ln(pᵢ)`.
    pub relative_prices: Vec<f64>,

    /// Longitud total del contorno según métrica g. Geometric length on
    /// the manifold; not used by the typestate invariant but exposed for
    /// downstream metric / variance computations.
    pub contour_length: f64,

    /// Número de variedades en el lazo (≥ 3).
    pub loop_cardinality: usize,
}

impl ClosedContourTrajectory {
    /// Minimum manifold count for a non-degenerate holonomic loop.
    ///
    /// **Why 3**: A 2-pool round trip is the degenerate "buy here, sell
    /// there" case, which is already covered by the `DiracImpulseOnly`
    /// JIT-impulse path. A holonomic loop materially exists only when
    /// the contour encloses non-trivial topology — i.e., at least one
    /// inner manifold the trajectory wraps around. The lower bound of 3
    /// is the smallest such configuration (triangle).
    pub const MIN_LOOP_SIZE: usize = 3;

    /// Verifica que el contorno sea cerrado: γ(0) = γ(1) en ℝᵈ con
    /// tolerancia `1e-9`. Returns `false` if either:
    ///
    /// - `manifolds.len() < MIN_LOOP_SIZE`, or
    /// - `transition_points` is empty / missing either endpoint, or
    /// - `‖first − last‖₂ ≥ 1e-9`.
    ///
    /// The closure check is one of two pillars of the holonomic
    /// invariant (the other being `contour_integral() != 0`).
    pub fn is_closed(&self) -> bool {
        if self.manifolds.len() < Self::MIN_LOOP_SIZE {
            return false;
        }
        let first = self.transition_points.first();
        let last = self.transition_points.last();
        match (first, last) {
            (Some(f), Some(l)) if f.len() == l.len() => (f - l).norm() < 1e-9,
            // Mismatched dims or empty list → degenerate, not closed.
            _ => false,
        }
    }

    /// Calcula la integral de contorno de precios relativos:
    /// ∮_γ (dp/p) = Σᵢ ln(pᵢ).
    ///
    /// **Numerical note**: each `pᵢ` is the ratio `pᵢ₊₁/pᵢ`, so any
    /// non-positive entry yields `NaN` (via `(-x).ln()`) or `-inf` (via
    /// `0.0.ln()`). The constructor in
    /// [`BundlePosition<HolonomicLoopResolution>`] does NOT pre-validate
    /// price positivity — callers are expected to feed pre-validated
    /// data from the price oracle. A non-finite holonomy will fail the
    /// `holonomy.abs() > 1e-12` validation downstream, surfacing as
    /// `TrivialHolonomy` because `NaN > 1e-12` is `false`. That is the
    /// desired fail-honest outcome.
    ///
    /// [`BundlePosition<HolonomicLoopResolution>`]: crate::types::bundle_position::BundlePosition
    pub fn contour_integral(&self) -> f64 {
        self.relative_prices.iter().map(|p| p.ln()).sum()
    }
}

/// Topological yield = raw holonomy minus total network friction.
///
/// **Identity**:
/// `net_yield = raw_holonomy − network_friction`
///
/// **Viability**: `net_yield > MINIMUM_VIABLE_YIELD` (= 1e-15). The
/// floor is intentionally absurd-small — it filters out exactly-zero
/// and negative profits, while letting the operator's higher-level
/// profit-gate (CLAUDE.md §24 "Gas Protection: net ≥ 3× gas") enforce
/// the real economic threshold one layer up.
pub struct TopologicalYield {
    /// Valor bruto de la holonomía: ∮_γ (dp/p) ≠ 0. Must agree with
    /// `ClosedContourTrajectory::contour_integral()` to within `1e-9`
    /// inside the holonomic constructor (validation 4).
    pub raw_holonomy: f64,

    /// Costo de fricción de red (gas fees + slippage + LP fees) en
    /// la misma unidad log-precio que `raw_holonomy`. The contour
    /// integral lives in log-price space, so the friction MUST be
    /// pre-converted to that space by the caller (typically
    /// `ln(1 + cost_usd / notional_usd)` for small costs).
    pub network_friction: f64,

    /// Rendimiento neto: `raw_holonomy − network_friction`.
    /// INVARIANT (validation 5): `net_yield > MINIMUM_VIABLE_YIELD`.
    pub net_yield: f64,

    /// Factor de eficiencia: `net_yield / raw_holonomy`. Set to 0.0
    /// when `raw_holonomy.abs() < 1e-12` to avoid div-by-zero; the
    /// upstream `is_nontrivial_holonomy` check rejects that case
    /// before efficiency is consulted.
    pub efficiency_factor: f64,

    /// IDs (lower-cased pool addresses) of the manifolds traversed. Used
    /// by downstream metrics + dashboards (`sed_holonomic_yield`
    /// gauge label `manifolds`). MUST match the order of
    /// `ClosedContourTrajectory::manifolds` 1:1 — the constructor does
    /// not enforce this (it would require an extra allocation) but
    /// runtime telemetry assumes parity.
    pub manifold_ids: Vec<String>,

    /// Wall-clock unix timestamp (seconds) when the yield calculation
    /// resolved. Used by the half-life check (Bloque 3 invariant #5)
    /// once the dispatcher lands in Phase 4.
    pub resolved_at: u64,
}

impl TopologicalYield {
    /// Floor on viable net yield. Below this, the cycle is treated as
    /// economically dead — either truly zero-profit or in numerical
    /// noise where rounding alone can flip sign.
    pub const MINIMUM_VIABLE_YIELD: f64 = 1e-15;

    /// `true` iff `net_yield > MINIMUM_VIABLE_YIELD`. NaN net yields
    /// return `false` (NaN > anything is `false`), which is the
    /// fail-honest answer.
    pub fn is_economically_viable(&self) -> bool {
        self.net_yield > Self::MINIMUM_VIABLE_YIELD
    }

    /// `true` iff `|raw_holonomy| > 1e-12`. Mirrors the validation
    /// 3 threshold inside the holonomic constructor.
    pub fn is_nontrivial_holonomy(&self) -> bool {
        self.raw_holonomy.abs() > 1e-12
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dvec(coords: &[f64]) -> DVector<f64> {
        DVector::from_row_slice(coords)
    }

    fn manifold(id: &str) -> LiquidityManifold {
        LiquidityManifold::new(id)
    }

    // ── ClosedContourTrajectory ──────────────────────────────────────

    #[test]
    fn three_manifold_loop_with_matching_endpoints_is_closed() {
        let c = ClosedContourTrajectory {
            manifolds: vec![manifold("0xA"), manifold("0xB"), manifold("0xC")],
            transition_points: vec![dvec(&[1.0, 0.0]), dvec(&[0.0, 1.0]), dvec(&[1.0, 1.0]), dvec(&[1.0, 0.0])],
            relative_prices: vec![1.01, 1.02, 0.97],
            contour_length: 3.0,
            loop_cardinality: 3,
        };
        assert!(c.is_closed());
    }

    #[test]
    fn fewer_than_three_manifolds_is_not_closed_even_if_endpoints_match() {
        let c = ClosedContourTrajectory {
            manifolds: vec![manifold("0xA"), manifold("0xB")],
            transition_points: vec![dvec(&[0.0, 0.0]), dvec(&[1.0, 0.0]), dvec(&[0.0, 0.0])],
            relative_prices: vec![1.01, 0.99],
            contour_length: 2.0,
            loop_cardinality: 2,
        };
        assert!(!c.is_closed(), "MIN_LOOP_SIZE=3 must reject 2-pool round trip");
    }

    #[test]
    fn endpoint_mismatch_above_tolerance_is_not_closed() {
        let c = ClosedContourTrajectory {
            manifolds: vec![manifold("0xA"), manifold("0xB"), manifold("0xC")],
            transition_points: vec![dvec(&[0.0, 0.0]), dvec(&[1.0, 0.0]), dvec(&[1.0, 1.0]), dvec(&[1e-3, 0.0])],
            relative_prices: vec![1.01, 1.02, 0.97],
            contour_length: 3.0,
            loop_cardinality: 3,
        };
        assert!(!c.is_closed(), "1e-3 separation is above the 1e-9 tolerance");
    }

    #[test]
    fn mismatched_dim_endpoints_are_not_closed() {
        let c = ClosedContourTrajectory {
            manifolds: vec![manifold("0xA"), manifold("0xB"), manifold("0xC")],
            transition_points: vec![dvec(&[0.0, 0.0]), dvec(&[1.0]), dvec(&[1.0, 1.0]), dvec(&[0.0, 0.0, 0.0])],
            relative_prices: vec![1.01, 1.02, 0.97],
            contour_length: 3.0,
            loop_cardinality: 3,
        };
        assert!(!c.is_closed(), "mismatched dim → not closed (no panic)");
    }

    #[test]
    fn contour_integral_sums_log_prices() {
        let c = ClosedContourTrajectory {
            manifolds: vec![manifold("0xA"), manifold("0xB"), manifold("0xC")],
            transition_points: vec![dvec(&[0.0]), dvec(&[0.0]), dvec(&[0.0]), dvec(&[0.0])],
            relative_prices: vec![1.01_f64, 1.02_f64, 0.97_f64],
            contour_length: 0.0,
            loop_cardinality: 3,
        };
        let expected = 1.01_f64.ln() + 1.02_f64.ln() + 0.97_f64.ln();
        assert!((c.contour_integral() - expected).abs() < 1e-15);
    }

    // ── TopologicalYield ─────────────────────────────────────────────

    #[test]
    fn viable_yield_above_floor() {
        let y = TopologicalYield {
            raw_holonomy: 0.01,
            network_friction: 0.001,
            net_yield: 0.009,
            efficiency_factor: 0.9,
            manifold_ids: vec!["0xa".into(), "0xb".into(), "0xc".into()],
            resolved_at: 0,
        };
        assert!(y.is_economically_viable());
        assert!(y.is_nontrivial_holonomy());
    }

    #[test]
    fn yield_at_floor_is_not_viable() {
        let y = TopologicalYield {
            raw_holonomy: 1e-15,
            network_friction: 0.0,
            net_yield: TopologicalYield::MINIMUM_VIABLE_YIELD,
            efficiency_factor: 1.0,
            manifold_ids: vec!["0xa".into()],
            resolved_at: 0,
        };
        // Strictly greater-than gate; equality fails.
        assert!(!y.is_economically_viable());
    }

    #[test]
    fn nan_net_yield_is_not_viable() {
        let y = TopologicalYield {
            raw_holonomy: 0.01,
            network_friction: 0.0,
            net_yield: f64::NAN,
            efficiency_factor: 0.0,
            manifold_ids: vec!["0xa".into()],
            resolved_at: 0,
        };
        assert!(!y.is_economically_viable(), "NaN > x is false — fail-honest");
    }

    #[test]
    fn near_zero_holonomy_is_trivial() {
        let y = TopologicalYield {
            raw_holonomy: 1e-13,
            network_friction: 0.0,
            net_yield: 1e-13,
            efficiency_factor: 1.0,
            manifold_ids: vec!["0xa".into()],
            resolved_at: 0,
        };
        assert!(!y.is_nontrivial_holonomy());
    }
}
