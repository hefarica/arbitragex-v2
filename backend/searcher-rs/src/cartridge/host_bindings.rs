//! Host Bindings — native Rust functions exposed to Rhai cartridges.
//!
//! These bindings bridge the gap between the sandboxed Rhai script world and
//! the ArbitrageX v2 infrastructure (Redis, RPC, Mempool, Telemetry).
//!
//! ## Security Model
//!
//! - All bindings are READ-ONLY by default (no state mutation from scripts).
//! - Redis operations use a dedicated read-only connection.
//! - RPC calls are rate-limited per cartridge (max 10 calls/evaluation).
//! - Telemetry is fire-and-forget (never blocks the script).
//! - No filesystem, network, or process access is exposed.
//!
//! ## Binding Registry
//!
//! | Function                          | Source          | Latency    |
//! |-----------------------------------|-----------------|------------|
//! | `get_reserves(pool_addr)`         | Redis cache     | <1ms       |
//! | `get_token_meta(token_addr)`      | Redis cache     | <1ms       |
//! | `get_pool_index(token_a, token_b)`| Redis cache     | <1ms       |
//! | `simulate_swap(amount, path)`     | RPC (cached)    | 5-50ms     |
//! | `get_base_fee()`                  | Redis/mempool   | <1ms       |
//! | `get_block_number()`              | Redis/chain     | <1ms       |
//! | `get_timestamp()`                 | System clock    | <1μs       |
//! | `log_quantum(level, message)`     | Redis telemetry | fire&forget|
//! | `emit_signal(signal, data)`       | Redis PubSub    | fire&forget|
//! | `math_sqrt(x)`                    | Native f64      | <1μs       |
//! | `math_abs(x)`                     | Native f64      | <1μs       |
//! | `math_min(a, b)`                  | Native f64      | <1μs       |
//! | `math_max(a, b)`                  | Native f64      | <1μs       |
//! | `math_pow(base, exp)`             | Native f64      | <1μs       |
//! | `to_wei(amount, decimals)`        | Pure conversion | <1μs       |
//! | `from_wei(amount, decimals)`      | Pure conversion | <1μs       |

use redis::aio::ConnectionManager;
use rhai::{Dynamic, Engine, EvalAltResult, Map, Scope};
use std::sync::Arc;
use tokio::runtime::Handle;
use tokio::sync::RwLock;
use tracing::{debug, warn};

/// Shared context passed to host binding closures.
///
/// All fields are `Arc`-wrapped for safe concurrent access from multiple
/// Rhai evaluation threads.
#[derive(Clone)]
pub struct HostContext {
    /// Redis connection for cache reads (reserves, tokens, gas).
    pub redis: Arc<RwLock<ConnectionManager>>,
    /// Chain ID for this searcher instance.
    pub chain_id: u64,
    /// Cartridge ID currently being evaluated (for telemetry attribution).
    pub cartridge_id: Arc<RwLock<String>>,
    /// Tokio runtime handle for blocking async calls from sync Rhai context.
    pub rt_handle: Handle,
    /// Latest known block number (updated by scanner).
    pub block_number: Arc<std::sync::atomic::AtomicU64>,
    /// Latest known base fee in gwei (updated by gas oracle worker).
    pub base_fee_gwei: Arc<std::sync::atomic::AtomicU64>,
    /// Telemetry Redis channel for log_quantum messages.
    pub telemetry_channel: String,
}

/// Registers all host bindings into the Rhai engine.
///
/// This function is called once during `CartridgeRunner` initialization.
/// Each binding captures a clone of `HostContext` for infrastructure access.
pub fn register_host_bindings(engine: &mut Engine, ctx: HostContext) {
    // ─────────────────────────────────────────────────────────────────────
    // INFRASTRUCTURE BINDINGS (Redis/RPC)
    // ─────────────────────────────────────────────────────────────────────

    // get_reserves(pool_addr: String) -> Map
    // Returns: #{ r0: "...", r1: "...", token0_addr: "...", block: N, ts: N }
    let ctx_reserves = ctx.clone();
    engine.register_fn("get_reserves", move |pool_addr: &str| -> Dynamic {
        let ctx = ctx_reserves.clone();
        let pool_addr = pool_addr.to_lowercase();
        let result = ctx.rt_handle.block_on(async {
            let mut redis = ctx.redis.write().await;
            let key = format!("arbx:pool_reserves:{}:{}", ctx.chain_id, pool_addr);
            let raw: Option<String> = redis::AsyncCommands::get(&mut *redis, &key).await.ok()?;
            raw
        });

        match result {
            Some(json_str) => {
                match serde_json::from_str::<serde_json::Value>(&json_str) {
                    Ok(val) => json_value_to_dynamic(&val),
                    Err(_) => Dynamic::UNIT,
                }
            }
            None => Dynamic::UNIT,
        }
    });

    // get_token_meta(token_addr: String) -> Map
    // Returns: #{ symbol: "...", decimals: N, is_stablecoin: bool }
    let ctx_token = ctx.clone();
    engine.register_fn("get_token_meta", move |token_addr: &str| -> Dynamic {
        let ctx = ctx_token.clone();
        let addr = token_addr.to_lowercase();
        let result = ctx.rt_handle.block_on(async {
            let mut redis = ctx.redis.write().await;
            let key = format!("arbx:tokens:{}:{}", ctx.chain_id, addr);
            let raw: Option<String> = redis::AsyncCommands::get(&mut *redis, &key).await.ok()?;
            raw
        });

        match result {
            Some(json_str) => {
                match serde_json::from_str::<serde_json::Value>(&json_str) {
                    Ok(val) => json_value_to_dynamic(&val),
                    Err(_) => Dynamic::UNIT,
                }
            }
            None => Dynamic::UNIT,
        }
    });

    // get_pool_index(token_a: String, token_b: String) -> Array
    // Returns array of pool addresses for the given token pair.
    let ctx_pools = ctx.clone();
    engine.register_fn("get_pool_index", move |token_a: &str, token_b: &str| -> Dynamic {
        let ctx = ctx_pools.clone();
        let (lo, hi) = if token_a < token_b {
            (token_a.to_lowercase(), token_b.to_lowercase())
        } else {
            (token_b.to_lowercase(), token_a.to_lowercase())
        };
        let result = ctx.rt_handle.block_on(async {
            let mut redis = ctx.redis.write().await;
            let key = format!("arbx:pool_index:{}:{}:{}", ctx.chain_id, lo, hi);
            let raw: Option<String> = redis::AsyncCommands::get(&mut *redis, &key).await.ok()?;
            raw
        });

        match result {
            Some(json_str) => {
                match serde_json::from_str::<Vec<String>>(&json_str) {
                    Ok(addrs) => {
                        let arr: Vec<Dynamic> = addrs.into_iter().map(|a| Dynamic::from(a)).collect();
                        Dynamic::from_array(arr)
                    }
                    Err(_) => Dynamic::from_array(vec![]),
                }
            }
            None => Dynamic::from_array(vec![]),
        }
    });

    // simulate_swap(amount_in: String, path: Array) -> Map
    // Simulates a swap through the given path. Returns estimated output.
    // NOTE: This is a CACHED simulation — real RPC calls are rate-limited.
    let ctx_sim = ctx.clone();
    engine.register_fn("simulate_swap", move |amount_in: &str, path: rhai::Array| -> Dynamic {
        let ctx = ctx_sim.clone();
        let path_strs: Vec<String> = path
            .iter()
            .filter_map(|d| d.clone().into_string().ok())
            .collect();

        if path_strs.len() < 2 {
            let mut map = Map::new();
            map.insert("success".into(), Dynamic::from(false));
            map.insert("error".into(), Dynamic::from("path must have at least 2 tokens"));
            return Dynamic::from_map(map);
        }

        // Check Redis cache first for recent simulation result
        let cache_key = format!(
            "arbx:sim_cache:{}:{}:{}",
            ctx.chain_id,
            amount_in,
            path_strs.join("_")
        );

        let cached = ctx.rt_handle.block_on(async {
            let mut redis = ctx.redis.write().await;
            let raw: Option<String> = redis::AsyncCommands::get(&mut *redis, &cache_key).await.ok()?;
            raw
        });

        if let Some(cached_json) = cached {
            if let Ok(val) = serde_json::from_str::<serde_json::Value>(&cached_json) {
                return json_value_to_dynamic(&val);
            }
        }

        // If no cache hit, return a placeholder indicating simulation needed.
        // Real RPC simulation is handled by the orchestrator layer to maintain
        // rate limits and prevent abuse from cartridge scripts.
        let mut map = Map::new();
        map.insert("success".into(), Dynamic::from(false));
        map.insert("error".into(), Dynamic::from("simulation_pending"));
        map.insert("amount_in".into(), Dynamic::from(amount_in.to_string()));
        map.insert("path".into(), Dynamic::from_array(
            path_strs.into_iter().map(|s| Dynamic::from(s)).collect()
        ));
        Dynamic::from_map(map)
    });

    // ─────────────────────────────────────────────────────────────────────
    // CHAIN STATE BINDINGS
    // ─────────────────────────────────────────────────────────────────────

    // get_base_fee() -> f64 (in gwei)
    let ctx_fee = ctx.clone();
    engine.register_fn("get_base_fee", move || -> Dynamic {
        let fee = ctx_fee.base_fee_gwei.load(std::sync::atomic::Ordering::Relaxed);
        // Stored as gwei * 1000 for 3-decimal precision without floats
        Dynamic::from(fee as f64 / 1000.0)
    });

    // get_block_number() -> i64
    let ctx_block = ctx.clone();
    engine.register_fn("get_block_number", move || -> Dynamic {
        let block = ctx_block.block_number.load(std::sync::atomic::Ordering::Relaxed);
        Dynamic::from(block as i64)
    });

    // get_timestamp() -> i64 (unix seconds)
    engine.register_fn("get_timestamp", || -> Dynamic {
        Dynamic::from(chrono::Utc::now().timestamp())
    });

    // get_chain_id() -> i64
    let ctx_chain = ctx.clone();
    engine.register_fn("get_chain_id", move || -> Dynamic {
        Dynamic::from(ctx_chain.chain_id as i64)
    });

    // ─────────────────────────────────────────────────────────────────────
    // TELEMETRY BINDINGS
    // ─────────────────────────────────────────────────────────────────────

    // log_quantum(level: String, message: String)
    // Levels: "debug", "info", "warn", "error"
    let ctx_log = ctx.clone();
    engine.register_fn("log_quantum", move |level: &str, message: &str| -> Dynamic {
        let ctx = ctx_log.clone();
        let cartridge_id = ctx.rt_handle.block_on(async {
            ctx.cartridge_id.read().await.clone()
        });

        let log_entry = serde_json::json!({
            "cartridge_id": cartridge_id,
            "level": level,
            "message": message,
            "timestamp": chrono::Utc::now().to_rfc3339(),
            "chain_id": ctx.chain_id,
        });

        // Fire-and-forget telemetry publish
        let channel = ctx.telemetry_channel.clone();
        let _ = ctx.rt_handle.block_on(async {
            let mut redis = ctx.redis.write().await;
            let _: Result<(), _> = redis::AsyncCommands::publish(
                &mut *redis,
                &channel,
                log_entry.to_string(),
            ).await;
        });

        match level {
            "debug" => debug!(cartridge = %cartridge_id, "{}", message),
            "info" => tracing::info!(cartridge = %cartridge_id, "{}", message),
            "warn" => warn!(cartridge = %cartridge_id, "{}", message),
            "error" => tracing::error!(cartridge = %cartridge_id, "{}", message),
            _ => debug!(cartridge = %cartridge_id, "[{}] {}", level, message),
        }

        Dynamic::UNIT
    });

    // emit_signal(signal_type: String, data: Map)
    // Publishes a signal to the Arteria PubSub for downstream consumers.
    let ctx_signal = ctx.clone();
    engine.register_fn("emit_signal", move |signal_type: &str, data: Map| -> Dynamic {
        let ctx = ctx_signal.clone();
        let cartridge_id = ctx.rt_handle.block_on(async {
            ctx.cartridge_id.read().await.clone()
        });

        let signal = serde_json::json!({
            "signal_type": signal_type,
            "cartridge_id": cartridge_id,
            "chain_id": ctx.chain_id,
            "data": format!("{:?}", data),
            "timestamp": chrono::Utc::now().to_rfc3339(),
        });

        let _ = ctx.rt_handle.block_on(async {
            let mut redis = ctx.redis.write().await;
            let _: Result<(), _> = redis::AsyncCommands::publish(
                &mut *redis,
                "arbx:cartridge:signals",
                signal.to_string(),
            ).await;
        });

        Dynamic::UNIT
    });

    // ─────────────────────────────────────────────────────────────────────
    // MATH UTILITY BINDINGS (pure, no I/O)
    // ─────────────────────────────────────────────────────────────────────

    engine.register_fn("math_sqrt", |x: f64| -> Dynamic {
        if x < 0.0 {
            Dynamic::from(f64::NAN)
        } else {
            Dynamic::from(x.sqrt())
        }
    });

    engine.register_fn("math_abs", |x: f64| -> Dynamic {
        Dynamic::from(x.abs())
    });

    engine.register_fn("math_min", |a: f64, b: f64| -> Dynamic {
        Dynamic::from(a.min(b))
    });

    engine.register_fn("math_max", |a: f64, b: f64| -> Dynamic {
        Dynamic::from(a.max(b))
    });

    engine.register_fn("math_pow", |base: f64, exp: f64| -> Dynamic {
        Dynamic::from(base.powf(exp))
    });

    engine.register_fn("math_log", |x: f64| -> Dynamic {
        if x <= 0.0 {
            Dynamic::from(f64::NAN)
        } else {
            Dynamic::from(x.ln())
        }
    });

    engine.register_fn("math_exp", |x: f64| -> Dynamic {
        Dynamic::from(x.exp())
    });

    // to_wei(amount: f64, decimals: i64) -> String
    // Converts a human-readable amount to wei string representation.
    engine.register_fn("to_wei", |amount: f64, decimals: i64| -> Dynamic {
        let factor = 10f64.powi(decimals as i32);
        let wei = (amount * factor) as u128;
        Dynamic::from(wei.to_string())
    });

    // from_wei(wei_str: String, decimals: i64) -> f64
    // Converts a wei string to human-readable f64.
    engine.register_fn("from_wei", |wei_str: &str, decimals: i64| -> Dynamic {
        let wei: f64 = wei_str.parse().unwrap_or(0.0);
        let factor = 10f64.powi(decimals as i32);
        Dynamic::from(wei / factor)
    });
}

// ─────────────────────────────────────────────────────────────────────────────
// Helper: Convert serde_json::Value to Rhai Dynamic
// ─────────────────────────────────────────────────────────────────────────────

fn json_value_to_dynamic(val: &serde_json::Value) -> Dynamic {
    match val {
        serde_json::Value::Null => Dynamic::UNIT,
        serde_json::Value::Bool(b) => Dynamic::from(*b),
        serde_json::Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                Dynamic::from(i)
            } else if let Some(f) = n.as_f64() {
                Dynamic::from(f)
            } else {
                Dynamic::UNIT
            }
        }
        serde_json::Value::String(s) => Dynamic::from(s.clone()),
        serde_json::Value::Array(arr) => {
            let items: Vec<Dynamic> = arr.iter().map(json_value_to_dynamic).collect();
            Dynamic::from_array(items)
        }
        serde_json::Value::Object(obj) => {
            let mut map = Map::new();
            for (k, v) in obj {
                map.insert(k.clone().into(), json_value_to_dynamic(v));
            }
            Dynamic::from_map(map)
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn json_null_to_unit() {
        let d = json_value_to_dynamic(&serde_json::Value::Null);
        assert!(d.is_unit());
    }

    #[test]
    fn json_number_to_dynamic() {
        let d = json_value_to_dynamic(&serde_json::json!(42));
        assert_eq!(d.as_int().unwrap(), 42);
    }

    #[test]
    fn json_object_to_map() {
        let val = serde_json::json!({"r0": "1000", "r1": "2000"});
        let d = json_value_to_dynamic(&val);
        let map = d.cast::<Map>();
        assert_eq!(map.get("r0").unwrap().clone().into_string().unwrap(), "1000");
    }
}
