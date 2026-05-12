//! Detector-level strategy classification enum.
//!
//! This is the **local** (searcher-rs) enum that classifies the *detected route shape*.
//! It is distinct from `shared_rs::contracts::StrategyKind`, which is the 5-variant
//! persisted enum that the PostgreSQL schema and API wire-format use.
//!
//! ## Naming rationale
//!
//! The local enum is named `StrategyLabel` (not `StrategyKind`) to avoid shadowing
//! `shared_rs::contracts::StrategyKind` in the same scope. Using the same name for
//! two conceptually different enums produces confusing diagnostics and import ambiguity.
//!
//! ## Mapping to the persisted enum (spec §3.1)
//!
//! | Detector variant (this) | Persisted variant    | DB string      |
//! |-------------------------|----------------------|----------------|
//! | `DexArbV2V2`            | `DexArb`             | `dex_arb`      |
//! | `DexArbV2V3`            | `DexArb`             | `dex_arb`      |
//! | `DexArbV3V2`            | `DexArb`             | `dex_arb`      |
//! | `DexArbV3V3`            | `DexArb`             | `dex_arb`      |
//! | `TriangularArb`         | `Triangular`         | `triangular`   |
//! | `FlashloanArb`          | `FlashloanArb`       | `flashloan_arb`|
//! | `Liquidation`           | `Liquidation`        | `liquidation`  |
//!
//! The DEX-variant granularity (v2v2 / v2v3 / v3v2 / v3v3) is surfaced
//! separately through `RoutePlan.strategy_kind` for analytics/UI without
//! requiring a DB migration.

use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;

/// Detector-level enum classifying the route shape observed in the mempool.
///
/// All four `DexArb*` variants round-trip to the same DB string (`dex_arb`) via
/// the persisted enum, but each has a distinct `as_str()` for analytics labels.
///
/// **Serde note**: Each variant carries an explicit `#[serde(rename = "...")]` so
/// JSON serialization matches `as_str()` exactly (e.g. `"dex_arb_v2v2"`, not
/// `"dex_arb_v2_v2"` which serde `snake_case` would produce from `DexArbV2V2`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum StrategyLabel {
    /// Two-leg arb where both pools are Uniswap V2-style (constant product).
    #[serde(rename = "dex_arb_v2v2")]
    DexArbV2V2,
    /// Two-leg arb: V2 input pool → V3 output pool.
    #[serde(rename = "dex_arb_v2v3")]
    DexArbV2V3,
    /// Two-leg arb: V3 input pool → V2 output pool.
    #[serde(rename = "dex_arb_v3v2")]
    DexArbV3V2,
    /// Two-leg arb where both pools are Uniswap V3-style (concentrated liquidity).
    #[serde(rename = "dex_arb_v3v3")]
    DexArbV3V3,
    /// Three-leg cycle on the same or mixed DEX family (A→B→C→A).
    #[serde(rename = "triangular_arb")]
    TriangularArb,
    /// Any base route wrapped in a flashloan capital source. The underlying
    /// `base_strategy` is stored separately in the `StrategyCandidate`.
    #[serde(rename = "flashloan_arb")]
    FlashloanArb,
    /// Aave V3 or Compound V2 liquidation — repay debt, claim collateral at the
    /// protocol-defined bonus.
    #[serde(rename = "liquidation")]
    Liquidation,
}

impl StrategyLabel {
    /// Returns the granular analytics string for this variant.
    ///
    /// All four `DexArb*` variants return distinct strings (`dex_arb_v2v2`, etc.)
    /// rather than the collapsed `dex_arb` DB string, so Prometheus labels and
    /// the `RoutePlan.strategy_kind` field carry full resolution.
    pub fn as_str(&self) -> &'static str {
        match self {
            StrategyLabel::DexArbV2V2 => "dex_arb_v2v2",
            StrategyLabel::DexArbV2V3 => "dex_arb_v2v3",
            StrategyLabel::DexArbV3V2 => "dex_arb_v3v2",
            StrategyLabel::DexArbV3V3 => "dex_arb_v3v3",
            StrategyLabel::TriangularArb => "triangular_arb",
            StrategyLabel::FlashloanArb => "flashloan_arb",
            StrategyLabel::Liquidation => "liquidation",
        }
    }

    /// Strict parse: returns `Some` only when `s` exactly matches a known
    /// granular detector string. Unknown strings return `None` — never panic.
    pub fn from_str_strict(s: &str) -> Option<Self> {
        match s {
            "dex_arb_v2v2" => Some(StrategyLabel::DexArbV2V2),
            "dex_arb_v2v3" => Some(StrategyLabel::DexArbV2V3),
            "dex_arb_v3v2" => Some(StrategyLabel::DexArbV3V2),
            "dex_arb_v3v3" => Some(StrategyLabel::DexArbV3V3),
            "triangular_arb" => Some(StrategyLabel::TriangularArb),
            "flashloan_arb" => Some(StrategyLabel::FlashloanArb),
            "liquidation" => Some(StrategyLabel::Liquidation),
            _ => None,
        }
    }

    /// Maps this detector-level variant to the corresponding persisted
    /// `shared_rs::contracts::StrategyKind` variant (5-variant DB enum).
    ///
    /// All four `DexArb*` variants map to `DexArb` to keep the DB schema stable
    /// (spec §3.1, §6.1 — no migration required).
    ///
    /// Takes `self` by value (`Copy` type — no overhead).
    ///
    /// Named `to_contract_strategy_kind` (not `to_persisted`) to make the
    /// semantic explicit: this converts to the shared_rs canonical enum used
    /// for PostgreSQL persistence and the API wire format.
    pub fn to_contract_strategy_kind(self) -> shared_rs::contracts::StrategyKind {
        match self {
            StrategyLabel::DexArbV2V2
            | StrategyLabel::DexArbV2V3
            | StrategyLabel::DexArbV3V2
            | StrategyLabel::DexArbV3V3 => shared_rs::contracts::StrategyKind::DexArb,
            StrategyLabel::TriangularArb => shared_rs::contracts::StrategyKind::Triangular,
            StrategyLabel::FlashloanArb => shared_rs::contracts::StrategyKind::FlashloanArb,
            StrategyLabel::Liquidation => shared_rs::contracts::StrategyKind::Liquidation,
        }
    }
}

impl fmt::Display for StrategyLabel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// `FromStr` uses the same strict mapping as `from_str_strict`.
/// Returns an error for any unrecognised string — never silently coerces.
impl FromStr for StrategyLabel {
    type Err = UnknownStrategyLabel;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        StrategyLabel::from_str_strict(s).ok_or_else(|| UnknownStrategyLabel(s.to_owned()))
    }
}

/// Error produced when `FromStr` receives an unrecognised strategy string.
#[derive(Debug, PartialEq, Eq)]
pub struct UnknownStrategyLabel(pub String);

impl fmt::Display for UnknownStrategyLabel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "unknown strategy label: {:?}", self.0)
    }
}

impl std::error::Error for UnknownStrategyLabel {}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use std::str::FromStr;

    // ── as_str string-table pin (spec §3.1) ──────────────────────────────────

    #[test]
    fn as_str_table_pinned() {
        let cases: &[(StrategyLabel, &str)] = &[
            (StrategyLabel::DexArbV2V2, "dex_arb_v2v2"),
            (StrategyLabel::DexArbV2V3, "dex_arb_v2v3"),
            (StrategyLabel::DexArbV3V2, "dex_arb_v3v2"),
            (StrategyLabel::DexArbV3V3, "dex_arb_v3v3"),
            (StrategyLabel::TriangularArb, "triangular_arb"),
            (StrategyLabel::FlashloanArb, "flashloan_arb"),
            (StrategyLabel::Liquidation, "liquidation"),
        ];
        for &(label, expected) in cases {
            assert_eq!(
                label.as_str(),
                expected,
                "as_str mismatch for {label:?}: expected {expected:?}"
            );
        }
    }

    // ── round-trip: from_str_strict(as_str()) == original ───────────────────

    #[test]
    fn as_str_roundtrip() {
        let all = [
            StrategyLabel::DexArbV2V2,
            StrategyLabel::DexArbV2V3,
            StrategyLabel::DexArbV3V2,
            StrategyLabel::DexArbV3V3,
            StrategyLabel::TriangularArb,
            StrategyLabel::FlashloanArb,
            StrategyLabel::Liquidation,
        ];
        for label in all {
            let s = label.as_str();
            let back = StrategyLabel::from_str_strict(s);
            assert_eq!(
                back,
                Some(label),
                "from_str_strict({s:?}) did not round-trip to {label:?}"
            );
        }
    }

    // ── FromStr round-trip ───────────────────────────────────────────────────

    #[test]
    fn from_str_roundtrip() {
        let all = [
            StrategyLabel::DexArbV2V2,
            StrategyLabel::DexArbV2V3,
            StrategyLabel::DexArbV3V2,
            StrategyLabel::DexArbV3V3,
            StrategyLabel::TriangularArb,
            StrategyLabel::FlashloanArb,
            StrategyLabel::Liquidation,
        ];
        for label in all {
            let s = label.as_str();
            let parsed = StrategyLabel::from_str(s);
            assert!(parsed.is_ok(), "FromStr failed for {s:?}");
            assert_eq!(parsed.unwrap(), label);
        }
    }

    // ── unknown string → None / Err ──────────────────────────────────────────

    #[test]
    fn from_str_strict_unknown_returns_none() {
        let unknown = ["dex_arb", "dex_arb_v4v4", "", "LIQUIDATION", "flashloan"];
        for s in unknown {
            assert_eq!(
                StrategyLabel::from_str_strict(s),
                None,
                "from_str_strict should return None for unknown string {s:?}"
            );
        }
    }

    #[test]
    fn from_str_unknown_returns_err() {
        let err = StrategyLabel::from_str("dex_arb");
        assert!(
            err.is_err(),
            "FromStr should error on collapsed DB string 'dex_arb'"
        );
    }

    // ── four DexArb variants all map to DexArb in persisted enum ────────────

    #[test]
    fn dex_arb_variants_map_to_contract_strategy_kind_dex_arb() {
        use shared_rs::contracts::StrategyKind as Persisted;
        let dex_arb_variants = [
            StrategyLabel::DexArbV2V2,
            StrategyLabel::DexArbV2V3,
            StrategyLabel::DexArbV3V2,
            StrategyLabel::DexArbV3V3,
        ];
        for v in dex_arb_variants {
            assert_eq!(
                v.to_contract_strategy_kind(),
                Persisted::DexArb,
                "{v:?}.to_contract_strategy_kind() must be DexArb (DB string = 'dex_arb')"
            );
        }
    }

    // ── non-DexArb variants map to their own persisted variants ─────────────

    #[test]
    fn non_dex_arb_contract_strategy_kind_mapping() {
        use shared_rs::contracts::StrategyKind as Persisted;
        assert_eq!(
            StrategyLabel::TriangularArb.to_contract_strategy_kind(),
            Persisted::Triangular
        );
        assert_eq!(
            StrategyLabel::FlashloanArb.to_contract_strategy_kind(),
            Persisted::FlashloanArb
        );
        assert_eq!(
            StrategyLabel::Liquidation.to_contract_strategy_kind(),
            Persisted::Liquidation
        );
    }

    // ── Display uses as_str ──────────────────────────────────────────────────

    #[test]
    fn display_uses_as_str() {
        assert_eq!(StrategyLabel::DexArbV3V3.to_string(), "dex_arb_v3v3");
        assert_eq!(StrategyLabel::TriangularArb.to_string(), "triangular_arb");
    }

    // ── serde round-trip (JSON) ──────────────────────────────────────────────

    #[test]
    fn serde_json_roundtrip() {
        let label = StrategyLabel::DexArbV2V3;
        let json = serde_json::to_string(&label).expect("serialize");
        // Explicit #[serde(rename = "dex_arb_v2v3")] on the variant ensures
        // the JSON representation matches as_str() exactly.
        assert_eq!(json, r#""dex_arb_v2v3""#, "serde JSON must match as_str()");
        let back: StrategyLabel = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(back, label);
    }

    #[test]
    fn serde_json_all_variants_match_as_str() {
        let all = [
            StrategyLabel::DexArbV2V2,
            StrategyLabel::DexArbV2V3,
            StrategyLabel::DexArbV3V2,
            StrategyLabel::DexArbV3V3,
            StrategyLabel::TriangularArb,
            StrategyLabel::FlashloanArb,
            StrategyLabel::Liquidation,
        ];
        for label in all {
            let json = serde_json::to_string(&label).expect("serialize");
            let expected_json = format!(r#""{}""#, label.as_str());
            assert_eq!(
                json, expected_json,
                "serde JSON for {label:?} must match as_str()"
            );
        }
    }
}
