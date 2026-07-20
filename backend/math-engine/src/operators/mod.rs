//! FUSILE: math-physics-engine/operators — 31 Espacios de Hilbert Aislados
//!
//! Doctrina de Aislamiento Topológico:
//! - Cada operador existe en su propio archivo .rs
//! - Acoplamiento exclusivo vía trait TopologicalOperator
//! - Añadir op_32 → crear archivo + registrar en registry.rs
//! - El despachador (lib.rs) no requiere modificación

// Declaraciones de los 31 operadores
pub mod op_01_svd;
pub mod op_02_pca;
pub mod op_03_eigen;
pub mod op_04_von_neumann;
pub mod op_05_pdmp;
pub mod op_06_markov_chain;
pub mod op_07_hmm;
pub mod op_08_kalman;
pub mod op_09_levy;
pub mod op_10_welford;
pub mod op_11_bayes;
pub mod op_12_mle;
pub mod op_13_regression;
pub mod op_14_kl_divergence;
pub mod op_15_golden_section;
pub mod op_16_kelly;
pub mod op_17_pontryagin;
pub mod op_18_lagrangian;
pub mod op_19_simplex;
pub mod op_20_gradient_descent;
pub mod op_21_newton;
pub mod op_22_monte_carlo;
pub mod op_23_queueing;
pub mod op_24_nash;
pub mod op_25_bundle_recon;
pub mod op_26_flash_loan;
pub mod op_27_path_ordering;
pub mod op_28_jit_liquidity;
pub mod op_29_shapley;
pub mod op_30_gnn_encoder;
pub mod op_31_drl_agent;

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Estado de mercado normalizado — input universal para todos los operadores
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketState {
    /// Matriz de precios (n_venues × n_assets)
    pub price_matrix: Vec<Vec<f64>>,
    /// Reservas de liquidez por venue (n_venues × 2 para par token0/token1)
    pub liquidity_reserves: Vec<(f64, f64)>,
    /// Gas price estimado en gwei
    pub gas_price_gwei: f64,
    /// Timestamp del bloque actual
    pub block_timestamp: u64,
    /// Número de bloque
    pub block_number: u64,
    /// Features adicionales (volatilidad, volumen, etc.)
    pub features: HashMap<String, f64>,
}

/// Output de un operador topológico — transformación del estado
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OperatorOutput {
    /// ID del operador que produjo este output
    pub operator_id: u8,
    /// Nombre del operador
    pub operator_name: String,
    /// Métrica escalar principal (si aplica)
    pub scalar_value: Option<f64>,
    /// Vector resultado (si aplica)
    pub vector_result: Option<Vec<f64>>,
    /// Matriz resultado (si aplica)
    pub matrix_result: Option<Vec<Vec<f64>>>,
    /// Metadatos adicionales
    pub metadata: HashMap<String, f64>,
}

/// Trait que todo operador matemático-físico debe implementar
///
/// Invariante: el despachador solo conoce esta interfaz.
/// La implementación interna es opaca.
pub trait TopologicalOperator: Send + Sync {
    /// ID único del operador (1-31)
    fn id(&self) -> u8;

    /// Nombre humano del operador
    fn name(&self) -> &'static str;

    /// Categoría del operador
    fn category(&self) -> &'static str;

    /// Evaluar el operador sobre un estado de mercado
    fn evaluate(&self, state: &MarketState) -> OperatorOutput;

    /// Verificar si el operador está disponible (dependencias, features)
    fn is_available(&self) -> bool {
        true
    }
}

/// Registry de operadores — despachador central
pub struct OperatorRegistry {
    operators: HashMap<u8, Box<dyn TopologicalOperator>>,
}

impl OperatorRegistry {
    pub fn new() -> Self {
        let mut registry = Self {
            operators: HashMap::new(),
        };
        registry.register_all();
        registry
    }

    fn register_all(&mut self) {
        macro_rules! register {
            ($($id:expr => $ctor:expr),* $(,)?) => {
                $(
                    self.operators.insert($id, $ctor);
                )*
            };
        }

        register! {
            1 => Box::new(crate::operators::op_01_svd::SvdOperator::new()),
            2 => Box::new(crate::operators::op_02_pca::PCAOperator::new()),
            3 => Box::new(crate::operators::op_03_eigen::EigenOperator::new()),
            4 => Box::new(crate::operators::op_04_von_neumann::VonNeumannOperator::new()),
            5 => Box::new(crate::operators::op_05_pdmp::PDMPOperator::new()),
            6 => Box::new(crate::operators::op_06_markov_chain::MarkovChainOperator::new()),
            7 => Box::new(crate::operators::op_07_hmm::HMMOperator::new()),
            8 => Box::new(crate::operators::op_08_kalman::KalmanOperator::new()),
            9 => Box::new(crate::operators::op_09_levy::LevyOperator::new()),
            10 => Box::new(crate::operators::op_10_welford::WelfordOperator::new()),
            11 => Box::new(crate::operators::op_11_bayes::BayesOperator::new()),
            12 => Box::new(crate::operators::op_12_mle::MLEOperator::new()),
            13 => Box::new(crate::operators::op_13_regression::RegressionOperator::new()),
            14 => Box::new(crate::operators::op_14_kl_divergence::KlDivergenceOperator::new()),
            15 => Box::new(crate::operators::op_15_golden_section::GoldenSectionOperator::new()),
            16 => Box::new(crate::operators::op_16_kelly::KellyOperator::new()),
            17 => Box::new(crate::operators::op_17_pontryagin::PontryaginOperator::new()),
            18 => Box::new(crate::operators::op_18_lagrangian::LagrangianOperator::new()),
            19 => Box::new(crate::operators::op_19_simplex::SimplexOperator::new()),
            20 => Box::new(crate::operators::op_20_gradient_descent::GradientDescentOperator::new()),
            21 => Box::new(crate::operators::op_21_newton::NewtonOperator::new()),
            22 => Box::new(crate::operators::op_22_monte_carlo::MonteCarloOperator::new()),
            23 => Box::new(crate::operators::op_23_queueing::QueueingOperator::new()),
            24 => Box::new(crate::operators::op_24_nash::NashOperator::new()),
            25 => Box::new(crate::operators::op_25_bundle_recon::BundleReconOperator::new()),
            26 => Box::new(crate::operators::op_26_flash_loan::FlashLoanOperator::new()),
            27 => Box::new(crate::operators::op_27_path_ordering::PathOrderingOperator::new()),
            28 => Box::new(crate::operators::op_28_jit_liquidity::JitLiquidityOperator::new()),
            29 => Box::new(crate::operators::op_29_shapley::ShapleyOperator::new()),
            30 => Box::new(crate::operators::op_30_gnn_encoder::GnnEncoderOperator::new()),
            31 => Box::new(crate::operators::op_31_drl_agent::DrlAgentOperator::new()),
        }
    }

    pub fn get(&self, id: u8) -> Option<&dyn TopologicalOperator> {
        self.operators.get(&id).map(|b| b.as_ref())
    }

    pub fn all(&self) -> Vec<&dyn TopologicalOperator> {
        let mut ops: Vec<_> = self.operators.values().map(|b| b.as_ref()).collect();
        ops.sort_by_key(|o| o.id());
        ops
    }

    pub fn available(&self) -> Vec<&dyn TopologicalOperator> {
        self.all().into_iter().filter(|o| o.is_available()).collect()
    }

    pub fn dispatch(&self, id: u8, state: &MarketState) -> Option<OperatorOutput> {
        self.get(id).map(|op| op.evaluate(state))
    }

    pub fn dispatch_batch(&self, ids: &[u8], state: &MarketState) -> Vec<OperatorOutput> {
        ids.iter()
            .filter_map(|&id| self.dispatch(id, state))
            .collect()
    }
}

impl Default for OperatorRegistry {
    fn default() -> Self {
        Self::new()
    }
}
