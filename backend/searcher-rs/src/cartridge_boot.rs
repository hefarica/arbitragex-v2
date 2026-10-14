//! FASE OMEGA — Cartridge runtime boot wiring.
//!
//! Bridges the (already-implemented but previously **unspawned**) cartridge
//! subsystem into the searcher hot-path lifecycle. Before this module, the
//! `CartridgeSubscriber` was never started and `load_cartridges_from_dir` was
//! never called, so the registry stayed empty forever — the whole runtime was
//! dead code in production.
//!
//! Behavior is gated entirely by `ARBX_CARTRIDGE_MODE`:
//!
//! | Mode     | Behavior                                                                 |
//! |----------|--------------------------------------------------------------------------|
//! | `off`    | (default) Nothing is constructed. Byte-for-byte unchanged scanner.        |
//! | `shadow` | Cartridges load from `cartridges/` + Redis hot-reload subscriber runs.    |
//! | `active` | Reserved. Behaves as `shadow` today — execution wiring is deferred to a   |
//! |          | follow-up iteration gated by paper-trade evidence (see `arbx-paper-trade-first`). |
//!
//! The orchestrator evaluation hook (calling `runner.evaluate()` per pending tx
//! and routing candidates through the existing gate pipeline) is the **next**
//! iteration — this module only makes the subsystem boot and hot-reload.

use std::sync::atomic::AtomicU64;
use std::sync::{Arc, OnceLock};
use tokio::sync::{RwLock, Semaphore};
use tokio_util::sync::CancellationToken;
use tracing::{debug, info, warn};

use crate::cartridge::host_bindings::HostContext;
use crate::cartridge::runner::CartridgeRunner;
use crate::cartridge::subscriber::CartridgeSubscriber;
use crate::cartridge::types::{CartridgeEvalResult, CartridgeState};
use crate::cartridge_loader::{self, CARTRIDGE_DIR};
use crate::route_intent::{DetectionSource, ProtocolType, RouteIntent};
use crate::strategy_label::StrategyLabel;

/// Telemetry channel the host bindings publish `log_quantum` messages to.
/// Matches `cartridge::host_bindings` / `cartridge::runner` defaults.
const CARTRIDGE_TELEMETRY_CHANNEL: &str = "arbx:cartridge:telemetry";

/// Global cap on concurrent shadow evaluations across all chains. Shadow tasks are
/// detached per intent; this bounds the CPU-bound Rhai work so a mempool burst cannot
/// saturate the Tokio worker pool and starve the main pipeline. Excess tasks acquire-fail
/// and return immediately — observe-only, so dropping an evaluation is acceptable.
const SHADOW_MAX_CONCURRENCY: usize = 16;
static SHADOW_SEMAPHORE: OnceLock<Arc<Semaphore>> = OnceLock::new();

fn shadow_semaphore() -> &'static Arc<Semaphore> {
    SHADOW_SEMAPHORE.get_or_init(|| Arc::new(Semaphore::new(SHADOW_MAX_CONCURRENCY)))
}

/// Runtime mode for the cartridge subsystem, resolved from `ARBX_CARTRIDGE_MODE`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CartridgeMode {
    /// Subsystem fully disabled (default). Nothing is spawned, zero overhead.
    Off,
    /// Cartridges load + hot-reload subscriber runs (evaluation emits telemetry only).
    Shadow,
    /// Reserved for full hot-path evaluation → execution. Deferred; behaves as `Shadow` today.
    Active,
}

impl CartridgeMode {
    /// Reads `ARBX_CARTRIDGE_MODE`. Any unset / unknown value resolves to `Off`
    /// (dormant) — fail-safe by default.
    pub fn from_env() -> Self {
        Self::parse(&std::env::var("ARBX_CARTRIDGE_MODE").unwrap_or_default())
    }

    /// Pure parser (kept separate from `from_env` so it is testable without env mutation).
    fn parse(raw: &str) -> Self {
        match raw.trim().to_ascii_lowercase().as_str() {
            "shadow" => Self::Shadow,
            "active" => Self::Active,
            _ => Self::Off,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Off => "off",
            Self::Shadow => "shadow",
            Self::Active => "active",
        }
    }

    /// `true` for any mode other than `Off`.
    pub fn is_enabled(self) -> bool {
        !matches!(self, Self::Off)
    }
}

/// Spawns the per-chain cartridge runtime: builds the runner, then on a dedicated
/// tokio task loads filesystem cartridges from the `cartridges/` directory and runs
/// the Redis hot-reload subscriber until `cancel` fires.
///
/// Returns the shared `Arc<CartridgeRunner>` plus the AGENT v4
/// `Arc<ContextRouter>` (Fase 3a) so the orchestrator can also evaluate
/// cartridges and register per-intent real snapshot contexts (the registry is
/// shared via `Arc`/`RwLock`, so cartridges loaded by the subscriber are
/// visible to the orchestrator). The router half is `None` only when the
/// ContextRouter itself could not be created/seeded — the runner then falls
/// back to the Phase-1 static service. Returns `None` only when `REDIS_URL`
/// is absent (fail-honest, no boot).
///
/// Callers MUST only invoke this when `mode.is_enabled()`. The subscriber task is
/// fire-and-forget; any failure is logged and never fatal to the scanner.
///
/// # Panics
/// Must be called from within a Tokio runtime — it uses `Handle::current()` and
/// `tokio::spawn`. The sole call site is `scanner::run_chain`, which always runs
/// inside the runtime.
pub fn spawn_cartridge_runtime(
    chain_id: u64,
    redis: redis::aio::ConnectionManager,
    rpc_pool: Option<Arc<shared_rs::rpc_failover::HttpRpcPool>>,
    cancel: CancellationToken,
    mode: CartridgeMode,
) -> Option<(
    Arc<CartridgeRunner>,
    Option<Arc<crate::context_router::ContextRouter>>,
)> {
    // The hot-reload subscriber opens its OWN Redis client from a URL (see
    // `subscriber.rs`). Fail-honest: if `REDIS_URL` is absent we skip cartridge
    // boot rather than hardcode a localhost default (arbx-no-hardcode-doctrine).
    let redis_url = match std::env::var("REDIS_URL") {
        Ok(u) if !u.is_empty() => u,
        _ => {
            warn!(
                event = "cartridge.boot_skipped",
                chain_id,
                mode = mode.as_str(),
                reason = "REDIS_URL not set",
                "cartridge runtime not started — no Redis URL for hot-reload subscriber"
            );
            return None;
        }
    };

    // Clone the ConnectionManager BEFORE it is moved into HostContext, so the
    // registry publisher can write the loaded-cartridge snapshot to Redis.
    // ConnectionManager is an internally-multiplexed handle — clone is cheap.
    let mut registry_redis = redis.clone();

    // EXACT-QUOTES-PRODUCER-01 (2026-10-04): el pool RPC con failover que
    // `main.rs` construye llega a este módulo UNA vez, aquí, y se MUEVE al
    // `HostContext`. El camino del intent (orchestrator → active_evaluate_and_emit
    // → build_and_register_intent_context) corre en OTROS tasks y no lo recibe;
    // este registro lo publica para el productor de quotes exactas sin abrir una
    // segunda conexión, sin releer env y sin duplicar el presupuesto de RPC del
    // proceso. Sin pool registrado el productor no inventa nada: no cotiza.
    if let Some(pool) = rpc_pool.as_ref() {
        v4_exact_quote_pools()
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .insert(chain_id, pool.clone());
    }

    let host_ctx = HostContext {
        redis: Arc::new(RwLock::new(redis)),
        chain_id,
        cartridge_id: Arc::new(RwLock::new(String::new())),
        rt_handle: tokio::runtime::Handle::current(),
        // Updated by the scanner/gas-oracle later; start at 0 (host bindings read these atomics).
        block_number: Arc::new(AtomicU64::new(0)),
        base_fee_gwei: Arc::new(AtomicU64::new(0)),
        telemetry_channel: CARTRIDGE_TELEMETRY_CHANNEL.to_owned(),
        // simulate_swap RPC plumbing — read-only failover pool (None ⇒ V2 cached-
        // reserves path only; no RPC attempted). Token bucket (max=10, refill
        // 10/sec) + 100ms min-interval floor bound a runaway cartridge loop.
        rpc_pool,
        rpc_budget: Arc::new(std::sync::Mutex::new(
            crate::cartridge::host_bindings::RpcBudget::new(10, 10),
        )),
        rpc_min_interval_ns: Arc::new(AtomicU64::new(
            crate::cartridge::host_bindings::SIM_SWAP_RPC_MIN_INTERVAL_NS,
        )),
        rpc_last_call_ns: Arc::new(AtomicU64::new(0)),
    };

    // ── AGENT v4 (integration/agent-cartridges-v4): Phase-1 honest context ──
    // Registers the agent_v4_* bindings backed by a SnapshotServices built
    // from a DATA_GAP bundle: edges/prices/quotes are EMPTY (nothing
    // fabricated — R8), so v4 cartridges run their real flow and report the
    // missing producers as explicit reasons instead of silent zeros. Real
    // producers wire into this bundle in later phases (issue #647 family).
    // Ids are deterministic so `build_cartridge_pool_data` call sites can
    // stamp them into pool_data (SnapshotServices::check requires ctx
    // identity match). Validity is 24h — an empty DATA_GAP bundle has no
    // real data to go stale; a restart rebuilds it fresh.
    let v4_now_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0);
    let v4_context_id = format!("boot-chain-{chain_id}");
    let v4_snapshot_id = format!("boot-genesis-{chain_id}");
    let v4_policy = crate::rhai_agent_bridge::PolicyView {
        enabled: true,
        capital_cap_usd: "0".into(),
        min_profit_usd: None,
        max_gas_usd: None,
        snapshot_id: v4_snapshot_id.clone(),
        price_revision: "phase1_data_gap".into(),
        policy_revision: "phase1_data_gap".into(),
        execution_mode: "paper_shadow".into(),
        control_state: "phase1_data_gap".into(),
    };
    let v4_bundle = Arc::new(crate::snapshot_services::SnapshotBundle {
        context_id: v4_context_id.clone(),
        snapshot_id: v4_snapshot_id.clone(),
        observed_at_ms: v4_now_ms,
        valid_until_ms: v4_now_ms + 86_400_000,
        policy: v4_policy,
        start_token: String::new(),
        chain_id,
        edges: Vec::new(),
        limits: crate::agent_graph::SearchLimits {
            max_hops: 0,
            max_expansions: 0,
            max_paths: 0,
        },
        size_schedule_raw: Vec::new(),
        prices: Default::default(),
        exact_quotes: Default::default(),
        route_support: Default::default(),
        domain_plans: Default::default(),
        canonical_payloads: Default::default(),
        // ADMISSION-MANIFEST-01 (2026-10-01): el stub Phase-1 es el contexto
        // que se usa cuando el intent no arma aristas (v4_edges vacío) o el
        // router no está disponible. Antes llevaba `manifest_digests` VACÍO,
        // así que un cartucho DESPLEGADO —cuyo digest SÍ está en el escaneo
        // de CARTRIDGE_DIR— fallaba la admisión con
        // `manifest_not_admitted_by_backend` por un mapa que se construía
        // vacío en este camino y lleno en el del intent. Medido en producción:
        // ese motivo explica 2.488 de 20.000 outcomes. Los digests describen
        // la librería desplegada, que se conoce en boot: no se fabrica nada.
        manifest_digests: v4_manifest_digests().clone(),
        max_evaluations: 1,
        // COST-PRODUCERS-01: el stub no computa costes (sin gas observado ni
        // config del intent) → vacío = DATA_GAP honesto, nunca ceros.
        base_cost_lines: Vec::new(),
        // REDEMPTION-PRODUCER-01: el stub no lee estado on-chain de baskets.
        redemption_state: std::collections::BTreeMap::new(),
    });
    // Single-revision Phase-1 guard: this process serves exactly the bundle it
    // booted with; a restart rebuilds a fresh (equally-honest) bundle.
    let v4_revision: crate::snapshot_services::RevisionGuard =
        Arc::new(move |_policy_rev: &str, _price_rev: &str| true);
    let v4_services = match crate::snapshot_services::SnapshotServices::new(v4_bundle, v4_revision)
    {
        Ok(s) => Arc::new(s),
        Err(e) => {
            tracing::error!(
                event = "cartridge.v4_services_failed",
                chain_id,
                reason = %e,
                "AGENT v4 Phase-1 services rejected"
            );
            return None;
        }
    };

    // ── AGENT v4 Fase 3a (integration/agent-cartridges-v4): ContextRouter ──
    // El router implementa AgentServices por delegación resolviendo por
    // ctx["context_id"]. El contexto DATA_GAP existente queda insertado como
    // FALLBACK; la tarea ACTIVE registra además un SnapshotBundle REAL por
    // intent (context_id "intent-{uuid}") y lo retira al terminar (guard con
    // Drop). Ante un router indisponible se conserva el comportamiento
    // Phase-1 exacto (servicio estático único) — fail-honest, nunca fatal.
    let v4_router = match crate::context_router::ContextRouter::new(256) {
        Ok(router) => {
            let router = Arc::new(router);
            match router.insert(v4_context_id.clone(), v4_services.clone()) {
                Ok(()) => Some(router),
                Err(e) => {
                    warn!(
                        event = "cartridge.v4_router_data_gap_insert_failed",
                        chain_id,
                        reason = %e,
                        "ContextRouter sin contexto DATA_GAP; fallback a servicio estático Phase-1"
                    );
                    None
                }
            }
        }
        Err(e) => {
            warn!(
                event = "cartridge.v4_router_init_failed",
                chain_id,
                reason = %e,
                "ContextRouter no disponible; fallback a servicio estático Phase-1"
            );
            None
        }
    };

    // Build the runner BEFORE spawning so we can share the Arc with both the
    // subscriber task and the orchestrator (shadow/active evaluation). Con
    // router: los bindings agent_v4_* resuelven por context_id (DATA_GAP +
    // intents reales); sin router: servicio estático Phase-1 (idéntico a
    // main previo a Fase 3a).
    let runner = Arc::new(match v4_router.as_ref() {
        Some(router) => CartridgeRunner::new(host_ctx).with_agent_services(router.clone()),
        None => CartridgeRunner::new(host_ctx).with_agent_services(v4_services),
    });
    let runner_for_task = runner.clone();

    // CARTRIDGE-CONTROL: clone the cancellation token + Redis URL BEFORE the
    // boot task below moves them — the hot couple/decouple loop needs its own.
    let control_cancel = cancel.clone();
    let control_redis_url = redis_url.clone();

    tokio::spawn(async move {
        // Boot-load cartridges from the filesystem directory (dev/bootstrap path).
        // Redis-injected cartridges arrive later via the subscriber.
        let dir = std::path::Path::new(CARTRIDGE_DIR);
        let results =
            cartridge_loader::load_cartridges_from_dir(&runner_for_task, dir, chain_id).await;
        let loaded = results.iter().filter(|r| r.success).count();
        info!(
            event = "cartridge.boot_loaded",
            chain_id,
            mode = mode.as_str(),
            loaded,
            total = results.len(),
            "cartridge runtime booted; filesystem cartridges loaded"
        );

        // Publish the loaded-cartridge registry snapshot to Redis so the
        // api-server can serve GET /api/cartridges with the REAL loaded set
        // (not telemetry-gated). TTL is a safety net: if the searcher dies the
        // key expires and the API fails honest instead of serving stale rows.
        publish_cartridge_registry(&mut registry_redis, &runner_for_task, chain_id).await;

        // ── CARTRIDGE-CONTROL (boot application): acoplar/desacoplar según el
        // estado deseado persistido. Corre DESPUÉS de la carga para no correr
        // contra ella; sin hash alcanzable no desacopla nada (fail-open hacia
        // el estado de carga por defecto, documentado en cartridge_control).
        crate::cartridge_control::apply_desired_states(
            &runner_for_task,
            &mut registry_redis,
            chain_id,
        )
        .await;

        // Registry REFRESH loop — the snapshot TTL is 600s but the searcher
        // runs for days; without a periodic re-publish the key expires and
        // GET /api/cartridges/runtime falls back to "registry_unavailable"
        // even though cartridges are loaded. Re-publish every REFRESH_SECS
        // (< TTL) so the registry stays live AND reflects pause/resume toggles
        // in near-real-time. Fire-and-forget; failures are logged, never fatal.
        {
            let mut refresh_redis = registry_redis.clone();
            let runner_for_refresh = runner_for_task.clone();
            let refresh_cancel = cancel.clone();
            tokio::spawn(async move {
                const REFRESH_SECS: u64 = 240; // < 600s TTL with margin
                loop {
                    tokio::select! {
                        _ = tokio::time::sleep(std::time::Duration::from_secs(REFRESH_SECS)) => {
                            publish_cartridge_registry(&mut refresh_redis, &runner_for_refresh, chain_id).await;
                        }
                        _ = refresh_cancel.cancelled() => break,
                    }
                }
            });
        }

        // Run the hot-reload subscriber (long-running; returns on cancellation).
        let subscriber = CartridgeSubscriber::new(redis_url, runner_for_task, cancel);
        subscriber.run().await;

        info!(
            event = "cartridge.runtime_stopped",
            chain_id, "cartridge subscriber task exited"
        );
    });

    // ── CARTRIDGE-CONTROL: loop de comandos en caliente (canal PubSub) ────────
    {
        let runner_control = runner.clone();
        let control_cancel = control_cancel;
        tokio::spawn(async move {
            crate::cartridge_control::control_loop(
                runner_control,
                control_redis_url,
                chain_id,
                control_cancel,
            )
            .await;
        });
    }

    Some((runner, v4_router))
}

/// Maps a `ProtocolType` to the lowercase string cartridges expect in `pool_data`.
fn protocol_type_str(pt: ProtocolType) -> &'static str {
    match pt {
        ProtocolType::V2 => "v2",
        ProtocolType::V3 => "v3",
        ProtocolType::Curve => "curve",
        ProtocolType::Balancer => "balancer",
        ProtocolType::Unknown => "unknown",
    }
}

/// Protocol-aware cartridge boundary. RouteIntent V3 fees are raw pips;
/// `fee_bps` is retained in actual basis points for legacy cartridges.
fn insert_cartridge_fee_fields(map: &mut rhai::Map, leg: &crate::route_intent::RouteIntentLeg) {
    use rhai::Dynamic;
    let Some(fee) = leg.fee_bps else {
        return;
    };
    if leg.protocol_type == ProtocolType::V3 {
        if fee >= 1_000_000 {
            map.insert(
                "fee_error".into(),
                Dynamic::from("v3_fee_out_of_range".to_string()),
            );
            return;
        }
        map.insert("fee_pips".into(), Dynamic::from(fee as i64));
        let bps = if fee % 100 == 0 {
            Dynamic::from((fee / 100) as i64)
        } else {
            Dynamic::from(f64::from(fee) / 100.0)
        };
        map.insert("fee_bps".into(), bps);
    } else {
        map.insert("fee_bps".into(), Dynamic::from(fee as i64));
    }
}

/// Builds the `pool_data` Rhai `Map` passed to a cartridge's `evaluate_opportunity`.
///
/// Pure function (no I/O), built from the first leg of the route intent plus the
/// pre-fetched source-pool reserves. `reserves_source` is injected as a nested Rhai
/// Map `#{ r0, r1, block, ts, token0_addr }` (the field names dex_arb.rhai reads —
/// note `block`, not the Rust `blk`). When `None` (no fresh reserves in Redis), the
/// key is omitted → `pool_data.reserves_source == ()` → reserve-dependent cartridges
/// fail-honest (R8). Likewise `source_pool` is empty when `pool_hint` is None.
/// Gas / block number stay host-binding-sourced (`get_base_fee`, `get_block_number`).
pub fn build_cartridge_pool_data(
    intent: &RouteIntent,
    reserves_source: Option<&crate::reserves::ReservesEntry>,
) -> rhai::Map {
    use rhai::Dynamic;
    let mut m = rhai::Map::new();
    m.insert("chain_id".into(), Dynamic::from(intent.chain_id as i64));
    m.insert(
        "amount_in".into(),
        Dynamic::from(intent.amount_in.to_string()),
    );
    if let Some(leg) = intent.legs.first() {
        m.insert(
            "token_in".into(),
            Dynamic::from(format!("{:#x}", leg.token_in)),
        );
        m.insert(
            "token_out".into(),
            Dynamic::from(format!("{:#x}", leg.token_out)),
        );
        m.insert(
            "protocol_type".into(),
            Dynamic::from(protocol_type_str(leg.protocol_type).to_string()),
        );
        insert_cartridge_fee_fields(&mut m, leg);
        let pool = leg
            .pool_hint
            .map(|p| format!("{:#x}", p))
            .unwrap_or_default();
        m.insert("source_pool".into(), Dynamic::from(pool));
    }

    // CartridgeContextV3: pass the FULL route (all legs) so multi-leg cartridges
    // (triangular, N-leg, cross-pool, cross-DEX) can see the complete cycle — not
    // just the first pool. Each leg: { pool, token_in, token_out, protocol_type,
    // fee_bps }. The cartridge calls get_reserves(pool) per leg to compose
    // executable quotes Q_R(x) = depth-aware detection. Read-only — no signer,
    // no broadcast, pure math.
    let mut route_arr: Vec<Dynamic> = Vec::with_capacity(intent.legs.len());
    for leg in &intent.legs {
        let mut lm = rhai::Map::new();
        let pool = leg
            .pool_hint
            .map(|p| format!("{:#x}", p))
            .unwrap_or_default();
        lm.insert("pool".into(), Dynamic::from(pool));
        lm.insert(
            "token_in".into(),
            Dynamic::from(format!("{:#x}", leg.token_in)),
        );
        lm.insert(
            "token_out".into(),
            Dynamic::from(format!("{:#x}", leg.token_out)),
        );
        lm.insert(
            "protocol_type".into(),
            Dynamic::from(protocol_type_str(leg.protocol_type).to_string()),
        );
        insert_cartridge_fee_fields(&mut lm, leg);
        route_arr.push(Dynamic::from(lm));
    }
    m.insert("route".into(), Dynamic::from(route_arr));
    m.insert(
        "route_legs_count".into(),
        Dynamic::from(intent.legs.len() as i64),
    );
    let route_closed = intent.legs.len() >= 2
        && intent.legs.last().map(|l| l.token_out) == intent.legs.first().map(|l| l.token_in);
    m.insert("route_closed".into(), Dynamic::from(route_closed));

    // WIRE-1 (PR-ROUTE-03): inject the route-shape dispatch key the
    // `omega_strategy_pack.rhai` dispatch table reads at `pool_data["strategy_kind"]`
    // (line 107). Without it, the pack rejects every intent with
    // `missing_strategy_kind` (line 103-104) and its `multi_hop_cycle` /
    // `flashloan_atomic` / `flashmint_atomic` arms stay structurally dead. The
    // classifier is the SAME pure function already used for shadow telemetry
    // (build_shadow_outcome, line 562) — re-derived here so the cartridge map is
    // self-describing. Fail-honest: an unclassifiable shape yields the empty
    // string, which the pack rejects with `unsupported_strategy_kind` (R8).
    let applicability =
        crate::route_discovery::strategy_applicability::classify_route_legs(&intent.legs);
    m.insert(
        "strategy_kind".into(),
        Dynamic::from(applicability.strategy_kind.clone()),
    );

    // Triangular contract (D-01/F3 follow-up): the triangular_arb cartridge reads
    // `pool_data.token_a/b/c` — the cycle's three vertices. For a closed 3-leg
    // cycle A→B→C→A those are the legs' token_ins. Rhai resolves a MISSING map
    // field to `()`, which cascades into a misleading
    // `Function not found: get_token_meta (())` (observed live 2026-08-17 after
    // dispatch enabled) — so populate the vertices whenever the shape is a
    // triangle. Extra legs beyond the third are visible via `route[]`.
    if route_closed && intent.legs.len() >= 3 {
        m.insert(
            "token_a".into(),
            Dynamic::from(format!("{:#x}", intent.legs[0].token_in)),
        );
        m.insert(
            "token_b".into(),
            Dynamic::from(format!("{:#x}", intent.legs[1].token_in)),
        );
        m.insert(
            "token_c".into(),
            Dynamic::from(format!("{:#x}", intent.legs[2].token_in)),
        );
    }

    if let Some(rs) = reserves_source {
        let mut rmap = rhai::Map::new();
        rmap.insert("r0".into(), Dynamic::from(rs.r0.clone()));
        rmap.insert("r1".into(), Dynamic::from(rs.r1.clone()));
        rmap.insert("block".into(), Dynamic::from(rs.blk as i64));
        rmap.insert("ts".into(), Dynamic::from(rs.ts as i64));
        if let Some(t0) = &rs.token0_addr {
            rmap.insert("token0_addr".into(), Dynamic::from(t0.clone()));
        }
        m.insert("reserves_source".into(), Dynamic::from(rmap));
    }
    m
}

// ── FASE B — Route-discovery outcome emitter (gated off by default, NO-ACTIVE) ─
//
// Persists each resolved `CartridgeEvalResult` from `shadow_evaluate_intent` AND
// `active_evaluate_and_emit` to a NEW, SEPARATE Redis stream so the ≥2-week
// hit-rate dataset can accrue. This is the dry-run→stream link the paper-trade
// archiver header documents. Telemetry is mode-invariant (§34.1): both call
// sites use this same emitter — only the downstream terminus differs by mode.
// (BUG-003/RD-06/ARBX-0024: the Active call-site was missing until 2026-08-31.)
//   - Stream: `arbx:route_discovery:outcomes` (NEVER `arbx:opps:detected`).
//   - Gate: `ARBX_ROUTE_DISCOVERY_OUTCOMES` ∈ {shadow,on,1,true}; default off.
//   - Zero-Mocks: only emits with a REAL eval result in hand; nothing fabricated.
//   - Fail-closed: any Redis error is logged (warn!) and swallowed.

/// Independent gate for the outcomes emitter. Default off (NO-ACTIVE).
fn outcomes_emission_enabled() -> bool {
    matches!(
        std::env::var("ARBX_ROUTE_DISCOVERY_OUTCOMES")
            .unwrap_or_default()
            .to_ascii_lowercase()
            .as_str(),
        "shadow" | "on" | "1" | "true"
    )
}

/// Outcomes stream — SEPARATE from `arbx:opps:detected` (which is never touched).
const ROUTE_DISCOVERY_OUTCOMES_STREAM: &str = "arbx:route_discovery:outcomes";
/// Outcomes stream cap (`XADD ... MAXLEN ~`). STREAM-MAXLEN-01 (2026-10-02,
/// incidente medido en producción): el tope anterior de 1.000.000 llenó Redis
/// (maxmemory 1G, policy noeviction) y rechazó TODA escritura — heartbeat,
/// quote anchor, tick y el propio stream quedaron congelados; los endpoints
/// del DApp devolvían 404/503 fail-honest durante horas. Cada entrada pesa
/// ~800B: 1M ≈ 800MB. El histórico vive en PG (route_discovery_outcomes,
/// retención diaria); Redis es SOLO el bus caliente. 150.000 entradas ≈
/// ~120-150MB — cabe con holgura junto al resto del keyspace (~75MB) en 1G.
const OUTCOMES_STREAM_MAXLEN: u64 = 150_000;

/// Persist a resolved shadow eval outcome (fire-and-forget, fail-closed).
async fn emit_shadow_outcome(
    runner: &Arc<CartridgeRunner>,
    chain_id: u64,
    cartridge_id: &str,
    intent: &RouteIntent,
    res: &CartridgeEvalResult,
    had_reserves: bool,
    // SHADOW-CANONICAL-01: sustrato de datos de ESTA evaluación. Se publica para
    // que una fila jamás pueda leerse como "real" sin serlo.
    substrate: ShadowSubstrate,
) {
    if !outcomes_emission_enabled() {
        return; // gate off → nothing emitted
    }

    let ts_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0);

    // Schema selection: v2 (topology/route[]/waterfall) behind its own opt-in flag,
    // v1 (flat single-leg) otherwise. Both are pure builders (unit-tested). The v2
    // schema is additive and default-off — it never alters v1 behaviour or touches
    // `arbx:opps:detected`.
    let payload = if outcomes_v2_schema_enabled() {
        build_rd_outcome_v2(
            chain_id,
            cartridge_id,
            intent,
            res,
            had_reserves,
            substrate,
            ts_ms,
        )
    } else {
        build_rd_outcome_v1(
            chain_id,
            cartridge_id,
            intent,
            res,
            had_reserves,
            substrate,
            ts_ms,
        )
    };

    let json = match serde_json::to_string(&payload) {
        Ok(s) => s,
        Err(e) => {
            warn!(event = "route_discovery.outcome_serialize_failed", error = %e);
            return;
        }
    };

    match runner
        .xadd_shadow_outcome(
            ROUTE_DISCOVERY_OUTCOMES_STREAM,
            OUTCOMES_STREAM_MAXLEN,
            &json,
        )
        .await
    {
        Ok(id) => debug!(
            event = "route_discovery.outcome_emitted",
            chain_id,
            cartridge_id = %cartridge_id,
            stream_id = %id,
            is_opportunity = res.is_opportunity,
        ),
        Err(e) => warn!(
            event = "route_discovery.outcome_emit_failed",
            chain_id,
            error = %e,
        ),
    }
}

/// Opt-in for the richer `rd_outcome_v2` schema (topology / route[] / waterfall /
/// simulation / ethics / live_gate). Default off → v1 stays the wire format until an
/// operator turns this on. Independent of the emission gate, so v2 can be staged
/// without changing what is emitted.
fn outcomes_v2_schema_enabled() -> bool {
    matches!(
        std::env::var("ARBX_ROUTE_DISCOVERY_OUTCOMES_V2_SCHEMA")
            .unwrap_or_default()
            .to_ascii_lowercase()
            .as_str(),
        "shadow" | "on" | "1" | "true"
    )
}

/// route_family label from hop count (the closed-cycle taxonomy).
fn route_family(hop_count: usize) -> &'static str {
    match hop_count {
        0 | 1 => "single_leg",
        2 => "spatial_or_pair",
        3 => "triangular",
        4 => "quadrangular",
        5 => "deep_solver",
        6 => "long_tail",
        _ => "supreme_graph",
    }
}

/// Best-effort topology environment. The discovery worker is single-chain, so a route
/// is never interchain here; we distinguish intradex vs interdex by distinct dex hints
/// (R8: only what the decoder actually extracted — no fabrication).
fn topology_environment(intent: &RouteIntent) -> &'static str {
    let mut dexes = std::collections::HashSet::new();
    for leg in &intent.legs {
        if let Some(d) = leg.dex_hint.as_ref() {
            dexes.insert(d.as_str());
        }
    }
    if dexes.len() > 1 {
        "interdex_intrachain"
    } else {
        "intradex"
    }
}

/// Map a leg's protocol type to its AMM invariant family (fail-honest: unknown stays
/// "unknown" rather than guessing). Matches on the Debug form so it survives new
/// `ProtocolType` variants without a compile break.
fn invariant_of(protocol_debug: &str) -> &'static str {
    let p = protocol_debug.to_ascii_lowercase();
    if p.contains("v3") || p.contains("concentrated") {
        "concentrated_liquidity"
    } else if p.contains("curve") || p.contains("stable") {
        "stableswap"
    } else if p.contains("balancer") || p.contains("weighted") {
        "weighted"
    } else if p.contains("v2") || p.contains("constantproduct") || p.contains("constant_product") {
        "constant_product"
    } else {
        "unknown"
    }
}

/// Pure builder for the legacy `rd_outcome_v1` payload (flat single-leg).
///
/// REPAIRS-OBSERVABILITY-01 (2026-10-01, enmienda): extrae el diagnóstico v4
/// hacia el outcome durable. Contrato del PRODUCTOR (rhai_agent_bridge.rs
/// `repairs_push` L178): cada repair es `{"field": ..., "reason": ...}` — se
/// conservan AMBOS: distinguir un fallo de identidad de plan de un operador
/// sin resultado exige la causa, no solo el componente. El `reason` general
/// usa el campo preservado por el parser (`CartridgeEvalResult.reason`, que
/// el parser EXCLUYE del proposal sellado) y cae al del proposal solo si el
/// parser no trajo ninguno. Sin esto, la razón dominante
/// (`applicable_data_or_constraint_gap`, 69% medido) no permite saber CUÁL
/// reparación bloquea. Sólo observabilidad: sin cambios en reglas ni
/// umbrales; los números son los propios del proposal (R8).
fn v4_repairs_summary(res: &CartridgeEvalResult) -> serde_json::Value {
    let Some(p) = res.metadata.get("proposal_v4") else {
        return serde_json::Value::Null;
    };
    let repairs: Vec<&serde_json::Value> = p
        .get("repairs")
        .and_then(|r| r.as_array())
        .map(|arr| arr.iter().collect())
        .unwrap_or_default();
    serde_json::json!({
        "status": p.get("status").cloned().unwrap_or(serde_json::Value::Null),
        "reason": res
            .reason
            .clone()
            .or_else(|| {
                p.get("reason")
                    .and_then(|r| r.as_str())
                    .map(std::string::ToString::to_string)
            })
            .map(serde_json::Value::String)
            .unwrap_or(serde_json::Value::Null),
        "repairs": repairs,
    })
}

fn build_rd_outcome_v1(
    chain_id: u64,
    cartridge_id: &str,
    intent: &RouteIntent,
    res: &CartridgeEvalResult,
    had_reserves: bool,
    substrate: ShadowSubstrate,
    ts_ms: u64,
) -> serde_json::Value {
    let first = intent.legs.first();
    let last = intent.legs.last();
    let pool_hint = first
        .and_then(|l| l.pool_hint)
        .map(|p| format!("{:#x}", p))
        .unwrap_or_default();

    serde_json::json!({
        "schema": "rd_outcome_v1",
        "ts_ms": ts_ms,
        "chain_id": chain_id,
        "cartridge_id": cartridge_id,
        "tx_hash": format!("{:#x}", intent.tx_hash),
        "source_event": intent.source_event.as_str(),
        "pool_hint": pool_hint,
        "token_in": first.map(|l| format!("{:#x}", l.token_in)).unwrap_or_default(),
        "token_out": last.map(|l| format!("{:#x}", l.token_out)).unwrap_or_default(),
        "is_opportunity": res.is_opportunity,
        "estimated_profit": res.estimated_profit,
        "confidence": res.confidence,
        "urgency": res.urgency.clone(),
        "reason": res.reason.clone(),
        // REPAIRS-OBSERVABILITY-01: diagnóstico v4 (status/reason/nombres de
        // repairs) desde el proposal sellado. Null si no hay proposal_v4.
        "v4_repairs": v4_repairs_summary(res),
        "had_reserves": had_reserves,
        "mode": "shadow",
        // SHADOW-CANONICAL-01 — sustrato REAL de la evaluación: `intent_bundle`
        // (SnapshotBundle real del intent) o `static_boot_stub` (contexto
        // DATA_GAP estático). Campo ADITIVO: el sink de api-server valida solo
        // sus campos requeridos y lo ignora.
        "context_provenance": substrate.as_str(),
    })
}

/// Pure builder for the enriched `rd_outcome_v2` payload.
///
/// Adds topology + full route[] legs + a net-profit waterfall + simulation / ethics /
/// live_gate objects. FAIL-HONEST: the discovery layer does NOT compute itemized costs
/// or run a fork simulation, so every cost field that was not computed is emitted as
/// `null` (never a fabricated number — R8), `simulation.status = "disabled"`, and
/// `live_gate.eligible = false`. The only profit figure emitted is the cartridge's own
/// `estimated_profit` (clearly named), with `net_computed = false`.
fn build_rd_outcome_v2(
    chain_id: u64,
    cartridge_id: &str,
    intent: &RouteIntent,
    res: &CartridgeEvalResult,
    had_reserves: bool,
    substrate: ShadowSubstrate,
    ts_ms: u64,
) -> serde_json::Value {
    let hop_count = intent.legs.len();

    // Classify the route SHAPE into an omega_strategy_pack dispatch family — a pure
    // function of the legs we already decoded (no pricing, no RPC, no sim). This is
    // OBSERVABLE telemetry only: it is NOT used to dispatch a cartridge here (live
    // dispatch wiring is gated separately and out of scope for shadow discovery).
    // `strategy_family_supported` flags whether the committed pack could service the
    // family, so a non-dispatchable (e.g. cross-chain) family is surfaced honestly
    // rather than silently rejected downstream. Fail-honest: an unclassifiable shape
    // ⇒ `strategy_family = null` with the reason carried in `strategy_family_reason`.
    let applicability =
        crate::route_discovery::strategy_applicability::classify_route_legs(&intent.legs);
    let strategy_family_supported =
        crate::route_discovery::strategy_applicability::is_pack_supported(
            &applicability.strategy_kind,
        );
    let strategy_family = if applicability.applicable {
        serde_json::Value::String(applicability.strategy_kind.clone())
    } else {
        serde_json::Value::Null
    };
    let strategy_family_reason = applicability.reason.clone();

    let route: Vec<serde_json::Value> = intent
        .legs
        .iter()
        .enumerate()
        .map(|(i, leg)| {
            let proto = format!("{:?}", leg.protocol_type);
            serde_json::json!({
                "leg": i + 1,
                "token_in": format!("{:#x}", leg.token_in),
                "token_out": format!("{:#x}", leg.token_out),
                "pool": leg.pool_hint.map(|p| format!("{:#x}", p)),
                "dex": leg.dex_hint.clone(),
                "fee_bps": leg.fee_bps,
                "invariant": invariant_of(&proto),
                "protocol_type": proto,
                "chain_id": chain_id,
            })
        })
        .collect();

    serde_json::json!({
        "schema": "rd_outcome_v2",
        "snapshot_id": format!("{}:{:#x}:{}", chain_id, intent.tx_hash, ts_ms),
        "ts_ms": ts_ms,
        "chain_id": chain_id,
        "strategy_kind": cartridge_id,
        "cartridge_id": cartridge_id,
        "mode": "shadow",
        // SHADOW-CANONICAL-01 — ver build_rd_outcome_v1: sustrato real usado.
        "context_provenance": substrate.as_str(),
        "is_opportunity": res.is_opportunity,
        "status": if res.is_opportunity { "shadow_visible" } else { "rejected_with_reason" },
        "topology": {
            "environment": topology_environment(intent),
            "hop_count": hop_count,
            "route_family": route_family(hop_count),
            // Classified dispatch family (route shape → omega_strategy_pack key).
            // Observational telemetry; null when the shape is unclassifiable (R8).
            "strategy_family": strategy_family,
            "strategy_family_supported": strategy_family_supported,
            "strategy_family_reason": strategy_family_reason,
        },
        "route": route,
        "source_event": intent.source_event.as_str(),
        // ── Net-profit waterfall — FAIL-HONEST (R8) ──
        // Only the cartridge's own estimate is known at the discovery layer; the
        // itemized costs and the simulated net are NOT computed here → null, not 0.
        // RC-2: estimated_profit is token-units, not USD (types.rs:45). Only emit a USD
        // figure when the cartridge self-priced (profit_usd_hint); else null (R8),
        // consistent with the null waterfall below (net_computed=false).
        "estimated_profit_usd": res.metadata.get("profit_usd_hint").and_then(|v| v.as_f64()).filter(|p| *p > 0.0),
        "gross_profit_usd": serde_json::Value::Null,
        "gas_cost_usd": serde_json::Value::Null,
        "dex_fees_usd": serde_json::Value::Null,
        "bridge_fees_usd": serde_json::Value::Null,
        "flashloan_fee_usd": serde_json::Value::Null,
        "slippage_cost_usd": serde_json::Value::Null,
        "latency_decay_usd": serde_json::Value::Null,
        "risk_penalty_usd": serde_json::Value::Null,
        "net_profit_usd": serde_json::Value::Null,
        "net_computed": false,
        "roi_pct": serde_json::Value::Null,
        "confidence": res.confidence,
        "risk_score": serde_json::Value::Null,
        "priority_score": serde_json::Value::Null,
        "urgency": res.urgency.clone(),
        "reason": res.reason.clone(),
        // REPAIRS-OBSERVABILITY-01: mismo diagnóstico v4 que v1.
        "v4_repairs": v4_repairs_summary(res),
        "had_reserves": had_reserves,
        "simulation": {
            "status": "disabled",
            "note": "shadow discovery eval; REVM fork simulation not run at this layer",
            "fork_block": serde_json::Value::Null,
            "revert_reason": serde_json::Value::Null,
        },
        "ethics": {
            "status": "permitted",
            "gate": "arbx-mev-ethics-gate",
            "notes": ["shadow_only", "no_sandwich", "no_frontrun", "post_state_or_confirmed_only"],
        },
        "live_gate": {
            "eligible": false,
            "reason": "shadow_only_no_simulation_no_live",
        },
    })
}

/// Smart intent routing — returns `true` iff a cartridge of the given `category` can
/// MEANINGFULLY evaluate this intent's shape. Without this filter, every active cartridge
/// is fed every intent, so a single-leg V2 swap (the block scanner's output) is handed to
/// `liquidation` (which reads `pool_data.debt_token` / `collateral_token`) and
/// `triangular_arb` (which reads `pool_data.token_a` / `token_b` / `token_c`). Those keys
/// are absent on a swap, so the cartridge calls `get_token_meta(())` → "Function not found"
/// and floods `cartridge.shadow_eval_error` while wasting Rhai cycles.
///
/// Pertinence keys on the intent's SHAPE — `source_event` for the observation
/// vs position origin, plus the route's closed-cycle geometry:
///   - swap observations (`public_mempool`, `filtered_mempool`, `private_hint`,
///     `new_block`) carry one observed swap leg → `dex_arb` (cross-DEX spread on
///     the observed pair), EXCLUDING closed ≥3-leg cycles.
///   - lending / oracle events (`lending_position_update`, `oracle_update`) carry a
///     debt/collateral position → `liquidation`.
///   - `triangular_arb` matches closed ≥3-leg cycles: route_discovery's 3-hop
///     closed routes are the triangle source (D-01, closed 2026-08-16).
///   - Unknown / custom categories are always evaluated: the operator who installed the
///     cartridge owns its input-shape contract, so we never silently drop it.
fn cartridge_matches_intent(category: &str, intent: &RouteIntent) -> bool {
    let is_swap_observation = matches!(
        intent.source_event,
        DetectionSource::PublicMempool
            | DetectionSource::FilteredMempool
            | DetectionSource::PrivateHint
            | DetectionSource::NewBlock
    );
    let is_position_event = matches!(
        intent.source_event,
        DetectionSource::LendingPositionUpdate | DetectionSource::OracleUpdate
    );
    // A closed ≥3-leg cycle (last leg's token_out = first leg's token_in) is a
    // TRIANGLE shape: route_discovery's Triangular classification (D-01).
    let is_closed_cycle = intent.legs.len() >= 3
        && intent.legs.last().map(|l| l.token_out) == intent.legs.first().map(|l| l.token_in);
    match category {
        // Consumes one observed swap leg (token_in/out + reserves_source) — never
        // a closed ≥3-leg cycle (a triangle is not a single-pair spread).
        "dex_arb" => is_swap_observation && !is_closed_cycle,
        // Needs a debt/collateral position; only lending/oracle events carry it.
        "liquidation" => is_position_event,
        // Closed ≥3-leg cycles from route_discovery's triangle source (D-01).
        // Swap-observation sources only: a closed lending/oracle position shape
        // is not a triangle even if it geometrically closes (directiva F1-d).
        "triangular_arb" => is_swap_observation && is_closed_cycle,
        // Custom / unknown cartridge: don't gate it — evaluate against everything.
        _ => true,
    }
}

/// Closed-cycle geometry from the built RoutePlan legs — same definition as the
/// `is_closed_cycle` check in [`cartridge_matches_intent`] (last leg's
/// token_out == first leg's token_in, ≥3 legs), computed from the canonical hex
/// strings the plan carries. Decides whether the G03/G04 state-event families
/// can evaluate as a closed-cycle triangle at the label-mapping site.
fn route_plan_is_closed_cycle(legs: &[prioritization_spine::route_plan::RouteLeg]) -> bool {
    legs.len() >= 3
        && legs.last().map(|l| l.token_out.as_str()) == legs.first().map(|l| l.token_in.as_str())
}

/// Maps a cartridge category to its canonical [`StrategyLabel`] (Task 3,
/// `docs/superpowers/plans/2026-08-17-cartridge-math-264.md`).
///
/// The 11 Excel families of the 264-cartridge catalog map onto EXISTING labels;
/// C.5 is preserved — the granular cartridge identity lives in
/// `RoutePlan.strategy_kind` (`cartridge_{category}`), never collapsed here:
///
///   - `cross_domain_engine` → `CrossChainArb` (cross-domain spread)
///   - `credit_liquidation_engine` → `LiquidationSnipe` (health-factor sniping)
///   - `route_graph_engine` | `amm_curve_engine` → `DexArbV2V2` (G01/G02
///     spot-DEX; the granular variant is preserved in `RoutePlan.strategy_kind`)
///   - `state_event_engine` | `parity_redemption_engine` → `TriangularArb`
///     ONLY when the route shape is a closed ≥3-leg cycle (G03/G04 state-event
///     evaluation is measurable today as a triangle; the dedicated profile
///     arrives in RU-4). Non-closed → `Err(reason)`, fail-honest.
///
/// Everything else — `derivatives_engine`, `cex_external_engine`,
/// `intents_solver_engine`, `nft_engine`, `prediction_engine` and unknown
/// categories — has no honest engine yet and returns `Err(reason)` so the
/// caller rejects with an explicit reason instead of silently mis-gating (R8).
fn category_to_strategy_label(
    category: &str,
    is_closed_cycle: bool,
) -> Result<StrategyLabel, String> {
    match category {
        "dex_arb" | "dex_arb_v2v2" => Ok(StrategyLabel::DexArbV2V2),
        "dex_arb_v2v3" => Ok(StrategyLabel::DexArbV2V3),
        "dex_arb_v3v2" => Ok(StrategyLabel::DexArbV3V2),
        "dex_arb_v3v3" => Ok(StrategyLabel::DexArbV3V3),
        "triangular_arb" => Ok(StrategyLabel::TriangularArb),
        "flashloan_arb" => Ok(StrategyLabel::FlashloanArb),
        "liquidation" => Ok(StrategyLabel::Liquidation),
        "spanning_tree_arb" => Ok(StrategyLabel::SpanningTreeArb),
        "cross_chain_arb" | "cross_domain_engine" => Ok(StrategyLabel::CrossChainArb),
        "liquidation_snipe" | "credit_liquidation_engine" => Ok(StrategyLabel::LiquidationSnipe),
        // G01/G02 spot-DEX families: the granular variant lives in
        // RoutePlan.strategy_kind (C.5 — never collapse variants here).
        "route_graph_engine" | "amm_curve_engine" => Ok(StrategyLabel::DexArbV2V2),
        // G03/G04: state-event / parity-redemption evaluation — measurable today
        // as a closed-cycle triangle; the dedicated profile arrives in RU-4.
        // FIX (2026-08-19): removed `if is_closed_cycle` guard — it was killing
        // 6/10 opportunities with `cartridge_unmapped_strategy_label` before the
        // degenerate-route check could run. Now the cartridge evaluates the route,
        // and degenerate routes (token_in == token_out, empty metadata) get
        // rejected by the proper downstream gates instead of dying at label mapping.
        "state_event_engine" | "parity_redemption_engine" => Ok(StrategyLabel::TriangularArb),
        // No honest engine yet (derivatives, cex_external, intents_solver, nft,
        // prediction, unknown) → explicit reason (R8).
        other => Err(format!("cartridge_unmapped_strategy_label:{other}")),
    }
}

/// Sustrato de datos con el que se resolvió UNA fila del stream shadow.
///
/// SHADOW-CANONICAL-01 — existe para que el stream NUNCA presente como real una
/// evaluación hecha contra el stub Phase-1: el `mode` sigue siendo `"shadow"`
/// (correcto: es el stream shadow), pero `context_provenance` declara si la
/// evaluación usó el `SnapshotBundle` REAL del intent o el contexto DATA_GAP
/// estático. Antes de este cambio TODAS las filas describían el stub sin
/// declararlo, y el `costs.financing :: mandatory_route_cost_missing` de ~100%
/// de las 600 filas medidas era el síntoma.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ShadowSubstrate {
    /// `SnapshotBundle` REAL del intent, registrado en el `ContextRouter`.
    IntentBundle,
    /// Contexto DATA_GAP estático (`boot-chain-{chain_id}`): no se pudo componer
    /// el bundle real. La razón exacta queda en `cartridge.intent_context_unavailable`.
    StaticBootStub,
}

impl ShadowSubstrate {
    fn as_str(self) -> &'static str {
        match self {
            Self::IntentBundle => "intent_bundle",
            Self::StaticBootStub => "static_boot_stub",
        }
    }
}

/// Shadow-evaluates every ACTIVE cartridge against one live route intent and emits
/// the result to logs/telemetry. **Read-only / observe-only**: it never constructs a
/// `StrategyCandidate`, never touches `process_candidate`, and never reaches the
/// execution pipeline. Designed to be `tokio::spawn`-ed off the orchestrator hot
/// path so it adds no latency to intent processing. Per-cartridge errors are logged,
/// never propagated (one bad cartridge cannot affect the others or the scanner).
///
/// SHADOW-CANONICAL-01 (2026-10-03) — §34.1 hot-path mode-invariant: esta ruta
/// compone y registra el MISMO contexto v4 real por intent que la ruta ACTIVE
/// (`build_and_register_intent_context`) y sella sus ids en `pool_data`, de modo
/// que los bindings v4 resuelven el grafo/precios/costes REALES del intent. Los
/// modos difieren SÓLO en el terminus de ejecución: aquí NO hay emisor, NO se
/// persiste oportunidad y NO se emite nada a `arbx:opps:detected` — la única
/// salida sigue siendo `arbx:route_discovery:outcomes` más los logs.
///
/// `deps` es `None` cuando el llamador no dispone de las dependencias de
/// composición (config del operador, registro de operadores, router); la
/// evaluación cae entonces al contexto DATA_GAP estático y lo declara con la
/// razón exacta (`no_context_deps`) en vez de simular datos reales.
///
/// Concurrency is globally bounded by [`SHADOW_MAX_CONCURRENCY`]; at capacity an
/// evaluation is dropped rather than queued (observe-only). NOTE: a cartridge's own
/// `log_quantum`/`emit_signal` telemetry is tagged via the runner-shared
/// `host_ctx.cartridge_id`, so under concurrent shadow tasks that tag is best-effort —
/// the authoritative attribution is the `cartridge.shadow_eval` event below, which
/// always carries the correct `cartridge_id`. A per-call host context is a follow-up.
pub async fn shadow_evaluate_intent(
    runner: Arc<CartridgeRunner>,
    intent: RouteIntent,
    chain_id: u64,
    deps: Option<IntentContextDeps>,
) {
    // Bound global shadow-eval concurrency; drop (don't queue) when at capacity.
    let _permit = match shadow_semaphore().clone().try_acquire_owned() {
        Ok(p) => p,
        Err(_) => {
            debug!(
                event = "cartridge.shadow_eval_skipped",
                chain_id,
                tx_hash = %intent.tx_hash,
                "shadow eval at capacity; dropping (observe-only)"
            );
            return;
        }
    };

    // (id, category) for every Active cartridge — category drives smart routing below.
    let actives: Vec<(String, String)> = runner
        .list_cartridges()
        .await
        .into_iter()
        .filter(|(_, _, state)| *state == CartridgeState::Active)
        .map(|(id, meta, _)| (id, meta.category))
        .collect();
    // Probe (debug-level): fires IFF this task ran; `active_count` reveals whether the
    // orchestrator's shared runner sees the loaded cartridges. Demoted from info! so the
    // block scanner's per-block intent volume does not flood the logs.
    debug!(
        event = "cartridge.shadow_enter",
        chain_id,
        tx_hash = %intent.tx_hash,
        active_count = actives.len(),
        "shadow eval task entered"
    );
    if actives.is_empty() {
        return;
    }

    // Smart intent routing: keep only cartridges pertinent to THIS intent's shape, so a
    // single-leg swap is never handed to `liquidation`/`triangular_arb` (which would just
    // error on the missing position/triangle fields). See `cartridge_matches_intent`.
    let pertinent: Vec<(String, String)> = actives
        .into_iter()
        .filter(|(_, category)| cartridge_matches_intent(category, &intent))
        .collect();
    if pertinent.is_empty() {
        debug!(
            event = "cartridge.shadow_eval_no_pertinent",
            chain_id,
            tx_hash = %intent.tx_hash,
            source_event = %intent.source_event.as_str(),
            "no active cartridge is pertinent to this intent source; skipping (observe-only)"
        );
        return;
    }

    // Enrich pool_data with the source pool's reserves so reserve-dependent cartridges
    // (e.g. dex_arb) can evaluate. Best-effort: None when the pool has no fresh reserves
    // in Redis (R8 fail-honest — the cartridge then returns no opportunity).
    let reserves_source = match intent.legs.first().and_then(|l| l.pool_hint) {
        Some(p) => runner.read_pool_reserves(&format!("{:#x}", p)).await,
        None => None,
    };
    // SHADOW-CANONICAL-01 (2026-10-03) — §34.1 hot-path mode-invariant: la ruta
    // SHADOW evalúa contra el MISMO sustrato de datos que la ruta ACTIVE. Antes
    // construía SOLO `build_cartridge_pool_data` y no registraba contexto, así
    // que los bindings v4 (`agent_v4_quote`/`operators`/`economic_check`/
    // `discover`) resolvían al stub estático de boot (`boot-chain-{chain_id}`:
    // `base_cost_lines`/`prices`/`edges`/`size_schedule_raw` VACÍOS) y toda fila
    // del stream describía ese stub — de ahí el
    // `costs.financing :: mandatory_route_cost_missing` en ~100% de las 600
    // filas medidas. Ahora se compone y registra el contexto REAL del intent y
    // se sellan sus ids en `pool_data`, idéntico a la ruta ACTIVE.
    let v4_context = match deps.as_ref() {
        Some(deps) => {
            build_and_register_intent_context(
                &runner,
                &intent,
                chain_id,
                &deps.cfg_provider,
                &deps.math_registry,
                deps.router.as_ref(),
            )
            .await
        }
        None => {
            // Sin dependencias de composición (config del operador, registro de
            // operadores, router) NO se puede armar el bundle real: la ruta cae
            // al comportamiento Phase-1 y lo declara con la razón exacta.
            intent_context_unavailable(chain_id, &intent, "no_context_deps");
            IntentContextOutcome {
                guard: None,
                census: IntentContextCensus::default(),
            }
        }
    };
    // Sustrato REALMENTE usado por ESTA evaluación: se publica en cada fila del
    // stream para que ninguna fila pueda presentarse como real sin serlo.
    let substrate = if v4_context.guard.is_some() {
        ShadowSubstrate::IntentBundle
    } else {
        ShadowSubstrate::StaticBootStub
    };
    let (v4_stamp_context, v4_stamp_snapshot) = v4_context.stamp_ids(chain_id);
    // Guard con Drop: el contexto del intent se retira del router en TODA
    // salida (return, panic o fin del bucle) — misma garantía que la ruta ACTIVE.
    let _v4_intent_guard = v4_context.guard;
    let mut pool_data = build_cartridge_pool_data(&intent, reserves_source.as_ref());
    pool_data.insert("context_id".into(), rhai::Dynamic::from(v4_stamp_context));
    pool_data.insert("snapshot_id".into(), rhai::Dynamic::from(v4_stamp_snapshot));

    for (id, _category) in pertinent {
        match runner.evaluate(&id, pool_data.clone()).await {
            Ok(res) => {
                // Only POSITIVE detections are logged at info!; negatives are the
                // overwhelming majority under the block scanner and stay at debug!.
                if res.is_opportunity {
                    info!(
                        event = "cartridge.shadow_eval",
                        chain_id,
                        tx_hash = %intent.tx_hash,
                        cartridge_id = %id,
                        is_opportunity = true,
                        estimated_profit = res.estimated_profit,
                        confidence = res.confidence,
                        urgency = %res.urgency,
                        substrate = substrate.as_str(),
                        "cartridge shadow OPPORTUNITY detected (observe-only, no execution)"
                    );
                } else {
                    debug!(
                        event = "cartridge.shadow_eval_negative",
                        chain_id,
                        cartridge_id = %id,
                        substrate = substrate.as_str(),
                        "cartridge shadow eval: no opportunity"
                    );
                }

                // FASE B — persist the resolved outcome to the shadow outcomes
                // stream (gated OFF by default; NO-ACTIVE: writes ONLY
                // arbx:route_discovery:outcomes, never arbx:opps:detected).
                emit_shadow_outcome(
                    &runner,
                    chain_id,
                    &id,
                    &intent,
                    &res,
                    reserves_source.is_some(),
                    substrate,
                )
                .await;
            }
            Err(e) => {
                warn!(
                    event = "cartridge.shadow_eval_error",
                    chain_id,
                    tx_hash = %intent.tx_hash,
                    cartridge_id = %id,
                    error = %e,
                    "cartridge shadow evaluation failed; skipping"
                );
            }
        }
    }
}

/// SHADOW-CANONICAL-01 — dependencias que el camino SHADOW necesita para
/// componer el MISMO contexto por-intent que el camino ACTIVE. Espeja 1:1 lo
/// que `active_evaluate_and_emit` recibe del `OrchestratorContext` (config del
/// operador, registro nativo de operadores y router de contextos); todo lo
/// demás (índice de identidad, snapshot de precios, conexión Redis, gas) lo
/// deriva el helper desde el propio `runner`, igual que la ruta ACTIVE.
///
/// Sin estas dependencias no se puede componer el bundle REAL: el camino
/// shadow conserva entonces su comportamiento Phase-1 (contexto DATA_GAP
/// estático) y lo declara con la razón exacta — jamás simula tener datos.
pub struct IntentContextDeps {
    pub cfg_provider: Arc<crate::orchestrator::ConfigProvider>,
    pub math_registry: Arc<math_engine::OperatorRegistry>,
    pub router: Option<Arc<crate::context_router::ContextRouter>>,
}

/// Fase 3a — retira el contexto por-intent del ContextRouter en TODA salida
/// de la tarea (return temprano, panic o fin normal del loop). Sin este guard,
/// cada intent dejaría una entrada ocupando la capacidad 256 del router hasta
/// reiniciar. Lo comparten la ruta ACTIVE y la ruta SHADOW: SHADOW-CANONICAL-01
/// registra el mismo contexto real, así que necesita la misma retirada.
pub struct IntentContextGuard {
    router: Option<Arc<crate::context_router::ContextRouter>>,
    context_id: String,
}
impl IntentContextGuard {
    /// `context_id` con el que quedó registrado el contexto real del intent.
    /// Es el valor que el camino ACTIVE (y ahora también el SHADOW) sella en
    /// `pool_data` para que los cartuchos v4 lo copien a su propio ctx.
    pub fn context_id(&self) -> &str {
        &self.context_id
    }
}
impl Drop for IntentContextGuard {
    fn drop(&mut self) {
        if let Some(router) = self.router.take() {
            if let Err(e) = router.remove(&self.context_id) {
                debug!(
                    event = "cartridge.v4_intent_context_remove_failed",
                    context_id = %self.context_id,
                    reason = %e,
                    "no se pudo retirar el contexto del intent (capacidad del router puede agotarse)"
                );
            }
        }
    }
}

/// Fase 3a — decimales REALES por dirección desde `arbx:tokens:*`. Es la
/// MISMA fuente Redis que alimenta `crate::token_identity::index_for`
/// (scan_token_universe); `TokenIdentityIndex` no expone decimales (se
/// descartan en `resolve`), así que se lee el origen directo. `None` =
/// meta ausente/ilegible → la pierna se OMITE (R8, sin decimales asumidos).
/// Cacheada por intent para no repetir GETs del mismo token entre piernas.
async fn v4_token_decimals(
    redis: &mut redis::aio::ConnectionManager,
    chain_id: u64,
    addr_lower: &str,
    cache: &mut std::collections::HashMap<String, Option<u8>>,
) -> Option<u8> {
    if let Some(hit) = cache.get(addr_lower) {
        return *hit;
    }
    let decimals = crate::reserves::get_token_meta(redis, chain_id, addr_lower)
        .await
        .ok()
        .flatten()
        .map(|meta| meta.decimals);
    cache.insert(addr_lower.to_owned(), decimals);
    decimals
}

/// Fase 3d — slot0 V3 cacheado para el edge (misma fuente Redis que el
/// binding `get_v3_slot0`: `arbx:v3_slot0:<chain>:<pool>`, TTL del writer).
/// Cacheado por intent; `None` = sin slot0 → edge honesto sin campos V3 y
/// `quote_path` exigirá productor exacto para esa pierna (R8).
async fn v4_slot0(
    redis: &mut redis::aio::ConnectionManager,
    chain_id: u64,
    pool_addr_lower: &str,
    cache: &mut std::collections::HashMap<String, Option<(String, u128, u64)>>,
) -> Option<(String, u128, u64)> {
    if let Some(hit) = cache.get(pool_addr_lower) {
        return hit.clone();
    }
    let slot0 = crate::reserves::get_v3_slot0(redis, chain_id, pool_addr_lower)
        .await
        .ok()
        .flatten()
        .and_then(|entry| {
            let liquidity: u128 = entry.liquidity.parse().ok()?;
            // V3-LEG-GRAPH-01: el ts del round de sync acompaña al slot0 — es
            // la identidad de coherencia del Edge V3 (todas las entradas del
            // mismo round comparten ts, verificado en producción).
            Some((entry.sqrt_price_x96, liquidity, entry.ts))
        });
    cache.insert(pool_addr_lower.to_owned(), slot0.clone());
    slot0
}

// ── OPERATOR-DISPATCH-WIRING-01 (2026-10-02) ─────────────────────────────────
// El bundle del intent construía SnapshotServices SIN dispatcher de operadores:
// `operators()` caía al soporte derivado (evidencia vacía) y los recibos nativos
// quedaban FAIL constante. Esto adjunta el registro nativo REAL cuando el
// intent produjo un MarketState con precios observados.

/// MarketState desde los edges del intent — SOLO piernas V2 (reservas
/// orientadas por token0_addr + decimales ya resueltos). El precio V3 desde
/// slot0 exige la verificación contra referencia de EXACT-CLASS-01 y queda
/// como seguimiento: jamás alimentar a los operadores un precio posiblemente
/// mal orientado. Sin precios → None → no se adjunta dispatch (R8).
///
/// MARKET-FEATURES-WIRE-01 (2026-10-03): `features` entra como PARÁMETRO. Este
/// sitio construía el mapa con `HashMap::new()` — nacía vacío y moría vacío, de
/// modo que todo lector de `features` en la ruta v4 recibía nada. Ante el hueco,
/// varios operadores no repartían DATA_GAP: fabricaban un valor con
/// `unwrap_or(...)` (premium flash a 0.0, `max_capital` a $1.0, fee a 30 bps).
/// La lectura async de los productores vive en el llamador
/// (`build_and_register_intent_context`, que es async); esta función es sync por
/// contrato y sólo recibe el mapa YA producido. R8 fail-honest: una clave
/// ausente significa "no computado" y aquí NO se rellena jamás con un default.
fn v4_market_state_from_edges(
    edges: &[crate::agent_graph::Edge],
    block_number: u64,
    gas_price_gwei: f64,
    features: std::collections::HashMap<String, f64>,
) -> Option<std::sync::Arc<math_engine::MarketState>> {
    use math_engine::MarketState;
    let mut price_matrix: Vec<Vec<f64>> = Vec::new();
    let mut pair_keys: Vec<String> = Vec::new();
    let mut liquidity_reserves: Vec<(f64, f64)> = Vec::new();
    for e in edges {
        if e.protocol != "cpmm_v2" {
            continue;
        }
        let (Some(ri_raw), Some(ro_raw)) = (&e.reserve_in_raw, &e.reserve_out_raw) else {
            continue;
        };
        let (Ok(ri), Ok(ro)) = (
            ethers::types::U256::from_dec_str(ri_raw),
            ethers::types::U256::from_dec_str(ro_raw),
        ) else {
            continue;
        };
        let Some(price) = crate::math_evidence::normalized_price(
            ri,
            ro,
            e.token_in_decimals,
            e.token_out_decimals,
        ) else {
            continue;
        };
        price_matrix.push(vec![price]);
        pair_keys.push(crate::math_evidence::canonical_pair_key(
            &e.token_in,
            &e.token_out,
        ));
        liquidity_reserves.push((
            ri_raw.parse::<f64>().unwrap_or(0.0),
            ro_raw.parse::<f64>().unwrap_or(0.0),
        ));
    }
    if price_matrix.is_empty() {
        return None;
    }
    Some(std::sync::Arc::new(MarketState {
        price_matrix,
        pair_keys,
        liquidity_reserves,
        gas_price_gwei,
        block_timestamp: 0, // no viaja en el intent — honesto (observe-only)
        block_number,
        features,
    }))
}

/// Admission ESTRUCTURAL de los inputs compartidos del MarketState. Alcance
/// declarado: valida que el estado que consumen los operadores tiene precios,
/// claves de par alineadas, gas finito positivo y bloque conocido — y emite un
/// recibo con la identidad del plan. NO es la admisión por-operador con
/// lineage completo (varios operadores viejos no la satisfacen — skill
/// arbx-rhai-cartridge-v4); esa es seguimiento. Un operador que consuma inputs
/// no cubiertos por esta validación estructural reparte su propio DATA_GAP.
struct V4StructuralInputAdmission;
impl crate::native_operator_adapter::OperatorInputAdmission for V4StructuralInputAdmission {
    fn validate(
        &self,
        _id: u8,
        state: &math_engine::MarketState,
        snapshot_id: &str,
        plan_hash: &str,
    ) -> Result<String, String> {
        if state.price_matrix.is_empty() {
            return Err("market_state_price_matrix_empty".into());
        }
        if state.pair_keys.len() != state.price_matrix.len() {
            return Err("market_state_pair_keys_misaligned".into());
        }
        if !state.gas_price_gwei.is_finite() || state.gas_price_gwei <= 0.0 {
            return Err("market_state_gas_not_positive".into());
        }
        if state.block_number == 0 {
            return Err("market_state_block_unknown".into());
        }
        Ok(format!(
            "structural:{snapshot_id}:{plan_hash}:{}px",
            state.price_matrix.len()
        ))
    }
}

// ── AGENT v4 Fase 3b — admisión EXPLÍCITA de manifiestos ─────────────────────
// El backend ADMITE el par (mev_id, source_digest) que cada script v4-sellado
// desplegado declara en su PROPIO agent_manifest() (escaneo de texto del
// archivo — sin compilar Rhai ni depender de registros externos). Sólo
// participan scripts con `arbx.cartridge.agent/4`; los v3 no aportan
// admisiones. R8: un manifiesto ausente, ambiguo o malformado NO se admite —
// el cartucho recibirá `manifest_not_admitted_by_backend`, la razón honesta.

/// Extrae todos los valores `"clave": "valor"` (estilo JSON) de un texto.
/// Escaneo literal delimitado por comillas — sin dependencia regex.
fn v4_extract_quoted_values<'a>(source: &'a str, key: &str) -> Vec<&'a str> {
    let needle = format!("\"{key}\"");
    let mut out = Vec::new();
    let mut rest = source;
    while let Some(pos) = rest.find(&needle) {
        let Some(after_colon) = rest[pos + needle.len()..].trim_start().strip_prefix(':') else {
            break;
        };
        let Some(value) = after_colon.trim_start().strip_prefix('"') else {
            break;
        };
        let Some(end) = value.find('"') else {
            break;
        };
        out.push(&value[..end]);
        rest = &value[end + 1..];
    }
    out
}

/// Admite UN script: devuelve el par (mev_id, source_digest) auto-declarado.
/// Exige contrato v4, valores únicos y consistentes dentro del archivo,
/// mev_id `[A-Z0-9-]` y digest de 64 hex (SHA-256).
fn v4_admit_script(source: &str) -> Option<(String, String)> {
    if !source.contains("arbx.cartridge.agent/4") {
        return None;
    }
    let mevs = v4_extract_quoted_values(source, "mev_id");
    let digests = v4_extract_quoted_values(source, "source_digest");
    let mev = *mevs.first()?;
    if mevs.iter().any(|m| *m != mev)
        || mev.is_empty()
        || !mev
            .bytes()
            .all(|b| b.is_ascii_uppercase() || b.is_ascii_digit() || b == b'-')
    {
        return None;
    }
    let digest = *digests.first()?;
    if digests.iter().any(|d| *d != digest)
        || digest.len() != 64
        || !digest.bytes().all(|b| b.is_ascii_hexdigit())
    {
        return None;
    }
    Some((mev.to_owned(), digest.to_owned()))
}

/// Escaneo determinista (orden por nombre de archivo) de los scripts v4
/// desplegados en `<dir>/strategies/*.rhai`. Un `mev_id` repetido entre
/// archivos conserva el PRIMERO (orden alfabético) y lo registra — el
/// despliegue real es 1:1 por mev_id, el conflicto es anomalía observable.
fn v4_scan_manifest_digests(dir: &std::path::Path) -> std::collections::BTreeMap<String, String> {
    let strategies = dir.join("strategies");
    let mut out = std::collections::BTreeMap::new();
    let Ok(entries) = std::fs::read_dir(&strategies) else {
        info!(
            event = "cartridge.v4_manifests_dir_missing",
            dir = %strategies.display(),
            "sin directorio strategies desplegado; cero admisiones v4 (R8)"
        );
        return out;
    };
    let mut paths: Vec<std::path::PathBuf> = entries
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().and_then(|e| e.to_str()) == Some("rhai"))
        .collect();
    paths.sort();
    let mut scanned = 0usize;
    let mut skipped = 0usize;
    let mut conflicts = 0usize;
    for path in paths {
        let Ok(source) = std::fs::read_to_string(&path) else {
            skipped += 1;
            continue;
        };
        scanned += 1;
        match v4_admit_script(&source) {
            Some((mev, digest)) => {
                if out.insert(mev.clone(), digest).is_some() {
                    conflicts += 1;
                    warn!(
                        event = "cartridge.v4_manifest_conflict",
                        file = %path.display(),
                        mev_id = %mev,
                        "mev_id duplicado entre scripts desplegados; se conserva el primero (orden alfabético)"
                    );
                }
            }
            None => {
                skipped += 1;
            }
        }
    }
    info!(
        event = "cartridge.v4_manifests_admitted",
        admitted = out.len(),
        scanned,
        skipped,
        conflicts,
        dir = %strategies.display(),
        "admisión explícita de manifiestos v4 (auto-declarados por el script desplegado)"
    );
    out
}

/// HOPS-CARD-05 — the cartridge-side DFS hop ceiling, sourced from the canonical
/// workbook knob (`Max_Hops`, default 7) instead of the previous hard-coded 4.
///
/// Why it matters: `SnapshotBundle::limits.max_hops` bounds the agent graph the
/// cartridge evaluates, so a hard-coded 4 made every 5-7 hop route intent
/// unrepresentable inside the runtime — the operator's 2..7 mandate stopped at
/// the cartridge boundary even though discovery, sizing
/// (`size_triangular_with_reason` is generic in N) and the per-hop ledger all
/// support 7. `agent_graph::SearchLimits` requires 2..=7, so the knob is clamped
/// to that same closed interval (a malformed env value can never admit an
/// unbounded DFS).
pub fn canonical_max_hops() -> u8 {
    crate::canonical_knobs::CanonicalKnobs::from_env()
        .max_hops
        .clamp(2, 7)
}

/// Mapa (mev_id → source_digest) de la librería desplegada, calculado UNA vez
/// por proceso. Alcance Fase 3b: los scripts DEPLOYADOS en `CARTRIDGE_DIR`;
/// un cartucho inyectado en caliente con digest nuevo NO estará en este mapa
/// y fallará la admisión con razón explícita hasta reinicio (la admisión
/// hot-reload es una fase posterior, documentada en el PR).
pub fn v4_manifest_digests() -> &'static std::collections::BTreeMap<String, String> {
    static V4_MANIFESTS: OnceLock<std::collections::BTreeMap<String, String>> = OnceLock::new();
    V4_MANIFESTS.get_or_init(|| v4_scan_manifest_digests(std::path::Path::new(CARTRIDGE_DIR)))
}

/// Fase 3c/3d — mapea la pierna del intent a (protocolo, fee) del Edge v4.
///
/// - Familia V2 (`ProtocolType::V2`) → `"cpmm_v2"`: `quote_path` computa el
///   quote LOCAL con `cpmm_exact_in` (sin productor externo). La fee viene del
///   PROPIO intent (`fee_bps`, basis points del leg) → units = bps,
///   denominator = 10_000. Sin fee declarada → `None`: el quote fallará con
///   `missing_fee_units` — honesto, igual que hoy (R8: sin fee inferida).
/// - `V3` → `"uniswap_v3"` con la fee PIPS del intent (units = pips,
///   denominator = 1_000_000 — el formato exacto del pool): la usa la rama
///   within-tick de `quote_path` cuando el edge porta slot0 cacheado; sin
///   slot0 el quote exige productor exacto (sin fallback constant-product —
///   doctrina del contrato).
/// - `Curve`/`Balancer`/`Unknown` → protocolo nominal y fee `None`: quote de
///   productor exacto.
fn v4_edge_protocol_and_fee(
    leg: &crate::route_intent::RouteIntentLeg,
) -> (String, Option<u32>, Option<u32>) {
    let fee_bps = leg.fee_bps.map(|bps| (bps, 10_000u32));
    match leg.protocol_type {
        ProtocolType::V2 => match fee_bps {
            Some((units, den)) => ("cpmm_v2".to_string(), Some(units), Some(den)),
            None => ("cpmm_v2".to_string(), None, None),
        },
        ProtocolType::V3 => match leg.fee_bps {
            Some(pips) => ("uniswap_v3".to_string(), Some(pips), Some(1_000_000)),
            None => ("uniswap_v3".to_string(), None, None),
        },
        ProtocolType::Curve => ("curve".to_string(), None, None),
        ProtocolType::Balancer => ("balancer".to_string(), None, None),
        ProtocolType::Unknown => ("unknown".to_string(), None, None),
    }
}

// ── SHADOW-CANONICAL-01 (2026-10-03) — composición PURA del grafo por-intent ──
// El grafo v4 del intent se compone en dos mitades separadas a propósito:
//   · la LECTURA (async): reservas/slot0/decimales desde Redis, con las mismas
//     claves y el mismo orden que ya usaba la ruta ACTIVE;
//   · la COMPOSICIÓN (pura): decidir qué piernas entran, con qué orientación y
//     con qué razón exacta se omiten las demás.
// La frontera hace la composición testeable SIN Redis y, sobre todo, garantiza
// que las rutas ACTIVE y SHADOW producen el MISMO grafo: no hay dos
// implementaciones que puedan divergir, hay una sola función pura invocada por
// ambos caminos (§34.1 hot-path mode-invariant).

/// Cuerpo ya resuelto de una pierna V2 o V3. Identidad de coherencia incluida:
/// el `sync_ts` del ROUND (todas las entradas de un round comparten `ts`), que
/// es la única frontera honesta para rutas mixtas V2/V3 — jamás un hash
/// fabricado (R8).
pub(crate) enum IntentLegBody {
    V2 {
        reserve_in_raw: String,
        reserve_out_raw: String,
        sync_ts: u64,
    },
    V3 {
        sqrt_price_x96: String,
        liquidity: u128,
        sync_ts: u64,
    },
}

/// Lectura resuelta de UNA pierna del intent. `Skip(reason)` lleva la razón
/// EXACTA de omisión (misma taxonomía que el histograma de producción:
/// `missing_pool_hint`, `degenerate_self_pair`, `reserves_missing`,
/// `v3_slot0_missing`, `token0_addr_missing`, `token0_addr_out_of_route`,
/// `token_in_decimals_missing`, `token_out_decimals_missing`).
pub(crate) enum IntentLegRead {
    Skip(&'static str),
    Ready {
        /// `{:#x}` del `pool_hint` (la lectura sólo ocurre con pool presente).
        pool_id: String,
        body: IntentLegBody,
        /// `(decimales token_in, decimales token_out)` resueltos.
        decimals: (u8, u8),
    },
}

/// Resultado de componer el grafo v4 de un intent.
pub(crate) struct IntentEdgeComposition {
    pub edges: Vec<crate::agent_graph::Edge>,
    /// `token_in` de la PRIMERA pierna admitida — el `start_token` que
    /// `build_v4_intent_bundle` exige. `None` cuando ninguna pierna entró.
    pub first_token_in: Option<String>,
    /// Histograma de omisiones (LOGFLOOD-01: una línea agregada, sin muestreo).
    pub skip_reasons: std::collections::BTreeMap<&'static str, u64>,
}

/// SHADOW-CANONICAL-01 — compone el grafo v4 a partir de lecturas YA resueltas.
/// Pura: sin Redis, sin env, sin reloj. `reads` va alineada 1:1 con
/// `intent.legs`; una pierna sin lectura se omite con su razón.
pub(crate) fn compose_intent_edges(
    chain_id: u64,
    ctx_id: &str,
    intent: &RouteIntent,
    reads: &[IntentLegRead],
) -> IntentEdgeComposition {
    let mut edges: Vec<crate::agent_graph::Edge> = Vec::new();
    let mut first_token_in: Option<String> = None;
    let mut skip_reasons: std::collections::BTreeMap<&'static str, u64> =
        std::collections::BTreeMap::new();
    for (leg, read) in intent.legs.iter().zip(reads.iter()) {
        match read {
            IntentLegRead::Skip(reason) => {
                *skip_reasons.entry(reason).or_insert(0) += 1;
            }
            IntentLegRead::Ready {
                pool_id,
                body,
                decimals,
            } => {
                if first_token_in.is_none() {
                    first_token_in = Some(format!("{:#x}", leg.token_in));
                }
                edges.push(intent_edge_from_ready(
                    chain_id, ctx_id, leg, pool_id, body, *decimals,
                ));
            }
        }
    }
    IntentEdgeComposition {
        edges,
        first_token_in,
        skip_reasons,
    }
}

/// Compone UNA arista del grafo v4 a partir de una pierna ya resuelta. La
/// identidad de coherencia es el `ts` del ROUND de sync (V2 y V3 del mismo
/// round lo comparten — verificado en producción), nunca un hash fabricado.
/// El protocolo y su fee salen de `v4_edge_protocol_and_fee` — la MISMA
/// derivación (y la única) que usa la lectura para elegir reservas o slot0.
fn intent_edge_from_ready(
    chain_id: u64,
    ctx_id: &str,
    leg: &crate::route_intent::RouteIntentLeg,
    pool_id: &str,
    body: &IntentLegBody,
    decimals: (u8, u8),
) -> crate::agent_graph::Edge {
    let (protocol, fee_units, fee_denominator) = v4_edge_protocol_and_fee(leg);
    let (reserve_in_raw, reserve_out_raw, sqrt_price_x96_raw, liquidity, block_identity) =
        match body {
            IntentLegBody::V2 {
                reserve_in_raw,
                reserve_out_raw,
                sync_ts,
            } => (
                Some(reserve_in_raw.clone()),
                Some(reserve_out_raw.clone()),
                None,
                None,
                format!("sync-ts-{sync_ts}"),
            ),
            IntentLegBody::V3 {
                sqrt_price_x96,
                liquidity,
                sync_ts,
            } => (
                None,
                None,
                Some(sqrt_price_x96.clone()),
                Some(*liquidity),
                format!("sync-ts-{sync_ts}"),
            ),
        };
    crate::agent_graph::Edge {
        edge_id: pool_id.to_owned(),
        pool_id: pool_id.to_owned(),
        chain_id,
        token_in: format!("{:#x}", leg.token_in),
        token_out: format!("{:#x}", leg.token_out),
        protocol: protocol.to_owned(),
        snapshot_id: ctx_id.to_owned(),
        block_hash: block_identity,
        reserve_in_raw,
        reserve_out_raw,
        fee_units,
        fee_denominator,
        token_in_decimals: decimals.0,
        token_out_decimals: decimals.1,
        // Adaptador que respalda estos edges: la caché de reservas del
        // searcher (procedencia real, no una versión de protocolo).
        adapter_version: "reserves_cache_v1".to_string(),
        sqrt_price_x96_raw,
        liquidity,
    }
}

/// BASKET-WORKER-01 (2026-10-03) — presupuesto TOTAL (wall-clock) de la lectura
/// on-chain de baskets dentro de la ruta del intent. El lector ya acota cada
/// `eth_call` a 3s; este tope acota la ESPERA COMPLETA del intent (≤2 baskets
/// relevantes, 2 llamadas cada uno) para que un RPC degradado jamás estire el
/// hot path. Presupuesto vencido ⇒ mapa vacío y FAIL honesto del verificador.
const BASKET_READ_BUDGET_MS: u64 = 1_500;

/// BASKET-WORKER-01 — los DOS tokens que `SnapshotServices::verify` consulta en
/// `redemption_state`: el `token_in` de la PRIMERA arista y el `token_out` de la
/// ÚLTIMA del grafo REAL que va al bundle (las aristas omitidas por R8 ya no
/// cuentan). Puro: sin env ni RPC, para poder probarlo directamente.
fn v4_redemption_tokens(edges: &[crate::agent_graph::Edge]) -> Vec<String> {
    let mut tokens: Vec<String> = Vec::with_capacity(2);
    if let Some(first) = edges.first() {
        tokens.push(first.token_in.to_ascii_lowercase());
    }
    if let Some(last) = edges.last() {
        let token_out = last.token_out.to_ascii_lowercase();
        if !tokens.contains(&token_out) {
            tokens.push(token_out);
        }
    }
    tokens
}

/// BASKET-WORKER-01 — subconjunto RELEVANTE de los baskets del operador
/// (`ARBX_BASKET_CONTRACTS`): sólo las direcciones que SON uno de los tokens
/// consultados arriba. Gate de COSTE del hot path: un basket configurado que no
/// participa en ESTA ruta no se consulta, y sin ningún basket relevante el
/// llamador NO toca la red (cero RPC). Puro: sin env ni RPC.
fn v4_relevant_baskets(configured: &[String], lookup_tokens: &[String]) -> Vec<String> {
    configured
        .iter()
        .filter(|basket| lookup_tokens.contains(*basket))
        .cloned()
        .collect()
}

/// BASKET-WORKER-01 — estado on-chain de los baskets relevantes a este intent
/// (o mapa vacío). Camino honesto y de coste acotado:
///
/// 1. Sin aristas, o sin basket configurado que participe en la ruta → mapa
///    vacío SIN RPC (el verificador reporta el FAIL honesto con su razón).
/// 2. BASKET-OWNER-01: con `ARBX_BASKET_CONTRACTS` configurado pero SIN
///    `ARBX_BASKET_OWNER` utilizable (ausente o malformada) → mapa vacío SIN
///    RPC. Sin owner no hay `maxRedeem` que consultar: el de `address(0)` es 0
///    en cualquier ERC-4626 estándar y el verificador lo leería como "el
///    importe excede el límite" — un diagnóstico FALSO. "Sin estado" es lo
///    honesto mientras el operador no termine de configurar (R8).
/// 3. Endpoints del operador: `RPC_HTTP_<chain_id>`, la MISMA var que alimenta
///    `HttpRpcPool`/`price_worker` (jamás una URL literal:
///    arbx-no-hardcode-doctrine). Sin endpoints → mapa vacío (R8).
/// 4. Lectura acotada por `BASKET_READ_BUDGET_MS`: RPC caído, error o
///    presupuesto vencido se registran a debug y dejan el mapa vacío — el
///    intent NUNCA se rompe y jamás se fabrica un `max_redeem` (R8).
///
/// La lectura async vive AQUÍ, en el llamador async: `build_v4_intent_bundle`
/// es sync por contrato y sólo recibe el resultado ya leído.
async fn v4_relevant_basket_state(
    chain_id: u64,
    edges: &[crate::agent_graph::Edge],
) -> std::collections::BTreeMap<String, serde_json::Value> {
    use crate::basket_reader::{
        basket_owner_from_env, baskets_from_env, plan_read, BasketReadPlan,
    };
    use std::collections::BTreeMap;

    // BASKET-OWNER-01: el plan resuelve de una vez QUÉ baskets y CON QUÉ owner
    // (executor del operador, `ARBX_BASKET_OWNER`). `NoOwner` corta antes de
    // cualquier RPC: `maxRedeem(0x0)` sería un límite falso (R8).
    let (baskets, owner) = match plan_read(
        v4_relevant_baskets(&baskets_from_env(), &v4_redemption_tokens(edges)),
        basket_owner_from_env(),
    ) {
        BasketReadPlan::Ready { baskets, owner } => (baskets, owner),
        BasketReadPlan::NoContracts => return BTreeMap::new(), // ningún basket en la ruta → CERO RPC
        BasketReadPlan::NoOwner { contracts, reason } => {
            debug!(
                event = "cartridge.basket_owner_unset",
                chain_id,
                contracts,
                reason,
                "ARBX_BASKET_CONTRACTS configurado sin ARBX_BASKET_OWNER utilizable; \
                 redemption_state vacío, sin RPC (R8 fail-honest)"
            );
            return BTreeMap::new();
        }
    };
    let rpc_urls: Vec<String> = crate::workers::price_worker::rpc_http_url_from_env(chain_id)
        .into_iter()
        .collect();
    if rpc_urls.is_empty() {
        debug!(
            event = "cartridge.basket_rpc_unset",
            chain_id,
            baskets = baskets.len(),
            "RPC_HTTP_<chain_id> sin endpoint HTTP; redemption_state vacío (R8 fail-honest)"
        );
        return BTreeMap::new();
    }
    let read = crate::basket_reader::read_baskets(&baskets, &rpc_urls, &owner);
    match tokio::time::timeout(
        std::time::Duration::from_millis(BASKET_READ_BUDGET_MS),
        read,
    )
    .await
    {
        Ok(state) => {
            debug!(
                event = "cartridge.basket_state_read",
                chain_id,
                baskets = baskets.len(),
                read = state.len(),
                "estado de redemption on-chain adjuntado al bundle del intent"
            );
            state
        }
        Err(_) => {
            debug!(
                event = "cartridge.basket_read_timeout",
                chain_id,
                baskets = baskets.len(),
                budget_ms = BASKET_READ_BUDGET_MS,
                "lectura de baskets agotó el presupuesto; redemption_state vacío (R8)"
            );
            BTreeMap::new()
        }
    }
}

// ── EXACT-QUOTES-PRODUCER-01 (2026-10-04) — productor ENCADENADO de quotes ────
// El mapa `exact_quotes` del bundle estaba VACÍO (`Default::default()`): el
// certificado `protocol_exact_quotes` sólo podía caer a la hipótesis within-tick
// — medido 61 de 269 filas en las ventanas 04:17:22 y 04:44:52 con
// `v3_within_tick_is_hypothesis_not_protocol_verified`, y 61 en la ventana
// 04:00 con la incoherencia de round de t22. AGENT-GRAPH-QUOTE-01 ya invirtió la
// precedencia (el mapa se consulta PRIMERO) y desacopló `firm_depth`/
// `firm_unwind` del slot0, así que poblarlo YA paga: esto es el productor.
//
// CADENA, no quote suelta: `quote_request_key` liga el IMPORTE exacto y el
// importe de la pierna `i+1` es la SALIDA de la pierna `i`, así que se recorre
// la ruta en orden arrastrando el importe. Las piernas CPMM no piden quote (su
// aritmética entera exacta ya certifica como `cpmm_exact_integer`) pero su salida
// se computa con ESA MISMA función — la del ledger — para alimentar la
// siguiente. CERO CPMM como sustituto de una quote V3, cero default, cero
// aproximación: lo que no cotiza NO entra al mapa.

/// Presupuesto wall-clock TOTAL del productor dentro del intent. Las quotes están
/// ENCADENADAS (no se pueden paralelizar sin conocer la salida previa) y cada
/// una es un `eth_call` al QuoterV2 con failover y breaker propios; este tope
/// acota la espera completa del intent para que un RPC degradado jamás estire el
/// hot path. Presupuesto vencido ⇒ las piernas restantes quedan SIN entrada y su
/// ausencia se declara con razón propia (R8).
const V4_EXACT_QUOTE_BUDGET_MS: u64 = 1_200;

/// Una pierna que NO obtuvo quote exacta: la entrada NO se inserta y la ausencia
/// viaja con su razón concreta. `detail` conserva el error crudo del quoter para
/// la línea agregada del log — jamás para rellenar un valor.
#[derive(Debug, Clone)]
struct V4ExactQuoteGap {
    index: usize,
    edge_id: String,
    protocol: String,
    reason: &'static str,
    detail: String,
}

/// Resultado del productor. `quotes` SÓLO contiene entradas de FUENTE REAL,
/// indexadas por `quote_request_key` — la clave amount-bound que `quote_path`
/// vuelve a computar. `chained_until_raw` es la evidencia de hasta dónde llegó la
/// cadena (la salida de la última pierna cotizada).
#[derive(Debug)]
struct V4ExactQuotes {
    quotes: std::collections::BTreeMap<String, crate::agent_graph::ExactHopQuote>,
    gaps: Vec<V4ExactQuoteGap>,
    chained_until_raw: String,
}

/// Pool RPC de la cadena, publicado por `spawn_cartridge_runtime` (que lo recibe
/// de `main.rs`). Ver el comentario de la inserción: el camino del intent corre
/// en otros tasks y no lo recibe por parámetro.
type V4ExactQuotePools =
    std::sync::Mutex<std::collections::HashMap<u64, Arc<shared_rs::rpc_failover::HttpRpcPool>>>;
/// Proveedores ya construidos, uno por cadena (conservan su caché TTL).
type V4ExactQuoteProviders = std::sync::Mutex<
    std::collections::HashMap<u64, Arc<crate::v3_quote_provider::MulticallV3QuoteProvider>>,
>;

fn v4_exact_quote_pools() -> &'static V4ExactQuotePools {
    static POOLS: OnceLock<V4ExactQuotePools> = OnceLock::new();
    POOLS.get_or_init(|| std::sync::Mutex::new(std::collections::HashMap::new()))
}

/// Proveedor de quotes V3 (QuoterV2) por cadena, construido PEREZOSAMENTE una
/// vez sobre el pool registrado: conserva su caché TTL + single-flight + backoff
/// de lote ENTRE intents (reutiliza la maquinaria existente —
/// `v3_quote_provider::MulticallV3QuoteProvider` sobre
/// `amm_math::v3_quote_exact_in_multicall` — jamás un quoter propio). `None` =
/// sin pool registrado o sin catálogo de quoter para esa cadena: fail-honest, el
/// mapa queda sin entradas y el certificado lo declara. El fallo NO se cachea:
/// el pool puede registrarse (o recuperarse) después.
fn v4_exact_quote_provider(
    chain_id: u64,
) -> Option<Arc<crate::v3_quote_provider::MulticallV3QuoteProvider>> {
    static PROVIDERS: OnceLock<V4ExactQuoteProviders> = OnceLock::new();
    let cache = PROVIDERS.get_or_init(|| std::sync::Mutex::new(std::collections::HashMap::new()));
    {
        let guard = cache.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(hit) = guard.get(&chain_id) {
            return Some(hit.clone());
        }
    }
    let pool = v4_exact_quote_pools()
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .get(&chain_id)
        .cloned();
    let built = pool.and_then(|p| {
        crate::v3_quote_provider::MulticallV3QuoteProvider::from_pool_and_chain(p, chain_id)
    });
    let built = built.map(Arc::new);
    if let Some(provider) = built.as_ref() {
        cache
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .insert(chain_id, provider.clone());
    }
    built
}

/// Productor de quotes exactas POR PIERNA, ENCADENADO. `quote_v3` inyecta el I/O
/// (una llamada al QuoterV2 para ESA pierna y ESE importe) de modo que la cadena
/// — justo la parte que `quote_request_key` hace amount-bound — sea testeable sin
/// red. Devuelve el mapa listo para el bundle y las ausencias declaradas.
async fn v4_exact_quotes<F, Fut>(
    edges: &[crate::agent_graph::Edge],
    amount_in_raw: &str,
    budget_ms: u64,
    mut quote_v3: F,
) -> V4ExactQuotes
where
    F: FnMut(crate::amm_math::V3QuoteRequest) -> Fut,
    Fut: std::future::Future<Output = Result<String, String>>,
{
    let started = std::time::Instant::now();
    let mut quotes: std::collections::BTreeMap<String, crate::agent_graph::ExactHopQuote> =
        std::collections::BTreeMap::new();
    let mut gaps: Vec<V4ExactQuoteGap> = Vec::new();
    let mut current = amount_in_raw.to_owned();

    for (index, edge) in edges.iter().enumerate() {
        if started.elapsed().as_millis() >= u128::from(budget_ms) {
            gaps.push(V4ExactQuoteGap {
                index,
                edge_id: edge.edge_id.clone(),
                protocol: edge.protocol.clone(),
                reason: "v4_exact_quote_budget_exhausted",
                detail: format!("{}ms", budget_ms),
            });
            break;
        }
        if edge.protocol == "cpmm_v2" {
            // Aritmética ENTERA EXACTA sobre las reservas del MISMO edge — la
            // función del ledger, no una aproximación nueva. La pierna no pide
            // quote de protocolo (el ledger la certifica como
            // `cpmm_exact_integer`), pero su salida alimenta la pierna siguiente.
            let Some(ri) = edge.reserve_in_raw.as_deref() else {
                gaps.push(V4ExactQuoteGap {
                    index,
                    edge_id: edge.edge_id.clone(),
                    protocol: edge.protocol.clone(),
                    reason: "cpmm_leg_missing_reserve_in",
                    detail: String::new(),
                });
                break;
            };
            let Some(ro) = edge.reserve_out_raw.as_deref() else {
                gaps.push(V4ExactQuoteGap {
                    index,
                    edge_id: edge.edge_id.clone(),
                    protocol: edge.protocol.clone(),
                    reason: "cpmm_leg_missing_reserve_out",
                    detail: String::new(),
                });
                break;
            };
            let (Some(fee), Some(den)) = (edge.fee_units, edge.fee_denominator) else {
                gaps.push(V4ExactQuoteGap {
                    index,
                    edge_id: edge.edge_id.clone(),
                    protocol: edge.protocol.clone(),
                    reason: "cpmm_leg_missing_fee",
                    detail: String::new(),
                });
                break;
            };
            match crate::agent_graph::cpmm_exact_in(&current, ri, ro, fee, den) {
                Ok(next) => current = next,
                Err(reason) => {
                    gaps.push(V4ExactQuoteGap {
                        index,
                        edge_id: edge.edge_id.clone(),
                        protocol: edge.protocol.clone(),
                        reason: "cpmm_leg_not_computable",
                        detail: reason,
                    });
                    break;
                }
            }
            continue;
        }
        if edge.protocol != "uniswap_v3" {
            // Sin productor para este protocolo: la cadena se corta aquí y la
            // ausencia se declara. Jamás se aproxima.
            gaps.push(V4ExactQuoteGap {
                index,
                edge_id: edge.edge_id.clone(),
                protocol: edge.protocol.clone(),
                reason: "protocol_quote_producer_absent",
                detail: String::new(),
            });
            break;
        }
        let (Ok(pool_addr), Ok(token_in_addr), Ok(token_out_addr)) = (
            edge.pool_id.parse::<ethers::types::Address>(),
            edge.token_in.parse::<ethers::types::Address>(),
            edge.token_out.parse::<ethers::types::Address>(),
        ) else {
            gaps.push(V4ExactQuoteGap {
                index,
                edge_id: edge.edge_id.clone(),
                protocol: edge.protocol.clone(),
                reason: "v3_leg_address_not_parseable",
                detail: format!("{} {} {}", edge.pool_id, edge.token_in, edge.token_out),
            });
            break;
        };
        let Ok(amount_in) = ethers::types::U256::from_dec_str(&current) else {
            gaps.push(V4ExactQuoteGap {
                index,
                edge_id: edge.edge_id.clone(),
                protocol: edge.protocol.clone(),
                reason: "v3_leg_amount_not_decimal",
                detail: String::new(),
            });
            break;
        };
        let Some(fee_bps) = edge.fee_units else {
            gaps.push(V4ExactQuoteGap {
                index,
                edge_id: edge.edge_id.clone(),
                protocol: edge.protocol.clone(),
                reason: "v3_leg_missing_fee",
                detail: String::new(),
            });
            break;
        };
        let request = crate::amm_math::V3QuoteRequest {
            pool_addr,
            token_in: token_in_addr,
            token_out: token_out_addr,
            amount_in,
            fee_bps,
        };
        match quote_v3(request).await {
            Ok(amount_out_raw) => {
                // La entrada se ata al MISMO edge e identidad que el ledger
                // exige: `quote_path` rechaza cualquier desajuste de edge_id /
                // snapshot_id / block_hash / tokens / importe / adapter_version
                // (por eso se construye con la MISMA función de clave).
                let key = crate::agent_graph::quote_request_key(edge, &current);
                quotes.insert(
                    key,
                    crate::agent_graph::ExactHopQuote {
                        edge_id: edge.edge_id.clone(),
                        snapshot_id: edge.snapshot_id.clone(),
                        block_hash: edge.block_hash.clone(),
                        token_in: edge.token_in.clone(),
                        token_out: edge.token_out.clone(),
                        amount_in_raw: current.clone(),
                        amount_out_raw: amount_out_raw.clone(),
                        // Procedencia: el hash canónico de la petición + la
                        // respuesta REALES. No es un id inventado: identifica
                        // exactamente el par (edge, importe, salida) que el
                        // ledger va a consumir.
                        quote_id: crate::rhai_agent_bridge::canonical_hash(&serde_json::json!({
                            "producer": "v3_quote_provider",
                            "model": "quoter_v2_exact_input_single",
                            "edge_id": edge.edge_id,
                            "snapshot_id": edge.snapshot_id,
                            "block_hash": edge.block_hash,
                            "token_in": edge.token_in,
                            "token_out": edge.token_out,
                            "amount_in_raw": current,
                            "amount_out_raw": amount_out_raw,
                            "adapter_version": edge.adapter_version,
                        })),
                        precision: "protocol_exact_integer".into(),
                        fees_and_impact_embedded: true,
                        adapter_version: edge.adapter_version.clone(),
                        metrics: serde_json::json!({
                            "status": "COMPUTED",
                            "model": "quoter_v2_exact_input_single",
                            "source": "v3_quote_provider",
                            "fee_bps": fee_bps,
                            "hop_index": index,
                        }),
                    },
                );
                current = amount_out_raw;
            }
            Err(reason) => {
                gaps.push(V4ExactQuoteGap {
                    index,
                    edge_id: edge.edge_id.clone(),
                    protocol: edge.protocol.clone(),
                    reason: "v3_protocol_quote_unavailable",
                    detail: reason,
                });
                break;
            }
        }
    }

    V4ExactQuotes {
        quotes,
        gaps,
        chained_until_raw: current,
    }
}

/// Fase 3a — construye el SnapshotBundle REAL del intent: policy honesta
/// desde la config del operador (`TradingConfigState`), precios canónicos
/// por token distinto de las piernas (dirección → símbolo del universo de
/// identidad → precio del snapshot Redis o de trading_config), el tamaño
/// REAL observado del intent como único tamaño del schedule y la admisión
/// EXPLÍCITA de manifiestos v4 desplegados (Fase 3b). `exact_quotes` llega
/// PRODUCIDO por el llamador async (EXACT-QUOTES-PRODUCER-01: quotes reales del
/// QuoterV2, encadenadas por pierna); lo que sigue ausente (`domain_plans`,
/// `canonical_payloads`) queda vacío y el contrato v4 lo reporta como DATA_GAP
/// con razón explícita — nunca se fabrica (R8). `None` sólo si el reloj no
/// permite una ventana temporal honesta.
///
/// BASKET-WORKER-01: el estado de redemption on-chain de los baskets
/// RELEVANTES a este intent llega YA LEÍDO por el llamador async
/// (`basket_state`); esta función es sync por contrato y nunca toca la red.
///
/// PRICE-COVERAGE-01 (§38): el mapa canónico de precios tiene TRES fuentes
/// ORDENADAS por confianza — (1) el snapshot de Redis que publica
/// `price_worker` (`arbx:token_prices:<chain>`, hash de símbolos), (2) el
/// `token_prices_usd` curado por el operador en `trading_config`, y (3) el
/// **PriceBus en proceso** (`price_bus_global::get()`, Binance WS bookTicker
/// fusionado con los anchors Chainlink). La tercera cierra el hueco real de
/// cobertura: el snapshot es un hash *polado y acotado*
/// (`MAX_PRICED_TOKENS`) y `token_prices_usd` es una lista curada, así que un
/// token del grafo puede no estar en ninguno de los dos mientras el bus —la
/// MISMA pila soberana de la que el snapshot se nutre— sí lo tiene vivo. La
/// lectura es lock-free y en proceso (`ArcSwap::load`), jamás un RPC por token
/// en el hot path.
///
/// `price_bus: None` (bus no inicializado, p.ej. en tests) degrada honesto:
/// las dos primeras fuentes siguen, y un token que ninguna cubre queda SIN
/// entrada (R8) — exactamente la conducta previa, nunca un precio inventado.
#[allow(clippy::too_many_arguments)]
fn build_v4_intent_bundle(
    chain_id: u64,
    ctx_snapshot_id: &str,
    edges: Vec<crate::agent_graph::Edge>,
    start_token: &str,
    amount_in_raw: &str,
    cfg: &shared_rs::trading_config::TradingConfigState,
    identity: &shared_rs::token_identity::TokenIdentityIndex,
    price_snapshot: &std::collections::HashMap<String, f64>,
    manifest_digests: &std::collections::BTreeMap<String, String>,
    // COST-PRODUCERS-01: gas OBSERVADO del runner (el getter ya decodifica
    // milli-gwei → gwei) — el mismo que alimenta el MarketState del
    // dispatcher. Sin observación la línea de gas no se emite (R8).
    gas_price_gwei: f64,
    // BASKET-WORKER-01: estado on-chain de los baskets RELEVANTES a este
    // intent (token_in de la primera arista / token_out de la última), leído
    // por el llamador async. Sin basket en la ruta, sin RPC configurado o con
    // lectura fallida llega VACÍO: el verificador `redemption_within_limits`
    // reporta entonces su FAIL honesto y jamás se fabrica un `max_redeem`.
    basket_state: &std::collections::BTreeMap<String, serde_json::Value>,
    // PRICE-COVERAGE-01: bus de precios en proceso (tercera fuente, §38). Se
    // recibe por parámetro en vez de leerse del global dentro de la función
    // para que el llamador —y los tests— fijen explícitamente la dependencia:
    // `None` es el degradado honesto (sin bus no hay tercera fuente).
    price_bus: Option<&shared_rs::price_bus::PriceBus>,
    // EXACT-QUOTES-PRODUCER-01: las quotes EXACTAS por pierna ya producidas por
    // el llamador async (`v4_exact_quotes`), indexadas por `quote_request_key`.
    // Este campo era `Default::default()`: el certificado `protocol_exact_quotes`
    // sólo podía caer a la hipótesis within-tick. Sólo entran entradas de FUENTE
    // REAL; un mapa vacío significa "no se pudo cotizar", jamás "cotizó cero".
    exact_quotes: std::collections::BTreeMap<String, crate::agent_graph::ExactHopQuote>,
) -> Option<crate::snapshot_services::SnapshotBundle> {
    let observed_at_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .ok()?
        .as_millis() as u64;
    let valid_until_ms = observed_at_ms + 60_000;
    // Revisiones ancladas al instante de composición sobre la config real:
    // "cfg-<ts_ms>" (origen "cfg" + timestamp, según spec de Fase 3a).
    let revision = format!("cfg-{observed_at_ms}");
    let policy = crate::rhai_agent_bridge::PolicyView {
        enabled: cfg.enabled,
        capital_cap_usd: cfg.capital_usd.to_string(),
        // simulation_target_profit_usd es el target de simulación del
        // operador; ausente → None (el gate de build_payload lo omite — R8).
        min_profit_usd: cfg.simulation_target_profit_usd.map(|v| v.to_string()),
        // TradingConfigState no trae tope de gas a nivel cadena → None
        // honesto (nunca un tope inventado).
        max_gas_usd: None,
        snapshot_id: ctx_snapshot_id.to_owned(),
        price_revision: revision.clone(),
        policy_revision: revision.clone(),
        // Modo canónico del bridge (economic_check exige
        // LIVE_MAINNET|TESTNET|PAPER_SHADOW): esta ruta evalúa en papel y la
        // intercepción v4 rechaza a observación; el terminus real vive en
        // relays-client (§34.3).
        execution_mode: "PAPER_SHADOW".into(),
        control_state: if cfg.enabled {
            "operator_config_enabled".into()
        } else {
            "operator_config_disabled".into()
        },
    };
    let mut tokens: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
    for edge in &edges {
        tokens.insert(edge.token_in.clone());
        tokens.insert(edge.token_out.clone());
    }
    let mut prices = std::collections::BTreeMap::new();
    for token in tokens {
        let Some(symbol) = identity.symbol_for_addr(&token) else {
            continue; // sin identidad de universo → sin precio (R8)
        };
        let from_snapshot = price_snapshot
            .get(&symbol.to_ascii_uppercase())
            .copied()
            .map(|v| (v, "price_snapshot"));
        let from_config = cfg
            .token_prices_usd
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case(symbol))
            .map(|(_, v)| (*v, "trading_config"));
        // PRICE-COVERAGE-01 (§38) — tercera fuente: el PriceBus EN PROCESO.
        // `price_with_verdict` devuelve `None` tanto en `NoSource` (ni Binance
        // ni anchor para el símbolo) como en `DivergenceFrozen` (la banda
        // Binance↔Chainlink se congeló: un depeg o un feed roto DEBE parar la
        // valuación, no colarse promediado). El `filter` interno repite el
        // invariante del `filter` externo para que un valor no finito o no
        // positivo del bus no llegue siquiera a etiquetarse.
        //
        // El `evidence_id` nombra la fuente REAL y su veredicto REAL: en
        // `price_bus:stale_anchor` el número servido es el de Binance sin
        // verificación de anchor, y eso queda declarado en la evidencia en vez
        // de disfrazarse de precio verificado. `producer` sigue siendo
        // "PriceBus" porque el bus ES literalmente el productor — es la
        // identidad que exige `SnapshotServices::price()`, no una etiqueta
        // prestada para pasar el gate. El id es `&'static str` (como los de las
        // dos fuentes previas): el hot path no asigna para etiquetar.
        let from_bus = price_bus.and_then(|bus| {
            let (price, verdict) = bus.view().price_with_verdict(symbol);
            price
                .filter(|v| v.is_finite() && *v > 0.0)
                .map(|v| (v, v4_price_bus_evidence_id(verdict)))
        });
        let Some((usd, evidence_id)) = from_snapshot
            .or(from_config)
            .or(from_bus)
            .filter(|(v, _)| v.is_finite() && *v > 0.0)
        else {
            continue; // sin precio real positivo → SIN entrada (R8)
        };
        prices.insert(
            (chain_id, token.clone()),
            crate::snapshot_services::CanonicalPrice {
                chain_id,
                token_address: token,
                usd: usd.to_string(),
                revision: revision.clone(),
                observed_at_ms,
                valid_until_ms,
                evidence_id: evidence_id.to_owned(),
                // Productor canónico del bus de precios que respalda el
                // snapshot Redis (identidad exigida por
                // SnapshotServices::price — "PriceBus").
                producer: "PriceBus".into(),
            },
        );
    }
    // COST-PRODUCERS-01: las líneas se computan ANTES del literal del struct
    // (edges/prices se mueven dentro de él — evaluación de campos en orden).
    let base_costs = v4_base_cost_lines(
        cfg,
        chain_id,
        start_token,
        amount_in_raw,
        edges.first().map(|e| e.token_in_decimals).unwrap_or(18),
        gas_price_gwei,
        &prices,
    );
    Some(crate::snapshot_services::SnapshotBundle {
        context_id: ctx_snapshot_id.to_owned(),
        snapshot_id: ctx_snapshot_id.to_owned(),
        observed_at_ms,
        valid_until_ms,
        policy,
        start_token: start_token.to_owned(),
        chain_id,
        edges,
        limits: crate::agent_graph::SearchLimits {
            max_hops: canonical_max_hops() as usize,
            max_expansions: 512,
            max_paths: 64,
        },
        // Tamaño REAL observado del intent — el único tamaño honesto
        // conocido para este contexto (spec Fase 3a).
        size_schedule_raw: vec![amount_in_raw.to_owned()],
        prices,
        exact_quotes,
        route_support: Default::default(),
        domain_plans: Default::default(),
        canonical_payloads: Default::default(),
        // Fase 3b: admisión explícita de los manifiestos v4 desplegados
        // (mev_id → source_digest auto-declarado por cada script).
        manifest_digests: manifest_digests.clone(),
        max_evaluations: 8,
        // COST-PRODUCERS-01: líneas BASE con productores reales del dueño del
        // contexto (gas observado × unidades de config, financiación según la
        // tasa declarada, comisiones embebidas en las cotizaciones).
        base_cost_lines: base_costs,
        // BASKET-WORKER-01: estado on-chain REAL de los baskets del operador
        // que participan en ESTA ruta (leído por el llamador async). Vacío =
        // sin dato → el verificador reporta el FAIL honesto (R8).
        redemption_state: basket_state.clone(),
    })
}

/// COST-PRODUCERS-01 (2026-10-02): las tres líneas de coste que el bridge
/// exige para `atomic_quote` (gas, financing, execution_fees), cada una con
/// su PRODUCTOR real y su tratamiento declarado. Antes `quote()` dejaba
/// `costs=[]` cuando el soporte precomputado por plan no existía (el caso de
/// TODA ruta del grafo en la vía del intent) y el bridge reportaba
/// `mandatory_route_cost_missing` — el bloqueo dominante medido en outcomes.
///
/// - **gas** (external, se resta una vez): unidades de la CONFIG del operador
///   (`gas_estimate_units`) × gas OBSERVADO del runner × precio base. Sin
///   alguno de los tres → la línea no se emite (DATA_GAP honesto, no cero).
/// - **financiación**: reserva conservadora al importe del intent ×
///   `flashloan_fee_pct` de la config — si el plan usa flash este es el
///   coste; con capital propio el coste real es 0 y la reserva SOBREestima
///   (sesgo conservador, jamás infla el neto). Tasa 0 (config actual) →
///   `not_applicable` con evidencia explícita. La lectura on-chain del
///   premium real (`FLASHLOAN_PREMIUM_TOTAL`) sigue pendiente (doctrina
///   flash-loan: fees on-chain, jamás hardcode) — por eso la fuente ES la
///   config del operador, no un literal.
/// - **execution_fees** (embedded): las comisiones e impacto YA viven dentro
///   de las cotizaciones del ledger (`fees_and_impact_embedded`) — se
///   declara para satisfacer el contrato del bridge SIN restar dos veces.
fn v4_base_cost_lines(
    cfg: &shared_rs::trading_config::TradingConfigState,
    chain_id: u64,
    start_token: &str,
    amount_in_raw: &str,
    start_token_decimals: u8,
    gas_price_gwei: f64,
    prices: &std::collections::BTreeMap<(u64, String), crate::snapshot_services::CanonicalPrice>,
) -> Vec<crate::rhai_agent_bridge::CostLine> {
    use crate::rhai_agent_bridge::CostLine;
    let mut lines: Vec<CostLine> = Vec::new();

    // ── GAS: unidades de config × gwei observado × 1e-9 → ETH → USD base ──
    let base_usd = cfg.base_token_price_usd;
    if cfg.gas_estimate_units > 0 && gas_price_gwei > 0.0 && base_usd > 0.0 {
        let gas_usd = cfg.gas_estimate_units as f64 * gas_price_gwei * 1e-9 * base_usd;
        if gas_usd.is_finite() && gas_usd > 0.0 {
            lines.push(CostLine {
                kind: "gas".into(),
                treatment: "external".into(),
                usd: Some(format!("{gas_usd:.6}")),
                reason: Some(format!(
                    "{}units x {:.4}gwei x {:.4}usd (config:gas_estimate_units, runner:host_gas_price_gwei)",
                    cfg.gas_estimate_units, gas_price_gwei, base_usd
                )),
                evidence_id: "config:gas_estimate_units+runner:observed_gas".into(),
            });
        }
    }

    // ── FINANCIACIÓN: tasa DECLARADA por el operador en la config ──
    //
    // V4-SNAPSHOT-PRODUCERS-01 (2026-10-04): la línea de financiación NO puede
    // OMITIRSE. Antes, con `flashloan_fee_pct > 0` y sin precio canónico del
    // start token, el `if let Some(amount_usd)` no tenía `else`: la línea
    // desaparecía en silencio y el bridge reportaba
    // `costs.financing::mandatory_route_cost_missing` — una razón que MIENTE,
    // porque el productor existe y lo que falta es el precio. Medido en
    // producción (chain 1, 2026-10-03): `flashloan_fee_pct = 0.0009 > 0`,
    // `token_prices_usd` = 21 símbolos, hash de precios `arbx:token_prices:1` =
    // 447 símbolos frente a un universo de identidad de 2747 tokens → las rutas
    // long-tail llegan sin precio, y ese reparto aparecía 64/269 veces.
    //
    // R8: un coste obligatorio NO COMPUTABLE se DECLARA (línea presente, `usd`
    // ausente, razón explícita), nunca se omite ni se sustituye por un valor por
    // defecto. La línea queda `external` — la financiación APLICA (tasa > 0):
    // declararla `not_applicable` sería afirmar que el coste no existe y
    // inflaría el neto.
    if cfg.flashloan_fee_pct > 0.0 {
        // Importe del intent valorado al precio del start token del bundle.
        let amount_usd = prices
            .get(&(chain_id, start_token.to_string()))
            .and_then(|p| p.usd.parse::<f64>().ok())
            .and_then(|px| {
                amount_in_raw
                    .parse::<f64>()
                    .ok()
                    .map(|raw| raw / 10f64.powi(start_token_decimals as i32) * px)
            });
        match amount_usd {
            // Valorado: la reserva es tasa-declarada × importe del intent.
            Some(amount_usd_val) => {
                let fin_usd = amount_usd_val * cfg.flashloan_fee_pct / 100.0;
                if fin_usd.is_finite() && fin_usd > 0.0 {
                    lines.push(CostLine {
                        kind: "financing".into(),
                        treatment: "external".into(),
                        usd: Some(format!("{fin_usd:.6}")),
                        reason: Some(format!(
                            "reserva conservadora al flash: {:.4}usd x {:.4}% (config:flashloan_fee_pct; capital propio => coste real 0)",
                            amount_usd_val, cfg.flashloan_fee_pct
                        )),
                        evidence_id: "config:flashloan_fee_pct".into(),
                    });
                } else {
                    lines.push(CostLine {
                        kind: "financing".into(),
                        treatment: "external".into(),
                        usd: None,
                        reason: Some(format!(
                            "reserva flash NO COMPUTADA: importe valorado no positivo (config:flashloan_fee_pct={:.6}%) — el importe NO se estima (R8)",
                            cfg.flashloan_fee_pct
                        )),
                        evidence_id: "config:flashloan_fee_pct+uncomputed:non_positive_reserve".into(),
                    });
                }
            }
            // Sin precio canónico del start token: DECLARADA, jamás omitida.
            None => lines.push(CostLine {
                kind: "financing".into(),
                treatment: "external".into(),
                usd: None,
                reason: Some(format!(
                    "reserva flash NO COMPUTADA: sin precio canonico del start token {start_token} \
                     en el bundle (prices map miss) — tasa {:.6}% declarada en \
                     config:flashloan_fee_pct; el importe NO se estima (R8)",
                    cfg.flashloan_fee_pct
                )),
                evidence_id: "config:flashloan_fee_pct+unpriced:start_token".into(),
            }),
        }
    } else {
        // Tasa 0 (config actual): capital propio — not_applicable con
        // evidencia explícita (el contrato CostLine lo exige).
        lines.push(CostLine {
            kind: "financing".into(),
            treatment: "not_applicable".into(),
            usd: None,
            reason: Some(
                "flashloan_fee_pct=0 en trading_config (capital propio, sin reserva flash)".into(),
            ),
            evidence_id: "config:flashloan_fee_pct:zero".into(),
        });
    }

    // ── COMISIONES: embebidas en las cotizaciones — declaradas, no restadas ──
    lines.push(CostLine {
        kind: "execution_fees".into(),
        treatment: "embedded".into(),
        usd: None,
        reason: Some(
            "fees_and_impact_embedded en el ledger de quotes (ya reflejadas; no se restan dos veces)"
                .into(),
        ),
        evidence_id: "quote:ledger:fees_and_impact_embedded".into(),
    });

    lines
}

/// PRICE-COVERAGE-01 (§38) — `evidence_id` de un precio servido por el PriceBus
/// EN PROCESO: nombra la fuente (`price_bus`) y el veredicto REAL con el que el
/// bus lo sirvió, para que la evidencia diga la verdad completa (un número de
/// Binance sin anchor de verificación se declara `stale_anchor`, no se disfraza
/// de verificado).
///
/// `&'static str` y no `format!`: el `evidence_id` de las otras dos fuentes ya
/// es estático y el hot path no debe asignar sólo para etiquetar. El `match` es
/// exhaustivo a propósito — un `Verdict` nuevo rompe la compilación en vez de
/// degradar en silencio, y `price_bus_evidence_ids_mirror_the_verdict_slugs`
/// fija que cada slug siga siendo el de `Verdict::as_str()`.
fn v4_price_bus_evidence_id(verdict: shared_rs::price_bus::Verdict) -> &'static str {
    use shared_rs::price_bus::Verdict;
    match verdict {
        Verdict::Ok => "price_bus:ok",
        Verdict::StaleBinance => "price_bus:stale_binance",
        Verdict::StaleAnchor => "price_bus:stale_anchor",
        // Mismo slug que `Verdict::as_str()`, que es como el resto del sistema
        // publica esta razón.
        Verdict::DivergenceFrozen => "price_bus:price_divergence_binance_chainlink",
        Verdict::NoSource => "price_bus:no_live_price",
    }
}

/// SHADOW-CANONICAL-01 — LECTURA de las piernas del intent con las MISMAS
/// claves Redis y el MISMO orden que usaba la ruta ACTIVE (reservas V2
/// orientadas por `token0_addr` / slot0 V3, y después decimales). El orden
/// importa: fija qué razón de omisión se reporta cuando fallan varios datos.
/// Cachea por intent para no repetir GETs del mismo token/pool entre piernas.
async fn read_intent_legs(
    runner: &Arc<CartridgeRunner>,
    redis: &mut redis::aio::ConnectionManager,
    intent: &RouteIntent,
    chain_id: u64,
) -> Vec<IntentLegRead> {
    // Camino feliz (el normal): UNA pasada y, si las piernas resueltas comparten
    // round, se devuelve tal cual — la alineación no cuesta nada cuando la caché
    // no está rotando.
    let mut reads = read_intent_legs_pass(runner, redis, intent, chain_id).await;
    if intent_legs_share_one_round(&reads) {
        return reads;
    }
    let started = std::time::Instant::now();
    let mut passes: u8 = 1;
    while passes < INTENT_ROUND_ALIGN_MAX_PASSES
        && started.elapsed().as_millis() < u128::from(INTENT_ROUND_ALIGN_BUDGET_MS)
    {
        let laggards = leg_round_laggard_indices(&reads);
        if laggards.is_empty() {
            break;
        }
        let rounds: Vec<Option<u64>> = reads.iter().map(leg_round_ts).collect();
        debug!(
            event = "cartridge.v4_intent_round_split",
            chain_id,
            tx_hash = %intent.tx_hash,
            pass = passes,
            laggards = ?laggards,
            newest_round = ?newest_leg_round(&reads),
            rounds = ?rounds,
            "piernas del intent repartidas entre rounds de sync; re-lectura acotada de las rezagadas (R8)"
        );
        // El writer refresca la población POOL A POOL (medido: ~3 s para 557
        // pools), así que un re-read inmediato devolvería el MISMO valor viejo.
        // Este backoff acotado es el que deja llegar al round nuevo.
        tokio::time::sleep(std::time::Duration::from_millis(
            INTENT_ROUND_ALIGN_BACKOFF_MS,
        ))
        .await;
        // Cachés FRESCAS por pasada: el `slot0_cache` de la lectura anterior
        // serviría el valor del round viejo y la alineación nunca convergería.
        // Los decimales no dependen del round y no hacen falta entre pasadas.
        let mut decimal_cache: std::collections::HashMap<String, Option<u8>> =
            std::collections::HashMap::new();
        let mut slot0_cache: std::collections::HashMap<String, Option<(String, u128, u64)>> =
            std::collections::HashMap::new();
        for index in laggards {
            // `reads` va alineada 1:1 con `intent.legs` (una lectura por pierna,
            // en orden) ⇒ el índice del laggard identifica la pierna.
            let Some(leg) = intent.legs.get(index) else {
                continue;
            };
            reads[index] = read_intent_leg(
                runner,
                redis,
                leg,
                chain_id,
                &mut decimal_cache,
                &mut slot0_cache,
            )
            .await;
        }
        passes += 1;
    }
    reads
}

/// PROTOCOL-COHERENCE-01 (2026-10-04) — el `ts` del round de sync es la
/// identidad de coherencia de una ruta atómica, y la caché NO la sostiene
/// durante la rotación: el writer refresca la población pool a pool, así que
/// existe una ventana en la que parte de las entradas ya están en el bloque N+1
/// y el resto sigue en N. MEDIDO en producción (solo-lectura, 2026-10-04):
/// `arbx:pool_reserves:1:*` (557 entradas) partido en la misma muestra entre
/// `blk 26116479` (30) y `blk 26116480` (527), y en otra entre `blk ...480`
/// (384) y `blk ...481` (173); `arbx:v3_slot0:1:*` (318) con `ts` idéntico
/// dentro de cada round. Una ruta con piernas de DOS rounds mezcla estado de
/// dos bloques distintos: `quote_path_progress` la rechaza con
/// `mixed_block_or_domain_in_atomic_route` y TODOS los recibos de la ruta caen
/// — medido como
/// `protocol_exact_quotes::hop_Some(1):mixed_block_or_domain_in_atomic_route`
/// (61 de 269) más `same_snapshot::edges_span_multiple_sync_rounds_or_snapshots`
/// (37) en el gate v4 (ventana 2026-10-04 04:00).
///
/// Por eso la lectura ALINEA el round antes de componer: si las piernas
/// resueltas quedaron en rounds distintos, re-lee SÓLO las rezagadas (las demás
/// ya están en el round más nuevo) dentro de un presupuesto wall-clock acotado.
/// Si el presupuesto se agota sin alinear, devuelve la lectura TAL CUAL y es el
/// llamador quien declara el contexto NO COMPUTADO (R8/R10) — jamás se
/// re-etiqueta la identidad para tapar que el grafo mezcla dos bloques.
const INTENT_ROUND_ALIGN_BUDGET_MS: u64 = 600;
const INTENT_ROUND_ALIGN_MAX_PASSES: u8 = 4;
const INTENT_ROUND_ALIGN_BACKOFF_MS: u64 = 150;

/// El `ts` del round de sync de una pierna RESUELTA. `None` en las omitidas: una
/// pierna omitida no aporta estado al grafo y por tanto no puede desalinearlo.
fn leg_round_ts(read: &IntentLegRead) -> Option<u64> {
    match read {
        IntentLegRead::Ready { body, .. } => Some(match body {
            IntentLegBody::V2 { sync_ts, .. } | IntentLegBody::V3 { sync_ts, .. } => *sync_ts,
        }),
        IntentLegRead::Skip(_) => None,
    }
}

/// Round MÁS NUEVO observado entre las piernas resueltas del intent.
fn newest_leg_round(reads: &[IntentLegRead]) -> Option<u64> {
    reads.iter().filter_map(leg_round_ts).max()
}

/// Todas las piernas RESUELTAS comparten UN round ⇔ el grafo es coherente: una
/// sola frontera de estado, una sola identidad de bloque para la ruta atómica.
/// 0 o 1 pierna resuelta son coherentes por vacuidad (no hay ruta que mezcle).
fn intent_legs_share_one_round(reads: &[IntentLegRead]) -> bool {
    match newest_leg_round(reads) {
        None => true,
        Some(newest) => reads.iter().filter_map(leg_round_ts).all(|ts| ts == newest),
    }
}

/// Índices de las piernas RESUELTAS que quedaron en un round ANTERIOR al más
/// nuevo: son exactamente las que hay que re-leer para alinear el grafo.
fn leg_round_laggard_indices(reads: &[IntentLegRead]) -> Vec<usize> {
    let Some(newest) = newest_leg_round(reads) else {
        return Vec::new();
    };
    reads
        .iter()
        .enumerate()
        .filter(|(_, read)| leg_round_ts(read).is_some_and(|ts| ts < newest))
        .map(|(index, _)| index)
        .collect()
}

/// Lectura de UNA pierna del intent (el cuerpo que antes vivía inline en el
/// bucle de `read_intent_legs`, ahora invocable para la pasada completa y para
/// la re-lectura acotada de una rezagada).
async fn read_intent_leg(
    runner: &Arc<CartridgeRunner>,
    redis: &mut redis::aio::ConnectionManager,
    leg: &crate::route_intent::RouteIntentLeg,
    chain_id: u64,
    decimal_cache: &mut std::collections::HashMap<String, Option<u8>>,
    slot0_cache: &mut std::collections::HashMap<String, Option<(String, u128, u64)>>,
) -> IntentLegRead {
    // Gate de COSTE: una pierna sin pool o degenerada se descarta ANTES de
    // cualquier I/O (idéntico a la ruta ACTIVE previa).
    let Some(pool) = leg.pool_hint else {
        return IntentLegRead::Skip("missing_pool_hint");
    };
    let token_in = format!("{:#x}", leg.token_in);
    let token_out = format!("{:#x}", leg.token_out);
    if token_in == token_out {
        // Una pierna degenerada invalidaría TODO el grafo
        // (enumerate_cycles: invalid_or_duplicate_graph_edge); se omite.
        return IntentLegRead::Skip("degenerate_self_pair");
    }
    // El protocolo decide la FUENTE del dato (V3 → slot0; resto →
    // reservas). El fee lo deriva la composición desde la misma función
    // pura, así que aquí sólo se necesita el protocolo.
    let (protocol, _, _) = v4_edge_protocol_and_fee(leg);
    let pool_id = format!("{:#x}", pool);
    // V3-LEG-GRAPH-01 (RESERVES-CACHE-01, 2026-10-01): el protocolo se
    // computa ANTES del gate de reservas. Un pool V3 NO tiene
    // getReserves(): su dato vive en `arbx:v3_slot0`, jamás en
    // `arbx:pool_reserves`. Exigirle la entrada V2 descartaba TODA pierna
    // V3 con el motivo engañoso `reserves_missing` — 98% de las piernas
    // descartadas en producción (medido 2026-10-01). El Edge ya soporta V3
    // (reserve_in_raw: Option) y quote_path lo resuelve con
    // v3_spot_within_tick (orientación derivada de token_in/token_out).
    let body = if protocol == "uniswap_v3" {
        match v4_slot0(redis, chain_id, &pool_id, slot0_cache).await {
            Some((sp, liq, ts)) => IntentLegBody::V3 {
                sqrt_price_x96: sp,
                liquidity: liq,
                sync_ts: ts,
            },
            // Sin slot0 cacheado → skip con motivo PROPIO, no el engañoso.
            None => return IntentLegRead::Skip("v3_slot0_missing"),
        }
    } else {
        let Some(entry) = runner.read_pool_reserves(&pool_id).await else {
            return IntentLegRead::Skip("reserves_missing");
        };
        // Orientación exacta: token0_addr declara cuál reserva es "in"
        // para esta pierna. Sin token0_addr o con token0 fuera de la
        // ruta → OMITIR (R8: sin dual-orientation ni inferencia).
        let Some(token0) = entry.token0_addr.as_deref() else {
            return IntentLegRead::Skip("token0_addr_missing");
        };
        let pair = if token0 == token_in {
            (entry.r0.clone(), entry.r1.clone())
        } else if token0 == token_out {
            (entry.r1.clone(), entry.r0.clone())
        } else {
            return IntentLegRead::Skip("token0_addr_out_of_route");
        };
        // Identidad de coherencia del ROUND DE SYNC (PLAN-SUPPORT-WIRING-
        // 01): V2 y V3 del mismo round comparten ts (verificado en
        // producción), mientras que blk solo existe en V2 y ts solo se
        // usaba en V3 — con identidades distintas, quote_path_progress
        // rechazaba TODA ruta mixta con mixed_block_or_domain. El ts del
        // round ES la frontera real de coherencia (todas las entradas se
        // escribieron juntas). JAMÁS se fabrica un hash (R8).
        IntentLegBody::V2 {
            reserve_in_raw: pair.0,
            reserve_out_raw: pair.1,
            sync_ts: entry.ts,
        }
    };
    let Some(dec_in) = v4_token_decimals(redis, chain_id, &token_in, decimal_cache).await else {
        return IntentLegRead::Skip("token_in_decimals_missing");
    };
    let Some(dec_out) = v4_token_decimals(redis, chain_id, &token_out, decimal_cache).await else {
        return IntentLegRead::Skip("token_out_decimals_missing");
    };
    IntentLegRead::Ready {
        pool_id,
        body,
        decimals: (dec_in, dec_out),
    }
}

/// Una pasada completa: una lectura por pierna, en el orden del intent (el
/// orden fija qué razón de omisión se reporta cuando fallan varios datos).
async fn read_intent_legs_pass(
    runner: &Arc<CartridgeRunner>,
    redis: &mut redis::aio::ConnectionManager,
    intent: &RouteIntent,
    chain_id: u64,
) -> Vec<IntentLegRead> {
    let mut decimal_cache: std::collections::HashMap<String, Option<u8>> =
        std::collections::HashMap::new();
    let mut slot0_cache: std::collections::HashMap<String, Option<(String, u128, u64)>> =
        std::collections::HashMap::new();
    let mut reads: Vec<IntentLegRead> = Vec::with_capacity(intent.legs.len());
    for leg in &intent.legs {
        reads.push(
            read_intent_leg(
                runner,
                redis,
                leg,
                chain_id,
                &mut decimal_cache,
                &mut slot0_cache,
            )
            .await,
        );
    }
    reads
}

/// Censo del grafo v4 compuesto para UN intent.
///
/// OBSERVABILITY-V4-EDGES-01: existe SEPARADO del guard porque el diagnóstico
/// debe distinguir "grafo vacío" de "router lleno" — el censo es válido aunque
/// el registro no llegue a ocurrir. La ruta ACTIVE lo emite en su summary
/// (una línea por tx, nunca por intent: R9).
#[derive(Debug, Default, Clone)]
pub struct IntentContextCensus {
    pub edges_built: usize,
    pub legs_skipped: std::collections::BTreeMap<&'static str, u64>,
}

/// SHADOW-CANONICAL-01 — resultado de intentar componer y registrar el
/// contexto v4 REAL del intent.
pub struct IntentContextOutcome {
    /// `Some` ⇔ el contexto REAL quedó registrado en el router bajo
    /// `intent-{uuid}`, y su `Drop` lo retira al terminar la evaluación.
    /// `None` ⇔ la evaluación debe caer al contexto DATA_GAP estático; la razón
    /// exacta ya quedó registrada a debug (`cartridge.intent_context_unavailable`).
    pub guard: Option<IntentContextGuard>,
    pub census: IntentContextCensus,
}

impl IntentContextOutcome {
    /// Fallback honesto: sin contexto real, con el censo ya computado.
    fn unavailable(
        census: IntentContextCensus,
        chain_id: u64,
        intent: &RouteIntent,
        reason: &'static str,
    ) -> Self {
        intent_context_unavailable(chain_id, intent, reason);
        Self {
            guard: None,
            census,
        }
    }

    /// `context_id`/`snapshot_id` que los cartuchos v4 deben copiar a su ctx,
    /// o los ids DATA_GAP estáticos cuando no hubo contexto real. `SnapshotServices::check`
    /// exige `ctx["context_id"] == bundle.context_id` y lo mismo para el snapshot.
    pub fn stamp_ids(&self, chain_id: u64) -> (String, String) {
        match &self.guard {
            Some(g) => (g.context_id().to_owned(), g.context_id().to_owned()),
            None => (
                format!("boot-chain-{chain_id}"),
                format!("boot-genesis-{chain_id}"),
            ),
        }
    }
}

/// SHADOW-CANONICAL-01 — compone y REGISTRA el contexto v4 REAL del intent.
///
/// Es la ÚNICA implementación de esta lógica: la ruta ACTIVE y la ruta SHADOW
/// la invocan por igual, de modo que ambas evaluaciones usan el MISMO sustrato
/// de datos (§34.1 hot-path mode-invariant: los modos difieren SÓLO en el
/// terminus de ejecución, jamás en la matemática ni en los datos de entrada).
///
/// Preserva, pieza por pieza, el bloque que la ruta ACTIVE tenía inline:
/// 1. snapshot de la config del operador y (con él) el índice de identidad
///    address-keyed, más el snapshot de precios canónicos;
/// 2. grafo de aristas desde las reservas/slot0/decimales REALES de Redis
///    (orientación exacta por `token0_addr`; omisión honesta con razón propia);
/// 3. estado on-chain de los baskets relevantes (cero RPC sin basket en ruta);
/// 4. `MarketState` para el dispatcher de operadores nativos (sólo aristas V2);
/// 5. gas OBSERVADO del runner (una sola lectura del getter);
/// 6. `build_v4_intent_bundle` (mismos inputs, mismos manifiestos);
/// 7. `SnapshotServices::new` + `with_operator_dispatch` con
///    `V4StructuralInputAdmission`;
/// 8. `router.insert` bajo el `context_id` del intent.
///
/// Coste: UNA invocación por intent (no por cartucho) y dentro del semáforo
/// global que ya acota ambas rutas; las lecturas Redis están cacheadas por
/// intent (reservas/slot0/decimales) o por proceso (identidad, TTL 30s), y la
/// única lectura on-chain (baskets) tiene gate de relevancia + presupuesto.
pub async fn build_and_register_intent_context(
    runner: &Arc<CartridgeRunner>,
    intent: &RouteIntent,
    chain_id: u64,
    cfg_provider: &Arc<crate::orchestrator::ConfigProvider>,
    math_registry: &Arc<math_engine::OperatorRegistry>,
    router: Option<&Arc<crate::context_router::ContextRouter>>,
) -> IntentContextOutcome {
    let ctx_id = format!("intent-{}", uuid::Uuid::new_v4());
    // Snapshot config ONCE for all candidates (same as orchestrator).
    let cfg_snapshot = cfg_provider.snapshot(chain_id).await;
    // ARBX-R-0002: address-keyed token identity, built ONCE per intent and
    // threaded into every candidate. Cartridges emit ADDRESSES in
    // `candidate.token_addresses`; under the legacy symbol-compare every one
    // failed the operator's symbol allowlist (TokenNotAllowed:<addr> 100% —
    // the AGLD/1INCH flood). Identity mode binds (chain_id, address); the 30s
    // cache is shared with the scanner path (same composition site).
    let identity_idx = match cfg_snapshot.as_ref() {
        Some(state) => {
            let mut redis_conn = runner.redis_connection().await;
            Some(crate::token_identity::index_for(&mut redis_conn, chain_id, state).await)
        }
        None => None,
    };
    // FIX (review V2 #9): fetch the live price snapshot ONCE per intent (not per
    // cartridge) and thread it into every candidate evaluation. Empty snapshot
    // degrades to the evaluator's ConfigPriceOracle fallback — never fabricated.
    let price_snapshot: std::collections::HashMap<String, f64> = {
        let mut redis_conn = runner.redis_connection().await;
        let oracle = shared_rs::price_oracle::RedisCachedPriceOracle::snapshot_from_redis(
            &mut redis_conn,
            chain_id,
        )
        .await;
        oracle.into_snapshot()
    };
    // ── AGENT v4 Fase 3a — grafo REAL del intent ──────────────────────────
    // Edges orientados desde las reservas REALES de Redis (orientación EXACTA
    // por token0_addr — jamás heurística de magnitud: R8). Una pierna sin
    // pool, sin reservas, sin token0_addr, sin decimales o degenerada se OMITE
    // con razón explícita.
    let reads = {
        let mut redis_conn = runner.redis_connection().await;
        read_intent_legs(runner, &mut redis_conn, intent, chain_id).await
    };
    let IntentEdgeComposition {
        edges: v4_edges,
        first_token_in: v4_first_token_in,
        skip_reasons: v4_skip_reasons,
    } = compose_intent_edges(chain_id, &ctx_id, intent, &reads);
    // LOGFLOOD-01: omisiones por-pierna a DEBUG con histograma agregado de
    // razones (sin muestreo — R8), una sola línea por intent.
    if !v4_skip_reasons.is_empty() {
        debug!(
            event = "cartridge.v4_intent_legs_skipped",
            chain_id,
            tx_hash = %intent.tx_hash,
            legs_total = intent.legs.len(),
            edges_built = v4_edges.len(),
            ?v4_skip_reasons,
            "piernas omitidas al componer el grafo v4 del intent (omisión honesta, sin heurísticas)"
        );
    }
    let census = IntentContextCensus {
        edges_built: v4_edges.len(),
        legs_skipped: v4_skip_reasons,
    };
    // PROTOCOL-COHERENCE-01 (2026-10-04): si tras la alineación acotada las
    // piernas RESUELTAS siguen repartidas entre dos rounds de sync, el grafo
    // mezcla estado de dos bloques y NINGUNA ruta puede ser atómica: el propio
    // contrato lo detecta en `quote_path_progress`
    // (`mixed_block_or_domain_in_atomic_route`) y con él caen TODOS los recibos
    // de la ruta — medido en el gate v4 como
    // `protocol_exact_quotes::hop_Some(1):mixed_block_or_domain_in_atomic_route`
    // (61 de 269) y `same_snapshot::edges_span_multiple_sync_rounds_or_snapshots`
    // (37), más los cuatro recibos económicos dependientes de la cotización
    // (65 cada uno). Componer el bundle con esa identidad mixta es AFIRMAR una
    // coherencia que el dato desmiente: se declara el contexto NO COMPUTADO con
    // la razón real (R8/R10) en vez de fabricar una identidad única que taparía
    // la incoherencia. El fallback ya existe y es el mismo de `no_graph_edges`.
    if !intent_legs_share_one_round(&reads) {
        let rounds: Vec<Option<u64>> = reads.iter().map(leg_round_ts).collect();
        debug!(
            event = "cartridge.v4_intent_round_unaligned",
            chain_id,
            tx_hash = %intent.tx_hash,
            legs_total = intent.legs.len(),
            edges_built = v4_edges.len(),
            newest_round = ?newest_leg_round(&reads),
            laggards = ?leg_round_laggard_indices(&reads),
            rounds = ?rounds,
            "piernas del intent en rounds de sync distintos tras la alineación acotada: contexto NO COMPUTADO (R8)"
        );
        return IntentContextOutcome::unavailable(
            census,
            chain_id,
            intent,
            "legs_span_multiple_sync_rounds",
        );
    }
    // BASKET-WORKER-01 (2026-10-03): estado on-chain de los baskets ERC-4626
    // RELEVANTES a este intent. La lectura async vive AQUÍ (el llamador) y el
    // bundle sync la recibe ya leída. Gate de coste: sin basket del operador
    // entre los tokens del grafo el mapa queda VACÍO SIN RPC; con RPC caído,
    // timeout o presupuesto vencido el intent sigue igual (R8 fail-honest).
    let v4_basket_state = v4_relevant_basket_state(chain_id, &v4_edges).await;
    // OPERATOR-DISPATCH-WIRING-01 (2026-10-02): MarketState REAL desde los
    // edges ya computados (reservas V2 orientadas + decimales), para adjuntar
    // el dispatcher de operadores nativos al contexto del intent. Sólo edges
    // V2: su precio por reservas es no ambiguo; el precio V3 desde slot0 exige
    // la verificación contra referencia del EXACT-CLASS-01 y queda como
    // seguimiento — jamás alimentar a los operadores un precio posiblemente mal
    // orientado. Sin estado → no se adjunta dispatch (receipts honestos).
    // COST-PRODUCERS-01: la misma lectura de gas (getter; el atómico guarda
    // MILLI-gwei y el getter decodifica) alimenta el MarketState y la línea de
    // gas del bundle.
    let v4_intent_gas_gwei: f64;
    let v4_dispatch_state: Option<std::sync::Arc<math_engine::MarketState>>;
    {
        let block = runner
            .host_block_number_handle()
            .load(std::sync::atomic::Ordering::Relaxed);
        v4_intent_gas_gwei = runner.host_gas_price_gwei();
        // MARKET-FEATURES-WIRE-01: features REALES para el MarketState del
        // dispatch v4. La lectura async vive AQUÍ (el llamador async) y el
        // constructor sync recibe el mapa ya producido — mismo idioma que
        // `v4_basket_state` arriba. Hoy la única fuente viva alcanzable sin
        // tocar la zona congelada es `regime_features_from_redis`
        // (`parity_deviation` desde el PriceBus de Redis); no inserta la clave
        // si ningún stable tiene precio válido, así que el mapa puede llegar
        // VACÍO — y eso es "no computado", nunca un cero fabricado (R8).
        let v4_features = {
            let mut features_redis = runner.redis_connection().await;
            crate::math_evidence::regime_features_from_redis(&mut features_redis, chain_id).await
        };
        v4_dispatch_state =
            v4_market_state_from_edges(&v4_edges, block, v4_intent_gas_gwei, v4_features);
    }
    if v4_edges.is_empty() {
        // Nombre de evento CONSERVADO del bloque original de la ruta ACTIVE (las
        // consultas/greps de producción siguen funcionando; la razón exacta
        // viaja además en `cartridge.intent_context_unavailable`).
        debug!(
            event = "cartridge.v4_intent_no_edges",
            chain_id,
            tx_hash = %intent.tx_hash,
            "grafo v4 vacío tras omisiones R8; la evaluación usa el contexto estático"
        );
        return IntentContextOutcome::unavailable(census, chain_id, intent, "no_graph_edges");
    }
    // Gate de composición. Razón EXACTA por input ausente (ACTIVE-REPAIRS-01:
    // un agregado que no dice QUÉ falta es inobservable) — mismo gate que la
    // ruta ACTIVE tenía inline.
    let Some(cfg) = cfg_snapshot.as_ref() else {
        return IntentContextOutcome::unavailable(
            census,
            chain_id,
            intent,
            "no_trading_config_snapshot",
        );
    };
    let Some(identity) = identity_idx.as_ref() else {
        return IntentContextOutcome::unavailable(
            census,
            chain_id,
            intent,
            "no_token_identity_index",
        );
    };
    let Some(start_token) = v4_first_token_in.as_deref() else {
        return IntentContextOutcome::unavailable(census, chain_id, intent, "no_start_token");
    };
    let Some(router) = router else {
        return IntentContextOutcome::unavailable(census, chain_id, intent, "no_context_router");
    };
    // EXACT-QUOTES-PRODUCER-01 (2026-10-04): quotes EXACTAS por pierna,
    // ENCADENADAS (la salida de la pierna `i` es la entrada de la `i+1`) contra
    // el QuoterV2 real. La lectura async vive AQUÍ — ANTES de que `v4_edges` se
    // mueva al bundle. Sólo entran entradas de FUENTE REAL: una pierna que no
    // cotiza NO inserta nada y su ausencia queda declarada; el
    // `Default::default()` que había en el bundle era un NO COMPUTADO
    // disfrazado de mapa vacío (R8/R10).
    //
    // POR QUÉ NO SE RECHAZA EL CONTEXTO ENTERO cuando una pierna no cotiza: la
    // ausencia ya se DECLARA por pierna (evento agregado con su razón + la
    // entrada que no entra) y el certificado cae al respaldo within-tick con su
    // motivo, que es la conducta de HOY. Rechazar el contexto borraría esa
    // evaluación — las 61 filas medidas de `v3_within_tick_is_hypothesis_…` se
    // miden sobre rutas que SÍ se cotizan y se evalúan: negarlas no arregla
    // ninguna, sólo esconde la medición. Lo que se elimina es la FABRICACIÓN
    // (rellenar el hueco con un valor), no la evaluación honesta con respaldo.
    let v4_exact = {
        let amount_raw = intent.amount_in.to_string();
        match v4_exact_quote_provider(chain_id) {
            Some(provider) => {
                v4_exact_quotes(
                    &v4_edges,
                    &amount_raw,
                    V4_EXACT_QUOTE_BUDGET_MS,
                    move |req| {
                        let provider = provider.clone();
                        async move {
                            use crate::state_projector::V3QuoteProvider as _;
                            match provider
                                .quote_exact_input_single(
                                    req.pool_addr,
                                    req.token_in,
                                    req.token_out,
                                    req.amount_in,
                                    req.fee_bps,
                                )
                                .await
                            {
                                // Salida cero = el quoter contestó y el pool no
                                // entrega nada: NO es una quote exacta usable.
                                Ok(out) if !out.is_zero() => Ok(out.to_string()),
                                Ok(_) => Err("quoter_returned_zero".to_string()),
                                Err(e) => Err(e.to_string()),
                            }
                        }
                    },
                )
                .await
            }
            // Sin pool RPC registrado o sin catálogo de quoter para la cadena: se
            // declara la AUSENCIA DE PRODUCTOR, no se inventa una quote.
            None => {
                v4_exact_quotes(
                    &v4_edges,
                    &amount_raw,
                    V4_EXACT_QUOTE_BUDGET_MS,
                    |_req| async { Err("no_quoter_or_rpc_pool_for_chain".to_string()) },
                )
                .await
            }
        }
    };
    let V4ExactQuotes {
        quotes: v4_exact_quote_map,
        gaps: v4_exact_quote_gaps,
        chained_until_raw: v4_exact_quote_chained,
    } = v4_exact;
    // LOGFLOOD-01: UNA línea agregada por intent (jamás una por pierna). El
    // resumen se arma LEYENDO cada campo del hueco: el detalle crudo del quoter
    // es lo que hace diagnosticable la ausencia (y no queda código muerto).
    if !v4_exact_quote_gaps.is_empty() {
        let gap_summary: Vec<String> = v4_exact_quote_gaps
            .iter()
            .map(|g| {
                format!(
                    "hop{}:{}:{}:{}:{}",
                    g.index, g.edge_id, g.protocol, g.reason, g.detail
                )
            })
            .collect();
        debug!(
            event = "cartridge.v4_exact_quotes_absent",
            chain_id,
            tx_hash = %intent.tx_hash,
            edges_built = v4_edges.len(),
            quoted = v4_exact_quote_map.len(),
            chained_until_raw = %v4_exact_quote_chained,
            gaps = ?gap_summary,
            "piernas sin quote exacta de protocolo: la entrada NO se inserta y la ausencia queda declarada (R8)"
        );
    }
    // PRICE-COVERAGE-01 (§38): tercera fuente del mapa de precios — el bus de
    // precios EN PROCESO. `get()` clona un `Arc` ya inicializado o devuelve
    // `None` cuando `main.rs` todavía no llamó a `init()` (tests, arranques
    // parciales); con `None` la composición se queda con las dos fuentes de
    // Redis/config y no inventa nada (R8). Coste: un load atómico, sin I/O.
    let price_bus = crate::price_bus_global::get();
    let Some(bundle) = build_v4_intent_bundle(
        chain_id,
        &ctx_id,
        v4_edges,
        start_token,
        &intent.amount_in.to_string(),
        cfg,
        identity,
        &price_snapshot,
        v4_manifest_digests(),
        v4_intent_gas_gwei,
        &v4_basket_state,
        price_bus.as_deref(),
        v4_exact_quote_map,
    ) else {
        // `None` sólo si el reloj no permite una ventana temporal honesta.
        // Evento CONSERVADO del bloque original de la ruta ACTIVE.
        debug!(
            event = "cartridge.v4_intent_clock_invalid",
            chain_id,
            tx_hash = %intent.tx_hash,
            "sin base temporal honesta para el bundle; contexto estático"
        );
        return IntentContextOutcome::unavailable(census, chain_id, intent, "bundle_clock_invalid");
    };
    // Guarda de revisión de un solo bundle: este intent sirve exactamente el
    // contexto que acaba de componer (mismo enfoque honesto del contexto
    // DATA_GAP Phase-1).
    let v4_revision: crate::snapshot_services::RevisionGuard = Arc::new(|_: &str, _: &str| true);
    let services =
        match crate::snapshot_services::SnapshotServices::new(Arc::new(bundle), v4_revision) {
            Ok(services) => services,
            Err(e) => {
                warn!(
                    event = "cartridge.v4_intent_bundle_rejected",
                    chain_id,
                    tx_hash = %intent.tx_hash,
                    reason = %e,
                    "bundle v4 del intent rechazado; fallback a DATA_GAP sin romper el flujo"
                );
                return IntentContextOutcome::unavailable(
                    census,
                    chain_id,
                    intent,
                    "bundle_rejected",
                );
            }
        };
    // OPERATOR-DISPATCH-WIRING-01: adjuntar el dispatcher del registro nativo
    // cuando hay MarketState real. Sin estado los servicios quedan sin dispatch
    // y los recibos nativos siguen FAIL honesto.
    let services = match v4_dispatch_state {
        Some(state) => {
            let registry = math_registry.clone();
            services.with_operator_dispatch(Arc::new(
                move |ctx: &serde_json::Value,
                      spec: &serde_json::Value,
                      cand: &serde_json::Value|
                      -> Result<serde_json::Value, String> {
                    // Identidad del plan/snapshot desde el ctx y el candidato
                    // del propio flujo.
                    let snapshot_id = ctx
                        .get("snapshot_id")
                        .and_then(|v| v.as_str())
                        .unwrap_or_default()
                        .to_string();
                    let plan_hash = cand
                        .get("plan_hash")
                        .and_then(|v| v.as_str())
                        .unwrap_or_default()
                        .to_string();
                    crate::native_operator_adapter::evaluate_declared(
                        &registry,
                        &state,
                        spec,
                        &snapshot_id,
                        &plan_hash,
                        &V4StructuralInputAdmission,
                        crate::operator_toggles::is_disabled,
                    )
                },
            ))
        }
        None => services,
    };
    match router.insert(ctx_id.clone(), Arc::new(services)) {
        Ok(()) => IntentContextOutcome {
            guard: Some(IntentContextGuard {
                router: Some(router.clone()),
                context_id: ctx_id,
            }),
            census,
        },
        Err(e) => {
            debug!(
                event = "cartridge.v4_intent_insert_failed",
                chain_id,
                tx_hash = %intent.tx_hash,
                reason = %e,
                "router sin capacidad para el contexto del intent; contexto estático"
            );
            IntentContextOutcome::unavailable(census, chain_id, intent, "router_insert_failed")
        }
    }
}

/// SHADOW-CANONICAL-01 — razón EXACTA por la que una ruta evaluó contra el
/// contexto DATA_GAP estático en lugar del bundle real del intent. `debug!`
/// (no `info!`): corre por intent en el block scanner (LOGFLOOD-01).
fn intent_context_unavailable(chain_id: u64, intent: &RouteIntent, reason: &'static str) {
    debug!(
        event = "cartridge.intent_context_unavailable",
        chain_id,
        tx_hash = %intent.tx_hash,
        reason,
        "sin contexto v4 real para este intent; la evaluación usa el stub DATA_GAP (razón exacta)"
    );
}

/// ACTIVE MODE — evaluate cartridges and emit real StrategyCandidates through the full pipeline.
///
/// This is the FASE OMEGA follow-up that wires cartridge evaluation → execution:
/// 1. Evaluate all pertinent active cartridges against the intent
/// 2. For each `CartridgeEvalResult` with `is_opportunity=true`:
///    a. Transform into `StrategyCandidate` (Opportunity + OpportunityCandidate + RoutePlan)
///    b. Call `process_candidate` (same pipeline as native engines)
///    c. Emit via OpportunityEmitter → Redis/Postgres → /api/opportunities/live
///
/// Unlike `shadow_evaluate_intent`, this path:
/// - Builds real `StrategyCandidate` structs (not just telemetry)
/// - Passes through `ConfigAwareEvaluator` (gates: min_profit, min_roi, strategy_enabled)
/// - Persists to Postgres and publishes to Redis stream `arbx:opps:detected`
/// - Appears in the dashboard at /opportunities
///
/// R8 fail-honest: unknown figures are `None`, never fabricated. Candidates with
/// missing data (no pool_hint, no token addresses) are rejected with explicit reason.
#[allow(clippy::too_many_arguments)] // 10 params: eval pipeline + STRAT-IDENT-01 evidence deps + router v4
pub async fn active_evaluate_and_emit(
    runner: Arc<CartridgeRunner>,
    intent: RouteIntent,
    chain_id: u64,
    emitter: Arc<crate::opportunity_emitter::OpportunityEmitter>,
    cfg_provider: Arc<crate::orchestrator::ConfigProvider>,
    size_optimizer: Arc<crate::size_optimizer::SizeOptimizer>,
    ctx_chain_id: u64,
    math_registry: Arc<math_engine::OperatorRegistry>,
    reserves_cache: Arc<crate::engines::triangular_engine::ReservesCache>,
    v4_router: Option<Arc<crate::context_router::ContextRouter>>,
) {
    use crate::engines::StrategyCandidate;
    use crate::metrics::REJECTED_NO_PROFIT_TOTAL;
    use crate::size_optimizer::{OptimizeOutcome, OptimizeRejectReason};
    use ethers::types::{Address, U256};
    use shared_rs::contracts::{Opportunity, StrategyKind};
    use uuid::Uuid;

    info!(
        event = "cartridge.active_eval_enter",
        chain_id,
        tx_hash = %intent.tx_hash,
        "ACTIVE evaluation entered"
    );

    // Bound global active-eval concurrency; drop (don't queue) when at capacity.
    let _permit = match shadow_semaphore().clone().try_acquire_owned() {
        Ok(p) => p,
        Err(_) => {
            debug!(
                event = "cartridge.active_eval_skipped",
                chain_id,
                tx_hash = %intent.tx_hash,
                "active eval at capacity; dropping (would overwhelm hot path)"
            );
            return;
        }
    };

    // Get active cartridges pertinent to this intent. STRAT-IDENT-01: keep the
    // strategy's DECLARED operator combo (primary/secondary from
    // `init_strategy()`, mirroring the canonical STRATEGY.json) — each strategy
    // says which structures apply to IT; no class-level flattening.
    let actives: Vec<(String, String, Vec<u32>, Vec<u32>)> = runner
        .list_cartridges()
        .await
        .into_iter()
        .filter(|(_, _, state)| *state == CartridgeState::Active)
        .map(|(id, meta, _)| {
            (
                id,
                meta.category,
                meta.primary_operators,
                meta.secondary_operators,
            )
        })
        .collect();

    info!(
        event = "cartridge.active_eval_actives",
        chain_id,
        tx_hash = %intent.tx_hash,
        active_count = actives.len(),
        "active cartridges counted"
    );

    if actives.is_empty() {
        return;
    }

    // Smart routing: only evaluate cartridges pertinent to this intent shape
    let pertinent: Vec<(String, String, Vec<u32>, Vec<u32>)> = actives
        .into_iter()
        .filter(|(_, category, _, _)| cartridge_matches_intent(category, &intent))
        .collect();

    info!(
        event = "cartridge.active_eval_pertinent",
        chain_id,
        tx_hash = %intent.tx_hash,
        pertinent_count = pertinent.len(),
        source_event = %intent.source_event.as_str(),
        "pertinent cartridges for this intent"
    );

    if pertinent.is_empty() {
        debug!(
            event = "cartridge.active_eval_no_pertinent",
            chain_id,
            tx_hash = %intent.tx_hash,
            source_event = %intent.source_event.as_str(),
            "no active cartridge is pertinent to this intent source; skipping"
        );
        return;
    }

    // Enrich pool_data with reserves for reserve-dependent cartridges
    let reserves_source = match intent.legs.first().and_then(|l| l.pool_hint) {
        Some(p) => runner.read_pool_reserves(&format!("{:#x}", p)).await,
        None => None,
    };
    // Fase 3a: `mut` porque la ruta ACTIVE sella context_id/snapshot_id del
    // contexto por-intent (ver bloque v4 más abajo) antes de evaluar.
    let mut pool_data = build_cartridge_pool_data(&intent, reserves_source.as_ref());

    // Snapshot config once for all candidates (same as orchestrator)
    let cfg_snapshot = cfg_provider.snapshot(chain_id).await;

    // ARBX-R-0002: address-keyed token identity, built ONCE per intent and
    // threaded into every candidate. Cartridges emit ADDRESSES in
    // `candidate.token_addresses`; under the legacy symbol-compare every
    // one failed the operator's symbol allowlist (TokenNotAllowed:<addr>
    // 100% — the AGLD/1INCH flood, 112×1INCH + 6×USDC in 6h). Identity
    // mode binds (chain_id, address); the 30s cache is shared with the
    // scanner path (same composition site).
    let identity_idx = match cfg_snapshot.as_ref() {
        Some(state) => {
            let mut redis_conn = runner.redis_connection().await;
            Some(crate::token_identity::index_for(&mut redis_conn, chain_id, state).await)
        }
        None => None,
    };

    // FIX (review V2 #9): fetch the live price snapshot ONCE per intent (not per
    // cartridge) and thread it into every candidate evaluation. Empty snapshot
    // degrades to the evaluator's ConfigPriceOracle fallback — never fabricated.
    let price_snapshot: std::collections::HashMap<String, f64> = {
        let mut redis_conn = runner.redis_connection().await;
        let oracle = shared_rs::price_oracle::RedisCachedPriceOracle::snapshot_from_redis(
            &mut redis_conn,
            chain_id,
        )
        .await;
        oracle.into_snapshot()
    };

    // ── AGENT v4 Fase 3a — SnapshotBundle REAL por intent (ContextRouter) ──
    // SHADOW-CANONICAL-01 (2026-10-03): la composición del grafo, el bundle
    // real, los servicios v4 y el registro del contexto viven AHORA en
    // `build_and_register_intent_context`, compartido con la ruta SHADOW. Las
    // dos rutas evalúan por tanto contra el MISMO sustrato de datos (reservas/
    // slot0/decimales reales, precios canónicos, gas observado, líneas de coste
    // del operador, dispatcher de operadores nativos): §34.1 exige que los
    // modos difieran SÓLO en el terminus de ejecución. Sin contexto real la
    // evaluación continúa contra el contexto DATA_GAP estático (ids
    // boot-chain/boot-genesis), que responde con razones honestas, y la razón
    // exacta queda registrada a debug por el propio helper.
    let v4_context = build_and_register_intent_context(
        &runner,
        &intent,
        chain_id,
        &cfg_provider,
        &math_registry,
        v4_router.as_ref(),
    )
    .await;
    // Sustrato REALMENTE usado: la telemetría de outcomes se comparte con la
    // ruta SHADOW (mode-invariant, BUG-003/RD-06), así que también aquí se
    // declara si la evaluación usó el bundle real o el stub DATA_GAP.
    let v4_registered = v4_context.guard.is_some();
    let v4_substrate = if v4_registered {
        ShadowSubstrate::IntentBundle
    } else {
        ShadowSubstrate::StaticBootStub
    };
    // OBSERVABILITY-V4-EDGES-01: censo del grafo compuesto para ESTE intent —
    // válido aunque el registro no ocurriera (distingue "grafo vacío" de
    // "router lleno"). Se emite en el summary de abajo, una línea por tx.
    let v4_edges_built = v4_context.census.edges_built;
    let v4_skip_reasons = v4_context.census.legs_skipped.clone();
    // Sello de identidad del contexto para ESTA evaluación: los cartuchos v4
    // copian estos campos a su ctx y SnapshotServices::check exige
    // ctx["context_id"] == bundle.context_id && ctx["snapshot_id"] ==
    // bundle.snapshot_id. Registrado → ids del intent; fallback → ids
    // estáticos DATA_GAP (coinciden con el contexto registrado en boot).
    // Se computa ANTES de mover el guard (toma `&self`).
    let (v4_stamp_context, v4_stamp_snapshot) = v4_context.stamp_ids(chain_id);
    // Guard con Drop: retira el contexto por-intent del router en TODA salida
    // de la tarea (incluye panic) — sin fugas de capacidad del router.
    let _v4_intent_guard = v4_context.guard;

    pool_data.insert("context_id".into(), rhai::Dynamic::from(v4_stamp_context));
    pool_data.insert("snapshot_id".into(), rhai::Dynamic::from(v4_stamp_snapshot));

    // LOGFLOOD-01: per-cartridge negatives below log at DEBUG (was INFO —
    // ~183 lines/s evicted every other log from the 50MB docker rotation
    // window). One post-loop INFO summary preserves the full per-reason
    // histogram (R8 fail-honest: reasons kept, never sampled).
    let pertinent_count = pertinent.len();
    let mut negative_reasons: std::collections::BTreeMap<String, u64> =
        std::collections::BTreeMap::new();
    // ACTIVE-REPAIRS-01 (2026-10-03): histograma de PARES (campo::razón) de los
    // repairs v4 — el top-level `reason` es un agregado ("applicable_data_or_
    // constraint_gap") que NO dice QUÉ requisito falta. Medición que lo destapó:
    // el stream `arbx:route_discovery:outcomes` solo contiene la ruta SHADOW
    // (mode=shadow, stub estático Phase-1, 100% de las filas), mientras la ruta
    // ACTIVE —la única con bundles reales, costes y precios— publica únicamente
    // este summary. Sin el drill-down aquí, los bloqueos reales de la ruta
    // activa eran inobservables: cualquier histograma del stream describía el
    // stub, no la evaluación real.
    let mut negative_repairs: std::collections::BTreeMap<String, u64> =
        std::collections::BTreeMap::new();
    let mut negative_total: u64 = 0;
    let mut positive_total: u64 = 0;

    // ── EVIDENCE-WIRING-01 (paso 2/2, 2026-10-01) ───────────────────────────
    // Ultimo instante (epoch secs) en que se publico evidencia por cartucho. El
    // bucle corre por cada intent sobre 269 cartuchos, asi que sin este registro
    // serian 269 escrituras Redis POR TRANSACCION (diluvio, R9). La decision vive
    // en `should_publish_evidence`, que es pura y esta cubierta por tests.
    fn evidence_publish_last() -> &'static std::sync::Mutex<std::collections::HashMap<String, u64>>
    {
        static LAST: std::sync::OnceLock<std::sync::Mutex<std::collections::HashMap<String, u64>>> =
            std::sync::OnceLock::new();
        LAST.get_or_init(|| std::sync::Mutex::new(std::collections::HashMap::new()))
    }

    for (cartridge_id, category, declared_primary_ops, declared_secondary_ops) in pertinent {
        // ── EVIDENCE-WIRING-01: publicar la evidencia declarada de ESTE cartucho
        // AQUI, en el bucle de EVALUACION y ANTES del gate de dispatch de abajo.
        //
        // Antes el unico call site vivia dentro del bloque que construye el
        // `Opportunity` de cartucho (ruta de emision de candidato). Con
        // `positive=0` en cada tx ese bloque NUNCA corre, asi que la evidencia
        // por-cartucho nunca se publicaba y en Redis solo existia la clave
        // regime-keyed (`arbx:math_evidence:1:dex_arb`). Es una dependencia
        // circular: la evidencia por-cartucho requiere un positivo, el positivo
        // requiere operadores computando, y esa evidencia es justo lo que
        // serviria para diagnosticarlo.
        //
        // Ponerlo ANTES del gate importa: los 174 `dispatch_needs_route_data`
        // hacen `continue` abajo sin evaluarse, y son precisamente los que mas
        // necesitan quedar diagnosticables.
        //
        // R9: el throttle limita a una publicacion por cartucho cada
        // EVIDENCE_PUBLISH_SECS (muy por debajo del TTL de 120s de la clave), de
        // modo que la clave se mantiene viva con ~4.5 escrituras/s para 269
        // cartuchos (269/60) en vez de 269 por transaccion.
        {
            const EVIDENCE_PUBLISH_SECS: u64 = 60;
            let due = {
                let mut last = evidence_publish_last()
                    .lock()
                    .unwrap_or_else(|poisoned| poisoned.into_inner());
                crate::math_evidence::should_publish_evidence(
                    &mut last,
                    &cartridge_id,
                    chrono::Utc::now().timestamp().max(0) as u64,
                    EVIDENCE_PUBLISH_SECS,
                )
            };
            if due {
                let runner_ev = runner.clone();
                let registry_ev = math_registry.clone();
                let reserves_ev = reserves_cache.clone();
                // MATH-04-FOLLOWUP: el par de tokens viaja con el pool — sin el,
                // `build_market_state` no puede normalizar la price_matrix y los
                // operadores devuelven `scalar: null`.
                let pools_ev: Vec<(Address, Address, Address)> = intent
                    .legs
                    .iter()
                    .filter_map(|l| l.pool_hint.map(|p| (p, l.token_in, l.token_out)))
                    .collect();
                let strategy_key_ev = cartridge_id.clone();
                let primary_ev = declared_primary_ops.clone();
                let secondary_ev = declared_secondary_ops.clone();
                let chain_ev = chain_id;
                tokio::spawn(async move {
                    let mut redis_ev = runner_ev.redis_connection().await;
                    let block_number = runner_ev
                        .host_block_number_handle()
                        .load(std::sync::atomic::Ordering::Relaxed);
                    // CORE-01/MATH-01: el atomico guarda MILLI-gwei (wei/1e6);
                    // `host_gas_price_gwei()` ya decodifica ÷1e3.
                    let base_fee_gwei = runner_ev.host_gas_price_gwei();
                    crate::math_evidence::publish_declared_combo_evidence(
                        &reserves_ev,
                        &registry_ev,
                        &mut redis_ev,
                        &pools_ev,
                        chain_ev,
                        &strategy_key_ev,
                        &primary_ev,
                        &secondary_ev,
                        base_fee_gwei,
                        block_number,
                        0, // block_timestamp — no viaja en el intent (observe-only)
                    )
                    .await;
                });
            }
        }

        // ── CORE-04 fix (2026-09-24): apply the workbook dispatch doctrine ──
        // (strategy_dispatch_status.rs) on the CANDIDATE path too. Previously
        // only route_discovery_worker consulted it — a NEEDS_ROUTE_DATA or
        // NO_COMPATIBLE_ROUTE cartridge with a permissive Execution_Class
        // could form a candidate here, bypassing the workbook's "NEEDS_ROUTE_
        // DATA nunca fabrica ruta" doctrine. Fail-closed: unknown MEV ids pass
        // (the workbook covers the 264; root masters + custom are exempt).
        let mev_id = crate::signal_tier::mev_id_from_cartridge_id(&cartridge_id);
        if let Some(mev_id) = &mev_id {
            let disposition = crate::strategy_dispatch_status::disposition(mev_id);
            if !disposition.may_form_candidate() {
                debug!(
                    event = "cartridge.active_dispatch_blocked",
                    chain_id,
                    cartridge_id = %cartridge_id,
                    mev_id = %mev_id,
                    reason = disposition.reason(),
                    "workbook dispatch status blocks candidate formation on the cartridge path"
                );
                let reason_key = format!("dispatch_{}", disposition.reason());
                *negative_reasons.entry(reason_key).or_insert(0) += 1;
                negative_total += 1;
                continue;
            }
        }

        match runner.evaluate(&cartridge_id, pool_data.clone()).await {
            Ok(eval_result) => {
                // BUG-003/RD-06/ARBX-0024 (2026-08-31): telemetry is hot-path
                // MODE-INVARIANT (§34.1) — the route-discovery outcomes dataset
                // must accrue in Active exactly as in Shadow; only the execution
                // terminus differs. This path previously consumed the resolved
                // result and never persisted an outcome row, so
                // XLEN arbx:route_discovery:outcomes stayed 0 while cartridges
                // ran Active (the S4 calibration labels starve). Same gate
                // (ARBX_ROUTE_DISCOVERY_OUTCOMES), same stream, fire-and-forget.
                emit_shadow_outcome(
                    &runner,
                    chain_id,
                    &cartridge_id,
                    &intent,
                    &eval_result,
                    reserves_source.is_some(),
                    v4_substrate,
                )
                .await;

                if !eval_result.is_opportunity {
                    debug!(
                        event = "cartridge.active_eval_negative",
                        chain_id,
                        cartridge_id = %cartridge_id,
                        reason = ?eval_result.reason,
                        "cartridge active eval: no opportunity"
                    );
                    let reason_key = eval_result
                        .reason
                        .clone()
                        .unwrap_or_else(|| "none".to_string());
                    *negative_reasons.entry(reason_key).or_insert(0) += 1;
                    // ACTIVE-REPAIRS-01: drill-down por (campo::razón) del
                    // proposal sellado de ESTA evaluación negativa.
                    for r in v4_repairs_summary(&eval_result)["repairs"]
                        .as_array()
                        .map(Vec::as_slice)
                        .unwrap_or_default()
                    {
                        let field = r["field"].as_str().unwrap_or("?");
                        let why = r["reason"].as_str().unwrap_or("?");
                        *negative_repairs
                            .entry(format!("{field}::{why}"))
                            .or_insert(0) += 1;
                    }
                    negative_total += 1;
                    continue;
                }
                // ── AGENT v4 interception (integration/agent-cartridges-v4) ──────
                // A v4 proposal must NEVER flow into the v3 candidate path: the
                // v3 adapter rebuilds the plan from intent.legs (RHAI-12) and
                // sizes with the legacy f64 contract. Until the snapshot store
                // + plan-identity consumer ((plan_hash, snapshot_id,
                // amount_in_raw) lookup) is wired, an eligible v4 CANDIDATE is
                // emitted as an honest REJECTED row with its exact status —
                // observe-only, no fabricated candidate, no silent drop (R8).
                if eval_result.metadata.contains_key("proposal_v4") {
                    let v4_status = eval_result
                        .metadata
                        .get("proposal_v4")
                        .and_then(|p| p.get("status"))
                        .and_then(|s| s.as_str())
                        .unwrap_or("UNKNOWN");
                    let v4_reason = format!("agent_v4_{v4_status}_snapshot_store_not_wired");
                    info!(
                        event = "cartridge.active_v4_intercepted",
                        chain_id,
                        cartridge_id = %cartridge_id,
                        v4_status,
                        "v4 proposal intercepted: awaiting snapshot-store consumer (no v3 candidate formed)"
                    );
                    // Honest rejection row WITHOUT the v3 candidate machinery:
                    // identity from the intent (causal origin). The economics
                    // surfaced are the PROPOSAL'S OWN validated figures
                    // (gross/net/amount — decimal strings parsed to f64; R8:
                    // absent/unparseable stays None, never invented). The
                    // api-server SIM-TS then composes the full cost ladder
                    // from these + trading_config (prices/targets seeded).
                    let proposal_v4_json = eval_result
                        .metadata
                        .get("proposal_v4")
                        .cloned()
                        .unwrap_or(serde_json::Value::Null);
                    let v4_amount_raw = proposal_v4_json
                        .get("amount_in_raw")
                        .and_then(|v| v.as_str())
                        .map(String::from);
                    let v4_gross_usd = proposal_v4_json
                        .get("gross_profit_usd")
                        .and_then(|v| v.as_str())
                        .and_then(|s| s.parse::<f64>().ok());
                    let v4_net_usd = proposal_v4_json
                        .get("net_profit_usd")
                        .and_then(|v| v.as_str())
                        .and_then(|s| s.parse::<f64>().ok());
                    let first_leg_v4 = intent.legs.first();
                    let last_leg_v4 = intent.legs.last();
                    let (token_in_v4, token_out_v4) = match (first_leg_v4, last_leg_v4) {
                        (Some(f), Some(l)) => {
                            (format!("{:#x}", f.token_in), format!("{:#x}", l.token_out))
                        }
                        _ => (String::new(), String::new()),
                    };
                    let opp_v4 = shared_rs::contracts::Opportunity {
                        id: uuid::Uuid::new_v4(),
                        chain_id,
                        strategy_kind: shared_rs::contracts::StrategyKind::cartridge(
                            cartridge_id.clone(),
                        ),
                        dex_a: first_leg_v4
                            .and_then(|l| l.dex_hint.clone())
                            .unwrap_or_else(|| "unknown".to_string()),
                        dex_b: last_leg_v4.and_then(|l| l.dex_hint.clone()),
                        pair_symbol: format!("{}/{}", token_in_v4, token_out_v4),
                        token_in: token_in_v4,
                        token_out: token_out_v4,
                        amount_in_wei: v4_amount_raw
                            .unwrap_or_else(|| intent.amount_in.to_string()),
                        expected_profit_usd: v4_gross_usd,
                        net_expected_profit_usd: v4_net_usd,
                        roi_pct: None,
                        risk_score: None,
                        block_number: intent.observed_block().or_else(|| {
                            let head = runner
                                .host_block_number_handle()
                                .load(std::sync::atomic::Ordering::Relaxed);
                            (head > 0).then_some(head)
                        }),
                        rejection_reason: Some(v4_reason.clone()),
                        cartridge_id: Some(cartridge_id.clone()),
                        detector_id: Some(cartridge_id.clone()),
                        pipeline_latency_ms: None,
                        detected_at: chrono::Utc::now(),
                        trace_id: uuid::Uuid::new_v4(),
                        economics: None,
                    };
                    if let Err(e) = emitter
                        .emit_rejected(
                            &opp_v4,
                            crate::strategy_label::StrategyLabel::DexArbV2V2,
                            &v4_reason,
                            None,
                        )
                        .await
                    {
                        warn!(
                            event = "cartridge.v4_emit_rejected_failed",
                            chain_id,
                            cartridge_id = %cartridge_id,
                            error = %e,
                            "failed to emit v4 interception rejection"
                        );
                    }
                    *negative_reasons.entry(v4_reason).or_insert(0) += 1;
                    negative_total += 1;
                    continue;
                }

                positive_total += 1;

                info!(
                    event = "cartridge.active_opportunity_detected",
                    chain_id,
                    tx_hash = %intent.tx_hash,
                    cartridge_id = %cartridge_id,
                    estimated_profit = eval_result.estimated_profit,
                    confidence = eval_result.confidence,
                    urgency = %eval_result.urgency,
                    "cartridge ACTIVE OPPORTUNITY — wiring to StrategyCandidate pipeline"
                );

                // ── Transform CartridgeEvalResult → StrategyCandidate ──────────
                // Build the Opportunity row for PG + Redis emission.
                // R8: token_in/out from intent legs (fail-honest when missing).
                let first_leg = intent.legs.first();
                let last_leg = intent.legs.last();
                let (token_in, token_out) = match (first_leg, last_leg) {
                    (Some(f), Some(l)) => {
                        (format!("{:#x}", f.token_in), format!("{:#x}", l.token_out))
                    }
                    _ => {
                        warn!(
                            event = "cartridge.active_rejected_no_legs",
                            chain_id,
                            cartridge_id = %cartridge_id,
                            "rejected: intent has no legs for token_in/out"
                        );
                        continue;
                    }
                };

                let opportunity = Opportunity {
                    id: Uuid::new_v4(),
                    chain_id,
                    strategy_kind: StrategyKind::cartridge(cartridge_id.clone()),
                    dex_a: first_leg
                        .and_then(|l| l.dex_hint.clone())
                        .unwrap_or_else(|| "unknown".to_string()),
                    dex_b: last_leg.and_then(|l| l.dex_hint.clone()),
                    pair_symbol: format!("{}/{}", token_in, token_out),
                    token_in,
                    token_out,
                    amount_in_wei: intent.amount_in.to_string(),
                    // RC-2 unit-scale fix: `estimated_profit` is TOKEN UNITS, not USD
                    // (types.rs:45 doc; runner.rs:564 passes it raw). dex_engine.rs
                    // compute_gross_usd (5e9d222) does the units->USD step this path skipped.
                    // The cartridge already did wei/10^dec, so do NOT divide again.
                    // Prefer its priced `profit_usd_hint`; else None (R8) -- NEVER token-as-USD.
                    expected_profit_usd: eval_result
                        .metadata
                        .get("profit_usd_hint")
                        .and_then(|v| v.as_f64())
                        .filter(|p| *p > 0.0),
                    net_expected_profit_usd: None, // Filled by spine evaluator
                    roi_pct: None,
                    risk_score: None,
                    // §30 contract: prefer the intent's observed block; fall
                    // back to the cartridge host's live head atomic (the same
                    // handle the §IV evidence publisher below reads) so the row
                    // is never persisted without a block anchor when a head is
                    // known. 0 = genuinely no head observed yet (R8: keep None).
                    block_number: intent.observed_block().or_else(|| {
                        let head = runner
                            .host_block_number_handle()
                            .load(std::sync::atomic::Ordering::Relaxed);
                        (head > 0).then_some(head)
                    }),
                    rejection_reason: None,
                    cartridge_id: Some(cartridge_id.clone()),
                    // WO-CARDS-COMPLETE-01 (2026-09-17): cartridge detector
                    // identity — the cartridge stem/id itself (the struct's
                    // canonical cartridge identity), stamped at construction.
                    detector_id: Some(cartridge_id.clone()),
                    pipeline_latency_ms: None,
                    detected_at: chrono::Utc::now(),
                    trace_id: Uuid::new_v4(), // Generate new trace ID for cartridge path
                    economics: None,
                };

                // ── STRAT-IDENT-01: publish THIS strategy's declared-combo §IV
                // evidence (observe-only, detached — zero hot-path latency).
                // The strategy declares its applicable operators; we evaluate
                // exactly that combo and key the snapshot by the strategy's own
                // identity — the canonical key the emitter reads and the
                // api-server /api/math/evidence route serves.
                {
                    let runner_ev = runner.clone();
                    let registry_ev = math_registry.clone();
                    let reserves_ev = reserves_cache.clone();
                    // MATH-04-FOLLOWUP (2026-09-30): conservar el PAR de tokens de
                    // cada pierna, no solo el pool — mismo fix que en
                    // orchestrator.rs. `RouteIntentLeg` ya los lleva; sin ellos la
                    // price_matrix solo puede llevar el ratio crudo r1/r0 y los
                    // operadores devuelven `scalar: null`.
                    let pools_ev: Vec<(Address, Address, Address)> = intent
                        .legs
                        .iter()
                        .filter_map(|l| l.pool_hint.map(|p| (p, l.token_in, l.token_out)))
                        .collect();
                    let strategy_key_ev = cartridge_id.clone();
                    let primary_ev = declared_primary_ops.clone();
                    let secondary_ev = declared_secondary_ops.clone();
                    let chain_ev = chain_id;
                    tokio::spawn(async move {
                        let mut redis_ev = runner_ev.redis_connection().await;
                        let block_number = runner_ev
                            .host_block_number_handle()
                            .load(std::sync::atomic::Ordering::Relaxed);
                        // CORE-01/MATH-01 (t_26eaafc3): the host atomic stores
                        // MILLI-gwei (wei/1e6, see GasBlockSink.publish_head) —
                        // decode ÷1e3, not ÷1e9 (1e6× off before this fix).
                        let base_fee_gwei = runner_ev.host_gas_price_gwei();
                        crate::math_evidence::publish_declared_combo_evidence(
                            &reserves_ev,
                            &registry_ev,
                            &mut redis_ev,
                            &pools_ev,
                            chain_ev,
                            &strategy_key_ev,
                            &primary_ev,
                            &secondary_ev,
                            base_fee_gwei,
                            block_number,
                            0, // block_timestamp — not carried on the intent (observe-only)
                        )
                        .await;
                    });
                }

                let Some(token_path) = intent.token_path() else {
                    warn!(
                        event = "cartridge.invalid_token_path",
                        chain_id,
                        cartridge_id = %cartridge_id,
                        "candidate has discontinuous token geometry"
                    );
                    continue;
                };

                // Build OpportunityCandidate for ConfigAwareEvaluator
                // Uses the real prioritization_spine::types::OpportunityCandidate shape
                let candidate = prioritization_spine::types::OpportunityCandidate {
                    // Intra-tx index keeps same-pair multi-swap UR txs from
                    // colliding into one deduped observation.
                    route_fingerprint: format!(
                        "{}:{}:{}",
                        cartridge_id, intent.tx_hash, intent.intra_tx_index
                    ),
                    pool_addresses: intent
                        .legs
                        .iter()
                        .filter_map(|l| l.pool_hint.map(|p| format!("{:#x}", p)))
                        .collect(),
                    token_addresses: token_path
                        .iter()
                        .map(|token| format!("{token:#x}"))
                        .collect(),
                    dex_adapters: intent
                        .legs
                        .iter()
                        .filter_map(|l| l.dex_hint.clone())
                        .collect(),
                    amount_in: intent.amount_in.as_u128() as f64,
                    expected_amount_out: 0.0, // Unknown at this layer; spine evaluator computes
                    gross_profit: eval_result
                        .metadata
                        .get("profit_usd_hint")
                        .and_then(|v| v.as_f64())
                        .filter(|p| *p > 0.0)
                        .unwrap_or(0.0),
                };

                // Build minimal RoutePlan for the evaluator
                // Uses the real prioritization_spine::route_plan::RoutePlan shape
                let route_plan = prioritization_spine::route_plan::RoutePlan {
                    route_id: Some(format!("{}-{}", cartridge_id, uuid::Uuid::new_v4())),
                    strategy_kind: format!("cartridge_{}", category),
                    chain_id,
                    legs: intent
                        .legs
                        .iter()
                        .map(|leg| {
                            prioritization_spine::route_plan::RouteLeg {
                                dex_id: leg
                                    .dex_hint
                                    .clone()
                                    .unwrap_or_else(|| "unknown".to_string()),
                                dex_name: leg
                                    .dex_hint
                                    .clone()
                                    .unwrap_or_else(|| "unknown".to_string()),
                                protocol_type: format!("{:?}", leg.protocol_type),
                                // Unknown at cartridge layer — empty like every
                                // other producer (scanner.rs/dex_engine.rs). The
                                // zero address is a sentinel (RULE 02) and
                                // build_route_metadata_from_plan's factory
                                // fallback would persist it as a fake pool on
                                // mempool-decoded rows (pool_hint is never set
                                // by the decoder — HOPS-EMIT-01 review note).
                                factory_address: String::new(),
                                pool_id: None,
                                pool_address: leg.pool_hint.map(|p| format!("{:#x}", p)),
                                token_in: format!("{:#x}", leg.token_in),
                                token_out: format!("{:#x}", leg.token_out),
                                fee_bps: leg.fee_bps,
                                // HOPS-LEDGER-04: the f64 cast of raw wei loses
                                // precision above 2^53 and no reader consumes
                                // these leg-level fields (verified) — honest
                                // None beats a lossy figure. The real per-leg
                                // wei rides RouteMetadata.leg_amounts_*.
                                amount_in: None,
                                amount_out: None,
                                tvl_usd: None,
                                volume_24h_usd: None,
                                pool_is_active: true,
                            }
                        })
                        .collect(),
                    atomic: true,
                    estimated_slippage_pct: None,
                    price_impact_pct: None,
                };

                // Resolve topology once for accepted and rejected emissions.
                // Candidate and plan now preserve the same h+1 token traversal.
                // The plan remains the source for per-leg pool/DEX identity;
                // missing geometry stays absent, never a fabricated route.
                let route_metadata = {
                    let rm = crate::persistence::build_route_metadata_from_plan(&route_plan);
                    if rm.is_populated() {
                        Some(rm)
                    } else {
                        None
                    }
                };
                let route_ref = route_metadata.as_ref();

                // Map the cartridge category to its canonical StrategyLabel so the
                // downstream evaluator (config_aware.rs) branches on the TRUE strategy
                // family (capital caps, flash detection, etc.).
                //
                // C.5 FIX: the prior `_ => DexArbV2V2` arm collapsed ~250/264
                // auto-generated cartridges into the v2v2 family and the comment above
                // it claimed the opposite (that each cartridge got its own identity).
                // R8 fail-honest: an unmapped category is REJECTED with an explicit
                // reason rather than silently mis-gated. The dex_arb family cannot be
                // disambiguated into v2v2/v2v3/v3v2/v3v3 from the category string
                // alone; the granular variant is preserved in RoutePlan.strategy_kind.
                //
                // Task 3 (docs/superpowers/plans/2026-08-17-cartridge-math-264.md):
                // the 11 Excel families map onto EXISTING labels via
                // `category_to_strategy_label`; the closed-cycle geometry of the
                // built plan decides the G03/G04 state-event families.
                let label = match category_to_strategy_label(
                    &category,
                    route_plan_is_closed_cycle(&route_plan.legs),
                ) {
                    Ok(label) => label,
                    Err(reason) => {
                        // R8: unknown category → reject, never silently collapse.
                        // The label passed to emit_rejected is a required-by-API
                        // placeholder (StrategyLabel has no Unknown variant); the
                        // rejection_reason carries the true category for diagnostics.
                        warn!(
                            event = "cartridge.unmapped_strategy_label",
                            chain_id,
                            cartridge_id = %cartridge_id,
                            category = %category,
                            "rejecting cartridge with unmapped strategy category"
                        );
                        let mut opp = opportunity.clone();
                        opp.rejection_reason = Some(reason.clone());
                        if let Err(e) = emitter
                            .emit_rejected(&opp, StrategyLabel::DexArbV2V2, &reason, route_ref)
                            .await
                        {
                            warn!(
                                event = "cartridge.emit_rejected_failed",
                                chain_id,
                                cartridge_id = %cartridge_id,
                                error = %e,
                                "failed to emit unmapped-label rejection"
                            );
                        }
                        continue;
                    }
                };

                let strategy_candidate = StrategyCandidate {
                    label,
                    opportunity,
                    candidate,
                    route_plan,
                    gross_profit_usd: eval_result
                        .metadata
                        .get("profit_usd_hint")
                        .and_then(|v| v.as_f64())
                        .filter(|p| *p > 0.0),
                    net_expected_profit_usd: None,
                    rejection_reason: None,
                    source_intent_hash: intent.tx_hash,
                    base_strategy: None,
                };

                // ── C.1: run SizeOptimizer BEFORE the gates ───────────────────
                // The cartridge path previously bypassed GATE 1 (SizeOptimizer),
                // emitting candidates sized at the raw mempool `amount_in` with only
                // a Rhai `$5` precheck — where native would reject
                // (NonPositiveNetUsd / GasFloorBreach). Mirror the native path
                // (orchestrator.rs on_route_intent ~773-841): size first, then gate.
                let chain_str = chain_id.to_string();
                let (strategy_candidate, sized_leg_ledger) = {
                    let outcome = match size_optimizer
                        .optimize_with_reason(
                            strategy_candidate.clone(),
                            &intent,
                            cfg_snapshot.as_ref(),
                        )
                        .await
                    {
                        Ok(o) => o,
                        Err(e) => {
                            warn!(
                                event = "cartridge.size_optimizer_error",
                                chain_id,
                                tx_hash = %intent.tx_hash,
                                cartridge_id = %cartridge_id,
                                error = %e,
                                "size_optimizer returned Err — treating as no profit"
                            );
                            // R8: infra error, no economic value was computed — None.
                            OptimizeOutcome::Rejected(OptimizeRejectReason::NonPositiveProfit, None)
                        }
                    };

                    match outcome {
                        OptimizeOutcome::Sized(sized) => {
                            // Update the candidate with optimal sizing data (mirrors
                            // native: gross/net on both candidate + opportunity row).
                            let s = *sized;
                            // ALWAYS-COMPUTE (2026-09-27): the complete economics
                            // object on the accepted row too (additive wire). Built
                            // before `s.candidate` moves out of `s`.
                            let economics_obj =
                                crate::economics::economics_from_sized(&s, cfg_snapshot.as_ref());
                            let mut c = s.candidate;
                            c.gross_profit_usd = Some(s.gross_profit_usd);
                            c.net_expected_profit_usd = Some(s.estimated_net_profit_usd);
                            c.opportunity.expected_profit_usd = Some(s.gross_profit_usd);
                            c.opportunity.net_expected_profit_usd =
                                Some(s.estimated_net_profit_usd);
                            // ALWAYS-COMPUTE (2026-09-27): the complete economics
                            // object on the accepted row too (additive wire).
                            if crate::economics::always_compute_enabled() {
                                c.opportunity.economics = Some(economics_obj);
                            }
                            // HOPS-LEDGER-04: thread the kernel's exact per-leg
                            // wei through to persistence — attached there onto
                            // the plan-built RouteMetadata. Some only when BOTH
                            // arrays exist (all-or-nothing, R8).
                            let legs = match (s.leg_amounts_in, s.leg_amounts_out) {
                                (Some(amounts_in), Some(amounts_out)) => {
                                    Some((amounts_in, amounts_out))
                                }
                                _ => None,
                            };
                            (c, legs)
                        }
                        // ALWAYS-COMPUTE (2026-09-27): a rejection whose path HAD
                        // computed the full economics. The pre-mandate code wiped
                        // `expected_profit_usd = None` at this exact line — the
                        // null factory behind the audit's 39/40 null gross on the
                        // live feed. The row now keeps EVERY number (gross AND
                        // net, the sized amount, the ledger, the full computation
                        // object): FAIL = "se hizo el cálculo y no cumple el
                        // criterio", not "no tengo números".
                        OptimizeOutcome::RejectedComputed(reason, boxed) => {
                            let reason_str = reason.as_str().to_owned();
                            REJECTED_NO_PROFIT_TOTAL
                                .with_label_values(&[&chain_str, label.as_str(), &reason_str])
                                .inc();
                            let s = *boxed;
                            let mut opp = strategy_candidate.opportunity.clone();
                            opp.rejection_reason = Some(reason_str.clone());
                            if crate::economics::always_compute_enabled() {
                                opp.expected_profit_usd = Some(s.gross_profit_usd);
                                opp.net_expected_profit_usd = Some(s.estimated_net_profit_usd);
                                if s.optimal_amount_in > ethers::types::U256::zero() {
                                    opp.amount_in_wei = s.optimal_amount_in.to_string();
                                }
                                opp.economics = Some(crate::economics::economics_from_sized(
                                    &s,
                                    cfg_snapshot.as_ref(),
                                ));
                            } else {
                                // Knob OFF: pre-mandate semantics — gross wiped,
                                // net = the kernel's single scalar payload.
                                opp.expected_profit_usd = None;
                                opp.net_expected_profit_usd = Some(s.estimated_net_profit_usd);
                            }
                            // PER-HOP upgrade: the rejected ledger attaches to
                            // the row's RouteMetadata exactly like the native
                            // path (all-or-nothing; a mismatched length skips).
                            let route_with_ledger = route_ref.map(|rm| {
                                let mut rm = rm.clone();
                                if let (Some(ins), Some(outs)) =
                                    (s.leg_amounts_in.clone(), s.leg_amounts_out.clone())
                                {
                                    let _ = rm.attach_leg_ledger(&ins, &outs);
                                }
                                rm
                            });
                            if let Err(e) = emitter
                                .emit_rejected(&opp, label, &reason_str, route_with_ledger.as_ref())
                                .await
                            {
                                warn!(
                                    event = "cartridge.emit_rejected_failed",
                                    chain_id,
                                    cartridge_id = %cartridge_id,
                                    error = %e,
                                    "failed to emit optimizer-rejected cartridge candidate"
                                );
                            }
                            continue;
                        }
                        // PER-HOP: the ledger-carrying variant is treated exactly
                        // like Rejected here; the cartridge emission path does not
                        // attach per-leg metadata yet (the native orchestrator path
                        // does — follow-up documented in the PR).
                        OptimizeOutcome::Rejected(reason, rejected_net)
                        | OptimizeOutcome::RejectedWithLedger(reason, rejected_net, _) => {
                            let reason_str = reason.as_str().to_owned();
                            REJECTED_NO_PROFIT_TOTAL
                                .with_label_values(&[&chain_str, label.as_str(), &reason_str])
                                .inc();
                            // R8: expected_profit_usd=None — never a fabricated figure
                            // on a rejected candidate. Deuda 4-(B): the kernel's
                            // computed net DOES travel (Some = computed, usually
                            // <= 0) so the emitter does not fall back to the raw
                            // detection estimate.
                            let mut opp = strategy_candidate.opportunity.clone();
                            opp.rejection_reason = Some(reason_str.clone());
                            opp.expected_profit_usd = None;
                            opp.net_expected_profit_usd = rejected_net;
                            // ALWAYS-COMPUTE: no-quote reject ⇒ honest object —
                            // "partial" when a payload figure exists, else "error"
                            // with the verbatim reason (gate 2: never fabricated).
                            if crate::economics::always_compute_enabled() {
                                opp.economics = Some(match rejected_net {
                                    Some(v) => crate::economics::economics_partial(
                                        None,
                                        Some(v),
                                        None,
                                        Some(&reason_str),
                                    ),
                                    None => crate::economics::economics_error(&reason_str),
                                });
                            }
                            if let Err(e) = emitter
                                .emit_rejected(&opp, label, &reason_str, route_ref)
                                .await
                            {
                                warn!(
                                    event = "cartridge.emit_rejected_failed",
                                    chain_id,
                                    cartridge_id = %cartridge_id,
                                    error = %e,
                                    "failed to emit optimizer-rejected cartridge candidate"
                                );
                            }
                            continue;
                        }
                    }
                };

                // Process through the same pipeline as native engines (price
                // snapshot threaded from the per-intent fetch above — FIX #9).
                if let Err(e) = process_cartridge_candidate(
                    strategy_candidate,
                    sized_leg_ledger,
                    cfg_snapshot.as_ref(),
                    ctx_chain_id,
                    emitter.clone(),
                    price_snapshot.clone(),
                    identity_idx.clone(),
                )
                .await
                {
                    warn!(
                        event = "cartridge.active_process_failed",
                        chain_id,
                        cartridge_id = %cartridge_id,
                        error = %e,
                        "failed to process cartridge candidate"
                    );
                }
            }
            Err(e) => {
                warn!(
                    event = "cartridge.active_eval_error",
                    chain_id,
                    tx_hash = %intent.tx_hash,
                    cartridge_id = %cartridge_id,
                    error = %e,
                    "cartridge active evaluation failed; skipping"
                );
            }
        }
    }

    info!(
        event = "cartridge.active_eval_summary",
        chain_id,
        tx_hash = %intent.tx_hash,
        pertinent = pertinent_count,
        negative = negative_total,
        positive = positive_total,
        reasons = ?negative_reasons,
        // ACTIVE-REPAIRS-01: top pares (campo::razón) de los repairs v4 de las
        // evaluaciones negativas de ESTA tx — el drill-down que la ruta activa
        // no publicaba en ningún sitio. Ordenado y truncado a 12 para no
        // reproducir LOGFLOOD-01 (R9): una línea por tx.
        top_repairs = ?{
            let mut v: Vec<(String, u64)> = negative_repairs
                .iter()
                .map(|(k, n)| (k.clone(), *n))
                .collect();
            v.sort_by(|a, b| b.1.cmp(&a.1));
            v.truncate(12);
            v
        },
        // OBSERVABILITY-V4-EDGES-01 (2026-10-01): censo del grafo v4 dentro del
        // summary que YA existe. Una línea por tx, nunca por intent: esto corre
        // ~24.800 veces por ventana y un log por ítem reproduciría LOGFLOOD-01
        // (R9). Sin este dato no se puede distinguir si los cartuchos que
        // buscan lo hacen sobre un grafo vacío o sobre uno real con piernas
        // descartadas — todo el diagnóstico de esa ruta estaba en `debug!` y
        // producción emite en `INFO`, así que la pregunta era inrespondible.
        v4_edges_built,
        v4_legs_skipped = ?v4_skip_reasons,
        "cartridge active eval summary (per-reason negatives)"
    );
}

/// REASON-TAG-NOT-DEBUG-01 — the stable snake_case label for a spine rejection.
///
/// `RejectReason::tag()` exists for exactly this purpose; its own doc says it
/// "Avoids leaking enum variant Debug formatting (which can change across Rust
/// versions)" (`prioritization-spine/src/decision.rs`). Both rejection sites in
/// this file formatted the reason with `{:?}` instead, so the wire carried the
/// Rust VARIANT NAME in PascalCase.
///
/// MEASURED (VPS PostgreSQL, 2026-09-27, 87 697 rows / 30 min): `NegativeNetProfit`
/// on 14 rows, while the code's own mapping defines `negative_net_profit` (and the
/// other six gate reasons are snake_case). A consumer matching the documented tag
/// silently missed those rows, and the reason histogram split one condition into
/// two buckets. Every rejection label in this file goes through here now.
fn reject_reason_label(reason: &prioritization_spine::decision::RejectReason) -> String {
    reason.tag().to_string()
}

/// Process a cartridge-generated candidate through the full evaluation + emission pipeline.
/// Mirrors `Orchestrator::process_candidate` but accessible from cartridge_boot context.
/// `price_snapshot` is the live Redis price map fetched once per intent by the caller.
async fn process_cartridge_candidate(
    sc: crate::engines::StrategyCandidate,
    // HOPS-LEDGER-04: exact per-leg wei (in, out) computed by the sizing
    // kernel, aligned with route_plan legs. `None` when sizing didn't run or
    // the kernel has no per-leg math (triangular). Attached onto the
    // plan-built metadata below so every emit — accepted AND rejected —
    // persists the ledger (RULE 00: the operator must see it on ~100% of
    // live rows, which are rejections).
    sized_leg_ledger: Option<(Vec<String>, Vec<String>)>,
    cfg: Option<&shared_rs::trading_config::TradingConfigState>,
    chain_id: u64,
    emitter: Arc<crate::opportunity_emitter::OpportunityEmitter>,
    price_snapshot: std::collections::HashMap<String, f64>,
    identity: Option<std::sync::Arc<shared_rs::token_identity::TokenIdentityIndex>>,
) -> anyhow::Result<()> {
    use crate::strategy_label::StrategyLabel;
    use prioritization_spine::config_aware::{ConfigAwareEvaluator, NetworkSignals};
    use std::collections::HashMap;

    let label = sc.label;
    let label_str = label.as_str();

    // HOPS-EMIT-01 (same rationale as active_evaluate_and_emit): every emit
    // below — engine rejection, NoTradingConfig, spine gate verdicts, and the
    // accepted arm — persists the topology built from the plan legs, so
    // rejected cartridge rows keep their visible route (RULE 00: ~100% of live
    // rows are rejections; a route on accepted-only emissions would leave the
    // dashboard blind exactly where the operator looks). sc.candidate's
    // flattened token pairs are NOT a valid source (structural gate), only the
    // plan legs are.
    let route_metadata = {
        let mut rm = crate::persistence::build_route_metadata_from_plan(&sc.route_plan);
        // HOPS-LEDGER-04: attach the kernel's per-leg ledger when present.
        // attach_leg_ledger is all-or-nothing: on a length mismatch against
        // the plan hops nothing is attached — never a partial ledger (R8).
        if let Some((amounts_in, amounts_out)) = sized_leg_ledger {
            if !rm.attach_leg_ledger(&amounts_in, &amounts_out) {
                debug!(
                    event = "cartridge.leg_ledger_attach_mismatch",
                    hops = rm.dex_adapters.len(),
                    amounts_in_len = amounts_in.len(),
                    amounts_out_len = amounts_out.len(),
                    "leg ledger length mismatch vs plan hops — ledger omitted (R8)"
                );
            }
        }
        if rm.is_populated() {
            Some(rm)
        } else {
            None
        }
    };
    let route_ref = route_metadata.as_ref();

    // Engine-level rejection: emit rejected immediately
    if let Some(reason) = &sc.rejection_reason {
        let mut opp = sc.opportunity.clone();
        opp.rejection_reason = Some(reason.clone());
        emitter
            .emit_rejected(&opp, label, reason, route_ref)
            .await?;
        return Ok(());
    }

    // FIX (review V2 #1, applied to cartridge path): NO config → explicit
    // rejection, NEVER emit_accepted. A cartridge opportunity that skips the
    // allowlist/pricing/strategy-enable/risk gates must NOT be classified as
    // accepted in the live emitter — that is the same fail-open the reviewer
    // flagged on the orchestrator path. Surface as rejected with the reason.
    let Some(state) = cfg else {
        let reason = "NoTradingConfig".to_string();
        let mut opp = sc.opportunity.clone();
        opp.rejection_reason = Some(reason.clone());
        emitter
            .emit_rejected(&opp, label, &reason, route_ref)
            .await?;
        return Ok(());
    };

    // Build evaluator and run spine gate. The live price snapshot is threaded in
    // from the caller (active_evaluate_and_emit), which fetches it once per intent
    // from Redis — see price_snapshot param (FIX review V2 #9).
    let signals = NetworkSignals::unknown(sc.opportunity.block_number.unwrap_or(0));
    // ARBX-R-0002: identity mode — the cartridge candidate's token_addresses
    // are addresses; gate on them addr-keyed (see caller note).
    let ev = ConfigAwareEvaluator::with_cache(state, signals, price_snapshot)
        .with_token_identity(identity);

    let spine_gate_outcome = ev.evaluate_with_route_plan(
        &sc.candidate,
        Some(&sc.route_plan),
        label_str,
        chain_id,
        "rpc-pool".to_string(),
        60_000,
    );

    use prioritization_spine::config_aware::ConfigGateOutcome;
    match spine_gate_outcome {
        ConfigGateOutcome::Evaluated {
            outcome, rejection, ..
        } => {
            match rejection {
                Some(reject_reason) => {
                    let reason = reject_reason_label(&reject_reason);
                    let mut opp = sc.opportunity.clone();
                    opp.rejection_reason = Some(reason.clone());
                    emitter
                        .emit_rejected(&opp, label, &reason, route_ref)
                        .await?;
                }
                None => {
                    // Accepted — outcome carries net profit and ROI
                    let mut opp = sc.opportunity.clone();
                    opp.net_expected_profit_usd = Some(outcome.net_profit_usd);
                    opp.roi_pct = Some(outcome.net_roi_pct);
                    emitter
                        .emit_accepted_with_plan(&opp, label, route_ref, &sc.route_plan)
                        .await?;
                }
            }
        }
        ConfigGateOutcome::TokenNotAllowed {
            token_symbol_or_addr,
        } => {
            let reason = format!("TokenNotAllowed:{}", token_symbol_or_addr);
            let mut opp = sc.opportunity.clone();
            opp.rejection_reason = Some(reason.clone());
            emitter
                .emit_rejected(&opp, label, &reason, route_ref)
                .await?;
        }
        ConfigGateOutcome::StrategyDisabled { strategy_kind: sk } => {
            let reason = format!("StrategyDisabled:{}", sk);
            let mut opp = sc.opportunity.clone();
            opp.rejection_reason = Some(reason.clone());
            emitter
                .emit_rejected(&opp, label, &reason, route_ref)
                .await?;
        }
        ConfigGateOutcome::StrategyConfigGateBlocked {
            reason: reject_reason,
        } => {
            let reason = format!(
                "StrategyConfigGateBlocked:{}",
                reject_reason_label(&reject_reason)
            );
            let mut opp = sc.opportunity.clone();
            opp.rejection_reason = Some(reason.clone());
            emitter
                .emit_rejected(&opp, label, &reason, route_ref)
                .await?;
        }
    }

    Ok(())
}

/// Redis key the searcher publishes its loaded-cartridge registry snapshot to.
/// Read by api-server `GET /api/cartridges`. One key per chain.
pub fn cartridge_registry_redis_key(chain_id: u64) -> String {
    format!("arbx:cartridges:registry:{chain_id}")
}

/// Publishes the current loaded-cartridge registry to Redis as JSON so the
/// api-server can serve the REAL loaded set on `GET /api/cartridges` (instead
/// of the telemetry-gated empty-until-first-evaluation cache).
///
/// R8 fail-honest: TTL = 10 min. The boot path re-publishes on every restart;
/// if the searcher dies the key expires and the API returns an honest
/// "registry unavailable" rather than stale rows. Best-effort — a Redis hiccup
/// is logged and never fatal to the scanner.
///
/// CARTRIDGE-CONTROL: `pub` so the hot couple/decouple loop can refresh the
/// registry snapshot immediately after a pause/resume (not just every 240s).
pub async fn publish_cartridge_registry(
    redis: &mut redis::aio::ConnectionManager,
    runner: &Arc<CartridgeRunner>,
    chain_id: u64,
) {
    use redis::AsyncCommands;

    let cartridges = runner.list_cartridges().await;
    let active = runner.active_count().await;
    let entries: Vec<serde_json::Value> = cartridges
        .iter()
        .map(|(id, meta, state)| {
            serde_json::json!({
                "id": id,
                "name": meta.name,
                "version": meta.version,
                "author": meta.author,
                "description": meta.description,
                "category": meta.category,
                "target_chains": meta.target_chains,
                "state": format!("{:?}", state),
            })
        })
        .collect();

    let payload = serde_json::json!({
        "chain_id": chain_id,
        "total": entries.len(),
        "active": active,
        "updated_at": chrono::Utc::now().to_rfc3339(),
        "cartridges": entries,
    });

    let key = cartridge_registry_redis_key(chain_id);
    let json = match serde_json::to_string(&payload) {
        Ok(j) => j,
        Err(e) => {
            warn!(event = "cartridge.registry_serialize_failed", chain_id, error = %e);
            return;
        }
    };
    // TTL 600s — refreshed at every boot; expires if the searcher is gone.
    let res: redis::RedisResult<()> = redis.set_ex(&key, json, 600).await;
    match res {
        Ok(()) => info!(
            event = "cartridge.registry_published",
            chain_id,
            total = entries.len(),
            active,
            "cartridge registry snapshot published to Redis"
        ),
        Err(e) => warn!(
            event = "cartridge.registry_publish_failed",
            chain_id,
            error = %e,
            "failed to publish cartridge registry to Redis (non-fatal)"
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// REASON-TAG-NOT-DEBUG-01. El defecto medido: la fila viva llevaba
    /// `NegativeNetProfit` (Debug del variante) mientras el propio codigo define
    /// `negative_net_profit` como tag estable.
    #[test]
    fn reject_reason_label_is_the_stable_snake_case_tag() {
        use prioritization_spine::decision::RejectReason as R;
        assert_eq!(
            reject_reason_label(&R::NegativeNetProfit),
            "negative_net_profit"
        );
        assert_eq!(reject_reason_label(&R::LowLiquidity), "low_liquidity");
        assert_eq!(
            reject_reason_label(&R::ExcessiveSlippage),
            "excessive_slippage"
        );
    }

    #[test]
    fn reject_reason_label_never_leaks_debug_formatting() {
        use prioritization_spine::decision::RejectReason as R;
        let label = reject_reason_label(&R::NegativeNetProfit);
        // La firma exacta del defecto: `{:?}` daba el nombre del variante.
        assert_ne!(label, format!("{:?}", R::NegativeNetProfit));
        assert!(
            !label.chars().any(|c| c.is_ascii_uppercase()),
            "el label debe ser snake_case, llego: {label}"
        );
    }

    #[test]
    fn parse_defaults_to_off_for_unset_or_unknown() {
        assert_eq!(CartridgeMode::parse(""), CartridgeMode::Off);
        assert_eq!(CartridgeMode::parse("garbage"), CartridgeMode::Off);
        assert_eq!(CartridgeMode::parse("0"), CartridgeMode::Off);
        assert!(!CartridgeMode::parse("").is_enabled());
    }

    #[test]
    fn parse_known_modes_case_and_whitespace_insensitive() {
        assert_eq!(CartridgeMode::parse("shadow"), CartridgeMode::Shadow);
        assert_eq!(CartridgeMode::parse("  SHADOW "), CartridgeMode::Shadow);
        assert_eq!(CartridgeMode::parse("Active"), CartridgeMode::Active);
        assert!(CartridgeMode::parse("shadow").is_enabled());
        assert!(CartridgeMode::parse("active").is_enabled());
    }

    #[test]
    fn as_str_matches_variants() {
        assert_eq!(CartridgeMode::Off.as_str(), "off");
        assert_eq!(CartridgeMode::Shadow.as_str(), "shadow");
        assert_eq!(CartridgeMode::Active.as_str(), "active");
    }

    /// HOPS-CARD-05 gate: the cartridge-side DFS ceiling must carry the full
    /// canonical 2..7 range (it was hard-coded to 4, so 5-7 hop intents could
    /// never be represented inside the runtime) and must stay inside the
    /// `agent_graph::SearchLimits` invariant.
    #[test]
    fn cartridge_dfs_ceiling_uses_the_canonical_max_hops() {
        let hops = canonical_max_hops();
        assert!(
            (2..=7).contains(&hops),
            "cartridge max_hops {hops} outside the canonical 2..=7 range"
        );
        // The runtime consumer's OWN invariant (`enumerate_cycles` rejects
        // anything outside 2..=7 before touching the graph): proven by calling
        // it with the cartridge limits over an edge-less graph — valid limits
        // return Ok(empty), invalid ones Err("invalid_search_limits").
        let limits = crate::agent_graph::SearchLimits {
            max_hops: hops as usize,
            max_expansions: 512,
            max_paths: 64,
        };
        let report = crate::agent_graph::enumerate_cycles(&[], "0xstart", &limits)
            .expect("agent_graph must accept the cartridge DFS ceiling");
        assert!(report.paths.is_empty() && !report.truncated);
        // With no operator override the ceiling IS the workbook default (7).
        if std::env::var("ARBX_KNOB_MAX_HOPS").is_err() {
            assert_eq!(
                hops,
                crate::canonical_knobs::CanonicalKnobs::default().max_hops
            );
        }
    }

    #[test]
    fn pool_data_adapter_extracts_first_leg() {
        use crate::route_intent::{DetectionSource, RouteIntentLeg, RouterKind, SwapExactMode};
        use ethers::types::{Address, H256, U256};
        let leg = RouteIntentLeg {
            token_in: Address::from_low_u64_be(0xAAAA),
            token_out: Address::from_low_u64_be(0xBBBB),
            pool_hint: Some(Address::from_low_u64_be(0xCCCC)),
            dex_hint: None,
            fee_bps: None,
            protocol_type: ProtocolType::V2,
        };
        let intent = RouteIntent::new(
            1,
            H256::zero(),
            Address::zero(),
            RouterKind::UniswapV2,
            Address::zero(),
            vec![leg],
            U256::from(1234u64),
            None,
            SwapExactMode::ExactIn,
            DetectionSource::PublicMempool,
        )
        .expect("valid intent");
        let m = build_cartridge_pool_data(&intent, None);
        assert_eq!(m.get("chain_id").unwrap().to_string(), "1");
        assert_eq!(m.get("amount_in").unwrap().to_string(), "1234");
        assert_eq!(m.get("protocol_type").unwrap().to_string(), "v2");
        assert!(!m.get("source_pool").unwrap().to_string().is_empty());
    }

    #[test]
    fn pool_data_triangle_vertices_present_for_closed_cycle_only() {
        // D-01/F3 follow-up: the triangular cartridge reads token_a/b/c. A closed
        // 3-leg cycle must carry its vertices; a 2-leg cycle (dex_arb shape)
        // must NOT (missing-field→() is the mis-eval cascade the shape gate
        // exists to prevent).
        let tri = three_leg_intent();
        let m = build_cartridge_pool_data(&tri, None);
        let va = format!("{:#x}", ethers::types::Address::from_low_u64_be(0xA));
        assert_eq!(m.get("token_a").unwrap().to_string(), va);
        assert!(m.contains_key("token_b"));
        assert!(m.contains_key("token_c"));
        assert_eq!(m.get("route_closed").unwrap().to_string(), "true");

        let two = two_leg_cycle_intent(DetectionSource::NewBlock);
        let m2 = build_cartridge_pool_data(&two, None);
        assert!(
            !m2.contains_key("token_a"),
            "2-leg cycles must not carry triangle vertices"
        );
    }

    #[test]
    fn pool_data_empty_source_pool_when_pool_hint_none() {
        use crate::route_intent::{DetectionSource, RouteIntentLeg, RouterKind, SwapExactMode};
        use ethers::types::{Address, H256, U256};
        let leg = RouteIntentLeg {
            token_in: Address::from_low_u64_be(0x1),
            token_out: Address::from_low_u64_be(0x2),
            pool_hint: None,
            dex_hint: None,
            fee_bps: None,
            protocol_type: ProtocolType::Unknown,
        };
        let intent = RouteIntent::new(
            1,
            H256::zero(),
            Address::zero(),
            RouterKind::Unknown,
            Address::zero(),
            vec![leg],
            U256::zero(),
            None,
            SwapExactMode::Unknown,
            DetectionSource::PublicMempool,
        )
        .expect("valid intent");
        let m = build_cartridge_pool_data(&intent, None);
        assert_eq!(m.get("source_pool").unwrap().to_string(), "");
        assert_eq!(m.get("protocol_type").unwrap().to_string(), "unknown");
        // No reserves provided -> key absent -> Rhai sees () -> reserve-dependent cartridges fail-honest.
        assert!(!m.contains_key("reserves_source"));
    }

    #[test]
    fn pool_data_injects_reserves_source_when_provided() {
        use crate::reserves::ReservesEntry;
        use crate::route_intent::{DetectionSource, RouteIntentLeg, RouterKind, SwapExactMode};
        use ethers::types::{Address, H256, U256};
        let leg = RouteIntentLeg {
            token_in: Address::from_low_u64_be(0xAAAA),
            token_out: Address::from_low_u64_be(0xBBBB),
            pool_hint: Some(Address::from_low_u64_be(0xCCCC)),
            dex_hint: None,
            fee_bps: None,
            protocol_type: ProtocolType::V2,
        };
        let intent = RouteIntent::new(
            1,
            H256::zero(),
            Address::zero(),
            RouterKind::UniswapV2,
            Address::zero(),
            vec![leg],
            U256::from(1234u64),
            None,
            SwapExactMode::ExactIn,
            DetectionSource::NewBlock,
        )
        .expect("valid intent");
        let rs = ReservesEntry {
            r0: "1500000000000000000000".to_string(),
            r1: "4800000000000".to_string(),
            token0_addr: Some("0xc02aaa39".to_string()),
            blk: 18_500_000,
            ts: 1_714_857_600,
        };
        let m = build_cartridge_pool_data(&intent, Some(&rs));
        let rsrc = m
            .get("reserves_source")
            .expect("reserves_source must be present when provided")
            .clone();
        let rmap = rsrc.cast::<rhai::Map>();
        assert_eq!(
            rmap.get("r0").unwrap().to_string(),
            "1500000000000000000000"
        );
        assert_eq!(rmap.get("r1").unwrap().to_string(), "4800000000000");
        assert_eq!(rmap.get("block").unwrap().to_string(), "18500000");
        assert_eq!(rmap.get("ts").unwrap().to_string(), "1714857600");
    }

    /// Build a minimal single-leg intent with the given detection source, for routing tests.
    fn intent_with_source(source: crate::route_intent::DetectionSource) -> RouteIntent {
        use crate::route_intent::{RouteIntentLeg, RouterKind, SwapExactMode};
        use ethers::types::{Address, H256, U256};
        let leg = RouteIntentLeg {
            token_in: Address::from_low_u64_be(0xAAAA),
            token_out: Address::from_low_u64_be(0xBBBB),
            pool_hint: Some(Address::from_low_u64_be(0xCCCC)),
            dex_hint: None,
            fee_bps: None,
            protocol_type: ProtocolType::V2,
        };
        RouteIntent::new(
            1,
            H256::zero(),
            Address::zero(),
            RouterKind::UniswapV2,
            Address::zero(),
            vec![leg],
            U256::from(1234u64),
            None,
            SwapExactMode::ExactIn,
            source,
        )
        .expect("valid intent")
    }

    /// Two-leg cycle (A→B→A) — the shape the dispatcher's `build_intent` emits
    /// for 2-token routes. Closed in the ≥2 sense but NOT a triangle (D-01's
    /// shape gate keys on ≥3 legs): dex_arb's shape.
    fn two_leg_cycle_intent(source: crate::route_intent::DetectionSource) -> RouteIntent {
        use crate::route_intent::{RouteIntentLeg, RouterKind, SwapExactMode};
        use ethers::types::{Address, H256, U256};
        let mk = |a: u64, b: u64| RouteIntentLeg {
            token_in: Address::from_low_u64_be(a),
            token_out: Address::from_low_u64_be(b),
            pool_hint: Some(Address::from_low_u64_be(0xCCCC)),
            dex_hint: None,
            fee_bps: None,
            protocol_type: ProtocolType::V2,
        };
        RouteIntent::new(
            1,
            H256::zero(),
            Address::zero(),
            RouterKind::UniswapV2,
            Address::zero(),
            vec![mk(0xA, 0xB), mk(0xB, 0xA)],
            U256::from(1234u64),
            None,
            SwapExactMode::ExactIn,
            source,
        )
        .expect("valid intent")
    }

    #[test]
    fn routing_swap_intent_goes_only_to_dex_arb() {
        // A confirmed V2 swap from the block scanner (NewBlock) must reach dex_arb and
        // NOT liquidation / triangular_arb (they would error on the missing position /
        // triangle fields — this is the get_token_meta(()) flood we are eliminating).
        let swap = intent_with_source(DetectionSource::NewBlock);
        assert!(cartridge_matches_intent("dex_arb", &swap));
        assert!(!cartridge_matches_intent("liquidation", &swap));
        assert!(!cartridge_matches_intent("triangular_arb", &swap));
        // Same routing for pending-mempool swap observations.
        let pending = intent_with_source(DetectionSource::PublicMempool);
        assert!(cartridge_matches_intent("dex_arb", &pending));
        assert!(!cartridge_matches_intent("liquidation", &pending));
    }

    #[test]
    fn routing_closed_cycle_goes_to_triangular_not_dex_arb() {
        // Shape rule (D-01): a closed ≥3-leg cycle is a TRIANGLE → triangular_arb;
        // dex_arb (cross-DEX spread on ONE observed pair) must not mis-evaluate it.
        let tri = three_leg_intent();
        assert!(cartridge_matches_intent("triangular_arb", &tri));
        assert!(!cartridge_matches_intent("dex_arb", &tri));
        assert!(!cartridge_matches_intent("liquidation", &tri));
        // A 2-leg swap cycle (the dispatcher's dex_arb shape) stays dex_arb's.
        let two = two_leg_cycle_intent(DetectionSource::NewBlock);
        assert!(cartridge_matches_intent("dex_arb", &two));
        assert!(!cartridge_matches_intent("triangular_arb", &two));
        // A 1-leg swap is still not a triangle (the original flood guard holds).
        let one = intent_with_source(DetectionSource::NewBlock);
        assert!(!cartridge_matches_intent("triangular_arb", &one));
        // Directiva F1-d: a geometrically-closed 3-leg shape from a POSITION
        // source (lending/oracle) is NOT a triangle — swap-observation only.
        let lending_tri = three_leg_intent_with_source(DetectionSource::LendingPositionUpdate);
        assert!(!cartridge_matches_intent("triangular_arb", &lending_tri));
        assert!(cartridge_matches_intent("liquidation", &lending_tri));
    }

    #[test]
    fn routing_lending_intent_goes_only_to_liquidation() {
        let lending = intent_with_source(DetectionSource::LendingPositionUpdate);
        assert!(cartridge_matches_intent("liquidation", &lending));
        assert!(!cartridge_matches_intent("dex_arb", &lending));
        assert!(!cartridge_matches_intent("triangular_arb", &lending));
        // Oracle updates also route to liquidation (price moves trigger liquidations).
        let oracle = intent_with_source(DetectionSource::OracleUpdate);
        assert!(cartridge_matches_intent("liquidation", &oracle));
        assert!(!cartridge_matches_intent("dex_arb", &oracle));
    }

    #[test]
    fn routing_unknown_category_is_never_dropped() {
        // Operator-installed custom cartridges own their input contract — evaluate them
        // against every intent rather than silently skipping.
        let swap = intent_with_source(DetectionSource::NewBlock);
        let lending = intent_with_source(DetectionSource::LendingPositionUpdate);
        assert!(cartridge_matches_intent("my_custom_strategy", &swap));
        assert!(cartridge_matches_intent("my_custom_strategy", &lending));
    }

    // ── Task 3: category → StrategyLabel mapping (11 Excel families) ─────────
    // docs/superpowers/plans/2026-08-17-cartridge-math-264.md

    #[test]
    fn category_label_legacy_categories_unchanged() {
        // The 10 legacy arms keep their exact labels (regression guard).
        assert_eq!(
            category_to_strategy_label("dex_arb", false),
            Ok(StrategyLabel::DexArbV2V2)
        );
        assert_eq!(
            category_to_strategy_label("dex_arb_v2v2", false),
            Ok(StrategyLabel::DexArbV2V2)
        );
        assert_eq!(
            category_to_strategy_label("dex_arb_v2v3", false),
            Ok(StrategyLabel::DexArbV2V3)
        );
        assert_eq!(
            category_to_strategy_label("dex_arb_v3v2", false),
            Ok(StrategyLabel::DexArbV3V2)
        );
        assert_eq!(
            category_to_strategy_label("dex_arb_v3v3", false),
            Ok(StrategyLabel::DexArbV3V3)
        );
        assert_eq!(
            category_to_strategy_label("triangular_arb", false),
            Ok(StrategyLabel::TriangularArb)
        );
        assert_eq!(
            category_to_strategy_label("flashloan_arb", false),
            Ok(StrategyLabel::FlashloanArb)
        );
        assert_eq!(
            category_to_strategy_label("liquidation", false),
            Ok(StrategyLabel::Liquidation)
        );
        assert_eq!(
            category_to_strategy_label("spanning_tree_arb", false),
            Ok(StrategyLabel::SpanningTreeArb)
        );
        assert_eq!(
            category_to_strategy_label("cross_chain_arb", false),
            Ok(StrategyLabel::CrossChainArb)
        );
        assert_eq!(
            category_to_strategy_label("liquidation_snipe", false),
            Ok(StrategyLabel::LiquidationSnipe)
        );
        // Unknown category: still rejected with the exact reason (R8).
        assert_eq!(
            category_to_strategy_label("totally_unknown", true),
            Err("cartridge_unmapped_strategy_label:totally_unknown".to_string())
        );
    }

    #[test]
    fn category_label_maps_six_engine_families() {
        // The 6 of the 11 Excel families with an honest label today.
        assert_eq!(
            category_to_strategy_label("cross_domain_engine", false),
            Ok(StrategyLabel::CrossChainArb)
        );
        assert_eq!(
            category_to_strategy_label("credit_liquidation_engine", false),
            Ok(StrategyLabel::LiquidationSnipe)
        );
        // G01/G02 spot-DEX — DexArbV2V2 label; the granular identity stays in
        // RoutePlan.strategy_kind (C.5: variants are NOT collapsed).
        assert_eq!(
            category_to_strategy_label("route_graph_engine", false),
            Ok(StrategyLabel::DexArbV2V2)
        );
        assert_eq!(
            category_to_strategy_label("amm_curve_engine", false),
            Ok(StrategyLabel::DexArbV2V2)
        );
        // G03/G04 — TriangularArb ONLY on a closed ≥3-leg cycle.
        assert_eq!(
            category_to_strategy_label("state_event_engine", true),
            Ok(StrategyLabel::TriangularArb)
        );
        assert_eq!(
            category_to_strategy_label("parity_redemption_engine", true),
            Ok(StrategyLabel::TriangularArb)
        );
    }

    #[test]
    fn category_label_state_event_families_map_unconditionally() {
        // FIX (2026-08-19): the `if is_closed_cycle` guard was removed — it
        // killed 6/10 opportunities with `cartridge_unmapped_strategy_label`
        // before the degenerate-route gates could reject honestly. G03/G04
        // now map to the triangle label unconditionally; degenerate routes
        // are rejected by the proper downstream gates, not at label mapping.
        for cat in ["state_event_engine", "parity_redemption_engine"] {
            assert!(
                matches!(
                    category_to_strategy_label(cat, false),
                    Ok(StrategyLabel::TriangularArb)
                ),
                "{cat} must map to the triangle label regardless of closed-cycle; degenerate rejection belongs downstream"
            );
        }
    }

    #[test]
    fn category_label_five_no_engine_families_reject_with_exact_reason() {
        // derivatives, cex_external, intents_solver, nft, prediction: no honest
        // engine yet → fail-honest reject with the exact reason (never silence).
        for cat in [
            "derivatives_engine",
            "cex_external_engine",
            "intents_solver_engine",
            "nft_engine",
            "prediction_engine",
        ] {
            assert_eq!(
                category_to_strategy_label(cat, true),
                Err(format!("cartridge_unmapped_strategy_label:{cat}")),
                "{cat} has no honest engine yet and must reject with reason"
            );
        }
    }

    #[test]
    fn route_plan_closed_cycle_matches_intent_leg_definition() {
        use prioritization_spine::route_plan::RouteLeg;
        let leg = |token_in: &str, token_out: &str| RouteLeg {
            dex_id: "dex".to_string(),
            dex_name: "dex".to_string(),
            protocol_type: "v2".to_string(),
            factory_address: "0x0".to_string(),
            pool_id: None,
            pool_address: None,
            token_in: token_in.to_string(),
            token_out: token_out.to_string(),
            fee_bps: Some(30),
            amount_in: None,
            amount_out: None,
            tvl_usd: None,
            volume_24h_usd: None,
            pool_is_active: true,
        };
        // Closed 3-leg triangle A→B→C→A.
        let tri = vec![leg("0xa", "0xb"), leg("0xb", "0xc"), leg("0xc", "0xa")];
        assert!(route_plan_is_closed_cycle(&tri));
        // Open 3-leg path A→B→C→D.
        let open = vec![leg("0xa", "0xb"), leg("0xb", "0xc"), leg("0xc", "0xd")];
        assert!(!route_plan_is_closed_cycle(&open));
        // Closed 2-leg cycle is NOT a triangle (≥3 legs required — D-01 shape).
        let two = vec![leg("0xa", "0xb"), leg("0xb", "0xa")];
        assert!(!route_plan_is_closed_cycle(&two));
        // Equivalence with the intent-legs definition used by smart routing:
        // the same Address-formatting the call site performs must close.
        let plan_from_intent: Vec<RouteLeg> = three_leg_intent()
            .legs
            .iter()
            .map(|l| {
                leg(
                    &format!("{:#x}", l.token_in),
                    &format!("{:#x}", l.token_out),
                )
            })
            .collect();
        assert!(route_plan_is_closed_cycle(&plan_from_intent));
    }

    // ── rd_outcome_v2 schema builders (Phase 1) ──────────────────────────────

    /// 3-leg cross-DEX cycle (V3 → Curve → V2) with distinct dex hints + fee tiers.
    fn three_leg_intent() -> RouteIntent {
        three_leg_intent_with_source(crate::route_intent::DetectionSource::NewBlock)
    }

    fn three_leg_intent_with_source(source: crate::route_intent::DetectionSource) -> RouteIntent {
        use crate::route_intent::{DetectionSource, RouteIntentLeg, RouterKind, SwapExactMode};
        use ethers::types::{Address, H256, U256};
        let mk = |a: u64, b: u64, pool: u64, dex: &str, proto: ProtocolType, fee: Option<u32>| {
            RouteIntentLeg {
                token_in: Address::from_low_u64_be(a),
                token_out: Address::from_low_u64_be(b),
                pool_hint: Some(Address::from_low_u64_be(pool)),
                dex_hint: Some(dex.to_string()),
                fee_bps: fee,
                protocol_type: proto,
            }
        };
        RouteIntent::new(
            1,
            H256::zero(),
            Address::zero(),
            RouterKind::UniswapV3,
            Address::zero(),
            vec![
                mk(0xA, 0xB, 0x1, "uniswap-v3", ProtocolType::V3, Some(500)),
                mk(0xB, 0xC, 0x2, "curve", ProtocolType::Curve, Some(4)),
                mk(0xC, 0xA, 0x3, "sushi", ProtocolType::V2, Some(30)),
            ],
            U256::from(1000u64),
            None,
            SwapExactMode::ExactIn,
            source,
        )
        .expect("valid intent")
    }

    fn eval_result(is_opp: bool) -> CartridgeEvalResult {
        CartridgeEvalResult {
            is_opportunity: is_opp,
            estimated_profit: 9.64,
            confidence: 0.82,
            metadata: std::collections::HashMap::new(),
            urgency: "high".to_string(),
            reason: Some("net_profit_positive_after_costs".to_string()),
        }
    }

    #[test]
    fn rd_outcome_v1_is_flat_no_topology() {
        let v = build_rd_outcome_v1(
            1,
            "omega_strategy_pack",
            &three_leg_intent(),
            &eval_result(true),
            true,
            ShadowSubstrate::IntentBundle,
            1_700_000_000_000,
        );
        assert_eq!(v["schema"].as_str().unwrap(), "rd_outcome_v1");
        assert_eq!(v["mode"].as_str().unwrap(), "shadow");
        assert!(v["is_opportunity"].as_bool().unwrap());
        // v1 is flat: no topology object, no route[] array.
        assert!(v.get("topology").is_none());

        // REPAIRS-OBSERVABILITY-01: sin proposal_v4 → null (fail-honest).
        assert!(v["v4_repairs"].is_null(), "sin proposal_v4 debe ser null");
        assert!(v.get("route").is_none());
    }

    /// REPAIRS-OBSERVABILITY-01 (enmienda): prueba el contrato del PRODUCTOR
    /// real — `repairs_push` (rhai_agent_bridge.rs L178) emite
    /// `{"field": ..., "reason": ...}`, y el parser promueve el `reason` del
    /// mapa a CartridgeEvalResult.reason EXCLUYÉNDOLO del proposal sellado.
    /// El primer test de este PR fabricaba objetos con "name" — certificaba
    /// un contrato distinto del productivo y devolvía lista vacía ante
    /// repairs reales (defecto señalado en revisión).
    #[test]
    fn rd_outcome_v1_carries_v4_repair_names() {
        let mut res = eval_result(false);
        // El parser preserva el reason FUERA del proposal (exclusion list).
        res.reason = Some("applicable_data_or_constraint_gap".to_string());
        res.metadata.insert(
            "proposal_v4".to_string(),
            serde_json::json!({
                "contract_version": "arbx.cartridge.agent/4",
                "status": "DATA_GAP",
                "repairs": [
                    {"field": "capital_usd", "reason": "capital_missing_or_cap_exceeded"},
                    {"field": "operators", "reason": "operator_context_mismatch"}
                ],
                "net_profit_usd": null
            }),
        );
        let v = build_rd_outcome_v1(
            1,
            "mev_01_015_two_leg_arbitrage",
            &three_leg_intent(),
            &res,
            true,
            ShadowSubstrate::IntentBundle,
            1_700_000_000_000,
        );
        let r = &v["v4_repairs"];
        assert_eq!(r["status"].as_str().unwrap(), "DATA_GAP");
        // El reason viene del campo preservado por el parser, no del proposal.
        assert_eq!(
            r["reason"].as_str().unwrap(),
            "applicable_data_or_constraint_gap"
        );
        // Formato NATIVO: cada repair conserva field Y reason.
        let repairs = r["repairs"].as_array().unwrap();
        assert_eq!(repairs.len(), 2);
        assert_eq!(repairs[0]["field"].as_str().unwrap(), "capital_usd");
        assert_eq!(
            repairs[0]["reason"].as_str().unwrap(),
            "capital_missing_or_cap_exceeded"
        );
        assert_eq!(repairs[1]["field"].as_str().unwrap(), "operators");
        assert_eq!(
            repairs[1]["reason"].as_str().unwrap(),
            "operator_context_mismatch"
        );
    }

    #[test]
    fn rd_outcome_v2_topology_route_and_failhonest_waterfall() {
        let v = build_rd_outcome_v2(
            1,
            "omega_strategy_pack",
            &three_leg_intent(),
            &eval_result(true),
            true,
            ShadowSubstrate::IntentBundle,
            1_700_000_000_000,
        );

        assert_eq!(v["schema"].as_str().unwrap(), "rd_outcome_v2");
        assert_eq!(v["status"].as_str().unwrap(), "shadow_visible");
        assert_eq!(v["strategy_kind"].as_str().unwrap(), "omega_strategy_pack");

        // Topology derived honestly from the legs.
        assert_eq!(v["topology"]["hop_count"].as_u64().unwrap(), 3);
        assert_eq!(
            v["topology"]["route_family"].as_str().unwrap(),
            "triangular"
        );
        assert_eq!(
            v["topology"]["environment"].as_str().unwrap(),
            "interdex_intrachain"
        );

        // Classifier→cartridge bridge: 3-leg cross-dex cycle (uniswap-v3/curve/sushi)
        // ⇒ triangular_cross_dex, which the committed omega_strategy_pack can dispatch.
        assert_eq!(
            v["topology"]["strategy_family"].as_str().unwrap(),
            "triangular_cross_dex"
        );
        assert!(v["topology"]["strategy_family_supported"]
            .as_bool()
            .unwrap());
        assert_eq!(
            v["topology"]["strategy_family_reason"].as_str().unwrap(),
            "three_leg_cross_dex_cycle"
        );

        // route[] carries per-leg dex / fee_bps / invariant.
        let route = v["route"].as_array().unwrap();
        assert_eq!(route.len(), 3);
        assert_eq!(
            route[0]["invariant"].as_str().unwrap(),
            "concentrated_liquidity"
        ); // V3
        assert_eq!(route[1]["invariant"].as_str().unwrap(), "stableswap"); // Curve
        assert_eq!(route[2]["invariant"].as_str().unwrap(), "constant_product"); // V2
        assert_eq!(route[0]["fee_bps"].as_u64().unwrap(), 500);
        assert_eq!(route[0]["dex"].as_str().unwrap(), "uniswap-v3");

        // FAIL-HONEST: uncomputed costs are null (never fabricated 0), net not computed.
        assert!(v["gas_cost_usd"].is_null());
        assert!(v["slippage_cost_usd"].is_null());
        assert!(v["net_profit_usd"].is_null());
        assert!(!v["net_computed"].as_bool().unwrap());
        // R8 fail-honest: the fixture's eval_result carries no `profit_usd_hint`
        // (only the token-units `estimated_profit` field), so build_rd_outcome_v2
        // emits null — never token-units-as-USD (the RC-2 unit-scale fix). This
        // matches the adjacent gas/slippage/net nulls above.
        assert!(v["estimated_profit_usd"].is_null());

        // Gates: simulation disabled here, live blocked, ethics permitted.
        assert_eq!(v["simulation"]["status"].as_str().unwrap(), "disabled");
        assert!(!v["live_gate"]["eligible"].as_bool().unwrap());
        assert_eq!(v["ethics"]["status"].as_str().unwrap(), "permitted");
    }

    #[test]
    fn rd_outcome_v2_rejected_status_when_not_opportunity() {
        let v = build_rd_outcome_v2(
            1,
            "omega_strategy_pack",
            &three_leg_intent(),
            &eval_result(false),
            false,
            ShadowSubstrate::StaticBootStub,
            1,
        );
        assert!(!v["is_opportunity"].as_bool().unwrap());
        assert_eq!(v["status"].as_str().unwrap(), "rejected_with_reason");
    }

    #[test]
    fn route_family_and_invariant_helpers() {
        assert_eq!(route_family(2), "spatial_or_pair");
        assert_eq!(route_family(3), "triangular");
        assert_eq!(route_family(4), "quadrangular");
        assert_eq!(route_family(7), "supreme_graph");
        assert_eq!(invariant_of("V3"), "concentrated_liquidity");
        assert_eq!(invariant_of("Curve"), "stableswap");
        assert_eq!(invariant_of("Balancer"), "weighted");
        assert_eq!(invariant_of("V2"), "constant_product");
        assert_eq!(invariant_of("Mystery"), "unknown");
    }
    #[test]
    fn v3_review_cartridge_fee_fields_are_protocol_aware() {
        for raw in [0u32, 1, 100, 150, 500, 3000, 10000, 999999] {
            let mut intent = three_leg_intent();
            intent.legs[0].fee_bps = Some(raw);
            let map = build_cartridge_pool_data(&intent, None);
            assert_eq!(map["fee_pips"].as_int().unwrap(), i64::from(raw));
            if raw % 100 == 0 {
                assert_eq!(map["fee_bps"].as_int().unwrap(), i64::from(raw / 100));
            } else {
                assert_eq!(map["fee_bps"].as_float().unwrap(), f64::from(raw) / 100.0);
            }
            let route = map["route"].clone().into_array().unwrap();
            let v3 = route[0].clone().cast::<rhai::Map>();
            let v2 = route[2].clone().cast::<rhai::Map>();
            assert_eq!(v3["fee_pips"].as_int().unwrap(), i64::from(raw));
            assert_eq!(v2["fee_bps"].as_int().unwrap(), 30);
            assert!(!v2.contains_key("fee_pips"));
        }
    }

    #[test]
    fn v3_review_missing_or_invalid_fee_is_not_fabricated() {
        for fee in [None, Some(1_000_000), Some(u32::MAX)] {
            let mut intent = three_leg_intent();
            intent.legs[0].fee_bps = fee;
            let map = build_cartridge_pool_data(&intent, None);
            assert!(!map.contains_key("fee_bps"));
            assert!(!map.contains_key("fee_pips"));
        }
    }

    #[test]
    fn v3_review_graph_fee_reaches_real_rhai_math_without_double_conversion() {
        use ethers::types::U256;
        let sqrt = (U256::one() << 96).to_string();
        let liquidity = U256::exp10(24).to_string();
        let amount = U256::exp10(12).to_string();
        for raw in [0u32, 1, 100, 150, 500, 3000, 10000, 999999] {
            let mut intent = three_leg_intent();
            intent.legs[0].fee_bps = Some(raw);
            let map = build_cartridge_pool_data(&intent, None);
            let mut engine = rhai::Engine::new();
            crate::cartridge::host_bindings::register_v3_amount_out_bindings(&mut engine);
            for direction in [true, false] {
                let expected = crate::amm_math::v3_amount_out_single_tick(
                    U256::exp10(12),
                    U256::one() << 96,
                    U256::exp10(24),
                    raw,
                    direction,
                )
                .to_string();
                let mut scope = rhai::Scope::new();
                scope.push("pd", map.clone());
                scope.push("amount", amount.clone());
                scope.push("sqrt", sqrt.clone());
                scope.push("liquidity", liquidity.clone());
                scope.push("direction", direction);
                for script in [
                    "v3_amount_out_single_tick_pips(amount,sqrt,liquidity,pd.fee_pips,direction)",
                    "v3_amount_out_single_tick(amount,sqrt,liquidity,pd.fee_bps,direction)",
                ] {
                    let result = engine
                        .eval_with_scope::<String>(&mut scope, script)
                        .unwrap();
                    assert_eq!(result, expected, "raw={raw}, script={script}");
                }
            }
        }
    }
}

#[cfg(test)]
mod v4_manifest_tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)] // test module — panics are acceptable
    use super::*;

    const SAMPLE_V4: &str = r#"fn agent_manifest() {
    #{
        "contract": "arbx.cartridge.agent/4",
        "mev_id": "MEV-01-002",
        "detector_id": "R_CLOSED_CYCLE",
        "source_digest": "abce7b713e0839bc99a162331e0768d364fa13681a18b7e2f5948538e20be891",
    }
}"#;

    #[test]
    fn admits_sealed_script_with_consistent_manifest() {
        let (mev, digest) = v4_admit_script(SAMPLE_V4).expect("v4 script must be admitted");
        assert_eq!(mev, "MEV-01-002");
        assert_eq!(
            digest,
            "abce7b713e0839bc99a162331e0768d364fa13681a18b7e2f5948538e20be891"
        );
    }

    #[test]
    fn rejects_v3_script_without_v4_contract() {
        let v3 = r#"fn agent_manifest() { #{ "mev_id": "MEV-01-002", "source_digest": "abce7b713e0839bc99a162331e0768d364fa13681a18b7e2f5948538e20be891", } }"#;
        assert!(v4_admit_script(v3).is_none(), "v3 scripts are not admitted");
    }

    #[test]
    fn rejects_ambiguous_mev_id() {
        let ambiguous = format!("{SAMPLE_V4} let other = #{{ \"mev_id\": \"MEV-01-016\" }};");
        assert!(v4_admit_script(&ambiguous).is_none());
    }

    #[test]
    fn rejects_malformed_digest() {
        let short = SAMPLE_V4.replace(
            "abce7b713e0839bc99a162331e0768d364fa13681a18b7e2f5948538e20be891",
            "abce7b71",
        );
        assert!(v4_admit_script(&short).is_none(), "digest must be 64 hex");
        let non_hex = SAMPLE_V4.replace(
            "abce7b713e0839bc99a162331e0768d364fa13681a18b7e2f5948538e20be891",
            &"z".repeat(64),
        );
        assert!(v4_admit_script(&non_hex).is_none(), "digest must be hex");
    }

    #[test]
    fn deployed_library_is_fully_admitted() {
        // Fase 3b/#655 (sync incremental): TODO script v4-sellado desplegado
        // en strategies/ debe estar admissionado — en este PR la wave B
        // completa (mev_03_*/mev_04_*, 62 scripts); las waves restantes
        // llegan con la migración de SUS fixtures (los scripts v3 no
        // participan: sin contrato v4 no hay admisión).
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("cartridges");
        let map = v4_scan_manifest_digests(&dir);
        let strategies = dir.join("strategies");
        let mut v4_deployed = 0usize;
        for entry in std::fs::read_dir(&strategies).expect("strategies dir must exist") {
            let path = entry.expect("dir entry").path();
            if path.extension().and_then(|e| e.to_str()) != Some("rhai") {
                continue;
            }
            let source = std::fs::read_to_string(&path).expect("read script");
            if source.contains("arbx.cartridge.agent/4") {
                v4_deployed += 1;
            }
        }
        assert!(
            v4_deployed >= 62,
            "wave B = 62 v4 scripts, found {v4_deployed}"
        );
        assert_eq!(
            map.len(),
            v4_deployed,
            "todo script v4 desplegado debe estar admissionado"
        );
        assert!(
            map.contains_key("MEV-03-001"),
            "MEV-03-001 must be admitted"
        );
        assert!(
            map.contains_key("MEV-04-031"),
            "MEV-04-031 must be admitted"
        );
        for (mev, digest) in &map {
            assert_eq!(digest.len(), 64, "{mev}: digest must be 64 hex");
        }
    }
}

#[cfg(test)]
mod v4_edge_protocol_tests {
    use super::*;

    fn leg(
        protocol_type: ProtocolType,
        fee_bps: Option<u32>,
    ) -> crate::route_intent::RouteIntentLeg {
        crate::route_intent::RouteIntentLeg {
            token_in: Default::default(),
            token_out: Default::default(),
            pool_hint: None,
            dex_hint: None,
            fee_bps,
            protocol_type,
        }
    }

    #[test]
    fn v2_leg_maps_to_cpmm_with_intent_fee() {
        let (protocol, units, den) = v4_edge_protocol_and_fee(&leg(ProtocolType::V2, Some(30)));
        assert_eq!(protocol, "cpmm_v2");
        assert_eq!(units, Some(30));
        assert_eq!(den, Some(10_000));
    }

    #[test]
    fn v2_leg_without_fee_stays_honest() {
        // Sin fee declarada el edge sigue siendo cpmm_v2 pero sin fee:
        // el quote fallará con missing_fee_units, jamás una fee inferida.
        let (protocol, units, den) = v4_edge_protocol_and_fee(&leg(ProtocolType::V2, None));
        assert_eq!(protocol, "cpmm_v2");
        assert_eq!(units, None);
        assert_eq!(den, None);
    }

    #[test]
    fn v3_leg_carries_pips_fee_for_within_tick() {
        // fee_bps de V3 son PIPS con denominator 1_000_000 — se portan al
        // edge en SU formato para la rama within-tick de quote_path (que
        // igualmente exige slot0 cacheado; sin slot0 → productor exacto).
        let (protocol, units, den) = v4_edge_protocol_and_fee(&leg(ProtocolType::V3, Some(3_000)));
        assert_eq!(protocol, "uniswap_v3");
        assert_eq!(units, Some(3_000));
        assert_eq!(den, Some(1_000_000));
        let (protocol, units, den) = v4_edge_protocol_and_fee(&leg(ProtocolType::V3, None));
        assert_eq!(protocol, "uniswap_v3");
        assert_eq!(units, None);
        assert_eq!(den, None);
    }

    #[test]
    fn other_families_keep_nominal_protocol_without_fee() {
        for (pt, expected) in [
            (ProtocolType::Curve, "curve"),
            (ProtocolType::Balancer, "balancer"),
            (ProtocolType::Unknown, "unknown"),
        ] {
            let (protocol, units, den) = v4_edge_protocol_and_fee(&leg(pt, Some(30)));
            assert_eq!(protocol, expected);
            assert_eq!(units, None);
            assert_eq!(den, None);
        }
    }

    // ── BASKET-WORKER-01 (2026-10-03): relevancia de baskets ────────────

    /// Arista mínima: sólo `token_in`/`token_out` importan a la relevancia.
    fn basket_edge(token_in: &str, token_out: &str) -> crate::agent_graph::Edge {
        crate::agent_graph::Edge {
            edge_id: "0xpool".into(),
            pool_id: "0xpool".into(),
            chain_id: 1,
            token_in: token_in.into(),
            token_out: token_out.into(),
            protocol: "cpmm_v2".into(),
            snapshot_id: "snap".into(),
            block_hash: "sync-ts-1".into(),
            reserve_in_raw: Some("1".into()),
            reserve_out_raw: Some("1".into()),
            fee_units: Some(30),
            fee_denominator: Some(10_000),
            token_in_decimals: 18,
            token_out_decimals: 18,
            adapter_version: "reserves_cache_v1".into(),
            sqrt_price_x96_raw: None,
            liquidity: None,
        }
    }

    #[test]
    fn redemption_tokens_are_first_token_in_and_last_token_out() {
        // Los DOS tokens que `SnapshotServices::verify` consulta en
        // `redemption_state` — los del mismo grafo que va al bundle.
        let edges = vec![basket_edge("0xAAA", "0xBBB"), basket_edge("0xBBB", "0xCCC")];
        assert_eq!(v4_redemption_tokens(&edges), vec!["0xaaa", "0xccc"]);
    }

    #[test]
    fn redemption_tokens_collapse_when_the_cycle_closes_on_one_token() {
        // Ciclo cerrado A→B→A: primera `in` y última `out` son el MISMO token
        // (una sola consulta, sin duplicar la lectura on-chain).
        let edges = vec![basket_edge("0xAAA", "0xBBB"), basket_edge("0xBBB", "0xAAA")];
        assert_eq!(v4_redemption_tokens(&edges), vec!["0xaaa"]);
    }

    #[test]
    fn redemption_tokens_of_an_empty_graph_are_empty() {
        // Sin grafo no hay tokens: el llamador devuelve mapa vacío sin RPC.
        assert!(v4_redemption_tokens(&[]).is_empty());
    }

    #[test]
    fn relevant_baskets_keep_only_tokens_present_in_the_route() {
        let configured = vec!["0xaaa".to_string(), "0xbbb".to_string()];
        let lookup = vec!["0xaaa".to_string()];
        assert_eq!(v4_relevant_baskets(&configured, &lookup), vec!["0xaaa"]);
    }

    #[test]
    fn relevant_baskets_is_empty_without_a_basket_in_the_route() {
        // Gate de COSTE: sin basket del operador en la ruta el mapa queda
        // vacío y NO se hace ninguna llamada RPC (hot path intacto).
        let lookup = vec!["0xaaa".to_string(), "0xccc".to_string()];
        assert!(v4_relevant_baskets(&["0xdead".to_string()], &lookup).is_empty());
        assert!(v4_relevant_baskets(&[], &lookup).is_empty());
    }
}

/// SHADOW-CANONICAL-01 — la ruta SHADOW evalúa contra el MISMO sustrato de datos
/// que la ACTIVE (§34.1 hot-path mode-invariant).
///
/// Los tests de este módulo son PUROS (sin Redis, sin RPC, sin reloj): la
/// frontera lectura/composición deja el grafo y el bundle compuestos en
/// funciones deterministas, de modo que se puede afirmar exactamente qué datos
/// consumió una evaluación — que es justo lo que estaba indocumentado cuando
/// todas las filas del stream describían el stub de boot.
#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)] // test module — panics are acceptable
mod shadow_canonical_tests {
    use super::*;
    use crate::rhai_agent_bridge::AgentServices;
    use ethers::types::{Address, H256, U256};
    use std::collections::HashMap;

    const CHAIN: u64 = 1;
    const CONTEXT_ID: &str = "intent-shadow-canonical-test";
    const MEV_ID: &str = "MEV-TEST-01";
    const DIGEST: &str = "abce7b713e0839bc99a162331e0768d364fa13681a18b7e2f5948538e20be891";
    /// Importe del intent, en unidades base — el ÚNICO tamaño del schedule.
    const AMOUNT_RAW: &str = "1000";
    /// `ts` del round de sync compartido por las dos piernas (identidad de
    /// coherencia del grafo; nunca un hash fabricado).
    const SYNC_TS: u64 = 1_700_000_000;

    fn tok(n: u64) -> Address {
        Address::from_low_u64_be(n)
    }

    fn hex(a: Address) -> String {
        format!("{a:#x}")
    }

    /// Ciclo cerrado A→B→A (dos piernas V2 con fee de 30 bps) — la forma que el
    /// dispatcher produce para una ruta de dos tokens.
    fn cycle_intent() -> RouteIntent {
        use crate::route_intent::{RouteIntentLeg, RouterKind, SwapExactMode};
        let mk = |a: Address, b: Address, pool: Address| RouteIntentLeg {
            token_in: a,
            token_out: b,
            pool_hint: Some(pool),
            dex_hint: None,
            fee_bps: Some(30),
            protocol_type: ProtocolType::V2,
        };
        RouteIntent::new(
            CHAIN,
            H256::from_low_u64_be(0xC0FFEE),
            Address::zero(),
            RouterKind::UniswapV2,
            Address::zero(),
            vec![
                mk(tok(0xA), tok(0xB), tok(0x1)),
                mk(tok(0xB), tok(0xA), tok(0x2)),
            ],
            U256::from(1000u64),
            None,
            SwapExactMode::ExactIn,
            crate::route_intent::DetectionSource::NewBlock,
        )
        .expect("valid intent")
    }

    /// Lecturas RESUELTAS (reservas orientadas + decimales) para las dos piernas.
    fn resolved_reads(intent: &RouteIntent) -> Vec<IntentLegRead> {
        vec![
            IntentLegRead::Ready {
                pool_id: hex(intent.legs[0].pool_hint.unwrap()),
                body: IntentLegBody::V2 {
                    reserve_in_raw: U256::exp10(21).to_string(),
                    reserve_out_raw: U256::exp10(20).to_string(),
                    sync_ts: SYNC_TS,
                },
                decimals: (18, 6),
            },
            IntentLegRead::Ready {
                pool_id: hex(intent.legs[1].pool_hint.unwrap()),
                body: IntentLegBody::V2 {
                    reserve_in_raw: U256::exp10(20).to_string(),
                    reserve_out_raw: U256::exp10(21).to_string(),
                    sync_ts: SYNC_TS,
                },
                decimals: (6, 18),
            },
        ]
    }

    /// Config del operador con los TRES productores de coste declarados: gas
    /// (unidades + precio base), financiación (tasa flash) y precios por token.
    fn operator_cfg() -> shared_rs::trading_config::TradingConfigState {
        use shared_rs::trading_config::TradingConfigState;
        TradingConfigState {
            chain_id: CHAIN,
            capital_usd: 10_000.0,
            base_token_symbol: "WETH".into(),
            base_token_price_usd: 3_000.0,
            allowed_token_symbols: vec![],
            token_prices_usd: HashMap::new(),
            simulation_capital_usd: None,
            simulation_per_token_amounts_usd: HashMap::new(),
            simulation_per_strategy_caps_usd: HashMap::new(),
            simulation_target_profit_usd: None,
            simulation_target_roi_pct: None,
            min_profit_usd: 0.01,
            min_roi_pct: 0.0,
            min_landing_probability: 0.0,
            min_liquidity_confidence: 0.0,
            max_token_risk_score: 1.0,
            gas_price_strategy: shared_rs::trading_config::GasPriceStrategy::Fixed,
            fixed_gas_price_gwei: Some(20.0),
            gas_estimate_units: 200_000,
            max_slippage_pct: 1.0,
            failure_risk_buffer_pct: 0.001,
            flashloan_fee_pct: 0.09,
            enabled_strategies: vec!["dex_arb".into()],
            enabled_dex_ids: None,
            strategy_configs: HashMap::new(),
            capital_cost_rate_annual_pct: 0.0,
            ops_overhead_usd_per_attempt: 0.0,
            spread_sanity_mult: 3.0,
            p_copied_volume_threshold_usd: 1_000_000.0,
            p_copied_max: 0.5,
            lp_fee_default_pct: 0.003,
            kelly_multiplier: 0.5,
            kelly_max_per_trade_fraction: 1.0,
            kelly_gas_safety_multiplier: 1.0,
            enabled: true,
            updated_at: chrono::Utc::now(),
            updated_by: None,
        }
    }

    /// Índice de identidad address-keyed del grafo: A→WETH, B→USDC.
    fn identity() -> shared_rs::token_identity::TokenIdentityIndex {
        shared_rs::token_identity::TokenIdentityIndex::resolve(
            CHAIN,
            &[],
            &[
                (hex(tok(0xA)), "WETH".to_string()),
                (hex(tok(0xB)), "USDC".to_string()),
            ],
        )
    }

    fn manifests() -> std::collections::BTreeMap<String, String> {
        let mut m = std::collections::BTreeMap::new();
        m.insert(MEV_ID.to_string(), DIGEST.to_string());
        m
    }

    /// Bundle REAL compuesto por la MISMA función que usan las rutas ACTIVE y
    /// EXACT-QUOTES-PRODUCER-01 — el productor NO es INERTE y la cadena es
    /// AMOUNT-BOUND. El I/O del quoter se inyecta (así la cadena se prueba sin
    /// red) y el mapa resultante se pasa por el `quote_path` REAL — con la
    /// precedencia ya invertida por AGENT-GRAPH-QUOTE-01 — para demostrar que una
    /// pierna V3 **con slot0** pasa a `protocol_exact_integer`: exactamente lo que
    /// era imposible mientras el mapa estaba vacío.
    #[tokio::test]
    async fn exact_quotes_producer_chains_and_certifies_a_v3_leg_with_slot0() {
        let amount = "1000000000000000000";
        let (pool_v2, pool_v3) = (hex(tok(0x11)), hex(tok(0x22)));
        let (tok_a, tok_b) = (hex(tok(0xAA)), hex(tok(0xBB)));
        let block = format!("sync-ts-{SYNC_TS}");
        let v2 = crate::agent_graph::Edge {
            edge_id: pool_v2.clone(),
            pool_id: pool_v2.clone(),
            chain_id: CHAIN,
            token_in: tok_a.clone(),
            token_out: tok_b.clone(),
            protocol: "cpmm_v2".into(),
            snapshot_id: CONTEXT_ID.into(),
            block_hash: block.clone(),
            reserve_in_raw: Some(U256::exp10(24).to_string()),
            reserve_out_raw: Some(U256::exp10(24).to_string()),
            fee_units: Some(30),
            fee_denominator: Some(10_000),
            token_in_decimals: 18,
            token_out_decimals: 18,
            adapter_version: "reserves_cache_v1".into(),
            sqrt_price_x96_raw: None,
            liquidity: None,
        };
        // Pierna V3 CON slot0: es la que hoy sale por la hipótesis within-tick.
        let v3 = crate::agent_graph::Edge {
            edge_id: pool_v3.clone(),
            pool_id: pool_v3.clone(),
            chain_id: CHAIN,
            token_in: tok_b.clone(),
            token_out: tok_a.clone(),
            protocol: "uniswap_v3".into(),
            snapshot_id: CONTEXT_ID.into(),
            block_hash: block,
            reserve_in_raw: None,
            reserve_out_raw: None,
            fee_units: Some(3_000),
            fee_denominator: Some(1_000_000),
            token_in_decimals: 18,
            token_out_decimals: 18,
            adapter_version: "reserves_cache_v1".into(),
            sqrt_price_x96_raw: Some("79228162514264337593543950336".into()),
            liquidity: Some(1_000_000_000_000_000_000_000),
        };
        // El importe que ENTRA en la pierna 1 es la SALIDA de la pierna 0,
        // computada con la MISMA función que usa el ledger.
        let chained = crate::agent_graph::cpmm_exact_in(
            amount,
            &U256::exp10(24).to_string(),
            &U256::exp10(24).to_string(),
            30,
            10_000,
        )
        .expect("la pierna CPMM del fixture cotiza");

        // (1) CON quoter: el mapa NO queda vacío, la clave encadena la salida
        //     real de la pierna 0 y el ledger lo CERTIFICA pese al slot0.
        let quoted = v4_exact_quotes(
            &[v2.clone(), v3.clone()],
            amount,
            V4_EXACT_QUOTE_BUDGET_MS,
            |req| {
                assert_eq!(req.fee_bps, 3_000, "el fee de la pierna viaja al quoter");
                assert_eq!(req.pool_addr, tok(0x22), "el quoter recibe ESA pierna");
                assert_eq!(
                    req.amount_in,
                    U256::from_dec_str(&chained).unwrap(),
                    "el quoter recibe el importe ENCADENADO"
                );
                async move { Ok("700000000000000000".to_string()) }
            },
        )
        .await;
        assert!(quoted.gaps.is_empty(), "sin huecos: {:?}", quoted.gaps);
        assert_eq!(quoted.quotes.len(), 1, "sólo la pierna V3 pide quote");
        assert_eq!(quoted.chained_until_raw, "700000000000000000");
        let entry = quoted
            .quotes
            .get(&crate::agent_graph::quote_request_key(&v3, &chained))
            .expect("la clave debe usar la SALIDA real de la pierna 0 (amount-bound)");
        assert_eq!(entry.amount_in_raw, chained);
        assert_eq!(entry.amount_out_raw, "700000000000000000");
        assert_eq!(entry.precision, "protocol_exact_integer");
        assert!(entry.fees_and_impact_embedded);
        assert!(!entry.quote_id.is_empty(), "procedencia obligatoria");
        assert_eq!(entry.adapter_version, v3.adapter_version);

        let legs =
            crate::agent_graph::quote_path(&[v2.clone(), v3.clone()], amount, &quoted.quotes)
                .expect("la ruta encadenada cotiza");
        assert_eq!(legs[0]["quote_method"], "cpmm_exact_integer");
        assert_eq!(
            legs[1]["quote_method"], "protocol_exact_integer",
            "el productor activo convierte la hipótesis within-tick en quote certificada"
        );
        assert_eq!(legs[1]["amount_out_raw"], "700000000000000000");

        // (2) SIN quoter: NADA se inserta, la ausencia se declara con su razón
        //     CONCRETA y el respaldo within-tick sigue cotizando — cero regresión
        //     para lo que ya funcionaba.
        let unquoted = v4_exact_quotes(
            &[v2.clone(), v3.clone()],
            amount,
            V4_EXACT_QUOTE_BUDGET_MS,
            |_req| async { Err("rpc_unavailable".to_string()) },
        )
        .await;
        assert!(unquoted.quotes.is_empty(), "sin fuente NO se inserta nada");
        assert_eq!(unquoted.gaps.len(), 1);
        assert_eq!(unquoted.gaps[0].index, 1);
        assert_eq!(unquoted.gaps[0].reason, "v3_protocol_quote_unavailable");
        assert_eq!(unquoted.gaps[0].detail, "rpc_unavailable");
        assert_eq!(
            unquoted.chained_until_raw, chained,
            "la cadena se detiene donde se detuvo la evidencia, sin inventar salida"
        );
        let legs = crate::agent_graph::quote_path(&[v2, v3], amount, &unquoted.quotes)
            .expect("el respaldo local sigue cotizando");
        assert_eq!(legs[1]["quote_method"], "v3_spot_within_tick");
    }

    /// SHADOW (`build_v4_intent_bundle`) desde el grafo compuesto.
    fn real_bundle() -> crate::snapshot_services::SnapshotBundle {
        let intent = cycle_intent();
        let composition =
            compose_intent_edges(CHAIN, CONTEXT_ID, &intent, &resolved_reads(&intent));
        assert_eq!(
            composition.edges.len(),
            2,
            "las dos piernas resueltas deben entrar al grafo"
        );
        let mut price_snapshot: HashMap<String, f64> = HashMap::new();
        price_snapshot.insert("WETH".to_string(), 3_000.0);
        price_snapshot.insert("USDC".to_string(), 1.0);
        build_v4_intent_bundle(
            CHAIN,
            CONTEXT_ID,
            composition.edges,
            &hex(tok(0xA)),
            AMOUNT_RAW,
            &operator_cfg(),
            &identity(),
            &price_snapshot,
            &manifests(),
            // gas OBSERVADO del runner (el getter ya decodifica milli-gwei).
            20.0,
            &std::collections::BTreeMap::new(),
            // PRICE-COVERAGE-01: sin bus en este fixture — el snapshot ya cubre
            // WETH/USDC, así que la tercera fuente no participa (y su ausencia
            // es el degradado honesto, no una carencia del test).
            None,
            // EXACT-QUOTES-PRODUCER-01: sin quotes de protocolo en este fixture
            // (el mapa estaba vacío antes de este PR; el test no cambia eso).
            std::collections::BTreeMap::new(),
        )
        .expect("un bundle con grafo y config reales SIEMPRE se compone")
    }

    // ── PRICE-COVERAGE-01 (§38): sustrato del mapa canónico de precios ──────
    //
    // El defecto medido: en producción, un token del grafo (LAR, pair
    // LAR/WETH) tenía IDENTIDAD (`arbx:tokens:1:<addr>` → `{"symbol":"LAR",…}`)
    // pero NINGUNA fuente de precio, así que el bundle salía sin su entrada y
    // la cascada `amount_in_usd` → `capital_usd` → cashflows → costes → quote →
    // operadores quedaba en DATA_GAP. Estos tests fijan las tres propiedades
    // que el PR debe garantizar: la tercera fuente SÍ valora, la ausencia
    // honesta se conserva, y jamás entra un valor fabricado.

    /// Bundle compuesto con las TRES fuentes de precio fijadas por el test:
    /// snapshot de Redis, `token_prices_usd` del operador y bus en proceso.
    fn bundle_with_price_sources(
        price_snapshot: &HashMap<String, f64>,
        token_prices_usd: &HashMap<String, f64>,
        price_bus: Option<&shared_rs::price_bus::PriceBus>,
    ) -> crate::snapshot_services::SnapshotBundle {
        let intent = cycle_intent();
        let composition =
            compose_intent_edges(CHAIN, CONTEXT_ID, &intent, &resolved_reads(&intent));
        assert_eq!(
            composition.edges.len(),
            2,
            "las dos piernas resueltas deben entrar al grafo"
        );
        let mut cfg = operator_cfg();
        cfg.token_prices_usd = token_prices_usd.clone();
        build_v4_intent_bundle(
            CHAIN,
            CONTEXT_ID,
            composition.edges,
            &hex(tok(0xA)),
            AMOUNT_RAW,
            &cfg,
            &identity(),
            price_snapshot,
            &manifests(),
            20.0,
            &std::collections::BTreeMap::new(),
            price_bus,
            std::collections::BTreeMap::new(),
        )
        .expect("un bundle con grafo y config reales SIEMPRE se compone")
    }

    fn now_secs() -> u64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0)
    }

    fn now_ns() -> u64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos() as u64)
            .unwrap_or(0)
    }

    /// Un anchor Chainlink fresco para el símbolo dado (recv reciente, round
    /// del aggregator hace 10 s — dentro de la ventana de los estables).
    fn anchor(answer: f64) -> shared_rs::price_bus::Anchor {
        shared_rs::price_bus::Anchor {
            answer,
            updated_at: now_secs().saturating_sub(10),
            recv_ns: now_ns(),
        }
    }

    /// Precio del token en el mapa del bundle, tal como lo verá
    /// `SnapshotServices::price`.
    fn price_of(
        bundle: &crate::snapshot_services::SnapshotBundle,
        token: Address,
    ) -> Option<&crate::snapshot_services::CanonicalPrice> {
        bundle.prices.get(&(CHAIN, hex(token)))
    }

    /// Espejo EXACTO de las condiciones que `SnapshotServices::price()` exige
    /// antes de servir un `CanonicalPrice` (`snapshot_services.rs:831-853`).
    ///
    /// `price` es un método de un trait PRIVADO de `snapshot_services`, así que
    /// no es invocable desde aquí — y este PR tiene prohibido tocar ese módulo
    /// más allá de lo que exige la procedencia. El espejo fija las MISMAS
    /// condiciones, en el MISMO orden, devolviendo la MISMA razón de error, de
    /// modo que un test pueda afirmar "el gate de procedencia se satisface" sin
    /// debilitarlo ni ensancharlo. Si el contrato cambia, este espejo deja de
    /// representarlo y el test debe actualizarse junto con él.
    fn price_contract_violation(
        bundle: &crate::snapshot_services::SnapshotBundle,
        token: &str,
        now_ms: u64,
    ) -> Option<&'static str> {
        let Some(p) = bundle.prices.get(&(bundle.chain_id, token.to_owned())) else {
            return Some("canonical_price_missing");
        };
        if p.producer != "PriceBus"
            || p.evidence_id.is_empty()
            || p.revision != bundle.policy.price_revision
            || p.token_address != token
            || p.chain_id != bundle.chain_id
        {
            return Some("canonical_price_provenance_mismatch");
        }
        if now_ms < p.observed_at_ms
            || now_ms > p.valid_until_ms
            || p.valid_until_ms < p.observed_at_ms
        {
            return Some("canonical_price_stale_or_future");
        }
        match crate::rhai_agent_bridge::Usd::parse(&p.usd) {
            Err(_) => Some("invalid_usd_decimal"),
            Ok(u) if !u.is_positive() => Some("canonical_price_nonpositive"),
            Ok(_) => None,
        }
    }

    fn now_ms() -> u64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0)
    }

    #[test]
    fn price_bus_evidence_ids_mirror_the_verdict_slugs() {
        // Anti-deriva: el `evidence_id` estático debe seguir nombrando EXACTAMENTE
        // el veredicto que el bus reporta (su slug público), para que la
        // evidencia insertada sea legible con las mismas herramientas que leen
        // `Verdict::as_str()` en el resto del sistema.
        use shared_rs::price_bus::Verdict;
        for v in [
            Verdict::Ok,
            Verdict::StaleBinance,
            Verdict::StaleAnchor,
            Verdict::DivergenceFrozen,
            Verdict::NoSource,
        ] {
            assert_eq!(
                v4_price_bus_evidence_id(v),
                format!("price_bus:{}", v.as_str()),
                "el evidence_id de {v:?} debe espejar su slug"
            );
            assert!(
                !v4_price_bus_evidence_id(v).is_empty(),
                "price() exige un evidence_id no vacío"
            );
        }
    }

    #[test]
    fn bus_source_fills_a_token_missing_from_snapshot_and_config() {
        // Snapshot de Redis y `token_prices_usd` VACÍOS: antes de
        // PRICE-COVERAGE-01 el bundle salía sin ninguna entrada. El bus en
        // proceso (misma pila soberana que alimenta el snapshot) sí tiene los
        // dos tokens vivos, así que ahora se valoran.
        use shared_rs::price_bus::{BookTicker, PriceBus, PriceBusConfig};
        let bus = PriceBus::new(PriceBusConfig::default());
        // El quote del par ETHUSDC necesita su propio anchor fresco (USDC).
        bus.update_anchor("USDC", anchor(0.9998));
        // bookTicker de Binance: WETH se valora con el BID (lo que se puede
        // vender AHORA — el lado conservador), no con el mid.
        bus.update_binance(
            "ETHUSDC",
            BookTicker {
                bid: 2_625.46,
                ask: 2_625.47,
                event_ms: 0,
                recv_ns: now_ns(),
            },
        );
        let bundle =
            bundle_with_price_sources(&HashMap::new(), &HashMap::new(), Some(bus.as_ref()));
        assert_eq!(
            bundle.prices.len(),
            2,
            "los dos tokens del ciclo deben quedar valorados por el bus"
        );

        // WETH ← Binance bid × anchor del quote. Sin anchor PROPIO de WETH el
        // veredicto es `stale_anchor`: la evidencia lo DECLARA (precio servido
        // sin verificación de anchor) en vez de disfrazarlo de verificado.
        let weth = price_of(&bundle, tok(0xA)).expect("WETH valorado por el bus");
        assert_eq!(weth.producer, "PriceBus");
        assert_eq!(weth.evidence_id, "price_bus:stale_anchor");
        assert_eq!(weth.chain_id, CHAIN);
        assert_eq!(weth.token_address, hex(tok(0xA)));
        let expected = 2_625.46_f64 * 0.9998;
        let got: f64 = weth.usd.parse().expect("usd es decimal plano");
        assert!(
            (got - expected).abs() < 1e-9,
            "WETH = {got}, esperado {expected}"
        );

        // USDC ← su anchor Chainlink (no hay libro Binance USDCUSDT en el bus).
        let usdc = price_of(&bundle, tok(0xB)).expect("USDC valorado por el bus");
        assert_eq!(usdc.producer, "PriceBus");
        assert_eq!(usdc.evidence_id, "price_bus:stale_binance");
        assert_eq!(usdc.usd, "0.9998");

        // El contrato `SnapshotServices::price` se satisface SIN tocarlo:
        // producer "PriceBus" (el bus ES literalmente el productor), evidencia
        // no vacía, revisión compartida y ventana temporal coherente. El espejo
        // devuelve la MISMA razón que devolvería `price()`; aquí debe ser `None`
        // (= serviría el precio) para los DOS tokens.
        let now = now_ms();
        for token in [tok(0xA), tok(0xB)] {
            assert_eq!(
                price_contract_violation(&bundle, &hex(token), now),
                None,
                "el precio del bus debe pasar el gate de procedencia sin relajarlo"
            );
        }
    }

    #[test]
    fn snapshot_and_config_still_outrank_the_bus() {
        // La tercera fuente NO desplaza a las dos primeras: si el snapshot (o
        // la config del operador) ya trae el token, ese valor manda y la
        // evidencia sigue siendo la de siempre. PRICE-COVERAGE-01 sólo AÑADE
        // cobertura; no reordena las fuentes existentes.
        use shared_rs::price_bus::{PriceBus, PriceBusConfig};
        let bus = PriceBus::new(PriceBusConfig::default());
        bus.update_anchor("USDC", anchor(0.5)); // valor deliberadamente distinto
        let mut snapshot: HashMap<String, f64> = HashMap::new();
        snapshot.insert("USDC".to_string(), 0.999_844_88);
        let bundle = bundle_with_price_sources(&snapshot, &HashMap::new(), Some(bus.as_ref()));
        let usdc = price_of(&bundle, tok(0xB)).expect("USDC");
        assert_eq!(usdc.usd, "0.99984488", "el snapshot debe ganarle al bus");
        assert_eq!(usdc.evidence_id, "price_snapshot");
    }

    #[test]
    fn a_token_absent_from_every_source_keeps_no_entry() {
        // Bus VIVO pero sin dato para WETH ni USDC, snapshot y config vacíos:
        // SIN entrada. Es el caso LAR medido en producción — identidad sí,
        // precio no — y la respuesta honesta es `None`, jamás un relleno.
        use shared_rs::price_bus::{PriceBus, PriceBusConfig};
        let bus = PriceBus::new(PriceBusConfig::default());
        let bundle =
            bundle_with_price_sources(&HashMap::new(), &HashMap::new(), Some(bus.as_ref()));
        assert!(
            bundle.prices.is_empty(),
            "sin precio real positivo no se inserta NINGUNA entrada (R8): {:?}",
            bundle.prices.keys().collect::<Vec<_>>()
        );

        // El `None` honesto se conserva: el espejo del contrato devuelve la
        // razón exacta que devolvería `price()` — no un precio de relleno.
        let now = now_ms();
        for token in [tok(0xA), tok(0xB)] {
            assert_eq!(
                price_contract_violation(&bundle, &hex(token), now),
                Some("canonical_price_missing"),
                "sin fuente real el token queda SIN precio (R8)"
            );
        }
    }

    #[test]
    fn bus_absent_degrades_to_the_two_redis_sources() {
        // `price_bus: None` (bus no inicializado) NO inventa nada: la
        // composición se queda con snapshot + config, exactamente la conducta
        // previa a este PR. Es el degradado honesto que documenta la firma.
        let mut snapshot: HashMap<String, f64> = HashMap::new();
        snapshot.insert("USDC".to_string(), 1.0);
        let bundle = bundle_with_price_sources(&snapshot, &HashMap::new(), None);
        assert_eq!(
            bundle.prices.len(),
            1,
            "sólo el token que el snapshot cubre"
        );
        assert_eq!(
            price_of(&bundle, tok(0xB)).map(|p| p.evidence_id.as_str()),
            Some("price_snapshot")
        );
        assert!(
            price_of(&bundle, tok(0xA)).is_none(),
            "WETH no está en ninguna de las dos fuentes ⇒ SIN entrada"
        );
    }

    #[test]
    fn bus_price_tracks_the_live_source_and_is_never_a_constant() {
        // El número insertado sigue a la fuente viva: dos anchors distintos
        // producen dos precios distintos. Un fallback constante (1.0, 0.0, o
        // cualquier valor fijo) daría el MISMO resultado en ambos casos, así
        // que este test es la cerradura anti-constante del PR.
        use shared_rs::price_bus::{PriceBus, PriceBusConfig};
        let priced_at = |answer: f64| {
            let bus = PriceBus::new(PriceBusConfig::default());
            bus.update_anchor("USDC", anchor(answer));
            let bundle =
                bundle_with_price_sources(&HashMap::new(), &HashMap::new(), Some(bus.as_ref()));
            let usdc = price_of(&bundle, tok(0xB)).cloned().expect("USDC");
            (usdc.usd, usdc.evidence_id)
        };
        let (low, ev_low) = priced_at(0.9998);
        let (high, ev_high) = priced_at(1_000_000.0);
        assert_ne!(
            low, high,
            "el precio debe seguir al anchor; un valor constante sería el delator"
        );
        assert_eq!(low, "0.9998");
        assert_eq!(high, "1000000");
        assert_eq!(ev_low, "price_bus:stale_binance");
        assert_eq!(ev_high, "price_bus:stale_binance");
    }

    #[test]
    fn a_frozen_bus_pair_yields_no_entry() {
        // Banda Binance↔Chainlink CONGELADA por divergencia: el bus devuelve
        // `None` (Verdict::DivergenceFrozen) y el bundle DEBE respetarlo — un
        // depeg o un feed roto para la valuación, no se promedia ni se cuela.
        //
        // Mecánica del bus (price_bus.rs `sample_divergence`): la banda se
        // muestrea cuando llega el dato del SÍMBOLO — un `bookTicker` de ETHUSDC
        // muestrea "ETH" y un anchor muestrea su propio símbolo. Para congelar
        // WETH hay que refrescar SU anchor con el libro ya desviado; por eso el
        // bucle calienta con `update_anchor("WETH", …)` y no con ETH a secas.
        use shared_rs::price_bus::{BookTicker, PriceBus, PriceBusConfig, Verdict};
        let bus = PriceBus::new(PriceBusConfig::default());
        bus.update_anchor("USDC", anchor(1.0)); // quote del par ETHUSDC
        let ticker = |bid: f64, ask: f64| BookTicker {
            bid,
            ask,
            event_ms: 0,
            recv_ns: now_ns(),
        };
        // Calienta la banda de WETH con la ventana de muestras del bus
        // (band_warmup = 30) y divergencias pequeñas: nada se congela.
        for i in 0..30 {
            bus.update_binance("ETHUSDC", ticker(2_600.0 + (i % 3) as f64 * 0.5, 2_601.0));
            bus.update_anchor("WETH", anchor(2_600.0));
        }
        assert_eq!(
            bus.view().price_with_verdict("WETH").1,
            Verdict::Ok,
            "la banda caliente con divergencia pequeña NO debe congelar"
        );
        // Libro ~3% desviado del anchor + refresco del anchor ⇒ la muestra cae
        // fuera de la banda (2σ, suelo 1%) ⇒ el par se CONGELA.
        bus.update_binance("ETHUSDC", ticker(2_680.0, 2_680.5));
        bus.update_anchor("WETH", anchor(2_600.0));
        assert_eq!(
            bus.view().price_with_verdict("WETH").1,
            Verdict::DivergenceFrozen,
            "el par WETH debe quedar congelado en el bus"
        );

        let bundle =
            bundle_with_price_sources(&HashMap::new(), &HashMap::new(), Some(bus.as_ref()));
        assert!(
            price_of(&bundle, tok(0xA)).is_none(),
            "un par congelado NO se valora: sin entrada, no un promedio"
        );
        // USDC no está congelado (sin libro Binance): su anchor sigue mandando,
        // y la evidencia declara que la valuación vino del anchor.
        assert_eq!(
            price_of(&bundle, tok(0xB)).map(|p| p.evidence_id.as_str()),
            Some("price_bus:stale_binance")
        );
    }

    fn spec() -> serde_json::Value {
        serde_json::json!({
            "mev_id": MEV_ID,
            "detector_id": "R_CLOSED_CYCLE",
            "source_digest": DIGEST,
            "logic": "closed_route",
            "allowed_search_hops": [2],
            // Sin roles de operador declarados: este fixture mide el sustrato de
            // COSTES, no el gate de operadores nativos.
            "operator_requirements": [],
        })
    }

    fn ctx() -> serde_json::Value {
        serde_json::json!({ "context_id": CONTEXT_ID, "snapshot_id": CONTEXT_ID })
    }

    /// `field::reason` de cada repair — la forma exacta que publica el stream
    /// (`v4_repairs.repairs[]`).
    fn repair_tags(repairs: &serde_json::Value) -> Vec<String> {
        repairs
            .as_array()
            .map(|rows| {
                rows.iter()
                    .filter_map(|r| {
                        Some(format!(
                            "{}::{}",
                            r.get("field")?.as_str()?,
                            r.get("reason")?.as_str()?
                        ))
                    })
                    .collect()
            })
            .unwrap_or_default()
    }

    // ── Composición del grafo (frontera pura) ───────────────────────────────

    #[test]
    fn resolved_legs_compose_a_real_graph_for_the_intent() {
        let intent = cycle_intent();
        let c = compose_intent_edges(CHAIN, CONTEXT_ID, &intent, &resolved_reads(&intent));

        assert_eq!(c.edges.len(), 2, "una pierna resuelta por leg");
        assert!(
            c.skip_reasons.is_empty(),
            "sin omisiones: {:?}",
            c.skip_reasons
        );
        // Orientación EXACTA por intención de la pierna (no heurística).
        assert_eq!(c.edges[0].token_in, hex(intent.legs[0].token_in));
        assert_eq!(c.edges[0].token_out, hex(intent.legs[0].token_out));
        assert_eq!(c.edges[1].token_in, hex(intent.legs[1].token_in));
        // El fee del intent viaja con su PROTOCOLO (V2 = bps/10_000).
        assert_eq!(c.edges[0].fee_units, Some(30));
        assert_eq!(c.edges[0].fee_denominator, Some(10_000));
        assert_eq!(c.edges[0].protocol, "cpmm_v2");
        // Identidad de coherencia = ts del ROUND de sync compartido.
        assert_eq!(c.edges[0].block_hash, format!("sync-ts-{SYNC_TS}"));
        assert_eq!(c.edges[1].block_hash, c.edges[0].block_hash);
        // Decimales resueltos por pierna.
        assert_eq!(c.edges[0].token_in_decimals, 18);
        assert_eq!(c.edges[0].token_out_decimals, 6);
        // El start_token del bundle es el `token_in` de la PRIMERA pierna.
        assert_eq!(c.first_token_in.as_deref(), Some(hex(tok(0xA)).as_str()));
        // Todo edge queda ATADO al snapshot del intent.
        assert!(c.edges.iter().all(|e| e.snapshot_id == CONTEXT_ID));
    }

    /// PROTOCOL-COHERENCE-01 — la PARTICIÓN que decide si un intent se puede
    /// evaluar. Con las piernas resueltas en UN round el grafo lleva UNA sola
    /// identidad de bloque (ruta atómica coherente: el gate puede avanzar); con
    /// las piernas repartidas entre DOS rounds lleva DOS identidades — lo que
    /// `quote_path_progress` rechaza con `mixed_block_or_domain_in_atomic_route`,
    /// la razón medida en el gate v4. Se prueban las DOS direcciones: un test
    /// que sólo cubriera el caso coherente no probaría la partición.
    #[test]
    fn leg_round_alignment_partitions_coherent_from_split_reads() {
        let intent = cycle_intent();

        // (1) COHERENTE — el caso normal: el writer escribe la población con el
        //     MISMO `ts`, así que las dos piernas comparten round.
        let coherent = resolved_reads(&intent);
        assert!(
            intent_legs_share_one_round(&coherent),
            "dos piernas del mismo round son un grafo coherente"
        );
        assert!(leg_round_laggard_indices(&coherent).is_empty());
        assert_eq!(newest_leg_round(&coherent), Some(SYNC_TS));
        let composed_ok = compose_intent_edges(CHAIN, CONTEXT_ID, &intent, &coherent);
        assert_eq!(
            composed_ok.edges[0].block_hash, composed_ok.edges[1].block_hash,
            "un solo round ⇒ UNA identidad de bloque para la ruta atómica"
        );

        // (2) REPARTIDO — la rotación cae EN MEDIO de la lectura (medido en
        //     producción: la población queda partida, p.ej. 30 entradas en el
        //     bloque N y 527 en N+1). El predicado lo declara y nombra la
        //     rezagada...
        let mut split = resolved_reads(&intent);
        split[1] = IntentLegRead::Ready {
            pool_id: hex(intent.legs[1].pool_hint.unwrap()),
            body: IntentLegBody::V2 {
                reserve_in_raw: U256::exp10(20).to_string(),
                reserve_out_raw: U256::exp10(21).to_string(),
                sync_ts: SYNC_TS + 1,
            },
            decimals: (6, 18),
        };
        assert!(
            !intent_legs_share_one_round(&split),
            "piernas de dos rounds NO son un grafo coherente"
        );
        assert_eq!(leg_round_laggard_indices(&split), vec![0]);
        assert_eq!(newest_leg_round(&split), Some(SYNC_TS + 1));
        // ...y el grafo que se compondría lleva DOS identidades: el bundle no
        // puede afirmar coherencia sobre esto, por eso el llamador declara el
        // contexto NO COMPUTADO con `legs_span_multiple_sync_rounds`.
        let composed_split = compose_intent_edges(CHAIN, CONTEXT_ID, &intent, &split);
        assert_ne!(
            composed_split.edges[0].block_hash, composed_split.edges[1].block_hash,
            "dos rounds ⇒ dos identidades de bloque (lo que el contrato rechaza)"
        );
        assert_eq!(
            composed_split.edges[0].block_hash,
            format!("sync-ts-{SYNC_TS}")
        );
        assert_eq!(
            composed_split.edges[1].block_hash,
            format!("sync-ts-{}", SYNC_TS + 1)
        );

        // (3) Una pierna OMITIDA no tiene round: no puede desalinear el grafo ni
        //     contarse como rezagada.
        let with_skip = vec![
            IntentLegRead::Skip("missing_pool_hint"),
            IntentLegRead::Ready {
                pool_id: hex(intent.legs[0].pool_hint.unwrap()),
                body: IntentLegBody::V2 {
                    reserve_in_raw: U256::exp10(21).to_string(),
                    reserve_out_raw: U256::exp10(20).to_string(),
                    sync_ts: SYNC_TS,
                },
                decimals: (18, 6),
            },
            IntentLegRead::Skip("reserves_missing"),
            IntentLegRead::Ready {
                pool_id: hex(intent.legs[1].pool_hint.unwrap()),
                body: IntentLegBody::V2 {
                    reserve_in_raw: U256::exp10(20).to_string(),
                    reserve_out_raw: U256::exp10(21).to_string(),
                    sync_ts: SYNC_TS,
                },
                decimals: (6, 18),
            },
        ];
        assert!(
            intent_legs_share_one_round(&with_skip),
            "sólo cuentan las piernas resueltas: las omitidas no aportan round"
        );
        assert!(leg_round_laggard_indices(&with_skip).is_empty());

        // (4) 0 y 1 pierna resuelta: coherencia VACUA (no hay ruta que mezcle).
        assert!(intent_legs_share_one_round(&[]));
        assert_eq!(newest_leg_round(&[]), None);
        assert!(leg_round_laggard_indices(&[]).is_empty());
        assert!(intent_legs_share_one_round(&[IntentLegRead::Skip(
            "v3_slot0_missing"
        )]));
    }

    #[test]
    fn unresolved_legs_leave_an_honest_empty_graph_with_exact_reasons() {
        let intent = cycle_intent();
        // Ninguna lectura resolvió: cada pierna se omite con SU razón exacta.
        let reads = vec![
            IntentLegRead::Skip("reserves_missing"),
            IntentLegRead::Skip("token_in_decimals_missing"),
        ];
        let c = compose_intent_edges(CHAIN, CONTEXT_ID, &intent, &reads);

        assert!(c.edges.is_empty(), "sin lecturas resueltas no hay grafo");
        assert!(c.first_token_in.is_none());
        assert_eq!(c.skip_reasons.get("reserves_missing"), Some(&1));
        assert_eq!(c.skip_reasons.get("token_in_decimals_missing"), Some(&1));

        // Fallback honesto: sin grafo el contexto REAL no se compone, así que la
        // evaluación se sella con los ids DATA_GAP estáticos y queda declarada
        // como `static_boot_stub` — jamás presentada como bundle real.
        let outcome = IntentContextOutcome {
            guard: None,
            census: IntentContextCensus {
                edges_built: c.edges.len(),
                legs_skipped: c.skip_reasons.clone(),
            },
        };
        assert_eq!(outcome.census.edges_built, 0);
        assert_eq!(
            outcome.stamp_ids(CHAIN),
            (
                format!("boot-chain-{CHAIN}"),
                format!("boot-genesis-{CHAIN}")
            )
        );
    }

    #[test]
    fn the_guard_releases_the_router_slot_on_drop() {
        // El contexto por-intent ocupa una entrada del router (capacidad 256 en
        // producción): sin el guard con Drop, cada intent la agotaría hasta
        // reiniciar. Vale para AMBAS rutas, que ahora comparten el guard.
        let router = Arc::new(crate::context_router::ContextRouter::new(1).expect("router"));
        let rev: crate::snapshot_services::RevisionGuard = Arc::new(|_: &str, _: &str| true);
        let services =
            crate::snapshot_services::SnapshotServices::new(Arc::new(real_bundle()), rev)
                .expect("services");
        router
            .insert(CONTEXT_ID.to_string(), Arc::new(services))
            .expect("el router tiene capacidad para una entrada");
        {
            let _guard = IntentContextGuard {
                router: Some(router.clone()),
                context_id: CONTEXT_ID.to_string(),
            };
            assert_eq!(_guard.context_id(), CONTEXT_ID);
        }
        // Capacidad liberada: la MISMA entrada puede volver a registrarse.
        let rev2: crate::snapshot_services::RevisionGuard = Arc::new(|_: &str, _: &str| true);
        let services2 =
            crate::snapshot_services::SnapshotServices::new(Arc::new(real_bundle()), rev2)
                .expect("services");
        router
            .insert(CONTEXT_ID.to_string(), Arc::new(services2))
            .expect("el Drop del guard debe haber liberado la entrada");
    }

    // ── Sustrato de datos ⇒ costes (la medición del defecto) ────────────────

    #[test]
    fn real_bundle_costs_are_non_empty_and_carry_no_mandatory_cost_repair() {
        let bundle = real_bundle();
        let kinds: Vec<&str> = bundle
            .base_cost_lines
            .iter()
            .map(|l| l.kind.as_str())
            .collect();
        // El bundle REAL siempre declara los productores de coste: gas (unidades
        // de config × gas observado × precio base) y financiación (tasa del
        // operador sobre el importe del intent valorado).
        assert!(kinds.contains(&"gas"), "kinds = {kinds:?}");
        assert!(kinds.contains(&"financing"), "kinds = {kinds:?}");
        assert!(kinds.contains(&"execution_fees"), "kinds = {kinds:?}");
        assert!(
            bundle.base_cost_lines.iter().any(|l| l.usd.is_some()),
            "al menos una línea de coste debe traer su USD real"
        );

        let rev: crate::snapshot_services::RevisionGuard = Arc::new(|_: &str, _: &str| true);
        let services = crate::snapshot_services::SnapshotServices::new(Arc::new(bundle), rev);
        let services = services.expect("el bundle real debe ser admitido");
        let router = Arc::new(crate::context_router::ContextRouter::new(4).expect("router"));
        router
            .insert(CONTEXT_ID.to_string(), Arc::new(services))
            .expect("insert del contexto real");

        // La evaluación que hace un cartucho v4: descubrir sobre el grafo del
        // contexto y cotizar el candidato contra el MISMO contexto.
        let c = ctx();
        let discovery = router
            .discover(&c, &spec())
            .expect("discover sobre el grafo real");
        assert_eq!(discovery["status"], "READY");
        let candidate = discovery["candidates"]
            .as_array()
            .and_then(|v| v.first())
            .cloned()
            .expect("el ciclo cerrado A→B→A debe producir un candidato");
        let quote = router
            .quote(&c, &spec(), &candidate)
            .expect("quote del candidato real");
        let quote = serde_json::to_value(&quote).expect("quote serializable");

        let costs = quote["costs"]
            .as_array()
            .expect("el quote real trae costs[]");
        assert!(
            !costs.is_empty(),
            "el bundle real produce costes NO vacíos (era el stub vacío el que no)"
        );
        let cost_kinds: Vec<&str> = costs.iter().filter_map(|l| l["kind"].as_str()).collect();
        assert!(cost_kinds.contains(&"gas"), "costs = {cost_kinds:?}");
        assert!(cost_kinds.contains(&"financing"), "costs = {cost_kinds:?}");

        let check = crate::rhai_agent_bridge::economic_check(
            router.as_ref(),
            &c,
            &spec(),
            &candidate,
            &quote,
            &serde_json::json!({}),
            &[],
        );
        let tags = repair_tags(&check["repairs"]);
        assert!(
            !tags
                .iter()
                .any(|t| t.ends_with("mandatory_route_cost_missing")),
            "el bundle real NO puede reportar costes obligatorios ausentes: {tags:?}"
        );
        assert!(
            !tags
                .iter()
                .any(|t| t == "costs.financing::mandatory_route_cost_missing"),
            "costs.financing :: mandatory_route_cost_missing es la firma del STUB: {tags:?}"
        );
    }

    #[test]
    fn the_empty_cost_substrate_reproduces_the_measured_repair_signature() {
        // Contraste: el MISMO candidato/cotización, pero con el sustrato de
        // coste del stub Phase-1 (costs=[] + los tres kinds obligatorios, que es
        // exactamente lo que `SnapshotServices::quote` emite cuando
        // `base_cost_lines` está vacío). Es la firma medida en producción:
        // `costs.financing :: mandatory_route_cost_missing` en ~100% de las filas.
        let rev: crate::snapshot_services::RevisionGuard = Arc::new(|_: &str, _: &str| true);
        let services =
            crate::snapshot_services::SnapshotServices::new(Arc::new(real_bundle()), rev)
                .expect("services");
        let router = Arc::new(crate::context_router::ContextRouter::new(4).expect("router"));
        router
            .insert(CONTEXT_ID.to_string(), Arc::new(services))
            .expect("insert");

        let c = ctx();
        let discovery = router.discover(&c, &spec()).expect("discover");
        let candidate = discovery["candidates"]
            .as_array()
            .and_then(|v| v.first())
            .cloned()
            .expect("candidato");
        let mut stub_quote = serde_json::to_value(
            router
                .quote(&c, &spec(), &candidate)
                .expect("quote del candidato"),
        )
        .expect("quote serializable");
        stub_quote["costs"] = serde_json::json!([]);
        stub_quote["required_cost_kinds"] =
            serde_json::json!(["gas", "financing", "execution_fees"]);

        let check = crate::rhai_agent_bridge::economic_check(
            router.as_ref(),
            &c,
            &spec(),
            &candidate,
            &stub_quote,
            &serde_json::json!({}),
            &[],
        );
        let tags = repair_tags(&check["repairs"]);
        for kind in ["gas", "financing", "execution_fees"] {
            let want = format!("costs.{kind}::mandatory_route_cost_missing");
            assert!(
                tags.contains(&want),
                "el sustrato sin productores de coste debe reportar {want}: {tags:?}"
            );
        }
        assert!(
            !check["net_profit_usd"].is_number(),
            "sin costes completos el neto NO se computa (jamás un cero decorativo)"
        );
    }

    // ── V4-SNAPSHOT-PRODUCERS-01 — la financiación se DECLARA, no se omite ───
    //
    // Medición de producción (chain 1, 2026-10-03, `trading_config`):
    // `flashloan_fee_pct = 0.0009` (> 0), `token_prices_usd` = 21 símbolos,
    // `gas_estimate_units = 250000`, `base_token_price_usd = 2693.5435`.
    // Mientras el hash `arbx:token_prices:1` cubre 447 símbolos y el universo de
    // identidad tiene 2747 tokens, una ruta long-tail llega SIN precio canónico.
    // Antes de este cambio la rama de financiación sólo empujaba la línea dentro
    // del `if let Some(amount_usd)`: sin precio la línea DESAPARECÍA y el bridge
    // reportaba `costs.financing::mandatory_route_cost_missing` (64/269 medidos)
    // — una razón que miente, porque el productor existe. Estos tests fijan las
    // dos direcciones y el motivo VERDADERO.

    fn cost_line_of<'a>(
        lines: &'a [crate::rhai_agent_bridge::CostLine],
        kind: &str,
    ) -> &'a crate::rhai_agent_bridge::CostLine {
        lines.iter().find(|l| l.kind == kind).unwrap_or_else(|| {
            panic!(
                "falta la línea obligatoria {kind}; presentes: {:?}",
                lines.iter().map(|l| l.kind.as_str()).collect::<Vec<&str>>()
            )
        })
    }

    fn canonical_price_of_a(usd: &str) -> crate::snapshot_services::CanonicalPrice {
        let now = now_ms();
        crate::snapshot_services::CanonicalPrice {
            chain_id: CHAIN,
            token_address: hex(tok(0xA)),
            usd: usd.to_string(),
            revision: "cfg-test".to_string(),
            observed_at_ms: now.saturating_sub(1_000),
            valid_until_ms: now + 60_000,
            evidence_id: "test:canonical_price".to_string(),
            producer: "PriceBus".to_string(),
        }
    }

    #[test]
    fn financing_line_is_declared_when_the_start_token_has_no_canonical_price() {
        let cfg = operator_cfg();
        assert!(
            cfg.flashloan_fee_pct > 0.0,
            "el fixture del operador declara una tasa flash > 0 (como producción)"
        );
        // SIN ninguna entrada de precio: el caso medido de las rutas long-tail.
        let no_prices = std::collections::BTreeMap::new();
        let lines = v4_base_cost_lines(
            &cfg,
            CHAIN,
            &hex(tok(0xA)),
            AMOUNT_RAW,
            18,
            20.0,
            &no_prices,
        );

        let financing = cost_line_of(&lines, "financing");
        assert_eq!(
            financing.usd, None,
            "sin precio canónico el importe NO se computa: ni ceros ni estimaciones (R8)"
        );
        assert_eq!(
            financing.treatment, "external",
            "la financiación APLICA (tasa declarada > 0): not_applicable afirmaría que el coste no existe"
        );
        assert!(
            financing.reason.as_deref().is_some_and(|r| !r.is_empty()),
            "un coste NO COMPUTADO exige razón explícita"
        );
        assert!(
            !financing.evidence_id.is_empty(),
            "el contrato CostLine exige evidence_id no vacío"
        );
        // Las otras dos obligatorias no dependen del precio del token.
        assert_eq!(cost_line_of(&lines, "execution_fees").treatment, "embedded");
        assert!(
            cost_line_of(&lines, "gas").usd.is_some(),
            "el gas se computa con el precio base de la CONFIG, no con el del token"
        );
    }

    #[test]
    fn financing_line_carries_the_declared_rate_when_the_start_token_is_priced() {
        use crate::snapshot_services::CanonicalPrice;
        let cfg = operator_cfg();
        let mut prices: std::collections::BTreeMap<(u64, String), CanonicalPrice> =
            std::collections::BTreeMap::new();
        prices.insert((CHAIN, hex(tok(0xA))), canonical_price_of_a("2"));
        // 1e18 unidades raw = 1 token a 2 USD → importe 2 USD → reserva
        // 2 × 0.09% = 0.0018 USD (positiva, no subnormal en el formato de 6).
        let lines = v4_base_cost_lines(
            &cfg,
            CHAIN,
            &hex(tok(0xA)),
            "1000000000000000000",
            18,
            20.0,
            &prices,
        );
        let financing = cost_line_of(&lines, "financing");
        assert_eq!(financing.treatment, "external");
        let usd: f64 = financing
            .usd
            .as_deref()
            .expect("con precio canónico la reserva SÍ se computa")
            .parse()
            .expect("usd decimal");
        assert!(usd > 0.0, "reserva positiva: {usd}");
        assert_eq!(
            financing.evidence_id, "config:flashloan_fee_pct",
            "la evidencia debe nombrar la fuente REAL (la config del operador)"
        );
    }

    #[test]
    fn economic_check_distinguishes_an_absent_line_from_a_declared_uncomputable_one() {
        let rev: crate::snapshot_services::RevisionGuard = Arc::new(|_: &str, _: &str| true);
        let services =
            crate::snapshot_services::SnapshotServices::new(Arc::new(real_bundle()), rev)
                .expect("services");
        let router = Arc::new(crate::context_router::ContextRouter::new(4).expect("router"));
        router
            .insert(CONTEXT_ID.to_string(), Arc::new(services))
            .expect("insert");
        let c = ctx();
        let discovery = router.discover(&c, &spec()).expect("discover");
        let candidate = discovery["candidates"]
            .as_array()
            .and_then(|v| v.first())
            .cloned()
            .expect("candidato");
        let quote = serde_json::to_value(
            router
                .quote(&c, &spec(), &candidate)
                .expect("quote del candidato"),
        )
        .expect("quote serializable");

        let run = |q: &serde_json::Value| {
            crate::rhai_agent_bridge::economic_check(
                router.as_ref(),
                &c,
                &spec(),
                &candidate,
                q,
                &serde_json::json!({}),
                &[],
            )
        };

        // (a) CON la línea computada: no hay coste obligatorio ausente.
        let tags_with = repair_tags(&run(&quote)["repairs"]);
        assert!(
            !tags_with
                .iter()
                .any(|t| t == "costs.financing::mandatory_route_cost_missing"),
            "con la línea presente esa razón mentiría: {tags_with:?}"
        );

        // (b) SIN la línea (ausencia real de productor): la razón exacta medida.
        let mut without = quote.clone();
        let filtered: Vec<serde_json::Value> = without["costs"]
            .as_array()
            .expect("costs[]")
            .iter()
            .filter(|l| l["kind"] != "financing")
            .cloned()
            .collect();
        without["costs"] = serde_json::json!(filtered);
        let absent = run(&without);
        let tags_absent = repair_tags(&absent["repairs"]);
        assert!(
            tags_absent
                .iter()
                .any(|t| t == "costs.financing::mandatory_route_cost_missing"),
            "sin productor la razón debe ser la de ausencia: {tags_absent:?}"
        );
        assert_eq!(absent["candidate_eligible"], serde_json::json!(false));
        assert_eq!(
            absent["net_profit_usd"],
            serde_json::Value::Null,
            "sin coste completo el neto queda NO COMPUTADO (jamás un cero decorativo)"
        );

        // (c) Línea DECLARADA pero no computable (el caso de producción tras este
        //     cambio): el motivo es el VERDADERO — el importe no se pudo computar
        //     — no la ausencia del productor.
        let mut declared = quote.clone();
        for line in declared["costs"].as_array_mut().expect("costs[]") {
            if line["kind"] == "financing" {
                line["usd"] = serde_json::Value::Null;
                line["reason"] = serde_json::json!(
                    "reserva flash NO COMPUTADA: sin precio canonico del start token"
                );
                line["evidence_id"] =
                    serde_json::json!("config:flashloan_fee_pct+unpriced:start_token");
            }
        }
        let tags_declared = repair_tags(&run(&declared)["repairs"]);
        assert!(
            tags_declared
                .iter()
                .any(|t| t == "costs.financing::missing_or_invalid_cost"),
            "una línea declarada sin valor debe reportarse como tal: {tags_declared:?}"
        );
        assert!(
            !tags_declared
                .iter()
                .any(|t| t == "costs.financing::mandatory_route_cost_missing"),
            "la línea ESTÁ declarada: la razón de 'sin productor' mentiría: {tags_declared:?}"
        );
    }

    #[test]
    fn the_v4_bindings_resolve_the_registered_real_context() {
        // Extremo a extremo por el MISMO camino que un cartucho: engine Rhai +
        // `rhai_agent_bridge::register` + ContextRouter. Antes de
        // SHADOW-CANONICAL-01 la ruta SHADOW no registraba contexto alguno, así
        // que `agent_v4_quote` caía al stub y devolvía costes VACÍOS.
        let rev: crate::snapshot_services::RevisionGuard = Arc::new(|_: &str, _: &str| true);
        let services =
            crate::snapshot_services::SnapshotServices::new(Arc::new(real_bundle()), rev)
                .expect("services");
        let router = Arc::new(crate::context_router::ContextRouter::new(4).expect("router"));
        router
            .insert(CONTEXT_ID.to_string(), Arc::new(services))
            .expect("insert");

        let mut engine = rhai::Engine::new();
        crate::rhai_agent_bridge::register(&mut engine, router.clone());
        let mut scope = rhai::Scope::new();
        scope.push("ctx_id", CONTEXT_ID.to_string());
        scope.push("mev_id", MEV_ID.to_string());
        scope.push("digest", DIGEST.to_string());
        let script = r#"
            let ctx = #{ "context_id": ctx_id, "snapshot_id": ctx_id };
            let spec = #{ "mev_id": mev_id, "detector_id": "R_CLOSED_CYCLE",
                         "source_digest": digest, "logic": "closed_route",
                         "allowed_search_hops": [2], "operator_requirements": [] };
            let discovery = agent_v4_discover(ctx, spec);
            let candidates = discovery["candidates"];
            if candidates == () || candidates.len() == 0 {
                return #{ "discovery": "NO_CANDIDATE", "costs": 0 };
            }
            let quote = agent_v4_quote(ctx, spec, candidates[0]);
            let costs = quote["costs"];
            let n = if costs == () { 0 } else { costs.len() };
            #{ "discovery": "READY", "costs": n, "quote_status": quote["status"] }
        "#;
        let out = engine
            .eval_with_scope::<rhai::Map>(&mut scope, script)
            .expect("el script v4 debe evaluar");
        assert_eq!(
            out.get("discovery").map(|d| d.to_string()),
            Some("READY".to_string()),
            "el binding debe descubrir sobre el grafo del contexto registrado"
        );
        let costs = out
            .get("costs")
            .and_then(|d| d.as_int().ok())
            .expect("costs debe ser entero");
        assert!(
            costs > 0,
            "agent_v4_quote sobre el contexto REAL devuelve costes no vacíos"
        );
    }

    #[test]
    fn shadow_outcome_rows_declare_the_real_substrate() {
        // `mode` sigue siendo "shadow" (es el stream shadow), pero la fila
        // declara si la evaluación usó el bundle real o el stub — antes TODAS
        // las filas describían el stub sin decirlo.
        let intent = cycle_intent();
        let real = build_rd_outcome_v1(
            CHAIN,
            "dex_arb",
            &intent,
            &CartridgeEvalResult {
                is_opportunity: false,
                estimated_profit: 0.0,
                confidence: 0.0,
                metadata: HashMap::new(),
                urgency: "low".to_string(),
                reason: Some("no_opportunity".to_string()),
            },
            true,
            ShadowSubstrate::IntentBundle,
            1_700_000_000_000,
        );
        assert_eq!(real["mode"], "shadow");
        assert_eq!(real["context_provenance"], "intent_bundle");

        let stub = build_rd_outcome_v2(
            CHAIN,
            "dex_arb",
            &intent,
            &CartridgeEvalResult {
                is_opportunity: false,
                estimated_profit: 0.0,
                confidence: 0.0,
                metadata: HashMap::new(),
                urgency: "low".to_string(),
                reason: Some("no_opportunity".to_string()),
            },
            false,
            ShadowSubstrate::StaticBootStub,
            1_700_000_000_000,
        );
        assert_eq!(stub["mode"], "shadow");
        assert_eq!(stub["context_provenance"], "static_boot_stub");
    }

    // ── MARKET-FEATURES-WIRE-01 (2026-10-03): el mapa de features del v4 ──
    //
    // El defecto que estos tests cierran: `v4_market_state_from_edges`
    // construía `features` con `HashMap::new()`. Todo lector de `features` en la
    // ruta v4 recibía nada y varios operadores, ante el hueco, FABRICABAN un
    // valor con `unwrap_or` (premium flash a 0.0, capital a $1.0, fee a 30 bps)
    // en vez de repartir DATA_GAP. Los tests ejercitan el sitio REAL — la
    // construcción del MarketState que va al dispatch v4 —, no una copia.

    /// Las claves que los operadores nativos y `regime_router` LEEN del mapa.
    /// Ante productor sin dato, NINGUNA debe existir.
    const OPERATOR_READ_FEATURE_KEYS: [&str; 6] = [
        "volatility",
        "health_factor",
        "oracle_price",
        "onchain_price",
        "parity_deviation",
        "pool_fee",
    ];

    /// Pierna V2 mínima y BIEN FORMADA: es el único tipo de edge que el
    /// constructor acepta (protocolo `cpmm_v2`, reservas orientadas y decimales
    /// ya resueltos). Reservas no degeneradas para que `normalized_price`
    /// compute — con decimales 6/18 la normalización es la que decide el precio.
    fn market_state_edge() -> crate::agent_graph::Edge {
        crate::agent_graph::Edge {
            edge_id: "0xfeat".into(),
            pool_id: "0xfeat".into(),
            chain_id: 1,
            token_in: "0xusdc".into(),
            token_out: "0xweth".into(),
            protocol: "cpmm_v2".into(),
            snapshot_id: "snap".into(),
            block_hash: "sync-ts-1".into(),
            reserve_in_raw: Some("1000000000".into()), // 1_000 USDC (6 dec)
            reserve_out_raw: Some("500000000000000000".into()), // 0.5 WETH (18 dec)
            fee_units: Some(30),
            fee_denominator: Some(10_000),
            token_in_decimals: 6,
            token_out_decimals: 18,
            adapter_version: "reserves_cache_v1".into(),
            sqrt_price_x96_raw: None,
            liquidity: None,
        }
    }

    #[test]
    fn producer_with_data_reaches_the_v4_dispatch_state() {
        let edges = vec![market_state_edge()];
        let mut produced = HashMap::new();
        produced.insert("parity_deviation".to_owned(), 0.000_5);
        produced.insert("volatility".to_owned(), 0.42);
        let state = v4_market_state_from_edges(&edges, 21_000_000, 12.5, produced)
            .expect("una pierna V2 con decimales resueltos produce MarketState");
        assert_eq!(state.features.len(), 2, "el mapa producido llega íntegro");
        assert_eq!(state.features.get("parity_deviation"), Some(&0.000_5));
        assert_eq!(state.features.get("volatility"), Some(&0.42));
    }

    #[test]
    fn producer_without_data_leaves_the_v4_dispatch_state_featureless() {
        let edges = vec![market_state_edge()];
        let state = v4_market_state_from_edges(&edges, 21_000_000, 12.5, HashMap::new())
            .expect("el MarketState se construye aunque no haya ninguna feature");
        assert!(
            state.features.is_empty(),
            "sin productor el mapa queda VACÍO; llegó {:?}",
            state.features
        );
    }

    #[test]
    fn no_operator_read_key_is_fabricated_when_the_producer_has_no_data() {
        // Gate anti-fabricación. Si alguien vuelve a insertar un default en el
        // sitio de construcción — 0.0 para `flash_premium`, 1.0 para
        // `max_capital`, 0.003 para `fee_bps`/`pool_fee` — este test FALLA.
        let edges = vec![market_state_edge()];
        let state = v4_market_state_from_edges(&edges, 21_000_000, 12.5, HashMap::new())
            .expect("el MarketState se construye aunque no haya ninguna feature");
        for key in OPERATOR_READ_FEATURE_KEYS {
            assert!(
                !state.features.contains_key(key),
                "features[{key}] NO debe existir sin productor: el hueco se declara, \
                 no se rellena con un valor fabricado"
            );
        }
    }

    #[test]
    fn a_produced_zero_is_a_measurement_and_survives_the_wire() {
        // R8/R10: `None` = no computado, `Some(0.0)` = computado y exactamente
        // cero. Una serie de precios plana sobre un lapso real es volatilidad
        // CERO — es una medición y debe viajar. El test fija la distinción para
        // que nadie "corrija" la fabricación borrando también los ceros reales.
        let edges = vec![market_state_edge()];
        let mut produced = HashMap::new();
        produced.insert("volatility".to_owned(), 0.0);
        let state = v4_market_state_from_edges(&edges, 21_000_000, 12.5, produced)
            .expect("una pierna V2 con decimales resueltos produce MarketState");
        assert_eq!(
            state.features.get("volatility"),
            Some(&0.0),
            "un cero MEDIDO viaja; la ausencia se representa por clave ausente"
        );
    }

    #[test]
    fn wired_state_still_passes_structural_admission_before_dispatch() {
        // El MarketState con features pobladas sigue pasando la admisión
        // estructural que precede al dispatch de operadores: poblarlas no rompe
        // el camino que las consume.
        use crate::native_operator_adapter::OperatorInputAdmission;
        let edges = vec![market_state_edge()];
        let mut produced = HashMap::new();
        produced.insert("parity_deviation".to_owned(), 0.000_5);
        let state = v4_market_state_from_edges(&edges, 21_000_000, 12.5, produced)
            .expect("una pierna V2 con decimales resueltos produce MarketState");
        let receipt = V4StructuralInputAdmission.validate(1, &state, "snap", "plan");
        assert!(receipt.is_ok(), "admisión estructural: {receipt:?}");
        assert_eq!(state.features.len(), 1);
    }
}
