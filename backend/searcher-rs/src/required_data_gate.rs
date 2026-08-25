//! RequiredDataGate — workbook sheet `13_DETECTOR_POLICY` col `Required_Data`
//! as a runtime gate (ARBX-DP-002).
//!
//! The directive's pipeline position: EVENT → HotSeed → DetectorDispatch →
//! **RequiredDataGate** → GraphAdapter → detector math. The gate answers ONE
//! question per detector, from REAL tick artifacts only: "did the data this
//! family's exact criterion consumes exist at all this tick?" — and when it
//! did not, the state is an honest `NeedsData`, NEVER an approximation
//! substitute (sheet 13's universal guard: "Do not replace detector math
//! with generic spot-price spread"; the `Required_Data` sentences are the
//! per-family contract, e.g. CF_CLAMM demands sqrtPriceX96 / active
//! liquidity / tick bitmap — a missing tick snapshot blocks the family, it
//! does not degrade to a spot-price guess).
//!
//! ## Data classes vs. surfaces (honest scope, v1)
//!
//! The mapping key is the workbook's own `Example_Surface` token (closed
//! 10-token vocabulary, `DetectorSurface`): a data-domain classification,
//! not a heuristic over the free-text sentence. The tick pipeline tracks
//! exactly ONE data class today — weighted graph edges (built from V2
//! reserves / V3 slot0 snapshots, per `graph_builder::build_edges_for_pool`)
//! — so only the `DEX_AMM` surface can gate `Ready`/`NeedsData`. Every other
//! surface (LENDING positions, DERIVATIVES feeds, CROSS_CHAIN bridge quotes,
//! …) has NO adapter in the tick pipeline yet: the gate reports
//! `NotTracked` — an honest unknown. It is NEVER `Ready`-by-default (that
//! would fabricate availability) and NEVER `NeedsData`-by-guess (that would
//! fabricate a block). As adapters land, their classes grow this enum and
//! their surfaces leave `NotTracked`.
//!
//! ## Behavior note
//!
//! v1 is observational: when `NeedsData` fires for the selected detector,
//! the graph it would search has zero admitted pools, so cycle search
//! already yields nothing — the gate makes the WHY observable instead of a
//! silent 0 (R8: nada muere en silencio). The verdicts ride `tick_summary`
//! (`required_data_gate`); the taxonomy feeds (DP-003) consume them next.

use crate::detector_policy::{DetectorPolicy, DetectorSurface};

/// A category of runtime data the tick pipeline can actually observe.
/// v1: graph edges only (V2 reserves + V3 slot0 admitted by the builder).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum DataClass {
    /// ≥1 weighted edge admitted by `graph_builder` this tick.
    GraphEdges,
}

impl DataClass {
    /// Stable wire/telemetry token.
    pub fn as_str(self) -> &'static str {
        match self {
            DataClass::GraphEdges => "graph_edges",
        }
    }
}

/// Real tick availability, measured from one `GraphBuildOutcome`
/// (pure counts — no fetches, no clocks).
#[derive(Clone, Copy, Debug, Default)]
pub struct TickDataCoverage {
    /// Pools in the registry slice offered to the builder this tick.
    pub pools_total: usize,
    /// Pools that yielded edges (== `pools_total - rejected.len()`, clamped).
    pub pools_admitted: usize,
}

impl TickDataCoverage {
    /// Derive from the build outcome's raw counts (saturating: a census bug
    /// can never fabricate availability — `admitted` only shrinks).
    pub fn from_counts(pools_total: usize, rejected: usize) -> Self {
        Self {
            pools_total,
            pools_admitted: pools_total.saturating_sub(rejected),
        }
    }
}

/// The gate's three-valued honest verdict (R8: `Ready`/`NeedsData` are
/// computed facts; `NotTracked` is "no runtime adapter observes this class"
/// — distinct from both, never a default).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum GateVerdict {
    Ready,
    NeedsData(&'static str),
    NotTracked,
}

impl GateVerdict {
    /// Stable wire/telemetry token.
    pub fn as_str(self) -> &'static str {
        match self {
            GateVerdict::Ready => "ready",
            GateVerdict::NeedsData(_) => "needs_data",
            GateVerdict::NotTracked => "not_tracked",
        }
    }

    /// The specific honest reason — `Some` only for `NeedsData`.
    pub fn reason(self) -> Option<&'static str> {
        match self {
            GateVerdict::NeedsData(r) => Some(r),
            _ => None,
        }
    }
}

/// Workbook surface → data classes the tick pipeline REALLY observes.
/// Surfaces absent from this table have no adapter: `NotTracked`.
pub fn surface_data_classes(surface: DetectorSurface) -> &'static [DataClass] {
    match surface {
        DetectorSurface::DexAmm => &[DataClass::GraphEdges],
        DetectorSurface::ParityRedemption
        | DetectorSurface::DexState
        | DetectorSurface::Derivatives
        | DetectorSurface::Lending
        | DetectorSurface::Nft
        | DetectorSurface::IntentAuction
        | DetectorSurface::Prediction
        | DetectorSurface::CrossChain
        | DetectorSurface::CexDex => &[],
    }
}

/// Gate one detector against one tick's real coverage. Pure; no per-detector
/// hardcode — the policy's surface drives the required classes.
pub fn verdict(policy: &DetectorPolicy, coverage: &TickDataCoverage) -> GateVerdict {
    for class in surface_data_classes(policy.example_surface) {
        match class {
            DataClass::GraphEdges => {
                if coverage.pools_admitted > 0 {
                    continue;
                }
                return GateVerdict::NeedsData(if coverage.pools_total == 0 {
                    "universe_empty"
                } else {
                    "all_pools_missing_required_data"
                });
            }
        }
    }
    if surface_data_classes(policy.example_surface).is_empty() {
        return GateVerdict::NotTracked;
    }
    GateVerdict::Ready
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::detector_policy::{detector_policy, DetectorSurface, DETECTOR_POLICIES};

    fn policy_of(id: &str) -> &'static DetectorPolicy {
        detector_policy(id).expect("canonical detector")
    }

    #[test]
    fn dex_amm_ready_when_edges_admitted() {
        let p = policy_of("R_CLOSED_CYCLE"); // Example_Surface DEX_AMM
        let cov = TickDataCoverage {
            pools_total: 62,
            pools_admitted: 17,
        };
        assert_eq!(verdict(p, &cov), GateVerdict::Ready);
        assert_eq!(verdict(p, &cov).as_str(), "ready");
    }

    #[test]
    fn dex_amm_needs_data_when_every_pool_rejected() {
        let p = policy_of("R_CLOSED_CYCLE");
        let cov = TickDataCoverage {
            pools_total: 45,
            pools_admitted: 0,
        };
        let v = verdict(p, &cov);
        assert_eq!(v.as_str(), "needs_data");
        assert_eq!(v.reason(), Some("all_pools_missing_required_data"));
    }

    #[test]
    fn dex_amm_needs_data_names_the_empty_universe() {
        let p = policy_of("CF_CLAMM");
        let cov = TickDataCoverage {
            pools_total: 0,
            pools_admitted: 0,
        };
        assert_eq!(
            verdict(p, &cov).reason(),
            Some("universe_empty"),
            "0 pools offered is a universe fact, not a data outage"
        );
    }

    #[test]
    fn coverage_is_saturating_never_fabricates_availability() {
        let cov = TickDataCoverage::from_counts(10, 12);
        assert_eq!(cov.pools_admitted, 0, "census bug must not admit pools");
    }

    #[test]
    fn untracked_surfaces_report_not_tracked_never_ready() {
        // One REAL detector per non-DEX_AMM surface from the workbook.
        let untracked = [
            ("P_4626", "PARITY_REDEMPTION"),
            ("E_POST", "DEX_STATE"),
            ("D_OPTIONS_PARITY", "DERIVATIVES"),
            ("L_LIQ", "LENDING"),
            ("N_FLOOR", "NFT"),
            ("I_BATCH", "INTENT_AUCTION"),
            ("M_AMM", "PREDICTION"),
            ("X_BRIDGE", "CROSS_CHAIN"),
            ("C_CEXDEX", "CEX_DEX"),
        ];
        for (id, surface) in untracked {
            let p = policy_of(id);
            assert_eq!(p.example_surface.as_str(), surface, "{} surface", id);
            let v = verdict(
                p,
                &TickDataCoverage {
                    pools_total: 99,
                    pools_admitted: 99,
                },
            );
            assert_eq!(v, GateVerdict::NotTracked, "{}", id);
            assert_eq!(v.reason(), None, "NotTracked carries no fake reason");
            assert_eq!(v.as_str(), "not_tracked");
        }
    }

    #[test]
    fn full_table_sweep_matches_surface_partition() {
        // Workbook tripwire: 19 DEX_AMM detectors gate on graph edges; the
        // other 41 are honestly NotTracked until adapters exist.
        let cov_empty = TickDataCoverage {
            pools_total: 0,
            pools_admitted: 0,
        };
        let mut needs = 0;
        let mut not_tracked = 0;
        for p in &DETECTOR_POLICIES {
            match verdict(p, &cov_empty) {
                GateVerdict::NeedsData(_) => needs += 1,
                GateVerdict::NotTracked => not_tracked += 1,
                GateVerdict::Ready => panic!("{} ready on empty coverage", p.detector_id),
            }
        }
        assert_eq!(needs, 19, "DEX_AMM detector count drift");
        assert_eq!(not_tracked, 41);
    }

    #[test]
    fn data_class_vocabulary() {
        assert_eq!(DataClass::GraphEdges.as_str(), "graph_edges");
        // Every surface resolves a class list — closed vocabularies on both
        // sides (10 surfaces).
        let all = [
            DetectorSurface::DexAmm,
            DetectorSurface::ParityRedemption,
            DetectorSurface::DexState,
            DetectorSurface::Derivatives,
            DetectorSurface::Lending,
            DetectorSurface::Nft,
            DetectorSurface::IntentAuction,
            DetectorSurface::Prediction,
            DetectorSurface::CrossChain,
            DetectorSurface::CexDex,
        ];
        assert_eq!(all.len(), 10);
        assert_eq!(surface_data_classes(DetectorSurface::DexAmm).len(), 1);
        for s in all.iter().skip(1) {
            assert!(surface_data_classes(*s).is_empty());
        }
    }
}
