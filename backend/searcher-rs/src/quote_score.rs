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
}
