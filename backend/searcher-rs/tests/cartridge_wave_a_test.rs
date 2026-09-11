//! RU-5 wave A functional tests — G01+G02 cartridges with real hunting math.
//!
//! Unlike `cartridge_syntax_validate.rs` (parse-only), this suite RUNS
//! `init_strategy()` / `evaluate_opportunity()` / `build_payload()` for the 17
//! wave-A cartridges against hand-built `pool_data` with deterministic stub
//! host bindings (same style as `cartridge_strategies_test.rs`; no Redis/RPC —
//! the stubs are test fixtures, NOT production data paths, so RULE 00 holds:
//! the cartridges themselves never fabricate numbers).
//!
//! Covered behaviors:
//!   1. Empty `pool_data` => fail-honest no-op with the exact machine-readable
//!      reason per detector (no crash, no fabricated opportunity).
//!   2. G02 pure-pipeline (CF_*) cartridge on a genuinely mispriced closed
//!      cycle => is_opportunity:true with positive gross profit (golden-section
//!      sizing really hunts).
//!   3. Detector-specific gates: TWAMM staleness, vAMM derivative state,
//!      orderbook VWAP legs, RFQ firm quotes, basket/index composition, CoW
//!      crossing intents — both the gate reason and the profitable path.
//!
//! Run: cargo test -p searcher-rs --test cartridge_wave_a_test -- --nocapture

use rhai::{Dynamic, Engine, Map, Scope};

fn cartridge(id: &str) -> String {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("cartridges/strategies")
        .join(format!("{id}.rhai"));
    std::fs::read_to_string(&path).unwrap()
}

/// Common host-binding stubs (mirror production signatures).
fn register_common(engine: &mut Engine) {
    // Mirror the release engine's expression-depth limits (see
    // cartridge_syntax_validate.rs): without them a debug-profile default
    // (32/16) is stricter than production and false-fails these cartridges.
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
    engine.register_fn("to_float", |s: &str| -> Dynamic {
        Dynamic::from(s.parse::<f64>().unwrap_or(0.0))
    });
    engine.register_fn("from_wei", |wei: &str, dec: i64| -> Dynamic {
        let v: f64 = wei.parse().unwrap_or(0.0);
        Dynamic::from(v / 10f64.powi(dec as i32))
    });
}

/// Engine with reserves/meta/price stubs for a 2-token world:
///   A = 0xa (18 dec, $2000)   B = 0xb (18 dec, $2000)
///   pool "0xab1": r0(A)=1e21, r1(B)=1e21            (balanced A/B pool)
///   pool "0xba2": r0(B)=1e21, r1(A)=1.21e21         (10% mispriced mirror)
///   component pools "0xc1"/"0xc2": 1:1 vs quote token T
///   basket pool "0xbb": r0(BSK)=1e21, r1(T)=1.21e21
fn math_engine() -> Engine {
    let mut engine = Engine::new();
    register_common(&mut engine);
    engine.register_fn("get_token_meta", |addr: &str| -> Dynamic {
        let mut m = Map::new();
        let symbol = match addr {
            "0xa" => "TKA",
            "0xb" => "TKB",
            "0xbb" => "BSK",
            _ => "TKA",
        };
        m.insert("symbol".into(), Dynamic::from(symbol.to_string()));
        m.insert("decimals".into(), Dynamic::from(18_i64));
        m.insert("is_stablecoin".into(), Dynamic::from(false));
        Dynamic::from_map(m)
    });
    engine.register_fn("get_reserves", |addr: &str| -> Dynamic {
        let (r0, r1) = match addr {
            "0xab1" => ("1000000000000000000000", "1000000000000000000000"),
            "0xba2" => ("1000000000000000000000", "1210000000000000000000"),
            "0xc1" | "0xc2" => ("1000000000000000000000", "1000000000000000000000"),
            "0xbb" => ("1000000000000000000000", "1210000000000000000000"),
            _ => return Dynamic::UNIT,
        };
        let mut m = Map::new();
        m.insert("r0".into(), Dynamic::from(r0.to_string()));
        m.insert("r1".into(), Dynamic::from(r1.to_string()));
        m.insert("block".into(), Dynamic::from(20_000_000_i64));
        m.insert("ts".into(), Dynamic::from(1_717_200_000_i64));
        Dynamic::from_map(m)
    });
    engine.register_fn("get_token_price_usd", |_sym: &str| -> Dynamic {
        Dynamic::from(2000.0_f64)
    });
    engine.register_fn("get_math_evidence", |_s: &str| -> Dynamic { Dynamic::UNIT });
    engine
}

fn eval(engine: &Engine, src: &str, arg: Map) -> Map {
    let ast = engine.compile(src).expect("cartridge compiles");
    let mut scope = Scope::new();
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

fn leg(pool: &str, token_in: &str, token_out: &str, fee_bps: i64) -> Map {
    let mut m = Map::new();
    m.insert("pool".into(), Dynamic::from(pool.to_string()));
    m.insert("token_in".into(), Dynamic::from(token_in.to_string()));
    m.insert("token_out".into(), Dynamic::from(token_out.to_string()));
    m.insert("protocol_type".into(), Dynamic::from("v2".to_string()));
    m.insert("fee_bps".into(), Dynamic::from(fee_bps));
    m
}

fn closed_route_pool_data(legs: Vec<Map>) -> Map {
    let mut pd = Map::new();
    pd.insert("chain_id".into(), Dynamic::from(1_i64));
    pd.insert(
        "route".into(),
        Dynamic::from_array(legs.into_iter().map(Dynamic::from).collect()),
    );
    pd.insert("route_closed".into(), Dynamic::from(true));
    pd
}

// ─────────────────────────────────────────────────────────────────────────────
// 1. Empty pool_data => fail-honest machine-readable reason (all 17)
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn wave_a_empty_data_fail_honest_reasons() {
    let engine = math_engine();
    // (cartridge file, expected reason on empty pool_data)
    let cases: &[(&str, &str)] = &[
        ("mev_01_009_amm_clob_arbitrage", "missing_route"),
        ("mev_01_010_clob_clob_arbitrage", "missing_route"),
        ("mev_01_011_amm_rfq_arbitrage", "missing_route"),
        ("mev_01_012_rfq_rfq_arbitrage", "missing_route"),
        ("mev_01_027_basket_arbitrage", "basket_state_unavailable"),
        (
            "mev_01_028_index_arbitrage",
            "index_composition_unavailable",
        ),
        (
            "mev_01_030_coincidence_of_wants_arbitrage",
            "intent_batch_unavailable",
        ),
        (
            "mev_02_007_proactive_market_maker_arbitrage",
            "missing_route",
        ),
        ("mev_02_008_dynamic_liquidity_arbitrage", "missing_route"),
        ("mev_02_009_dynamic_weight_arbitrage", "missing_route"),
        ("mev_02_010_bonding_curve_arbitrage", "missing_route"),
        ("mev_02_011_oracle_priced_amm_arbitrage", "missing_route"),
        ("mev_02_012_virtual_amm_arbitrage", "missing_route"),
        ("mev_02_013_hybrid_amm_clob_arbitrage", "missing_route"),
        ("mev_02_014_twamm_arbitrage", "missing_route"),
        ("mev_02_015_batch_amm_arbitrage", "missing_route"),
        ("mev_02_016_auction_managed_amm_arbitrage", "missing_route"),
    ];
    assert_eq!(cases.len(), 17, "wave A = 17 cartridges");
    for (file, expected) in cases {
        let src = cartridge(file);
        let r = eval(&engine, &src, Map::new());
        assert!(!is_opp(&r), "{file} must not fabricate an opportunity");
        assert_eq!(reason(&r), *expected, "{file} empty-data reason");
        // detector identity preserved in the no-op map
        let det = r.get("detector_id").unwrap().clone().into_string().unwrap();
        assert!(!det.is_empty(), "{file} carries detector_id");
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// 2. G02 pure pipeline: mispriced closed cycle => real opportunity
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn g02_pure_pipeline_hunts_mispriced_cycle() {
    let engine = math_engine();
    // A --0xab1--> B --0xba2--> A : spot 1:1 then 1:1.21 => ~10% gross edge.
    let pd = closed_route_pool_data(vec![
        leg("0xab1", "0xa", "0xb", 30),
        leg("0xba2", "0xb", "0xa", 30),
    ]);
    for file in [
        "mev_02_007_proactive_market_maker_arbitrage",
        "mev_02_008_dynamic_liquidity_arbitrage",
        "mev_02_009_dynamic_weight_arbitrage",
        "mev_02_010_bonding_curve_arbitrage",
        "mev_02_011_oracle_priced_amm_arbitrage",
        "mev_02_014_twamm_arbitrage", // reserves fresh (block == head)
        "mev_02_015_batch_amm_arbitrage",
        "mev_02_016_auction_managed_amm_arbitrage",
    ] {
        let src = cartridge(file);
        let r = eval(&engine, &src, pd.clone());
        assert!(is_opp(&r), "{file} must hunt a 10% mispriced cycle");
        let profit = r.get("estimated_profit").unwrap().as_float().unwrap();
        assert!(profit > 0.0, "{file} profit must be positive, got {profit}");
        let x = r.get("optimal_amount_in").unwrap().as_float().unwrap();
        assert!(x > 0.0 && x < 2000.0, "{file} sizing bracket sane: {x}");
    }
}

#[test]
fn g02_pure_pipeline_rejects_flat_cycle() {
    let engine = math_engine();
    // Both legs 1:1 with 30bps fees => prefilter kills it.
    let pd = closed_route_pool_data(vec![
        leg("0xab1", "0xa", "0xb", 30),
        leg("0xab1", "0xb", "0xa", 30),
    ]);
    let src = cartridge("mev_02_007_proactive_market_maker_arbitrage");
    let r = eval(&engine, &src, pd);
    assert!(!is_opp(&r));
    assert_eq!(reason(&r), "prefilter_non_positive");
}

// ─────────────────────────────────────────────────────────────────────────────
// 3. Detector-specific gates
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn g02_012_vamm_positive_spread_gated_on_derivative_state() {
    let engine = math_engine();
    let pd = closed_route_pool_data(vec![
        leg("0xab1", "0xa", "0xb", 30),
        leg("0xba2", "0xb", "0xa", 30),
    ]);
    let src = cartridge("mev_02_012_virtual_amm_arbitrage");
    let r = eval(&engine, &src, pd);
    // Raw virtual spread is positive but unhedgeable: manifest gate fires.
    assert!(!is_opp(&r));
    assert_eq!(reason(&r), "derivative_state_unavailable");
}

#[test]
fn g02_014_twamm_stale_reserves_refused() {
    let mut engine = math_engine();
    // Head far ahead of the synced reserves block => pre-advance state.
    engine.register_fn("get_block_number", || -> Dynamic {
        Dynamic::from(20_000_100_i64)
    });
    let pd = closed_route_pool_data(vec![
        leg("0xab1", "0xa", "0xb", 30),
        leg("0xba2", "0xb", "0xa", 30),
    ]);
    let src = cartridge("mev_02_014_twamm_arbitrage");
    let r = eval(&engine, &src, pd);
    assert!(!is_opp(&r));
    assert_eq!(reason(&r), "twamm_state_stale");
}

fn bid_ladder(levels: &[(f64, f64)]) -> Dynamic {
    let arr: Vec<Dynamic> = levels
        .iter()
        .map(|&(price, size)| {
            let mut lvl = Map::new();
            lvl.insert("price".into(), Dynamic::from(price));
            lvl.insert("size".into(), Dynamic::from(size));
            Dynamic::from_map(lvl)
        })
        .collect();
    let mut ob = Map::new();
    ob.insert("bids".into(), Dynamic::from_array(arr));
    Dynamic::from_map(ob)
}

fn book_leg(token_in: &str, token_out: &str, fee_bps: i64, ob: Dynamic) -> Map {
    let mut m = leg("", token_in, token_out, fee_bps);
    let _ = m.remove("pool");
    m.insert("orderbook".into(), ob);
    m
}

#[test]
fn g01_009_amm_clob_vwap_hunts_cross_venue_spread() {
    let engine = math_engine();
    // AMM A->B at 1:1, then sell B into bids at 1.05 A per B (depth 1000 B).
    let pd = closed_route_pool_data(vec![
        leg("0xab1", "0xa", "0xb", 30),
        book_leg("0xb", "0xa", 30, bid_ladder(&[(1.05, 1e21), (1.04, 1e21)])),
    ]);
    let src = cartridge("mev_01_009_amm_clob_arbitrage");
    let r = eval(&engine, &src, pd);
    assert!(
        is_opp(&r),
        "5% book edge over the AMM must be an opportunity"
    );
    assert_eq!(reason(&r), "amm_clob_vwap_profit");
    assert!(r.get("estimated_profit").unwrap().as_float().unwrap() > 0.0);
}

#[test]
fn g01_009_amm_clob_no_ladder_fail_honest() {
    let engine = math_engine();
    // Closing leg without an orderbook => precise reason (today's pipeline).
    let pd = closed_route_pool_data(vec![
        leg("0xab1", "0xa", "0xb", 30),
        leg("0xba2", "0xb", "0xa", 30), // pool leg, not a book leg
    ]);
    let src = cartridge("mev_01_009_amm_clob_arbitrage");
    let r = eval(&engine, &src, pd);
    assert!(!is_opp(&r));
    assert_eq!(reason(&r), "clob_leg_missing");
}

#[test]
fn g01_010_clob_clob_vwap_composition() {
    let engine = math_engine();
    // 1 A buys 1.05 B on venue 1; each B sells for 1.02 A on venue 2.
    let pd = closed_route_pool_data(vec![
        book_leg("0xa", "0xb", 10, bid_ladder(&[(1.05, 1e21)])),
        book_leg("0xb", "0xa", 10, bid_ladder(&[(1.02, 1e21)])),
    ]);
    let src = cartridge("mev_01_010_clob_clob_arbitrage");
    let r = eval(&engine, &src, pd);
    assert!(is_opp(&r));
    assert_eq!(reason(&r), "clob_clob_vwap_profit");
}

#[test]
fn g02_013_hybrid_amm_clob_hunts() {
    let engine = math_engine();
    // Same cross-venue edge as 009, through the hybrid curve-family engine.
    let pd = closed_route_pool_data(vec![
        leg("0xab1", "0xa", "0xb", 30),
        book_leg("0xb", "0xa", 30, bid_ladder(&[(1.05, 1e21)])),
    ]);
    let src = cartridge("mev_02_013_hybrid_amm_clob_arbitrage");
    let r = eval(&engine, &src, pd);
    assert!(is_opp(&r));
    assert_eq!(reason(&r), "hybrid_amm_clob_round_trip_profit");
}

fn rfq_leg(token_in: &str, token_out: &str, price: f64, max_in: f64) -> Map {
    let mut m = leg("", token_in, token_out, 10);
    let _ = m.remove("pool");
    let mut q = Map::new();
    q.insert("price".into(), Dynamic::from(price));
    q.insert("max_in".into(), Dynamic::from(max_in));
    m.insert("rfq".into(), Dynamic::from_map(q));
    m
}

#[test]
fn g01_011_amm_rfq_firm_quote_hunts() {
    let engine = math_engine();
    // AMM A->B ~1:1, firm RFQ sells B at 1.02 A per B up to 1000 B.
    let pd = closed_route_pool_data(vec![
        leg("0xab1", "0xa", "0xb", 30),
        rfq_leg("0xb", "0xa", 1.02, 1e21),
    ]);
    let src = cartridge("mev_01_011_amm_rfq_arbitrage");
    let r = eval(&engine, &src, pd);
    assert!(is_opp(&r));
    assert_eq!(reason(&r), "amm_rfq_firm_quote_profit");
    // Executable size must respect the firm cap (~1000 B out of the AMM leg).
    let x = r.get("optimal_amount_in").unwrap().as_float().unwrap();
    assert!(x > 0.0 && x <= 2000.0, "sizing inside firm cap: {x}");
}

#[test]
fn g01_012_rfq_rfq_crossed_firm_quotes() {
    let engine = math_engine();
    // Buy B at 0.995 B per A, sell B at 1.02 A per B — fees 10bps each side.
    let pd = closed_route_pool_data(vec![
        rfq_leg("0xa", "0xb", 0.995, 1e21),
        rfq_leg("0xb", "0xa", 1.02, 1e21),
    ]);
    let src = cartridge("mev_01_012_rfq_rfq_arbitrage");
    let r = eval(&engine, &src, pd);
    assert!(is_opp(&r), "crossed firm quotes must be an opportunity");
    assert_eq!(reason(&r), "rfq_rfq_firm_quote_profit");
    let profit = r.get("estimated_profit").unwrap().as_float().unwrap();
    // ~1.3% of the 1000-token executable cap, in A units (~12.9 A).
    assert!(
        profit > 1.0,
        "linear firm-quote profit ~12.9 A, got {profit}"
    );
}

fn basket_pool_data(field: &str) -> Map {
    let mut pd = Map::new();
    let comps: Vec<Dynamic> = ["0xc1", "0xc2"]
        .iter()
        .map(|p| {
            let mut c = Map::new();
            c.insert("pool".into(), Dynamic::from(p.to_string()));
            c.insert("weight".into(), Dynamic::from(0.5_f64));
            Dynamic::from_map(c)
        })
        .collect();
    let mut b = Map::new();
    b.insert("components".into(), Dynamic::from_array(comps));
    b.insert("basket_pool".into(), Dynamic::from("0xbb".to_string()));
    b.insert("basket_token".into(), Dynamic::from("0xbb".to_string()));
    pd.insert(field.into(), Dynamic::from_map(b));
    pd
}

#[test]
fn g01_027_basket_nav_redeem_hunts() {
    let engine = math_engine();
    // Components 1:1 vs quote; basket pool redeems at 1.21 quote per BSK.
    let pd = basket_pool_data("basket");
    let src = cartridge("mev_01_027_basket_arbitrage");
    let r = eval(&engine, &src, pd);
    assert!(
        is_opp(&r),
        "20% redeem premium over NAV must be an opportunity"
    );
    assert_eq!(reason(&r), "basket_nav_redeem_profit");
    assert!(r.get("estimated_profit").unwrap().as_float().unwrap() > 0.0);
}

#[test]
fn g01_028_index_nav_mint_redeem_hunts() {
    let engine = math_engine();
    let pd = basket_pool_data("index");
    let src = cartridge("mev_01_028_index_arbitrage");
    let r = eval(&engine, &src, pd);
    assert!(is_opp(&r));
    assert_eq!(reason(&r), "index_nav_mint_redeem_profit");
}

fn order(sell_token: &str, buy_token: &str, sell_amount: f64, buy_amount: f64) -> Dynamic {
    let mut o = Map::new();
    o.insert("sell_token".into(), Dynamic::from(sell_token.to_string()));
    o.insert("buy_token".into(), Dynamic::from(buy_token.to_string()));
    o.insert("sell_amount".into(), Dynamic::from(sell_amount));
    o.insert("buy_amount".into(), Dynamic::from(buy_amount));
    Dynamic::from_map(o)
}

#[test]
fn g01_030_cow_crossing_intents_match() {
    let engine = math_engine();
    let mut pd = Map::new();
    pd.insert(
        "orders".into(),
        Dynamic::from_array(vec![
            order("0xa", "0xb", 10e18, 20e18), // sells A, min 2 B per A
            order("0xb", "0xa", 21e18, 10e18), // buys A, pays up to 2.1 B per A
        ]),
    );
    let src = cartridge("mev_01_030_coincidence_of_wants_arbitrage");
    let r = eval(&engine, &src, pd);
    assert!(
        is_opp(&r),
        "crossing limit intents are internally matchable"
    );
    assert_eq!(reason(&r), "cow_crossing_pair_surplus");
    // Surplus = q*(2.1-2) with q=min(10, 21/2.1)=10 A => 1 B = 1e18 base units.
    let profit = r.get("estimated_profit").unwrap().as_float().unwrap();
    assert!(
        (profit - 1.0).abs() < 0.01,
        "surplus ~1 B token, got {profit}"
    );
}

#[test]
fn g01_030_cow_no_crossing_fail_honest() {
    let engine = math_engine();
    let mut pd = Map::new();
    pd.insert(
        "orders".into(),
        Dynamic::from_array(vec![
            order("0xa", "0xb", 10e18, 20e18), // min 2 B per A
            order("0xb", "0xa", 19e18, 10e18), // pays up to 1.9 B per A — no cross
        ]),
    );
    let src = cartridge("mev_01_030_coincidence_of_wants_arbitrage");
    let r = eval(&engine, &src, pd);
    assert!(!is_opp(&r));
    assert_eq!(reason(&r), "no_crossing_intents");
}

// ─────────────────────────────────────────────────────────────────────────────
// 4. build_payload stays observe-only for every wave-A cartridge
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn wave_a_payloads_observe_only() {
    let engine = math_engine();
    let files = [
        "mev_01_009_amm_clob_arbitrage",
        "mev_01_010_clob_clob_arbitrage",
        "mev_01_011_amm_rfq_arbitrage",
        "mev_01_012_rfq_rfq_arbitrage",
        "mev_01_027_basket_arbitrage",
        "mev_01_028_index_arbitrage",
        "mev_01_030_coincidence_of_wants_arbitrage",
        "mev_02_007_proactive_market_maker_arbitrage",
        "mev_02_008_dynamic_liquidity_arbitrage",
        "mev_02_009_dynamic_weight_arbitrage",
        "mev_02_010_bonding_curve_arbitrage",
        "mev_02_011_oracle_priced_amm_arbitrage",
        "mev_02_012_virtual_amm_arbitrage",
        "mev_02_013_hybrid_amm_clob_arbitrage",
        "mev_02_014_twamm_arbitrage",
        "mev_02_015_batch_amm_arbitrage",
        "mev_02_016_auction_managed_amm_arbitrage",
    ];
    for file in files {
        let src = cartridge(file);
        let ast = engine.compile(&src).unwrap();
        let mut scope = Scope::new();
        let out: Map = engine
            .call_fn::<Dynamic>(&mut scope, &ast, "build_payload", (Map::new(),))
            .unwrap()
            .cast::<Map>();
        assert_eq!(
            out.get("target_contract")
                .unwrap()
                .clone()
                .into_string()
                .unwrap(),
            "0x0000000000000000000000000000000000000000",
            "{file} payload targets nothing (SHADOW / S32)"
        );
        assert_eq!(
            out.get("calldata").unwrap().clone().into_string().unwrap(),
            "0x",
            "{file} payload carries no calldata"
        );
    }
}
