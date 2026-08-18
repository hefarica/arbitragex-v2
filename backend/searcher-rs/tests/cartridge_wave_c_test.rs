//! RU-5 wave C functional tests — G05+G06+G07 cartridges with real hunting math.
//!
//! Same style as `cartridge_wave_b_test.rs`: RUNS `init_strategy()` /
//! `evaluate_opportunity()` / `build_payload()` for the 74 wave-C cartridges
//! against hand-built `pool_data` with deterministic stub host bindings (test
//! fixtures, NOT production data paths — RULE 00 holds: the cartridges
//! themselves never fabricate numbers).
//!
//! Wave C families (all PAPER per the manifest — emit, never execute):
//!   G05 cex_external_engine (14): CEX/order-book anchor vs the executable
//!      on-chain route (exact CPMM + golden-section sizing). Gate when the
//!      feed is absent: `cex_feed_unavailable`.
//!   G06 cross_domain_engine (30): two-domain composition over the documented
//!      bridge_state payload (src pool → bridge → dst pool, USD FX, settlement
//!      haircut). Gate: `bridge_state_unavailable`.
//!   G07 derivatives_engine (30): basis / funding / options parity / surface /
//!      settlement math over the derivatives payload + live spot leg. Gate:
//!      `funding_feed_unavailable`.
//!
//! Covered behaviors:
//!   1. Empty `pool_data` => fail-honest no-op with the exact machine-readable
//!      reason per family (all 74).
//!   2. G05: spot spread both directions, triangular/multi-DEX shape gates,
//!      multi-venue best-bid, order-book VWAP, futures/perp carry, latency
//!      bands, OTC/MM/custodian/fiat-stable variants.
//!   3. G06: cross-domain spread hunts, domain shape gates (kinds/vm/
//!      consensus/canonical/fast/atomic), bridge-rate attribution, liquidity
//!      and inventory caps, intent bounty, liquidation reward, oracle band.
//!   4. G07: spot-perp basis, peer venue basis + funding differential, funding
//!      carry horizon gates, put-call parity, conversion, box, butterfly
//!      surface, calendar, IV venue spread, vAMM two-pool, settlement,
//!      liquidation, NAV parity, leverage rebalance.
//!   5. Payload sweep: every one of the 74 evaluates a rich union payload
//!      without a runtime error (catches unit-method bugs in any branch).
//!
//! Run: cargo test -p searcher-rs --test cartridge_wave_c_test -- --nocapture

use rhai::{Dynamic, Engine, Map};
use std::collections::HashMap;

fn cartridge(id: &str) -> String {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("cartridges/strategies")
        .join(format!("{id}.rhai"));
    std::fs::read_to_string(&path).unwrap()
}

/// Common host-binding stubs (mirror production signatures).
fn register_common(engine: &mut Engine) {
    engine.set_max_expr_depths(64, 32);
    engine.set_max_operations(2_000_000);
    engine.register_fn("get_chain_id", || -> Dynamic { Dynamic::from(1_i64) });
    engine.register_fn("get_block_number", || -> Dynamic {
        Dynamic::from(20_000_000_i64)
    });
    engine.register_fn("get_base_fee", || -> Dynamic { Dynamic::from(30.0_f64) });
    engine.register_fn("get_timestamp", || -> Dynamic {
        Dynamic::from(1_717_200_000_i64)
    });
    engine.register_fn("math_abs", |x: f64| -> Dynamic { Dynamic::from(x.abs()) });
    engine.register_fn("math_min", |a: f64, b: f64| -> Dynamic {
        Dynamic::from(a.min(b))
    });
    engine.register_fn("math_max", |a: f64, b: f64| -> Dynamic {
        Dynamic::from(a.max(b))
    });
    engine.register_fn("math_pow", |b: f64, e: f64| -> Dynamic {
        Dynamic::from(b.powf(e))
    });
    engine.register_fn("math_log", |x: f64| -> Dynamic { Dynamic::from(x.ln()) });
    engine.register_fn("math_exp", |x: f64| -> Dynamic { Dynamic::from(x.exp()) });
    engine.register_fn("math_sqrt", |x: f64| -> Dynamic { Dynamic::from(x.sqrt()) });
    engine.register_fn("to_float", |s: &str| -> Dynamic {
        Dynamic::from(s.parse::<f64>().unwrap_or(0.0))
    });
    engine.register_fn("from_wei", |wei: &str, dec: i64| -> Dynamic {
        let v: f64 = wei.parse().unwrap_or(0.0);
        Dynamic::from(v / 10f64.powi(dec as i32))
    });
}

/// Token universe: 0xa TKA(18,$2000) 0xb TKB(18,$2000) 0xc TKC(18,$2000)
/// 0xs1 USDX(18,stable,$1) 0xs2 USDY(18,stable,$1).
fn register_tokens(engine: &mut Engine) {
    let meta: HashMap<String, (&'static str, i64, bool)> = [
        ("0xa", ("TKA", 18, false)),
        ("0xb", ("TKB", 18, false)),
        ("0xc", ("TKC", 18, false)),
        ("0xs1", ("USDX", 18, true)),
        ("0xs2", ("USDY", 18, true)),
    ]
    .into_iter()
    .map(|(k, v)| (k.to_string(), v))
    .collect();
    engine.register_fn("get_token_meta", move |x: &str| -> Dynamic {
        match meta.get(x) {
            Some((sym, dec, stable)) => {
                let mut m = Map::new();
                m.insert("symbol".into(), Dynamic::from((*sym).to_string()));
                m.insert("decimals".into(), Dynamic::from(*dec));
                m.insert("is_stablecoin".into(), Dynamic::from(*stable));
                Dynamic::from_map(m)
            }
            None => Dynamic::UNIT,
        }
    });
    let prices: HashMap<String, f64> = [
        ("TKA", 2000.0),
        ("TKB", 2000.0),
        ("TKC", 2000.0),
        ("USDX", 1.0),
        ("USDY", 1.0),
    ]
    .into_iter()
    .map(|(k, v)| (k.to_string(), v))
    .collect();
    engine.register_fn("get_token_price_usd", move |sym: &str| -> Dynamic {
        match prices.get(sym) {
            Some(p) => Dynamic::from(*p),
            None => Dynamic::UNIT,
        }
    });
}

/// Pool reserves universe (pool -> (r0, r1)); all synced at block 20_000_000.
///   0xab1 A/B 1:1          0xba2 1:1.21 mirror   0xbc3 B/C 1:1
///   0xca4 C/A 1:1.21       0xss1 USDX/USDY 1:1.01
///   0xva1 vAMM A/B 1:1.21  0xva2 vAMM A/B 1:0.9
fn register_pools(engine: &mut Engine) {
    let pools: HashMap<String, (String, String)> = [
        (
            "0xab1",
            ("1000000000000000000000", "1000000000000000000000"),
        ),
        (
            "0xba2",
            ("1000000000000000000000", "1210000000000000000000"),
        ),
        (
            "0xbc3",
            ("1000000000000000000000", "1000000000000000000000"),
        ),
        (
            "0xca4",
            ("1000000000000000000000", "1210000000000000000000"),
        ),
        (
            "0xss1",
            ("1000000000000000000000", "1010000000000000000000"),
        ),
        (
            "0xva1",
            ("1000000000000000000000", "1210000000000000000000"),
        ),
        ("0xva2", ("1000000000000000000000", "900000000000000000000")),
    ]
    .into_iter()
    .map(|(k, (r0, r1))| (k.to_string(), (r0.to_string(), r1.to_string())))
    .collect();
    engine.register_fn("get_reserves", move |x: &str| -> Dynamic {
        match pools.get(x) {
            Some((r0, r1)) => {
                let mut m = Map::new();
                m.insert("r0".into(), Dynamic::from(r0.clone()));
                m.insert("r1".into(), Dynamic::from(r1.clone()));
                m.insert("block".into(), Dynamic::from(20_000_000_i64));
                m.insert("ts".into(), Dynamic::from(1_717_200_000_i64));
                Dynamic::from_map(m)
            }
            None => Dynamic::UNIT,
        }
    });
    engine.register_fn("get_math_evidence", |_s: &str| -> Dynamic { Dynamic::UNIT });
}

fn math_engine() -> Engine {
    let mut engine = Engine::new();
    register_common(&mut engine);
    register_tokens(&mut engine);
    register_pools(&mut engine);
    engine
}

fn eval(engine: &Engine, src: &str, arg: Map) -> Map {
    let ast = engine.compile(src).expect("cartridge compiles");
    let mut scope = rhai::Scope::new();
    let out: Dynamic = engine
        .call_fn(&mut scope, &ast, "evaluate_opportunity", (arg,))
        .expect("evaluate_opportunity runs without runtime error");
    out.cast::<Map>()
}

fn reason(m: &Map) -> String {
    m.get("reason").unwrap().clone().into_string().unwrap()
}

fn is_opp(m: &Map) -> bool {
    m.get("is_opportunity").unwrap().as_bool().unwrap()
}

fn leg(pool: &str, token_in: &str, token_out: &str) -> Map {
    let mut m = Map::new();
    m.insert("pool".into(), Dynamic::from(pool.to_string()));
    m.insert("token_in".into(), Dynamic::from(token_in.to_string()));
    m.insert("token_out".into(), Dynamic::from(token_out.to_string()));
    m.insert("protocol_type".into(), Dynamic::from("v2".to_string()));
    m.insert("fee_bps".into(), Dynamic::from(30_i64));
    m
}

fn route_pool_data(legs: &[(&str, &str, &str)]) -> Map {
    let mut pd = Map::new();
    pd.insert("chain_id".into(), Dynamic::from(1_i64));
    let arr: Vec<Dynamic> = legs
        .iter()
        .map(|(p, a, b)| Dynamic::from(leg(p, a, b)))
        .collect();
    pd.insert("route".into(), Dynamic::from_array(arr));
    pd
}

fn s(v: &str) -> Dynamic {
    Dynamic::from(v.to_string())
}

fn f(v: f64) -> Dynamic {
    Dynamic::from(v)
}

fn i(v: i64) -> Dynamic {
    Dynamic::from(v)
}

/// Rich CEX book payload (bid 1.15 / ask 0.85 around a ~0.997 pool marginal).
fn cex_payload(base: &str, quote: &str, bid: f64, ask: f64) -> Map {
    let mut m = Map::new();
    m.insert("venue".into(), s("test_cex"));
    m.insert("base_token".into(), s(base));
    m.insert("quote_token".into(), s(quote));
    m.insert("bid_px".into(), f(bid));
    m.insert("bid_depth".into(), f(50.0));
    m.insert("ask_px".into(), f(ask));
    m.insert("ask_depth".into(), f(50.0));
    m.insert("fee_bps".into(), i(10));
    m.insert("ts".into(), i(1_717_200_000));
    m
}

/// Bridge-state payload: src pool A/B 1:1, dst pool A/B 1:1.21 (both base=0xa).
fn bridge_payload() -> Map {
    let mut m = Map::new();
    m.insert("src_chain".into(), i(1));
    m.insert("dst_chain".into(), i(2));
    m.insert("src_token".into(), s("0xa"));
    m.insert("src_quote_token".into(), s("0xb"));
    m.insert("src_pool".into(), s("0xab1"));
    m.insert("src_r0".into(), s("1000000000000000000000"));
    m.insert("src_r1".into(), s("1000000000000000000000"));
    m.insert("src_token0".into(), s("0xa"));
    m.insert("src_fee_bps".into(), i(30));
    m.insert("dst_token".into(), s("0xa"));
    m.insert("dst_quote_token".into(), s("0xb"));
    m.insert("dst_pool".into(), s("0xdst1"));
    m.insert("dst_r0".into(), s("1000000000000000000000"));
    m.insert("dst_r1".into(), s("1210000000000000000000"));
    m.insert("dst_token0".into(), s("0xa"));
    m.insert("dst_fee_bps".into(), i(30));
    m.insert("risk_discount".into(), f(0.0));
    m.insert("ts".into(), i(1_717_200_000));
    m
}

/// Derivatives payload skeleton (mark vs the 0xab1 ~0.997 pool marginal).
fn deriv_payload(kind: &str, mark: f64) -> Map {
    let mut m = Map::new();
    m.insert("kind".into(), s(kind));
    m.insert("venue".into(), s("test_venue"));
    m.insert("base_token".into(), s("0xa"));
    m.insert("quote_token".into(), s("0xb"));
    m.insert("mark_px".into(), f(mark));
    m.insert("fee_bps".into(), i(10));
    m.insert("ts".into(), i(1_717_200_000));
    m
}

// ─────────────────────────────────────────────────────────────────────────────
// 1. Empty pool_data => fail-honest machine-readable reason (all 74)
// ─────────────────────────────────────────────────────────────────────────────

const G05: &[&str] = &[
    "mev_05_001_cex_dex_spot_arbitrage",
    "mev_05_002_dex_cex_spot_arbitrage",
    "mev_05_003_cex_dex_triangular_arbitrage",
    "mev_05_004_cex_multi_dex_arbitrage",
    "mev_05_005_multi_cex_dex_arbitrage",
    "mev_05_006_cex_order_book_amm_arbitrage",
    "mev_05_007_cex_futures_dex_spot_arbitrage",
    "mev_05_008_cex_perpetual_dex_spot_arbitrage",
    "mev_05_009_cex_price_lead_latency_arbitrage",
    "mev_05_010_dex_price_lead_arbitrage",
    "mev_05_011_otc_dex_arbitrage",
    "mev_05_012_market_maker_inventory_dex_arbitrage",
    "mev_05_013_cross_custodian_arbitrage",
    "mev_05_014_fiat_stablecoin_dex_arbitrage",
];

const G06: &[&str] = &[
    "mev_06_001_l1_l1_arbitrage",
    "mev_06_002_l1_l2_arbitrage",
    "mev_06_003_l2_l1_arbitrage",
    "mev_06_004_l2_l2_cross_rollup_arbitrage",
    "mev_06_005_rollup_sidechain_arbitrage",
    "mev_06_006_sidechain_sidechain_arbitrage",
    "mev_06_007_appchain_arbitrage",
    "mev_06_008_cross_vm_arbitrage",
    "mev_06_009_cross_consensus_arbitrage",
    "mev_06_010_cross_domain_dex_dex_arbitrage",
    "mev_06_011_cross_domain_triangular_arbitrage",
    "mev_06_012_cross_chain_cyclic_arbitrage",
    "mev_06_013_multi_chain_n_leg_arbitrage",
    "mev_06_014_canonical_bridge_arbitrage",
    "mev_06_015_fast_bridge_arbitrage",
    "mev_06_016_bridge_rate_arbitrage",
    "mev_06_017_bridge_liquidity_arbitrage",
    "mev_06_018_bridge_rebalancing_arbitrage",
    "mev_06_019_canonical_vs_fast_bridge_arbitrage",
    "mev_06_020_wrapped_representation_arbitrage",
    "mev_06_021_message_latency_arbitrage",
    "mev_06_022_finality_delay_arbitrage",
    "mev_06_023_pre_positioned_inventory_arbitrage",
    "mev_06_024_bridge_in_the_loop_arbitrage",
    "mev_06_025_cross_chain_intent_arbitrage",
    "mev_06_026_cross_chain_solver_arbitrage",
    "mev_06_027_shared_sequencer_arbitrage",
    "mev_06_028_sequencer_builder_cross_domain_arbitrage",
    "mev_06_029_cross_domain_oracle_arbitrage",
    "mev_06_030_cross_chain_liquidation_arbitrage",
];

const G07: &[&str] = &[
    "mev_07_001_spot_perpetual_arbitrage",
    "mev_07_002_perpetual_perpetual_arbitrage",
    "mev_07_003_spot_future_arbitrage",
    "mev_07_004_future_future_arbitrage",
    "mev_07_005_cash_and_carry_arbitrage",
    "mev_07_006_reverse_cash_and_carry_arbitrage",
    "mev_07_007_basis_arbitrage",
    "mev_07_008_funding_rate_arbitrage",
    "mev_07_009_cross_exchange_funding_arbitrage",
    "mev_07_010_calendar_spread_arbitrage",
    "mev_07_011_cross_maturity_futures_arbitrage",
    "mev_07_012_index_price_arbitrage",
    "mev_07_013_mark_price_arbitrage",
    "mev_07_014_oracle_perpetual_arbitrage",
    "mev_07_015_vamm_spot_arbitrage",
    "mev_07_016_options_put_call_parity_arbitrage",
    "mev_07_017_conversion_arbitrage",
    "mev_07_018_reversal_arbitrage",
    "mev_07_019_box_spread_arbitrage",
    "mev_07_020_cross_strike_arbitrage",
    "mev_07_021_cross_expiry_options_arbitrage",
    "mev_07_022_options_amm_order_book_arbitrage",
    "mev_07_023_cross_venue_implied_volatility_arbitrage",
    "mev_07_024_volatility_surface_inconsistency_arbitrage",
    "mev_07_025_delta_hedged_options_arbitrage",
    "mev_07_026_settlement_price_arbitrage",
    "mev_07_027_expiry_arbitrage",
    "mev_07_028_derivative_liquidation_arbitrage",
    "mev_07_029_structured_product_parity_arbitrage",
    "mev_07_030_leverage_token_rebalancing_arbitrage",
];

#[test]
fn wave_c_empty_data_fail_honest_reasons() {
    let engine = math_engine();
    let mut cases: Vec<(&str, &str)> = Vec::new();
    for id in G05 {
        cases.push((id, "cex_feed_unavailable"));
    }
    for id in G06 {
        cases.push((id, "bridge_state_unavailable"));
    }
    for id in G07 {
        cases.push((id, "funding_feed_unavailable"));
    }
    assert_eq!(cases.len(), 74, "wave C = 74 cartridges");
    for (file, expected) in &cases {
        let src = cartridge(file);
        let r = eval(&engine, &src, Map::new());
        assert!(!is_opp(&r), "{file} must not fabricate an opportunity");
        assert_eq!(reason(&r), *expected, "{file} empty-data reason");
        let det = r.get("detector_id").unwrap().clone().into_string().unwrap();
        assert!(!det.is_empty(), "{file} carries detector_id");
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// 2. G05 — CEX anchor vs executable on-chain route
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn g05_spot_spread_both_directions() {
    let engine = math_engine();

    // 001 CEX-DEX: pool 0xab1 marginal ~0.997 B per A, CEX bid 1.15 net.
    let mut pd = route_pool_data(&[("0xab1", "0xb", "0xa")]);
    pd.insert(
        "cex".into(),
        Dynamic::from(cex_payload("0xa", "0xb", 1.15, 0.85)),
    );
    let r = eval(
        &engine,
        &cartridge("mev_05_001_cex_dex_spot_arbitrage"),
        pd.clone(),
    );
    assert!(
        is_opp(&r),
        "001 must hunt the 15% CEX premium — got reason={}",
        reason(&r)
    );
    assert_eq!(reason(&r), "cex_dex_spot_spread_profit");
    let profit = r.get("estimated_profit").unwrap().as_float().unwrap();
    assert!(profit > 0.0, "001 profit positive, got {profit}");
    let x = r.get("optimal_amount_in").unwrap().as_float().unwrap();
    assert!(x > 0.0 && x < 100.0, "001 sizing sane: {x}");

    // No edge: CEX bid below the pool marginal.
    let mut flat = route_pool_data(&[("0xab1", "0xb", "0xa")]);
    flat.insert(
        "cex".into(),
        Dynamic::from(cex_payload("0xa", "0xb", 0.95, 0.85)),
    );
    let r2 = eval(
        &engine,
        &cartridge("mev_05_001_cex_dex_spot_arbitrage"),
        flat,
    );
    assert!(!is_opp(&r2));
    assert_eq!(reason(&r2), "no_cex_edge");

    // 002 DEX-CEX: sell into the 1:1.21 mirror pool, buy CEX at 0.85.
    let mut pd2 = route_pool_data(&[("0xba2", "0xa", "0xb")]);
    pd2.insert(
        "cex".into(),
        Dynamic::from(cex_payload("0xa", "0xb", 1.15, 0.85)),
    );
    let r3 = eval(
        &engine,
        &cartridge("mev_05_002_dex_cex_spot_arbitrage"),
        pd2,
    );
    assert!(
        is_opp(&r3),
        "002 must hunt the rich pool vs cheap CEX — got reason={}",
        reason(&r3)
    );
    assert_eq!(reason(&r3), "dex_cex_spot_spread_profit");
}

#[test]
fn g05_triangular_multi_dex_and_venue_gates() {
    let engine = math_engine();

    // 003 triangular identity: exactly 2 on-chain legs (3 economic with CEX).
    let mut one_leg = route_pool_data(&[("0xab1", "0xb", "0xa")]);
    one_leg.insert(
        "cex".into(),
        Dynamic::from(cex_payload("0xa", "0xb", 1.15, 0.85)),
    );
    let r = eval(
        &engine,
        &cartridge("mev_05_003_cex_dex_triangular_arbitrage"),
        one_leg,
    );
    assert!(!is_opp(&r));
    assert_eq!(reason(&r), "route_shape_out_of_bounds");

    let mut tri = route_pool_data(&[("0xbc3", "0xb", "0xc"), ("0xca4", "0xc", "0xa")]);
    tri.insert(
        "cex".into(),
        Dynamic::from(cex_payload("0xa", "0xb", 1.15, 0.85)),
    );
    let r2 = eval(
        &engine,
        &cartridge("mev_05_003_cex_dex_triangular_arbitrage"),
        tri.clone(),
    );
    assert!(
        is_opp(&r2),
        "003 hunts the 2-leg DEX triangle + CEX exit — got reason={}",
        reason(&r2)
    );
    assert_eq!(reason(&r2), "cex_dex_triangular_profit");

    // 004 multi-DEX: >= 2 distinct pools.
    let r3 = eval(
        &engine,
        &cartridge("mev_05_004_cex_multi_dex_arbitrage"),
        tri.clone(),
    );
    assert!(
        is_opp(&r3),
        "004 hunts a 2-distinct-pool route — got reason={}",
        reason(&r3)
    );
    assert_eq!(reason(&r3), "cex_multi_dex_spread_profit");

    // 005 multi-venue: best executable bid across venues wins.
    let v1 = cex_payload("0xa", "0xb", 1.05, 0.95);
    let v2 = cex_payload("0xa", "0xb", 1.20, 0.95);
    let mut mv = route_pool_data(&[("0xab1", "0xb", "0xa")]);
    mv.insert(
        "cex_venues".into(),
        Dynamic::from_array(vec![Dynamic::from(v1), Dynamic::from(v2)]),
    );
    let r4 = eval(
        &engine,
        &cartridge("mev_05_005_multi_cex_dex_arbitrage"),
        mv,
    );
    assert!(
        is_opp(&r4),
        "005 picks the best venue bid — got reason={}",
        reason(&r4)
    );
    assert_eq!(reason(&r4), "multi_cex_best_bid_spread_profit");
}

#[test]
fn g05_order_book_vwap_walks_levels() {
    let engine = math_engine();
    let mut cex = cex_payload("0xa", "0xb", 1.15, 0.85);
    cex.insert(
        "bid_levels".into(),
        Dynamic::from_array(vec![
            Dynamic::from_array(vec![f(1.15), f(20.0)]),
            Dynamic::from_array(vec![f(1.05), f(30.0)]),
        ]),
    );
    let mut pd = route_pool_data(&[("0xab1", "0xb", "0xa")]);
    pd.insert("cex".into(), Dynamic::from(cex));
    let r = eval(
        &engine,
        &cartridge("mev_05_006_cex_order_book_amm_arbitrage"),
        pd,
    );
    assert!(
        is_opp(&r),
        "006 walks the firm level book — got reason={}",
        reason(&r)
    );
    assert_eq!(reason(&r), "order_book_amm_vwap_profit");
}

#[test]
fn g05_derivatives_carry_hunts() {
    let engine = math_engine();

    // 007 futures: mark 1.30 vs pool ~0.997, 30d expiry.
    let mut fut = cex_payload("0xa", "0xb", 1.30, 0.90);
    fut.insert("mark_px".into(), f(1.30));
    fut.insert("expiry_ts".into(), i(1_717_200_000 + 30 * 86400));
    fut.insert("carry_rate_annual".into(), f(0.0));
    let mut pd = route_pool_data(&[("0xab1", "0xb", "0xa")]);
    pd.insert("cex".into(), Dynamic::from(fut.clone()));
    let r = eval(
        &engine,
        &cartridge("mev_05_007_cex_futures_dex_spot_arbitrage"),
        pd.clone(),
    );
    assert!(
        is_opp(&r),
        "007 shorts the rich future vs cheap spot — got reason={}",
        reason(&r)
    );
    assert_eq!(reason(&r), "cex_future_dex_basis_profit");

    // Expired future fails honestly.
    let mut exp = fut.clone();
    exp.insert("expiry_ts".into(), i(1_717_200_000 - 10));
    let mut pdx = route_pool_data(&[("0xab1", "0xb", "0xa")]);
    pdx.insert("cex".into(), Dynamic::from(exp));
    let r2 = eval(
        &engine,
        &cartridge("mev_05_007_cex_futures_dex_spot_arbitrage"),
        pdx,
    );
    assert!(!is_opp(&r2));
    assert_eq!(reason(&r2), "contract_expired");

    // 008 perp: funding receipt adds to the short carry.
    let mut perp = cex_payload("0xa", "0xb", 1.20, 0.90);
    perp.insert("mark_px".into(), f(1.20));
    perp.insert("funding_rate".into(), f(0.001));
    perp.insert("funding_interval_sec".into(), i(28800));
    perp.insert("horizon_sec".into(), i(86400));
    let mut pd2 = route_pool_data(&[("0xab1", "0xb", "0xa")]);
    pd2.insert("cex".into(), Dynamic::from(perp));
    let r3 = eval(
        &engine,
        &cartridge("mev_05_008_cex_perpetual_dex_spot_arbitrage"),
        pd2.clone(),
    );
    assert!(
        is_opp(&r3),
        "008 hunts perp carry with funding — got reason={}",
        reason(&r3)
    );
    assert_eq!(reason(&r3), "cex_perp_funding_carry_profit");

    // Perp without the funding component fails honestly.
    let mut nof = cex_payload("0xa", "0xb", 1.20, 0.90);
    nof.insert("mark_px".into(), f(1.20));
    let mut pd3 = route_pool_data(&[("0xab1", "0xb", "0xa")]);
    pd3.insert("cex".into(), Dynamic::from(nof));
    let r4 = eval(
        &engine,
        &cartridge("mev_05_008_cex_perpetual_dex_spot_arbitrage"),
        pd3,
    );
    assert!(!is_opp(&r4));
    assert_eq!(reason(&r4), "funding_component_missing");
}

#[test]
fn g05_latency_band_gates() {
    let engine = math_engine();

    // 009: within band — no divergence worth chasing.
    let mut flat = route_pool_data(&[("0xab1", "0xb", "0xa")]);
    flat.insert(
        "cex".into(),
        Dynamic::from(cex_payload("0xa", "0xb", 1.005, 0.995)),
    );
    let r = eval(
        &engine,
        &cartridge("mev_05_009_cex_price_lead_latency_arbitrage"),
        flat,
    );
    assert!(!is_opp(&r));
    assert_eq!(reason(&r), "latency_band_not_breached");

    // 009: CEX bid leads far above the pool -> buy on-chain, sell CEX.
    let mut lead = route_pool_data(&[("0xab1", "0xb", "0xa")]);
    lead.insert(
        "cex".into(),
        Dynamic::from(cex_payload("0xa", "0xb", 1.15, 0.99)),
    );
    let r2 = eval(
        &engine,
        &cartridge("mev_05_009_cex_price_lead_latency_arbitrage"),
        lead,
    );
    assert!(is_opp(&r2));
    assert_eq!(reason(&r2), "cex_lead_latency_profit");

    // 010: pool-side lead, cheap CEX ask -> sell-side route hunts.
    let mut lead2 = route_pool_data(&[("0xab1", "0xa", "0xb")]);
    lead2.insert(
        "cex".into(),
        Dynamic::from(cex_payload("0xa", "0xb", 1.005, 0.85)),
    );
    let r3 = eval(
        &engine,
        &cartridge("mev_05_010_dex_price_lead_arbitrage"),
        lead2,
    );
    assert!(
        is_opp(&r3),
        "010 hunts the pool-rich divergence — got reason={}",
        reason(&r3)
    );
    assert_eq!(reason(&r3), "dex_lead_latency_profit");
}

#[test]
fn g05_otc_mm_custodian_fiat_variants() {
    let engine = math_engine();

    // 011 OTC desk ask 0.85, sell into the 1:1.21 mirror pool.
    let mut otc = route_pool_data(&[("0xba2", "0xa", "0xb")]);
    otc.insert(
        "cex".into(),
        Dynamic::from(cex_payload("0xa", "0xb", 1.15, 0.85)),
    );
    let r = eval(&engine, &cartridge("mev_05_011_otc_dex_arbitrage"), otc);
    assert!(is_opp(&r));
    assert_eq!(reason(&r), "otc_dex_desk_spread_profit");

    // 012 MM inventory: buy-side route takes the bid side.
    let mut mm = route_pool_data(&[("0xab1", "0xb", "0xa")]);
    mm.insert(
        "cex".into(),
        Dynamic::from(cex_payload("0xa", "0xb", 1.15, 0.85)),
    );
    let r2 = eval(
        &engine,
        &cartridge("mev_05_012_market_maker_inventory_dex_arbitrage"),
        mm.clone(),
    );
    assert!(is_opp(&r2));
    assert_eq!(reason(&r2), "mm_inventory_buy_dex_profit");

    // 012 sell-side route takes the ask side.
    let mut mm2 = route_pool_data(&[("0xba2", "0xa", "0xb")]);
    mm2.insert(
        "cex".into(),
        Dynamic::from(cex_payload("0xa", "0xb", 1.15, 0.85)),
    );
    let r3 = eval(
        &engine,
        &cartridge("mev_05_012_market_maker_inventory_dex_arbitrage"),
        mm2,
    );
    assert!(is_opp(&r3));
    assert_eq!(reason(&r3), "mm_inventory_sell_dex_profit");

    // 013 cross-custodian: needs >= 2 venues, picks the cheapest ask.
    let v1 = cex_payload("0xa", "0xb", 1.15, 0.90);
    let v2 = cex_payload("0xa", "0xb", 1.15, 0.85);
    let mut cc = route_pool_data(&[("0xba2", "0xa", "0xb")]);
    cc.insert(
        "cex_venues".into(),
        Dynamic::from_array(vec![Dynamic::from(v1), Dynamic::from(v2)]),
    );
    let r4 = eval(
        &engine,
        &cartridge("mev_05_013_cross_custodian_arbitrage"),
        cc,
    );
    assert!(is_opp(&r4));
    assert_eq!(reason(&r4), "custodian_dex_spread_profit");

    // 014 fiat-stablecoin: base must be a stablecoin.
    let mut bad = route_pool_data(&[("0xab1", "0xb", "0xa")]);
    bad.insert(
        "cex".into(),
        Dynamic::from(cex_payload("0xa", "0xb", 1.15, 0.85)),
    );
    let r5 = eval(
        &engine,
        &cartridge("mev_05_014_fiat_stablecoin_dex_arbitrage"),
        bad,
    );
    assert!(!is_opp(&r5));
    assert_eq!(reason(&r5), "not_fiat_stable_pair");

    let mut ok = route_pool_data(&[("0xss1", "0xs2", "0xs1")]);
    ok.insert(
        "cex".into(),
        Dynamic::from(cex_payload("0xs1", "0xs2", 1.15, 0.95)),
    );
    let r6 = eval(
        &engine,
        &cartridge("mev_05_014_fiat_stablecoin_dex_arbitrage"),
        ok,
    );
    assert!(
        is_opp(&r6),
        "014 hunts the fiat premium on a stable base — got reason={}",
        reason(&r6)
    );
    assert_eq!(reason(&r6), "fiat_stablecoin_premium_profit");
}

// ─────────────────────────────────────────────────────────────────────────────
// 3. G06 — cross-domain composition over bridge_state
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn g06_l1_l1_hunts_cross_domain_spread() {
    let engine = math_engine();
    let mut bs = bridge_payload();
    bs.insert("src_kind".into(), s("l1"));
    bs.insert("dst_kind".into(), s("l1"));
    let mut pd = Map::new();
    pd.insert("bridge_state".into(), Dynamic::from(bs));
    let r = eval(&engine, &cartridge("mev_06_001_l1_l1_arbitrage"), pd);
    assert!(
        is_opp(&r),
        "001 hunts the 21% two-domain spread — got reason={}",
        reason(&r)
    );
    assert_eq!(reason(&r), "l1_l1_prepos_spread_profit");
    let profit = r.get("estimated_profit").unwrap().as_float().unwrap();
    assert!(profit > 0.0, "profit in USD positive, got {profit}");
    let x = r.get("optimal_amount_in").unwrap().as_float().unwrap();
    assert!(x > 0.0 && x < 200.0, "sizing sane: {x}");
}

#[test]
fn g06_domain_shape_gates() {
    let engine = math_engine();

    let mk = |sk: &str, dk: &str| {
        let mut bs = bridge_payload();
        bs.insert("src_kind".into(), s(sk));
        bs.insert("dst_kind".into(), s(dk));
        let mut pd = Map::new();
        pd.insert("bridge_state".into(), Dynamic::from(bs));
        pd
    };

    // 001 is L1-L1 only: an L1-L2 shape is rejected.
    let r = eval(
        &engine,
        &cartridge("mev_06_001_l1_l1_arbitrage"),
        mk("l1", "l2"),
    );
    assert!(!is_opp(&r));
    assert_eq!(reason(&r), "domain_shape_mismatch");

    // 002 is the L1-L2 identity and hunts it.
    let r2 = eval(
        &engine,
        &cartridge("mev_06_002_l1_l2_arbitrage"),
        mk("l1", "l2"),
    );
    assert!(is_opp(&r2));
    assert_eq!(reason(&r2), "l1_l2_prepos_spread_profit");

    // 008 cross-VM requires differing VMs.
    let mut same_vm = bridge_payload();
    same_vm.insert("src_vm".into(), s("evm"));
    same_vm.insert("dst_vm".into(), s("evm"));
    let mut pd1 = Map::new();
    pd1.insert("bridge_state".into(), Dynamic::from(same_vm.clone()));
    let r3 = eval(&engine, &cartridge("mev_06_008_cross_vm_arbitrage"), pd1);
    assert!(!is_opp(&r3));
    assert_eq!(reason(&r3), "domain_shape_mismatch");

    let mut diff_vm = same_vm;
    diff_vm.insert("dst_vm".into(), s("svm"));
    let mut pd2 = Map::new();
    pd2.insert("bridge_state".into(), Dynamic::from(diff_vm));
    let r4 = eval(&engine, &cartridge("mev_06_008_cross_vm_arbitrage"), pd2);
    assert!(is_opp(&r4));
    assert_eq!(reason(&r4), "cross_vm_bridge_spread_profit");

    // 014 canonical bridge requires attested canonical flow.
    let mut noncanon = bridge_payload();
    noncanon.insert("canonical".into(), Dynamic::from(false));
    let mut pd3 = Map::new();
    pd3.insert("bridge_state".into(), Dynamic::from(noncanon));
    let r5 = eval(
        &engine,
        &cartridge("mev_06_014_canonical_bridge_arbitrage"),
        pd3,
    );
    assert!(!is_opp(&r5));
    assert_eq!(reason(&r5), "bridge_not_canonical");

    let mut canon = bridge_payload();
    canon.insert("canonical".into(), Dynamic::from(true));
    canon.insert("attested".into(), Dynamic::from(true));
    let mut pd4 = Map::new();
    pd4.insert("bridge_state".into(), Dynamic::from(canon));
    let r6 = eval(
        &engine,
        &cartridge("mev_06_014_canonical_bridge_arbitrage"),
        pd4,
    );
    assert!(is_opp(&r6));
    assert_eq!(reason(&r6), "canonical_bridge_spread_profit");

    // 015 fast bridge rejects the slow canonical flow.
    let mut slow = bridge_payload();
    slow.insert("canonical".into(), Dynamic::from(true));
    let mut pd5 = Map::new();
    pd5.insert("bridge_state".into(), Dynamic::from(slow));
    let r7 = eval(&engine, &cartridge("mev_06_015_fast_bridge_arbitrage"), pd5);
    assert!(!is_opp(&r7));
    assert_eq!(reason(&r7), "not_fast_bridge");

    let mut fast = bridge_payload();
    fast.insert("canonical".into(), Dynamic::from(false));
    fast.insert("eta_sec".into(), i(300));
    let mut pd6 = Map::new();
    pd6.insert("bridge_state".into(), Dynamic::from(fast));
    let r8 = eval(&engine, &cartridge("mev_06_015_fast_bridge_arbitrage"), pd6);
    assert!(is_opp(&r8));
    assert_eq!(reason(&r8), "fast_bridge_spread_profit");

    // 027 shared sequencer requires atomic inclusion.
    let mut pd7 = Map::new();
    pd7.insert("bridge_state".into(), Dynamic::from(bridge_payload()));
    let r9 = eval(
        &engine,
        &cartridge("mev_06_027_shared_sequencer_arbitrage"),
        pd7,
    );
    assert!(!is_opp(&r9));
    assert_eq!(reason(&r9), "not_shared_sequencer");

    let mut atom = bridge_payload();
    atom.insert("atomic".into(), Dynamic::from(true));
    let mut pd8 = Map::new();
    pd8.insert("bridge_state".into(), Dynamic::from(atom));
    let r10 = eval(
        &engine,
        &cartridge("mev_06_027_shared_sequencer_arbitrage"),
        pd8,
    );
    assert!(is_opp(&r10));
    assert_eq!(reason(&r10), "shared_sequencer_spread_profit");
}

#[test]
fn g06_mid_legs_intent_liquidation_and_oracle() {
    let engine = math_engine();

    // 011 cross-domain triangular requires a documented mid leg.
    let mut pd = Map::new();
    pd.insert("bridge_state".into(), Dynamic::from(bridge_payload()));
    let r = eval(
        &engine,
        &cartridge("mev_06_011_cross_domain_triangular_arbitrage"),
        pd,
    );
    assert!(!is_opp(&r));
    assert_eq!(reason(&r), "mid_leg_missing");

    let mut with_mid = bridge_payload();
    let mut mid = Map::new();
    mid.insert("pool".into(), s("0xmd"));
    mid.insert("r0".into(), s("1000000000000000000000"));
    mid.insert("r1".into(), s("1000000000000000000000"));
    mid.insert("token0".into(), s("0xa"));
    mid.insert("fee_bps".into(), i(30));
    mid.insert("token_in".into(), s("0xa"));
    mid.insert("token_out".into(), s("0xa"));
    with_mid.insert(
        "mid_legs".into(),
        Dynamic::from_array(vec![Dynamic::from(mid)]),
    );
    let mut pd2 = Map::new();
    pd2.insert("bridge_state".into(), Dynamic::from(with_mid));
    let r2 = eval(
        &engine,
        &cartridge("mev_06_011_cross_domain_triangular_arbitrage"),
        pd2,
    );
    assert!(
        is_opp(&r2),
        "011 hunts with the mid leg folded in — got reason={}",
        reason(&r2)
    );
    assert_eq!(reason(&r2), "cross_domain_triangular_spread_profit");

    // 025 intent: bounty must cover the exact fill cost.
    let mut intent = bridge_payload();
    let mut im = Map::new();
    im.insert("bounty_usd".into(), f(100.0));
    im.insert("min_out".into(), f(5.0));
    intent.insert("intent".into(), Dynamic::from(im));
    let mut pd3 = Map::new();
    pd3.insert("bridge_state".into(), Dynamic::from(intent.clone()));
    let r3 = eval(
        &engine,
        &cartridge("mev_06_025_cross_chain_intent_arbitrage"),
        pd3,
    );
    assert!(!is_opp(&r3));
    assert_eq!(reason(&r3), "intent_bounty_insufficient");

    let mut rich = intent;
    rich.insert(
        "intent".into(),
        Dynamic::from({
            let mut im2 = Map::new();
            im2.insert("bounty_usd".into(), f(100_000.0));
            im2.insert("min_out".into(), f(5.0));
            im2
        }),
    );
    let mut pd4 = Map::new();
    pd4.insert("bridge_state".into(), Dynamic::from(rich));
    let r4 = eval(
        &engine,
        &cartridge("mev_06_025_cross_chain_intent_arbitrage"),
        pd4,
    );
    assert!(
        is_opp(&r4),
        "025 fills a rich intent — got reason={}",
        reason(&r4)
    );
    assert_eq!(reason(&r4), "cross_chain_intent_bounty_profit");

    // 030 cross-chain liquidation: reward must beat the acquisition cost.
    let mut liq = bridge_payload();
    let mut lm = Map::new();
    lm.insert("size".into(), f(5.0));
    lm.insert("reward_usd".into(), f(100.0));
    liq.insert("liquidation".into(), Dynamic::from(lm));
    let mut pd5 = Map::new();
    pd5.insert("bridge_state".into(), Dynamic::from(liq.clone()));
    let r5 = eval(
        &engine,
        &cartridge("mev_06_030_cross_chain_liquidation_arbitrage"),
        pd5,
    );
    assert!(!is_opp(&r5));
    assert_eq!(reason(&r5), "liquidation_reward_insufficient");

    let mut rich_liq = liq;
    rich_liq.insert(
        "liquidation".into(),
        Dynamic::from({
            let mut lm2 = Map::new();
            lm2.insert("size".into(), f(5.0));
            lm2.insert("reward_usd".into(), f(50_000.0));
            lm2
        }),
    );
    let mut pd6 = Map::new();
    pd6.insert("bridge_state".into(), Dynamic::from(rich_liq));
    let r6 = eval(
        &engine,
        &cartridge("mev_06_030_cross_chain_liquidation_arbitrage"),
        pd6,
    );
    assert!(
        is_opp(&r6),
        "030 hunts the liquidation reward — got reason={}",
        reason(&r6)
    );
    assert_eq!(reason(&r6), "cross_chain_liquidation_reward_profit");

    // 029 oracle-normalized: matching oracle ratio => within band, no emit.
    let mut oracle = bridge_payload();
    oracle.insert("src_oracle_px".into(), f(1.0));
    oracle.insert("dst_oracle_px".into(), f(1.21));
    let mut pd7 = Map::new();
    pd7.insert("bridge_state".into(), Dynamic::from(oracle));
    let r7 = eval(
        &engine,
        &cartridge("mev_06_029_cross_domain_oracle_arbitrage"),
        pd7,
    );
    assert!(!is_opp(&r7));
    assert_eq!(reason(&r7), "oracle_domains_within_band");

    let mut skewed = bridge_payload();
    skewed.insert("src_oracle_px".into(), f(1.0));
    skewed.insert("dst_oracle_px".into(), f(1.0));
    let mut pd8 = Map::new();
    pd8.insert("bridge_state".into(), Dynamic::from(skewed));
    let r8 = eval(
        &engine,
        &cartridge("mev_06_029_cross_domain_oracle_arbitrage"),
        pd8,
    );
    assert!(
        is_opp(&r8),
        "029 hunts the oracle-normalized deviation — got reason={}",
        reason(&r8)
    );
    assert_eq!(reason(&r8), "cross_domain_oracle_spread_profit");
}

#[test]
fn g06_bridge_variant_hunts() {
    let engine = math_engine();

    // A spread rich enough that every bridge variant clears the net gates.
    let cases: &[(&str, Map, &str)] = &[
        (
            "mev_06_009_cross_consensus_arbitrage",
            {
                let mut bs = bridge_payload();
                bs.insert("src_consensus".into(), s("pow"));
                bs.insert("dst_consensus".into(), s("pos"));
                bs
            },
            "cross_consensus_bridge_spread_profit",
        ),
        (
            "mev_06_016_bridge_rate_arbitrage",
            bridge_payload(),
            "bridge_rate_spread_profit",
        ),
        (
            "mev_06_017_bridge_liquidity_arbitrage",
            {
                let mut bs = bridge_payload();
                bs.insert("bridge_liquidity".into(), f(10.0));
                bs
            },
            "bridge_liquidity_spread_profit",
        ),
        (
            "mev_06_018_bridge_rebalancing_arbitrage",
            {
                let mut bs = bridge_payload();
                bs.insert("rebalance_incentive_bps".into(), f(5.0));
                bs
            },
            "bridge_rebalance_incentive_profit",
        ),
        (
            "mev_06_020_wrapped_representation_arbitrage",
            {
                let mut bs = bridge_payload();
                bs.insert("wrap_rate".into(), f(1.0));
                bs
            },
            "wrapped_representation_parity_profit",
        ),
        (
            "mev_06_024_bridge_in_the_loop_arbitrage",
            {
                let mut bs = bridge_payload();
                bs.insert("reverse_fee_bps".into(), f(10.0));
                bs.insert("reverse_rate".into(), f(1.0));
                bs
            },
            "bridge_loop_round_trip_profit",
        ),
    ];
    for (file, bs, why) in cases {
        let mut pd = Map::new();
        pd.insert("bridge_state".into(), Dynamic::from(bs.clone()));
        let r = eval(&engine, &cartridge(file), pd);
        assert!(
            is_opp(&r),
            "{file} must hunt the 21% two-domain spread — got reason={}",
            reason(&r)
        );
        assert_eq!(reason(&r), *why, "{file} hunt reason");
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// 4. G07 — derivatives math
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn g07_spot_perp_basis_hunts_and_gates() {
    let engine = math_engine();

    // 001: perp mark 1.30 vs pool ~0.997.
    let mut pd = route_pool_data(&[("0xab1", "0xb", "0xa")]);
    pd.insert(
        "derivatives".into(),
        Dynamic::from(deriv_payload("perpetual", 1.30)),
    );
    let r = eval(
        &engine,
        &cartridge("mev_07_001_spot_perpetual_arbitrage"),
        pd.clone(),
    );
    assert!(
        is_opp(&r),
        "001 shorts the rich perp vs cheap spot — got reason={}",
        reason(&r)
    );
    assert_eq!(reason(&r), "spot_perp_basis_profit");
    let profit = r.get("estimated_profit").unwrap().as_float().unwrap();
    assert!(profit > 0.0, "profit positive, got {profit}");

    // Within the 0.3% band: no trade.
    let mut pd2 = route_pool_data(&[("0xab1", "0xb", "0xa")]);
    pd2.insert(
        "derivatives".into(),
        Dynamic::from(deriv_payload("perpetual", 1.001)),
    );
    let r2 = eval(
        &engine,
        &cartridge("mev_07_001_spot_perpetual_arbitrage"),
        pd2,
    );
    assert!(!is_opp(&r2));
    assert_eq!(reason(&r2), "basis_within_band");

    // Kind gate: a future payload is not a perpetual.
    let mut pd3 = route_pool_data(&[("0xab1", "0xb", "0xa")]);
    pd3.insert(
        "derivatives".into(),
        Dynamic::from(deriv_payload("future", 1.30)),
    );
    let r3 = eval(
        &engine,
        &cartridge("mev_07_001_spot_perpetual_arbitrage"),
        pd3,
    );
    assert!(!is_opp(&r3));
    assert_eq!(reason(&r3), "derivative_kind_mismatch");

    // 006 reverse cash-and-carry: sell-side route, backwardation required.
    let mut pd4 = route_pool_data(&[("0xab1", "0xa", "0xb")]);
    let mut d = deriv_payload("future", 0.85);
    d.insert("expiry_ts".into(), i(1_717_200_000 + 30 * 86400));
    pd4.insert("derivatives".into(), Dynamic::from(d));
    let r4 = eval(
        &engine,
        &cartridge("mev_07_006_reverse_cash_and_carry_arbitrage"),
        pd4,
    );
    assert!(
        is_opp(&r4),
        "006 longs the cheap future, sells the rich spot — got reason={}",
        reason(&r4)
    );
    assert_eq!(reason(&r4), "reverse_cash_and_carry_backwardation_profit");
}

#[test]
fn g07_peer_venue_and_funding_math() {
    let engine = math_engine();

    // 002 perp-perp across venues: 30% venue gap.
    let mut d = deriv_payload("perpetual", 1.30);
    let mut peer = deriv_payload("perpetual", 1.0);
    peer.insert("funding_rate".into(), f(0.0));
    d.insert("funding_rate".into(), f(0.0));
    d.insert("peer".into(), Dynamic::from(peer));
    let mut pd = Map::new();
    pd.insert("derivatives".into(), Dynamic::from(d));
    let r = eval(
        &engine,
        &cartridge("mev_07_002_perpetual_perpetual_arbitrage"),
        pd,
    );
    assert!(
        is_opp(&r),
        "002 trades the venue basis — got reason={}",
        reason(&r)
    );
    assert_eq!(reason(&r), "perp_perp_venue_basis_profit");

    // 004 future-future requires matching maturities.
    let mut d2 = deriv_payload("future", 1.30);
    d2.insert("expiry_ts".into(), i(1_717_200_000 + 30 * 86400));
    let mut peer2 = deriv_payload("future", 1.0);
    peer2.insert("expiry_ts".into(), i(1_717_200_000 + 60 * 86400));
    d2.insert("peer".into(), Dynamic::from(peer2));
    let mut pd2 = Map::new();
    pd2.insert("derivatives".into(), Dynamic::from(d2));
    let r2 = eval(
        &engine,
        &cartridge("mev_07_004_future_future_arbitrage"),
        pd2,
    );
    assert!(!is_opp(&r2));
    assert_eq!(reason(&r2), "maturity_mismatch");

    // 008 funding carry: positive funding over one day.
    let mut d3 = deriv_payload("perpetual", 1.10);
    d3.insert("funding_rate".into(), f(0.001));
    d3.insert("funding_interval_sec".into(), i(28800));
    d3.insert("horizon_sec".into(), i(86400));
    let mut pd3 = route_pool_data(&[("0xab1", "0xb", "0xa")]);
    pd3.insert("derivatives".into(), Dynamic::from(d3));
    let r3 = eval(
        &engine,
        &cartridge("mev_07_008_funding_rate_arbitrage"),
        pd3.clone(),
    );
    assert!(
        is_opp(&r3),
        "008 receives funding over the horizon — got reason={}",
        reason(&r3)
    );
    assert_eq!(reason(&r3), "funding_rate_carry_profit");

    // Horizon is mandatory for the funding family.
    let mut d4 = deriv_payload("perpetual", 1.10);
    d4.insert("funding_rate".into(), f(0.001));
    let mut pd4 = route_pool_data(&[("0xab1", "0xb", "0xa")]);
    pd4.insert("derivatives".into(), Dynamic::from(d4));
    let r4 = eval(
        &engine,
        &cartridge("mev_07_008_funding_rate_arbitrage"),
        pd4,
    );
    assert!(!is_opp(&r4));
    assert_eq!(reason(&r4), "funding_horizon_unavailable");

    // 009 cross-exchange funding: differential within band.
    let mut d5 = deriv_payload("perpetual", 1.0);
    d5.insert("funding_rate".into(), f(0.0010));
    d5.insert("funding_interval_sec".into(), i(28800));
    d5.insert("horizon_sec".into(), i(28800));
    let mut peer3 = deriv_payload("perpetual", 1.0);
    peer3.insert("funding_rate".into(), f(0.0015));
    peer3.insert("funding_interval_sec".into(), i(28800));
    d5.insert("peer".into(), Dynamic::from(peer3));
    let mut pd5 = Map::new();
    pd5.insert("derivatives".into(), Dynamic::from(d5));
    let r5 = eval(
        &engine,
        &cartridge("mev_07_009_cross_exchange_funding_arbitrage"),
        pd5,
    );
    assert!(!is_opp(&r5));
    assert_eq!(reason(&r5), "funding_differential_within_band");
}

#[test]
fn g07_options_parity_and_structures() {
    let engine = math_engine();

    // 016 put-call parity: C-P far above S-PV(K) with S ~1.
    let mut d = deriv_payload("option", 1.0);
    let mut opt = Map::new();
    opt.insert("strike".into(), f(1.0));
    opt.insert("expiry_ts".into(), i(1_717_200_000 + 30 * 86400));
    opt.insert("call_bid".into(), f(0.10));
    opt.insert("call_ask".into(), f(0.12));
    opt.insert("put_bid".into(), f(0.02));
    opt.insert("put_ask".into(), f(0.03));
    d.insert("options".into(), Dynamic::from(opt));
    let mut pd = route_pool_data(&[("0xab1", "0xb", "0xa")]);
    pd.insert("derivatives".into(), Dynamic::from(d));
    let r = eval(
        &engine,
        &cartridge("mev_07_016_options_put_call_parity_arbitrage"),
        pd,
    );
    assert!(
        is_opp(&r),
        "016 trades the parity violation — got reason={}",
        reason(&r)
    );
    assert_eq!(reason(&r), "put_call_parity_violation_profit");

    // 019 box: K1/K2 box worth 10, executable cost 6.5.
    let mut d2 = deriv_payload("option", 1.0);
    let mut box_opt = Map::new();
    box_opt.insert("strike".into(), f(100.0));
    box_opt.insert("expiry_ts".into(), i(1_717_200_000 + 30 * 86400));
    box_opt.insert("call_bid".into(), f(11.0));
    box_opt.insert("call_ask".into(), f(13.0));
    box_opt.insert("put_bid".into(), f(2.0));
    box_opt.insert("put_ask".into(), f(3.0));
    box_opt.insert("box_strike".into(), f(110.0));
    box_opt.insert("box_call_bid".into(), f(6.0));
    box_opt.insert("box_call_ask".into(), f(7.0));
    box_opt.insert("box_put_bid".into(), f(1.0));
    box_opt.insert("box_put_ask".into(), f(1.5));
    d2.insert("options".into(), Dynamic::from(box_opt));
    d2.insert("size".into(), f(10.0));
    let mut pd2 = Map::new();
    pd2.insert("derivatives".into(), Dynamic::from(d2));
    let r2 = eval(&engine, &cartridge("mev_07_019_box_spread_arbitrage"), pd2);
    assert!(
        is_opp(&r2),
        "019 trades the cheap box — got reason={}",
        reason(&r2)
    );
    assert_eq!(reason(&r2), "box_spread_parity_profit");

    // 020 butterfly: 2*C(K2) bid > wings ask — convexity violation.
    let mut d3 = deriv_payload("option", 1.0);
    let mut surf = Map::new();
    surf.insert(
        "strikes".into(),
        Dynamic::from_array(vec![f(100.0), f(110.0), f(120.0)]),
    );
    surf.insert(
        "call_bids".into(),
        Dynamic::from_array(vec![f(11.5), f(8.0), f(1.0)]),
    );
    surf.insert(
        "call_asks".into(),
        Dynamic::from_array(vec![f(12.0), f(8.5), f(1.5)]),
    );
    surf.insert("expiry_ts".into(), i(1_717_200_000 + 30 * 86400));
    d3.insert("options".into(), Dynamic::from(surf));
    d3.insert("size".into(), f(10.0));
    let mut pd3 = Map::new();
    pd3.insert("derivatives".into(), Dynamic::from(d3));
    let r3 = eval(
        &engine,
        &cartridge("mev_07_020_cross_strike_arbitrage"),
        pd3,
    );
    assert!(
        is_opp(&r3),
        "020 trades the convexity violation — got reason={}",
        reason(&r3)
    );
    assert_eq!(reason(&r3), "cross_strike_butterfly_profit");

    // 023 cross-venue: same strike/expiry, 0.07 venue gap.
    let mut d4 = deriv_payload("option", 1.0);
    let mut oa = Map::new();
    oa.insert("strike".into(), f(1.0));
    oa.insert("expiry_ts".into(), i(1_717_200_000 + 30 * 86400));
    oa.insert("call_bid".into(), f(1.09));
    oa.insert("call_ask".into(), f(1.11));
    let mut ob = Map::new();
    ob.insert("strike".into(), f(1.0));
    ob.insert("expiry_ts".into(), i(1_717_200_000 + 30 * 86400));
    ob.insert("call_bid".into(), f(0.98));
    ob.insert("call_ask".into(), f(1.02));
    let mut peer4 = deriv_payload("option", 1.0);
    peer4.insert("options".into(), Dynamic::from(ob));
    d4.insert("options".into(), Dynamic::from(oa));
    d4.insert("peer".into(), Dynamic::from(peer4));
    d4.insert("size".into(), f(10.0));
    let mut pd4 = Map::new();
    pd4.insert("derivatives".into(), Dynamic::from(d4));
    let r4 = eval(
        &engine,
        &cartridge("mev_07_023_cross_venue_implied_volatility_arbitrage"),
        pd4,
    );
    assert!(
        is_opp(&r4),
        "023 trades the venue gap — got reason={}",
        reason(&r4)
    );
    assert_eq!(reason(&r4), "cross_venue_iv_spread_profit");
}

#[test]
fn g07_vamm_settlement_nav_leverage() {
    let engine = math_engine();

    // 015 vAMM vs spot: two live pools, 21% basis.
    let mut d = deriv_payload("option", 1.0);
    d.insert("vamm_pool".into(), s("0xva1"));
    d.insert("vamm_fee_bps".into(), i(30));
    let mut pd = route_pool_data(&[("0xab1", "0xb", "0xa")]);
    pd.insert("derivatives".into(), Dynamic::from(d));
    let r = eval(&engine, &cartridge("mev_07_015_vamm_spot_arbitrage"), pd);
    assert!(
        is_opp(&r),
        "015 composes the two-pool round trip — got reason={}",
        reason(&r)
    );
    assert_eq!(reason(&r), "vamm_spot_two_pool_profit");

    // 026 settlement: settled above the pool marginal.
    let mut d2 = deriv_payload("future", 1.0);
    d2.insert("settle_px".into(), f(1.15));
    d2.insert("expiry_ts".into(), i(1_717_200_000 - 60));
    let mut pd2 = route_pool_data(&[("0xab1", "0xb", "0xa")]);
    pd2.insert("derivatives".into(), Dynamic::from(d2.clone()));
    let r2 = eval(
        &engine,
        &cartridge("mev_07_026_settlement_price_arbitrage"),
        pd2,
    );
    assert!(
        is_opp(&r2),
        "026 captures the settlement premium — got reason={}",
        reason(&r2)
    );
    assert_eq!(reason(&r2), "settlement_premium_capture_profit");

    // Not yet settled: wait.
    let mut d3 = d2;
    d3.insert("expiry_ts".into(), i(1_717_200_000 + 60));
    let mut pd3 = route_pool_data(&[("0xab1", "0xb", "0xa")]);
    pd3.insert("derivatives".into(), Dynamic::from(d3));
    let r3 = eval(
        &engine,
        &cartridge("mev_07_026_settlement_price_arbitrage"),
        pd3,
    );
    assert!(!is_opp(&r3));
    assert_eq!(reason(&r3), "not_settled");

    // 028 derivative liquidation: acquire at the pool, close for the reward.
    let mut d4 = deriv_payload("future", 1.0);
    let mut lm = Map::new();
    lm.insert("size".into(), f(5.0));
    lm.insert("reward_usd".into(), f(50_000.0));
    d4.insert("liquidation".into(), Dynamic::from(lm));
    let mut pd4 = route_pool_data(&[("0xab1", "0xb", "0xa")]);
    pd4.insert("derivatives".into(), Dynamic::from(d4));
    let r4 = eval(
        &engine,
        &cartridge("mev_07_028_derivative_liquidation_arbitrage"),
        pd4,
    );
    assert!(
        is_opp(&r4),
        "028 hunts the derivative liquidation — got reason={}",
        reason(&r4)
    );
    assert_eq!(reason(&r4), "derivative_liquidation_reward_profit");

    // 029 structured product: NAV 15% above the pool marginal.
    let mut d5 = deriv_payload("option", 1.0);
    d5.insert("nav_px".into(), f(1.15));
    let mut pd5 = route_pool_data(&[("0xab1", "0xb", "0xa")]);
    pd5.insert("derivatives".into(), Dynamic::from(d5.clone()));
    let r5 = eval(
        &engine,
        &cartridge("mev_07_029_structured_product_parity_arbitrage"),
        pd5,
    );
    assert!(
        is_opp(&r5),
        "029 buys the discount vs NAV — got reason={}",
        reason(&r5)
    );
    assert_eq!(reason(&r5), "structured_product_nav_profit");

    // Within the NAV band: no trade.
    let mut d6 = deriv_payload("option", 1.0);
    d6.insert("nav_px".into(), f(1.001));
    let mut pd6 = route_pool_data(&[("0xab1", "0xb", "0xa")]);
    pd6.insert("derivatives".into(), Dynamic::from(d6));
    let r6 = eval(
        &engine,
        &cartridge("mev_07_029_structured_product_parity_arbitrage"),
        pd6,
    );
    assert!(!is_opp(&r6));
    assert_eq!(reason(&r6), "nav_within_band");

    // 030 leverage rebalance: out-of-band leverage + NAV below the pool.
    let mut d7 = deriv_payload("option", 1.0);
    d7.insert("target_leverage".into(), f(2.0));
    d7.insert("current_leverage".into(), f(3.0));
    d7.insert("nav_px".into(), f(0.90));
    let mut pd7 = route_pool_data(&[("0xab1", "0xa", "0xb")]);
    pd7.insert("derivatives".into(), Dynamic::from(d7));
    let r7 = eval(
        &engine,
        &cartridge("mev_07_030_leverage_token_rebalancing_arbitrage"),
        pd7,
    );
    assert!(
        is_opp(&r7),
        "030 trades ahead of the rebalance flow — got reason={}",
        reason(&r7)
    );
    assert_eq!(reason(&r7), "leverage_rebalance_flow_profit");

    // Balanced leverage: nothing to do.
    let mut d8 = deriv_payload("option", 1.0);
    d8.insert("target_leverage".into(), f(2.0));
    d8.insert("current_leverage".into(), f(2.0));
    let mut pd8 = route_pool_data(&[("0xab1", "0xa", "0xb")]);
    pd8.insert("derivatives".into(), Dynamic::from(d8));
    let r8 = eval(
        &engine,
        &cartridge("mev_07_030_leverage_token_rebalancing_arbitrage"),
        pd8,
    );
    assert!(!is_opp(&r8));
    assert_eq!(reason(&r8), "no_rebalance_needed");
}

#[test]
fn g07_delta_hedge_and_amm_ob() {
    let engine = math_engine();

    // 025 delta-hedged: model 1.2 vs call ask 1.0, delta 0.5.
    let mut d = deriv_payload("option", 1.0);
    let mut opt = Map::new();
    opt.insert("strike".into(), f(1.0));
    opt.insert("expiry_ts".into(), i(1_717_200_000 + 30 * 86400));
    opt.insert("call_bid".into(), f(0.95));
    opt.insert("call_ask".into(), f(1.00));
    opt.insert("put_bid".into(), f(0.02));
    opt.insert("put_ask".into(), f(0.03));
    opt.insert("model_px".into(), f(1.20));
    opt.insert("delta".into(), f(0.5));
    d.insert("options".into(), Dynamic::from(opt));
    d.insert("size".into(), f(10.0));
    let mut pd = route_pool_data(&[("0xab1", "0xb", "0xa")]);
    pd.insert("derivatives".into(), Dynamic::from(d));
    let r = eval(
        &engine,
        &cartridge("mev_07_025_delta_hedged_options_arbitrage"),
        pd,
    );
    assert!(
        is_opp(&r),
        "025 buys the underpriced call, hedges delta — got reason={}",
        reason(&r)
    );
    assert_eq!(reason(&r), "delta_hedged_model_edge_profit");

    // 022 options AMM vs order book: AMM 1:1.21 pool vs rich book bid 1.5.
    let mut d2 = deriv_payload("option", 1.0);
    let mut opt2 = Map::new();
    opt2.insert("strike".into(), f(1.0));
    opt2.insert("expiry_ts".into(), i(1_717_200_000 + 30 * 86400));
    opt2.insert("call_bid".into(), f(1.5));
    opt2.insert("call_ask".into(), f(1.55));
    opt2.insert("put_bid".into(), f(0.02));
    opt2.insert("put_ask".into(), f(0.03));
    d2.insert("options".into(), Dynamic::from(opt2));
    d2.insert("vamm_pool".into(), s("0xva1"));
    d2.insert("vamm_fee_bps".into(), i(30));
    let mut pd2 = Map::new();
    pd2.insert("derivatives".into(), Dynamic::from(d2));
    let r2 = eval(
        &engine,
        &cartridge("mev_07_022_options_amm_order_book_arbitrage"),
        pd2,
    );
    assert!(
        is_opp(&r2),
        "022 buys the AMM, sells the book — got reason={}",
        reason(&r2)
    );
    assert_eq!(reason(&r2), "options_amm_order_book_profit");
}

// ─────────────────────────────────────────────────────────────────────────────
// 5. Payload sweep — every cartridge runs a rich union payload without error
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn wave_c_full_payload_sweep_no_runtime_errors() {
    let engine = math_engine();

    // Union payload: route + cex + cex_venues + bridge_state + derivatives.
    let mut pd = route_pool_data(&[("0xab1", "0xb", "0xa"), ("0xba2", "0xa", "0xb")]);

    let mut cex = cex_payload("0xa", "0xb", 1.15, 0.85);
    cex.insert("mark_px".into(), f(1.30));
    cex.insert("expiry_ts".into(), i(1_717_200_000 + 30 * 86400));
    cex.insert("carry_rate_annual".into(), f(0.0));
    cex.insert("funding_rate".into(), f(0.001));
    cex.insert("funding_interval_sec".into(), i(28800));
    cex.insert("horizon_sec".into(), i(86400));
    pd.insert("cex".into(), Dynamic::from(cex.clone()));
    pd.insert(
        "cex_venues".into(),
        Dynamic::from_array(vec![Dynamic::from(cex.clone()), Dynamic::from(cex)]),
    );

    let mut bs = bridge_payload();
    bs.insert("src_kind".into(), s("l1"));
    bs.insert("dst_kind".into(), s("l1"));
    bs.insert("src_vm".into(), s("evm"));
    bs.insert("dst_vm".into(), s("svm"));
    bs.insert("src_consensus".into(), s("pow"));
    bs.insert("dst_consensus".into(), s("pos"));
    bs.insert("bridge_fee_bps".into(), f(25.0));
    bs.insert("eta_sec".into(), i(600));
    bs.insert("finality_sec".into(), i(300));
    bs.insert("canonical".into(), Dynamic::from(false));
    bs.insert("attested".into(), Dynamic::from(true));
    bs.insert("bridge_liquidity".into(), f(50.0));
    bs.insert("rebalance_incentive_bps".into(), f(5.0));
    bs.insert("wrap_rate".into(), f(1.0));
    bs.insert("sigma_per_sec".into(), f(0.00001));
    bs.insert("reorg_prob".into(), f(0.001));
    bs.insert("inventory_qty".into(), f(50.0));
    bs.insert("reverse_fee_bps".into(), f(10.0));
    bs.insert("reverse_rate".into(), f(1.0));
    bs.insert("atomic".into(), Dynamic::from(true));
    bs.insert("builder_slot".into(), i(5));
    bs.insert("canonical_fee_bps".into(), f(50.0));
    bs.insert("fast_fee_bps".into(), f(10.0));
    bs.insert("src_oracle_px".into(), f(1.0));
    bs.insert("dst_oracle_px".into(), f(1.0));
    bs.insert(
        "mid_legs".into(),
        Dynamic::from_array(vec![{
            let mut mid = Map::new();
            mid.insert("pool".into(), s("0xmd"));
            mid.insert("r0".into(), s("1000000000000000000000"));
            mid.insert("r1".into(), s("1000000000000000000000"));
            mid.insert("token0".into(), s("0xa"));
            mid.insert("fee_bps".into(), i(30));
            mid.insert("token_in".into(), s("0xa"));
            mid.insert("token_out".into(), s("0xa"));
            Dynamic::from(mid)
        }]),
    );
    bs.insert(
        "intent".into(),
        Dynamic::from({
            let mut im = Map::new();
            im.insert("bounty_usd".into(), f(100_000.0));
            im.insert("min_out".into(), f(5.0));
            im
        }),
    );
    bs.insert(
        "liquidation".into(),
        Dynamic::from({
            let mut lm = Map::new();
            lm.insert("size".into(), f(5.0));
            lm.insert("reward_usd".into(), f(50_000.0));
            lm
        }),
    );
    pd.insert("bridge_state".into(), Dynamic::from(bs));

    let mut deriv = deriv_payload("perpetual", 1.30);
    deriv.insert("index_px".into(), f(1.05));
    deriv.insert("funding_rate".into(), f(0.001));
    deriv.insert("funding_interval_sec".into(), i(28800));
    deriv.insert("horizon_sec".into(), i(86400));
    deriv.insert("expiry_ts".into(), i(1_717_200_000 + 30 * 86400));
    deriv.insert("size".into(), f(10.0));
    deriv.insert("vamm_pool".into(), s("0xva1"));
    deriv.insert("vamm_fee_bps".into(), i(30));
    deriv.insert("nav_px".into(), f(1.15));
    deriv.insert("target_leverage".into(), f(2.0));
    deriv.insert("current_leverage".into(), f(3.0));
    deriv.insert("settle_px".into(), f(1.15));
    let mut opt = Map::new();
    opt.insert("strike".into(), f(1.0));
    opt.insert("expiry_ts".into(), i(1_717_200_000 + 30 * 86400));
    opt.insert("call_bid".into(), f(0.10));
    opt.insert("call_ask".into(), f(0.12));
    opt.insert("put_bid".into(), f(0.02));
    opt.insert("put_ask".into(), f(0.03));
    opt.insert("model_px".into(), f(1.20));
    opt.insert("delta".into(), f(0.5));
    opt.insert("box_strike".into(), f(110.0));
    opt.insert("box_call_bid".into(), f(6.0));
    opt.insert("box_call_ask".into(), f(7.0));
    opt.insert("box_put_bid".into(), f(1.0));
    opt.insert("box_put_ask".into(), f(1.5));
    opt.insert(
        "strikes".into(),
        Dynamic::from_array(vec![f(100.0), f(110.0), f(120.0)]),
    );
    opt.insert(
        "call_bids".into(),
        Dynamic::from_array(vec![f(11.5), f(8.0), f(1.0)]),
    );
    opt.insert(
        "call_asks".into(),
        Dynamic::from_array(vec![f(12.0), f(8.5), f(1.5)]),
    );
    deriv.insert("options".into(), Dynamic::from(opt));
    let mut peer = deriv_payload("perpetual", 1.0);
    peer.insert("funding_rate".into(), f(0.0));
    peer.insert("expiry_ts".into(), i(1_717_200_000 + 60 * 86400));
    let mut peer_opt = Map::new();
    peer_opt.insert("strike".into(), f(1.0));
    peer_opt.insert("expiry_ts".into(), i(1_717_200_000 + 60 * 86400));
    peer_opt.insert("call_bid".into(), f(0.20));
    peer_opt.insert("call_ask".into(), f(0.22));
    peer.insert("options".into(), Dynamic::from(peer_opt));
    deriv.insert("peer".into(), Dynamic::from(peer));
    deriv.insert(
        "liquidation".into(),
        Dynamic::from({
            let mut lm = Map::new();
            lm.insert("size".into(), f(5.0));
            lm.insert("reward_usd".into(), f(50_000.0));
            lm
        }),
    );
    pd.insert("derivatives".into(), Dynamic::from(deriv));

    let all: Vec<&str> = G05
        .iter()
        .chain(G06.iter())
        .chain(G07.iter())
        .copied()
        .collect();
    assert_eq!(all.len(), 74);
    for file in &all {
        let src = cartridge(file);
        let ast = engine.compile(&src).expect("{file} compiles");
        let mut scope = rhai::Scope::new();
        let out: std::result::Result<Dynamic, _> =
            engine.call_fn(&mut scope, &ast, "evaluate_opportunity", (pd.clone(),));
        let dyn_out = out.unwrap_or_else(|e| panic!("{file} runtime error: {e}"));
        let m = dyn_out.cast::<Map>();
        let why = m.get("reason").unwrap().clone().into_string().unwrap();
        assert!(!why.is_empty(), "{file} always reports a reason");
        // Also run build_payload on the result (contract completeness).
        let mut scope2 = rhai::Scope::new();
        let payload: std::result::Result<Dynamic, _> =
            engine.call_fn(&mut scope2, &ast, "build_payload", (m,));
        assert!(
            payload.is_ok(),
            "{file} build_payload must run: {:?}",
            payload.err()
        );
    }
}
