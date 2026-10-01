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
    /// Depeg — desviación de paridad de un activo anclado (stablecoin/LST).
    DepegDeviation,
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
    /// Desviación de paridad de un activo anclado (de `features["parity_deviation"]`).
    pub parity_deviation: Option<f64>,
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
    /// Desviación mínima de paridad para considerar depeg.
    pub parity_deviation: f64,
}

impl Default for RegimeThresholds {
    fn default() -> Self {
        Self {
            volatility: 0.02,        // 2% std dev de retornos
            arbitrage_gap: 0.003,    // 0.3% gap entre venues
            health_factor: 1.1,      // por debajo de 1.1 hay riesgo de liquidación
            oracle_bias: 0.002,      // 0.2% divergencia oracle vs on-chain
            parity_deviation: 0.005, // 0.5% desviación de paridad (depeg)
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

        // Gap de arbitraje: max/min - 1 SOLO entre venues del MISMO par.
        //
        // `price_matrix` lleva una fila por pool, y los pools de una ruta son
        // pares DISTINTOS (hop1 = A/B, hop2 = B/C, ...): comparar el precio de un
        // pool A/B contra el de un pool B/C no mide ningun arbitraje, solo produce
        // un numero plausible-pero-falso que contamina la clasificacion de
        // regimen. En cambio un arbitraje cross-venue legitimo (MISMO par en 2
        // DEXes distintos) SI es exactamente max/min - 1. La identidad que separa
        // ambos casos es `pair_keys` (FEATURES-01b).
        //
        // FEATURES-01b: se agrupa por par y se publica el MAYOR gap entre los
        // pares con >=2 venues. Filas sin identidad (`""`) no se agrupan. Si
        // ningun par tiene >=2 venues -> `None` (R8: ausencia, no 0.0 y jamas un
        // gap cruzado entre pares distintos).
        //
        // Acumulador por par: (precio_max, precio_min, n_venues).
        let mut venues_por_par: HashMap<&str, (f64, f64, usize)> = HashMap::new();
        for (row, pair_key) in state.price_matrix.iter().zip(state.pair_keys.iter()) {
            if pair_key.is_empty() {
                continue; // sin identidad de par: no se agrupa (R8)
            }
            let Some(price) = row.first().copied() else {
                continue;
            };
            if !price.is_finite() || price <= 0.0 {
                continue; // mismo filtro que la volatilidad
            }
            let entry = venues_por_par.entry(pair_key.as_str()).or_insert((
                f64::NEG_INFINITY,
                f64::INFINITY,
                0,
            ));
            entry.0 = entry.0.max(price);
            entry.1 = entry.1.min(price);
            entry.2 += 1;
        }

        // El mayor gap entre pares con >=2 venues (orden-independiente: es un max).
        let mut arbitrage_gap: Option<f64> = None;
        for &(max, min, n_venues) in venues_por_par.values() {
            if n_venues < 2 || min <= 0.0 {
                continue; // 1 sola venue no tiene gap entre venues
            }
            let gap = max / min - 1.0;
            arbitrage_gap = Some(match arbitrage_gap {
                Some(prev) => prev.max(gap),
                None => gap,
            });
        }
        m.arbitrage_gap = arbitrage_gap;

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

        // Desviación de paridad (depeg) — de features["parity_deviation"]. Un
        // activo anclado (stablecoin/LST) que pierde su ancla abre una captura
        // de paridad (redimir al valor anclado). Fail-honest: None si ausente.
        if let Some(&pd) = state.features.get("parity_deviation") {
            if pd.is_finite() && pd >= 0.0 {
                m.parity_deviation = Some(pd);
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
        if let Some(pd) = metrics.parity_deviation {
            if pd >= self.thresholds.parity_deviation {
                regimes.push(Regime::DepegDeviation);
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
    /// - DepegDeviation → Kelly (16), MonteCarlo (22), Regression (13) — captura
    ///   de paridad rota (redimir al valor anclado) con sizing + predicción.
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
                Regime::DepegDeviation => push(&[16, 22, 13]),
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
    use std::collections::HashMap;

    /// Clave canonica del par simulado A/B: dos tokens hex en minusculas,
    /// ordenados lexicograficamente y unidos por '|' (mismo formato que produce
    /// `canonical_pair_key` en searcher-rs).
    const PAR_AB: &str =
        "0xaaaa0000000000000000000000000000000000aa|0xbbbb0000000000000000000000000000000000bb";
    /// Clave canonica del par simulado A/C (hop 2 de una ruta A/B -> C/B).
    const PAR_AC: &str =
        "0xaaaa0000000000000000000000000000000000aa|0xcccc0000000000000000000000000000000000cc";
    /// Clave canonica del par simulado C/D.
    const PAR_CD: &str =
        "0xcccc0000000000000000000000000000000000cc|0xdddd0000000000000000000000000000000000dd";

    fn state_from(prices: &[f64], pair_keys: Vec<String>) -> MarketState {
        MarketState {
            price_matrix: prices.iter().map(|p| vec![*p]).collect(),
            pair_keys,
            liquidity_reserves: Vec::new(),
            gas_price_gwei: 20.0,
            block_timestamp: 1_700_000_000,
            block_number: 18_000_000,
            features: HashMap::new(),
        }
    }

    /// Serie de precios del MISMO par simulado (una fila por observacion):
    /// todas las filas comparten la clave, que es el caso cross-venue/serie.
    fn state(prices: &[f64]) -> MarketState {
        state_from(prices, vec![PAR_AB.to_string(); prices.len()])
    }

    #[test]
    fn detects_arbitrage_gap_across_venues() {
        // Two venues of the SAME pair with a 1% price gap. Contrato
        // FEATURES-01b: el gap solo se calcula entre venues del mismo par
        // (`pair_keys` identicas); este es el caso cross-venue legitimo donde
        // max/min - 1 SI es la metrica correcta, y sigue calculandose.
        let st = state_from(
            &[100.0, 101.0],
            vec![PAR_AB.to_string(), PAR_AB.to_string()],
        );
        let (_r, metrics, _ops) = RegimeRouter::default().route(&st);
        let gap = metrics.arbitrage_gap.unwrap();
        assert!((gap - 0.01).abs() < 1e-6, "gap should be ~1% (got {gap})");
    }

    // ── FEATURES-01b: el gap exige identidad de par ──────────────────────────
    // Sin `pair_keys` el router comparaba el precio de un pool A/B contra el de
    // un pool B/C (hops de una MISMA ruta, pares distintos) y publicaba un numero
    // plausible-pero-falso. Estos tests fijan el contrato nuevo: agrupar por par,
    // publicar el mayor gap entre pares con >=2 venues, y `None` cuando ningun
    // par tiene 2 venues (R8: ausencia, jamas un valor fabricado).

    #[test]
    fn distinct_pairs_never_produce_a_gap() {
        // 3 filas = 3 hops de pares DISTINTOS (A/B, A/C, C/D): no hay 2 venues de
        // un mismo par, asi que no hay arbitraje cross-venue medible -> None.
        let st = state_from(
            &[100.0, 101.0, 130.0],
            vec![PAR_AB.to_string(), PAR_AC.to_string(), PAR_CD.to_string()],
        );
        let (_r, metrics, _ops) = RegimeRouter::default().route(&st);
        assert!(
            metrics.arbitrage_gap.is_none(),
            "pares distintos no son venues comparables (got {:?})",
            metrics.arbitrage_gap
        );
    }

    #[test]
    fn same_pair_two_venues_gap_is_one_percent() {
        // Mismo par en 2 venues: 100.0 y 101.0 -> 1%.
        let st = state_from(
            &[100.0, 101.0],
            vec![PAR_AB.to_string(), PAR_AB.to_string()],
        );
        let (_r, metrics, _ops) = RegimeRouter::default().route(&st);
        let gap = metrics
            .arbitrage_gap
            .expect("2 venues del mismo par => gap");
        assert!(
            (gap - 0.01).abs() < 1e-9,
            "gap esperado 0.01, obtenido {gap}"
        );
    }

    #[test]
    fn largest_gap_wins_across_pairs() {
        // Par A/B en 2 venues (1%) y par C/D en 2 venues (5%) -> se publica 0.05.
        let st = state_from(
            &[100.0, 101.0, 100.0, 105.0],
            vec![
                PAR_AB.to_string(),
                PAR_AB.to_string(),
                PAR_CD.to_string(),
                PAR_CD.to_string(),
            ],
        );
        let (_r, metrics, _ops) = RegimeRouter::default().route(&st);
        let gap = metrics.arbitrage_gap.expect("ambos pares tienen 2 venues");
        assert!(
            (gap - 0.05).abs() < 1e-9,
            "debe publicarse el MAYOR gap (0.05), obtenido {gap}"
        );
    }

    #[test]
    fn single_venue_pair_has_no_gap() {
        // Dos filas con pares DISTINTOS y una sola venue cada uno: ninguno llega a
        // 2 venues -> None (1 venue no tiene "gap entre venues").
        let st = state_from(
            &[100.0, 100.0],
            vec![PAR_AB.to_string(), PAR_CD.to_string()],
        );
        let (_r, metrics, _ops) = RegimeRouter::default().route(&st);
        assert!(
            metrics.arbitrage_gap.is_none(),
            "una sola venue por par no es un gap (got {:?})",
            metrics.arbitrage_gap
        );
    }

    #[test]
    fn rows_without_pair_identity_are_ignored() {
        // Dos filas con la MISMA clave vacia ("") no se agrupan entre si: sin
        // identidad conocida no se afirma que sean el mismo par (R8) -> None.
        let st = state_from(&[100.0, 101.0], vec![String::new(), String::new()]);
        let (_r, metrics, _ops) = RegimeRouter::default().route(&st);
        assert!(
            metrics.arbitrage_gap.is_none(),
            "filas sin identidad no se agrupan (got {:?})",
            metrics.arbitrage_gap
        );
    }

    #[test]
    fn empty_pair_key_does_not_hide_a_real_gap() {
        // La fila sin identidad se ignora, pero las 2 venues identificadas del
        // mismo par SI producen su gap (no se contamina ni se pierde).
        let st = state_from(
            &[100.0, 999.0, 101.0],
            vec![PAR_AB.to_string(), String::new(), PAR_AB.to_string()],
        );
        let (_r, metrics, _ops) = RegimeRouter::default().route(&st);
        let gap = metrics
            .arbitrage_gap
            .expect("2 venues identificadas del mismo par");
        assert!(
            (gap - 0.01).abs() < 1e-9,
            "la fila sin identidad no debe entrar en el max/min (got {gap})"
        );
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
    fn detects_depeg_deviation() {
        let mut st = state(&[100.0, 100.2, 100.1]);
        st.features.insert("parity_deviation".to_string(), 0.008); // 0.8% depeg
        let (regimes, _m, ops) = RegimeRouter::default().route(&st);
        assert!(regimes.contains(&Regime::DepegDeviation));
        assert!(ops.contains(&16)); // Kelly (sizing on broken parity)
        assert!(ops.contains(&13)); // Regression (fair value estimate)
    }

    #[test]
    fn fail_honest_on_empty_state() {
        let st = MarketState {
            price_matrix: Vec::new(),
            pair_keys: Vec::new(),
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
