//! Math Evidence — cableado Fix B (observe-only).
//!
//! Construye un `MarketState` real desde el `ReservesCache` del orchestrator
//! (precios por pool = reserve1/reserve0) + gas, y evalúa los operadores
//! matemáticos que el `RegimeRouter` recomienda para el régimen detectado.
//!
//! FASE OBSERVE-ONLY: los outputs se registran como telemetría/log estructurado
//! (qué régimen, qué operadores, qué valores computaron). NO alteran el scoring
//! todavía — esa es la siguiente iteración, gated por evidencia de que los
//! outputs son estables y correctos en producción. Doctrina anti-reincidencia:
//! nunca cablear matemática no validada directo al hot-path de decisión.
//!
//! R8 fail-honest: si no hay reservas suficientes para construir el estado, se
//! emite evidencia `insufficient_state`, nunca un MarketState fabricado.

use std::sync::Arc;

use ethers::types::Address;
use math_engine::{MarketState, OperatorRegistry, Regime, RegimeRouter};
use tracing::{debug, info};

use crate::engines::triangular_engine::ReservesCache;

/// Convierte reserve0/reserve1 (U256) a un precio f64 (r1/r0) cuando r0 > 0.
/// Devuelve None si r0 == 0 o el valor no cabe en f64 de forma finita.
fn price_from_reserves(r0: ethers::types::U256, r1: ethers::types::U256) -> Option<f64> {
    if r0.is_zero() {
        return None;
    }
    let r0f = r0.as_u128() as f64;
    let r1f = r1.as_u128() as f64;
    if r0f <= 0.0 {
        return None;
    }
    let p = r1f / r0f;
    if p.is_finite() && p > 0.0 {
        Some(p)
    } else {
        None
    }
}

/// Precio del par en UNIDADES HUMANAS: `(r1 / 10^dec_out) / (r0 / 10^dec_in)`.
///
/// MATH-04-FOLLOWUP (2026-09-30): el ratio crudo `r1/r0` mezcla unidades mínimas
/// de dos tokens con decimales distintos — WETH(18)/USDC(6) daba ~0.002 en vez
/// de ~2000. Dividir cada lado por su propia escala lo lleva a unidades del
/// token, el mismo criterio que `wei_str_to_token_units` / `weiUnits` en el
/// resto del repo.
///
/// R8: escala no finita, reserva cero o resultado no finito/no positivo → `None`.
fn normalized_price(
    r0: ethers::types::U256,
    r1: ethers::types::U256,
    dec_in: u8,
    dec_out: u8,
) -> Option<f64> {
    let raw = price_from_reserves(r0, r1)?;
    let scale_in = 10f64.powi(dec_in as i32);
    let scale_out = 10f64.powi(dec_out as i32);
    if !scale_in.is_finite() || !scale_out.is_finite() || scale_in <= 0.0 || scale_out <= 0.0 {
        return None;
    }
    // raw = r1/r0  ⇒  humano = (r1/scale_out)/(r0/scale_in) = raw · scale_in/scale_out
    let human = raw * (scale_in / scale_out);
    if human.is_finite() && human > 0.0 {
        Some(human)
    } else {
        None
    }
}

/// Decimales por token, cacheados por intent (mismo contrato que
/// `cartridge_boot::v4_token_decimals`). R8: meta ausente/ilegible → `None`,
/// y la pierna que lo necesite se OMITE — nunca se asume 18.
async fn leg_token_decimals(
    redis: &mut redis::aio::ConnectionManager,
    chain_id: u64,
    addr_lower: &str,
    cache: &mut std::collections::HashMap<String, Option<u8>>,
) -> Option<u8> {
    if let Some(hit) = cache.get(addr_lower) {
        return *hit;
    }
    let decimals = crate::reserves::get_token_meta(redis, chain_id, addr_lower)
        .await
        .ok()
        .flatten()
        .map(|meta| meta.decimals);
    cache.insert(addr_lower.to_owned(), decimals);
    decimals
}

/// Construye un `MarketState` desde el ReservesCache.
///
/// `pool_legs`: `(pool, token_in, token_out)` por pierna, en el orden de la ruta.
/// Para cada pool con reservas deriva el precio en UNIDADES HUMANAS del par:
/// `(r1 / 10^dec_out) / (r0 / 10^dec_in)`.
///
/// MATH-04-FOLLOWUP (2026-09-30): cierra el "follow-up thread" que MATH-04 dejó
/// abierto (2026-09-24). Antes la `price_matrix` llevaba el ratio CRUDO `r1/r0`:
/// para WETH(18)/USDC(6) eso da ~0.002 en vez de ~2000, y los consumidores
/// (op_27 path ordering, op_15/op_21 `reference_price` = media) calculaban
/// spreads y medias sobre valores desviados por 10^3 — de modo que devolvían
/// `scalar: null` y la evidencia salía con `operators_computed: 0`.
///
/// El insumo nunca faltó: `RouteIntentLeg` YA lleva `token_in`/`token_out`
/// (`route_intent.rs:145,147`); el llamador los descartaba al quedarse solo con
/// `pool_hint`. Aquí se conservan.
///
/// features: gas_price_gwei + cualquier feature de régimen provista por el
/// caller (health_factor, parity_deviation, oracle/onchain si aplica).
///
/// R8 fail-honest: si los decimales de CUALQUIERA de los dos tokens no están
/// disponibles, la pierna se OMITE. Nunca se asume 18, nunca se normaliza a
/// medias. Si ninguna pierna sobrevive → `None` (`insufficient_state`).
#[allow(clippy::too_many_arguments)] // market-state inputs (reservas + piernas + contexto de bloque + features); mismo criterio que evaluate_math_evidence
pub async fn build_market_state(
    reserves_cache: &Arc<ReservesCache>,
    pool_legs: &[(Address, Address, Address)],
    chain_id: u64,
    redis: &mut redis::aio::ConnectionManager,
    gas_price_gwei: f64,
    block_number: u64,
    block_timestamp: u64,
    features: std::collections::HashMap<String, f64>,
) -> Option<MarketState> {
    let mut price_matrix: Vec<Vec<f64>> = Vec::new();
    let mut liquidity_reserves: Vec<(f64, f64)> = Vec::new();
    let mut decimal_cache: std::collections::HashMap<String, Option<u8>> =
        std::collections::HashMap::new();

    for (pool, token_in, token_out) in pool_legs {
        let Some((r0, r1)) = reserves_cache.get(pool).await else {
            continue;
        };
        let in_lc = format!("0x{:040x}", token_in);
        let out_lc = format!("0x{:040x}", token_out);
        // R8: sin decimales de AMBOS tokens no hay precio honesto — se omite.
        let (Some(dec_in), Some(dec_out)) = (
            leg_token_decimals(redis, chain_id, &in_lc, &mut decimal_cache).await,
            leg_token_decimals(redis, chain_id, &out_lc, &mut decimal_cache).await,
        ) else {
            debug!(
                event = "math_evidence.leg_skipped_no_decimals",
                chain_id,
                pool = %pool,
                token_in = %in_lc,
                token_out = %out_lc,
                "pierna omitida: decimales ausentes (R8, no se asume 18)"
            );
            continue;
        };
        if let Some(price) = normalized_price(r0, r1, dec_in, dec_out) {
            price_matrix.push(vec![price]);
            liquidity_reserves.push((r0.as_u128() as f64, r1.as_u128() as f64));
        }
    }

    if price_matrix.is_empty() {
        return None; // insufficient_state — no priced pool with known decimals
    }

    Some(MarketState {
        price_matrix,
        liquidity_reserves,
        gas_price_gwei,
        block_timestamp,
        block_number,
        features,
    })
}

/// FEATURES-01a (2026-10-01): features de régimen desde fuentes VIVAS.
///
/// El call site pasaba `std::collections::HashMap::new()` — el mapa nacía vacío y
/// moría vacío. Por eso `regime_router` deja las 5 métricas en `null`, clasifica
/// siempre `["Neutral"]` (que recomienda solo 2 operadores) y la evidencia sale
/// con `operators_computed: 0` y `scalar: null`.
///
/// Esta entrega alimenta `parity_deviation` desde el PriceBus
/// (`arbx:token_prices:<chain>`): la MAYOR desviación de paridad de un stablecoin
/// respecto a $1. Es la única de las cinco con fuente Redis viva y verificada
/// (medido: USDC 1.0000972 · USDT 0.99949749 · DAI 0.99985272 · LUSD 1.0056).
///
/// Lo que NO se alimenta aquí, y por qué (R8: no se inventa):
/// * `oracle_price` / `onchain_price` (sesgo oracle): las anclas Chainlink viven
///   en el PriceBus en proceso, NO en Redis. `arbx:quote:anchor:1` se inspeccionó
///   y es salud del grafo (`cross_dex`/`liquidity`/`stability`/`venues`), no
///   precios de oráculo. Requiere productor propio.
/// * `health_factor`: estado de lending, sin productor.
///
/// R8 fail-honest: sin ningún stable con precio, el mapa va VACÍO y
/// `regime_router` deja la métrica en `null` — nunca un cero fabricado.
pub async fn regime_features_from_redis(
    redis: &mut redis::aio::ConnectionManager,
    chain_id: u64,
) -> std::collections::HashMap<String, f64> {
    let mut out = std::collections::HashMap::new();
    let key = format!("arbx:token_prices:{}", chain_id);
    let raw: Option<std::collections::HashMap<String, String>> =
        redis::AsyncCommands::hgetall(&mut *redis, &key).await.ok();
    let Some(map) = raw else {
        return out;
    };
    if let Some(dev) = worst_stable_deviation(&map) {
        out.insert("parity_deviation".to_owned(), dev);
    }
    out
}

/// Stables reconocidos. La paridad se mide contra $1; el símbolo debe existir en
/// el PriceBus para contar (si no hay dato, no hay métrica).
const PARITY_STABLES: &[&str] = &[
    "USDC", "USDT", "DAI", "FRAX", "TUSD", "USDP", "GUSD", "LUSD", "USDD", "PYUSD",
];

/// MAYOR desviación de paridad de un stable respecto a $1, o `None` si ninguno
/// tiene precio parseable. Pura (sin Redis) y por tanto testeable.
///
/// R8: precios no finitos o ≤ 0 se ignoran en vez de contar como desviación;
/// sin ningún stable válido devuelve `None` y el llamador NO inserta la métrica.
fn worst_stable_deviation(map: &std::collections::HashMap<String, String>) -> Option<f64> {
    let mut worst: Option<f64> = None;
    for sym in PARITY_STABLES {
        let Some(v) = map.get(*sym) else { continue };
        let Ok(px) = v.parse::<f64>() else { continue };
        if !px.is_finite() || px <= 0.0 {
            continue;
        }
        let dev = (px - 1.0).abs();
        worst = Some(worst.map_or(dev, |w: f64| w.max(dev)));
    }
    worst
}

/// Evalúa el régimen y los operadores recomendados sobre un candidato, y emite
/// evidencia estructurada (observe-only). Devuelve el número de operadores que
/// computaron un valor (para el log).
#[allow(clippy::too_many_arguments)] // market-state inputs; bundle into a struct if it grows
/// Evaluate a specific set of operators (by ID 1-31) against a MarketState —
/// **strategy-keyed** evidence (Plan 264×31 matrix wiring step 2). Unlike
/// `evaluate_math_evidence` (regime-keyed via `RegimeRouter`), this evaluates the
/// operators a specific cartridge declares (`CartridgeMetadata.primary_operators`
/// / `secondary_operators`), enabling per-strategy math evidence. R8 fail-honest:
/// an operator that can't compute returns `None` (never fabricated).
///
/// Returns `(op_id, scalar_value, operator_name)` per requested operator.
pub fn evaluate_strategy_operators(
    state: &MarketState,
    registry: &OperatorRegistry,
    operator_ids: &[u32],
) -> Vec<(u32, Option<f64>, String)> {
    operator_ids
        .iter()
        .filter(|&id| !crate::operator_toggles::is_disabled(*id as u8))
        .filter_map(|&id| {
            let out = registry.dispatch(id as u8, state)?;
            Some((id, out.scalar_value, out.operator_name))
        })
        .collect()
}

/// Pure snapshot builder for a strategy's declared-combo evidence (testable).
///
/// `roles` marks each operator as `"primary"` / `"secondary"` — the strategy's
/// OWN declaration of which structures apply to it, not a regime or class
/// guess. R8 fail-honest: an operator that cannot compute keeps `scalar: null`.
pub fn declared_combo_snapshot(
    chain_id: u64,
    strategy_key: &str,
    primary: &[(u32, Option<f64>, String)],
    secondary: &[(u32, Option<f64>, String)],
) -> serde_json::Value {
    fn to_values(entries: &[(u32, Option<f64>, String)], role: &str) -> Vec<serde_json::Value> {
        entries
            .iter()
            .map(|(id, scalar, name)| {
                serde_json::json!({
                    "op": id,
                    "role": role,
                    "name": name,
                    "scalar": scalar,
                })
            })
            .collect()
    }
    let computed = primary
        .iter()
        .chain(secondary.iter())
        .filter(|(_, s, _)| s.is_some())
        .count();
    serde_json::json!({
        "chain_id": chain_id,
        "strategy_kind": strategy_key,
        "source": "declared_combo",
        "primary_operators": to_values(primary, "primary"),
        "secondary_operators": to_values(secondary, "secondary"),
        "operators": primary.len() + secondary.len(),
        "operators_computed": computed,
        "updated_at_ms": std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0),
    })
}

/// Redis key for a strategy's §IV evidence snapshot. ONE canonical key format
/// for every reader/writer (STRAT-IDENT-01): before this, the orchestrator
/// writer keyed `{:?}` of `RouterKind` (a DEX router class) while the emitter
/// reader keyed `{:?}` of the `StrategyKind` newtype — the keys NEVER matched
/// and `evidence_vector` was structurally always null.
pub fn strategy_evidence_key(chain_id: u64, strategy_key: &str) -> String {
    format!("arbx:math_evidence:{}:{}", chain_id, strategy_key)
}

/// STRAT-IDENT-01: publish the per-strategy §IV evidence from the strategy's
/// OWN declared combo (operator directive 2026-08-23: "evaluar directamente
/// cada estrategia y esta dirá cuáles son las estructuras que le aplican y con
/// ello armas el combo particular y específico").
///
/// Evaluates `evaluate_strategy_operators` over the operators the strategy
/// DECLARES (`CartridgeMetadata.primary_operators` / `secondary_operators`,
/// mirroring the canonical STRATEGY.json) and publishes the snapshot to
/// `strategy_evidence_key` (TTL 120s) — the key the emitter's
/// `score_and_publish` reads and the api-server `/api/math/evidence` route
/// serves. Observe-only: never alters scoring decisions.
///
/// R8 fail-honest: no reserves for any pool → logs `insufficient_state` and
/// publishes NOTHING (never a fabricated MarketState). Returns the number of
/// operators that computed a value.
#[allow(clippy::too_many_arguments)] // strategy declaration + market context; mirrors evaluate_math_evidence
pub async fn publish_declared_combo_evidence(
    reserves_cache: &Arc<ReservesCache>,
    registry: &OperatorRegistry,
    redis: &mut redis::aio::ConnectionManager,
    pool_legs: &[(Address, Address, Address)],
    chain_id: u64,
    strategy_key: &str,
    primary_operator_ids: &[u32],
    secondary_operator_ids: &[u32],
    gas_price_gwei: f64,
    block_number: u64,
    block_timestamp: u64,
) -> usize {
    // Features: the strategy evaluation context carries none today (same as
    // evaluate_math_evidence's callers) — operators requiring features compute
    // None honestly.
    let state = match build_market_state(
        reserves_cache,
        pool_legs,
        chain_id,
        &mut *redis,
        gas_price_gwei,
        block_number,
        block_timestamp,
        std::collections::HashMap::new(),
    )
    .await
    {
        Some(s) => s,
        None => {
            debug!(
                event = "math_evidence.combo_insufficient_state",
                chain_id,
                strategy_key,
                pools = pool_legs.len(),
                "declared-combo evidence skipped — no reserves to build MarketState"
            );
            return 0;
        }
    };

    let primary = evaluate_strategy_operators(&state, registry, primary_operator_ids);
    let secondary = evaluate_strategy_operators(&state, registry, secondary_operator_ids);
    let computed = primary
        .iter()
        .chain(secondary.iter())
        .filter(|(_, s, _)| s.is_some())
        .count();

    let snapshot = declared_combo_snapshot(chain_id, strategy_key, &primary, &secondary);
    if let Ok(json) = serde_json::to_string(&snapshot) {
        use redis::AsyncCommands;
        let key = strategy_evidence_key(chain_id, strategy_key);
        if let Err(e) = redis.set_ex::<_, _, ()>(&key, json, 120).await {
            debug!(
                event = "math_evidence.combo_persist_failed",
                chain_id,
                strategy_key,
                error = %e,
                "failed to persist declared-combo evidence (non-fatal)"
            );
        }
    }
    debug!(
        event = "math_evidence.combo_published",
        chain_id,
        strategy_key,
        declared = primary_operator_ids.len() + secondary_operator_ids.len(),
        computed,
        "declared-combo evidence published (observe-only)"
    );
    computed
}

#[allow(clippy::too_many_arguments)] // 11 params: the full math-evidence pipeline context (reserves, registry, router, redis, pools, chain, gas, block, features, strategy). Refactoring to a struct would obscure the data flow.
pub async fn evaluate_math_evidence(
    reserves_cache: &Arc<ReservesCache>,
    registry: &OperatorRegistry,
    router: &RegimeRouter,
    redis: &mut redis::aio::ConnectionManager,
    pool_legs: &[(Address, Address, Address)],
    chain_id: u64,
    gas_price_gwei: f64,
    block_number: u64,
    block_timestamp: u64,
    features: std::collections::HashMap<String, f64>,
    strategy_kind: &str,
) -> usize {
    let state = match build_market_state(
        reserves_cache,
        pool_legs,
        chain_id,
        &mut *redis,
        gas_price_gwei,
        block_number,
        block_timestamp,
        features,
    )
    .await
    {
        Some(s) => s,
        None => {
            debug!(
                event = "math_evidence.insufficient_state",
                chain_id,
                strategy_kind,
                pools = pool_legs.len(),
                "math evidence skipped — no reserves to build MarketState"
            );
            return 0;
        }
    };

    let (regimes, metrics, op_ids) = router.route(&state);

    let regime_names: Vec<String> = regimes.iter().map(|r| format!("{:?}", r)).collect();

    let mut computed = 0usize;
    let mut op_values: Vec<serde_json::Value> = Vec::new();
    for id in &op_ids {
        if crate::operator_toggles::is_disabled(*id) {
            continue;
        }
        if let Some(out) = registry.dispatch(*id, &state) {
            if out.scalar_value.is_some() {
                computed += 1;
            }
            op_values.push(serde_json::json!({
                "op": id,
                "name": out.operator_name,
                "scalar": out.scalar_value,
                "computed": out.metadata.get("computed").copied().unwrap_or(0.0),
            }));
        }
    }

    info!(
        event = "math_evidence.evaluated",
        chain_id,
        strategy_kind,
        regimes = ?regime_names,
        volatility = metrics.volatility,
        arbitrage_gap = metrics.arbitrage_gap,
        health_factor = metrics.health_factor,
        oracle_bias = metrics.oracle_bias,
        parity_deviation = metrics.parity_deviation,
        operators = ?op_ids,
        operators_computed = computed,
        op_values = %serde_json::to_string(&op_values).unwrap_or_default(),
        "math evidence evaluated (observe-only)"
    );

    // Persist the snapshot to Redis so the api-server can serve the LIVE regime
    // + per-operator values to the dashboard. Key per (chain, strategy_kind).
    // TTL 120s: refreshed on every intent; expires if the searcher goes quiet
    // (R8 fail-honest — the API then reports "no recent evidence").
    let snapshot = serde_json::json!({
        "chain_id": chain_id,
        "strategy_kind": strategy_kind,
        "regimes": regime_names,
        "metrics": {
            "volatility": metrics.volatility,
            "arbitrage_gap": metrics.arbitrage_gap,
            "health_factor": metrics.health_factor,
            "oracle_bias": metrics.oracle_bias,
            "parity_deviation": metrics.parity_deviation,
        },
        "operators": op_values,
        "operators_computed": computed,
        "updated_at_ms": std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0),
    });
    if let Ok(json) = serde_json::to_string(&snapshot) {
        use redis::AsyncCommands;
        let key = format!("arbx:math_evidence:{}:{}", chain_id, strategy_kind);
        if let Err(e) = redis.set_ex::<_, _, ()>(&key, json, 120).await {
            debug!(
                event = "math_evidence.persist_failed",
                chain_id,
                strategy_kind,
                error = %e,
                "failed to persist math evidence snapshot (non-fatal)"
            );
        }
    }

    computed
}

// ─── §IV: primitivas puras para el posterior calibrado (Stage 1) ──────────────
//
// Base del cableado math-evidence → scoring (dictamen §IV). PURAS (sin I/O),
// unit-testeables, fundamento de la calibración Stage 2. El cableado al hot-path
// de emisión (snapshot per-oportunidad + aplicación en evaluate_paper_opportunity)
// es el paso siguiente enfocado — estas primitivas son lo que ese paso requiere.

/// Construye el vector de evidencia per-oportunidad sobre un `MarketState`,
/// despachando los operadores registrados. None → 0.0 (token "no computado";
/// su LR_k calibra a ~1). Índice = operator_id − 1. Devuelve Vec<f64> de
/// largo OPERATOR_COUNT (32 desde op_32 NSGA-II — MATH-08 fix 2026-09-24;
/// antes era 31 fijo, excluyendo op_32 del espacio de calibración).
/// Observe-only: el llamador decide si persiste / alimenta el posterior.
pub fn build_evidence_vector(state: &MarketState, registry: &OperatorRegistry) -> Vec<f64> {
    let count = math_engine::operators::OPERATOR_COUNT as usize;
    let mut e = vec![0.0_f64; count];
    for id in 1u8..=math_engine::operators::OPERATOR_COUNT {
        if crate::operator_toggles::is_disabled(id) {
            continue;
        }
        if let Some(out) = registry.dispatch(id, state) {
            let idx = usize::from(id).wrapping_sub(1);
            if idx < count {
                e[idx] = out.scalar_value.unwrap_or(0.0);
            }
        }
    }
    e
}

/// §IV posterior: log-odds = prior_log_odds + Σ_k (log_lr_k · e_k), con la
/// convención de que un log_lr_k ≈ 0 (no calibrado, LR = e^0 = 1) NO contribuye.
/// `calibration` = slice de log-LR por operador (índice id−1, largo ≤ 31).
/// Devuelve (posterior_log_odds, source_context) donde source_context =
/// "calibrated" si algún |log_lr_k| > ε, sino "flat_prior" (honesto: sin
/// calibración el posterior colapsa al prior — el motor está cableado pero OFF).
/// P(yield) = sigmoid(posterior_log_odds); f* = (b·p̂−q)/b (Kelly) downstream.
pub fn evidence_posterior_log_odds(
    prior_log_odds: f64,
    evidence: &[f64],
    calibration: &[f64],
) -> (f64, &'static str) {
    let n = evidence.len().min(calibration.len()).min(31);
    let mut sum = 0.0_f64;
    let mut calibrated = false;
    for k in 0..n {
        let lr_k = calibration[k];
        if lr_k.abs() > 1e-12 {
            calibrated = true;
            sum += lr_k * evidence[k];
        }
    }
    let log_odds = prior_log_odds + sum;
    let ctx: &'static str = if calibrated {
        "calibrated"
    } else {
        "flat_prior"
    };
    (log_odds, ctx)
}

#[cfg(test)]
mod evidence_tests {
    use super::*;
    use std::collections::HashMap;

    // ── FEATURES-01a: parity_deviation desde el PriceBus ────────────────────
    #[test]
    fn parity_deviation_is_the_worst_stable_and_ignores_the_rest() {
        let mut m = HashMap::new();
        // Valores REALES medidos en produccion (2026-09-30/10-01).
        m.insert("USDC".to_owned(), "1.000097188494".to_owned());
        m.insert("USDT".to_owned(), "0.99949749".to_owned());
        m.insert("DAI".to_owned(), "0.99985272".to_owned());
        m.insert("LUSD".to_owned(), "1.0056".to_owned());
        // Un no-stable con desviacion enorme NO debe contar como paridad.
        m.insert("PEPE".to_owned(), "0.0000042".to_owned());
        let d = worst_stable_deviation(&m).expect("hay stables");
        assert!(
            (d - 0.0056).abs() < 1e-9,
            "debe ser la PEOR desviacion de stable (LUSD 0.0056), dio {d}"
        );
    }

    #[test]
    fn parity_deviation_is_none_without_a_usable_stable_never_a_fabricated_zero() {
        // Sin ningun stable: None (el llamador NO inserta la metrica).
        let mut m = HashMap::new();
        m.insert("PEPE".to_owned(), "0.0000042".to_owned());
        assert!(worst_stable_deviation(&m).is_none());
        // Map vacio: None.
        assert!(worst_stable_deviation(&HashMap::new()).is_none());
        // Stable con basura / no positivo: se ignora, no cuenta como desviacion.
        let mut bad = HashMap::new();
        bad.insert("USDC".to_owned(), "no-es-un-numero".to_owned());
        bad.insert("USDT".to_owned(), "0".to_owned());
        bad.insert("DAI".to_owned(), "-1.0".to_owned());
        assert!(worst_stable_deviation(&bad).is_none());
        // Un solo stable valido entre basura SI cuenta.
        let mut one = HashMap::new();
        one.insert("USDC".to_owned(), "0.998".to_owned());
        one.insert("USDT".to_owned(), "NaN".to_owned());
        let d = worst_stable_deviation(&one).expect("USDC es valido");
        assert!((d - 0.002).abs() < 1e-9, "dio {d}");
    }

    // ── MATH-04-FOLLOWUP: vector dorado de normalización ────────────────────
    // La puerta del PR. Sin esto, el cambio de escala es un cambio a ciegas.
    #[test]
    fn weth_usdc_normalizes_to_human_units_not_the_raw_ratio() {
        // Pool WETH(18)/USDC(6) con reservas 1 WETH : 2000 USDC.
        let r0 = ethers::types::U256::from(1_000_000_000_000_000_000u128); // 1 WETH
        let r1 = ethers::types::U256::from(2_000_000_000u128); // 2000 USDC

        // El ratio CRUDO es ~0.002: es exactamente el bug que este PR cierra.
        let raw = price_from_reserves(r0, r1).expect("ratio crudo");
        assert!(raw < 1.0, "ratio crudo esperado ~0.002, dio {raw}");

        let human = normalized_price(r0, r1, 18, 6).expect("precio normalizado");
        assert!(
            (human - 2000.0).abs() < 0.01,
            "WETH/USDC normalizado debe ser ~2000, no ~0.002 (dio {human})"
        );
    }

    #[test]
    fn degenerate_reserves_yield_none_never_a_fabricated_price() {
        let r0 = ethers::types::U256::from(1_000_000_000_000_000_000u128);
        let r1 = ethers::types::U256::from(2_000_000_000u128);

        // Reserva de entrada cero → None (R8: nunca se inventa un precio).
        assert!(normalized_price(ethers::types::U256::zero(), r1, 18, 6).is_none());
        // Reserva de salida cero → None.
        assert!(normalized_price(r0, ethers::types::U256::zero(), 18, 6).is_none());
        // Extremos del rango de decimales siguen dando un valor finito y > 0.
        let extreme = normalized_price(r0, r1, 0, 36).expect("escala extrema");
        assert!(extreme.is_finite() && extreme > 0.0);
    }

    #[test]
    fn build_evidence_vector_has_31_slots_and_none_to_zero() {
        // Estado degenerado (sin reservas ⇒ operadores devuelven None) ⇒ 31 ceros.
        let state = MarketState {
            price_matrix: vec![],
            liquidity_reserves: vec![],
            gas_price_gwei: 0.0,
            block_timestamp: 0,
            block_number: 0,
            features: HashMap::new(),
        };
        let registry = OperatorRegistry::new();
        let e = build_evidence_vector(&state, &registry);
        // MATH-08 fix: 32 slots since op_32 NSGA-II (was 31 — excluded op_32
        // from the calibration space).
        assert_eq!(
            e.len(),
            math_engine::operators::OPERATOR_COUNT as usize,
            "evidence vector must have OPERATOR_COUNT slots"
        );
        assert!(e.iter().all(|&v| v == 0.0), "degenerate state → all zeros");
    }

    #[test]
    fn posterior_is_flat_prior_with_empty_calibration() {
        let count = math_engine::operators::OPERATOR_COUNT as usize;
        let evidence = vec![0.5; count];
        let empty_cal = vec![0.0; count]; // sin calibrar
        let (lo, ctx) = evidence_posterior_log_odds(0.1, &evidence, &empty_cal);
        assert!(
            (lo - 0.1).abs() < 1e-12,
            "empty calibration ⇒ posterior = prior"
        );
        assert_eq!(ctx, "flat_prior");
    }

    #[test]
    fn posterior_applies_calibrated_log_odds() {
        // log_lr[0] = 1.0, evidence = [0.5;31] ⇒ Σ = 1.0·0.5 = 0.5; +prior 0.1 ⇒ 0.6.
        let evidence = vec![0.5; 31];
        let mut cal = vec![0.0; 31];
        cal[0] = 1.0; // operator 1 (idx 0) calibrado
        let (lo, ctx) = evidence_posterior_log_odds(0.1, &evidence, &cal);
        assert!(
            (lo - 0.6).abs() < 1e-9,
            "posterior = prior + lr·e = 0.1+0.5 = 0.6: {lo}"
        );
        assert_eq!(ctx, "calibrated");
    }
}

#[cfg(test)]
mod combo_tests {
    use super::*;

    /// STRAT-IDENT-01: the ONE canonical key format — plain strategy identity,
    /// no Debug-wrapped newtype, no RouterKind class.
    #[test]
    fn strategy_evidence_key_is_plain_strategy_identity() {
        assert_eq!(
            strategy_evidence_key(1, "mev_01_001_dex_dex"),
            "arbx:math_evidence:1:mev_01_001_dex_dex"
        );
        assert_eq!(
            strategy_evidence_key(1, "flashloan_arb"),
            "arbx:math_evidence:1:flashloan_arb"
        );
    }

    /// The snapshot carries the strategy's declared roles and honest nulls.
    #[test]
    fn declared_combo_snapshot_marks_roles_and_counts_computed() {
        let primary = vec![(15u32, Some(0.42), "op_15".to_string())];
        let secondary = vec![
            (1u32, None, "op_01".to_string()),
            (22u32, Some(0.7), "op_22".to_string()),
        ];
        let s = declared_combo_snapshot(1, "mev_01_001", &primary, &secondary);
        assert_eq!(s["strategy_kind"], "mev_01_001");
        assert_eq!(s["source"], "declared_combo");
        assert_eq!(s["operators"], 3);
        assert_eq!(s["operators_computed"], 2);
        assert_eq!(s["primary_operators"][0]["role"], "primary");
        assert_eq!(s["secondary_operators"][0]["role"], "secondary");
        // R8: un-computable operator keeps scalar null — never fabricated.
        assert!(s["secondary_operators"][0]["scalar"].is_null());
        assert_eq!(s["primary_operators"][0]["scalar"], 0.42);
    }
}
