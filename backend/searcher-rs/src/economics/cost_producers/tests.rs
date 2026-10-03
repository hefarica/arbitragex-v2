//! §14 — pruebas obligatorias de la resolución de costos.
//!
//! Cubre, una por una, las exigencias del prompt:
//! fees cero acreditados · tarifa ausente · pierna incompleta · cambio dinámico
//! de fee · precisión/overflow · rounding · tokens con distintos decimals ·
//! precio stale · procedencia incorrecta · y la verificación explícita de que
//! fees embebidos, protocol fee, financing, priority tip y builder **no se
//! descuentan dos veces**.
//!
//! Los valores de estos tests son FIXTURES: viven sólo aquí y no son valores
//! productivos (RULE 00). El módulo nunca los usa como defecto.

use bigdecimal::BigDecimal;
use ethers::types::U256;
use std::str::FromStr;

use super::*;

// ─────────────────────────── fixtures de contexto ───────────────────────────

fn asset(chain: u64, token: &str, dec: u8) -> AssetRef {
    AssetRef::new(chain, token, dec)
}

fn price_ref(chain: u64, token: &str, dec: u8, usd: &str, rev: &str) -> PriceRef {
    PriceRef {
        chain_id: chain,
        token: token.into(),
        decimals: dec,
        usd: BigDecimal::from_str(usd).unwrap(),
        revision: rev.into(),
        evidence_id: format!("pricebus:{token}:{rev}"),
        producer: "PriceBus".into(),
        observed_at_ms: 1_000,
        valid_until_ms: Some(2_000),
    }
}

fn anchor(chain: u64) -> VenueAnchor {
    VenueAnchor::evm(chain, 21_000_000, Some("0xabc123"))
}

fn bd(s: &str) -> BigDecimal {
    BigDecimal::from_str(s).unwrap()
}

fn ctx<'a>(
    scope: Scope,
    asset: &AssetRef,
    price: &'a PriceRef,
    rev: &'a str,
    now: u64,
    anchor: &'a VenueAnchor,
) -> ComponentCtx<'a> {
    ComponentCtx {
        chain_id: asset.chain_id,
        scope,
        asset: asset.clone(),
        price,
        price_revision: rev,
        now_ms: now,
        anchor,
    }
}

fn route_scope() -> Scope {
    Scope::route("evm:1")
}

fn seed(kind: &str, treatment: Treatment) -> ComponentSeed {
    ComponentSeed {
        kind: kind.into(),
        scope: route_scope(),
        asset: AssetRef::native(1, 18),
        treatment,
        payer: "searcher".into(),
        source: "fixture".into(),
        adapter_version: "fixture/1".into(),
    }
}

// ═════════════════════════ FEE CERO ACREDITADO ═════════════════════════

#[test]
fn zero_fee_is_preserved_as_zero_not_absence() {
    let a = asset(1, "0xusdc", 6);
    let p = price_ref(1, "0xusdc", 6, "1.000000", "r1");
    let an = anchor(1);
    let c = ctx(Scope::leg(0, "v2pair", "evm:1"), &a, &p, "r1", 1_500, &an);
    // Deployment con numerator == denominator: comisión exactamente cero.
    let terms = univ2::V2FeeTerms::from_read(
        U256::from(10_000u64),
        U256::from(10_000u64),
        univ2::V2Fork::Unknown,
        "pair.fee()",
        "ev:pair:fee",
    )
    .unwrap();
    let comp = univ2::zero_fee_component(&c, &terms, "searcher");
    assert_eq!(comp.state, CostState::ZeroAttested);
    assert_eq!(comp.usd, Some(bd("0")));

    let mut r = CostResolution::new(1, "snap", "r1", "before_financing", "atomic_quote");
    r.push(comp);
    let lines = r.to_cost_lines().unwrap();
    let line = lines
        .iter()
        .find(|l| l.kind == KIND_EXECUTION_FEES)
        .unwrap();
    assert_eq!(line.usd.as_deref(), Some("0"));
    // Cero computado: NO bloquea el neto (a diferencia de la ausencia).
    assert!(r.blocking().is_empty());
    assert_eq!(r.external_usd(), Some(bd("0")));
}

#[test]
fn v3_protocol_fee_is_zero_when_no_tick_is_crossed() {
    let reads = univ3::V3FeeReads {
        fee_raw: U256::from(3_000u64),
        protocol_fee_num_raw: U256::from(1u64),
        protocol_fee_den_raw: U256::from(10u64),
        crosses_initialized_tick: false,
        source: "pool".into(),
        evidence_id: "ev:pool:fee".into(),
    };
    let t = univ3::V3FeeTerms::resolve(&reads).unwrap();
    assert_eq!(t.fee_pips, 3_000);
    assert_eq!(
        t.protocol_pips, 0,
        "sin cruce de tick la comisión protocolar es 0"
    );
    assert_eq!(t.lp_pips, 3_000);

    // Con cruce sí se aplica el corte del factory.
    let crossed = univ3::V3FeeReads {
        crosses_initialized_tick: true,
        ..reads.clone()
    };
    let t2 = univ3::V3FeeTerms::resolve(&crossed).unwrap();
    assert_eq!(t2.protocol_pips, 300);
    assert_eq!(t2.lp_pips, 2_700);
}

#[test]
fn v4_dynamic_lp_fee_flag_is_not_a_fee() {
    let dynamic_key = univ4::PoolKey {
        currency0: "0xa".into(),
        currency1: "0xb".into(),
        fee_raw: univ4::DYNAMIC_FEE_FLAG | 3_000,
        tick_spacing: 60,
        hooks: "0xh".into(),
    };
    assert!(dynamic_key.is_dynamic_fee());
    assert_eq!(dynamic_key.static_lp_fee_pips(), None);

    let inputs = univ4::V4FeeInputs {
        key: dynamic_key,
        protocol_fee_packed: 0,
        dynamic_lp_fee_pips: None,
        hook_fee_pips: Some(0),
        before_swap_rebate_raw: None,
        source: "poolmanager".into(),
        evidence_id: "ev:v4".into(),
    };
    let err = univ4::V4FeeTerms::resolve(&inputs, true).unwrap_err();
    match err {
        CostError::MissingRead { read, .. } => {
            assert!(read.contains("dynamic_lp_fee"), "read exacto: {read}");
        }
        other => panic!("esperaba MissingRead, obtuve {other:?}"),
    }
}

// ═════════════════════════ TARIFA AUSENTE ═════════════════════════

#[test]
fn missing_aave_premium_read_yields_a_task_never_a_default() {
    let a = asset(1, "0xweth", 18);
    let reads = funding::ProviderReads {
        provider: "aave_v3".into(),
        pool: "0xpool".into(),
        chain_id: 1,
        method: funding::FundingMethod::AaveV3FlashLoan,
        premium_total_bps: None, // ← la lectura NO existe
        premium_to_protocol_bps: None,
        capacity_raw: Some(U256::from(1_000_000u64)),
        source: "evm".into(),
        evidence_id: "ev:aave".into(),
    };
    let err = funding::resolve(&reads, &a, &U256::from(1_000u64)).unwrap_err();
    assert_eq!(err.state(), CostState::PendingResolution);
    let task = err.task(funding::KIND_FINANCING_PREMIUM, &route_scope());
    assert!(
        task.read.contains("FLASHLOAN_PREMIUM_TOTAL"),
        "la tarea debe nombrar la lectura exacta, obtuve {}",
        task.read
    );

    // Y la resolución degrada a un componente PENDIENTE, no a un cero.
    let mut r = CostResolution::new(1, "snap", "r1", "before_financing", "atomic_quote");
    r.absorb(
        seed(funding::KIND_FINANCING_PREMIUM, Treatment::External),
        Err(err),
    );
    assert_eq!(r.components.len(), 1);
    assert_eq!(r.components[0].state, CostState::PendingResolution);
    assert!(r.components[0].usd.is_none());
    assert_eq!(r.external_usd(), None, "sin la tarifa no hay neto honesto");
    assert!(!r.tasks.is_empty());
}

#[test]
fn aave_premium_comes_from_the_read_not_from_a_constant() {
    // La MISMA función con tres lecturas distintas produce tres precios
    // distintos: el 5/9 bps de los fixtures no está en ninguna parte.
    let principal = U256::from(1_000_000u64);
    assert_eq!(
        funding::aave_premium_raw(&principal, &U256::from(5u64)).unwrap(),
        U256::from(500u64)
    );
    assert_eq!(
        funding::aave_premium_raw(&principal, &U256::from(9u64)).unwrap(),
        U256::from(900u64)
    );
    // Una lectura que no sea 5 ni 9 también funciona → no hay constante.
    assert_eq!(
        funding::aave_premium_raw(&principal, &U256::from(7u64)).unwrap(),
        U256::from(700u64)
    );
    // Gobernanza que sube el premium a 1% se refleja exacta.
    assert_eq!(
        funding::aave_premium_raw(&principal, &U256::from(100u64)).unwrap(),
        U256::from(10_000u64)
    );
    // Redondeo floor, nunca hacia arriba.
    assert_eq!(
        funding::aave_premium_raw(&U256::from(1u64), &U256::from(5u64)).unwrap(),
        U256::from(0u64),
        "1 wei × 5 bps = 0.0005 → floor = 0"
    );
}

#[test]
fn v2_fee_terms_are_read_and_a_documented_mismatch_raises_a_task() {
    // Lectura 9975/10000 en un deployment declarado UniswapV2 → discrepancia.
    let terms = univ2::V2FeeTerms::from_read(
        U256::from(9_975u64),
        U256::from(10_000u64),
        univ2::V2Fork::UniswapV2,
        "pair.fee()",
        "ev:fee",
    )
    .unwrap();
    let task = terms
        .disagreement_task(&route_scope())
        .expect("discrepancia");
    assert_eq!(
        task.receipt.as_deref(),
        Some("exact_invariant_version_and_rates")
    );
    // La lectura coherente no genera tarea.
    let ok = univ2::V2FeeTerms::from_read(
        U256::from(997u64),
        U256::from(1_000u64),
        univ2::V2Fork::SushiSwap,
        "pair.fee()",
        "ev:fee",
    )
    .unwrap();
    assert!(ok.disagreement_task(&route_scope()).is_none());
    // Un fork SIN tarifa documentada nunca produce un valor por defecto.
    assert_eq!(univ2::V2Fork::Unknown.documented_terms(), None);
}

// ═════════════════════════ UNA PIERNA INCOMPLETA ═════════════════════════

#[test]
fn an_incomplete_leg_blocks_the_net_instead_of_contributing_zero() {
    // Pierna 0 completa (gas), pierna 1 sin lectura de fee → la resolución NO
    // puede cerrar el neto.
    let mut r = CostResolution::new(1, "snap", "r1", "before_financing", "atomic_quote");
    let a = asset(1, "0xusdc", 6);
    let p = price_ref(1, "0xusdc", 6, "1.0", "r1");
    let an = anchor(1);
    let c = ctx(Scope::leg(0, "pair0", "evm:1"), &a, &p, "r1", 1_500, &an);
    let terms = univ2::V2FeeTerms::from_read(
        U256::from(997u64),
        U256::from(1_000u64),
        univ2::V2Fork::SushiSwap,
        "pair0.fee()",
        "ev:p0",
    )
    .unwrap();
    r.push(univ2::lp_fee_component(&c, &terms, &U256::from(10_000u64), "searcher").unwrap());
    r.absorb(
        ComponentSeed {
            kind: univ2::KIND_V2_LP_FEE.into(),
            scope: Scope::leg(1, "pair1", "evm:1"),
            asset: a.clone(),
            treatment: Treatment::Embedded,
            payer: "searcher".into(),
            source: "pair1.reserves()".into(),
            adapter_version: univ2::ADAPTER.into(),
        },
        Err(CostError::MissingRead {
            provider: "pair1".into(),
            read: "getReserves()".into(),
        }),
    );
    assert!(!r.is_net_closable());
    assert_eq!(r.blocking().len(), 1);
    assert!(r.external_usd().is_none());
    // La línea agregada NO inventa un importe: queda sin usd.
    let lines = r.to_cost_lines().unwrap();
    let line = lines
        .iter()
        .find(|l| l.kind == KIND_EXECUTION_FEES)
        .unwrap();
    assert!(line.usd.is_none());
    assert!(line
        .reason
        .as_deref()
        .unwrap()
        .contains("pending_resolution"));
}

// ═════════════════════════ CAMBIO DINÁMICO DE FEE ═════════════════════════

#[test]
fn curve_cryptoswap_fee_moves_between_mid_and_out() {
    let mid = U256::from(1_000_000u64); // 0.01% en 1e10
    let out = U256::from(50_000_000u64); // 0.5% en 1e10
    let gamma = U256::from(10_000_000_000_000_000u64); // 1e16
    let one = U256::from(1_000_000_000_000_000_000u64); // 1e18

    // Pool perfectamente balanceado (K = 1e18) → tarifa = mid_fee.
    let balanced = curve::cryptoswap_dynamic_fee(&mid, &out, &gamma, &one).unwrap();
    assert_eq!(balanced, bd("1000000"));

    // Pool totalmente desequilibrado (K = 0) → la tarifa sube hacia out_fee.
    let imbalanced = curve::cryptoswap_dynamic_fee(&mid, &out, &gamma, &U256::zero()).unwrap();
    assert!(
        imbalanced > bd("1000000"),
        "desequilibrio debe encarecer: {imbalanced}"
    );
    assert!(
        imbalanced < bd("50000000"),
        "y quedar por debajo de out_fee: {imbalanced}"
    );

    // Un mismo pool da dos tarifas distintas según el estado: NO es constante.
    assert_ne!(balanced, imbalanced);

    // fee_gamma = 0 no se puede reducir: error, no valor.
    assert!(matches!(
        curve::cryptoswap_dynamic_fee(&mid, &out, &U256::zero(), &one),
        Err(CostError::ZeroDenominator { .. })
    ));
}

#[test]
fn v4_fee_is_additive_unlike_v3() {
    // V4: el swapper paga lp + protocol + hook (aditivo).
    let key = univ4::PoolKey {
        currency0: "0xa".into(),
        currency1: "0xb".into(),
        fee_raw: 3_000,
        tick_spacing: 60,
        hooks: "0x0000000000000000000000000000000000000000".into(),
    };
    let inputs = univ4::V4FeeInputs {
        key: key.clone(),
        // carril zeroForOne = 5 → 5 × 100 = 500 pips (ProtocolFeeLibrary ×100)
        protocol_fee_packed: 5,
        dynamic_lp_fee_pips: None,
        hook_fee_pips: None,
        before_swap_rebate_raw: None,
        source: "poolmanager".into(),
        evidence_id: "ev:v4".into(),
    };
    let v4 = univ4::V4FeeTerms::resolve(&inputs, true).unwrap();
    assert_eq!(v4.lp_pips, 3_000);
    assert_eq!(v4.protocol_pips, 500);
    assert_eq!(v4.hook_pips, 0);
    assert_eq!(v4.total_pips(), 3_500, "V4 suma; no corta");
    // El carril del OTRO sentido es independiente: aquí vale 0.
    let other = univ4::V4FeeTerms::resolve(&inputs, false).unwrap();
    assert_eq!(other.protocol_pips, 0);
    assert_eq!(other.total_pips(), 3_000);

    // Un carril que excede el máximo documentado (1000 pips) es dato corrupto.
    let corrupt = univ4::V4FeeInputs {
        protocol_fee_packed: 200, // 200 × 100 = 20000 pips > 1000
        ..inputs.clone()
    };
    assert!(matches!(
        univ4::V4FeeTerms::resolve(&corrupt, true),
        Err(CostError::InvalidRead { .. })
    ));

    // La suma aditiva no puede pasar de 1e6 pips.
    let overflow = univ4::V4FeeInputs {
        key: univ4::PoolKey {
            fee_raw: 1_000_000,
            hooks: "0xhook".into(),
            ..key.clone()
        },
        hook_fee_pips: Some(200),
        protocol_fee_packed: 0,
        ..inputs.clone()
    };
    assert!(matches!(
        univ4::V4FeeTerms::resolve(&overflow, true),
        Err(CostError::InvalidRead { .. })
    ));

    // V3 con el MISMO fee tier y protocolFee 1/10: el protocolo sale DE los 3000.
    let v3 = univ3::V3FeeTerms::resolve(&univ3::V3FeeReads {
        fee_raw: U256::from(3_000u64),
        protocol_fee_num_raw: U256::from(1u64),
        protocol_fee_den_raw: U256::from(10u64),
        crosses_initialized_tick: true,
        source: "pool".into(),
        evidence_id: "ev:v3".into(),
    })
    .unwrap();
    assert_eq!(v3.fee_pips, 3_000);
    assert_eq!(v3.lp_pips + v3.protocol_pips, 3_000, "V3 reparte, no suma");

    // El hook sólo se cobra si hay hook; con hook desplegado sin lectura → pendiente.
    let hooked = univ4::V4FeeInputs {
        key: univ4::PoolKey {
            hooks: "0xhook".into(),
            ..key
        },
        ..inputs
    };
    match univ4::V4FeeTerms::resolve(&hooked, true) {
        Err(CostError::MissingRead { read, .. }) => {
            assert!(read.contains("getHookFee"), "read exacto: {read}")
        }
        other => panic!("esperaba MissingRead(getHookFee), obtuve {other:?}"),
    }
}

#[test]
fn curve_ramp_and_offpeg_move_the_fee() {
    assert_eq!(curve::ramped_a(100, 200, 1_000, 2_000, 500), Some(100));
    assert_eq!(curve::ramped_a(100, 200, 1_000, 2_000, 1_500), Some(150));
    assert_eq!(curve::ramped_a(100, 200, 1_000, 2_000, 2_500), Some(200));
    assert_eq!(curve::ramped_a(100, 200, 2_000, 2_000, 2_500), None);

    // offpeg multiplica la tarifa sólo cuando el pool está off-peg.
    let fee = U256::from(4_000_000u64);
    let mult = U256::from(20_000_000_000u64); // 2× en 1e10
    assert_eq!(curve::offpeg_fee_rate(&fee, &mult, false).unwrap(), fee);
    assert_eq!(
        curve::offpeg_fee_rate(&fee, &mult, true).unwrap(),
        U256::from(8_000_000u64)
    );
}

// ═════════════════════════ PRECISIÓN / OVERFLOW ═════════════════════════

#[test]
fn overflow_is_an_error_not_a_wrapped_number() {
    let max = U256::MAX;
    assert!(matches!(
        mul_checked(&max, &U256::from(2u64), "t"),
        Err(CostError::Overflow { .. })
    ));
    assert!(matches!(
        add_checked(&max, &U256::from(1u64), "t"),
        Err(CostError::Overflow { .. })
    ));
    // División por cero explícita.
    assert!(matches!(
        div_floor(&U256::from(1u64), &U256::zero(), "t"),
        Err(CostError::ZeroDenominator { .. })
    ));
    // El `getAmountOut` de V2 con reservas enormes no desborda en silencio.
    let terms = univ2::V2FeeTerms::from_read(
        U256::from(997u64),
        U256::from(1_000u64),
        univ2::V2Fork::UniswapV2,
        "pair.fee()",
        "ev",
    )
    .unwrap();
    let huge = U256::MAX / U256::from(2u64);
    assert!(matches!(
        univ2::amount_out(&terms, &huge, &huge, &huge),
        Err(CostError::Overflow { .. })
    ));
}

#[test]
fn monetary_amounts_never_pass_through_f64() {
    // 1 wei de un token de 18 decimales a 1 USD: el valor debe ser exacto y
    // diminuto, no 0 por redondeo de punto flotante.
    let p = price_ref(1, "0xweth", 18, "1.0", "r1");
    let v = p.value_min_units(&U256::from(1u64)).unwrap();
    assert_eq!(v, bd("0.000000000000000001"));
    // La aritmética decimal conserva 30 dígitos significativos.
    let p2 = price_ref(1, "0xweth", 18, "3456.789012345678901234", "r1");
    let v2 = p2
        .value_min_units(&U256::from(1_000_000_000_000_000_000u64))
        .unwrap();
    assert_eq!(v2, bd("3456.789012345678901234"));
}

// ═════════════════════════ ROUNDING ═════════════════════════

#[test]
fn rounding_is_floor_and_never_favours_the_net() {
    // V2: amount_in=1000, 997/1000 → input efectivo 997, comisión 3 exactos.
    let terms = univ2::V2FeeTerms::from_read(
        U256::from(997u64),
        U256::from(1_000u64),
        univ2::V2Fork::UniswapV2,
        "pair.fee()",
        "ev",
    )
    .unwrap();
    assert_eq!(
        univ2::fee_taken_raw(&terms, &U256::from(1_000u64)).unwrap(),
        U256::from(3u64)
    );
    // 999 × 997/1000 = 996.003 → floor 996 → comisión 3 (no 2.997 redondeado a 3
    // por casualidad: el input efectivo es 996).
    assert_eq!(
        univ2::fee_taken_raw(&terms, &U256::from(999u64)).unwrap(),
        U256::from(3u64)
    );
    assert_eq!(
        proportion_floor(
            &U256::from(999u64),
            &U256::from(997u64),
            &U256::from(1_000u64),
            "t"
        )
        .unwrap(),
        U256::from(996u64)
    );

    // Curve: el reparto admin/LP conserva el total exacto.
    let fee = U256::from(1_000u64);
    let admin_fee = U256::from(3_333_333_333u64);
    let (lp, admin) = curve::admin_split(&fee, &admin_fee).unwrap();
    assert_eq!(admin, U256::from(333u64));
    assert_eq!(lp, U256::from(667u64));
    assert_eq!(lp + admin, fee, "el reparto no crea ni pierde un mínimo");

    // floor_scale redondea hacia abajo.
    assert_eq!(floor_scale(&bd("1.9999999"), 4), bd("1.9999"));
    assert_eq!(floor_scale(&bd("-0.0001"), 2), bd("-0.01"));
}

#[test]
fn curve_fee_base_is_the_output_not_the_input() {
    let fee_rate = U256::from(4_000_000u64); // 0.04% en 1e10
    let gross_out = U256::from(1_000_000u64);
    let amount_in = U256::from(2_000_000u64);
    let on_output = curve::fee_on_output(&gross_out, &fee_rate).unwrap();
    let on_input = curve::fee_on_input_wrong_base(&amount_in, &fee_rate).unwrap();
    assert_eq!(on_output, U256::from(400u64));
    assert_eq!(on_input, U256::from(800u64));
    // Bases distintas ⇒ resultados distintos. Si el motor usara la base de
    // Uniswap, cobraría el doble.
    assert_ne!(on_output, on_input);
}

// ═════════════════════════ DISTINTOS DECIMALS ═════════════════════════

#[test]
fn token_decimals_change_the_fee_valuation() {
    let amount = U256::from(1_000_000u64); // 1 USDC (6) o 1e-12 WETH (18)
    let usdc = price_ref(1, "0xusdc", 6, "1.0", "r1");
    let weth = price_ref(1, "0xweth", 18, "3000.0", "r1");
    assert_eq!(usdc.value_min_units(&amount).unwrap(), bd("1.0"));
    assert_eq!(weth.value_min_units(&amount).unwrap(), bd("0.000000003000"));

    // Un precio con decimals DISTINTOS a los del activo es procedencia inválida.
    let wrong = price_ref(1, "0xusdc", 18, "1.0", "r1");
    let err = wrong
        .check(1_500, "r1", &asset(1, "0xusdc", 6))
        .unwrap_err();
    assert!(matches!(err, CostError::PriceProvenance { .. }));
    assert_eq!(err.state(), CostState::Absent);
}

// ═════════════════════════ PRECIO STALE ═════════════════════════

#[test]
fn stale_price_blocks_valuation_with_an_explicit_reason() {
    let p = price_ref(1, "0xusdc", 6, "1.0", "r1"); // valid_until_ms = 2_000
    let err = p.check(2_001, "r1", &asset(1, "0xusdc", 6)).unwrap_err();
    match &err {
        CostError::StalePrice {
            observed_at_ms,
            valid_until_ms,
            now_ms,
            ..
        } => {
            assert_eq!(
                (*observed_at_ms, *valid_until_ms, *now_ms),
                (1_000, 2_000, 2_001)
            );
        }
        other => panic!("esperaba StalePrice, obtuve {other:?}"),
    }
    assert_eq!(err.state(), CostState::Absent);
    let task = err.task(univ2::KIND_V2_LP_FEE, &route_scope());
    assert!(task.read.contains("refresh_price"));
}

#[test]
fn unanchored_evm_read_is_not_a_valid_cost_source() {
    let a = asset(1, "0xusdc", 6);
    let p = price_ref(1, "0xusdc", 6, "1.0", "r1");
    // observed_at_ms sólo NO es un ancla (§13).
    let weak = VenueAnchor {
        chain_id: 1,
        block_number: None,
        block_hash: None,
        observed_at_ms: 1_500,
        venue_time_ms: None,
    };
    let c = ctx(Scope::leg(0, "pair", "evm:1"), &a, &p, "r1", 1_500, &weak);
    match c.check_price() {
        Err(CostError::MissingAnchor { read }) => assert!(read.contains("anchor")),
        other => panic!("esperaba MissingAnchor, obtuve {other:?}"),
    }
}

// ═════════════════════════ PROCEDENCIA INCORRECTA ═════════════════════════

#[test]
fn wrong_provenance_is_rejected() {
    let a = asset(1, "0xusdc", 6);

    // Sin productor.
    let mut no_producer = price_ref(1, "0xusdc", 6, "1.0", "r1");
    no_producer.producer = "  ".into();
    assert!(matches!(
        no_producer.check(1_500, "r1", &a),
        Err(CostError::PriceProvenance { .. })
    ));

    // Sin evidence_id.
    let mut no_evidence = price_ref(1, "0xusdc", 6, "1.0", "r1");
    no_evidence.evidence_id = String::new();
    assert!(matches!(
        no_evidence.check(1_500, "r1", &a),
        Err(CostError::PriceProvenance { .. })
    ));

    // Revisión que no es la del plan.
    assert!(matches!(
        price_ref(1, "0xusdc", 6, "1.0", "r-old").check(1_500, "r-new", &a),
        Err(CostError::PriceRevisionMismatch { .. })
    ));

    // Activo/cadena distintos.
    assert!(matches!(
        price_ref(137, "0xusdc", 6, "1.0", "r1").check(1_500, "r1", &a),
        Err(CostError::PriceProvenance { .. })
    ));

    // Precio no positivo.
    assert!(matches!(
        price_ref(1, "0xusdc", 6, "0", "r1").check(1_500, "r1", &a),
        Err(CostError::InvalidRead { .. })
    ));

    // Un fee leído sin procedencia tampoco entra.
    assert!(matches!(
        univ2::V2FeeTerms::from_read(
            U256::from(997u64),
            U256::from(1_000u64),
            univ2::V2Fork::UniswapV2,
            "",
            "ev"
        ),
        Err(CostError::InvalidRead { .. })
    ));
}

// ═════════════════════════ NO DOBLE CONTEO ═════════════════════════

#[test]
fn priority_fee_is_inside_gas_and_never_a_second_line() {
    let market = gas::FeeMarket {
        base_fee_per_gas: U256::from(10u64),
        max_priority_fee_per_gas: Some(U256::from(2u64)),
        max_fee_per_gas: Some(U256::from(15u64)),
        legacy_gas_price: None,
    };
    // min(maxFee=15, base+tip=12) = 12 → el tip ya está dentro.
    assert_eq!(
        gas::effective_gas_price(&market).unwrap(),
        U256::from(12u64)
    );
    let capped = gas::FeeMarket {
        max_fee_per_gas: Some(U256::from(11u64)),
        ..market
    };
    assert_eq!(
        gas::effective_gas_price(&capped).unwrap(),
        U256::from(11u64)
    );
    // Sin mercado de fees no se asume un precio.
    let empty = gas::FeeMarket {
        base_fee_per_gas: U256::zero(),
        max_priority_fee_per_gas: None,
        max_fee_per_gas: None,
        legacy_gas_price: None,
    };
    assert!(matches!(
        gas::effective_gas_price(&empty),
        Err(CostError::MissingRead { .. })
    ));
}

#[test]
fn intrinsics_and_l2_and_blobs_do_not_double_count() {
    let payload = gas::Payload {
        calldata: vec![0, 0, 1, 2],
        is_create: false,
        access_list: vec![("0xaa".into(), vec![U256::from(1u64), U256::from(2u64)])],
    };
    assert_eq!(payload.zero_bytes(), 2);
    assert_eq!(payload.nonzero_bytes(), 2);
    assert_eq!(payload.calldata_gas(), 2 * 4 + 2 * 16);
    assert_eq!(payload.access_list_gas(), 2_400 + 2 * 1_900);
    // intrinsic = 21000 + 40 + 6200
    assert_eq!(gas::intrinsic_gas(&payload).unwrap(), 27_240);
    assert_eq!(
        gas::estimate_from_payload(&payload, 100_000).unwrap(),
        127_240
    );
    // Creación de contrato: +32000.
    let create = gas::Payload {
        is_create: true,
        ..payload.clone()
    };
    assert_eq!(gas::intrinsic_gas(&create).unwrap(), 27_240 + 32_000);

    // OP-stack pre-Ecotone: l1BaseFee × (txDataGas + overhead) / scalar.
    let inputs = gas::GasInputs {
        chain_id: 10,
        payload: payload.clone(),
        execution_gas: 100_000,
        market: gas::FeeMarket {
            base_fee_per_gas: U256::from(1_000_000u64),
            max_priority_fee_per_gas: Some(U256::from(100u64)),
            max_fee_per_gas: Some(U256::from(2_000_000u64)),
            legacy_gas_price: None,
        },
        l2_data_fee: gas::L2DataFee::OpStackPreEcotone {
            l1_base_fee: U256::from(20_000_000_000u64),
            overhead: U256::from(188u64),
            scalar: U256::from(1u64),
        },
        tx_data_gas: U256::from(1_000u64),
        blobs: None,
        source: "op".into(),
        evidence_id: "ev:gas".into(),
    };
    let cost = gas::cost_from_payload(&inputs).unwrap();
    let l1 = (1_000u64 + 188) * 20_000_000_000u64;
    assert_eq!(cost.l1_data_wei, Some(U256::from(l1)));
    assert_eq!(cost.blob_wei, None, "sin blobs no hay comisión de blob");
    // total = gas_units × precio_efectivo + coste L1
    assert_eq!(
        cost.total_wei,
        cost.effective_gas_price * U256::from(127_240u64) + U256::from(l1)
    );

    // Ecotone: el L1 fee del nodo YA incluye blobs → no se suma otra vez.
    let ecotone = gas::GasInputs {
        l2_data_fee: gas::L2DataFee::OpStackEcotoneRead {
            l1_fee_wei: U256::from(7_777u64),
            includes_blob_gas: true,
        },
        blobs: Some((U256::from(1_000u64), U256::from(131_072u64))),
        ..inputs.clone()
    };
    let e = gas::cost_from_payload(&ecotone).unwrap();
    assert_eq!(e.blob_wei, None, "blob ya dentro del L1 fee");
    assert_eq!(e.l1_data_wei, Some(U256::from(7_777u64)));
    assert_eq!(e.total_wei, e.execution_wei + U256::from(7_777u64));

    // Pero si el L1 fee NO trae blobs, la comisión se cobra aparte — una vez.
    let with_blobs = gas::GasInputs {
        l2_data_fee: gas::L2DataFee::NotApplicable {
            reason: "fixture".into(),
        },
        blobs: Some((U256::from(1_000u64), U256::from(131_072u64))),
        ..inputs.clone()
    };
    let w = gas::cost_from_payload(&with_blobs).unwrap();
    assert_eq!(w.blob_wei, Some(U256::from(131_072_000u64)));
    assert_eq!(w.total_wei, w.execution_wei + U256::from(131_072_000u64));

    // Conciliación: la varianza tiene SIGNO y no es un coste extra.
    let actual = gas::cost_from_receipt(&inputs, 130_000).unwrap();
    let rec = gas::GasReconciliation::new(&cost, Some(&actual));
    assert_eq!(rec.estimated_gas_units, 127_240);
    assert_eq!(rec.actual_gas_units, Some(130_000));
    assert!(!rec.variance.as_ref().unwrap().negative);
    assert!(!rec.variance.as_ref().unwrap().is_zero());
}

#[test]
fn builder_payment_via_priority_fee_is_embedded_not_external() {
    use builder::{BuilderBid, BuilderPaymentMode, KIND_BUILDER_BID};

    // Vía priority fee → EMBEDDED (ya dentro de `gas`).
    let via_tip = BuilderBid {
        relay: "flashbots".into(),
        mode: BuilderPaymentMode::PriorityFeeTopUp,
        amount_wei: U256::from(1_000_000_000_000_000u64),
        source: "eth_sendBundle".into(),
        evidence_id: "ev:bid".into(),
    };
    assert_eq!(via_tip.mode.treatment(), Treatment::Embedded);

    // Transferencia explícita → EXTERNAL (coste adicional real).
    let explicit = BuilderBid {
        mode: BuilderPaymentMode::SeparateCoinbaseTransfer,
        ..via_tip.clone()
    };
    assert_eq!(explicit.mode.treatment(), Treatment::External);

    // Sin pago propuesto → not_applicable con fundamento.
    let none = BuilderBid {
        mode: BuilderPaymentMode::None,
        amount_wei: U256::zero(),
        ..via_tip.clone()
    };
    assert_eq!(none.mode.treatment(), Treatment::NotApplicable);

    // Un bid sin evidencia no es un pago.
    let naked = BuilderBid {
        evidence_id: String::new(),
        ..via_tip.clone()
    };
    assert!(matches!(
        naked.validate(),
        Err(CostError::InvalidRead { .. })
    ));

    // Auditoría: declarar external un pago hecho por priority fee es doble conteo.
    let a = AssetRef::native(1, 18);
    let p = price_ref(1, "native", 18, "3000.0", "r1");
    let an = anchor(1);
    let c = ctx(route_scope(), &a, &p, "r1", 1_500, &an);
    let mut r = CostResolution::new(1, "snap", "r1", "before_financing", "atomic_quote");
    // gas (external) + bid declarado external pero embebido → la auditoría lo ve.
    let mut rogue = builder::bid_component(&c, &via_tip).unwrap();
    rogue.treatment = Treatment::External;
    rogue.embedded_in_quote = true;
    r.components.push(rogue); // push directo: se prueba la auditoría, no `validate`
    let findings = r.double_count_audit();
    assert!(
        findings
            .iter()
            .any(|f| f.code == "builder_paid_via_priority_fee_counted_twice"),
        "hallazgos: {findings:?}"
    );
}

#[test]
fn embedded_execution_fees_are_declared_but_not_subtracted() {
    let a = asset(1, "0xweth", 18);
    let p = price_ref(1, "0xweth", 18, "3000.0", "r1");
    let an = anchor(1);
    let c = ctx(Scope::leg(0, "pair", "evm:1"), &a, &p, "r1", 1_500, &an);
    let terms = univ2::V2FeeTerms::from_read(
        U256::from(997u64),
        U256::from(1_000u64),
        univ2::V2Fork::UniswapV2,
        "pair.fee()",
        "ev",
    )
    .unwrap();
    // 0.01 WETH de comisión a 3000 USD/WETH. amount_in = 0.01 WETH (1e16 wei):
    // comisión = 1e16 × 3/1000 = 3e13 wei = 0.00003 WETH → 0.09 USD.
    let comp =
        univ2::lp_fee_component(&c, &terms, &U256::from(10_000_000_000_000_000u64), "s").unwrap();
    assert_eq!(comp.amount_raw, U256::from(30_000_000_000_000u64));
    assert_eq!(comp.usd.as_ref().unwrap(), &bd("0.09"));

    let mut r = CostResolution::new(1, "snap", "r1", "before_financing", "atomic_quote");
    r.push(comp);
    // Está valorado y declarado, pero NO es un coste externo.
    assert_eq!(r.embedded_fee_usd(), Some(bd("0.09")));
    assert_eq!(r.external_usd(), Some(bd("0")));
    let line = r
        .to_cost_lines()
        .unwrap()
        .into_iter()
        .find(|l| l.kind == KIND_EXECUTION_FEES)
        .unwrap();
    assert_eq!(line.treatment, "embedded");
    assert!(line
        .reason
        .as_deref()
        .unwrap()
        .contains("incluido; no se descuenta otra vez"));

    // Declarar esa comisión como external en un quote atómico es doble conteo.
    let mut rogue = r.clone();
    rogue.components[0].treatment = Treatment::External;
    rogue.components[0].embedded_in_quote = false;
    let findings = rogue.double_count_audit();
    assert!(findings
        .iter()
        .any(|f| f.code == "execution_fee_external_on_atomic_quote"));
}

#[test]
fn v3_protocol_fee_external_is_flagged_as_double_count() {
    let a = asset(1, "0xusdc", 6);
    let p = price_ref(1, "0xusdc", 6, "1.0", "r1");
    let an = anchor(1);
    let c = ctx(Scope::leg(0, "poolv3", "evm:1"), &a, &p, "r1", 1_500, &an);
    let terms = univ3::V3FeeTerms::resolve(&univ3::V3FeeReads {
        fee_raw: U256::from(500u64),
        protocol_fee_num_raw: U256::from(1u64),
        protocol_fee_den_raw: U256::from(4u64),
        crosses_initialized_tick: true,
        source: "poolv3".into(),
        evidence_id: "ev:v3".into(),
    })
    .unwrap();
    let mut comp =
        univ3::protocol_fee_component(&c, &terms, &U256::from(1_000_000u64), "s").unwrap();
    assert_eq!(
        comp.treatment,
        Treatment::Embedded,
        "por defecto es un corte"
    );
    let mut r = CostResolution::new(1, "snap", "r1", "before_financing", "atomic_quote");
    r.push(comp.clone());
    assert!(r.double_count_audit().is_empty());
    // Forzarlo a external produce el hallazgo.
    comp.treatment = Treatment::External;
    comp.embedded_in_quote = false;
    let mut rogue = CostResolution::new(1, "snap", "r1", "before_financing", "atomic_quote");
    rogue.push(comp);
    assert!(rogue
        .double_count_audit()
        .iter()
        .any(|f| f.code == "v3_protocol_fee_is_a_cut_of_the_lp_fee"));
}

#[test]
fn financing_external_in_retained_spread_is_flagged() {
    let a = asset(1, "0xusdc", 6);
    let p = price_ref(1, "0xusdc", 6, "1.0", "r1");
    let an = anchor(1);
    let c = ctx(route_scope(), &a, &p, "r1", 1_500, &an);
    let reads = funding::ProviderReads {
        provider: "aave_v3".into(),
        pool: "0xpool".into(),
        chain_id: 1,
        method: funding::FundingMethod::AaveV3FlashLoan,
        premium_total_bps: Some(U256::from(7u64)),
        premium_to_protocol_bps: Some(U256::from(1_000u64)),
        capacity_raw: Some(U256::from(10_000_000u64)),
        source: "evm".into(),
        evidence_id: "ev:aave".into(),
    };
    let q = funding::resolve(&reads, &a, &U256::from(1_000_000u64)).unwrap();
    assert_eq!(q.premium_raw, U256::from(700u64));
    assert_eq!(q.repayment_raw, U256::from(1_000_700u64));
    assert_eq!(q.protocol_share_raw, Some(U256::from(70u64)));
    let comp = funding::financing_component(&c, &q, "searcher").unwrap();
    assert_eq!(comp.treatment, Treatment::External);

    // Con `profit_basis = retained_after_repayment` ese externo es doble cobro.
    let mut r = CostResolution::new(1, "snap", "r1", "retained_after_repayment", "atomic_quote");
    r.push(comp);
    assert!(r
        .double_count_audit()
        .iter()
        .any(|f| f.code == "financing_inside_retained_spread"));
    // Y con el basis correcto no hay hallazgo.
    let mut ok = CostResolution::new(1, "snap", "r1", "before_financing", "atomic_quote");
    ok.push(funding::financing_component(&c, &q, "searcher").unwrap());
    assert!(ok.double_count_audit().is_empty());
}

#[test]
fn own_capital_is_not_applicable_with_grounding_and_flash_zero_needs_attestation() {
    let a = asset(1, "0xusdc", 6);
    let p = price_ref(1, "0xusdc", 6, "1.0", "r1");
    let an = anchor(1);
    let c = ctx(route_scope(), &a, &p, "r1", 1_500, &an);

    let own = funding::ProviderReads {
        provider: "own".into(),
        pool: "own".into(),
        chain_id: 1,
        method: funding::FundingMethod::OwnCapital,
        premium_total_bps: None,
        premium_to_protocol_bps: None,
        capacity_raw: Some(U256::from(5_000u64)),
        source: "config".into(),
        evidence_id: "ev:own".into(),
    };
    let q = funding::resolve(&own, &a, &U256::from(1_000u64)).unwrap();
    let comp = funding::financing_component(&c, &q, "searcher").unwrap();
    assert_eq!(comp.state, CostState::NotApplicable);
    assert_eq!(comp.treatment, Treatment::NotApplicable);
    assert!(comp.usd.is_none());
    assert!(comp.note.as_deref().unwrap().contains("capital propio"));

    // Un flash con 0 bps SIN acreditación → no es un cero, es una lectura.
    let unattested = funding::ProviderReads {
        method: funding::FundingMethod::Erc3156FlashLoan,
        premium_total_bps: Some(U256::zero()),
        source: String::new(), // ← sin procedencia
        ..own.clone()
    };
    let err = funding::resolve(&unattested, &a, &U256::from(1_000u64)).unwrap_err();
    assert!(matches!(err, CostError::InvalidRead { .. }));

    // Un flash con lectura de 0 bps CON acreditación → cero conservado.
    let attested = funding::ProviderReads {
        method: funding::FundingMethod::AaveV3FlashLoanSimple,
        premium_total_bps: Some(U256::zero()),
        premium_to_protocol_bps: Some(U256::zero()),
        source: "evm".into(),
        evidence_id: "ev:aave:zero".into(),
        ..own.clone()
    };
    let q0 = funding::resolve(&attested, &a, &U256::from(1_000u64)).unwrap();
    assert!(q0.zero_attested);
    let comp0 = funding::financing_component(&c, &q0, "searcher").unwrap();
    assert_eq!(comp0.state, CostState::ZeroAttested);
    assert_eq!(comp0.usd, Some(bd("0")));
}

#[test]
fn a_zero_premium_is_preserved_only_with_provenance() {
    let a = asset(1, "0xusdc", 6);

    // (1) Lectura = 0 CON procedencia completa → cero acreditado y conservado.
    let attested = funding::ProviderReads {
        provider: "aave_v3".into(),
        pool: "0xpool".into(),
        chain_id: 1,
        method: funding::FundingMethod::AaveV3FlashLoan,
        premium_total_bps: Some(U256::zero()),
        premium_to_protocol_bps: Some(U256::zero()),
        capacity_raw: Some(U256::from(10_000u64)),
        source: "eth_call".into(),
        evidence_id: "ev:aave:FLASHLOAN_PREMIUM_TOTAL".into(),
    };
    let q = funding::resolve(&attested, &a, &U256::from(1_000u64)).unwrap();
    assert!(q.zero_attested);
    let p = price_ref(1, "0xusdc", 6, "1.0", "r1");
    let an = anchor(1);
    let c = ctx(route_scope(), &a, &p, "r1", 1_500, &an);
    let comp = funding::financing_component(&c, &q, "searcher").unwrap();
    assert_eq!(comp.state, CostState::ZeroAttested);
    assert_eq!(comp.usd, Some(bd("0")));

    // (2) Lectura = 0 SIN procedencia → no llega ni a ser un cero.
    let naked = funding::ProviderReads {
        source: "  ".into(),
        ..attested.clone()
    };
    assert!(matches!(
        funding::resolve(&naked, &a, &U256::from(1_000u64)),
        Err(CostError::InvalidRead { .. })
    ));

    // (3) Lectura AUSENTE → pendiente de resolución, jamás cero.
    let absent = funding::ProviderReads {
        premium_total_bps: None,
        ..attested.clone()
    };
    let err = funding::resolve(&absent, &a, &U256::from(1_000u64)).unwrap_err();
    assert!(matches!(err, CostError::MissingRead { .. }));
    assert_eq!(err.state(), CostState::PendingResolution);

    // (4) Método que no expresa su fee en bps (p. ej. vault 0-fee): sólo un
    //     CERO ACREDITADO lo resuelve; con fee distinto de cero → no soportado.
    let vault_zero = funding::ProviderReads {
        provider: "balancer_v2".into(),
        pool: "0xvault".into(),
        method: funding::FundingMethod::BalancerV2FlashLoan,
        premium_total_bps: Some(U256::zero()),
        premium_to_protocol_bps: None,
        capacity_raw: Some(U256::from(10_000u64)),
        source: "vault".into(),
        evidence_id: "ev:vault:no_fee".into(),
        ..attested.clone()
    };
    let qz = funding::resolve(&vault_zero, &a, &U256::from(1_000u64)).unwrap();
    assert_eq!(qz.premium_raw, U256::zero());
    assert!(qz.zero_attested);

    let vault_nonzero = funding::ProviderReads {
        premium_total_bps: Some(U256::from(3u64)),
        ..vault_zero.clone()
    };
    assert!(matches!(
        funding::resolve(&vault_nonzero, &a, &U256::from(1_000u64)),
        Err(CostError::UnsupportedInvariant { .. })
    ));
}

#[test]
fn capacity_and_comparison_do_not_fabricate_a_cheaper_flash() {
    let a = asset(1, "0xusdc", 6);
    let principal = U256::from(1_000_000u64);
    let small = funding::ProviderReads {
        provider: "aave_v3".into(),
        pool: "0xpool".into(),
        chain_id: 1,
        method: funding::FundingMethod::AaveV3FlashLoan,
        premium_total_bps: Some(U256::from(5u64)),
        premium_to_protocol_bps: None,
        capacity_raw: Some(U256::from(100u64)), // ← capacidad insuficiente
        source: "evm".into(),
        evidence_id: "ev".into(),
    };
    let err = funding::resolve(&small, &a, &principal).unwrap_err();
    assert!(err.reason().contains("capacity_insufficient"));

    let big = funding::ProviderReads {
        capacity_raw: Some(U256::from(10_000_000u64)),
        ..small.clone()
    };
    let q_big = funding::resolve(&big, &a, &principal).unwrap();
    let cheap = funding::ProviderReads {
        premium_total_bps: Some(U256::from(1u64)),
        ..big.clone()
    };
    let q_cheap = funding::resolve(&cheap, &a, &principal).unwrap();

    let cands = funding::candidates(&[q_big.clone(), q_cheap.clone()], &U256::from(50u64));
    let best = funding::cheapest_admissible(&cands).unwrap();
    // El capital propio (50) NO alcanza el principal: no puede ganar por barato.
    assert_eq!(best.provider, "aave_v3");
    assert_eq!(best.premium_raw, U256::from(100u64));
    let own = cands
        .iter()
        .find(|c| c.method == funding::FundingMethod::OwnCapital)
        .unwrap();
    assert!(!own.admissible);
    assert_eq!(own.reason.as_deref(), Some("own_capital_insufficient"));
}

#[test]
fn rebates_are_signed_flows_not_negative_costs() {
    let a = AssetRef::native(1, 18);
    let p = price_ref(1, "native", 18, "3000.0", "r1");
    let an = anchor(1);
    let c = ctx(route_scope(), &a, &p, "r1", 1_500, &an);
    let rebate = builder::rebate_component(
        &c,
        &U256::from(1_000_000_000_000_000u64),
        "mev_share",
        "ev:rebate",
    )
    .unwrap();
    assert_eq!(rebate.direction, Direction::Rebate);
    let mut r = CostResolution::new(1, "snap", "r1", "before_financing", "atomic_quote");
    r.push(rebate);
    assert_eq!(r.rebates.len(), 1);
    assert_eq!(r.rebates[0].usd, bd("-3.000000000000000000"));
    // El CostLine nunca lleva un importe negativo.
    for line in r.to_cost_lines().unwrap() {
        if let Some(u) = line.usd {
            assert!(!u.starts_with('-'), "CostLine no admite negativos: {u}");
        }
    }
}

#[test]
fn a_hook_rebate_keeps_its_sign_in_the_resolution() {
    let a = asset(1, "0xusdc", 6);
    let p = price_ref(1, "0xusdc", 6, "1.0", "r1");
    let an = anchor(1);
    let c = ctx(Scope::leg(0, "v4pool", "evm:1"), &a, &p, "r1", 1_500, &an);
    let reb = univ4::hook_rebate_component(
        &c,
        &U256::from(5_000u64),
        "hook.beforeSwap",
        "ev:hook:delta",
    )
    .unwrap();
    assert_eq!(reb.direction, Direction::Rebate);
    assert_eq!(reb.usd.as_ref().unwrap(), &bd("0.005"));
    let mut r = CostResolution::new(1, "snap", "r1", "before_financing", "atomic_quote");
    r.push(reb);
    assert_eq!(r.rebates[0].usd, bd("-0.005"));
}

// ═════════════════════════ CONTRATO DEL BRIDGE ═════════════════════════

#[test]
fn components_aggregate_into_one_line_per_category() {
    let a = asset(1, "0xusdc", 6);
    let p = price_ref(1, "0xusdc", 6, "1.0", "r1");
    let an = anchor(1);
    let terms = univ2::V2FeeTerms::from_read(
        U256::from(997u64),
        U256::from(1_000u64),
        univ2::V2Fork::UniswapV2,
        "pair.fee()",
        "ev",
    )
    .unwrap();
    let mut r = CostResolution::new(1, "snap", "r1", "before_financing", "atomic_quote");
    // Dos piernas del MISMO tipo → dos componentes, UNA línea.
    for leg in 0..2 {
        let c = ctx(
            Scope::leg(leg, &format!("pair{leg}"), "evm:1"),
            &a,
            &p,
            "r1",
            1_500,
            &an,
        );
        r.push(univ2::lp_fee_component(&c, &terms, &U256::from(1_000_000u64), "s").unwrap());
    }
    assert_eq!(r.components.len(), 2);
    let lines = r.to_cost_lines().unwrap();
    assert_eq!(lines.len(), 1, "una sola línea por categoría");
    assert_eq!(lines[0].kind, KIND_EXECUTION_FEES);
    // 1 USDC por pierna: comisión = 1e6 × 3/1000 = 3000 raw = 0.003 USD.
    assert_eq!(lines[0].usd.as_deref(), Some("0.006"), "2 × 0.003");
    // Sin duplicado de categoría (el bridge lo rechazaría).
    let mut kinds: Vec<&str> = lines.iter().map(|l| l.kind.as_str()).collect();
    kinds.sort_unstable();
    let before = kinds.len();
    kinds.dedup();
    assert_eq!(before, kinds.len(), "duplicate_cost_kind");

    // Un V3 embebido + un V2 embebido siguen siendo UNA categoría.
    let c3 = ctx(Scope::leg(2, "poolv3", "evm:1"), &a, &p, "r1", 1_500, &an);
    let t3 = univ3::V3FeeTerms::resolve(&univ3::V3FeeReads {
        fee_raw: U256::from(100u64),
        protocol_fee_num_raw: U256::zero(),
        protocol_fee_den_raw: U256::from(100u64),
        crosses_initialized_tick: true,
        source: "poolv3".into(),
        evidence_id: "ev:v3".into(),
    })
    .unwrap();
    r.push(univ3::lp_fee_component(&c3, &t3, &U256::from(1_000_000u64), "s").unwrap());
    let lines = r.to_cost_lines().unwrap();
    assert_eq!(lines.len(), 1);
    // + 100 pips de 1e6 sobre 1 USDC = 100 raw = 0.0001 USD.
    assert_eq!(lines[0].usd.as_deref(), Some("0.0061"), "0.006 + 0.0001");
    assert!(lines[0].reason.as_deref().unwrap().contains("v2_lp_fee"));
    assert!(lines[0].reason.as_deref().unwrap().contains("v3_lp_fee"));
}

#[test]
fn mixed_treatments_in_one_category_are_a_contract_conflict() {
    let a = asset(1, "0xusdc", 6);
    let p = price_ref(1, "0xusdc", 6, "1.0", "r1");
    let an = anchor(1);
    let terms = univ2::V2FeeTerms::from_read(
        U256::from(997u64),
        U256::from(1_000u64),
        univ2::V2Fork::UniswapV2,
        "pair.fee()",
        "ev",
    )
    .unwrap();
    let c = ctx(Scope::leg(0, "pair0", "evm:1"), &a, &p, "r1", 1_500, &an);
    let mut embedded = univ2::lp_fee_component(&c, &terms, &U256::from(1_000_000u64), "s").unwrap();
    let mut external = embedded.clone();
    external.treatment = Treatment::External;
    external.embedded_in_quote = false;
    let mut r = CostResolution::new(1, "snap", "r1", "before_financing", "atomic_quote");
    r.push(embedded.clone());
    r.push(external);
    assert!(matches!(
        r.to_cost_lines(),
        Err(CostError::MixedTreatment { .. })
    ));
    embedded.treatment = Treatment::Embedded; // silencio de tipos: el valor sigue
    assert_eq!(embedded.treatment.as_str(), "embedded");
}

#[test]
fn not_applicable_without_grounding_is_a_construction_error() {
    let bad = CostComponent {
        kind: funding::KIND_FINANCING_PREMIUM.into(),
        scope: route_scope(),
        asset: AssetRef::native(1, 18),
        amount_raw: U256::zero(),
        unit: Unit::MinUnits,
        direction: Direction::Charge,
        usd: None,
        payer: "searcher".into(),
        beneficiary: None,
        source: "config".into(),
        adapter_version: "v/1".into(),
        anchor: Some(anchor(1)),
        evidence_id: "ev".into(),
        state: CostState::NotApplicable,
        treatment: Treatment::NotApplicable,
        embedded_in_quote: false,
        note: Some("   ".into()), // ← vacío: sin fundamento
    };
    assert!(matches!(
        bad.validate(),
        Err(CostError::NotApplicableNeedsGrounding { .. })
    ));

    // Y un estado que bloquea no puede llevar importe (sería un cero disfrazado).
    let disguised = CostComponent {
        state: CostState::PendingResolution,
        usd: Some(bd("0")),
        ..bad.clone()
    };
    assert!(matches!(
        disguised.validate(),
        Err(CostError::InvalidRead { .. })
    ));

    // Un kind sin categoría contable no entra al contrato.
    let unmapped = CostComponent {
        kind: "mystery_fee".into(),
        state: CostState::Resolved,
        treatment: Treatment::External,
        usd: Some(bd("1")),
        note: Some("x".into()),
        ..bad
    };
    assert!(matches!(
        unmapped.validate(),
        Err(CostError::UnmappedKind { .. })
    ));
}

#[test]
fn emitted_lines_satisfy_the_bridge_contract_shape() {
    let a = asset(1, "0xusdc", 6);
    let p = price_ref(1, "0xusdc", 6, "1.0", "r1");
    let an = anchor(1);
    let terms = univ2::V2FeeTerms::from_read(
        U256::from(997u64),
        U256::from(1_000u64),
        univ2::V2Fork::UniswapV2,
        "pair.fee()",
        "ev",
    )
    .unwrap();
    let c = ctx(Scope::leg(0, "pair0", "evm:1"), &a, &p, "r1", 1_500, &an);
    let mut r = CostResolution::new(1, "snap", "r1", "before_financing", "atomic_quote");
    r.push(univ2::lp_fee_component(&c, &terms, &U256::from(1_000_000u64), "s").unwrap());
    r.absorb(
        ComponentSeed {
            kind: funding::KIND_FINANCING_PREMIUM.into(),
            scope: route_scope(),
            asset: a.clone(),
            treatment: Treatment::External,
            payer: "searcher".into(),
            source: "aave".into(),
            adapter_version: funding::ADAPTER.into(),
        },
        Err(CostError::MissingRead {
            provider: "aave_v3".into(),
            read: "0xpool.FLASHLOAN_PREMIUM_TOTAL()".into(),
        }),
    );
    for line in r.to_cost_lines().unwrap() {
        assert!(!line.evidence_id.is_empty(), "evidence_id obligatorio");
        assert!(
            ["external", "embedded", "not_applicable"].contains(&line.treatment.as_str()),
            "tratamiento fuera del vocabulario: {}",
            line.treatment
        );
        if line.treatment == "not_applicable" {
            assert!(line.usd.is_none());
            assert!(line.reason.as_deref().is_some_and(|s| !s.is_empty()));
        }
        if let Some(u) = &line.usd {
            assert!(
                BigDecimal::from_str(u).is_ok(),
                "usd debe ser un decimal plano parseable: {u}"
            );
        }
    }
}

#[test]
fn bridge_kind_covers_every_emitted_granular_kind() {
    for k in [
        gas::KIND_GAS_EXECUTION,
        gas::KIND_L2_DATA_FEE,
        gas::KIND_BLOB_FEE,
        funding::KIND_FINANCING_PREMIUM,
        univ2::KIND_V2_LP_FEE,
        univ2::KIND_V2_FLASH_PREMIUM,
        univ3::KIND_V3_LP_FEE,
        univ3::KIND_V3_PROTOCOL_FEE,
        univ4::KIND_V4_LP_FEE,
        univ4::KIND_V4_PROTOCOL_FEE,
        univ4::KIND_V4_HOOK_FEE,
        univ4::KIND_V4_HOOK_REBATE,
        curve::KIND_CURVE_LP_FEE,
        curve::KIND_CURVE_ADMIN_FEE,
        builder::KIND_BUILDER_BID,
        builder::KIND_BUILDER_REBATE,
    ] {
        assert!(
            bridge_kind(k).is_some(),
            "kind sin categoría de bridge: {k}"
        );
    }
    assert!(bridge_kind("invented_fee").is_none());
    // Los tres kinds que el bridge EXIGE para `atomic_quote` están cubiertos.
    let covered = [
        univ2::KIND_V2_LP_FEE,
        funding::KIND_FINANCING_PREMIUM,
        gas::KIND_GAS_EXECUTION,
    ];
    let cats: Vec<&str> = covered.iter().filter_map(|k| bridge_kind(k)).collect();
    for required in [KIND_GAS, KIND_FINANCING, KIND_EXECUTION_FEES] {
        assert!(
            cats.contains(&required),
            "categoría exigida ausente: {required}"
        );
    }
}

#[test]
fn v2_denominator_changes_the_output_so_forks_are_not_interchangeable() {
    let uni = univ2::V2FeeTerms::from_read(
        U256::from(997u64),
        U256::from(1_000u64),
        univ2::V2Fork::UniswapV2,
        "pair.fee()",
        "ev",
    )
    .unwrap();
    let cake = univ2::V2FeeTerms::from_read(
        U256::from(9_975u64),
        U256::from(10_000u64),
        univ2::V2Fork::PancakeSwapV2,
        "pair.fee()",
        "ev",
    )
    .unwrap();
    let amount_in = U256::from(1_000u64);
    let reserve_in = U256::from(100_000u64);
    let reserve_out = U256::from(200_000u64);
    let a = univ2::amount_out(&uni, &amount_in, &reserve_in, &reserve_out).unwrap();
    let b = univ2::amount_out(&cake, &amount_in, &reserve_in, &reserve_out).unwrap();
    assert_eq!(a.amount_out, U256::from(1_974u64));
    assert_eq!(b.amount_out, U256::from(1_975u64));
    assert_ne!(
        a.amount_out, b.amount_out,
        "tarifa y denominador efectivos cambian la salida"
    );
    // La comisión retenida es lo que el pool deja de recibir, exacto.
    assert_eq!(a.fee_raw, U256::from(3u64));
    assert_eq!(a.amount_in_with_fee_effective + a.fee_raw, amount_in);

    // Frontera de reserva: pedir más que la reserva no es un cero, es un error.
    assert!(matches!(
        univ2::amount_out(&uni, &U256::from(100_001u64), &reserve_in, &reserve_out),
        Err(CostError::ReserveBoundary { .. })
    ));
}

#[test]
fn curve_admin_split_and_resolution_are_consistent_end_to_end() {
    let a = asset(1, "0xdai", 18);
    let p = price_ref(1, "0xdai", 18, "1.0", "r1");
    let an = anchor(1);
    let c = ctx(Scope::leg(0, "3pool", "evm:1"), &a, &p, "r1", 1_500, &an);
    let gross = U256::from(1_000_000_000_000_000_000u64); // 1 DAI
    let fee_rate = U256::from(1_000_000u64); // 0.01% en 1e10
    let admin_fee = U256::from(5_000_000_000u64); // 50%

    let lp = curve::lp_fee_component(&c, &fee_rate, &admin_fee, &gross, "s", "3pool").unwrap();
    let admin =
        curve::admin_fee_component(&c, &admin_fee, &gross, &fee_rate, "s", "3pool").unwrap();
    let total_fee = curve::fee_on_output(&gross, &fee_rate).unwrap();
    assert_eq!(total_fee, U256::from(100_000_000_000_000u64)); // 0.0001 DAI
    assert_eq!(
        lp.amount_raw + admin.amount_raw,
        total_fee,
        "LP + admin = comisión total, sin crear ni perder"
    );
    assert_eq!(
        lp.amount_raw + admin.amount_raw,
        curve::fee_on_output(&gross, &fee_rate).unwrap(),
        "la suma de los dos componentes es EXACTAMENTE la comisión cobrada"
    );
    assert_eq!(lp.usd.as_ref().unwrap(), &bd("0.00005"));
    assert_eq!(admin.usd.as_ref().unwrap(), &bd("0.00005"));

    // Versionado: StableSwap-NG escala el fee del contrato por N/(4(N-1)).
    let ng = curve::CurveInvariant::StableSwapNg { n_coins: 2 };
    assert_eq!(
        curve::scaled_fee_rate(&ng, &U256::from(4_000_000u64)).unwrap(),
        U256::from(2_000_000u64)
    );
    let classic = curve::CurveInvariant::StableSwapClassic { n_coins: 2 };
    assert_eq!(
        curve::scaled_fee_rate(&classic, &U256::from(4_000_000u64)).unwrap(),
        U256::from(4_000_000u64)
    );
    // Versión no resuelta → no se adivina.
    let unknown = curve::CurveInvariant::detect(None, true, false, None);
    assert_eq!(unknown, curve::CurveInvariant::Unknown);
    let task = unknown
        .version_task(&route_scope(), "0xpool")
        .expect("tarea de versión");
    assert_eq!(
        task.receipt.as_deref(),
        Some("exact_invariant_version_and_rates")
    );
    assert!(curve::scaled_fee_rate(&unknown, &U256::from(1u64)).is_err());
}

#[test]
fn curve_oracle_branch_never_assumes_on_peg() {
    let one = U256::from(1_000_000_000_000_000_000u64);
    assert_eq!(
        curve::oracle_branch(None, &one, 50),
        curve::OracleBranch::Unknown
    );
    assert_eq!(
        curve::oracle_branch(Some(&one), &one, 50),
        curve::OracleBranch::OnPeg
    );
    let off = U256::from(1_100_000_000_000_000_000u64);
    assert_eq!(
        curve::oracle_branch(Some(&off), &one, 50),
        curve::OracleBranch::OffPeg
    );
    // Sin spot tampoco se decide.
    assert_eq!(
        curve::oracle_branch(Some(&one), &U256::zero(), 50),
        curve::OracleBranch::Unknown
    );
    let task = curve::oracle_task(&route_scope(), "0xpool");
    assert_eq!(
        task.receipt.as_deref(),
        Some("oracle_round_and_inventory_branch")
    );
}

#[test]
fn v3_ticks_requirement_task_is_emitted_when_traversal_is_incomplete() {
    let task = univ3::full_traversal_task(&Scope::leg(0, "pool", "evm:1"), "0xpool");
    assert_eq!(
        task.receipt.as_deref(),
        Some("full_tick_traversal_or_protocol_quoter")
    );
    let task4 = univ4::hook_state_task(&route_scope(), "0xhook", "dynamic_fee_unknown");
    assert_eq!(
        task4.receipt.as_deref(),
        Some("dynamic_parameters_and_hook_state")
    );
}

#[test]
fn per_kind_breakdown_keeps_traceability_after_aggregation() {
    let a = asset(1, "0xusdc", 6);
    let p = price_ref(1, "0xusdc", 6, "1.0", "r1");
    let an = anchor(1);
    let terms = univ2::V2FeeTerms::from_read(
        U256::from(997u64),
        U256::from(1_000u64),
        univ2::V2Fork::UniswapV2,
        "pair.fee()",
        "ev",
    )
    .unwrap();
    let c = ctx(Scope::leg(0, "pair0", "evm:1"), &a, &p, "r1", 1_500, &an);
    let mut r = CostResolution::new(1, "snap", "r1", "before_financing", "atomic_quote");
    r.push(univ2::lp_fee_component(&c, &terms, &U256::from(1_000_000u64), "s").unwrap());
    let t3 = univ3::V3FeeTerms::resolve(&univ3::V3FeeReads {
        fee_raw: U256::from(500u64),
        protocol_fee_num_raw: U256::from(1u64),
        protocol_fee_den_raw: U256::from(4u64),
        crosses_initialized_tick: true,
        source: "poolv3".into(),
        evidence_id: "ev:v3".into(),
    })
    .unwrap();
    let c3 = ctx(Scope::leg(1, "poolv3", "evm:1"), &a, &p, "r1", 1_500, &an);
    r.push(univ3::lp_fee_component(&c3, &t3, &U256::from(1_000_000u64), "s").unwrap());
    r.push(univ3::protocol_fee_component(&c3, &t3, &U256::from(1_000_000u64), "s").unwrap());
    let per_kind = r.per_kind_usd();
    // La agregación a una sola línea NO pierde la identidad por componente.
    // 1 USDC: v2 3/1000 → 0.003; v3 375/1e6 → 0.000375; protocol 125/1e6 → 0.000125.
    assert_eq!(per_kind.get(univ2::KIND_V2_LP_FEE), Some(&bd("0.003")));
    assert_eq!(per_kind.get(univ3::KIND_V3_LP_FEE), Some(&bd("0.000375")));
    assert_eq!(
        per_kind.get(univ3::KIND_V3_PROTOCOL_FEE),
        Some(&bd("0.000125"))
    );
    assert_eq!(r.to_cost_lines().unwrap().len(), 1);
    assert_eq!(r.embedded_fee_usd(), Some(bd("0.0035")));
}

#[test]
fn scope_key_separates_legs_of_the_same_venue() {
    let a = Scope::leg(0, "pool", "evm:1");
    let b = Scope::leg(1, "pool", "evm:1");
    let c = Scope::leg(0, "other", "evm:1");
    assert_ne!(a.key(), b.key());
    assert_ne!(a.key(), c.key());
}
