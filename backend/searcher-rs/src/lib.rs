//! searcher-rs library facade.
//!
//! Exposes internal modules for integration tests and future orchestrator crates.
//! The binary entry point is `main.rs`.

// Suppress the same lints as main.rs for consistency. Individual modules
// carry their own allows where the pattern is demonstrably safe.
#![cfg_attr(not(test), deny(clippy::unwrap_used, clippy::expect_used))]

// Phase 1-3 modules re-exported so the library target compiles standalone.
pub mod amm_math;
pub mod calldata;
pub mod chain_client;
pub mod counters;
pub mod dedup;
// Phase 16: per-strategy Prometheus metrics for the orchestrator.
pub mod impact_index;
pub mod metrics;
pub mod models;
pub mod opportunity_emitter;
pub mod patterns;
pub mod persistence;
pub mod pool_candidate;
pub mod pool_discovery;
pub mod pool_sources;
pub mod publisher;
pub mod reserves;
pub mod route_decoder;
pub mod route_discovery;
pub mod route_intent;
pub mod scoring;
pub mod scoring_pipeline;
pub mod source_supervisor;
pub mod strategy_label;
// Phase 7-8: orchestrator + engines exposed for integration tests.
pub mod engines;
pub mod orchestrator;
// Phase 9: workers module exposed so engines can reuse pure math kernels.
// Workers themselves contain I/O-heavy code that is not called from tests;
// the lib target only needs the pure-function submodules (triangular_worker,
// flashloan_arb_worker) which have no async I/O in their math kernels.
#[allow(dead_code)]
pub mod workers;
// Phase 12: StateProjector -- virtual post-tx pool state projection.
pub mod state_projector;
// V3 oracle: on-chain QuoterV2 read-only provider feeding StateProjector.
pub mod v3_quote_provider;
// Phase 13: SizeOptimizer -- optimal amount_in sizing for arb candidates.
pub mod size_optimizer;
// Phase 11: LendingPositionIndexer -- Redis-backed watchlist + cache for
// Aave V3 / Compound V2 positions, consumed by LiquidationEngine.
pub mod lending_position_indexer;
// Phase A.3.a: `OpportunityCandidate -> RoundTripContext` encoder. Exposed
// here so integration tests can drive the encoder against real candidate
// shapes without going through `decode_and_score_tx`.
pub mod sim_encoder;
// Phase A.3.b: PostgreSQL-backed `TokenDecimalsProvider`. Exposed for
// integration tests that want to drive the provider against a test DB.
pub mod sim_encoder_pg;
// Phase A.3.c: REVM orchestrator. Exposed so integration tests can drive
// it against a real fork without going through the full hot path.
pub mod sim_orchestrator;
// Phase A.3.c.2: ERC-20 storage prefund computation. Exposed so integration
// tests + the future A.3.c.3 multi-step orchestrator can consume the
// `PrefundPlan` API to apply storage overrides on REVM state.
//
// G-SIM-1 PR-B1 (Option B): the implementation moved VERBATIM to the shared
// `sim-core` crate so sim-ctl can consume the SAME sim path. Re-exported here
// so every existing `searcher_rs::sim_prefund::*` / `crate::sim_prefund::*`
// call site keeps compiling unchanged.
pub use sim_core::sim_prefund;
// Phase OMEGA: Kelly Criterion + V3 concentrated liquidity math primitives.
// Pure module exposed so size_optimizer + tests can consume Kelly sizing.
pub mod kelly_sizing;
// Phase OMEGA 3.2: Bayesian inference + VPIN/PIN filters. Pure module
// exposed so the prioritization-spine evaluator can consume posterior
// acceptance gates without re-implementing the math.
pub mod bayesian_filter;
// Phase A.3.c.2: Multi-step REVM orchestrator. Exposed so integration
// tests + the future A.3.c.3 REVM CacheDB executor can drive the plan
// builder against fixtures and real chain state.
//
// G-SIM-1 PR-B1 (Option B): the implementation moved VERBATIM to the shared
// `sim-core` crate so sim-ctl can run the SAME wrapped-flash sim (SAME encoder,
// SAME fail-closed guards, SAME `wrapped_calldata`) rather than a second
// divergent encoder. Re-exported here so every existing
// `searcher_rs::sim_multistep::*` / `crate::sim_multistep::*` call site keeps
// compiling unchanged.
pub use sim_core::sim_multistep;
// B1.c: chain config hot-reload subscriber. Listens on the Redis pub/sub
// channel `arbx:config:chains:reload` emitted by the api-server admin
// endpoint when an operator mutates chains_runtime; tracks last-seen
// config_hash per chain to skip no-op reloads. Public so the future
// B1.d chain-task supervisor can consume the dedup map.
pub mod config_reload;
// Phase 2 Topology Vault runtime: durable fallback + Redis Pub/Sub + atomic RPC/WS hot-swap.
pub mod topology_reload;
// FASE OMEGA — Dynamic strategy cartridge runtime (Rhai scripting engine).
// Implements the "PlayStation MEV" architecture: sandboxed Rhai scripts that
// can be hot-loaded at runtime via Redis PubSub without recompiling the binary.
// Each cartridge exports `init_strategy`, `evaluate_opportunity`, `build_payload`
// and communicates with infrastructure through registered host bindings.
pub mod cartridge;
// FASE OMEGA — Filesystem-based cartridge loader for dev/bootstrap.
// Scans `cartridges/` directory at boot and loads all `.rhai` files.
// In production, hot-reload is handled by `cartridge::subscriber`.
pub mod cartridge_loader;
// FASE OMEGA — Cartridge runtime boot wiring (gated by ARBX_CARTRIDGE_MODE; default off).
// Spawns the per-chain runner + hot-reload subscriber so the (previously unspawned)
// cartridge subsystem actually loads. Dormant unless the mode flag is enabled.
pub mod cartridge_boot;
// FASE OMEGA — Block/log backrunning scanner (ARBX_MEMPOOL_MODE=block).
pub mod block_scanner;

// -- SOP-EDGE-001: Edge Node modules (paper-shadow feature gate) -------
// These modules implement the Alloy anti-mock layer, U256<->f64 normalization,
// and the 6-phase SED Engine for the Edge Node deployment.
#[cfg(feature = "paper-shadow")]
pub mod connectors;
#[cfg(feature = "paper-shadow")]
pub mod normalization;
#[cfg(feature = "paper-shadow")]
pub mod sed_engine;
// SED Bridge: connects searcher-rs I/O pipeline to sed-core math layer.
// Feeds gas-price log-returns into CDC calculator and runs eigenstate
// decomposition for stochastic dispatch decisions.
#[cfg(feature = "paper-shadow")]
pub mod sed_bridge;

// OMEGA Nivel 0 — Telemetry publisher (ConvergenceSignal → Redis).
// Fire-and-forget: tokio::spawn, never blocks the pipeline.
// NOTE: gated behind paper-shadow because it imports from sed-core,
//       which is an optional path dependency only available with that feature.
#[cfg(feature = "paper-shadow")]
pub mod telemetry_publisher;

// Observer telemetry — real-node head divergence (reorg) → arbx:telemetry:observability.
// Ungated: depends only on redis + serde, used by block_scanner in all builds.
pub mod telemetry_observability;
