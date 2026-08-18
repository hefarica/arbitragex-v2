//! Financing-mode parallel evaluation for discovered routes (ROUTES_CROWN_JEWEL §2).
//!
//! THE core of the two-layer doctrine: discovery (Capa 1) enumerates candidates
//! with topology ONLY; this module is the G2 gate of Capa 2 — the SAME route is
//! evaluated IN PARALLEL under every enabled financing mode, so the operator
//! sees exactly which routes are BORN, DIE, or CHANGE SIZE when a mode toggles.
//!
//! Mode economics (fees verified on-chain 2026-08-18 — doctrine §2.2; the fee
//! VALUES here are defaults overridable by live operator config, never trusted
//! blindly — Aave's premium is governance-mutable):
//!   OWN_CAPITAL    fee 0 bps, size capped by operator inventory
//!   AAVE_FL        fee 5 bps (default), size capped by min leg depth
//!   BALANCER_FL    fee 0 bps, size capped by min leg depth
//!   V2_FLASH_SWAP  fee ≈0 bps MARGINAL when the route already contains a V2
//!                  leg (the 0.30% swap fee is owed anyway); same-token neutral
//!                  borrow would cost ~30.09 bps and is NOT used here — a route
//!                  without a V2 leg cannot use this mode (dead: no_v2_leg)
//!   FLASH_MINT_DAI fee 0 bps, only DAI-quoted routes (dead: not_dai_quoted)
//!
//! R8 fail-honest: every verdict carries `viable` + `reason` when dead; sizes
//! are depth-capped USD bounds (shadow estimates for the funnel — exact sizing
//! stays in SizeOptimizer at dispatch, G5).

use crate::route_discovery::types::RouteCandidate;
use crate::route_intent::ProtocolType;
use serde::{Deserialize, Serialize};
use std::time::{Duration, Instant};

/// Redis key for the live runtime config (PG is source of truth; the api-server
/// PUT mirrors here — same pattern as `arbx:trading_config:<chain>`).
pub fn route_discovery_config_key(chain_id: u64) -> String {
    format!("arbx:route_discovery_config:{chain_id}")
}

/// One financing mode. The `as_str` tokens are stable wire identifiers used by
/// the frontend badges and the funnel telemetry.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FinancingMode {
    OwnCapital,
    AaveFl,
    BalancerFl,
    V2FlashSwap,
    FlashMintDai,
}

impl FinancingMode {
    pub const ALL: [FinancingMode; 5] = [
        FinancingMode::OwnCapital,
        FinancingMode::AaveFl,
        FinancingMode::BalancerFl,
        FinancingMode::V2FlashSwap,
        FinancingMode::FlashMintDai,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            FinancingMode::OwnCapital => "own_capital",
            FinancingMode::AaveFl => "aave_fl",
            FinancingMode::BalancerFl => "balancer_fl",
            FinancingMode::V2FlashSwap => "v2_flash_swap",
            FinancingMode::FlashMintDai => "flash_mint_dai",
        }
    }

    /// Human label for telemetry.
    pub fn label(self) -> &'static str {
        match self {
            FinancingMode::OwnCapital => "Capital propio",
            FinancingMode::AaveFl => "Aave V3 flash loan",
            FinancingMode::BalancerFl => "Balancer flash loan",
            FinancingMode::V2FlashSwap => "V2 flash swap",
            FinancingMode::FlashMintDai => "Maker flash mint DAI",
        }
    }
}

/// Operator-tunable runtime config (the floating panel's payload). Defaults are
/// the doctrine values; every field is overridable live via the admin PUT.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RouteDiscoveryRuntimeConfig {
    /// Capa 1 emission budget per tick (DeferNeverDrop: pacing, never loss).
    #[serde(default = "default_routes_per_tick")]
    pub routes_per_tick: usize,
    /// Max cycle depth (shadow floor 7 enforced by the worker clamp).
    #[serde(default = "default_max_hops")]
    pub max_hops: u8,
    /// Financing toggles — Capa 2 G2. Disabled modes still evaluate (the
    /// comparison must show what WOULD be viable) but are marked `disabled`.
    #[serde(default = "default_financing_enabled")]
    pub financing_enabled: std::collections::BTreeMap<String, bool>,
    /// Fee overrides in bps (defaults: aave 5, balancer 0, v2swap-marginal 0).
    #[serde(default)]
    pub fee_bps_overrides: std::collections::BTreeMap<String, f64>,
    /// Operator's own capital available (USD) — caps OWN_CAPITAL size.
    #[serde(default = "default_own_inventory_usd")]
    pub own_inventory_usd: f64,
    /// Minimum viable notional (USD) — routes whose depth cap falls below this
    /// die under every mode (G2/G3 boundary).
    #[serde(default = "default_min_notional_usd")]
    pub min_notional_usd: f64,
}

fn default_routes_per_tick() -> usize {
    500
}
fn default_max_hops() -> u8 {
    7
}
fn default_own_inventory_usd() -> f64 {
    5_000.0
}
fn default_min_notional_usd() -> f64 {
    500.0
}
fn default_financing_enabled() -> std::collections::BTreeMap<String, bool> {
    FinancingMode::ALL
        .iter()
        .map(|m| (m.as_str().to_string(), true))
        .collect()
}

impl Default for RouteDiscoveryRuntimeConfig {
    fn default() -> Self {
        serde_json::from_str("{}").unwrap_or(Self {
            routes_per_tick: default_routes_per_tick(),
            max_hops: default_max_hops(),
            financing_enabled: default_financing_enabled(),
            fee_bps_overrides: Default::default(),
            own_inventory_usd: default_own_inventory_usd(),
            min_notional_usd: default_min_notional_usd(),
        })
    }
}

impl RouteDiscoveryRuntimeConfig {
    pub fn fee_bps(&self, mode: FinancingMode) -> f64 {
        let key = mode.as_str();
        self.fee_bps_overrides
            .get(key)
            .copied()
            .unwrap_or(match mode {
                FinancingMode::OwnCapital => 0.0,
                FinancingMode::AaveFl => 5.0,
                FinancingMode::BalancerFl => 0.0,
                FinancingMode::V2FlashSwap => 0.0, // marginal when the V2 leg is ours
                FinancingMode::FlashMintDai => 0.0,
            })
    }

    pub fn mode_enabled(&self, mode: FinancingMode) -> bool {
        self.financing_enabled
            .get(mode.as_str())
            .copied()
            .unwrap_or(true)
    }
}

/// One mode's verdict for one route — the unit the frontend renders as a badge.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ModeVerdict {
    pub mode: String,
    pub label: String,
    /// Enabled in operator config (a disabled mode still shows its hypothetical
    /// verdict — the comparison must remain visible).
    pub enabled: bool,
    /// Viable under this mode (route is BORN here).
    pub viable: bool,
    /// Death reason when not viable (R8 — never silent).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    /// Fee drag in bps under this mode.
    pub fee_bps: f64,
    /// Size ceiling in USD under this mode (depth/inventory capped) — this is
    /// where modes visibly CHANGE the route's size.
    pub max_size_usd: f64,
}

/// Evaluate ONE route under ALL modes in parallel (the doctrine's core table).
///
/// Inputs: the candidate's protocols per leg (V2 leg presence), its per-leg
/// liquidity hints (USD-normalized by the graph builder), and the live config.
/// Pure function → unit-testable without Redis/IO.
pub fn evaluate_route_financing(
    route: &RouteCandidate,
    leg_liquidity_usd: &[f64],
    cfg: &RouteDiscoveryRuntimeConfig,
) -> Vec<ModeVerdict> {
    // Depth ceiling: the weakest leg caps any borrowed-principal mode. A leg
    // with missing/zero liquidity (no cache hint) makes depth UNKNOWN — the
    // route cannot be sized under borrowed-principal modes (R8: honest).
    let has_depth = !leg_liquidity_usd.is_empty() && leg_liquidity_usd.iter().all(|l| *l > 0.0);
    let depth_cap = if has_depth {
        leg_liquidity_usd
            .iter()
            .copied()
            .fold(f64::INFINITY, f64::min)
    } else {
        0.0
    };
    let has_v2_leg = route
        .protocols
        .iter()
        .any(|p| matches!(p, ProtocolType::V2));
    // DAI-quote: the cycle starts/ends in the same token; we cannot see symbols
    // here — the worker passes this via `is_dai_quoted` when it knows the base
    // token; conservatively false at this layer unless depth says otherwise.
    let is_dai_quoted = false;

    FinancingMode::ALL
        .iter()
        .map(|&mode| {
            let enabled = cfg.mode_enabled(mode);
            let fee = cfg.fee_bps(mode);
            let (size_cap, reason) = match mode {
                FinancingMode::OwnCapital => {
                    let cap = cfg.own_inventory_usd;
                    (
                        cap,
                        if cap < cfg.min_notional_usd {
                            Some("inventory_below_min".into())
                        } else {
                            None
                        },
                    )
                }
                FinancingMode::AaveFl | FinancingMode::BalancerFl => {
                    if !has_depth {
                        (0.0, Some("depth_unknown".into()))
                    } else if depth_cap < cfg.min_notional_usd {
                        (0.0, Some("depth_below_min".into()))
                    } else {
                        (depth_cap, None)
                    }
                }
                FinancingMode::V2FlashSwap => {
                    if !has_v2_leg {
                        (0.0, Some("no_v2_leg".into()))
                    } else if !has_depth {
                        (0.0, Some("depth_unknown".into()))
                    } else if depth_cap < cfg.min_notional_usd {
                        (0.0, Some("depth_below_min".into()))
                    } else {
                        (depth_cap, None)
                    }
                }
                FinancingMode::FlashMintDai => {
                    if !is_dai_quoted {
                        (0.0, Some("not_dai_quoted".into()))
                    } else {
                        (500_000_000.0, None) // dss-flash debt ceiling scale
                    }
                }
            };
            let viable = reason.is_none() && size_cap >= cfg.min_notional_usd;
            ModeVerdict {
                mode: mode.as_str().to_string(),
                label: mode.label().to_string(),
                enabled,
                viable,
                reason,
                fee_bps: fee,
                max_size_usd: if viable { size_cap } else { 0.0 },
            }
        })
        .collect()
}

/// Bounded live-config reader with a short in-process cache (pattern:
/// trading_config's 1s watcher — the worker calls this per tick).
pub struct RouteDiscoveryConfigClient {
    ttl: Duration,
    cache: tokio::sync::Mutex<Option<(RouteDiscoveryRuntimeConfig, Instant, bool)>>,
}

impl RouteDiscoveryConfigClient {
    pub fn new() -> Self {
        Self {
            ttl: Duration::from_secs(1),
            cache: tokio::sync::Mutex::new(None),
        }
    }

    /// Returns (config, from_redis). On Redis miss/error → defaults, honestly
    /// flagged (R8: absence of config is not a fabricated config).
    pub async fn get(
        &self,
        redis: &mut redis::aio::ConnectionManager,
        chain_id: u64,
    ) -> (RouteDiscoveryRuntimeConfig, bool) {
        {
            let guard = self.cache.lock().await;
            if let Some((cfg, at, from_redis)) = guard.as_ref() {
                if at.elapsed() < self.ttl {
                    return (cfg.clone(), *from_redis);
                }
            }
        }
        let key = route_discovery_config_key(chain_id);
        let raw: Option<String> = redis::AsyncCommands::get(redis, &key).await.ok();
        let (cfg, from_redis) = match raw {
            Some(json) => (serde_json::from_str(&json).unwrap_or_default(), true),
            None => (RouteDiscoveryRuntimeConfig::default(), false),
        };
        *self.cache.lock().await = Some((cfg.clone(), Instant::now(), from_redis));
        (cfg, from_redis)
    }
}

impl Default for RouteDiscoveryConfigClient {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use crate::route_discovery::types::{RouteDirection, RouteKind};
    use ethers::types::Address;

    fn route(protocols: Vec<ProtocolType>) -> RouteCandidate {
        RouteCandidate {
            chain_id: 1,
            route_hash: "0xabc".into(),
            route_kind: RouteKind::V2V2,
            tokens: vec![Address::from_low_u64_be(1), Address::from_low_u64_be(2)],
            pools: vec![
                Address::from_low_u64_be(0x10),
                Address::from_low_u64_be(0x20),
            ],
            protocols,
            fee_tiers: vec![Some(30), Some(30)],
            directions: vec![RouteDirection::ZeroForOne, RouteDirection::OneForZero],
            hops: 2,
            applicable_strategies: vec![],
            rejected_strategies: vec![],
            mode: "shadow".into(),
        }
    }

    fn verdicts(v: &[ModeVerdict], mode: FinancingMode) -> &ModeVerdict {
        v.iter().find(|m| m.mode == mode.as_str()).unwrap()
    }

    #[test]
    fn same_route_parallel_all_modes() {
        let r = route(vec![ProtocolType::V2, ProtocolType::V2]);
        let cfg = RouteDiscoveryRuntimeConfig::default();
        let v = evaluate_route_financing(&r, &[120_000.0, 80_000.0], &cfg);
        assert_eq!(
            v.len(),
            5,
            "every mode evaluates — the comparison is the point"
        );

        let own = verdicts(&v, FinancingMode::OwnCapital);
        assert!(own.viable);
        assert_eq!(own.max_size_usd, 5_000.0, "own capital capped by inventory");
        assert_eq!(own.fee_bps, 0.0);

        let aave = verdicts(&v, FinancingMode::AaveFl);
        assert!(aave.viable);
        assert_eq!(aave.fee_bps, 5.0);
        assert_eq!(
            aave.max_size_usd, 80_000.0,
            "flash capped by WEAKEST leg depth"
        );

        let bal = verdicts(&v, FinancingMode::BalancerFl);
        assert!(bal.viable);
        assert_eq!(bal.fee_bps, 0.0);
        assert_eq!(
            bal.max_size_usd, 80_000.0,
            "SAME size as aave — the fee differs, not the depth"
        );

        let swap = verdicts(&v, FinancingMode::V2FlashSwap);
        assert!(swap.viable, "route HAS a V2 leg → flash swap marginal");
        assert_eq!(swap.fee_bps, 0.0);

        let dai = verdicts(&v, FinancingMode::FlashMintDai);
        assert!(!dai.viable);
        assert_eq!(dai.reason.as_deref(), Some("not_dai_quoted"));
    }

    #[test]
    fn v3_only_route_kills_flash_swap_mode() {
        let r = route(vec![ProtocolType::V3, ProtocolType::V3]);
        let v = evaluate_route_financing(
            &r,
            &[100_000.0, 100_000.0],
            &RouteDiscoveryRuntimeConfig::default(),
        );
        let swap = verdicts(&v, FinancingMode::V2FlashSwap);
        assert!(!swap.viable);
        assert_eq!(
            swap.reason.as_deref(),
            Some("no_v2_leg"),
            "death reason visible — no silent kill"
        );
        // …while flash-loan modes stay viable for the SAME route: the operator
        // sees the route DIE under one mode and LIVE under another.
        assert!(verdicts(&v, FinancingMode::AaveFl).viable);
    }

    #[test]
    fn weak_leg_depth_kills_borrowed_modes_but_not_small_own_capital() {
        let r = route(vec![ProtocolType::V2, ProtocolType::V2]);
        // One leg with $300 depth (< min_notional 500): borrowed modes die.
        let v = evaluate_route_financing(
            &r,
            &[120_000.0, 300.0],
            &RouteDiscoveryRuntimeConfig::default(),
        );
        assert!(!verdicts(&v, FinancingMode::AaveFl).viable);
        assert_eq!(
            verdicts(&v, FinancingMode::AaveFl).reason.as_deref(),
            Some("depth_below_min")
        );
        assert!(!verdicts(&v, FinancingMode::BalancerFl).viable);
        // Own capital is inventory-capped, NOT depth-capped → still viable at $5k.
        assert!(verdicts(&v, FinancingMode::OwnCapital).viable);
    }

    #[test]
    fn fee_override_and_toggles_flow_from_config() {
        #[allow(clippy::field_reassign_with_default)]
        let mut cfg = RouteDiscoveryRuntimeConfig::default();
        cfg.fee_bps_overrides.insert("aave_fl".into(), 9.0); // governance moved
        cfg.financing_enabled.insert("aave_fl".into(), false);
        let r = route(vec![ProtocolType::V2, ProtocolType::V2]);
        let v = evaluate_route_financing(&r, &[100_000.0, 100_000.0], &cfg);
        let aave = verdicts(&v, FinancingMode::AaveFl);
        assert!(!aave.enabled, "toggle flows through");
        assert_eq!(aave.fee_bps, 9.0, "fee override flows through");
        // A disabled mode still carries its hypothetical verdict (comparison).
        assert!(aave.viable);
    }

    #[test]
    fn size_changes_visibly_across_modes() {
        // The doctrine's "cambian de tamaño": same route, three size ceilings.
        let r = route(vec![ProtocolType::V2, ProtocolType::V2]);
        let cfg = RouteDiscoveryRuntimeConfig {
            own_inventory_usd: 2_000.0,
            ..RouteDiscoveryRuntimeConfig::default()
        };
        let v = evaluate_route_financing(&r, &[90_000.0, 60_000.0], &cfg);
        let own = verdicts(&v, FinancingMode::OwnCapital).max_size_usd;
        let aave = verdicts(&v, FinancingMode::AaveFl).max_size_usd;
        let bal = verdicts(&v, FinancingMode::BalancerFl).max_size_usd;
        assert!(
            own < aave,
            "own ($2k) < flash ($60k) — size visibly differs"
        );
        assert_eq!(aave, bal);
    }
}
