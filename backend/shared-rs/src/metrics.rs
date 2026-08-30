//! Prometheus metrics registry + canonical counters shared across services.

use axum::{body::Body, http::StatusCode, response::Response};
use once_cell::sync::Lazy;
use prometheus::{
    Encoder, HistogramVec, IntCounter, IntCounterVec, IntGauge, Registry, TextEncoder,
};

pub static REGISTRY: Lazy<Registry> = Lazy::new(Registry::new);

pub static HTTP_REQUESTS_TOTAL: Lazy<IntCounterVec> = Lazy::new(|| {
    let c = IntCounterVec::new(
        prometheus::opts!(
            "arbx_http_requests_total",
            "Total HTTP requests by service/method/path/status"
        ),
        &["service", "method", "path", "status"],
    )
    .expect("metric");
    REGISTRY.register(Box::new(c.clone())).expect("register");
    c
});

pub static HTTP_REQUEST_DURATION: Lazy<HistogramVec> = Lazy::new(|| {
    let h = HistogramVec::new(
        prometheus::histogram_opts!(
            "arbx_http_request_duration_seconds",
            "HTTP request duration in seconds",
            prometheus::exponential_buckets(0.001, 2.0, 12).unwrap()
        ),
        &["service", "method", "path"],
    )
    .expect("metric");
    REGISTRY.register(Box::new(h.clone())).expect("register");
    h
});

pub static OPPORTUNITIES_TOTAL: Lazy<IntCounterVec> = Lazy::new(|| {
    let c = IntCounterVec::new(
        prometheus::opts!(
            "arbx_opportunity_total",
            "Opportunities by chain/strategy/status"
        ),
        &["chain_id", "strategy_kind", "status"],
    )
    .expect("metric");
    REGISTRY.register(Box::new(c.clone())).expect("register");
    c
});

pub static SIMULATIONS_TOTAL: Lazy<IntCounterVec> = Lazy::new(|| {
    let c = IntCounterVec::new(
        prometheus::opts!(
            "arbx_simulation_total",
            "Simulations by simulator and pass/fail"
        ),
        &["simulator", "passed"],
    )
    .expect("metric");
    REGISTRY.register(Box::new(c.clone())).expect("register");
    c
});

pub static EXECUTIONS_TOTAL: Lazy<IntCounterVec> = Lazy::new(|| {
    let c = IntCounterVec::new(
        prometheus::opts!("arbx_execution_total", "Executions by relay/status/chain"),
        &["relay", "status", "chain_id"],
    )
    .expect("metric");
    REGISTRY.register(Box::new(c.clone())).expect("register");
    c
});

pub static KILLSWITCH_ENABLED: Lazy<IntGauge> = Lazy::new(|| {
    let g = IntGauge::new("arbx_killswitch_enabled", "1 when kill switch is ON").expect("metric");
    REGISTRY.register(Box::new(g.clone())).expect("register");
    g
});

pub static SERVICE_UP: Lazy<IntGauge> = Lazy::new(|| {
    let g = IntGauge::new("arbx_service_up", "Service up gauge (1)").expect("metric");
    REGISTRY.register(Box::new(g.clone())).expect("register");
    g
});

pub static SEARCHER_BACKEND_STATE: Lazy<prometheus::IntGaugeVec> = Lazy::new(|| {
    let g = prometheus::IntGaugeVec::new(
        prometheus::opts!(
            "arbx_searcher_backend_state",
            "One-hot searcher backend state per chain (1 if matches)"
        ),
        &["chain_id", "state"],
    )
    .expect("metric");
    REGISTRY.register(Box::new(g.clone())).expect("register");
    g
});

pub static SEARCHER_PENDING_TOTAL: Lazy<IntCounterVec> = Lazy::new(|| {
    let c = IntCounterVec::new(
        prometheus::opts!(
            "arbx_searcher_pending_total",
            "Pending tx hashes seen from WS"
        ),
        &["chain_id"],
    )
    .expect("metric");
    REGISTRY.register(Box::new(c.clone())).expect("register");
    c
});

pub static SEARCHER_DECODED_TOTAL: Lazy<IntCounterVec> = Lazy::new(|| {
    let c = IntCounterVec::new(
        prometheus::opts!(
            "arbx_searcher_decoded_total",
            "Pending txs successfully decoded"
        ),
        &["chain_id", "router"],
    )
    .expect("metric");
    REGISTRY.register(Box::new(c.clone())).expect("register");
    c
});

pub static SEARCHER_UNDECODED_TOTAL: Lazy<IntCounterVec> = Lazy::new(|| {
    let c = IntCounterVec::new(
        prometheus::opts!(
            "arbx_searcher_undecoded_total",
            "Pending txs not decoded, by reason"
        ),
        &["chain_id", "reason"],
    )
    .expect("metric");
    REGISTRY.register(Box::new(c.clone())).expect("register");
    c
});

pub static SEARCHER_DROPPED_TOTAL: Lazy<IntCounterVec> = Lazy::new(|| {
    let c = IntCounterVec::new(
        prometheus::opts!(
            "arbx_searcher_dropped_total",
            "Pending txs dropped before processing"
        ),
        &["chain_id", "reason"],
    )
    .expect("metric");
    REGISTRY.register(Box::new(c.clone())).expect("register");
    c
});

pub static SEARCHER_WS_RECONNECTS_TOTAL: Lazy<IntCounterVec> = Lazy::new(|| {
    let c = IntCounterVec::new(
        prometheus::opts!(
            "arbx_searcher_ws_reconnects_total",
            "WebSocket reconnection attempts"
        ),
        &["chain_id"],
    )
    .expect("metric");
    REGISTRY.register(Box::new(c.clone())).expect("register");
    c
});

// ---- RPC failover pool metrics (G-RPC-1, doctrine: arbx-rpc-failover-discipline) ----

pub static RPC_PROVIDER_STATE: Lazy<prometheus::IntGaugeVec> = Lazy::new(|| {
    let g = prometheus::IntGaugeVec::new(
        prometheus::opts!(
            "arbx_rpc_provider_state",
            "RPC provider state: 0=Healthy 1=Degraded 2=Open"
        ),
        &["provider", "kind"],
    )
    .expect("metric");
    REGISTRY.register(Box::new(g.clone())).expect("register");
    g
});

pub static RPC_PROVIDER_LATENCY_MS: Lazy<HistogramVec> = Lazy::new(|| {
    let h = HistogramVec::new(
        prometheus::histogram_opts!(
            "arbx_rpc_provider_latency_ms",
            "RPC provider call latency in milliseconds (success path only)",
            vec![5.0, 10.0, 25.0, 50.0, 100.0, 200.0, 400.0, 800.0, 1600.0, 3200.0, 6400.0]
        ),
        &["provider", "kind"],
    )
    .expect("metric");
    REGISTRY.register(Box::new(h.clone())).expect("register");
    h
});

pub static RPC_PROVIDER_ERRORS_TOTAL: Lazy<IntCounterVec> = Lazy::new(|| {
    let c = IntCounterVec::new(
        prometheus::opts!(
            "arbx_rpc_provider_errors_total",
            "RPC provider errors by classified cause"
        ),
        &["provider", "kind", "cause"],
    )
    .expect("metric");
    REGISTRY.register(Box::new(c.clone())).expect("register");
    c
});

pub static RPC_PROVIDER_BLOCK_HEIGHT: Lazy<prometheus::IntGaugeVec> = Lazy::new(|| {
    let g = prometheus::IntGaugeVec::new(
        prometheus::opts!(
            "arbx_rpc_provider_block_height",
            "Latest block height observed per provider — drift detector input"
        ),
        &["provider", "kind"],
    )
    .expect("metric");
    REGISTRY.register(Box::new(g.clone())).expect("register");
    g
});

pub static RPC_POOL_FAILOVERS_TOTAL: Lazy<IntCounterVec> = Lazy::new(|| {
    let c = IntCounterVec::new(
        prometheus::opts!(
            "arbx_rpc_pool_failovers_total",
            "Times the pool fell back to a backup provider after a primary error"
        ),
        &["chain_id"],
    )
    .expect("metric");
    REGISTRY.register(Box::new(c.clone())).expect("register");
    c
});

pub static RPC_POOL_DRIFT_DETECTED_TOTAL: Lazy<IntCounterVec> = Lazy::new(|| {
    let c = IntCounterVec::new(
        prometheus::opts!(
            "arbx_rpc_pool_drift_detected_total",
            "Times a provider was demoted to Degraded for drifting behind tip"
        ),
        &["provider", "chain_id"],
    )
    .expect("metric");
    REGISTRY.register(Box::new(c.clone())).expect("register");
    c
});

// ---- Credential rotation counters (RunFullSyncCycle FASE 4 — svc_cred) ----
//
// `arbx_credential_rotation_total`    — every titular→fallback advance of the
//   rotation state for a credential CSV (rpc_http / rpc_ws arrays; first entry
//   is the titular, the rest are fallbacks in priority order).
// `arbx_credential_all_failed_total`  — times EVERY entry in a CSV failed
//   (wrap-around). The consumer must then declare itself degraded with reason
//   `all_credentials_failed` (RULE 00: no synthetic data, no last-good-value).

pub static CREDENTIAL_ROTATION_TOTAL: Lazy<IntCounterVec> = Lazy::new(|| {
    let c = IntCounterVec::new(
        prometheus::opts!(
            "arbx_credential_rotation_total",
            "Credential rotations titular→fallback by provider (svc_cred FASE 4)"
        ),
        &["provider"],
    )
    .expect("metric");
    REGISTRY.register(Box::new(c.clone())).expect("register");
    c
});

pub static CREDENTIAL_ALL_FAILED_TOTAL: Lazy<IntCounterVec> = Lazy::new(|| {
    let c = IntCounterVec::new(
        prometheus::opts!(
            "arbx_credential_all_failed_total",
            "Times every entry in a credential CSV failed — consumer degrades (all_credentials_failed)"
        ),
        &["provider"],
    )
    .expect("metric");
    REGISTRY.register(Box::new(c.clone())).expect("register");
    c
});

// ---- Bundle inclusion counters (N7 — audit 2026-05-10) ----
//
// These counters back the BundleSnipedRateHigh alert in
// monitoring/alerts.rules.yml. Both are scoped by chain_id + relay so
// operators can identify which relay / chain is the sniping surface.
//
// arbx_bundle_included_total        — every on-chain inclusion (profitable or not)
// arbx_bundle_included_no_profit_total — inclusions where expected profit <= 0
//   (proxy for "sniped / front-run / spread already gone by the time we landed")
//   until recon writes realized profit back, expected_profit_usd is the best
//   available signal.

pub static BUNDLE_INCLUDED_TOTAL: Lazy<IntCounterVec> = Lazy::new(|| {
    let c = IntCounterVec::new(
        prometheus::opts!(
            "arbx_bundle_included_total",
            "Total bundles included on chain (regardless of profit outcome)"
        ),
        &["chain_id", "relay"],
    )
    .expect("metric");
    REGISTRY.register(Box::new(c.clone())).expect("register");
    c
});

pub static BUNDLE_INCLUDED_NO_PROFIT_TOTAL: Lazy<IntCounterVec> = Lazy::new(|| {
    let c = IntCounterVec::new(
        prometheus::opts!(
            "arbx_bundle_included_no_profit_total",
            "Bundles included on chain that netted zero or negative expected profit (sniped / front-run)"
        ),
        &["chain_id", "relay"],
    )
    .expect("metric");
    REGISTRY.register(Box::new(c.clone())).expect("register");
    c
});

pub static GAS_PRICE_TS_SECONDS: Lazy<prometheus::IntGauge> = Lazy::new(|| {
    let g = prometheus::IntGauge::new(
        "arbx_gas_price_ts_seconds",
        "Timestamp in seconds of the last updated gas price",
    )
    .expect("metric");
    REGISTRY.register(Box::new(g.clone())).expect("register");
    g
});

// ---- SIMWIRE-02: stream consumer PEL health (pending-entries-list) ----
//
// The sim-ctl consumer leaves entries UNACKED on transient infra errors; the
// entries live in the group's PEL until XAUTOCLAIM recovers them. These four
// metrics make that recovery observable:
//   arbx_sim_stream_pending_count      — entries currently in the PEL (XPENDING)
//   arbx_sim_stream_oldest_pending_ms  — idle time of the oldest PEL entry;
//                                        growth without claims = stuck entries
//   arbx_sim_stream_claimed_count      — entries reclaimed via XAUTOCLAIM
//   arbx_sim_stream_claim_failures     — XPENDING/XAUTOCLAIM command errors

pub static SIM_STREAM_PENDING_COUNT: Lazy<IntGauge> = Lazy::new(|| {
    let g = IntGauge::new(
        "arbx_sim_stream_pending_count",
        "Entries in the sim-ctl consumer group PEL (unacked, awaiting XAUTOCLAIM recovery)",
    )
    .expect("metric");
    REGISTRY.register(Box::new(g.clone())).expect("register");
    g
});

pub static SIM_STREAM_OLDEST_PENDING_MS: Lazy<IntGauge> = Lazy::new(|| {
    let g = IntGauge::new(
        "arbx_sim_stream_oldest_pending_ms",
        "Idle time in ms of the oldest pending (unacked) entry in the sim-ctl consumer group PEL",
    )
    .expect("metric");
    REGISTRY.register(Box::new(g.clone())).expect("register");
    g
});

pub static SIM_STREAM_CLAIMED_COUNT: Lazy<IntCounter> = Lazy::new(|| {
    let c = IntCounter::new(
        "arbx_sim_stream_claimed_count",
        "Entries reclaimed from the PEL via XAUTOCLAIM by the sim-ctl consumer",
    )
    .expect("metric");
    REGISTRY.register(Box::new(c.clone())).expect("register");
    c
});

pub static SIM_STREAM_CLAIM_FAILURES: Lazy<IntCounter> = Lazy::new(|| {
    let c = IntCounter::new(
        "arbx_sim_stream_claim_failures",
        "XPENDING/XAUTOCLAIM command failures during sim-ctl PEL observation/recovery",
    )
    .expect("metric");
    REGISTRY.register(Box::new(c.clone())).expect("register");
    c
});

/// SIMWIRE-02b: stale PEL entries dead-lettered because their payload can
/// never complete (opportunity row purged / malformed). Watch this alongside
/// `arbx_sim_stream_pending_count` during the staged activation: it should
/// spike ONCE when the pre-existing ghosts drain, then stay flat.
pub static SIM_STREAM_GHOST_ACKED: Lazy<IntCounter> = Lazy::new(|| {
    let c = IntCounter::new(
        "arbx_sim_stream_ghost_acked",
        "Stale PEL entries terminally ACKed as ghosts (opportunity_row_missing / malformed_payload)",
    )
    .expect("metric");
    REGISTRY.register(Box::new(c.clone())).expect("register");
    c
});

/// Record a bundle inclusion event.
///
/// `profitable` is `true` when the opportunity carried a positive
/// `net_expected_profit_usd` (or `expected_profit_usd` as fallback).
/// Until recon writes realized profit back to the opportunity record,
/// this is the best proxy available — false negatives (we thought it
/// was profitable but got sniped) will be resolved in Sprint 6 when
/// actual_profit_usd lands via trace reconciliation.
pub fn record_inclusion(chain_id: u64, relay: &str, profitable: bool) {
    let chain = chain_id.to_string();
    BUNDLE_INCLUDED_TOTAL
        .with_label_values(&[chain.as_str(), relay])
        .inc();
    if !profitable {
        BUNDLE_INCLUDED_NO_PROFIT_TOTAL
            .with_label_values(&[chain.as_str(), relay])
            .inc();
    }
}

pub fn init_metrics() {
    // Force Lazy init so all collectors are registered.
    let _ = &*HTTP_REQUESTS_TOTAL;
    let _ = &*HTTP_REQUEST_DURATION;
    let _ = &*OPPORTUNITIES_TOTAL;
    let _ = &*SIMULATIONS_TOTAL;
    let _ = &*EXECUTIONS_TOTAL;
    let _ = &*KILLSWITCH_ENABLED;
    let _ = &*SERVICE_UP;
    let _ = &*SEARCHER_BACKEND_STATE;
    let _ = &*SEARCHER_PENDING_TOTAL;
    let _ = &*SEARCHER_DECODED_TOTAL;
    let _ = &*SEARCHER_UNDECODED_TOTAL;
    let _ = &*SEARCHER_DROPPED_TOTAL;
    let _ = &*SEARCHER_WS_RECONNECTS_TOTAL;
    let _ = &*RPC_PROVIDER_STATE;
    let _ = &*RPC_PROVIDER_LATENCY_MS;
    let _ = &*RPC_PROVIDER_ERRORS_TOTAL;
    let _ = &*RPC_PROVIDER_BLOCK_HEIGHT;
    let _ = &*RPC_POOL_FAILOVERS_TOTAL;
    let _ = &*RPC_POOL_DRIFT_DETECTED_TOTAL;
    let _ = &*BUNDLE_INCLUDED_TOTAL;
    let _ = &*BUNDLE_INCLUDED_NO_PROFIT_TOTAL;
    let _ = &*GAS_PRICE_TS_SECONDS;
    let _ = &*CREDENTIAL_ROTATION_TOTAL;
    let _ = &*CREDENTIAL_ALL_FAILED_TOTAL;
    let _ = &*SIM_STREAM_PENDING_COUNT;
    let _ = &*SIM_STREAM_OLDEST_PENDING_MS;
    let _ = &*SIM_STREAM_CLAIMED_COUNT;
    let _ = &*SIM_STREAM_CLAIM_FAILURES;
    let _ = &*SIM_STREAM_GHOST_ACKED;
    SERVICE_UP.set(1);
}

pub async fn metrics_handler() -> Response<Body> {
    let encoder = TextEncoder::new();
    let metric_families = REGISTRY.gather();
    let mut buffer = Vec::new();
    if let Err(e) = encoder.encode(&metric_families, &mut buffer) {
        return Response::builder()
            .status(StatusCode::INTERNAL_SERVER_ERROR)
            .body(Body::from(format!("encode error: {e}")))
            .unwrap();
    }
    Response::builder()
        .status(StatusCode::OK)
        .header("content-type", encoder.format_type())
        .body(Body::from(buffer))
        .unwrap()
}
