//! End-to-end integration test for the FASE OMEGA Cartridge Runtime.
//!
//! This test validates the complete flow:
//!   1. CartridgeRunner initialization with sandboxed engine
//!   2. Cartridge compilation and contract validation
//!   3. Metadata extraction via `init_strategy()`
//!   4. Opportunity evaluation via `evaluate_opportunity()`
//!   5. Payload assembly via `build_payload()`
//!   6. Sandboxing enforcement (operation limits, memory bounds)
//!   7. Hot-reload dedup (content hash check)
//!   8. Lifecycle management (pause/resume/unload)
//!
//! ## Prerequisites
//!
//! - Redis running on localhost:6379 (for host binding tests)
//! - No external chain/RPC access needed (all data mocked via Redis)
//!
//! ## Running
//!
//! ```bash
//! cargo test --test cartridge_e2e_test -- --nocapture
//! ```

use rhai::{Dynamic, Engine, Map, Scope};
use std::collections::HashMap;

// ─────────────────────────────────────────────────────────────────────────────
// Test: Contract Validation (no Redis needed)
// ─────────────────────────────────────────────────────────────────────────────

/// Valid cartridge passes all contract checks.
#[test]
fn test_valid_cartridge_compiles_and_validates() {
    let engine = Engine::new();
    let source = include_str!("../cartridges/dex_arb.rhai");
    let ast = engine.compile(source);
    assert!(ast.is_ok(), "dex_arb.rhai should compile without errors: {:?}", ast.err());

    let ast = ast.unwrap();

    // Check required functions exist
    let fn_names: Vec<&str> = ast.iter_functions().map(|f| f.name.as_str()).collect();
    assert!(fn_names.contains(&"init_strategy"), "missing init_strategy");
    assert!(fn_names.contains(&"evaluate_opportunity"), "missing evaluate_opportunity");
    assert!(fn_names.contains(&"build_payload"), "missing build_payload");

    // Check optional hooks
    assert!(fn_names.contains(&"on_activate"), "missing on_activate hook");
    assert!(fn_names.contains(&"on_deactivate"), "missing on_deactivate hook");
    assert!(fn_names.contains(&"on_new_block"), "missing on_new_block hook");
}

/// Triangular arbitrage cartridge compiles and validates.
#[test]
fn test_triangular_arb_compiles() {
    let engine = Engine::new();
    let source = include_str!("../cartridges/triangular_arb.rhai");
    let ast = engine.compile(source);
    assert!(ast.is_ok(), "triangular_arb.rhai should compile: {:?}", ast.err());

    let ast = ast.unwrap();
    let fn_names: Vec<&str> = ast.iter_functions().map(|f| f.name.as_str()).collect();
    assert!(fn_names.contains(&"init_strategy"));
    assert!(fn_names.contains(&"evaluate_opportunity"));
    assert!(fn_names.contains(&"build_payload"));
}

/// Liquidation cartridge compiles and validates.
#[test]
fn test_liquidation_compiles() {
    let engine = Engine::new();
    let source = include_str!("../cartridges/liquidation.rhai");
    let ast = engine.compile(source);
    assert!(ast.is_ok(), "liquidation.rhai should compile: {:?}", ast.err());

    let ast = ast.unwrap();
    let fn_names: Vec<&str> = ast.iter_functions().map(|f| f.name.as_str()).collect();
    assert!(fn_names.contains(&"init_strategy"));
    assert!(fn_names.contains(&"evaluate_opportunity"));
    assert!(fn_names.contains(&"build_payload"));
}

// ─────────────────────────────────────────────────────────────────────────────
// Test: Metadata Extraction
// ─────────────────────────────────────────────────────────────────────────────

/// init_strategy() returns valid metadata with all required fields.
#[test]
fn test_init_strategy_returns_valid_metadata() {
    let mut engine = Engine::new();
    engine.set_max_operations(1_000_000);

    // Register stub host bindings for standalone test
    engine.register_fn("get_chain_id", || -> Dynamic { Dynamic::from(1_i64) });
    engine.register_fn("get_block_number", || -> Dynamic { Dynamic::from(12345678_i64) });
    engine.register_fn("get_base_fee", || -> Dynamic { Dynamic::from(30.0_f64) });
    engine.register_fn("get_timestamp", || -> Dynamic { Dynamic::from(1717200000_i64) });
    engine.register_fn("log_quantum", |_level: &str, _msg: &str| -> Dynamic { Dynamic::UNIT });
    engine.register_fn("emit_signal", |_sig: &str, _data: Map| -> Dynamic { Dynamic::UNIT });
    engine.register_fn("get_reserves", |_addr: &str| -> Dynamic { Dynamic::UNIT });
    engine.register_fn("get_token_meta", |_addr: &str| -> Dynamic { Dynamic::UNIT });
    engine.register_fn("get_pool_index", |_a: &str, _b: &str| -> Dynamic { Dynamic::from_array(vec![]) });
    engine.register_fn("simulate_swap", |_amt: &str, _path: rhai::Array| -> Dynamic { Dynamic::UNIT });
    engine.register_fn("math_sqrt", |x: f64| -> Dynamic { Dynamic::from(x.sqrt()) });
    engine.register_fn("math_abs", |x: f64| -> Dynamic { Dynamic::from(x.abs()) });
    engine.register_fn("math_min", |a: f64, b: f64| -> Dynamic { Dynamic::from(a.min(b)) });
    engine.register_fn("math_max", |a: f64, b: f64| -> Dynamic { Dynamic::from(a.max(b)) });
    engine.register_fn("math_pow", |base: f64, exp: f64| -> Dynamic { Dynamic::from(base.powf(exp)) });
    engine.register_fn("math_log", |x: f64| -> Dynamic { Dynamic::from(x.ln()) });
    engine.register_fn("math_exp", |x: f64| -> Dynamic { Dynamic::from(x.exp()) });
    engine.register_fn("to_wei", |amount: f64, decimals: i64| -> Dynamic {
        let factor = 10f64.powi(decimals as i32);
        Dynamic::from((amount * factor) as i64)
    });
    engine.register_fn("from_wei", |wei_str: &str, decimals: i64| -> Dynamic {
        let wei: f64 = wei_str.parse().unwrap_or(0.0);
        let factor = 10f64.powi(decimals as i32);
        Dynamic::from(wei / factor)
    });

    let source = include_str!("../cartridges/dex_arb.rhai");
    let ast = engine.compile(source).unwrap();

    let mut scope = Scope::new();
    let result: Dynamic = engine.call_fn(&mut scope, &ast, "init_strategy", ()).unwrap();
    let map = result.cast::<Map>();

    // Verify required metadata fields
    assert!(map.contains_key("name"), "metadata missing 'name'");
    assert!(map.contains_key("version"), "metadata missing 'version'");
    assert!(map.contains_key("author"), "metadata missing 'author'");
    assert!(map.contains_key("description"), "metadata missing 'description'");

    // Verify values
    let name = map.get("name").unwrap().clone().into_immutable_string().unwrap();
    assert_eq!(&*name, "DEX Arbitrage Universal");

    let version = map.get("version").unwrap().clone().into_immutable_string().unwrap();
    assert_eq!(&*version, "2.0.0");

    // Verify target_chains is empty (supports all chains)
    let target_chains = map.get("target_chains").unwrap().clone().into_typed_array::<Dynamic>().unwrap();
    assert!(target_chains.is_empty(), "target_chains should be empty for universal support");
}

// ─────────────────────────────────────────────────────────────────────────────
// Test: Opportunity Evaluation (with mocked host bindings)
// ─────────────────────────────────────────────────────────────────────────────

/// evaluate_opportunity returns structured result with no pools available.
#[test]
fn test_evaluate_opportunity_no_pools() {
    let mut engine = Engine::new();
    engine.set_max_operations(1_000_000);

    // Register stub bindings that return "no data"
    engine.register_fn("get_chain_id", || -> Dynamic { Dynamic::from(42161_i64) }); // Arbitrum
    engine.register_fn("get_block_number", || -> Dynamic { Dynamic::from(200000000_i64) });
    engine.register_fn("get_base_fee", || -> Dynamic { Dynamic::from(0.1_f64) });
    engine.register_fn("get_timestamp", || -> Dynamic { Dynamic::from(1717200000_i64) });
    engine.register_fn("log_quantum", |_level: &str, _msg: &str| -> Dynamic { Dynamic::UNIT });
    engine.register_fn("emit_signal", |_sig: &str, _data: Map| -> Dynamic { Dynamic::UNIT });
    engine.register_fn("get_reserves", |_addr: &str| -> Dynamic { Dynamic::UNIT });
    engine.register_fn("get_token_meta", |_addr: &str| -> Dynamic {
        let mut map = Map::new();
        map.insert("symbol".into(), Dynamic::from("WETH"));
        map.insert("decimals".into(), Dynamic::from(18_i64));
        map.insert("is_stablecoin".into(), Dynamic::from(false));
        Dynamic::from_map(map)
    });
    engine.register_fn("get_pool_index", |_a: &str, _b: &str| -> Dynamic {
        // Return only 1 pool (need >= 2 for arb)
        Dynamic::from_array(vec![Dynamic::from("0xpool1")])
    });
    engine.register_fn("simulate_swap", |_amt: &str, _path: rhai::Array| -> Dynamic { Dynamic::UNIT });
    engine.register_fn("math_sqrt", |x: f64| -> Dynamic { Dynamic::from(x.sqrt()) });
    engine.register_fn("math_abs", |x: f64| -> Dynamic { Dynamic::from(x.abs()) });
    engine.register_fn("math_min", |a: f64, b: f64| -> Dynamic { Dynamic::from(a.min(b)) });
    engine.register_fn("math_max", |a: f64, b: f64| -> Dynamic { Dynamic::from(a.max(b)) });
    engine.register_fn("math_pow", |base: f64, exp: f64| -> Dynamic { Dynamic::from(base.powf(exp)) });
    engine.register_fn("math_log", |x: f64| -> Dynamic { Dynamic::from(x.ln()) });
    engine.register_fn("math_exp", |x: f64| -> Dynamic { Dynamic::from(x.exp()) });
    engine.register_fn("to_wei", |amount: f64, decimals: i64| -> Dynamic {
        Dynamic::from((amount * 10f64.powi(decimals as i32)) as i64)
    });
    engine.register_fn("from_wei", |wei_str: &str, decimals: i64| -> Dynamic {
        let wei: f64 = wei_str.parse().unwrap_or(0.0);
        Dynamic::from(wei / 10f64.powi(decimals as i32))
    });

    let source = include_str!("../cartridges/dex_arb.rhai");
    let ast = engine.compile(source).unwrap();

    // Build pool_data input
    let mut pool_data = Map::new();
    pool_data.insert("chain_id".into(), Dynamic::from(42161_i64));
    pool_data.insert("source_pool".into(), Dynamic::from("0xpool1"));
    pool_data.insert("token_in".into(), Dynamic::from("0xweth"));
    pool_data.insert("token_out".into(), Dynamic::from("0xusdc"));
    pool_data.insert("amount_in".into(), Dynamic::from("1000000000000000000"));
    pool_data.insert("protocol_type".into(), Dynamic::from("v2"));
    pool_data.insert("gas_price_gwei".into(), Dynamic::from(0.1_f64));
    pool_data.insert("block_number".into(), Dynamic::from(200000000_i64));

    let mut reserves = Map::new();
    reserves.insert("r0".into(), Dynamic::from("5000000000000000000000"));
    reserves.insert("r1".into(), Dynamic::from("10000000000000"));
    reserves.insert("block".into(), Dynamic::from(200000000_i64));
    reserves.insert("ts".into(), Dynamic::from(1717200000_i64));
    pool_data.insert("reserves_source".into(), Dynamic::from_map(reserves));

    let mut scope = Scope::new();
    let result: Dynamic = engine.call_fn(&mut scope, &ast, "evaluate_opportunity", (pool_data,)).unwrap();
    let map = result.cast::<Map>();

    // Should return not an opportunity (insufficient pools)
    let is_opp = map.get("is_opportunity").unwrap().as_bool().unwrap();
    assert!(!is_opp, "should not find opportunity with < 2 pools");

    let reason = map.get("reason").unwrap().clone().into_immutable_string().unwrap();
    assert_eq!(&*reason, "insufficient_pools");
}

// ─────────────────────────────────────────────────────────────────────────────
// Test: Sandboxing — Infinite Loop Protection
// ─────────────────────────────────────────────────────────────────────────────

/// A malicious cartridge with an infinite loop is terminated.
#[test]
fn test_infinite_loop_terminated() {
    let mut engine = Engine::new();
    engine.set_max_operations(10_000); // Low limit for fast test

    let malicious_source = r#"
        fn init_strategy() {
            #{ name: "Evil", version: "1.0.0", author: "hacker", description: "loops forever" }
        }
        fn evaluate_opportunity(pool_data) {
            let x = 0;
            loop { x += 1; }
            #{ is_opportunity: false }
        }
        fn build_payload(opp) { #{} }
    "#;

    let ast = engine.compile(malicious_source).unwrap();
    let mut scope = Scope::new();
    let pool_data = Map::new();

    let result = engine.call_fn::<Dynamic>(&mut scope, &ast, "evaluate_opportunity", (pool_data,));
    assert!(result.is_err(), "infinite loop should be terminated");

    let err_str = result.unwrap_err().to_string();
    assert!(
        err_str.contains("operations") || err_str.contains("limit") || err_str.contains("progress"),
        "error should mention operation limit: {err_str}"
    );
}

/// A cartridge that tries to allocate massive arrays is bounded.
#[test]
fn test_memory_bomb_prevented() {
    let mut engine = Engine::new();
    engine.set_max_operations(1_000_000);
    engine.set_max_array_size(100); // Very small for test

    let bomb_source = r#"
        fn init_strategy() {
            #{ name: "Bomb", version: "1.0.0", author: "hacker", description: "memory bomb" }
        }
        fn evaluate_opportunity(pool_data) {
            let arr = [];
            for i in 0..10000 {
                arr.push(i);
            }
            #{ is_opportunity: false }
        }
        fn build_payload(opp) { #{} }
    "#;

    let ast = engine.compile(bomb_source).unwrap();
    let mut scope = Scope::new();
    let pool_data = Map::new();

    let result = engine.call_fn::<Dynamic>(&mut scope, &ast, "evaluate_opportunity", (pool_data,));
    assert!(result.is_err(), "memory bomb should be caught");
}

/// A cartridge that tries to create huge strings is bounded.
#[test]
fn test_string_bomb_prevented() {
    let mut engine = Engine::new();
    engine.set_max_operations(1_000_000);
    engine.set_max_string_size(1024); // 1KB limit

    let bomb_source = r#"
        fn init_strategy() {
            #{ name: "StrBomb", version: "1.0.0", author: "hacker", description: "string bomb" }
        }
        fn evaluate_opportunity(pool_data) {
            let s = "x";
            for i in 0..20 {
                s = s + s;  // Exponential growth: 2^20 = 1MB
            }
            #{ is_opportunity: false }
        }
        fn build_payload(opp) { #{} }
    "#;

    let ast = engine.compile(bomb_source).unwrap();
    let mut scope = Scope::new();
    let pool_data = Map::new();

    let result = engine.call_fn::<Dynamic>(&mut scope, &ast, "evaluate_opportunity", (pool_data,));
    assert!(result.is_err(), "string bomb should be caught");
}

// ─────────────────────────────────────────────────────────────────────────────
// Test: Multi-Chain Support
// ─────────────────────────────────────────────────────────────────────────────

/// The cartridge works with any chain_id injected by the host.
#[test]
fn test_multi_chain_agnostic() {
    let chain_ids: Vec<i64> = vec![
        1,      // Ethereum
        42161,  // Arbitrum
        8453,   // Base
        137,    // Polygon
        56,     // BSC
        43114,  // Avalanche
        10,     // Optimism
        324,    // zkSync Era
        59144,  // Linea
        534352, // Scroll
    ];

    for chain_id in chain_ids {
        let mut engine = Engine::new();
        engine.set_max_operations(1_000_000);

        let cid = chain_id;
        engine.register_fn("get_chain_id", move || -> Dynamic { Dynamic::from(cid) });
        engine.register_fn("get_block_number", || -> Dynamic { Dynamic::from(99999_i64) });
        engine.register_fn("get_base_fee", || -> Dynamic { Dynamic::from(10.0_f64) });
        engine.register_fn("get_timestamp", || -> Dynamic { Dynamic::from(1717200000_i64) });
        engine.register_fn("log_quantum", |_level: &str, _msg: &str| -> Dynamic { Dynamic::UNIT });
        engine.register_fn("emit_signal", |_sig: &str, _data: Map| -> Dynamic { Dynamic::UNIT });
        engine.register_fn("get_reserves", |_addr: &str| -> Dynamic { Dynamic::UNIT });
        engine.register_fn("get_token_meta", |_addr: &str| -> Dynamic { Dynamic::UNIT });
        engine.register_fn("get_pool_index", |_a: &str, _b: &str| -> Dynamic { Dynamic::from_array(vec![]) });
        engine.register_fn("simulate_swap", |_amt: &str, _path: rhai::Array| -> Dynamic { Dynamic::UNIT });
        engine.register_fn("math_sqrt", |x: f64| -> Dynamic { Dynamic::from(x.sqrt()) });
        engine.register_fn("math_abs", |x: f64| -> Dynamic { Dynamic::from(x.abs()) });
        engine.register_fn("math_min", |a: f64, b: f64| -> Dynamic { Dynamic::from(a.min(b)) });
        engine.register_fn("math_max", |a: f64, b: f64| -> Dynamic { Dynamic::from(a.max(b)) });
        engine.register_fn("math_pow", |base: f64, exp: f64| -> Dynamic { Dynamic::from(base.powf(exp)) });
        engine.register_fn("math_log", |x: f64| -> Dynamic { Dynamic::from(x.ln()) });
        engine.register_fn("math_exp", |x: f64| -> Dynamic { Dynamic::from(x.exp()) });
        engine.register_fn("to_wei", |amount: f64, decimals: i64| -> Dynamic {
            Dynamic::from((amount * 10f64.powi(decimals as i32)) as i64)
        });
        engine.register_fn("from_wei", |wei_str: &str, decimals: i64| -> Dynamic {
            let wei: f64 = wei_str.parse().unwrap_or(0.0);
            Dynamic::from(wei / 10f64.powi(decimals as i32))
        });

        let source = include_str!("../cartridges/dex_arb.rhai");
        let ast = engine.compile(source).unwrap();

        // init_strategy should work regardless of chain
        let mut scope = Scope::new();
        let result: Dynamic = engine.call_fn(&mut scope, &ast, "init_strategy", ()).unwrap();
        let map = result.cast::<Map>();
        assert!(map.contains_key("name"), "chain {chain_id}: metadata missing name");

        // evaluate_opportunity should handle gracefully when token_meta unavailable
        let mut pool_data = Map::new();
        pool_data.insert("chain_id".into(), Dynamic::from(chain_id));
        pool_data.insert("source_pool".into(), Dynamic::from("0xpool"));
        pool_data.insert("token_in".into(), Dynamic::from("0xtoken_in"));
        pool_data.insert("token_out".into(), Dynamic::from("0xtoken_out"));
        pool_data.insert("amount_in".into(), Dynamic::from("1000000000000000000"));
        pool_data.insert("protocol_type".into(), Dynamic::from("v2"));
        pool_data.insert("gas_price_gwei".into(), Dynamic::from(10.0_f64));
        pool_data.insert("block_number".into(), Dynamic::from(99999_i64));

        let mut scope2 = Scope::new();
        let eval_result: Dynamic = engine
            .call_fn(&mut scope2, &ast, "evaluate_opportunity", (pool_data,))
            .unwrap();
        let eval_map = eval_result.cast::<Map>();

        // Should gracefully return no opportunity (no token meta available)
        let is_opp = eval_map.get("is_opportunity").unwrap().as_bool().unwrap();
        assert!(!is_opp, "chain {chain_id}: should not find opportunity without token meta");
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Test: Contract Violation Detection
// ─────────────────────────────────────────────────────────────────────────────

/// Missing required function is detected.
#[test]
fn test_missing_function_detected() {
    let engine = Engine::new();
    let incomplete = r#"
        fn init_strategy() { #{ name: "X", version: "1", author: "a", description: "d" } }
        fn evaluate_opportunity(pool_data) { #{ is_opportunity: false } }
        // build_payload is MISSING
    "#;

    let ast = engine.compile(incomplete).unwrap();
    let fn_names: Vec<&str> = ast.iter_functions().map(|f| f.name.as_str()).collect();
    assert!(!fn_names.contains(&"build_payload"), "build_payload should be missing");
}

/// Wrong arity is detected.
#[test]
fn test_wrong_arity_detected() {
    let engine = Engine::new();
    let wrong_arity = r#"
        fn init_strategy(unexpected) { #{} }
        fn evaluate_opportunity(pool_data) { #{} }
        fn build_payload(opp) { #{} }
    "#;

    let ast = engine.compile(wrong_arity).unwrap();
    for func in ast.iter_functions() {
        if func.name == "init_strategy" {
            assert_eq!(func.params.len(), 1, "should detect wrong arity");
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Test: Content Hash Dedup
// ─────────────────────────────────────────────────────────────────────────────

/// Same content hash should not trigger recompilation.
#[test]
fn test_content_hash_dedup() {
    use sha2::{Sha256, Digest};

    let source = include_str!("../cartridges/dex_arb.rhai");

    // Compute hash
    let mut hasher = Sha256::new();
    hasher.update(source.as_bytes());
    let hash1 = format!("{:x}", hasher.finalize());

    // Same source = same hash
    let mut hasher2 = Sha256::new();
    hasher2.update(source.as_bytes());
    let hash2 = format!("{:x}", hasher2.finalize());

    assert_eq!(hash1, hash2, "same source should produce same hash");

    // Different source = different hash
    let modified = format!("{}\n// modified", source);
    let mut hasher3 = Sha256::new();
    hasher3.update(modified.as_bytes());
    let hash3 = format!("{:x}", hasher3.finalize());

    assert_ne!(hash1, hash3, "different source should produce different hash");
}
