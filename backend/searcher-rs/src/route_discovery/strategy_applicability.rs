//! StrategyApplicabilityEngine — maps a `RouteKind` to the strategies that
//! apply (granular `StrategyLabel`s) plus machine-readable rejections.
//!
//! Config-driven via `config/strategies/route_applicability.yaml`, **fail-safe**:
//! - Missing / unreadable / invalid file ⇒ embedded SAFE defaults.
//! - `shadow_only` is **forced true** on every profile at load — a config can
//!   never enable execution (there is no active route-discovery mode).
//!
//! Phase 1 measures topology + applicability only; nothing here sizes or
//! executes. `liquidation` is intentionally `route_based: false` — it is found
//! by health-factor scan, not the DEX graph (corpus §finding 7).

use crate::route_discovery::types::{RejectedStrategy, RouteKind};
use crate::strategy_label::StrategyLabel;
use serde::Deserialize;
use std::collections::HashMap;
use std::path::Path;
use tracing::{info, warn};

/// Fixed evaluation order so telemetry output is stable across runs.
const STRATEGY_ORDER: [&str; 5] = [
    "dex_arb",
    "triangular_arb",
    "flashloan_arb",
    "stable_arb",
    "liquidation",
];

fn yes() -> bool {
    true
}
fn default_max_pools_per_pair() -> usize {
    8
}
fn default_max_depth() -> u8 {
    3
}

/// Discovery tunables (graph + DFS) read from the YAML `discovery:` section.
/// Env caps (`ARBX_ROUTE_DISCOVERY_MAX_*`) override these in the worker.
#[derive(Debug, Clone, Deserialize)]
pub struct DiscoverySettings {
    /// Start tokens (lowercase 0x hex). Empty ⇒ every token in the graph.
    #[serde(default)]
    pub base_tokens: Vec<String>,
    #[serde(default)]
    pub min_liquidity_hint: f64,
    #[serde(default = "default_max_pools_per_pair")]
    pub max_pools_per_pair: usize,
    #[serde(default = "default_max_depth")]
    pub max_depth: u8,
}

impl Default for DiscoverySettings {
    fn default() -> Self {
        Self {
            base_tokens: Vec::new(),
            min_liquidity_hint: 0.0,
            max_pools_per_pair: 8,
            max_depth: 3,
        }
    }
}

/// One strategy's applicability profile.
#[derive(Debug, Clone, Deserialize)]
pub struct StrategyProfile {
    pub enabled: bool,
    #[serde(default = "yes")]
    pub shadow_only: bool,
    /// `false` ⇒ this strategy is not discovered by the DEX route graph at all
    /// (e.g. liquidation) ⇒ every route is rejected `strategy_not_route_based`.
    #[serde(default = "yes")]
    pub route_based: bool,
    /// Accepted route_kind tokens (`v2v2`/`v2v3`/`v3v2`/`v3v3`/`triangular`).
    #[serde(default)]
    pub accepts: Vec<String>,
    /// Whether an executable cartridge exists for this strategy (drives dispatch
    /// in a later commit; metadata only here).
    #[serde(default)]
    pub has_cartridge: bool,
    /// Optional stablecoin allowlist (stable_arb). Parsed but not enforced in
    /// Phase 1 (stable_arb is disabled by default).
    #[serde(default)]
    pub token_allowlist: Vec<String>,
}

/// Whole applicability config: discovery tunables + per-strategy profiles.
#[derive(Debug, Clone, Deserialize)]
pub struct ApplicabilityConfig {
    #[serde(default)]
    pub discovery: DiscoverySettings,
    #[serde(default)]
    pub strategies: HashMap<String, StrategyProfile>,
}

impl Default for ApplicabilityConfig {
    /// Embedded SAFE defaults — identical to the shipped YAML. Every profile is
    /// `shadow_only`; `stable_arb` + `liquidation` disabled.
    fn default() -> Self {
        let mut strategies = HashMap::new();
        strategies.insert(
            "dex_arb".to_string(),
            StrategyProfile {
                enabled: true,
                shadow_only: true,
                route_based: true,
                accepts: vec!["v2v2".into(), "v2v3".into(), "v3v2".into(), "v3v3".into()],
                has_cartridge: true,
                token_allowlist: Vec::new(),
            },
        );
        strategies.insert(
            "triangular_arb".to_string(),
            StrategyProfile {
                enabled: true,
                shadow_only: true,
                route_based: true,
                accepts: vec!["triangular".into()],
                has_cartridge: true,
                token_allowlist: Vec::new(),
            },
        );
        strategies.insert(
            "flashloan_arb".to_string(),
            StrategyProfile {
                enabled: true,
                shadow_only: true,
                route_based: true,
                accepts: vec![
                    "v2v2".into(),
                    "v2v3".into(),
                    "v3v2".into(),
                    "v3v3".into(),
                    "triangular".into(),
                ],
                has_cartridge: false,
                token_allowlist: Vec::new(),
            },
        );
        strategies.insert(
            "stable_arb".to_string(),
            StrategyProfile {
                enabled: false,
                shadow_only: true,
                route_based: true,
                accepts: vec!["v2v2".into(), "v2v3".into(), "v3v2".into(), "v3v3".into()],
                has_cartridge: false,
                token_allowlist: Vec::new(),
            },
        );
        strategies.insert(
            "liquidation".to_string(),
            StrategyProfile {
                enabled: false,
                shadow_only: true,
                route_based: false,
                accepts: Vec::new(),
                has_cartridge: true,
                token_allowlist: Vec::new(),
            },
        );
        Self {
            discovery: DiscoverySettings::default(),
            strategies,
        }
    }
}

/// The applicability verdict for one route.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Applicability {
    /// Granular labels that apply (e.g. `DexArbV2V3`, `FlashloanArb`).
    pub applicable: Vec<StrategyLabel>,
    /// Strategies that do not apply, with reasons.
    pub rejected: Vec<RejectedStrategy>,
    /// Non-`StrategyLabel` tags that apply (e.g. `stable_arb`).
    pub tags: Vec<String>,
}

/// Maps a coarse strategy name + route_kind to its granular `StrategyLabel`,
/// or `None` for name/route combos that have no single label (e.g. stable_arb,
/// or dex_arb on a triangular route).
fn applicable_label(name: &str, route_kind: RouteKind) -> Option<StrategyLabel> {
    match name {
        "dex_arb" => match route_kind {
            RouteKind::V2V2 => Some(StrategyLabel::DexArbV2V2),
            RouteKind::V2V3 => Some(StrategyLabel::DexArbV2V3),
            RouteKind::V3V2 => Some(StrategyLabel::DexArbV3V2),
            RouteKind::V3V3 => Some(StrategyLabel::DexArbV3V3),
            _ => None,
        },
        "triangular_arb" => Some(StrategyLabel::TriangularArb),
        "flashloan_arb" => Some(StrategyLabel::FlashloanArb),
        "liquidation" => Some(StrategyLabel::Liquidation),
        _ => None, // stable_arb → tag, not a label
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// strategy_applicability_v2 — route shape → omega_strategy_pack dispatch key
//
// ADDITIVE bridge (Phase 2). Maps a measured route shape to the polymorphic
// omega_strategy_pack's `strategy_kind` string (the cartridge DISPATCH key — NOT
// the persisted `StrategyLabel`/`StrategyKind` enum, so this changes no DB/API
// contract and needs no migration). Pure + deterministic + unit-tested.
//
// NO-ACTIVE: `shadow_allowed` is always true, `live_allowed` is always false here —
// live eligibility is decided downstream by the execution gates, never by the
// classifier. Fail-honest: an unrecognised shape → applicable=false with a reason.
// ─────────────────────────────────────────────────────────────────────────────

/// Measured shape of a candidate route (what route_discovery already knows about it).
/// All fields are observations, never fabricated — absent signals stay at their
/// conservative default (R8).
#[derive(Debug, Clone, Default)]
pub struct RouteShapeV2 {
    /// Number of swap legs (2..=7).
    pub hop_count: usize,
    /// Distinct DEX venues across the legs.
    pub distinct_dexes: usize,
    /// Distinct fee tiers across the legs.
    pub distinct_fee_tiers: usize,
    /// True when all legs trade the same token pair (2-leg spatial / fee-tier).
    pub same_pair: bool,
    /// True when the route mixes a constant-product (V2) and a concentrated (V3) leg.
    pub mixes_v2_and_v3: bool,
    /// True when every token in the route is a stablecoin (basket / depeg).
    pub all_stablecoins: bool,
    /// True when the trigger is a confirmed new-block post-state imbalance (backrun).
    pub post_block_imbalance: bool,
    /// True when the route spans more than one chain.
    pub cross_chain: bool,
    /// True when own inventory is available on both chains (cross-chain w/o bridge).
    pub inventory_available: bool,
}

/// Result of v2 classification — mirrors the master-prompt applicability contract.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StrategyApplicabilityV2 {
    /// The omega_strategy_pack dispatch key, or "" when not applicable.
    pub strategy_kind: String,
    pub applicable: bool,
    pub reason: String,
    pub required_bindings: Vec<String>,
    pub missing_bindings: Vec<String>,
    /// Always true (route discovery is shadow-only by construction).
    pub shadow_allowed: bool,
    /// Always false here (live is gated downstream, never granted by the classifier).
    pub live_allowed: bool,
}

impl StrategyApplicabilityV2 {
    fn applicable(kind: &str, reason: &str, missing: &[&str]) -> Self {
        Self {
            strategy_kind: kind.to_string(),
            applicable: true,
            reason: reason.to_string(),
            required_bindings: Vec::new(),
            missing_bindings: missing.iter().map(|s| s.to_string()).collect(),
            shadow_allowed: true,
            live_allowed: false,
        }
    }
    fn not_applicable(reason: &str) -> Self {
        Self {
            strategy_kind: String::new(),
            applicable: false,
            reason: reason.to_string(),
            required_bindings: Vec::new(),
            missing_bindings: Vec::new(),
            shadow_allowed: true,
            live_allowed: false,
        }
    }
}

/// Classify a measured route shape into an omega_strategy_pack dispatch key.
///
/// Precedence follows the master-prompt §6 matrix: cross-chain and post-block
/// signals dominate (they constrain the surface), then stable baskets, then hop
/// count, then 2-leg sub-shapes (fee-tier vs spatial vs cross-invariant).
pub fn classify_v2(shape: &RouteShapeV2) -> StrategyApplicabilityV2 {
    // Cross-chain is shadow-only and flags the bindings it still lacks (R8 honesty).
    if shape.cross_chain {
        return if shape.inventory_available {
            StrategyApplicabilityV2::applicable(
                "cross_chain_inventory_shadow",
                "cross_chain_with_inventory",
                &["finality_model", "settlement_risk_model", "cross_chain_accounting"],
            )
        } else {
            StrategyApplicabilityV2::applicable(
                "omnichain_shadow_route",
                "cross_chain_bridge_required",
                &["bridge_latency_model", "finality_model", "settlement_risk_model", "rollback_killswitch"],
            )
        };
    }

    // Confirmed post-state imbalance → non-extractive backrun (no pending-tx surface).
    if shape.post_block_imbalance {
        return StrategyApplicabilityV2::applicable(
            "post_block_residual_backrun",
            "new_block_post_state_imbalance",
            &[],
        );
    }

    // Stablecoin basket → depeg/curve family.
    if shape.all_stablecoins && shape.hop_count >= 2 {
        return StrategyApplicabilityV2::applicable("stable_depeg", "stablecoin_basket", &[]);
    }

    match shape.hop_count {
        0 | 1 => StrategyApplicabilityV2::not_applicable("hop_count_below_2"),
        2 => {
            if shape.same_pair && shape.distinct_dexes <= 1 && shape.distinct_fee_tiers > 1 {
                StrategyApplicabilityV2::applicable("v3_fee_tier", "same_pair_same_dex_diff_fee_tier", &[])
            } else if shape.same_pair && shape.distinct_dexes > 1 {
                StrategyApplicabilityV2::applicable("spatial_cross_dex", "same_pair_diff_dex", &[])
            } else if shape.mixes_v2_and_v3 {
                StrategyApplicabilityV2::applicable("v2_v3_cross_invariant", "mixed_v2_v3_invariants", &[])
            } else {
                StrategyApplicabilityV2::applicable("spatial_cross_dex", "two_leg_default", &[])
            }
        }
        3 => {
            if shape.distinct_dexes > 1 {
                StrategyApplicabilityV2::applicable("triangular_cross_dex", "three_leg_cross_dex_cycle", &[])
            } else {
                StrategyApplicabilityV2::applicable("triangular_same_dex", "three_leg_same_dex_cycle", &[])
            }
        }
        n if n >= 4 && n <= 7 => {
            StrategyApplicabilityV2::applicable("multi_hop_cycle", "four_to_seven_leg_cycle", &[])
        }
        _ => StrategyApplicabilityV2::not_applicable("hop_count_above_7_unsupported"),
    }
}

/// Strategy applicability engine.
#[derive(Debug, Clone, Default)]
pub struct StrategyApplicabilityEngine {
    config: ApplicabilityConfig,
}

impl StrategyApplicabilityEngine {
    pub fn new(config: ApplicabilityConfig) -> Self {
        let mut config = config;
        force_shadow_only(&mut config);
        Self { config }
    }

    /// Parse a YAML config string. Invalid YAML ⇒ safe defaults (logged).
    /// `shadow_only` is forced true regardless of the file's contents.
    pub fn from_yaml_str(s: &str) -> Self {
        match serde_yaml::from_str::<ApplicabilityConfig>(s) {
            Ok(config) => Self::new(config),
            Err(e) => {
                warn!(
                    event = "route_discovery.config_parse_failed",
                    error = %e,
                    "route_applicability config invalid; using embedded safe defaults"
                );
                Self::default()
            }
        }
    }

    /// Load from a path; missing/unreadable file ⇒ safe defaults (logged).
    pub fn load_or_default(path: &Path) -> Self {
        match std::fs::read_to_string(path) {
            Ok(s) => {
                info!(
                    event = "route_discovery.config_loaded",
                    path = %path.display(),
                    bytes = s.len(),
                    "route_applicability config file read; parsing"
                );
                Self::from_yaml_str(&s)
            }
            Err(_) => {
                info!(
                    event = "route_discovery.config_default",
                    path = %path.display(),
                    "route_applicability config absent; using embedded safe defaults"
                );
                Self::default()
            }
        }
    }

    pub fn config(&self) -> &ApplicabilityConfig {
        &self.config
    }

    /// `true` when the named strategy has an executable cartridge AND is enabled
    /// (consulted by the dispatcher in a later commit).
    pub fn has_cartridge(&self, name: &str) -> bool {
        self.config
            .strategies
            .get(name)
            .map(|p| p.enabled && p.has_cartridge)
            .unwrap_or(false)
    }

    /// Evaluate which strategies apply to a route of the given kind.
    pub fn evaluate(&self, route_kind: RouteKind) -> Applicability {
        let mut out = Applicability::default();
        for name in STRATEGY_ORDER {
            let profile = match self.config.strategies.get(name) {
                Some(p) => p,
                None => continue,
            };

            if !profile.route_based {
                out.rejected.push(RejectedStrategy {
                    strategy: name.to_string(),
                    reason: "strategy_not_route_based".to_string(),
                });
                continue;
            }
            if !profile.enabled {
                out.rejected.push(RejectedStrategy {
                    strategy: name.to_string(),
                    reason: "disabled".to_string(),
                });
                continue;
            }

            let accepts = profile.accepts.iter().any(|s| s == route_kind.as_str());
            if accepts {
                match applicable_label(name, route_kind) {
                    Some(label) => out.applicable.push(label),
                    None => out.tags.push(name.to_string()),
                }
            } else {
                let reason = if name == "triangular_arb" {
                    "requires_3_legs"
                } else if route_kind == RouteKind::Triangular {
                    "requires_two_cycle"
                } else {
                    "route_kind_not_accepted"
                };
                out.rejected.push(RejectedStrategy {
                    strategy: name.to_string(),
                    reason: reason.to_string(),
                });
            }
        }
        out
    }
}

/// Force `shadow_only = true` on every profile so a hostile/incorrect config can
/// never enable execution.
fn force_shadow_only(config: &mut ApplicabilityConfig) {
    for p in config.strategies.values_mut() {
        p.shadow_only = true;
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    fn names(rejected: &[RejectedStrategy]) -> Vec<(String, String)> {
        rejected
            .iter()
            .map(|r| (r.strategy.clone(), r.reason.clone()))
            .collect()
    }

    #[test]
    fn default_v2v3_matches_plan_example() {
        let eng = StrategyApplicabilityEngine::default();
        let a = eng.evaluate(RouteKind::V2V3);
        assert!(a.applicable.contains(&StrategyLabel::DexArbV2V3));
        assert!(a.applicable.contains(&StrategyLabel::FlashloanArb));
        assert!(!a.applicable.contains(&StrategyLabel::TriangularArb));
        let rej = names(&a.rejected);
        assert!(rej.contains(&("triangular_arb".into(), "requires_3_legs".into())));
        assert!(rej.contains(&("liquidation".into(), "strategy_not_route_based".into())));
        assert!(rej.contains(&("stable_arb".into(), "disabled".into())));
        assert!(a.tags.is_empty(), "stable_arb disabled ⇒ no tag");
    }

    #[test]
    fn default_triangular_applies_triangular_and_flashloan() {
        let eng = StrategyApplicabilityEngine::default();
        let a = eng.evaluate(RouteKind::Triangular);
        assert!(a.applicable.contains(&StrategyLabel::TriangularArb));
        assert!(a.applicable.contains(&StrategyLabel::FlashloanArb));
        // dex_arb does not do triangular.
        let rej = names(&a.rejected);
        assert!(rej.contains(&("dex_arb".into(), "requires_two_cycle".into())));
        assert!(rej.contains(&("liquidation".into(), "strategy_not_route_based".into())));
    }

    #[test]
    fn dex_arb_granular_label_tracks_route_kind() {
        let eng = StrategyApplicabilityEngine::default();
        assert!(eng
            .evaluate(RouteKind::V2V2)
            .applicable
            .contains(&StrategyLabel::DexArbV2V2));
        assert!(eng
            .evaluate(RouteKind::V3V3)
            .applicable
            .contains(&StrategyLabel::DexArbV3V3));
        assert!(eng
            .evaluate(RouteKind::V3V2)
            .applicable
            .contains(&StrategyLabel::DexArbV3V2));
    }

    #[test]
    fn has_cartridge_reflects_config() {
        let eng = StrategyApplicabilityEngine::default();
        assert!(eng.has_cartridge("dex_arb"));
        assert!(eng.has_cartridge("triangular_arb"));
        // flashloan has no cartridge; liquidation is disabled.
        assert!(!eng.has_cartridge("flashloan_arb"));
        assert!(!eng.has_cartridge("liquidation"));
        assert!(!eng.has_cartridge("nonexistent"));
    }

    #[test]
    fn yaml_shadow_only_false_is_forced_to_shadow() {
        let yaml = r#"
version: 1
strategies:
  dex_arb:
    enabled: true
    shadow_only: false
    route_based: true
    accepts: [v2v2, v2v3, v3v2, v3v3]
    has_cartridge: true
"#;
        let eng = StrategyApplicabilityEngine::from_yaml_str(yaml);
        assert!(
            eng.config().strategies.get("dex_arb").unwrap().shadow_only,
            "shadow_only:false must be forced to true"
        );
    }

    #[test]
    fn invalid_yaml_falls_back_to_safe_defaults() {
        let eng = StrategyApplicabilityEngine::from_yaml_str("::: not yaml :::\n  - [");
        // Default behavior intact: V2V3 still classifies dex_arb.
        assert!(eng
            .evaluate(RouteKind::V2V3)
            .applicable
            .contains(&StrategyLabel::DexArbV2V3));
        // And nothing is enabled for execution (shadow defaults).
        assert!(eng.config().strategies.get("dex_arb").unwrap().shadow_only);
    }

    #[test]
    fn full_shipped_yaml_parses_and_matches_defaults() {
        // The exact content shipped in config/strategies/route_applicability.yaml.
        let yaml = r#"
version: 1
discovery:
  base_tokens: []
  min_liquidity_hint: 0.0
  max_pools_per_pair: 8
  max_depth: 3
strategies:
  dex_arb: { enabled: true, shadow_only: true, route_based: true, accepts: [v2v2, v2v3, v3v2, v3v3], has_cartridge: true }
  triangular_arb: { enabled: true, shadow_only: true, route_based: true, accepts: [triangular], has_cartridge: true }
  flashloan_arb: { enabled: true, shadow_only: true, route_based: true, accepts: [v2v2, v2v3, v3v2, v3v3, triangular], has_cartridge: false }
  stable_arb: { enabled: false, shadow_only: true, route_based: true, accepts: [v2v2, v2v3, v3v2, v3v3], has_cartridge: false, token_allowlist: [] }
  liquidation: { enabled: false, shadow_only: true, route_based: false, accepts: [], has_cartridge: true }
"#;
        let eng = StrategyApplicabilityEngine::from_yaml_str(yaml);
        assert_eq!(eng.config().discovery.max_depth, 3);
        assert_eq!(eng.config().discovery.max_pools_per_pair, 8);
        let a = eng.evaluate(RouteKind::V2V3);
        assert!(a.applicable.contains(&StrategyLabel::DexArbV2V3));
        assert!(a.applicable.contains(&StrategyLabel::FlashloanArb));
    }

    // ── classify_v2 — route shape → omega_strategy_pack dispatch key (Phase 2) ──

    fn shape(hop: usize) -> RouteShapeV2 {
        RouteShapeV2 { hop_count: hop, ..Default::default() }
    }

    #[test]
    fn v2_two_leg_same_pair_diff_fee_is_v3_fee_tier() {
        let s = RouteShapeV2 { hop_count: 2, same_pair: true, distinct_dexes: 1, distinct_fee_tiers: 2, ..Default::default() };
        let r = classify_v2(&s);
        assert_eq!(r.strategy_kind, "v3_fee_tier");
        assert!(r.applicable && r.shadow_allowed && !r.live_allowed);
    }

    #[test]
    fn v2_two_leg_same_pair_diff_dex_is_spatial() {
        let s = RouteShapeV2 { hop_count: 2, same_pair: true, distinct_dexes: 2, ..Default::default() };
        assert_eq!(classify_v2(&s).strategy_kind, "spatial_cross_dex");
    }

    #[test]
    fn v2_two_leg_mixed_invariant_is_cross_invariant() {
        let s = RouteShapeV2 { hop_count: 2, same_pair: false, mixes_v2_and_v3: true, ..Default::default() };
        assert_eq!(classify_v2(&s).strategy_kind, "v2_v3_cross_invariant");
    }

    #[test]
    fn v2_three_leg_cross_vs_same_dex() {
        let cross = RouteShapeV2 { hop_count: 3, distinct_dexes: 3, ..Default::default() };
        assert_eq!(classify_v2(&cross).strategy_kind, "triangular_cross_dex");
        let same = RouteShapeV2 { hop_count: 3, distinct_dexes: 1, ..Default::default() };
        assert_eq!(classify_v2(&same).strategy_kind, "triangular_same_dex");
    }

    #[test]
    fn v2_four_to_seven_leg_is_multi_hop() {
        for h in 4..=7 {
            assert_eq!(classify_v2(&shape(h)).strategy_kind, "multi_hop_cycle", "hop {h}");
        }
        // 8+ unsupported, fail-honest
        assert!(!classify_v2(&shape(8)).applicable);
        assert_eq!(classify_v2(&shape(1)).applicable, false);
    }

    #[test]
    fn v2_post_block_imbalance_dominates() {
        let s = RouteShapeV2 { hop_count: 2, post_block_imbalance: true, same_pair: true, distinct_dexes: 2, ..Default::default() };
        assert_eq!(classify_v2(&s).strategy_kind, "post_block_residual_backrun");
    }

    #[test]
    fn v2_cross_chain_inventory_vs_bridge_shadow_only() {
        let inv = RouteShapeV2 { hop_count: 2, cross_chain: true, inventory_available: true, ..Default::default() };
        let r1 = classify_v2(&inv);
        assert_eq!(r1.strategy_kind, "cross_chain_inventory_shadow");
        assert!(!r1.live_allowed && r1.shadow_allowed);
        assert!(!r1.missing_bindings.is_empty(), "cross-chain must flag missing bindings (R8)");

        let bridge = RouteShapeV2 { hop_count: 2, cross_chain: true, inventory_available: false, ..Default::default() };
        let r2 = classify_v2(&bridge);
        assert_eq!(r2.strategy_kind, "omnichain_shadow_route");
        assert!(r2.missing_bindings.iter().any(|b| b.contains("bridge")));
    }

    #[test]
    fn v2_stablecoin_basket_is_stable_depeg() {
        let s = RouteShapeV2 { hop_count: 3, all_stablecoins: true, distinct_dexes: 2, ..Default::default() };
        assert_eq!(classify_v2(&s).strategy_kind, "stable_depeg");
    }

    #[test]
    fn v2_never_grants_live() {
        // NO-ACTIVE invariant: no shape ever yields live_allowed=true.
        for h in 0..=8 {
            assert!(!classify_v2(&shape(h)).live_allowed);
        }
    }
}
