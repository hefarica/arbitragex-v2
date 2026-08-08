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
    /// Graph-theoretic cycle detection via Bellman-Ford on token liquidity graphs.
    /// Detects negative cycles (Holonomic Loop Resolutions) using -ln(rate) edge weights.
    #[serde(rename = "spanning_tree_arb")]
    SpanningTreeArb,
    /// Cross-chain Holonomic Loop Resolution via bridge protocols (LayerZero, Wormhole).
    /// Detects price divergences across chains accounting for bridge fees and latency.
    #[serde(rename = "cross_chain_arb")]
    CrossChainArb,
    /// High-frequency liquidation sniping on Aave V3 and Compound V2.
    /// Monitors health_factor < 1.0 positions for profitable liquidation opportunities.
    #[serde(rename = "liquidation_snipe")]
    LiquidationSnipe,
}

impl StrategyLabel {
    /// Classify the strategy variant from a decoded pending swap WITHOUT
    /// fabrication. The mapping reflects what we can legitimately infer
    /// from a single observed transaction:
    ///
    ///   - The pending tx tells us the SOURCE pool's protocol (V2 or V3).
    ///   - We do NOT yet know what other pool the searcher will route
    ///     against; the engines (dex_engine, etc.) determine that.
    ///   - For the LEGACY V1 single-leg-RoutePlan path, we use the source
    ///     protocol on both legs (V2 source → DexArbV2V2, V3 source →
    ///     DexArbV3V3). Multi-hop swaps inside the same protocol family
    ///     still classify based on the source protocol.
    ///
    /// Returns `DexArbV2V2` as the safest legacy default when protocol is
    /// Unknown — this preserves bit-for-bit compatibility with the prior
    /// hardcoded behaviour while letting V3 swaps surface their true
    /// classification.
    ///
    /// This function is NOT the orchestrator's classification path —
    /// dex_engine does that with full impact data (source pool + other
    /// pools). This is the legacy fallback used by scanner.rs's V1 mode
    /// when the operator hasn't migrated to ARBX_ORCHESTRATOR_MODE=v2.
    pub fn classify_from_decoded(decoded: &crate::calldata::DecodedSwap) -> Self {
        use crate::calldata::ProtocolType;
        match decoded.protocol_type {
            ProtocolType::V2 => Self::DexArbV2V2,
            ProtocolType::V3 => Self::DexArbV3V3,
            // Curve, Balancer, Unknown: no V4/Curve/Balancer engines exist yet
            // in V1 path. Fall back to the most-likely arb target (V2/V2)
            // and let the gate evaluator decide if it matches a configured
            // strategy. Future phases add Curve/Balancer variants.
            _ => Self::DexArbV2V2,
        }
    }

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
            StrategyLabel::SpanningTreeArb => "spanning_tree_arb",
            StrategyLabel::CrossChainArb => "cross_chain_arb",
            StrategyLabel::LiquidationSnipe => "liquidation_snipe",
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
            "spanning_tree_arb" => Some(StrategyLabel::SpanningTreeArb),
            "cross_chain_arb" => Some(StrategyLabel::CrossChainArb),
            "liquidation_snipe" => Some(StrategyLabel::LiquidationSnipe),
            _ => None,
        }
    }

    /// Maps this detector-level variant to the corresponding persisted
    /// `shared_rs::contracts::StrategyKind` variant (5-variant DB enum).
    ///
    /// All four `DexArb*` variants map to `DexArb` to keep the DB schema stable
    /// (spec §3.1, §6.1 — no migration required).
    /// New experimental engines map to appropriate existing kinds:
    /// - SpanningTreeArb → Triangular (similar graph-cycle nature)
    /// - CrossChainArb → DexArb (cross-DEX arbitrage pattern)
    /// - LiquidationSnipe → Liquidation (same protocol family)
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
            | StrategyLabel::DexArbV3V3
            | StrategyLabel::CrossChainArb => shared_rs::contracts::StrategyKind::dex_arb(),
            StrategyLabel::TriangularArb | StrategyLabel::SpanningTreeArb => {
                shared_rs::contracts::StrategyKind::triangular()
            }
            StrategyLabel::FlashloanArb => shared_rs::contracts::StrategyKind::flashloan_arb(),
            StrategyLabel::Liquidation | StrategyLabel::LiquidationSnipe => {
                shared_rs::contracts::StrategyKind::liquidation()
            }
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
                Persisted::dex_arb(),
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
            Persisted::triangular()
        );
        assert_eq!(
            StrategyLabel::FlashloanArb.to_contract_strategy_kind(),
            Persisted::flashloan_arb()
        );
        assert_eq!(
            StrategyLabel::Liquidation.to_contract_strategy_kind(),
            Persisted::liquidation()
        );
    }

    // ── classify_from_decoded — protocol_type → StrategyLabel ────────────────

    fn make_decoded(protocol_type: crate::calldata::ProtocolType) -> crate::calldata::DecodedSwap {
        use ethers::types::{Address, U256};
        crate::calldata::DecodedSwap {
            router: "test",
            token_in: Address::zero(),
            token_out: Address::zero(),
            amount_in: U256::zero(),
            min_amount_out: U256::zero(),
            path_len: 2,
            deadline: U256::zero(),
            recipient: Address::zero(),
            selector_hex: "00000000".to_string(),
            path_tokens: vec![Address::zero(), Address::zero()],
            path_fees_bps: vec![30],
            exact_mode: crate::calldata::SwapExactMode::ExactIn,
            protocol_type,
        }
    }

    #[test]
    fn classify_v2_protocol_returns_v2v2() {
        use crate::calldata::ProtocolType;
        let decoded = make_decoded(ProtocolType::V2);
        assert_eq!(
            StrategyLabel::classify_from_decoded(&decoded),
            StrategyLabel::DexArbV2V2,
            "V2 source protocol must classify as DexArbV2V2"
        );
    }

    #[test]
    fn classify_v3_protocol_returns_v3v3() {
        use crate::calldata::ProtocolType;
        let decoded = make_decoded(ProtocolType::V3);
        assert_eq!(
            StrategyLabel::classify_from_decoded(&decoded),
            StrategyLabel::DexArbV3V3,
            "V3 source protocol must classify as DexArbV3V3"
        );
    }

    #[test]
    fn classify_curve_falls_back_to_v2v2() {
        use crate::calldata::ProtocolType;
        let decoded = make_decoded(ProtocolType::Curve);
        assert_eq!(
            StrategyLabel::classify_from_decoded(&decoded),
            StrategyLabel::DexArbV2V2,
            "Curve has no dedicated V1-path variant — fallback to DexArbV2V2"
        );
    }

    #[test]
    fn classify_unknown_falls_back_to_v2v2() {
        use crate::calldata::ProtocolType;
        let decoded = make_decoded(ProtocolType::Unknown);
        assert_eq!(
            StrategyLabel::classify_from_decoded(&decoded),
            StrategyLabel::DexArbV2V2,
            "Unknown protocol must fall back to DexArbV2V2 (legacy compat)"
        );
    }

    #[test]
    fn classify_from_decoded_string_matches_as_str() {
        use crate::calldata::ProtocolType;
        // Verify that the strings produced by classify_from_decoded are the same
        // ones the orchestrator (V2 mode) uses — they come from as_str().
        let v2_decoded = make_decoded(ProtocolType::V2);
        assert_eq!(
            StrategyLabel::classify_from_decoded(&v2_decoded).as_str(),
            "dex_arb_v2v2",
            "V2 decoded must yield as_str() == 'dex_arb_v2v2'"
        );
        let v3_decoded = make_decoded(ProtocolType::V3);
        assert_eq!(
            StrategyLabel::classify_from_decoded(&v3_decoded).as_str(),
            "dex_arb_v3v3",
            "V3 decoded must yield as_str() == 'dex_arb_v3v3'"
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
