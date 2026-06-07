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
}
