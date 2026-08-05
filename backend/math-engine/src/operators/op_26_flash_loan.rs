//! FUSILE: Implementacion propia -- Optimizador de Liquidez Flash (TLS)
//! Categoria: finance
//!
//! CPMM arbitrage: optimal Temporal-Liquidity-Superposition (flash-borrowable)
//! principal. Primary pool (r0,r1) from liquidity_reserves[0]; reference price
//! p_ref from the cross-venue reserve ratios (or price_matrix first column).
//! Fee retention gamma = 1 - fee; flash premium phi. The profit-maximizing
//! input under the constant-product invariant:
//!   x* = (1/gamma) * ( sqrt( r1*gamma*r0 / (p_ref*(1+phi)) ) - r0 )
//! Edge exists iff gamma*p_pool > (1+phi)*p_ref  (equivalently x* > 0).
//!
//! R8 fail-honest: None when reserves are empty/degenerate, p_ref undefined,
//! gamma <= 0, (1+phi) <= 0, or no edge (x* <= 0). Net Topological Yield Y_net
//! at the optimum is reported in metadata.

use super::{MarketState, OperatorOutput, TopologicalOperator};
use std::collections::HashMap;

#[derive(Default)]
pub struct FlashLoanOperator;

impl FlashLoanOperator {
    pub fn new() -> Self {
        Self
    }
}

impl TopologicalOperator for FlashLoanOperator {
    fn id(&self) -> u8 {
        26
    }

    fn name(&self) -> &'static str {
        "Flash Loan"
    }

    fn category(&self) -> &'static str {
        "finance"
    }

    fn evaluate(&self, state: &MarketState) -> OperatorOutput {
        let mut metadata = HashMap::new();
        metadata.insert("computed".to_string(), 0.0);

        // Primary pool from the first reserve entry.
        let (r0, r1) = match state.liquidity_reserves.first() {
            Some(&pair) => pair,
            None => {
                metadata.insert("reason_no_reserves".to_string(), 1.0);
                return none_output(self.id(), self.name(), metadata);
            }
        };
        if !r0.is_finite() || !r1.is_finite() || r0 <= 0.0 || r1 <= 0.0 {
            metadata.insert("reason_degenerate_pool".to_string(), 1.0);
            return none_output(self.id(), self.name(), metadata);
        }
        let p_pool = r1 / r0;

        // Fee retention and flash-loan premium.
        let fee = state.features.get("pool_fee").copied().unwrap_or(0.003);
        let gamma = 1.0 - fee;
        if !gamma.is_finite() || gamma <= 0.0 {
            metadata.insert("reason_invalid_fee".to_string(), 1.0);
            return none_output(self.id(), self.name(), metadata);
        }
        let phi = state.features.get("flash_premium").copied().unwrap_or(0.0);
        let repayment = 1.0 + phi;
        if !repayment.is_finite() || repayment <= 0.0 {
            metadata.insert("reason_invalid_premium".to_string(), 1.0);
            return none_output(self.id(), self.name(), metadata);
        }

        // Reference price: cross-venue reserve ratios, else price_matrix col 0.
        let p_ref = cross_venue_reference(&state.liquidity_reserves[1..])
            .or_else(|| price_matrix_reference(&state.price_matrix));
        let p_ref = match p_ref {
            Some(v) if v.is_finite() && v > 0.0 => v,
            _ => {
                metadata.insert("reason_no_reference".to_string(), 1.0);
                return none_output(self.id(), self.name(), metadata);
            }
        };

        // Optimal flash principal.
        let radicand = (r1 * gamma * r0) / (p_ref * repayment);
        if !radicand.is_finite() || radicand < 0.0 {
            metadata.insert("reason_non_finite".to_string(), 1.0);
            return none_output(self.id(), self.name(), metadata);
        }
        let x_star = (radicand.sqrt() - r0) / gamma;

        // Edge condition: x* > 0  <=>  gamma*p_pool > (1+phi)*p_ref.
        if !x_star.is_finite() || x_star <= 0.0 {
            metadata.insert("reason_no_edge".to_string(), 1.0);
            metadata.insert("x_star_raw".to_string(), x_star);
            return none_output(self.id(), self.name(), metadata);
        }

        // Net Topological Yield at the optimum (metadata).
        let gas_units = state.features.get("gas_units").copied().unwrap_or(0.0);
        let token0_per_eth = state.features.get("token0_per_eth").copied().unwrap_or(0.0);
        let gas_cost = (state.gas_price_gwei * gas_units * 1e-9 * token0_per_eth).max(0.0);
        let delta_out = (r1 * gamma * x_star) / (r0 + gamma * x_star);
        let y_net = delta_out / p_ref - repayment * x_star - gas_cost;

        metadata.insert("computed".to_string(), 1.0);
        metadata.insert("x_star".to_string(), x_star);
        metadata.insert("y_net".to_string(), y_net);
        metadata.insert("p_pool".to_string(), p_pool);
        metadata.insert("p_ref".to_string(), p_ref);
        metadata.insert("gamma".to_string(), gamma);
        metadata.insert("flash_premium".to_string(), phi);
        metadata.insert("gas_cost".to_string(), gas_cost);
        metadata.insert("delta_out".to_string(), delta_out);

        OperatorOutput {
            operator_id: self.id(),
            operator_name: self.name().to_string(),
            scalar_value: Some(x_star),
            vector_result: Some(vec![x_star, y_net, p_pool, p_ref]),
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

/// Arithmetic mean of r1/r0 over valid (r0>0, r1>0, finite) reserve pairs.
fn cross_venue_reference(reserves: &[(f64, f64)]) -> Option<f64> {
    let prices: Vec<f64> = reserves
        .iter()
        .copied()
        .filter(|(a, b)| a.is_finite() && b.is_finite() && *a > 0.0 && *b > 0.0)
        .map(|(a, b)| b / a)
        .collect();
    if prices.is_empty() {
        return None;
    }
    Some(prices.iter().sum::<f64>() / prices.len() as f64)
}

/// Arithmetic mean of the first column of price_matrix over finite, >0 rows.
fn price_matrix_reference(matrix: &[Vec<f64>]) -> Option<f64> {
    let prices: Vec<f64> = matrix
        .iter()
        .filter_map(|row| row.first().copied())
        .filter(|p| p.is_finite() && *p > 0.0)
        .collect();
    if prices.is_empty() {
        return None;
    }
    Some(prices.iter().sum::<f64>() / prices.len() as f64)
}
