//! Type-system primitives that anchor the SED's compile-time guarantees.
//!
//! Three submodules:
//!
//! - [`bundle_position`] — the typestate. Defines `BundlePosition<T>` and the
//!   sealed trait `PostResolutionTopology` whose only implementors are
//!   `OrthogonalEquilibrium` and `DiracImpulseOnly`. External crates cannot
//!   add variants.
//! - [`kill_switch`] — `KillSwitchGate` consulting the shared kill-switch
//!   state before every dispatch. Returns `DispatchError::KillSwitchSuspended`
//!   or `DispatchError::KillSwitchTerminated` when not Active.
//! - [`infrastructure`] — `InfrastructurePrerequisite` for the 501 fall-back
//!   when required services (mempool-node, anvil-node, flashbots-relay, …)
//!   are unhealthy.
//! - [`errors`] — explicit error enums shared across the crate.

pub mod bundle_position;
pub mod errors;
pub mod infrastructure;
pub mod kill_switch;
