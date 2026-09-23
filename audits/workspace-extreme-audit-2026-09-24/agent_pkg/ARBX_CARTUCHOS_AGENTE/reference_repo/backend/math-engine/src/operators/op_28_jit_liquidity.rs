//! FUSILE: Implementacion propia -- Liquidez Justo a Tiempo (JIT)
//! Categoria: finance
//!
//! Ephemeral-liquidity decay model L(t) = L_target * exp(-k*t). Each reserve
//! entry is one snapshot; L_i = sqrt(r0_i * r1_i) (CPMM liquidity invariant),
//! y_i = ln L_i linearized to y = ln L_target - k*t and fit by OLS over the
//! sample index. scalar_value = k (decay constant; k > 0 required).
//! features["jit_decay_rate"], when present and positive, overrides the OLS fit.
//!
//! R8 fail-honest: None when reserves are empty, any venue is degenerate
//! (L_i <= 0), the regression is singular (n < 2), or no decay is detected
//! (k <= 0). A non-decaying manifold is reported None, never a fabricated k.

use super::{MarketState, OperatorOutput, TopologicalOperator};
use std::collections::HashMap;

#[derive(Default)]
pub struct JitLiquidityOperator;

impl JitLiquidityOperator {
    pub fn new() -> Self {
        Self
    }
}

impl TopologicalOperator for JitLiquidityOperator {
    fn id(&self) -> u8 {
        28
    }

    fn name(&self) -> &'static str {
        "JIT Liquidity"
    }

    fn category(&self) -> &'static str {
        "finance"
    }

    fn evaluate(&self, state: &MarketState) -> OperatorOutput {
        let mut metadata = HashMap::new();
        metadata.insert("computed".to_string(), 0.0);
        metadata.insert("n".to_string(), state.liquidity_reserves.len() as f64);

        if state.liquidity_reserves.is_empty() {
            metadata.insert("reason_no_reserves".to_string(), 1.0);
            return none_output(self.id(), self.name(), metadata);
        }

        // Any degenerate venue makes y_i = ln L_i undefined.
        for (r0, r1) in &state.liquidity_reserves {
            if !r0.is_finite() || !r1.is_finite() || *r0 <= 0.0 || *r1 <= 0.0 {
                metadata.insert("reason_degenerate_venue".to_string(), 1.0);
                return none_output(self.id(), self.name(), metadata);
            }
        }

        // Decay constant k: upstream feature override, else OLS fit on ln L.
        let (k, l_target_fit) = match state.features.get("jit_decay_rate").copied() {
            Some(feat_k) if feat_k.is_finite() && feat_k > 0.0 => (feat_k, None),
            _ => {
                let n = state.liquidity_reserves.len();
                if n < 2 {
                    metadata.insert("reason_insufficient_samples".to_string(), 1.0);
                    return none_output(self.id(), self.name(), metadata);
                }
                let ys: Vec<f64> = state
                    .liquidity_reserves
                    .iter()
                    .map(|(r0, r1)| (r0 * r1).ln() * 0.5)
                    .collect();
                let n_f = n as f64;
                let sum_t: f64 = (0..n).map(|i| i as f64).sum();
                let sum_t2: f64 = (0..n).map(|i| (i * i) as f64).sum();
                let sum_y: f64 = ys.iter().sum();
                let sum_ty: f64 = (0..n).map(|i| i as f64 * ys[i]).sum();
                let t_bar = sum_t / n_f;
                let y_bar = sum_y / n_f;
                let denom = sum_t2 - sum_t * sum_t / n_f;
                if !denom.is_finite() || denom <= 0.0 {
                    metadata.insert("reason_singular_fit".to_string(), 1.0);
                    return none_output(self.id(), self.name(), metadata);
                }
                let slope = (sum_ty - sum_t * sum_y / n_f) / denom;
                let k = -slope;
                if !k.is_finite() || k <= 0.0 {
                    metadata.insert("reason_no_decay".to_string(), 1.0);
                    metadata.insert("k_raw".to_string(), k);
                    return none_output(self.id(), self.name(), metadata);
                }
                let l_target = (y_bar + k * t_bar).exp();
                (k, Some(l_target))
            }
        };

        // Peak liquidity: OLS-intercept estimate, else first snapshot.
        let l_target = l_target_fit.unwrap_or_else(|| {
            let (r0, r1) = state.liquidity_reserves[0];
            (r0 * r1).sqrt()
        });
        let t_half = 2.0f64.ln() / k;

        metadata.insert("computed".to_string(), 1.0);
        metadata.insert("k".to_string(), k);
        metadata.insert("l_target".to_string(), l_target);
        metadata.insert("t_half".to_string(), t_half);

        // Validity window T_valid = (1/k) ln(L_target / L_min).
        if let Some(&l_min) = state.features.get("min_liquidity") {
            if l_min.is_finite() && l_min > 0.0 && l_target > l_min {
                metadata.insert("t_valid".to_string(), (l_target / l_min).ln() / k);
            } else {
                metadata.insert("t_valid_invalid".to_string(), 1.0);
            }
        }

        OperatorOutput {
            operator_id: self.id(),
            operator_name: self.name().to_string(),
            scalar_value: Some(k),
            vector_result: Some(vec![k, l_target, t_half]),
            matrix_result: None,
            metadata,
        }
    }
}

fn none_output(
    operator_id: u8,
    operator_name: &'static str,
    metadata: HashMap<String, f64>,
) -> OperatorOutput {
    OperatorOutput {
        operator_id,
        operator_name: operator_name.to_string(),
        scalar_value: None,
        vector_result: None,
        matrix_result: None,
        metadata,
    }
}
