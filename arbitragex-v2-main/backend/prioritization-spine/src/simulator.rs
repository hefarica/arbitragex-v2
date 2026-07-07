//! ⚠️ DEPRECATED — v1 simulator stub (Phase A.1/A.2 fail-closed neutralization).
//!
//! ## What this file used to do
//!
//! The legacy `EvmSimulator::simulate_candidate` built a `revm::EVM` against a
//! `LazyRpcDatabase`, then ran a fabricated transaction with:
//!   - `caller   = 0x1111…1111` (RULE 00 hardcode violation)
//!   - `target   = 0x2222…2222` (RULE 00 hardcode violation)
//!   - `value    = 0`
//!   - `calldata = Bytes::new()` (empty — REVM had nothing to execute)
//!
//! revm always returned `Success` for that empty no-op transaction, so the stub
//! reported `simulation_status = "PASS"` for every candidate. Downstream
//! (`gates.rs::EvidenceGate`) accepted "PASS" as a valid simulation, and the
//! paper-trade pipeline persisted "viable" opportunities that had never been
//! verified against any real state.
//!
//! This was a RULE 00 violation: the system claimed to simulate but did not.
//!
//! ## What this file does now
//!
//! It is wired **fail-closed**. `EvmSimulator::simulate_candidate` returns the
//! sentinel string `"SIM_DISABLED_FAIL_CLOSED"`, which `gates.rs:11` maps to
//! `RejectReason::SimulationFailed`. Every candidate routed through this stub
//! is rejected at the simulator gate.
//!
//! ## Why the type is kept (not deleted)
//!
//! The scanner's hot path (`backend/searcher-rs/src/scanner.rs`) no longer
//! constructs `EvmSimulator`. The struct stays here as a deprecation shim so
//! that any forgotten caller (CI, integration tests, downstream forks) fails
//! loud with a deprecation warning at compile time AND fails closed at
//! runtime. RULE 16: changes must be idempotent and reversible.
//!
//! ## When this file goes away
//!
//! Phase A.3 lands the `OpportunityCandidate → CandidateInput` encoder and
//! wires the scanner directly to `simulator_v2::SimulatorV2`. At that point
//! this file is removed and `EvmSimulator` deleted from the public API.

use crate::types::OpportunityCandidate;
use tracing::warn;

/// Sentinel value returned by the deprecated v1 stub.
///
/// `gates.rs::EvidenceGate::validate` maps every value other than `"PASS"` or
/// `"SIM_SUCCESS"` (the Phase A.3 sentinel for real REVM success) to
/// `RejectReason::SimulationFailed` — so this string causes a fail-closed
/// rejection at the simulator gate.
pub const FAIL_CLOSED_SENTINEL: &str = "SIM_DISABLED_FAIL_CLOSED";

/// ⚠️ DEPRECATED — legacy v1 simulator. Use `simulator_v2::SimulatorV2`.
///
/// This shim exists only so that out-of-tree callers fail loudly. The scanner
/// no longer constructs it. See the module-level docs for context.
#[deprecated(
    since = "0.2.0",
    note = "v1 stub fabricated PASS results (RULE 00 violation). \
            Use `simulator_v2::SimulatorV2` once the Phase A.3 encoder lands. \
            This shim returns SIM_DISABLED_FAIL_CLOSED and triggers fail-closed \
            rejection in gates.rs. See feat/wire-simulator-v2-revm."
)]
#[allow(dead_code)]
pub struct EvmSimulator {
    _private: (),
}

#[allow(deprecated)]
impl EvmSimulator {
    /// Construct the deprecated shim. The argument is ignored; we kept a unit
    /// type as a no-op constructor surface so old call sites compile against
    /// the deprecation warning while the migration to simulator-v2 lands.
    pub fn new<T>(_provider: T) -> Self {
        warn!(
            event = "simulator.v1_stub_constructed",
            "EvmSimulator constructed (DEPRECATED). v1 stub is wired fail-closed \
             since Phase A.1/A.2. Migrate to simulator_v2::SimulatorV2."
        );
        Self { _private: () }
    }

    /// Returns `FAIL_CLOSED_SENTINEL` unconditionally. Phase A.3 will route
    /// real candidates to `simulator_v2::SimulatorV2::simulate()` and remove
    /// this shim entirely.
    pub fn simulate_candidate(&mut self, candidate: &OpportunityCandidate) -> String {
        warn!(
            event = "simulator.v1_stub_fail_closed",
            candidate_fingerprint = %candidate.route_fingerprint,
            sentinel = FAIL_CLOSED_SENTINEL,
            "v1 simulator stub invoked — returning fail-closed sentinel. \
             No REVM simulation performed. Candidate will be rejected by \
             EvidenceGate (RULE 00 honesty)."
        );
        FAIL_CLOSED_SENTINEL.to_string()
    }
}

#[cfg(test)]
#[allow(deprecated)]
mod tests {
    use super::*;

    /// The shim MUST always return the fail-closed sentinel. Any change to
    /// this contract is a RULE 00 regression — if a future edit makes this
    /// test return "PASS" without real REVM execution, the system is back to
    /// fabricating simulation results.
    #[test]
    fn v1_stub_always_returns_fail_closed_sentinel() {
        let mut sim = EvmSimulator::new(());
        let candidate = OpportunityCandidate {
            route_fingerprint: "test_route".into(),
            pool_addresses: vec![],
            token_addresses: vec![],
            dex_adapters: vec![],
            amount_in: 1.0,
            expected_amount_out: 1.0,
            gross_profit: 0.0,
        };
        let result = sim.simulate_candidate(&candidate);
        assert_eq!(result, FAIL_CLOSED_SENTINEL);
        assert_ne!(result, "PASS", "RULE 00: v1 stub must never return PASS");
        assert_ne!(
            result, "SIM_SUCCESS",
            "v1 stub must not impersonate the Phase A.3 success sentinel"
        );
    }
}
