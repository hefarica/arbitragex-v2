//! Trait TopologicalStrategy y struct StrategyOutput
//! Analogo a TopologicalOperator en operators/

use crate::operators::MarketState;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Output producido por la evaluacion de una estrategia topologica
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StrategyOutput {
    pub mev_id: String,
    pub topological_yield: Option<f64>,
    pub decoherencia: Option<f64>,
    pub gas_estimate: Option<f64>,
    pub viability_score: f64,
    pub metadata: HashMap<String, f64>,
}

/// Perfil declarativo de una estrategia (serializable, sin logica)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StrategyProfile {
    pub mev_id: String,
    pub group: u8,
    pub name: String,
    pub family: String,
    pub math_domain: String,
    pub atomic_possible: bool,
    pub nonatomic_possible: bool,
    pub min_legs: u32,
    pub max_legs: u32,
    pub applicable_operators: Vec<u8>,
}

/// Trait que todo vector estrategico MEV debe implementar
///
/// Invariante: el despachador solo conoce esta interfaz.
/// La implementacion interna es opaca.
pub trait TopologicalStrategy: Send + Sync {
    /// ID unico MEV (ej. "MEV-01-001")
    fn mev_id(&self) -> &'static str;

    /// Grupo (1-11)
    fn group(&self) -> u8;

    /// Nombre humano de la estrategia
    fn name(&self) -> &'static str;

    /// Familia estrategica
    fn family(&self) -> &'static str;

    /// Evaluar la estrategia sobre un estado de mercado
    fn evaluate(&self, state: &MarketState) -> StrategyOutput;

    /// Operadores matematicos aplicables (IDs 1-31)
    fn applicable_operators(&self) -> Vec<u8>;

    /// Posible en modo atomico
    fn atomic_possible(&self) -> bool;

    /// Posible en modo no-atomico
    fn nonatomic_possible(&self) -> bool;

    /// Minimo numero de patas
    fn min_legs(&self) -> u32;

    /// Maximo numero de patas
    fn max_legs(&self) -> u32;
}
