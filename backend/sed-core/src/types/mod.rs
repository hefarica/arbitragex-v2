//! Type-system primitives that anchor the SED's compile-time guarantees.
//!
//! Submodules:
//!
//! - [`bundle_position`] — the typestate. Defines `BundlePosition<T>` and the
//!   sealed trait `PostResolutionTopology` whose only implementors are
//!   `OrthogonalEquilibrium`, `DiracImpulseOnly`, and (V1.2 amendment,
//!   2026-05-13) `HolonomicLoopResolution`. External crates cannot add
//!   variants.
//! - [`kill_switch`] — `KillSwitchGate` consulting the shared kill-switch
//!   state before every dispatch. Returns `DispatchError::KillSwitchSuspended`
//!   or `DispatchError::KillSwitchTerminated` when not Active.
//! - [`infrastructure`] — `InfrastructurePrerequisite` for the 501 fall-back
//!   when required services (mempool-node, anvil-node, flashbots-relay, …)
//!   are unhealthy.
//! - [`holonomic`] — (V1.2) `ClosedContourTrajectory` + `TopologicalYield`,
//!   the mathematical evidence carried into the `HolonomicLoopResolution`
//!   constructor. See `ANEXOS_V1.2.md` §4.1.3–§4.1.4.
//! - [`gate_manager`] — (Phase 4) `GateManager` — the invariant guardian that
//!   enforces four sequential barriers before any bundle dispatch:
//!   infrastructure, kill-switch, stochastic gate (eigenstate O(1)), and
//!   thermodynamic variance ceiling (monotone non-increasing invariant).
//! - [`errors`] — explicit error enums shared across the crate.

pub mod bundle_position;
pub mod errors;
pub mod gate_manager;
pub mod holonomic;
pub mod infrastructure;
pub mod kill_switch;

