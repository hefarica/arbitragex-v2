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
//! degenerado, o no-convergencia ⇒ scalar_value None. FEATURES-DEFAULTS-01:
//! sin NINGUNA clave de fee (`fee_bps` / `pool_fee`) ⇒ hueco declarado
//! (`reason_fee_unavailable`), nunca los 30 bps de convención.

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

    /// Fee del pool como FRACCION, leida de `features`:
    ///   * `fee_bps`  → bps / 1e4 (convencion del repo: 30 bps = 0.003)
    ///   * `pool_fee` → fraccion directa
    ///
    /// `None` cuando NINGUNA de las dos esta presente. Un fee DESCONOCIDO no se
    /// asume: `gamma = 1 - fee` entra directamente en la curva, asi que el
    /// defecto anterior (0.003 = "30 bps, convencion Uniswap-V2") fabricaba la
    /// friccion del pool y el operador publicaba igualmente un tamaño optimo
    /// `x*` como si la hubiera medido (FEATURES-DEFAULTS-01, R8).
    fn fee_fraction(state: &MarketState) -> Option<f64> {
        state
            .features
            .get("fee_bps")
            .map(|bps| *bps / 10_000.0)
            .or_else(|| state.features.get("pool_fee").copied())
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

        let Some(fee) = Self::fee_fraction(state) else {
            return none_out("fee_unavailable");
        };
        let gamma = 1.0 - fee; // γ = factor de retención post-fee
        if !gamma.is_finite() || gamma <= 0.0 {
            return none_out("invalid_fee");
        }

        // Precio de referencia para costear el gas; fallback precio implícito del pool.
        let price = Self::reference_price(state).unwrap_or(r1 / r0);
        if !price.is_finite() || price <= 0.0 {
            return none_out("invalid_price");
        }

        // MATH-03 fix (2026-09-24): the previous objective mixed token1 (output)
        // minus token0 (input) without conversion — on any pool whose price
        // ratio departs from 1:1 (every WETH/* pair) the "yield" was fiction
        // (counterexample: r0=10 WETH, r1=30000 USDC at a fair price produced
        // ~15k phantom yield-units). Now everything is measured in the SAME
        // numerary (token0): the output is valued at the reference price, the
        // input is costed at that same price, and gas uses swap-real units
        // (21000 was a plain ETH transfer, not a swap).
        // gas_units: swap on V2 ≈ 150k gas (conservative default; the host
        // features map can override — admission gates remain authoritative).
        let gas_units = state
            .features
            .get("gas_units")
            .copied()
            .filter(|v| *v > 0.0 && v.is_finite())
            .unwrap_or(150_000.0);
        let gas = state.gas_price_gwei * gas_units * 1e-9 * price;

        // f(x) = [out(x)·p − x·p] − gas = p · (out(x) − x) − gas, where out(x)
        // is in token1 and p converts token1 → token0 units. Measuring the
        // spread in token0 (the input asset) keeps gross_yield, optimal_size
        // and the bracket [0, r0] dimensionally coherent.
        let f = |x: f64| -> f64 {
            let denom = r0 + gamma * x;
            if denom <= 0.0 {
                return f64::NEG_INFINITY;
            }
            let out = (r1 * gamma * x) / denom; // token1 units
            price * (out - x) - gas // token0-numerary net
        };

        // Maximización por sección áurea sobre [a, b] = [0, r0].
        let mut a = 0.0_f64;
        let mut b = r0;
        if b.partial_cmp(&a) != Some(std::cmp::Ordering::Greater) {
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

#[cfg(test)]
mod tests {
    use super::*;

    /// Clave canonica del par del fixture (formato FEATURES-01b):
    /// pair_keys.len() == price_matrix.len(), mismo par en cada fila.
    const PAR_FIXTURE: &str =
        "0xaaaa0000000000000000000000000000000000aa|0xbbbb0000000000000000000000000000000000bb";

    /// Pool primario con edge (r1/r0 = 1.05) y referencia cross-venue 1.01.
    fn state_with(feats: &[(&str, f64)]) -> MarketState {
        MarketState {
            price_matrix: vec![vec![1.01], vec![1.01]],
            pair_keys: vec![PAR_FIXTURE.to_string(); 2],
            liquidity_reserves: vec![(1_000_000.0, 1_050_000.0)],
            gas_price_gwei: 20.0,
            block_timestamp: 1_700_000_000,
            block_number: 18_000_000,
            features: feats.iter().map(|(k, v)| (k.to_string(), *v)).collect(),
        }
    }

    /// FEATURES-DEFAULTS-01: sin NINGUNA clave de fee el operador declara el
    /// hueco. Antes fabricaba 30 bps ("convencion V2") y publicaba igualmente un
    /// tamaño optimo como si ese fee lo hubiera medido.
    #[test]
    fn fee_absent_declares_the_gap_instead_of_fabricating_30bps() {
        let op = GoldenSectionOperator::new();
        let out = op.evaluate(&state_with(&[]));
        assert!(
            out.scalar_value.is_none(),
            "sin fee medido no hay optimo publicable: {:?}",
            out.scalar_value
        );
        assert!(out.vector_result.is_none());
        assert_eq!(out.metadata.get("computed"), Some(&0.0));
        assert_eq!(out.metadata.get("reason_fee_unavailable"), Some(&1.0));
    }

    /// Camino bueno intacto: con fee medido el operador computa igual que antes.
    #[test]
    fn fee_present_computes_as_before() {
        let op = GoldenSectionOperator::new();
        let by_bps = op.evaluate(&state_with(&[("fee_bps", 30.0)]));
        let by_fraction = op.evaluate(&state_with(&[("pool_fee", 0.003)]));
        for out in [&by_bps, &by_fraction] {
            assert_eq!(out.metadata.get("computed"), Some(&1.0));
            assert!(out.scalar_value.unwrap().is_finite());
            assert!((out.metadata.get("gamma").unwrap() - 0.997).abs() < 1e-12);
        }
        // Mismo fee ⇒ mismo optimo, sin importar la unidad de entrada.
        assert!((by_bps.scalar_value.unwrap() - by_fraction.scalar_value.unwrap()).abs() < 1e-9);
    }

    /// Precedencia preservada: `fee_bps` gana sobre `pool_fee` (semantica previa).
    #[test]
    fn fee_bps_takes_precedence_over_pool_fee() {
        let op = GoldenSectionOperator::new();
        let out = op.evaluate(&state_with(&[("fee_bps", 100.0), ("pool_fee", 0.003)]));
        assert!((out.metadata.get("gamma").unwrap() - 0.99).abs() < 1e-12);
    }

    /// Un fee PRESENTE pero degenerado conserva su propia razon: "medido y
    /// invalido" no es lo mismo que "no medido".
    #[test]
    fn present_but_degenerate_fee_keeps_its_own_reason() {
        let op = GoldenSectionOperator::new();
        let out = op.evaluate(&state_with(&[("pool_fee", 1.0)]));
        assert!(out.scalar_value.is_none());
        assert_eq!(out.metadata.get("reason_invalid_fee"), Some(&1.0));
    }
}
