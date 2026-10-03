//! Integration module for the EXISTING searcher-rs workspace only.
//! Uses the current math-engine registry; does not reimplement operators, set
//! weights, alter enabled states or add a 32nd role to the source 31-column map.
//! Native defaults must be excluded by input admission BEFORE dispatch. A
//! numerical result by itself does not prove the financial validity of an op.
//!
//! ── F3-NATIVEADAPTER-01 (2026-10-03) — la mitad que faltaba del desbloqueo ──
//!
//! HALLAZGO: este adaptador NO construye un `MarketState` parcial — pasa el
//! estado COMPLETO a `registry.dispatch` (ver `evaluate_declared`, la llamada al
//! dispatch). El estado parcial nace en el CALL SITE v4:
//! `cartridge_boot.rs:1427-1435` construye el `MarketState` con
//! `features: std::collections::HashMap::new()` (`cartridge_boot.rs:1434`), así
//! que todo operador que lea `state.features` está estructuralmente condenado a
//! DATA_GAP en esa ruta. `cartridge_boot.rs` es ZONA CALIENTE (sesiones vivas lo
//! editan): no se toca aquí — se expone la función pura y se reporta la conexión
//! pendiente (abajo).
//!
//! Este módulo aporta las tres piezas que el adaptador SÍ puede aportar:
//!
//! 1. `declared_contract` / `missing_required_inputs` / `defaulted_inputs` — el
//!    CONTRATO de entrada por operador, medido de la guarda REAL de cada
//!    operador (archivo:línea citado en cada doc), para que un recibo DATA_GAP
//!    nombre la entrada exacta que falta en vez del genérico
//!    `native_operator_returned_no_finite_value`, y para que un valor COMPUTADO
//!    con defaults hardcodeados viaje marcado como tal (`defaulted_inputs`).
//! 2. `market_features_from_sources` — productor PURO del mapa `features`. Sólo
//!    inserta una clave cuando el llamador entrega un valor REAL; una fuente
//!    ausente deja la clave AUSENTE, jamás un `0.0` (RULE 00 / R8).
//! 3. `ContractInputAdmission` — envoltorio opt-in que superpone el contrato
//!    por-operador sobre la admisión estructural del sitio sin editarla.
//!
//! CONEXIÓN PENDIENTE (una línea, en archivo de la zona caliente):
//! ```text
//! cartridge_boot.rs:1434
//!   -  features: std::collections::HashMap::new(),
//!   +  features: crate::native_operator_adapter::market_features_from_sources(&sources),
//! ```
//! con `sources` alimentado desde fuentes REALES ya presentes en el camino v4:
//! la fee declarada del leg del intent (`RouteIntentLeg::fee_bps`), los knobs
//! canónicos (`TradingConfigState::gas_estimate_units`,
//! `TradingConfigState::base_token_price_usd`,
//! `TradingConfigState::token_prices_usd`) y el libro de resultados del paper
//! ledger para `bayes_wins`/`bayes_losses`. Lo que NO tenga fuente real queda
//! fuera del mapa y el operador reparte su DATA_GAP — que es el comportamiento
//! correcto.
use crate::rhai_agent_bridge::canonical_hash;
use math_engine::operators::{MarketState, OperatorRegistry};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::panic::{catch_unwind, AssertUnwindSafe};

/// The backend supplies this admission hook from field lineage/type/unit checks.
/// It must validate ALL inputs consumed by the operator, including any that the
/// legacy implementation would otherwise fill with defaults. Return a receipt
/// identifier on success; an empty ID or absent input is a data error.
pub trait OperatorInputAdmission {
    fn validate(
        &self,
        id: u8,
        state: &MarketState,
        snapshot_id: &str,
        plan_hash: &str,
    ) -> Result<String, String>;
}

pub fn evaluate_declared(
    registry: &OperatorRegistry,
    state: &MarketState,
    spec: &Value,
    snapshot_id: &str,
    plan_hash: &str,
    admission: &dyn OperatorInputAdmission,
    is_disabled: impl Fn(u8) -> bool,
) -> Result<Value, String> {
    if snapshot_id.is_empty() || plan_hash.is_empty() {
        return Err("operator_context_missing".into());
    }
    let roles = spec["operator_requirements"]
        .as_array()
        .ok_or("missing_declared_operator_roles")?;
    let mut outputs = serde_json::Map::new();
    for r in roles {
        let id = r["id"]
            .as_u64()
            .filter(|i| (1..=31).contains(i))
            .ok_or("invalid_source_operator_id")? as u8;
        let key = id.to_string();
        if outputs.contains_key(&key) {
            return Err("duplicate_source_operator_id".into());
        }
        if r["role"] == "N/A" {
            outputs.insert(key,json!({"status":"NOT_APPLICABLE","reason":"source_matrix_role_na","role":r["role"]}));
            continue;
        }
        if is_disabled(id) {
            outputs.insert(key,json!({"status":"DISABLED","reason":"operator_switch_off","role":r["role"],"requirement":r["requirement"]}));
            continue;
        }
        let input_receipt = match admission.validate(id, state, snapshot_id, plan_hash) {
            Ok(id) if !id.is_empty() => id,
            Ok(_) => {
                outputs.insert(
                    key,
                    json!({"status":"DATA_GAP","reason":"empty_input_admission_receipt"}),
                );
                continue;
            }
            Err(e) => {
                outputs.insert(key, json!({"status":"DATA_GAP","reason":e}));
                continue;
            }
        };
        let dispatched = catch_unwind(AssertUnwindSafe(|| registry.dispatch(id, state)));
        let out = match dispatched {
            Ok(Some(o)) => o,
            Ok(None) => {
                outputs.insert(
                    key,
                    json!({"status":"MISSING","reason":"operator_not_registered"}),
                );
                continue;
            }
            Err(_) => {
                outputs.insert(
                    key,
                    json!({"status":"ERROR","reason":"native_operator_panicked"}),
                );
                continue;
            }
        };
        let scalar = out.scalar_value.filter(|v| v.is_finite());
        let vector = out
            .vector_result
            .filter(|v| !v.is_empty() && v.iter().all(|x| x.is_finite()));
        let matrix = out.matrix_result.filter(|m| {
            !m.is_empty()
                && m.iter()
                    .all(|v| !v.is_empty() && v.iter().all(|x| x.is_finite()))
        });
        let value = scalar
            .map(|v| json!(v))
            .or_else(|| vector.as_ref().map(|v| json!(v)))
            .or_else(|| matrix.as_ref().map(|v| json!(v)));
        // ── F3: diagnóstico ANTES de mover `out.metadata` al recibo ──────────
        // El contrato declarado del operador se evalúa en ambos sentidos: la
        // entrada que FALTA (DATA_GAP nombrado) y la que se rellenó con un
        // default hardcodeado (COMPUTED pero no enteramente sourced).
        let contract = declared_contract(id);
        let missing = missing_required_inputs(id, state);
        let defaulted = defaulted_inputs(id, state);
        let operator_reason = operator_metadata_reason(&out.metadata);
        let mut receipt = json!({"operator_id":id,"operator_name":out.operator_name,"snapshot_id":snapshot_id,"plan_hash":plan_hash,
            "input_receipt":input_receipt,"phase":r["phase"],"role":r["role"],"scalar":scalar,"vector":vector,"matrix":matrix,
            "metadata":out.metadata,"weight":null,"calibration_state":"UNCALIBRATED"});
        if !contract.is_empty() {
            receipt["missing_inputs"] = json!(missing);
            receipt["defaulted_inputs"] = json!(defaulted);
        }
        if SERIES_OPERATOR_IDS.contains(&id) {
            // FEATURES-01b: una serie de pares DISTINTOS no es una serie
            // temporal del mismo activo. Se DECLARA, no se bloquea.
            receipt["series_pair_scope"] = json!(price_series_pair_scope(state));
        }
        if let Some(v) = value {
            receipt["status"] = json!("COMPUTED");
            receipt["value"] = v;
        } else {
            receipt["status"] = json!("DATA_GAP");
            receipt["reason"] = json!(data_gap_reason(&missing, operator_reason.as_deref()));
        }
        receipt["evidence_id"] = json!(canonical_hash(&receipt));
        outputs.insert(key, receipt);
    }
    Ok(
        json!({"snapshot_id":snapshot_id,"plan_hash":plan_hash,"operators":outputs,"source_operator_count":31,
        "runtime_operator_32":"unchanged_unassigned_by_these_sources","weighted_confidence":null,
        "reason":"source_weights_uncalibrated_no_fabricated_confidence"}),
    )
}

// ══════════════════════════════════════════════════════════════════════════════
// F3-NATIVEADAPTER-01 — CONTRATO DE ENTRADA POR OPERADOR
// ══════════════════════════════════════════════════════════════════════════════
//
// Medido de la guarda REAL de cada operador (no de la intención declarada en
// documentos). Léase: "el operador X devuelve `scalar_value: None` si falta esta
// entrada" — con la línea exacta que lo prueba.

/// Dominio válido de un valor de entrada.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Domain {
    /// Finito (cualquier signo).
    Finite,
    /// Finito y `> 0`.
    Positive,
    /// Finito y `>= 0`.
    NonNegative,
}

/// Fuente autoritativa de una entrada declarada.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputSource {
    /// Clave de `MarketState.features` (`HashMap<String, f64>`).
    Feature(&'static str),
    /// CUALQUIERA de estas claves de `features`, en el orden de preferencia del
    /// operador (p. ej. `fee_bps` antes que `pool_fee`).
    FeatureAnyOf(&'static [&'static str]),
    /// Columna 0 de `price_matrix`: una observación por fila. Unidad: precio del
    /// par de esa fila (`pair_keys[i]`). `min_observations` = mínimo exigido.
    PriceSeries,
    /// La MISMA serie, exigiendo además ≥1 retorno simple positivo y ≥1 negativo.
    SignedReturns,
    /// `liquidity_reserves[0]` (el pool primario), ambos lados finitos y `> 0`.
    PrimaryReserves,
    /// Precio de referencia cross-venue: un 2º par válido en
    /// `liquidity_reserves[1..]` o una entrada finita `> 0` en la columna 0.
    ReferencePrice,
}

/// Qué significa que la entrada esté ausente.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AbsentMeaning {
    /// El operador devuelve `None` (DATA_GAP honesto). Nunca fabrica un valor.
    DataGap,
    /// El operador sustituye el default hardcodeado citado y publica un valor
    /// COMPUTADO que NO está enteramente sourced. Viaja en `defaulted_inputs`.
    HardCodedDefault(&'static str),
}

/// Una entrada declarada por el CONTRATO de un operador.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DeclaredInput {
    /// Identificador estable que viaja en `missing_inputs` / `defaulted_inputs`.
    pub key: &'static str,
    /// Fuente autoritativa del valor.
    pub source: InputSource,
    /// Mínimo de observaciones (sólo se usa en `PriceSeries` / `SignedReturns`).
    pub min_observations: usize,
    /// Dominio que el valor debe satisfacer para contar como "utilizable".
    pub domain: Domain,
    /// Significado de la ausencia.
    pub absent: AbsentMeaning,
}

/// `key` es la clave REAL del mapa `features`; `display` es el identificador
/// legible que viaja en el recibo (con el prefijo `features.` para que un
/// consumidor no lo confunda con un campo del `MarketState`).
const fn feature(
    key: &'static str,
    display: &'static str,
    domain: Domain,
    absent: AbsentMeaning,
) -> DeclaredInput {
    DeclaredInput {
        key: display,
        source: InputSource::Feature(key),
        min_observations: 0,
        domain,
        absent,
    }
}

/// `fee_bps` (unidad del contrato del pool) o `pool_fee` (fracción): el
/// operador prefiere la primera y cae a la segunda
/// (`op_21_newton.rs:49-56`). `Domain::Finite`: el operador NO filtra dominio,
/// así que un `pool_fee = 0.0` presente es un dato real, no un default.
const FEE_INPUT: DeclaredInput = DeclaredInput {
    key: "features.fee_bps|pool_fee",
    source: InputSource::FeatureAnyOf(&["fee_bps", "pool_fee"]),
    min_observations: 0,
    domain: Domain::Finite,
    absent: AbsentMeaning::HardCodedDefault("0.003"),
};

const PRICE_SERIES_6: DeclaredInput = DeclaredInput {
    key: "price_matrix.col0[6]",
    source: InputSource::PriceSeries,
    min_observations: 6,
    domain: Domain::Positive,
    absent: AbsentMeaning::DataGap,
};
const PRICE_SERIES_3: DeclaredInput = DeclaredInput {
    key: "price_matrix.col0[3]",
    source: InputSource::PriceSeries,
    min_observations: 3,
    domain: Domain::Positive,
    absent: AbsentMeaning::DataGap,
};
const SIGNED_RETURNS: DeclaredInput = DeclaredInput {
    key: "price_matrix.col0.signed_returns",
    source: InputSource::SignedReturns,
    min_observations: 3,
    domain: Domain::Positive,
    absent: AbsentMeaning::DataGap,
};
const PRIMARY_RESERVES: DeclaredInput = DeclaredInput {
    key: "liquidity_reserves[0]",
    source: InputSource::PrimaryReserves,
    min_observations: 0,
    domain: Domain::Positive,
    absent: AbsentMeaning::DataGap,
};
const REFERENCE_PRICE: DeclaredInput = DeclaredInput {
    key: "reference_price(reserves[1..]|price_matrix.col0)",
    source: InputSource::ReferencePrice,
    min_observations: 0,
    domain: Domain::Positive,
    absent: AbsentMeaning::DataGap,
};

// op_05 PDMP (`op_05_pdmp.rs:69`, `if n < 5` con n = retornos = precios − 1).
const CONTRACT_05: &[DeclaredInput] = &[PRICE_SERIES_6];
// op_08 Kalman (`op_08_kalman.rs:65`, `if prices.len() < 3`) y además
// `op_08_kalman.rs:84` exige `Var(precios) >= 1e-12` — condición de MERCADO, no
// de fuente: no se declara como entrada faltante.
const CONTRACT_08: &[DeclaredInput] = &[PRICE_SERIES_3];
// op_10 Welford (`op_10_welford.rs:65`, `if n < 2` con n = retornos).
const CONTRACT_10: &[DeclaredInput] = &[PRICE_SERIES_3];
// op_11 Bayes (`op_11_bayes.rs:35-36` lee `features["bayes_wins"/"bayes_losses"]`;
// `:72` exige `wins + losses >= 1`; `:37-46` rellena las priors con 1.0).
const CONTRACT_11: &[DeclaredInput] = &[
    feature(
        "bayes_wins",
        "features.bayes_wins",
        Domain::NonNegative,
        AbsentMeaning::DataGap,
    ),
    feature(
        "bayes_losses",
        "features.bayes_losses",
        Domain::NonNegative,
        AbsentMeaning::DataGap,
    ),
    feature(
        "bayes_prior_alpha",
        "features.bayes_prior_alpha",
        Domain::Positive,
        AbsentMeaning::HardCodedDefault("1.0"),
    ),
    feature(
        "bayes_prior_beta",
        "features.bayes_prior_beta",
        Domain::Positive,
        AbsentMeaning::HardCodedDefault("1.0"),
    ),
];
// op_13 Regresión (`op_13_regression.rs:50`, `if n < 3`).
const CONTRACT_13: &[DeclaredInput] = &[PRICE_SERIES_3];
// op_16 Kelly (`op_16_kelly.rs:64`: `returns.len() < 2 || wins.is_empty() ||
// losses.is_empty()`).
const CONTRACT_16: &[DeclaredInput] = &[PRICE_SERIES_3, SIGNED_RETURNS];
// op_21 Newton (`op_21_newton.rs:87-93` reservas primarias; `:109-114`
// `gas_units` con default 150_000; `:49-56` `fee_bps`/`pool_fee` con default
// 0.003; `:116-121` `break_even_target` con default 0.0).
const CONTRACT_21: &[DeclaredInput] = &[
    PRIMARY_RESERVES,
    feature(
        "gas_units",
        "features.gas_units",
        Domain::Positive,
        AbsentMeaning::HardCodedDefault("150000"),
    ),
    FEE_INPUT,
    feature(
        "break_even_target",
        "features.break_even_target",
        Domain::Finite,
        AbsentMeaning::HardCodedDefault("0.0"),
    ),
];
// op_22 MonteCarlo (`op_22_monte_carlo.rs:66`, `prices.len() < 3`).
const CONTRACT_22: &[DeclaredInput] = &[PRICE_SERIES_3];
// op_26 Flash Loan (`op_26_flash_loan.rs:46-56` reservas primarias; `:74-82`
// referencia cross-venue; `:105-121` exige `gas_units` Y `token0_per_eth` para
// publicar `computed = 1` — MATH-05: sin ellas `computed = 0`, nunca un neto
// sin gas; `:60` `pool_fee` default 0.003; `:66` `flash_premium` default 0.0).
const CONTRACT_26: &[DeclaredInput] = &[
    PRIMARY_RESERVES,
    REFERENCE_PRICE,
    feature(
        "gas_units",
        "features.gas_units",
        Domain::Positive,
        AbsentMeaning::DataGap,
    ),
    feature(
        "token0_per_eth",
        "features.token0_per_eth",
        Domain::Positive,
        AbsentMeaning::DataGap,
    ),
    // `op_26_flash_loan.rs:60` NO filtra dominio (`unwrap_or(0.003)`): un
    // `pool_fee = 0.0` presente es una fee real de pool, no un default.
    feature(
        "pool_fee",
        "features.pool_fee",
        Domain::Finite,
        AbsentMeaning::HardCodedDefault("0.003"),
    ),
    feature(
        "flash_premium",
        "features.flash_premium",
        Domain::Finite,
        AbsentMeaning::HardCodedDefault("0.0"),
    ),
];

/// Operadores que consumen `price_matrix` columna 0 como SERIE (los seis del
/// DATA_GAP que NO dependen de `features`): 5, 8, 10, 13, 16, 22.
pub const SERIES_OPERATOR_IDS: &[u8] = &[5, 8, 10, 13, 16, 22];

/// Contrato declarado del operador `id`. Vacío = el operador no está en el
/// conjunto medido (no se le inventa un contrato).
pub fn declared_contract(id: u8) -> &'static [DeclaredInput] {
    match id {
        5 => CONTRACT_05,
        8 => CONTRACT_08,
        10 => CONTRACT_10,
        11 => CONTRACT_11,
        13 => CONTRACT_13,
        16 => CONTRACT_16,
        21 => CONTRACT_21,
        22 => CONTRACT_22,
        26 => CONTRACT_26,
        _ => &[],
    }
}

fn value_in_domain(v: f64, domain: Domain) -> bool {
    v.is_finite()
        && match domain {
            Domain::Finite => true,
            Domain::Positive => v > 0.0,
            Domain::NonNegative => v >= 0.0,
        }
}

/// Columna 0 de `price_matrix`, sólo finitos. PERMISIVO a propósito: el conteo
/// se usa también como GATE (`ContractInputAdmission`) y los operadores son más
/// estrictos (`> 0`); un conteo permisivo nunca puede declarar "falta" cuando el
/// operador SÍ computa (falso rojo). El exceso sólo puede producir un
/// diagnóstico menos preciso — nunca un valor fabricado.
fn series_col0(state: &MarketState) -> Vec<f64> {
    state
        .price_matrix
        .iter()
        .filter_map(|row| row.first().copied())
        .filter(|p| p.is_finite())
        .collect()
}

/// ¿La entrada está PRESENTE y es UTILIZABLE? "Ausente" en este módulo cubre
/// ausente O presente-pero-inutilizable (no finito / fuera de dominio).
fn input_usable(input: &DeclaredInput, state: &MarketState) -> bool {
    match input.source {
        InputSource::Feature(k) => state
            .features
            .get(k)
            .is_some_and(|v| value_in_domain(*v, input.domain)),
        InputSource::FeatureAnyOf(keys) => keys.iter().any(|k| {
            state
                .features
                .get(*k)
                .is_some_and(|v| value_in_domain(*v, input.domain))
        }),
        InputSource::PriceSeries => series_col0(state).len() >= input.min_observations,
        InputSource::SignedReturns => {
            let prices = series_col0(state);
            if prices.len() < input.min_observations {
                return false;
            }
            let mut up = false;
            let mut down = false;
            for w in prices.windows(2) {
                if w[0] > 0.0 {
                    let r = w[1] / w[0] - 1.0;
                    if r > 0.0 {
                        up = true;
                    } else if r < 0.0 {
                        down = true;
                    }
                }
            }
            up && down
        }
        InputSource::PrimaryReserves => state
            .liquidity_reserves
            .first()
            .is_some_and(|(a, b)| a.is_finite() && b.is_finite() && *a > 0.0 && *b > 0.0),
        InputSource::ReferencePrice => {
            let cross = state
                .liquidity_reserves
                .iter()
                .skip(1)
                .any(|(a, b)| a.is_finite() && b.is_finite() && *a > 0.0 && *b > 0.0);
            let from_matrix = state
                .price_matrix
                .iter()
                .filter_map(|row| row.first().copied())
                .any(|p| p.is_finite() && p > 0.0);
            cross || from_matrix
        }
    }
}

/// Entradas DECLARADAS como DATA_GAP que no están presentes/utilizables.
/// Es la razón exacta que el recibo debe nombrar (R8: una fuente ausente se
/// reporta como ausente, nunca como `0.0`).
pub fn missing_required_inputs(id: u8, state: &MarketState) -> Vec<&'static str> {
    declared_contract(id)
        .iter()
        .filter(|i| i.absent == AbsentMeaning::DataGap && !input_usable(i, state))
        .map(|i| i.key)
        .collect()
}

/// Entradas que el operador va a rellenar con un default HARDCODEADO. Un
/// COMPUTED con esta lista no vacía es un valor real pero NO enteramente
/// sourced: se distingue en el recibo en vez de fingir cobertura total.
pub fn defaulted_inputs(id: u8, state: &MarketState) -> Vec<&'static str> {
    declared_contract(id)
        .iter()
        .filter(|i| {
            matches!(i.absent, AbsentMeaning::HardCodedDefault(_)) && !input_usable(i, state)
        })
        .map(|i| i.key)
        .collect()
}

/// Alcance de par de la serie de precios consumida (FEATURES-01b).
/// `"mixed_pairs"` = las filas son pares DISTINTOS, así que tratarlas como una
/// serie del mismo activo es un número plausible-pero-falso. Se declara en el
/// recibo; no se bloquea (el bloqueo lo decide el contrato de cada operador).
pub fn price_series_pair_scope(state: &MarketState) -> &'static str {
    let mut seen: Option<&str> = None;
    let mut rows = 0usize;
    for (i, row) in state.price_matrix.iter().enumerate() {
        if !row.first().copied().is_some_and(|p| p.is_finite()) {
            continue;
        }
        rows += 1;
        match state.pair_keys.get(i).map(String::as_str) {
            Some(k) if !k.is_empty() => match seen {
                None => seen = Some(k),
                Some(prev) if prev == k => {}
                Some(_) => return "mixed_pairs",
            },
            // Fila sin identidad de par: no se puede probar que sea el mismo par.
            _ => return "unkeyed",
        }
    }
    if rows == 0 {
        "empty"
    } else {
        "single_pair"
    }
}

/// Razón propia del operador, leída de su `metadata`. El primer `reason*` con
/// valor ≠ 0 en orden ALFABÉTICO (determinista: `HashMap` no lo es), o
/// `reason_code=<n>` si no hay etiquetas.
fn operator_metadata_reason(metadata: &HashMap<String, f64>) -> Option<String> {
    let mut reasons: Vec<&str> = metadata
        .iter()
        .filter(|(k, v)| k.starts_with("reason") && v.abs() > 0.0)
        .map(|(k, _)| k.as_str())
        .collect();
    reasons.sort_unstable();
    reasons.first().map(|k| (*k).to_string()).or_else(|| {
        metadata
            .get("reason_code")
            .copied()
            .filter(|c| c.is_finite())
            .map(|c| format!("reason_code={c}"))
    })
}

/// Razón del DATA_GAP, en orden de autoridad: (1) la entrada declarada que
/// falta — accionable; (2) la razón propia del operador; (3) el genérico.
fn data_gap_reason(missing: &[&'static str], operator_reason: Option<&str>) -> String {
    if !missing.is_empty() {
        return format!("missing_inputs:{}", missing.join("+"));
    }
    match operator_reason {
        Some(r) => format!("operator_reason:{r}"),
        None => "native_operator_returned_no_finite_value".to_string(),
    }
}

/// Superpone el CONTRATO por-operador a la admisión estructural del sitio
/// (`cartridge_boot.rs:1446`) sin editarla: se envuelve y se pasa como
/// `&dyn OperatorInputAdmission`. Opt-in — el call site v4 actual usa la
/// admisión estructural sola.
///
/// Seguro por construcción: `missing_required_inputs` sólo nombra entradas cuya
/// ausencia GARANTIZA `None` en el operador (los tests lo verifican), así que
/// envolver no puede convertir en DATA_GAP un operador que sí computa.
pub struct ContractInputAdmission<A> {
    pub inner: A,
}

impl<A: OperatorInputAdmission> OperatorInputAdmission for ContractInputAdmission<A> {
    fn validate(
        &self,
        id: u8,
        state: &MarketState,
        snapshot_id: &str,
        plan_hash: &str,
    ) -> Result<String, String> {
        let receipt = self.inner.validate(id, state, snapshot_id, plan_hash)?;
        let missing = missing_required_inputs(id, state);
        if missing.is_empty() {
            Ok(receipt)
        } else {
            Err(format!("missing_inputs:{}", missing.join("+")))
        }
    }
}

// ══════════════════════════════════════════════════════════════════════════════
// F3-NATIVEADAPTER-01 — PRODUCTOR PURO DE `MarketState.features`
// ══════════════════════════════════════════════════════════════════════════════

/// Fuentes REALES de las claves de `features` que consumen los operadores
/// nativos. Cada campo declara su fuente autoritativa; `None` significa "esa
/// fuente no está disponible en este call site" y la clave correspondiente
/// queda AUSENTE del mapa — nunca `0.0` (RULE 00 / R8).
#[derive(Debug, Clone, Default)]
pub struct OperatorFeatureSources<'a> {
    /// Fee declarada del leg de la ruta, en basis points (unidad del contrato
    /// del pool). Fuente: `RouteIntentLeg::fee_bps` del intent v4.
    pub fee_bps: Option<f64>,
    /// Fee del pool como FRACCIÓN. Fuente: catálogo de pools / fee del edge.
    /// Si se declara, gana sobre el `pool_fee` derivado de `fee_bps`.
    pub pool_fee: Option<f64>,
    /// Presupuesto de gas en unidades de gas. Fuente canónica:
    /// `TradingConfigState::gas_estimate_units`.
    pub gas_units: Option<f64>,
    /// Precio USD del token de gas/base. Fuente canónica:
    /// `TradingConfigState::base_token_price_usd`.
    pub gas_token_price_usd: Option<f64>,
    /// Símbolo del token de gas (p. ej. `"WETH"`), para la identidad exacta.
    pub gas_token_symbol: Option<&'a str>,
    /// Símbolo del token0 del pool primario (numerador de `token0_per_eth`).
    pub token0_symbol: Option<&'a str>,
    /// Tabla de precios USD del operador (símbolos case-insensitive). Fuente
    /// canónica: `TradingConfigState::token_prices_usd`.
    pub token_prices_usd: Option<&'a HashMap<String, f64>>,
    /// Premium del flash loan como FRACCIÓN (`0.0009` = 9 bps).
    ///
    /// NO se acepta `TradingConfigState::flashloan_fee_pct` directamente: el
    /// repo lo trata con DOS unidades incompatibles —
    /// `prioritization-spine/src/config_aware.rs:939` y
    /// `math-engine/src/roi_engine.rs:153` lo multiplican como fracción,
    /// mientras `cartridge_boot.rs:2156` divide por 100 (lo trata como
    /// porcentaje). Elegir una sería inventar la unidad: el llamador entrega la
    /// fracción ya resuelta.
    pub flash_premium_fraction: Option<f64>,
    /// Desviación de paridad de stablecoins medida del PriceBus vivo
    /// (`math_evidence.rs:238-253` la produce en la ruta de evidencia).
    pub parity_deviation: Option<f64>,
    /// Resultados CERRADOS del paper ledger para esta estrategia
    /// (`bayes_wins`/`bayes_losses` del posterior Beta-Binomial).
    pub bayes_wins: Option<f64>,
    pub bayes_losses: Option<f64>,
}

fn insert_if_finite(map: &mut HashMap<String, f64>, key: &str, value: Option<f64>) {
    if let Some(v) = value.filter(|v| v.is_finite()) {
        map.insert(key.to_owned(), v);
    }
}

/// `token0_per_eth` = cuántas unidades de token0 equivalen a 1 unidad del token
/// de gas. Fuente: los DOS precios USD reales (`base_token_price_usd` y
/// `token_prices_usd[token0]`). Sin ambos → `None` (clave ausente), nunca 1.0
/// inventado salvo identidad exacta token0 == token de gas.
fn token0_per_gas_token(s: &OperatorFeatureSources<'_>) -> Option<f64> {
    let gas_usd = s
        .gas_token_price_usd
        .filter(|v| v.is_finite() && *v > 0.0)?;
    let token0 = s.token0_symbol?;
    if s.gas_token_symbol
        .is_some_and(|g| g.eq_ignore_ascii_case(token0))
    {
        return Some(1.0);
    }
    let table = s.token_prices_usd?;
    let px = table
        .iter()
        .find(|(k, _)| k.eq_ignore_ascii_case(token0))
        .map(|(_, v)| *v)?;
    if !px.is_finite() || px <= 0.0 {
        return None;
    }
    Some(gas_usd / px)
}

/// Construye el mapa `features` a partir de fuentes REALES. Puras, sin I/O.
///
/// Invariante (RULE 00 / R8): una clave aparece SÓLO si su fuente existe y es
/// utilizable. El mapa vacío es una respuesta legítima — significa "ninguna
/// fuente disponible", que es exactamente lo que el operador debe ver como
/// DATA_GAP y no como cero.
pub fn market_features_from_sources(s: &OperatorFeatureSources<'_>) -> HashMap<String, f64> {
    let mut f: HashMap<String, f64> = HashMap::new();

    // fee_bps (unidad del contrato del pool) y su derivada pool_fee (fracción).
    // Mismo dato en las dos convenciones que leen los operadores: op_21/op_15
    // prefieren `fee_bps` (`op_21_newton.rs:49-56`), op_26 lee `pool_fee`
    // (`op_26_flash_loan.rs:60`).
    if let Some(bps) = s.fee_bps.filter(|v| v.is_finite() && *v >= 0.0) {
        f.insert("fee_bps".to_owned(), bps);
        f.insert("pool_fee".to_owned(), bps / 10_000.0);
    }
    // Fee explícita en fracción: gana sobre la derivada.
    if let Some(fee) = s.pool_fee.filter(|v| v.is_finite() && *v > 0.0) {
        f.insert("pool_fee".to_owned(), fee);
    }

    insert_if_finite(&mut f, "gas_units", s.gas_units.filter(|v| *v > 0.0));
    insert_if_finite(
        &mut f,
        "flash_premium",
        s.flash_premium_fraction.filter(|v| *v >= 0.0),
    );
    insert_if_finite(&mut f, "parity_deviation", s.parity_deviation);
    insert_if_finite(&mut f, "bayes_wins", s.bayes_wins.filter(|v| *v >= 0.0));
    insert_if_finite(&mut f, "bayes_losses", s.bayes_losses.filter(|v| *v >= 0.0));
    if let Some(rate) = token0_per_gas_token(s) {
        f.insert("token0_per_eth".to_owned(), rate);
    }
    f
}
