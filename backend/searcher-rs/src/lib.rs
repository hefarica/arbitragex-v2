//! searcher-rs library facade.
//!
//! Exposes internal modules for integration tests and future orchestrator crates.
//! hash-rotation 2026-08-24 (WP-V V-004): la metadata de la lib rota el disambiguador de TODOS los bins de test dependientes — workaround documentado AppControl 4551 (bloqueo por hash de binario). Contenido semántico inalterado.
//! The binary entry point is `main.rs`.
//! hash-rotation 2026-08-25 (WP-F merge): segunda rotacion post-merge main (QB-02..04 landings).

// Suppress the same lints as main.rs for consistency. Individual modules
// carry their own allows where the pattern is demonstrably safe.
#![allow(
    unused_imports,
    unused_variables,
    unreachable_patterns,
    unexpected_cfgs,
    clippy::unwrap_used,
    clippy::manual_unwrap_or_default,
    clippy::manual_unwrap_or,
    unused_mut
)]

// Phase 1-3 modules re-exported so the library target compiles standalone.
// XLS-QB / ARBX-0008: N-bucket amount sweep surface — pure bounds + sweep
// types; the motor side lives in `size_optimizer::bucket_sweep_2leg_curve`
// over the SAME curve the golden-section kernel maximizes.
pub mod amm_math;
pub mod amount_buckets;
pub mod calldata;
pub mod canonical_enums;
pub mod canonical_knobs;
pub mod chain_client;
pub mod counters;
// XLS-QB-05c / ARBX-0003: pair→cycles inverted index — the scoped re-evaluation
// set `affected_cycles(dirty)` is linear in the dirty set, not the cycle universe.
pub mod cycle_index;
pub mod dedup;
// QUOTEBASE-264 09_RUNTIME_STRUCTURES: DirtyPairs bitset + PoolToPair fan-out
// + bounded HotSeedQueue (XLS-QB-05). Lib-only: the reserve-update hot path is
// the future consumer of `DirtyPairEngine::on_pool_event`.
pub mod dirty_pairs;
// XLS-QB-05b / ARBX-0003: consumer side of the dirty-pool signal — one drain
// per discovery tick replays `arbx:dirty_pools:<chain>` through the engine.
pub mod dirty_consumer;
// XLS-QB-05 / ARBX-0003: cross-worker dirty-pool signal (writer side lives in
// `workers::pool_sync_worker`; consumer side drains it in route discovery).
pub mod dirty_signal;
// XLS-QB-06b / ARBX-0024 (REQ-QB-008): F_e normalization — fair rate r13,
// normalized edge F_e r14, directed pair alpha r15, QuoteState r23 with the
// r25 version keys. The route-discovery prefilter consumes it as a SIGNAL
// (señal≠prueba); the exact net gate stays the only PASS authority.
pub mod fe_normalization;
// ARBX-0007: financing-mode route dimension — fee constants, per-mode
// evaluation of a sized route, and the legacy-preserving selection policy.
pub mod financing;
// ARBX-0009: sheet-07 Net_bps contract + deterministic ranking (QB 07).
pub mod net_bps_ranking;
// ARBX-QB-07-006: canonical discovery-workload builders (bench + unit tests
// share one source — the workload the Discovery_SLA gate judges).
pub mod discovery_workload;
// ARBX-0018: address-keyed token identity — per-chain TokenIdentityIndex
// cache composing the token universe (reserves::scan_token_universe) with
// the operator allowlist; the scanner attaches it to the spine evaluator.
pub mod token_identity;
// FE-MASTER EMIT-01 (ARBX-FE-EMIT-01): pre-indexed token-universe snapshot
// (norm_symbol → addresses + §6 KPIs from the REAL pair_index functions)
// published by `token_identity::index_for` on each rebuild — the Rust half
// behind POST /api/admin/tokens/resolve.
pub mod token_resolve_signal;
// Phase 16: per-strategy Prometheus metrics for the orchestrator.
pub mod impact_index;
pub mod metrics;
pub mod models;
pub mod opportunity_emitter;
pub mod pair_alpha_runtime;
pub mod pair_index;
pub mod patterns;
pub mod persistence;
pub mod pool_candidate;
pub mod pool_discovery;
pub mod pool_sources;
// Stage 2c (§IV read side): per-operator log-LR cache + the posterior fold.
pub mod priors_cache;
pub mod publisher;
// QUOTEBASE-264 05_QUOTE_BASE: QuoteScore weighted form + workbook fixtures
// (XLS-QB-06). Lib-only: consumers are the future dense-id quote-base layer.
pub mod quote_score;
// FE-MASTER EMIT-02/03 (ARBX-FE-EMIT-02/03): quote-anchor wire payloads —
// the Rust half of `frontend/lib/apex/schemas/quote.ts` (exact field names;
// absence is envelope-level, payloads are total-when-computed).
pub mod quote_anchor_runtime;
pub mod quote_anchor_signal;
pub mod reserves;
pub mod route_api;
pub mod route_decoder;
pub mod route_discovery;
pub mod route_intent;
pub mod scoring;
pub mod scoring_pipeline;
pub mod shared;
pub mod source_supervisor;
pub mod strategy_hop_mask;
// ARBX-0021: workbook col Status dispatch table (79/174/8/3), same
// generated+fixture pattern as strategy_hop_mask.
pub mod strategy_dispatch_status;
// ARBX-TW-005: workbook col Execution_Class (29 classes) — execution-
// precondition annotation enriching the dispatch reasons.
pub mod strategy_execution_class;
// ARBX-0026: workbook sheet 13_DETECTOR_POLICY (60 detectors) — graph family,
// family hop envelope, Do_Not_Do guard and hot-seed admission, consumed
// generically via the hop map's Detector_ID link.
pub mod detector_policy;
// ARBX-DP-002: sheet 13 col Required_Data as a runtime gate — data-class
// availability per detector BEFORE the math; honest NEEDS_DATA (R8), never
// an approximation substitute (13_DETECTOR_POLICY prohibits "use approximate
// price instead").
pub mod required_data_gate;
// ARBX-DP-003: sheet 13/11 col Execution_Class folded into the four emission
// tiers SIGNAL/OBSERVATION/CANDIDATE/EXECUTABLE — distinct feed fields, with
// the publish-seam gate keeping OBSERVE_ONLY and signal-tier strategies out
// of the Opportunity{confidence} shape.
pub mod signal_tier;
// ARBX-DP-004: sheet 13 col Hot_Seed (5 patterns) as HotSeedClassifier →
// DetectorMask — event kind → the detectors admissible to wake, so dispatch
// never fans out 60 × 264 × pool × block.
pub mod hot_seed_mask;
pub mod strategy_label;
// Task 2: HotPathEmitter for sub-100ms detection pipeline (Redis streams)
pub mod hot_path_emitter;
// FASE OMEGA: Gate subsystem with energy-state control plane
pub mod gates;
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
// G-SIM-1 PR-B2a: moved VERBATIM to the shared `sim-core` crate; re-exported
// so sim-ctl can consume the SAME encoder and every `crate::sim_encoder::*` /
// `searcher_rs::sim_encoder::*` call site keeps compiling unchanged.
pub use sim_core::sim_encoder;
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
// QUOTEBASE-264 10_LATENCY: the 8-stage discovery budget (lat.* keys),
// p50/p95 recorder + PASS_p95 gate (XLS-QB-07). Lib-only instrument; the
// discovery hot path is the future wiring consumer.
pub mod latency_budget;
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
// Fix B — math evidence (observe-only): builds MarketState from reserves and
// evaluates RegimeRouter-recommended operators, logging their outputs.
pub mod math_evidence;
// Carnot Orchestrator v2 — thermodynamic control plane.
pub mod thermodynamics;

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
