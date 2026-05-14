//! Phase 6 — Hedger & Holonomic Resolution
//!
//! Último eslabón del SED. Compuesto de tres sub-módulos:
//!
//! 1. `orthogonal_variance_hedger`: Entrelazamiento DEX↔CEX, covarianza nula.
//! 2. `holonomic_loop_resolution`: Búsqueda topológica de lazos de resolución holonómica.
//! 3. `temporal_liquidity_superposition`: Flash-loan / LP borrowing atómico.
//!
//! Consume:
//! - `DiracImpulse` desde Phase 5 (Allocator)
//! - `LiquidityManifold` desde Phase 5
//! - `AllocationResult` desde Phase 5 (bundle + impulse + ocp_solution)
//! - `GateVerdict::Dispatch(bundle)` desde Phase 4
//!
//! Produce:
//! - `OrthogonalHedgeResult` con covarianza nula certificada
//! - `ClosedContourTrajectory` con holonomía calculada
//! - `BundlePosition<HolonomicLoopResolution>` sellado

pub mod orthogonal_variance_hedger;
pub mod holonomic_loop_resolution;
pub mod temporal_liquidity_superposition;

pub use orthogonal_variance_hedger::*;
pub use holonomic_loop_resolution::*;
pub use temporal_liquidity_superposition::*;
