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
//!
//! ## Excel family profiles (RU-4)
//!
//! The 264-cartridge canon declares 11 Excel categories. Each gets a FAMILY
//! profile here (same YAML file, `families:` section): which `RouteKind`s it
//! accepts, which engine(s) own its evaluation, and the additional gates that
//! must pass before any dispatch. Family verdicts are pure applicability
//! metadata — they never produce `StrategyLabel`s (that mapping is
//! `cartridge_boot`'s job, RU-3), never size, never execute, and their
//! `shadow_only` is forced true with the same invariant as the coarse
//! strategies.

use crate::route_discovery::types::{RejectedStrategy, RouteKind};
use crate::route_intent::{ProtocolType, RouteIntentLeg};
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

/// Fixed evaluation order for Excel-family verdicts so telemetry output is
/// stable across runs. Declaration order of the 264-cartridge Excel canon
/// (36/31/31/30/30/25/20/18/17/14/12 cartridges per family).
const FAMILY_ORDER: [&str; 11] = [
    "route_graph_engine",
    "state_event_engine",
    "parity_redemption_engine",
    "derivatives_engine",
    "cross_domain_engine",
    "credit_liquidation_engine",
    "intents_solver_engine",
    "nft_engine",
    "amm_curve_engine",
    "cex_external_engine",
    "prediction_engine",
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

/// Constructor helper for the embedded SAFE family defaults (RU-4). Every
/// family profile is `shadow_only` and has cartridges on disk (264 canon).
fn family_profile(
    enabled: bool,
    route_based: bool,
    accepts: &[&str],
    target_engines: &[&str],
    gates: &[&str],
) -> FamilyProfile {
    FamilyProfile {
        enabled,
        shadow_only: true,
        route_based,
        accepts: accepts.iter().map(|s| s.to_string()).collect(),
        target_engines: target_engines.iter().map(|s| s.to_string()).collect(),
        gates: gates.iter().map(|s| s.to_string()).collect(),
        has_cartridge: true,
    }
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
            max_depth: 5,
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

/// One Excel strategy-family profile (264-cartridge canon, 11 categories — RU-4).
///
/// A FAMILY aggregates the cartridges of one Excel category and states, per
/// `RouteKind`: whether the family applies, which engine(s) own its evaluation,
/// and which additional gates must pass. Everything here is APPLICABILITY
/// METADATA — no family path sizes or executes, and `shadow_only` is forced
/// true at load exactly like the coarse strategy profiles.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct FamilyProfile {
    /// `false` ⇒ stub that does not participate at all (late-phase families
    /// nft/prediction, waves G10/G11) ⇒ rejected `family_stub_not_implemented`.
    pub enabled: bool,
    #[serde(default = "yes")]
    pub shadow_only: bool,
    /// `false` ⇒ the DEX route graph can never DISCOVER this family (external
    /// anchor / off-graph surface) ⇒ every route is rejected
    /// `family_not_route_based` (same semantics as the coarse `liquidation`).
    #[serde(default = "yes")]
    pub route_based: bool,
    /// Accepted route_kind tokens (`v2v2`/`v2v3`/`v3v2`/`v3v3`/`triangular`).
    #[serde(default)]
    pub accepts: Vec<String>,
    /// Engine(s) that own this family's evaluation. Metadata only — the
    /// dispatch wiring ships with each family's wave; nothing here executes.
    #[serde(default)]
    pub target_engines: Vec<String>,
    /// Additional REQUIRED gates: identifiers for the data/behaviour bindings
    /// the family needs beyond route shape (e.g. `impact_index_post_state`).
    /// A gate listed here must PASS before any dispatch decision.
    #[serde(default)]
    pub gates: Vec<String>,
    /// Cartridges exist on disk for this category (all 11 do — 264 total).
    #[serde(default)]
    pub has_cartridge: bool,
}

/// Whole applicability config: discovery tunables + per-strategy profiles +
/// Excel family profiles (RU-4).
#[derive(Debug, Clone, Deserialize)]
pub struct ApplicabilityConfig {
    #[serde(default)]
    pub discovery: DiscoverySettings,
    #[serde(default)]
    pub strategies: HashMap<String, StrategyProfile>,
    /// Excel family profiles (11 categories). A YAML without a `families:`
    /// section yields an empty map — no family verdicts, nothing fabricated.
    #[serde(default)]
    pub families: HashMap<String, FamilyProfile>,
}

impl Default for ApplicabilityConfig {
    /// Embedded SAFE defaults — identical to the shipped YAML. Every profile
    /// (strategies AND families) is `shadow_only`; `stable_arb` +
    /// `liquidation` disabled; nft/prediction families are honest stubs.
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
                    "multihop".into(),
                ],
                has_cartridge: true,
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
        // Excel family defaults (RU-4) — byte-identical to the shipped YAML
        // `families:` section. Route-based families name the shapes the DEX
        // graph can discover; `route_based: false` families name an off-graph
        // surface (their verdict reports the missing surface honestly).
        let mut families = HashMap::new();
        families.insert(
            "route_graph_engine".to_string(),
            family_profile(
                true,
                true,
                &["v2v2", "v2v3", "v3v2", "v3v3", "triangular"],
                &["dex_engine", "spatial_engine", "triangular_engine"],
                &[],
            ),
        );
        families.insert(
            "state_event_engine".to_string(),
            family_profile(
                true,
                true,
                &["v2v2", "v2v3", "v3v2", "v3v3", "triangular"],
                &["backrun_engine"],
                &["impact_index_post_state"],
            ),
        );
        families.insert(
            "parity_redemption_engine".to_string(),
            family_profile(
                true,
                true,
                &["v2v2", "v2v3", "v3v2", "v3v3"],
                &["dex_engine"],
                &["equivalence_edge_registry"],
            ),
        );
        families.insert(
            "derivatives_engine".to_string(),
            family_profile(
                true,
                false,
                &[],
                &["funding_rate_engine"],
                &["derivative_oracle_feed"],
            ),
        );
        families.insert(
            "cross_domain_engine".to_string(),
            family_profile(
                true,
                false,
                &[],
                &["cross_chain_bridge_engine"],
                &["bridge_leg_bindings"],
            ),
        );
        families.insert(
            "credit_liquidation_engine".to_string(),
            family_profile(
                true,
                false,
                &[],
                &["liquidation_engine", "liquidation_snipe_engine"],
                &["health_factor_index"],
            ),
        );
        families.insert(
            "intents_solver_engine".to_string(),
            family_profile(
                true,
                false,
                &[],
                &["backrun_engine"],
                &["relay_intent_feed"],
            ),
        );
        families.insert(
            "nft_engine".to_string(),
            // Late-phase stub (wave G10): no honest engine yet — fail-honest.
            family_profile(false, false, &[], &[], &[]),
        );
        families.insert(
            "amm_curve_engine".to_string(),
            family_profile(
                true,
                true,
                &["v2v2", "v2v3", "v3v2", "v3v3", "triangular"],
                &["dex_engine"],
                &["typed_edge_protocol_coverage"],
            ),
        );
        families.insert(
            "cex_external_engine".to_string(),
            family_profile(true, false, &[], &["cex_dex_engine"], &["cex_price_feed"]),
        );
        families.insert(
            "prediction_engine".to_string(),
            // Late-phase stub (wave G11): no honest engine yet — fail-honest.
            family_profile(false, false, &[], &[], &[]),
        );
        Self {
            discovery: DiscoverySettings::default(),
            strategies,
            families,
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
                &[
                    "finality_model",
                    "settlement_risk_model",
                    "cross_chain_accounting",
                ],
            )
        } else {
            StrategyApplicabilityV2::applicable(
                "omnichain_shadow_route",
                "cross_chain_bridge_required",
                &[
                    "bridge_latency_model",
                    "finality_model",
                    "settlement_risk_model",
                    "rollback_killswitch",
                ],
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
                StrategyApplicabilityV2::applicable(
                    "v3_fee_tier",
                    "same_pair_same_dex_diff_fee_tier",
                    &[],
                )
            } else if shape.same_pair && shape.distinct_dexes > 1 {
                StrategyApplicabilityV2::applicable("spatial_cross_dex", "same_pair_diff_dex", &[])
            } else if shape.mixes_v2_and_v3 {
                StrategyApplicabilityV2::applicable(
                    "v2_v3_cross_invariant",
                    "mixed_v2_v3_invariants",
                    &[],
                )
            } else {
                StrategyApplicabilityV2::applicable("spatial_cross_dex", "two_leg_default", &[])
            }
        }
        3 => {
            if shape.distinct_dexes > 1 {
                StrategyApplicabilityV2::applicable(
                    "triangular_cross_dex",
                    "three_leg_cross_dex_cycle",
                    &[],
                )
            } else {
                StrategyApplicabilityV2::applicable(
                    "triangular_same_dex",
                    "three_leg_same_dex_cycle",
                    &[],
                )
            }
        }
        n if (4..=7).contains(&n) => {
            StrategyApplicabilityV2::applicable("multi_hop_cycle", "four_to_seven_leg_cycle", &[])
        }
        _ => StrategyApplicabilityV2::not_applicable("hop_count_above_7_unsupported"),
    }
}

/// Dispatch keys that the committed `omega_strategy_pack.rhai` actually handles
/// AND that `classify_v2` can emit for a single-chain route — the INTERSECTION.
///
/// `classify_v2` can also emit two cross-chain keys (`cross_chain_inventory_shadow`,
/// `omnichain_shadow_route`) for which the pack has **no handler** (it would hit the
/// pack's `reject("unsupported_strategy_kind")` fallthrough); and the pack handles
/// ~10 more keys the classifier never emits. This allowlist is therefore the set a
/// classified family can be *dispatched* on — used only to flag a family as routable
/// vs. observational shadow telemetry. SINGLE SOURCE OF TRUTH: keep in sync with the
/// pack's `evaluate_opportunity` dispatch table.
pub const SUPPORTED_DISPATCH_KEYS: [&str; 8] = [
    "v3_fee_tier",
    "spatial_cross_dex",
    "v2_v3_cross_invariant",
    "triangular_same_dex",
    "triangular_cross_dex",
    "multi_hop_cycle",
    "stable_depeg",
    "post_block_residual_backrun",
];

/// `true` when `key` is one the committed omega_strategy_pack can dispatch on.
/// The two cross-chain keys are deliberately ABSENT — they stay observational
/// (shadow telemetry), never dispatched, until finality/settlement/rollback
/// bindings exist. The empty string (unclassifiable shape) is unsupported.
pub fn is_pack_supported(key: &str) -> bool {
    SUPPORTED_DISPATCH_KEYS.contains(&key)
}

impl RouteShapeV2 {
    /// Derive a route shape from the legs the discovery layer already decoded.
    ///
    /// PURE + HONEST (R8): every field comes from data the legs carry; anything
    /// not derivable from legs alone stays at its conservative default rather than
    /// being fabricated:
    /// - `all_stablecoins` → always `false`: stablecoin-ness needs a token registry
    ///   this function does not have. A real stable route just classifies by
    ///   hop-count instead of `stable_depeg` (less specific, never wrong).
    /// - `post_block_imbalance` → always `false`: backrun provenance lives on
    ///   `RouteIntent::source_event` (`DetectionSource::NewBlock`), NOT on the route
    ///   geometry. The classifier must never infer it from legs — a backrun-wiring
    ///   caller sets it from the real confirmed-block source, downstream of here.
    /// - `cross_chain` / `inventory_available` → always `false`: the discovery worker
    ///   is single-chain and legs carry no per-leg chain id.
    ///
    /// DOCUMENTED CONSERVATIVE DEGRADATION (not hidden — see math-validator note):
    /// when venue/fee data is absent (`dex_hint`/`fee_bps` = `None`, the common case
    /// since the dispatcher emits `dex_hint: None`), `distinct_dexes` /
    /// `distinct_fee_tiers` count only KNOWN values, so unknown-venue routes
    /// under-claim toward `triangular_same_dex` (3-leg) or `spatial_cross_dex`
    /// (`two_leg_default`, 2-leg). These are dispatch HINTS for shadow telemetry, not
    /// money decisions; the net-profit gate re-derives everything before any action.
    pub fn from_legs(legs: &[RouteIntentLeg]) -> Self {
        use std::collections::HashSet;
        let distinct_dexes = legs
            .iter()
            .filter_map(|l| l.dex_hint.as_deref())
            .collect::<HashSet<&str>>()
            .len();
        let distinct_fee_tiers = legs
            .iter()
            .filter_map(|l| l.fee_bps)
            .collect::<HashSet<u32>>()
            .len();
        // `same_pair`: every leg trades the SAME unordered token pair as the first
        // leg. A 2-leg spatial round-trip has leg[1] reversed (A→B then B→A), so the
        // pair is canonicalized by address order before comparing. Empty ⇒ false.
        let same_pair = match legs.first() {
            None => false,
            Some(first) => {
                let canon = |l: &RouteIntentLeg| {
                    if l.token_in <= l.token_out {
                        (l.token_in, l.token_out)
                    } else {
                        (l.token_out, l.token_in)
                    }
                };
                let p0 = canon(first);
                legs.iter().all(|l| canon(l) == p0)
            }
        };
        let any_v2 = legs.iter().any(|l| l.protocol_type == ProtocolType::V2);
        let any_v3 = legs.iter().any(|l| l.protocol_type == ProtocolType::V3);
        Self {
            hop_count: legs.len(),
            distinct_dexes,
            distinct_fee_tiers,
            same_pair,
            mixes_v2_and_v3: any_v2 && any_v3,
            // Not derivable from legs alone — forced conservative (R8 honest).
            all_stablecoins: false,
            post_block_imbalance: false,
            cross_chain: false,
            inventory_available: false,
        }
    }
}

/// Classify a route directly from its decoded legs (shadow-telemetry helper).
/// Equivalent to `classify_v2(&RouteShapeV2::from_legs(legs))`. Pure + deterministic.
pub fn classify_route_legs(legs: &[RouteIntentLeg]) -> StrategyApplicabilityV2 {
    classify_v2(&RouteShapeV2::from_legs(legs))
}

/// Applicability verdict of one Excel family for one discovered route kind
/// (RU-4). Carries the profile's engine/gate metadata alongside the verdict so
/// telemetry is self-describing, even on rejection (R8: never silence).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FamilyVerdict {
    pub family: String,
    pub applicable: bool,
    /// Machine-readable reason: `family_route_kind_accepted` |
    /// `family_route_kind_not_accepted` | `family_not_route_based` |
    /// `family_stub_not_implemented`.
    pub reason: String,
    /// Engine(s) that own this family's evaluation (profile metadata; also
    /// returned on rejection so the missing surface is visible).
    pub target_engines: Vec<String>,
    /// Additional gates the family requires (also returned on rejection).
    pub gates: Vec<String>,
    /// Always `true` — forced at load; a config can never enable execution.
    pub shadow_only: bool,
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

    // ── Excel family verdicts (RU-4) ─────────────────────────────────────────

    /// Evaluate one Excel family against a route kind. `None` when the family
    /// is unknown to the config (typo / not declared) — never a fabricated
    /// verdict.
    ///
    /// Reason precedence is ENABLED first, then route-basedness, then the
    /// accepted-kind check. (Deliberate difference from `evaluate`'s coarse
    /// path, which checks `route_based` first: for families, a disabled profile
    /// is a STUB that is not participating at all — nft/prediction, waves
    /// G10/G11 — and that is the more important fact to surface.)
    pub fn evaluate_family(&self, family: &str, route_kind: RouteKind) -> Option<FamilyVerdict> {
        let p = self.config.families.get(family)?;
        let (applicable, reason) = if !p.enabled {
            (false, "family_stub_not_implemented")
        } else if !p.route_based {
            (false, "family_not_route_based")
        } else if p.accepts.iter().any(|s| s == route_kind.as_str()) {
            (true, "family_route_kind_accepted")
        } else {
            (false, "family_route_kind_not_accepted")
        };
        Some(FamilyVerdict {
            family: family.to_string(),
            applicable,
            reason: reason.to_string(),
            target_engines: p.target_engines.clone(),
            gates: p.gates.clone(),
            shadow_only: p.shadow_only,
        })
    }

    /// Verdicts for every declared family, in `FAMILY_ORDER` (stable telemetry
    /// output). Unknown-to-config families are skipped, not fabricated.
    pub fn evaluate_families(&self, route_kind: RouteKind) -> Vec<FamilyVerdict> {
        FAMILY_ORDER
            .iter()
            .filter_map(|f| self.evaluate_family(f, route_kind))
            .collect()
    }
}

/// Force `shadow_only = true` on every profile — coarse strategies AND Excel
/// families — so a hostile/incorrect config can never enable execution.
fn force_shadow_only(config: &mut ApplicabilityConfig) {
    for p in config.strategies.values_mut() {
        p.shadow_only = true;
    }
    for p in config.families.values_mut() {
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
        // PR-ROUTE-04: flashloan now dispatches via the omega_strategy_pack
        // (polymorphic_pack catch-all). liquidation is still disabled.
        assert!(eng.has_cartridge("flashloan_arb"));
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
families:
  route_graph_engine: { enabled: true, shadow_only: true, route_based: true, accepts: [v2v2, v2v3, v3v2, v3v3, triangular], target_engines: [dex_engine, spatial_engine, triangular_engine], gates: [], has_cartridge: true }
  state_event_engine: { enabled: true, shadow_only: true, route_based: true, accepts: [v2v2, v2v3, v3v2, v3v3, triangular], target_engines: [backrun_engine], gates: [impact_index_post_state], has_cartridge: true }
  parity_redemption_engine: { enabled: true, shadow_only: true, route_based: true, accepts: [v2v2, v2v3, v3v2, v3v3], target_engines: [dex_engine], gates: [equivalence_edge_registry], has_cartridge: true }
  derivatives_engine: { enabled: true, shadow_only: true, route_based: false, accepts: [], target_engines: [funding_rate_engine], gates: [derivative_oracle_feed], has_cartridge: true }
  cross_domain_engine: { enabled: true, shadow_only: true, route_based: false, accepts: [], target_engines: [cross_chain_bridge_engine], gates: [bridge_leg_bindings], has_cartridge: true }
  credit_liquidation_engine: { enabled: true, shadow_only: true, route_based: false, accepts: [], target_engines: [liquidation_engine, liquidation_snipe_engine], gates: [health_factor_index], has_cartridge: true }
  intents_solver_engine: { enabled: true, shadow_only: true, route_based: false, accepts: [], target_engines: [backrun_engine], gates: [relay_intent_feed], has_cartridge: true }
  nft_engine: { enabled: false, shadow_only: true, route_based: false, accepts: [], target_engines: [], gates: [], has_cartridge: true }
  amm_curve_engine: { enabled: true, shadow_only: true, route_based: true, accepts: [v2v2, v2v3, v3v2, v3v3, triangular], target_engines: [dex_engine], gates: [typed_edge_protocol_coverage], has_cartridge: true }
  cex_external_engine: { enabled: true, shadow_only: true, route_based: false, accepts: [], target_engines: [cex_dex_engine], gates: [cex_price_feed], has_cartridge: true }
  prediction_engine: { enabled: false, shadow_only: true, route_based: false, accepts: [], target_engines: [], gates: [], has_cartridge: true }
"#;
        let eng = StrategyApplicabilityEngine::from_yaml_str(yaml);
        assert_eq!(eng.config().discovery.max_depth, 3);
        assert_eq!(eng.config().discovery.max_pools_per_pair, 8);
        let a = eng.evaluate(RouteKind::V2V3);
        assert!(a.applicable.contains(&StrategyLabel::DexArbV2V3));
        assert!(a.applicable.contains(&StrategyLabel::FlashloanArb));
        // RU-4: the shipped YAML families are EXACTLY the embedded defaults.
        assert_eq!(eng.config().families.len(), 11);
        assert_eq!(
            eng.config().families,
            ApplicabilityConfig::default().families,
            "shipped YAML families must match the embedded SAFE defaults"
        );
    }

    // ── RU-4: Excel family profiles (11 categories, 264 cartridges) ──────────

    const ALL_KINDS: [RouteKind; 6] = [
        RouteKind::V2V2,
        RouteKind::V2V3,
        RouteKind::V3V2,
        RouteKind::V3V3,
        RouteKind::Triangular,
        RouteKind::MultiHop,
    ];

    #[test]
    fn family_defaults_cover_the_11_excel_categories() {
        let eng = StrategyApplicabilityEngine::default();
        assert_eq!(eng.config().families.len(), 11);
        for f in FAMILY_ORDER {
            assert!(eng.config().families.contains_key(f), "missing family {f}");
        }
    }

    #[test]
    fn family_route_graph_accepts_every_cycle_kind() {
        let eng = StrategyApplicabilityEngine::default();
        for k in [
            RouteKind::V2V2,
            RouteKind::V2V3,
            RouteKind::V3V2,
            RouteKind::V3V3,
            RouteKind::Triangular,
        ] {
            let v = eng.evaluate_family("route_graph_engine", k).unwrap();
            assert!(v.applicable, "route_graph must accept {k:?}");
            assert_eq!(v.reason, "family_route_kind_accepted");
            assert_eq!(
                v.target_engines,
                ["dex_engine", "spatial_engine", "triangular_engine"]
            );
            assert!(v.shadow_only, "family verdicts are always shadow-only");
        }
        // MultiHop is Phase-2 reserved — not an accepted kind for any family.
        let v = eng
            .evaluate_family("route_graph_engine", RouteKind::MultiHop)
            .unwrap();
        assert!(!v.applicable);
        assert_eq!(v.reason, "family_route_kind_not_accepted");
    }

    #[test]
    fn family_amm_curve_accepts_cycle_kinds_with_typed_edge_gate() {
        let eng = StrategyApplicabilityEngine::default();
        for k in [
            RouteKind::V2V2,
            RouteKind::V2V3,
            RouteKind::V3V2,
            RouteKind::V3V3,
            RouteKind::Triangular,
        ] {
            let v = eng.evaluate_family("amm_curve_engine", k).unwrap();
            assert!(v.applicable, "amm_curve must accept {k:?}");
            assert_eq!(v.target_engines, ["dex_engine"]);
        }
        // The gate records that Curve/Balancer edge typing is outside the
        // V2/V3-only Phase-1 graph (RouteKind::classify rejects those cycles).
        let v = eng
            .evaluate_family("amm_curve_engine", RouteKind::V2V2)
            .unwrap();
        assert_eq!(v.gates, ["typed_edge_protocol_coverage"]);
    }

    #[test]
    fn family_state_event_gated_on_impact_index() {
        let eng = StrategyApplicabilityEngine::default();
        for k in [
            RouteKind::V2V2,
            RouteKind::V2V3,
            RouteKind::V3V2,
            RouteKind::V3V3,
            RouteKind::Triangular,
        ] {
            let v = eng.evaluate_family("state_event_engine", k).unwrap();
            assert!(v.applicable, "state_event must accept {k:?}");
            assert_eq!(v.target_engines, ["backrun_engine"]);
            // Geometry alone never proves a backrun — the post-state signal
            // (ImpactIndex) is a required gate.
            assert_eq!(v.gates, ["impact_index_post_state"]);
        }
    }

    #[test]
    fn family_parity_redemption_is_two_cycle_only() {
        let eng = StrategyApplicabilityEngine::default();
        for k in [
            RouteKind::V2V2,
            RouteKind::V2V3,
            RouteKind::V3V2,
            RouteKind::V3V3,
        ] {
            let v = eng.evaluate_family("parity_redemption_engine", k).unwrap();
            assert!(v.applicable, "parity must accept {k:?}");
            assert_eq!(v.target_engines, ["dex_engine"]);
            assert_eq!(v.gates, ["equivalence_edge_registry"]);
        }
        // A 3-leg cycle is not an equivalence round-trip — rejected.
        let v = eng
            .evaluate_family("parity_redemption_engine", RouteKind::Triangular)
            .unwrap();
        assert!(!v.applicable);
        assert_eq!(v.reason, "family_route_kind_not_accepted");
    }

    #[test]
    fn family_off_graph_families_reject_every_route_kind() {
        // cex_external / cross_domain / derivatives / credit_liquidation /
        // intents_solver: real engines exist, but their surface is OFF the DEX
        // route graph (external price anchor, bridge legs, oracle legs,
        // health-factor scan, relay intent feed) — every discovered route kind
        // is rejected honestly, with the engine + gate metadata still reported.
        let eng = StrategyApplicabilityEngine::default();
        let off_graph: [(&str, Vec<&str>, Vec<&str>); 5] = [
            (
                "cex_external_engine",
                vec!["cex_dex_engine"],
                vec!["cex_price_feed"],
            ),
            (
                "cross_domain_engine",
                vec!["cross_chain_bridge_engine"],
                vec!["bridge_leg_bindings"],
            ),
            (
                "derivatives_engine",
                vec!["funding_rate_engine"],
                vec!["derivative_oracle_feed"],
            ),
            (
                "credit_liquidation_engine",
                vec!["liquidation_engine", "liquidation_snipe_engine"],
                vec!["health_factor_index"],
            ),
            (
                "intents_solver_engine",
                vec!["backrun_engine"],
                vec!["relay_intent_feed"],
            ),
        ];
        for (fam, engines, gates) in off_graph {
            for k in ALL_KINDS {
                let v = eng.evaluate_family(fam, k).unwrap();
                assert!(!v.applicable, "{fam} must not apply to {k:?}");
                assert_eq!(v.reason, "family_not_route_based", "{fam} {k:?}");
                assert_eq!(v.target_engines, engines, "{fam}");
                assert_eq!(v.gates, gates, "{fam}");
                assert!(v.shadow_only, "{fam}");
            }
        }
    }

    #[test]
    fn family_nft_and_prediction_are_honest_stubs() {
        // Late-phase waves G10/G11: profile declared, no honest engine yet —
        // every kind rejects with the stub reason, never silence.
        let eng = StrategyApplicabilityEngine::default();
        for fam in ["nft_engine", "prediction_engine"] {
            for k in ALL_KINDS {
                let v = eng.evaluate_family(fam, k).unwrap();
                assert!(!v.applicable, "{fam} {k:?}");
                assert_eq!(v.reason, "family_stub_not_implemented", "{fam} {k:?}");
                assert!(v.target_engines.is_empty(), "stub must not claim an engine");
                assert!(v.shadow_only, "{fam}");
            }
        }
    }

    #[test]
    fn family_shadow_only_false_in_yaml_is_forced_true() {
        let yaml = r#"
version: 1
families:
  route_graph_engine:
    enabled: true
    shadow_only: false
    route_based: true
    accepts: [v2v2]
    target_engines: [dex_engine]
"#;
        let eng = StrategyApplicabilityEngine::from_yaml_str(yaml);
        assert!(
            eng.config()
                .families
                .get("route_graph_engine")
                .unwrap()
                .shadow_only,
            "shadow_only:false must be forced to true for families too"
        );
        // And the verdict reflects it — no family path can enable execution.
        let v = eng
            .evaluate_family("route_graph_engine", RouteKind::V2V2)
            .unwrap();
        assert!(v.applicable);
        assert!(v.shadow_only, "a verdict must never grant execution");
    }

    #[test]
    fn evaluate_families_returns_stable_order_all_shadow() {
        let eng = StrategyApplicabilityEngine::default();
        let vs = eng.evaluate_families(RouteKind::V2V2);
        assert_eq!(vs.len(), 11);
        let names: Vec<&str> = vs.iter().map(|v| v.family.as_str()).collect();
        assert_eq!(names, FAMILY_ORDER.to_vec());
        assert!(vs.iter().all(|v| v.shadow_only), "NO-ACTIVE invariant");
        // Exactly the 4 graph-expressible families apply to a V2V2 2-cycle.
        let applicable: Vec<&str> = vs
            .iter()
            .filter(|v| v.applicable)
            .map(|v| v.family.as_str())
            .collect();
        assert_eq!(
            applicable,
            [
                "route_graph_engine",
                "state_event_engine",
                "parity_redemption_engine",
                "amm_curve_engine",
            ]
        );
    }

    #[test]
    fn unknown_family_returns_none_never_fabricated() {
        let eng = StrategyApplicabilityEngine::default();
        assert!(eng
            .evaluate_family("does_not_exist", RouteKind::V2V2)
            .is_none());
    }

    // ── classify_v2 — route shape → omega_strategy_pack dispatch key (Phase 2) ──

    fn shape(hop: usize) -> RouteShapeV2 {
        RouteShapeV2 {
            hop_count: hop,
            ..Default::default()
        }
    }

    #[test]
    fn v2_two_leg_same_pair_diff_fee_is_v3_fee_tier() {
        let s = RouteShapeV2 {
            hop_count: 2,
            same_pair: true,
            distinct_dexes: 1,
            distinct_fee_tiers: 2,
            ..Default::default()
        };
        let r = classify_v2(&s);
        assert_eq!(r.strategy_kind, "v3_fee_tier");
        assert!(r.applicable && r.shadow_allowed && !r.live_allowed);
    }

    #[test]
    fn v2_two_leg_same_pair_diff_dex_is_spatial() {
        let s = RouteShapeV2 {
            hop_count: 2,
            same_pair: true,
            distinct_dexes: 2,
            ..Default::default()
        };
        assert_eq!(classify_v2(&s).strategy_kind, "spatial_cross_dex");
    }

    #[test]
    fn v2_two_leg_mixed_invariant_is_cross_invariant() {
        let s = RouteShapeV2 {
            hop_count: 2,
            same_pair: false,
            mixes_v2_and_v3: true,
            ..Default::default()
        };
        assert_eq!(classify_v2(&s).strategy_kind, "v2_v3_cross_invariant");
    }

    #[test]
    fn v2_three_leg_cross_vs_same_dex() {
        let cross = RouteShapeV2 {
            hop_count: 3,
            distinct_dexes: 3,
            ..Default::default()
        };
        assert_eq!(classify_v2(&cross).strategy_kind, "triangular_cross_dex");
        let same = RouteShapeV2 {
            hop_count: 3,
            distinct_dexes: 1,
            ..Default::default()
        };
        assert_eq!(classify_v2(&same).strategy_kind, "triangular_same_dex");
    }

    #[test]
    fn v2_four_to_seven_leg_is_multi_hop() {
        for h in 4..=7 {
            assert_eq!(
                classify_v2(&shape(h)).strategy_kind,
                "multi_hop_cycle",
                "hop {h}"
            );
        }
        // 8+ unsupported, fail-honest
        assert!(!classify_v2(&shape(8)).applicable);
        assert!(!classify_v2(&shape(1)).applicable);
    }

    #[test]
    fn v2_post_block_imbalance_dominates() {
        let s = RouteShapeV2 {
            hop_count: 2,
            post_block_imbalance: true,
            same_pair: true,
            distinct_dexes: 2,
            ..Default::default()
        };
        assert_eq!(classify_v2(&s).strategy_kind, "post_block_residual_backrun");
    }

    #[test]
    fn v2_cross_chain_inventory_vs_bridge_shadow_only() {
        let inv = RouteShapeV2 {
            hop_count: 2,
            cross_chain: true,
            inventory_available: true,
            ..Default::default()
        };
        let r1 = classify_v2(&inv);
        assert_eq!(r1.strategy_kind, "cross_chain_inventory_shadow");
        assert!(!r1.live_allowed && r1.shadow_allowed);
        assert!(
            !r1.missing_bindings.is_empty(),
            "cross-chain must flag missing bindings (R8)"
        );

        let bridge = RouteShapeV2 {
            hop_count: 2,
            cross_chain: true,
            inventory_available: false,
            ..Default::default()
        };
        let r2 = classify_v2(&bridge);
        assert_eq!(r2.strategy_kind, "omnichain_shadow_route");
        assert!(r2.missing_bindings.iter().any(|b| b.contains("bridge")));
    }

    #[test]
    fn v2_stablecoin_basket_is_stable_depeg() {
        let s = RouteShapeV2 {
            hop_count: 3,
            all_stablecoins: true,
            distinct_dexes: 2,
            ..Default::default()
        };
        assert_eq!(classify_v2(&s).strategy_kind, "stable_depeg");
    }

    #[test]
    fn v2_never_grants_live() {
        // NO-ACTIVE invariant: no shape ever yields live_allowed=true.
        for h in 0..=8 {
            assert!(!classify_v2(&shape(h)).live_allowed);
        }
    }

    // ── RouteShapeV2::from_legs — honest shape derivation from decoded legs ──
    // (Phase-2 bridge; conditions imposed by cs-validator + math-validator Fase 3.)

    use crate::route_intent::RouteIntentLeg;
    use ethers::types::Address;

    fn leg(
        ti: u64,
        to: u64,
        dex: Option<&str>,
        fee: Option<u32>,
        pt: ProtocolType,
    ) -> RouteIntentLeg {
        RouteIntentLeg {
            token_in: Address::from_low_u64_be(ti),
            token_out: Address::from_low_u64_be(to),
            pool_hint: None,
            dex_hint: dex.map(|s| s.to_string()),
            fee_bps: fee,
            protocol_type: pt,
        }
    }

    #[test]
    fn from_legs_empty_is_not_applicable_no_panic() {
        // math-validator guard: must not index legs[0] on an empty slice.
        let r = classify_route_legs(&[]);
        assert!(!r.applicable);
        assert_eq!(r.strategy_kind, "");
        assert!(!RouteShapeV2::from_legs(&[]).same_pair);
    }

    #[test]
    fn from_legs_single_leg_rejected() {
        let legs = vec![leg(0xA, 0xB, Some("uni"), Some(30), ProtocolType::V2)];
        assert!(!classify_route_legs(&legs).applicable);
    }

    #[test]
    fn from_legs_same_pair_canonicalizes_reversed_round_trip() {
        // leg0 A→B, leg1 B→A is a round trip on the SAME unordered pair {A,B}.
        let legs = vec![
            leg(0xA, 0xB, Some("uni"), None, ProtocolType::V2),
            leg(0xB, 0xA, Some("sushi"), None, ProtocolType::V2),
        ];
        let s = RouteShapeV2::from_legs(&legs);
        assert!(s.same_pair, "reversed round-trip must read as same_pair");
        assert_eq!(s.distinct_dexes, 2);
        // same_pair + 2 distinct dexes ⇒ spatial_cross_dex.
        assert_eq!(classify_v2(&s).strategy_kind, "spatial_cross_dex");
    }

    #[test]
    fn from_legs_fee_none_does_not_classify_v3_fee_tier() {
        // cs-validator HIGH-1: same pair / single dex but fee_bps unknown (None) ⇒
        // distinct_fee_tiers = 0, so it must NOT claim v3_fee_tier (which needs >1).
        let legs = vec![
            leg(0xA, 0xB, Some("uni"), None, ProtocolType::V3),
            leg(0xB, 0xA, Some("uni"), None, ProtocolType::V3),
        ];
        let s = RouteShapeV2::from_legs(&legs);
        assert_eq!(s.distinct_fee_tiers, 0);
        assert_ne!(classify_v2(&s).strategy_kind, "v3_fee_tier");
    }

    #[test]
    fn from_legs_two_leg_default_else_arm_unknown_venue() {
        // math-validator: pin the `two_leg_default` else-arm — the COMMON production
        // case (dispatcher emits dex_hint: None ⇒ distinct_dexes=0).
        let legs = vec![
            leg(0xA, 0xB, None, None, ProtocolType::Unknown),
            leg(0xB, 0xA, None, None, ProtocolType::Unknown),
        ];
        let s = RouteShapeV2::from_legs(&legs);
        assert_eq!(s.distinct_dexes, 0);
        let r = classify_v2(&s);
        assert_eq!(r.strategy_kind, "spatial_cross_dex");
        assert_eq!(r.reason, "two_leg_default");
    }

    #[test]
    fn from_legs_three_leg_unknown_venue_is_conservative_same_dex() {
        // Unknown venues ⇒ distinct_dexes=0 ⇒ triangular_same_dex (do NOT over-claim
        // cross-dex without venue evidence — the honest direction).
        let legs = vec![
            leg(0xA, 0xB, None, None, ProtocolType::V2),
            leg(0xB, 0xC, None, None, ProtocolType::V2),
            leg(0xC, 0xA, None, None, ProtocolType::V2),
        ];
        assert_eq!(
            classify_route_legs(&legs).strategy_kind,
            "triangular_same_dex"
        );
    }

    #[test]
    fn from_legs_three_leg_cross_dex_cycle_is_pack_supported() {
        let legs = vec![
            leg(0xA, 0xB, Some("uni"), Some(30), ProtocolType::V2),
            leg(0xB, 0xC, Some("curve"), Some(4), ProtocolType::Curve),
            leg(0xC, 0xA, Some("sushi"), Some(30), ProtocolType::V2),
        ];
        let r = classify_route_legs(&legs);
        assert_eq!(r.strategy_kind, "triangular_cross_dex");
        assert!(is_pack_supported(&r.strategy_kind));
    }

    #[test]
    fn from_legs_mixes_v2_v3_flag() {
        let legs = vec![
            leg(0xA, 0xB, Some("uni"), None, ProtocolType::V2),
            leg(0xB, 0xC, Some("uni"), None, ProtocolType::V3),
        ];
        assert!(RouteShapeV2::from_legs(&legs).mixes_v2_and_v3);
    }

    #[test]
    fn from_legs_never_fabricates_unverifiable_signals() {
        // all_stablecoins / post_block_imbalance / cross_chain / inventory are NOT
        // derivable from legs ⇒ always false (R8 honest, never fabricated).
        let legs = vec![
            leg(0xA, 0xB, Some("curve"), Some(4), ProtocolType::Curve),
            leg(0xB, 0xA, Some("curve"), Some(4), ProtocolType::Curve),
        ];
        let s = RouteShapeV2::from_legs(&legs);
        assert!(!s.all_stablecoins);
        assert!(!s.post_block_imbalance);
        assert!(!s.cross_chain);
        assert!(!s.inventory_available);
    }

    #[test]
    fn is_pack_supported_excludes_cross_chain_keys() {
        // cs-validator HIGH-2: single-source allowlist; cross-chain keys unsupported.
        assert!(is_pack_supported("v3_fee_tier"));
        assert!(is_pack_supported("triangular_cross_dex"));
        assert!(is_pack_supported("post_block_residual_backrun"));
        assert!(!is_pack_supported("cross_chain_inventory_shadow"));
        assert!(!is_pack_supported("omnichain_shadow_route"));
        assert!(!is_pack_supported(""));
    }

    // ── Rust↔Rhai DISPATCH-KEY DRIFT CONTRACT + end-to-end bridge guarantees ──
    // OMEGA Council (cs-validator 9/10): the SUPPORTED_DISPATCH_KEYS doc-comment
    // claims "single source of truth: keep in sync with the pack" — nothing enforced
    // it until now. These lock it as a compile/CI tripwire (include_str! triggers
    // recompilation when the pack changes), plus pin the NO-ACTIVE + no-dangling-family
    // contract that cartridge_boot.rs's strategy_family_supported stamp consumes.

    /// The committed cartridge pack, embedded at COMPILE TIME.
    const OMEGA_STRATEGY_PACK_RHAI: &str =
        include_str!("../../cartridges/omega_strategy_pack.rhai");

    #[test]
    fn supported_keys_have_a_quoted_dispatch_arm_in_the_pack() {
        // Each allowlisted key must appear as a QUOTED string literal in the pack
        // (its `s == "<key>"` dispatch arm / `accept("<key>")`). The quoted form is
        // precise: it cannot be satisfied by a bare `eval_<key>` fn name. If the pack
        // drops or renames a dispatch arm, this fails — Rust allowlist ↔ Rhai drift.
        for key in SUPPORTED_DISPATCH_KEYS {
            let quoted = format!("\"{key}\"");
            assert!(
                OMEGA_STRATEGY_PACK_RHAI.contains(&quoted),
                "DRIFT: SUPPORTED_DISPATCH_KEYS lists `{key}` but omega_strategy_pack.rhai \
                 has no quoted dispatch arm for it — the Rust allowlist and the Rhai pack diverged"
            );
        }
    }

    #[test]
    fn cross_chain_keys_are_applicable_but_not_pack_dispatchable() {
        // The ONE deliberate divergence between `applicable` and "dispatchable":
        // cross-chain shapes ARE a real strategy (applicable=true) yet have NO pack
        // handler (is_pack_supported=false) → observational telemetry only, never
        // dispatched. Pin BOTH halves so a future allowlist edit can't silently
        // green-light an undispatchable family.
        let inv = RouteShapeV2 {
            hop_count: 2,
            cross_chain: true,
            inventory_available: true,
            ..Default::default()
        };
        let r1 = classify_v2(&inv);
        assert!(r1.applicable, "cross-chain IS a strategy");
        assert!(
            !is_pack_supported(&r1.strategy_kind),
            "but the pack cannot dispatch it"
        );
        assert!(!r1.live_allowed);

        let bridge = RouteShapeV2 {
            hop_count: 2,
            cross_chain: true,
            inventory_available: false,
            ..Default::default()
        };
        let r2 = classify_v2(&bridge);
        assert!(r2.applicable);
        assert!(!is_pack_supported(&r2.strategy_kind));
        assert!(!r2.live_allowed);
    }

    #[test]
    fn from_legs_applicable_implies_dispatchable_and_no_live() {
        // End-to-end bridge contract (the triple cartridge_boot.rs stamps): for EVERY
        // realistic single-chain leg shape, an `applicable` classification is ALWAYS
        // pack-dispatchable (no dangling telemetry family reaches /opportunities) and
        // NEVER live (NO-ACTIVE end-to-end from real legs).
        let dex = |i: usize| match i % 3 {
            0 => Some("uniswap-v3"),
            1 => Some("curve"),
            _ => Some("sushi"),
        };
        let proto = |i: usize| match i % 3 {
            0 => ProtocolType::V3,
            1 => ProtocolType::Curve,
            _ => ProtocolType::V2,
        };
        for hops in 2..=7usize {
            for unknown_venue in [false, true] {
                let mut legs = Vec::new();
                for i in 0..hops {
                    let a = (0xA0 + i) as u64;
                    let b = (0xA0 + (i + 1) % hops) as u64;
                    legs.push(leg(
                        a,
                        b,
                        if unknown_venue { None } else { dex(i) },
                        if unknown_venue { None } else { Some(30) },
                        proto(i),
                    ));
                }
                let r = classify_route_legs(&legs);
                if r.applicable {
                    assert!(
                        is_pack_supported(&r.strategy_kind),
                        "hops={hops} unknown_venue={unknown_venue}: applicable family `{}` is NOT pack-dispatchable",
                        r.strategy_kind
                    );
                }
                assert!(
                    !r.live_allowed,
                    "NO-ACTIVE must hold end-to-end (hops={hops})"
                );
            }
        }
    }
}
