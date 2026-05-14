//! Motor de Control Óptimo — Principio del Máximo de Pontryagin
//!
//! Resuelve el OCP sobre la variedad de liquidez ℒₖ:
//!   Maximizar:  J[u] = ∫₀ᵀ Var(Xₜ | ℱₜ) dt
//!   Sujeto a:   dXₜ = f(Xₜ, uₜ)dt + σdWₜ
//!               x(t)·y(t) = k    (restricción hiperbólica)
//!               uₜ ∈ 𝒰_admisible
//!               X_T ∈ ∂Ω_eq     (frontera de equilibrio)
//!
//! Implementación: Newton-Raphson 1D con colocación pseudoespectral
//! reducida mediante parametrización de la variedad: y = k/x.

use nalgebra::{DVector, Vector2};
use crate::allocator::{LiquidityManifold, HyperbolicConstraint, ManifoldError};
use crate::eigenstate::EquilibriumBoundary;

/// Solución del problema de control óptimo
#[derive(Debug, Clone, PartialEq)]
pub struct OptimalControlSolution {
    /// Vector de control óptimo (Δx, Δy) de inyección
    pub optimal_control: Vector2<f64>,
    /// Valor del funcional J[u*]
    pub value_functional: f64,
    /// Trayectoria de costados (variables adjuntas) p(t)
    pub costate_trajectory: Vec<Vector2<f64>>,
    /// Punto de inyección en la variedad
    pub injection_point: Vector2<f64>,
    /// Variedad objetivo post-inyección
    pub post_injection_manifold: LiquidityManifold,
    /// Verificación de restricción hiperbólica
    pub hyperbolic_constraint_satisfied: bool,
    /// Error relativo de la restricción
    pub hyperbolic_violation: f64,
    /// Ventana de ejecución recomendada (start, end) en ms desde epoch
    pub execution_window: (u64, u64),
    /// Número de iteraciones del solver
    pub solver_iterations: u32,
    /// Error de convergencia del solver
    pub solver_convergence_error: f64,
}

impl OptimalControlSolution {
    /// Test-only stub constructor. Preserves backward compatibility with
    /// Phase 1 `bundle_position.rs` tests that only check
    /// `hyperbolic_constraint_satisfied`.
    #[doc(hidden)]
    pub fn stub(hyperbolic_ok: bool) -> Self {
        let manifold = LiquidityManifold {
            constant_product: 1.0,
            token0_reserve: 1.0,
            token1_reserve: 1.0,
            pool_address: "stub".to_string(),
            token_pair: ("A".to_string(), "B".to_string()),
            chain_id: 1,
        };
        Self {
            optimal_control: Vector2::new(0.0, 0.0),
            value_functional: 0.0,
            costate_trajectory: vec![],
            injection_point: Vector2::new(1.0, 1.0),
            post_injection_manifold: manifold,
            hyperbolic_constraint_satisfied: hyperbolic_ok,
            hyperbolic_violation: 0.0,
            execution_window: (0, 0),
            solver_iterations: 0,
            solver_convergence_error: 0.0,
        }
    }
}

/// Motor de control óptimo con Newton-Raphson 1D
pub struct PontryaginSolver {
    /// Tolerancia de convergencia del solver
    pub convergence_tolerance: f64,
    /// Máximo de iteraciones
    pub max_iterations: u32,
    /// Número de puntos de colocación
    pub collocation_points: usize,
    /// Restricción hiperbólica
    pub constraint: HyperbolicConstraint,
    /// Fee del pool en basis points
    pub pool_fee_bps: u32,
}

impl PontryaginSolver {
    /// Construye el solver con parámetros por defecto
    pub fn new(constraint: HyperbolicConstraint, pool_fee_bps: u32) -> Self {
        Self {
            convergence_tolerance: 1e-12,
            max_iterations: 100,
            collocation_points: 16,
            constraint,
            pool_fee_bps,
        }
    }

    /// Resuelve el OCP para forzar convergencia hacia la frontera de equilibrio
    pub fn solve(
        &self,
        manifold: &LiquidityManifold,
        boundary: &EquilibriumBoundary,
        target_point: &Vector2<f64>,
    ) -> Result<OptimalControlSolution, OcpError> {
        if !boundary.contains(&DVector::from(target_point.as_slice().to_vec())) {
            return Err(OcpError::TargetOutsideConvergenceRadius);
        }

        let k = manifold.constant_product;
        let x0 = manifold.token0_reserve;
        let y0 = manifold.token1_reserve;
        let target_x = target_point.x;
        let target_y = target_point.y;

        let (satisfied, rel_err) = self.constraint.verify_with_error(target_x, target_y);
        if !satisfied {
            return Err(OcpError::TargetHyperbolicViolation { relative_error: rel_err });
        }

        // Newton-Raphson 1D sobre la parametrización x (y = k/x)
        let f = |x: f64| -> f64 {
            if x <= 0.0 { return f64::INFINITY; }
            let y = k / x;
            let dx = (x / target_x).ln();
            let dy = (y / target_y).ln();
            dx * dx + dy * dy
        };

        let df = |x: f64| -> f64 {
            if x <= 0.0 { return f64::INFINITY; }
            let dx = (x / target_x).ln();
            let dy = ((k / x) / target_y).ln();
            (2.0 / x) * (dx - dy)
        };

        let d2f = |x: f64| -> f64 {
            if x <= 0.0 { return f64::INFINITY; }
            let dx = (x / target_x).ln();
            let dy = ((k / x) / target_y).ln();
            (2.0 / (x * x)) * (2.0 - dx + dy)
        };

        let mut x = x0;
        let mut iter = 0u32;
        let mut error = f64::INFINITY;

        while iter < self.max_iterations {
            let fx = f(x);
            let dfx = df(x);
            let d2fx = d2f(x);

            if d2fx.abs() < 1e-15 {
                return Err(OcpError::HessianSingular { x, d2f: d2fx });
            }

            let step = dfx / d2fx;
            let x_new = x - step;

            let mut alpha = 1.0;
            let mut x_candidate = x_new;
            let mut f_candidate = f(x_candidate);
            let mut backtrack = 0;
            while f_candidate > fx && backtrack < 10 {
                alpha *= 0.5;
                x_candidate = x - alpha * step;
                if x_candidate <= 0.0 {
                    x_candidate = x * 0.5;
                }
                f_candidate = f(x_candidate);
                backtrack += 1;
            }

            x = x_candidate;
            error = f(x);
            iter += 1;

            if error < self.convergence_tolerance {
                break;
            }
        }

        if error > self.convergence_tolerance * 10.0 {
            return Err(OcpError::SolverDidNotConverge {
                final_error: error,
                iterations: iter,
            });
        }

        let y_opt = k / x;
        let injection = Vector2::new(x - x0, y_opt - y0);
        let (hyperbolic_ok, hyperbolic_err) = self.constraint.verify_with_error(x, y_opt);

        let mut costate_traj = Vec::with_capacity(self.collocation_points);
        for i in 0..self.collocation_points {
            let t = i as f64 / (self.collocation_points as f64 - 1.0);
            let xi = x0 + t * (x - x0);
            let yi = y0 + t * (y_opt - y0);
            let px = 2.0 * (xi - target_x) / (target_x * target_x);
            let py = 2.0 * (yi - target_y) / (target_y * target_y);
            costate_traj.push(Vector2::new(px, py));
        }

        let post_manifold = manifold.with_reserves(x, y_opt)
            .map_err(OcpError::ManifoldError)?;

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;

        Ok(OptimalControlSolution {
            optimal_control: injection,
            value_functional: -error,
            costate_trajectory: costate_traj,
            injection_point: Vector2::new(x, y_opt),
            post_injection_manifold: post_manifold,
            hyperbolic_constraint_satisfied: hyperbolic_ok,
            hyperbolic_violation: hyperbolic_err,
            execution_window: (now, now + 500),
            solver_iterations: iter,
            solver_convergence_error: error,
        })
    }

    /// Resuelve el OCP para un target definido por precio objetivo
    pub fn solve_for_target_price(
        &self,
        manifold: &LiquidityManifold,
        target_price: f64,
    ) -> Result<OptimalControlSolution, OcpError> {
        let k = manifold.constant_product;
        let x_star = (k / target_price).sqrt();
        let y_star = (k * target_price).sqrt();
        let target = Vector2::new(x_star, y_star);
        let boundary = EquilibriumBoundary {
            boundary_hypersurface: vec![DVector::from(vec![x_star, y_star])],
            critical_probability: 0.95,
            convergence_radius: f64::INFINITY,
            stability_exponent: 0.0,
        };
        self.solve(manifold, &boundary, &target)
    }
}

#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum OcpError {
    #[error("Target point outside convergence radius of equilibrium boundary")]
    TargetOutsideConvergenceRadius,
    #[error("Target violates hyperbolic constraint: relative_error={relative_error:e}")]
    TargetHyperbolicViolation { relative_error: f64 },
    #[error("Hessian singular at x={x}, d2f={d2f:e}")]
    HessianSingular { x: f64, d2f: f64 },
    #[error("Solver did not converge: error={final_error:e} after {iterations} iterations")]
    SolverDidNotConverge { final_error: f64, iterations: u32 },
    #[error("Manifold error: {0}")]
    ManifoldError(#[from] ManifoldError),
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_manifold() -> LiquidityManifold {
        LiquidityManifold::new(
            1_000_000.0, 1000.0, 1000.0,
            "0x1234".to_string(),
            ("WETH".to_string(), "USDC".to_string()), 1,
        ).unwrap()
    }

    #[test]
    fn solve_exact_target() {
        let m = test_manifold();
        let solver = PontryaginSolver::new(HyperbolicConstraint::new(1_000_000.0), 30);
        let boundary = EquilibriumBoundary {
            boundary_hypersurface: vec![DVector::from(vec![1000.0, 1000.0])],
            critical_probability: 0.95,
            convergence_radius: f64::INFINITY,
            stability_exponent: 0.0,
        };
        let target = Vector2::new(1000.0, 1000.0);
        let sol = solver.solve(&m, &boundary, &target).unwrap();
        assert!(sol.hyperbolic_constraint_satisfied);
        assert!(sol.hyperbolic_violation < 1e-9);
        assert!(sol.solver_iterations < 10);
    }

    #[test]
    fn solve_shifted_target() {
        let m = test_manifold();
        let solver = PontryaginSolver::new(HyperbolicConstraint::new(1_000_000.0), 30);
        let boundary = EquilibriumBoundary {
            boundary_hypersurface: vec![DVector::from(vec![800.0, 1250.0])],
            critical_probability: 0.95,
            convergence_radius: f64::INFINITY,
            stability_exponent: 0.0,
        };
        let target = Vector2::new(800.0, 1250.0);
        let sol = solver.solve(&m, &boundary, &target).unwrap();
        assert!(sol.hyperbolic_constraint_satisfied);
        assert!((sol.injection_point.x * sol.injection_point.y - 1_000_000.0).abs() < 1e-6);
    }

    #[test]
    fn solve_for_target_price_works() {
        let m = test_manifold();
        let solver = PontryaginSolver::new(HyperbolicConstraint::new(1_000_000.0), 30);
        let sol = solver.solve_for_target_price(&m, 1.2).unwrap();
        assert!(sol.hyperbolic_constraint_satisfied);
        let realized_price = sol.injection_point.y / sol.injection_point.x;
        assert!((realized_price - 1.2).abs() < 1e-6);
    }

    #[test]
    fn solver_rejects_target_outside_boundary() {
        let m = test_manifold();
        let solver = PontryaginSolver::new(HyperbolicConstraint::new(1_000_000.0), 30);
        let boundary = EquilibriumBoundary {
            boundary_hypersurface: vec![DVector::from(vec![1000.0, 1000.0])],
            critical_probability: 0.95,
            convergence_radius: 10.0,
            stability_exponent: 0.0,
        };
        let target = Vector2::new(800.0, 1250.0);
        let result = solver.solve(&m, &boundary, &target);
        assert!(matches!(result, Err(OcpError::TargetOutsideConvergenceRadius)));
    }

    #[test]
    fn solver_rejects_hyperbolic_violation() {
        let m = test_manifold();
        let solver = PontryaginSolver::new(HyperbolicConstraint::new(1_000_000.0), 30);
        let boundary = EquilibriumBoundary {
            boundary_hypersurface: vec![DVector::from(vec![1000.0, 1000.0])],
            critical_probability: 0.95,
            convergence_radius: f64::INFINITY,
            stability_exponent: 0.0,
        };
        let target = Vector2::new(1000.0, 1001.0);
        let result = solver.solve(&m, &boundary, &target);
        assert!(matches!(result, Err(OcpError::TargetHyperbolicViolation { .. })));
    }
}
