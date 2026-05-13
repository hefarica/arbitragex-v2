//! `sed-core::prelude` — opinionated re-export surface for downstream crates.
//!
//! Downstream consumers (`searcher-rs`, `sim-ctl`, `relays-client`, `recon`)
//! typically need:
//!
//! - The typestate marker types + sealed trait (to construct
//!   [`BundlePosition`] values).
//! - The kill-switch / infrastructure gates.
//! - The canonical error enums.
//!
//! Importing `use sed_core::prelude::*;` brings these into scope without
//! pulling in math-bearing types prematurely.

pub use crate::types::bundle_position::{
    BundlePosition, DiracImpulseOnly, OrthogonalEquilibrium, PostResolutionTopology, Resolved,
    Unresolved,
};
pub use crate::types::errors::{DispatchError, InfrastructureError, TopologyValidationError};
pub use crate::types::infrastructure::{InfrastructurePrerequisite, NotImplementedResponse};
pub use crate::types::kill_switch::{KillSwitchGate, KillSwitchState};
