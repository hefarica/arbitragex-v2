//! Regime Router — árbol de decisión de operadores por régimen de mercado.
//!
//! Analiza un `MarketState`, deriva el régimen de mercado observable
//! (volatilidad, gap de arbitraje entre venues, proximidad a liquidación,
//! sesgo de oracle), y selecciona el subconjunto de los 31 operadores que
//! matemáticamente hacen sentido en ese régimen.
//!
//! Diseño doctrinal:
//! - Solo recomienda operadores; el consumidor (searcher) decide si los ejecuta
//!   con sus propios gates (simulación, net-profit, risk limits).
//! - R8 fail-honest: si no hay datos para medir un régimen, ese régimen no se
//!   reporta (None), nunca se fabrica.
//! - Los operadores recomendados se intersectan después con (a) los habilitados
//!   por toggle y (b) los `applicable_operators` de la estrategia (264×31).

use crate::operators::MarketState;
use std::collections::HashMap;

/// Régimen de mercado observable derivado del MarketState.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Regime {
    /// Alta volatilidad — dispersión de retornos por encima del umbral.
    HighVolatility,
    /// Gap de arbitraje — discrepancia de precio entre venues por el mismo activo.
    ArbitrageGap,
    /// Proximidad a liquidación — health factor cercano al umbral de liquidación.
    LiquidationProximity,
    /// Sesgo de oracle — divergencia oracle vs precio on-chain.
    OracleBias,
    /// Régimen neutral / sin señal dominante.
    Neutral,
}

/// Métricas observables derivadas del estado. Cada una es `Option` (fail-honest).
#[derive(Debug, Clone, Default)]
pub struct RegimeMetrics {
    /// Desviación estándar de los retornos logarítmicos del activo principal.
    pub volatility: Option<f64>,
    /// Gap relativo de precio entre venues (max/min - 1).
    pub arbitrage_gap: Option<f64>,
    /// Health factor mínimo observado (de `features["health_factor"]`).
    pub health_factor: Option<f64>,
    /// Sesgo absoluto oracle vs on-chain (de `features`).
    pub oracle_bias: Option<f64>,
}

/// Umbrales de régimen (configurables; defaults conservadores).
#[derive(Debug, Clone, Copy)]
pub struct RegimeThresholds {
    /// Desviación estándar mínima para considerar alta volatilidad.
    pub volatility: f64,
    /// Gap relativo mínimo para considerar arbitraje.
    pub arbitrage_gap: f64,
    /// Health factor bajo el cual hay proximidad a liquidación.
    pub health_factor: f64,
    /// Sesgo mínimo de oracle para considerar bias.
    pub oracle_bias: f64,
}

impl Default for RegimeThresholds {
    fn default() -> Self {
        Self {
            volatility: 0.02,     // 2% std dev de retornos
            arbitrage_gap: 0.003, // 0.3% gap entre venues
            health_factor: 1.1,   // por debajo de 1.1 hay riesgo de liquidación
            oracle_bias: 0.002,   // 0.2% divergencia oracle vs on-chain
        }
    }
}

/// Router de régimen — el "árbol de decisión" de operadores.
#[derive(Debug, Clone, Copy, Default)]
pub struct RegimeRouter {
    thresholds: RegimeThresholds,
}

impl RegimeRouter {
    pub fn new(thresholds: RegimeThresholds) -> Self {
        Self { thresholds }
    }

    /// Deriva las métricas observables del estado (fail-honest: None si no hay datos).
    pub fn analyze(state: &MarketState) -> RegimeMetrics {
        let mut m = RegimeMetrics::default();

        // Volatilidad: std dev de retornos logarítmicos de la serie (col 0 por fila).
        let prices: Vec<f64> = state
            .price_matrix
            .iter()
            .filter_map(|row| row.first().copied())
            .filter(|p| p.is_finite() && *p > 0.0)
            .collect();
        if prices.len() >= 3 {
            let rets: Vec<f64> = prices
                .windows(2)
                .filter(|w| w[0] > 0.0)
                .map(|w| (w[1] / w[0]).ln())
                .collect();
            if rets.len() >= 2 {
                let n = rets.len() as f64;
                let mu = rets.iter().sum::<f64>() / n;
                let var = rets.iter().map(|r| (r - mu).powi(2)).sum::<f64>() / n;
                m.volatility = Some(var.sqrt());
            }
        }

        // Gap de arbitraje: max/min del activo 0 entre venues (filas) - 1.
        let venue_prices: Vec<f64> = state
            .price_matrix
            .iter()
            .filter_map(|row| row.first().copied())
            .filter(|p| p.is_finite() && *p > 0.0)
            .collect();
        if venue_prices.len() >= 2 {
            let max = venue_prices.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
            let min = venue_prices.iter().cloned().fold(f64::INFINITY, f64::min);
            if min > 0.0 {
                m.arbitrage_gap = Some(max / min - 1.0);
            }
        }

        // Health factor (de features).
        if let Some(&hf) = state.features.get("health_factor") {
            if hf.is_finite() && hf > 0.0 {
                m.health_factor = Some(hf);
            }
        }

        // Sesgo oracle (de features: oracle_price vs onchain_price).
        if let (Some(&oracle), Some(&onchain)) = (
            state.features.get("oracle_price"),
            state.features.get("onchain_price"),
        ) {
            if onchain > 0.0 && oracle.is_finite() {
                m.oracle_bias = Some(((oracle - onchain) / onchain).abs());
            }
        }

        m
    }

    /// Clasifica los regímenes activos según los umbrales.
    pub fn classify(&self, metrics: &RegimeMetrics) -> Vec<Regime> {
        let mut regimes = Vec::new();
        if let Some(v) = metrics.volatility {
            if v >= self.thresholds.volatility {
                regimes.push(Regime::HighVolatility);
            }
        }
        if let Some(g) = metrics.arbitrage_gap {
            if g >= self.thresholds.arbitrage_gap {
                regimes.push(Regime::ArbitrageGap);
            }
        }
        if let Some(hf) = metrics.health_factor {
            if hf <= self.thresholds.health_factor {
                regimes.push(Regime::LiquidationProximity);
            }
        }
        if let Some(b) = metrics.oracle_bias {
            if b >= self.thresholds.oracle_bias {
                regimes.push(Regime::OracleBias);
            }
        }
        if regimes.is_empty() {
            regimes.push(Regime::Neutral);
        }
        regimes
    }

    /// El árbol de decisión: régimen → operadores recomendados (IDs 1-31).
    ///
    /// Mapeo doctrinal (operador matemático que hace sentido en cada régimen):
    /// - HighVolatility → MonteCarlo (22, predicción), Kelly (16, sizing), SVD (1)
    /// - ArbitrageGap → SVD (1, extracción), Regression (13, fair value), PCA (2)
    /// - LiquidationProximity → Kelly (16), MonteCarlo (22), BundleRecon (25)
    /// - OracleBias → Regression (13), KlDivergence (14), Eigen (3)
    /// - Neutral → Welford (10), Regression (13) — observación ligera
    pub fn recommend(&self, regimes: &[Regime]) -> Vec<u8> {
        let mut ops: Vec<u8> = Vec::new();
        let mut push = |ids: &[u8]| {
            for &id in ids {
                if !ops.contains(&id) {
                    ops.push(id);
                }
            }
        };
        for r in regimes {
            match r {
                Regime::HighVolatility => push(&[22, 16, 1]),
                Regime::ArbitrageGap => push(&[1, 13, 2]),
                Regime::LiquidationProximity => push(&[16, 22, 25]),
                Regime::OracleBias => push(&[13, 14, 3]),
                Regime::Neutral => push(&[10, 13]),
            }
        }
        ops
    }

    /// Pipeline completo: estado → régimen → operadores recomendados.
    /// Devuelve (regímenes detectados, métricas, operadores recomendados).
    pub fn route(&self, state: &MarketState) -> (Vec<Regime>, RegimeMetrics, Vec<u8>) {
        let metrics = Self::analyze(state);
        let regimes = self.classify(&metrics);
        let ops = self.recommend(&regimes);
        (regimes, metrics, ops)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn state(prices: &[f64]) -> MarketState {
        MarketState {
            price_matrix: prices.iter().map(|p| vec![*p]).collect(),
            liquidity_reserves: Vec::new(),
            gas_price_gwei: 20.0,
            block_timestamp: 1_700_000_000,
            block_number: 18_000_000,
            features: HashMap::new(),
        }
    }

    #[test]
    fn detects_arbitrage_gap_across_venues() {
        // Two venues with a 1% price gap.
        let st = MarketState {
            price_matrix: vec![vec![100.0], vec![101.0]],
            liquidity_reserves: Vec::new(),
            gas_price_gwei: 20.0,
            block_timestamp: 1_700_000_000,
            block_number: 18_000_000,
            features: HashMap::new(),
        };
        let (_r, metrics, _ops) = RegimeRouter::default().route(&st);
        let gap = metrics.arbitrage_gap.unwrap();
        assert!((gap - 0.01).abs() < 1e-6, "gap should be ~1% (got {gap})");
    }

    #[test]
    fn detects_high_volatility() {
        // Alternating ±5% moves → high volatility.
        let st = state(&[100.0, 105.0, 99.75, 104.7, 99.5, 104.4]);
        let (regimes, metrics, ops) = RegimeRouter::default().route(&st);
        let vol = metrics.volatility.unwrap();
        assert!(vol > 0.02, "volatility should exceed threshold (got {vol})");
        assert!(regimes.contains(&Regime::HighVolatility));
        assert!(ops.contains(&22)); // MonteCarlo recommended
        assert!(ops.contains(&16)); // Kelly recommended
    }

    #[test]
    fn detects_liquidation_proximity() {
        let mut st = state(&[100.0, 100.5, 100.2]);
        st.features.insert("health_factor".to_string(), 1.05);
        let (regimes, _m, ops) = RegimeRouter::default().route(&st);
        assert!(regimes.contains(&Regime::LiquidationProximity));
        assert!(ops.contains(&25)); // BundleRecon recommended
    }

    #[test]
    fn neutral_when_no_signal() {
        // Flat prices → no volatility, no gap → Neutral.
        let st = state(&[100.0, 100.0, 100.0, 100.0]);
        let (regimes, _m, ops) = RegimeRouter::default().route(&st);
        assert!(regimes.contains(&Regime::Neutral));
        assert!(ops.contains(&13)); // Regression (light observation)
    }

    #[test]
    fn fail_honest_on_empty_state() {
        let st = MarketState {
            price_matrix: Vec::new(),
            liquidity_reserves: Vec::new(),
            gas_price_gwei: 20.0,
            block_timestamp: 0,
            block_number: 0,
            features: HashMap::new(),
        };
        let (_r, metrics, _ops) = RegimeRouter::default().route(&st);
        assert!(metrics.volatility.is_none());
        assert!(metrics.arbitrage_gap.is_none());
    }
}
