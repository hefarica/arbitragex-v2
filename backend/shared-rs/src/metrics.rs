//! Prometheus metrics registry + canonical counters shared across services.

use axum::{body::Body, http::StatusCode, response::Response};
use once_cell::sync::Lazy;
use prometheus::{Encoder, HistogramVec, IntCounterVec, IntGauge, Registry, TextEncoder};

pub static REGISTRY: Lazy<Registry> = Lazy::new(Registry::new);

pub static HTTP_REQUESTS_TOTAL: Lazy<IntCounterVec> = Lazy::new(|| {
    let c = IntCounterVec::new(
        prometheus::opts!("arbx_http_requests_total", "Total HTTP requests by service/method/path/status"),
        &["service", "method", "path", "status"],
    ).expect("metric");
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
    ).expect("metric");
    REGISTRY.register(Box::new(h.clone())).expect("register");
    h
});

pub static OPPORTUNITIES_TOTAL: Lazy<IntCounterVec> = Lazy::new(|| {
    let c = IntCounterVec::new(
        prometheus::opts!("arbx_opportunity_total", "Opportunities by chain/strategy/status"),
        &["chain_id", "strategy_kind", "status"],
    ).expect("metric");
    REGISTRY.register(Box::new(c.clone())).expect("register");
    c
});

pub static SIMULATIONS_TOTAL: Lazy<IntCounterVec> = Lazy::new(|| {
    let c = IntCounterVec::new(
        prometheus::opts!("arbx_simulation_total", "Simulations by simulator and pass/fail"),
        &["simulator", "passed"],
    ).expect("metric");
    REGISTRY.register(Box::new(c.clone())).expect("register");
    c
});

pub static EXECUTIONS_TOTAL: Lazy<IntCounterVec> = Lazy::new(|| {
    let c = IntCounterVec::new(
        prometheus::opts!("arbx_execution_total", "Executions by relay/status/chain"),
        &["relay", "status", "chain_id"],
    ).expect("metric");
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
        prometheus::opts!("arbx_searcher_backend_state",
            "One-hot searcher backend state per chain (1 if matches)"),
        &["chain_id", "state"],
    ).expect("metric");
    REGISTRY.register(Box::new(g.clone())).expect("register");
    g
});

pub static SEARCHER_PENDING_TOTAL: Lazy<IntCounterVec> = Lazy::new(|| {
    let c = IntCounterVec::new(
        prometheus::opts!("arbx_searcher_pending_total", "Pending tx hashes seen from WS"),
        &["chain_id"],
    ).expect("metric");
    REGISTRY.register(Box::new(c.clone())).expect("register");
    c
});

pub static SEARCHER_DECODED_TOTAL: Lazy<IntCounterVec> = Lazy::new(|| {
    let c = IntCounterVec::new(
        prometheus::opts!("arbx_searcher_decoded_total", "Pending txs successfully decoded"),
        &["chain_id", "router"],
    ).expect("metric");
    REGISTRY.register(Box::new(c.clone())).expect("register");
    c
});

pub static SEARCHER_UNDECODED_TOTAL: Lazy<IntCounterVec> = Lazy::new(|| {
    let c = IntCounterVec::new(
        prometheus::opts!("arbx_searcher_undecoded_total", "Pending txs not decoded, by reason"),
        &["chain_id", "reason"],
    ).expect("metric");
    REGISTRY.register(Box::new(c.clone())).expect("register");
    c
});

pub static SEARCHER_DROPPED_TOTAL: Lazy<IntCounterVec> = Lazy::new(|| {
    let c = IntCounterVec::new(
        prometheus::opts!("arbx_searcher_dropped_total", "Pending txs dropped before processing"),
        &["chain_id", "reason"],
    ).expect("metric");
    REGISTRY.register(Box::new(c.clone())).expect("register");
    c
});

pub static SEARCHER_WS_RECONNECTS_TOTAL: Lazy<IntCounterVec> = Lazy::new(|| {
    let c = IntCounterVec::new(
        prometheus::opts!("arbx_searcher_ws_reconnects_total", "WebSocket reconnection attempts"),
        &["chain_id"],
    ).expect("metric");
    REGISTRY.register(Box::new(c.clone())).expect("register");
    c
});

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
