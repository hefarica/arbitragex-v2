//! FUSILE: Implementacion propia -- Teoria de Colas (M/G/1)
//! Categoria: operations
//!
//! Cola M/G/1 con tiempo de servicio general. Entradas (features, con defaults):
//!   arrivals_per_block = features["mempool_arrivals_per_block"]  (default 10)
//!   block_time         = features["block_time_sec"]              (default 12 s)
//! Conversion de unidades para ρ adimensional: λ en s⁻¹, E[S] en s.
//!   λ    = arrivals_per_block / block_time           (tasa de arribo, s⁻¹)
//!   E[S] = block_time                                (servicio ≈ inclusion 1 bloque)
//!   E[S²]= Var(S) + E[S]² ; Var(S) de features["block_time_variance_sec2"]
//!          si se provee, o servicio exponencial por defecto (Var = E[S]²):
//!          E[S²] = 2 · E[S]².
//!   ρ    = λ · E[S]                                  (intensidad de trafico)
//! Pollaczek–Khinchine (estado estacionario, ρ < 1):
//!   E[W_q] = λ · E[S²] / (2 · (1 − ρ))              (espera media en cola)
//! scalar_value = E[W_q]  (latencia esperada en cola, segundos).
//!
//! R8 fail-honest: None si ρ ≥ 1 (cola inestable, diverge), 1 − ρ < ε, o
//!                 entradas degeneradas (block_time ≤ 0). Default inestable
//!                 (arrivals_per_block=10) ⇒ None honesto con reason_unstable.

use super::{MarketState, OperatorOutput, TopologicalOperator};
use std::collections::HashMap;

#[derive(Default)]
pub struct QueueingOperator;

impl QueueingOperator {
    pub fn new() -> Self {
        Self
    }

    /// Feature finita o default (defaults usados solo si la feature falta o es NaN).
    fn feat(state: &MarketState, key: &str, default: f64) -> f64 {
        state
            .features
            .get(key)
            .copied()
            .filter(|v| v.is_finite())
            .unwrap_or(default)
    }
}

impl TopologicalOperator for QueueingOperator {
    fn id(&self) -> u8 {
        23
    }
    fn name(&self) -> &'static str {
        "Teoria de Colas"
    }
    fn category(&self) -> &'static str {
        "operations"
    }

    fn evaluate(&self, state: &MarketState) -> OperatorOutput {
        let mut metadata = HashMap::new();

        let arrivals_per_block = Self::feat(state, "mempool_arrivals_per_block", 10.0);
        let block_time = Self::feat(state, "block_time_sec", 12.0);
        metadata.insert("mempool_arrivals_per_block".to_string(), arrivals_per_block);
        metadata.insert("block_time_sec".to_string(), block_time);

        // Entradas degeneradas: block_time ≤ 0 (sin unidad de tiempo) o arribos
        // negativos ⇒ modelo no definido.
        if block_time <= 0.0 || arrivals_per_block < 0.0 {
            metadata.insert("computed".to_string(), 0.0);
            metadata.insert("reason_degenerate_inputs".to_string(), 1.0);
            return OperatorOutput {
                operator_id: self.id(),
                operator_name: self.name().to_string(),
                scalar_value: None,
                vector_result: None,
                matrix_result: None,
                metadata,
            };
        }

        // Unidades consistentes: λ en s⁻¹, E[S] en s ⇒ ρ = λ·E[S] adimensional.
        let lambda = arrivals_per_block / block_time;
        let es = block_time;
        // E[S²] = Var(S) + E[S]². Si se provee varianza finita ≥ 0, se usa;
        // si no, servicio exponencial (M/M/1): Var = E[S]² ⇒ E[S²] = 2·E[S]².
        let var_s = Self::feat(state, "block_time_variance_sec2", -1.0);
        let es2 = if var_s >= 0.0 {
            var_s + es * es
        } else {
            2.0 * es * es
        };

        let rho = lambda * es;
        metadata.insert("lambda_per_sec".to_string(), lambda);
        metadata.insert("mean_service_s".to_string(), es);
        metadata.insert("second_moment_service_s2".to_string(), es2);
        metadata.insert("rho".to_string(), rho);

        // ρ ≥ 1 (o 1 − ρ < ε) ⇒ cola inestable: el estado estacionario M/G/1
        // no existe y E[W_q] → ∞. Formula indefinida ⇒ None.
        if !rho.is_finite() || rho >= 1.0 || (1.0 - rho).abs() < 1e-9 {
            metadata.insert("computed".to_string(), 0.0);
            metadata.insert("reason_unstable".to_string(), 1.0);
            return OperatorOutput {
                operator_id: self.id(),
                operator_name: self.name().to_string(),
                scalar_value: None,
                vector_result: None,
                matrix_result: None,
                metadata,
            };
        }

        let denom = 2.0 * (1.0 - rho);
        let w_q = lambda * es2 / denom;
        let w_total = w_q + es; // W = W_q + E[S] (sojorno total en sistema)

        if !w_q.is_finite() {
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

        metadata.insert("computed".to_string(), 1.0);
        metadata.insert("expected_wait_queue_s".to_string(), w_q);
        metadata.insert("expected_sojourn_s".to_string(), w_total);

        OperatorOutput {
            operator_id: self.id(),
            operator_name: self.name().to_string(),
            scalar_value: Some(w_q),
            vector_result: Some(vec![w_q, w_total, rho]),
            matrix_result: None,
            metadata,
        }
    }
}
