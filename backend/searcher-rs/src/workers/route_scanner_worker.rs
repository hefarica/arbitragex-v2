//! RU-3 — RouteScannerWorker: PROACTIVE per-block multi-hop cycle scan.
//!
//! Today `multi_hop_search::find_profitable_cycles` only runs inside the
//! on-demand 12s `route_discovery` shadow tick. This worker activates it per
//! NEW BLOCK: it subscribes to `newHeads` (same WS pool the block scanner
//! uses), rebuilds the token graph from the live pool set + Redis reserves
//! (`graph_builder`), runs the bounded negative-`log_weight` cycle search
//! (`Σ log_weight < 0` ⇒ `Π (1-fee)·rate > 1`), and emits every profitable
//! cycle that passes the RU-3 anchor gate as a `RouteIntent` to the existing
//! orchestrator — no mempool event needed to trigger discovery.
//!
//! ## Anchor policy (RU-3 spec)
//! - h ≤ 3   → full universe → dispatchable.
//! - h 4..=5 → requires ≥ 1 anchor token in the cycle → dispatchable.
//! - h 6..=8 → requires ≥ 2 anchors → **SHADOW FORCED** (telemetry-only, never
//!   dispatched — long cycles are the deepest speculation surface).
//! - Anchors: {WETH, USDC, USDT, DAI, WBTC} (canonical mainnet), overridable
//!   via `ARBX_ROUTE_SCANNER_ANCHORS` (comma-separated hex).
//!
//! ## Budget
//! Enumeration is bounded by the per-block cycle cap (the finder's own honest
//! `capped`/`dropped_for_cap` counters) and the p95 wall-clock target of 250ms
//! per ~12s block (`route_scanner.done.enumeration_ms` + `capped` expose the
//! overshoot honestly — an overshoot is reported, never hidden).
//!
//! ## Safety posture (RULE 00 / R8 / NO-ACTIVE)
//! Gated by `ARBX_ROUTE_SCANNER_MODE` (**default `off`** — fail-safe parse,
//! nothing spawned, zero overhead). The scan layer carries NO sizing
//! (`amount_in = 0`, exactly like `route_intent_dispatcher::build_intent`):
//! sizing is the cartridge/orchestrator's job downstream. V3 legs without a
//! computable weight are skipped + counted by the finder, never faked. The
//! worker NEVER writes `arbx:opps:detected`; its only Redis writes are
//! telemetry PUBLISHes to `arbx:route_discovery:telemetry`.

use crate::cartridge::runner::CartridgeRunner;
use crate::cartridge_boot::shadow_evaluate_intent;
use crate::chain_client::WsChainClient;
use crate::impact_index::ImpactIndex;
use crate::orchestrator::Orchestrator;
use crate::route_discovery::graph_builder::{build_graph, GraphBuildConfig, TokenGraph};
use crate::route_discovery::multi_hop_search::{find_profitable_cycles, ProfitableCycle};
use crate::route_discovery::telemetry;
use crate::route_intent::{
    DetectionSource, RouteIntent, RouteIntentLeg, RouterKind, SwapExactMode,
};
use ethers::providers::StreamExt as _;
use ethers::types::{Address, H256, U256};
use redis::aio::ConnectionManager;
use std::collections::HashSet;
use std::str::FromStr;
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tokio::sync::RwLock;
use tokio_util::sync::CancellationToken;
use tracing::{debug, info, warn};

/// Algorithm tag stamped onto all RU-3 telemetry (bounded negative-`log_weight`
/// cycle search — the MMBF line-graph scale-up is the later optimization).
pub const ALGORITHM: &str = "multihop_negcycle";

/// Default per-block cycle cap (`ARBX_ROUTE_SCANNER_MAX_ROUTES_PER_BLOCK`).
/// Same magnitude as route_discovery's per-tick cap — one block ≈ one tick.
pub const DEFAULT_MAX_ROUTES_PER_BLOCK: usize = 500;

/// p95 enumeration budget per block (`ARBX_ROUTE_SCANNER_MAX_SCAN_MS`).
pub const DEFAULT_MAX_ENUMERATION_MS: u64 = 250;

/// Default hop ceiling (`ARBX_ROUTE_SCANNER_MAX_HOPS`); the finder clamps to 2..=7.
pub const DEFAULT_MAX_HOPS: usize = 7;

/// R9 anti-flood: per-cycle telemetry events per block are capped; the
/// aggregate `route_scanner.done` summary always carries the full counts.
pub const DEFAULT_MAX_CYCLE_EVENTS: usize = 50;

/// Canonical mainnet anchor tokens (lowercase hex). Override for other chains
/// or universes via `ARBX_ROUTE_SCANNER_ANCHORS` (CSV) — these defaults are
/// the RU-3 spec set.
pub const DEFAULT_ANCHORS: [&str; 5] = [
    "0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2", // WETH
    "0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48", // USDC
    "0xdac17f958d2ee523a2206206994597c13d831ec7", // USDT
    "0x6b175474e89094c44da98b954eedeac495271d0f", // DAI
    "0x2260fac5e5542a773aa44fbcfedf7c193bc2c599", // WBTC
];

fn env_usize(key: &str, default: usize) -> usize {
    std::env::var(key)
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(default)
}
fn env_u64(key: &str, default: u64) -> u64 {
    std::env::var(key)
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(default)
}

/// Worker gate (`ARBX_ROUTE_SCANNER_MODE`). Default `Off` — fail-safe: any
/// unset/garbage value resolves to `Off` and nothing is spawned.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RouteScannerMode {
    Off,
    On,
}

impl RouteScannerMode {
    pub fn from_env() -> Self {
        Self::parse(&std::env::var("ARBX_ROUTE_SCANNER_MODE").unwrap_or_default())
    }

    /// Pure parser (separate from `from_env` so it tests without env mutation).
    fn parse(raw: &str) -> Self {
        match raw.trim().to_ascii_lowercase().as_str() {
            "on" => Self::On,
            _ => Self::Off,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Off => "off",
            Self::On => "on",
        }
    }
}

/// Per-block enumeration budget: cycle-count cap (enforced inside the finder
/// with its honest `capped`/`dropped_for_cap` counters) + wall-clock target.
#[derive(Debug, Clone)]
pub struct ScanBudget {
    pub max_cycles_per_block: usize,
    pub max_enumeration_ms: u64,
}

impl Default for ScanBudget {
    fn default() -> Self {
        Self {
            max_cycles_per_block: DEFAULT_MAX_ROUTES_PER_BLOCK,
            max_enumeration_ms: DEFAULT_MAX_ENUMERATION_MS,
        }
    }
}

impl ScanBudget {
    /// `true` when the enumeration blew its wall-clock budget (p95 target).
    /// Overshoot is a REPORTING signal — the DFS is bounded by the cycle cap
    /// and terminates; we surface the overshoot instead of hiding it (R8).
    pub fn exceeded(&self, enumeration_ms: u64) -> bool {
        enumeration_ms > self.max_enumeration_ms
    }
}

/// Anchor-gate verdict for one discovered cycle (RU-3).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CycleAdmission {
    /// h ≤ 3 (full universe) or h 4-5 with ≥ 1 anchor — may be dispatched.
    Dispatch,
    /// h 6-8 with ≥ 2 anchors — observe-only: telemetry, never dispatched.
    ShadowForced,
}

/// Pure RU-3 anchor filter.
///
/// - `2..=3` → `Dispatch` (full universe, no anchor requirement)
/// - `4..=5` → `Dispatch` iff `anchor_hits >= 1`, else rejected
/// - `6..=8` → `ShadowForced` iff `anchor_hits >= 2`, else rejected
/// - anything else (`< 2`, `> 8`) → `None` (the finder yields 2..=7; the h=8
///   arm keeps the policy spec-complete if the clamp ever widens)
pub fn anchor_verdict(hop_count: usize, anchor_hits: usize) -> Option<CycleAdmission> {
    match hop_count {
        2..=3 => Some(CycleAdmission::Dispatch),
        4..=5 if anchor_hits >= 1 => Some(CycleAdmission::Dispatch),
        6..=8 if anchor_hits >= 2 => Some(CycleAdmission::ShadowForced),
        _ => None,
    }
}

/// Parse a CSV of hex anchor addresses, skipping (with a warn) any that don't
/// parse — R8 fail-honest, never a sentinel address.
pub fn parse_anchors(spec: &str) -> HashSet<Address> {
    let mut out = HashSet::new();
    for s in spec.split(',') {
        let t = s.trim();
        if t.is_empty() {
            continue;
        }
        match Address::from_str(t) {
            Ok(a) => {
                out.insert(a);
            }
            Err(_) => warn!(event = "route_scanner.bad_anchor", anchor = %t),
        }
    }
    out
}

/// The canonical RU-3 anchor set (mainnet WETH/USDC/USDT/DAI/WBTC).
pub fn default_anchors() -> HashSet<Address> {
    parse_anchors(&DEFAULT_ANCHORS.join(","))
}

/// Resolved worker tunables from env.
#[derive(Debug, Clone)]
pub struct ScannerConfig {
    pub budget: ScanBudget,
    /// Hop ceiling; clamped to 2..=7 (the finder's own supported range).
    pub max_hops: usize,
    pub anchors: HashSet<Address>,
    pub graph: GraphBuildConfig,
    pub max_cycle_events: usize,
}

impl ScannerConfig {
    pub fn from_env() -> Self {
        let anchors = match std::env::var("ARBX_ROUTE_SCANNER_ANCHORS") {
            Ok(spec) if !spec.trim().is_empty() => parse_anchors(&spec),
            _ => default_anchors(),
        };
        Self {
            budget: ScanBudget {
                max_cycles_per_block: env_usize(
                    "ARBX_ROUTE_SCANNER_MAX_ROUTES_PER_BLOCK",
                    DEFAULT_MAX_ROUTES_PER_BLOCK,
                ),
                max_enumeration_ms: env_u64(
                    "ARBX_ROUTE_SCANNER_MAX_SCAN_MS",
                    DEFAULT_MAX_ENUMERATION_MS,
                ),
            },
            max_hops: env_usize("ARBX_ROUTE_SCANNER_MAX_HOPS", DEFAULT_MAX_HOPS).clamp(2, 7),
            anchors,
            graph: GraphBuildConfig::default(),
            max_cycle_events: env_usize(
                "ARBX_ROUTE_SCANNER_MAX_CYCLE_EVENTS",
                DEFAULT_MAX_CYCLE_EVENTS,
            ),
        }
    }
}

/// Pure per-block scan output — everything except the wall clock, so the
/// classification logic unit-tests offline.
#[derive(Debug, Default)]
pub struct ScanOutput {
    /// Profitable cycles admitted for dispatch (h ≤ 3, or h 4-5 with ≥ 1 anchor).
    pub dispatchable: Vec<ProfitableCycle>,
    /// Profitable long cycles (h 6-8, ≥ 2 anchors) — shadow forced.
    pub shadow_only: Vec<ProfitableCycle>,
    /// Profitable cycles dropped by the anchor gate (h 4-5 with 0 anchors,
    /// h 6-8 with < 2 anchors).
    pub anchor_rejected: usize,
    /// Total profitable cycles the finder returned, pre-anchor-filter.
    pub cycles_found: usize,
    /// The finder's honest cap flag (cycle budget hit — `cycles_found` is a
    /// lower bound when `true`).
    pub capped: bool,
    pub dropped_for_cap: usize,
    /// V3 (or any) legs skipped for a `None` weight — honest finder counter.
    pub v3_skipped: usize,
    /// Cycles whose edge indices did not resolve in the graph (defensive —
    /// indices are self-produced by the finder; anything here is a bug, not a
    /// market signal).
    pub malformed: usize,
}

/// Pure scan evaluation: enumerate profitable cycles over `graph` and classify
/// each by the RU-3 anchor gate. No Redis, no clock, no dispatch — the async
/// loop injects time and executes the emission.
pub fn evaluate_scan(
    graph: &TokenGraph,
    anchors: &HashSet<Address>,
    max_hops: usize,
    max_cycles: usize,
) -> ScanOutput {
    let result = find_profitable_cycles(graph, max_hops, max_cycles);
    let mut out = ScanOutput {
        cycles_found: result.cycles.len(),
        capped: result.capped,
        dropped_for_cap: result.dropped_for_cap,
        v3_skipped: result.v3_skipped,
        ..ScanOutput::default()
    };
    for cycle in result.cycles {
        // Anchor hits over the cycle's DISTINCT tokens (a cycle may legally
        // revisit an intermediate token; presence is what the gate asks for).
        let mut tokens: HashSet<Address> = HashSet::new();
        let mut malformed = false;
        for &idx in &cycle.edges {
            match graph.edges.get(idx) {
                Some(e) => {
                    tokens.insert(e.token_in);
                }
                None => {
                    malformed = true;
                    break;
                }
            }
        }
        if malformed {
            out.malformed += 1;
            continue;
        }
        let hits = tokens.intersection(anchors).count();
        match anchor_verdict(cycle.hop_count, hits) {
            Some(CycleAdmission::Dispatch) => out.dispatchable.push(cycle),
            Some(CycleAdmission::ShadowForced) => out.shadow_only.push(cycle),
            None => out.anchor_rejected += 1,
        }
    }
    out
}

/// Deterministic synthetic identity for one (block, cycle) emission:
/// `keccak256(chain_id ‖ block_number ‖ edge_indices)`. Unique per block and
/// cycle — never a shared `H256::zero()` sentinel that would collide on any
/// dedup keyed by `tx_hash` (same discipline as the route_hash digest the
/// route-discovery dispatcher uses).
fn synthetic_tx_hash(chain_id: u64, block_number: u64, edges: &[usize]) -> H256 {
    let mut buf: Vec<u8> = Vec::with_capacity(16 + edges.len() * 8);
    buf.extend_from_slice(&chain_id.to_be_bytes());
    buf.extend_from_slice(&block_number.to_be_bytes());
    for &i in edges {
        buf.extend_from_slice(&(i as u64).to_be_bytes());
    }
    H256::from(ethers::utils::keccak256(&buf))
}

/// Build a synthetic per-block `RouteIntent` from a profitable cycle (RU-3).
///
/// - Legs come straight from the cycle's graph edges (`pool_hint`, `fee_bps`,
///   `protocol_type` included — no re-derivation, no fabrication).
/// - `tx_hash` = the synthetic (block, cycle) digest; `router`/`sender` are
///   `zero` because a proactively-discovered cycle has no originating router
///   call — honest "N/A" on an observe-only intent.
/// - `amount_in = 0`: the scan layer carries no sizing (mirrors
///   `route_intent_dispatcher::build_intent`); sizing happens downstream.
/// - `source_event = NewBlock` so `cartridge_matches_intent` routes it by
///   shape (`dex_arb` 2-leg, `triangular_arb` closed ≥ 3-leg).
///
/// Defensive (trading-path checks are mandatory, not speculative): rejects an
/// out-of-range edge index, a non-profitable (`Σ log_weight >= 0`) cycle, and
/// a cycle that does not close (last `token_out` != first `token_in`).
pub fn cycle_to_intent(
    graph: &TokenGraph,
    chain_id: u64,
    block_number: u64,
    cycle: &ProfitableCycle,
) -> Option<RouteIntent> {
    if cycle.edges.is_empty() || cycle.sum_log_weight >= 0.0 {
        return None;
    }
    let mut legs: Vec<RouteIntentLeg> = Vec::with_capacity(cycle.edges.len());
    for &idx in &cycle.edges {
        let e = graph.edges.get(idx)?;
        legs.push(RouteIntentLeg {
            token_in: e.token_in,
            token_out: e.token_out,
            pool_hint: Some(e.pool),
            dex_hint: None,
            fee_bps: e.fee_bps,
            protocol_type: e.protocol,
        });
    }
    // The cycle must close: the last edge must land back on the first leg's
    // input token. A broken adjacency would otherwise ship an open route.
    if legs.first()?.token_in != legs.last()?.token_out {
        return None;
    }
    RouteIntent::new(
        chain_id,
        synthetic_tx_hash(chain_id, block_number, &cycle.edges),
        Address::zero(),
        RouterKind::Unknown,
        Address::zero(),
        legs,
        U256::zero(),
        None,
        SwapExactMode::ExactIn,
        DetectionSource::NewBlock,
    )
}

// ---------------------------------------------------------------------------
// Telemetry event builders (pure; published via route_discovery::publish to
// the same dedicated channel — `arbx:opps:detected` is never touched)
// ---------------------------------------------------------------------------

/// `route_scanner.done` — one per scanned block. Spec keys
/// {`cycles_found`, `elapsed_ms`, `capped`} plus the honest detail counters.
#[allow(clippy::too_many_arguments)]
pub fn done_event(
    chain_id: u64,
    block_number: u64,
    pools_total: usize,
    edges_built: usize,
    scan: &ScanOutput,
    dispatched: usize,
    enumeration_ms: u64,
    elapsed_ms: u64,
    capped: bool,
) -> serde_json::Value {
    serde_json::json!({
        "event": "route_scanner.done",
        "chain_id": chain_id,
        "block_number": block_number,
        "algorithm": ALGORITHM,
        "cycles_found": scan.cycles_found,
        "cycles_dispatched": dispatched,
        "cycles_shadow_forced": scan.shadow_only.len(),
        "cycles_anchor_rejected": scan.anchor_rejected,
        "dropped_for_cap": scan.dropped_for_cap,
        "v3_skipped": scan.v3_skipped,
        "malformed": scan.malformed,
        "pools_total": pools_total,
        "edges_built": edges_built,
        "enumeration_ms": enumeration_ms,
        "elapsed_ms": elapsed_ms,
        "capped": capped,
    })
}

/// `route_scanner.cycle` — one ADMITTED cycle (dispatch or shadow-forced),
/// capped per block by `max_cycle_events` (R9).
pub fn cycle_event(
    chain_id: u64,
    block_number: u64,
    cycle: &ProfitableCycle,
    mode: &str,
) -> serde_json::Value {
    serde_json::json!({
        "event": "route_scanner.cycle",
        "chain_id": chain_id,
        "block_number": block_number,
        "algorithm": ALGORITHM,
        "hops": cycle.hop_count,
        "sum_log_weight": cycle.sum_log_weight,
        "mode": mode,
    })
}

// ---------------------------------------------------------------------------
// Async loop
// ---------------------------------------------------------------------------

/// One scanned block: snapshot pools → build graph → enumerate (blocking) →
/// classify → dispatch/telemetry. All failures are logged + counted, never
/// propagated (a bad block must not kill the subscription).
#[allow(clippy::too_many_arguments)] // per-block coordinator wiring
async fn scan_block(
    redis: &mut ConnectionManager,
    impact_index: &Arc<RwLock<ImpactIndex>>,
    chain_id: u64,
    block_number: u64,
    cfg: &ScannerConfig,
    runner: &Option<Arc<CartridgeRunner>>,
    orchestrator: &Option<Arc<Orchestrator>>,
) {
    let t0 = Instant::now();
    let pools = { impact_index.read().await.all_pools() };
    let pools_total = pools.len();
    if pools.is_empty() {
        debug!(
            event = "route_scanner.no_pools",
            chain_id,
            block_number,
            "empty pool universe — honest skip (no fabricated graph)"
        );
        return;
    }
    let now_ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let outcome = build_graph(redis, chain_id, &pools, now_ts, &cfg.graph).await;
    let edges_built = outcome.graph.edges.len();
    let graph = outcome.graph;

    // CPU-bound DFS on the blocking pool so a big graph cannot stall the async
    // executor; the cycle cap bounds worst-case completion.
    let anchors = cfg.anchors.clone();
    let max_hops = cfg.max_hops;
    let max_cycles = cfg.budget.max_cycles_per_block;
    let enum_started = Instant::now();
    let joined = tokio::task::spawn_blocking(move || {
        let scan = evaluate_scan(&graph, &anchors, max_hops, max_cycles);
        (scan, graph)
    })
    .await;
    let enumeration_ms = enum_started.elapsed().as_millis() as u64;
    let (scan, graph) = match joined {
        Ok(pair) => pair,
        Err(e) => {
            warn!(
                event = "route_scanner.enumeration_panic",
                chain_id,
                block_number,
                error = %e,
                "spawn_blocking join failed — block skipped honestly"
            );
            return;
        }
    };

    let capped = scan.capped || cfg.budget.exceeded(enumeration_ms);
    if cfg.budget.exceeded(enumeration_ms) {
        warn!(
            event = "route_scanner.budget_exceeded",
            chain_id,
            block_number,
            enumeration_ms,
            budget_ms = cfg.budget.max_enumeration_ms,
            "enumeration exceeded the p95 wall-clock budget (reported, not hidden)"
        );
    }

    // ── Emission ── dispatch admitted cycles as RouteIntents to the EXISTING
    // orchestrator (mirrors route_discovery_worker: orchestrator first —
    // spawn_cartridge_eval acts only in cartridge Active mode — else the
    // cartridge runner's observe-only shadow_evaluate_intent; with neither, the
    // cycle is telemetry-only). Shadow-forced cycles are NEVER dispatched.
    let mut dispatched = 0usize;
    let mut cycle_events = 0usize;
    for cycle in &scan.dispatchable {
        let Some(intent) = cycle_to_intent(&graph, chain_id, block_number, cycle) else {
            debug!(
                event = "route_scanner.cycle_malformed",
                chain_id,
                block_number,
                hops = cycle.hop_count,
                "cycle_to_intent rejected a finder cycle (defensive guard)"
            );
            continue;
        };
        debug!(
            event = "route_scanner.cycle_emitted",
            chain_id,
            block_number,
            hops = cycle.hop_count,
            sum_log_weight = cycle.sum_log_weight,
            tx_hash = %intent.tx_hash,
            "profitable cycle admitted + emitted as RouteIntent"
        );
        if let Some(orch) = orchestrator {
            orch.spawn_cartridge_eval(intent);
        } else if let Some(r) = runner {
            tokio::spawn(shadow_evaluate_intent(r.clone(), intent, chain_id));
        }
        dispatched += 1;
        if cycle_events < cfg.max_cycle_events {
            telemetry::publish(
                redis,
                &cycle_event(chain_id, block_number, cycle, "dispatch"),
            )
            .await;
            cycle_events += 1;
        }
    }
    for cycle in &scan.shadow_only {
        debug!(
            event = "route_scanner.cycle_shadow",
            chain_id,
            block_number,
            hops = cycle.hop_count,
            sum_log_weight = cycle.sum_log_weight,
            "long cycle (h>=6, >=2 anchors) — SHADOW FORCED, telemetry only"
        );
        if cycle_events < cfg.max_cycle_events {
            telemetry::publish(
                redis,
                &cycle_event(chain_id, block_number, cycle, "shadow_forced"),
            )
            .await;
            cycle_events += 1;
        }
    }

    let elapsed_ms = t0.elapsed().as_millis() as u64;
    let done = done_event(
        chain_id,
        block_number,
        pools_total,
        edges_built,
        &scan,
        dispatched,
        enumeration_ms,
        elapsed_ms,
        capped,
    );
    telemetry::publish(redis, &done).await;

    // R9: one aggregated INFO per block (per-cycle detail stays at debug).
    info!(
        event = "route_scanner.done",
        chain_id,
        block_number,
        cycles_found = scan.cycles_found,
        cycles_dispatched = dispatched,
        cycles_shadow_forced = scan.shadow_only.len(),
        cycles_anchor_rejected = scan.anchor_rejected,
        capped,
        enumeration_ms,
        elapsed_ms
    );
}

/// One WS connection: subscribe to `newHeads`, scan each block. `Ok(())` on
/// cancellation, `Err` on disconnect (caller reconnects + rotates endpoint).
#[allow(clippy::too_many_arguments)] // per-block coordinator wiring
async fn run_scan_subscription(
    mut redis: ConnectionManager,
    chain_id: u64,
    url: &str,
    impact_index: &Arc<RwLock<ImpactIndex>>,
    runner: &Option<Arc<CartridgeRunner>>,
    orchestrator: &Option<Arc<Orchestrator>>,
    cfg: &ScannerConfig,
    cancel: &CancellationToken,
) -> anyhow::Result<()> {
    let client = WsChainClient::connect(chain_id, url).await?;
    let mut blocks = client.subscribe_blocks().await?;
    info!(
        event = "route_scanner.connected",
        chain_id,
        "subscribed to newHeads (per-block multi-hop scan active)"
    );
    loop {
        tokio::select! {
            _ = cancel.cancelled() => return Ok(()),
            blk = blocks.next() => {
                let Some(block) = blk else {
                    return Err(anyhow::anyhow!("newHeads stream ended"));
                };
                let Some(number) = block.number else { continue };
                scan_block(
                    &mut redis,
                    impact_index,
                    chain_id,
                    number.as_u64(),
                    cfg,
                    runner,
                    orchestrator,
                )
                .await;
            }
        }
    }
}

/// Reconnect loop: rotate WS endpoints with exponential backoff, mirroring the
/// block scanner's posture. Never panics; exits only on cancellation.
#[allow(clippy::too_many_arguments)] // per-chain coordinator wiring
async fn run_loop(
    redis: ConnectionManager,
    chain_id: u64,
    ws_urls: Vec<String>,
    impact_index: Arc<RwLock<ImpactIndex>>,
    runner: Option<Arc<CartridgeRunner>>,
    orchestrator: Option<Arc<Orchestrator>>,
    cfg: ScannerConfig,
    cancel: CancellationToken,
) {
    let mut backoff_ms: u64 = 1_000;
    let max_backoff_ms: u64 = 30_000;
    let mut url_idx = 0usize;
    loop {
        if cancel.is_cancelled() {
            return;
        }
        let url = &ws_urls[url_idx % ws_urls.len()];
        match run_scan_subscription(
            redis.clone(),
            chain_id,
            url,
            &impact_index,
            &runner,
            &orchestrator,
            &cfg,
            &cancel,
        )
        .await
        {
            Ok(()) => {
                info!(event = "route_scanner.stopped", chain_id);
                return;
            }
            Err(e) => {
                warn!(
                    event = "route_scanner.reconnect",
                    chain_id,
                    error = %e,
                    backoff_ms,
                    next_endpoint = (url_idx + 1) % ws_urls.len(),
                    "block subscription dropped; rotating endpoint + backing off"
                );
                url_idx = url_idx.wrapping_add(1);
                tokio::time::sleep(Duration::from_millis(backoff_ms)).await;
                backoff_ms = (backoff_ms * 2).min(max_backoff_ms);
            }
        }
    }
}

/// Spawn the RU-3 per-block route scanner, gated by `ARBX_ROUTE_SCANNER_MODE`.
///
/// `Off` (the default) ⇒ nothing is spawned (zero overhead). Requires an
/// `ImpactIndex` (the live pool source) and at least one WS endpoint; when
/// either is absent the worker skips with an honest reason rather than
/// fabricating a graph.
#[allow(clippy::too_many_arguments)] // per-chain coordinator wiring
pub fn spawn_route_scanner(
    chain_id: u64,
    redis: ConnectionManager,
    ws_urls: Vec<String>,
    impact_index: Option<Arc<RwLock<ImpactIndex>>>,
    runner: Option<Arc<CartridgeRunner>>,
    orchestrator: Option<Arc<Orchestrator>>,
    cancel: CancellationToken,
) {
    let mode = RouteScannerMode::from_env();
    info!(
        event = "route_scanner.mode",
        chain_id,
        mode = mode.as_str(),
        dispatch_path = if orchestrator.is_some() { "orchestrator" } else if runner.is_some() { "cartridge_shadow" } else { "telemetry_only" }
    );
    if mode != RouteScannerMode::On {
        return; // off → dormant, nothing spawned
    }
    let Some(impact_index) = impact_index else {
        warn!(
            event = "route_scanner.skipped",
            chain_id,
            reason = "no_impact_index",
            "route scanner enabled but ImpactIndex unavailable (orchestrator off) — R8 fail-honest"
        );
        return;
    };
    if ws_urls.is_empty() {
        warn!(
            event = "route_scanner.skipped",
            chain_id,
            reason = "no_ws_endpoints",
            "route scanner enabled but no WS endpoints — R8 fail-honest"
        );
        return;
    }
    let cfg = ScannerConfig::from_env();
    info!(
        event = "route_scanner.config",
        chain_id,
        max_routes_per_block = cfg.budget.max_cycles_per_block,
        max_enumeration_ms = cfg.budget.max_enumeration_ms,
        max_hops = cfg.max_hops,
        anchors = cfg.anchors.len(),
        max_cycle_events = cfg.max_cycle_events,
        algorithm = ALGORITHM,
        "per-block multi-hop route scanner starting (proactive discovery)"
    );
    tokio::spawn(async move {
        run_loop(
            redis,
            chain_id,
            ws_urls,
            impact_index,
            runner,
            orchestrator,
            cfg,
            cancel,
        )
        .await;
    });
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use crate::route_discovery::types::{RouteDirection, RouteEdge};
    use crate::route_intent::ProtocolType;
    use std::collections::HashMap;

    fn addr(n: u64) -> Address {
        Address::from_low_u64_be(n)
    }

    fn edge(token_in: u64, token_out: u64, pool: u64, log_weight: Option<f64>) -> RouteEdge {
        RouteEdge {
            chain_id: 1,
            pool: addr(pool),
            token_in: addr(token_in),
            token_out: addr(token_out),
            token0: addr(token_in),
            token1: addr(token_out),
            protocol: if log_weight.is_some() {
                ProtocolType::V2
            } else {
                ProtocolType::V3
            },
            fee_bps: Some(30),
            liquidity_hint: Some(1_000_000.0),
            log_weight,
            freshness_ts: 1_700_000_000,
            blk: 20_000_000,
            direction: RouteDirection::ZeroForOne,
        }
    }

    fn graph_from(edges: Vec<RouteEdge>) -> TokenGraph {
        let mut adjacency: HashMap<Address, Vec<usize>> = HashMap::new();
        for (i, e) in edges.iter().enumerate() {
            adjacency.entry(e.token_in).or_default().push(i);
        }
        TokenGraph { edges, adjacency }
    }

    // ── mode gate ────────────────────────────────────────────────────────────

    #[test]
    fn mode_parse_is_fail_safe_off() {
        assert_eq!(RouteScannerMode::parse("on"), RouteScannerMode::On);
        assert_eq!(RouteScannerMode::parse(" ON "), RouteScannerMode::On);
        assert_eq!(RouteScannerMode::parse(""), RouteScannerMode::Off);
        assert_eq!(RouteScannerMode::parse("off"), RouteScannerMode::Off);
        assert_eq!(RouteScannerMode::parse("active"), RouteScannerMode::Off);
        assert_eq!(RouteScannerMode::parse("garbage"), RouteScannerMode::Off);
    }

    // ── anchor filter ────────────────────────────────────────────────────────

    #[test]
    fn anchor_filter_h2_h3_full_universe() {
        assert_eq!(anchor_verdict(2, 0), Some(CycleAdmission::Dispatch));
        assert_eq!(anchor_verdict(3, 0), Some(CycleAdmission::Dispatch));
        // Anchors don't change the verdict for short cycles.
        assert_eq!(anchor_verdict(3, 2), Some(CycleAdmission::Dispatch));
    }

    #[test]
    fn anchor_filter_h4_h5_requires_one_anchor() {
        assert_eq!(anchor_verdict(4, 0), None, "h4 with 0 anchors rejected");
        assert_eq!(anchor_verdict(5, 0), None, "h5 with 0 anchors rejected");
        assert_eq!(anchor_verdict(4, 1), Some(CycleAdmission::Dispatch));
        assert_eq!(anchor_verdict(5, 1), Some(CycleAdmission::Dispatch));
        assert_eq!(anchor_verdict(5, 3), Some(CycleAdmission::Dispatch));
    }

    #[test]
    fn anchor_filter_h6_h8_requires_two_anchors_and_forces_shadow() {
        assert_eq!(anchor_verdict(6, 0), None);
        assert_eq!(anchor_verdict(6, 1), None, "h6 with 1 anchor rejected");
        assert_eq!(anchor_verdict(6, 2), Some(CycleAdmission::ShadowForced));
        assert_eq!(anchor_verdict(7, 5), Some(CycleAdmission::ShadowForced));
        assert_eq!(anchor_verdict(8, 2), Some(CycleAdmission::ShadowForced));
        // Out-of-range hop counts are always rejected (defensive: the finder
        // yields 2..=7; h=8 keeps the policy spec-complete).
        assert_eq!(anchor_verdict(9, 5), None);
        assert_eq!(anchor_verdict(1, 5), None);
        assert_eq!(anchor_verdict(0, 5), None);
    }

    #[test]
    fn default_anchors_are_the_five_canonical_tokens() {
        let anchors = default_anchors();
        assert_eq!(anchors.len(), 5, "WETH/USDC/USDT/DAI/WBTC");
        for spec in DEFAULT_ANCHORS {
            let a = Address::from_str(spec).unwrap();
            assert!(anchors.contains(&a), "{spec} missing from the anchor set");
        }
    }

    #[test]
    fn parse_anchors_skips_garbage_honestly() {
        let parsed = parse_anchors(&format!(
            "{:#x}, not-an-address, {:#x},, ",
            addr(0x11),
            addr(0x22)
        ));
        assert_eq!(parsed.len(), 2, "garbage skipped, empties ignored");
        assert!(parsed.contains(&addr(0x11)));
        assert!(parsed.contains(&addr(0x22)));
        assert!(parse_anchors("").is_empty());
    }

    // ── budget cap ───────────────────────────────────────────────────────────

    #[test]
    fn budget_cap_is_honest_via_finder_counters() {
        // Two independent profitable 2-cycles, cap=1 → the finder must cap:
        // capped=true, dropped_for_cap>=1, and evaluate_scan surfaces it.
        let g = graph_from(vec![
            edge(0xA, 0xB, 1, Some(-0.02)),
            edge(0xB, 0xA, 2, Some(-0.02)),
            edge(0xC, 0xD, 3, Some(-0.02)),
            edge(0xD, 0xC, 4, Some(-0.02)),
        ]);
        let scan = evaluate_scan(&g, &default_anchors(), 7, 1);
        assert!(scan.capped, "hitting the cycle cap must set capped=true");
        assert!(scan.dropped_for_cap >= 1);
        assert_eq!(
            scan.dispatchable.len() + scan.shadow_only.len() + scan.anchor_rejected,
            1,
            "only the capped number of cycles survives"
        );
        assert_eq!(scan.cycles_found, 1);
    }

    #[test]
    fn budget_time_exceeded_is_reported() {
        let b = ScanBudget::default();
        assert_eq!(b.max_enumeration_ms, 250);
        assert!(!b.exceeded(250));
        assert!(b.exceeded(251), "1ms over budget is flagged (p95 target)");
        assert!(b.exceeded(10_000));
    }

    // ── scan classification (anchor partition over a live-shaped graph) ─────

    /// Same shape as [`edge`] but with full `Address` endpoints (canonical
    /// anchor tokens are 160-bit — they cannot ride the low-u64 test shortcut).
    fn edge_a(
        token_in: Address,
        token_out: Address,
        pool: u64,
        log_weight: Option<f64>,
    ) -> RouteEdge {
        RouteEdge {
            chain_id: 1,
            pool: addr(pool),
            token_in,
            token_out,
            token0: token_in,
            token1: token_out,
            protocol: if log_weight.is_some() {
                ProtocolType::V2
            } else {
                ProtocolType::V3
            },
            fee_bps: Some(30),
            liquidity_hint: Some(1_000_000.0),
            log_weight,
            freshness_ts: 1_700_000_000,
            blk: 20_000_000,
            direction: RouteDirection::from_in_token0(token_in, token_in),
        }
    }

    /// A graph with four profitable cycle classes (each cycle gets found once
    /// per rotation start token — 2 rotations for the pair, 4 for the squares,
    /// 6 for the hexagon; clusters have no cross edges so nothing else closes):
    /// - 2-hop no anchors (A↔B)          → dispatchable (full universe)
    /// - 4-hop WITH 1 anchor (WETH)      → dispatchable
    /// - 4-hop NO anchor (F→G→H→I→F)     → anchor_rejected
    /// - 6-hop with 2 anchors (DAI+USDC) → shadow_only
    fn classified_graph() -> (TokenGraph, HashSet<Address>) {
        let weth = Address::from_str(DEFAULT_ANCHORS[0]).unwrap(); // WETH
        let dai = Address::from_str(DEFAULT_ANCHORS[3]).unwrap(); // DAI
        let usdc = Address::from_str(DEFAULT_ANCHORS[1]).unwrap(); // USDC
        let edges = vec![
            // 2-hop, no anchors: A→B→A
            edge(0xA, 0xB, 1, Some(-0.02)),
            edge(0xB, 0xA, 2, Some(-0.02)),
            // 4-hop, 1 anchor: WETH→C→D→E→WETH
            edge_a(weth, addr(0xC), 3, Some(-0.01)),
            edge_a(addr(0xC), addr(0xD), 4, Some(-0.01)),
            edge_a(addr(0xD), addr(0xE), 5, Some(-0.01)),
            edge_a(addr(0xE), weth, 6, Some(-0.01)),
            // 4-hop, no anchor: F→G→H→I→F
            edge(0xF, 0x10, 7, Some(-0.01)),
            edge(0x10, 0x11, 8, Some(-0.01)),
            edge(0x11, 0x12, 9, Some(-0.01)),
            edge(0x12, 0xF, 10, Some(-0.01)),
            // 6-hop, 2 anchors: DAI→J→K→USDC→L→M→DAI
            edge_a(dai, addr(0x13), 11, Some(-0.01)),
            edge_a(addr(0x13), addr(0x14), 12, Some(-0.01)),
            edge_a(addr(0x14), usdc, 13, Some(-0.01)),
            edge_a(usdc, addr(0x15), 14, Some(-0.01)),
            edge_a(addr(0x15), addr(0x16), 15, Some(-0.01)),
            edge_a(addr(0x16), dai, 16, Some(-0.01)),
        ];
        let anchors: HashSet<Address> = [weth, dai, usdc].into_iter().collect();
        (graph_from(edges), anchors)
    }

    #[test]
    fn evaluate_scan_partitions_cycles_by_anchor_policy() {
        let (g, anchors) = classified_graph();
        let scan = evaluate_scan(&g, &anchors, 7, 500);
        assert!(!scan.capped);
        assert_eq!(scan.malformed, 0);
        assert_eq!(scan.v3_skipped, 0, "all edges are V2 (weighted)");
        // 2 two-hop rotations + 4 anchored four-hop rotations.
        assert_eq!(scan.dispatchable.len(), 6);
        assert!(
            scan.dispatchable
                .iter()
                .all(|c| c.hop_count == 2 || c.hop_count == 4)
        );
        // 6 rotations of the hexagon, all carrying both anchors.
        assert_eq!(scan.shadow_only.len(), 6);
        assert!(scan.shadow_only.iter().all(|c| c.hop_count == 6));
        assert!(
            scan.shadow_only
                .iter()
                .all(|c| c.sum_log_weight < 0.0),
            "shadow-forced cycles are still profitable — just not dispatched"
        );
        // 4 rotations of the un-anchored square, all rejected by the gate.
        assert_eq!(scan.anchor_rejected, 4);
        // Honest totals: 2 + 4 + 6 + 4 = 16 profitable cycles found.
        assert_eq!(scan.cycles_found, 16);
    }

    // ── cycle → RouteIntent emission shape ───────────────────────────────────

    #[test]
    fn cycle_to_intent_emits_closed_legs_with_new_block_source() {
        // Profitable triangle A→B→C→A.
        let g = graph_from(vec![
            edge(0xA, 0xB, 1, Some(-0.02)),
            edge(0xB, 0xC, 2, Some(-0.02)),
            edge(0xC, 0xA, 3, Some(-0.02)),
        ]);
        let cycle = ProfitableCycle {
            edges: vec![0, 1, 2],
            sum_log_weight: -0.06,
            hop_count: 3,
        };
        let intent = cycle_to_intent(&g, 1, 20_000_000, &cycle).unwrap();
        assert_eq!(intent.chain_id, 1);
        assert_eq!(intent.legs.len(), 3);
        // Legs chain: leg[i].token_out == leg[i+1].token_in, and the cycle
        // closes back on the first leg's input.
        assert_eq!(intent.legs[0].token_in, addr(0xA));
        assert_eq!(intent.legs[0].token_out, addr(0xB));
        assert_eq!(intent.legs[1].token_in, addr(0xB));
        assert_eq!(intent.legs[2].token_out, addr(0xA));
        // Provenance per leg: pool hint, fee tier, protocol — no fabrication.
        assert_eq!(intent.legs[0].pool_hint, Some(addr(1)));
        assert_eq!(intent.legs[0].fee_bps, Some(30));
        assert_eq!(intent.legs[0].protocol_type, ProtocolType::V2);
        // Scan layer carries no sizing (mirrors build_intent).
        assert_eq!(intent.amount_in, U256::zero());
        assert_eq!(intent.source_event, DetectionSource::NewBlock);
        // Synthetic identity is real and non-zero.
        assert_ne!(intent.tx_hash, H256::zero());
    }

    #[test]
    fn cycle_to_intent_rejects_unprofitable_open_and_out_of_range() {
        let g = graph_from(vec![
            edge(0xA, 0xB, 1, Some(-0.02)),
            edge(0xB, 0xA, 2, Some(-0.02)),
        ]);
        // Non-profitable (Σ >= 0) → None, even though the shape is valid.
        let flat = ProfitableCycle {
            edges: vec![0, 1],
            sum_log_weight: 0.0,
            hop_count: 2,
        };
        assert!(cycle_to_intent(&g, 1, 100, &flat).is_none());
        // Out-of-range edge index → None (defensive guard).
        let oob = ProfitableCycle {
            edges: vec![0, 99],
            sum_log_weight: -0.04,
            hop_count: 2,
        };
        assert!(cycle_to_intent(&g, 1, 100, &oob).is_none());
        // Empty cycle → None.
        let empty = ProfitableCycle {
            edges: vec![],
            sum_log_weight: -0.01,
            hop_count: 0,
        };
        assert!(cycle_to_intent(&g, 1, 100, &empty).is_none());
        // Open cycle (does not close) → None.
        let open = ProfitableCycle {
            edges: vec![0],
            sum_log_weight: -0.02,
            hop_count: 1,
        };
        assert!(cycle_to_intent(&g, 1, 100, &open).is_none());
    }

    #[test]
    fn synthetic_tx_hash_varies_by_block_and_cycle() {
        assert_ne!(
            synthetic_tx_hash(1, 100, &[0, 1, 2]),
            synthetic_tx_hash(1, 101, &[0, 1, 2]),
            "same cycle in a different block is a different emission"
        );
        assert_ne!(
            synthetic_tx_hash(1, 100, &[0, 1, 2]),
            synthetic_tx_hash(1, 100, &[2, 1, 0]),
            "different cycles in the same block stay distinct"
        );
        assert_ne!(
            synthetic_tx_hash(1, 100, &[0, 1]),
            synthetic_tx_hash(137, 100, &[0, 1]),
            "chain separation"
        );
    }

    // ── telemetry shape ──────────────────────────────────────────────────────

    #[test]
    fn done_event_carries_spec_keys() {
        let scan = ScanOutput {
            cycles_found: 7,
            capped: true,
            dropped_for_cap: 2,
            anchor_rejected: 3,
            v3_skipped: 4,
            ..ScanOutput::default()
        };
        let e = done_event(1, 20_000_000, 93, 180, &scan, 2, 210, 400, true);
        assert_eq!(e["event"], "route_scanner.done");
        assert_eq!(e["cycles_found"], 7);
        assert_eq!(e["elapsed_ms"], 400);
        assert_eq!(e["capped"], true);
        assert_eq!(e["enumeration_ms"], 210);
        assert_eq!(e["cycles_dispatched"], 2);
        assert_eq!(e["algorithm"], ALGORITHM);
        assert_eq!(e["dropped_for_cap"], 2);
        assert_eq!(e["v3_skipped"], 4);
        // Never aimed at the native opportunity stream.
        assert!(!serde_json::to_string(&e).unwrap().contains("opps:detected"));
    }

    #[test]
    fn cycle_event_names_shadow_forced_mode() {
        let cycle = ProfitableCycle {
            edges: vec![0, 1, 2, 3, 4, 5],
            sum_log_weight: -0.03,
            hop_count: 6,
        };
        let e = cycle_event(1, 20_000_000, &cycle, "shadow_forced");
        assert_eq!(e["event"], "route_scanner.cycle");
        assert_eq!(e["hops"], 6);
        assert_eq!(e["mode"], "shadow_forced");
    }
}
