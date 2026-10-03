//! FUSILE: Implementacion propia -- Inferencia Bayesiana (Beta-Binomial)
//! Formula: posterior P(θ|D) = Beta(α + wins, β + losses) sobre la tasa de éxito θ.
//!   media posterior = (α + wins) / (α + β + wins + losses)
//! Categoria: inference
//!
//! R8 fail-honest: sin historial de éxitos/derrotas en `features`, devuelve
//! scalar_value: None (no computado) — nunca un posterior fabricado.

use super::{MarketState, OperatorOutput, TopologicalOperator};
use std::collections::HashMap;

/// PRIOR Beta(1,1) DECLARADO — decision de MODELO, no una medicion.
///
/// `bayes_prior_alpha` / `bayes_prior_beta` son los parametros del prior
/// θ ~ Beta(α₀, β₀) sobre la tasa de exito. Beta(1,1) es la densidad UNIFORME
/// sobre [0,1]: expresa "ninguna informacion previa sobre θ", que es la eleccion
/// neutral estandar cuando no hay historial ni configuracion cargada.
///
/// FEATURES-DEFAULTS-01 — por que son CONSTANTES NOMBRADAS y no un
/// `unwrap_or(1.0)` inline: un `1.0` suelto es indistinguible de una medicion y
/// esconde que la clave no cargo (config ausente ≠ prior uniforme elegido). Con
/// nombre y este comentario la eleccion queda explicita y auditable.
///
/// NO se convierten en hueco: un prior sin configurar es una DECISION valida del
/// modelo. Un CONTEO sin medir no lo es — por eso `bayes_wins`/`bayes_losses`,
/// que si son mediciones, declaran DATA_GAP en vez de un default.
const DECLARED_UNIFORM_PRIOR_ALPHA: f64 = 1.0;
const DECLARED_UNIFORM_PRIOR_BETA: f64 = 1.0;

#[derive(Default)]
pub struct BayesOperator;

impl BayesOperator {
    pub fn new() -> Self {
        Self
    }
}

impl TopologicalOperator for BayesOperator {
    fn id(&self) -> u8 {
        11
    }

    fn name(&self) -> &'static str {
        "Inferencia Bayesiana"
    }

    fn category(&self) -> &'static str {
        "inference"
    }

    fn evaluate(&self, state: &MarketState) -> OperatorOutput {
        let data_gap = |reason_code: f64, reason: &str| OperatorOutput {
            operator_id: self.id(),
            operator_name: self.name().to_string(),
            scalar_value: None,
            vector_result: None,
            matrix_result: None,
            metadata: {
                let mut m = HashMap::new();
                m.insert("computed".to_string(), 0.0);
                m.insert("reason_code".to_string(), reason_code);
                m.insert(format!("reason_{reason}"), 1.0);
                m
            },
        };

        // FEATURES-DEFAULTS-01 (R8 / RULE 00): `bayes_wins` y `bayes_losses` son
        // MEDICIONES del historial de outcomes. Una ausencia NO es un cero.
        //
        // El defecto anterior (`unwrap_or(0.0)`) era peor que un cero silencioso:
        // con `bayes_wins` PRESENTE y `bayes_losses` AUSENTE el operador
        // publicaba un posterior que contaba cero derrotas que nadie midio
        // (p.ej. wins=5 ⇒ E[θ]=(1+5)/(1+1+5)=6/7≈0.857 sobre un historial
        // incompleto). El guard `wins+losses<1.0` solo enmascaraba el caso en
        // que faltaban AMBAS. Ahora cada clave ausente declara su hueco.
        let wins = match state.features.get("bayes_wins") {
            Some(v) => *v,
            None => return data_gap(3.0, "bayes_wins_unavailable"),
        };
        let losses = match state.features.get("bayes_losses") {
            Some(v) => *v,
            None => return data_gap(4.0, "bayes_losses_unavailable"),
        };
        let alpha0 = state
            .features
            .get("bayes_prior_alpha")
            .copied()
            .unwrap_or(DECLARED_UNIFORM_PRIOR_ALPHA);
        let beta0 = state
            .features
            .get("bayes_prior_beta")
            .copied()
            .unwrap_or(DECLARED_UNIFORM_PRIOR_BETA);

        // MATH-06 fix (2026-09-24): the previous guard `wins + losses < 1.0`
        // passed NaN through (NaN < 1.0 is false) and accepted negative
        // counts (alpha/beta could go ≤ 0, mean outside [0,1]). Now: any
        // non-finite or negative input is an honest DATA_GAP.
        if !wins.is_finite() || !losses.is_finite() || wins < 0.0 || losses < 0.0 {
            return data_gap(1.0, "bayes_invalid_counts_nonfinite_or_negative");
        }
        if !alpha0.is_finite() || !beta0.is_finite() || alpha0 <= 0.0 || beta0 <= 0.0 {
            return data_gap(2.0, "bayes_invalid_prior_nonfinite_or_nonpositive");
        }

        if wins + losses < 1.0 {
            return OperatorOutput {
                operator_id: self.id(),
                operator_name: self.name().to_string(),
                scalar_value: None,
                vector_result: None,
                matrix_result: None,
                metadata: {
                    let mut m = HashMap::new();
                    m.insert("computed".to_string(), 0.0);
                    m.insert("reason_no_history".to_string(), 1.0);
                    m
                },
            };
        }

        let alpha = alpha0 + wins;
        let beta = beta0 + losses;
        let mean = alpha / (alpha + beta);
        let var = (alpha * beta) / ((alpha + beta).powi(2) * (alpha + beta + 1.0));
        let std = var.sqrt();

        let mut metadata = HashMap::new();
        metadata.insert("computed".to_string(), 1.0);
        metadata.insert("posterior_mean".to_string(), mean);
        metadata.insert("posterior_alpha".to_string(), alpha);
        metadata.insert("posterior_beta".to_string(), beta);
        metadata.insert("posterior_std".to_string(), std);
        metadata.insert("wins".to_string(), wins);
        metadata.insert("losses".to_string(), losses);

        OperatorOutput {
            operator_id: self.id(),
            operator_name: self.name().to_string(),
            scalar_value: Some(mean),
            vector_result: Some(vec![mean, std]),
            matrix_result: None,
            metadata,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn st(feats: &[(&str, f64)]) -> MarketState {
        MarketState {
            price_matrix: Vec::new(),
            // Sin precios no hay par que identificar (invariante FEATURES-01b:
            // pair_keys.len() == price_matrix.len() == 0).
            pair_keys: Vec::new(),
            liquidity_reserves: Vec::new(),
            gas_price_gwei: 20.0,
            block_timestamp: 0,
            block_number: 0,
            features: feats.iter().map(|(k, v)| (k.to_string(), *v)).collect(),
        }
    }

    #[test]
    fn posterior_shifts_with_evidence() {
        let op = BayesOperator::new();
        let out = op.evaluate(&st(&[("bayes_wins", 8.0), ("bayes_losses", 2.0)]));
        let mean = out.scalar_value.unwrap();
        assert!(
            (mean - 0.75).abs() < 1e-9,
            "posterior mean should be 0.75 (got {mean})"
        );
        assert_eq!(out.metadata.get("computed"), Some(&1.0));
    }

    #[test]
    fn none_without_history() {
        let op = BayesOperator::new();
        let out = op.evaluate(&st(&[]));
        assert!(out.scalar_value.is_none());
        assert_eq!(out.metadata.get("computed"), Some(&0.0));
    }

    // ── FEATURES-DEFAULTS-01: una AUSENCIA no se vuelve un conteo ────────────

    /// El caso que el default silencioso convertia en computo: wins medido,
    /// losses SIN MEDIR. Antes publicaba E[θ]=(1+5)/(1+1+5) con cero derrotas
    /// inventadas; ahora declara el hueco.
    #[test]
    fn wins_without_losses_is_a_declared_gap_not_a_posterior() {
        let op = BayesOperator::new();
        let out = op.evaluate(&st(&[("bayes_wins", 5.0)]));
        assert!(
            out.scalar_value.is_none(),
            "losses ausente NO puede producir un posterior: {:?}",
            out.scalar_value
        );
        assert!(out.vector_result.is_none());
        assert_eq!(out.metadata.get("computed"), Some(&0.0));
        assert_eq!(out.metadata.get("reason_bayes_losses_unavailable"), Some(&1.0));
        assert_eq!(out.metadata.get("reason_code"), Some(&4.0));
    }

    #[test]
    fn losses_without_wins_is_a_declared_gap_not_a_posterior() {
        let op = BayesOperator::new();
        let out = op.evaluate(&st(&[("bayes_losses", 3.0)]));
        assert!(out.scalar_value.is_none(), "wins ausente ⇒ sin posterior");
        assert_eq!(out.metadata.get("computed"), Some(&0.0));
        assert_eq!(out.metadata.get("reason_bayes_wins_unavailable"), Some(&1.0));
        assert_eq!(out.metadata.get("reason_code"), Some(&3.0));
    }

    /// Camino bueno intacto: con AMBAS claves presentes el posterior es el mismo
    /// de siempre (no se rompe el computo legitimo).
    #[test]
    fn present_counts_still_compute_identically() {
        let op = BayesOperator::new();
        let out = op.evaluate(&st(&[("bayes_wins", 8.0), ("bayes_losses", 2.0)]));
        assert_eq!(out.metadata.get("computed"), Some(&1.0));
        assert!((out.scalar_value.unwrap() - 0.75).abs() < 1e-9);
        // Ceros MEDIDOS siguen siendo mediciones (no huecos): el guard de
        // "sin historial" los trata por su valor, no por su ausencia.
        let zero = op.evaluate(&st(&[("bayes_wins", 0.0), ("bayes_losses", 0.0)]));
        assert!(zero.scalar_value.is_none());
        assert_eq!(zero.metadata.get("reason_no_history"), Some(&1.0));
    }

    /// El prior ausente es una DECISION DE MODELO declarada: Beta(1,1) uniforme.
    /// Se verifica contra la constante nombrada (no contra un 1.0 suelto) para
    /// que renombrarla o cambiarla obligue a revisar esta prueba.
    #[test]
    fn absent_prior_applies_the_declared_uniform_beta() {
        assert_eq!(DECLARED_UNIFORM_PRIOR_ALPHA, 1.0);
        assert_eq!(DECLARED_UNIFORM_PRIOR_BETA, 1.0);
        let op = BayesOperator::new();
        // Sin claves de prior: E[θ] = (α₀+8)/(α₀+β₀+10) = 9/12 = 0.75.
        let out = op.evaluate(&st(&[("bayes_wins", 8.0), ("bayes_losses", 2.0)]));
        let expected = (DECLARED_UNIFORM_PRIOR_ALPHA + 8.0)
            / (DECLARED_UNIFORM_PRIOR_ALPHA + DECLARED_UNIFORM_PRIOR_BETA + 10.0);
        assert!((out.scalar_value.unwrap() - expected).abs() < 1e-12);
        // Un prior CONFIGURADO sigue mandando sobre la constante.
        let configured = op.evaluate(&st(&[
            ("bayes_wins", 8.0),
            ("bayes_losses", 2.0),
            ("bayes_prior_alpha", 4.0),
            ("bayes_prior_beta", 4.0),
        ]));
        assert!((configured.scalar_value.unwrap() - 12.0 / 18.0).abs() < 1e-12);
    }
}
