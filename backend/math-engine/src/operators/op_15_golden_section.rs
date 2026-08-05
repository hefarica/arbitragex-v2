//! FUSILE: Implementacion propia -- Busqueda Seccion Aurea (Golden-Section)
//! Maximiza el Topological Yield neto en función del tamaño de operación x sobre
//! la curva CPMM (slippage/Decoherencia embebido en la curva, como dictamina
//! op_15/op_20: "slippage is embedded in the CPMM curvature"):
//!   gross_yield(x) = r1·γ·x / (r0 + γ·x) − x        (output AMM − input)
//!   f(x)           = gross_yield(x) − gas
//!   τ              = (√5 − 1) / 2  ≈ 0.61803
//!   x*             = argmax_{x ∈ [0, r0]} f(x)      (sección áurea)
//! f es estrictamente cóncva (f''<0) ⇒ x* es el maximizador interior único.
//! gas = gas_price_gwei · 21000 · 1e-9 · p_ref   (costo de gas en token1).
//! Categoria: optimization
//!
//! R8 fail-honest: liquidity_reserves vacío, r0≤0, r1≤0, γ≤0, bracket
//! degenerado, o no-convergencia ⇒ scalar_value None.

use super::{MarketState, OperatorOutput, TopologicalOperator};
use std::collections::HashMap;

#[derive(Default)]
pub struct GoldenSectionOperator;

impl GoldenSectionOperator {
    pub fn new() -> Self {
        Self
    }

    /// p_ref = media de la columna 0 de price_matrix (consenso cross-venue).
    fn reference_price(state: &MarketState) -> Option<f64> {
        let col: Vec<f64> = state
            .price_matrix
            .iter()
            .filter_map(|row| row.first().copied())
            .filter(|p| p.is_finite() && *p > 0.0)
            .collect();
        if col.is_empty() {
            return None;
        }
        Some(col.iter().sum::<f64>() / col.len() as f64)
    }

    /// Fee en bps (features["fee_bps"]/1e4) o fracción directa (features["pool_fee"]);
    /// default 0.003 (30 bps, convención Uniswap-V2).
    fn fee_fraction(state: &MarketState) -> f64 {
        state
            .features
            .get("fee_bps")
            .map(|bps| *bps / 10_000.0)
            .or_else(|| state.features.get("pool_fee").copied())
            .unwrap_or(0.003)
    }
}

impl TopologicalOperator for GoldenSectionOperator {
    fn id(&self) -> u8 {
        15
    }

    fn name(&self) -> &'static str {
        "Golden Section"
    }

    fn category(&self) -> &'static str {
        "optimization"
    }

    fn evaluate(&self, state: &MarketState) -> OperatorOutput {
        let none_out = |reason: &str| OperatorOutput {
            operator_id: 15,
            operator_name: "Golden Section".to_string(),
            scalar_value: None,
            vector_result: None,
            matrix_result: None,
            metadata: {
                let mut m = HashMap::new();
                m.insert("computed".to_string(), 0.0);
                m.insert(format!("reason_{reason}").to_string(), 1.0);
                m
            },
        };

        // Pool primario = liquidity_reserves[0].
        if state.liquidity_reserves.is_empty() {
            return none_out("no_reserves");
        }
        let (r0, r1) = state.liquidity_reserves[0];
        if !r0.is_finite() || !r1.is_finite() || r0 <= 0.0 || r1 <= 0.0 {
            return none_out("degenerate_pool");
        }

        let fee = Self::fee_fraction(state);
        let gamma = 1.0 - fee; // γ = factor de retención post-fee
        if !gamma.is_finite() || gamma <= 0.0 {
            return none_out("invalid_fee");
        }

        // Precio de referencia para costear el gas; fallback precio implícito del pool.
        let price = Self::reference_price(state).unwrap_or(r1 / r0);
        if !price.is_finite() || price <= 0.0 {
            return none_out("invalid_price");
        }

        let gas = state.gas_price_gwei * 21_000.0 * 1e-9 * price;

        // f(x) = gross_yield(x) − gas; gross_yield = r1·γ·x/(r0+γ·x) − x.
        let f = |x: f64| -> f64 {
            let denom = r0 + gamma * x;
            if denom <= 0.0 {
                return f64::NEG_INFINITY;
            }
            (r1 * gamma * x) / denom - x - gas
        };

        // Maximización por sección áurea sobre [a, b] = [0, r0].
        let mut a = 0.0_f64;
        let mut b = r0;
        if !(b > a) {
            return none_out("degenerate_bracket");
        }
        let tau = ((5.0_f64).sqrt() - 1.0) / 2.0;
        let tol = 1e-9 * r0; // tolerancia absoluta escalada al bracket
        let k_max = 200_usize;
        let mut iters = 0_usize;

        let mut x1 = b - tau * (b - a);
        let mut x2 = a + tau * (b - a);
        let mut f1 = f(x1);
        let mut f2 = f(x2);
        while (b - a) > tol && iters < k_max {
            // Maximizar: si f(x1) > f(x2) el máximo está en [a, x2]; else en [x1, b].
            if f1 > f2 {
                b = x2;
                x2 = x1;
                f2 = f1;
                x1 = b - tau * (b - a);
                f1 = f(x1);
            } else {
                a = x1;
                x1 = x2;
                f1 = f2;
                x2 = a + tau * (b - a);
                f2 = f(x2);
            }
            iters += 1;
        }

        if (b - a) > tol {
            return none_out("non_converged");
        }

        let x_star = 0.5 * (a + b);
        let f_star = f(x_star);
        if !f_star.is_finite() {
            return none_out("non_finite");
        }

        let mut metadata = HashMap::new();
        metadata.insert("computed".to_string(), 1.0);
        metadata.insert("optimal_size".to_string(), x_star);
        metadata.insert("optimal_yield".to_string(), f_star);
        metadata.insert("iterations".to_string(), iters as f64);
        metadata.insert("bracket_residual".to_string(), b - a);
        metadata.insert("gas_cost".to_string(), gas);
        metadata.insert("gamma".to_string(), gamma);
        metadata.insert("reference_price".to_string(), price);
        metadata.insert("r0".to_string(), r0);
        metadata.insert("r1".to_string(), r1);

        OperatorOutput {
            operator_id: self.id(),
            operator_name: self.name().to_string(),
            scalar_value: Some(f_star),
            vector_result: Some(vec![x_star, f_star, r0]),
            matrix_result: None,
            metadata,
        }
    }
}
