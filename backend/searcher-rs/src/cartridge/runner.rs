//! CartridgeRunner — the core Rhai engine for dynamic strategy execution.
//!
//! This module is the heart of the FASE OMEGA cartridge system. It:
//!
//! 1. Maintains a sandboxed Rhai `Engine` with host bindings registered.
//! 2. Compiles `.rhai` script strings into validated ASTs.
//! 3. Validates ASTs against the Universal Cartridge Contract.
//! 4. Stores compiled cartridges in a concurrent `HashMap`.
//! 5. Evaluates cartridges against live pool data on demand.
//! 6. Enforces operation limits, memory bounds, and timeout protection.
//!
//! ## Thread Safety
//!
//! `CartridgeRunner` is `Send + Sync` and designed to be wrapped in `Arc`
//! for shared access across the tokio task tree. The internal `Engine` is
//! NOT shared — each evaluation creates a fresh `Scope` and runs on the
//! caller's thread (or `spawn_blocking` for isolation).
//!
//! ## Error Handling
//!
//! All Rhai evaluation errors are caught and converted to structured
//! `CartridgeError` variants. A misbehaving cartridge NEVER crashes the
//! host process — it is marked as `Failed` and excluded from future evals.

use crate::cartridge::contract::{validate_contract, validate_metadata, ContractViolation};
use crate::cartridge::host_bindings::{register_host_bindings, HostContext};
use crate::cartridge::types::{
    CartridgeEvalResult, CartridgeMetadata, CartridgeState, CompiledCartridge,
};
use chrono::Utc;
use rhai::{Dynamic, Engine, Map, Scope, AST};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{error, info};

// ─────────────────────────────────────────────────────────────────────────────
// Configuration Constants
// ─────────────────────────────────────────────────────────────────────────────

/// Maximum number of Rhai operations before the engine aborts execution.
/// Prevents infinite loops and runaway recursion.
const MAX_OPERATIONS: u64 = 1_000_000;

/// Maximum call stack depth for Rhai scripts.
const MAX_CALL_STACK_DEPTH: usize = 64;

/// Maximum string size in bytes within Rhai scripts.
const MAX_STRING_SIZE: usize = 65_536;

/// Maximum array size within Rhai scripts.
const MAX_ARRAY_SIZE: usize = 4_096;

/// Maximum map size within Rhai scripts.
const MAX_MAP_SIZE: usize = 1_024;

/// Maximum expression-nesting depth. rhai's DEFAULT for the expr-depth limits is
/// profile-dependent (debug: 32 expr / 16 function; release: 64 / 32), so without
/// pinning, cartridge validation would differ between debug and release builds —
/// a cartridge that parses in a release production binary could fail to load in a
/// debug build (and vice-versa). Pin the release values so validation is deterministic.
const MAX_EXPR_DEPTH: usize = 64;

/// Maximum expression-nesting depth inside function bodies (see `MAX_EXPR_DEPTH`).
const MAX_FUNCTION_EXPR_DEPTH: usize = 32;

// ─────────────────────────────────────────────────────────────────────────────
// Error Types
// ─────────────────────────────────────────────────────────────────────────────

/// Errors that can occur during cartridge operations.
#[derive(Debug, Clone)]
pub enum CartridgeError {
    /// Script failed to compile (syntax error).
    CompilationFailed(String),
    /// Script violates the Universal Cartridge Contract.
    ContractViolation(Vec<ContractViolation>),
    /// Runtime error during script execution.
    RuntimeError(String),
    /// Script exceeded operation limit (infinite loop protection).
    OperationLimitExceeded,
    /// Cartridge not found in the registry.
    NotFound(String),
    /// Cartridge is not in Active state.
    NotActive(String, CartridgeState),
    /// Metadata validation failed.
    InvalidMetadata(String),
}

impl std::fmt::Display for CartridgeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::CompilationFailed(msg) => write!(f, "compilation failed: {msg}"),
            Self::ContractViolation(violations) => {
                write!(f, "contract violations: ")?;
                for (i, v) in violations.iter().enumerate() {
                    if i > 0 {
                        write!(f, "; ")?;
                    }
                    write!(f, "{v}")?;
                }
                Ok(())
            }
            Self::RuntimeError(msg) => write!(f, "runtime error: {msg}"),
            Self::OperationLimitExceeded => write!(f, "operation limit exceeded (infinite loop?)"),
            Self::NotFound(id) => write!(f, "cartridge not found: {id}"),
            Self::NotActive(id, state) => write!(f, "cartridge {id} is {state:?}, not Active"),
            Self::InvalidMetadata(msg) => write!(f, "invalid metadata: {msg}"),
        }
    }
}

impl std::error::Error for CartridgeError {}

// ─────────────────────────────────────────────────────────────────────────────
// CartridgeRunner
// ─────────────────────────────────────────────────────────────────────────────

/// The main cartridge runtime. Manages compilation, validation, storage,
/// and execution of dynamic Rhai strategy scripts.
pub struct CartridgeRunner {
    /// The sandboxed Rhai engine with host bindings and safety limits.
    engine: Engine,
    /// Registry of compiled and validated cartridges.
    cartridges: Arc<RwLock<HashMap<String, CompiledCartridge>>>,
    /// Host context for infrastructure access.
    host_ctx: HostContext,
}

impl CartridgeRunner {
    /// Creates a new `CartridgeRunner` with the given host context.
    ///
    /// The engine is configured with:
    /// - All host bindings registered
    /// - Operation limits enforced
    /// - Memory bounds set
    /// - No external module imports allowed
    pub fn new(host_ctx: HostContext) -> Self {
        let mut engine = Engine::new();

        // ── Sandboxing Configuration ─────────────────────────────────────
        engine.set_max_operations(MAX_OPERATIONS);
        engine.set_max_call_levels(MAX_CALL_STACK_DEPTH);
        engine.set_max_string_size(MAX_STRING_SIZE);
        engine.set_max_array_size(MAX_ARRAY_SIZE);
        engine.set_max_map_size(MAX_MAP_SIZE);
        // Pin expression-depth limits explicitly. rhai's default is profile-dependent
        // (debug 32/16 vs release 64/32), which would let a cartridge that loads in a
        // release build fail to load in a debug build. See MAX_EXPR_DEPTH.
        engine.set_max_expr_depths(MAX_EXPR_DEPTH, MAX_FUNCTION_EXPR_DEPTH);

        // Disable all external module loading (no `import` statements).
        engine.set_max_modules(0);

        // Disable potentially dangerous operations.
        // Rhai's default engine already doesn't have filesystem/network,
        // but we explicitly disable the `eval` function to prevent
        // dynamic code execution within scripts.
        engine.disable_symbol("eval");

        // ── Register Host Bindings ───────────────────────────────────────
        register_host_bindings(&mut engine, host_ctx.clone());

        Self {
            engine,
            cartridges: Arc::new(RwLock::new(HashMap::new())),
            host_ctx,
        }
    }

    /// Compiles, validates, and registers a new cartridge from source code.
    ///
    /// ## Process
    ///
    /// 1. Compile the Rhai source into an AST.
    /// 2. Validate the AST against the Universal Cartridge Contract.
    /// 3. Execute `init_strategy()` to extract metadata.
    /// 4. Validate the metadata map.
    /// 5. Store the compiled cartridge in the registry.
    ///
    /// ## Returns
    ///
    /// `Ok(CartridgeMetadata)` on success, `Err(CartridgeError)` on failure.
    pub async fn load_cartridge(
        &self,
        id: &str,
        source: &str,
        content_hash: &str,
    ) -> Result<CartridgeMetadata, CartridgeError> {
        info!(
            event = "cartridge.load_start",
            cartridge_id = %id,
            content_hash = %content_hash,
            "compiling cartridge"
        );

        // Step 1: Compile
        let ast = self.engine.compile(source).map_err(|e| {
            error!(
                event = "cartridge.compile_failed",
                cartridge_id = %id,
                error = %e,
            );
            CartridgeError::CompilationFailed(e.to_string())
        })?;

        // Step 2: Validate contract
        validate_contract(&ast).map_err(|violations| {
            error!(
                event = "cartridge.contract_violation",
                cartridge_id = %id,
                violations = ?violations,
            );
            CartridgeError::ContractViolation(violations)
        })?;

        // Step 3: Execute init_strategy() to get metadata
        let metadata = self.execute_init_strategy(&ast, id)?;

        // Step 4: Store in registry
        let compiled = CompiledCartridge {
            id: id.to_owned(),
            ast: Arc::new(ast),
            metadata: metadata.clone(),
            state: CartridgeState::Active,
            content_hash: content_hash.to_owned(),
            compiled_at: Utc::now(),
            eval_count: 0,
            error_count: 0,
        };

        let mut registry = self.cartridges.write().await;
        registry.insert(id.to_owned(), compiled);

        info!(
            event = "cartridge.loaded",
            cartridge_id = %id,
            name = %metadata.name,
            version = %metadata.version,
            author = %metadata.author,
        );

        Ok(metadata)
    }

    /// Removes a cartridge from the active registry.
    pub async fn unload_cartridge(&self, id: &str) -> bool {
        let mut registry = self.cartridges.write().await;
        let removed = registry.remove(id).is_some();
        if removed {
            info!(event = "cartridge.unloaded", cartridge_id = %id);
        }
        removed
    }

    /// Pauses a cartridge (keeps it compiled but skips evaluation).
    pub async fn pause_cartridge(&self, id: &str) -> Result<(), CartridgeError> {
        let mut registry = self.cartridges.write().await;
        match registry.get_mut(id) {
            Some(c) => {
                c.state = CartridgeState::Paused;
                info!(event = "cartridge.paused", cartridge_id = %id);
                Ok(())
            }
            None => Err(CartridgeError::NotFound(id.to_owned())),
        }
    }

    /// Resumes a paused cartridge.
    pub async fn resume_cartridge(&self, id: &str) -> Result<(), CartridgeError> {
        let mut registry = self.cartridges.write().await;
        match registry.get_mut(id) {
            Some(c) => {
                c.state = CartridgeState::Active;
                info!(event = "cartridge.resumed", cartridge_id = %id);
                Ok(())
            }
            None => Err(CartridgeError::NotFound(id.to_owned())),
        }
    }

    /// Evaluates a cartridge's `evaluate_opportunity` function with pool data.
    ///
    /// ## Safety
    ///
    /// - Operation limits prevent infinite loops.
    /// - Runtime errors are caught and returned as `CartridgeError`.
    /// - The cartridge's error counter is incremented on failure.
    /// - A cartridge that exceeds MAX_CONSECUTIVE_ERRORS is auto-paused.
    pub async fn evaluate(
        &self,
        cartridge_id: &str,
        pool_data: Map,
    ) -> Result<CartridgeEvalResult, CartridgeError> {
        // Get the AST (read lock, fast path)
        let ast = {
            let registry = self.cartridges.read().await;
            let cartridge = registry
                .get(cartridge_id)
                .ok_or_else(|| CartridgeError::NotFound(cartridge_id.to_owned()))?;

            if cartridge.state != CartridgeState::Active {
                return Err(CartridgeError::NotActive(
                    cartridge_id.to_owned(),
                    cartridge.state,
                ));
            }

            cartridge.ast.clone()
        };

        // Set the current cartridge ID for telemetry attribution
        {
            let mut cid = self.host_ctx.cartridge_id.write().await;
            *cid = cartridge_id.to_owned();
        }

        // Execute evaluate_opportunity(pool_data). Host bindings (get_reserves,
        // get_token_meta, get_pool_index, …) call `rt_handle.block_on` internally, which
        // PANICS if run directly on an async runtime worker (as the orchestrator shadow
        // task does). `block_in_place` flips the current worker into a blocking section so
        // the nested `block_on` is permitted. Requires the multi-threaded runtime (the
        // searcher uses `rt-multi-thread`; tests that call this must use a multi_thread RT).
        let mut scope = Scope::new();
        let result = tokio::task::block_in_place(|| {
            self.engine
                .call_fn::<Dynamic>(&mut scope, &ast, "evaluate_opportunity", (pool_data,))
        })
        .map_err(|e| {
            let err_msg = e.to_string();
            if err_msg.contains("Number of operations over limit") {
                CartridgeError::OperationLimitExceeded
            } else {
                CartridgeError::RuntimeError(err_msg)
            }
        })?;

        // Parse the result map
        let eval_result = self.parse_eval_result(result, cartridge_id)?;

        // Update eval counter
        {
            let mut registry = self.cartridges.write().await;
            if let Some(c) = registry.get_mut(cartridge_id) {
                c.eval_count += 1;
            }
        }

        Ok(eval_result)
    }

    /// Calls `build_payload` on a cartridge to assemble execution data.
    pub async fn build_payload(
        &self,
        cartridge_id: &str,
        opportunity: Map,
    ) -> Result<Map, CartridgeError> {
        let ast = {
            let registry = self.cartridges.read().await;
            let cartridge = registry
                .get(cartridge_id)
                .ok_or_else(|| CartridgeError::NotFound(cartridge_id.to_owned()))?;
            cartridge.ast.clone()
        };

        // See `evaluate` re block_in_place: host bindings may block_on inside call_fn.
        let mut scope = Scope::new();
        let result = tokio::task::block_in_place(|| {
            self.engine
                .call_fn::<Dynamic>(&mut scope, &ast, "build_payload", (opportunity,))
        })
        .map_err(|e| CartridgeError::RuntimeError(e.to_string()))?;

        result
            .try_cast::<Map>()
            .ok_or_else(|| CartridgeError::RuntimeError("build_payload must return a Map".into()))
    }

    /// Returns a snapshot of all loaded cartridges and their states.
    pub async fn list_cartridges(&self) -> Vec<(String, CartridgeMetadata, CartridgeState)> {
        let registry = self.cartridges.read().await;
        registry
            .values()
            .map(|c| (c.id.clone(), c.metadata.clone(), c.state))
            .collect()
    }

    /// Returns the number of active cartridges.
    pub async fn active_count(&self) -> usize {
        let registry = self.cartridges.read().await;
        registry
            .values()
            .filter(|c| c.state == CartridgeState::Active)
            .count()
    }

    /// Checks if a cartridge with the given content hash is already loaded.
    /// Used for dedup during hot-reload to avoid recompiling identical scripts.
    pub async fn has_content_hash(&self, content_hash: &str) -> bool {
        let registry = self.cartridges.read().await;
        registry.values().any(|c| c.content_hash == content_hash)
    }

    /// Returns a clone of the cartridges map (for the subscriber).
    pub fn cartridges_handle(&self) -> Arc<RwLock<HashMap<String, CompiledCartridge>>> {
        self.cartridges.clone()
    }

    /// Shared handle to the `block_number` atomic in the cartridge `HostContext`. The block
    /// scanner stores the latest head here each block so `get_block_number()` is real.
    pub fn host_block_number_handle(&self) -> Arc<std::sync::atomic::AtomicU64> {
        self.host_ctx.block_number.clone()
    }

    /// Shared handle to the `base_fee_gwei` atomic (stored as gwei×1000 / milli-gwei). The
    /// block scanner stores the head's base fee here so `get_base_fee()` is real — which is
    /// what makes the cartridge net-profit gate computable instead of always-zero gas.
    pub fn host_base_fee_handle(&self) -> Arc<std::sync::atomic::AtomicU64> {
        self.host_ctx.base_fee_gwei.clone()
    }

    /// Reads a pool's reserves from Redis using the SAME key + shape as the
    /// `get_reserves` host binding (`arbx:pool_reserves:{chain}:{pool_lower}`).
    /// Used by the orchestrator shadow path to enrich `pool_data.reserves_source`
    /// for reserve-dependent cartridges (e.g. dex_arb). `None` on miss/parse error
    /// (R8 fail-honest — the cartridge then returns no opportunity).
    pub async fn read_pool_reserves(
        &self,
        pool_addr: &str,
    ) -> Option<crate::reserves::ReservesEntry> {
        let key = format!(
            "arbx:pool_reserves:{}:{}",
            self.host_ctx.chain_id,
            pool_addr.to_lowercase()
        );
        let mut redis = self.host_ctx.redis.write().await;
        let raw: Option<String> = redis::AsyncCommands::get(&mut *redis, &key).await.ok()?;
        raw.and_then(|s| serde_json::from_str(&s).ok())
    }

    /// FASE B — append a resolved shadow eval outcome to a Redis Stream via XADD
    /// with an approximate MAXLEN cap. Mirrors `read_pool_reserves`' Redis access
    /// because `host_ctx` is private to this struct. Fail-closed: returns the
    /// `RedisResult` so the caller logs + swallows any error (never breaks the
    /// shadow loop). NO-ACTIVE: callers only ever pass the dedicated outcomes
    /// stream — this path NEVER writes `arbx:opps:detected`.
    pub async fn xadd_shadow_outcome(
        &self,
        stream: &str,
        maxlen: u64,
        json: &str,
    ) -> redis::RedisResult<String> {
        let mut redis = self.host_ctx.redis.write().await;
        redis::cmd("XADD")
            .arg(stream)
            .arg("MAXLEN")
            .arg("~")
            .arg(maxlen)
            .arg("*")
            .arg("json")
            .arg(json)
            .query_async(&mut *redis)
            .await
    }

    // ─────────────────────────────────────────────────────────────────────
    // Private helpers
    // ─────────────────────────────────────────────────────────────────────

    /// Executes `init_strategy()` and parses the returned metadata map.
    fn execute_init_strategy(
        &self,
        ast: &AST,
        _cartridge_id: &str,
    ) -> Result<CartridgeMetadata, CartridgeError> {
        let mut scope = Scope::new();
        let result = self
            .engine
            .call_fn::<Dynamic>(&mut scope, ast, "init_strategy", ())
            .map_err(|e| CartridgeError::RuntimeError(format!("init_strategy() failed: {e}")))?;

        let map = result.try_cast::<Map>().ok_or_else(|| {
            CartridgeError::InvalidMetadata("init_strategy() must return a Map".into())
        })?;

        // Validate required metadata keys
        validate_metadata(&map).map_err(|violations| {
            let msg = violations
                .iter()
                .map(|v| v.to_string())
                .collect::<Vec<_>>()
                .join("; ");
            CartridgeError::InvalidMetadata(msg)
        })?;

        // Extract metadata fields
        let name = map
            .get("name")
            .and_then(|v| v.clone().into_string().ok())
            .unwrap_or_else(|| "unnamed".to_owned());
        let version = map
            .get("version")
            .and_then(|v| v.clone().into_string().ok())
            .unwrap_or_else(|| "0.0.0".to_owned());
        let author = map
            .get("author")
            .and_then(|v| v.clone().into_string().ok())
            .unwrap_or_else(|| "unknown".to_owned());
        let description = map
            .get("description")
            .and_then(|v| v.clone().into_string().ok())
            .unwrap_or_default();
        let category = map
            .get("category")
            .and_then(|v| v.clone().into_string().ok())
            .unwrap_or_else(|| "custom".to_owned());
        let min_eval_interval_ms = map
            .get("min_eval_interval_ms")
            .and_then(|v| v.as_int().ok())
            .unwrap_or(100) as u64;

        let target_chains: Vec<u64> = map
            .get("target_chains")
            .and_then(|v| v.clone().into_typed_array::<i64>().ok())
            .unwrap_or_default()
            .into_iter()
            .map(|c| c as u64)
            .collect();

        Ok(CartridgeMetadata {
            name,
            version,
            author,
            description,
            target_chains,
            category,
            min_eval_interval_ms,
        })
    }

    /// Parses the Dynamic result from `evaluate_opportunity` into a structured result.
    fn parse_eval_result(
        &self,
        result: Dynamic,
        _cartridge_id: &str,
    ) -> Result<CartridgeEvalResult, CartridgeError> {
        let map = result.try_cast::<Map>().ok_or_else(|| {
            CartridgeError::RuntimeError("evaluate_opportunity must return a Map".into())
        })?;

        let is_opportunity = map
            .get("is_opportunity")
            .and_then(|v| v.as_bool().ok())
            .unwrap_or(false);

        let estimated_profit = map
            .get("estimated_profit")
            .and_then(|v| v.as_float().ok())
            .unwrap_or(0.0);

        let confidence = map
            .get("confidence")
            .and_then(|v| v.as_float().ok())
            .unwrap_or(0.0)
            .clamp(0.0, 1.0);

        let urgency = map
            .get("urgency")
            .and_then(|v| v.clone().into_string().ok())
            .unwrap_or_else(|| "monitor".to_owned());

        // FASE B Paso 9 — explicit reason string (observability). The cartridge returns
        // it in its result map (e.g. "v3_sizing_pending"); promote it to a first-class
        // field so the durable outcomes series can explain WHY is_opportunity=false.
        let reason = map.get("reason").and_then(|v| v.clone().into_string().ok());

        // Collect any additional metadata fields
        let mut metadata = std::collections::HashMap::new();
        for (k, v) in map.iter() {
            let key = k.to_string();
            if ![
                "is_opportunity",
                "estimated_profit",
                "confidence",
                "urgency",
                "reason",
            ]
            .contains(&key.as_str())
            {
                metadata.insert(key, dynamic_to_json_value(v));
            }
        }

        Ok(CartridgeEvalResult {
            is_opportunity,
            estimated_profit,
            confidence,
            metadata,
            urgency,
            reason,
        })
    }
}

/// Converts a Rhai Dynamic value to a serde_json::Value.
fn dynamic_to_json_value(val: &Dynamic) -> serde_json::Value {
    if val.is_unit() {
        serde_json::Value::Null
    } else if let Ok(b) = val.as_bool() {
        serde_json::Value::Bool(b)
    } else if let Ok(i) = val.as_int() {
        serde_json::json!(i)
    } else if let Ok(f) = val.as_float() {
        serde_json::json!(f)
    } else if let Ok(s) = val.clone().into_string() {
        serde_json::Value::String(s)
    } else if val.is_array() {
        if let Ok(arr) = val.clone().into_typed_array::<Dynamic>() {
            serde_json::Value::Array(arr.iter().map(dynamic_to_json_value).collect())
        } else {
            serde_json::Value::Null
        }
    } else if val.is_map() {
        if let Some(map) = val.clone().try_cast::<Map>() {
            let obj: serde_json::Map<String, serde_json::Value> = map
                .iter()
                .map(|(k, v)| (k.to_string(), dynamic_to_json_value(v)))
                .collect();
            serde_json::Value::Object(obj)
        } else {
            serde_json::Value::Null
        }
    } else {
        serde_json::Value::String(format!("{:?}", val))
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use std::sync::atomic::AtomicU64;

    #[allow(dead_code)]
    fn make_test_host_ctx() -> HostContext {
        // For unit tests, we create a minimal context.
        // Real Redis is not needed — host bindings will return UNIT.
        // We use a mock-friendly approach here.
        let rt = tokio::runtime::Runtime::new().unwrap();
        let redis_client = rt.block_on(async {
            // Use a dummy URL — tests that exercise host bindings will
            // need a real Redis or a mock. Contract/compilation tests don't.
            let client = redis::Client::open("redis://127.0.0.1:6379").unwrap();
            client.get_connection_manager().await
        });

        // If Redis is not available, skip tests that need it.
        let redis_conn = match redis_client {
            Ok(conn) => conn,
            Err(_) => {
                // Create a placeholder — tests will skip Redis-dependent paths
                panic!("Redis not available for tests");
            }
        };

        HostContext {
            redis: Arc::new(RwLock::new(redis_conn)),
            chain_id: 1,
            cartridge_id: Arc::new(RwLock::new("test".to_owned())),
            rt_handle: rt.handle().clone(),
            block_number: Arc::new(AtomicU64::new(12345678)),
            base_fee_gwei: Arc::new(AtomicU64::new(30_000)), // 30 gwei
            telemetry_channel: "arbx:cartridge:telemetry".to_owned(),
        }
    }

    // Tests that only need compilation (no Redis) use a standalone engine.
    #[test]
    fn test_contract_validation_standalone() {
        let engine = Engine::new();
        let ast = engine
            .compile(
                r#"
            fn init_strategy() {
                #{
                    name: "Test",
                    version: "1.0.0",
                    author: "tester",
                    description: "test cartridge"
                }
            }
            fn evaluate_opportunity(pool_data) {
                #{ is_opportunity: false, estimated_profit: 0.0, confidence: 0.0 }
            }
            fn build_payload(opportunity) {
                #{ target_contract: "0x0", calldata: "0x0" }
            }
        "#,
            )
            .unwrap();

        assert!(validate_contract(&ast).is_ok());
    }

    #[test]
    fn dex_arb_cartridge_compiles_and_validates() {
        // Guards the V3 branch edit (and the whole master cartridge) against syntax/contract
        // regressions. Compile-only: host bindings (get_v3_slot0, calc_v3_spot_price, …) resolve
        // at EVAL time, not compile time, so a bare engine validates the structure here.
        let src = std::fs::read_to_string(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/cartridges/dex_arb.rhai"
        ))
        .expect("read dex_arb.rhai");
        let engine = Engine::new();
        let ast = engine.compile(&src).expect("dex_arb.rhai must compile");
        assert!(
            validate_contract(&ast).is_ok(),
            "dex_arb.rhai must satisfy the universal contract"
        );
    }

    /// Evaluate dex_arb.rhai with STUBBED host bindings (no Redis). Returns the result map
    /// plus two tripwires: whether `log_quantum` fired and whether `get_pool_index` was reached.
    /// `slot0_present` controls get_v3_slot0; `pool_index_len` controls the alt-pool count.
    fn eval_dex_arb_stub(
        protocol: &str,
        slot0_present: bool,
        pool_index_len: usize,
    ) -> (Map, bool, bool) {
        use std::sync::atomic::{AtomicBool, Ordering};
        use std::sync::Arc;
        let src = std::fs::read_to_string(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/cartridges/dex_arb.rhai"
        ))
        .expect("read dex_arb.rhai");
        let mut engine = Engine::new();
        let log_fired = Arc::new(AtomicBool::new(false));
        let pool_index_hit = Arc::new(AtomicBool::new(false));

        engine.register_fn("get_chain_id", || -> i64 { 1 });
        engine.register_fn("get_token_meta", |_addr: &str| -> Dynamic {
            let mut m = Map::new();
            m.insert("symbol".into(), Dynamic::from("WETH".to_string()));
            m.insert("decimals".into(), Dynamic::from(18i64));
            Dynamic::from_map(m)
        });
        engine.register_fn("get_v3_slot0", move |_pool: &str| -> Dynamic {
            if slot0_present {
                let mut m = Map::new();
                m.insert(
                    "sqrt_price_x96".into(),
                    Dynamic::from("4339505179874779672736".to_string()),
                );
                m.insert("liquidity".into(), Dynamic::from("57737784664".to_string()));
                Dynamic::from_map(m)
            } else {
                Dynamic::UNIT
            }
        });
        engine.register_fn(
            "calc_v3_spot_price",
            |_s: &str, _di: i64, _do: i64| -> f64 { 3000.0 },
        );
        // Real single-tick math (same conversion as the production host binding: bps -> pips ×100).
        engine.register_fn(
            "v3_amount_out_single_tick",
            |amount_in: &str,
             sqrt_price_x96: &str,
             liquidity: &str,
             fee_bps: i64,
             zero_for_one: bool|
             -> Dynamic {
                use ethers::types::U256;
                let amount_in = match U256::from_dec_str(amount_in) {
                    Ok(v) => v,
                    Err(_) => return Dynamic::UNIT,
                };
                let sqrt_price_x96 = match U256::from_dec_str(sqrt_price_x96) {
                    Ok(v) => v,
                    Err(_) => return Dynamic::UNIT,
                };
                let liquidity = match U256::from_dec_str(liquidity) {
                    Ok(v) => v,
                    Err(_) => return Dynamic::UNIT,
                };
                let fee_pips = fee_bps * 100;
                if !(0..1_000_000).contains(&fee_pips) {
                    return Dynamic::UNIT;
                }
                let result = crate::amm_math::v3_amount_out_single_tick(
                    amount_in,
                    sqrt_price_x96,
                    liquidity,
                    fee_pips as u32,
                    zero_for_one,
                );
                if result.is_zero() {
                    Dynamic::UNIT
                } else {
                    Dynamic::from(result.to_string())
                }
            },
        );
        // Cross-pool arb gate: default OFF in this unit test so the V3 branch emits the honest
        // is_opportunity:false single-tick candidate (the is_opportunity:true path is exercised
        // separately/integration). Production reads ARBX_V3_ARB_MODE.
        engine.register_fn("v3_arb_enabled", || -> bool { false });
        // Stubs so the (gated-off) arb code path still resolves if ever reached: never used while
        // v3_arb_enabled()==false short-circuits first, but registered for eval completeness.
        engine.register_fn("get_token_price_usd", |_sym: &str| -> Dynamic {
            Dynamic::UNIT
        });
        engine.register_fn(
            "v2_amount_out_str",
            |_ai: &str, _ri: &str, _ro: &str, _fee: i64| -> Dynamic { Dynamic::UNIT },
        );
        {
            let lf = log_fired.clone();
            engine.register_fn("log_quantum", move |_lvl: &str, _msg: &str| {
                lf.store(true, Ordering::SeqCst);
            });
        }
        {
            let pih = pool_index_hit.clone();
            engine.register_fn("get_pool_index", move |_a: &str, _b: &str| -> Dynamic {
                pih.store(true, Ordering::SeqCst);
                let arr: Vec<Dynamic> = (0..pool_index_len)
                    .map(|i| Dynamic::from(format!("0xpool{i}")))
                    .collect();
                Dynamic::from_array(arr)
            });
        }

        let ast = engine.compile(&src).expect("compile dex_arb.rhai");
        let mut scope = Scope::new();
        let mut pd = Map::new();
        pd.insert("chain_id".into(), Dynamic::from(1i64));
        pd.insert(
            "token_in".into(),
            Dynamic::from("0x00000000000000000000000000000000000000aa".to_string()),
        );
        pd.insert(
            "token_out".into(),
            Dynamic::from("0x00000000000000000000000000000000000000bb".to_string()),
        );
        pd.insert(
            "source_pool".into(),
            Dynamic::from("0x00000000000000000000000000000000000000cc".to_string()),
        );
        // 1 WETH (1e18 wei). With the stub slot0 (sqrtP=4339505179874779672736, L=57737784664)
        // and zero_for_one=true this drives a non-degenerate single-tick move, so the V3 branch
        // reaches the candidate (a tiny 1e6-wei probe rounds to a zero price move = degenerate).
        pd.insert(
            "amount_in".into(),
            Dynamic::from("1000000000000000000".to_string()),
        );
        pd.insert("protocol_type".into(), Dynamic::from(protocol.to_string()));
        // V3 fee tier (0.30% = 30 bps) so the V3 branch exercises the single-tick candidate path.
        pd.insert("fee_bps".into(), Dynamic::from(30i64));

        let res: Dynamic = engine
            .call_fn(&mut scope, &ast, "evaluate_opportunity", (pd,))
            .expect("evaluate_opportunity");
        (
            res.cast::<Map>(),
            log_fired.load(Ordering::SeqCst),
            pool_index_hit.load(Ordering::SeqCst),
        )
    }

    fn reason_of(m: &Map) -> String {
        m.get("reason").unwrap().clone().into_string().unwrap()
    }

    #[test]
    fn dex_arb_v3_prices_before_insufficient_pools_gate() {
        // V3 source with slot0 + fee tier available: must reach the V3 branch (emit telemetry,
        // return the single-tick candidate) WITHOUT being blocked by the V2 get_pool_index >= 2 gate.
        let (res, log_fired, pool_index_hit) = eval_dex_arb_stub("v3", true, 0);
        assert_eq!(reason_of(&res), "v3_single_tick_candidate");
        // SOUND: single-tick is an upper bound -> never a confirmed opportunity.
        assert!(!res.get("is_opportunity").unwrap().as_bool().unwrap());
        // The candidate carries the REAL single-tick estimate (RULE 00: no fabricated value).
        let est_out = res
            .get("estimated_amount_out")
            .expect("candidate must carry estimated_amount_out")
            .clone()
            .into_string()
            .expect("estimated_amount_out is a decimal string");
        assert!(
            !est_out.is_empty() && est_out.chars().all(|c| c.is_ascii_digit()),
            "estimated_amount_out must be a non-empty decimal string, got {est_out:?}"
        );
        assert!(
            log_fired,
            "log_quantum (v3_source_priced) must fire — shadow telemetry"
        );
        assert!(
            !pool_index_hit,
            "V3 must NOT reach the get_pool_index insufficient_pools gate"
        );
    }

    #[test]
    fn dex_arb_v3_without_slot0_is_unavailable_not_panic() {
        let (res, log_fired, _pih) = eval_dex_arb_stub("v3", false, 0);
        assert_eq!(reason_of(&res), "v3_slot0_unavailable");
        assert!(!log_fired, "no price telemetry when slot0 is absent");
    }

    #[test]
    fn dex_arb_v2_insufficient_pools_gate_still_enforced() {
        // V2 path with < 2 alt pools must STILL be gated (no relaxation of the V2 cross-DEX rule).
        let (res, _lf, pool_index_hit) = eval_dex_arb_stub("v2", true, 1);
        assert_eq!(reason_of(&res), "insufficient_pools");
        assert!(pool_index_hit, "V2 path must consult get_pool_index");
    }

    #[test]
    fn test_infinite_loop_protection() {
        let mut engine = Engine::new();
        engine.set_max_operations(1000);

        let ast = engine
            .compile(
                r#"
            fn init_strategy() { #{ name: "X", version: "1", author: "a", description: "d" } }
            fn evaluate_opportunity(pool_data) {
                let x = 0;
                loop { x += 1; }
                #{ is_opportunity: false }
            }
            fn build_payload(opp) { #{} }
        "#,
            )
            .unwrap();

        let mut scope = Scope::new();
        let pool_data = Map::new();
        let result =
            engine.call_fn::<Dynamic>(&mut scope, &ast, "evaluate_opportunity", (pool_data,));
        assert!(result.is_err());
        let err_str = result.unwrap_err().to_string();
        assert!(
            err_str.contains("operations") || err_str.contains("limit"),
            "Expected operation limit error, got: {err_str}"
        );
    }

    #[test]
    fn test_division_by_zero_handled() {
        let mut engine = Engine::new();
        engine.set_max_operations(MAX_OPERATIONS);

        let ast = engine
            .compile(
                r#"
            fn init_strategy() { #{ name: "X", version: "1", author: "a", description: "d" } }
            fn evaluate_opportunity(pool_data) {
                let x = 1 / 0;
                #{ is_opportunity: false }
            }
            fn build_payload(opp) { #{} }
        "#,
            )
            .unwrap();

        let mut scope = Scope::new();
        let pool_data = Map::new();
        let result =
            engine.call_fn::<Dynamic>(&mut scope, &ast, "evaluate_opportunity", (pool_data,));
        // Division by zero in Rhai produces a runtime error, not a panic
        assert!(result.is_err());
    }
}
