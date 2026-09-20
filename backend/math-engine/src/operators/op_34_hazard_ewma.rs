//! Op 34: Hazard EWMA — refuerzo de confianza ante degradación de evidencia
//! (WO-15, PERF-STACK-2026-09-20).
//!
//! Deterministic closed form of the per-candidate EWMA hazard of adverse
//! events (429/rate-limit/toxic-flow) over a bounded observation window:
//!     λᵢ = (1 − (1−γ)^nᵢ) · pᵢ
//! where pᵢ = adverse/total and nᵢ = total observations in the window. This
//! is the steady-state value the recursive filter λ ← γ·1 + (1−γ)λ converges
//! to for a constant-rate sequence — same signal as shared-rs `HazardEwma429`
//! at O(1) with no state, so `evaluate` stays a pure function of MarketState
//! (mode-invariant §34). The output downweights candidates whose evidence
//! channel is degrading:
//! - `hazard.count`: candidate count, 0..=512;
//! - `hazard.{i}.adverse_events`: adverse event count in window (≥0);
//! - `hazard.{i}.total_events`: total attempts in window (≥ adverse, ≥0);
//! - `hazard.gamma`: optional forgetting factor (0,1), default 0.3.
//!
//! R8 fail-honest: no features ⇒ computed=0. Arms with zero observations get
//! λ=0 (nothing observed ⇒ no estimated risk), flagged in metadata via
//! `no_evidence_arms`. Scalar = maximum hazard; vector = per-candidate λ;
//! matrix rows = [λᵢ, confidence multiplier 1−λᵢ].

use super::{MarketState, OperatorOutput, TopologicalOperator};
use std::collections::HashMap;

const API_MAX_ARMS: usize = 512;
const DEFAULT_GAMMA: f64 = 0.3;

#[derive(Default)]
pub struct HazardEwmaOperator;

impl HazardEwmaOperator {
    pub fn new() -> Self {
        Self
    }

    fn unavailable(&self, reason: &'static str) -> OperatorOutput {
        OperatorOutput {
            operator_id: self.id(),
            operator_name: self.name().to_string(),
            scalar_value: None,
            vector_result: None,
            matrix_result: None,
            metadata: HashMap::from([
                ("computed".to_string(), 0.0),
                (format!("reason_{reason}"), 1.0),
            ]),
        }
    }
}

impl TopologicalOperator for HazardEwmaOperator {
    fn id(&self) -> u8 {
        34
    }

    fn name(&self) -> &'static str {
        "Hazard EWMA"
    }

    fn category(&self) -> &'static str {
        "risk"
    }

    fn evaluate(&self, state: &MarketState) -> OperatorOutput {
        let count = match state.features.get("hazard.count") {
            None => return self.unavailable("missing_event_window"),
            Some(&v) => match integer_feature(v, 0, API_MAX_ARMS) {
                Some(c) => c,
                None => return self.unavailable("invalid_arm_count"),
            },
        };
        let gamma = state.features.get("hazard.gamma").copied().unwrap_or(DEFAULT_GAMMA);
        if !(gamma.is_finite() && gamma > 0.0 && gamma < 1.0) {
            return self.unavailable("invalid_gamma");
        }

        let mut lambdas = Vec::with_capacity(count);
        let mut no_evidence_arms = 0u32;
        let mut worst = 0usize;
        let mut worst_lambda = f64::NEG_INFINITY;
        for i in 0..count {
            let adverse = state.features.get(&format!("hazard.{i}.adverse_events"));
            let total = state.features.get(&format!("hazard.{i}.total_events"));
            let (adverse, total) = match (adverse, total) {
                (Some(&a), Some(&t))
                    if a.is_finite() && t.is_finite() && a >= 0.0 && t >= a =>
                {
                    (a, t)
                }
                _ => return self.unavailable("incomplete_event_window"),
            };
            let lambda = if total < 1.0 {
                no_evidence_arms += 1;
                0.0
            } else {
                // Closed form of the EWMA steady state at rate p over n obs.
                (1.0 - (1.0 - gamma).powi(total as i32)) * (adverse / total)
            };
            if lambda > worst_lambda {
                worst_lambda = lambda;
                worst = i;
            }
            lambdas.push(lambda);
        }

        // All arms without observations: no estimated risk anywhere — refuse
        // honestly rather than emitting a fabricated all-zero risk surface.
        if no_evidence_arms as usize == count {
            return self.unavailable("no_evidence");
        }

        let mut metadata = HashMap::new();
        metadata.insert("computed".to_string(), 1.0);
        metadata.insert("arm_count".to_string(), count as f64);
        metadata.insert("gamma".to_string(), gamma);
        metadata.insert("max_hazard_index".to_string(), worst as f64);
        metadata.insert("max_hazard".to_string(), worst_lambda);
        metadata.insert("mean_hazard".to_string(), {
            let s: f64 = lambdas.iter().sum();
            s / count.max(1) as f64
        });
        metadata.insert("no_evidence_arms".to_string(), no_evidence_arms as f64);

        OperatorOutput {
            operator_id: self.id(),
            operator_name: self.name().to_string(),
            scalar_value: Some(worst_lambda),
            vector_result: Some(lambdas.clone()),
            matrix_result: Some(
                lambdas
                    .iter()
                    .map(|&l| vec![l, 1.0 - l])
                    .collect(),
            ),
            metadata,
        }
    }
}

fn integer_feature(value: f64, min: usize, max: usize) -> Option<usize> {
    if value.is_finite() && value.fract() == 0.0 && value >= min as f64 && value <= max as f64 {
        Some(value as usize)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn st(feats: &[(&str, f64)]) -> MarketState {
        MarketState {
            price_matrix: Vec::new(),
            liquidity_reserves: Vec::new(),
            gas_price_gwei: 20.0,
            block_timestamp: 0,
            block_number: 0,
            features: feats.iter().map(|(k, v)| (k.to_string(), *v)).collect(),
        }
    }

    #[test]
    fn no_features_is_honest() {
        let out = HazardEwmaOperator::new().evaluate(&st(&[]));
        assert!(out.scalar_value.is_none());
        assert_eq!(out.metadata.get("computed"), Some(&0.0));
    }

    /// Vector independiente: γ=0.5, brazo con 4 adversos / 10 totales ⇒
    /// λ = (1−0.5^10)·0.4 ≈ 0.399609; brazo limpio (0/10) ⇒ λ = 0.
    #[test]
    fn closed_form_matches_hand_computed_vector() {
        let out = HazardEwmaOperator::new().evaluate(&st(&[
            ("hazard.count", 2.0),
            ("hazard.gamma", 0.5),
            ("hazard.0.adverse_events", 4.0),
            ("hazard.0.total_events", 10.0),
            ("hazard.1.adverse_events", 0.0),
            ("hazard.1.total_events", 10.0),
        ]));
        assert_eq!(out.metadata.get("computed"), Some(&1.0));
        let v = out.vector_result.unwrap();
        assert!((v[0] - (1.0 - 0.5f64.powi(10)) * 0.4).abs() < 1e-12, "λ0={}", v[0]);
        assert_eq!(v[1], 0.0);
        assert_eq!(out.scalar_value, Some(v[0]));
        assert_eq!(out.metadata.get("max_hazard_index"), Some(&0.0));
    }

    /// Tasa adversa mayor ⇒ hazard mayor (monotonía en p).
    #[test]
    fn higher_adverse_rate_means_higher_hazard() {
        let out = HazardEwmaOperator::new().evaluate(&st(&[
            ("hazard.count", 2.0),
            ("hazard.0.adverse_events", 8.0),
            ("hazard.0.total_events", 10.0),
            ("hazard.1.adverse_events", 1.0),
            ("hazard.1.total_events", 10.0),
        ]));
        let v = out.vector_result.unwrap();
        assert!(v[0] > v[1]);
    }

    /// Brazo sin observaciones no fabrica riesgo: λ=0 + flag no_evidence_arms.
    #[test]
    fn unobserved_arm_is_zero_and_flagged() {
        let out = HazardEwmaOperator::new().evaluate(&st(&[
            ("hazard.count", 2.0),
            ("hazard.0.adverse_events", 3.0),
            ("hazard.0.total_events", 9.0),
            ("hazard.1.adverse_events", 0.0),
            ("hazard.1.total_events", 0.0),
        ]));
        assert_eq!(out.vector_result.unwrap()[1], 0.0);
        assert_eq!(out.metadata.get("no_evidence_arms"), Some(&1.0));
    }

    #[test]
    fn all_unobserved_refuses_honestly() {
        let out = HazardEwmaOperator::new().evaluate(&st(&[
            ("hazard.count", 2.0),
            ("hazard.0.adverse_events", 0.0),
            ("hazard.0.total_events", 0.0),
            ("hazard.1.adverse_events", 0.0),
            ("hazard.1.total_events", 0.0),
        ]));
        assert!(out.scalar_value.is_none());
        assert_eq!(out.metadata.get("computed"), Some(&0.0));
    }

    #[test]
    fn invalid_inputs_refuse() {
        let adverse_gt_total = HazardEwmaOperator::new().evaluate(&st(&[
            ("hazard.count", 1.0),
            ("hazard.0.adverse_events", 5.0),
            ("hazard.0.total_events", 2.0),
        ]));
        assert!(adverse_gt_total.scalar_value.is_none());
        let bad_gamma = HazardEwmaOperator::new().evaluate(&st(&[
            ("hazard.count", 1.0),
            ("hazard.gamma", 1.5),
            ("hazard.0.adverse_events", 0.0),
            ("hazard.0.total_events", 5.0),
        ]));
        assert!(bad_gamma.scalar_value.is_none());
        let negative = HazardEwmaOperator::new().evaluate(&st(&[
            ("hazard.count", 1.0),
            ("hazard.0.adverse_events", -1.0),
            ("hazard.0.total_events", 5.0),
        ]));
        assert!(negative.scalar_value.is_none());
    }
}
