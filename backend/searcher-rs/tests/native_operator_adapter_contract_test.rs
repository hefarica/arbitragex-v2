//! F3-NATIVEADAPTER-01 — contrato de entrada del adaptador de operadores nativos.
//!
//! Prueba las DOS direcciones del cableado, en el adaptador REAL
//! (`searcher_rs::native_operator_adapter::evaluate_declared`), no en una copia:
//!
//! 1. Con las claves de `MarketState.features` pobladas desde fuentes reales, los
//!    operadores que LEEN `features` (11 Bayes, 21 Newton, 26 FlashLoan) dejan de
//!    devolver DATA_GAP y publican un valor finito.
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
    let state = featured_state(&prices);
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
    // El objetivo de break-even NO es un hueco ni un default: con las fuentes
    // presentes el operador resuelve el EQUILIBRIO por definición del modelo
    // (`op_21_newton.rs:6`, `target = 0 ⇒ break-even`) y eso viaja declarado.
    let target_defined = string_list(&receipt(&out, 21)["defined_inputs"]);
    assert!(
        target_defined.contains(&"features.break_even_target".to_string()),
        "op_21: el objetivo ausente se resuelve por DEFINICIÓN, no por default: {target_defined:?}"
    );
    assert!(
        !string_list(&receipt(&out, 21)["defaulted_inputs"])
            .contains(&"features.break_even_target".to_string()),
        "op_21: una definición no puede viajar como default hardcodeado"
    );
    assert!(
        !string_list(&receipt(&out, 21)["missing_inputs"])
            .contains(&"features.break_even_target".to_string()),
        "op_21: una definición tampoco es una entrada faltante"
    );
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
        "missing_inputs:features.gas_units+features.token0_per_eth+features.pool_fee+features.flash_premium",
        "la razón debe nombrar TODAS las entradas exactas: el contrato de op_26 declara\n         también la comisión de pool y el premium flash, que FEATURES-DEFAULTS-01 dejó\n         sin default (un flash a 0.0 era «financiación gratis»)"
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

// ── 4. La ausencia de un DATO no se rellena; el objetivo de break-even SÍ se
//      resuelve por definición. Son dos cosas distintas y viajan separadas. ────
//
// El escenario que este bloque reemplazaba (`hardcoded_defaults_are_declared_
// instead_of_faked_as_sourced`) exigía COMPUTED con TODAS las claves de
// `features` borradas, es decir computar SIN comisión. Satisfacerlo sólo era
// posible reinstaurando el `unwrap_or(0.003)` que `FEATURES-DEFAULTS-01` cortó.
// El contrato se CORRIGE: la comisión ausente es un hueco y el objetivo ausente
// es una definición, y cada uno se prueba por separado.

/// Helper local: `features` exactamente como las emite el productor. `None` deja
/// la clave AUSENTE (no en 0.0) — R8: no se puede distinguir "sin dato" de "dato
/// cero" si ambos son `0.0`.
fn with_features(entries: &[(&str, Option<f64>)]) -> HashMap<String, f64> {
    entries
        .iter()
        .filter_map(|(k, v)| v.map(|v| (k.to_string(), v)))
        .collect()
}

/// Fuentes reales de op_21: reservas primarias, gas medido y comisión declarada.
fn op21_state(features: HashMap<String, f64>) -> MarketState {
    let mut state = same_pair_series(&REAL_SERIES);
    state.liquidity_reserves = vec![(1_000_000.0, 1_050_000.0)];
    state.features = features;
    state
}

/// El escenario que el test original exigía resolver computando. Se CORRIGE en
/// vez de satisfacerse: sin NINGUNA clave de fee el operador NO computa, porque
/// la comisión es un dato de mercado (`op_21_newton.rs:57-63` → `None`,
/// `:102-104` → `reason_fee_unavailable`) y un fee inventado mueve `γ` y con él
/// la raíz publicada.
#[test]
fn absent_fee_is_a_declared_gap_never_a_default_003() {
    let state = op21_state(with_features(&[
        ("gas_units", Some(180_000.0)),
        ("break_even_target", Some(0.0)),
    ]));
    let out = run(&state, &[21]);

    assert_eq!(
        status(&out, 21),
        "DATA_GAP",
        "sin fee el operador no computa: {:?}",
        receipt(&out, 21)
    );
    assert!(
        reason(&out, 21).contains("features.fee_bps|pool_fee"),
        "la razón debe nombrar la comisión ausente, no un genérico: {:?}",
        reason(&out, 21)
    );
    let missing = string_list(&receipt(&out, 21)["missing_inputs"]);
    assert!(
        missing.contains(&"features.fee_bps|pool_fee".to_string()),
        "la comisión ausente viaja como entrada FALTANTE: {missing:?}"
    );
    assert!(
        !string_list(&receipt(&out, 21)["defaulted_inputs"])
            .contains(&"features.fee_bps|pool_fee".to_string()),
        "una comisión ausente NO puede figurar como default aplicado"
    );
    assert!(
        receipt(&out, 21)["value"].is_null(),
        "un DATA_GAP no puede publicar un valor — y menos un 0.0"
    );
    // El fee ausente no debe reaparecer por la puerta de atrás de `pool_fee`.
    assert!(
        !state.features.contains_key("pool_fee"),
        "el fixture no debe reintroducir la comisión por `pool_fee`"
    );
}

/// Un `pool_fee = 0.0` PRESENTE es una comisión real de pool (una pool sin fee
/// existe), no una ausencia: el operador la acepta y computa. `Domain::Finite` en
/// el contrato declara exactamente eso.
#[test]
fn explicit_zero_fee_is_measured_and_computes() {
    let state = op21_state(with_features(&[
        ("pool_fee", Some(0.0)),
        ("gas_units", Some(180_000.0)),
        ("break_even_target", Some(0.0)),
    ]));
    let out = run(&state, &[21]);

    assert_eq!(
        status(&out, 21),
        "COMPUTED",
        "una comisión de 0.0 declarada es un DATO: {:?}",
        receipt(&out, 21)
    );
    assert!(
        string_list(&receipt(&out, 21)["missing_inputs"]).is_empty(),
        "un cero acreditado no es una entrada faltante"
    );
    assert!(
        !string_list(&receipt(&out, 21)["defaulted_inputs"])
            .contains(&"features.break_even_target".to_string()),
        "una comisión declarada no convierte el objetivo en un default"
    );
}

/// El objetivo de break-even AUSENTE con el resto de los insumos válidos: el
/// operador resuelve EQUILIBRIO por la definición del modelo y computa, con la
/// resolución declarada. No es un default hardcodeado ni un hueco.
#[test]
fn break_even_target_absent_with_valid_inputs_computes_by_definition() {
    let state = op21_state(with_features(&[
        ("fee_bps", Some(30.0)),
        ("gas_units", Some(180_000.0)),
        // `break_even_target` AUSENTE a propósito.
    ]));
    let out = run(&state, &[21]);

    assert_eq!(
        status(&out, 21),
        "COMPUTED",
        "con el resto de insumos válidos el objetivo ausente se resuelve por          definición (op_21_newton.rs:6), no aborta: {:?}",
        receipt(&out, 21)
    );
    let defined = string_list(&receipt(&out, 21)["defined_inputs"]);
    assert!(
        defined.contains(&"features.break_even_target".to_string()),
        "la definición aplicada debe viajar en `defined_inputs`: {defined:?}"
    );
    for other in ["missing_inputs", "defaulted_inputs"] {
        assert!(
            !string_list(&receipt(&out, 21)[other])
                .contains(&"features.break_even_target".to_string()),
            "`{other}` no puede reclamar el objetivo: una definición no es un              hueco ni un default"
        );
    }
    let x = receipt(&out, 21)["value"]
        .as_f64()
        .expect("raíz de equilibrio publicada");
    assert!(x.is_finite() && x > 0.0, "raíz admisible, fue {x}");
    // El recibo declara de dónde salió el valor: `computed` describe el CÓMPUTO,
    // no una autorización de ejecución.
    assert_eq!(
        receipt(&out, 21)["metadata"]["computed"],
        1.0,
        "el operador debe declarar su estado de cómputo"
    );
}

/// Un objetivo CONFIGURADO (incluido un `0.0` explícito) se mide y se respeta:
/// no se confunde con la definición, y su procedencia queda registrada.
#[test]
fn configured_target_is_honoured_and_distinct_from_the_definition() {
    let with_target = |target: f64| {
        op21_state(with_features(&[
            ("fee_bps", Some(30.0)),
            ("gas_units", Some(180_000.0)),
            ("break_even_target", Some(target)),
        ]))
    };

    let zero = run(&with_target(0.0), &[21]);
    assert_eq!(status(&zero, 21), "COMPUTED");
    assert!(
        string_list(&receipt(&zero, 21)["defined_inputs"]).is_empty(),
        "un 0.0 PRESENTE es un cero acreditado: no es la definición"
    );
    assert!(
        !string_list(&receipt(&zero, 21)["defaulted_inputs"])
            .contains(&"features.break_even_target".to_string()),
        "un objetivo declarado tampoco es un default"
    );

    // El hurdle es real: subirlo desplaza el equilibrio hacia arriba.
    let raised = run(&with_target(10.0), &[21]);
    assert_eq!(status(&raised, 21), "COMPUTED");
    let x0 = receipt(&zero, 21)["value"].as_f64().unwrap();
    let x10 = receipt(&raised, 21)["value"].as_f64().unwrap();
    assert!(
        x10 > x0,
        "hurdle mayor ⇒ break-even mayor (base={x0}, raised={x10})"
    );

    // Y ausente == 0.0 acreditado: mismo equilibrio, distinta procedencia.
    let absent = run(
        &op21_state(with_features(&[
            ("fee_bps", Some(30.0)),
            ("gas_units", Some(180_000.0)),
        ])),
        &[21],
    );
    let x_absent = receipt(&absent, 21)["value"].as_f64().unwrap();
    assert!(
        (x_absent - x0).abs() < 1e-12,
        "definición y cero acreditado deben dar el mismo equilibrio: {x_absent} vs {x0}"
    );
}

/// Un objetivo PRESENTE pero no finito es un DATO INVÁLIDO, no una ausencia: se
/// reporta como tal y NO se degrada a la definición. Se inyecta directamente
/// porque el escenario es un `MarketState` corrupto por construcción — no se
/// puede pedir al productor que emita `NaN` (lo filtraría, y con razón).
#[test]
fn invalid_target_is_reported_as_invalid_not_silently_degraded() {
    for invalid in [f64::NAN, f64::INFINITY] {
        let state = op21_state(with_features(&[
            ("fee_bps", Some(30.0)),
            ("gas_units", Some(180_000.0)),
            ("break_even_target", Some(invalid)),
        ]));
        let out = run(&state, &[21]);

        assert_eq!(
            status(&out, 21),
            "DATA_GAP",
            "un objetivo ilegible ({invalid}) no puede computar"
        );
        assert!(
            receipt(&out, 21)["value"].is_null(),
            "un objetivo inválido no puede publicar una raíz"
        );
        let defined = string_list(&receipt(&out, 21)["defined_inputs"]);
        assert!(
            !defined.contains(&"features.break_even_target".to_string()),
            "un dato inválido NO es una definición: {defined:?}"
        );
        let reason = reason(&out, 21);
        // Un valor PRESENTE pero ilegible no es una entrada faltante — la clave
        // está— ni una definición aplicable: es un dato inválido, y la razón lo
        // declara con el motivo propio del operador, no degradándolo a la
        // definición ni etiquetándolo como ausencia.
        assert_eq!(
            reason, "operator_reason:reason_break_even_target_unavailable",
            "la razón debe declarar el objetivo inválido como dato ilegible"
        );
        assert!(
            !string_list(&receipt(&out, 21)["missing_inputs"])
                .contains(&"features.break_even_target".to_string()),
            "una clave PRESENTE no puede declararse como faltante"
        );
    }
}

/// Falta de datos: sin ninguna fuente no hay cómputo, y el recibo nombra TODAS
/// las entradas que faltan en vez de inventar un valor. `COMPUTED` jamás es una
/// autorización de ejecución, y `DATA_GAP` nunca es un cero.
#[test]
fn missing_data_yields_a_data_gap_naming_every_absent_input() {
    let state = op21_state(HashMap::new());
    let out = run(&state, &[21]);

    assert_eq!(status(&out, 21), "DATA_GAP");
    assert!(receipt(&out, 21)["value"].is_null());
    assert!(receipt(&out, 21)["scalar"].is_null(), "ni un escalar 0.0");
    assert_eq!(
        receipt(&out, 21)["metadata"]["computed"],
        0.0,
        "el operador debe declarar que no computó"
    );
    let missing = string_list(&receipt(&out, 21)["missing_inputs"]);
    assert!(
        missing.contains(&"features.fee_bps|pool_fee".to_string()),
        "la comisión ausente debe estar declarada como faltante: {missing:?}"
    );
    // `break_even_target` NO puede figurar como faltante ni como default: su
    // ausencia la resuelve la DEFINICIÓN del modelo, y el contrato lo declara así
    // (`AbsentMeaning::DeclaredByDefinition`). El hueco real de este escenario es
    // la comisión — un dato de mercado que nadie midió.
    assert!(
        !missing.contains(&"features.break_even_target".to_string()),
        "el objetivo ausente no es un hueco: se resuelve por definición: {missing:?}"
    );
    assert!(
        string_list(&receipt(&out, 21)["defined_inputs"])
            .contains(&"features.break_even_target".to_string()),
        "…y esa resolución debe viajar declarada"
    );
    assert!(
        reason(&out, 21).starts_with("missing_inputs:"),
        "la razón debe nombrar lo que falta: {:?}",
        reason(&out, 21)
    );
}

/// No convergencia: un sistema mal condicionado del que el operador SALE sin
/// publicar nada. La guarda se declara; no se disfraza de valor.
#[test]
fn non_convergence_is_declared_and_never_disguised_as_a_value() {
    let mut state = same_pair_series(&[10.0]);
    state.liquidity_reserves = vec![(1_000_000.0, 1_010_000.0)];
    state.features = with_features(&[
        ("fee_bps", Some(30.0)),
        // Hurdle de 20 en un pool que sólo rinde ~20.7 en el pico: la semilla de
        // Newton cae fuera del bracket y el paso cruza a negativo.
        ("break_even_target", Some(20.0)),
    ]);
    let out = run(&state, &[21]);

    assert_eq!(
        status(&out, 21),
        "DATA_GAP",
        "una divergencia numérica no puede publicar una raíz: {:?}",
        receipt(&out, 21)
    );
    assert!(
        receipt(&out, 21)["value"].is_null(),
        "no puede haber valor en una no convergencia"
    );
    assert_eq!(receipt(&out, 21)["metadata"]["computed"], 0.0);
    assert!(
        !string_list(&receipt(&out, 21)["defined_inputs"])
            .contains(&"features.break_even_target".to_string()),
        "el objetivo ESTABA configurado: no puede declararse como definición"
    );

    // Control: el mismo pool con un hurdle alcanzable converge.
    let mut reachable = state.clone();
    reachable
        .features
        .insert("break_even_target".to_string(), 10.0);
    let ok = run(&reachable, &[21]);
    assert_eq!(
        status(&ok, 21),
        "COMPUTED",
        "el rechazo no es ciego: con hurdle alcanzable computa: {:?}",
        receipt(&ok, 21)
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
        Err(
            "missing_inputs:features.gas_units+features.token0_per_eth+features.pool_fee+\
            features.flash_premium"
                .to_string()
        )
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
