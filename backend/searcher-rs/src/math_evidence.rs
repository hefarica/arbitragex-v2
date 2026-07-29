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
use math_engine::{
    MarketState, OperatorRegistry, Regime, RegimeRouter,
};
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
pub async fn evaluate_math_evidence(
    reserves_cache: &Arc<ReservesCache>,
    registry: &OperatorRegistry,
    router: &RegimeRouter,
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

    let regime_names: Vec<String> = regimes
        .iter()
        .map(|r| format!("{:?}", r))
        .collect();

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

    computed
}
