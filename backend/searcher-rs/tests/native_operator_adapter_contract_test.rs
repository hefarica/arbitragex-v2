//! F3-NATIVEADAPTER-01 — contrato de entrada del adaptador de operadores nativos.
//!
//! Prueba las DOS direcciones del cableado, en el adaptador REAL
//! (`searcher_rs::native_operator_adapter::evaluate_declared`), no en una copia:
//!
//! 1. Con las claves de `MarketState.features` pobladas desde fuentes reales, los
//!    operadores que LEEN `features` (11 Bayes, 21 Newton, 26 FlashLoan) dejan de
//!    devolver DATA_GAP y publican un valor finito. Para op_21 eso incluye el
//!    OBJETIVO de break-even, que su guarda exige desde FEATURES-DEFAULTS-01
//!    (OP21-CONTRACT-01).
//! 2. Con las claves AUSENTES, siguen devolviendo DATA_GAP **nombrando la clave
//!    exacta** — y jamás un `0.0` fabricado (`value`/`scalar` en `null`). RULE 00
//!    / R8: un DATA_GAP honesto vale más que un cero inventado.
//! 3. Los seis operadores de serie (5, 8, 10, 13, 16, 22) NO dependen de
//!    `features`: su productor es una SERIE de precios del mismo par. Se prueba
//!    que con la serie computan y sin ella el recibo nombra la entrada que falta.
//! 4. Propiedad de seguridad del contrato: si declara entradas faltantes, el
//!    operador NO puede computar (el contrato nunca bloquea a un operador que sí
//!    computa — no hay falso rojo).

use math_engine::operators::{MarketState, OperatorRegistry};
use searcher_rs::native_operator_adapter::{
    declared_contract, evaluate_declared, market_features_from_sources, missing_required_inputs,
    price_series_pair_scope, ContractInputAdmission, OperatorFeatureSources,
    OperatorInputAdmission,
};
use serde_json::{json, Value};
use std::collections::HashMap;

/// Admisión estructural IDÉNTICA a la del call site v4
/// (`cartridge_boot.rs:1446-1471`): así, un DATA_GAP observado en estos tests
/// viene del operador o de su contrato, nunca de la admisión de test.
struct StructuralLike;

impl OperatorInputAdmission for StructuralLike {
    fn validate(
        &self,
        _id: u8,
        state: &MarketState,
        snapshot_id: &str,
        plan_hash: &str,
    ) -> Result<String, String> {
        if state.price_matrix.is_empty() {
            return Err("market_state_price_matrix_empty".into());
        }
        if state.pair_keys.len() != state.price_matrix.len() {
            return Err("market_state_pair_keys_misaligned".into());
        }
        if !state.gas_price_gwei.is_finite() || state.gas_price_gwei <= 0.0 {
            return Err("market_state_gas_not_positive".into());
        }
        if state.block_number == 0 {
            return Err("market_state_block_unknown".into());
        }
        Ok(format!(
            "structural:{snapshot_id}:{plan_hash}:{}px",
            state.price_matrix.len()
        ))
    }
}

fn spec(ids: &[u8]) -> Value {
    json!({
        "operator_requirements": ids
            .iter()
            .map(|id| json!({
                "id": id,
                "role": "primary",
                "phase": "contract_test",
                "requirement": "f3_native_adapter"
            }))
            .collect::<Vec<Value>>()
    })
}

fn run(state: &MarketState, ids: &[u8]) -> Value {
    let registry = OperatorRegistry::new();
    evaluate_declared(
        &registry,
        state,
        &spec(ids),
        "snap-f3",
        "plan-f3",
        &StructuralLike,
        |_| false,
    )
    .expect("evaluate_declared debe admitir el contexto de test")
}

fn receipt(out: &Value, id: u8) -> &Value {
    &out["operators"][id.to_string()]
}

fn status(out: &Value, id: u8) -> String {
    receipt(out, id)["status"]
        .as_str()
        .unwrap_or("<sin status>")
        .to_string()
}

fn reason(out: &Value, id: u8) -> String {
    receipt(out, id)["reason"]
        .as_str()
        .unwrap_or("<sin reason>")
        .to_string()
}

fn string_list(v: &Value) -> Vec<String> {
    v.as_array()
        .map(|a| {
            a.iter()
                .filter_map(|x| x.as_str())
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default()
}

/// Serie del MISMO par (`pair_keys` idéntica) con subidas y bajadas — la
/// entrada real de los 6 operadores de serie.
fn same_pair_series(prices: &[f64]) -> MarketState {
    MarketState {
        price_matrix: prices.iter().map(|p| vec![*p]).collect(),
        pair_keys: prices.iter().map(|_| "0xaaa|0xbbb".to_string()).collect(),
        liquidity_reserves: vec![(1_000_000.0, 2_000_000.0), (1_000_000.0, 1_000_000.0)],
        gas_price_gwei: 20.0,
        block_timestamp: 1_700_000_000,
        block_number: 21_000_000,
        features: HashMap::new(),
    }
}

/// Serie de 5 retornos (+1.0%, −1.49%, +2.51%, −3.92%, +5.10%).
const REAL_SERIES: [f64; 6] = [100.0, 101.0, 99.5, 102.0, 98.0, 103.0];

fn token_prices() -> HashMap<String, f64> {
    let mut m = HashMap::new();
    m.insert("USDC".to_string(), 1.0);
    m
}

/// Fuentes REALES: fee declarada del leg (30 bps), knobs canónicos de gas y
/// precios, premium de flash en fracción y resultados del paper ledger.
fn real_sources<'a>(prices: &'a HashMap<String, f64>) -> OperatorFeatureSources<'a> {
    OperatorFeatureSources {
        fee_bps: Some(30.0),
        pool_fee: None,
        gas_units: Some(180_000.0),
        gas_token_price_usd: Some(2_000.0),
        gas_token_symbol: Some("WETH"),
        token0_symbol: Some("USDC"),
        token_prices_usd: Some(prices),
        flash_premium_fraction: Some(0.0009),
        parity_deviation: Some(0.0000972),
        bayes_wins: Some(8.0),
        bayes_losses: Some(2.0),
    }
}

fn featured_state(prices: &HashMap<String, f64>) -> MarketState {
    let mut s = same_pair_series(&REAL_SERIES);
    s.features = market_features_from_sources(&real_sources(prices));
    s
}

/// Objetivo de break-even DEL FIXTURE — **PRESENTE ⇒ acreditado**.
///
/// OP21-CONTRACT-01: el productor real es
/// `CostInputs::break_even_target_min_units`
/// (`native_operator_adapter/market_features/cost_inputs.rs:448`), emitido por
/// `add_cost_features` (`market_features/mod.rs:235-237`) en unidades mínimas del
/// numerario y **jamás 0.0** — el 0.0 es el default PROHIBIDO
/// (`market_features/cost_tests.rs:386`: "El default prohibido (0.0) haria
/// desaparecer el objetivo del operador"). El valor de este fixture imita la
/// ESCALA que la propia guarda del operador usa en sus tests
/// (`op_21_newton.rs:320-340`: hurdle 10.0 con reservas 1e6); lo que este archivo
/// fija es PRESENCIA vs AUSENCIA y la coherencia del recibo, no la economía del
/// fixture.
const FIXTURE_BREAK_EVEN_TARGET: f64 = 10.0;

/// Fuentes reales presentes **más** el objetivo acreditado: el caso en que los
/// tres operadores de `features` tienen TODO lo que su guarda exige.
fn featured_state_with_objective(prices: &HashMap<String, f64>) -> MarketState {
    let mut s = featured_state(prices);
    s.features
        .insert("break_even_target".to_string(), FIXTURE_BREAK_EVEN_TARGET);
    s
}

// ── 1. El productor sólo emite claves con fuente real ────────────────────────

#[test]
fn producer_emits_only_keys_with_real_sources_never_zero() {
    let empty = market_features_from_sources(&OperatorFeatureSources::default());
    assert!(
        empty.is_empty(),
        "sin fuentes el mapa debe ir VACÍO (ausencia), no relleno de ceros: {empty:?}"
    );

    let partial = market_features_from_sources(&OperatorFeatureSources {
        fee_bps: Some(30.0),
        ..Default::default()
    });
    assert_eq!(partial.get("fee_bps"), Some(&30.0));
    assert_eq!(partial.get("pool_fee"), Some(&0.003));
    assert_eq!(
        partial.len(),
        2,
        "sólo el dato real y su derivada: {partial:?}"
    );
    for absent in [
        "gas_units",
        "token0_per_eth",
        "bayes_wins",
        "bayes_losses",
        "flash_premium",
        "parity_deviation",
    ] {
        assert!(
            !partial.contains_key(absent),
            "la clave `{absent}` sin fuente debe estar AUSENTE, no en 0.0"
        );
    }

    // token0_per_eth desde DOS precios reales (2000 USD/ETH ÷ 1 USD/USDC).
    let prices = token_prices();
    let derived = market_features_from_sources(&real_sources(&prices));
    assert_eq!(derived.get("token0_per_eth"), Some(&2_000.0));
    assert_eq!(derived.get("gas_units"), Some(&180_000.0));
    assert_eq!(derived.get("flash_premium"), Some(&0.0009));
    // identidad exacta: token0 es el propio token de gas ⇒ 1.0, no un cálculo.
    let identity = market_features_from_sources(&OperatorFeatureSources {
        gas_token_price_usd: Some(2_000.0),
        gas_token_symbol: Some("WETH"),
        token0_symbol: Some("weth"),
        ..Default::default()
    });
    assert_eq!(identity.get("token0_per_eth"), Some(&1.0));
}

// ── 2. Con las claves pobladas, los operadores de `features` computan ─────────

#[test]
fn features_present_unlock_the_feature_keyed_operators() {
    let prices = token_prices();
    // OP21-CONTRACT-01: para op_21 "las fuentes presentes" incluyen el OBJETIVO de
    // break-even (su guarda lo exige desde FEATURES-DEFAULTS-01,
    // `op_21_newton.rs:130-135`); sin él, el resultado correcto es DATA_GAP, que es
    // lo que la dirección contraria de este archivo fija.
    let state = featured_state_with_objective(&prices);
    let out = run(&state, &[11, 21, 26]);

    for id in [11u8, 21, 26] {
        assert_eq!(
            status(&out, id),
            "COMPUTED",
            "op {id}: {:?}",
            receipt(&out, id)
        );
        assert!(
            receipt(&out, id)["value"]
                .as_f64()
                .is_some_and(f64::is_finite),
            "op {id}: COMPUTED debe traer un valor finito"
        );
        assert!(
            string_list(&receipt(&out, id)["missing_inputs"]).is_empty(),
            "op {id}: sin entradas faltantes cuando las fuentes están"
        );
    }
    // El posterior Bayesiano tiene que ser el real (8 wins, 2 losses, prior 1/1).
    let mean = receipt(&out, 11)["value"].as_f64().unwrap();
    assert!(
        (mean - 0.75).abs() < 1e-9,
        "posterior esperado 0.75, fue {mean}"
    );
}

// ── 3. Sin las claves: DATA_GAP nombrado, NUNCA un cero ──────────────────────

#[test]
fn features_absent_keep_data_gap_and_name_the_missing_key() {
    let prices = token_prices();
    let mut state = featured_state(&prices);
    state.features.clear();
    let out = run(&state, &[11, 21, 26]);

    assert_eq!(status(&out, 26), "DATA_GAP");
    assert_eq!(
        reason(&out, 26),
        "missing_inputs:features.gas_units+features.token0_per_eth",
        "la razón debe nombrar la entrada exacta, no el genérico"
    );
    assert_eq!(status(&out, 11), "DATA_GAP");
    assert_eq!(
        reason(&out, 11),
        "missing_inputs:features.bayes_wins+features.bayes_losses"
    );

    for id in [11u8, 26] {
        assert!(
            receipt(&out, id)["value"].is_null(),
            "op {id}: un DATA_GAP NO puede llevar valor"
        );
        assert!(
            receipt(&out, id)["scalar"].is_null(),
            "op {id}: un DATA_GAP NO puede publicar un escalar (ni 0.0)"
        );
        assert_eq!(
            receipt(&out, id)["metadata"]["computed"],
            0.0,
            "op {id}: el operador debe declarar computed = 0"
        );
        assert!(
            !string_list(&receipt(&out, id)["missing_inputs"]).is_empty(),
            "op {id}: el recibo debe listar la entrada faltante"
        );
    }
}

// ── 4. Defaults: el ÚNICO vivo se declara, los borrados son huecos ───────────
//
// OP21-CONTRACT-01 (2026-10-04). Este test afirmaba `op_21 COMPUTED` sin fee ni
// objetivo: eso codificaba un catálogo STALE del adaptador
// (`native_operator_adapter.rs::CONTRACT_21` seguía declarando
// `HardCodedDefault("0.003")` para el fee y `HardCodedDefault("0.0")` para el
// objetivo, default que FEATURES-DEFAULTS-01 había BORRADO de la guarda real).
// La conclusión del recibo era autocontradictoria: `status=DATA_GAP` con
// `defaulted_inputs=["features.break_even_target"]`, y la razón
// `operator_reason:…` en vez de la entrada que falta de verdad.
//
// El lado equivocado era el CATÁLOGO, no la guarda del operador (probada por
// `op_21_newton.rs::tests::{fee_absent_declares_the_gap,
// break_even_target_absent_declares_the_gap, accredited_zero_target_still_computes,
// larger_target_moves_the_break_even_up}`, todas verdes) ni el productor (que
// declara el 0.0 como default PROHIBIDO, `cost_tests.rs:386`). Aquí se fijan las
// tres direcciones del comportamiento REAL.

#[test]
fn op_21_declares_its_single_live_default_and_names_the_removed_ones_as_gaps() {
    let prices = token_prices();

    // (a) SIN ninguna fuente: el único default vivo (`gas_units` = 150_000,
    //     `op_21_newton.rs:118-124`) viaja declarado, y las DOS entradas que la
    //     guarda exige se nombran como faltantes — nunca como defaults aplicados.
    let mut starved = featured_state(&prices);
    starved.features.clear();
    let out = run(&starved, &[21]);
    assert_eq!(status(&out, 21), "DATA_GAP", "{:?}", receipt(&out, 21));
    assert_eq!(
        reason(&out, 21),
        "missing_inputs:features.fee_bps|pool_fee+features.break_even_target",
        "la razón debe nombrar las entradas que la guarda REAL exige"
    );
    assert!(
        receipt(&out, 21)["value"].is_null(),
        "un DATA_GAP no publica valor"
    );
    assert_eq!(
        string_list(&receipt(&out, 21)["defaulted_inputs"]),
        vec!["features.gas_units".to_string()],
        "el único default hardcodeado vivo de op_21 es el gas"
    );

    // (b) Objetivo AUSENTE con el resto sourced: hueco SOLO por el objetivo, y
    //     jamás listado como default (la contradicción que este test fija).
    let no_objective = run(&featured_state(&prices), &[21]);
    assert_eq!(
        status(&no_objective, 21),
        "DATA_GAP",
        "{:?}",
        receipt(&no_objective, 21)
    );
    assert_eq!(
        reason(&no_objective, 21),
        "missing_inputs:features.break_even_target"
    );
    assert!(
        !string_list(&receipt(&no_objective, 21)["defaulted_inputs"])
            .contains(&"features.break_even_target".to_string()),
        "el objetivo ausente NO puede viajar como default: el operador no lo aplica \
         (FEATURES-DEFAULTS-01) y declararlo así fabricaba un hurdle"
    );

    // (c) Todo sourced (fee, gas, reservas y objetivo acreditado): computa y NO
    //     queda ningún default declarado.
    let sourced = run(&featured_state_with_objective(&prices), &[21]);
    assert_eq!(
        status(&sourced, 21),
        "COMPUTED",
        "{:?}",
        receipt(&sourced, 21)
    );
    assert!(
        string_list(&receipt(&sourced, 21)["missing_inputs"]).is_empty(),
        "con todas las fuentes no hay entradas faltantes"
    );
    assert!(
        string_list(&receipt(&sourced, 21)["defaulted_inputs"]).is_empty(),
        "ninguna entrada del contrato es default cuando todas tienen fuente: {:?}",
        receipt(&sourced, 21)["defaulted_inputs"]
    );
    assert!(
        receipt(&sourced, 21)["value"]
            .as_f64()
            .is_some_and(|v| v.is_finite() && v > 0.0),
        "un COMPUTED de op_21 publica la raíz de break-even finita y positiva"
    );
}

// ── 5. Los seis de serie no leen `features`: necesitan una serie ─────────────

#[test]
fn series_operators_need_a_price_series_not_features() {
    let out = run(&same_pair_series(&REAL_SERIES), &[5, 8, 10, 13, 16, 22]);
    for id in [5u8, 8, 10, 13, 16, 22] {
        assert_eq!(
            status(&out, id),
            "COMPUTED",
            "op {id} con serie real de 6 filas del mismo par: {:?}",
            receipt(&out, id)
        );
        assert_eq!(receipt(&out, id)["series_pair_scope"], "single_pair");
    }

    // Con 3 precios, op_22 computa y op_05 (que exige 6) nombra su entrada.
    let short = run(&same_pair_series(&[100.0, 101.0, 102.0]), &[5, 22]);
    assert_eq!(status(&short, 22), "COMPUTED");
    assert_eq!(status(&short, 5), "DATA_GAP");
    assert_eq!(reason(&short, 5), "missing_inputs:price_matrix.col0[6]");
    assert!(receipt(&short, 5)["value"].is_null());
}

#[test]
fn mixed_pair_series_is_declared_not_hidden_nor_blocked() {
    let mut mixed = same_pair_series(&REAL_SERIES);
    for (i, key) in mixed.pair_keys.iter_mut().enumerate() {
        *key = format!("0xpair{i}");
    }
    assert_eq!(price_series_pair_scope(&mixed), "mixed_pairs");
    let out = run(&mixed, &[22]);
    assert_eq!(
        receipt(&out, 22)["series_pair_scope"],
        "mixed_pairs",
        "FEATURES-01b: una serie de pares DISTINTOS se declara en el recibo"
    );
    assert_eq!(
        status(&out, 22),
        "COMPUTED",
        "…y no se bloquea: el bloqueo lo decide el contrato, no una suposición"
    );
}

// ── 6. Propiedad de seguridad: el contrato nunca bloquea a quien computa ─────

#[test]
fn contract_never_blocks_a_computable_operator() {
    let registry = OperatorRegistry::new();
    let prices = token_prices();

    let with_features = featured_state(&prices);
    let mut cleared = with_features.clone();
    cleared.features.clear();
    let mut mixed = same_pair_series(&REAL_SERIES);
    for (i, key) in mixed.pair_keys.iter_mut().enumerate() {
        *key = format!("0xpair{i}");
    }
    let mut no_reserves = same_pair_series(&REAL_SERIES);
    no_reserves.liquidity_reserves.clear();
    let mut no_pair_keys = same_pair_series(&REAL_SERIES);
    no_pair_keys.pair_keys.clear();

    let states = vec![
        same_pair_series(&[]),
        same_pair_series(&[100.0]),
        same_pair_series(&[100.0, 101.0, 102.0]),
        same_pair_series(&REAL_SERIES),
        with_features.clone(),
        cleared,
        mixed,
        no_reserves,
        no_pair_keys,
    ];

    for state in &states {
        for id in [5u8, 8, 10, 11, 13, 16, 21, 22, 26] {
            let missing = missing_required_inputs(id, state);
            if missing.is_empty() {
                continue;
            }
            let dispatched = registry.dispatch(id, state).expect("operador registrado");
            assert!(
                dispatched.scalar_value.is_none(),
                "op {id}: el contrato declara faltantes {missing:?} pero el operador \
                 computó {:?} — el gate bloquearía un valor real (falso rojo)",
                dispatched.scalar_value
            );
        }
    }
}

// ── 7. La admisión por contrato, opt-in, sin editar la del sitio ─────────────

#[test]
fn contract_admission_layers_over_the_structural_one() {
    let admission = ContractInputAdmission {
        inner: StructuralLike,
    };
    let prices = token_prices();
    let good = featured_state(&prices);

    let receipt = admission
        .validate(26, &good, "snap-f3", "plan-f3")
        .expect("con fuentes reales la admisión pasa");
    assert!(receipt.starts_with("structural:snap-f3:plan-f3:"));

    let mut starved = good.clone();
    starved.features.clear();
    assert_eq!(
        admission.validate(26, &starved, "snap-f3", "plan-f3"),
        Err("missing_inputs:features.gas_units+features.token0_per_eth".to_string())
    );

    // La razón ESTRUCTURAL del sitio se preserva (no se enmascara).
    let mut no_gas = good.clone();
    no_gas.gas_price_gwei = 0.0;
    assert_eq!(
        admission.validate(26, &no_gas, "snap-f3", "plan-f3"),
        Err("market_state_gas_not_positive".to_string())
    );

    // Un operador fuera del conjunto medido pasa intacto (sin contrato inventado).
    let plain = same_pair_series(&[100.0]);
    assert!(admission.validate(1, &plain, "snap-f3", "plan-f3").is_ok());
}

// ── 8. Cobertura del contrato: exactamente los 9 operadores medidos ──────────

#[test]
fn declared_contract_covers_exactly_the_measured_operators() {
    for id in [5u8, 8, 10, 11, 13, 16, 21, 22, 26] {
        assert!(
            !declared_contract(id).is_empty(),
            "op {id} está en el conjunto DATA_GAP medido y debe tener contrato"
        );
    }
    for id in [
        1u8, 2, 3, 4, 6, 7, 9, 12, 14, 15, 17, 18, 19, 20, 23, 24, 25, 27, 28, 29, 30, 31, 32,
    ] {
        assert!(
            declared_contract(id).is_empty(),
            "op {id} no fue medido: no se le inventa contrato"
        );
    }
}
