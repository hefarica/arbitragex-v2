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
pub mod backrun_engine;
pub mod spatial_engine;
pub mod cex_dex_engine;
