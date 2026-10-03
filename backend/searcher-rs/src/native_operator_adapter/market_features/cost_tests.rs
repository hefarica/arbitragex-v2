//! MARKET-FEATURES-01 / F7 — tests de los productores que faltaban del censo.
//!
//! Cada test afirma una PROPIEDAD del contrato, no que el código corra. La
//! estructura:
//!
//! 1. `pool_fee` / `fee_bps` — con lectura real: presentes y con el valor
//!    correcto en SU unidad; sin lectura: AUSENTES (nunca 0.003).
//! 2. `flash_premium` — con lectura autoritativa: fracción correcta; sin ella:
//!    AUSENTE + requisito externo exacto. Y el test que EXIGE el encargo:
//!    **falla si se inserta 0.0 sin acreditación** (barrido exhaustivo).
//! 3. `max_capital` / `break_even_target` — conversión real USD → unidades
//!    mínimas del numerario; sin config o sin escala: AUSENTES.
//! 4. Guardas de fabricación — ninguna clave por defecto, ningún 0.0 gratuito.
//! 5. Contrato — las cinco claves declaradas, con unidad/fuente/ventana/ausencia.

use super::*;
use std::collections::HashMap;

// ───────────────────────────── fixtures ─────────────────────────────

/// Un par V2 real: 30 bps como `30/10_000` (la forma en que el grafo lo lee).
fn v2_pair_read() -> PoolFeeRead {
    PoolFeeRead::new(30, 10_000)
}

/// Un tier V3 real: 3000 pips sobre 1e6 — el MISMO fee por otro camino.
fn v3_tier_read() -> PoolFeeRead {
    PoolFeeRead::new(3_000, 1_000_000)
}

/// Una lectura Aave V3 con procedencia completa, de 9 bps.
fn aave_read(premium_bps: u32) -> PremiumRead<'static> {
    PremiumRead {
        premium_bps,
        provider: "aave_v3",
        pool: "0x87870Bca3F3fD6335C3F4ce8392D69350B4fA4E2",
        chain_id: 1,
        block_ref: Some("0xabc123"),
        source: "eth_call Pool.FLASHLOAN_PREMIUM_TOTAL() @blockHash",
        evidence_id: "ev:rpc:1:0xabc123:FLASHLOAN_PREMIUM_TOTAL",
    }
}

/// Un numerario real: ETH a 2500 USD con 18 decimales.
fn eth_scale() -> TokenScale {
    TokenScale::new(2_500.0, 18)
}

/// `produce` con un `CostInputs` dado y nada más — el camino real de F7.
fn produce_cost_only(costs: &CostInputs<'_>) -> HashMap<String, f64> {
    let mut store = SeriesStore::default();
    let req = FeatureRequest {
        symbol: "WETH",
        onchain_price_usd: None,
        live_price_usd: None,
    };
    produce_with_costs(&mut store, &FeatureConfig::default(), &req, None, costs, 0)
}

/// Comparación con tolerancia RELATIVA para magnitudes derivadas.
///
/// `usd / precio × 10^decimales` es una cadena de operaciones en `f64`: su
/// resultado es el redondeo de un cociente que casi nunca es representable (p.
/// ej. `0.01` o `0.4` no lo son). Exigir igualdad de bits sobre esa cadena
/// haría fallar el test por el redondeo y no por la lógica — que es justo lo
/// contrario de lo que un test debe medir. La tolerancia es 1e-12 relativo:
/// suficiente para detectar cualquier error de escala (que sería de órdenes de
/// magnitud) y muy por debajo de cualquier diferencia significativa.
fn approx(actual: Option<f64>, expected: f64) -> bool {
    match actual {
        Some(a) => {
            let diff = (a - expected).abs();
            diff <= 1e-12 * expected.abs().max(1.0)
        }
        None => false,
    }
}

// ─────────────── 1. `pool_fee` / `fee_bps` ───────────────

#[test]
fn pool_fee_and_fee_bps_are_present_with_the_real_read_in_their_own_units() {
    let costs = CostInputs {
        pool_fee: Some(v2_pair_read()),
        ..Default::default()
    };
    let out = produce_cost_only(&costs);

    // `pool_fee` es una FRACCIÓN — el lector hace `gamma = 1.0 − fee`.
    assert_eq!(out.get(POOL_FEE_KEY).copied(), Some(0.003));
    // `fee_bps` son BASIS POINTS — el lector hace `bps / 10_000.0`.
    assert_eq!(out.get(FEE_BPS_KEY).copied(), Some(30.0));
    // Las dos unidades del MISMO hecho no pueden coincidir: si coincidieran,
    // una de las dos estaría en la unidad equivocada.
    assert_ne!(
        out.get(POOL_FEE_KEY).copied(),
        out.get(FEE_BPS_KEY).copied(),
        "fraccion y bps no pueden valer lo mismo"
    );
}

#[test]
fn a_v3_tier_and_a_v2_pair_of_the_same_fee_agree_after_conversion() {
    // 3000/1e6 y 30/1e4 son el mismo 0.30 % por caminos distintos: la
    // conversion debe converger, y por tanto `fee_bps` es comparable entre forks.
    let v2 = produce_cost_only(&CostInputs {
        pool_fee: Some(v2_pair_read()),
        ..Default::default()
    });
    let v3 = produce_cost_only(&CostInputs {
        pool_fee: Some(v3_tier_read()),
        ..Default::default()
    });
    assert_eq!(v2.get(FEE_BPS_KEY), v3.get(FEE_BPS_KEY));
    assert_eq!(v2.get(POOL_FEE_KEY), v3.get(POOL_FEE_KEY));
}

#[test]
fn pool_fee_is_absent_without_a_read_and_never_the_003_default() {
    let out = produce_cost_only(&CostInputs::default());
    assert!(
        !out.contains_key(POOL_FEE_KEY),
        "sin lectura la clave debe estar AUSENTE, no valer el default 0.003"
    );
    assert!(!out.contains_key(FEE_BPS_KEY), "el par es atomico");
    assert!(
        !out.values().any(|v| *v == 0.003),
        "el 0.003 del lector es un default de CONSUMO: el productor no lo emite jamas"
    );
}

#[test]
fn a_degenerate_fee_read_is_absent_rather_than_reinterpreted() {
    for (units, den) in [(30u32, 0u32), (10_001, 10_000), (u32::MAX, 10_000)] {
        let out = produce_cost_only(&CostInputs {
            pool_fee: Some(PoolFeeRead::new(units, den)),
            ..Default::default()
        });
        assert!(
            !out.contains_key(POOL_FEE_KEY) && !out.contains_key(FEE_BPS_KEY),
            "una lectura degenerada ({units}/{den}) se rechaza, no se reinterpreta"
        );
    }
}

#[test]
fn a_pool_with_no_commission_is_a_measurement_and_is_emitted_as_zero() {
    // `Some(0.0)` = computado y exactamente cero (R8). Un pool sin fee es un
    // hecho del despliegue, no una ausencia — al contrario que la falta de
    // lectura, que omite la clave.
    let out = produce_cost_only(&CostInputs {
        pool_fee: Some(PoolFeeRead::new(0, 10_000)),
        ..Default::default()
    });
    assert_eq!(out.get(POOL_FEE_KEY).copied(), Some(0.0));
    assert_eq!(out.get(FEE_BPS_KEY).copied(), Some(0.0));
}

// ─────────────── 2. `flash_premium` ───────────────

#[test]
fn flash_premium_is_the_fraction_the_reader_expects() {
    let out = produce_cost_only(&CostInputs {
        flash_premium: Some(aave_read(9)),
        ..Default::default()
    });
    // 9 bps => 0.0009 (op_26:67 hace `repayment = 1.0 + phi`).
    assert_eq!(out.get(FLASH_PREMIUM_KEY).copied(), Some(0.0009));
    // Explicitamente NO 9.0 (bps sin convertir) y NO 0.0 (flash gratis).
    assert_ne!(out.get(FLASH_PREMIUM_KEY).copied(), Some(9.0));
    assert_ne!(out.get(FLASH_PREMIUM_KEY).copied(), Some(0.0));

    // 5 bps (el valor que el prompt prohibe COPIAR de documentos) tiene que
    // salir de la MISMA via: si el productor lo emitiera desde el literal
    // `financing.rs:38`, este test seguiria pasando — por eso el valor no
    // aparece en ninguna parte del codigo de produccion (lo vigila el gate y lo
    // declara el contrato).
    let five = produce_cost_only(&CostInputs {
        flash_premium: Some(aave_read(5)),
        ..Default::default()
    });
    assert_eq!(five.get(FLASH_PREMIUM_KEY).copied(), Some(0.0005));
}

#[test]
fn flash_premium_is_absent_when_the_authoritative_read_is_missing() {
    let out = produce_cost_only(&CostInputs::default());
    assert!(
        !out.contains_key(FLASH_PREMIUM_KEY),
        "sin lectura NO hay premium: la clave queda ausente, jamas 0.0"
    );
}

#[test]
fn an_unprovenanced_read_is_not_a_having_of_the_datum() {
    // bps sin pool/fuente/evidencia son indistinguibles de un literal: se
    // rechazan, igual que `funding::resolve` rechaza `provider_reads_without_provenance`.
    let mut read = aave_read(9);
    read.evidence_id = "";
    assert_eq!(flash_premium_fraction(Some(&read)), None);

    let mut read = aave_read(9);
    read.pool = "   ";
    assert_eq!(flash_premium_fraction(Some(&read)), None);

    let mut read = aave_read(9);
    read.source = "";
    assert_eq!(flash_premium_fraction(Some(&read)), None);
}

#[test]
fn an_out_of_range_premium_is_rejected_not_clamped() {
    // Una lectura > 100 % es imposible; se rechaza en vez de recortarla.
    assert_eq!(flash_premium_fraction(Some(&aave_read(10_001))), None);
    // El limite exacto SI es valido (100 %): un borde no se inventa.
    assert_eq!(flash_premium_fraction(Some(&aave_read(10_000))), Some(1.0));
}

/// EL TEST QUE EXIGE EL ENCARGO: falla si se inserta `0.0` para
/// `flash_premium` sin que la fuente autoritativa lo acredite.
///
/// Barre TODAS las configuraciones de lectura alcanzables — incluida la
/// realista de producción (fee del pool conocido, premium desconocido) — y
/// exige la invariante: `flash_premium == 0.0` ⇒ la lectura existía Y estaba
/// acreditada como cero. Cualquier otro camino a un `0.0` es "financiación
/// flash gratuita" y hace fallar este test.
#[test]
fn flash_premium_is_never_zero_without_an_attested_read() {
    let reads: [Option<PremiumRead<'static>>; 6] = [
        // El caso de produccion: la lectura no llego.
        None,
        // Cero SIN procedencia: la fabricacion exacta que hay que cortar.
        Some(PremiumRead {
            premium_bps: 0,
            provider: "",
            pool: "",
            chain_id: 1,
            block_ref: None,
            source: "",
            evidence_id: "",
        }),
        // Cero con procedencia INCOMPLETA (sin evidencia).
        Some(PremiumRead {
            evidence_id: "",
            ..aave_read(0)
        }),
        // Cero CON procedencia completa: la unica via legitima a un 0.0.
        Some(aave_read(0)),
        // Valores reales.
        Some(aave_read(5)),
        Some(aave_read(9)),
    ];

    for read in reads {
        let costs = CostInputs {
            // El fee SI esta: el 0.0 no puede venir de "no habia nada".
            pool_fee: Some(v2_pair_read()),
            flash_premium: read,
            ..Default::default()
        };
        let out = produce_cost_only(&costs);

        match out.get(FLASH_PREMIUM_KEY).copied() {
            None => {}
            Some(0.0) => {
                let attested = read.as_ref().is_some_and(PremiumRead::zero_attested);
                assert!(
                    attested,
                    "flash_premium = 0.0 SIN lectura acreditada de cero en {read:?} \
                     -> financiacion flash gratuita fabricada"
                );
            }
            Some(v) => {
                assert!(v > 0.0 && v <= 1.0, "premium fuera de rango: {v}");
                let expected =
                    f64::from(read.expect("un valor exige una lectura").premium_bps) / 10_000.0;
                assert_eq!(v, expected);
            }
        }
    }
}

#[test]
fn the_flash_premium_requirement_names_the_exact_contract_method_and_network() {
    let req = flash_premium_requirement(1);
    assert_eq!(req.key, FLASH_PREMIUM_KEY);
    assert_eq!(req.status, RequirementStatus::BlockedExternal);
    assert_eq!(req.status.as_str(), "BLOCKED_EXTERNAL");
    // El metodo exacto y su denominador.
    assert!(
        req.call.contains("FLASHLOAN_PREMIUM_TOTAL()"),
        "el requisito debe nombrar el metodo exacto: {}",
        req.call
    );
    // El denominador se DERIVA de la constante canonica en vez de exigir el
    // literal `10_000`: la constante se renderiza sin separador de millares
    // (`10000`), asi que un literal con guion bajo es fragil por construccion y
    // convierte un cambio de constante en un falso rojo. Esto sigue anclando el
    // valor real, pero sin acoplar el test a su formato de impresion.
    let den = BPS_DENOMINATOR.to_string();
    let den_grouped: String = {
        let d = den.as_str();
        let mut out = String::new();
        for (i, ch) in d.chars().enumerate() {
            if i > 0 && (d.len() - i) % 3 == 0 {
                out.push('_');
            }
            out.push(ch);
        }
        out
    };
    assert!(
        req.call.contains(&den) || req.call.contains(&den_grouped) || req.call.contains("10^4"),
        "el denominador de bps debe viajar en la llamada ({den} o {den_grouped}): {}",
        req.call
    );
    // La red y la exigencia de ancla.
    assert!(req.network.contains("evm:1"));
    assert!(req.network.contains("blockHash"));
    // Los lectores que YA existen (evidencia reutilizable, no promesa).
    assert_eq!(req.existing_readers.len(), 2);
    for site in &req.existing_readers {
        assert!(
            site.contains(".rs:"),
            "cada lector cita archivo:linea: {site}"
        );
    }
    // Y lo que falta, nombrado.
    assert!(req.missing.contains("Provider<Http>"));
    assert!(req.missing.contains("cartridge_boot.rs:2107"));
}

#[test]
fn a_missing_premium_yields_the_requirement_and_never_a_not_applicable() {
    let costs = CostInputs {
        pool_fee: Some(v2_pair_read()),
        ..Default::default()
    };
    let reqs = costs.external_requirements(1);
    assert_eq!(reqs.len(), 1, "exactamente el premium esta bloqueado");
    assert_eq!(reqs[0].key, FLASH_PREMIUM_KEY);
    assert_eq!(reqs[0].status, RequirementStatus::BlockedExternal);

    // Con la lectura presente, el requisito desaparece: no es un cartel fijo.
    let solved = CostInputs {
        flash_premium: Some(aave_read(9)),
        ..Default::default()
    };
    assert!(solved.external_requirements(1).is_empty());
}

// ─────────────── 3. `max_capital` / `break_even_target` ───────────────

#[test]
fn max_capital_is_the_configured_capital_in_numeraire_min_units() {
    let out = produce_cost_only(&CostInputs {
        capital_usd: Some(1_000.0),
        numeraire: Some(eth_scale()),
        ..Default::default()
    });
    // 1000 USD / 2500 USD-por-ETH = 0.4 ETH = 0.4e18 unidades minimas.
    assert!(
        approx(out.get(MAX_CAPITAL_KEY).copied(), 4e17),
        "esperado 4e17, obtenido {:?}",
        out.get(MAX_CAPITAL_KEY)
    );
    // Ni el default fabricado (1.0) ni el USD crudo (1000.0).
    assert_ne!(out.get(MAX_CAPITAL_KEY).copied(), Some(1.0));
    assert_ne!(out.get(MAX_CAPITAL_KEY).copied(), Some(1_000.0));
}

#[test]
fn break_even_target_is_the_configured_objective_in_numeraire_min_units() {
    let out = produce_cost_only(&CostInputs {
        min_profit_usd: Some(25.0),
        numeraire: Some(eth_scale()),
        ..Default::default()
    });
    // 25 USD / 2500 = 0.01 ETH = 1e16 unidades minimas.
    assert!(
        approx(out.get(BREAK_EVEN_TARGET_KEY).copied(), 1e16),
        "esperado 1e16, obtenido {:?}",
        out.get(BREAK_EVEN_TARGET_KEY)
    );
    // El default prohibido (0.0) haria desaparecer el objetivo del operador.
    assert_ne!(out.get(BREAK_EVEN_TARGET_KEY).copied(), Some(0.0));
}

#[test]
fn a_six_decimal_numeraire_scales_the_cap_by_its_own_decimals() {
    // USDC como numerario: 6 decimales, 1 USD. Si el productor ignorara los
    // decimales, el cupo saldria 1e12 veces mayor.
    let out = produce_cost_only(&CostInputs {
        capital_usd: Some(1_000.0),
        numeraire: Some(TokenScale::new(1.0, 6)),
        ..Default::default()
    });
    assert!(
        approx(out.get(MAX_CAPITAL_KEY).copied(), 1_000.0 * 1e6),
        "esperado 1e9, obtenido {:?}",
        out.get(MAX_CAPITAL_KEY)
    );
}

#[test]
fn the_solver_keys_are_absent_without_config_or_without_scale() {
    // Sin configuracion.
    let out = produce_cost_only(&CostInputs {
        numeraire: Some(eth_scale()),
        ..Default::default()
    });
    assert!(!out.contains_key(MAX_CAPITAL_KEY));
    assert!(!out.contains_key(BREAK_EVEN_TARGET_KEY));

    // Con configuracion pero sin escala: la conversion no existe.
    let out = produce_cost_only(&CostInputs {
        capital_usd: Some(1_000.0),
        min_profit_usd: Some(25.0),
        numeraire: None,
        ..Default::default()
    });
    assert!(
        !out.contains_key(MAX_CAPITAL_KEY) && !out.contains_key(BREAK_EVEN_TARGET_KEY),
        "sin precio/decimales del numerario no hay unidades minimas que emitir"
    );
    assert!(!out.values().any(|v| *v == 0.0), "{out:?}");
}

#[test]
fn an_unusable_scale_or_a_non_positive_target_is_absent() {
    for scale in [
        TokenScale::new(0.0, 18),
        TokenScale::new(-1.0, 18),
        TokenScale::new(f64::NAN, 18),
        TokenScale::new(f64::INFINITY, 18),
        TokenScale::new(2_500.0, 78),
    ] {
        assert!(!scale.is_usable(), "{scale:?}");
        let out = produce_cost_only(&CostInputs {
            capital_usd: Some(1_000.0),
            numeraire: Some(scale),
            ..Default::default()
        });
        assert!(!out.contains_key(MAX_CAPITAL_KEY), "{scale:?}");
    }

    for usd in [0.0, -5.0, f64::NAN, f64::INFINITY] {
        let out = produce_cost_only(&CostInputs {
            capital_usd: Some(usd),
            numeraire: Some(eth_scale()),
            ..Default::default()
        });
        assert!(!out.contains_key(MAX_CAPITAL_KEY), "capital_usd={usd}");
    }
}

// ─────────────── 4. Guardas de fabricacion ───────────────

#[test]
fn no_cost_key_is_emitted_as_a_default_when_everything_is_missing() {
    let out = produce_cost_only(&CostInputs::default());
    assert!(
        out.is_empty(),
        "sin ninguna lectura el mapa debe quedar VACIO, no poblado con defaults: {out:?}"
    );
    for key in COST_KEYS {
        assert!(
            !out.contains_key(*key),
            "`{key}` debe estar AUSENTE, nunca con un default"
        );
    }
}

#[test]
fn every_cost_input_present_still_produces_only_the_five_declared_keys() {
    // El caso MAXIMO: ningun hueco que excuse una clave de mas ni de menos.
    let out = produce_cost_only(&CostInputs {
        pool_fee: Some(v2_pair_read()),
        flash_premium: Some(aave_read(9)),
        capital_usd: Some(1_000.0),
        numeraire: Some(eth_scale()),
        min_profit_usd: Some(25.0),
    });
    let produced: std::collections::HashSet<&str> = out.keys().map(String::as_str).collect();
    let declared: std::collections::HashSet<&str> = COST_KEYS.iter().copied().collect();
    assert_eq!(
        produced, declared,
        "el productor emite exactamente las cinco claves que declara"
    );
}

#[test]
fn the_cost_keys_never_duplicate_a_live_producer() {
    for key in COST_KEYS {
        let c = contract_for(key).unwrap_or_else(|| panic!("sin contrato para `{key}`"));
        assert!(
            matches!(c.owner, Owner::ThisModule),
            "`{key}` es de este modulo; declararlo Elsewhere duplicaria un productor vivo"
        );
    }
    // Y las claves de otros siguen siendo de otros.
    for key in ["parity_deviation", "health_factor"] {
        let c = contract_for(key).expect("documented");
        assert!(matches!(c.owner, Owner::Elsewhere(_)), "`{key}`");
    }
}

// ─────────────── 5. Contrato ───────────────

#[test]
fn the_five_new_keys_have_a_complete_contract_with_their_reader_cited() {
    for key in COST_KEYS {
        let c = contract_for(key).unwrap_or_else(|| panic!("sin contrato para `{key}`"));
        for (field, value) in [
            ("unit", c.unit),
            ("source", c.source),
            ("window", c.window),
            ("absent_means", c.absent_means),
        ] {
            assert!(
                value.trim().len() > 20,
                "`{key}`.{field} debe ser una declaracion real: {value:?}"
            );
        }
        // La FUENTE tiene que citar evidencia verificable: archivo:linea (o el
        // contrato del proveedor, que en `flash_premium` va por `pending`).
        assert!(
            c.source.contains(".rs:"),
            "`{key}`.source debe citar archivo:linea: {}",
            c.source
        );
        // La ausencia NUNCA se describe como un numero por defecto: el contrato
        // debe DECLARAR la omision de la clave, no heredar el mecanismo de otra.
        // Se acepta cualquier forma de declararla; exigir una frase unica
        // convierte el test en un corrector de estilo, no en una verificacion.
        let a = c.absent_means.to_ascii_lowercase();
        assert!(
            a.contains("no se inserta") || a.contains("no se emite"),
            "`{key}`.absent_means debe declarar la omision de la clave, no un default: {}",
            c.absent_means
        );
    }
}

#[test]
fn the_monetary_solver_keys_declare_themselves_monetary_and_the_f64_limit() {
    // Doctrina §4: `features` es HashMap<String,f64>, asi que una clave monetaria
    // no puede evitar el tipo — se declara, y su unidad dice que es una MAGNITUD
    // de una familia concreta, no un importe contable.
    for key in [MAX_CAPITAL_KEY, BREAK_EVEN_TARGET_KEY] {
        let c = contract_for(key).unwrap();
        assert!(c.monetary, "`{key}` es una magnitud y debe declararlo");
        assert!(
            c.unit.contains("unidades mínimas"),
            "`{key}` debe declarar su familia de unidades: {}",
            c.unit
        );
    }
    // Las tres no monetarias no se declaran monetarias.
    for key in [POOL_FEE_KEY, FEE_BPS_KEY, FLASH_PREMIUM_KEY] {
        assert!(
            !contract_for(key).unwrap().monetary,
            "`{key}` es adimensional"
        );
    }
}

#[test]
fn the_owned_key_list_covers_exactly_the_contracts_this_module_claims() {
    let owned: std::collections::HashSet<&str> = owned_contracts().map(|c| c.key).collect();
    let declared: std::collections::HashSet<&str> = OWNED_KEYS.iter().copied().collect();
    assert_eq!(owned, declared, "OWNED_KEYS y CONTRACTS deben coincidir");
    for key in COST_KEYS {
        assert!(declared.contains(key), "`{key}` debe estar en OWNED_KEYS");
    }
}

// ─────────────── 6. Camino global (adaptador) ───────────────

#[test]
fn the_global_adapter_without_a_bus_reports_the_requirement_instead_of_a_zero() {
    // Sin bus no hay precios, pero el fee del pool SI es un dato del grafo y
    // debe llegar igual: el adaptador global no puede perder lo que ya tenia.
    let costs = CostInputs {
        pool_fee: Some(v2_pair_read()),
        flash_premium: None,
        numeraire: Some(eth_scale()),
        capital_usd: Some(1_000.0),
        ..Default::default()
    };
    let out = produce_from_global_with_costs(
        &FeatureConfig::default(),
        None,
        "WETH",
        None,
        &costs,
        1_700_000_000_000_000_000,
    );
    assert_eq!(out.get(POOL_FEE_KEY).copied(), Some(0.003));
    assert!(
        approx(out.get(MAX_CAPITAL_KEY).copied(), 4e17),
        "obtenido {:?}",
        out.get(MAX_CAPITAL_KEY)
    );
    assert!(
        !out.contains_key(FLASH_PREMIUM_KEY),
        "sin lectura del proveedor no hay premium, ni siquiera 0.0"
    );
    assert!(
        !out.contains_key(VOLATILITY_KEY),
        "sin bus no hay serie, asi que no hay volatilidad"
    );
}
