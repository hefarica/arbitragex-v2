//! `KillSwitchGate` — pre-dispatch validation that the global kill-switch is
//! in the `Active` state (spec §4.2).
//!
//! Integrates with `shared_rs::killswitch::KillSwitchClient` which already
//! sits in front of every relay-bound code path in `relays-client`. The
//! `KillSwitchGate` here is the SED's local wrapper that adapts the existing
//! client to the SED's three-state model (`Active` / `Suspended` /
//! `Terminated`) used by the spec's gating diagrams.
//!
//! ## Doctrinal posture
//!
//! Every dispatch path MUST call [`KillSwitchGate::require_active`] before
//! enqueueing a [`crate::types::bundle_position::BundlePosition`]. The
//! shared kill-switch is the single source of truth; the SED never caches
//! the state. Stale local cache + emergency stop is exactly the
//! "fail-honest" pattern the repo doctrine forbids.

use serde::{Deserialize, Serialize};

use crate::types::errors::{DispatchError, InfrastructureError};

/// Tri-state kill-switch report. The underlying shared `KillSwitchClient`
/// exposes a boolean `is_enabled()`; the SED's pipeline diagram distinguishes
/// a *paused* state (no new dispatches, but ongoing tasks finish) from a
/// *terminated* state (full halt). Until shared-rs exposes the distinction,
/// `Suspended` is reserved for future use and `is_enabled() == true` maps to
/// `Active` while `is_enabled() == false` maps to `Terminated`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum KillSwitchState {
    /// Normal operation. New dispatches allowed.
    Active,

    /// Temporarily paused. No new dispatches; in-flight tasks finish.
    /// Reserved for future shared-rs API expansion.
    Suspended,

    /// Hard halt. Reject every dispatch.
    Terminated,
}

/// Pre-dispatch gate that validates the kill-switch is `Active`.
///
/// Construction is cheap (no I/O). The state is queried at the moment of
/// `check()` so the gate always reflects the current operator decision.
pub struct KillSwitchGate;

impl KillSwitchGate {
    /// Query the current kill-switch state.
    ///
    /// Real integration with `shared_rs::killswitch::KillSwitchClient` lands
    /// when the SED pipeline wires up to the relays-client crate (Phase 2).
    /// For Phase 1 the implementation is intentionally `todo!()` — calling
    /// this from a stub path panics loudly, surfacing the integration gap
    /// rather than silently returning a default that could mask an emergency
    /// stop.
    pub async fn check() -> Result<KillSwitchState, InfrastructureError> {
        // Phase 2 wiring will:
        //   let ks: &KillSwitchClient = /* injected at construction */;
        //   let enabled: bool = ks.is_enabled().await;
        //   Ok(if enabled { KillSwitchState::Active } else { KillSwitchState::Terminated })
        //
        // For now: explicit unimplemented to honour R8 fail-honest. The crate
        // compiles, the types exist, the pipeline can be wired, but the lookup
        // itself is not silently fabricated.
        todo!("Phase 2: wire to shared_rs::killswitch::KillSwitchClient")
    }

    /// Require the state to be `Active`. Used as a synchronous guard right
    /// after `check()` so the calling pipeline can decide on a single error
    /// site.
    ///
    /// # Errors
    ///
    /// - `DispatchError::KillSwitchSuspended` when state is `Suspended`.
    /// - `DispatchError::KillSwitchTerminated` when state is `Terminated`.
    pub fn require_active(state: KillSwitchState) -> Result<(), DispatchError> {
        match state {
            KillSwitchState::Active => Ok(()),
            KillSwitchState::Suspended => Err(DispatchError::KillSwitchSuspended),
            KillSwitchState::Terminated => Err(DispatchError::KillSwitchTerminated),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn active_passes() {
        let r = KillSwitchGate::require_active(KillSwitchState::Active);
        assert!(r.is_ok());
    }

    #[test]
    fn suspended_blocks_with_specific_error() {
        let r = KillSwitchGate::require_active(KillSwitchState::Suspended);
        assert!(matches!(r, Err(DispatchError::KillSwitchSuspended)));
    }

    #[test]
    fn terminated_blocks_with_specific_error() {
        let r = KillSwitchGate::require_active(KillSwitchState::Terminated);
        assert!(matches!(r, Err(DispatchError::KillSwitchTerminated)));
    }

    #[test]
    fn state_serializes_round_trip() {
        for s in [
            KillSwitchState::Active,
            KillSwitchState::Suspended,
            KillSwitchState::Terminated,
        ] {
            let j = serde_json::to_string(&s).expect("serialise");
            let back: KillSwitchState = serde_json::from_str(&j).expect("deserialise");
            assert_eq!(s, back);
        }
    }
}
