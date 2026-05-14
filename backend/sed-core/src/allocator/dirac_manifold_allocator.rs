//! Dirac Manifold Allocator — Inyección de funciones impulso sobre ℒₖ
//!
//! Orquestador de Phase 5. Consume Phase 3/4 outputs y produce el
//! DiracImpulse regularizado + BundlePosition<DiracImpulseOnly> sellado.

use nalgebra::Vector2;
use crate::types::bundle_position::{BundlePosition, DiracImpulseOnly};
use crate::types::errors::TopologyValidationError;
use crate::eigenstate::{EquilibriumBoundary, EigenState};
use crate::allocator::{
    LiquidityManifold, HyperbolicConstraint, PontryaginSolver,
    OptimalControlSolution, OcpError, ManifoldError,
};

/// Función impulso de Dirac regularizada sobre una variedad de liquidez
#[derive(Debug, Clone, PartialEq)]
pub struct DiracImpulse {
    /// Variedad de liquidez objetivo
    pub manifold: LiquidityManifold,
    /// Punto de inyección (x₀, y₀) en la variedad
    pub injection_point: Vector2<f64>,
    /// Amplitud del impulso (cantidad de energía/liquidez inyectada)
    pub amplitude: f64,
    /// Radio de soporte de la regularización gaussiana (ε)
    pub support_radius: f64,
    /// Variedad post-inyección
    pub post_injection_manifold: LiquidityManifold,
}

impl DiracImpulse {
    /// Radio de soporte por defecto: 0.1% de la reserva
    pub const DEFAULT_SUPPORT_RADIUS_FACTOR: f64 = 0.001;

    /// Evaluación regularizada de la delta de Dirac
    /// δ_ε(p) = (1/ε√π) · exp(-||p - p₀||²_g / ε²)
    pub fn evaluate(&self, point: &Vector2<f64>) -> f64 {
        let distance = self.geodesic_distance(point);
        if distance >= self.support_radius {
            return 0.0;
        }
        let norm = 1.0 / (self.support_radius * std::f64::consts::PI.sqrt());
        norm * (-distance.powi(2) / self.support_radius.powi(2)).exp()
    }

    /// Distancia geodésica inducida por la métrica hiperbólica
    fn geodesic_distance(&self, point: &Vector2<f64>) -> f64 {
        let dx = point.x - self.injection_point.x;
        let dy = point.y - self.injection_point.y;
        let gxx = 1.0 / (self.manifold.token0_reserve * self.manifold.token0_reserve);
        let gyy = 1.0 / (self.manifold.token1_reserve * self.manifold.token1_reserve);
        (gxx * dx * dx + gyy * dy * dy).sqrt()
    }

    /// Energía total del impulso
    pub fn total_energy(&self) -> f64 {
        self.amplitude
    }

    /// Verifica la restricción hiperbólica post-inyección
    pub fn verify_hyperbolic_constraint(&self, tolerance: f64) -> bool {
        let x = self.post_injection_manifold.token0_reserve;
        let y = self.post_injection_manifold.token1_reserve;
        let k = self.manifold.constant_product;
        (x * y - k).abs() / k < tolerance
    }
}

/// Allocator principal de Phase 5
pub struct DiracManifoldAllocator {
    /// Solver de control óptimo
    pub solver: PontryaginSolver,
    /// Restricción hiperbólica compartida
    pub constraint: HyperbolicConstraint,
}

impl DiracManifoldAllocator {
    /// Construye el allocator
    pub fn new(constraint: HyperbolicConstraint, pool_fee_bps: u32) -> Self {
        Self {
            solver: PontryaginSolver::new(constraint, pool_fee_bps),
            constraint,
        }
    }

    /// Asignación completa desde eigenstate → impulso Dirac → bundle sellado
    pub fn allocate(
        &self,
        manifold: &LiquidityManifold,
        boundary: &EquilibriumBoundary,
        equilibrium_state: &EigenState,
    ) -> Result<AllocationResult, AllocationError> {
        let target = Self::extract_target_from_eigenstate(equilibrium_state, manifold)?;

        let ocp_solution = self.solver.solve(manifold, boundary, &target)
            .map_err(AllocationError::OptimalControlFailed)?;

        if !ocp_solution.hyperbolic_constraint_satisfied {
            return Err(AllocationError::HyperbolicInvariantViolation {
                violation: ocp_solution.hyperbolic_violation,
            });
        }

        let amplitude = ocp_solution.optimal_control.norm();
        let support_radius = Self::compute_support_radius(manifold, &ocp_solution);

        let impulse = DiracImpulse {
            manifold: manifold.clone(),
            injection_point: ocp_solution.injection_point,
            amplitude,
            support_radius,
            post_injection_manifold: ocp_solution.post_injection_manifold.clone(),
        };

        if !impulse.verify_hyperbolic_constraint(self.constraint.tolerance) {
            return Err(AllocationError::HyperbolicInvariantViolation {
                violation: (impulse.post_injection_manifold.token0_reserve
                    * impulse.post_injection_manifold.token1_reserve
                    - manifold.constant_product).abs()
                    / manifold.constant_product,
            });
        }

        let bundle = BundlePosition::new_dirac_impulse_only(
            manifold.pool_address.clone(),
            manifold.token_pair.clone(),
            amplitude,
            ocp_solution.value_functional.abs(),
            &ocp_solution,
        ).map_err(AllocationError::TopologyValidationFailed)?;

        Ok(AllocationResult { bundle, impulse, ocp_solution })
    }

    /// Asignación simplificada por precio objetivo
    pub fn allocate_for_target_price(
        &self,
        manifold: &LiquidityManifold,
        target_price: f64,
    ) -> Result<AllocationResult, AllocationError> {
        let ocp_solution = self.solver.solve_for_target_price(manifold, target_price)
            .map_err(AllocationError::OptimalControlFailed)?;

        if !ocp_solution.hyperbolic_constraint_satisfied {
            return Err(AllocationError::HyperbolicInvariantViolation {
                violation: ocp_solution.hyperbolic_violation,
            });
        }

        let amplitude = ocp_solution.optimal_control.norm();
        let support_radius = Self::compute_support_radius(manifold, &ocp_solution);

        let impulse = DiracImpulse {
            manifold: manifold.clone(),
            injection_point: ocp_solution.injection_point,
            amplitude,
            support_radius,
            post_injection_manifold: ocp_solution.post_injection_manifold.clone(),
        };

        let bundle = BundlePosition::new_dirac_impulse_only(
            manifold.pool_address.clone(),
            manifold.token_pair.clone(),
            amplitude,
            ocp_solution.value_functional.abs(),
            &ocp_solution,
        ).map_err(AllocationError::TopologyValidationFailed)?;

        Ok(AllocationResult { bundle, impulse, ocp_solution })
    }

    fn extract_target_from_eigenstate(
        eigenstate: &EigenState,
        manifold: &LiquidityManifold,
    ) -> Result<Vector2<f64>, AllocationError> {
        let amp = &eigenstate.amplitude;
        if amp.len() < 2 {
            return Err(AllocationError::InvalidEigenstateDimension);
        }
        let norm: f64 = amp.iter().map(|c| c.norm_sqr()).sum::<f64>().sqrt();
        if norm < 1e-15 {
            return Err(AllocationError::DegenerateEigenstate);
        }
        // Extract directional weight ratios from the eigenstate
        let w0 = amp[0].norm() / norm;
        let w1 = amp[1].norm() / norm;
        // Compute raw target from eigenstate weights
        let x_raw = w0 * manifold.token0_reserve;
        let y_raw = w1 * manifold.token1_reserve;
        // Project onto hyperbola x·y = k:
        // Target price from eigenstate: p* = y_raw / x_raw
        // Then x* = sqrt(k / p*), y* = sqrt(k * p*)
        let k = manifold.constant_product;
        if x_raw < 1e-15 {
            return Err(AllocationError::DegenerateEigenstate);
        }
        let price = y_raw / x_raw;
        let x_star = (k / price).sqrt();
        let y_star = (k * price).sqrt();
        Ok(Vector2::new(x_star, y_star))
    }

    fn compute_support_radius(
        manifold: &LiquidityManifold,
        ocp: &OptimalControlSolution,
    ) -> f64 {
        let control_norm = ocp.optimal_control.norm();
        let reserve_norm = (manifold.token0_reserve * manifold.token0_reserve
            + manifold.token1_reserve * manifold.token1_reserve).sqrt();
        let radius = DiracImpulse::DEFAULT_SUPPORT_RADIUS_FACTOR
            * reserve_norm.max(control_norm);
        radius.max(1e-9)
    }
}

/// Resultado completo de una asignación
#[derive(Debug, Clone, PartialEq)]
pub struct AllocationResult {
    pub bundle: BundlePosition<DiracImpulseOnly>,
    pub impulse: DiracImpulse,
    pub ocp_solution: OptimalControlSolution,
}

#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum AllocationError {
    #[error("Optimal control solver failed: {0}")]
    OptimalControlFailed(OcpError),
    #[error("Hyperbolic invariant violated: violation={violation:e}")]
    HyperbolicInvariantViolation { violation: f64 },
    #[error("Topology validation failed: {0}")]
    TopologyValidationFailed(TopologyValidationError),
    #[error("Invalid eigenstate dimension (need >= 2)")]
    InvalidEigenstateDimension,
    #[error("Degenerate eigenstate (zero norm)")]
    DegenerateEigenstate,
    #[error("Manifold error: {0}")]
    ManifoldFailed(#[from] ManifoldError),
}

#[cfg(test)]
mod tests {
    use super::*;
    use num_complex::Complex64;
    use nalgebra::DVector;

    fn test_manifold() -> LiquidityManifold {
        LiquidityManifold::new(
            1_000_000.0, 1000.0, 1000.0,
            "0xPool".to_string(),
            ("WETH".to_string(), "USDC".to_string()), 1,
        ).unwrap()
    }

    fn test_boundary() -> EquilibriumBoundary {
        EquilibriumBoundary {
            boundary_hypersurface: vec![
                DVector::from(vec![900.0, 1111.111]),
                DVector::from(vec![1000.0, 1000.0]),
                DVector::from(vec![1100.0, 909.091]),
            ],
            critical_probability: 0.95,
            convergence_radius: f64::INFINITY,
            stability_exponent: 0.0,
        }
    }

    fn test_eigenstate() -> EigenState {
        // Asymmetric amplitudes: |ψ₀|=0.6, |ψ₁|=0.8 → price hint ≈ (0.8/0.6)·(1000/1000)
        // This drives the target away from spot, requiring non-zero allocation.
        EigenState {
            amplitude: DVector::from(vec![
                Complex64::new(0.6, 0.0),
                Complex64::new(0.8, 0.0),
            ]),
            energy: 1.0,
            degeneracy: 1,
            quantum_numbers: vec![0, 1],
        }
    }

    #[test]
    fn allocate_full_pipeline() {
        let manifold = test_manifold();
        let boundary = test_boundary();
        let eigenstate = test_eigenstate();
        let allocator = DiracManifoldAllocator::new(
            HyperbolicConstraint::new(1_000_000.0), 30,
        );
        let result = allocator.allocate(&manifold, &boundary, &eigenstate);
        assert!(result.is_ok(), "Allocation failed: {:?}", result.err());
        let alloc = result.unwrap();
        // Non-trivial allocation (eigenstate encodes different equilibrium)
        assert!(alloc.impulse.amplitude > 0.0);
        assert!(alloc.ocp_solution.hyperbolic_constraint_satisfied);
        assert!(alloc.impulse.verify_hyperbolic_constraint(1e-9));
    }

    #[test]
    fn allocate_for_target_price_pipeline() {
        let manifold = test_manifold();
        let allocator = DiracManifoldAllocator::new(
            HyperbolicConstraint::new(1_000_000.0), 30,
        );
        let result = allocator.allocate_for_target_price(&manifold, 1.5);
        assert!(result.is_ok(), "Price allocation failed: {:?}", result.err());
        let alloc = result.unwrap();
        let realized_price = alloc.impulse.injection_point.y / alloc.impulse.injection_point.x;
        assert!((realized_price - 1.5).abs() < 1e-4);
    }

    #[test]
    fn dirac_impulse_evaluates_zero_outside_support() {
        let manifold = test_manifold();
        let impulse = DiracImpulse {
            manifold: manifold.clone(),
            injection_point: Vector2::new(1000.0, 1000.0),
            amplitude: 100.0,
            support_radius: 1.0,
            post_injection_manifold: manifold,
        };
        let far_point = Vector2::new(2000.0, 2000.0);
        assert_eq!(impulse.evaluate(&far_point), 0.0);
    }

    #[test]
    fn allocate_rejects_degenerate_eigenstate() {
        let manifold = test_manifold();
        let boundary = test_boundary();
        let bad = EigenState {
            amplitude: DVector::from(vec![
                Complex64::new(0.0, 0.0),
                Complex64::new(0.0, 0.0),
            ]),
            energy: 0.0, degeneracy: 1, quantum_numbers: vec![],
        };
        let allocator = DiracManifoldAllocator::new(
            HyperbolicConstraint::new(1_000_000.0), 30,
        );
        let result = allocator.allocate(&manifold, &boundary, &bad);
        assert!(matches!(result, Err(AllocationError::DegenerateEigenstate)));
    }
}
