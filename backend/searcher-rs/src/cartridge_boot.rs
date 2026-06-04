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
    };

    // Build the runner BEFORE spawning so we can share the Arc with both the
    // subscriber task and the orchestrator (shadow evaluation).
    let runner = Arc::new(CartridgeRunner::new(host_ctx));
    let runner_for_task = runner.clone();

    tokio::spawn(async move {
        // Boot-load cartridges from the filesystem directory (dev/bootstrap path).
        // Redis-injected cartridges arrive later via the subscriber.
        let dir = std::path::Path::new(CARTRIDGE_DIR);
        let results = cartridge_loader::load_cartridges_from_dir(&runner_for_task, dir, chain_id).await;
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
            chain_id,
            "cartridge subscriber task exited"
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
pub fn build_cartridge_pool_data(
    intent: &RouteIntent,
    reserves_source: Option<&crate::reserves::ReservesEntry>,
) -> rhai::Map {
    use rhai::Dynamic;
    let mut m = rhai::Map::new();
    m.insert("chain_id".into(), Dynamic::from(intent.chain_id as i64));
    m.insert("amount_in".into(), Dynamic::from(intent.amount_in.to_string()));
    if let Some(leg) = intent.legs.first() {
        m.insert("token_in".into(), Dynamic::from(format!("{:#x}", leg.token_in)));
        m.insert("token_out".into(), Dynamic::from(format!("{:#x}", leg.token_out)));
        m.insert(
            "protocol_type".into(),
            Dynamic::from(protocol_type_str(leg.protocol_type).to_string()),
        );
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
    m
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

    let first = intent.legs.first();
    let last = intent.legs.last();
    let pool_hint = first
        .and_then(|l| l.pool_hint)
        .map(|p| format!("{:#x}", p))
        .unwrap_or_default();

    let payload = serde_json::json!({
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
    });

    let json = match serde_json::to_string(&payload) {
        Ok(s) => s,
        Err(e) => {
            warn!(event = "route_discovery.outcome_serialize_failed", error = %e);
            return;
        }
    };

    match runner
        .xadd_shadow_outcome(ROUTE_DISCOVERY_OUTCOMES_STREAM, OUTCOMES_STREAM_MAXLEN, &json)
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
    let reserves_source = match intent.legs.first().and_then(|l| l.pool_hint) {
        Some(p) => runner.read_pool_reserves(&format!("{:#x}", p)).await,
        None => None,
    };
    let pool_data = build_cartridge_pool_data(&intent, reserves_source.as_ref());

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
        let m = build_cartridge_pool_data(&intent, None);
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
        let m = build_cartridge_pool_data(&intent, None);
        assert_eq!(m.get("source_pool").unwrap().to_string(), "");
        assert_eq!(m.get("protocol_type").unwrap().to_string(), "unknown");
        // No reserves provided -> key absent -> Rhai sees () -> reserve-dependent cartridges fail-honest.
        assert!(m.get("reserves_source").is_none());
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
        assert_eq!(rmap.get("r0").unwrap().to_string(), "1500000000000000000000");
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
}
