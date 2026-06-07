//! Real evaluation tests for the polymorphic omega_strategy_pack cartridge.
//!
//! The pack is PURE (no host bindings): every evaluator works off the `pool_data`
//! map that route_discovery hands it. These tests run the real dispatch + several
//! strategy evaluators and assert the decisions, satisfying the Definition-of-Done
//! "evaluate_opportunity test" for the strategy pack. Shadow-only; capital=0.

use rhai::{Dynamic, Engine, Map, Scope};

const PACK: &str = include_str!("../cartridges/omega_strategy_pack.rhai");

fn engine() -> Engine {
    let mut e = Engine::new();
    e.set_max_operations(2_000_000);
    e
}

fn run(func: &str, arg: Map) -> Map {
    let e = engine();
    let ast = e.compile(PACK).expect("omega_strategy_pack.rhai compiles");
    let mut scope = Scope::new();
    e.call_fn::<Dynamic>(&mut scope, &ast, func, (arg,))
        .expect("function runs")
        .cast::<Map>()
}

fn num(m: &mut Map, k: &str, v: f64) {
    m.insert(k.into(), Dynamic::from(v));
}
fn int(m: &mut Map, k: &str, v: i64) {
    m.insert(k.into(), Dynamic::from(v));
}
fn s(m: &mut Map, k: &str, v: &str) {
    m.insert(k.into(), Dynamic::from(v.to_string()));
}
fn boolean(m: &mut Map, k: &str, v: bool) {
    m.insert(k.into(), Dynamic::from(v));
}

fn is_opp(r: &Map) -> bool {
    r.get("is_opportunity").unwrap().as_bool().unwrap()
}
fn reason(r: &Map) -> String {
    r.get("reason").unwrap().clone().into_string().unwrap()
}
fn kind(r: &Map) -> String {
    r.get("strategy_kind")
        .unwrap()
        .clone()
        .into_string()
        .unwrap()
}

// ─── Contract + metadata ──────────────────────────────────────────────────────

#[test]
fn omega_contract_complete() {
    let e = Engine::new();
    let ast = e.compile(PACK).expect("pack compiles");
    let fns: Vec<&str> = ast.iter_functions().map(|f| f.name).collect();
    for required in ["init_strategy", "evaluate_opportunity", "build_payload"] {
        assert!(fns.contains(&required), "pack missing `{required}`");
    }
}

#[test]
fn omega_metadata_shadow_zero_capital() {
    let e = engine();
    let ast = e.compile(PACK).unwrap();
    let mut scope = Scope::new();
    let meta: Map = e
        .call_fn::<Dynamic>(&mut scope, &ast, "init_strategy", ())
        .unwrap()
        .cast::<Map>();
    for k in ["name", "version", "author", "description"] {
        assert!(meta.contains_key(k), "metadata missing `{k}`");
    }
    assert_eq!(
        meta.get("mode").unwrap().clone().into_string().unwrap(),
        "shadow"
    );
    assert_eq!(meta.get("capital_exposed").unwrap().as_int().unwrap(), 0);
}

// ─── Dispatch guards ──────────────────────────────────────────────────────────

#[test]
fn omega_missing_strategy_kind_rejected() {
    let r = run("evaluate_opportunity", Map::new());
    assert!(!is_opp(&r));
    assert_eq!(reason(&r), "missing_strategy_kind");
}

#[test]
fn omega_unsupported_strategy_kind_rejected() {
    let mut d = Map::new();
    s(&mut d, "strategy_kind", "definitely_not_a_real_strategy");
    let r = run("evaluate_opportunity", d);
    assert!(!is_opp(&r));
    assert_eq!(reason(&r), "unsupported_strategy_kind");
}

// ─── GOLD strategies ──────────────────────────────────────────────────────────

#[test]
fn omega_spatial_cross_dex_profitable() {
    let mut d = Map::new();
    s(&mut d, "strategy_kind", "spatial_cross_dex");
    num(&mut d, "buy_quote_usd", 1000.0);
    num(&mut d, "sell_quote_usd", 1010.0);
    num(&mut d, "gas_cost_usd", 1.5);
    num(&mut d, "dex_fees_usd", 0.6);
    num(&mut d, "slippage_cost_usd", 0.9);
    let r = run("evaluate_opportunity", d);
    assert!(
        is_opp(&r),
        "10 USD gross minus ~3 USD costs must clear threshold"
    );
    assert_eq!(kind(&r), "spatial_cross_dex");
    assert_eq!(reason(&r), "spatial_cross_dex_net_positive");
}

#[test]
fn omega_spatial_below_threshold_rejected() {
    let mut d = Map::new();
    s(&mut d, "strategy_kind", "spatial_cross_dex");
    num(&mut d, "buy_quote_usd", 1000.0);
    num(&mut d, "sell_quote_usd", 1000.2); // 0.2 gross, below 0.5 default
    let r = run("evaluate_opportunity", d);
    assert!(!is_opp(&r));
    assert_eq!(reason(&r), "spatial_net_profit_below_threshold");
}

#[test]
fn omega_triangular_cross_dex_profitable() {
    let mut d = Map::new();
    s(&mut d, "strategy_kind", "triangular_cross_dex");
    d.insert(
        "route_legs".into(),
        Dynamic::from_array(vec![
            Dynamic::from("leg1".to_string()),
            Dynamic::from("leg2".to_string()),
            Dynamic::from("leg3".to_string()),
        ]),
    );
    int(&mut d, "leg_count", 3);
    num(&mut d, "amount_in_usd", 1000.0);
    num(&mut d, "amount_out_usd", 1009.0);
    num(&mut d, "gas_cost_usd", 1.2);
    num(&mut d, "dex_fees_usd", 0.8);
    num(&mut d, "slippage_cost_usd", 0.6);
    let r = run("evaluate_opportunity", d);
    assert!(is_opp(&r));
    assert_eq!(kind(&r), "triangular_cross_dex");
}

#[test]
fn omega_triangular_cross_dex_wrong_leg_count_rejected() {
    let mut d = Map::new();
    s(&mut d, "strategy_kind", "triangular_cross_dex");
    d.insert("route_legs".into(), Dynamic::from_array(vec![]));
    int(&mut d, "leg_count", 4);
    let r = run("evaluate_opportunity", d);
    assert!(!is_opp(&r));
    assert_eq!(reason(&r), "triangular_cross_dex_requires_3_legs");
}

#[test]
fn omega_stable_depeg_profitable() {
    let mut d = Map::new();
    s(&mut d, "strategy_kind", "stable_depeg");
    num(&mut d, "token_price_usd", 0.985); // 1.5c off peg
    num(&mut d, "target_peg_usd", 1.0);
    num(&mut d, "amount_in_usd", 1000.0);
    num(&mut d, "amount_out_usd", 1004.0);
    num(&mut d, "gas_cost_usd", 1.0);
    let r = run("evaluate_opportunity", d);
    assert!(is_opp(&r));
    assert_eq!(kind(&r), "stable_depeg");
}

// ─── Backrun POST-BLOCK (no mempool, zero-victim) ──────────────────────────────

#[test]
fn omega_post_block_backrun_confirmed_profitable() {
    let mut d = Map::new();
    s(&mut d, "strategy_kind", "post_block_residual_backrun");
    s(&mut d, "source_event", "new_block");
    num(&mut d, "pre_state_price", 3500.0);
    num(&mut d, "post_state_price", 3512.0);
    num(&mut d, "reference_price", 3502.0);
    num(&mut d, "amount_in_usd", 1000.0);
    num(&mut d, "amount_out_usd", 1005.6);
    num(&mut d, "gas_cost_usd", 1.1);
    num(&mut d, "dex_fees_usd", 0.4);
    num(&mut d, "slippage_cost_usd", 0.5);
    num(&mut d, "mev_competition_penalty_usd", 0.6);
    let r = run("evaluate_opportunity", d);
    assert!(is_opp(&r));
    assert_eq!(kind(&r), "post_block_residual_backrun");
    assert_eq!(reason(&r), "post_block_residual_imbalance_positive");
}

/// ETHICS: a pending-tx event is REFUSED — this surface is post-block only.
#[test]
fn omega_post_block_backrun_rejects_pending_tx() {
    let mut d = Map::new();
    s(&mut d, "strategy_kind", "post_block_residual_backrun");
    s(&mut d, "source_event", "new_block");
    boolean(&mut d, "pending_tx", true);
    num(&mut d, "pre_state_price", 3500.0);
    num(&mut d, "post_state_price", 3512.0);
    let r = run("evaluate_opportunity", d);
    assert!(!is_opp(&r));
    assert_eq!(
        reason(&r),
        "pending_tx_not_supported_by_current_cartridge_surface"
    );
}

// ─── Oracle divergence = SIGNAL ONLY (never executes) ──────────────────────────

#[test]
fn omega_oracle_divergence_is_signal_only() {
    let mut d = Map::new();
    s(&mut d, "strategy_kind", "oracle_divergence_signal");
    num(&mut d, "oracle_price", 3500.0);
    num(&mut d, "dex_price", 3480.0); // ~0.57% divergence
    let r = run("evaluate_opportunity", d);
    assert!(
        !is_opp(&r),
        "oracle divergence is a signal, never an executable opportunity"
    );
    assert_eq!(reason(&r), "oracle_divergence_signal_only_no_execution");
}

// ─── build_payload is always shadow/observe-only ───────────────────────────────

#[test]
fn omega_payload_is_shadow_observe_only() {
    let mut opp = Map::new();
    s(&mut opp, "strategy_kind", "spatial_cross_dex");
    let p = run("build_payload", opp);
    assert_eq!(
        p.get("mode").unwrap().clone().into_string().unwrap(),
        "shadow"
    );
    assert_eq!(
        p.get("action").unwrap().clone().into_string().unwrap(),
        "observe_only"
    );
    assert!(!p.get("can_execute").unwrap().as_bool().unwrap());
    assert_eq!(p.get("capital_exposed").unwrap().as_int().unwrap(), 0);
}
