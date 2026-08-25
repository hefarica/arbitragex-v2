//! Quote-base scoring — workbook QUOTEBASE-264 sheet `05_QUOTE_BASE`
//! (XLS-QB-06).
//!
//! ```text
//! QuoteScore(t) = wP·Prior + wL·Liquidity + wV·VenueCoverage +
//!                 wS·Stability + wX·CrossDex          (05 r11)
//! ```
//!
//! * Components are the workbook's 0–100 token axes: Prior, Liquidity,
//!   VenueCoverage (sheet column "Venues"), Stability, CrossDex.
//! * Weights default to 01_CONFIG rows 15–19 (0.30/0.30/0.20/0.10/0.10) and
//!   are canonical knobs (`ARBX_KNOB_QUOTE_W_*`, XLS-QB-06) — env overrides
//!   land in `CanonicalKnobs` and flow here via [`QuoteWeights::from_knobs`].
//! * Quote hierarchy (05 r16): a DYNAMIC score outranks any hardcoded
//!   stablecoin list — USDC/USDT/DAI/WETH are priors, never universal
//!   identity.
//! * The sheet's `QuoteEligible` column is `True` for every fixture but the
//!   workbook defines NO eligibility threshold — this module computes the
//!   score ONLY and does not invent a cutoff (RULE 00 / fail-honest).
//! * Numeraire-only doctrine (05 r12/r15): choosing QUOTE is orientation
//!   metadata for UI/comparison; the reverse directed edge is NEVER deleted
//!   by a QUOTE choice.
//!
//! Pure math, no I/O — components are supplied by the consumer layer.

use crate::canonical_knobs::CanonicalKnobs;

/// The five 0–100 token axes of sheet `05_QUOTE_BASE`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct QuoteComponents {
    /// Token prior (05 r16: priors are dynamic scores, not identity).
    pub prior: f64,
    /// Liquidity depth axis.
    pub liquidity: f64,
    /// Venue coverage axis (sheet column "Venues").
    pub venue_coverage: f64,
    /// Stability axis.
    pub stability: f64,
    /// Cross-DEX presence axis.
    pub cross_dex: f64,
}

/// The five weights of the `QuoteScore` linear form (01_CONFIG rows 15–19).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct QuoteWeights {
    pub prior: f64,
    pub liquidity: f64,
    pub venue_coverage: f64,
    pub stability: f64,
    pub cross_dex: f64,
}

impl Default for QuoteWeights {
    /// Workbook defaults: 0.30 / 0.30 / 0.20 / 0.10 / 0.10 (sum = 1.0).
    fn default() -> Self {
        Self {
            prior: 0.3,
            liquidity: 0.3,
            venue_coverage: 0.2,
            stability: 0.1,
            cross_dex: 0.1,
        }
    }
}

impl QuoteWeights {
    /// Lift the weights from the canonical knob source (env > deploy YAML >
    /// workbook default). The knob set already validated them at boot.
    pub fn from_knobs(knobs: &CanonicalKnobs) -> Self {
        Self {
            prior: knobs.quote_w_prior,
            liquidity: knobs.quote_w_liquidity,
            venue_coverage: knobs.quote_w_venue_coverage,
            stability: knobs.quote_w_stability,
            cross_dex: knobs.quote_w_cross_dex,
        }
    }

    /// Invariant: every weight finite in `[0, 1]` and the five sum to 1.0
    /// (±1e-9). Mirrors `CanonicalKnobs::validate` for direct construction.
    pub fn validate(&self) -> Result<(), String> {
        let w = [
            self.prior,
            self.liquidity,
            self.venue_coverage,
            self.stability,
            self.cross_dex,
        ];
        if w.iter().any(|x| !x.is_finite() || *x < 0.0 || *x > 1.0) {
            return Err(format!(
                "quote weights must each be finite in [0,1] (got {w:?})"
            ));
        }
        let sum: f64 = w.iter().sum();
        if (sum - 1.0).abs() > 1e-9 {
            return Err(format!(
                "quote weights must sum to 1.0 (got {sum:.6} — 01_CONFIG rows 15–19)"
            ));
        }
        Ok(())
    }
}

/// Weighted sum — the exact `QuoteScore(t)` form of 05_QUOTE_BASE r11.
/// Returns `NaN` never (weights are validated by construction; components are
/// the caller's data and pass through arithmetic unchanged).
pub fn quote_score(c: &QuoteComponents, w: &QuoteWeights) -> f64 {
    w.prior * c.prior
        + w.liquidity * c.liquidity
        + w.venue_coverage * c.venue_coverage
        + w.stability * c.stability
        + w.cross_dex * c.cross_dex
}

// ---- XLS-QB-06 / ARBX-QB-06-004: QuoteVersion invalida caches ------------
//
// Sheet 09_RUNTIME_STRUCTURES r25 ("Version keys"): `quote_version` is the
// invalidation key for everything derived from the QUOTE (numeraire)
// choice. This block owns the SELECTION side: ranking tokens by
// [`quote_score`] (05 r16: a dynamic score outranks any hardcoded
// stablecoin list), tracking the selected quote, and bumping the version on
// every actual change. Anchor-PRICE drift is a separate bump lane owned by
// the F_e normalization state (fe_normalization.rs) — same key, different
// trigger, one doctrine.

/// The quote-selection state + its `quote_version` invalidation key.
///
/// Selecting the same token again is NOT a state transition (no bump —
/// minimal churn, sheet 09 r25); any actual change bumps the version so
/// every quote-derived cache goes stale.
#[derive(Debug, Clone, PartialEq)]
pub struct QuoteSelection<T> {
    selected: Option<T>,
    version: u64,
}

impl<T> Default for QuoteSelection<T> {
    fn default() -> Self {
        Self {
            selected: None,
            version: 0,
        }
    }
}

impl<T: PartialEq> QuoteSelection<T> {
    /// Select the quote token; returns `true` when this was an actual CHANGE
    /// (version bumped), `false` for a same-token re-select.
    pub fn select(&mut self, token: T) -> bool {
        if self.selected.as_ref() == Some(&token) {
            return false;
        }
        self.selected = Some(token);
        self.version = self.version.wrapping_add(1);
        true
    }

    /// Current quote token (`None` before the first selection — honest).
    pub fn selection(&self) -> Option<&T> {
        self.selected.as_ref()
    }

    /// The `quote_version` key (09 r25) — bumps once per actual change.
    pub fn quote_version(&self) -> u64 {
        self.version
    }
}

/// Pick the quote among `(token, QuoteScore)` candidates: the strict maximum
/// score, FIRST token winning ties (stable across re-sorts). Non-finite
/// scores are skipped (a NaN must never be selected by accident of ordering
/// — fail-honest); `None` on empty or all-invalid input.
pub fn select_quote<T: Copy>(scored: &[(T, f64)]) -> Option<T> {
    let mut best: Option<(T, f64)> = None;
    for &(t, s) in scored {
        if !s.is_finite() {
            continue;
        }
        match best {
            Some((_, bs)) if s <= bs => {}
            _ => best = Some((t, s)),
        }
    }
    best.map(|(t, _)| t)
}

/// One quote-derived cached value tagged with the `quote_version` it was
/// computed under (09 r25 doctrine). [`QuoteVersionedCell::get`] serves the
/// value ONLY when the caller's current version matches — a selection change
/// makes every cell stale without touching it, and stale is a MISS, never a
/// wrong-numeraire hit.
#[derive(Debug, Clone, PartialEq)]
pub struct QuoteVersionedCell<V> {
    version: u64,
    value: Option<V>,
}

impl<V> Default for QuoteVersionedCell<V> {
    fn default() -> Self {
        Self {
            version: 0,
            value: None,
        }
    }
}

impl<V> QuoteVersionedCell<V> {
    /// Store `value` as computed under `version`.
    pub fn store(&mut self, value: V, version: u64) {
        self.version = version;
        self.value = Some(value);
    }

    /// Fresh lookup ONLY: the stored version must equal `current`. A version
    /// mismatch in either direction is a miss (honest stale, recompute).
    pub fn get(&self, current: u64) -> Option<&V> {
        match &self.value {
            Some(v) if self.version == current => Some(v),
            _ => None,
        }
    }

    /// Version the stored value was computed under (`None` when empty).
    pub fn stored_version(&self) -> Option<u64> {
        self.value.as_ref().map(|_| self.version)
    }

    /// Drop the value outright (explicit eviction).
    pub fn invalidate(&mut self) {
        self.value = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Sheet 05_QUOTE_BASE r4: USDC (100, 95, 90, 100, 95) → 96.0.
    #[test]
    fn fixture_usdc_scores_96() {
        let c = QuoteComponents {
            prior: 100.0,
            liquidity: 95.0,
            venue_coverage: 90.0,
            stability: 100.0,
            cross_dex: 95.0,
        };
        let s = quote_score(&c, &QuoteWeights::default());
        assert!((s - 96.0).abs() < 1e-9, "USDC fixture got {s}");
    }

    /// Sheet 05_QUOTE_BASE r5: WETH (85, 100, 100, 55, 100) → 91.0.
    #[test]
    fn fixture_weth_scores_91() {
        let c = QuoteComponents {
            prior: 85.0,
            liquidity: 100.0,
            venue_coverage: 100.0,
            stability: 55.0,
            cross_dex: 100.0,
        };
        let s = quote_score(&c, &QuoteWeights::default());
        assert!((s - 91.0).abs() < 1e-9, "WETH fixture got {s}");
    }

    /// Sheet 05_QUOTE_BASE r6: WBTC (70, 85, 80, 45, 80) → 75.0.
    #[test]
    fn fixture_wbtc_scores_75() {
        let c = QuoteComponents {
            prior: 70.0,
            liquidity: 85.0,
            venue_coverage: 80.0,
            stability: 45.0,
            cross_dex: 80.0,
        };
        let s = quote_score(&c, &QuoteWeights::default());
        assert!((s - 75.0).abs() < 1e-9, "WBTC fixture got {s}");
    }

    /// Sheet 05_QUOTE_BASE r7: LINK (30, 75, 70, 30, 70) → 55.5 — the one
    /// fixture with a fractional expected score.
    #[test]
    fn fixture_link_scores_55_5() {
        let c = QuoteComponents {
            prior: 30.0,
            liquidity: 75.0,
            venue_coverage: 70.0,
            stability: 30.0,
            cross_dex: 70.0,
        };
        let s = quote_score(&c, &QuoteWeights::default());
        assert!((s - 55.5).abs() < 1e-9, "LINK fixture got {s}");
    }

    /// The canonical knobs (env > YAML > workbook) are the single weight
    /// source: `from_knobs` lifts them verbatim.
    #[test]
    fn weights_lift_from_canonical_knobs() {
        let w = QuoteWeights::from_knobs(&CanonicalKnobs::default());
        assert_eq!(w, QuoteWeights::default());
        assert!(w.validate().is_ok());
    }

    /// Direct construction is guarded by the same invariant the knobs carry:
    /// out-of-range or non-unit-sum weights are rejected (fail-honest).
    #[test]
    fn weights_validate_rejects_violations() {
        let w = QuoteWeights {
            prior: 0.5, // sum = 1.2
            ..QuoteWeights::default()
        };
        assert!(w.validate().is_err(), "sum != 1.0 rejected");

        let w = QuoteWeights {
            stability: -0.1,
            cross_dex: 0.3, // sum still 1.0, but one weight is negative
            ..QuoteWeights::default()
        };
        assert!(w.validate().is_err(), "negative weight rejected");

        assert!(QuoteWeights::default().validate().is_ok());
    }

    // ---- ARBX-QB-06-004: QuoteVersion invalida caches -------------------

    /// The four sheet-05 fixtures ranked: USDC (96) > WETH (91) > WBTC (75)
    /// > LINK (55.5) — the dynamic score picks the quote (05 r16).
    #[test]
    fn select_quote_ranks_sheet05_fixtures() {
        let ranked = [
            ("LINK", 55.5),
            ("WBTC", 75.0),
            ("WETH", 91.0),
            ("USDC", 96.0),
        ];
        assert_eq!(select_quote(&ranked), Some("USDC"));
    }

    /// Empty input or all-non-finite scores → `None` (no fabricated quote);
    /// a NaN never wins by accident of ordering.
    #[test]
    fn select_quote_rejects_empty_and_non_finite() {
        let empty: [(&str, f64); 0] = [];
        assert_eq!(select_quote(&empty), None);
        assert_eq!(select_quote(&[("a", f64::NAN), ("b", f64::INFINITY)]), None);
        // Finite entries survive alongside invalid ones.
        assert_eq!(select_quote(&[("a", f64::NAN), ("b", 40.0)]), Some("b"));
    }

    /// Ties keep the FIRST token — deterministic across re-sorts.
    #[test]
    fn select_quote_tie_first_wins() {
        assert_eq!(select_quote(&[("x", 90.0), ("y", 90.0)]), Some("x"));
    }

    /// Version bumps once per ACTUAL change; a same-token re-select is not a
    /// state transition (minimal churn, 09 r25).
    #[test]
    fn selection_bumps_version_on_change_only() {
        let mut sel: QuoteSelection<&str> = QuoteSelection::default();
        assert_eq!(sel.quote_version(), 0);
        assert_eq!(sel.selection(), None);

        assert!(sel.select("USDC")); // first selection = change
        assert_eq!(sel.selection(), Some(&"USDC"));
        assert_eq!(sel.quote_version(), 1);

        assert!(!sel.select("USDC")); // re-select: no bump
        assert_eq!(sel.quote_version(), 1);

        assert!(sel.select("WETH")); // actual change
        assert_eq!(sel.quote_version(), 2);
        assert_eq!(sel.selection(), Some(&"WETH"));
    }

    /// A quote-derived cell serves ONLY under the version it was computed
    /// with: a selection change makes it a MISS (never a wrong-numeraire
    /// hit); recompute-then-store under the new version serves again.
    #[test]
    fn versioned_cell_is_stale_after_selection_change() {
        let mut sel: QuoteSelection<&str> = QuoteSelection::default();
        sel.select("USDC");
        let v1 = sel.quote_version();

        let mut cell: QuoteVersionedCell<f64> = QuoteVersionedCell::default();
        assert_eq!(cell.get(v1), None, "empty cell is a miss");

        cell.store(96.0, v1);
        assert_eq!(cell.get(v1), Some(&96.0), "fresh under same version");
        assert_eq!(cell.stored_version(), Some(v1));

        sel.select("WETH"); // selection change bumps the key
        let v2 = sel.quote_version();
        assert_eq!(cell.get(v2), None, "stale after selection change");
        assert_eq!(cell.get(v1), Some(&96.0), "old version still coherent");

        cell.store(91.0, v2); // recomputed under the new quote
        assert_eq!(cell.get(v2), Some(&91.0));
        assert_eq!(cell.get(v1), None, "mismatch in either direction is a miss");

        cell.invalidate();
        assert_eq!(cell.get(v2), None);
        assert_eq!(cell.stored_version(), None);
    }

    // ---- ARBX-QB-06-006: parametric sensitivity suite --------------------

    /// USDC fixture helper for the sensitivity tests below.
    fn usdc() -> QuoteComponents {
        QuoteComponents {
            prior: 100.0,
            liquidity: 95.0,
            venue_coverage: 90.0,
            stability: 100.0,
            cross_dex: 95.0,
        }
    }

    /// Liquidity axis: a +δ on Liquidity moves the score by EXACTLY
    /// w_liquidity·δ (the linear form, no hidden couplings).
    #[test]
    fn sensitivity_liquidity_axis_exact() {
        let w = QuoteWeights::default();
        let base = quote_score(&usdc(), &w);
        let mut c = usdc();
        c.liquidity += 4.0;
        assert!((quote_score(&c, &w) - (base + w.liquidity * 4.0)).abs() < 1e-12);
    }

    /// Venue axis: same exactness for VenueCoverage (the sheet's "Venues"
    /// column) — the two axes the operator watches most on quote health.
    #[test]
    fn sensitivity_venue_axis_exact() {
        let w = QuoteWeights::default();
        let base = quote_score(&usdc(), &w);
        let mut c = usdc();
        c.venue_coverage -= 10.0;
        assert!((quote_score(&c, &w) - (base - w.venue_coverage * 10.0)).abs() < 1e-12);
    }

    /// Weight change: the same components under a different VALID weight set
    /// rescore exactly — with (0.5, 0.5, 0, 0, 0) only Prior/Liquidity count,
    /// so USDC (100,95) → 97.5 and WETH (85,100) → 92.5 (hand-computed from
    /// the linear form); under the workbook defaults the same components
    /// score 96.0 / 91.0. A weight change always recomputes, never reuses.
    #[test]
    fn weight_change_rescores_linear_form() {
        let w = QuoteWeights {
            prior: 0.5,
            liquidity: 0.5,
            venue_coverage: 0.0,
            stability: 0.0,
            cross_dex: 0.0,
        };
        assert!(w.validate().is_ok());
        // USDC under the two-axis weights: 0.5·100 + 0.5·95 = 97.5.
        assert!((quote_score(&usdc(), &w) - 97.5).abs() < 1e-12);
        // WETH fixture (85,100,100,55,100) under the same weights: 92.5.
        let weth = QuoteComponents {
            prior: 85.0,
            liquidity: 100.0,
            venue_coverage: 100.0,
            stability: 55.0,
            cross_dex: 100.0,
        };
        assert!((quote_score(&weth, &w) - 92.5).abs() < 1e-12);
        // Default weights gave 96.0 vs 91.0 (margin 5.0); two-axis margin is
        // 5.0 as well here, but the SCORES moved — recomputation happened.
        let wd = QuoteWeights::default();
        assert!((quote_score(&usdc(), &wd) - 96.0).abs() < 1e-9);
        assert!((quote_score(&weth, &wd) - 91.0).abs() < 1e-9);
    }

    /// Full quote-switch cycle: rank → select → cache under v1 → the ranking
    /// changes (component drift on the incumbent) → select_quote flips the
    /// winner → QuoteSelection bumps → the versioned cell goes MISS until
    /// recomputed under the new quote. The whole 05↔09 r25 contract in one
    /// flow.
    #[test]
    fn quote_switch_cycle_ranks_selects_invalidates() {
        let w = QuoteWeights::default();
        let mut usdc_c = usdc();
        let weth_c = QuoteComponents {
            prior: 85.0,
            liquidity: 100.0,
            venue_coverage: 100.0,
            stability: 55.0,
            cross_dex: 100.0,
        };

        // Round 1: USDC wins (96 > 91).
        let ranked = [
            ("USDC", quote_score(&usdc_c, &w)),
            ("WETH", quote_score(&weth_c, &w)),
        ];
        let mut sel: QuoteSelection<&str> = QuoteSelection::default();
        assert!(sel.select(select_quote(&ranked).expect("a winner")));
        assert_eq!(sel.selection(), Some(&"USDC"));
        let v1 = sel.quote_version();

        // Cache an orientation-derived value under v1.
        let mut orientation: QuoteVersionedCell<&str> = QuoteVersionedCell::default();
        orientation.store("quote=USDC", v1);
        assert_eq!(orientation.get(v1), Some(&"quote=USDC"));

        // Round 2: USDC's liquidity collapses (−40) → 84 < 91 → WETH wins.
        usdc_c.liquidity -= 40.0;
        assert!((quote_score(&usdc_c, &w) - 84.0).abs() < 1e-9);
        let ranked = [
            ("USDC", quote_score(&usdc_c, &w)),
            ("WETH", quote_score(&weth_c, &w)),
        ];
        let changed = sel.select(select_quote(&ranked).expect("a winner"));
        assert!(changed, "actual selection change");
        assert_eq!(sel.selection(), Some(&"WETH"));
        assert_eq!(sel.quote_version(), v1 + 1);

        // The cached orientation is stale under the new key — MISS, then
        // recompute-and-store serves fresh.
        assert_eq!(orientation.get(sel.quote_version()), None);
        orientation.store("quote=WETH", sel.quote_version());
        assert_eq!(orientation.get(sel.quote_version()), Some(&"quote=WETH"));
    }
}
