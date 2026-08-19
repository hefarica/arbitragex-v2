//! OMEGA Route Discovery — Phase 1 radar (topology-only, shadow-only).
//!
//! A Rust engine, separate from the Rhai cartridges, that enumerates *unique
//! routes* (closed cycles) over the live pool graph, canonicalizes them,
//! decides which strategies apply, and emits shadow evidence on
//! `arbx:route_discovery:telemetry` — **without touching capital, execution,
//! or the native opportunity stream** (`arbx:opps:detected` is never written).
//!
//! ## Phase 1 scope (this module today)
//! `RouteGraphBuilder → UniqueRouteFinder (bounded DFS 2–3 hops) →
//! RouteCanonicalizer → StrategyApplicabilityEngine → RouteIntentDispatcher →
//! shadow_evaluate_intent → telemetry`.
//!
//! Phase 1 measures **topology**: it carries no sizing and no profit. The
//! Phase 2 (MMBF line-graph), Phase 3 (Marginal Price Optimization / V3
//! sizing), and later cross-rollup / GNN work are deliberately out of scope —
//! see the plan and the module docs.
//!
//! ## Safety posture
//! Gated by [`RouteDiscoveryMode`] (`ARBX_ROUTE_DISCOVERY_MODE`, **default
//! `Off`**). There is intentionally **no `Active` variant**: the radar can
//! never execute; its strongest mode is observe-only `Shadow`. When `Off`
//! (the default) nothing is spawned — zero overhead, binary behavior unchanged.

pub mod canonicalizer;
pub mod cycle_enumerator;
pub mod graph_builder;
pub mod multi_hop_search;
pub mod route_discovery_worker;
pub mod route_intent_dispatcher;
pub mod strategy_applicability;
pub mod telemetry;
pub mod triangular_adapter;
pub mod types;
pub mod unique_route_finder;

/// Cross-module safety guarantees (NO-ACTIVE / opps-untouched). Test-only.
#[cfg(test)]
mod guarantees;

use std::env;

/// Runtime mode for the route-discovery subsystem, resolved from
/// `ARBX_ROUTE_DISCOVERY_MODE`.
///
/// Mirrors [`crate::cartridge_boot::CartridgeMode`] in spirit (default-off,
/// fail-safe parse) but **omits `Active` on purpose** — there is no execution
/// path out of route discovery, so the type system forbids one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RouteDiscoveryMode {
    /// Subsystem fully disabled (default). The worker is never spawned.
    Off,
    /// Discovery + classification + telemetry; evaluation is observe-only via
    /// `shadow_evaluate_intent`. Never writes `arbx:opps:detected`.
    Shadow,
}

impl RouteDiscoveryMode {
    /// Reads `ARBX_ROUTE_DISCOVERY_MODE`. Any unset / unknown value — including
    /// the string `"active"` — resolves to `Off` (fail-safe by default).
    pub fn from_env() -> Self {
        Self::parse(&env::var("ARBX_ROUTE_DISCOVERY_MODE").unwrap_or_default())
    }

    /// Pure parser (kept separate from `from_env` so it is testable without env
    /// mutation). `"active"` deliberately maps to `Off`: no active route
    /// discovery exists.
    fn parse(raw: &str) -> Self {
        match raw.trim().to_ascii_lowercase().as_str() {
            "shadow" => Self::Shadow,
            _ => Self::Off,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Off => "off",
            Self::Shadow => "shadow",
        }
    }

    /// `true` only in `Shadow` — the sole enabled mode.
    pub fn is_enabled(self) -> bool {
        matches!(self, Self::Shadow)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mode_parse_is_fail_safe_off() {
        assert_eq!(
            RouteDiscoveryMode::parse("shadow"),
            RouteDiscoveryMode::Shadow
        );
        assert_eq!(
            RouteDiscoveryMode::parse("SHADOW"),
            RouteDiscoveryMode::Shadow
        );
        assert_eq!(
            RouteDiscoveryMode::parse("  Shadow  "),
            RouteDiscoveryMode::Shadow
        );
        // Everything else — unset, garbage, and crucially "active" — is Off.
        assert_eq!(RouteDiscoveryMode::parse(""), RouteDiscoveryMode::Off);
        assert_eq!(RouteDiscoveryMode::parse("off"), RouteDiscoveryMode::Off);
        assert_eq!(RouteDiscoveryMode::parse("active"), RouteDiscoveryMode::Off);
        assert_eq!(RouteDiscoveryMode::parse("on"), RouteDiscoveryMode::Off);
    }

    #[test]
    fn is_enabled_only_for_shadow() {
        assert!(!RouteDiscoveryMode::Off.is_enabled());
        assert!(RouteDiscoveryMode::Shadow.is_enabled());
        assert_eq!(RouteDiscoveryMode::Off.as_str(), "off");
        assert_eq!(RouteDiscoveryMode::Shadow.as_str(), "shadow");
    }
}
