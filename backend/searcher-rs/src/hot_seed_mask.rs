//! HotSeedClassifier → DetectorMask (ARBX-DP-004) — sheet 13 col `Hot_Seed`
//! (5 patterns) as a dispatch selectivity primitive: event kind → the set of
//! detectors whose pattern admits being woken by that kind, so dispatch never
//! fans out 60 detectors × 264 strategies × pool × block.
//!
//! ## Admit rule (derived from the workbook sentences)
//!
//! - **PoolReserveUpdate** (dirty-pool drain / Sync / slot0 move) →
//!   `SpreadDislocation` (8: the dislocation pattern consumes the moved
//!   AMM state) + `DetectorThreshold` (43: their exact criterion re-evaluates
//!   over the new reserves). 51/60.
//! - **ExternalPriceUpdate** (off-chain reference move: Chainlink / CEX
//!   tick) → `SpreadDislocation` (the CEX side of a CEX-DEX spread moved) +
//!   `CrossDomainDislocation` (3: the external leg of the cross-domain
//!   quote). 11/60.
//! - **StateLogEvent** (non-price state change: oracle/auction/position
//!   logs — producer lands with log ingestion) → `StateEventDelta` (5).
//! - **BlockAdvance** alone → NOBODY (0/60): a bare block with no observed
//!   delta is not seed evidence for any pattern. This empty mask IS the
//!   anti-fanout guard the directive asks for.
//! - `TelemetryOnly` (OBSERVE) is admitted by nothing — cross-checked with
//!   `HotSeed::may_seed() == false` and DP-003's Observation tier.
//!
//! Bit i of the mask is the i-th row of `DETECTOR_POLICIES` (generated
//! table, sorted by Detector_ID — same stability the binary-search lookup
//! relies on). Unknown detector ids admit `false` (fail-closed).

use crate::detector_policy::{DetectorPolicy, HotSeed, DETECTOR_POLICIES};
use std::sync::OnceLock;

/// The runtime event kinds the classifier closes over. The tick pipeline's
/// v1 evidence is `PoolReserveUpdate` (the dirty-pool drain); the others are
/// the taxonomy's input contract for their producers.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum HotSeedEvent {
    /// A pool's reserves/slot0 changed (dirty-pool signal, graph rebuild).
    PoolReserveUpdate,
    /// An off-chain/external price reference moved (oracle feed, CEX tick).
    ExternalPriceUpdate,
    /// A non-price state change event arrived (oracle/auction/position log).
    StateLogEvent,
    /// A new block with no observed delta (state root advanced).
    BlockAdvance,
}

impl HotSeedEvent {
    /// Stable wire/telemetry token.
    pub fn as_str(self) -> &'static str {
        match self {
            HotSeedEvent::PoolReserveUpdate => "pool_reserve_update",
            HotSeedEvent::ExternalPriceUpdate => "external_price_update",
            HotSeedEvent::StateLogEvent => "state_log_event",
            HotSeedEvent::BlockAdvance => "block_advance",
        }
    }
}

/// Which `HotSeed` patterns an event kind admits. Pure pattern-level rule —
/// the mask is the table-wide application of it.
fn pattern_admits(event: HotSeedEvent, seed: HotSeed) -> bool {
    match event {
        HotSeedEvent::PoolReserveUpdate => matches!(
            seed,
            HotSeed::SpreadDislocation | HotSeed::DetectorThreshold
        ),
        HotSeedEvent::ExternalPriceUpdate => matches!(
            seed,
            HotSeed::SpreadDislocation | HotSeed::CrossDomainDislocation
        ),
        HotSeedEvent::StateLogEvent => matches!(seed, HotSeed::StateEventDelta),
        HotSeedEvent::BlockAdvance => false,
    }
}

fn cached_mask(event: HotSeedEvent) -> u64 {
    static MASKS: OnceLock<[u64; 4]> = OnceLock::new();
    let masks = MASKS.get_or_init(|| {
        let mut out = [0u64; 4];
        for (slot, event) in [
            HotSeedEvent::PoolReserveUpdate,
            HotSeedEvent::ExternalPriceUpdate,
            HotSeedEvent::StateLogEvent,
            HotSeedEvent::BlockAdvance,
        ]
        .into_iter()
        .enumerate()
        {
            let mut mask = 0u64;
            for (i, p) in DETECTOR_POLICIES.iter().enumerate() {
                if pattern_admits(event, p.hot_seed) {
                    mask |= 1 << i;
                }
            }
            out[slot] = mask;
        }
        out
    });
    masks[event as usize]
}

/// The detector mask for one event kind: bit i = the i-th
/// `DETECTOR_POLICIES` row is admissible to wake.
pub fn detector_mask(event: HotSeedEvent) -> u64 {
    cached_mask(event)
}

/// Row index of a detector id in `DETECTOR_POLICIES` (sorted table).
pub fn detector_index(detector_id: &str) -> Option<usize> {
    DETECTOR_POLICIES
        .binary_search_by(|p| p.detector_id.cmp(detector_id))
        .ok()
}

/// Does the mask admit this detector id? Unknown ids admit `false`
/// (fail-closed — an unregistered detector is never woken).
pub fn admits(mask: u64, detector_id: &str) -> bool {
    detector_index(detector_id)
        .map(|i| mask & (1 << i) != 0)
        .unwrap_or(false)
}

/// Policy-based admit check (avoids the id round-trip when the caller
/// already holds the row).
pub fn admits_policy(mask: u64, policy: &DetectorPolicy) -> bool {
    admits(mask, policy.detector_id)
}

/// How many detectors the mask admits (popcount).
pub fn admitted_count(mask: u64) -> u32 {
    mask.count_ones()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::detector_policy::detector_policy;

    fn policy_of(id: &str) -> &'static DetectorPolicy {
        detector_policy(id).expect("canonical detector")
    }

    #[test]
    fn census_pins_the_admit_partition() {
        // Workbook tripwire: 51 / 11 / 5 / 0 over the 60 rows.
        let reserve = detector_mask(HotSeedEvent::PoolReserveUpdate);
        let external = detector_mask(HotSeedEvent::ExternalPriceUpdate);
        let log = detector_mask(HotSeedEvent::StateLogEvent);
        let block = detector_mask(HotSeedEvent::BlockAdvance);
        assert_eq!(admitted_count(reserve), 51, "8 spread + 43 threshold");
        assert_eq!(admitted_count(external), 11, "8 spread + 3 cross-domain");
        assert_eq!(admitted_count(log), 5, "state-event-delta family");
        assert_eq!(admitted_count(block), 0, "bare block wakes nobody");
        // Table exhausts the masks: every pattern is reachable except
        // TelemetryOnly (by design).
        let union = reserve | external | log;
        assert_eq!(admitted_count(union), 59, "60 minus the telemetry-only row");
    }

    #[test]
    fn real_detectors_admit_by_pattern() {
        // IDs verified against the canon JSON.
        let reserve = detector_mask(HotSeedEvent::PoolReserveUpdate);
        let external = detector_mask(HotSeedEvent::ExternalPriceUpdate);
        let log = detector_mask(HotSeedEvent::StateLogEvent);
        let block = detector_mask(HotSeedEvent::BlockAdvance);

        // R_CLOSED_CYCLE — Spread/log-alpha/depth dislocation.
        assert!(admits_policy(reserve, policy_of("R_CLOSED_CYCLE")));
        assert!(admits_policy(external, policy_of("R_CLOSED_CYCLE")));
        assert!(!admits_policy(log, policy_of("R_CLOSED_CYCLE")));
        assert!(!admits_policy(block, policy_of("R_CLOSED_CYCLE")));
        // CF_CLAMM — detector-specific threshold from exact criterion.
        assert!(admits_policy(reserve, policy_of("CF_CLAMM")));
        assert!(!admits_policy(external, policy_of("CF_CLAMM")));
        // X_BRIDGE — cross-domain price/settlement dislocation.
        assert!(admits_policy(external, policy_of("X_BRIDGE")));
        assert!(!admits_policy(reserve, policy_of("X_BRIDGE")));
        // E_POST — state change / post-event delta.
        assert!(admits_policy(log, policy_of("E_POST")));
        assert!(!admits_policy(reserve, policy_of("E_POST")));
    }

    #[test]
    fn telemetry_only_is_admitted_by_nothing() {
        for event in [
            HotSeedEvent::PoolReserveUpdate,
            HotSeedEvent::ExternalPriceUpdate,
            HotSeedEvent::StateLogEvent,
            HotSeedEvent::BlockAdvance,
        ] {
            let mask = detector_mask(event);
            assert!(
                !admits_policy(mask, policy_of("OBSERVE")),
                "{:?} must not wake the telemetry-only detector",
                event
            );
        }
        // Cross-check the workbook's own flag.
        assert!(!policy_of("OBSERVE").hot_seed.may_seed());
    }

    #[test]
    fn unknown_detector_never_admits() {
        let mask = detector_mask(HotSeedEvent::PoolReserveUpdate);
        assert!(!admits(mask, "E_STATE_GHOST"));
        assert!(!admits(mask, ""));
        assert_eq!(detector_index("E_STATE_GHOST"), None);
    }

    #[test]
    fn index_agrees_with_the_sorted_table() {
        for (i, p) in DETECTOR_POLICIES.iter().enumerate() {
            assert_eq!(detector_index(p.detector_id), Some(i), "{}", p.detector_id);
        }
    }

    #[test]
    fn event_tokens_are_stable() {
        assert_eq!(
            HotSeedEvent::PoolReserveUpdate.as_str(),
            "pool_reserve_update"
        );
        assert_eq!(
            HotSeedEvent::ExternalPriceUpdate.as_str(),
            "external_price_update"
        );
        assert_eq!(HotSeedEvent::StateLogEvent.as_str(), "state_log_event");
        assert_eq!(HotSeedEvent::BlockAdvance.as_str(), "block_advance");
    }
}
