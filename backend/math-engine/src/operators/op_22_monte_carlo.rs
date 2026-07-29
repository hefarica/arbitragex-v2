//! FUSILE: Implementacion propia -- Simulacion Estocastica (Geometric Brownian Motion)
//! Formula: dS_t = μ·S_t·dt + σ·S_t·dW_t, con dW_t ~ N(0, √dt)
//! Categoria: simulation
//!
//! R8 fail-honest: si no hay datos suficientes para estimar μ/σ, devuelve
//! scalar_value: None (no computado) — nunca una trayectoria fabricada.

use super::{MarketState, OperatorOutput, TopologicalOperator};
use rand::SeedableRng;
use rand_distr::{Distribution, Normal};
use std::collections::HashMap;

#[derive(Default)]
pub struct MonteCarloOperator;

impl MonteCarloOperator {
    pub fn new() -> Self {
        Self
    }

    /// Extrae la serie de precios desde `MarketState.price_matrix`.
    /// La price_matrix es n_venues × n_assets; usamos la media por fila
    /// (venue) como el "nivel" agregado del activo. Devuelve una serie
    /// temporal aplastada (la columna de más datos) si existe.
    fn price_series(state: &MarketState) -> Vec<f64> {
        if state.price_matrix.is_empty() {
            return Vec::new();
        }
        // Usar la primera columna (asset 0) a través de las filas como serie.
        state
            .price_matrix
            .iter()
            .filter_map(|row| row.first().copied())
            .filter(|p| p.is_finite() && *p > 0.0)
            .collect()
    }

    /// Retornos logarítmicos de la serie.
    fn log_returns(prices: &[f64]) -> Vec<f64> {
        prices
            .windows(2)
            .filter(|w| w[0] > 0.0)
            .map(|w| (w[1] / w[0]).ln())
            .collect()
    }
}

impl TopologicalOperator for MonteCarloOperator {
    fn id(&self) -> u8 {
        22
    }

    fn name(&self) -> &'static str {
        "Simulacion Estocastica"
    }

    fn category(&self) -> &'static str {
        "simulation"
    }

    fn evaluate(&self, state: &MarketState) -> OperatorOutput {
        let prices = Self::price_series(state);
        let returns = Self::log_returns(&prices);

        // Necesitamos al menos 2 retornos para estimar drift/volatilidad.
        if prices.len() < 3 || returns.len() < 2 {
            return OperatorOutput {
                operator_id: self.id(),
                operator_name: self.name().to_string(),
                scalar_value: None,
                vector_result: None,
                matrix_result: None,
                metadata: {
                    let mut m = HashMap::new();
                    m.insert("computed".to_string(), 0.0);
                    m.insert("reason_insufficient_data".to_string(), 1.0);
                    m
                },
            };
        }

        let n = returns.len() as f64;
        let mu = returns.iter().sum::<f64>() / n;
        let var = returns.iter().map(|r| (r - mu).powi(2)).sum::<f64>() / n;
        let sigma = var.sqrt();
        let s0 = *prices.last().unwrap_or(&1.0);

        // GBM: simular N trayectorias a 1 paso (dt = 1).
        // dW ~ N(0, √dt); con dt=1, dW ~ N(0,1).
        const PATHS: usize = 1000;
        let normal = match Normal::new(0.0, 1.0) {
            Ok(n) => n,
            Err(_) => {
                return OperatorOutput {
                    operator_id: self.id(),
                    operator_name: self.name().to_string(),
                    scalar_value: None,
                    vector_result: None,
                    matrix_result: None,
                    metadata: {
                        let mut m = HashMap::new();
                        m.insert("computed".to_string(), 0.0);
                        m.insert("reason_normal_init_failed".to_string(), 1.0);
                        m
                    },
                }
            }
        };

        // RNG determinista por estado (reproducible): semilla derivada del
        // block_number + nº de precios — NO Date::now (mantiene determinismo
        // entre llamadas con el mismo estado, importante para backtesting).
        let seed = state
            .block_number
            .wrapping_mul(0x9E3779B97F4A7C15)
            .wrapping_add(prices.len() as u64);
        let mut rng = rand::rngs::SmallRng::seed_from_u64(seed);

        let mut terminals: Vec<f64> = Vec::with_capacity(PATHS);
        for _ in 0..PATHS {
            let dw = normal.sample(&mut rng);
            // Terminal S_T = S0 * exp((μ - σ²/2)·dt + σ·√dt·Z), forma exacta GBM.
            let drift = (mu - 0.5 * sigma * sigma) + sigma * dw;
            let s_t = s0 * drift.exp();
            if s_t.is_finite() && s_t > 0.0 {
                terminals.push(s_t);
            }
        }

        if terminals.is_empty() {
            return OperatorOutput {
                operator_id: self.id(),
                operator_name: self.name().to_string(),
                scalar_value: None,
                vector_result: None,
                matrix_result: None,
                metadata: {
                    let mut m = HashMap::new();
                    m.insert("computed".to_string(), 0.0);
                    m.insert("reason_all_paths_degenerate".to_string(), 1.0);
                    m
                },
            };
        }

        let m_len = terminals.len() as f64;
        let expected_pv = terminals.iter().sum::<f64>() / m_len;
        let t_var = terminals
            .iter()
            .map(|p| (p - expected_pv).powi(2))
            .sum::<f64>()
            / m_len;
        let std_dev = t_var.sqrt();

        // Percentil 5 (cola izquierda — estimación de riesgo de caída).
        terminals.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        let p5 = terminals[(terminals.len() as f64 * 0.05) as usize];

        let mut metadata = HashMap::new();
        metadata.insert("computed".to_string(), 1.0);
        metadata.insert("expected_pv".to_string(), expected_pv);
        metadata.insert("drift_mu".to_string(), mu);
        metadata.insert("volatility_sigma".to_string(), sigma);
        metadata.insert("p5_tail".to_string(), p5);
        metadata.insert("paths".to_string(), terminals.len() as f64);

        OperatorOutput {
            operator_id: self.id(),
            operator_name: self.name().to_string(),
            // Magnitud principal: desviación estándar de las trayectorias
            // (la "volatilidad de extracción" — dispersión del precio futuro).
            scalar_value: Some(std_dev),
            vector_result: Some(vec![std_dev, expected_pv, p5]),
            matrix_result: None,
            metadata,
        }
    }
}
