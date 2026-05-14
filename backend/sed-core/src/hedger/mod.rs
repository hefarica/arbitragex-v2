//! Phase 6 — Hedger & Holonomic Resolution
//!
//! Último eslabón del SED. Compuesto de tres sub-módulos:
//!
//! 1. `orthogonal_variance_hedger`: Entrelazamiento DEX↔CEX, covarianza nula.
//! 2. `holonomic_loop_resolution`: Búsqueda topológica de ciclos de arbitraje.
//! 3. `temporal_liquidity_superposition`: Flash-loan / LP borrowing atómico.
//!
//! Consume:
//! - `DiracImpulse` desde Phase 5 (Allocator)
//! - `LiquidityManifold` desde Phase 5
//! - `GateVerdict::Dispatch` desde Phase 4
//!
//! Produce:
//! - `OrthogonalHedgeResult` con covarianza nula certificada
//! - `ClosedContourTrajectory` con holonomía calculada
//! - `BundlePosition<HolonomicLoopResolution>` sellado

// Phase 6 submodules — only compiled with the `hedger` feature.
#[cfg(feature = "hedger")]
pub mod orthogonal_variance_hedger;
#[cfg(feature = "hedger")]
pub mod holonomic_loop_resolution;
#[cfg(feature = "hedger")]
pub mod temporal_liquidity_superposition;

#[cfg(feature = "hedger")]
pub use orthogonal_variance_hedger::*;
#[cfg(feature = "hedger")]
pub use holonomic_loop_resolution::*;
#[cfg(feature = "hedger")]
pub use temporal_liquidity_superposition::*;

// ── Unconditional stub: OrthogonalHedgeResult ─────────────────────────
//
// The `BundlePosition<OrthogonalEquilibrium>` constructor (Phase 1) needs
// `OrthogonalHedgeResult` at all times. When the full Phase 6 is active
// (`hedger` feature), the real `OrthogonalHedgeResult` from
// `orthogonal_variance_hedger` is re-exported above and shadows this stub.
// When `hedger` is OFF, this minimal proof-of-hedge struct provides the
// `verify_null_covariance` API that the typestate constructor requires.

#[cfg(not(feature = "hedger"))]
mod stub {
    /// Stub proof-of-hedge result (Phase 1 backward compatibility).
    ///
    /// Replaced by the real `OrthogonalHedgeResult` when the `hedger`
    /// feature is active.
    #[derive(Debug, Clone, PartialEq)]
    pub struct OrthogonalHedgeResult {
        /// True only when the real hedger certifies null covariance.
        pub is_fully_neutralized: bool,
        /// Off-diagonal covariance entry (quick check).
        pub covariance_off_diagonal: f64,
    }

    impl OrthogonalHedgeResult {
        /// Phase 1 stub: caller supplies is_null manually.
        pub fn stub(is_null: bool) -> Self {
            Self {
                is_fully_neutralized: is_null,
                covariance_off_diagonal: if is_null { 0.0 } else { f64::NAN },
            }
        }

        /// Verifica covarianza nula dentro de tolerancia numérica.
        pub fn verify_null_covariance(&self, tolerance: f64) -> bool {
            self.covariance_off_diagonal.abs() < tolerance && self.is_fully_neutralized
        }
    }
}

#[cfg(not(feature = "hedger"))]
pub use stub::OrthogonalHedgeResult;
