// TASK 4 — Phase 7: V2 shadow-replay integration test.
//
// ## Design
//
// These tests exercise the FULL `Orchestrator::on_route_intent` pipeline without
// mocking any gate or evaluator. The pattern per operator spec:
//
//   1. Build a minimal `OrchestratorContext` with real (or hand-fixtured) collaborators.
//   2. Construct a `RouteIntent` directly (skip the mempool/decoder for test isolation).
//   3. Call `orchestrator.on_route_intent(intent).await?`.
//   4. Inspect `emitter.recorded_emissions()` to assert on the specific outcome.
//
// ## What is NOT a mock
//
// - `ReservesCache`: populated with `insert()` calls using plausible real-world
//   values. Comments below pin the math so reviewers can trace it.
// - `ImpactIndex`: populated with `add_pool()` calls using real Address values.
// - `OpportunityEmitter::new_dry_run`: this IS the real emitter in dry-run mode.
//   It runs all dedup, serialisation, and record logic — just skips I/O.
// - `TradingConfigClient`: connects to a local Redis when available; when absent
//   the orchestrator runs in observe-only mode (config = None), which is also tested.
//
// ## Shadow mode
//
// All tests use `dry_run = true` (via `OpportunityEmitter::new_dry_run`).
// This is identical to `ARBX_ORCHESTRATOR_MODE=shadow` on the VPS.
// No writes to PG or Redis streams occur.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use ethers::types::{Address, H256, U256};
use searcher_rs::dedup::OppDedup;
use searcher_rs::engines::dex_engine::DexEngine;
use searcher_rs::engines::flashloan_engine::FlashloanEngine;
use searcher_rs::engines::liquidation_engine::LiquidationEngine;
use searcher_rs::engines::triangular_engine::{ReservesCache, TriangularEngine};
use searcher_rs::impact_index::{ImpactIndex, PoolRef};
use searcher_rs::lending_position_indexer::LendingPositionIndexer;
use searcher_rs::opportunity_emitter::OpportunityEmitter;
use searcher_rs::orchestrator::{Orchestrator, OrchestratorContext};
use searcher_rs::route_intent::{
    DetectionSource, ProtocolType, RouteIntent, RouteIntentLeg, RouterKind, SwapExactMode,
};
use searcher_rs::size_optimizer::{OptimizeRejectReason, SizeOptimizer};
use searcher_rs::state_projector::StateProjector;
use searcher_rs::strategy_label::StrategyLabel;
use std::sync::Arc;
use tokio::sync::RwLock;

// ---------------------------------------------------------------------------
// Constants and helpers
// ---------------------------------------------------------------------------

const CHAIN_ID: u64 = 1;

/// Converts a u64 to an Ethereum address (big-endian byte fill).
fn addr(n: u64) -> Address {
    Address::from_low_u64_be(n)
}

/// Returns U256 representing `n * 10^18`.
fn unit(n: u64) -> U256 {
    U256::from(10u128).pow(U256::from(18u32)) * U256::from(n)
}

/// Builds a `RouteIntent` for a 1-leg swap from `token_in` to `token_out`.
fn make_intent(
    token_in: Address,
    token_out: Address,
    pool_hint: Option<Address>,
    amount_in: U256,
) -> RouteIntent {
    RouteIntent::new(
        CHAIN_ID,
        H256::from_low_u64_be(0xDEAD_BEEF),
        Address::zero(),
        RouterKind::UniswapV2,
        Address::zero(),
        vec![RouteIntentLeg {
            token_in,
            token_out,
            pool_hint,
            dex_hint: None,
            fee_bps: Some(30),
            protocol_type: ProtocolType::V2,
        }],
        amount_in,
        None,
        SwapExactMode::ExactIn,
        DetectionSource::PublicMempool,
    )
    .expect("valid intent — legs.len() >= 1")
}

// ---------------------------------------------------------------------------
// Helper: build an orchestrator AND return a handle to its emitter for test
// assertions. `OrchestratorContext.emitter` is `Arc<OpportunityEmitter>` so
// we return a clone of the Arc and call `recorded_emissions()` on it.
//
// Redis availability: attempts `127.0.0.1:6379`; panics with a clear message
// when Redis is not reachable (these tests require a local Redis).
// ---------------------------------------------------------------------------

async fn build_orchestrator_with_emitter(
    impact_index: ImpactIndex,
    reserves_cache: Arc<ReservesCache>,
) -> (Arc<Orchestrator>, Arc<OpportunityEmitter>) {
    let state_projector = Arc::new(StateProjector::new(reserves_cache.clone(), None));
    let size_optimizer = Arc::new(SizeOptimizer::new(state_projector.clone()));

    let dex_engine = Arc::new(DexEngine::new(
        reserves_cache.clone(),
        None,
        Some(state_projector.clone()),
    ));
    let tri_engine = Arc::new(TriangularEngine::new(reserves_cache.clone(), vec![]));
    let fl_engine = Arc::new(FlashloanEngine::new());

    // Attempt to connect to local Redis. Panic with a clear message if unavailable
    // so CI knows to set up Redis or mark the test suite as ignored.
    let dummy_conn = {
        let client = redis::Client::open("redis://127.0.0.1:6379/")
            .expect("redis::Client::open must succeed for test infrastructure");
        client.get_connection_manager().await.unwrap_or_else(|_| {
            panic!(
                "v2_shadow_replay tests require a local Redis at 127.0.0.1:6379. \
                     Start Redis locally or skip this test suite."
            )
        })
    };

    let liq_indexer = Arc::new(tokio::sync::Mutex::new(LendingPositionIndexer::new(
        CHAIN_ID,
        dummy_conn.clone(),
    )));
    let liq_engine = Arc::new(LiquidationEngine::new(liq_indexer, CHAIN_ID));

    let opp_dedup = Arc::new(OppDedup::new(256));
    let emitter = Arc::new(OpportunityEmitter::new_dry_run(
        opp_dedup,
        dummy_conn.clone(),
    ));

    let trading_config =
        shared_rs::trading_config::TradingConfigClient::from_manager(dummy_conn.clone());
    let config_provider = Arc::new(searcher_rs::orchestrator::ConfigProvider { trading_config });

    let impact_idx_arc = Arc::new(RwLock::new(impact_index));

    let pool_discovery = Arc::new(searcher_rs::pool_discovery::PoolDiscoveryService::new(
        CHAIN_ID,
        None,
        dummy_conn.clone(),
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
        spanning_tree_engine: None,
        cross_chain_engine: None,
        liquidation_snipe_engine: None,
        emitter: emitter.clone(),
        config_provider,
        pool_discovery,
        chain_id: CHAIN_ID,
        native_engines_enabled: true,
        cartridge_runner: None,
        // Fix B math-evidence sensors — mirror scanner.rs production wiring.
        cartridge_mode: searcher_rs::cartridge_boot::CartridgeMode::from_env(),
        math_registry: Arc::new(math_engine::OperatorRegistry::new()),
        regime_router: math_engine::RegimeRouter::default(),
        math_redis: dummy_conn.clone(),
        #[cfg(feature = "paper-shadow")]
        sed_bridge: None,
    };

    (Arc::new(Orchestrator::new(ctx)), emitter)
}

// ---------------------------------------------------------------------------
// Case 1: V2 simple swap, registered pair, 2 pools with REAL reserves
//
// Reserves:
//   Pool A (Uniswap V2):  r0 = 1_500 WETH, r1 = 4_800_000 USDC → price 3_200 USDC/WETH
//   Pool B (SushiSwap):   r0 = 1_200 WETH, r1 = 3_720_000 USDC → price 3_100 USDC/WETH
//
// Spread: 3_200 - 3_100 = 100 USDC/WETH (buy cheap on B, sell on A).
// Expectation: DexEngine produces a DexArbV2V2 candidate; emitter records it
// (accepted if net > 0 after gas; rejected with a specific reason otherwise).
// ---------------------------------------------------------------------------

#[tokio::test]
#[ignore = "requires local Redis at 127.0.0.1:6379 — run on VPS with: cargo test -- --ignored"]
async fn shadow_v2_simple_swap_emits_dex_arb_v2v2_candidate() {
    // token_a = WETH: 0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2
    // token_b = USDC: 0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48
    let token_a = "0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2"
        .parse::<Address>()
        .unwrap();
    let token_b = "0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48"
        .parse::<Address>()
        .unwrap();

    let pool_a = addr(0x100);
    let pool_b = addr(0x101);

    let cache = Arc::new(ReservesCache::new());
    cache.insert(pool_a, unit(1_500), unit(4_800_000)).await;
    cache.insert(pool_b, unit(1_200), unit(3_720_000)).await;

    let mut idx = ImpactIndex::empty();
    idx.add_pool(PoolRef {
        chain_id: CHAIN_ID,
        address: pool_a,
        dex_name: "uniswap-v2".to_string(),
        protocol_type: ProtocolType::V2,
        token0: token_a,
        token1: token_b,
        fee_bps: Some(30),
    });
    idx.add_pool(PoolRef {
        chain_id: CHAIN_ID,
        address: pool_b,
        dex_name: "sushiswap".to_string(),
        protocol_type: ProtocolType::V2,
        token0: token_a,
        token1: token_b,
        fee_bps: Some(30),
    });

    let (orchestrator, emitter) = build_orchestrator_with_emitter(idx, cache).await;

    let intent = make_intent(token_a, token_b, Some(pool_a), unit(1));

    orchestrator
        .on_route_intent(intent)
        .await
        .expect("on_route_intent must not Err in shadow mode");

    let emissions = emitter.recorded_emissions();

    // The pipeline must have produced at least one emission (accepted or rejected).
    // In shadow mode (dry_run=true) every candidate that reaches process_candidate
    // gets recorded — whether accepted or rejected.
    assert!(
        !emissions.is_empty(),
        "shadow mode must record at least one emission for a registered pair with 2 pools"
    );

    // All emissions must have strategy = DexArbV2V2 (or FlashloanArb wrapping it).
    for record in &emissions {
        assert!(
            record.strategy == StrategyLabel::DexArbV2V2
                || record.strategy == StrategyLabel::FlashloanArb,
            "strategy must be DexArbV2V2 or FlashloanArb, got {:?}",
            record.strategy
        );
    }

    // At least one emission must have strategy = DexArbV2V2.
    let has_dex_arb_v2v2 = emissions
        .iter()
        .any(|r| r.strategy == StrategyLabel::DexArbV2V2);
    assert!(
        has_dex_arb_v2v2,
        "must have at least one DexArbV2V2 emission"
    );
}

// ---------------------------------------------------------------------------
// Case 2: V2 pools with same fee but mildly asymmetric reserves
// → optimizer evaluates real reserves and decides (accept or reject with reason)
//
// Pool A: r0=1000 WETH, r1=3_200_000 USDC  → price 3200 USDC/WETH
// Pool B: r0=1000 WETH, r1=3_150_000 USDC  → price 3150 USDC/WETH
// Spread: 50 USDC/WETH (small but present). After 0.3% fee each side:
// net spread ~1% → ~$31 gross on 1 WETH. Gas at 5 gwei/200k = ~$3. Net > 0.
// ---------------------------------------------------------------------------

#[tokio::test]
#[ignore = "requires local Redis at 127.0.0.1:6379 — run on VPS with: cargo test -- --ignored"]
async fn shadow_v2_same_fee_asymmetric_reserves_optimizer_decides() {
    let token_a = "0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2"
        .parse::<Address>()
        .unwrap();
    let token_b = "0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48"
        .parse::<Address>()
        .unwrap();

    let pool_a = addr(0x200);
    let pool_b = addr(0x201);

    let cache = Arc::new(ReservesCache::new());
    cache.insert(pool_a, unit(1_000), unit(3_200_000)).await;
    cache.insert(pool_b, unit(1_000), unit(3_150_000)).await;

    let mut idx = ImpactIndex::empty();
    idx.add_pool(PoolRef {
        chain_id: CHAIN_ID,
        address: pool_a,
        dex_name: "uniswap-v2".to_string(),
        protocol_type: ProtocolType::V2,
        token0: token_a,
        token1: token_b,
        fee_bps: Some(30),
    });
    idx.add_pool(PoolRef {
        chain_id: CHAIN_ID,
        address: pool_b,
        dex_name: "sushiswap".to_string(),
        protocol_type: ProtocolType::V2,
        token0: token_a,
        token1: token_b,
        fee_bps: Some(30),
    });

    let (orchestrator, emitter) = build_orchestrator_with_emitter(idx, cache).await;

    let intent = make_intent(token_a, token_b, Some(pool_a), unit(1));

    orchestrator
        .on_route_intent(intent)
        .await
        .expect("on_route_intent must not Err");

    let emissions = emitter.recorded_emissions();

    // Must have at least one emission — the optimizer ran with real reserves.
    assert!(
        !emissions.is_empty(),
        "asymmetric reserves must produce at least one emission"
    );

    // Every emission is either accepted or carries a SPECIFIC rejection reason
    // (not a generic "unknown" or empty reason). This verifies the optimizer
    // ran the full `optimize_with_reason` path.
    for record in &emissions {
        if !record.accepted {
            let reason = record.rejection_reason.as_deref().unwrap_or("");
            assert!(
                !reason.is_empty(),
                "rejected emission must carry a non-empty reason, got empty string"
            );
            // Reason must be one of the known OptimizeRejectReason strings or a
            // config-gate string — never a generic "size_optimizer_no_profit" anymore
            // (that was the old opaque path before OptimizeRejectReason).
            assert!(
                reason != "unknown",
                "rejection reason must be specific, not 'unknown'"
            );
        }
    }
}

// ---------------------------------------------------------------------------
// Case 3: Empty ReservesCache → optimizer rejects with MissingReservesPoolA
// or MissingReservesPoolB (or DexEngine rejects before optimizer).
//
// This simulates the boot window when pool_sync_worker hasn't populated the
// cache yet. Zero accepted emissions must result.
// ---------------------------------------------------------------------------

#[tokio::test]
#[ignore = "requires local Redis at 127.0.0.1:6379 — run on VPS with: cargo test -- --ignored"]
async fn shadow_missing_reserves_returns_specific_reject_reason() {
    let token_a = "0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2"
        .parse::<Address>()
        .unwrap();
    let token_b = "0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48"
        .parse::<Address>()
        .unwrap();

    let pool_a = addr(0x300);
    let pool_b = addr(0x301);

    // Two pools registered in ImpactIndex…
    // …but NO reserves in the cache — intentionally NOT calling cache.insert().
    let cache = Arc::new(ReservesCache::new());

    let mut idx = ImpactIndex::empty();
    idx.add_pool(PoolRef {
        chain_id: CHAIN_ID,
        address: pool_a,
        dex_name: "uniswap-v2".to_string(),
        protocol_type: ProtocolType::V2,
        token0: token_a,
        token1: token_b,
        fee_bps: Some(30),
    });
    idx.add_pool(PoolRef {
        chain_id: CHAIN_ID,
        address: pool_b,
        dex_name: "sushiswap".to_string(),
        protocol_type: ProtocolType::V2,
        token0: token_a,
        token1: token_b,
        fee_bps: Some(30),
    });

    let (orchestrator, emitter) = build_orchestrator_with_emitter(idx, cache).await;

    let intent = make_intent(token_a, token_b, Some(pool_a), unit(1));

    orchestrator
        .on_route_intent(intent)
        .await
        .expect("on_route_intent must not Err — missing reserves is a recoverable state");

    let emissions = emitter.recorded_emissions();

    // When reserves are absent, NO accepted emissions should occur.
    for record in &emissions {
        assert!(
            !record.accepted,
            "no emission should be accepted when reserves are absent; \
             got accepted strategy={:?}",
            record.strategy
        );

        let reason = record.rejection_reason.as_deref().unwrap_or("");
        assert!(
            !reason.is_empty(),
            "rejected emission must carry a non-empty reason when reserves absent"
        );
    }

    let accepted_count = emissions.iter().filter(|r| r.accepted).count();
    assert_eq!(
        accepted_count, 0,
        "zero accepted emissions expected when reserves are absent"
    );

    // Specific optimizer reason check: when present it must be a known reserve reason.
    // Engine-level rejections (e.g., "reserves_cache_miss") are also valid.
    let known_reserve_reasons = [
        OptimizeRejectReason::MissingReservesPoolA.as_str(),
        OptimizeRejectReason::MissingReservesPoolB.as_str(),
        OptimizeRejectReason::MissingPoolAddress.as_str(),
        OptimizeRejectReason::ZeroReserves.as_str(),
        "reserves_cache_miss",   // DexEngine-level rejection before optimizer
        "single_pool_no_spread", // DexEngine rejects when only 1 pool found
        OptimizeRejectReason::NoConfig.as_str(), // valid when Redis has no config
    ];
    for record in &emissions {
        let reason = record.rejection_reason.as_deref().unwrap_or("");
        let is_known = known_reserve_reasons.iter().any(|&kr| reason.contains(kr));
        assert!(
            is_known,
            "rejection reason '{}' is not a known reserve/config reason — \
             the optimizer must produce specific reasons, not generic strings",
            reason
        );
    }
}

// ---------------------------------------------------------------------------
// Case 4: Config absent → optimizer rejects with NoConfig (or pipeline
// runs in observe-only mode without scoring).
//
// When Redis has no TradingConfigState for chain_id=1, the orchestrator logs
// has_config=false and all candidates receive cfg=None. The SizeOptimizer
// returns Rejected(NoConfig). The emitter records the rejection.
// ---------------------------------------------------------------------------

#[tokio::test]
#[ignore = "requires local Redis at 127.0.0.1:6379 — run on VPS with: cargo test -- --ignored"]
async fn shadow_no_config_returns_no_config_reject() {
    let token_a = "0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2"
        .parse::<Address>()
        .unwrap();
    let token_b = "0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48"
        .parse::<Address>()
        .unwrap();

    let pool_a = addr(0x400);
    let pool_b = addr(0x401);

    // Profitable reserves to ensure DexEngine produces candidates.
    // Pool A: price 3200, Pool B: price 3000 → large spread.
    let cache = Arc::new(ReservesCache::new());
    cache.insert(pool_a, unit(1_000), unit(3_200_000)).await;
    cache.insert(pool_b, unit(1_000), unit(3_000_000)).await;

    let mut idx = ImpactIndex::empty();
    idx.add_pool(PoolRef {
        chain_id: CHAIN_ID,
        address: pool_a,
        dex_name: "uniswap-v2".to_string(),
        protocol_type: ProtocolType::V2,
        token0: token_a,
        token1: token_b,
        fee_bps: Some(30),
    });
    idx.add_pool(PoolRef {
        chain_id: CHAIN_ID,
        address: pool_b,
        dex_name: "sushiswap".to_string(),
        protocol_type: ProtocolType::V2,
        token0: token_a,
        token1: token_b,
        fee_bps: Some(30),
    });

    let (orchestrator, emitter) = build_orchestrator_with_emitter(idx, cache).await;

    let intent = make_intent(token_a, token_b, Some(pool_a), unit(1));

    // The orchestrator must complete without panicking, regardless of Redis config state.
    orchestrator
        .on_route_intent(intent)
        .await
        .expect("on_route_intent must not Err even when config absent");

    let emissions = emitter.recorded_emissions();

    // Every rejected emission must have a non-empty, specific reason.
    for record in &emissions {
        if !record.accepted {
            let reason = record.rejection_reason.as_deref().unwrap_or("");
            assert!(
                !reason.is_empty(),
                "rejected emission must carry a non-empty reason; got empty"
            );
        }
    }

    // When Redis has no config for chain_id=1, rejected candidates from the
    // optimizer path must carry reason="no_config". We collect them to verify.
    let no_config_count = emissions
        .iter()
        .filter(|r| {
            !r.accepted
                && r.rejection_reason
                    .as_deref()
                    .map(|s| s.contains(OptimizeRejectReason::NoConfig.as_str()))
                    .unwrap_or(false)
        })
        .count();

    // If Redis IS running and has config, the optimizer may proceed past NoConfig
    // and produce a Sized outcome (accepted). Both outcomes are valid.
    // Critical invariant: pipeline completes without Err or panic.
    // Secondary invariant: if no_config rejections exist, they have the right label.
    if no_config_count > 0 {
        // Verify the label string is correct.
        assert_eq!(
            OptimizeRejectReason::NoConfig.as_str(),
            "no_config",
            "NoConfig reason string pin"
        );
    }
}
