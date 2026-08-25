//! SignalTier — workbook `Execution_Class` (sheets 11 + 13, closed 29-token
//! vocabulary) folded into the four emission tiers of the detector-policy
//! directive (ARBX-DP-003): SIGNAL / OBSERVATION / CANDIDATE / EXECUTABLE as
//! DISTINCT feed fields — never one flattened "arbitrage" feed.
//!
//! ## Tier derivation rule (faithful to the class tokens, not invented)
//!
//! - **OBSERVATION** — `OBSERVE_ONLY`: the detector's output is informational
//!   by construction. It emits an Observation, NEVER an
//!   `Opportunity{confidence}` (sheet 13 universal guard).
//! - **SIGNAL** — the class's precondition lives OUTSIDE the atomic execution
//!   envelope, so the output is a signal until firm evidence arrives:
//!   `SIGNAL_UNLESS_FIRM_EXIT`, `EXTERNAL_DATA_REQUIRED`,
//!   `EXTERNAL_SETTLEMENT_REQUIRED`, `DERIVATIVE_DATA_REQUIRED`,
//!   `NONATOMIC_BRIDGE_REQUIRED`, `NONATOMIC_INVENTORY_REQUIRED`,
//!   `LATENCY_SENSITIVE`, `SETTLEMENT_DELAY_SENSITIVE`.
//! - **EXECUTABLE** — `DETERMINISTIC_EXECUTABLE` only: the one class that
//!   asserts determinism with no external precondition.
//! - **CANDIDATE** — every remaining `DETERMINISTIC_*` (deterministic math
//!   under a runtime-observable precondition: `IF_*`, `POST_*`, `WITH_*`,
//!   `SETTLEMENT`, `AUCTION`, `LIQUIDATION`, `POSITION_STRATEGY`) plus
//!   `AUTHORIZED_FLOW_ONLY` (deterministic given an observed authorized
//!   flow). Candidates graduate at the evaluation gates, not here.
//!
//! Family-uniform by construction (DP-001 generator invariant): a strategy's
//! class equals its detector's class, so this table serves both sides.
//!
//! ## Fail-closed
//!
//! `tier_for_execution_class` returns `None` for any token outside the 29 —
//! a drifted workbook class NEVER silently maps to a permissive tier. The
//! 60-row sweep test is the tripwire (CI fails before a deploy could ship a
//! missing mapping), and the emission gate blocks unknown classes with the
//! honest reason `tier_unknown_execution_class` (R8).

use crate::detector_policy::DetectorPolicy;
use crate::strategy_execution_class;

/// The four emission tiers — the feed taxonomy's closed vocabulary.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SignalTier {
    /// Informational only; NEVER an `Opportunity{confidence}`.
    Observation,
    /// Real-valued/boolean signal; needs firm evidence or an external
    /// confirmation before it can be acted on.
    Signal,
    /// Exact detector math under a runtime-observable precondition;
    /// graduates at the evaluation gates.
    Candidate,
    /// Deterministic with no external precondition — may enter the
    /// executable feed.
    Executable,
}

impl SignalTier {
    /// Stable wire/telemetry token.
    pub fn as_str(self) -> &'static str {
        match self {
            SignalTier::Observation => "observation",
            SignalTier::Signal => "signal",
            SignalTier::Candidate => "candidate",
            SignalTier::Executable => "executable",
        }
    }
}

/// Workbook `Execution_Class` token → tier. `None` = token outside the
/// closed 29-class vocabulary (fail-closed; never a permissive default).
pub fn tier_for_execution_class(class: &str) -> Option<SignalTier> {
    match class {
        "OBSERVE_ONLY" => Some(SignalTier::Observation),
        // Precondition outside the atomic execution envelope → signal
        // until firm evidence.
        "SIGNAL_UNLESS_FIRM_EXIT"
        | "EXTERNAL_DATA_REQUIRED"
        | "EXTERNAL_SETTLEMENT_REQUIRED"
        | "DERIVATIVE_DATA_REQUIRED"
        | "NONATOMIC_BRIDGE_REQUIRED"
        | "NONATOMIC_INVENTORY_REQUIRED"
        | "LATENCY_SENSITIVE"
        | "SETTLEMENT_DELAY_SENSITIVE" => Some(SignalTier::Signal),
        "DETERMINISTIC_EXECUTABLE" => Some(SignalTier::Executable),
        // Deterministic under a runtime-observable precondition.
        "AUTHORIZED_FLOW_ONLY"
        | "DETERMINISTIC_AUCTION"
        | "DETERMINISTIC_IF_ADAPTER"
        | "DETERMINISTIC_IF_COMPLETE_SET"
        | "DETERMINISTIC_IF_CONVERTIBLE"
        | "DETERMINISTIC_IF_FIRM_BID"
        | "DETERMINISTIC_IF_FIRM_EXIT"
        | "DETERMINISTIC_IF_MATCHED_CLAIM"
        | "DETERMINISTIC_IF_PAYOFF_MODEL"
        | "DETERMINISTIC_IF_POSITIONS"
        | "DETERMINISTIC_IF_REDEEMABLE"
        | "DETERMINISTIC_IF_SETTLEABLE"
        | "DETERMINISTIC_LIQUIDATION"
        | "DETERMINISTIC_POSITION_STRATEGY"
        | "DETERMINISTIC_POST_ORACLE"
        | "DETERMINISTIC_POST_STATE"
        | "DETERMINISTIC_SETTLEMENT"
        | "DETERMINISTIC_WITH_DERIVATIVE_STATE"
        | "DETERMINISTIC_WITH_ORACLE" => Some(SignalTier::Candidate),
        _ => None,
    }
}

/// Strategy-side lookup: workbook MEV id (`MEV-08-018`) → tier.
pub fn tier_for_mev_id(mev_id: &str) -> Option<SignalTier> {
    strategy_execution_class::execution_class(mev_id).and_then(tier_for_execution_class)
}

/// Normalize a cartridge stem id (`mev_08_018_liquidation_auction`, the
/// `Opportunity::cartridge_id` format) to its workbook MEV id
/// (`MEV-08-018`). `None` when the stem does not carry the
/// `mev_<family>_<number>` prefix — such rows are not workbook strategies
/// and the taxonomy does not cover them.
pub fn mev_id_from_cartridge_id(cartridge_id: &str) -> Option<String> {
    let mut parts = cartridge_id.split('_');
    let prefix = parts.next()?;
    if !prefix.eq_ignore_ascii_case("mev") {
        return None;
    }
    let family: u32 = parts.next()?.parse().ok()?;
    let number: u32 = parts.next()?.parse().ok()?;
    if family == 0 || number == 0 || family > 99 || number > 999 {
        return None;
    }
    Some(format!("MEV-{:02}-{:03}", family, number))
}

/// The typed per-detector output shape — formalizes `DetectorOutput::
/// Observation` as a tier distinct from an `Opportunity{confidence}`:
/// `carries_confidence()` is `false` exactly for Observation/Signal rows,
/// so the signal-taxonomy feeds (DP-005) can never launder an observation
/// into an opportunity card.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct DetectorOutput {
    pub detector_id: &'static str,
    pub tier: SignalTier,
}

impl DetectorOutput {
    pub fn new(detector_id: &'static str, tier: SignalTier) -> Self {
        Self { detector_id, tier }
    }

    /// `true` only for tiers that may carry a confidence-bearing
    /// opportunity payload (Candidate/Executable).
    pub fn carries_confidence(&self) -> bool {
        matches!(self.tier, SignalTier::Candidate | SignalTier::Executable)
    }
}

/// Verdict of the DP-003 taxonomy gate at the `Opportunity` publish seam.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum EmissionVerdict {
    /// The row may take the `Opportunity{confidence}` shape (tier is
    /// Candidate/Executable, or the row carries no workbook identity —
    /// core engines / non-workbook cartridges are out of taxonomy scope).
    Pass,
    /// Blocked from the Opportunity shape; reclassify honestly (R8) with
    /// the carried reason.
    Blocked { reason: &'static str },
}

/// Class-level gate: which tiers may emit an `Opportunity{confidence}`.
pub fn class_verdict(class: &str) -> EmissionVerdict {
    match tier_for_execution_class(class) {
        Some(SignalTier::Observation) => EmissionVerdict::Blocked {
            reason: "tier_observation_never_opportunity",
        },
        Some(SignalTier::Signal) => EmissionVerdict::Blocked {
            reason: "tier_signal_until_firm_evidence",
        },
        Some(SignalTier::Candidate) | Some(SignalTier::Executable) => EmissionVerdict::Pass,
        None => EmissionVerdict::Blocked {
            reason: "tier_unknown_execution_class",
        },
    }
}

/// Publish-seam gate: resolve an `Opportunity::cartridge_id` to its
/// workbook strategy tier and decide Opportunity-shape eligibility.
///
/// - `cartridge_id = None` (core engines) → `Pass`: the taxonomy covers the
///   264 workbook strategies, not the core engine paths.
/// - stem without the `mev_<family>_<number>` prefix → `Pass` (custom /
///   non-workbook cartridge — no class exists to gate on).
/// - valid MEV id missing from the 264, or a class outside the closed
///   vocabulary → `Blocked` (fail-closed, named reason).
pub fn opportunity_verdict(cartridge_id: Option<&str>) -> EmissionVerdict {
    let Some(cartridge_id) = cartridge_id else {
        return EmissionVerdict::Pass;
    };
    let Some(mev_id) = mev_id_from_cartridge_id(cartridge_id) else {
        return EmissionVerdict::Pass;
    };
    match strategy_execution_class::execution_class(&mev_id) {
        Some(class) => class_verdict(class),
        None => EmissionVerdict::Blocked {
            reason: "tier_unknown_execution_class",
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::detector_policy::DETECTOR_POLICIES;

    fn policy_of(id: &str) -> &'static DetectorPolicy {
        crate::detector_policy::detector_policy(id).expect("canonical detector")
    }

    #[test]
    fn full_table_sweep_pins_the_four_way_partition() {
        // Workbook tripwire: 1 Observation / 15 Signal / 33 Candidate /
        // 11 Executable over the 60 detectors, and EVERY row resolves.
        let mut counts = [0usize; 4];
        for p in &DETECTOR_POLICIES {
            let tier = tier_for_execution_class(p.execution_class)
                .expect("closed 29-class vocabulary covers the table");
            counts[tier as usize] += 1;
        }
        assert_eq!(counts[0], 1, "OBSERVE_ONLY detectors");
        assert_eq!(counts[1], 15, "SIGNAL detectors");
        assert_eq!(counts[2], 33, "CANDIDATE detectors");
        assert_eq!(counts[3], 11, "EXECUTABLE detectors");
    }

    #[test]
    fn tier_rule_matches_class_token_semantics() {
        // Real detectors, one per tier (IDs verified against the canon JSON).
        assert_eq!(
            tier_for_execution_class(policy_of("OBSERVE").execution_class),
            Some(SignalTier::Observation)
        );
        assert_eq!(
            tier_for_execution_class(policy_of("C_CEXDEX").execution_class),
            Some(SignalTier::Signal)
        );
        assert_eq!(
            tier_for_execution_class(policy_of("CF_DYNAMIC").execution_class),
            Some(SignalTier::Candidate)
        );
        assert_eq!(
            tier_for_execution_class(policy_of("CF_CLAMM").execution_class),
            Some(SignalTier::Executable)
        );
        // Every CANDIDATE class is DETERMINISTIC_* or AUTHORIZED_FLOW_ONLY;
        // every EXECUTABLE class is exactly DETERMINISTIC_EXECUTABLE.
        for p in &DETECTOR_POLICIES {
            let tier = tier_for_execution_class(p.execution_class).unwrap();
            match tier {
                SignalTier::Candidate => assert!(
                    p.execution_class.starts_with("DETERMINISTIC_")
                        || p.execution_class == "AUTHORIZED_FLOW_ONLY",
                    "{}",
                    p.execution_class
                ),
                SignalTier::Executable => {
                    assert_eq!(p.execution_class, "DETERMINISTIC_EXECUTABLE")
                }
                _ => {}
            }
        }
    }

    #[test]
    fn unknown_class_is_none_fail_closed() {
        assert_eq!(tier_for_execution_class("DETERMINISTIC_IF_NEWTHING"), None);
        assert_eq!(tier_for_execution_class(""), None);
        assert_eq!(tier_for_execution_class("observe_only"), None);
    }

    #[test]
    fn cartridge_stem_normalizes_to_mev_id() {
        assert_eq!(
            mev_id_from_cartridge_id("mev_08_018_liquidation_auction"),
            Some("MEV-08-018".to_string())
        );
        // Lenient zero-padding; strict prefix shape.
        assert_eq!(
            mev_id_from_cartridge_id("mev_3_7_x"),
            Some("MEV-03-007".to_string())
        );
        assert_eq!(mev_id_from_cartridge_id("triangular"), None);
        assert_eq!(mev_id_from_cartridge_id("mev_08"), None);
        assert_eq!(mev_id_from_cartridge_id("mev_ab_cd"), None);
        assert_eq!(mev_id_from_cartridge_id(""), None);
    }

    #[test]
    fn strategy_side_lookup_uses_real_mev_ids() {
        // Sheet-11 rows (family-uniform with sheet 13).
        assert_eq!(tier_for_mev_id("MEV-03-029"), Some(SignalTier::Observation));
        assert_eq!(tier_for_mev_id("MEV-03-007"), Some(SignalTier::Signal));
        assert_eq!(tier_for_mev_id("MEV-01-027"), Some(SignalTier::Candidate));
        assert_eq!(tier_for_mev_id("MEV-01-001"), Some(SignalTier::Executable));
        assert_eq!(tier_for_mev_id("MEV-99-999"), None);
    }

    #[test]
    fn class_verdict_blocks_observation_and_signal_only() {
        assert_eq!(
            class_verdict("OBSERVE_ONLY"),
            EmissionVerdict::Blocked {
                reason: "tier_observation_never_opportunity"
            }
        );
        assert_eq!(
            class_verdict("SIGNAL_UNLESS_FIRM_EXIT"),
            EmissionVerdict::Blocked {
                reason: "tier_signal_until_firm_evidence"
            }
        );
        assert_eq!(
            class_verdict("EXTERNAL_DATA_REQUIRED"),
            EmissionVerdict::Blocked {
                reason: "tier_signal_until_firm_evidence"
            }
        );
        assert_eq!(
            class_verdict("DETERMINISTIC_IF_REDEEMABLE"),
            EmissionVerdict::Pass
        );
        assert_eq!(
            class_verdict("DETERMINISTIC_EXECUTABLE"),
            EmissionVerdict::Pass
        );
        assert_eq!(
            class_verdict("SOME_NEW_CLASS"),
            EmissionVerdict::Blocked {
                reason: "tier_unknown_execution_class"
            }
        );
    }

    #[test]
    fn opportunity_verdict_covers_the_identity_shapes() {
        // No workbook identity (core engines) → Pass (out of taxonomy scope).
        assert_eq!(opportunity_verdict(None), EmissionVerdict::Pass);
        // Non-workbook cartridge stem → Pass (no class to gate on).
        assert_eq!(
            opportunity_verdict(Some("custom_operator_cart")),
            EmissionVerdict::Pass
        );
        // Workbook identities (suffix is parser-irrelevant — only the
        // mev_<family>_<number> prefix resolves): observation + signal
        // blocked, candidate passes.
        assert_eq!(
            opportunity_verdict(Some("mev_03_029_fixture")),
            EmissionVerdict::Blocked {
                reason: "tier_observation_never_opportunity"
            }
        );
        assert_eq!(
            opportunity_verdict(Some("mev_03_007_fixture")),
            EmissionVerdict::Blocked {
                reason: "tier_signal_until_firm_evidence"
            }
        );
        assert_eq!(
            opportunity_verdict(Some("mev_01_027_fixture")),
            EmissionVerdict::Pass
        );
        // Valid shape, absent from the 264 → fail-closed.
        assert_eq!(
            opportunity_verdict(Some("mev_99_999_ghost")),
            EmissionVerdict::Blocked {
                reason: "tier_unknown_execution_class"
            }
        );
    }

    #[test]
    fn detector_output_formalizes_the_observation_shape() {
        let obs = DetectorOutput::new("OBSERVE", SignalTier::Observation);
        assert!(!obs.carries_confidence());
        let sig = DetectorOutput::new("C_CEXDEX", SignalTier::Signal);
        assert!(!sig.carries_confidence());
        let cand = DetectorOutput::new("CF_DYNAMIC", SignalTier::Candidate);
        assert!(cand.carries_confidence());
        let exe = DetectorOutput::new("CF_CLAMM", SignalTier::Executable);
        assert!(exe.carries_confidence());
        assert_eq!(obs.tier.as_str(), "observation");
        assert_eq!(exe.tier.as_str(), "executable");
    }
}
