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
//!
//! FEATURES-DEFAULTS-01: `pool_fee`, `flash_premium`, `gas_units` y
//! `token0_per_eth` son MEDICIONES; si falta cualquiera el operador declara el
//! hueco (`reason_*_unavailable`) en lugar de fabricar 30 bps de fee, un flash
//! premium de 0.0 (financiacion gratuita, prohibido por el prompt §5) o un gas
//! de 0. Un cero PRESENTE es un cero acreditado y se conserva.

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

        // FEATURES-DEFAULTS-01: `pool_fee` es una MEDICION. Ausente ⇒ hueco
        // declarado (antes 0.003 = "30 bps por convencion V2"), que fabricaba la
        // friccion y movia `x*` y el yield neto.
        let fee = match state.features.get("pool_fee") {
            Some(v) if v.is_finite() => *v,
            _ => {
                metadata.insert("reason_fee_unavailable".to_string(), 1.0);
                return none_output(self.id(), self.name(), metadata);
            }
        };
        let gamma = 1.0 - fee;
        if !gamma.is_finite() || gamma <= 0.0 {
            metadata.insert("reason_invalid_fee".to_string(), 1.0);
            return none_output(self.id(), self.name(), metadata);
        }
        // Premium del prestamo flash = COSTO DE FINANCIACION. Ausente NO es 0.0:
        // `flash_premium = 0.0` significa "financiacion flash GRATUITA", que el
        // prompt §5 prohibe asumir ("No asumas financiación flash gratuita
        // porque la card muestre cero… Un fee desconocido debe generar una tarea
        // concreta de resolución"). Y no es decorativo: `phi` entra en
        // `repayment = 1 + phi`, que escala el principal optimo `x*` y el yield
        // neto `y_net`.
        //
        // Un 0.0 PRESENTE si se conserva tal cual: es un CERO ACREDITADO (el
        // productor probo que el premium es 0 — p.ej. un flash swap V2 o un
        // prestamo sin fee). "No medido" y "medido exactamente cero" son
        // estados distintos, y confundirlos es justo el defecto que se corta.
        let phi = match state.features.get("flash_premium") {
            Some(v) if v.is_finite() => *v,
            _ => {
                metadata.insert("reason_flash_premium_unavailable".to_string(), 1.0);
                return none_output(self.id(), self.name(), metadata);
            }
        };
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
        // MATH-05 fix (2026-09-24): the previous defaults (gas_units=0,
        // token0_per_eth=0) made gas_cost=0 — a fail-OPEN that published
        // y_net without the gas discount. Now: absent features ⇒ the operator
        // reports a DATA_GAP (computed=0, reason in metadata), never a
        // fabricated gas-free net. Present features are validated finite/positive.
        let gas_units = state
            .features
            .get("gas_units")
            .copied()
            .filter(|v| v.is_finite() && *v > 0.0);
        let token0_per_eth = state
            .features
            .get("token0_per_eth")
            .copied()
            .filter(|v| v.is_finite() && *v > 0.0);
        let (gas_cost, costs_complete) = match (gas_units, token0_per_eth) {
            (Some(units), Some(eth_price)) => (
                (state.gas_price_gwei * units * 1e-9 * eth_price).max(0.0),
                true,
            ),
            _ => (0.0, false),
        };
        let delta_out = (r1 * gamma * x_star) / (r0 + gamma * x_star);
        let y_net = delta_out / p_ref - repayment * x_star - gas_cost;

        if !costs_complete {
            metadata.insert("computed".to_string(), 0.0);
            // Metadata is HashMap<String, f64> — reason_code 1 = gas features missing.
            metadata.insert("reason_code".to_string(), 1.0);
            // Report the gross-only x* (still useful as an upper bound) but
            // never claim a complete net figure.
            metadata.insert("x_star".to_string(), x_star);
            metadata.insert("y_net_gross_only".to_string(), y_net);
            metadata.insert("gas_cost".to_string(), 0.0);
            metadata.insert("gas_features_status".to_string(), 0.0); // 0 = MISSING
            return none_output(self.id(), self.name(), metadata);
        }

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

#[cfg(test)]
mod tests {
    use super::*;

    /// Clave canonica del par del fixture (formato FEATURES-01b):
    /// pair_keys.len() == price_matrix.len(), mismo par en cada fila.
    const PAR_FIXTURE: &str =
        "0xaaaa0000000000000000000000000000000000aa|0xbbbb0000000000000000000000000000000000bb";

    /// Pool primario con edge (p_pool = 1.05) + referencia cross-venue 1.0.
    fn state_with(feats: &[(&str, f64)]) -> MarketState {
        MarketState {
            price_matrix: vec![vec![1.01]],
            pair_keys: vec![PAR_FIXTURE.to_string(); 1],
            liquidity_reserves: vec![(1_000_000.0, 1_050_000.0), (500_000.0, 500_000.0)],
            gas_price_gwei: 20.0,
            block_timestamp: 1_700_000_000,
            block_number: 18_000_000,
            features: feats.iter().map(|(k, v)| (k.to_string(), *v)).collect(),
        }
    }

    /// Features COMPLETAS: fee medido, premium medido y costo de gas medido.
    fn complete(premium: f64) -> MarketState {
        state_with(&[
            ("pool_fee", 0.003),
            ("flash_premium", premium),
            ("gas_units", 200_000.0),
            ("token0_per_eth", 2_000.0),
        ])
    }

    /// FEATURES-DEFAULTS-01: sin `pool_fee` medido ⇒ hueco (antes 30 bps de
    /// convencion V2, que movia `x*` y `y_net`).
    #[test]
    fn pool_fee_absent_declares_the_gap() {
        let op = FlashLoanOperator::new();
        let out = op.evaluate(&state_with(&[
            ("flash_premium", 0.0005),
            ("gas_units", 200_000.0),
            ("token0_per_eth", 2_000.0),
        ]));
        assert!(out.scalar_value.is_none());
        assert_eq!(out.metadata.get("computed"), Some(&0.0));
        assert_eq!(out.metadata.get("reason_fee_unavailable"), Some(&1.0));
    }

    /// EL CASO QUE EL PROMPT §5 PROHIBE: sin `flash_premium` el operador asumia
    /// financiacion GRATUITA (`unwrap_or(0.0)`) y publicaba `x*`/`y_net` como si
    /// el costo de financiacion fuera cero. Ahora declara el hueco.
    #[test]
    fn flash_premium_absent_never_assumes_free_financing() {
        let op = FlashLoanOperator::new();
        let out = op.evaluate(&state_with(&[
            ("pool_fee", 0.003),
            ("gas_units", 200_000.0),
            ("token0_per_eth", 2_000.0),
        ]));
        assert!(
            out.scalar_value.is_none(),
            "flash premium desconocido NO puede producir un principal optimo: {:?}",
            out.scalar_value
        );
        assert!(out.vector_result.is_none());
        assert_eq!(out.metadata.get("computed"), Some(&0.0));
        assert_eq!(
            out.metadata.get("reason_flash_premium_unavailable"),
            Some(&1.0)
        );
        // Y no queda un `flash_premium` implicito publicado en metadata.
        assert!(!out.metadata.contains_key("flash_premium"));
    }

    /// Un CERO ACREDITADO si se conserva: el productor probo que el premium es 0
    /// (flash swap sin fee). "Medido cero" ≠ "no medido".
    #[test]
    fn accredited_zero_premium_still_computes() {
        let op = FlashLoanOperator::new();
        let out = op.evaluate(&complete(0.0));
        assert_eq!(out.metadata.get("computed"), Some(&1.0));
        assert_eq!(out.metadata.get("flash_premium"), Some(&0.0));
        assert!(out.scalar_value.unwrap().is_finite());
    }

    /// El premium es REAL: encarecer la financiacion reduce el principal optimo.
    /// Esta prueba demuestra que asumir 0.0 no era neutral — movia el resultado.
    #[test]
    fn premium_actually_shrinks_the_optimal_principal() {
        let op = FlashLoanOperator::new();
        let free = op
            .evaluate(&complete(0.0))
            .scalar_value
            .expect("x* sin premium");
        let costly = op
            .evaluate(&complete(0.0009))
            .scalar_value
            .expect("x* con premium Aave 0.09%");
        assert!(
            costly < free,
            "premium mayor ⇒ principal optimo menor (free={free}, costly={costly})"
        );
    }

    /// Un premium presente pero degenerado (repayment ≤ 0) conserva su razon.
    #[test]
    fn degenerate_present_premium_keeps_its_own_reason() {
        let op = FlashLoanOperator::new();
        let out = op.evaluate(&complete(-1.0));
        assert!(out.scalar_value.is_none());
        assert_eq!(out.metadata.get("reason_invalid_premium"), Some(&1.0));
    }

    /// MATH-05 preservado: si falta el costo de gas, el operador sigue negandose
    /// a publicar un neto (computed=0, gas_features_status=0) aunque fee y
    /// premium esten medidos.
    #[test]
    fn missing_gas_features_still_block_the_net_figure() {
        let op = FlashLoanOperator::new();
        let out = op.evaluate(&state_with(&[
            ("pool_fee", 0.003),
            ("flash_premium", 0.0005),
        ]));
        assert!(out.scalar_value.is_none());
        assert_eq!(out.metadata.get("computed"), Some(&0.0));
        assert_eq!(out.metadata.get("gas_features_status"), Some(&0.0));
        // El hueco reportado es el de GAS, no el de fee/premium (ambos medidos).
        assert!(!out.metadata.contains_key("reason_fee_unavailable"));
        assert!(!out
            .metadata
            .contains_key("reason_flash_premium_unavailable"));
    }
}
