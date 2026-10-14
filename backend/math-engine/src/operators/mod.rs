//! FUSILE: math-physics-engine/operators — 32 Espacios de Hilbert Aislados
//!
//! Doctrina de Aislamiento Topológico:
//! - Cada operador existe en su propio archivo .rs
//! - Acoplamiento exclusivo vía trait TopologicalOperator
//! - Añadir op_XX → crear archivo + registrar en el registry (mod.rs) — así se
//!   añadió op_32 // HP-03 (2026-09-08), luego sustituido por NSGA-II (alta
//!   topología, 2026-09-11)
//! - El despachador (lib.rs) no requiere modificación

// Declaraciones de los 32 operadores; se preservan los IDs históricos 1–31.
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
// Operador 32 — brazo canónico ÚNICO: NSGA-II (alta topología, 2026-09-11),
// consumido por api.rs y searcher/live_risk_ranker. El brazo HP-03 (2026-09-08,
// escalarización ponderada mo_weight_*) se conserva compilable como capacidad
// de referencia; SOLO nsga2 está registrado en el OperatorRegistry.
pub mod op_32_multi_objective;
pub mod op_32_nsga2;

#[cfg(test)]
mod real_ops_tests;

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Number of operators REGISTERED IN THE RUNTIME, i.e. the size of the registry
/// built by `register_all()` below (IDs 1..=32; op_32 = NSGA-II).
///
/// This is NOT the number the strategy catalog uses. See
/// [`SOURCE_OPERATOR_COUNT`] — the two numbers are both true on different axes and
/// the gap is a declared design decision, not drift.
pub const OPERATOR_COUNT: u8 = 32;

/// Number of operators of the SOURCE catalog: the 264-strategy Master Matrix
/// (`strategy_mapping.json`, one record per `MEV-XX-YYY`) and the cartridge bridge
/// (`searcher-rs::native_operator_adapter`) reference only IDs 1..=31.
///
/// ARSE-264-01 (`t3`) — DECISION, declared and machine-checked (see
/// `real_ops_tests::source_catalog_boundary`): **op_32 stays compiled, registered
/// and tested at runtime, and stays DELIBERATELY UNREACHABLE from a cartridge.**
/// Reasons, in order of weight:
///
/// 1. **Shape mismatch.** op_32 = NSGA-II returns a Pareto FRONT (a non-dominated
///    SET, exposed as `matrix_result` + `scalar_value` = front cardinality). The
///    cartridge bridge builds one per-operator evidence receipt whose value is the
///    operator's own scalar/vector/matrix (`native_operator_adapter.rs`); a
///    multi-objective front has no single comparable figure to weight against the
///    31 scalar/vector operators, so a "32nd role" would be a category error, not
///    an addition.
/// 2. **The data matrix does not have a 32nd column.** `strategy_mapping.json`
///    (264 records) references exactly IDs 1..=31 — measured: union of
///    `primary_operators` ∪ `secondary_operators` ∪ `applicable_operators` ∪
///    `operator_weights` keys = {1..31}, zero occurrences of 32. Inventing the 32nd
///    column to "make the numbers match" is forbidden: the matrix is the
///    operator's data and the code adapts to it — never the reverse.
/// 3. **Nothing becomes reachable by wiring it.** No cartridge declares op_32, so
///    widening the bridge's admission range to 1..=32 would add a path with zero
///    callers while weakening an explicit boundary.
///
/// The invariant that keeps the two numbers honest:
/// `OPERATOR_COUNT == SOURCE_OPERATOR_COUNT + 1`, every ID in
/// `1..=SOURCE_OPERATOR_COUNT` resolves in the registry, and the single extra ID is
/// op_32. `real_ops_tests::source_catalog_boundary` fails if that arithmetic drifts.
pub const SOURCE_OPERATOR_COUNT: u8 = 31;

/// Estado de mercado normalizado — input universal para todos los operadores
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketState {
    /// Matriz de precios (n_venues × n_assets)
    pub price_matrix: Vec<Vec<f64>>,
    /// Clave canonica del par (pool) descrito por CADA fila de `price_matrix`.
    ///
    /// Formato: los dos tokens del pool en minusculas, ordenados
    /// lexicograficamente y unidos por `'|'`, p.ej. `"0xaaa...|0xbbb..."`. El
    /// orden lexicografico hace que `(A,B)` y `(B,A)` den la MISMA clave, y las
    /// minusculas evitan que el checksum EIP-55 parta en dos un mismo par.
    ///
    /// Por que existe (FEATURES-01b): `price_matrix` lleva una fila por pool y
    /// los pools de una ruta son pares DISTINTOS (hop1 = A/B, hop2 = B/C, ...).
    /// Sin identidad de par, `RegimeRouter::analyze` no puede distinguir "el
    /// mismo par en 2 venues" (arbitraje cross-venue legitimo, donde max/min - 1
    /// SI es correcto) de "2 hops de pares distintos" (donde max/min - 1 es un
    /// numero plausible-pero-falso). Con la clave el gap se calcula SOLO entre
    /// venues del mismo par; si no hay 2 venues de un mismo par, el gap queda
    /// en `None` (R8 fail-honest: nunca un valor fabricado).
    ///
    /// Invariante: `pair_keys.len() == price_matrix.len()`, MISMO orden
    /// (`pair_keys[i]` describe la fila `i`). Una fila sin identidad conocida
    /// lleva cadena vacia `""` y queda fuera de cualquier agrupamiento por par.
    ///
    /// `serde(default)`: los clientes del API (`ComputeRequest`, api.rs) que
    /// mandan el JSON previo a este campo siguen deserializando; el default
    /// vacio significa "sin identidad" (fail-honest), nunca "mismo par".
    #[serde(default)]
    pub pair_keys: Vec<String>,
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
    /// ID único del operador (1-32)
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
            // Op 32 canónico: NSGA-II (alta topología). Un solo brazo registrado.
            32 => Box::new(crate::operators::op_32_nsga2::Nsga2Operator::new()),
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
        self.all()
            .into_iter()
            .filter(|o| o.is_available())
            .collect()
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
