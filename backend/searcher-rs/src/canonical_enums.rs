//! Canonical enums — workbook ULTRA sheet `10_LISTAS` (XLS-ENUM-01).
//!
//! Single-source-of-truth vocabulary for the lists whose exact tokens the
//! repo did not yet carry (`ALGORITHMS`, `DEXES`; then `SURFACES`,
//! `GRAPH_MODELS`). The runtime lists that already have canonical homes are
//! NOT duplicated here:
//!   - `FINANCING_MODES` / `EXECUTION_MODES` → [`crate::canonical_knobs`]
//!     (the 43-knob surface).
//!   - `ROUTE_KINDS` / `STRATEGY_IDS` / `DETECTOR_IDS` → carried
//!     by their real consumers (route dispatch, capability matrix, cartridges).
//!
//! XLS-QB-02 added the vocabulary of workbook QUOTEBASE-264
//! (`11_STRATEGY_HOP_MAP`): [`SURFACES`] ×10 and its positionally-paired
//! [`GRAPH_MODELS`] ×10; the 264-row static Strategy×Hop mask table lives in
//! its own module ([`crate::strategy_hop_mask`]).
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

/// Workbook QUOTEBASE-264 `11_STRATEGY_HOP_MAP` column `Surface` — the 10
/// canonical surfaces, in first-seen workbook row order. Verified bijective
/// (1:1, positionally) with [`GRAPH_MODELS`] across all 264 strategy rows.
///
/// | Surface | Strategies | Graph model |
/// |---------|-----------:|-------------|
/// | `DEX_AMM` | 53 | `TOKEN_MULTIGRAPH` |
/// | `DEX_STATE` | 31 | `TOKEN_MULTIGRAPH_EVENT` |
/// | `PARITY_REDEMPTION` | 31 | `TOKEN_ACTION_GRAPH` |
/// | `CEX_DEX` | 14 | `HYBRID_MARKET_GRAPH` |
/// | `CROSS_CHAIN` | 30 | `DOMAIN_MULTIGRAPH` |
/// | `DERIVATIVES` | 30 | `INSTRUMENT_ACTION_GRAPH` |
/// | `LENDING` | 25 | `POSITION_ACTION_GRAPH` |
/// | `INTENT_AUCTION` | 20 | `ORDER_HYPERGRAPH` |
/// | `NFT` | 18 | `ASSET_ACTION_GRAPH` |
/// | `PREDICTION` | 12 | `CLAIM_ACTION_GRAPH` |
pub const SURFACES: [&str; 10] = [
    "DEX_AMM",
    "DEX_STATE",
    "PARITY_REDEMPTION",
    "CEX_DEX",
    "CROSS_CHAIN",
    "DERIVATIVES",
    "LENDING",
    "INTENT_AUCTION",
    "NFT",
    "PREDICTION",
];

/// `11_STRATEGY_HOP_MAP` column `Graph_Model` — the 10 canonical graph models,
/// positionally paired with [`SURFACES`] (`SURFACES[i] ↔ GRAPH_MODELS[i]`).
pub const GRAPH_MODELS: [&str; 10] = [
    "TOKEN_MULTIGRAPH",
    "TOKEN_MULTIGRAPH_EVENT",
    "TOKEN_ACTION_GRAPH",
    "HYBRID_MARKET_GRAPH",
    "DOMAIN_MULTIGRAPH",
    "INSTRUMENT_ACTION_GRAPH",
    "POSITION_ACTION_GRAPH",
    "ORDER_HYPERGRAPH",
    "ASSET_ACTION_GRAPH",
    "CLAIM_ACTION_GRAPH",
];

/// Whether `surface` is one of the canonical surface tokens (exact,
/// case-sensitive — workbook tokens).
pub fn is_canonical_surface(surface: &str) -> bool {
    SURFACES.contains(&surface)
}

/// Whether `model` is one of the canonical graph-model tokens.
pub fn is_canonical_graph_model(model: &str) -> bool {
    GRAPH_MODELS.contains(&model)
}

/// The canonical graph model for a surface (`SURFACES[i] ↔ GRAPH_MODELS[i]`).
/// `None` for a non-canonical surface token — honest-empty, never a default.
pub fn graph_model_for_surface(surface: &str) -> Option<&'static str> {
    SURFACES
        .iter()
        .position(|s| *s == surface)
        .map(|i| GRAPH_MODELS[i])
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

    /// Pins the EXACT first-seen workbook row order of 11_STRATEGY_HOP_MAP.
    #[test]
    fn surfaces_match_workbook_exactly() {
        assert_eq!(
            SURFACES,
            [
                "DEX_AMM",
                "DEX_STATE",
                "PARITY_REDEMPTION",
                "CEX_DEX",
                "CROSS_CHAIN",
                "DERIVATIVES",
                "LENDING",
                "INTENT_AUCTION",
                "NFT",
                "PREDICTION",
            ]
        );
    }

    /// Pins the positional pairing — surface i ↔ graph model i, all 10.
    #[test]
    fn graph_models_match_workbook_exactly() {
        assert_eq!(
            GRAPH_MODELS,
            [
                "TOKEN_MULTIGRAPH",
                "TOKEN_MULTIGRAPH_EVENT",
                "TOKEN_ACTION_GRAPH",
                "HYBRID_MARKET_GRAPH",
                "DOMAIN_MULTIGRAPH",
                "INSTRUMENT_ACTION_GRAPH",
                "POSITION_ACTION_GRAPH",
                "ORDER_HYPERGRAPH",
                "ASSET_ACTION_GRAPH",
                "CLAIM_ACTION_GRAPH",
            ]
        );
        // The pairing is total: every canonical surface resolves, and the
        // mapping is its own inverse.
        for (surface, model) in SURFACES.iter().zip(GRAPH_MODELS.iter()) {
            assert_eq!(graph_model_for_surface(surface), Some(*model));
        }
    }

    #[test]
    fn surface_membership_and_unknowns() {
        assert!(is_canonical_surface("DEX_AMM"));
        assert!(!is_canonical_surface("DEX-AMM")); // drift spelling is NOT canonical
        assert!(is_canonical_graph_model("ORDER_HYPERGRAPH"));
        assert!(!is_canonical_graph_model("hypergraph")); // implementation word ≠ canonical token
        assert_eq!(graph_model_for_surface("NOT_A_SURFACE"), None);
    }
}
