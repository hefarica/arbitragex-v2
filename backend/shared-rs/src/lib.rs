//! shared-rs — ArbitrageX v2 common runtime for Rust services.
//!
//! Exposes:
//! - `config`   : TOML + env loader validated against JSON Schema (app.schema.json).
//! - `logging`  : tracing subscriber with JSON output + trace-id propagation.
//! - `metrics`  : Prometheus registry + canonical counters/histograms.
//! - `health`   : Axum `/health` + `/metrics` router.
//! - `killswitch`: Redis-backed client with 1s TTL cache + pub/sub subscription.
//! - `contracts`: canonical types mirroring configs/schemas/*.json.
//!
//! Used by: searcher-rs, sim-ctl, relays-client, recon.

pub mod chains;
pub mod config;
pub mod contracts;
pub mod health;
pub mod killswitch;
pub mod logging;
pub mod metrics;
pub mod rpc_failover;

pub use config::{AppConfig, ConfigError};
pub use health::{ServiceInfo, build_health_router};
pub use killswitch::{KillSwitchClient, KillSwitchError, KillSwitchState};
pub use logging::init_tracing;
pub use metrics::{init_metrics, metrics_handler};
pub use rpc_failover::{
    HttpRpcPool, PoolError, ProviderState, RpcPoolSnapshot, RpcProviderSnapshot, WsEndpoint,
    WsRpcPool,
};
