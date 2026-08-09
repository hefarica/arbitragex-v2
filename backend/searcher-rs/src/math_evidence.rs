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

/// Construye un `MarketState` desde el ReservesCache.
///
/// `pool_addresses`: pools del candidato (las venues de la ruta). Para cada una
/// con reservas, deriva el precio r1/r0 → una fila de la price_matrix (1 asset).
/// features: gas_price_gwei + cualquier feature de régimen provista por el
/// caller (health_factor, parity_deviation, oracle/onchain si aplica).
pub async fn build_market_state(
    reserves_cache: &Arc<ReservesCache>,
    pool_addresses: &[Address],
    gas_price_gwei: f64,
    block_number: u64,
    block_timestamp: u64,
    features: std::collections::HashMap<String, f64>,
) -> Option<MarketState> {
    let mut price_matrix: Vec<Vec<f64>> = Vec::new();
    let mut liquidity_reserves: Vec<(f64, f64)> = Vec::new();

    for pool in pool_addresses {
        if let Some((r0, r1)) = reserves_cache.get(pool).await {
            if let Some(price) = price_from_reserves(r0, r1) {
                price_matrix.push(vec![price]);
                liquidity_reserves.push((r0.as_u128() as f64, r1.as_u128() as f64));
            }
        }
    }

    if price_matrix.is_empty() {
        return None; // insufficient_state — no reserves for any pool
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
        .filter_map(|&id| {
            let out = registry.dispatch(id as u8, state)?;
            Some((id, out.scalar_value, out.operator_name))
        })
        .collect()
}

pub async fn evaluate_math_evidence(
    reserves_cache: &Arc<ReservesCache>,
    registry: &OperatorRegistry,
    router: &RegimeRouter,
    redis: &mut redis::aio::ConnectionManager,
    pool_addresses: &[Address],
    chain_id: u64,
    gas_price_gwei: f64,
    block_number: u64,
    block_timestamp: u64,
    features: std::collections::HashMap<String, f64>,
    strategy_kind: &str,
) -> usize {
    let state = match build_market_state(
        reserves_cache,
        pool_addresses,
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
                pools = pool_addresses.len(),
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

/// Construye el vector de evidencia per-oportunidad e = (O_1, …, O_31) sobre un
/// `MarketState`, despachando los 31 operadores. None → 0.0 (token "no computado";
/// su LR_k calibra a ~1). Índice = operator_id − 1 (0..30). Devuelve Vec<f64>
/// de largo 31. Observe-only: el llamador decide si persiste / alimenta el posterior.
pub fn build_evidence_vector(state: &MarketState, registry: &OperatorRegistry) -> Vec<f64> {
    let mut e = vec![0.0_f64; 31];
    for id in 1u8..=31u8 {
        if let Some(out) = registry.dispatch(id, state) {
            let idx = usize::from(id).wrapping_sub(1);
            if idx < 31 {
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
        assert_eq!(e.len(), 31, "evidence vector must have 31 slots");
        assert!(e.iter().all(|&v| v == 0.0), "degenerate state → all zeros");
    }

    #[test]
    fn posterior_is_flat_prior_with_empty_calibration() {
        let evidence = vec![0.5; 31];
        let empty_cal = vec![0.0; 31]; // sin calibrar
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
