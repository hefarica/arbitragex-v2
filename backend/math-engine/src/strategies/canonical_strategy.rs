//! Estrategia Canonica Parametrizada
//!
//! Una unica struct que implementa TopologicalStrategy para cualquiera de los 264
//! vectores estrategicos MEV.  Cada instancia se construye con sus metadatos
//! inmutables; el evaluate() es una funcion de caja negra que computa el viability
//! score a partir del MarketState.

use super::{StrategyOutput, TopologicalStrategy};
use crate::operators::MarketState;
use std::collections::HashMap;

/// Estrategia canonica parametrizada — representa un vector MEV del catalogo 264.
pub struct CanonicalStrategy {
    mev_id: &'static str,
    group: u8,
    name: &'static str,
    family: &'static str,
    applicable_operators: &'static [u8],
    atomic_possible: bool,
    nonatomic_possible: bool,
    min_legs: u32,
    max_legs: u32,
}

impl CanonicalStrategy {
    /// Constructor usado por el macro del registro.
    #[allow(clippy::too_many_arguments)]
    pub const fn new(
        mev_id: &'static str,
        group: u8,
        name: &'static str,
        family: &'static str,
        applicable_operators: &'static [u8],
        atomic_possible: bool,
        nonatomic_possible: bool,
        min_legs: u32,
        max_legs: u32,
    ) -> Self {
        Self {
            mev_id,
            group,
            name,
            family,
            applicable_operators,
            atomic_possible,
            nonatomic_possible,
            min_legs,
            max_legs,
        }
    }

    /// Computa un viability score heuristico a partir del estado de mercado.
    ///
    /// Formula: score = 1 / (1 + exp(-z))  (sigmoide logistico)
    /// donde z es una combinacion lineal de features disponibles.
    fn compute_viability(&self, state: &MarketState) -> f64 {
        let mut z = 0.0;

        // Penalizacion por gas elevado
        z -= state.gas_price_gwei * 0.01;

        // Bonificacion por liquidez agregada
        let total_liq: f64 = state
            .liquidity_reserves
            .iter()
            .map(|(r0, r1)| r0 + r1)
            .sum();
        z += (total_liq / 1_000_000.0).ln_1p();

        // Bonificacion por volatilidad (features["volatility"])
        if let Some(&vol) = state.features.get("volatility") {
            z += vol * 0.5;
        }

        // Penalizacion por decoherencia estimada (features["decoherencia"])
        if let Some(&dec) = state.features.get("decoherencia") {
            z -= dec * 2.0;
        }

        // Factor de escala segun grupo (algunos grupos son mas dificiles)
        let group_factor = match self.group {
            1 | 2 => 1.0,   // Spot / AMM — mas predecibles
            3 | 4 => 0.9,   // Estado / Paridad — requieren timing
            5 | 6 => 0.7,   // CEX-DEX / Cross-chain — alta latencia
            7 | 8 => 0.8,   // Derivados / Credito — complejidad
            9 | 10 => 0.75, // Intents / NFT — menor liquidez
            11 => 0.85,     // Prediction — datos externos
            _ => 0.5,
        };
        z *= group_factor;

        // Sigmoide para acotar en [0, 1]
        1.0 / (1.0 + (-z).exp())
    }

    /// Estima el yield topologico (usd-normalizado) a partir del estado.
    fn estimate_yield(&self, state: &MarketState) -> Option<f64> {
        let viability = self.compute_viability(state);
        if viability < 0.3 {
            return None; // Fracaso honesto: no hay yield computable
        }

        let base = match self.group {
            1 | 2 => 50.0,
            3 | 4 => 30.0,
            5 | 6 => 100.0,
            7 | 8 => 80.0,
            9 | 10 => 20.0,
            11 => 15.0,
            _ => 10.0,
        };

        Some(base * viability)
    }

    /// Estima la decoherencia de estado (slippage en terminos fisicos).
    fn estimate_decoherencia(&self, state: &MarketState) -> Option<f64> {
        // Decoherencia = f(volatilidad, 1/liquidez, gas)
        let vol = state.features.get("volatility").copied().unwrap_or(0.0);
        let total_liq: f64 = state
            .liquidity_reserves
            .iter()
            .map(|(r0, r1)| r0 + r1)
            .sum();
        let liq_factor = 1.0 / (1.0 + (total_liq / 100_000.0).ln_1p());
        let gas_factor = state.gas_price_gwei / 100.0;

        Some((vol * 0.01 + liq_factor * 0.005 + gas_factor * 0.001).min(0.5))
    }

    /// Estima el gas en USD.
    fn estimate_gas_usd(&self, state: &MarketState) -> Option<f64> {
        let base_gas = match self.group {
            1 | 2 => 80_000.0,
            3 | 4 => 120_000.0,
            5 | 6 => 250_000.0,
            7 | 8 => 180_000.0,
            9 | 10 => 150_000.0,
            11 => 100_000.0,
            _ => 100_000.0,
        };
        // gwei * gas_units * 1e-9 ETH * price_ETH_USD
        let eth_price = state
            .features
            .get("eth_price_usd")
            .copied()
            .unwrap_or(2000.0);
        let gas_units = base_gas * (1.0 + self.max_legs as f64 * 0.1);
        let gas_eth = state.gas_price_gwei * gas_units * 1e-9;
        Some(gas_eth * eth_price)
    }
}

impl TopologicalStrategy for CanonicalStrategy {
    fn mev_id(&self) -> &'static str {
        self.mev_id
    }

    fn group(&self) -> u8 {
        self.group
    }

    fn name(&self) -> &'static str {
        self.name
    }

    fn family(&self) -> &'static str {
        self.family
    }

    fn evaluate(&self, state: &MarketState) -> StrategyOutput {
        let viability = self.compute_viability(state);
        let mut metadata = HashMap::new();
        metadata.insert("group".to_string(), self.group as f64);
        metadata.insert("min_legs".to_string(), self.min_legs as f64);
        metadata.insert("max_legs".to_string(), self.max_legs as f64);
        metadata.insert(
            "atomic_possible".to_string(),
            if self.atomic_possible { 1.0 } else { 0.0 },
        );
        metadata.insert(
            "nonatomic_possible".to_string(),
            if self.nonatomic_possible { 1.0 } else { 0.0 },
        );

        StrategyOutput {
            mev_id: self.mev_id.to_string(),
            topological_yield: self.estimate_yield(state),
            decoherencia: self.estimate_decoherencia(state),
            gas_estimate: self.estimate_gas_usd(state),
            viability_score: viability,
            metadata,
        }
    }

    fn applicable_operators(&self) -> Vec<u8> {
        self.applicable_operators.to_vec()
    }

    fn atomic_possible(&self) -> bool {
        self.atomic_possible
    }

    fn nonatomic_possible(&self) -> bool {
        self.nonatomic_possible
    }

    fn min_legs(&self) -> u32 {
        self.min_legs
    }

    fn max_legs(&self) -> u32 {
        self.max_legs
    }
}
