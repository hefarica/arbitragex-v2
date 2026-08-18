//! RU-5 wave B functional tests — G03+G04 cartridges with real hunting math.
//!
//! Same style as `cartridge_wave_a_test.rs`: RUNS `init_strategy()` /
//! `evaluate_opportunity()` / `build_payload()` for the 62 wave-B cartridges
//! against hand-built `pool_data` with deterministic stub host bindings (test
//! fixtures, NOT production data paths — RULE 00 holds: the cartridges
//! themselves never fabricate numbers).
//!
//! Covered behaviors:
//!   1. Empty `pool_data` => fail-honest no-op with the exact machine-readable
//!      reason per detector (all 62).
//!   2. G03 event round-trip (E_POST): mispriced closed cycle on fresh
//!      post-event state => is_opportunity:true; placement freshness gates
//!      (top-of-block lag==0, inter-block lag 1..2); shape gates (multi-swap
//!      >=3 legs, ripple >=3 pools).
//!   3. G03 divergence triggers (E_LATENCY/E_ORACLE): fast executable pool
//!      marginal vs slow oracle reference, band gates, profitable round trip.
//!   4. G03 feed-gated state transitions + auctions: exact waiting reasons,
//!      auction Π(q) once the payload is present, observe-only families.
//!   5. G04 parity engines (P_PEG/P_WRAP/P_4626/P_NAV/P_LST): market leg vs
//!      NAV reference, peg/parity bands, decimal normalization, stable-pair
//!      identity, directional capture + golden-section sizing.
//!   6. G04 feed-gated redemption sources: waiting reasons, the withdrawal
//!      queue (incl. redemption_queue_full), FOT fee folding.
//!
//! Run: cargo test -p searcher-rs --test cartridge_wave_b_test -- --nocapture

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
    engine.register_fn("to_float", |s: &str| -> Dynamic {
        Dynamic::from(s.parse::<f64>().unwrap_or(0.0))
    });
    engine.register_fn("from_wei", |wei: &str, dec: i64| -> Dynamic {
        let v: f64 = wei.parse().unwrap_or(0.0);
        Dynamic::from(v / 10f64.powi(dec as i32))
    });
}

/// Token universe (address -> symbol, decimals, stable, usd).
///   0xa TKA(18,$2000)  0xb TKB(18,$2000)  0xc TKC(18,$2000)
///   0xs1 USDX(18,stable,$1.0) 0xs2 USDY(18,stable,$1.0)
///   0xs6 USDT6(6,stable,$1.0)
///   0xlst WSTETH(18,$3000)  0xweth WETH(18,$2500)
fn register_tokens(engine: &mut Engine, prices: &HashMap<&str, f64>) {
    // ONE closure per binding — registering the same fn name+arity multiple
    // times overwrites (last wins), so dispatch through a match table.
    let meta: HashMap<String, (&'static str, i64, bool)> = [
        ("0xa", ("TKA", 18, false)),
        ("0xb", ("TKB", 18, false)),
        ("0xc", ("TKC", 18, false)),
        ("0xs1", ("USDX", 18, true)),
        ("0xs2", ("USDY", 18, true)),
        ("0xs6", ("USDT6", 6, true)),
        ("0xlst", ("WSTETH", 18, false)),
        ("0xweth", ("WETH", 18, false)),
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
    let table: HashMap<String, f64> = prices.iter().map(|(k, v)| ((*k).to_string(), *v)).collect();
    engine.register_fn("get_token_price_usd", move |sym: &str| -> Dynamic {
        match table.get(sym) {
            Some(p) => Dynamic::from(*p),
            None => Dynamic::UNIT,
        }
    });
}

/// Pool reserves universe (pool -> (r0, r1)); all synced at block 20_000_000.
///   0xab1 A/B 1:1              0xba2 B/A 1:1.21 (mispriced mirror)
///   0xab9 A/B 1:1.06 (divergent) 0xbc3 B/C 1:1   0xca4 C/A 1:1.21
///   0xss1 USDX/USDY 1:1.01 (1M pool)  0xss2 USDX/USDY 1:1.001
///   0xss6 USDX(18)/USDT6(6) 1e24 / 1.01e12
///   0xwl1 WSTETH/WETH 1:1 (vs oracle parity 1.2)
///   0xnew1 A/B thin 5e18 / 6.03e18
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
            "0xab9",
            ("1000000000000000000000", "1060000000000000000000"),
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
            ("1000000000000000000000000", "1010000000000000000000000"),
        ),
        (
            "0xss2",
            ("1000000000000000000000000", "1001000000000000000000000"),
        ),
        ("0xss6", ("1000000000000000000000000", "1010000000000")),
        (
            "0xwl1",
            ("1000000000000000000000", "1000000000000000000000"),
        ),
        ("0xnew1", ("5000000000000000000", "6030000000000000000")),
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

/// Full engine: all tokens priced at their canonical USD values.
fn math_engine() -> Engine {
    let mut engine = Engine::new();
    register_common(&mut engine);
    let prices: HashMap<&str, f64> = [
        ("TKA", 2000.0),
        ("TKB", 2000.0),
        ("TKC", 2000.0),
        ("USDX", 1.0),
        ("USDY", 1.0),
        ("USDT6", 1.0),
        ("WSTETH", 3000.0),
        ("WETH", 2500.0),
    ]
    .into_iter()
    .collect();
    register_tokens(&mut engine, &prices);
    register_pools(&mut engine);
    engine
}

/// Depeg engine: USDX off its $1 anchor at 0.99 (MEV-04-001 driver).
fn depeg_engine() -> Engine {
    let mut engine = Engine::new();
    register_common(&mut engine);
    let prices: HashMap<&str, f64> = [
        ("USDX", 0.99),
        ("USDY", 1.0),
        ("TKA", 2000.0),
        ("TKB", 2000.0),
    ]
    .into_iter()
    .collect();
    register_tokens(&mut engine, &prices);
    register_pools(&mut engine);
    engine
}

/// Starved engine: no USD prices at all (NAV-source miss path).
fn no_price_engine() -> Engine {
    let mut engine = Engine::new();
    register_common(&mut engine);
    let prices: HashMap<&str, f64> = HashMap::new();
    register_tokens(&mut engine, &prices);
    register_pools(&mut engine);
    engine
}

/// L2 engine: chain 42161 ( Arbitrum ) for the sequencer-latency gate.
fn l2_engine() -> Engine {
    let mut engine = math_engine();
    engine.register_fn("get_chain_id", || -> Dynamic { Dynamic::from(42_161_i64) });
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

/// 1-leg (open) route map — the parity-engine market leg shape.
fn open_route_pool_data(legs: Vec<Map>) -> Map {
    let mut pd = Map::new();
    pd.insert("chain_id".into(), Dynamic::from(1_i64));
    pd.insert(
        "route".into(),
        Dynamic::from_array(legs.into_iter().map(Dynamic::from).collect()),
    );
    pd
}

/// Mispriced closed 2-leg A/B cycle: ~10% gross edge.
fn mispriced_cycle() -> Map {
    closed_route_pool_data(vec![
        leg("0xab1", "0xa", "0xb", 30),
        leg("0xba2", "0xb", "0xa", 30),
    ])
}

/// Mispriced closed 3-leg A/B/C triangle across 3 distinct pools.
fn mispriced_triangle() -> Map {
    closed_route_pool_data(vec![
        leg("0xab1", "0xa", "0xb", 30),
        leg("0xbc3", "0xb", "0xc", 30),
        leg("0xca4", "0xc", "0xa", 30),
    ])
}

// ─────────────────────────────────────────────────────────────────────────────
// 1. Empty pool_data => fail-honest machine-readable reason (all 62)
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn wave_b_empty_data_fail_honest_reasons() {
    let engine = math_engine();
    let cases: &[(&str, &str)] = &[
        // G03 computing
        ("mev_03_001_swap_backrun_arbitrage", "missing_route"),
        ("mev_03_002_multi_swap_backrun", "missing_route"),
        ("mev_03_003_cross_pool_ripple_arbitrage", "missing_route"),
        ("mev_03_004_top_of_block_arbitrage", "missing_route"),
        ("mev_03_005_end_of_block_arbitrage", "missing_route"),
        ("mev_03_006_inter_block_arbitrage", "missing_route"),
        ("mev_03_007_stale_price_arbitrage", "missing_route"),
        ("mev_03_008_latency_arbitrage", "missing_route"),
        ("mev_03_009_oracle_update_arbitrage", "missing_route"),
        ("mev_03_010_oev_oracle_extractable_value", "missing_route"),
        ("mev_03_020_pool_initialization_arbitrage", "missing_route"),
        ("mev_03_027_block_time_arbitrage", "missing_route"),
        ("mev_03_028_sequencer_latency_arbitrage", "missing_route"),
        // G03 waiting for feed
        ("mev_03_011_rebase_arbitrage", "rebase_feed_unavailable"),
        (
            "mev_03_012_interest_index_update_arbitrage",
            "interest_index_feed_unavailable",
        ),
        (
            "mev_03_013_funding_update_arbitrage",
            "funding_feed_unavailable",
        ),
        (
            "mev_03_014_epoch_rollover_arbitrage",
            "epoch_feed_unavailable",
        ),
        (
            "mev_03_015_settlement_arbitrage",
            "settlement_feed_unavailable",
        ),
        (
            "mev_03_016_mint_burn_state_arbitrage",
            "mint_burn_feed_unavailable",
        ),
        (
            "mev_03_017_redemption_state_arbitrage",
            "redemption_state_feed_unavailable",
        ),
        (
            "mev_03_018_liquidity_add_arbitrage",
            "lp_event_feed_unavailable",
        ),
        (
            "mev_03_019_liquidity_removal_arbitrage",
            "lp_event_feed_unavailable",
        ),
        (
            "mev_03_021_migration_arbitrage",
            "migration_feed_unavailable",
        ),
        (
            "mev_03_022_fee_switch_arbitrage",
            "fee_param_feed_unavailable",
        ),
        (
            "mev_03_023_parameter_update_arbitrage",
            "param_update_feed_unavailable",
        ),
        (
            "mev_03_024_governance_execution_arbitrage",
            "governance_feed_unavailable",
        ),
        (
            "mev_03_025_keeper_triggered_arbitrage",
            "auction_state_unavailable",
        ),
        (
            "mev_03_026_auction_clearing_arbitrage",
            "auction_state_unavailable",
        ),
        (
            "mev_03_031_fork_state_arbitrage",
            "fork_state_feed_unavailable",
        ),
        // G03 observe-only
        (
            "mev_03_029_spam_arbitrage",
            "observe_only_structured_evidence",
        ),
        (
            "mev_03_030_reorg_time_bandit_arbitrage",
            "observe_only_structured_evidence",
        ),
        // G04 computing
        ("mev_04_001_stablecoin_peg_arbitrage", "missing_route"),
        ("mev_04_002_cross_stablecoin_arbitrage", "missing_route"),
        (
            "mev_04_004_collateralized_stablecoin_arbitrage",
            "missing_route",
        ),
        ("mev_04_006_native_wrapped_arbitrage", "missing_route"),
        ("mev_04_008_cross_wrapper_arbitrage", "missing_route"),
        ("mev_04_010_receipt_token_arbitrage", "missing_route"),
        ("mev_04_011_erc_4626_share_price_arbitrage", "missing_route"),
        (
            "mev_04_012_vault_share_underlying_arbitrage",
            "missing_route",
        ),
        ("mev_04_014_index_token_basket_arbitrage", "missing_route"),
        ("mev_04_019_liquid_staking_token_arbitrage", "missing_route"),
        (
            "mev_04_021_liquid_restaking_token_arbitrage",
            "missing_route",
        ),
        ("mev_04_022_cross_lst_arbitrage", "missing_route"),
        // G04 waiting for feed
        (
            "mev_04_003_algorithmic_stablecoin_arbitrage",
            "algorithmic_supply_feed_unavailable",
        ),
        (
            "mev_04_005_mint_redeem_stablecoin_arbitrage",
            "mint_redeem_rate_unavailable",
        ),
        (
            "mev_04_007_canonical_bridged_token_arbitrage",
            "bridge_state_unavailable",
        ),
        (
            "mev_04_009_synthetic_underlying_arbitrage",
            "synthetic_venue_feed_unavailable",
        ),
        (
            "mev_04_013_lp_token_nav_arbitrage",
            "lp_composition_feed_unavailable",
        ),
        (
            "mev_04_015_etf_like_token_arbitrage",
            "issuer_nav_feed_unavailable",
        ),
        (
            "mev_04_016_rwa_nav_arbitrage",
            "issuer_nav_feed_unavailable",
        ),
        (
            "mev_04_017_rebasing_token_arbitrage",
            "rebase_state_feed_unavailable",
        ),
        (
            "mev_04_018_fee_on_transfer_token_arbitrage",
            "fot_fee_feed_unavailable",
        ),
        (
            "mev_04_020_lst_redemption_arbitrage",
            "redemption_queue_state_unavailable",
        ),
        (
            "mev_04_023_governance_wrapper_arbitrage",
            "lock_state_feed_unavailable",
        ),
        (
            "mev_04_024_vote_escrow_derivative_arbitrage",
            "vederiv_lock_feed_unavailable",
        ),
        (
            "mev_04_025_tokenized_position_arbitrage",
            "position_components_feed_unavailable",
        ),
        (
            "mev_04_026_principal_token_arbitrage",
            "pt_yt_rate_feed_unavailable",
        ),
        (
            "mev_04_027_yield_token_arbitrage",
            "pt_yt_rate_feed_unavailable",
        ),
        (
            "mev_04_028_pt_yt_parity_arbitrage",
            "pt_yt_rate_feed_unavailable",
        ),
        (
            "mev_04_029_cross_maturity_yield_arbitrage",
            "yield_curve_feed_unavailable",
        ),
        (
            "mev_04_030_fixed_yield_floating_yield_arbitrage",
            "yield_index_feed_unavailable",
        ),
        // G04 observe-only
        (
            "mev_04_031_points_pre_token_arbitrage",
            "observe_only_structured_evidence",
        ),
    ];
    assert_eq!(cases.len(), 62, "wave B = 62 cartridges");
    for (file, expected) in cases {
        let src = cartridge(file);
        let r = eval(&engine, &src, Map::new());
        assert!(!is_opp(&r), "{file} must not fabricate an opportunity");
        assert_eq!(reason(&r), *expected, "{file} empty-data reason");
        let det = r.get("detector_id").unwrap().clone().into_string().unwrap();
        assert!(!det.is_empty(), "{file} carries detector_id");
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// 2. G03 event round-trip (E_POST)
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn g03_post_event_hunts_mispriced_cycle() {
    let engine = math_engine();
    let pd = mispriced_cycle();
    for (file, why) in [
        (
            "mev_03_001_swap_backrun_arbitrage",
            "post_event_round_trip_profit",
        ),
        (
            "mev_03_004_top_of_block_arbitrage",
            "top_of_block_round_trip_profit",
        ),
        (
            "mev_03_005_end_of_block_arbitrage",
            "end_of_block_round_trip_profit",
        ),
    ] {
        let src = cartridge(file);
        let r = eval(&engine, &src, pd.clone());
        assert!(is_opp(&r), "{file} must hunt a 10% mispriced cycle");
        assert_eq!(reason(&r), why);
        let profit = r.get("estimated_profit").unwrap().as_float().unwrap();
        assert!(profit > 0.0, "{file} profit must be positive, got {profit}");
        let x = r.get("optimal_amount_in").unwrap().as_float().unwrap();
        assert!(x > 0.0 && x < 2000.0, "{file} sizing bracket sane: {x}");
    }
}

#[test]
fn g03_multi_swap_and_ripple_shape_gates() {
    let engine = math_engine();
    // 2-leg cycle: below the >=3-leg multi-swap shape.
    let src2 = cartridge("mev_03_002_multi_swap_backrun");
    let r = eval(&engine, &src2, mispriced_cycle());
    assert!(!is_opp(&r));
    assert_eq!(reason(&r), "route_shape_out_of_bounds");

    // 3-leg triangle across 3 distinct pools: both 002 and 003 hunt it.
    let tri = mispriced_triangle();
    for file in [
        "mev_03_002_multi_swap_backrun",
        "mev_03_003_cross_pool_ripple_arbitrage",
    ] {
        let src = cartridge(file);
        let r = eval(&engine, &src, tri.clone());
        assert!(is_opp(&r), "{file} must hunt the 3-pool ripple");
        assert!(r.get("estimated_profit").unwrap().as_float().unwrap() > 0.0);
    }
}

#[test]
fn g03_placement_freshness_gates() {
    // head == 20_000_000 in the default engine => reserves at head (lag 0).
    let engine = math_engine();
    // 006 inter-block requires lag 1..2: at-head state is not its shape.
    let src6 = cartridge("mev_03_006_inter_block_arbitrage");
    let r = eval(&engine, &src6, mispriced_cycle());
    assert!(!is_opp(&r));
    assert_eq!(reason(&r), "state_not_inter_block");

    // head advanced by 1 (lag 1): 006 accepts, 004 (lag==0) refuses.
    let mut engine1 = math_engine();
    engine1.register_fn("get_block_number", || -> Dynamic {
        Dynamic::from(20_000_001_i64)
    });
    let r6 = eval(&engine1, &src6, mispriced_cycle());
    assert!(
        is_opp(&r6),
        "inter-block accepts the just-sealed block state"
    );
    let src4 = cartridge("mev_03_004_top_of_block_arbitrage");
    let r4 = eval(&engine1, &src4, mispriced_cycle());
    assert!(!is_opp(&r4));
    assert_eq!(reason(&r4), "state_not_at_head");

    // head advanced by 10 (lag 10): even the lag<=1 placements refuse.
    let mut engine10 = math_engine();
    engine10.register_fn("get_block_number", || -> Dynamic {
        Dynamic::from(20_000_010_i64)
    });
    let src1 = cartridge("mev_03_001_swap_backrun_arbitrage");
    let r1 = eval(&engine10, &src1, mispriced_cycle());
    assert!(!is_opp(&r1));
    assert_eq!(reason(&r1), "state_stale_post_event");
}

#[test]
fn g03_thin_pool_initialization_gate() {
    let engine = math_engine();
    // Deep-pool route: not the initialization venue.
    let src = cartridge("mev_03_020_pool_initialization_arbitrage");
    let r = eval(&engine, &src, mispriced_cycle());
    assert!(!is_opp(&r));
    assert_eq!(reason(&r), "no_thin_recent_pool_leg");

    // Route seeded by a thin (fresh) pool: hunts the initialization dislocation.
    let pd = closed_route_pool_data(vec![
        leg("0xnew1", "0xa", "0xb", 30),
        leg("0xba2", "0xb", "0xa", 30),
    ]);
    let r2 = eval(&engine, &src, pd);
    assert!(is_opp(&r2), "thin fresh pool + mispriced mirror must hunt");
    assert_eq!(reason(&r2), "pool_initialization_dislocation_profit");
}

// ─────────────────────────────────────────────────────────────────────────────
// 3. G03 divergence triggers (E_LATENCY / E_ORACLE)
// ─────────────────────────────────────────────────────────────────────────────

/// A→B on a 1:1.06 pool then B→A on the 1:1.21 mirror: leg-0 marginal sits
/// ~5.7% above the (1:1) oracle reference — well past every divergence band —
/// and the closed cycle itself is ~20% profitable.
fn divergent_cycle() -> Map {
    closed_route_pool_data(vec![
        leg("0xab9", "0xa", "0xb", 30),
        leg("0xba2", "0xb", "0xa", 30),
    ])
}

#[test]
fn g03_latency_divergence_triggers() {
    let engine = math_engine();
    for file in [
        "mev_03_007_stale_price_arbitrage",
        "mev_03_008_latency_arbitrage",
        "mev_03_027_block_time_arbitrage",
    ] {
        let src = cartridge(file);
        let r = eval(&engine, &src, divergent_cycle());
        assert!(is_opp(&r), "{file} must hunt the 5.7% fast/slow divergence");
        assert!(r.get("estimated_profit").unwrap().as_float().unwrap() > 0.0);
        let z = r.get("divergence").unwrap().as_float().unwrap();
        assert!(z > 0.01, "{file} divergence hint above band: {z}");
    }

    // Flat 1:1 cycle with 30bps fees: 0.3% divergence is inside every
    // latency band (1.0%/1.2%/1.5%) — trigger refuses.
    let src7 = cartridge("mev_03_007_stale_price_arbitrage");
    let r = eval(&engine, &src7, mispriced_cycle());
    // leg-0 marginal 0.997 vs oracle 1.0 => 0.3% < 1.0% band.
    assert!(
        !is_opp(&r),
        "0.3% divergence must not trigger the 1% stale band"
    );
    assert_eq!(reason(&r), "divergence_below_band");
}

#[test]
fn g03_oracle_reprice_trigger() {
    let engine = math_engine();
    // E_ORACLE band is 0.2%: even the flat 1:1 leg-0 (0.3% fee-adj divergence)
    // clears the trigger, and the mispriced mirror closes the cycle.
    for file in [
        "mev_03_009_oracle_update_arbitrage",
        "mev_03_010_oev_oracle_extractable_value",
    ] {
        let src = cartridge(file);
        let r = eval(&engine, &src, mispriced_cycle());
        assert!(
            is_opp(&r),
            "{file} must hunt the oracle-lag repricing route"
        );
        assert!(r.get("estimated_profit").unwrap().as_float().unwrap() > 0.0);
    }

    // No oracle prices at all: honest miss, never a fabricated reference.
    let starved = no_price_engine();
    let src9 = cartridge("mev_03_009_oracle_update_arbitrage");
    let r = eval(&starved, &src9, mispriced_cycle());
    assert!(!is_opp(&r));
    assert_eq!(reason(&r), "oracle_round_unavailable");
}

#[test]
fn g03_sequencer_gate_l2_only() {
    // L1 (chain 1): no sequencer context — honest refusal.
    let engine = math_engine();
    let src = cartridge("mev_03_028_sequencer_latency_arbitrage");
    let r = eval(&engine, &src, divergent_cycle());
    assert!(!is_opp(&r));
    assert_eq!(reason(&r), "sequencer_context_unavailable");

    // L2 (chain 42161): same divergence math as the latency family.
    let l2 = l2_engine();
    let r2 = eval(&l2, &src, divergent_cycle());
    assert!(is_opp(&r2), "on an L2 the sequencer divergence must hunt");
    assert_eq!(reason(&r2), "sequencer_divergence_profit");
}

// ─────────────────────────────────────────────────────────────────────────────
// 4. G03 feed-gated transitions + auctions + observe
// ─────────────────────────────────────────────────────────────────────────────

/// Every waiting G03 detector keeps its exact reason even with a GOOD route:
/// the math goes live the moment the event feed lands.
#[test]
fn g03_state_feeds_wait_with_exact_reasons() {
    let engine = math_engine();
    let cases: &[(&str, &str)] = &[
        ("mev_03_011_rebase_arbitrage", "rebase_feed_unavailable"),
        (
            "mev_03_012_interest_index_update_arbitrage",
            "interest_index_feed_unavailable",
        ),
        (
            "mev_03_013_funding_update_arbitrage",
            "funding_feed_unavailable",
        ),
        (
            "mev_03_014_epoch_rollover_arbitrage",
            "epoch_feed_unavailable",
        ),
        (
            "mev_03_015_settlement_arbitrage",
            "settlement_feed_unavailable",
        ),
        (
            "mev_03_016_mint_burn_state_arbitrage",
            "mint_burn_feed_unavailable",
        ),
        (
            "mev_03_017_redemption_state_arbitrage",
            "redemption_state_feed_unavailable",
        ),
        (
            "mev_03_018_liquidity_add_arbitrage",
            "lp_event_feed_unavailable",
        ),
        (
            "mev_03_019_liquidity_removal_arbitrage",
            "lp_event_feed_unavailable",
        ),
        (
            "mev_03_021_migration_arbitrage",
            "migration_feed_unavailable",
        ),
        (
            "mev_03_022_fee_switch_arbitrage",
            "fee_param_feed_unavailable",
        ),
        (
            "mev_03_023_parameter_update_arbitrage",
            "param_update_feed_unavailable",
        ),
        (
            "mev_03_024_governance_execution_arbitrage",
            "governance_feed_unavailable",
        ),
        (
            "mev_03_031_fork_state_arbitrage",
            "fork_state_feed_unavailable",
        ),
    ];
    let pd = mispriced_cycle();
    for (file, expected) in cases {
        let src = cartridge(file);
        let r = eval(&engine, &src, pd.clone());
        assert!(!is_opp(&r), "{file} waits for its feed, never fabricates");
        assert_eq!(reason(&r), *expected, "{file}");
    }
}

fn auction_payload(deadline: i64) -> Map {
    let mut a = Map::new();
    a.insert(
        "venue".into(),
        Dynamic::from("liquidation_auction".to_string()),
    );
    a.insert("collateral_token".into(), Dynamic::from("0xa".to_string()));
    a.insert("pay_token".into(), Dynamic::from("0xb".to_string()));
    a.insert("lot_qty".into(), Dynamic::from(100.0_f64));
    a.insert("price_now".into(), Dynamic::from(0.7_f64)); // pay per collateral
    a.insert("deadline_ts".into(), Dynamic::from(deadline));
    a.insert("unwinder_pool".into(), Dynamic::from("0xba2".to_string()));
    let mut pd = mispriced_cycle();
    pd.insert("auction".into(), Dynamic::from(a));
    pd
}

#[test]
fn g03_auction_math_hunts_discounted_lot() {
    let engine = math_engine();
    // fair = 2000/2000 = 1.0 pay per collateral vs price_now 0.7 => 30% discount,
    // unwound through the 1:1.21 B/A pool (~0.82 marginal) — comfortably > 0.7.
    for file in [
        "mev_03_025_keeper_triggered_arbitrage",
        "mev_03_026_auction_clearing_arbitrage",
    ] {
        let src = cartridge(file);
        let r = eval(&engine, &src, auction_payload(1_800_000_000));
        assert!(is_opp(&r), "{file} must price the discounted lot");
        let profit = r.get("estimated_profit").unwrap().as_float().unwrap();
        assert!(profit > 0.0, "{file} auction profit positive, got {profit}");
    }

    // Deadline passed: the lot is not purchasable — honest refusal.
    let src = cartridge("mev_03_025_keeper_triggered_arbitrage");
    let r = eval(&engine, &src, auction_payload(1_000));
    assert!(!is_opp(&r));
    assert_eq!(reason(&r), "auction_deadline_passed");

    // Auction priced above fair value: nothing to extract.
    let mut over = auction_payload(1_800_000_000);
    let mut am = over.get("auction").unwrap().clone().cast::<Map>();
    am.insert("price_now".into(), Dynamic::from(1.2_f64));
    over.insert("auction".into(), Dynamic::from(am));
    let r2 = eval(&engine, &src, over);
    assert!(!is_opp(&r2));
    assert_eq!(reason(&r2), "auction_above_fair_value");
}

#[test]
fn g03_observe_only_never_fires() {
    let engine = math_engine();
    for file in [
        "mev_03_029_spam_arbitrage",
        "mev_03_030_reorg_time_bandit_arbitrage",
    ] {
        let src = cartridge(file);
        let r = eval(&engine, &src, mispriced_cycle());
        assert!(!is_opp(&r), "{file} is OBSERVE family — manifest law");
        assert_eq!(reason(&r), "observe_only_structured_evidence");
        let legs = r.get("evidence_route_legs").unwrap().as_int().unwrap();
        assert_eq!(legs, 2, "{file} reports the real route shape as evidence");
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// 5. G04 parity engines
// ─────────────────────────────────────────────────────────────────────────────

/// USDX -> USDY market leg on the 1:1.01 pool (1% pair deviation).
fn stable_pair_pd(pool: &str) -> Map {
    open_route_pool_data(vec![leg(pool, "0xs1", "0xs2", 10)])
}

#[test]
fn g04_cross_stable_pair_hunts_deviation() {
    let engine = math_engine();
    let src = cartridge("mev_04_002_cross_stablecoin_arbitrage");
    let r = eval(&engine, &src, stable_pair_pd("0xss1"));
    assert!(
        is_opp(&r),
        "0.5% stable-pair deviation > 0.2% band must hunt"
    );
    assert_eq!(reason(&r), "cross_stable_pair_profit");
    let profit = r.get("estimated_profit").unwrap().as_float().unwrap();
    assert!(profit > 0.0, "parity profit positive, got {profit}");
    let x = r.get("optimal_amount_in").unwrap().as_float().unwrap();
    assert!(
        x > 0.0,
        "golden-section sizing found a positive optimum: {x}"
    );

    // 0.1% pair deviation with 10bps fees: inside the band — no peg trade.
    let r2 = eval(&engine, &src, stable_pair_pd("0xss2"));
    assert!(!is_opp(&r2));
    assert_eq!(reason(&r2), "peg_within_band");

    // A non-stable pair is not a peg venue.
    let pd = open_route_pool_data(vec![leg("0xab1", "0xa", "0xb", 30)]);
    let r3 = eval(&engine, &src, pd);
    assert!(!is_opp(&r3));
    assert_eq!(reason(&r3), "not_stable_pair");
}

#[test]
fn g04_peg_pair_decimal_normalization() {
    let engine = math_engine();
    // USDX (18 dec) vs USDT6 (6 dec): 0.5% deviation across a 10^12 decimal
    // gap — the parity rate must be scaled to base units or the edge is garbage.
    let src = cartridge("mev_04_002_cross_stablecoin_arbitrage");
    let pd = open_route_pool_data(vec![leg("0xss6", "0xs1", "0xs6", 10)]);
    let r = eval(&engine, &src, pd);
    assert!(is_opp(&r), "decimal-normalized stable deviation must hunt");
    let edge = r.get("parity_edge").unwrap().as_float().unwrap();
    assert!(
        edge > 0.002 && edge < 0.01,
        "edge ~0.4% after decimal normalization, got {edge}"
    );
}

#[test]
fn g04_stable_peg_anchor_driver() {
    // USDX depegged to $0.99 (1% off anchor) with a 0.5% pair deviation:
    // the anchor driver fires and the oracle ratio (0.99) prices redemption.
    let engine = depeg_engine();
    let src = cartridge("mev_04_001_stablecoin_peg_arbitrage");
    let r = eval(&engine, &src, stable_pair_pd("0xss1"));
    assert!(is_opp(&r), "1% anchor deviation must drive the peg trade");
    assert_eq!(reason(&r), "stable_peg_redeem_profit");

    // Both sides anchored at $1: no depeg driver, no trade.
    let anchored = math_engine();
    let r2 = eval(&anchored, &src, stable_pair_pd("0xss1"));
    assert!(!is_opp(&r2));
    assert_eq!(reason(&r2), "peg_within_band");

    // No oracle at all: the anchor IS the NAV source — honest miss.
    let starved = no_price_engine();
    let r3 = eval(&starved, &src, stable_pair_pd("0xss1"));
    assert!(!is_opp(&r3));
    assert_eq!(reason(&r3), "nav_source_unavailable");

    // Collateralized variant shares the parity core (CDP par via oracle).
    let src4 = cartridge("mev_04_004_collateralized_stablecoin_arbitrage");
    let r4 = eval(&engine, &src4, stable_pair_pd("0xss1"));
    assert!(
        is_opp(&r4),
        "collateralized parity hunts the same deviation"
    );
    assert_eq!(reason(&r4), "collateralized_parity_redeem_profit");
}

/// WSTETH -> WETH market leg at 1:1 while the oracle parity is 3000/2500=1.2:
/// the LST trades ~17% under NAV — the classic LST discount capture.
fn lst_pd() -> Map {
    open_route_pool_data(vec![leg("0xwl1", "0xlst", "0xweth", 30)])
}

#[test]
fn g04_lst_nav_discount_capture() {
    let engine = math_engine();
    for file in [
        "mev_04_019_liquid_staking_token_arbitrage",
        "mev_04_021_liquid_restaking_token_arbitrage",
        "mev_04_022_cross_lst_arbitrage",
    ] {
        let src = cartridge(file);
        let r = eval(&engine, &src, lst_pd());
        assert!(is_opp(&r), "{file} must hunt the 17% NAV discount");
        let profit = r.get("estimated_profit").unwrap().as_float().unwrap();
        assert!(
            profit > 0.0,
            "{file} discount profit positive, got {profit}"
        );
        // Direction: pool UNDERPRICES the LST => acquire LST cheap, redeem.
        let dir = r
            .get("parity_direction")
            .unwrap()
            .clone()
            .into_string()
            .unwrap();
        assert_eq!(dir, "pool_short_s_in", "{file} direction");
    }
    let r = eval(
        &engine,
        &cartridge("mev_04_019_liquid_staking_token_arbitrage"),
        lst_pd(),
    );
    assert_eq!(reason(&r), "lst_nav_parity_profit");

    // Missing LST price: no NAV source, no trade.
    let starved = no_price_engine();
    let r2 = eval(
        &starved,
        &cartridge("mev_04_019_liquid_staking_token_arbitrage"),
        lst_pd(),
    );
    assert!(!is_opp(&r2));
    assert_eq!(reason(&r2), "nav_source_unavailable");
}

#[test]
fn g04_4626_and_index_share_nav_engines() {
    let engine = math_engine();
    // Share (WSTETH-like receipt) vs asset (WETH): same NAV-vs-market math.
    let cases: &[(&str, &str)] = &[
        (
            "mev_04_010_receipt_token_arbitrage",
            "receipt_nav_parity_profit",
        ),
        (
            "mev_04_011_erc_4626_share_price_arbitrage",
            "erc4626_nav_parity_profit",
        ),
        (
            "mev_04_012_vault_share_underlying_arbitrage",
            "vault_share_nav_parity_profit",
        ),
        (
            "mev_04_014_index_token_basket_arbitrage",
            "index_nav_parity_profit",
        ),
        (
            "mev_04_006_native_wrapped_arbitrage",
            "wrapped_parity_redeem_profit",
        ),
        (
            "mev_04_008_cross_wrapper_arbitrage",
            "cross_wrapper_parity_profit",
        ),
    ];
    for (file, why) in cases {
        let src = cartridge(file);
        let r = eval(&engine, &src, lst_pd());
        assert!(is_opp(&r), "{file} hunts the share-vs-NAV deviation");
        assert_eq!(reason(&r), *why);
        assert!(r.get("estimated_profit").unwrap().as_float().unwrap() > 0.0);
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// 6. G04 feed-gated redemption sources
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn g04_redemption_feeds_wait_with_exact_reasons() {
    let engine = math_engine();
    let cases: &[(&str, &str)] = &[
        (
            "mev_04_003_algorithmic_stablecoin_arbitrage",
            "algorithmic_supply_feed_unavailable",
        ),
        (
            "mev_04_005_mint_redeem_stablecoin_arbitrage",
            "mint_redeem_rate_unavailable",
        ),
        (
            "mev_04_007_canonical_bridged_token_arbitrage",
            "bridge_state_unavailable",
        ),
        (
            "mev_04_009_synthetic_underlying_arbitrage",
            "synthetic_venue_feed_unavailable",
        ),
        (
            "mev_04_013_lp_token_nav_arbitrage",
            "lp_composition_feed_unavailable",
        ),
        (
            "mev_04_015_etf_like_token_arbitrage",
            "issuer_nav_feed_unavailable",
        ),
        (
            "mev_04_016_rwa_nav_arbitrage",
            "issuer_nav_feed_unavailable",
        ),
        (
            "mev_04_017_rebasing_token_arbitrage",
            "rebase_state_feed_unavailable",
        ),
        (
            "mev_04_023_governance_wrapper_arbitrage",
            "lock_state_feed_unavailable",
        ),
        (
            "mev_04_024_vote_escrow_derivative_arbitrage",
            "vederiv_lock_feed_unavailable",
        ),
        (
            "mev_04_025_tokenized_position_arbitrage",
            "position_components_feed_unavailable",
        ),
        (
            "mev_04_026_principal_token_arbitrage",
            "pt_yt_rate_feed_unavailable",
        ),
        (
            "mev_04_027_yield_token_arbitrage",
            "pt_yt_rate_feed_unavailable",
        ),
        (
            "mev_04_028_pt_yt_parity_arbitrage",
            "pt_yt_rate_feed_unavailable",
        ),
        (
            "mev_04_029_cross_maturity_yield_arbitrage",
            "yield_curve_feed_unavailable",
        ),
        (
            "mev_04_030_fixed_yield_floating_yield_arbitrage",
            "yield_index_feed_unavailable",
        ),
    ];
    let pd = lst_pd();
    for (file, expected) in cases {
        let src = cartridge(file);
        let r = eval(&engine, &src, pd.clone());
        assert!(!is_opp(&r), "{file} waits for its feed, never fabricates");
        assert_eq!(reason(&r), *expected, "{file}");
    }
}

fn queue_payload(depth: i64, eta: i64) -> Map {
    let mut q = Map::new();
    q.insert("lst".into(), Dynamic::from("0xlst".to_string()));
    q.insert("nav_rate".into(), Dynamic::from(1.2_f64)); // WETH per LST
    q.insert("queue_depth".into(), Dynamic::from(depth));
    q.insert("eta_blocks".into(), Dynamic::from(eta));
    let mut pd = lst_pd();
    pd.insert("redemption_queue".into(), Dynamic::from(q));
    pd
}

#[test]
fn g04_lst_redemption_queue_gates_and_math() {
    let engine = math_engine();
    let src = cartridge("mev_04_020_lst_redemption_arbitrage");

    // Saturated queue: redemption leg unexecutable inside any actionable window.
    let r = eval(&engine, &src, queue_payload(500, 4));
    assert!(!is_opp(&r));
    assert_eq!(reason(&r), "redemption_queue_full");

    // Live queue with the NAV rate: the discount clears the queue-carry band.
    let r2 = eval(&engine, &src, queue_payload(10, 5));
    assert!(
        is_opp(&r2),
        "17% discount clears a 5-block queue-carry band"
    );
    assert_eq!(reason(&r2), "lst_redemption_queue_profit");
    assert!(r2.get("estimated_profit").unwrap().as_float().unwrap() > 0.0);
}

#[test]
fn g04_fot_fee_folding() {
    let engine = math_engine();
    let src = cartridge("mev_04_018_fee_on_transfer_token_arbitrage");

    // FOT feed present: the transfer levy folds into the executable gamma.
    let mut fs = Map::new();
    fs.insert("token".into(), Dynamic::from("0xs1".to_string()));
    fs.insert("fee_bps_on_transfer".into(), Dynamic::from(10_i64));
    let mut pd = stable_pair_pd("0xss1");
    pd.insert("fot_state".into(), Dynamic::from(fs));
    let r = eval(&engine, &src, pd);
    assert!(
        is_opp(&r),
        "0.5% deviation still clears with 10bps FOT folded in"
    );
    assert_eq!(reason(&r), "fot_parity_profit");

    // A pathological 100% transfer fee kills every quote — honest refusal.
    let mut fs2 = Map::new();
    fs2.insert("token".into(), Dynamic::from("0xs1".to_string()));
    fs2.insert("fee_bps_on_transfer".into(), Dynamic::from(9990_i64));
    let mut pd2 = stable_pair_pd("0xss1");
    pd2.insert("fot_state".into(), Dynamic::from(fs2));
    let r2 = eval(&engine, &src, pd2);
    assert!(!is_opp(&r2));
    assert_eq!(reason(&r2), "fot_fee_invalid");
}

#[test]
fn g04_observe_only_never_fires() {
    let engine = math_engine();
    let src = cartridge("mev_04_031_points_pre_token_arbitrage");
    let r = eval(&engine, &src, lst_pd());
    assert!(
        !is_opp(&r),
        "points/pre-token is OBSERVE family — manifest law"
    );
    assert_eq!(reason(&r), "observe_only_structured_evidence");
}

// ─────────────────────────────────────────────────────────────────────────────
// 7. build_payload stays observe-only for every wave-B cartridge
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn wave_b_payloads_observe_only() {
    let engine = math_engine();
    let mut files: Vec<String> = Vec::new();
    for entry in std::fs::read_dir(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("cartridges/strategies"),
    )
    .unwrap()
    {
        let name = entry.unwrap().file_name().to_string_lossy().to_string();
        if name.starts_with("mev_03_") || name.starts_with("mev_04_") {
            files.push(name.trim_end_matches(".rhai").to_string());
        }
    }
    files.sort();
    assert_eq!(files.len(), 62, "wave B payload sweep = 62 cartridges");
    for file in files {
        let src = cartridge(&file);
        let ast = engine.compile(&src).unwrap();
        let mut scope = rhai::Scope::new();
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
