//! Real evaluation tests for production cartridges (NOT compile-only, NOT stubbed-away).
//!
//! These tests load the actual `.rhai` cartridges and RUN `init_strategy()`,
//! `evaluate_opportunity()` and `build_payload()` against hand-built `pool_data`,
//! asserting the real decision logic — closing the coverage gap the cartridge
//! review flagged (existing tests only compiled or stubbed the strategies away).
//!
//! Host bindings are registered as deterministic in-test stubs (no Redis/RPC),
//! mirroring the production host-binding signatures. Run:
//!   cargo test --test cartridge_strategies_test -- --nocapture

use rhai::{Dynamic, Engine, Map, Scope};

// ─────────────────────────────────────────────────────────────────────────────
// Shared host-binding stubs
// ─────────────────────────────────────────────────────────────────────────────

/// Register the pure/math + logging host bindings every cartridge may call.
fn register_common(engine: &mut Engine) {
    engine.set_max_operations(2_000_000);
    engine.register_fn("get_chain_id", || -> Dynamic { Dynamic::from(1_i64) });
    engine.register_fn("get_block_number", || -> Dynamic { Dynamic::from(20_000_000_i64) });
    engine.register_fn("get_base_fee", || -> Dynamic { Dynamic::from(30.0_f64) });
    engine.register_fn("get_timestamp", || -> Dynamic { Dynamic::from(1_717_200_000_i64) });
    engine.register_fn("log_quantum", |_l: &str, _m: &str| -> Dynamic { Dynamic::UNIT });
    engine.register_fn("emit_signal", |_s: &str, _d: Map| -> Dynamic { Dynamic::UNIT });
    engine.register_fn("math_abs", |x: f64| -> Dynamic { Dynamic::from(x.abs()) });
    engine.register_fn("math_min", |a: f64, b: f64| -> Dynamic { Dynamic::from(a.min(b)) });
    engine.register_fn("math_max", |a: f64, b: f64| -> Dynamic { Dynamic::from(a.max(b)) });
    engine.register_fn("math_pow", |b: f64, e: f64| -> Dynamic { Dynamic::from(b.powf(e)) });
    engine.register_fn("from_wei", |wei: &str, dec: i64| -> Dynamic {
        let v: f64 = wei.parse().unwrap_or(0.0);
        Dynamic::from(v / 10f64.powi(dec as i32))
    });
    // Mirror production: host_bindings.rs registers `to_float(&str)` because rhai
    // has no native String::to_float and the dex_arb / triangular cartridges call
    // `s.to_float()`. REGRESSION GUARD: if this line is removed, the triangular
    // tests fail with ErrorFunctionNotFound — exactly the latent production bug
    // this binding fixes (see host_bindings.rs `to_float`).
    engine.register_fn("to_float", |s: &str| -> Dynamic {
        Dynamic::from(s.parse::<f64>().unwrap_or(0.0))
    });
}

fn token_meta(symbol: &str, decimals: i64) -> Dynamic {
    let mut m = Map::new();
    m.insert("symbol".into(), Dynamic::from(symbol.to_string()));
    m.insert("decimals".into(), Dynamic::from(decimals));
    m.insert("is_stablecoin".into(), Dynamic::from(symbol == "USDC" || symbol == "DAI"));
    Dynamic::from_map(m)
}

fn reserves(r0: &str, r1: &str) -> Dynamic {
    let mut m = Map::new();
    m.insert("r0".into(), Dynamic::from(r0.to_string()));
    m.insert("r1".into(), Dynamic::from(r1.to_string()));
    m.insert("block".into(), Dynamic::from(20_000_000_i64));
    m.insert("ts".into(), Dynamic::from(1_717_200_000_i64));
    Dynamic::from_map(m)
}

fn eval(engine: &Engine, src: &str, func: &str, arg: Map) -> Map {
    let ast = engine.compile(src).expect("cartridge compiles");
    let mut scope = Scope::new();
    let out: Dynamic = engine
        .call_fn(&mut scope, &ast, func, (arg,))
        .expect("function runs");
    out.cast::<Map>()
}

// ═════════════════════════════════════════════════════════════════════════════
// BACKRUN (zero-victim) — backrun.rhai
// ═════════════════════════════════════════════════════════════════════════════

const BACKRUN: &str = include_str!("../cartridges/backrun.rhai");

fn backrun_engine() -> Engine {
    let mut engine = Engine::new();
    register_common(&mut engine);
    engine.register_fn("get_token_meta", |addr: &str| -> Dynamic {
        match addr {
            "0xweth" => token_meta("WETH", 18),
            "0xusdc" => token_meta("USDC", 6),
            _ => Dynamic::UNIT,
        }
    });
    engine
}

fn backrun_pool_data(source_event: &str, amount_in: &str, r1_usdc: &str) -> Map {
    let mut pd = Map::new();
    pd.insert("chain_id".into(), Dynamic::from(1_i64));
    pd.insert("source_pool".into(), Dynamic::from("0xpool"));
    pd.insert("token_in".into(), Dynamic::from("0xweth"));
    pd.insert("token_out".into(), Dynamic::from("0xusdc"));
    pd.insert("amount_in".into(), Dynamic::from(amount_in.to_string()));
    pd.insert("protocol_type".into(), Dynamic::from("v2"));
    pd.insert("fair_price".into(), Dynamic::from(2000.0_f64));
    pd.insert("confirmed_tx_hash".into(), Dynamic::from("0xdead"));
    pd.insert("source_event".into(), Dynamic::from(source_event.to_string()));
    // reserves_post: 1000 WETH (18 dec) vs r1_usdc (6 dec) → price = r1_usdc/1000.
    pd.insert("reserves_post".into(), reserves("1000000000000000000000", r1_usdc));
    pd
}

/// Cartridge exports the 3 mandatory contract functions + lifecycle hooks.
#[test]
fn backrun_contract_complete() {
    let engine = Engine::new();
    let ast = engine.compile(BACKRUN).expect("backrun.rhai compiles");
    let fns: Vec<&str> = ast.iter_functions().map(|f| f.name).collect();
    for required in ["init_strategy", "evaluate_opportunity", "build_payload"] {
        assert!(fns.contains(&required), "backrun missing `{required}`");
    }
    for hook in ["on_activate", "on_deactivate", "on_new_block"] {
        assert!(fns.contains(&hook), "backrun missing hook `{hook}`");
    }
}

/// init_strategy() returns required metadata + the zero-victim ethics marker.
#[test]
fn backrun_metadata_declares_zero_victim() {
    let engine = backrun_engine();
    let ast = engine.compile(BACKRUN).unwrap();
    let mut scope = Scope::new();
    let meta: Map = engine
        .call_fn::<Dynamic>(&mut scope, &ast, "init_strategy", ())
        .unwrap()
        .cast::<Map>();
    for k in ["name", "version", "author", "description"] {
        assert!(meta.contains_key(k), "metadata missing `{k}`");
    }
    let ethics = meta.get("ethics").unwrap().clone().into_string().unwrap();
    assert_eq!(ethics, "non_extractive_zero_victim");
    let category = meta.get("category").unwrap().clone().into_string().unwrap();
    assert_eq!(category, "backrun");
}

/// ETHICS GUARD: a pending (non-confirmed) event is REFUSED — the cartridge never
/// acts ahead of a user's tx (that would be frontrun/sandwich, PROHIBITED).
#[test]
fn backrun_rejects_pending_event_zero_victim_guard() {
    let engine = backrun_engine();
    let r = eval(&engine, BACKRUN, "evaluate_opportunity",
        backrun_pool_data("pending", "100000000000000000000", "2100000000000"));
    assert_eq!(r.get("is_opportunity").unwrap().as_bool().unwrap(), false);
    assert_eq!(
        r.get("reason").unwrap().clone().into_string().unwrap(),
        "not_confirmed_zero_victim_guard"
    );
}

/// Confirmed post-state with a real 5% residual imbalance on a 100-WETH swap →
/// a profitable, positive-net backrun opportunity.
#[test]
fn backrun_confirmed_imbalance_is_opportunity() {
    let engine = backrun_engine();
    // r1=2,100,000 USDC vs 1000 WETH → price 2100 vs fair 2000 → 5% imbalance.
    let r = eval(&engine, BACKRUN, "evaluate_opportunity",
        backrun_pool_data("confirmed_block", "100000000000000000000", "2100000000000"));
    assert_eq!(r.get("is_opportunity").unwrap().as_bool().unwrap(), true,
        "5% imbalance on a 100-WETH swap must be an opportunity");
    assert_eq!(r.get("reason").unwrap().clone().into_string().unwrap(), "post_state_rebalance");
    assert!(r.get("net_profit").unwrap().as_float().unwrap() > 0.0);
    assert_eq!(r.get("target_tx_hash").unwrap().clone().into_string().unwrap(), "0xdead");
}

/// Confirmed but tiny imbalance on a tiny (0.1 WETH) swap → net below gas → no op.
#[test]
fn backrun_below_gas_threshold_no_opportunity() {
    let engine = backrun_engine();
    // r1=2,002,000 USDC → price 2002 vs fair 2000 → 0.1% imbalance, 0.1-WETH swap.
    let r = eval(&engine, BACKRUN, "evaluate_opportunity",
        backrun_pool_data("confirmed_block", "100000000000000000", "2002000000000"));
    assert_eq!(r.get("is_opportunity").unwrap().as_bool().unwrap(), false);
    assert_eq!(
        r.get("reason").unwrap().clone().into_string().unwrap(),
        "below_gas_threshold"
    );
}

/// build_payload() emits a backrun bundle that orders our tx AFTER the target,
/// carrying the zero-victim attestation.
#[test]
fn backrun_payload_is_backrun_bundle_zero_victim() {
    let engine = backrun_engine();
    let mut opp = Map::new();
    opp.insert("target_tx_hash".into(), Dynamic::from("0xdead"));
    opp.insert("net_profit".into(), Dynamic::from(2.5_f64));
    opp.insert("route_type".into(), Dynamic::from("backrun_rebalance"));
    let p = eval(&engine, BACKRUN, "build_payload", opp);

    let bundle = p.get("bundle").unwrap().clone().cast::<Map>();
    assert_eq!(bundle.get("bundle_type").unwrap().clone().into_string().unwrap(), "backrun");
    assert_eq!(bundle.get("ordering").unwrap().clone().into_string().unwrap(), "after_target");

    let risk = p.get("risk").unwrap().clone().cast::<Map>();
    assert_eq!(risk.get("zero_victim_attestation").unwrap().as_bool().unwrap(), true);
}

// ═════════════════════════════════════════════════════════════════════════════
// TRIANGULAR (3-token A→B→C→A) — triangular_arb.rhai
// ═════════════════════════════════════════════════════════════════════════════

const TRIANGULAR: &str = include_str!("../cartridges/triangular_arb.rhai");

/// Engine for triangular with symbol/address-aware pool + reserve stubs.
/// `profitable` toggles the C→A leg rate so the cycle either clears fees or not.
fn triangular_engine(profitable: bool) -> Engine {
    let mut engine = Engine::new();
    engine.set_max_expr_depths(64, 32); // mirror release limits (see e2e test)
    register_common(&mut engine);

    engine.register_fn("get_token_meta", |addr: &str| -> Dynamic {
        match addr {
            "0xa" => token_meta("WETH", 18),
            "0xb" => token_meta("DAI", 18),
            "0xc" => token_meta("USDC", 6),
            _ => Dynamic::UNIT,
        }
    });
    engine.register_fn("get_pool_index", |a: &str, b: &str| -> Dynamic {
        let pool = match (a, b) {
            ("WETH", "DAI") => Some("0xab"),
            ("DAI", "USDC") => Some("0xbc"),
            ("USDC", "WETH") => Some("0xca"),
            _ => None,
        };
        match pool {
            Some(p) => Dynamic::from_array(vec![Dynamic::from(p.to_string())]),
            None => Dynamic::from_array(vec![]),
        }
    });
    // C→A leg (0xca): r1=A reserve. 1.05e9 ⇒ +5% rate (clears 3×0.3% fees → profit);
    // 1.0e9 ⇒ flat (fees make the cycle unprofitable).
    let ca_r1 = if profitable { "1050000000" } else { "1000000000" };
    engine.register_fn("get_reserves", move |addr: &str| -> Dynamic {
        match addr {
            "0xab" => reserves("1000000000", "1000000000"),
            "0xbc" => reserves("1000000000", "1000000000"),
            "0xca" => reserves("1000000000", ca_r1),
            _ => Dynamic::UNIT,
        }
    });
    engine
}

fn triangle_pool_data() -> Map {
    let mut pd = Map::new();
    pd.insert("chain_id".into(), Dynamic::from(1_i64));
    pd.insert("token_a".into(), Dynamic::from("0xa"));
    pd.insert("token_b".into(), Dynamic::from("0xb"));
    pd.insert("token_c".into(), Dynamic::from("0xc"));
    pd.insert("amount_in".into(), Dynamic::from("1000"));
    pd.insert("gas_price_gwei".into(), Dynamic::from(30.0_f64));
    pd
}

/// Cartridge exports the 3 mandatory contract functions.
#[test]
fn triangular_contract_complete() {
    let mut engine = Engine::new();
    engine.set_max_expr_depths(64, 32);
    let ast = engine.compile(TRIANGULAR).expect("triangular compiles");
    let fns: Vec<&str> = ast.iter_functions().map(|f| f.name).collect();
    for required in ["init_strategy", "evaluate_opportunity", "build_payload"] {
        assert!(fns.contains(&required), "triangular missing `{required}`");
    }
}

/// A genuinely mispriced 3-token cycle (C→A leg +5%) clears 3×0.3% fees and
/// surfaces as a profitable triangular opportunity with all three legs set.
#[test]
fn triangular_profitable_cycle_is_opportunity() {
    let engine = triangular_engine(true);
    let ast = engine.compile(TRIANGULAR).unwrap();
    let mut scope = Scope::new();
    let r: Map = engine
        .call_fn::<Dynamic>(&mut scope, &ast, "evaluate_opportunity", (triangle_pool_data(),))
        .unwrap()
        .cast::<Map>();

    assert_eq!(r.get("is_opportunity").unwrap().as_bool().unwrap(), true,
        "a +5% cross-rate cycle must beat 3x0.3% fees");
    assert_eq!(r.get("route_type").unwrap().clone().into_string().unwrap(), "three_leg_arb");
    assert!(r.get("estimated_profit").unwrap().as_float().unwrap() > 0.0);
    // All three legs of the triangle are populated.
    assert_eq!(r.get("pool_ab").unwrap().clone().into_string().unwrap(), "0xab");
    assert_eq!(r.get("pool_bc").unwrap().clone().into_string().unwrap(), "0xbc");
    assert_eq!(r.get("pool_ca").unwrap().clone().into_string().unwrap(), "0xca");
}

/// A flat cycle (all 1:1 pools) cannot overcome 3×0.3% fees → no opportunity.
#[test]
fn triangular_flat_cycle_no_opportunity() {
    let engine = triangular_engine(false);
    let ast = engine.compile(TRIANGULAR).unwrap();
    let mut scope = Scope::new();
    let r: Map = engine
        .call_fn::<Dynamic>(&mut scope, &ast, "evaluate_opportunity", (triangle_pool_data(),))
        .unwrap()
        .cast::<Map>();
    assert_eq!(r.get("is_opportunity").unwrap().as_bool().unwrap(), false,
        "a flat 1:1 cycle loses to fees");
    let reason = r.get("reason").unwrap().clone().into_string().unwrap();
    assert!(
        reason == "no_profitable_cycle" || reason == "below_gas_threshold",
        "unexpected reason: {reason}"
    );
}

/// build_payload() for triangular emits a 3-leg route.
#[test]
fn triangular_payload_has_three_legs() {
    let engine = triangular_engine(true);
    let ast = engine.compile(TRIANGULAR).unwrap();
    let mut scope = Scope::new();
    let mut opp = Map::new();
    opp.insert("pool_ab".into(), Dynamic::from("0xab"));
    opp.insert("pool_bc".into(), Dynamic::from("0xbc"));
    opp.insert("pool_ca".into(), Dynamic::from("0xca"));
    opp.insert("token_a".into(), Dynamic::from("0xa"));
    opp.insert("token_b".into(), Dynamic::from("0xb"));
    opp.insert("token_c".into(), Dynamic::from("0xc"));
    opp.insert("estimated_profit".into(), Dynamic::from(40.0_f64));
    let p: Map = engine
        .call_fn::<Dynamic>(&mut scope, &ast, "build_payload", (opp,))
        .unwrap()
        .cast::<Map>();
    let route = p.get("route").unwrap().clone().cast::<Map>();
    let legs = route.get("legs").unwrap().clone().into_array().unwrap();
    assert_eq!(legs.len(), 3, "triangular route must have exactly 3 legs");
}
