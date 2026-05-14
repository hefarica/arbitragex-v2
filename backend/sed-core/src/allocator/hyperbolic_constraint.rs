//! Restricción hiperbólica del CPMM: x·y = k
//!
//! Invariante de runtime: |x·y − k| / k < tolerance

use serde::{Deserialize, Serialize};

/// Verificador de la restricción hiperbólica x·y = k.
/// Esta es la INVARIANTE INQUEBRANTABLE del allocator.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct HyperbolicConstraint {
    /// Producto constante k
    pub constant_product: f64,
    /// Tolerancia relativa: |x·y − k| / k < tolerance
    pub tolerance: f64,
}

impl HyperbolicConstraint {
    /// Tolerancia por defecto: 1e-9 (0.0000001%)
    pub const DEFAULT_TOLERANCE: f64 = 1e-9;

    /// Construye un verificador con tolerancia por defecto
    pub fn new(constant_product: f64) -> Self {
        Self {
            constant_product,
            tolerance: Self::DEFAULT_TOLERANCE,
        }
    }

    /// Construye con tolerancia custom
    pub fn with_tolerance(constant_product: f64, tolerance: f64) -> Self {
        Self {
            constant_product,
            tolerance,
        }
    }

    /// Verifica que (x, y) satisfaga la restricción hiperbólica
    pub fn verify(&self, x: f64, y: f64) -> bool {
        let product = x * y;
        let rel_err = (product - self.constant_product).abs() / self.constant_product;
        rel_err < self.tolerance
    }

    /// Verifica y retorna el error relativo (para logging/métricas)
    pub fn verify_with_error(&self, x: f64, y: f64) -> (bool, f64) {
        let product = x * y;
        let rel_err = (product - self.constant_product).abs() / self.constant_product;
        (rel_err < self.tolerance, rel_err)
    }

    /// Calcula la penalización Lagrangiana: L = ½·λ·(x·y − k)²
    pub fn lagrangian_penalty(&self, x: f64, y: f64, lambda: f64) -> f64 {
        let violation = x * y - self.constant_product;
        0.5 * lambda * violation * violation
    }

    /// Gradiente de la penalización respecto a (x, y)
    pub fn lagrangian_gradient(&self, x: f64, y: f64, lambda: f64) -> (f64, f64) {
        let violation = x * y - self.constant_product;
        let grad_x = lambda * violation * y;
        let grad_y = lambda * violation * x;
        (grad_x, grad_y)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn verify_exact_match() {
        let c = HyperbolicConstraint::new(1000.0);
        assert!(c.verify(10.0, 100.0));
    }

    #[test]
    fn verify_with_tiny_violation() {
        let c = HyperbolicConstraint::new(1000.0);
        assert!(c.verify(10.0, 100.0000000001));
    }

    #[test]
    fn verify_rejects_large_violation() {
        let c = HyperbolicConstraint::new(1000.0);
        assert!(!c.verify(10.0, 101.0));
    }

    #[test]
    fn lagrangian_penalty_zero_when_exact() {
        let c = HyperbolicConstraint::new(1000.0);
        let penalty = c.lagrangian_penalty(10.0, 100.0, 1e6);
        assert!(penalty.abs() < 1e-6);
    }

    #[test]
    fn lagrangian_gradient_zero_when_exact() {
        let c = HyperbolicConstraint::new(1000.0);
        let (gx, gy) = c.lagrangian_gradient(10.0, 100.0, 1e6);
        assert!(gx.abs() < 1e-6);
        assert!(gy.abs() < 1e-6);
    }
}
