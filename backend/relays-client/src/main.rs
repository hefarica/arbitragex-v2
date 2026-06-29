// M11 (audit 2026-05-10): gate PRs that introduce panicking paths in entry point.
// Promoted from warn→deny so CI fails on new unwrap/expect in this binary.
#![deny(clippy::unwrap_used, clippy::expect_used)]

//! relays-client main.
//!
//! Boots:
//!   - Axum HTTP /health /metrics /execute (hot-path invocation).
//!   - Redis Streams consumer on arbx:opps:simulated (if signer+RPC present).
//!
//! Without FLASHBOTS_SIGNER_KEY or RPC_HTTP_1: /execute returns 501,
//! consumer doesn't spawn. paper_mode=true by default; only builds+signs
//! (no submit).

mod bundle_builder;
mod consumer;
mod live_exec_policy;
mod multi_relay;
mod nonce_manager;
mod persistence;
mod relay_bloxroute;
mod relay_catalog;
mod relay_flashbots;
mod relay_titan;
mod signer;
mod submit_engine;
mod tracker;

use axum::{extract::State, http::StatusCode, response::IntoResponse, routing::post, Json, Router};
use shared_rs::{
    config::{require_env, AppConfig},
    contracts::{NotImplementedPayload, Opportunity},
    health::{build_health_router, ServiceInfo},
    killswitch::KillSwitchClient,
    logging::init_tracing,
    metrics::init_metrics,
    paper_mode::PaperModeClient,
    rpc_failover::HttpRpcPool,
};
use std::{net::SocketAddr, sync::Arc, time::Duration};
use tracing::{info, warn};

use crate::multi_relay::MultiRelayClient;
use crate::nonce_manager::NonceManager;
use crate::relay_bloxroute::BloXRouteClient;
use crate::relay_flashbots::FlashbotsClient;
use crate::relay_titan::TitanClient;
use crate::signer::Signer;
use crate::submit_engine::SubmitEngine;

const SERVICE: &str = "relays-client";
const VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Clone)]
struct AppState {
    engine: Arc<SubmitEngine>,
    has_signer: bool,
    env: String,
}

async fn execute_handler(
    State(st): State<Arc<AppState>>,
    Json(body): Json<serde_json::Value>,
) -> impl IntoResponse {
    if !st.has_signer {
        let payload = NotImplementedPayload::new(
            vec!["FLASHBOTS_SIGNER_KEY", "RPC_HTTP_1"],
            "S5",
            format!(
                "relays-client up but signer not configured (env={})",
                st.env
            ),
        );
        // serde_json::to_value only fails if the type contains a non-string map key
        // or an f32/f64 NaN. NotImplementedPayload is a flat struct with String fields;
        // serialisation is infallible in practice. Use unwrap_or to convert the
        // impossible error path to a generic 500 body instead of a panic.
        let body = serde_json::to_value(payload).unwrap_or_else(
            |e| serde_json::json!({"error":"serialisation_failure","detail":e.to_string()}),
        );
        return (StatusCode::NOT_IMPLEMENTED, Json(body));
    }
    let opp: Opportunity = match serde_json::from_value(body) {
        Ok(o) => o,
        Err(e) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({"error":"invalid_body","detail":e.to_string()})),
            )
        }
    };
    // Manual HTTP execute path does not carry a sim-computed exec_payload →
    // None (legacy direct-router encoding, still M1-gated).
    let result = st.engine.execute(&opp, None).await;
    let body = serde_json::to_value(result).unwrap_or_else(
        |e| serde_json::json!({"error":"serialisation_failure","detail":e.to_string()}),
    );
    (StatusCode::OK, Json(body))
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cfg = Arc::new(AppConfig::load()?);
    init_tracing(SERVICE, &cfg.observability.log_level)?;
    init_metrics();

    let redis_url = require_env("REDIS_URL")?;
    let killswitch = KillSwitchClient::connect(&redis_url, cfg.system.kill_switch_enabled_default)
        .await
        .map_err(|e| anyhow::anyhow!("killswitch: {e}"))?;

    let paper_mode = PaperModeClient::connect(&redis_url, cfg.execution.paper_mode)
        .await
        .map_err(|e| anyhow::anyhow!("papermode: {e}"))?;

    // SECURE_BOOT (audit A2, 2026-05-10): refuse to start if paper_mode=false
    // AND ARBX_SIMULATOR_V2_READY != "true".
    //
    // Rationale: simulator-v2 currently returns SimError::NotImplemented.
    // The v1 fallback in prioritization-spine emits a cosmetic PASS without
    // validating against real on-chain state. If paper_mode is disabled without
    // a real simulator, every bundle that reaches relays-client is effectively
    // unsimulated — a pre-mainnet landmine that can drain funds.
    //
    // The operator gate is ARBX_SIMULATOR_V2_READY=true. Set it ONLY after
    // completing the checklist in docs/operations/SIMULATOR_V2_READINESS.md.
    // Any other value (missing, "false", typo) keeps this guard armed.
    {
        let live_mode = !paper_mode.is_enabled().await;
        if live_mode {
            let sim_ready = std::env::var("ARBX_SIMULATOR_V2_READY")
                .map(|v| v.eq_ignore_ascii_case("true"))
                .unwrap_or(false);
            if !sim_ready {
                anyhow::bail!(
                    "SECURE_BOOT (audit A2): paper_mode=false but ARBX_SIMULATOR_V2_READY != \
                     'true'. simulator-v2 returns NotImplemented; the v1 fallback emits a \
                     cosmetic PASS without validating against real chain state — broadcasting \
                     unsimulated bundles is a pre-mainnet landmine. Set \
                     ARBX_SIMULATOR_V2_READY=true only after completing every item in \
                     docs/operations/SIMULATOR_V2_READINESS.md."
                );
            }
            tracing::warn!(
                event = "secure_boot.sim_v2_gate_passed",
                "paper_mode=false AND ARBX_SIMULATOR_V2_READY=true — live submission enabled"
            );
        }
    }

    // Try to load signer.
    let chain_id = cfg
        .chains
        .iter()
        .find(|c| c.enabled)
        .map(|c| c.chain_id)
        .unwrap_or(1);
    // M1 (2026-06-28): default-deny + testnet-only live-execution barrier.
    // relays-client is the ONLY binary that can sign+broadcast; unlike searcher-rs
    // (hard capital-key boot panic) it was gated only by soft flags, so an env
    // mistake could broadcast mainnet. Assert fail-fast at boot that a LIVE
    // (paper_mode=false) node may only target an allowlisted testnet — mainnet is
    // physically refused. The same policy is re-checked on every build_and_sign
    // call (the runtime barrier that also catches paper_mode flipped AFTER boot).
    {
        let policy = live_exec_policy::LiveExecPolicy::from_env();
        let live_mode = !paper_mode.is_enabled().await;
        info!(
            event = "live_exec.policy",
            enabled = policy.enabled,
            allowed_chains = ?policy.allowed_chains,
            chain_id,
            live_mode,
            "M1 live-execution policy resolved (default-deny, testnet-only; mainnet refused)"
        );
        if live_mode {
            if let Err(e) = policy.assert_broadcast_allowed(chain_id) {
                anyhow::bail!(
                    "M1 live-exec lockout: paper_mode=false but broadcasting on chain_id={chain_id} \
                     is refused — {e}. Live execution is default-deny + testnet-only: set \
                     ARBX_LIVE_EXEC_ENABLED=true and target an allowlisted testnet (default Sepolia \
                     11155111). Mainnet (chain_id=1) is physically refused in this phase."
                );
            }
        }
    }

    let signer = match Signer::from_env(chain_id)? {
        Some(s) => {
            info!(event = "signer.loaded", address = %s.address, chain_id = s.chain_id);
            Some(Arc::new(s))
        }
        None => {
            warn!(
                event = "signer.missing",
                "FLASHBOTS_SIGNER_KEY empty/unset — /execute stays 501, consumer idle"
            );
            None
        }
    };

    // RPC failover discipline (G-RPC-1): build a multi-vendor HTTP pool from
    // env `RPC_HTTP_<chain_id>` (CSV `name=url,name=url`). The pool validates
    // each provider's chain_id at boot, exposes Prometheus metrics, drift
    // detection and per-provider circuit breakers via the spawned health loop.
    //
    // BE-02 Step 1: the pool is stored as Arc<HttpRpcPool> and passed directly
    // into SubmitEngine + NonceManager. All RPC call sites (nonce fetch via
    // NonceManager::fetch, provider selection for build_and_sign and
    // wait_for_inclusion) now route through the pool so the circuit breaker
    // and EWMA-ranked failover fire on production traffic.
    //
    // spawn_health_loop spawns a detached tokio task; dropping the JoinHandle
    // does not cancel it (tokio 1.x semantics — the task continues until process
    // exit). The match-arm-scoped `_health_task` binding therefore documents
    // intent only; the loop's lifetime is the process, not the binding.
    let rpc_pool: Option<Arc<HttpRpcPool>> = match HttpRpcPool::from_env(chain_id).await {
        Ok(Some(pool)) => {
            let pool = Arc::new(pool);
            // Health loop runs until process exit (tokio detaches on drop).
            let _health_task = pool.spawn_health_loop();
            // Log the initial best-pick at boot so the operator can confirm
            // the pool is healthy before the first real submission.
            match pool.pick() {
                Ok(entry) => {
                    info!(
                        event = "rpc_pool.primary_selected",
                        provider = %entry.name,
                        chain_id,
                        "relays-client primary RPC selected via pool"
                    );
                }
                Err(e) => {
                    warn!(event = "rpc_pool.no_healthy", error = %e);
                }
            }
            Some(pool)
        }
        Ok(None) => {
            warn!(
                event = "rpc_pool.absent",
                chain_id, "RPC_HTTP_{chain_id} not set; relays-client stays in 501 mode"
            );
            None
        }
        Err(e) => {
            warn!(
                event = "rpc_pool.invalid",
                chain_id,
                error = %e,
                "RPC_HTTP_{chain_id} value did not parse; relays-client stays in 501 mode"
            );
            None
        }
    };

    let nonce = rpc_pool.clone().map(|p| Arc::new(NonceManager::new(p)));

    // DB pool — required for reading the operator-owned relay catalog (migration
    // 013). If DB is down at boot we still start the service and expose /health
    // so the on-call can see the failure; the catalog is simply empty until DB
    // is back and the service is restarted. No-hardcode doctrine: we never
    // silently fall back to a baked-in relay URL.
    let db_pool_opt: Option<sqlx::postgres::PgPool> = match std::env::var("DATABASE_URL") {
        Ok(url) if !url.is_empty() => {
            // OMEGA-8/M3 P1-2: timeouts applied.
            match shared_rs::db_pool::options_with_timeouts(
                &shared_rs::db_pool::PoolConfig::from_env(4),
            )
            .connect(&url)
            .await
            {
                Ok(pool) => {
                    info!(event = "db.connected", "postgres pool up");
                    Some(pool)
                }
                Err(e) => {
                    warn!(
                        event = "db.connect_failed", error = %e,
                        "continuing without DB — relay catalog will be empty until restart"
                    );
                    None
                }
            }
        }
        _ => {
            warn!(
                event = "db.not_configured",
                "DATABASE_URL not set; relay catalog cannot be loaded, consumer will not spawn"
            );
            None
        }
    };

    // ── Build multi-relay backend pool (BE-06) ─────────────────────────────
    //
    // Backends are added only when their credentials are present in the
    // environment or DB. No relay = `multi_relay` is None → engine stays in
    // NotSubmitted mode (same behaviour as when `flashbots` was None before).
    //
    // Flashbots URL resolution order (no-hardcode doctrine):
    //   1. DB `relays` table (operator-owned catalog, migration 013).
    //   2. FLASHBOTS_RELAY_URL env override (break-glass during onboarding).
    //   3. Nothing → flashbots backend excluded.
    //
    // BloXRoute: BLOXROUTE_AUTH_HEADER env var.
    // Titan:     TITAN_BUILDER_URL + TITAN_AUTH_HEADER env vars.
    let relay_timeout = Duration::from_millis(cfg.execution.flashbots_submit_timeout_ms);

    // BE-05: hold a direct Arc<FlashbotsClient> for eth_callBundle re-simulation.
    // We cannot extract it from multi_relay.backends because those are stored as
    // Arc<dyn RelayBackend> (type-erased). We construct the Arc first, clone it
    // for flashbots_for_callbundle, then coerce to Arc<dyn RelayBackend> for the
    // backends vec.
    let mut flashbots_for_callbundle: Option<Arc<FlashbotsClient>> = None;

    let multi_relay: Option<Arc<MultiRelayClient>> = {
        let mut backends: Vec<std::sync::Arc<dyn multi_relay::RelayBackend>> = Vec::new();

        // ── Flashbots ────────────────────────────────────────────────────────
        {
            let mut fb_endpoint: Option<String> = None;
            let mut fb_source: &'static str = "unset";

            if let Some(pool) = db_pool_opt.as_ref() {
                match relay_catalog::load_enabled(pool, chain_id as i32).await {
                    Ok(catalog) => {
                        if let Some(fb) = relay_catalog::find_flashbots(&catalog, chain_id as i32) {
                            fb_endpoint = Some(fb.endpoint.clone());
                            fb_source = "db";
                        }
                        relay_catalog::warn_if_empty(&catalog, chain_id as i32);
                    }
                    Err(e) => {
                        warn!(
                            event = "relay_catalog.query_failed",
                            error = %e,
                            "could not load relay catalog from DB — will check env override only"
                        );
                    }
                }
            }

            if fb_endpoint.is_none() {
                if let Ok(url) = std::env::var("FLASHBOTS_RELAY_URL") {
                    if !url.is_empty() {
                        fb_endpoint = Some(url);
                        fb_source = "env";
                    }
                }
            }

            match fb_endpoint {
                Some(url) => {
                    info!(event = "flashbots.configured", url = %url, source = fb_source, "flashbots backend added");
                    // Construct as typed Arc first so we can hold a clone for
                    // eth_callBundle (BE-05) before type-erasing for the relay pool.
                    let fb = Arc::new(FlashbotsClient::new(url, relay_timeout));
                    flashbots_for_callbundle = Some(fb.clone());
                    backends.push(fb as Arc<dyn multi_relay::RelayBackend>);
                }
                None => {
                    warn!(
                        event = "flashbots.disabled",
                        reason = "no_endpoint",
                        "flashbots backend absent: no row in relays table for chain {chain_id} \
                         and no FLASHBOTS_RELAY_URL. Populate via POST /admin/relays (onboarding step 4).",
                    );
                }
            }
        }

        // ── BloXRoute ────────────────────────────────────────────────────────
        if let Some(blx) = BloXRouteClient::from_env() {
            info!(event = "bloxroute.configured", "bloxroute backend added");
            backends.push(Arc::new(blx));
        } else {
            info!(
                event = "bloxroute.skipped",
                reason = "BLOXROUTE_AUTH_HEADER not set"
            );
        }

        // ── Titan ────────────────────────────────────────────────────────────
        if let Some(titan) = TitanClient::from_env() {
            info!(event = "titan.configured", "titan backend added");
            backends.push(Arc::new(titan));
        } else {
            info!(
                event = "titan.skipped",
                reason = "TITAN_BUILDER_URL or TITAN_AUTH_HEADER not set"
            );
        }

        if backends.is_empty() {
            warn!(
                event = "multi_relay.no_backends",
                "no relay backends configured — relays-client stays in NotSubmitted mode"
            );
            None
        } else {
            let names: Vec<&str> = backends.iter().map(|b| b.name()).collect();
            info!(
                event = "multi_relay.ready",
                backends = ?names,
                timeout_ms = cfg.execution.flashbots_submit_timeout_ms,
                "multi-relay broadcast pool ready"
            );
            Some(Arc::new(MultiRelayClient {
                backends,
                timeout_ms: cfg.execution.flashbots_submit_timeout_ms,
            }))
        }
    };

    // Redis connection manager for the pre-execute checklist.
    // ConnectionManager is Arc-backed; each clone is a logical alias.
    let redis_mgr_for_engine: redis::aio::ConnectionManager = {
        let client = redis::Client::open(redis_url.clone())?;
        client.get_connection_manager().await?
    };

    let engine = Arc::new(SubmitEngine {
        signer: signer.clone(),
        rpc_pool: rpc_pool.clone(),
        nonce: nonce.clone(),
        multi_relay: multi_relay.clone(),
        flashbots_for_callbundle: flashbots_for_callbundle.clone(),
        kill_switch: killswitch.clone(),
        paper_mode: paper_mode.clone(),
        cfg: cfg.clone(),
        pg: db_pool_opt.clone(),
        redis: redis_mgr_for_engine.clone(),
    });

    let state = Arc::new(AppState {
        engine: engine.clone(),
        has_signer: signer.is_some() && rpc_pool.is_some(),
        env: cfg.system.env.clone(),
    });

    let port: u16 = std::env::var("RELAYS_PORT")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(3005);
    let exec_router = Router::new()
        .route("/execute", post(execute_handler))
        .with_state(state);
    let app = build_health_router(ServiceInfo::new(SERVICE, VERSION)).merge(exec_router);

    info!(
        event = "service.boot",
        service = SERVICE, env = %cfg.system.env, port,
        has_signer = signer.is_some(),
        has_multi_relay = multi_relay.is_some(),
        relay_backends = multi_relay.as_ref().map(|mr| mr.backend_names()).unwrap_or_else(|| "none".to_string()),
        paper_mode = cfg.execution.paper_mode,
        max_value_eth = cfg.execution.max_value_eth,
        "relays-client listening"
    );

    // Consumer spawns only when signer + rpc_pool + DB pool are all present.
    // We reuse the pool opened above for the relay catalog lookup, so we don't
    // double up connections.
    if let (Some(pg_pool), true, true) = (db_pool_opt.clone(), signer.is_some(), rpc_pool.is_some())
    {
        let redis_client = redis::Client::open(redis_url.clone())?;
        let redis_conn = redis_client.get_connection_manager().await?;
        let consumer = consumer::Consumer {
            redis: redis_conn,
            pool: pg_pool.clone(),
            engine: SubmitEngine {
                signer: signer.clone(),
                rpc_pool: rpc_pool.clone(),
                nonce,
                multi_relay,
                flashbots_for_callbundle,
                kill_switch: killswitch.clone(),
                paper_mode: paper_mode.clone(),
                cfg: cfg.clone(),
                pg: Some(pg_pool),
                redis: redis_mgr_for_engine,
            },
            consumer_name: std::env::var("HOSTNAME").unwrap_or_else(|_| "relay-1".into()),
        };
        tokio::spawn(async move {
            if let Err(e) = consumer.run().await {
                tracing::error!(event = "relays_consumer.fatal", error = %e);
            }
        });
        info!(event = "relays_consumer.spawned");
    } else {
        info!(
            event = "relays_consumer.skipped",
            has_signer = signer.is_some(),
            has_rpc_pool = rpc_pool.is_some(),
            has_db = db_pool_opt.is_some(),
            "consumer not spawned — prerequisites missing (service stays up, /execute 501)"
        );
    }

    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app)
        .with_graceful_shutdown(async {
            tokio::signal::ctrl_c().await.ok();
        })
        .await?;
    Ok(())
}
