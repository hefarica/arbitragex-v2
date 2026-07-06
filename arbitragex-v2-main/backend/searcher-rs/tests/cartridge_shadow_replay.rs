// FASE OMEGA — Deterministic shadow-mode validation (Option A: no live mempool).
//
// These tests validate the cartridge → orchestrator SHADOW path deterministically,
// without a live mempool feed (which the public-RPC production posture cannot provide).
// They are the offline equivalent of "enable shadow + observe `cartridge.shadow_eval`".
//
// ## What is validated (and is NOT a mock)
//
//   - `build_cartridge_pool_data`: the REAL adapter from `RouteIntent` → Rhai `Map`.
//   - `CartridgeRunner` + a self-contained Rhai test cartridge whose result DEPENDS on
//     the adapter-delivered fields (`chain_id`, `source_pool`) — so a passing assert
//     proves the data actually flowed adapter → cartridge.
//   - `Orchestrator::on_route_intent` in shadow mode (dry-run emitter): asserts the
//     cartridge evaluation NEVER reaches the execution pipeline (zero emissions).
//
// ## Why a test cartridge instead of `dex_arb.rhai`
//
// `dex_arb.rhai` reads `pool_data.reserves_source`, which the iter-2 adapter does NOT
// yet populate (it only carries chain_id/amount_in/token_in/out/protocol_type/source_pool;
// reserves are fetched live via host bindings). Enriching the adapter with reserves is the
// active-path follow-up. The test cartridge exercises the shadow MECHANISM (eval + isolation)
// with the fields the current adapter delivers — a deterministic, honest validation.
//
// ## Redis
//
// Constructing a `CartridgeRunner` requires a live Redis `ConnectionManager` (HostContext).
// These tests are `#[ignore]` and read `REDIS_URL` (default `redis://127.0.0.1:6379/`). The
// test cartridge calls NO host bindings, so the eval performs no Redis I/O — only the
// connection is established. Run with: `cargo test --test cartridge_shadow_replay -- --ignored`.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use ethers::types::{Address, H256, U256};
use searcher_rs::cartridge::host_bindings::HostContext;
use searcher_rs::cartridge::runner::CartridgeRunner;
use searcher_rs::cartridge_boot::build_cartridge_pool_data;
use searcher_rs::dedup::OppDedup;
use searcher_rs::engines::dex_engine::DexEngine;
use searcher_rs::engines::flashloan_engine::FlashloanEngine;
use searcher_rs::engines::liquidation_engine::LiquidationEngine;
use searcher_rs::engines::triangular_engine::{ReservesCache, TriangularEngine};
use searcher_rs::impact_index::ImpactIndex;
use searcher_rs::lending_position_indexer::LendingPositionIndexer;
use searcher_rs::opportunity_emitter::OpportunityEmitter;
use searcher_rs::orchestrator::{Orchestrator, OrchestratorContext};
use searcher_rs::route_intent::{
    DetectionSource, ProtocolType, RouteIntent, RouteIntentLeg, RouterKind, SwapExactMode,
};
use searcher_rs::size_optimizer::SizeOptimizer;
use searcher_rs::state_projector::StateProjector;
use std::sync::atomic::AtomicU64;
use std::sync::Arc;
use tokio::sync::RwLock;

const CHAIN_ID: u64 = 1;

// A self-contained test cartridge. `evaluate_opportunity` returns an opportunity ONLY
// when the adapter delivered `chain_id == 1` AND a non-empty `source_pool` — so the
// assertions below prove the pool_data adapter actually fed the cartridge correctly.
// It calls NO host bindings (no Redis I/O during evaluation).
const TEST_CARTRIDGE: &str = r#"
fn init_strategy() {
    #{
        name: "Test Shadow Arb",
        version: "1.0.0",
        author: "omega-test",
        description: "Deterministic test cartridge for shadow-mode validation",
        category: "test",
        target_chains: [],
        min_eval_interval_ms: 0
    }
}

fn evaluate_opportunity(pool_data) {
    let ok = (pool_data.chain_id == 1) && (pool_data.source_pool != "");
    if ok {
        #{
            is_opportunity: true,
            estimated_profit: 42.0,
            confidence: 0.9,
            urgency: "immediate"
        }
    } else {
        #{
            is_opportunity: false,
            estimated_profit: 0.0,
            confidence: 0.0,
            urgency: "none"
        }
    }
}

fn build_payload(opportunity) {
    #{
        target_contract: "TEST",
        calldata: "0x",
        value_wei: "0",
        gas_limit: 100000,
        max_priority_fee_gwei: 1.0,
        deadline_ts: 0
    }
}
"#;

/// Returns a Redis `ConnectionManager`, or `None` when Redis is unreachable (graceful skip).
async fn try_redis() -> Option<redis::aio::ConnectionManager> {
    let url = std::env::var("REDIS_URL").unwrap_or_else(|_| "redis://127.0.0.1:6379/".to_string());
    let client = redis::Client::open(url).ok()?;
    client.get_connection_manager().await.ok()
}

fn addr(n: u64) -> Address {
    Address::from_low_u64_be(n)
}

fn unit(n: u64) -> U256 {
    U256::from(10u128).pow(U256::from(18u32)) * U256::from(n)
}

fn make_intent(pool_hint: Option<Address>) -> RouteIntent {
    RouteIntent::new(
        CHAIN_ID,
        H256::from_low_u64_be(0xDEAD_BEEF),
        Address::zero(),
        RouterKind::UniswapV2,
        Address::zero(),
        vec![RouteIntentLeg {
            token_in: addr(0xAAA),
            token_out: addr(0xBBB),
            pool_hint,
            dex_hint: None,
            fee_bps: Some(30),
            protocol_type: ProtocolType::V2,
        }],
        unit(5),
        None,
        SwapExactMode::ExactIn,
        DetectionSource::PublicMempool,
    )
    .expect("valid intent — legs.len() >= 1")
}

/// Builds a `CartridgeRunner` and loads the self-contained test cartridge as "test_arb".
async fn make_runner(conn: redis::aio::ConnectionManager) -> Arc<CartridgeRunner> {
    let host_ctx = HostContext {
        redis: Arc::new(RwLock::new(conn)),
        chain_id: CHAIN_ID,
        cartridge_id: Arc::new(RwLock::new(String::new())),
        rt_handle: tokio::runtime::Handle::current(),
        block_number: Arc::new(AtomicU64::new(0)),
        base_fee_gwei: Arc::new(AtomicU64::new(0)),
        telemetry_channel: "arbx:cartridge:telemetry".to_owned(),
    };
    let runner = Arc::new(CartridgeRunner::new(host_ctx));
    runner
        .load_cartridge("test_arb", TEST_CARTRIDGE, "test-hash-1")
        .await
        .expect("test cartridge must compile + validate the Universal Contract");
    runner
}

/// Builds a full `OrchestratorContext` (real collaborators, dry-run emitter) with the
/// given cartridge runner. Mirrors `tests/v2_shadow_replay.rs::build_orchestrator_with_emitter`.
async fn build_ctx(
    cartridge_runner: Option<Arc<CartridgeRunner>>,
    conn: redis::aio::ConnectionManager,
) -> (Arc<Orchestrator>, Arc<OpportunityEmitter>) {
    let reserves_cache = Arc::new(ReservesCache::new());
    let state_projector = Arc::new(StateProjector::new(reserves_cache.clone(), None));
    let size_optimizer = Arc::new(SizeOptimizer::new(state_projector.clone()));
    let dex_engine = Arc::new(DexEngine::new(
        reserves_cache.clone(),
        None,
        Some(state_projector.clone()),
    ));
    let tri_engine = Arc::new(TriangularEngine::new(reserves_cache.clone(), vec![]));
    let fl_engine = Arc::new(FlashloanEngine::new());
    let liq_indexer = Arc::new(tokio::sync::Mutex::new(LendingPositionIndexer::new(
        CHAIN_ID,
        conn.clone(),
    )));
    let liq_engine = Arc::new(LiquidationEngine::new(liq_indexer, CHAIN_ID));

    let opp_dedup = Arc::new(OppDedup::new(256));
    let emitter = Arc::new(OpportunityEmitter::new_dry_run(opp_dedup, conn.clone()));

    let trading_config = shared_rs::trading_config::TradingConfigClient::from_manager(conn.clone());
    let config_provider = Arc::new(searcher_rs::orchestrator::ConfigProvider { trading_config });

    let impact_idx_arc = Arc::new(RwLock::new(ImpactIndex::empty()));
    let pool_discovery = Arc::new(searcher_rs::pool_discovery::PoolDiscoveryService::new(
        CHAIN_ID,
        None,
        conn.clone(),
        impact_idx_arc.clone(),
        None,
        reserves_cache.clone(),
    ));

    let ctx = OrchestratorContext {
        impact_index: impact_idx_arc,
        dex_engine,
        triangular_engine: tri_engine,
        flashloan_engine: fl_engine,
        liquidation_engine: liq_engine,
        state_projector,
        size_optimizer,
        emitter: emitter.clone(),
        config_provider,
        pool_discovery,
        chain_id: CHAIN_ID,
        cartridge_runner,
    };

    (Arc::new(Orchestrator::new(ctx)), emitter)
}

// ---------------------------------------------------------------------------
// Test A — deterministic evaluation through the REAL adapter.
//
// Proves: build_cartridge_pool_data(intent) feeds the cartridge correct data, and the
// cartridge's evaluate_opportunity computes the expected result from it. The cartridge's
// result DEPENDS on chain_id==1 && source_pool!="", so the assertions prove the data flow.
// ---------------------------------------------------------------------------
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "requires Redis (REDIS_URL or 127.0.0.1:6379) — run with: cargo test --test cartridge_shadow_replay -- --ignored"]
async fn cartridge_evaluates_deterministically_from_adapter_pool_data() {
    let Some(conn) = try_redis().await else {
        eprintln!("SKIP: Redis unreachable — cannot construct CartridgeRunner");
        return;
    };
    let runner = make_runner(conn).await;

    // Positive: pool_hint set → adapter emits non-empty source_pool + chain_id=1 → opportunity.
    let intent = make_intent(Some(addr(0x100)));
    let pool_data = build_cartridge_pool_data(&intent, None);
    let result = runner
        .evaluate("test_arb", pool_data)
        .await
        .expect("cartridge evaluation must succeed");

    assert!(
        result.is_opportunity,
        "cartridge must detect an opportunity when the adapter delivers chain_id=1 + source_pool"
    );
    assert_eq!(
        result.estimated_profit, 42.0,
        "deterministic profit mismatch"
    );
    assert_eq!(result.confidence, 0.9, "deterministic confidence mismatch");
    assert_eq!(result.urgency, "immediate", "urgency mismatch");

    // Negative: pool_hint None → adapter emits empty source_pool (R8 fail-honest) → no opp.
    let intent_no_pool = make_intent(None);
    let pd2 = build_cartridge_pool_data(&intent_no_pool, None);
    let r2 = runner
        .evaluate("test_arb", pd2)
        .await
        .expect("cartridge evaluation must succeed");
    assert!(
        !r2.is_opportunity,
        "empty source_pool (pool_hint None) must yield no opportunity (R8 propagation)"
    );
}

// ---------------------------------------------------------------------------
// Test B — on_route_intent shadow isolation.
//
// Proves: calling Orchestrator::on_route_intent with a cartridge_runner present (shadow)
// runs to completion and records ZERO emissions — the cartridge evaluation never reaches
// the execution pipeline (no StrategyCandidate, no process_candidate, no emitter write).
// This is the no-capital-path guarantee, exercised end-to-end through the real orchestrator.
// ---------------------------------------------------------------------------
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "requires Redis (REDIS_URL or 127.0.0.1:6379) — run with: cargo test --test cartridge_shadow_replay -- --ignored"]
async fn on_route_intent_with_cartridge_runner_does_not_emit() {
    let Some(conn) = try_redis().await else {
        eprintln!("SKIP: Redis unreachable — cannot construct CartridgeRunner");
        return;
    };
    let runner = make_runner(conn.clone()).await;
    let (orchestrator, emitter) = build_ctx(Some(runner), conn).await;

    // Empty ImpactIndex → the built-in engines produce nothing (impact_zero early return),
    // so any non-empty emission could ONLY come from the cartridge path. The cartridge
    // shadow eval is spawned (observe-only) and must NOT emit.
    let intent = make_intent(Some(addr(0x100)));
    orchestrator
        .on_route_intent(intent)
        .await
        .expect("on_route_intent must not Err in shadow mode");

    // Let the detached shadow-eval task run.
    tokio::time::sleep(std::time::Duration::from_millis(400)).await;

    let emissions = emitter.recorded_emissions();
    assert!(
        emissions.is_empty(),
        "shadow cartridge evaluation must NOT emit to the execution pipeline (no capital path); \
         got {} emission(s): {:?}",
        emissions.len(),
        emissions.iter().map(|e| e.strategy).collect::<Vec<_>>()
    );
}

// A cartridge that CALLS a host binding (get_reserves), which internally does
// `rt_handle.block_on`. This is what dex_arb does and what panicked live.
const HOST_BINDING_CARTRIDGE: &str = r#"
fn init_strategy() {
    #{ name: "HB Probe", version: "1.0.0", author: "omega-test",
       description: "calls a host binding to exercise block_on", category: "test",
       target_chains: [], min_eval_interval_ms: 0 }
}
fn evaluate_opportunity(pool_data) {
    // Triggers rt_handle.block_on inside the get_reserves binding. We don't care about
    // the value (Redis may be empty -> ()), only that the call does not panic.
    let r = get_reserves(pool_data.source_pool);
    #{ is_opportunity: false, estimated_profit: 0.0, confidence: 0.0, urgency: "none" }
}
fn build_payload(opportunity) {
    #{ target_contract: "T", calldata: "0x", value_wei: "0", gas_limit: 1, max_priority_fee_gwei: 0.0, deadline_ts: 0 }
}
"#;

// ---------------------------------------------------------------------------
// Test C — host binding eval must not panic in an async runtime worker.
//
// Regression guard for the live block_on panic: a cartridge that calls get_reserves
// (which does rt_handle.block_on internally) must evaluate cleanly when run from an
// async context (the orchestrator shadow path). The fix is `block_in_place` around
// `call_fn` in CartridgeRunner::evaluate — which REQUIRES the multi-threaded runtime.
// ---------------------------------------------------------------------------
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "requires Redis (REDIS_URL or 127.0.0.1:6379) — run with: cargo test --test cartridge_shadow_replay -- --ignored"]
async fn host_binding_eval_does_not_panic_in_async_context() {
    let Some(conn) = try_redis().await else {
        eprintln!("SKIP: Redis unreachable");
        return;
    };
    let host_ctx = HostContext {
        redis: Arc::new(RwLock::new(conn)),
        chain_id: CHAIN_ID,
        cartridge_id: Arc::new(RwLock::new(String::new())),
        rt_handle: tokio::runtime::Handle::current(),
        block_number: Arc::new(AtomicU64::new(0)),
        base_fee_gwei: Arc::new(AtomicU64::new(0)),
        telemetry_channel: "arbx:cartridge:telemetry".to_owned(),
    };
    let runner = Arc::new(CartridgeRunner::new(host_ctx));
    runner
        .load_cartridge("hb_probe", HOST_BINDING_CARTRIDGE, "hb-hash-1")
        .await
        .expect("host-binding cartridge must compile + validate");

    let intent = make_intent(Some(addr(0x100)));
    let pool_data = build_cartridge_pool_data(&intent, None);
    // This drives get_reserves -> rt_handle.block_on from an async worker. Without
    // block_in_place it panics ("Cannot block the current thread from within a runtime").
    let res = runner
        .evaluate("hb_probe", pool_data)
        .await
        .expect("evaluate calling a host binding must not panic/err");
    assert!(!res.is_opportunity);
}
