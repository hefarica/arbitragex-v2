//! MARKET-FEATURES-01 — tests.
//!
//! Every test asserts a PROPERTY the contract promises, not that the code runs.
//! The suite is organised as:
//!
//! 1. `volatility` — present with a real datum; absent without one.
//! 2. `oracle_price` / `onchain_price` — present with a fresh fixture; absent
//!    otherwise; atomicity (both or neither); per-asset-class staleness.
//! 3. **Fabrication guards** — the core R8 doctrine: absence is never `0.0`,
//!    and a real `0.0` is never reported as absence.
//! 4. **Anti-duplication** — a key another module already owns is never emitted
//!    here.
//! 5. Contract completeness (unit / source / window / absent-means declared).
//! 6. **End-to-end unlock** — the produced map actually moves the real
//!    `RegimeRouter`, which is what "unblocks the native operators" means.

use super::*;
use math_engine::control::regime_router::{Regime, RegimeRouter};
use math_engine::operators::MarketState;
use shared_rs::chains::is_stablecoin_symbol;
use shared_rs::price_bus::{Anchor, PriceBus, PriceBusConfig, PriceSnapshot};
use std::collections::HashSet;

// ───────────────────────────── fixtures ─────────────────────────────

/// ~2023-11-14, an arbitrary but realistic wall-clock epoch in ns.
const T0: u64 = 1_700_000_000_000_000_000;
const SEC: u64 = 1_000_000_000;

/// The measured series. Its sample standard deviation (ddof = 1) of the log
/// returns was computed INDEPENDENTLY in Python (`statistics.stdev`, which
/// matched an explicit `ddof=1` computation to <1e-15) — the constant below is
/// that external reference, so this test cannot pass by re-deriving its own
/// answer:
///
/// ```text
/// p = [100,101,99,102,100,103,98,101] ; 7 returns ; sample sd = 0.031694824469859523
/// ```
const REFERENCE_PRICES: [f64; 8] = [100.0, 101.0, 99.0, 102.0, 100.0, 103.0, 98.0, 101.0];
/// The same `f64`, written at the fewest digits that still identify it (clippy
/// `excessive_precision`); the 17-digit decimal `0.031694824469859523` above is
/// the external reference it came from.
const REFERENCE_SD: f64 = 0.031_694_824_469_859_52;

/// Two minutes apart, so the 8-sample span is 420 s — inside the default 900 s
/// window and comfortably above the 60 s minimum span.
fn reference_request(i: usize) -> FeatureRequest<'static> {
    FeatureRequest {
        symbol: "WETH",
        onchain_price_usd: None,
        live_price_usd: Some(REFERENCE_PRICES[i]),
    }
}

fn fresh_anchor(answer: f64, now_ns: u64) -> Anchor {
    Anchor {
        answer,
        updated_at: now_ns / SEC,
        recv_ns: now_ns,
    }
}

fn snapshot_with(symbol: &str, anchor: Anchor) -> PriceSnapshot {
    let mut snap = PriceSnapshot::default();
    snap.chainlink.insert(symbol.to_owned(), anchor);
    snap
}

fn empty_state_with(features: std::collections::HashMap<String, f64>) -> MarketState {
    MarketState {
        price_matrix: vec![vec![100.0], vec![101.0], vec![99.0]],
        pair_keys: vec!["a|b".to_owned(); 3],
        liquidity_reserves: vec![(1.0, 1.0); 3],
        gas_price_gwei: 10.0,
        block_timestamp: 0,
        block_number: 1,
        features,
    }
}

// ───────────────────── 1. `volatility` ─────────────────────

#[test]
fn volatility_is_present_once_the_real_time_axis_supports_it() {
    let mut store = SeriesStore::default();
    let cfg = FeatureConfig::default();
    let mut last = std::collections::HashMap::new();

    // Eight ticks, one real observation each — exactly how the producer is used.
    for i in 0..REFERENCE_PRICES.len() {
        let out = produce(
            &mut store,
            &cfg,
            &reference_request(i),
            None,
            T0 + (i as u64) * 60 * SEC,
        );
        if i + 1 < DEFAULT_MIN_SAMPLES {
            assert!(
                !out.contains_key(VOLATILITY_KEY),
                "with only {} observations the key must be ABSENT, got {:?}",
                i + 1,
                out.get(VOLATILITY_KEY)
            );
        }
        last = out;
    }

    let got = *last
        .get(VOLATILITY_KEY)
        .expect("8 real observations over 420 s must produce the key");
    assert!(
        (got - REFERENCE_SD).abs() < 1e-12,
        "expected the externally computed sample sd {REFERENCE_SD}, got {got}"
    );
}

#[test]
fn volatility_is_absent_with_too_few_samples() {
    let mut store = SeriesStore::default();
    let cfg = FeatureConfig::default();
    let mut out = std::collections::HashMap::new();
    for i in 0..(DEFAULT_MIN_SAMPLES - 1) {
        out = produce(
            &mut store,
            &cfg,
            &reference_request(i),
            None,
            T0 + (i as u64) * 60 * SEC,
        );
    }
    assert!(
        !out.contains_key(VOLATILITY_KEY),
        "7 observations is below the 8-sample minimum: absence, not a number"
    );
}

#[test]
fn volatility_is_absent_when_the_span_is_too_short() {
    // Enough SAMPLES but not enough TIME: 8 observations inside 7 ms. That is
    // real arithmetic over a meaningless axis, and the module must refuse it.
    let mut store = SeriesStore::default();
    let cfg = FeatureConfig::default();
    let mut out = std::collections::HashMap::new();
    for i in 0..REFERENCE_PRICES.len() {
        out = produce(
            &mut store,
            &cfg,
            &reference_request(i),
            None,
            T0 + (i as u64) * 1_000_000, // 1 ms apart
        );
    }
    assert_eq!(store.len("WETH"), REFERENCE_PRICES.len());
    assert!(
        !out.contains_key(VOLATILITY_KEY),
        "a 7 ms span cannot support a volatility measurement: absence, not a tiny number"
    );
}

#[test]
fn a_flat_price_over_a_real_span_is_zero_and_that_is_not_absence() {
    // The mirror-image lie: a genuine measurement of "did not move" must be
    // reported as 0.0. `None = not computed`; `Some(0.0) = computed, exactly
    // zero` (R8, CLAUDE.md §3 R8).
    let mut store = SeriesStore::default();
    let cfg = FeatureConfig::default();
    for i in 0..REFERENCE_PRICES.len() {
        let req = FeatureRequest {
            symbol: "WETH",
            onchain_price_usd: None,
            live_price_usd: Some(100.0),
        };
        let out = produce(&mut store, &cfg, &req, None, T0 + (i as u64) * 60 * SEC);
        if i + 1 == REFERENCE_PRICES.len() {
            assert_eq!(
                out.get(VOLATILITY_KEY).copied(),
                Some(0.0),
                "a flat series over 420 s IS zero volatility"
            );
        }
    }
}

#[test]
fn invalid_prices_are_not_observations() {
    for bad in [0.0, -1.0, f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        let mut store = SeriesStore::default();
        assert!(
            !store.observe("WETH", T0, bad),
            "{bad} must be rejected as an observation"
        );
        assert_eq!(store.len("WETH"), 0);
    }
}

#[test]
fn non_monotone_timestamps_are_rejected() {
    let mut store = SeriesStore::default();
    assert!(store.observe("WETH", T0 + 10, 100.0));
    assert!(!store.observe("WETH", T0 + 10, 101.0), "duplicate ts");
    assert!(!store.observe("WETH", T0 + 5, 101.0), "out-of-order ts");
    assert_eq!(store.len("WETH"), 1);
}

#[test]
fn observations_older_than_the_window_are_pruned() {
    let mut store = SeriesStore::default();
    let w = VolatilityWindow::default();
    for i in 0..4u64 {
        assert!(store.observe("WETH", T0 + i * 100 * SEC, 100.0 + i as f64));
    }
    // Read 1 hour later: every observation is outside the 900 s window.
    let windowed = store.windowed("WETH", w, T0 + 3_600 * SEC);
    assert!(windowed.is_empty(), "stale observations must not be used");
    assert_eq!(store.len("WETH"), 0, "and must be pruned from the ring");
}

// ─────────────── 2. `oracle_price` / `onchain_price` ───────────────

#[test]
fn the_oracle_pair_is_present_from_a_fresh_anchor_fixture() {
    let snap = snapshot_with("WETH", fresh_anchor(3500.0, T0));
    let cfg = PriceBusConfig::default();
    let sources = OracleSources {
        snapshot: &snap,
        cfg: &cfg,
    };
    let mut store = SeriesStore::default();
    let req = FeatureRequest {
        symbol: "weth", // lower case on purpose: the producer normalises
        onchain_price_usd: Some(3400.0),
        live_price_usd: None,
    };
    let out = produce(
        &mut store,
        &FeatureConfig::default(),
        &req,
        Some(sources),
        T0,
    );

    assert_eq!(out.get(ORACLE_PRICE_KEY).copied(), Some(3500.0));
    assert_eq!(out.get(ONCHAIN_PRICE_KEY).copied(), Some(3400.0));
    // The value the router will derive from exactly these two numbers.
    let bias = (3500.0f64 - 3400.0f64).abs() / 3400.0f64;
    assert!((bias - 0.029_411_764_705_882_35).abs() < 1e-15);
}

#[test]
fn the_oracle_pair_is_absent_when_the_anchor_is_stale_for_a_volatile_asset() {
    // ETH heartbeat is 1 h; the anchor is 2 h old.
    let stale = Anchor {
        answer: 3500.0,
        updated_at: T0 / SEC - 7_200,
        recv_ns: T0,
    };
    let snap = snapshot_with("WETH", stale);
    let cfg = PriceBusConfig::default();
    let mut store = SeriesStore::default();
    let req = FeatureRequest {
        symbol: "WETH",
        onchain_price_usd: Some(3400.0),
        live_price_usd: None,
    };
    let out = produce(
        &mut store,
        &FeatureConfig::default(),
        &req,
        Some(OracleSources {
            snapshot: &snap,
            cfg: &cfg,
        }),
        T0,
    );
    assert!(
        !out.contains_key(ORACLE_PRICE_KEY) && !out.contains_key(ONCHAIN_PRICE_KEY),
        "a 2 h old ETH anchor is stale: absence, and BOTH keys (they are atomic)"
    );
}

#[test]
fn a_stablecoin_anchor_survives_13_7h_because_its_heartbeat_is_24h() {
    // Observed live on 2026-09-20: a healthy USDC feed 13.7 h old. One shared
    // staleness number would either throw this away or accept a dead ETH feed.
    assert!(
        is_stablecoin_symbol("USDC"),
        "premise: USDC is a recognised stablecoin"
    );
    let healthy = Anchor {
        answer: 0.999_84,
        updated_at: T0 / SEC - 49_320, // 13.7 h
        recv_ns: T0,
    };
    let snap = snapshot_with("USDC", healthy);
    let cfg = PriceBusConfig::default();
    let mut store = SeriesStore::default();
    let req = FeatureRequest {
        symbol: "USDC",
        onchain_price_usd: Some(1.000_1),
        live_price_usd: None,
    };
    let out = produce(
        &mut store,
        &FeatureConfig::default(),
        &req,
        Some(OracleSources {
            snapshot: &snap,
            cfg: &cfg,
        }),
        T0,
    );
    assert_eq!(out.get(ORACLE_PRICE_KEY).copied(), Some(0.999_84));
}

#[test]
fn the_oracle_pair_is_atomic_and_never_half_emitted() {
    let snap = snapshot_with("WETH", fresh_anchor(3500.0, T0));
    let cfg = PriceBusConfig::default();
    let sources = OracleSources {
        snapshot: &snap,
        cfg: &cfg,
    };

    // Oracle available, on-chain price missing -> NOTHING is emitted.
    let mut store = SeriesStore::default();
    let req = FeatureRequest {
        symbol: "WETH",
        onchain_price_usd: None,
        live_price_usd: None,
    };
    let out = produce(
        &mut store,
        &FeatureConfig::default(),
        &req,
        Some(sources),
        T0,
    );
    assert!(
        out.is_empty(),
        "half a measurement must not be published: {out:?}"
    );

    // On-chain price available, no snapshot at all -> NOTHING is emitted.
    let req2 = FeatureRequest {
        symbol: "WETH",
        onchain_price_usd: Some(3400.0),
        live_price_usd: None,
    };
    let out2 = produce(&mut store, &FeatureConfig::default(), &req2, None, T0);
    assert!(out2.is_empty(), "no oracle source, no pair: {out2:?}");
}

#[test]
fn the_oracle_pair_rejects_unusable_prices() {
    let cfg = PriceBusConfig::default();
    for bad_onchain in [Some(0.0), Some(-1.0), Some(f64::NAN), Some(f64::INFINITY)] {
        let snap = snapshot_with("WETH", fresh_anchor(3500.0, T0));
        let mut store = SeriesStore::default();
        let req = FeatureRequest {
            symbol: "WETH",
            onchain_price_usd: bad_onchain,
            live_price_usd: None,
        };
        let out = produce(
            &mut store,
            &FeatureConfig::default(),
            &req,
            Some(OracleSources {
                snapshot: &snap,
                cfg: &cfg,
            }),
            T0,
        );
        assert!(out.is_empty(), "onchain {bad_onchain:?} must be refused");
    }

    // A zero/negative oracle answer is a broken feed, not a free asset.
    for bad_answer in [0.0, -1.0, f64::NAN] {
        let snap = snapshot_with("WETH", fresh_anchor(bad_answer, T0));
        let mut store = SeriesStore::default();
        let req = FeatureRequest {
            symbol: "WETH",
            onchain_price_usd: Some(3400.0),
            live_price_usd: None,
        };
        let out = produce(
            &mut store,
            &FeatureConfig::default(),
            &req,
            Some(OracleSources {
                snapshot: &snap,
                cfg: &cfg,
            }),
            T0,
        );
        assert!(out.is_empty(), "oracle answer {bad_answer} must be refused");
    }
}

// ───────────────── 3. Fabrication guards (R8) ─────────────────

#[test]
fn no_key_is_inserted_as_zero_when_the_data_is_missing() {
    // Nothing available at all: no live price, no on-chain price, no snapshot.
    let mut store = SeriesStore::default();
    let req = FeatureRequest {
        symbol: "WETH",
        onchain_price_usd: None,
        live_price_usd: None,
    };
    let out = produce(&mut store, &FeatureConfig::default(), &req, None, T0);

    assert!(
        out.is_empty(),
        "with no real data the map must be EMPTY, got {out:?}"
    );
    for key in OWNED_KEYS {
        assert!(
            !out.contains_key(*key),
            "`{key}` must be ABSENT, never defaulted"
        );
    }
    assert!(
        !out.values().any(|v| *v == 0.0),
        "no fabricated 0.0 may appear anywhere: {out:?}"
    );
}

#[test]
fn a_tick_without_a_live_price_does_not_advance_the_series() {
    let mut store = SeriesStore::default();
    let req = FeatureRequest {
        symbol: "WETH",
        onchain_price_usd: None,
        live_price_usd: None,
    };
    for i in 0..10u64 {
        produce(
            &mut store,
            &FeatureConfig::default(),
            &req,
            None,
            T0 + i * 60 * SEC,
        );
    }
    assert_eq!(
        store.len("WETH"),
        0,
        "an absent live price must not be padded with the on-chain price or a repeat"
    );
}

#[test]
fn symbol_case_does_not_split_one_series_in_two() {
    let mut store = SeriesStore::default();
    let cfg = FeatureConfig::default();
    for (i, sym) in ["WETH", "weth", " Weth "].iter().enumerate() {
        let req = FeatureRequest {
            symbol: sym,
            onchain_price_usd: None,
            live_price_usd: Some(REFERENCE_PRICES[i]),
        };
        produce(&mut store, &cfg, &req, None, T0 + (i as u64) * 60 * SEC);
    }
    assert_eq!(store.len("WETH"), 3, "one canonical spelling per token");
}

// ─────────────── 4. Anti-duplication of live producers ───────────────

#[test]
fn already_produced_keys_are_never_emitted_here() {
    // A MAXIMAL request: every input this module can consume is provided, so
    // there is no excuse of "the data was missing" for a forbidden key.
    let snap = snapshot_with("WETH", fresh_anchor(3500.0, T0));
    let cfg = PriceBusConfig::default();
    let mut store = SeriesStore::default();
    let mut out = std::collections::HashMap::new();
    for i in 0..REFERENCE_PRICES.len() + 2 {
        let idx = i.min(REFERENCE_PRICES.len() - 1);
        let req = FeatureRequest {
            symbol: "WETH",
            onchain_price_usd: Some(3400.0),
            live_price_usd: Some(REFERENCE_PRICES[idx]),
        };
        out = produce(
            &mut store,
            &FeatureConfig::default(),
            &req,
            Some(OracleSources {
                snapshot: &snap,
                cfg: &cfg,
            }),
            T0 + (i as u64) * 60 * SEC,
        );
    }
    // Sanity: the module DID produce its own keys, so the absence assertions
    // below are about ownership, not about an empty map.
    assert!(out.contains_key(VOLATILITY_KEY));
    assert!(out.contains_key(ORACLE_PRICE_KEY));

    for forbidden in ["parity_deviation", "health_factor", "arbitrage_gap"] {
        assert!(
            !out.contains_key(forbidden),
            "`{forbidden}` already has an owner (or no consumer) — re-producing it \
             here would be duplication, not implementation"
        );
        let contract = contract_for(forbidden).expect("documented");
        assert!(
            !matches!(contract.owner, Owner::ThisModule),
            "`{forbidden}` must not be claimed by this module"
        );
    }
}

// ───────────────── 5. Contract completeness ─────────────────

#[test]
fn every_owned_key_has_a_complete_contract() {
    // F2 escribio tres productores (`volatility`, `oracle_price`,
    // `onchain_price`). F7 anadio los CINCO que el censo declaraba ABSENT y que
    // tienen lector real (`pool_fee`, `fee_bps`, `flash_premium`,
    // `max_capital`, `break_even_target`) — el porque de cada uno y la unidad
    // que su lector exige estan medidos en `cost_inputs.rs`. Las 16 claves
    // ABSENT restantes NO se producen: nadie las lee.
    assert_eq!(
        OWNED_KEYS.len(),
        8,
        "tres de F2 + los cinco del censo con lector real (F7)"
    );

    for key in OWNED_KEYS {
        let c = contract_for(key).unwrap_or_else(|| panic!("no contract for `{key}`"));
        assert!(matches!(c.owner, Owner::ThisModule));
        for (field, value) in [
            ("unit", c.unit),
            ("source", c.source),
            ("window", c.window),
            ("absent_means", c.absent_means),
        ] {
            assert!(
                value.trim().len() > 20,
                "`{key}`.{field} must be a real declaration, got {value:?}"
            );
        }
    }

    let owned: HashSet<&str> = owned_contracts().map(|c| c.key).collect();
    let declared: HashSet<&str> = OWNED_KEYS.iter().copied().collect();
    assert_eq!(
        owned, declared,
        "owned_contracts() and OWNED_KEYS must agree"
    );
}

#[test]
fn the_contract_table_has_no_duplicates_and_covers_the_other_keys() {
    let mut seen = HashSet::new();
    for c in CONTRACTS {
        assert!(seen.insert(c.key), "duplicate contract for `{}`", c.key);
    }
    // The two live producers and the no-consumer key must all be documented,
    // each pointing at its evidence.
    for key in ["parity_deviation", "health_factor", "arbitrage_gap"] {
        let c = contract_for(key).unwrap_or_else(|| panic!("`{key}` undocumented"));
        match c.owner {
            Owner::Elsewhere(site) => assert!(
                site.contains(".rs:"),
                "`{key}` must cite a file:line, got {site:?}"
            ),
            Owner::NoConsumer(why) => assert!(
                why.contains(".rs:"),
                "`{key}` must cite the reasoning site, got {why:?}"
            ),
            Owner::ThisModule => panic!("`{key}` is not this module's to produce"),
        }
    }
}

#[test]
fn monetary_keys_are_declared_as_such_and_the_limitation_is_recorded() {
    // Doctrine §4: no f64 for monetary amounts. `features` is HashMap<String,f64>
    // by design, so a monetary key cannot avoid the type — the contract records
    // it, and the values are unit PRICES whose only consumer uses a
    // scale-invariant ratio.
    for key in [ORACLE_PRICE_KEY, ONCHAIN_PRICE_KEY] {
        let c = contract_for(key).unwrap();
        assert!(c.monetary, "`{key}` is a USD magnitude and must say so");
        assert!(
            c.unit.contains("USD") && c.unit.contains("precio unitario"),
            "`{key}` must declare itself a unit price, not an amount: {}",
            c.unit
        );
    }
    assert!(
        !contract_for(VOLATILITY_KEY).unwrap().monetary,
        "volatility is dimensionless"
    );
    // No owned key may describe itself as an amount.
    for key in OWNED_KEYS {
        let c = contract_for(key).unwrap();
        assert!(
            !c.unit.contains("importe") && !c.unit.contains("amount"),
            "`{key}` must not be an amount (doctrine §4)"
        );
    }
}

// ─────────────── 6. End-to-end unlock of the real router ───────────────

#[test]
fn produced_volatility_replaces_the_cross_pair_proxy_in_the_real_router() {
    // A `price_matrix` whose rows are wildly dispersed: the router's cross-venue
    // proxy would report a large "volatility" from cross-PAIR dispersion
    // (regime_router.rs:115-135). With a REAL series the produced value must win
    // (FEATURES-01c preference, regime_router.rs:107) — that is the whole point
    // of the producer.
    let mut store = SeriesStore::default();
    let cfg = FeatureConfig::default();
    let mut features = std::collections::HashMap::new();
    for i in 0..REFERENCE_PRICES.len() {
        features = produce(
            &mut store,
            &cfg,
            &reference_request(i),
            None,
            T0 + (i as u64) * 60 * SEC,
        );
    }
    let produced = *features.get(VOLATILITY_KEY).expect("produced");

    let mut state = empty_state_with(features);
    state.price_matrix = vec![vec![100.0], vec![200.0], vec![400.0]];

    let metrics = RegimeRouter::analyze(&state);
    assert_eq!(
        metrics.volatility,
        Some(produced),
        "the real measurement must take precedence over the proxy"
    );
    assert!(
        produced < 0.05,
        "sanity: the produced value ({produced}) differs from the proxy magnitude"
    );
}

#[test]
fn the_produced_oracle_pair_unlocks_the_oracle_bias_regime() {
    // End-to-end: a fixture anchor + on-chain price -> produced features ->
    // `RegimeRouter` derives a real oracle bias and classifies OracleBias.
    // Before this producer, `oracle_bias` was unconditionally `None`.
    let now_ns = T0;
    let snap = snapshot_with("WETH", fresh_anchor(3500.0, now_ns));
    let cfg = PriceBusConfig::default();
    let mut store = SeriesStore::default();
    let req = FeatureRequest {
        symbol: "WETH",
        onchain_price_usd: Some(3400.0),
        live_price_usd: None,
    };
    let features = produce(
        &mut store,
        &FeatureConfig::default(),
        &req,
        Some(OracleSources {
            snapshot: &snap,
            cfg: &cfg,
        }),
        now_ns,
    );

    let state = empty_state_with(features);
    let metrics = RegimeRouter::analyze(&state);
    let bias = metrics
        .oracle_bias
        .expect("oracle_bias was unconditionally None before this producer");
    assert!((bias - 0.029_411_764_705_882_35).abs() < 1e-15);

    let regimes = RegimeRouter::default().classify(&metrics);
    assert!(
        regimes.contains(&Regime::OracleBias),
        "a 2.94% oracle bias must classify OracleBias (threshold 0.2%), got {regimes:?}"
    );
}

#[test]
fn without_the_producer_the_router_reports_no_oracle_bias_at_all() {
    // The counterfactual that motivates the task: same market state, NO features
    // -> `oracle_bias` is `None`. This is what `DATA_GAP` looked like.
    let state = empty_state_with(std::collections::HashMap::new());
    let metrics = RegimeRouter::analyze(&state);
    assert_eq!(metrics.oracle_bias, None);
    assert!(
        metrics.volatility.is_some(),
        "the cross-venue proxy still fills volatility — which is exactly why a real \
         measurement is needed to override it"
    );
}

// ─────────────── 7. The global adapter is fail-honest ───────────────

#[test]
fn produce_from_global_without_a_price_bus_neither_panics_nor_fabricates() {
    // No bus (a unit test that never boots `main.rs`): the adapter must take its
    // `None` branch, return no oracle keys, and invent no observation. This also
    // pins the reason the bus is a PARAMETER — `crate::price_bus_global` is
    // bin-only, so the lib target cannot look it up itself.
    let out = produce_from_global(None, "WETH", Some(3400.0), T0);
    assert!(!out.contains_key(ORACLE_PRICE_KEY));
    assert!(!out.contains_key(ONCHAIN_PRICE_KEY));
    assert!(
        !out.contains_key(VOLATILITY_KEY),
        "one tick is never enough for a realized volatility"
    );
}

#[test]
fn produce_from_global_accepts_a_missing_onchain_price() {
    let out = produce_from_global(None, "WETH", None, T0);
    assert!(
        !out.contains_key(ONCHAIN_PRICE_KEY),
        "no on-chain price -> no on-chain key"
    );
}

#[test]
fn produce_from_global_produces_the_pair_from_a_real_bus() {
    // The full adapter path, not just the pure core: real bus -> fused live
    // price resolved by the bus -> anchor read -> pair emitted.
    let bus: std::sync::Arc<PriceBus> = PriceBus::new(PriceBusConfig::default());
    bus.update_anchor("WETH", fresh_anchor(3500.0, T0));
    bus.update_anchor("USDC", fresh_anchor(1.0, T0));
    let ticker = shared_rs::price_bus::BookTicker {
        bid: 3500.0,
        ask: 3500.5,
        event_ms: 0,
        recv_ns: T0,
    };
    bus.update_binance("ETHUSDC", ticker);

    let out = produce_from_global(Some(bus.as_ref()), "WETH", Some(3400.0), T0);
    assert_eq!(out.get(ORACLE_PRICE_KEY).copied(), Some(3500.0));
    assert_eq!(out.get(ONCHAIN_PRICE_KEY).copied(), Some(3400.0));
}

// ─────────────── 8. A real PriceBus round trip ───────────────

#[test]
fn a_real_price_bus_anchor_reaches_the_feature_map() {
    // Writes through the bus's own public writer, reads through the module: no
    // fixture hand-rolling of the snapshot in this test.
    let bus: std::sync::Arc<PriceBus> = PriceBus::new(PriceBusConfig::default());
    bus.update_anchor("WETH", fresh_anchor(3500.0, T0));

    let view = bus.view();
    let mut store = SeriesStore::default();
    let req = FeatureRequest {
        symbol: "WETH",
        onchain_price_usd: Some(3400.0),
        live_price_usd: None,
    };
    let out = produce(
        &mut store,
        &FeatureConfig::default(),
        &req,
        Some(OracleSources {
            snapshot: view.snapshot(),
            cfg: bus.config(),
        }),
        T0,
    );
    assert_eq!(out.get(ORACLE_PRICE_KEY).copied(), Some(3500.0));
    assert_eq!(out.get(ONCHAIN_PRICE_KEY).copied(), Some(3400.0));
}
