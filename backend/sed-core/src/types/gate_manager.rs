//! `gate_manager` — the invariant guardian of the SED pipeline (spec §4,
//! Phase 4).
//!
//! ## Physical Model
//!
//! The GateManager is the single bottleneck through which every candidate
//! `BundlePosition<T>` must pass before being dispatched. It enforces
//! four sequential barriers, each more expensive than the last, so that
//! cheap checks prune early and the most costly check (the eigenstate
//! projection) runs only when the system is healthy and the emergency
//! stop is clear.
//!
//! ## Barrier Sequence (fail-fast ordering)
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────────┐
//! │ Barrier 1 — Infrastructure Prerequisite (501 Fail-Honest)      │
//! │   verify() → if missing service → DispatchRejection::Infra501  │
//! ├─────────────────────────────────────────────────────────────────┤
//! │ Barrier 2 — Kill Switch Emergency Gate                         │
//! │   require_active(state) → if !Active → Rejection::Emergency    │
//! ├─────────────────────────────────────────────────────────────────┤
//! │ Barrier 3 — Stochastic Gate (O(1) scalar comparison)           │
//! │   projection.should_dispatch() → if false → Rejection::Stable  │
//! ├─────────────────────────────────────────────────────────────────┤
//! │ Barrier 4 — Thermodynamic Variance Gate                        │
//! │   bundle.variance_exposure() ≤ ceiling → if > → Rejection::Var │
//! └─────────────────────────────────────────────────────────────────┘
//!              ↓ all barriers pass ↓
//!         GateVerdict::Dispatch(bundle)
//! ```
//!
//! ## Variance Monotone Non-Increasing Invariant
//!
//! The GateManager is the enforcement point for the "varianza monótona
//! no-creciente" doctrine. It tracks the system-level aggregate variance
//! (sum of all dispatched bundles' `variance_exposure()`). A new bundle
//! is admitted only if:
//!
//! ```text
//!     current_aggregate + bundle.variance_exposure() ≤ variance_ceiling
//! ```
//!
//! The ceiling is set by the operator at construction time and can be
//! tightened (never relaxed) at runtime via `tighten_variance_ceiling()`.
//! This ensures the aggregate variance is monotone non-increasing over
//! the operational session: once the ceiling drops, it never goes back up.
//!
//! ## Performance
//!
//! - Barriers 1-2: O(1) lookups (health cache, kill-switch flag).
//! - Barrier 3: O(1) — pre-computed `should_dispatch()` scalar comparison.
//! - Barrier 4: O(1) — single f64 comparison + addition.
//! - Total: O(1) per candidate. No matrix operations at dispatch time.

use crate::types::bundle_position::{BundlePosition, PostResolutionTopology};
use crate::types::errors::DispatchError;
use crate::types::infrastructure::NotImplementedResponse;
use crate::types::kill_switch::{KillSwitchGate, KillSwitchState};

/// The outcome of a `GateManager::evaluate()` call.
///
/// Either the bundle is cleared for dispatch, or one of the four barriers
/// rejected it with a specific reason.
#[derive(Debug)]
pub enum GateVerdict<T: PostResolutionTopology> {
    /// All four barriers passed. The bundle is cleared for dispatch.
    /// The caller owns the returned `BundlePosition<T>` and may proceed
    /// to the execution layer.
    Dispatch(BundlePosition<T>),

    /// One of the barriers rejected the bundle. The rejection carries
    /// the specific reason so the caller (and telemetry) know which
    /// gate fired.
    Rejected(DispatchRejection),
}

/// Specific reason a bundle was rejected by the GateManager.
///
/// Ordered by barrier number (1 → 4). The variant names are
/// self-documenting; the inner data carries diagnostics.
#[derive(Debug, Clone)]
pub enum DispatchRejection {
    /// Barrier 1 — infrastructure not ready. Carries the 501 response
    /// that the HTTP layer surfaces to the operator.
    InfrastructureNotReady(NotImplementedResponse),

    /// Barrier 2 — kill switch fired. Carries the specific kill-switch
    /// error (`Suspended` or `Terminated`).
    EmergencyStop(DispatchError),

    /// Barrier 3 — eigenstate projection says the ground state is
    /// stable (transition probability below threshold). Carries the
    /// actual transition probability for observability.
    StochasticStable {
        transition_probability: f64,
        spectral_gap: f64,
    },

    /// Barrier 4 — admitting this bundle would violate the monotone
    /// non-increasing variance invariant. Carries the arithmetic.
    VarianceCeilingExceeded {
        /// The bundle's individual variance exposure.
        bundle_variance: f64,
        /// The current aggregate variance across all dispatched bundles.
        current_aggregate: f64,
        /// The operator-configured ceiling.
        variance_ceiling: f64,
    },
}

/// A lightweight projection summary passed into `evaluate()`.
///
/// This exists so the GateManager doesn't depend directly on the
/// `eigenstate` feature at compile time. Callers construct it from
/// a `TransitionProjection` (Phase 3) or provide a manual override
/// for testing.
#[derive(Debug, Clone, Copy)]
pub struct StochasticGateInput {
    /// `true` when the eigenstate projection says the system is
    /// unstable enough to dispatch.
    pub should_dispatch: bool,
    /// The actual transition probability ∈ [0, 1] (for telemetry).
    pub transition_probability: f64,
    /// The spectral gap E₁ − E₀ (for telemetry).
    pub spectral_gap: f64,
}

/// The GateManager — invariant guardian of the SED pipeline.
///
/// Created once per operational session. Tracks the aggregate variance
/// across all dispatched bundles and enforces the four sequential barriers.
///
/// ## Thread Safety
///
/// The GateManager is NOT `Sync` by design — the variance accumulator
/// is `&mut self`. The pipeline owner holds the single mutable reference
/// and serialises dispatch decisions. If concurrent evaluation is needed
/// in Phase 5+, wrap in a `tokio::sync::Mutex`.
pub struct GateManager {
    /// Infrastructure prerequisite for this pipeline stage.
    infra_prereq: InfraPrereqConfig,
    /// The operator-configured variance ceiling. Can be tightened (never
    /// relaxed) at runtime via `tighten_variance_ceiling()`.
    variance_ceiling: f64,
    /// Running sum of `variance_exposure()` across all dispatched bundles.
    aggregate_variance: f64,
    /// Count of bundles dispatched through this gate.
    dispatch_count: u64,
    /// Count of bundles rejected (across all barriers).
    rejection_count: u64,
}

/// Configuration for the infrastructure prerequisite barrier.
///
/// Decoupled from `InfrastructurePrerequisite` so the GateManager can
/// be constructed without async health checks. The actual prerequisite
/// is built lazily from this config.
#[derive(Debug, Clone)]
pub struct InfraPrereqConfig {
    /// Service IDs required for this pipeline stage.
    pub required_services: Vec<String>,
    /// Sprint label for the 501 response.
    pub minimum_sprint: String,
}

impl Default for InfraPrereqConfig {
    fn default() -> Self {
        Self {
            required_services: Vec::new(),
            minimum_sprint: "S4".to_string(),
        }
    }
}

impl GateManager {
    /// Construct a new GateManager.
    ///
    /// # Arguments
    ///
    /// * `variance_ceiling` — the maximum allowed aggregate variance.
    ///   Must be positive and finite. Clamped to `[1e-15, f64::MAX]`.
    /// * `infra_config` — infrastructure prerequisite configuration.
    pub fn new(variance_ceiling: f64, infra_config: InfraPrereqConfig) -> Self {
        Self {
            infra_prereq: infra_config,
            variance_ceiling: variance_ceiling.clamp(1e-15, f64::MAX),
            aggregate_variance: 0.0,
            dispatch_count: 0,
            rejection_count: 0,
        }
    }

    /// Evaluate a candidate bundle against all four barriers.
    ///
    /// The barriers are checked in order 1 → 4. The FIRST barrier
    /// that fails produces a `GateVerdict::Rejected` and the remaining
    /// barriers are skipped (fail-fast).
    ///
    /// If all barriers pass, the aggregate variance is atomically
    /// updated and the bundle is returned in `GateVerdict::Dispatch`.
    ///
    /// # Arguments
    ///
    /// * `bundle` — the candidate `BundlePosition<T>`.
    /// * `kill_switch_state` — the current kill-switch state. The caller
    ///   is responsible for querying `KillSwitchGate::check()` and passing
    ///   the result here. This avoids making `evaluate()` async.
    /// * `stochastic_gate` — the pre-computed eigenstate projection summary.
    pub fn evaluate<T: PostResolutionTopology>(
        &mut self,
        bundle: BundlePosition<T>,
        kill_switch_state: KillSwitchState,
        stochastic_gate: &StochasticGateInput,
    ) -> GateVerdict<T> {
        // ── Barrier 1: Infrastructure Prerequisite ──────────────────
        if !self.infra_prereq.required_services.is_empty() {
            self.rejection_count += 1;
            return GateVerdict::Rejected(DispatchRejection::InfrastructureNotReady(
                NotImplementedResponse {
                    requires: self.infra_prereq.required_services.clone(),
                    sprint: self.infra_prereq.minimum_sprint.clone(),
                    message: format!(
                        "SED module requires infrastructure not yet available in {}",
                        self.infra_prereq.minimum_sprint,
                    ),
                },
            ));
        }

        // ── Barrier 2: Kill Switch Emergency Gate ───────────────────
        if let Err(dispatch_err) = KillSwitchGate::require_active(kill_switch_state) {
            self.rejection_count += 1;
            return GateVerdict::Rejected(DispatchRejection::EmergencyStop(dispatch_err));
        }

        // ── Barrier 3: Stochastic Gate (O(1)) ──────────────────────
        if !stochastic_gate.should_dispatch {
            self.rejection_count += 1;
            return GateVerdict::Rejected(DispatchRejection::StochasticStable {
                transition_probability: stochastic_gate.transition_probability,
                spectral_gap: stochastic_gate.spectral_gap,
            });
        }

        // ── Barrier 4: Thermodynamic Variance Gate ─────────────────
        let bundle_variance = bundle.variance_exposure();
        let proposed_aggregate = self.aggregate_variance + bundle_variance;

        if proposed_aggregate > self.variance_ceiling {
            self.rejection_count += 1;
            return GateVerdict::Rejected(DispatchRejection::VarianceCeilingExceeded {
                bundle_variance,
                current_aggregate: self.aggregate_variance,
                variance_ceiling: self.variance_ceiling,
            });
        }

        // ── All barriers passed ────────────────────────────────────
        self.aggregate_variance = proposed_aggregate;
        self.dispatch_count += 1;
        GateVerdict::Dispatch(bundle)
    }

    /// Tighten the variance ceiling. Can ONLY decrease the ceiling,
    /// never increase it. This enforces the monotone non-increasing
    /// invariant at the operator level.
    ///
    /// Returns the new effective ceiling.
    pub fn tighten_variance_ceiling(&mut self, new_ceiling: f64) -> f64 {
        if new_ceiling < self.variance_ceiling && new_ceiling > 0.0 {
            self.variance_ceiling = new_ceiling;
        }
        self.variance_ceiling
    }

    /// Current aggregate variance across all dispatched bundles.
    pub fn aggregate_variance(&self) -> f64 {
        self.aggregate_variance
    }

    /// Remaining variance headroom before the ceiling is hit.
    pub fn remaining_variance_headroom(&self) -> f64 {
        (self.variance_ceiling - self.aggregate_variance).max(0.0)
    }

    /// Current variance ceiling.
    pub fn variance_ceiling(&self) -> f64 {
        self.variance_ceiling
    }

    /// Number of bundles dispatched through this gate.
    pub fn dispatch_count(&self) -> u64 {
        self.dispatch_count
    }

    /// Number of bundles rejected (across all barriers).
    pub fn rejection_count(&self) -> u64 {
        self.rejection_count
    }

    /// Reset the aggregate variance accumulator. Use ONLY when the
    /// operator explicitly acknowledges a new risk epoch (e.g., after
    /// a portfolio rebalance). The ceiling is NOT reset.
    pub fn reset_aggregate_variance(&mut self) {
        self.aggregate_variance = 0.0;
    }
}

#[cfg(test)]
#[cfg(all(feature = "allocator", feature = "hedger"))]
mod tests {
    use super::*;
    use crate::hedger::OrthogonalHedgeResult;
    use crate::allocator::OptimalControlSolution;

    /// Helper: build an OrthogonalEquilibrium bundle with a given variance.
    fn make_ortho_bundle(variance: f64) -> BundlePosition<crate::types::bundle_position::OrthogonalEquilibrium> {
        let proof = OrthogonalHedgeResult::stub(true);
        BundlePosition::new_orthogonal_equilibrium(
            "ETH-USDC".to_string(),
            ("ETH".to_string(), "USDC".to_string()),
            1000.0,
            variance,
            &proof,
        ).expect("valid proof")
    }

    /// Helper: build a Dirac bundle with a given variance.
    fn make_dirac_bundle(variance: f64) -> BundlePosition<crate::types::bundle_position::DiracImpulseOnly> {
        let proof = OptimalControlSolution::stub(true);
        BundlePosition::new_dirac_impulse_only(
            "WETH-DAI".to_string(),
            ("WETH".to_string(), "DAI".to_string()),
            500.0,
            variance,
            &proof,
        ).expect("valid proof")
    }

    fn dispatch_input() -> StochasticGateInput {
        StochasticGateInput {
            should_dispatch: true,
            transition_probability: 0.7,
            spectral_gap: 1.5,
        }
    }

    fn stable_input() -> StochasticGateInput {
        StochasticGateInput {
            should_dispatch: false,
            transition_probability: 0.05,
            spectral_gap: 3.0,
        }
    }

    // ── Barrier 1: Infrastructure ──────────────────────────────────

    #[test]
    fn barrier_1_rejects_when_services_required() {
        let config = InfraPrereqConfig {
            required_services: vec!["mempool-node".into()],
            minimum_sprint: "S2".into(),
        };
        let mut gm = GateManager::new(100.0, config);
        let bundle = make_ortho_bundle(1.0);
        let verdict = gm.evaluate(bundle, KillSwitchState::Active, &dispatch_input());
        assert!(matches!(verdict, GateVerdict::Rejected(DispatchRejection::InfrastructureNotReady(_))));
        assert_eq!(gm.rejection_count(), 1);
    }

    #[test]
    fn barrier_1_passes_when_no_services_required() {
        let config = InfraPrereqConfig::default();
        let mut gm = GateManager::new(100.0, config);
        let bundle = make_ortho_bundle(1.0);
        let verdict = gm.evaluate(bundle, KillSwitchState::Active, &dispatch_input());
        assert!(matches!(verdict, GateVerdict::Dispatch(_)));
    }

    // ── Barrier 2: Kill Switch ─────────────────────────────────────

    #[test]
    fn barrier_2_rejects_on_terminated() {
        let mut gm = GateManager::new(100.0, InfraPrereqConfig::default());
        let bundle = make_ortho_bundle(1.0);
        let verdict = gm.evaluate(bundle, KillSwitchState::Terminated, &dispatch_input());
        assert!(matches!(
            verdict,
            GateVerdict::Rejected(DispatchRejection::EmergencyStop(DispatchError::KillSwitchTerminated))
        ));
    }

    #[test]
    fn barrier_2_rejects_on_suspended() {
        let mut gm = GateManager::new(100.0, InfraPrereqConfig::default());
        let bundle = make_ortho_bundle(1.0);
        let verdict = gm.evaluate(bundle, KillSwitchState::Suspended, &dispatch_input());
        assert!(matches!(
            verdict,
            GateVerdict::Rejected(DispatchRejection::EmergencyStop(DispatchError::KillSwitchSuspended))
        ));
    }

    // ── Barrier 3: Stochastic Gate ─────────────────────────────────

    #[test]
    fn barrier_3_rejects_when_ground_state_stable() {
        let mut gm = GateManager::new(100.0, InfraPrereqConfig::default());
        let bundle = make_ortho_bundle(1.0);
        let verdict = gm.evaluate(bundle, KillSwitchState::Active, &stable_input());
        match verdict {
            GateVerdict::Rejected(DispatchRejection::StochasticStable { transition_probability, .. }) => {
                assert!((transition_probability - 0.05).abs() < 1e-10);
            }
            _ => panic!("expected StochasticStable rejection"),
        }
    }

    #[test]
    fn barrier_3_passes_when_ground_state_unstable() {
        let mut gm = GateManager::new(100.0, InfraPrereqConfig::default());
        let bundle = make_ortho_bundle(1.0);
        let verdict = gm.evaluate(bundle, KillSwitchState::Active, &dispatch_input());
        assert!(matches!(verdict, GateVerdict::Dispatch(_)));
    }

    // ── Barrier 4: Variance ────────────────────────────────────────

    #[test]
    fn barrier_4_rejects_when_variance_exceeds_ceiling() {
        let mut gm = GateManager::new(5.0, InfraPrereqConfig::default());
        let bundle = make_ortho_bundle(10.0);
        let verdict = gm.evaluate(bundle, KillSwitchState::Active, &dispatch_input());
        match verdict {
            GateVerdict::Rejected(DispatchRejection::VarianceCeilingExceeded {
                bundle_variance,
                current_aggregate,
                variance_ceiling,
            }) => {
                assert!((bundle_variance - 10.0).abs() < 1e-10);
                assert!((current_aggregate - 0.0).abs() < 1e-10);
                assert!((variance_ceiling - 5.0).abs() < 1e-10);
            }
            _ => panic!("expected VarianceCeilingExceeded rejection"),
        }
    }

    #[test]
    fn barrier_4_accumulates_variance_across_dispatches() {
        let mut gm = GateManager::new(10.0, InfraPrereqConfig::default());
        let input = dispatch_input();

        // Dispatch 3 bundles with variance 3.0 each → aggregate = 9.0.
        for _ in 0..3 {
            let bundle = make_ortho_bundle(3.0);
            let verdict = gm.evaluate(bundle, KillSwitchState::Active, &input);
            assert!(matches!(verdict, GateVerdict::Dispatch(_)));
        }
        assert!((gm.aggregate_variance() - 9.0).abs() < 1e-10);
        assert_eq!(gm.dispatch_count(), 3);

        // 4th bundle with variance 3.0 → aggregate would be 12.0 > 10.0 → reject.
        let bundle = make_ortho_bundle(3.0);
        let verdict = gm.evaluate(bundle, KillSwitchState::Active, &input);
        assert!(matches!(verdict, GateVerdict::Rejected(DispatchRejection::VarianceCeilingExceeded { .. })));
        assert_eq!(gm.rejection_count(), 1);
    }

    // ── Variance Ceiling Monotonicity ──────────────────────────────

    #[test]
    fn tighten_ceiling_can_only_decrease() {
        let mut gm = GateManager::new(10.0, InfraPrereqConfig::default());

        // Tighten to 5.0 → succeeds.
        let new = gm.tighten_variance_ceiling(5.0);
        assert!((new - 5.0).abs() < 1e-10);

        // Attempt to relax back to 8.0 → ceiling stays at 5.0.
        let new = gm.tighten_variance_ceiling(8.0);
        assert!((new - 5.0).abs() < 1e-10);

        // Tighten further to 3.0 → succeeds.
        let new = gm.tighten_variance_ceiling(3.0);
        assert!((new - 3.0).abs() < 1e-10);
    }

    #[test]
    fn tighten_ceiling_rejects_non_positive() {
        let mut gm = GateManager::new(10.0, InfraPrereqConfig::default());
        let new = gm.tighten_variance_ceiling(-1.0);
        assert!((new - 10.0).abs() < 1e-10); // unchanged
    }

    // ── Headroom ───────────────────────────────────────────────────

    #[test]
    fn headroom_decreases_with_dispatches() {
        let mut gm = GateManager::new(10.0, InfraPrereqConfig::default());
        assert!((gm.remaining_variance_headroom() - 10.0).abs() < 1e-10);

        let bundle = make_ortho_bundle(3.0);
        let _ = gm.evaluate(bundle, KillSwitchState::Active, &dispatch_input());
        assert!((gm.remaining_variance_headroom() - 7.0).abs() < 1e-10);
    }

    // ── Reset ──────────────────────────────────────────────────────

    #[test]
    fn reset_clears_aggregate_but_not_ceiling() {
        let mut gm = GateManager::new(10.0, InfraPrereqConfig::default());
        let bundle = make_ortho_bundle(5.0);
        let _ = gm.evaluate(bundle, KillSwitchState::Active, &dispatch_input());
        assert!((gm.aggregate_variance() - 5.0).abs() < 1e-10);

        gm.reset_aggregate_variance();
        assert!((gm.aggregate_variance() - 0.0).abs() < 1e-10);
        assert!((gm.variance_ceiling() - 10.0).abs() < 1e-10);
    }

    // ── Cross-Topology ─────────────────────────────────────────────

    #[test]
    fn accepts_dirac_topology_through_gate() {
        let mut gm = GateManager::new(100.0, InfraPrereqConfig::default());
        let bundle = make_dirac_bundle(2.0);
        let verdict = gm.evaluate(bundle, KillSwitchState::Active, &dispatch_input());
        assert!(matches!(verdict, GateVerdict::Dispatch(_)));
        assert!((gm.aggregate_variance() - 2.0).abs() < 1e-10);
    }

    // ── Fail-fast ordering ─────────────────────────────────────────

    #[test]
    fn barriers_fire_in_order_infra_before_killswitch() {
        let config = InfraPrereqConfig {
            required_services: vec!["mempool-node".into()],
            minimum_sprint: "S2".into(),
        };
        let mut gm = GateManager::new(100.0, config);
        let bundle = make_ortho_bundle(1.0);
        // Both infra AND kill-switch should fail, but infra fires FIRST.
        let verdict = gm.evaluate(bundle, KillSwitchState::Terminated, &dispatch_input());
        assert!(matches!(verdict, GateVerdict::Rejected(DispatchRejection::InfrastructureNotReady(_))));
    }

    #[test]
    fn barriers_fire_in_order_killswitch_before_stochastic() {
        let mut gm = GateManager::new(100.0, InfraPrereqConfig::default());
        let bundle = make_ortho_bundle(1.0);
        // Both kill-switch AND stochastic should fail, but kill-switch fires FIRST.
        let verdict = gm.evaluate(bundle, KillSwitchState::Terminated, &stable_input());
        assert!(matches!(
            verdict,
            GateVerdict::Rejected(DispatchRejection::EmergencyStop(DispatchError::KillSwitchTerminated))
        ));
    }

    #[test]
    fn barriers_fire_in_order_stochastic_before_variance() {
        let mut gm = GateManager::new(0.1, InfraPrereqConfig::default()); // very low ceiling
        let bundle = make_ortho_bundle(100.0); // way over ceiling
        // Both stochastic AND variance should fail, but stochastic fires FIRST.
        let verdict = gm.evaluate(bundle, KillSwitchState::Active, &stable_input());
        assert!(matches!(
            verdict,
            GateVerdict::Rejected(DispatchRejection::StochasticStable { .. })
        ));
    }
}
