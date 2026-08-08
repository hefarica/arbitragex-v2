//! math-engine binary — serves the 31 topological operators over HTTP.
//!
//! The REST surface (operators list/toggle, compute, 264×31 matrix projection)
//! is implemented in `api.rs` behind the `api` feature; this binary only wires
//! tracing + a Tokio listener and mounts `api::create_router()`.
//!
//! Read-only / paper-shadow: the engine transforms a `MarketState` into
//! operator outputs. It never signs, never broadcasts, never touches capital.
//!
//! Env:
//!   MATH_ENGINE_PORT  — listen port (default 3006).
//!   RUST_LOG          — tracing filter (default `info`).

#![warn(clippy::unwrap_used, clippy::expect_used)]

use std::net::SocketAddr;
use tracing::{info, warn};

use math_engine::api;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    init_tracing();

    let port: u16 = std::env::var("MATH_ENGINE_PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(3006);
    let addr = SocketAddr::from(([0, 0, 0, 0], port));

    let app = api::create_router();

    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .map_err(|e| anyhow::anyhow!("failed to bind math-engine listener on {addr}: {e}"))?;
    info!(
        event = "service.boot",
        port, "math-engine listening (31 operators)"
    );
    axum::serve(listener, app)
        .await
        .map_err(|e| anyhow::anyhow!("math-engine server error: {e}"))?;
    Ok(())
}

/// Minimal tracing init (shared-rs is not a dependency of math-engine; keep
/// the binary self-contained). Honors RUST_LOG, defaults to `info`.
fn init_tracing() {
    use tracing_subscriber::{fmt, EnvFilter};
    let filter = std::env::var("RUST_LOG").unwrap_or_else(|_| "info".to_string());
    let env_filter = EnvFilter::try_new(&filter).unwrap_or_else(|_| {
        warn!(event = "logging.bad_filter", filter = %filter, "invalid RUST_LOG; falling back to info");
        EnvFilter::new("info")
    });
    let _ = fmt()
        .with_env_filter(env_filter)
        .with_target(true)
        .try_init();
}
