//! Strategy engines — one module per strategy family.
//!
//! Each engine converts an `ImpactSet` + `RouteIntent` into a
//! `Vec<StrategyCandidate>` — structured candidate records ready for
//! `ConfigAwareEvaluator` scoring and `OpportunityEmitter` dispatch.
//!
//! ## Module layout (Phase 7-8 skeleton)
//!
//! Only `dex_engine` ships in Phase 7-8. The remaining engines are
//! declared but commented out; they are filled in by Phases 9-11.
//!
//! | Engine             | Phase | Strategy       |
//! |--------------------|-------|----------------|
//! | `dex_engine`       |  8    | DEX arb V2/V3  |
//! | `triangular_engine`|  9    | Triangular arb |
//! | `flashloan_engine` | 10    | Flashloan wrap |
//! | `liquidation_engine`| 11   | Liquidation    |
//!
//! ## Rule: compile with only dex_engine present
//!
//! The `engines/` directory MUST compile cleanly with only `dex_engine.rs`
//! present (no triangular/flashloan/liquidation). The other modules stay
//! commented out until their respective phases ship.

pub mod dex_engine;
// pub mod triangular_engine; — Phase 9
// pub mod flashloan_engine;  — Phase 10
// pub mod liquidation_engine; — Phase 11
