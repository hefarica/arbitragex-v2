//! Config loader.
//!
//! Loads `configs/app.toml` (path configurable via env `ARBX_CONFIG_PATH`),
//! optionally validates against `configs/schemas/app.schema.json` when
//! `ARBX_VALIDATE_SCHEMA=1`, and fails fast on missing required env vars.

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("config file not found: {0}")]
    NotFound(PathBuf),
    #[error("config parse error: {0}")]
    Parse(String),
    #[error("schema validation error: {0}")]
    Schema(String),
    #[error("required env var missing: {0}")]
    MissingEnv(&'static str),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub system: SystemCfg,
    pub risk: RiskCfg,
    pub execution: ExecutionCfg,
    pub observability: ObsCfg,
    pub chains: Vec<ChainCfg>,
    pub relays: Vec<RelayCfg>,
    #[serde(default)]
    pub simulation: Option<SimulationCfg>,
    #[serde(default)]
    pub scoring: Option<ScoringCfg>,
    #[serde(default)]
    pub token_safety: Option<TokenSafetyCfg>,
    #[serde(default)]
    pub circuit_breakers: Vec<CircuitBreakerCfg>,
    #[serde(default)]
    pub recon: Option<ReconCfg>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReconCfg {
    pub pnl_source_default: String,
    pub variance_threshold_pct: f64,
    pub receipt_fetch_timeout_ms: u64,
    pub aggregator_interval_seconds: u64,
    pub strategy_score_window_hours: u64,
    pub anomaly_window_minutes: u64,
    pub anomaly_min_samples: u64,
    pub anomaly_revert_rate_pct: f64,
    pub auto_trip_on_high_revert_rate: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SimulationCfg {
    pub provider: String,
    pub fork_block: String,
    pub snapshot_pool_size: u32,
    pub sim_timeout_ms: u64,
    pub reset_interval_s: u64,
    pub gas_limit_safety_factor: f64,
    pub max_slippage_for_pass_pct: f64,
    pub probe_amount_fraction: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScoringCfg {
    pub min_accept_score: f64,
    pub weight_liquidity: f64,
    pub weight_depth: f64,
    pub weight_safety: f64,
    pub weight_slippage: f64,
    pub weight_gas: f64,
    pub weight_risk: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenSafetyCfg {
    pub provider: String,
    pub api_call_timeout_ms: u64,
    pub ttl_seconds_ok: u64,
    pub ttl_seconds_bad: u64,
    pub min_acceptable_score: i32,
    /// Required when provider="goplus". No-hardcode doctrine: the operator opts
    /// in by setting this in `configs/app.toml`. We never default in code.
    #[serde(default)]
    pub goplus_base_url: Option<String>,
    #[serde(default)]
    pub honeypot_is_base_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CircuitBreakerCfg {
    pub name: String,
    pub threshold: u32,
    pub window_ms: u64,
    pub cooldown_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemCfg {
    pub env: String,
    pub kill_switch_enabled_default: bool,
    #[serde(default)]
    pub service_name_prefix: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RiskCfg {
    pub max_revert_rate_pct: f64,
    pub max_execution_variance_pct: f64,
    pub min_token_safety_score: i32,
    pub simulation_required_for_new_routes: bool,
    pub max_gas_price_gwei: f64,
    pub max_slippage_pct: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionCfg {
    pub private_only: bool,
    #[serde(default = "default_paper_mode")]
    pub paper_mode: bool,
    pub max_parallel_executions: u32,
    pub retry_limit: u32,
    #[serde(default = "default_one")]
    pub target_block_offset: u64,
    #[serde(default = "default_five")]
    pub max_inclusion_wait_blocks: u32,
    #[serde(default = "default_max_value")]
    pub max_value_eth: f64,
    #[serde(default = "default_submit_timeout")]
    pub flashbots_submit_timeout_ms: u64,
    #[serde(default = "default_priority_fee_inc")]
    pub priority_fee_increment_pct: f64,
}
fn default_paper_mode() -> bool {
    true
}
fn default_one() -> u64 {
    1
}
fn default_five() -> u32 {
    5
}
fn default_max_value() -> f64 {
    1.0
}
fn default_submit_timeout() -> u64 {
    5000
}
fn default_priority_fee_inc() -> f64 {
    10.0
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ObsCfg {
    pub prometheus_enabled: bool,
    pub structured_logs: bool,
    #[serde(default = "default_log_level")]
    pub log_level: String,
}
fn default_log_level() -> String {
    "info".into()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChainCfg {
    pub chain_id: u64,
    pub name: String,
    pub enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RelayCfg {
    pub name: String,
    pub enabled: bool,
    pub chains: Vec<u64>,
    #[serde(default)]
    pub endpoint: Option<String>,
}

impl AppConfig {
    pub fn load() -> Result<Self, ConfigError> {
        let path = std::env::var("ARBX_CONFIG_PATH")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from("configs/app.toml"));
        Self::load_from(&path)
    }

    pub fn load_from(path: &Path) -> Result<Self, ConfigError> {
        if !path.exists() {
            return Err(ConfigError::NotFound(path.to_path_buf()));
        }
        let raw = std::fs::read_to_string(path)?;
        let cfg: AppConfig = toml::from_str(&raw).map_err(|e| ConfigError::Parse(e.to_string()))?;

        if std::env::var("ARBX_VALIDATE_SCHEMA").ok().as_deref() == Some("1") {
            cfg.validate_against_schema()?;
        }
        cfg.sanity_check()?;
        Ok(cfg)
    }

    fn validate_against_schema(&self) -> Result<(), ConfigError> {
        let schema_path = std::env::var("ARBX_CONFIG_SCHEMA")
            .unwrap_or_else(|_| "configs/schemas/app.schema.json".to_string());
        let schema_raw = std::fs::read_to_string(&schema_path)
            .map_err(|e| ConfigError::Schema(format!("cannot read schema {schema_path}: {e}")))?;
        let schema_json: serde_json::Value = serde_json::from_str(&schema_raw)
            .map_err(|e| ConfigError::Schema(format!("schema parse: {e}")))?;
        let compiled = jsonschema::JSONSchema::compile(&schema_json)
            .map_err(|e| ConfigError::Schema(format!("schema compile: {e}")))?;
        let instance = serde_json::to_value(self)
            .map_err(|e| ConfigError::Schema(format!("config serialize: {e}")))?;
        if let Err(errors) = compiled.validate(&instance) {
            let msgs: Vec<String> = errors.map(|e| e.to_string()).collect();
            return Err(ConfigError::Schema(msgs.join("; ")));
        }
        Ok(())
    }

    fn sanity_check(&self) -> Result<(), ConfigError> {
        if self.chains.iter().all(|c| !c.enabled) {
            return Err(ConfigError::Parse(
                "no chains enabled in configs/app.toml — at least one must be enabled".into(),
            ));
        }
        Ok(())
    }

    /// Returns the list of `chain_id` values currently enabled.
    ///
    /// Operator override: `ARBX_ENABLED_CHAINS` (CSV of u64 chain IDs) restricts
    /// the active set to a subset of the chains declared `enabled = true` in
    /// `configs/app.toml`. Only IDs present in BOTH the env var AND the TOML
    /// enabled list are activated; IDs in the env var that are absent from TOML
    /// are silently ignored (no-hardcode doctrine — TOML is the authoritative
    /// catalog, env var is a runtime filter, not an injection point).
    ///
    /// If `ARBX_ENABLED_CHAINS` is not set the full TOML-enabled list is returned.
    ///
    /// Example:
    ///   ARBX_ENABLED_CHAINS=1,42161   → enables Ethereum + Arbitrum only
    ///   ARBX_ENABLED_CHAINS=1         → Ethereum only (even if TOML enables more)
    ///   (unset)                       → all chains marked `enabled = true` in TOML
    pub fn enabled_chains(&self) -> Vec<u64> {
        let toml_enabled: Vec<u64> = self
            .chains
            .iter()
            .filter(|c| c.enabled)
            .map(|c| c.chain_id)
            .collect();

        match std::env::var("ARBX_ENABLED_CHAINS") {
            Ok(v) if !v.trim().is_empty() => {
                let env_ids: Vec<u64> = v
                    .split(',')
                    .filter_map(|s| s.trim().parse::<u64>().ok())
                    .collect();
                // Retain only IDs that are also enabled in TOML.
                toml_enabled
                    .into_iter()
                    .filter(|id| env_ids.contains(id))
                    .collect()
            }
            _ => toml_enabled,
        }
    }

    /// Returns relays enabled for a given chain_id.
    pub fn relays_for_chain(&self, chain_id: u64) -> Vec<&RelayCfg> {
        self.relays
            .iter()
            .filter(|r| r.enabled && r.chains.contains(&chain_id))
            .collect()
    }
}

/// Require a runtime env var; returns error with the var name when missing/empty.
pub fn require_env(name: &'static str) -> Result<String, ConfigError> {
    match std::env::var(name) {
        Ok(v) if !v.is_empty() => Ok(v),
        _ => Err(ConfigError::MissingEnv(name)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use std::sync::Mutex;

    // Serialize all tests that mutate ARBX_ENABLED_CHAINS — env::set_var/remove_var
    // are process-global and racy when tests run in parallel threads (default behaviour
    // for `cargo test`). A single module-level mutex ensures the env is stable for the
    // duration of each test body.
    static ENV_LOCK: Mutex<()> = Mutex::new(());

    fn minimal_toml() -> &'static str {
        r#"
[system]
env = "development"
kill_switch_enabled_default = true

[risk]
max_revert_rate_pct = 5.0
max_execution_variance_pct = 20.0
min_token_safety_score = 70
simulation_required_for_new_routes = true
max_gas_price_gwei = 200.0
max_slippage_pct = 1.5

[execution]
private_only = true
max_parallel_executions = 8
retry_limit = 2

[observability]
prometheus_enabled = true
structured_logs = true
log_level = "info"

[[chains]]
chain_id = 1
name = "ethereum"
enabled = true

[[relays]]
name = "flashbots"
enabled = true
chains = [1]
"#
    }

    fn multichain_toml() -> &'static str {
        r#"
[system]
env = "development"
kill_switch_enabled_default = true

[risk]
max_revert_rate_pct = 5.0
max_execution_variance_pct = 20.0
min_token_safety_score = 70
simulation_required_for_new_routes = true
max_gas_price_gwei = 200.0
max_slippage_pct = 1.5

[execution]
private_only = true
max_parallel_executions = 8
retry_limit = 2

[observability]
prometheus_enabled = true
structured_logs = true
log_level = "info"

[[chains]]
chain_id = 1
name = "ethereum"
enabled = true

[[chains]]
chain_id = 42161
name = "arbitrum"
enabled = true

[[chains]]
chain_id = 137
name = "polygon"
enabled = false

[[relays]]
name = "flashbots"
enabled = true
chains = [1]
"#
    }

    #[test]
    fn loads_minimal_config() {
        let _g = ENV_LOCK.lock().unwrap();
        // Guard: ensure env override is absent so TOML is authoritative.
        unsafe {
            std::env::remove_var("ARBX_ENABLED_CHAINS");
        }
        let tmp = tempfile::NamedTempFile::new().unwrap();
        let mut f = std::fs::File::create(tmp.path()).unwrap();
        f.write_all(minimal_toml().as_bytes()).unwrap();
        let cfg = AppConfig::load_from(tmp.path()).unwrap();
        assert_eq!(cfg.enabled_chains(), vec![1]);
        assert_eq!(cfg.relays_for_chain(1).len(), 1);
    }

    #[test]
    fn enabled_chains_env_override_filters_to_subset() {
        let _g = ENV_LOCK.lock().unwrap();
        // TOML enables chains 1 + 42161; env var keeps only 1.
        unsafe {
            std::env::set_var("ARBX_ENABLED_CHAINS", "1");
        }
        let tmp = tempfile::NamedTempFile::new().unwrap();
        std::fs::write(tmp.path(), multichain_toml()).unwrap();
        let cfg = AppConfig::load_from(tmp.path()).unwrap();
        let chains = cfg.enabled_chains();
        unsafe {
            std::env::remove_var("ARBX_ENABLED_CHAINS");
        }
        assert_eq!(
            chains,
            vec![1],
            "env override must restrict to requested subset"
        );
    }

    #[test]
    fn enabled_chains_env_override_multiple() {
        let _g = ENV_LOCK.lock().unwrap();
        // TOML enables chains 1 + 42161; env var requests both.
        unsafe {
            std::env::set_var("ARBX_ENABLED_CHAINS", "1,42161");
        }
        let tmp = tempfile::NamedTempFile::new().unwrap();
        std::fs::write(tmp.path(), multichain_toml()).unwrap();
        let cfg = AppConfig::load_from(tmp.path()).unwrap();
        let chains = cfg.enabled_chains();
        unsafe {
            std::env::remove_var("ARBX_ENABLED_CHAINS");
        }
        assert!(chains.contains(&1));
        assert!(chains.contains(&42161));
        assert_eq!(chains.len(), 2);
    }

    #[test]
    fn enabled_chains_env_override_ignores_unknown_chain() {
        let _g = ENV_LOCK.lock().unwrap();
        // Chain 999 is not in TOML — must be silently dropped (no-hardcode doctrine).
        unsafe {
            std::env::set_var("ARBX_ENABLED_CHAINS", "1,999");
        }
        let tmp = tempfile::NamedTempFile::new().unwrap();
        std::fs::write(tmp.path(), multichain_toml()).unwrap();
        let cfg = AppConfig::load_from(tmp.path()).unwrap();
        let chains = cfg.enabled_chains();
        unsafe {
            std::env::remove_var("ARBX_ENABLED_CHAINS");
        }
        assert_eq!(chains, vec![1], "unknown chain_id 999 must be dropped");
    }

    #[test]
    fn enabled_chains_env_override_ignores_disabled_chain() {
        let _g = ENV_LOCK.lock().unwrap();
        // Chain 137 is in TOML but enabled=false — env var cannot re-enable it.
        unsafe {
            std::env::set_var("ARBX_ENABLED_CHAINS", "1,137");
        }
        let tmp = tempfile::NamedTempFile::new().unwrap();
        std::fs::write(tmp.path(), multichain_toml()).unwrap();
        let cfg = AppConfig::load_from(tmp.path()).unwrap();
        let chains = cfg.enabled_chains();
        unsafe {
            std::env::remove_var("ARBX_ENABLED_CHAINS");
        }
        assert!(
            !chains.contains(&137),
            "env var must not re-enable a TOML-disabled chain"
        );
    }

    #[test]
    fn fails_when_no_chains_enabled() {
        let _g = ENV_LOCK.lock().unwrap();
        unsafe {
            std::env::remove_var("ARBX_ENABLED_CHAINS");
        }
        let mut bad = minimal_toml().to_string();
        bad = bad.replace(
            "enabled = true\n\n[[relays]]",
            "enabled = false\n\n[[relays]]",
        );
        let tmp = tempfile::NamedTempFile::new().unwrap();
        std::fs::write(tmp.path(), bad).unwrap();
        let err = AppConfig::load_from(tmp.path()).unwrap_err();
        assert!(matches!(err, ConfigError::Parse(_)));
    }

    #[test]
    fn require_env_missing_is_error() {
        std::env::remove_var("ARBX_TEST_UNSET");
        assert!(matches!(
            require_env("ARBX_TEST_UNSET"),
            Err(ConfigError::MissingEnv(_))
        ));
    }
}
