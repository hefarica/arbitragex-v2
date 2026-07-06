//! Token-enricher Prometheus metrics.
//!
//! All counters/gauges are registered against a **local** `Registry` (not the
//! shared `REGISTRY` in `shared-rs`) so the token-enricher binary can expose
//! `/metrics` without pulling in the full shared service telemetry.
//!
//! Metrics exposed:
//!   - `arbx_enricher_tokens_resolved_total{chain_id, resolved_via}` — counter
//!   - `arbx_enricher_batches_total{source}` — counter (source: "consumer"|"reconciliation")
//!   - `arbx_enricher_rpc_calls_total{chain_id, status}` — counter (status: "ok"|"err")
//!   - `arbx_enricher_pending_unresolved` — gauge (current backlog from reconciliation)
//!   - `arbx_enricher_up` — gauge (1 = service started OK)
//!
//! R8 fail-honest: counters only increment on real events. No phantom ticks.

use once_cell::sync::Lazy;
use prometheus::{Encoder, IntCounterVec, IntGauge, IntGaugeVec, Registry, TextEncoder};
use std::{sync::Arc, time::Duration};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpListener,
    sync::Semaphore,
};
use tracing::{error, info, warn};

// ---------------------------------------------------------------------------
// Registry (process-local — not shared with shared-rs)
// ---------------------------------------------------------------------------

pub static REGISTRY: Lazy<Registry> = Lazy::new(Registry::new);

// ---------------------------------------------------------------------------
// Metrics
// ---------------------------------------------------------------------------

/// Tokens that completed the resolution pipeline, by chain and outcome.
pub static TOKENS_RESOLVED_TOTAL: Lazy<IntCounterVec> = Lazy::new(|| {
    let c = IntCounterVec::new(
        prometheus::opts!(
            "arbx_enricher_tokens_resolved_total",
            "Tokens resolved by the enricher pipeline, by chain and resolution path"
        ),
        &["chain_id", "resolved_via"],
    )
    .expect("metric definition");
    REGISTRY.register(Box::new(c.clone())).expect("register");
    c
});

/// Batches processed from each source (Redis consumer or reconciliation tick).
pub static BATCHES_TOTAL: Lazy<IntCounterVec> = Lazy::new(|| {
    let c = IntCounterVec::new(
        prometheus::opts!(
            "arbx_enricher_batches_total",
            "Batches processed by the enricher, by source (consumer|reconciliation)"
        ),
        &["source"],
    )
    .expect("metric definition");
    REGISTRY.register(Box::new(c.clone())).expect("register");
    c
});

/// RPC eth_call attempts (Multicall3), by chain and outcome.
pub static RPC_CALLS_TOTAL: Lazy<IntCounterVec> = Lazy::new(|| {
    let c = IntCounterVec::new(
        prometheus::opts!(
            "arbx_enricher_rpc_calls_total",
            "Multicall3 eth_call attempts by chain and outcome (ok|err)"
        ),
        &["chain_id", "status"],
    )
    .expect("metric definition");
    REGISTRY.register(Box::new(c.clone())).expect("register");
    c
});

/// Current count of tokens pending resolution (from latest reconciliation scan).
/// This is a gauge: it reflects the most recent batch size from reconciliation,
/// not a running total. Operator value: tracks whether the backlog is shrinking.
pub static PENDING_UNRESOLVED: Lazy<IntGaugeVec> = Lazy::new(|| {
    let g = IntGaugeVec::new(
        prometheus::opts!(
            "arbx_enricher_pending_unresolved",
            "Tokens pending resolution as of the last reconciliation tick"
        ),
        &["chain_id"],
    )
    .expect("metric definition");
    REGISTRY.register(Box::new(g.clone())).expect("register");
    g
});

/// Service-up gauge. Set to 1 after successful init, 0 on clean shutdown.
pub static ENRICHER_UP: Lazy<IntGauge> = Lazy::new(|| {
    let g = IntGauge::new("arbx_enricher_up", "1 when token-enricher is running")
        .expect("metric definition");
    REGISTRY.register(Box::new(g.clone())).expect("register");
    g
});

// ---------------------------------------------------------------------------
// Init: force Lazy initialisation so all metrics appear in /metrics immediately
// ---------------------------------------------------------------------------

/// Force-initialise all Lazy metrics so they appear in the output even before
/// any event has been recorded.  Must be called from `main` before the HTTP
/// listener starts.
pub fn init_metrics() {
    let _ = &*TOKENS_RESOLVED_TOTAL;
    let _ = &*BATCHES_TOTAL;
    let _ = &*RPC_CALLS_TOTAL;
    let _ = &*PENDING_UNRESOLVED;
    let _ = &*ENRICHER_UP;
    ENRICHER_UP.set(1);
}

// ---------------------------------------------------------------------------
// Minimal TCP /metrics server — no axum dep, no extra crate
// ---------------------------------------------------------------------------

/// Render the Prometheus text output into a byte buffer.
fn render_metrics() -> Vec<u8> {
    let encoder = TextEncoder::new();
    let families = REGISTRY.gather();
    let mut buf = Vec::new();
    if let Err(e) = encoder.encode(&families, &mut buf) {
        warn!(event = "metrics.encode_error", err = %e);
    }
    buf
}

/// Spawn a lightweight TCP server that responds to any HTTP request on
/// `0.0.0.0:<port>` with the Prometheus text format.
///
/// - Accepts connections up to a cap of 16 concurrent (F-8 semaphore guard).
/// - Applies a 5-second read timeout to prevent slow-read DoS (ISSUE-5).
/// - Reads enough bytes to drain the request line (we don't parse it).
/// - Writes a minimal HTTP/1.1 200 response with `Content-Type: text/plain`.
/// - Logs and continues on individual connection errors — never panics.
///
/// Security: the server only binds to the port provided via `ENRICHER_METRICS_PORT`
/// (default 9004).  No auth — Prometheus scraping is expected to be internal.
pub async fn serve_metrics(port: u16) {
    let addr = format!("0.0.0.0:{port}");
    let listener = match TcpListener::bind(&addr).await {
        Ok(l) => {
            info!(event = "metrics.listen", addr = %addr);
            l
        }
        Err(e) => {
            error!(event = "metrics.bind_failed", addr = %addr, err = %e);
            return;
        }
    };

    // F-8: cap concurrent connections at 16 to bound memory and fd usage.
    let limit = Arc::new(Semaphore::new(16));

    loop {
        match listener.accept().await {
            Ok((mut stream, peer)) => {
                // Try to acquire a permit non-blocking; drop the connection if saturated.
                let permit = match limit.clone().try_acquire_owned() {
                    Ok(p) => p,
                    Err(_) => {
                        warn!(event = "enricher.metrics_saturated", peer = %peer);
                        continue;
                    }
                };
                tokio::spawn(async move {
                    let _permit = permit; // released when task completes

                    // ISSUE-5: 5-second read timeout prevents slow-read DoS from
                    // holding a semaphore permit indefinitely.
                    let mut buf = [0u8; 256];
                    match tokio::time::timeout(Duration::from_secs(5), stream.read(&mut buf)).await
                    {
                        Ok(Ok(_)) => {}
                        Ok(Err(e)) => {
                            warn!(event = "metrics.read_err", peer = %peer, err = %e);
                            return;
                        }
                        Err(_elapsed) => {
                            warn!(event = "metrics.read_timeout", peer = %peer);
                            return;
                        }
                    }

                    let body = render_metrics();
                    let header = format!(
                        "HTTP/1.1 200 OK\r\nContent-Type: text/plain; version=0.0.4\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                        body.len()
                    );
                    if let Err(e) = stream.write_all(header.as_bytes()).await {
                        warn!(event = "metrics.write_header_err", peer = %peer, err = %e);
                        return;
                    }
                    if let Err(e) = stream.write_all(&body).await {
                        warn!(event = "metrics.write_body_err", peer = %peer, err = %e);
                    }
                });
            }
            Err(e) => {
                warn!(event = "metrics.accept_err", err = %e);
            }
        }
    }
}
