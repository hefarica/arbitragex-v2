//! FUSILE: Implementacion propia -- Newton-Raphson
//! Halla la raíz de f(x)=0: el tamaño de break-even donde el Topological Yield
//! neto se anula sobre la curva CPMM (slippage/Decoherencia embebido en la
//! curva, como dictamina op_21: "algebraically reduces to"):
//!   gross_yield(x) = r1·γ·x / (r0 + γ·x) − x
//!   f(x)   = gross_yield(x) − gas − break_even_target   (target = 0 ⇒ break-even)
//!   f'(x)  = r1·γ·r0 / (r0 + γ·x)² − 1                  (derivada analítica)
//!   x_{k+1} = x_k − f(x_k) / f'(x_k)
//!
//! Sembrado robusto: x₀ = ½·(gas+target)/(γ·p_pool − 1) = ½·x_lin, donde x_lin
//! es la estimación lineal del break-even. Como f es estrictamente cóncva
//! (f''<0), f(x) ≤ tangente en 0 = f_lin(x), y por tanto x* ≥ x_lin > x₀: la
//! semilla queda estrictamente bajo la raíz con f(x₀) < 0 ⇒ Newton converge
//! monótonamente (x₀ < … < x_{k} ≤ x*) sin overshoot, y f' > f'(x*) > 0 en todo
//! paso (nunca se anula salvo x*→x_peak, caso legítimamente no admitible).
//! gas = gas_price_gwei · 21000 · 1e-9 · p_ref.
//! Categoria: numerical
//!
//! R8 fail-honest: sin reservas, r0≤0, r1≤0, γ≤0, sin edge (x_peak≤0), raíz no
//! rentable (f(x_peak)≤0), |f'|<1e-12 (divergencia), no-convergencia en 50 iters,
//! o raíz fuera de (0, r0] ⇒ scalar_value None.

use super::{MarketState, OperatorOutput, TopologicalOperator};
use std::collections::HashMap;

#[derive(Default)]
pub struct NewtonOperator;

impl NewtonOperator {
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

    /// Fee en bps (features["fee_bps"]/1e4) o fracción directa; default 0.003.
    fn fee_fraction(state: &MarketState) -> f64 {
        state
            .features
            .get("fee_bps")
            .map(|bps| *bps / 10_000.0)
            .or_else(|| state.features.get("pool_fee").copied())
            .unwrap_or(0.003)
    }
}

impl TopologicalOperator for NewtonOperator {
    fn id(&self) -> u8 {
        21
    }

    fn name(&self) -> &'static str {
        "Newton-Raphson"
    }

    fn category(&self) -> &'static str {
        "numerical"
    }

    fn evaluate(&self, state: &MarketState) -> OperatorOutput {
        let none_out = |reason: &str| OperatorOutput {
            operator_id: 21,
            operator_name: "Newton-Raphson".to_string(),
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

        if state.liquidity_reserves.is_empty() {
            return none_out("no_reserves");
        }
        let (r0, r1) = state.liquidity_reserves[0];
        if !r0.is_finite() || !r1.is_finite() || r0 <= 0.0 || r1 <= 0.0 {
            return none_out("degenerate_pool");
        }

        let fee = Self::fee_fraction(state);
        let gamma = 1.0 - fee;
        if !gamma.is_finite() || gamma <= 0.0 {
            return none_out("invalid_fee");
        }

        let price = Self::reference_price(state).unwrap_or(r1 / r0);
        if !price.is_finite() || price <= 0.0 {
            return none_out("invalid_price");
        }
        let gas = state.gas_price_gwei * 21_000.0 * 1e-9 * price;
        let break_even_target = state
            .features
            .get("break_even_target")
            .copied()
            .filter(|v| v.is_finite())
            .unwrap_or(0.0);

        // f(x)  = r1·γ·x/(r0+γ·x) − x − gas − break_even_target
        // f'(x) = r1·γ·r0/(r0+γ·x)² − 1
        let f = |x: f64| -> f64 {
            let denom = r0 + gamma * x;
            if denom <= 0.0 {
                return f64::INFINITY;
            }
            (r1 * gamma * x) / denom - x - gas - break_even_target
        };
        let df = |x: f64| -> f64 {
            let denom = r0 + gamma * x;
            if denom <= 0.0 {
                return f64::INFINITY;
            }
            (r1 * gamma * r0) / (denom * denom) - 1.0
        };

        let p_pool = r1 / r0;

        // (1) Edge: x_peak = (sqrt(r1·γ·r0) − r0)/γ > 0  ⇔  γ·p_pool > 1.
        let x_peak = ((r1 * gamma * r0).sqrt() - r0) / gamma;
        if !x_peak.is_finite() || x_peak <= 0.0 {
            return none_out("no_edge");
        }
        // (2) Rentabilidad en el pico: debe existir x donde f > 0.
        if f(x_peak) <= 0.0 {
            return none_out("no_profitable_root");
        }
        // (3) f(0) = −(gas+target) < 0; si no, la raíz está en el origen o antes.
        let offset = gas + break_even_target;
        if offset.partial_cmp(&0.0) != Some(std::cmp::Ordering::Greater) {
            return none_out("root_at_origin");
        }

        // Semilla x₀ = ½·x_lin < x* (demostrado vía concavidad). x_lin = offset/(γ·p_pool−1).
        let mut x = 0.5 * offset / (gamma * p_pool - 1.0);
        if !x.is_finite() || x <= 0.0 {
            x = 0.5 * x_peak; // fallback conservador
        }

        let tol = 1e-10 * r0;
        let deriv_floor = 1e-12_f64;
        let n_max = 50_usize;
        let mut iters = 0_usize;
        let mut converged = false;
        let mut f_at_root = f(x);
        for _ in 0..n_max {
            iters += 1;
            let fp = df(x);
            if !fp.is_finite() || fp.abs() < deriv_floor {
                return none_out("deriv_singular");
            }
            let fv = f(x);
            let step = fv / fp;
            let x_next = x - step;
            // x_next debe permanecer ≥ 0 (convergencia monótona desde abajo).
            if !x_next.is_finite() || x_next < 0.0 {
                return none_out("divergence");
            }
            x = x_next;
            f_at_root = f(x);
            if step.abs() < tol {
                converged = true;
                break;
            }
        }

        if !converged {
            return none_out("non_converged");
        }
        // Raíz admisible: estrictamente positiva y dentro del bracket (0, r0].
        if !x.is_finite() || x <= 0.0 || x > r0 {
            return none_out("root_out_of_range");
        }

        let mut metadata = HashMap::new();
        metadata.insert("computed".to_string(), 1.0);
        metadata.insert("break_even_size".to_string(), x);
        metadata.insert("residual_f".to_string(), f_at_root);
        metadata.insert("iterations".to_string(), iters as f64);
        metadata.insert("x_peak".to_string(), x_peak);
        metadata.insert("gas_cost".to_string(), gas);
        metadata.insert("gamma".to_string(), gamma);
        metadata.insert("reference_price".to_string(), price);
        metadata.insert("r0".to_string(), r0);
        metadata.insert("r1".to_string(), r1);

        OperatorOutput {
            operator_id: self.id(),
            operator_name: self.name().to_string(),
            scalar_value: Some(x),
            vector_result: Some(vec![x, f_at_root, x_peak]),
            matrix_result: None,
            metadata,
        }
    }
}
