//! FUSILE: Implementacion propia -- Valor de Shapley
//! Categoria: game
//!
//! Juego cooperativo sobre venues como jugadores. Sea p_i = price_matrix[i][0].
//! Funcion caracteristica (valor de coalicion = spread):
//!   v(S) = max_{i∈S} p_i − min_{i∈S} p_i,   v(∅) = v({i}) = 0.
//! Valor de Shapley exacto de cada pool i:
//!   φ_i = Σ_{S⊆N\{i}} [|S|! · (n − |S| − 1)! / n!] · (v(S ∪ {i}) − v(S))
//! computado por enumeracion de las 2^n coaliciones (factible para n ≤ 8).
//! scalar_value = max_i φ_i  (concentracion del aporte de valor: que venue
//!                            acapara mas valor de Asimetria Topologica).
//! vector_result = [φ_1, …, φ_n].  Axioma eficiencia: Σ φ_i = v(N).
//!
//! R8 fail-honest: None si n < 2 (sin estructura de coaliciones), n > 8
//!                 (exacto O(2^n) infactible ⇒ reason_too_many_venues, nunca
//!                 una aproximacion fabricada), o resultado no finito. Precios
//!                 identicos ⇒ φ_i = 0 ∀i ⇒ scalar = Some(0.0) honesto, NO None.

use super::{MarketState, OperatorOutput, TopologicalOperator};
use std::collections::HashMap;

#[derive(Default)]
pub struct ShapleyOperator;

impl ShapleyOperator {
    pub fn new() -> Self {
        Self
    }
}

impl TopologicalOperator for ShapleyOperator {
    fn id(&self) -> u8 {
        29
    }
    fn name(&self) -> &'static str {
        "Valor de Shapley"
    }
    fn category(&self) -> &'static str {
        "game"
    }

    fn evaluate(&self, state: &MarketState) -> OperatorOutput {
        let mut metadata = HashMap::new();

        // Jugadores = venues; precio p_i = price_matrix[i][0] (asset 0),
        // filtrado finito y > 0.
        let prices: Vec<f64> = state
            .price_matrix
            .iter()
            .filter_map(|row| row.first().copied())
            .filter(|p| p.is_finite() && *p > 0.0)
            .collect();
        let n = prices.len();
        metadata.insert("n_players".to_string(), n as f64);

        if n < 2 {
            metadata.insert("computed".to_string(), 0.0);
            metadata.insert("reason_insufficient_players".to_string(), 1.0);
            return OperatorOutput {
                operator_id: self.id(),
                operator_name: self.name().to_string(),
                scalar_value: None,
                vector_result: None,
                matrix_result: None,
                metadata,
            };
        }
        // Suma exacta es O(n · 2^n); capamos en n ≤ 8 (256 coaliciones).
        if n > 8 {
            metadata.insert("computed".to_string(), 0.0);
            metadata.insert("reason_too_many_venues".to_string(), 1.0);
            return OperatorOutput {
                operator_id: self.id(),
                operator_name: self.name().to_string(),
                scalar_value: None,
                vector_result: None,
                matrix_result: None,
                metadata,
            };
        }

        // v(S): spread max − min sobre la coalicion (0 si |S| < 2).
        let coalition_value = |mask: u32| -> f64 {
            if mask == 0 {
                return 0.0;
            }
            let mut mx = f64::NEG_INFINITY;
            let mut mn = f64::INFINITY;
            let mut cnt = 0u32;
            for i in 0..n {
                if mask & (1u32 << i) != 0 {
                    let p = prices[i];
                    if p > mx {
                        mx = p;
                    }
                    if p < mn {
                        mn = p;
                    }
                    cnt += 1;
                }
            }
            if cnt < 2 {
                0.0
            } else {
                mx - mn
            }
        };

        // Factoriales 0..=n para los pesos de Shapley.
        let mut fact = vec![1.0_f64; n + 1];
        for k in 1..=n {
            fact[k] = fact[k - 1] * k as f64;
        }
        let n_fact = fact[n];

        // φ_i = Σ_{S⊆N\{i}} weight(|S|) · (v(S∪{i}) − v(S)).
        // Iteramos sobre todas las coaliciones mask (= S sin i) y, para cada
        // no-miembro i, acumulamos su contribucion marginal. Cada par (i, S)
        // se visita exactamente una vez ⇒ O(n · 2^n).
        let mut phi = vec![0.0_f64; n];
        let total_masks = 1u32 << n;
        for mask in 0..total_masks {
            let s = mask.count_ones() as usize; // |S|
            let v_s = coalition_value(mask);
            for i in 0..n {
                if mask & (1u32 << i) != 0 {
                    continue; // i ∈ S ⇒ no es coalition subset de N\{i}
                }
                let v_si = coalition_value(mask | (1u32 << i));
                let marginal = v_si - v_s;
                let weight = fact[s] * fact[n - s - 1] / n_fact;
                phi[i] += weight * marginal;
            }
        }

        let phi_max = phi
            .iter()
            .copied()
            .fold(f64::NEG_INFINITY, f64::max);

        if !phi_max.is_finite() {
            metadata.insert("computed".to_string(), 0.0);
            metadata.insert("reason_non_finite".to_string(), 1.0);
            return OperatorOutput {
                operator_id: self.id(),
                operator_name: self.name().to_string(),
                scalar_value: None,
                vector_result: None,
                matrix_result: None,
                metadata,
            };
        }

        let sum_phi: f64 = phi.iter().copied().sum();
        let v_n = coalition_value(total_masks - 1);
        metadata.insert("computed".to_string(), 1.0);
        metadata.insert("phi_max".to_string(), phi_max);
        metadata.insert("sum_phi".to_string(), sum_phi); // axioma eficiencia
        metadata.insert("v_N".to_string(), v_n); // Σ φ_i debe igualar v(N)

        OperatorOutput {
            operator_id: self.id(),
            operator_name: self.name().to_string(),
            scalar_value: Some(phi_max),
            vector_result: Some(phi),
            matrix_result: None,
            metadata,
        }
    }
}
