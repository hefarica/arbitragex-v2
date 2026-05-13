//! Explicit error enums for the SED pipeline.
//!
//! Each error variant maps 1:1 to a specific failure mode named in the design
//! spec §4.1, §4.2, §4.3. The variants are exhaustive and consumers MUST
//! match all of them — no catch-all `Other`.

use serde::{Deserialize, Serialize};

use crate::types::infrastructure::NotImplementedResponse;

/// Construction-time validation failures for the `BundlePosition<T>` typestate.
///
/// Each variant corresponds to a specific mathematical proof the caller failed
/// to supply: orthogonality of the hedge, hyperbolic CPMM invariant, etc.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, thiserror::Error)]
pub enum TopologyValidationError {
    /// `OrthogonalHedgeResult::verify_null_covariance(tol)` returned `false`.
    /// Hedge has non-zero off-diagonal covariance — not safe to mark the
    /// bundle as `OrthogonalEquilibrium`.
    #[error("non-orthogonal covariance: hedge proof rejected")]
    NonOrthogonalCovariance,

    /// `OptimalControlSolution::hyperbolic_constraint_satisfied` is `false`.
    /// The Dirac impulse would violate `x · y = k`.
    #[error("hyperbolic constraint violated: x·y ≠ k")]
    HyperbolicViolation,

    /// The eigenstate transition projection was incomplete or the equilibrium
    /// boundary was not resolved at the time of construction.
    #[error("eigenstate unresolved: equilibrium boundary not computed")]
    EigenstateUnresolved,

    /// The filtration coefficient (CDC) was below the critical threshold OR
    /// confidence dropped under the 95% gate.
    #[error("filtration incomplete: CDC below threshold or low confidence")]
    FiltrationIncomplete,

    /// The Riemannian manifold metric tensor was degenerate at the
    /// injection point.
    #[error("invalid liquidity manifold: metric tensor degenerate")]
    InvalidManifold,
}

/// Runtime dispatch errors. Surface at the `GateManager` boundary.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum DispatchError {
    /// The bundle's compile-time typestate doesn't match the runtime gate.
    /// Should never happen — included for exhaustiveness of pattern matching.
    #[error("typestate mismatch: compile-time type vs runtime gate disagree")]
    TopologyMismatch,

    /// The post-Dirac state crossed the equilibrium boundary in an unexpected
    /// direction (positive geodesic signed distance).
    #[error("equilibrium boundary violation")]
    EquilibriumBoundaryViolation,

    /// Variance accumulator overflowed the operator's NAV-scaled cap.
    #[error("variance overflow: exposure exceeds NAV cap")]
    VarianceOverflow,

    /// Multi-market correlation engine lost synchronisation; cross-venue
    /// hedges cannot be trusted in this state.
    #[error("network desynchronisation: cross-venue correlation stale")]
    NetworkDesynchronization,

    /// Kill-switch reported `Suspended` — operator paused new dispatches.
    #[error("kill-switch suspended: no new dispatches accepted")]
    KillSwitchSuspended,

    /// Kill-switch reported `Terminated` — full system halt.
    #[error("kill-switch terminated: all dispatch rejected")]
    KillSwitchTerminated,

    /// One or more upstream services failed their health check. Carries the
    /// 501-shaped response that downstream HTTP layers can surface to clients.
    #[error("infrastructure not ready: {} (sprint {})", .0.requires.join(","), .0.sprint)]
    InfrastructureNotReady(NotImplementedResponse),
}

/// Errors from the infrastructure layer itself (not the dispatch). Distinct
/// from `DispatchError::InfrastructureNotReady` which is the user-facing 501;
/// this is the internal lookup failure.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum InfrastructureError {
    /// Could not read the kill-switch config from shared-rs.
    #[error("config unreadable: kill-switch state not available")]
    ConfigUnreadable,

    /// Network partition during health-check round trip.
    #[error("network partition: health endpoint unreachable")]
    NetworkPartition,

    /// A required service responded but reported its own degraded state.
    #[error("service unavailable: {service}")]
    ServiceUnavailable {
        /// The service that reported unavailable (e.g., "anvil-node").
        service: String,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn topology_error_serializes_round_trip() {
        let e = TopologyValidationError::HyperbolicViolation;
        let s = serde_json::to_string(&e).expect("serialise");
        let back: TopologyValidationError = serde_json::from_str(&s).expect("deserialise");
        assert_eq!(e, back);
    }

    #[test]
    fn dispatch_error_display_carries_context() {
        let e = DispatchError::KillSwitchTerminated;
        let msg = format!("{e}");
        assert!(msg.contains("terminated"), "got: {msg}");
    }

    #[test]
    fn infrastructure_error_service_name_visible() {
        let e = InfrastructureError::ServiceUnavailable {
            service: "anvil-node".into(),
        };
        let msg = format!("{e}");
        assert!(msg.contains("anvil-node"), "got: {msg}");
    }
}
