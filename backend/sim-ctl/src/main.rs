// M11 (audit 2026-05-10): surface panics in hot-path crate.
#![warn(clippy::unwrap_used, clippy::expect_used)]

//! sim-ctl main. Spawns:
//!   - Axum HTTP (/health, /metrics, /simulate)
//!   - Redis Streams consumer (if ANVIL_URL + DB reachable)
//!
//! When ANVIL_URL unset or anvil unreachable, /simulate keeps responding 501
//! and the consumer logs idle; never fabricates results.

mod anvil_backend;
mod consumer;
mod fork_manager;
mod persistence;
mod revm_backend;
mod route_lookup;
mod sim_engine;
mod sim_runner;
mod simulator_backend;
mod tx_builder;

use axum::{
    extract::State,
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use ethers::types::Address;
use shared_rs::{
    candidates::OpportunityCandidate,
    config::AppConfig,
    contracts::{NotImplementedPayload, Opportunity},
    health::{build_health_router, ServiceInfo},
    killswitch::KillSwitchClient,
    logging::init_tracing,
    metrics::init_metrics,
};
use std::{net::SocketAddr, str::FromStr, sync::Arc, time::Duration};
use tracing::{info, warn};

use crate::anvil_backend::AnvilBackend;
use crate::consumer::Consumer;
use crate::fork_manager::ForkManager;
use crate::revm_backend::RevmBackend;
use crate::sim_engine::SimEngine;
use crate::simulator_backend::SimulatorBackend;

const SERVICE: &str = "sim-ctl";
const VERSION: &str = env!("CARGO_PKG_VERSION");
// DEV-ONLY sentinel "caller" address used when simulating a probe and no signer
// is configured. Must NEVER be selected in staging/prod — guarded at use site.
const DEV_SENTINEL_SIGNER: &str = "0x000000000000000000000000000000000000dEaD";

/// Route source selector for triple-path enrichment (G-SIM-1 PR-B2b).
///
/// A1 = PG route_metadata (persistent, source of truth in opportunities table)
/// A2 = searcher-rs HTTP API (memory, fast, cached)
/// A3 = sim-ctl PG lookup (autonomous, independent)
#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "snake_case")]
enum RouteSource {
    PgMetadata,
    SearcherApi,
    SimctlLookup,
}

/// Simulation request body with route source selector (G-SIM-1 PR-B2b Fase 1.2).
///
/// The frontend includes `route_source` to choose between the three enrichment
/// paths (A1/A2/A3). Each path constructs the same `OpportunityCandidate` but
/// fetches route metadata from a different source.
///
/// When `candidate` is provided (pre-enriched by frontend or api-server),
/// sim-ctl skips enrichment and uses it directly. When null, sim-ctl enriches
/// via the selected `route_source` path (implemented in Fases 2-4).
#[derive(Debug, Clone, serde::Deserialize)]
struct SimulateRequest {
    route_source: RouteSource,
    /// Optional: pre-enriched candidate. When provided, enrichment is skipped.
    #[serde(default)]
    candidate: Option<OpportunityCandidate>,
    /// Opportunity ID for A3 path (sim-ctl PG lookup). When route_source =
    /// simctl_lookup and candidate is None, sim-ctl queries PG for this ID.
    /// Forwarded by api-server from the URL path param `/api/v1/opportunities/:id`.
    #[serde(default)]
    opportunity_id: Option<uuid::Uuid>,
    /// Optional block pin for deterministic replay (S4 PARCH-0). The
    /// drift-tracker sends the SETTLED block (`sim_block_number + 1`) so the
    /// Y-oracle measures at the block the opportunity would have landed in.
    /// When provided, the A3 path builds a per-request `SimulatorV2` pinned
    /// to this block (the shared handle memoizes one block forever).
    #[serde(default)]
    block_number: Option<u64>,
}

#[derive(Clone)]
struct AppState {
    /// HTTP /simulate handler uses the backend trait so either Anvil or REVM
    /// can serve the endpoint without changing the handler code.
    backend: Arc<dyn SimulatorBackend>,
    /// Consumer still uses SimEngine directly (Anvil-only path) until a
    /// consumer-level backend abstraction lands in a follow-up sprint.
    engine: Arc<SimEngine>,
    env: String,
    /// G-SIM-1 PR-B2b Fase 4 (A3): optional PG pool for autonomous route
    /// metadata lookup. None when DATABASE_URL is absent (A3 path disabled;
    /// caller falls back to A1/A2).
    db_pool: Option<sqlx::PgPool>,
    /// G-SIM-1 B2c: shared `SimulatorV2` handle for the REAL multi-step path.
    /// `None` when the backend is anvil (real-sim path disabled).
    simulator: Option<Arc<simulator_v2::SimulatorV2>>,
    /// G-SIM-1 B2c: env-driven config for the real sim (executor address, gas
    /// limit, min profit). `None` when `ARBITRAGE_EXECUTOR` is unset — the
    /// real-sim path returns a typed 501 in that case.
    real_sim_env: Option<sim_runner::RealSimEnvConfig>,
    /// G-SIM-1 B2c: Redis connection for reading live gas_price_wei.
    /// `None` when REDIS_URL is absent (real-sim returns a typed 501).
    redis: Option<Arc<tokio::sync::Mutex<redis::aio::ConnectionManager>>>,
}

async fn simulate_handler(
    State(st): State<Arc<AppState>>,
    Json(body): Json<serde_json::Value>,
) -> impl IntoResponse {
    // Anvil backend: if fork is absent, keep 501 by design (existing behaviour).
    // REVM backend: no fork dependency — always serves the request (result may be
    // `passed=false, fail_reason="revm_not_implemented_sprint4"` until 4.2/4.3 land).
    if st.engine.fork.is_none() && st.backend.name() == "anvil" {
        let payload = NotImplementedPayload::new(
            vec!["ANVIL_URL", "ANVIL_FORK_URL"],
            "S4",
            format!("sim-ctl up but anvil not configured (env={})", st.env),
        );
        let body = serde_json::to_value(payload).unwrap_or_else(
            |e| serde_json::json!({"error":"serialisation_failure","detail":e.to_string()}),
        );
        return (StatusCode::NOT_IMPLEMENTED, Json(body));
    }

    // G-SIM-1 PR-B2b Fase 1.2: parse new SimulateRequest with route_source selector.
    let req: SimulateRequest = match serde_json::from_value(body.clone()) {
        Ok(r) => r,
        Err(e) => {
            // Fallback: try legacy Opportunity schema for backward compatibility.
            let opp: Opportunity = match serde_json::from_value(body) {
                Ok(o) => o,
                Err(e2) => {
                    return (
                        StatusCode::BAD_REQUEST,
                        Json(serde_json::json!({
                            "error":"invalid_body",
                            "detail":format!("Neither SimulateRequest nor Opportunity: {} | {}", e, e2)
                        })),
                    );
                }
            };
            // Legacy path: convert Opportunity → SimulateRequest with default route_source.
            return simulate_legacy(st, opp).await;
        }
    };

    // G-SIM-1 PR-B2b Fase 1.2: if candidate is pre-enriched, use it directly.
    // Otherwise, enrich via route_source (A3 simctl_lookup constructs the
    // candidate itself — S4 PARCH-0).
    match req.candidate {
        Some(candidate) => match dispatch_b2c_real_sim(&st, candidate, req.block_number).await {
            Ok(mut response) => {
                response["route_source"] = serde_json::json!("pre_enriched");
                (StatusCode::OK, Json(response))
            }
            Err(tuple) => tuple,
        },
        None => {
            // No pre-enriched candidate: route_source determines the enrichment path.
            match req.route_source {
                RouteSource::SimctlLookup => {
                    // G-SIM-1 PR-B2b Fase 4 (A3): autonomous PG lookup.
                    let opp_id = match req.opportunity_id {
                        Some(id) => id,
                        None => {
                            let body = serde_json::json!({
                                "error": "missing_opportunity_id",
                                "detail": "route_source=simctl_lookup requires opportunity_id in the request body"
                            });
                            return (StatusCode::BAD_REQUEST, Json(body));
                        }
                    };
                    let pool = match &st.db_pool {
                        Some(p) => p,
                        None => {
                            let body = serde_json::json!({
                                "error": "a3_unavailable",
                                "detail": "sim-ctl has no PG pool (DATABASE_URL not configured); try route_source=pg_metadata or searcher_api"
                            });
                            return (StatusCode::SERVICE_UNAVAILABLE, Json(body));
                        }
                    };
                    // S4 PARCH-0: A3 constructs the OpportunityCandidate from the
                    // opportunities row + route_metadata, then dispatches to the
                    // REAL B2c multi-step REVM simulation. The old stub answered
                    // `{"status":"route_metadata_fetched"}` with NO `passed`
                    // field, so the drift-tracker could never parse a
                    // SimOutcome out of it → Canal B produced 0 labels ever.
                    let inputs = match route_lookup::fetch_candidate_inputs(pool, opp_id).await {
                        Ok(Some(i)) => i,
                        Ok(None) => {
                            let body = serde_json::json!({
                                "error": "route_metadata_not_found",
                                "opportunity_id": opp_id,
                                "detail": "opportunity has no route_metadata or does not exist"
                            });
                            return (StatusCode::NOT_FOUND, Json(body));
                        }
                        Err(e) => {
                            warn!(
                                event = "sim.a3_pg_error",
                                opportunity_id = %opp_id,
                                error = %e,
                                "A3 candidate-input lookup PG error"
                            );
                            let body = serde_json::json!({
                                "error": "pg_error",
                                "opportunity_id": opp_id,
                                "detail": e.to_string()
                            });
                            return (StatusCode::INTERNAL_SERVER_ERROR, Json(body));
                        }
                    };

                    // Completeness gates → typed 422 `candidate_incomplete`
                    // (S4-02 STRUCTURAL family: the row lacks what the encoder
                    // needs; retrying will not change the row).
                    let token_addresses = &inputs.route_metadata.token_addresses;
                    if token_addresses.is_empty() {
                        return a3_candidate_incomplete(
                            opp_id,
                            "token_addresses empty in route_metadata",
                        );
                    }
                    if let Err(missing) =
                        inputs.resolved_decimals.validate_complete(token_addresses)
                    {
                        return a3_candidate_incomplete(
                            opp_id,
                            format!(
                                "missing decimals for {missing:?} (route_metadata empty by design + tokens table unresolved)"
                            ),
                        );
                    }
                    if inputs.chain_id <= 0 {
                        return a3_candidate_incomplete(
                            opp_id,
                            format!("chain_id not a valid EVM chain: {}", inputs.chain_id),
                        );
                    }
                    let amount_in_wei: u128 = match inputs.amount_in_wei.trim().parse() {
                        Ok(w) if w > 0 => w,
                        _ => {
                            return a3_candidate_incomplete(
                                opp_id,
                                format!(
                                    "amount_in_wei unparseable or zero: {:?}",
                                    inputs.amount_in_wei
                                ),
                            );
                        }
                    };
                    // validate_complete already guaranteed this entry; the
                    // match keeps the trading path defensive (no unwrap).
                    let decimals_in = match inputs.resolved_decimals.get(&token_addresses[0]) {
                        Some(d) => d,
                        None => {
                            return a3_candidate_incomplete(
                                opp_id,
                                format!("missing decimals for token_in {}", token_addresses[0]),
                            );
                        }
                    };
                    let amount_in = amount_in_wei as f64 / 10f64.powi(i32::from(decimals_in));
                    if !amount_in.is_finite() || amount_in <= 0.0 {
                        return a3_candidate_incomplete(
                            opp_id,
                            format!("amount_in non-finite or non-positive after wei conversion ({amount_in})"),
                        );
                    }

                    let candidate = OpportunityCandidate {
                        opportunity_id: opp_id,
                        chain_id: inputs.chain_id as u64,
                        token_addresses: token_addresses.clone(),
                        pool_addresses: inputs.route_metadata.pool_addresses.clone(),
                        dex_adapters: inputs.route_metadata.dex_adapters.clone(),
                        amount_in,
                        // The encoder consumes ONLY amount_in + topology +
                        // decimals (build_round_trip_context_from_candidate);
                        // expected_amount_out / gross_profit are carried
                        // unused and are NOT persisted on the opportunities
                        // row — sourced honestly as 0.0 (R8, never fabricated).
                        expected_amount_out: 0.0,
                        gross_profit: 0.0,
                        decimals: inputs.resolved_decimals.clone(),
                        block_number: req
                            .block_number
                            .or(inputs.block_number.filter(|b| *b >= 0).map(|b| b as u64)),
                        // Deterministic A3 fingerprint. The executor's
                        // routeHash is event-only (no registry validation),
                        // and the searcher's engine-specific fingerprint is
                        // NOT persisted — information loss documented. Same
                        // convention as dex_engine.rs / variance_benchmark.
                        route_fingerprint: format!(
                            "{}_{}_{}",
                            inputs.dex_a, inputs.token_in, inputs.token_out
                        ),
                    };

                    match dispatch_b2c_real_sim(&st, candidate, req.block_number).await {
                        Ok(mut response) => {
                            response["route_source"] = serde_json::json!("simctl_lookup");
                            response["opportunity_id"] = serde_json::json!(opp_id.to_string());
                            (StatusCode::OK, Json(response))
                        }
                        Err(tuple) => tuple,
                    }
                }
                RouteSource::PgMetadata | RouteSource::SearcherApi => {
                    // A1 and A2 are handled by api-server before reaching sim-ctl.
                    // If sim-ctl receives these, the upstream didn't enrich —
                    // return honest 501 with a pointer to the correct path.
                    let (source_tag, source_label) = match req.route_source {
                        RouteSource::PgMetadata => ("pg_metadata", "pg_metadata (A1)"),
                        RouteSource::SearcherApi => ("searcher_api", "searcher_api (A2)"),
                        RouteSource::SimctlLookup => unreachable!(),
                    };
                    let payload = NotImplementedPayload::new(
                        vec![source_tag, "route_enrichment"],
                        "B2b",
                        format!(
                            "Route source '{}' should be enriched by api-server before reaching sim-ctl; no candidate provided",
                            source_label
                        ),
                    );
                    let body = serde_json::to_value(payload).unwrap_or_else(
                        |e| serde_json::json!({"error":"serialisation_failure","detail":e.to_string()}),
                    );
                    (StatusCode::NOT_IMPLEMENTED, Json(body))
                }
            }
        }
    }
}

/// Typed 422 for an A3 row that cannot yield a complete candidate (S4 PARCH-0).
///
/// `candidate_incomplete` is a STRUCTURAL family reason (S4-02): the row
/// itself lacks what the encoder needs — retrying will not change it, and the
/// drift-tracker must never turn it into a calibration label.
fn a3_candidate_incomplete(
    opp_id: uuid::Uuid,
    detail: impl std::fmt::Display,
) -> (StatusCode, Json<serde_json::Value>) {
    (
        StatusCode::UNPROCESSABLE_ENTITY,
        Json(serde_json::json!({
            "error": "candidate_incomplete",
            "opportunity_id": opp_id.to_string(),
            "detail": detail.to_string()
        })),
    )
}

/// Shared B2c REAL multi-step REVM dispatch (G-SIM-1 B2c + S4 PARCH-0).
///
/// Serves both the pre-enriched path (`candidate=Some`) and the A3
/// `simctl_lookup` path. Fail-honest typed 501/503 for missing prerequisites
/// (backend/env/Redis/gas); a `passed=false` outcome is a 200, not an HTTP
/// error (R8 — the simulation ran and the market rejected the route).
///
/// `block_override`: per-request block pin (the drift-tracker sends the
/// SETTLED block so the Y-oracle measures at the block the opportunity would
/// have landed in). The shared AppState simulator memoizes ONE block forever
/// (OnceLock), so any pinned request gets a dedicated `SimulatorV2`
/// instance — `sim_multistep` replays at `simulator.pinned_block()`, not at
/// the candidate's block_number.
async fn dispatch_b2c_real_sim(
    st: &AppState,
    candidate: OpportunityCandidate,
    block_override: Option<u64>,
) -> Result<serde_json::Value, (StatusCode, Json<serde_json::Value>)> {
    let simulator = match &st.simulator {
        Some(s) => s.clone(),
        None => {
            return Err((
                StatusCode::NOT_IMPLEMENTED,
                Json(serde_json::json!({
                    "error": "real_sim_unavailable",
                    "detail": "SIM_BACKEND!=revm — real multi-step REVM path requires SIM_BACKEND=revm"
                })),
            ));
        }
    };
    let simulator = match block_override.or(candidate.block_number) {
        Some(b) => {
            Arc::new(simulator_v2::SimulatorV2::new(simulator.rpc_url.clone()).with_block(b))
        }
        None => simulator,
    };
    let env_config = match &st.real_sim_env {
        Some(c) => c.clone(),
        None => {
            return Err((
                StatusCode::NOT_IMPLEMENTED,
                Json(serde_json::json!({
                    "error": "real_sim_env_missing",
                    "detail": "ARBITRAGE_EXECUTOR env var required for real simulation"
                })),
            ));
        }
    };
    let redis = match &st.redis {
        Some(r) => r.clone(),
        None => {
            return Err((
                StatusCode::SERVICE_UNAVAILABLE,
                Json(serde_json::json!({
                    "error": "gas_price_unavailable",
                    "detail": "REDIS_URL not configured — cannot read live gas_price_wei"
                })),
            ));
        }
    };

    // Read live gas_price_wei from Redis (same key scheme as RevmBackend).
    let gas_price_wei = match read_gas_price(&redis, candidate.chain_id).await {
        Ok(g) => g,
        Err(e) => {
            return Err((
                StatusCode::SERVICE_UNAVAILABLE,
                Json(serde_json::json!({
                    "error": "gas_price_read_failed",
                    "detail": e
                })),
            ));
        }
    };

    // Dispatch the REAL multi-step REVM simulation.
    let outcome =
        sim_runner::run_real_simulation(candidate, simulator, &env_config, gas_price_wei).await;

    // Map SimulationOutcome → JSON response with wrapped_calldata.
    Ok(serde_json::json!({
        "passed": outcome.passed,
        "gas_used_total": outcome.gas_used_total,
        "gas_price_wei": outcome.gas_price_wei.to_string(),
        "simulated_profit_token_in": outcome.simulated_profit_token_in.to_string(),
        "intermediate_amount_out": outcome.intermediate_amount_out.map(|v| v.to_string()),
        "fail_reason": outcome.fail_reason,
        "wrapped_calldata": outcome.wrapped_calldata.as_ref().map(|bytes| format!("0x{}", hex::encode(bytes))),
    }))
}

/// Read live gas_price_wei from Redis for the REAL sim path (G-SIM-1 B2c).
///
/// Uses the same key scheme as `RevmBackend` (`gas_price_wei_key(chain_id)`,
/// written by gas_oracle_worker every ~10s). Fail-honest: returns Err rather
/// than fabricating gas_price_wei=0 (which would report gross as net).
async fn read_gas_price(
    redis: &Arc<tokio::sync::Mutex<redis::aio::ConnectionManager>>,
    chain_id: u64,
) -> Result<ethers::types::U256, String> {
    use redis::AsyncCommands;
    use shared_rs::pre_execute_checklist::gas_price_wei_key;

    let key = gas_price_wei_key(chain_id);
    let mut conn = redis.lock().await;
    let val: Option<String> = conn
        .get(&key)
        .await
        .map_err(|e| format!("Redis GET {key} failed: {e}"))?;
    match val {
        Some(s) => s
            .parse::<ethers::types::U256>()
            .map_err(|_| format!("gas_price_wei unparseable: {s:?}"))
            .and_then(|v| {
                if v == ethers::types::U256::zero() {
                    Err("gas_price_wei is zero in Redis".into())
                } else {
                    Ok(v)
                }
            }),
        None => Err(format!(
            "gas_price_wei key {key} not in Redis — gas_oracle_worker not running or chain_id unknown"
        )),
    }
}

/// Legacy simulation path for backward compatibility (pre-B2b schema).
///
/// Returns the concrete `(StatusCode, Json<Value>)` type (not `impl IntoResponse`)
/// so the caller `simulate_handler` can unify this branch with its other return
/// paths that use the same concrete tuple type.
async fn simulate_legacy(
    st: Arc<AppState>,
    opp: Opportunity,
) -> (StatusCode, Json<serde_json::Value>) {
    let sim = match st.backend.simulate(&opp).await {
        Ok(r) => r,
        Err(e) => {
            warn!(event = "sim.backend_infra_error", backend = st.backend.name(), error = %e);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error":"backend_infra_error","detail":e.to_string()})),
            );
        }
    };
    let body = serde_json::to_value(sim).unwrap_or_else(
        |e| serde_json::json!({"error":"serialisation_failure","detail":e.to_string()}),
    );
    (StatusCode::OK, Json(body))
}

/// GET /fork-status — honest fork health for components/ForkValidationPanel.tsx,
/// consumed through the api-server proxy GET /api/sim-ctl/fork-status.
///
/// R8 fail-honest contract (mirrors what the proxy + panel expect):
///   - No fork configured (ANVIL_URL unset/unreachable at boot)  → 503, no block.
///   - Fork present but eth_blockNumber errors (anvil went away)  → 503, no block.
///   - Fork present and the block fetch succeeds → 200 with ONLY honestly-sourced
///     fields: `block_number` (real eth_blockNumber), `rpc_latency_ms` (measured
///     around that single call), `executor_address` (the configured signer), and
///     `status` ("HEALTHY", only reachable once the block fetch succeeds).
///
/// `fork_age_seconds` and `simulations_today` are deliberately NOT emitted:
/// sim-ctl keeps no fork-creation timestamp nor a per-day simulation counter, so
/// fabricating them would violate fail-honest. The api-server proxy already
/// defaults both to 0. `rpc_url_redacted` is also omitted on purpose — anvil/RPC
/// URLs can embed API keys, so we never echo them; the proxy substitutes
/// "sim-ctl:***". A non-2xx here is mapped by the proxy to an honest 404
/// "DEGRADED — paper mode" without ever inventing a block number.
async fn fork_status_handler(State(st): State<Arc<AppState>>) -> impl IntoResponse {
    let fork = match st.engine.fork.as_ref() {
        Some(f) => f,
        None => {
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(serde_json::json!({
                    "error": "fork_not_configured",
                    "detail": format!("sim-ctl up but anvil not configured (env={})", st.env),
                })),
            );
        }
    };

    let started = std::time::Instant::now();
    let block_number = match fork.current_block().await {
        Ok(bn) => bn,
        Err(e) => {
            warn!(event = "fork_status.block_query_failed", error = %e);
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(serde_json::json!({
                    "error": "fork_unreachable",
                    "detail": e.to_string(),
                })),
            );
        }
    };
    let rpc_latency_ms = u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX);

    (
        StatusCode::OK,
        Json(serde_json::json!({
            "metrics": {
                "block_number": block_number,
                "rpc_latency_ms": rpc_latency_ms,
                "executor_address": format!("{:?}", st.engine.signer_from),
                "status": "HEALTHY",
            },
            "generated_at": chrono::Utc::now().to_rfc3339(),
        })),
    )
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cfg = AppConfig::load()?;
    init_tracing(SERVICE, &cfg.observability.log_level)?;
    init_metrics();

    // Try to connect to anvil. If absent or unreachable → engine without fork → 501.
    let anvil_url = std::env::var("ANVIL_URL").unwrap_or_default();
    let sim_cfg = cfg.simulation.clone();
    let sim_timeout = sim_cfg
        .as_ref()
        .map(|c| Duration::from_millis(c.sim_timeout_ms))
        .unwrap_or(Duration::from_secs(3));
    let max_slippage = sim_cfg
        .as_ref()
        .map(|c| c.max_slippage_for_pass_pct)
        .unwrap_or(5.0);
    let pool_size = sim_cfg.as_ref().map(|c| c.snapshot_pool_size).unwrap_or(4);

    let fork = if anvil_url.is_empty() {
        warn!(
            event = "sim.anvil_not_configured",
            "ANVIL_URL empty — /simulate will return 501, consumer stays idle"
        );
        None
    } else {
        match ForkManager::connect(&anvil_url, pool_size, 5000).await {
            Ok(fm) => {
                let bn = fm.current_block().await.unwrap_or(0);
                info!(event = "sim.anvil_connected", url = %anvil_url, block = bn);
                Some(fm)
            }
            Err(e) => {
                warn!(event = "sim.anvil_unreachable", url = %anvil_url, error = %e,
                      "continuing with 501 responses; no fabrication");
                None
            }
        }
    };

    let signer = match std::env::var("SIM_SIGNER_ADDRESS") {
        Ok(v) if !v.is_empty() => Address::from_str(&v).unwrap_or_else(|_| Address::zero()),
        _ => {
            if cfg.system.env != "development" {
                anyhow::bail!(
                    "SIM_SIGNER_ADDRESS is required in {} (non-development). The dev sentinel \
                     signer is not allowed outside local dev — no-hardcode doctrine.",
                    cfg.system.env
                );
            }
            warn!(
                event = "sim.signer.dev_sentinel",
                sentinel = DEV_SENTINEL_SIGNER,
                "SIM_SIGNER_ADDRESS not set; using dev sentinel (development env only)"
            );
            Address::from_str(DEV_SENTINEL_SIGNER).unwrap_or_else(|_| Address::zero())
        }
    };

    let engine = Arc::new(SimEngine {
        fork: fork.clone(),
        signer_from: signer,
        timeout: sim_timeout,
        max_slippage_for_pass_pct: max_slippage,
    });

    // Backend selection: SIM_BACKEND env var chooses Anvil (default) or REVM.
    // Anvil path is unchanged for existing deploys; REVM is strictly opt-in.
    let backend: Arc<dyn SimulatorBackend> = match std::env::var("SIM_BACKEND")
        .as_deref()
        .unwrap_or("anvil")
    {
        "revm" => {
            // RevmBackend reads live gas_price_wei from Redis (sister key to
            // gas_price_ts) so it can charge gas inside revm. Required for
            // CRITICAL #2 fix (G-NET-1: net-of-gas P&L). REDIS_URL is
            // mandatory when SIM_BACKEND=revm — no silent fall-through.
            let redis_url = std::env::var("REDIS_URL").map_err(|_| {
                anyhow::anyhow!(
                    "SIM_BACKEND=revm requires REDIS_URL — RevmBackend reads live gas_price_wei from Redis"
                )
            })?;
            let redis_client = redis::Client::open(redis_url.clone())?;
            let redis_conn = redis::aio::ConnectionManager::new(redis_client).await?;
            let b = RevmBackend::from_env(redis_conn)
                .map_err(|e| anyhow::anyhow!("RevmBackend::from_env: {e}"))?;
            info!(event = "sim.backend_selected", backend = "revm-v2");
            Arc::new(b)
        }
        "anvil" | "" => {
            info!(event = "sim.backend_selected", backend = "anvil");
            Arc::new(AnvilBackend::new(engine.clone()))
        }
        other => {
            anyhow::bail!(
                "Unknown SIM_BACKEND value '{}'. Valid values: 'anvil' (default), 'revm'.",
                other
            );
        }
    };

    // G-SIM-1 PR-B2b Fase 4 (A3): optional PG pool for autonomous route lookup.
    // Best-effort: if DATABASE_URL is absent or connection fails, the A3 path
    // stays disabled (db_pool = None) and the handler returns 503 for
    // route_source=simctl_lookup. Fail-honest: no fabrication.
    let db_pool = match std::env::var("DATABASE_URL") {
        Ok(url) if !url.is_empty() => {
            match shared_rs::db_pool::options_with_timeouts(
                &shared_rs::db_pool::PoolConfig::from_env(2),
            )
            .connect(&url)
            .await
            {
                Ok(p) => {
                    info!(event = "sim.db_connected", "PG pool up for A3 route lookup");
                    Some(p)
                }
                Err(e) => {
                    warn!(
                        event = "sim.db_connect_failed",
                        error = %e,
                        "continuing without PG pool (A3 path disabled)"
                    );
                    None
                }
            }
        }
        _ => {
            warn!(
                event = "sim.db_not_configured",
                "DATABASE_URL absent; A3 route lookup path disabled"
            );
            None
        }
    };

    // G-SIM-1 B2c: build the REAL multi-step sim path components.
    // simulator: Option<Arc<SimulatorV2>> — only populated when SIM_BACKEND=revm,
    // so the handler can call execute_multistep_revm. Uses the SAME REVM_RPC_URL
    // the RevmBackend reads (both point at the same fork RPC).
    let simulator: Option<Arc<simulator_v2::SimulatorV2>> =
        if std::env::var("SIM_BACKEND").as_deref().unwrap_or("anvil") == "revm" {
            let rpc_url = std::env::var("REVM_RPC_URL").unwrap_or_default();
            if rpc_url.is_empty() {
                warn!(
                    event = "b2c.simulator_no_rpc",
                    "REVM_RPC_URL empty — real-sim path disabled (B2c inactive)"
                );
                None
            } else {
                Some(Arc::new(simulator_v2::SimulatorV2::new(rpc_url)))
            }
        } else {
            None
        };

    // real_sim_env: Option<RealSimEnvConfig> — best-effort load. None when
    // ARBITRAGE_EXECUTOR is unset (real-sim returns a typed 501 in that case).
    let real_sim_env = match sim_runner::RealSimEnvConfig::from_env() {
        Ok(c) => {
            info!(event = "b2c.env_loaded", "real-sim env config loaded");
            Some(c)
        }
        Err(e) => {
            warn!(event = "b2c.env_missing", error = %e, "real-sim path disabled");
            None
        }
    };

    // G-SIM-1 B2c: Redis handle for reading live gas_price_wei in the handler.
    // Best-effort: None when REDIS_URL is absent (real-sim returns a typed 501).
    let redis_handle: Option<Arc<tokio::sync::Mutex<redis::aio::ConnectionManager>>> =
        match std::env::var("REDIS_URL") {
            Ok(url) if !url.is_empty() => match redis::Client::open(url) {
                Ok(client) => match redis::aio::ConnectionManager::new(client).await {
                    Ok(cm) => Some(Arc::new(tokio::sync::Mutex::new(cm))),
                    Err(e) => {
                        warn!(event = "b2c.redis_cm_failed", error = %e, "gas_price_wei read disabled");
                        None
                    }
                },
                Err(e) => {
                    warn!(event = "b2c.redis_client_failed", error = %e);
                    None
                }
            },
            _ => None,
        };

    let state = Arc::new(AppState {
        backend: backend.clone(),
        engine: engine.clone(),
        env: cfg.system.env.clone(),
        db_pool,
        simulator,
        real_sim_env,
        redis: redis_handle,
    });

    // HTTP server
    let port: u16 = std::env::var("SIM_PORT")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(3003);
    let sim_router = Router::new()
        .route("/simulate", post(simulate_handler))
        .route("/fork-status", get(fork_status_handler))
        // G-SIM-1 FASE 1: topological truth for the readiness verifier.
        // Handler + tests live in the lib target (see src/lib.rs) so CI's
        // `cargo test --workspace --lib` gate actually executes them.
        .route(
            sim_ctl::capabilities::CAPABILITIES_PATH,
            get(sim_ctl::capabilities::capabilities_handler),
        )
        .with_state(state);
    let app = build_health_router(ServiceInfo::new(SERVICE, VERSION)).merge(sim_router);
    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    let listener = tokio::net::TcpListener::bind(addr).await?;
    info!(event = "service.boot", service = SERVICE, env = %cfg.system.env, port,
          fork_ready = fork.is_some(),
          "sim-ctl listening");

    // Spawn consumer if a simulator is actually available AND DB + Redis.
    // SIMWIRE-01: anvil needs a live fork; the REVM backend runs in-process
    // (LazyDb over REVM_RPC_URL) and must not be gated on ANVIL_URL.
    let backend_available = fork.is_some() || backend.name() != "anvil";
    if backend_available {
        if let (Ok(db_url), Ok(redis_url)) =
            (std::env::var("DATABASE_URL"), std::env::var("REDIS_URL"))
        {
            // OMEGA-8/M3 P1-2: timeouts applied.
            let pool = shared_rs::db_pool::options_with_timeouts(
                &shared_rs::db_pool::PoolConfig::from_env(4),
            )
            .connect(&db_url)
            .await?;
            let redis_client = redis::Client::open(redis_url.clone())?;
            let redis_conn = redis_client.get_connection_manager().await?;
            let killswitch =
                KillSwitchClient::connect(&redis_url, cfg.system.kill_switch_enabled_default)
                    .await
                    .map_err(|e| anyhow::anyhow!("killswitch: {e}"))?;
            let consumer = Consumer {
                redis: redis_conn,
                pool,
                // SIMWIRE-01: the stream path drives the SAME backend the
                // HTTP /simulate endpoint uses (SIM_BACKEND-selected).
                backend,
                killswitch,
                consumer_name: std::env::var("HOSTNAME").unwrap_or_else(|_| "sim-1".into()),
            };
            tokio::spawn(async move {
                if let Err(e) = consumer.run().await {
                    tracing::error!(event = "sim_consumer.fatal", error = %e);
                }
            });
            info!(event = "sim_consumer.spawned");
        } else {
            warn!(
                event = "sim_consumer.not_spawned",
                reason = "DATABASE_URL or REDIS_URL missing"
            );
        }
    }

    axum::serve(listener, app)
        .with_graceful_shutdown(async {
            tokio::signal::ctrl_c().await.ok();
        })
        .await?;
    Ok(())
}
