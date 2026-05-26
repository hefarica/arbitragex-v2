//! Strategy engines — one module per strategy family.
//!
//! Each engine converts an `ImpactSet` + `RouteIntent` into a
//! `Vec<StrategyCandidate>` — structured candidate records ready for
//! `ConfigAwareEvaluator` scoring and `OpportunityEmitter` dispatch.
//!
//! ## Module layout
//!
//! | Engine              | Phase | Strategy        |
//! |---------------------|-------|-----------------|
//! | `dex_engine`        |  8    | DEX arb V2/V3   |
//! | `triangular_engine` |  9    | Triangular arb  |
//! | `flashloan_engine`  | 10    | Flashloan wrap  |
//! | `liquidation_engine`| 11    | Liquidation     |
//!
//! ## `StrategyCandidate` ownership
//!
//! The shared output type lives in `engines/candidate.rs` and is re-exported
//! here so callers import from `crate::engines::StrategyCandidate`.
//! Individual engine modules re-import it from `super::StrategyCandidate`.

mod candidate;
pub use candidate::StrategyCandidate;

pub mod dex_engine;
pub mod flashloan_engine;
pub mod liquidation_engine;
pub mod triangular_engine;
// Experimental/legacy engines are kept behind an explicit feature because
// their candidate/route DTOs currently target an older prioritization-spine
// shape. The production orchestrator uses the four stable engines above.
#[cfg(feature = "experimental-engines")]
pub mod backrun_engine;
#[cfg(feature = "experimental-engines")]
pub mod spatial_engine;
#[cfg(feature = "experimental-engines")]
pub mod cex_dex_engine;
// APEX Phase 1.5 — Thermodynamic engines (server-side only)
#[cfg(feature = "experimental-engines")]
pub mod svs_engine;
#[cfg(feature = "experimental-engines")]
pub mod dlp_engine;
#[cfg(feature = "experimental-engines")]
pub mod triangular_atomic_engine;
#[cfg(feature = "experimental-engines")]
pub mod funding_rate_engine;
