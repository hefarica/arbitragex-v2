// CB-02 (2026-09-07)
// M11 allow: Lazy<Metric> initializers use .expect() on infallible paths
// (constant metric names, no duplicate registration possible across one binary)
// — the same pattern as searcher-rs/src/metrics.rs.
#![allow(clippy::expect_used)]
//! CB-02 — runtime control-plane knobs (class A), worker side.
//!
//! Implements CB-02-DISENO §3.1 (RuntimeToggleClient) + §3.2 (boot census) as
//! specified by the re-dispatch charter: structural clone of the kill-switch
//! pattern (cadence + fail-safe, `backend/shared-rs/src/killswitch.rs` — the
//! §3.1 map of CB-02-RUST-APPLY.md) with the canonical_knobs co-location style
//! for the census parses. `killswitch.rs` itself is NOT touched.
//!
//! ## Contract (INV-CB02-1..7, CB-02-DISENO §11)
//!
//! - Toggle key `arbx:controlboard:<module_id>` holds exactly `"true"` or
//!   `"false"` — the only values the api-server PUT writes
//!   (`backend/api-server/src/routes/control-board.ts::parseDeclaredValue`,
//!   format adjudicated §15-R2). This client is READ-ONLY: there is no `set()`
//!   here by design (INV-CB02-4 — the board is the single writer; no worker,
//!   script or agent may write `arbx:controlboard:*`).
//! - Fail-safe (INV-CB02-1): key absent, foreign value, or Redis down ⇒
//!   `default_when_absent` (the boot env verdict) ⇒ deployed behavior. A value
//!   the board did not write is NEVER interpreted (RULE 00 / R8).
//! - Cadence (INV-CB02-5): lazy TTL-cached poll, ≤1 GET/s per worker
//!   (kill-switch precedent, killswitch.rs `cache_ttl` 1s). A fresh read is a
//!   cached bool copy — zero allocs on the hot path.
//! - Heartbeat: `SETEX <toggle_key>:hb 75 {"state":"run"|"halted","block":N,
//!   "ts":<RFC3339>}` — exactly 1 write per received block (~0.08 QPS at 12s
//!   blocks). TTL 75s ≈ 6 blocks of grace; expiry ⇒ the board shows
//!   DESCONOCIDO (verified null), never a stale run. Writing EVERY block (not
//!   only on transitions) lets the board verify BOTH states (design §3.3).
//! - NO pub/sub in v1 (§15-R7): publishing without a subscriber is speculative
//!   (P-∅); the TTL poll is the source of truth.
//! - Mode-invariance (INV-CB02-6) / §37 Nivel-1 (INV-CB02-7): the toggle gates
//!   only WHETHER the already-deployed scan runs per block; it never changes
//!   the discovery math, sizing, or any per-mode semantics.

use crate::cartridge_boot::CartridgeMode;
use crate::chain_client::MempoolMode;
use crate::gates::MacroMevGateConfig;
use crate::scoring_pipeline::GateCScoringConfig;
use crate::workers::route_scanner_worker::RouteScannerMode;
use once_cell::sync::Lazy;
use prometheus::{IntCounter, IntGauge};
use redis::aio::ConnectionManager;
use redis::AsyncCommands;
use shared_rs::metrics::REGISTRY;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;

/// Board Redis namespace — mirror of `CONTROL_BOARD_REDIS_NAMESPACE` in
/// `backend/api-server/src/routes/control-board.ts` (CB-02-DISENO §1.2).
pub const CONTROL_BOARD_PREFIX: &str = "arbx:controlboard:";

/// Redis key of the searcher boot census (CB-02-DISENO §3.2). Consumed by the
/// CB-01 census publisher as the class-B declared side + class-A boot
/// defaults + CB-04 env-drift diff input (§15-R10 — NOT a GET requirement).
pub const BOOT_CENSUS_REDIS_KEY: &str = "arbx:config:boot_census";

/// Heartbeat TTL seconds (SETEX): ~6 blocks of grace before DESCONOCIDO.
pub const HEARTBEAT_TTL_SECS: u64 = 75;

/// Heartbeat states — consumer contract (`control-board.ts` reads
/// `state == "run"` for the verified LED; anything else is halted/unknown).
pub const HEARTBEAT_STATE_RUN: &str = "run";
pub const HEARTBEAT_STATE_HALTED: &str = "halted";

/// Poll cadence: max 1 GET/s per worker (kill-switch precedent).
const CACHE_TTL: Duration = Duration::from_secs(1);

// ─── Metrics (registered into shared_rs::metrics::REGISTRY — same pattern as
// searcher-rs/src/metrics.rs; the gauge mirrors KILLSWITCH_ENABLED which the
// kill-switch client itself sets, killswitch.rs:101/127) ─────────────────────

/// 1 while the control-board runtime gate lets the route scanner scan
/// (absent key ⇒ boot default — the gauge reflects the EFFECTIVE verdict).
pub static ROUTE_SCANNER_BOARD_ON: Lazy<IntGauge> = Lazy::new(|| {
    let g = IntGauge::new(
        "arbx_route_scanner_board_on",
        "1 when the control-board runtime gate lets the route scanner scan (absent key = boot default)",
    )
    .expect("metric");
    REGISTRY.register(Box::new(g.clone())).expect("register");
    g
});

/// Blocks received while the control-board runtime gate halted the scanner.
pub static ROUTE_SCANNER_BOARD_HALT_BLOCKS_TOTAL: Lazy<IntCounter> = Lazy::new(|| {
    let c = IntCounter::new(
        "arbx_route_scanner_board_halt_blocks_total",
        "Blocks received while the control-board runtime gate halted the route scanner",
    )
    .expect("metric");
    REGISTRY.register(Box::new(c.clone())).expect("register");
    c
});

/// Strict toggle-value resolution (design §3.1 + §15-R2): `"true"` → true,
/// `"false"` → false; absent OR any foreign value → the boot default.
/// A foreign value (wrong type, JSON, garbage) is never interpreted — the
/// board is the only writer, so anything else is drift, not a verdict (R8).
pub fn resolve_toggle(raw: Option<&str>, default_when_absent: bool) -> bool {
    match raw {
        Some("true") => true,
        Some("false") => false,
        _ => default_when_absent,
    }
}

/// Toggle key for a module id: `arbx:controlboard:<module_id>`.
pub fn toggle_key_for(module_id: &str) -> String {
    format!("{CONTROL_BOARD_PREFIX}{module_id}")
}

/// Heartbeat key for a module id: `arbx:controlboard:<module_id>:hb`.
pub fn heartbeat_key_for(module_id: &str) -> String {
    format!("{CONTROL_BOARD_PREFIX}{module_id}:hb")
}

/// Read-only runtime toggle client for one control-board module (class A).
///
/// Cheap to clone (Arc cache + ConnectionManager handle); create one per
/// spawned worker and keep it across reconnects so the 1s cache survives.
#[derive(Clone)]
pub struct RuntimeToggleClient {
    toggle_key: Arc<str>,
    heartbeat_key: Arc<str>,
    mgr: ConnectionManager,
    default_when_absent: bool,
    cache: Arc<RwLock<Option<(bool, Instant)>>>,
}

impl RuntimeToggleClient {
    /// Build from an ALREADY-CONNECTED manager (the one passed to the worker
    /// spawn — no extra connection). `default_when_absent` must be the boot
    /// env verdict so that key-absent == deployed behavior (INV-CB02-1).
    pub fn from_manager(
        mgr: ConnectionManager,
        module_id: &str,
        default_when_absent: bool,
    ) -> Self {
        Self {
            toggle_key: toggle_key_for(module_id).into(),
            heartbeat_key: heartbeat_key_for(module_id).into(),
            mgr,
            default_when_absent,
            cache: Arc::new(RwLock::new(None)),
        }
    }

    /// Fail-safe verdict: `true`/`false` per the board key, and the boot
    /// default on ANY error (kill-switch mirror, killswitch.rs:72-77).
    pub async fn is_on(&self) -> bool {
        self.state().await.unwrap_or(self.default_when_absent)
    }

    /// TTL-cached poll (≤1 GET/s). Fresh cache hit = bool copy, no allocs.
    /// Errors are propagated (and NOT cached) so callers can tell drift from
    /// Redis-down; `is_on()` folds them into the fail-safe default.
    pub async fn state(&self) -> Result<bool, redis::RedisError> {
        {
            let g = self.cache.read().await;
            if let Some((v, at)) = &*g {
                if at.elapsed() < CACHE_TTL {
                    return Ok(*v);
                }
            }
        }
        let mut mgr = self.mgr.clone();
        let raw: Option<String> = mgr.get(self.toggle_key.as_ref()).await?;
        let value = resolve_toggle(raw.as_deref(), self.default_when_absent);
        let mut g = self.cache.write().await;
        *g = Some((value, Instant::now()));
        Ok(value)
    }

    /// One heartbeat write per received block (telemetry, deliberately bounded:
    /// 1 SETEX/~12s). Written in BOTH states so the board can verify halted
    /// as well as run (design §3.3). Never panics on Redis failure — the error
    /// is returned; TTL expiry honestly surfaces as DESCONOCIDO upstream.
    pub async fn emit_heartbeat(
        &self,
        conn: &mut ConnectionManager,
        running: bool,
        block: u64,
    ) -> Result<(), redis::RedisError> {
        let payload = serde_json::json!({
            "state": if running { HEARTBEAT_STATE_RUN } else { HEARTBEAT_STATE_HALTED },
            "block": block,
            "ts": chrono::Utc::now().to_rfc3339(),
        });
        // CB-02 (2026-09-07): C infers from `conn` and T=() from the fn return —
        // no turbofish (a `::<()>` here binds the FIRST generic, the connection
        // type, and fails to compile).
        redis::cmd("SETEX")
            .arg(self.heartbeat_key.as_ref())
            .arg(HEARTBEAT_TTL_SECS)
            .arg(payload.to_string())
            .query_async(conn)
            .await
    }
}

// ─── Boot census (CB-02-DISENO §3.2) ────────────────────────────────────────
//
// Self-report of the env-derived boot gates the searcher itself consumes:
// the DECLARED side of class-B board rows + the boot defaults of class-A
// toggles + CB-04's env-drift diff input. Only fields with a cited consumer
// (RULE 00) — the field list is frozen by the design §3.2 table. The census
// is published once at boot (main.rs, next to the canonical-knobs snapshot)
// and is NOT a requirement of the control-board GET (§15-R10).

/// Mirror of scanner.rs:163-185 `OrchestratorMode::from_env` (v2/shadow/off,
/// anything else — including unset — ⇒ v1; NO trim). `scanner.rs` is declared
/// only in the main binary, while this module also compiles into the lib
/// target, so the parse is mirrored here next to its pin test; the cited
/// anchor is the source of truth and the test catches drift first.
pub fn orchestrator_mode_from_raw(raw: &str) -> &'static str {
    match raw.to_ascii_lowercase().as_str() {
        "v2" => "v2",
        "shadow" => "shadow",
        "off" => "off",
        _ => "v1",
    }
}

/// Mirror of scanner.rs:541-548 (`ARBX_NATIVE_ENGINES`, default `on`; only the
/// trimmed, case-insensitive value `off` disables). Same lib-target rationale
/// as `orchestrator_mode_from_raw`.
pub fn native_engines_from_raw(raw: &str) -> bool {
    !raw.trim().eq_ignore_ascii_case("off")
}

/// Mirror of cartridge_boot.rs:381-387 `outcomes_emission_enabled`
/// (`ARBX_ROUTE_DISCOVERY_OUTCOMES` ∈ {shadow,on,1,true}, lowercased, default
/// off). Private there, so the census mirrors it here with its pin test.
pub fn route_discovery_outcomes_from_raw(raw: &str) -> bool {
    matches!(
        raw.to_ascii_lowercase().as_str(),
        "shadow" | "on" | "1" | "true"
    )
}

/// Assemble the boot census snapshot (exact field set of design §3.2).
/// Reads the SAME public parses the workers/boot use where they exist
/// (RouteScannerMode / CartridgeMode / MempoolMode / GateCScoringConfig /
/// MacroMevGateConfig); the raw `ARBX_POOL_ENUM_MODE` string is published
/// verbatim — it is the exact input the spawn gate consumes
/// (pool_enumeration_worker.rs:187).
pub fn boot_census() -> serde_json::Value {
    let scoring = GateCScoringConfig::from_env();
    let macro_mev = MacroMevGateConfig::from_env();
    serde_json::json!({
        "published_at": chrono::Utc::now().to_rfc3339(),
        "arbx_route_scanner_mode": RouteScannerMode::from_env().as_str(),
        "arbx_orchestrator_mode": orchestrator_mode_from_raw(
            &std::env::var("ARBX_ORCHESTRATOR_MODE").unwrap_or_default()
        ),
        "arbx_cartridge_mode": CartridgeMode::from_env().as_str(),
        "arbx_mempool_mode": MempoolMode::from_env().as_str(),
        "arbx_native_engines": native_engines_from_raw(
            &std::env::var("ARBX_NATIVE_ENGINES").unwrap_or_else(|_| "on".to_string())
        ),
        "arbx_pool_enum_mode": std::env::var("ARBX_POOL_ENUM_MODE").unwrap_or_default(),
        "arbx_scoring_enabled": scoring.enabled,
        "arbx_scoring_hard_gate": scoring.hard_gate,
        "arbx_gate_macro_mev_enabled": macro_mev.enabled,
        "arbx_route_discovery_outcomes": route_discovery_outcomes_from_raw(
            &std::env::var("ARBX_ROUTE_DISCOVERY_OUTCOMES").unwrap_or_default()
        ),
    })
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    // ── GATE-CB02-2 (fail-safe Rust): absent → default, garbage → default ──

    #[test]
    fn resolve_toggle_absent_yields_boot_default() {
        assert!(!resolve_toggle(None, false));
        assert!(resolve_toggle(None, true));
    }

    #[test]
    fn resolve_toggle_strict_true_false() {
        assert!(resolve_toggle(Some("true"), false));
        assert!(!resolve_toggle(Some("false"), true));
    }

    #[test]
    fn resolve_toggle_foreign_values_never_interpreted() {
        // Values the board did not write (case variants, numerics, JSON,
        // empties) must all fold to the boot default (INV-CB02-1 / R8).
        for raw in [
            "True",
            "FALSE",
            "1",
            "0",
            "on",
            "off",
            "",
            " ",
            "null",
            "{\"enabled\":true}",
            "garbage",
        ] {
            assert!(
                !resolve_toggle(Some(raw), false),
                "raw={raw:?} must fall to default"
            );
            assert!(
                resolve_toggle(Some(raw), true),
                "raw={raw:?} must fall to default"
            );
        }
    }

    #[test]
    fn keys_follow_the_board_namespace_contract() {
        // control-board.ts: `<toggle-key>:hb` suffix + arbx:controlboard: prefix.
        assert_eq!(
            toggle_key_for("route_scanner"),
            "arbx:controlboard:route_scanner"
        );
        assert_eq!(
            heartbeat_key_for("route_scanner"),
            "arbx:controlboard:route_scanner:hb"
        );
    }

    // ── Parse mirrors: pins against their cited sources of truth ──────────

    #[test]
    fn orchestrator_mode_mirror_matches_scanner_parse() {
        // scanner.rs:165-177: v2/shadow/off explicit; unset/unknown ⇒ v1.
        assert_eq!(orchestrator_mode_from_raw("v2"), "v2");
        assert_eq!(orchestrator_mode_from_raw("SHADOW"), "shadow");
        assert_eq!(orchestrator_mode_from_raw("off"), "off");
        assert_eq!(orchestrator_mode_from_raw(""), "v1");
        assert_eq!(orchestrator_mode_from_raw("nonsense"), "v1");
    }

    #[test]
    fn native_engines_mirror_matches_scanner_parse() {
        // scanner.rs:541-548: default on; only trimmed case-insensitive "off".
        assert!(native_engines_from_raw(""));
        assert!(native_engines_from_raw("on"));
        assert!(!native_engines_from_raw("off"));
        assert!(!native_engines_from_raw(" OFF "));
        assert!(native_engines_from_raw("disable")); // not the exact "off" word
    }

    #[test]
    fn route_discovery_outcomes_mirror_matches_cartridge_boot_parse() {
        // cartridge_boot.rs:382-387: {shadow,on,1,true} lowercased; default off.
        assert!(route_discovery_outcomes_from_raw("shadow"));
        assert!(route_discovery_outcomes_from_raw("ON"));
        assert!(route_discovery_outcomes_from_raw("1"));
        assert!(route_discovery_outcomes_from_raw("True"));
        assert!(!route_discovery_outcomes_from_raw(""));
        assert!(!route_discovery_outcomes_from_raw("yes"));
    }

    // ── Census contract (consumer = CB-01 census publisher / CB-04 diff) ──

    #[test]
    fn boot_census_carries_exactly_the_design_field_set() {
        let census = boot_census();
        let obj = census.as_object().expect("census must be a JSON object");
        let expected = [
            "published_at",
            "arbx_route_scanner_mode",
            "arbx_orchestrator_mode",
            "arbx_cartridge_mode",
            "arbx_mempool_mode",
            "arbx_native_engines",
            "arbx_pool_enum_mode",
            "arbx_scoring_enabled",
            "arbx_scoring_hard_gate",
            "arbx_gate_macro_mev_enabled",
            "arbx_route_discovery_outcomes",
        ];
        // Exactly the design §3.2 fields — nothing invented, nothing missing
        // (RULE 00: a census reader must never meet surprise fields).
        assert_eq!(obj.len(), expected.len());
        for key in expected {
            assert!(obj.contains_key(key), "census missing field `{key}`");
        }
        for key in [
            "arbx_route_scanner_mode",
            "arbx_orchestrator_mode",
            "arbx_cartridge_mode",
            "arbx_mempool_mode",
            "arbx_pool_enum_mode",
        ] {
            assert!(
                obj[key].is_string(),
                "census field `{key}` must be a string"
            );
        }
        for key in [
            "arbx_native_engines",
            "arbx_scoring_enabled",
            "arbx_scoring_hard_gate",
            "arbx_gate_macro_mev_enabled",
            "arbx_route_discovery_outcomes",
        ] {
            assert!(
                obj[key].is_boolean(),
                "census field `{key}` must be a boolean"
            );
        }
        assert!(obj["published_at"].is_string());
    }
}
