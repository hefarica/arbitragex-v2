//! Canonical enums — workbook ULTRA sheet `10_LISTAS` (XLS-ENUM-01).
//!
//! Single-source-of-truth vocabulary for the two lists whose exact tokens the
//! repo did not yet carry (`ALGORITHMS`, `DEXES`). The runtime lists that
//! already have canonical homes are NOT duplicated here:
//!   - `FINANCING_MODES` / `EXECUTION_MODES` → [`crate::canonical_knobs`]
//!     (the 43-knob surface).
//!   - `ROUTE_KINDS` / `SURFACES` / `STRATEGY_IDS` / `DETECTOR_IDS` → carried
//!     by their real consumers (route dispatch, capability matrix, cartridges).
//!
//! Doc-map below ties each token to its implementation site so the vocabulary
//! can never silently drift from what actually runs. A token whose
//! implementation lands later is listed with its knob (gated deployment),
//! never dropped from the canonical set.

/// `10_LISTAS` column D — the 7 canonical route-discovery algorithms.
///
/// | Token | Implementation |
/// |-------|----------------|
/// | `BOUNDED_DFS` | `route_discovery::multi_hop_search::find_profitable_cycles` (bounded DFS cycle enumeration; telemetry tag `dfs_bounded` in `route_discovery_worker::ALGORITHM`) |
/// | `JOHNSON` | knob-gated (`ARBX_KNOB_ENABLE_JOHNSON`, default off) — Johnson's cycle enumeration |
/// | `BFM_NEG_CYCLE` | Bellman-Ford-Moore negative-cycle detection over the `−ln(rate)` weights (`graph_builder::log_weight`) — the `enable_bfm` pass |
/// | `MMBF_LINE_GRAPH` | knob-gated (`ARBX_KNOB_ENABLE_MMBF`) — MMBF line-graph pass |
/// | `RICH` | knob-gated (`ARBX_KNOB_ENABLE_RICH`) — real-time negative-cycle k-hop prioritization |
/// | `CONVEX_SIZE` | `SizeOptimizer` convex sizing (post-discovery; knob `enable_convex_size`) |
/// | `MPO` | Marginal Price Optimization — post-discovery sizing/ranking input |
pub const ALGORITHMS: [&str; 7] = [
    "BOUNDED_DFS",
    "JOHNSON",
    "BFM_NEG_CYCLE",
    "MMBF_LINE_GRAPH",
    "RICH",
    "CONVEX_SIZE",
    "MPO",
];

/// `10_LISTAS` column F — the 6 canonical DEX display names (workbook spelling,
/// incl. spaces). The runtime registry stays DB-driven (`dexes` table +
/// `GET /api/v1/dexes`); this list is the canonical display vocabulary that
/// seeds/validates it, so "Sushi V2" never drifts to "SushiSwap" in canonical
/// surfaces.
pub const DEXES: [&str; 6] = [
    "Uniswap V2",
    "Uniswap V3",
    "Sushi V2",
    "Curve",
    "Balancer",
    "Aerodrome",
];

/// Whether `symbol` is one of the canonical DEX display names (exact,
/// case-sensitive — workbook tokens).
pub fn is_canonical_dex(symbol: &str) -> bool {
    DEXES.contains(&symbol)
}

/// Whether `name` is one of the canonical algorithm tokens.
pub fn is_canonical_algorithm(name: &str) -> bool {
    ALGORITHMS.contains(&name)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Pins the EXACT workbook 10_LISTAS column D ordering — any drift from
    /// the canonical sheet is a compile-time-visible failure.
    #[test]
    fn algorithms_match_workbook_exactly() {
        assert_eq!(
            ALGORITHMS,
            [
                "BOUNDED_DFS",
                "JOHNSON",
                "BFM_NEG_CYCLE",
                "MMBF_LINE_GRAPH",
                "RICH",
                "CONVEX_SIZE",
                "MPO",
            ]
        );
    }

    /// Pins the EXACT workbook 10_LISTAS column F ordering (display spelling
    /// with spaces: "Sushi V2", not "SushiSwap").
    #[test]
    fn dexes_match_workbook_exactly() {
        assert_eq!(
            DEXES,
            [
                "Uniswap V2",
                "Uniswap V3",
                "Sushi V2",
                "Curve",
                "Balancer",
                "Aerodrome"
            ]
        );
    }

    #[test]
    fn membership_checks() {
        assert!(is_canonical_dex("Sushi V2"));
        assert!(!is_canonical_dex("SushiSwap")); // drift spelling is NOT canonical
        assert!(is_canonical_algorithm("BFM_NEG_CYCLE"));
        assert!(!is_canonical_algorithm("bellman")); // implementation word ≠ canonical token
    }
}
