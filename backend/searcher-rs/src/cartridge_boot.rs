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
/// Returns the shared `Arc<CartridgeRunner>` so the orchestrator can also evaluate
/// cartridges in shadow mode (the registry is shared via `Arc<RwLock<…>>`, so
/// cartridges loaded by the subscriber are visible to the orchestrator). Returns
/// `None` only when `REDIS_URL` is absent (fail-honest, no boot).
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
) -> Option<Arc<CartridgeRunner>> {
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

    // Build the runner BEFORE spawning so we can share the Arc with both the
    // subscriber task and the orchestrator (shadow evaluation).
    let runner = Arc::new(CartridgeRunner::new(host_ctx));
    let runner_for_task = runner.clone();

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

        // Run the hot-reload subscriber (long-running; returns on cancellation).
        let subscriber = CartridgeSubscriber::new(redis_url, runner_for_task, cancel);
        subscriber.run().await;

        info!(
            event = "cartridge.runtime_stopped",
            chain_id, "cartridge subscriber task exited"
        );
    });

    Some(runner)
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

/// Builds the `pool_data` Rhai `Map` passed to a cartridge's `evaluate_opportunity`.
///
/// Pure function (no I/O), built from the first leg of the route intent plus the
/// pre-fetched source-pool reserves. `reserves_source` is injected as a nested Rhai
/// Map `#{ r0, r1, block, ts, token0_addr }` (the field names dex_arb.rhai reads —
/// note `block`, not the Rust `blk`). When `None` (no fresh reserves in Redis), the
/// key is omitted → `pool_data.reserves_source == ()` → reserve-dependent cartridges
/// fail-honest (R8). Likewise `source_pool` is empty when `pool_hint` is None.
/// Gas / block number stay host-binding-sourced (`get_base_fee`, `get_block_number`).
///
/// ## FASE OMEGA — Data Vector Injection
/// Inyecta `market_state` completo con métricas de volatilidad, liquidez global,
/// y estado del mempool para decisión hamiltoniana del cartucho.
pub fn build_cartridge_pool_data(
    intent: &RouteIntent,
    reserves_source: Option<&crate::reserves::ReservesEntry>,
    market_state: Option<&MarketStateVector>,
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
        if let Some(fee) = leg.fee_bps {
            m.insert("fee_bps".into(), Dynamic::from(fee as i64));
        }
        let pool = leg
            .pool_hint
            .map(|p| format!("{:#x}", p))
            .unwrap_or_default();
        m.insert("source_pool".into(), Dynamic::from(pool));
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
    // ═══════════════════════════════════════════════════════════════════════════════
    // FASE OMEGA — Hamiltonian Data Vector Injection
    // ═══════════════════════════════════════════════════════════════════════════════
    if let Some(ms) = market_state {
        let mut market_map = rhai::Map::new();
        market_map.insert("volatility".into(), Dynamic::from(ms.volatility));
        market_map.insert("liquidity_depth".into(), Dynamic::from(ms.liquidity_depth));
        market_map.insert("mempool_pressure".into(), Dynamic::from(ms.mempool_pressure));
        market_map.insert("gas_trend".into(), Dynamic::from(ms.gas_trend));
        market_map.insert("block_time_ms".into(), Dynamic::from(ms.block_time_ms as i64));
        market_map.insert("price_momentum".into(), Dynamic::from(ms.price_momentum));
        market_map.insert("spread_variance".into(), Dynamic::from(ms.spread_variance));
        market_map.insert("timestamp_ms".into(), Dynamic::from(ms.timestamp_ms as i64));
        m.insert("market_state".into(), Dynamic::from(market_map));
    }
    m
}

/// Vector de estado de mercado para inyección hamiltoniana en cartuchos.
/// Calculado por el scanner a partir de métricas on-chain y mempool.
#[derive(Debug, Clone, Default)]
pub struct MarketStateVector {
    /// Volatilidad implícita del par (0.0 - 1.0)
    pub volatility: f64,
    /// Profundidad de liquidez agregada en USD
    pub liquidity_depth: f64,
    /// Presión del mempool (transacciones pendientes / bloque)
    pub mempool_pressure: f64,
    /// Tendencia de gas (diferencial vs últimos 10 bloques)
    pub gas_trend: f64,
    /// Tiempo entre bloques en ms
    pub block_time_ms: u64,
    /// Momentum de precio (dirección y magnitud)
    pub price_momentum: f64,
    /// Varianza del spread entre DEXs
    pub spread_variance: f64,
    /// Timestamp de cálculo
    pub timestamp_ms: u64,
}

/// Computa el vector de estado de mercado para inyección hamiltoniana.
/// Extrae métricas de Redis y calcula volatilidad, momentum, y presión.
async fn compute_market_state_vector(
    intent: &RouteIntent,
    runner: Arc<CartridgeRunner>,
) -> Option<MarketStateVector> {
    let start = std::time::Instant::now();

    // Extraer token pair del intent
    let (token_in, token_out) = intent.legs.first().map(|leg| {
        (format!("{:#x}", leg.token_in), format!("{:#x}", leg.token_out))
    })?;

    // Leer métricas de Redis (best-effort, fail-honest)
    let volatility = runner.read_market_metric(&format!("arbx:market:{}:{}:volatility", token_in, token_out)).await
        .unwrap_or(0.15); // Default 15% volatilidad

    let liquidity_depth = runner.read_market_metric(&format!("arbx:market:{}:{}:liquidity", token_in, token_out)).await
        .unwrap_or(0.0);

    let mempool_pressure = runner.read_market_metric("arbx:global:mempool_pressure").await
        .unwrap_or(0.5); // Default presión media

    let gas_trend = runner.read_market_metric("arbx:global:gas_trend").await
        .unwrap_or(0.0);

    let price_momentum = runner.read_market_metric(&format!("arbx:market:{}:{}:momentum", token_in, token_out)).await
        .unwrap_or(0.0);

    let spread_variance = runner.read_market_metric(&format!("arbx:market:{}:{}:spread_var", token_in, token_out)).await
        .unwrap_or(0.0);

    let block_time_ms = runner.read_market_metric("arbx:global:block_time_ms").await
        .map(|v| v as u64)
        .unwrap_or(12_000); // Default 12s

    let timestamp_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0);

    tracing::debug!(
        event = "market_state.computed",
        tx_hash = %intent.tx_hash,
        volatility = volatility,
        liquidity_depth = liquidity_depth,
        mempool_pressure = mempool_pressure,
        elapsed_ms = start.elapsed().as_millis(),
        "MarketStateVector calculado para inyección hamiltoniana"
    );

    Some(MarketStateVector {
        volatility,
        liquidity_depth,
        mempool_pressure,
        gas_trend,
        block_time_ms,
        price_momentum,
        spread_variance,
        timestamp_ms,
    })
}

// ── FASE B — Shadow outcome emitter (gated off by default, NO-ACTIVE) ─────────
//
// Persists each resolved `CartridgeEvalResult` from `shadow_evaluate_intent` to a
// NEW, SEPARATE Redis stream so the ≥2-week hit-rate dataset can accrue. This is
// the dry-run→stream link the paper-trade archiver header documents.
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
/// Approximate cap (`XADD ... MAXLEN ~`). ~2 weeks of ticks fit comfortably.
const OUTCOMES_STREAM_MAXLEN: u64 = 1_000_000;

/// Persist a resolved shadow eval outcome (fire-and-forget, fail-closed).
async fn emit_shadow_outcome(
    runner: &Arc<CartridgeRunner>,
    chain_id: u64,
    cartridge_id: &str,
    intent: &RouteIntent,
    res: &CartridgeEvalResult,
    had_reserves: bool,
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
        build_rd_outcome_v2(chain_id, cartridge_id, intent, res, had_reserves, ts_ms)
    } else {
        build_rd_outcome_v1(chain_id, cartridge_id, intent, res, had_reserves, ts_ms)
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
fn build_rd_outcome_v1(
    chain_id: u64,
    cartridge_id: &str,
    intent: &RouteIntent,
    res: &CartridgeEvalResult,
    had_reserves: bool,
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
        "had_reserves": had_reserves,
        "mode": "shadow",
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
        "estimated_profit_usd": res.estimated_profit,
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
/// Pertinence keys on `RouteIntent::source_event` (the intent's origin SHAPE):
///   - swap observations (`public_mempool`, `filtered_mempool`, `private_hint`,
///     `new_block`) carry one observed swap leg → only `dex_arb` (cross-DEX spread on the
///     observed pair) consumes that shape today.
///   - lending / oracle events (`lending_position_update`, `oracle_update`) carry a
///     debt/collateral position → `liquidation`.
///   - `triangular_arb` needs a `token_a/b/c` triangle that NO current detection source
///     emits; it stays loaded but dormant (skipped, not silently dropped) until a
///     triangle-scan source exists — re-enable its arm here when that lands.
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
    match category {
        // Consumes one observed swap leg (token_in/out + reserves_source).
        "dex_arb" => is_swap_observation,
        // Needs a debt/collateral position; only lending/oracle events carry it.
        "liquidation" => is_position_event,
        // Needs a token_a/b/c triangle; no current source emits that shape.
        "triangular_arb" => false,
        // Custom / unknown cartridge: don't gate it — evaluate against everything.
        _ => true,
    }
}

/// Shadow-evaluates every ACTIVE cartridge against one live route intent and emits
/// the result to logs/telemetry. **Read-only / observe-only**: it never constructs a
/// `StrategyCandidate`, never touches `process_candidate`, and never reaches the
/// execution pipeline. Designed to be `tokio::spawn`-ed off the orchestrator hot
/// path so it adds no latency to intent processing. Per-cartridge errors are logged,
/// never propagated (one bad cartridge cannot affect the others or the scanner).
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
    // FASE OMEGA: Inyecta MarketStateVector para decisión hamiltoniana.
    let reserves_source = match intent.legs.first().and_then(|l| l.pool_hint) {
        Some(p) => runner.read_pool_reserves(&format!("{:#x}", p)).await,
        None => None,
    };
    let market_state = compute_market_state_vector(&intent, runner.clone()).await;
    let pool_data = build_cartridge_pool_data(&intent, reserves_source.as_ref(), market_state.as_ref());

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
                        "cartridge shadow OPPORTUNITY detected (observe-only, no execution)"
                    );
                } else {
                    debug!(
                        event = "cartridge.shadow_eval_negative",
                        chain_id,
                        cartridge_id = %id,
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

#[cfg(test)]
mod tests {
    use super::*;

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
        let m = build_cartridge_pool_data(&intent, None, None);
        assert_eq!(m.get("chain_id").unwrap().to_string(), "1");
        assert_eq!(m.get("amount_in").unwrap().to_string(), "1234");
        assert_eq!(m.get("protocol_type").unwrap().to_string(), "v2");
        assert!(!m.get("source_pool").unwrap().to_string().is_empty());
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
        let m = build_cartridge_pool_data(&intent, None, None);
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
        let m = build_cartridge_pool_data(&intent, Some(&rs), None);
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

    // ── rd_outcome_v2 schema builders (Phase 1) ──────────────────────────────

    /// 3-leg cross-DEX cycle (V3 → Curve → V2) with distinct dex hints + fee tiers.
    fn three_leg_intent() -> RouteIntent {
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
            DetectionSource::NewBlock,
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
            1_700_000_000_000,
        );
        assert_eq!(v["schema"].as_str().unwrap(), "rd_outcome_v1");
        assert_eq!(v["mode"].as_str().unwrap(), "shadow");
        assert!(v["is_opportunity"].as_bool().unwrap());
        // v1 is flat: no topology object, no route[] array.
        assert!(v.get("topology").is_none());
        assert!(v.get("route").is_none());
    }

    #[test]
    fn rd_outcome_v2_topology_route_and_failhonest_waterfall() {
        let v = build_rd_outcome_v2(
            1,
            "omega_strategy_pack",
            &three_leg_intent(),
            &eval_result(true),
            true,
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
        assert_eq!(v["estimated_profit_usd"].as_f64().unwrap(), 9.64);

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
}
