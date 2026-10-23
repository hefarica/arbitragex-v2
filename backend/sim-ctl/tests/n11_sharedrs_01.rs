//! N11-SHAREDRS-01 — el descarte silencioso de `verdict`/`verdict_reason`.
//!
//! CONTEXTO MEDIDO (t115, PR #865). El selector publica `verdict` y
//! `verdict_reason` desde #843, pero `shared_rs::contracts::Opportunity` no las
//! declara y no lleva `deny_unknown_fields`. `serde` ignora las claves
//! desconocidas, así que **se descartan en SILENCIO**: el consumidor cree haber
//! leído un campo que nunca llegó. Cuantificado: **el 100% de la ventana**
//! (10.000/10.000) trae `"verdict":"reject"`.
//!
//! POR QUE ESTOS TESTS Y NO EL CAMBIO AL STRUCT. El arreglo natural —declarar
//! `#[serde(default)] pub verdict: Option<String>` en `Opportunity`, que es
//! exactamente lo que t115 propuso— **rompe 15 inicializadores en
//! `searcher-rs`** (`error[E0063]: missing fields 'verdict' and 'verdict_reason'
//! in initializer of Opportunity`), un crate **fuera del inScope** de t124, y que
//! además tiene **52 literales `Opportunity {` en 29 archivos**. El contrato
//! (AC5) manda **PARAR Y REPORTARLO** cuando eso pasa, y es lo que se hizo.
//!
//! Mientras tanto, este archivo fija las DOS cosas que no dependen de un cambio
//! fuera de alcance:
//!   1. que el descarte es **silencioso** (no un `Err` ruidoso) — la propiedad
//!      que lo vuelve peligroso, y que no debe cambiar por accidente;
//!   2. la **semántica exigida** al campo, ejecutable sobre un espejo local, de
//!      modo que cuando el struct la declare, el espejo ya es la especificación
//!      que el cambio tiene que satisfacer.

use shared_rs::contracts::Opportunity;

/// Payload REAL de `arbx:opps:validated`, capturado verbatim con
/// `XREVRANGE arbx:opps:validated + - COUNT 1`. Trae las dos claves.
const REAL_ENTRY: &str = r#"{"id":"28691901-840e-4eb5-9f24-77e6375304ab","chain_id":1,"strategy_kind":"dex_arb","dex_a":"UniswapV3","dex_b":"PancakeSwap V3","pair_symbol":"c02aaa…/dac17f…","token_in":"0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2","token_out":"0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2","amount_in_wei":"1000000000000000000","expected_profit_usd":-2.0276126852101792,"net_expected_profit_usd":-2.6785281852101788,"roi_pct":null,"risk_score":null,"block_number":26145729,"rejection_reason":"spread_negative_round_trip","cartridge_id":null,"detector_id":"dex_engine","pipeline_latency_ms":1048,"detected_at":"2026-10-08T05:56:37.111885481Z","trace_id":"ea6e06e6-9bdb-42ad-8a4b-35a1885aac34","economics":{"computation_status":"computed","error_reason":null,"amount_in_wei":"1000000000000000000","amount_out_wei":null,"amount_in_usd":2561.9524274451996,"amount_out_usd":2559.9248147599897,"gross_profit_usd":-2.0276126852101792,"gas_usd":0.6409155,"dex_fees_usd":null,"flash_fee_usd":0,"bribe_usd":0,"slippage_usd":null,"other_costs_usd":0.01,"total_cost_usd":0.6509155,"net_profit_usd":-2.6785281852101788,"roi_pct":-0.10455027019690719,"target_net_usd":50,"target_delta_usd":-52.678528185210176,"meets_target":false,"quote_block":26145729,"simulation_block":null,"legs":[],"not_computed_reasons":{"amount_out_wei":"cycle_output_not_exposed_by_kernel","dex_fees_usd":"included_in_amount_out_post_fee","simulation_block":"revm_simulation_is_sim_ctl_scope","slippage_usd":"priced_by_amm_curve"}},"verdict":"reject","verdict_reason":"producer_rejected"}"#;

/// (1) El descarte es SILENCIOSO. Esta es la medida, no una afirmación: con
/// `deny_unknown_fields` el parseo sería un `Err` — y un `Err` sería RUIDOSO.
/// Que sea `Ok` es precisamente lo que hace que el campo se pierda sin que
/// nadie se entere. **Si algún día esto pasa a `Err`, el diagnóstico cambia y
/// este test debe actualizarse a propósito, no por accidente.**
#[test]
fn v1_the_discard_is_silent_not_an_error() {
    // Control de productor: el payload REAL trae las dos claves.
    assert!(REAL_ENTRY.contains("\"verdict\":\"reject\""));
    assert!(REAL_ENTRY.contains("\"verdict_reason\":\"producer_rejected\""));

    let opp: Opportunity = serde_json::from_str(REAL_ENTRY)
        .expect("claves desconocidas se ignoran: el descarte es silencioso, no un error");

    // El resto del payload SÍ llega: lo que se pierde es sólo el veredicto.
    assert_eq!(opp.amount_in_wei, "1000000000000000000");
    assert_eq!(
        opp.rejection_reason.as_deref(),
        Some("spread_negative_round_trip")
    );
}

/// (2) ESPECIFICACIÓN EJECUTABLE del campo que falta.
///
/// Espeja EXACTAMENTE lo que `Opportunity` tiene que declarar —
/// `#[serde(default)] pub verdict: Option<String>` y lo mismo para
/// `verdict_reason`— y fija las DOS direcciones de AC3, porque **un lector que
/// inventa un default es tan malo como uno que pierde el dato**: el primero
/// fabrica (`reject`/`accept` sin que nadie lo haya dicho), el segundo oculta.
///
/// Cuando el struct declare el campo, este espejo es la especificación que el
/// cambio tiene que satisfacer. No se toca `Opportunity` desde acá: el arreglo
/// exige editar `searcher-rs`, fuera del inScope (ver el encabezado).
#[derive(Debug, serde::Deserialize)]
struct VerdictMirror {
    #[serde(default)]
    verdict: Option<String>,
    #[serde(default)]
    verdict_reason: Option<String>,
}

/// (2·i) CON las claves presentes se leen sus **VALORES**, tal cual vienen: el
/// lector no traduce ni normaliza el veredicto.
#[test]
fn ac3_present_keys_are_read_verbatim() {
    let m: VerdictMirror =
        serde_json::from_str(r#"{"verdict":"reject","verdict_reason":"producer_rejected"}"#)
            .expect("present keys must parse");
    assert_eq!(m.verdict.as_deref(), Some("reject"));
    assert_eq!(m.verdict_reason.as_deref(), Some("producer_rejected"));

    // Y el MISMO payload REAL del stream, que es el caso de producción.
    let m2: VerdictMirror = serde_json::from_str(REAL_ENTRY).expect("real entry must parse");
    assert_eq!(m2.verdict.as_deref(), Some("reject"));
    assert_eq!(m2.verdict_reason.as_deref(), Some("producer_rejected"));

    // Un veredicto DESCONOCIDO se expone tal cual, no se colapsa a un valor
    // conocido: el lector registra, no interpreta.
    let m3: VerdictMirror = serde_json::from_str(r#"{"verdict":"QUIZAS"}"#).unwrap();
    assert_eq!(m3.verdict.as_deref(), Some("QUIZAS"));
}

/// (2·ii) CON las claves **AUSENTES o MALFORMADAS** el lector **NO INVENTA** un
/// veredicto: queda `None`. **Nunca un `"reject"` fabricado.** Este es el
/// criterio de AC3 en la dirección que protege contra la fabricación, y es el
/// mismo que t115 fijó para su lector de JSON crudo.
#[test]
fn ac3_absent_or_malformed_keys_never_fabricate_a_verdict() {
    // Ausentes por completo.
    let m: VerdictMirror = serde_json::from_str(r#"{"id":"x"}"#).unwrap();
    assert_eq!(
        m.verdict, None,
        "ausente -> None, NUNCA un veredicto fabricado"
    );
    assert_eq!(m.verdict_reason, None);

    // Presentes pero `null` explícito -> None, no un valor por defecto.
    let m: VerdictMirror = serde_json::from_str(r#"{"verdict":null}"#).unwrap();
    assert_eq!(m.verdict, None);

    // MALFORMADAS: un número o un objeto NO se interpretan como veredicto.
    //
    // MEDIDO, y NO es lo que uno supone: `#[serde(default)]` cubre la clave
    // **AUSENTE**, no la de TIPO EQUIVOCADO. Con la clave presente y mal tipada,
    // serde devuelve un **`Err`** — no un `None`. El lector **se niega** en vez
    // de fabricar, que es la dirección correcta; pero **no es lo mismo que
    // `None`**, y la diferencia tiene consecuencia: en `sim-ctl`, un `Err` de
    // `serde_json::from_str::<Opportunity>` cae en `invalid_msg_parse`, se ACKea
    // y **se descarta el mensaje ENTERO**. Se mide y se declara; no se disimula.
    for (caso, json) in [
        ("entero", r#"{"verdict":123}"#),
        ("objeto", r#"{"verdict":{"a":1}}"#),
        ("booleano", r#"{"verdict":true}"#),
    ] {
        let r = serde_json::from_str::<VerdictMirror>(json);
        assert!(
            r.is_err(),
            "{caso}: tipo equivocado debe dar Err (el lector se NIEGA), no None ni un valor"
        );
        // Y lo que NUNCA debe pasar: que un valor mal tipado se convierta en un
        // veredicto. Si alguien "arreglara" esto con un default concreto, el
        // parseo devolvería Ok y este assert lo cazaría.
        if let Ok(m) = r {
            assert_ne!(m.verdict.as_deref(), Some("reject"));
            assert_ne!(m.verdict.as_deref(), Some("accept"));
        }
    }
}

/// (2·iii) CONTROL de la dirección opuesta: la clave **AUSENTE** sí queda en
/// `None`, que es lo que AC3 exige. Se separa del test de malformadas porque
/// **son dos comportamientos distintos** y mezclarlos fue el error que este
/// archivo tuvo que corregir al medirlo.
#[test]
fn ac3_absent_key_is_none_never_a_fabricated_verdict() {
    let m: VerdictMirror = serde_json::from_str(r#"{}"#).unwrap();
    assert_eq!(m.verdict, None, "ausente -> None");
    assert_eq!(m.verdict_reason, None);
    // Un default accidental inyectado en el struct real no podría pasar inadvertido.
    assert_ne!(m.verdict.as_deref(), Some("reject"));
    assert_ne!(m.verdict.as_deref(), Some("accept"));

    // `null` explícito -> también `None` (no es "ausente", pero tampoco un valor).
    let m: VerdictMirror = serde_json::from_str(r#"{"verdict":null}"#).unwrap();
    assert_eq!(m.verdict, None);
}
