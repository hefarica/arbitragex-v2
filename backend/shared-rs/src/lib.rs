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

pub mod candidates;
pub mod chains;
pub mod config;
pub mod contracts;
pub mod cred_rotation;
pub mod db_pool;
pub mod health;
pub mod killswitch;
pub mod logging;
pub mod metrics;
pub mod paper_mode;
pub mod pre_execute_checklist;
pub mod price_oracle;
pub mod risk_ledger;
pub mod rpc_failover;
pub mod tokens;
pub mod trading_config;

pub use config::{AppConfig, ConfigError};
pub use cred_rotation::{RotationOutcome, RotationState};
pub use db_pool::{connect_pool, options_with_timeouts, PoolConfig};
pub use health::{build_health_router, ServiceInfo};
pub use killswitch::{KillSwitchClient, KillSwitchError, KillSwitchState};
pub use logging::init_tracing;
pub use metrics::{init_metrics, metrics_handler};
pub use rpc_failover::{
    HttpRpcPool, PoolError, ProviderState, RpcPoolSnapshot, RpcProviderSnapshot, WsEndpoint,
    WsRpcPool,
};
