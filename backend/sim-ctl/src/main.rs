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
mod sim_engine;
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
    // Otherwise, return 501 for the selected route_source until Fases 2-4 land.
    match req.candidate {
        Some(_candidate) => {
            // Pre-enriched path: simulate with the full OpportunityCandidate.
            // TODO (B2c): wire candidate → sim-core encoder → execute_multistep_revm
            let payload = NotImplementedPayload::new(
                vec!["sim_core_encoder", "execute_multistep_revm"],
                "B2c",
                "OpportunityCandidate received but sim-core encoder not yet wired (Fase 1.2 stub)".into(),
            );
            let body = serde_json::to_value(payload).unwrap_or_else(
                |e| serde_json::json!({"error":"serialisation_failure","detail":e.to_string()}),
            );
            (StatusCode::NOT_IMPLEMENTED, Json(body))
        }
        None => {
            // Enrichment path selected but not yet implemented (Fases 2-4).
            let source_str = match req.route_source {
                RouteSource::PgMetadata => "pg_metadata (A1)",
                RouteSource::SearcherApi => "searcher_api (A2)",
                RouteSource::SimctlLookup => "simctl_lookup (A3)",
            };
            let payload = NotImplementedPayload::new(
                vec![source_str, "route_enrichment"],
                "B2b",
                format!(
                    "Route source '{}' selected but enrichment not yet implemented (Fases 2-4 pending)",
                    source_str
                ),
            );
            let body = serde_json::to_value(payload).unwrap_or_else(
                |e| serde_json::json!({"error":"serialisation_failure","detail":e.to_string()}),
            );
            (StatusCode::NOT_IMPLEMENTED, Json(body))
        }
    }
}

/// Legacy simulation path for backward compatibility (pre-B2b schema).
async fn simulate_legacy(st: Arc<AppState>, opp: Opportunity) -> impl IntoResponse {
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
            let redis_client = redis::Client::open(redis_url)?;
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

    let state = Arc::new(AppState {
        backend,
        engine: engine.clone(),
        env: cfg.system.env.clone(),
    });

    // HTTP server
    let port: u16 = std::env::var("SIM_PORT")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(3003);
    let sim_router = Router::new()
        .route("/simulate", post(simulate_handler))
        .route("/fork-status", get(fork_status_handler))
        .with_state(state);
    let app = build_health_router(ServiceInfo::new(SERVICE, VERSION)).merge(sim_router);
    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    let listener = tokio::net::TcpListener::bind(addr).await?;
    info!(event = "service.boot", service = SERVICE, env = %cfg.system.env, port,
          fork_ready = fork.is_some(),
          "sim-ctl listening");

    // Spawn consumer if we have fork AND DB + Redis.
    if fork.is_some() {
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
                engine: SimEngine {
                    fork,
                    signer_from: signer,
                    timeout: sim_timeout,
                    max_slippage_for_pass_pct: max_slippage,
                },
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
