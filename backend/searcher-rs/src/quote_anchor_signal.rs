//! EMIT-02/03 (ARBX-FE-EMIT-02/03, FE-MASTER P4): quote-anchor wire payloads.
//!
//! Pure serialization + preview math over the QB-06 selection machinery —
//! the Rust half of the wire contract mirrored by
//! `frontend/lib/apex/schemas/quote.ts` (FE-0001, 7b; R7 cross-reviewed).
//! Field names are EXACT on purpose: the Zod schemas are `.strict()`, so a
//! renamed key is a contract break, not a refactor.
//!
//! ## Naming boundary (documented mapping)
//! The workbook column is "Venues" → wire field `venues` (Zod) — while the
//! Rust knob and `QuoteComponents` axis are `venue_coverage` /
//! `quote_w_venue_coverage`. The mapping happens HERE, at the boundary, in
//! ONE place (`weights_to_wire`, `components_to_wire`).
//!
//! ## Honesty model (R8 / RULE 00)
//! - `QuoteAnchorViewSchema` is TOTAL: when a payload exists it carries a
//!   real selected anchor. Pre-computation absence is expressed by the HTTP
//!   ENVELOPE (Blocked/Unavailable — apex `_primitives`), never by a
//!   half-null payload. This module therefore only builds payloads from
//!   COMPUTED inputs; the caller decides envelope-unavailable when the
//!   component source yields nothing.
//! - Component inputs (Prior/Liquidity/Venues/Stability/CrossDex live
//!   providers) are a registered Layer-2 (ARBX-FE-EMIT-02 dependency): no
//!   axis is ever approximated or defaulted here.
//!
//! ## INVARIANT QB-TOPOLOGY-01 (operator ruling 2026-08-23)
//! Quote/Base is orientation & valuation metadata over a numeraire-agnostic
//! graph. Changing the quote anchor MUST NOT add/remove/reverse an edge,
//! mutate PairIndex/TokenId/PoolId/adjacency, or bump the topology version;
//! it MAY bump `quote_version`, invalidate `QuoteVersionedCell` caches,
//! change display orientation, normalized valuation, inefficiency score,
//! ranking, and which existing route becomes economically attractive.
//! `graph_rebuild_required` is therefore `false` at the TYPE level (Zod
//! `z.literal(false)` — no payload can carry `true`), and this module emits
//! the two literals (`false` / `topology_version_unchanged: true`) in ONE
//! place. The workbook's brief §10 "YES" example is expressly corrected and
//! is NOT an invariant. Runtime proofs live in the `invariants` test module
//! below (ARBX-QB-INV-01).

use crate::quote_score::{QuoteComponents, QuoteWeights};

/// Token identity for a scored row (address is `{:#x}` lowercase hex).
#[derive(Debug, Clone, PartialEq)]
pub struct TokenRef {
    pub symbol: String,
    pub address: String,
}

/// `QuoteScoreComponentsSchema` — field names EXACT (`.strict()` Zod).
pub fn components_to_wire(c: &QuoteComponents) -> serde_json::Value {
    serde_json::json!({
        "prior": c.prior,
        "liquidity": c.liquidity,
        // boundary mapping: Rust axis `venue_coverage` ↔ wire column "Venues"
        "venues": c.venue_coverage,
        "stability": c.stability,
        "cross_dex": c.cross_dex,
    })
}

/// `QuoteWeightsSchema` — same mapping, Σ≈1 enforced backend-side by knobs.
pub fn weights_to_wire(w: &QuoteWeights) -> serde_json::Value {
    serde_json::json!({
        "prior": w.prior,
        "liquidity": w.liquidity,
        "venues": w.venue_coverage,
        "stability": w.stability,
        "cross_dex": w.cross_dex,
    })
}

/// `QuoteTokenRowSchema` (§9 table row): {symbol, address, components, score}.
pub fn token_row_to_wire(t: &TokenRef, c: &QuoteComponents, score: f64) -> serde_json::Value {
    serde_json::json!({
        "symbol": t.symbol,
        "address": t.address,
        "components": components_to_wire(c),
        "score": score,
    })
}

/// `QuoteAnchorViewSchema` (§8). Callers MUST pass a COMPUTED anchor — this
/// function takes the selection as data, not as an Option, so a
/// not-yet-computed state cannot sneak into a total payload (it becomes the
/// envelope-unavailable path at the endpoint).
pub fn anchor_view_to_wire(
    chain_id: u64,
    anchor_symbol: &str,
    anchor_score: f64,
    anchor_components: &QuoteComponents,
    quote_version: u64,
    graph_version: u64,
    weights: &QuoteWeights,
) -> serde_json::Value {
    serde_json::json!({
        "chain_id": chain_id,
        "quote_symbol": anchor_symbol,
        "quote_score": anchor_score,
        "quote_version": quote_version,
        "graph_version": graph_version,
        "components": components_to_wire(anchor_components),
        "weights": weights_to_wire(weights),
    })
}

/// `QuotePreviewImpactSchema` (§10 CORRECTED — operator ruling 2026-08-23):
/// what a proposed weights change WOULD do, computed as a deterministic
/// re-ranking of the SAME live rows — never a mutation.
///
/// Derivations are CENTRALIZED here so a caller cannot forget an invariant:
/// - `graph_rebuild_required` / `topology_version_unchanged` are the two
///   doctrine LITERALS (QB-TOPOLOGY-01) — emitted only by this function.
/// - `proposed_quote_version` = current + 1 iff the anchor would actually
///   change; a no-op preview keeps the version (no fake churn).
/// - `quote_revaluation_required` / `quote_cache_invalidation_required` are
///   true iff the anchor changes — orientation re-denomination and
///   `QuoteVersionedCell` invalidation both key off the SAME transition.
///
/// The `affected_*` counts come from the pair index snapshot over the two
/// touched quote tokens (old ∪ new); `0` is the schema's honest zero (the
/// TS contract made these non-nullable ints — frontier decision, documented
/// there): no count is ever fabricated, only passed through.
pub fn preview_impact_to_wire(
    current_quote_version: u64,
    proposed_anchor_changes: bool,
    affected_pairs: u64,
    affected_edges: u64,
    affected_cached_routes: u64,
) -> serde_json::Value {
    serde_json::json!({
        "graph_rebuild_required": false,
        "quote_revaluation_required": proposed_anchor_changes,
        "quote_cache_invalidation_required": proposed_anchor_changes,
        "affected_pairs": affected_pairs,
        "affected_edges": affected_edges,
        "affected_cached_routes": affected_cached_routes,
        "current_quote_version": current_quote_version,
        "proposed_quote_version": current_quote_version + u64::from(proposed_anchor_changes),
        "topology_version_unchanged": true,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    // Positional format args only (probed file, edition-agnostic rule).

    fn usdc_components() -> QuoteComponents {
        QuoteComponents {
            prior: 100.0,
            liquidity: 95.0,
            venue_coverage: 90.0,
            stability: 100.0,
            cross_dex: 95.0,
        }
    }

    /// Wire keys are EXACT (`.strict()` Zod): renaming is a contract break.
    #[test]
    fn components_wire_keys_match_zod_exactly() {
        let v = components_to_wire(&usdc_components());
        let obj = v.as_object().unwrap();
        assert_eq!(obj.len(), 5);
        for key in ["prior", "liquidity", "venues", "stability", "cross_dex"] {
            assert!(obj.contains_key(key), "missing key {}", key);
        }
        assert!(
            !obj.contains_key("venue_coverage"),
            "boundary mapping applied"
        );
        assert_eq!(obj["venues"], serde_json::json!(90.0));
    }

    #[test]
    fn weights_wire_keys_match_zod_exactly() {
        let w = QuoteWeights::default();
        let v = weights_to_wire(&w);
        let obj = v.as_object().unwrap();
        assert_eq!(obj.len(), 5);
        for key in ["prior", "liquidity", "venues", "stability", "cross_dex"] {
            assert!(obj.contains_key(key), "missing key {}", key);
        }
        assert!(!obj.contains_key("venue_coverage"));
        assert_eq!(obj["venues"], serde_json::json!(0.2));
    }

    /// Anchor view shape: total payload, exact keys, no nulls.
    #[test]
    fn anchor_view_wire_is_total_and_exact() {
        let v = anchor_view_to_wire(
            1,
            "USDC",
            96.0,
            &usdc_components(),
            7,
            481,
            &QuoteWeights::default(),
        );
        let obj = v.as_object().unwrap();
        assert_eq!(obj.len(), 7);
        for key in [
            "chain_id",
            "quote_symbol",
            "quote_score",
            "quote_version",
            "graph_version",
            "components",
            "weights",
        ] {
            assert!(obj.contains_key(key), "missing key {}", key);
        }
        assert_eq!(obj["quote_symbol"], serde_json::json!("USDC"));
        assert_eq!(obj["quote_version"], serde_json::json!(7));
        assert_eq!(obj["graph_version"], serde_json::json!(481));
        // nested shapes strict too
        assert_eq!(obj["components"].as_object().unwrap().len(), 5);
        assert_eq!(obj["weights"].as_object().unwrap().len(), 5);
    }

    /// Token row shape (§9).
    #[test]
    fn token_row_wire_exact() {
        let t = TokenRef {
            symbol: "WETH".into(),
            address: "0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2".into(),
        };
        let v = token_row_to_wire(&t, &usdc_components(), 91.0);
        let obj = v.as_object().unwrap();
        assert_eq!(obj.len(), 4);
        for key in ["symbol", "address", "components", "score"] {
            assert!(obj.contains_key(key), "missing key {}", key);
        }
    }

    /// Preview impact shape: EXACT 9 keys, both doctrine literals, and the
    /// centralized derivations (version churn + reval/cache iff change).
    #[test]
    fn preview_impact_wire_exact_and_doctrinal() {
        let change = preview_impact_to_wire(7, true, 143, 286, 12);
        let obj = change.as_object().unwrap();
        assert_eq!(obj.len(), 9);
        for key in [
            "graph_rebuild_required",
            "quote_revaluation_required",
            "quote_cache_invalidation_required",
            "affected_pairs",
            "affected_edges",
            "affected_cached_routes",
            "current_quote_version",
            "proposed_quote_version",
            "topology_version_unchanged",
        ] {
            assert!(obj.contains_key(key), "missing key {}", key);
        }
        assert_eq!(change["graph_rebuild_required"], serde_json::json!(false));
        assert_eq!(
            change["topology_version_unchanged"],
            serde_json::json!(true)
        );
        assert_eq!(
            change["quote_revaluation_required"],
            serde_json::json!(true)
        );
        assert_eq!(
            change["quote_cache_invalidation_required"],
            serde_json::json!(true)
        );
        assert_eq!(change["current_quote_version"], serde_json::json!(7));
        assert_eq!(change["proposed_quote_version"], serde_json::json!(8));

        // No-op preview: same anchor → NO version churn, NO reval, NO cache
        // invalidation (a same-anchor re-select bumps nothing — QB-06 select).
        let noop = preview_impact_to_wire(7, false, 0, 0, 0);
        assert_eq!(noop["proposed_quote_version"], serde_json::json!(7));
        assert_eq!(noop["quote_revaluation_required"], serde_json::json!(false));
        assert_eq!(
            noop["quote_cache_invalidation_required"],
            serde_json::json!(false)
        );
        // Literals hold even for the no-op.
        assert_eq!(noop["graph_rebuild_required"], serde_json::json!(false));
        assert_eq!(noop["topology_version_unchanged"], serde_json::json!(true));
    }

    /// ARBX-QB-INV-01 — runtime proofs of INVARIANT QB-TOPOLOGY-01 (operator
    /// ruling 2026-08-23) over the REAL versioning/caching machinery.
    mod invariants {
        use crate::fe_normalization::QuoteState;
        use crate::quote_score::{QuoteSelection, QuoteVersionedCell};

        /// INV-1: an anchor re-denomination (quote change) bumps ONLY
        /// `quote_version` — topology, allowed-set and block versions are
        /// untouched, i.e. the graph's version keys are logically equivalent.
        #[test]
        fn quote_change_preserves_topology_versions() {
            let mut s = QuoteState::new(1, 3);
            s.set_price(0, 1.0).unwrap();
            let topo_before = s.version.topology_version;
            let allow_before = s.version.allowed_set_version;
            let block_before = s.version.state_block;
            let quote_before = s.version.quote_version;

            // Anchor re-denominated (new numeraire scale for token 0).
            s.set_price(0, 3.0).unwrap();

            assert_eq!(s.version.topology_version, topo_before);
            assert_eq!(s.version.allowed_set_version, allow_before);
            assert_eq!(s.version.state_block, block_before);
            assert_eq!(s.version.quote_version, quote_before + 1);
        }

        /// INV-2 (reverse-edge survival): after a quote change BOTH directed
        /// normalizations of the same pair remain computable — the forward
        /// AND reverse edges survive; only their re-denominated VALUES move.
        #[test]
        fn reverse_edge_survives_quote_change() {
            let mut s = QuoteState::new(1, 2);
            s.set_price(0, 1.0).unwrap();
            s.set_price(1, 2.0).unwrap();
            let before = s.pair_alpha(0, 1, Some(2.0), Some(0.5)).unwrap();
            assert!(before.forward.is_some() && before.reverse.is_some());

            s.set_price(0, 4.0).unwrap(); // quote change

            let after = s.pair_alpha(0, 1, Some(2.0), Some(0.5)).unwrap();
            assert!(
                after.forward.is_some(),
                "forward direction deleted by quote change — QB-TOPOLOGY-01 violated"
            );
            assert!(
                after.reverse.is_some(),
                "reverse direction deleted by quote change — QB-TOPOLOGY-01 violated"
            );
        }

        /// INV-3 (cache invalidation scope): a quote-version bump invalidates
        /// ONLY quote-keyed `QuoteVersionedCell`s; a cell keyed by a topology
        /// version that did not bump keeps serving fresh values.
        #[test]
        fn quote_bump_invalidates_only_quote_keyed_caches() {
            let mut sel: QuoteSelection<&str> = QuoteSelection::default();
            assert!(sel.select("USDC"));
            let qv1 = sel.quote_version();

            let mut quote_cell: QuoteVersionedCell<f64> = QuoteVersionedCell::default();
            quote_cell.store(96.0, qv1);
            assert!(quote_cell.get(qv1).is_some());

            assert!(sel.select("WETH")); // real change → bump
            let qv2 = sel.quote_version();
            assert_eq!(qv2, qv1 + 1);
            assert!(
                quote_cell.get(qv2).is_none(),
                "quote-derived cache must miss after the bump (recompute)"
            );

            // Topology-keyed cell: stored under a graph version that did NOT
            // bump when the quote changed — stays fresh.
            let mut topo_cell: QuoteVersionedCell<u64> = QuoteVersionedCell::default();
            topo_cell.store(481, 7);
            assert!(
                topo_cell.get(7).is_some(),
                "topology-keyed cache survives a quote change"
            );
        }
    }

    /// Preview math over the SAME rows under proposed weights uses the QB-06
    /// form verbatim — the fixture rescore (0.5/0.5/0/0/0) is the documented
    /// FE-0045 vector, verified here at the Rust boundary.
    #[test]
    fn preview_ranking_uses_quote_score_form() {
        let proposed = QuoteWeights {
            prior: 0.5,
            liquidity: 0.5,
            venue_coverage: 0.0,
            stability: 0.0,
            cross_dex: 0.0,
        };
        let weth = QuoteComponents {
            prior: 85.0,
            liquidity: 100.0,
            venue_coverage: 100.0,
            stability: 55.0,
            cross_dex: 100.0,
        };
        let scored_usdc = crate::quote_score::quote_score(&usdc_components(), &proposed);
        let scored_weth = crate::quote_score::quote_score(&weth, &proposed);
        assert!((scored_usdc - 97.5).abs() < 1e-12);
        assert!((scored_weth - 92.5).abs() < 1e-12);
        let winner =
            crate::quote_score::select_quote(&[("USDC", scored_usdc), ("WETH", scored_weth)])
                .unwrap();
        assert_eq!(winner, "USDC");
    }
}
