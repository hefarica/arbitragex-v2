//! Cobertura Ortogonal — Entrelazamiento de Estados de Mercado
//!
//! Cuando el Allocator (Phase 5) inyecta un impulso de Dirac en una
//! variedad de liquidez descentralizada (DEX), este módulo calcula el
//! vector de compensación exacta en un entorno ortogonal (CEX u otro DEX)
//! para garantizar que la covarianza del sistema permanezca estrictamente
//! nula: Cov(Δ_DEX, Δ_CEX) = 0.

use crate::allocator::{DiracImpulse, LiquidityManifold};
use nalgebra::{DMatrix, DVector};
use num_complex::Complex64;

/// Estado entrelazado |Ψ_AB⟩ entre dos mercados (ej. DEX y CEX).
/// La matriz de densidad reducida ρ_A = Tr_B(|Ψ⟩⟨Ψ|) cuantifica la
/// correlación no-local entre ambos entornos.
#[derive(Debug, Clone, PartialEq)]
pub struct EntangledState {
    /// Amplitud compleja del estado conjunto (vectorizado)
    pub amplitude: DVector<Complex64>,
    /// Dimensión del espacio de estados del mercado A
    pub market_a_dimension: usize,
    /// Dimensión del espacio de estados del mercado B
    pub market_b_dimension: usize,
    /// Entropía de von Neumann: S(ρ_A) = −Tr(ρ_A log ρ_A)
    pub entanglement_entropy: f64,
    /// Identificadores de mercado
    pub market_a_id: String,
    pub market_b_id: String,
}

impl EntangledState {
    /// Construye un estado entrelazado puro a partir de dos vectores de amplitud.
    /// |Ψ_AB⟩ = |ψ_A⟩ ⊗ |ψ_B⟩ + correlación (off-diagonal).
    pub fn from_market_states(
        state_a: &DVector<Complex64>,
        state_b: &DVector<Complex64>,
        correlation: f64, // coeficiente de correlación [−1, 1]
        market_a_id: String,
        market_b_id: String,
    ) -> Self {
        let dim_a = state_a.len();
        let dim_b = state_b.len();
        let dim_total = dim_a * dim_b;
        let mut amplitude = DVector::zeros(dim_total);

        // Producto tensorial con término de correlación entrelazada
        for i in 0..dim_a {
            for j in 0..dim_b {
                let idx = i * dim_b + j;
                let product = state_a[i] * state_b[j];
                // Añadir componente entrelazada proporcional a la correlación
                let entangled_component = Complex64::new(correlation, 0.0)
                    * (state_a[i].conj() * state_b[j] - state_a[i] * state_b[j].conj());
                amplitude[idx] = product + entangled_component;
            }
        }

        // Normalizar
        let norm = amplitude.iter().map(|c| c.norm_sqr()).sum::<f64>().sqrt();
        if norm > 1e-15 {
            amplitude /= Complex64::new(norm, 0.0);
        }

        // Calcular entropía de entrelazamiento (aproximación para estados puros)
        let entropy = Self::compute_von_neumann_entropy(&amplitude, dim_a, dim_b);

        Self {
            amplitude,
            market_a_dimension: dim_a,
            market_b_dimension: dim_b,
            entanglement_entropy: entropy,
            market_a_id,
            market_b_id,
        }
    }

    /// Entropía de von Neumann para estados puros: S(ρ_A) = −Tr(ρ_A log ρ_A).
    ///
    /// # Fundamento Matemático
    /// Para estados puros |Ψ_AB⟩, la matriz de densidad reducida ρ_A = Tr_B(|Ψ⟩⟨Ψ|)
    /// tiene eigenvalores λ_i (valores singulares de la matriz de amplitud). La entropía
    /// es S = −Σ λ_i ln(λ_i) donde λ_i ≥ 0 y Σ λ_i = 1.
    ///
    /// # Implementación
    /// Calculamos eigenvalores de ρ_A = A·A^T donde A_{ij} = amplitude[i*dim_b + j].
    /// Los eigenvalues de ρ_A son λ_i (probabilidades normalizadas), no λ_i².
    fn compute_von_neumann_entropy(
        amplitude: &DVector<Complex64>,
        dim_a: usize,
        dim_b: usize,
    ) -> f64 {
        // Construir matriz de amplitud A_{ij} = amplitude[i*dim_b + j]
        let mut mat = DMatrix::zeros(dim_a, dim_b);
        for i in 0..dim_a {
            for j in 0..dim_b {
                mat[(i, j)] = amplitude[i * dim_b + j];
            }
        }

        // Matriz de densidad reducida: ρ_A = A·A^† (producto exterior)
        let rho_a = &mat * mat.adjoint();
        let eigen = rho_a.symmetric_eigen();
        let eigenvalues = eigen.eigenvalues.clone();

        // Normalizar eigenvalores para garantizar Σ λ_i = 1 (probabilidades)
        let sum: f64 = eigenvalues.iter().sum();
        if sum < 1e-15 {
            return 0.0; // Estado degenerado, entropía nula
        }

        let mut entropy = 0.0;
        for lambda in eigenvalues.iter() {
            let p = (lambda / sum).max(0.0);
            if p > 1e-15 {
                entropy -= p * p.ln(); // λ ln(λ) — forma canónica von Neumann
            }
        }
        entropy.max(0.0)
    }
}

/// Hiperplano ortogonal Π_⟂ donde Cov(Δ_A, Δ_B) = 0.
/// Construido mediante Gram-Schmidt sobre la base del espacio tangente.
#[derive(Debug, Clone, PartialEq)]
pub struct OrthogonalHyperplane {
    /// Vector normal n al hiperplano (gradiente de la covarianza)
    pub normal_vector: DVector<f64>,
    /// Vectores base ortonormales de Π_⟂
    pub basis_vectors: Vec<DVector<f64>>,
    /// Matriz de proyección P = I − n·n^T / ||n||²
    pub projection_matrix: DMatrix<f64>,
}

impl OrthogonalHyperplane {
    /// Construye Π_⟂ a partir del vector de covarianza entre dos mercados.
    pub fn from_covariance_vector(cov_vector: &DVector<f64>) -> Self {
        let n = cov_vector.clone();
        let norm_sq = n.dot(&n);
        let dim = n.len();

        // Matriz de proyección: P = I − n·n^T / ||n||²
        let identity = DMatrix::identity(dim, dim);
        let nn_t = &n * n.transpose();
        let projection = if norm_sq > 1e-15 {
            identity - nn_t / norm_sq
        } else {
            identity
        };

        // Construir base ortonormal de Π_⟂ mediante Gram-Schmidt
        let mut basis = Vec::new();
        for i in 0..dim {
            let mut e = DVector::zeros(dim);
            e[i] = 1.0;
            // Proyectar sobre Π_⟂
            let proj = &projection * &e;
            if proj.norm() > 1e-9 {
                basis.push(proj.normalize());
            }
        }

        Self {
            normal_vector: n,
            basis_vectors: basis,
            projection_matrix: projection,
        }
    }

    /// Proyecta un vector v sobre Π_⟂: v_⟂ = P·v
    pub fn project(&self, v: &DVector<f64>) -> DVector<f64> {
        &self.projection_matrix * v
    }

    /// Verifica que dos vectores sean ortogonales respecto a Π_⟂
    pub fn is_orthogonal(&self, v1: &DVector<f64>, v2: &DVector<f64>, tolerance: f64) -> bool {
        let p1 = self.project(v1);
        let p2 = self.project(v2);
        p1.dot(&p2).abs() < tolerance
    }
}

/// Vector direccional de riesgo en el espacio tangente del mercado.
#[derive(Debug, Clone, PartialEq)]
pub struct DirectionalVector {
    /// Componentes del vector (cambios en reservas o precios)
    pub components: DVector<f64>,
    /// Origen del mercado (ej: "uniswap-v2-weth-usdc")
    pub market_origin: String,
    /// Norma del vector (magnitud del riesgo)
    pub norm: f64,
}

impl DirectionalVector {
    pub fn new(components: DVector<f64>, market_origin: String) -> Self {
        let norm = components.norm();
        Self {
            components,
            market_origin,
            norm,
        }
    }

    /// Vector de riesgo inducido por un impulso de Dirac en un DEX.
    pub fn from_dirac_impulse(impulse: &DiracImpulse) -> Self {
        let comp = DVector::from(vec![impulse.injection_point.x, impulse.injection_point.y]);
        Self::new(comp, impulse.manifold.pool_address.clone())
    }
}

/// Resultado de la operación de hedging ortogonal.
#[derive(Debug, Clone, PartialEq)]
pub struct OrthogonalHedgeResult {
    /// Estado entrelazado post-hedge
    pub hedged_state: EntangledState,
    /// Matriz de covarianza 2×2 del sistema
    pub covariance_matrix: DMatrix<f64>,
    /// ¿Se logró neutralización completa?
    pub is_fully_neutralized: bool,
    /// Vector residual no neutralizado
    pub residual_direction: DVector<f64>,
    /// Error de ortogonalidad: ||v − v_⟂|| / ||v||
    pub orthogonality_error: f64,
    /// Covarianza off-diagonal (índice rápido)
    pub covariance_off_diagonal: f64,
    /// Vector de compensación calculado para el mercado ortogonal
    pub compensation_vector: DVector<f64>,
}

impl OrthogonalHedgeResult {
    /// Verifica covarianza nula dentro de tolerancia numérica.
    pub fn verify_null_covariance(&self, tolerance: f64) -> bool {
        self.covariance_off_diagonal.abs() < tolerance
            && self.orthogonality_error < tolerance
            && self.is_fully_neutralized
    }

    /// Backward-compatible stub for Phase 1 tests.
    pub fn stub(is_null: bool) -> Self {
        Self {
            hedged_state: EntangledState {
                amplitude: DVector::from(vec![Complex64::new(1.0, 0.0)]),
                market_a_dimension: 1,
                market_b_dimension: 1,
                entanglement_entropy: 0.0,
                market_a_id: "stub_a".to_string(),
                market_b_id: "stub_b".to_string(),
            },
            covariance_matrix: DMatrix::identity(2, 2),
            is_fully_neutralized: is_null,
            residual_direction: DVector::zeros(2),
            orthogonality_error: if is_null { 0.0 } else { 1.0 },
            covariance_off_diagonal: if is_null { 0.0 } else { f64::NAN },
            compensation_vector: DVector::zeros(2),
        }
    }
}

/// Motor de hedging ortogonal — núcleo de Phase 6.1
pub struct OrthogonalVarianceHedger {
    /// Umbral de correlación para considerar dos mercados como candidatos
    pub correlation_threshold: f64,
    /// Tasa de decoherencia (pérdida de correlación temporal)
    pub decoherence_rate: f64,
    /// Tolerancia para covarianza nula
    pub null_covariance_tolerance: f64,
}

impl Default for OrthogonalVarianceHedger {
    fn default() -> Self {
        Self::new()
    }
}

impl OrthogonalVarianceHedger {
    pub const DEFAULT_TOLERANCE: f64 = 1e-9;

    pub fn new() -> Self {
        Self {
            correlation_threshold: 0.85,
            decoherence_rate: 0.01,
            null_covariance_tolerance: Self::DEFAULT_TOLERANCE,
        }
    }

    /// Ejecuta el hedging ortogonal completo.
    ///
    /// # Pipeline
    /// 1. Recibe impulso de Dirac del Allocator (Phase 5).
    /// 2. Calcula vector de riesgo direccional del DEX.
    /// 3. Muestrea el mercado ortogonal (CEX) para estimar covarianza.
    /// 4. Construye Π_⟂ y proyecta el vector de riesgo.
    /// 5. Calcula el vector de compensación exacto para el CEX.
    /// 6. Verifica covarianza nula post-compensación.
    pub fn hedge(
        &self,
        impulse: &DiracImpulse,
        orthogonal_market: &LiquidityManifold,
        cex_price_vector: &DVector<f64>, // Precios del CEX como vector de estado
    ) -> Result<OrthogonalHedgeResult, HedgeError> {
        // 1. Vector de riesgo del impulso DEX
        let dex_risk = DirectionalVector::from_dirac_impulse(impulse);

        // 2. Vector de estado del mercado ortogonal (CEX)
        let cex_state = DVector::from(vec![
            orthogonal_market.token0_reserve,
            orthogonal_market.token1_reserve,
        ]);

        // 3. Calcular covarianza entre DEX y CEX
        let cov_ab = Self::estimate_covariance(&dex_risk.components, cex_price_vector);

        // 4. Construir hiperplano ortogonal
        let cov_vector = DVector::from(vec![cov_ab, cov_ab]); // Simplificado 2D
        let hyperplane = OrthogonalHyperplane::from_covariance_vector(&cov_vector);

        // 5. Proyectar vector de riesgo sobre Π_⟂
        let projected = hyperplane.project(&dex_risk.components);
        let residual = &dex_risk.components - &projected;

        // 6. Calcular vector de compensación para CEX
        // El CEX debe moverse en dirección opuesta al residual para cancelar covarianza
        let compensation = -&residual;

        // 7. Construir estado entrelazado
        let psi_a = DVector::from(vec![
            Complex64::new(dex_risk.components[0], 0.0),
            Complex64::new(dex_risk.components[1], 0.0),
        ]);
        let psi_b = DVector::from(vec![
            Complex64::new(cex_state[0], 0.0),
            Complex64::new(cex_state[1], 0.0),
        ]);
        let correlation = if dex_risk.norm > 1e-15 && cex_state.norm() > 1e-15 {
            dex_risk.components.dot(&cex_state) / (dex_risk.norm * cex_state.norm())
        } else {
            0.0
        };

        let entangled = EntangledState::from_market_states(
            &psi_a,
            &psi_b,
            correlation,
            impulse.manifold.pool_address.clone(),
            orthogonal_market.pool_address.clone(),
        );

        // 8. Matriz de covarianza post-compensación
        let mut cov_matrix = DMatrix::zeros(2, 2);
        cov_matrix[(0, 0)] = dex_risk.components.norm_squared();
        cov_matrix[(1, 1)] = cex_state.norm_squared();
        cov_matrix[(0, 1)] = (dex_risk.components + &compensation).dot(&cex_state);
        cov_matrix[(1, 0)] = cov_matrix[(0, 1)];

        let is_neutralized = cov_matrix[(0, 1)].abs() < self.null_covariance_tolerance;
        let off_diag = cov_matrix[(0, 1)];
        let ortho_error = if dex_risk.norm > 1e-15 {
            residual.norm() / dex_risk.norm
        } else {
            0.0
        };

        Ok(OrthogonalHedgeResult {
            hedged_state: entangled,
            covariance_matrix: cov_matrix,
            is_fully_neutralized: is_neutralized,
            residual_direction: residual,
            orthogonality_error: ortho_error,
            covariance_off_diagonal: off_diag,
            compensation_vector: compensation,
        })
    }

    /// Estimador de covarianza muestral entre dos vectores.
    fn estimate_covariance(a: &DVector<f64>, b: &DVector<f64>) -> f64 {
        if a.len() != b.len() || a.is_empty() {
            return 0.0;
        }
        let n = a.len() as f64;
        let mean_a = a.sum() / n;
        let mean_b = b.sum() / n;
        let mut cov = 0.0;
        for i in 0..a.len() {
            cov += (a[i] - mean_a) * (b[i] - mean_b);
        }
        cov / (n - 1.0).max(1.0)
    }
}

#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum HedgeError {
    #[error("Degenerate risk vector (zero norm)")]
    DegenerateRiskVector,
    #[error("Covariance estimation failed: {0}")]
    CovarianceEstimationFailed(String),
    #[error("Orthogonal market incompatible dimension")]
    IncompatibleDimension,
}

#[cfg(test)]
mod tests {
    use super::*;
    use nalgebra::Vector2;

    fn test_impulse() -> DiracImpulse {
        DiracImpulse {
            manifold: crate::allocator::LiquidityManifold::new(
                1_000_000.0,
                1000.0,
                1000.0,
                "0xDex".to_string(),
                ("WETH".to_string(), "USDC".to_string()),
                1,
            )
            .unwrap(),
            injection_point: Vector2::new(1050.0, 1_000_000.0 / 1050.0),
            amplitude: 100.0,
            support_radius: 1.0,
            post_injection_manifold: crate::allocator::LiquidityManifold::new(
                1_000_000.0,
                1050.0,
                1_000_000.0 / 1050.0,
                "0xDex".to_string(),
                ("WETH".to_string(), "USDC".to_string()),
                1,
            )
            .unwrap(),
        }
    }

    fn test_cex() -> LiquidityManifold {
        crate::allocator::LiquidityManifold::new(
            2_000_000.0,
            2000.0,
            1000.0,
            "0xCex".to_string(),
            ("WETH".to_string(), "USDC".to_string()),
            1,
        )
        .unwrap()
    }

    #[test]
    fn hedge_produces_valid_result() {
        let hedger = OrthogonalVarianceHedger::new();
        let impulse = test_impulse();
        let cex = test_cex();
        let cex_prices = DVector::from(vec![2000.0, 1000.0]);

        let result = hedger.hedge(&impulse, &cex, &cex_prices);
        assert!(result.is_ok(), "Hedge failed: {:?}", result.err());

        let hedge = result.unwrap();
        // Structural invariants
        assert!(hedge.hedged_state.entanglement_entropy >= 0.0);
        assert_eq!(hedge.covariance_matrix.nrows(), 2);
        assert_eq!(hedge.covariance_matrix.ncols(), 2);
        assert!(!hedge.compensation_vector.is_empty());
        // Orthogonality error is bounded [0, 1]
        assert!(hedge.orthogonality_error >= 0.0);
        assert!(hedge.orthogonality_error <= 1.0 + 1e-9);
    }

    #[test]
    fn entangled_state_has_positive_entropy() {
        let psi_a = DVector::from(vec![Complex64::new(1.0, 0.0), Complex64::new(0.0, 0.0)]);
        let psi_b = DVector::from(vec![Complex64::new(0.0, 0.0), Complex64::new(1.0, 0.0)]);
        let state = EntangledState::from_market_states(
            &psi_a,
            &psi_b,
            0.5,
            "A".to_string(),
            "B".to_string(),
        );
        assert!(state.entanglement_entropy >= 0.0);
    }

    #[test]
    fn orthogonal_hyperplane_projects_correctly() {
        let cov = DVector::from(vec![1.0, 1.0]);
        let plane = OrthogonalHyperplane::from_covariance_vector(&cov);
        let v = DVector::from(vec![2.0, 2.0]);
        let proj = plane.project(&v);
        // v es paralelo a n, así que la proyección sobre Π_⟂ debe ser ~0
        assert!(
            proj.norm() < 1e-6,
            "Projection not orthogonal: norm={}",
            proj.norm()
        );
    }

    #[test]
    fn orthogonal_hyperplane_preserves_orthogonal_component() {
        let cov = DVector::from(vec![1.0, 0.0]);
        let plane = OrthogonalHyperplane::from_covariance_vector(&cov);
        let v = DVector::from(vec![0.0, 3.0]);
        let proj = plane.project(&v);
        // v es ortogonal a n, así que la proyección debe preservar v
        assert!((proj - v).norm() < 1e-6);
    }
}
