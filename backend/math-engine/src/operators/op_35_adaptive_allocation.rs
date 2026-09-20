//! Op 35: Asignación Proporcional Adaptativa — refuerzo de sizing/split por
//! candidato (WO-15, PERF-STACK-2026-09-20).
//!
//! Closed-form traffic/capital split across candidates:
//!     wᵢ* ∝ quotaᵢ · μᵢ · (1 − hazardᵢ),  Σwᵢ = 1
//! Mirror of shared-rs `rpc_allocation::constrained_proportional_allocation`
//! (WO-14 Task 3) — port-with-validation: no shared-rs dependency here
//! (math-engine stays dependency-light; shared-rs pulls sqlx/ethers/alloy),
//! and both implementations are pinned to the same independently
//! hand-computed test vectors so drift fails a test on either side.
//! The quota enters multiplicatively (smooth decay); the water-filling hard
//! cap was evaluated and REJECTED (distorts proportional fairness — see
//! shared-rs rpc_allocation module docs).
//! Wire format:
//! - `alloc.count`: candidate count, 0..=512;
//! - `alloc.{i}.quota_remaining`: remaining budget (≥0);
//! - `alloc.{i}.posterior_mean`: E[reward] ∈ [0,1] (e.g. from op_33);
//! - `alloc.{i}.hazard`: adverse-event risk ∈ [0,1] (e.g. from op_34).
//!
//! R8 fail-honest: no features ⇒ computed=0. All-zero scores ⇒ neutral
//! uniform over positive-quota arms (recorded as fallback_neutral=1), never
//! an invented winner. Scalar = argmax index; vector = weights Σ=1.

use super::{MarketState, OperatorOutput, TopologicalOperator};
use std::collections::HashMap;

const API_MAX_ARMS: usize = 512;

#[derive(Default)]
pub struct AdaptiveAllocationOperator;

impl AdaptiveAllocationOperator {
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

impl TopologicalOperator for AdaptiveAllocationOperator {
    fn id(&self) -> u8 {
        35
    }

    fn name(&self) -> &'static str {
        "Asignación Proporcional Adaptativa"
    }

    fn category(&self) -> &'static str {
        "allocation"
    }

    fn evaluate(&self, state: &MarketState) -> OperatorOutput {
        let count = match state.features.get("alloc.count") {
            None => return self.unavailable("missing_allocation_inputs"),
            Some(&v) => match integer_feature(v, 0, API_MAX_ARMS) {
                Some(c) => c,
                None => return self.unavailable("invalid_arm_count"),
            },
        };

        let mut quotas = Vec::with_capacity(count);
        let mut mus = Vec::with_capacity(count);
        let mut hazards = Vec::with_capacity(count);
        for i in 0..count {
            let q = state.features.get(&format!("alloc.{i}.quota_remaining"));
            let m = state.features.get(&format!("alloc.{i}.posterior_mean"));
            let h = state.features.get(&format!("alloc.{i}.hazard"));
            match (q, m, h) {
                (Some(&q), Some(&m), Some(&h)) => {
                    quotas.push(q);
                    mus.push(clamp_unit(m));
                    hazards.push(clamp_unit(h));
                }
                _ => return self.unavailable("incomplete_allocation_inputs"),
            }
        }

        let score: Vec<f64> = (0..count)
            .map(|i| {
                let q = if quotas[i].is_finite() && quotas[i] > 0.0 { quotas[i] } else { 0.0 };
                q * mus[i] * (1.0 - hazards[i])
            })
            .collect();
        let total: f64 = score.iter().sum();

        let weights: Vec<f64> = if total > 0.0 {
            score.iter().map(|s| s / total).collect()
        } else {
            // Neutral honest fallback: uniform over positive-quota arms.
            let eligible = quotas.iter().filter(|q| **q > 0.0).count();
            if eligible == 0 {
                return self.unavailable("no_eligible_candidates");
            }
            quotas
                .iter()
                .map(|q| if *q > 0.0 { 1.0 / eligible as f64 } else { 0.0 })
                .collect()
        };

        let best = weights
            .iter()
            .enumerate()
            .max_by(|(_, a), (_, b)| a.partial_cmp(b).expect("finite weights"))
            .map(|(i, _)| i)
            .unwrap_or(0);

        let sum: f64 = weights.iter().sum();
        let mut metadata = HashMap::new();
        metadata.insert("computed".to_string(), 1.0);
        metadata.insert("arm_count".to_string(), count as f64);
        metadata.insert("best_index".to_string(), best as f64);
        metadata.insert("best_share".to_string(), weights[best]);
        metadata.insert("weight_sum".to_string(), sum);
        if total <= 0.0 {
            metadata.insert("fallback_neutral".to_string(), 1.0);
        }

        OperatorOutput {
            operator_id: self.id(),
            operator_name: self.name().to_string(),
            scalar_value: Some(best as f64),
            vector_result: Some(weights),
            matrix_result: None,
            metadata,
        }
    }
}

fn clamp_unit(v: f64) -> f64 {
    if v.is_finite() {
        v.clamp(0.0, 1.0)
    } else {
        0.0
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
        let out = AdaptiveAllocationOperator::new().evaluate(&st(&[]));
        assert!(out.scalar_value.is_none());
        assert_eq!(out.metadata.get("computed"), Some(&0.0));
    }

    /// Vector independiente (idéntico al pin de shared-rs rpc_allocation):
    /// scores 2:1 con hazard 0 ⇒ pesos 2/3 y 1/3 exactos.
    #[test]
    fn weights_match_hand_computed_2to1_vector() {
        let out = AdaptiveAllocationOperator::new().evaluate(&st(&[
            ("alloc.count", 2.0),
            ("alloc.0.quota_remaining", 10.0),
            ("alloc.0.posterior_mean", 0.8),
            ("alloc.0.hazard", 0.0),
            ("alloc.1.quota_remaining", 10.0),
            ("alloc.1.posterior_mean", 0.4),
            ("alloc.1.hazard", 0.0),
        ]));
        assert_eq!(out.metadata.get("computed"), Some(&1.0));
        let w = out.vector_result.unwrap();
        assert!((w[0] - 2.0 / 3.0).abs() < 1e-9, "w0={}", w[0]);
        assert!((w[1] - 1.0 / 3.0).abs() < 1e-9, "w1={}", w[1]);
        assert_eq!(out.scalar_value, Some(0.0));
    }

    /// Hazard alto desplaza el argmax aunque el posterior sea igual.
    #[test]
    fn hazard_flips_best_candidate() {
        let out = AdaptiveAllocationOperator::new().evaluate(&st(&[
            ("alloc.count", 2.0),
            ("alloc.0.quota_remaining", 10.0),
            ("alloc.0.posterior_mean", 0.8),
            ("alloc.0.hazard", 0.9),
            ("alloc.1.quota_remaining", 10.0),
            ("alloc.1.posterior_mean", 0.8),
            ("alloc.1.hazard", 0.0),
        ]));
        assert_eq!(out.scalar_value, Some(1.0));
    }

    /// Cuota cero ⇒ share cero (sin derecho a tráfico).
    #[test]
    fn zero_quota_arm_gets_zero_share() {
        let out = AdaptiveAllocationOperator::new().evaluate(&st(&[
            ("alloc.count", 3.0),
            ("alloc.0.quota_remaining", 100.0),
            ("alloc.0.posterior_mean", 0.5),
            ("alloc.0.hazard", 0.0),
            ("alloc.1.quota_remaining", 0.0),
            ("alloc.1.posterior_mean", 1.0),
            ("alloc.1.hazard", 0.0),
            ("alloc.2.quota_remaining", 100.0),
            ("alloc.2.posterior_mean", 0.5),
            ("alloc.2.hazard", 0.0),
        ]));
        let w = out.vector_result.unwrap();
        assert_eq!(w[1], 0.0);
        let s: f64 = w.iter().sum();
        assert!((s - 1.0).abs() < 1e-9);
    }

    /// Scores todos cero ⇒ neutral uniforme sobre cuotas positivas, marcada.
    #[test]
    fn all_zero_scores_fall_back_to_neutral() {
        let out = AdaptiveAllocationOperator::new().evaluate(&st(&[
            ("alloc.count", 2.0),
            ("alloc.0.quota_remaining", 5.0),
            ("alloc.0.posterior_mean", 0.0),
            ("alloc.0.hazard", 0.0),
            ("alloc.1.quota_remaining", 5.0),
            ("alloc.1.posterior_mean", 0.0),
            ("alloc.1.hazard", 0.0),
        ]));
        assert_eq!(out.vector_result.unwrap(), vec![0.5, 0.5]);
        assert_eq!(out.metadata.get("fallback_neutral"), Some(&1.0));
    }

    #[test]
    fn all_zero_quota_refuses() {
        let out = AdaptiveAllocationOperator::new().evaluate(&st(&[
            ("alloc.count", 1.0),
            ("alloc.0.quota_remaining", 0.0),
            ("alloc.0.posterior_mean", 0.5),
            ("alloc.0.hazard", 0.0),
        ]));
        assert!(out.scalar_value.is_none());
        assert_eq!(out.metadata.get("computed"), Some(&0.0));
    }

    #[test]
    fn invalid_or_partial_inputs_refuse() {
        let partial = AdaptiveAllocationOperator::new().evaluate(&st(&[
            ("alloc.count", 2.0),
            ("alloc.0.quota_remaining", 1.0),
        ]));
        assert!(partial.scalar_value.is_none());
        let bad_count = AdaptiveAllocationOperator::new()
            .evaluate(&st(&[("alloc.count", -1.0)]));
        assert!(bad_count.scalar_value.is_none());
    }
}
