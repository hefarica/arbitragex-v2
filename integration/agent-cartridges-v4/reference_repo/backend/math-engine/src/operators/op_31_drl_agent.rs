//! FUSILE: Implementacion propia -- Agente DRL (PPO) — gate de política no entrenada
//! Formula objetivo (cuando la política esté entrenada): PPO clipped surrogate
//!   L^CLIP(θ) = Ê_t[min(r_t Â_t, clip(r_t, 1−ε, 1+ε) Â_t)],  r_t = π_θ(a_t|s_t)/π_θ_old(a_t|s_t)
//!   Â_t = R_t − V(s_t)   (ventaja crítico-actor)
//!   scalar_value = V(s_t)  (valor esperado del estado = Topological Yield esperado)
//!
//! GATE OBLIGATORIO (fail-honest, anti-RULE 00): V(s_t) NO es computable hasta que la
//! política π_θ haya sido entrenada sobre ≥ MIN_TRAJECTORIES trayectorias etiquetadas
//! (X_t, a_t, R_t=Y_{t+1}) del ledger paper-shadow. Sin entrenamiento, devolver un
//! V(s_t) "realizado" sería fabricar una decisión no validada — prohibido por la
//! doctrina (la matemática no validada nunca toca el hot-path; ver §IV del dictamen).
//!
//! Estado actual: SIN infra de entrenamiento ni trayectorias etiquetadas ⇒ el operador
//! declara honestamente que la política está sin entrenar (None). Cuando se añada el
//! pipeline de calibración (recopilar trayectorias → entrenar π_θ → persistir pesos),
//! este gate se abre y V(s_t) se vuelve real.
//! Categoria: ml

use super::{MarketState, OperatorOutput, TopologicalOperator};
use std::collections::HashMap;

/// Mínimo de trayectorias paper etiquetadas requeridas antes de que V(s_t) sea
/// declarado (calibración del crítico). Por debajo de esto ⇒ None.
const MIN_TRAJECTORIES: usize = 200;

#[derive(Default)]
pub struct DrlAgentOperator;

impl DrlAgentOperator {
    pub fn new() -> Self {
        Self
    }

    /// Hook futuro: número de trayectorias etiquetadas disponibles en el ledger.
    /// Hoy siempre 0 (sin infra de entrenamiento) ⇒ el gate permanece cerrado.
    fn labeled_trajectories_available(_state: &MarketState) -> usize {
        0
    }
}

impl TopologicalOperator for DrlAgentOperator {
    fn id(&self) -> u8 {
        31
    }

    fn name(&self) -> &'static str {
        "Agente de Control Estocastico"
    }

    fn category(&self) -> &'static str {
        "ml"
    }

    /// Dispone del feature `ml` (candle) sólo cuando la política se entrene;
    /// el operador despacha siempre (no requiere el feature para emitir el gate
    /// honesto de "sin entrenar").
    fn is_available(&self) -> bool {
        true
    }

    fn evaluate(&self, state: &MarketState) -> OperatorOutput {
        let n_traj = Self::labeled_trajectories_available(state);
        let mut metadata = HashMap::new();
        metadata.insert("min_trajectories".to_string(), MIN_TRAJECTORIES as f64);
        metadata.insert("labeled_trajectories".to_string(), n_traj as f64);

        // GATE: política sin entrenar ⇒ V(s_t) no es computable honestamente.
        // Devolver None (fail-honest), nunca un V(s_t) fabricado.
        if n_traj < MIN_TRAJECTORIES {
            metadata.insert("computed".to_string(), 0.0);
            metadata.insert("reason_untrained_policy".to_string(), 1.0);
            metadata.insert(
                "gate".to_string(),
                1.0, // gate cerrado: requiere entrenamiento previo
            );
            return OperatorOutput {
                operator_id: self.id(),
                operator_name: self.name().to_string(),
                scalar_value: None,
                vector_result: None,
                matrix_result: None,
                metadata,
            };
        }

        // ── PATH FUTURO (no alcanzado hoy) ───────────────────────────────────
        // Cuando labeled_trajectories_available >= MIN_TRAJECTORIES y los pesos
        // de π_θ estén persistidos, aquí se cargaría el modelo (feature `ml`:
        // candle-core/candle-nn), se construiría el vector de estado s_t desde
        // MarketState (gaps de precio, imbalance de reservas, volatilidad), y se
        // evaluaría V_θ(s_t) → scalar_value. Hasta entonces, el gate anterior
        // retorna None. NO descomentar/fabricar sin el pipeline de entrenamiento.
        metadata.insert("computed".to_string(), 0.0);
        metadata.insert("reason_model_not_loaded".to_string(), 1.0);
        OperatorOutput {
            operator_id: self.id(),
            operator_name: self.name().to_string(),
            scalar_value: None,
            vector_result: None,
            matrix_result: None,
            metadata,
        }
    }
}
