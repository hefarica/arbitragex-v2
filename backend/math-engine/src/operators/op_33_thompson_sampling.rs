//! Op 33: Thompson Sampling — refuerzo explore/exploit entre candidatos de
//! una estrategia (WO-15, PERF-STACK-2026-09-20).
//!
//! Deterministic Bayes-UCB realization of Beta(α,β) Thompson posteriors:
//! scoreᵢ = μᵢ + z·σᵢ with μ=α/(α+β), σ=sqrt(αβ/((α+β)²(α+β+1))).
//! Sampling θᵢ ~ Beta is the stochastic counterpart; a deterministic score is
//! required here so `evaluate` is a pure function of MarketState (reproducible,
//! mode-invariant §34). The live stochastic selector (RNG) lives in
//! shared-rs `rpc_bandit` — this op is the strategy-side reinforcement over
//! candidate routes/pools/providers declared via `features`:
//! - `ts.count`: candidate count, 0..=512;
//! - `ts.{i}.successes`, `ts.{i}.failures`: cumulative observed counts (≥0);
//! - `ts.exploration_z`: optional optimism multiplier (default 1.0).
//!
//! R8 fail-honest: no candidates or zero total evidence across all arms ⇒
//! `computed=0`, no scalar (never invent a winner among equals). Missing
//! counts for a declared candidate ⇒ whole batch honest-refused.

use super::{MarketState, OperatorOutput, TopologicalOperator};
use std::collections::HashMap;

const API_MAX_ARMS: usize = 512;

#[derive(Default)]
pub struct ThompsonSamplingOperator;

impl ThompsonSamplingOperator {
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

impl TopologicalOperator for ThompsonSamplingOperator {
    fn id(&self) -> u8 {
        33
    }

    fn name(&self) -> &'static str {
        "Thompson Sampling"
    }

    fn category(&self) -> &'static str {
        "exploration_exploitation"
    }

    fn evaluate(&self, state: &MarketState) -> OperatorOutput {
        let count = match state.features.get("ts.count") {
            None => return self.unavailable("missing_arm_counts"),
            Some(&v) => match integer_feature(v, 0, API_MAX_ARMS) {
                Some(c) => c,
                None => return self.unavailable("invalid_arm_count"),
            },
        };
        let z = state
            .features
            .get("ts.exploration_z")
            .copied()
            .unwrap_or(1.0);
        if !(z.is_finite() && z >= 0.0) {
            return self.unavailable("invalid_exploration_z");
        }

        let mut alphas = Vec::with_capacity(count);
        let mut betas = Vec::with_capacity(count);
        for i in 0..count {
            let s = state.features.get(&format!("ts.{i}.successes"));
            let f = state.features.get(&format!("ts.{i}.failures"));
            match (s, f) {
                (Some(&s), Some(&f))
                    if s.is_finite() && f.is_finite() && s >= 0.0 && f >= 0.0 =>
                {
                    alphas.push(1.0 + s);
                    betas.push(1.0 + f);
                }
                _ => return self.unavailable("incomplete_arm_counts"),
            }
        }

        // Prior Beta(1,1) everywhere and no observations ⇒ all posteriors
        // identical: honest refusal, exploration is the caller's policy.
        let total_evidence: f64 = alphas.iter().zip(&betas).map(|(a, b)| a + b - 2.0).sum();
        if total_evidence <= 0.0 {
            return self.unavailable("no_evidence");
        }

        let mut best = 0usize;
        let mut best_score = f64::NEG_INFINITY;
        let mut stats = Vec::with_capacity(count);
        for (i, (&a, &b)) in alphas.iter().zip(&betas).enumerate() {
            let n = a + b;
            let mean = a / n;
            let var = (a * b) / (n * n * (n + 1.0));
            let std = var.sqrt();
            let score = mean + z * std;
            stats.push((mean, std, score));
            if score > best_score {
                best_score = score;
                best = i;
            }
        }

        let mut metadata = HashMap::new();
        metadata.insert("computed".to_string(), 1.0);
        metadata.insert("arm_count".to_string(), count as f64);
        metadata.insert("best_index".to_string(), best as f64);
        metadata.insert("exploration_z".to_string(), z);
        metadata.insert(
            "best_posterior_mean".to_string(),
            stats[best].0,
        );

        OperatorOutput {
            operator_id: self.id(),
            operator_name: self.name().to_string(),
            scalar_value: Some(best as f64),
            vector_result: Some(stats.iter().map(|(m, s, _)| *m + z * s).collect()),
            matrix_result: Some(
                stats
                    .iter()
                    .map(|(m, s, sc)| vec![*m, *s, *sc])
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
        let out = ThompsonSamplingOperator::new().evaluate(&st(&[]));
        assert!(out.scalar_value.is_none());
        assert_eq!(out.metadata.get("computed"), Some(&0.0));
    }

    /// Vector independiente: brazo 0 con 90/10 éxitos, brazo 1 con 10/90 ⇒
    /// argmax = 0 y su media posterior ≈ 91/102 ≈ 0.8922.
    #[test]
    fn optimistic_score_prefers_dominant_arm() {
        let out = ThompsonSamplingOperator::new().evaluate(&st(&[
            ("ts.count", 2.0),
            ("ts.0.successes", 90.0),
            ("ts.0.failures", 10.0),
            ("ts.1.successes", 10.0),
            ("ts.1.failures", 90.0),
        ]));
        assert_eq!(out.scalar_value, Some(0.0));
        assert_eq!(out.metadata.get("computed"), Some(&1.0));
        let mean0 = out.metadata.get("best_posterior_mean").copied().unwrap();
        assert!((mean0 - 91.0 / 102.0).abs() < 1e-9, "mean0={mean0}");
        assert!(out.vector_result.unwrap()[0] > 0.0);
    }

    /// La exploración (z>0) puede promover un brazo con alta varianza sobre
    /// uno con media apenas superior pero ya consolidado.
    #[test]
    fn exploration_z_promotes_uncertain_arm() {
        // Brazo 0: 2/2 éxitos (media 0.75, std alto). Brazo 1: 900/100
        // (media ~0.901, std bajo). Con z grande gana el exploratorio.
        let base = &[
            ("ts.count", 2.0),
            ("ts.0.successes", 2.0),
            ("ts.0.failures", 2.0),
            ("ts.1.successes", 900.0),
            ("ts.1.failures", 100.0),
        ];
        let z0 = ThompsonSamplingOperator::new().evaluate(&st(base));
        assert_eq!(z0.scalar_value, Some(1.0), "sin exploración gana el mejor");
        let mut explorative = base.to_vec();
        explorative.push(("ts.exploration_z", 10.0));
        let z10 = ThompsonSamplingOperator::new().evaluate(&st(&explorative));
        assert_eq!(z10.scalar_value, Some(0.0), "con z=10 gana el incierto");
    }

    #[test]
    fn zero_evidence_across_arms_refuses() {
        let out = ThompsonSamplingOperator::new().evaluate(&st(&[
            ("ts.count", 3.0),
            ("ts.0.successes", 0.0),
            ("ts.0.failures", 0.0),
            ("ts.1.successes", 0.0),
            ("ts.1.failures", 0.0),
            ("ts.2.successes", 0.0),
            ("ts.2.failures", 0.0),
        ]));
        assert!(out.scalar_value.is_none());
        assert_eq!(out.metadata.get("computed"), Some(&0.0));
    }

    #[test]
    fn partial_or_invalid_counts_refuse() {
        let partial = ThompsonSamplingOperator::new().evaluate(&st(&[
            ("ts.count", 2.0),
            ("ts.0.successes", 5.0),
            ("ts.0.failures", 1.0),
        ]));
        assert!(partial.scalar_value.is_none());
        let negative = ThompsonSamplingOperator::new().evaluate(&st(&[
            ("ts.count", 1.0),
            ("ts.0.successes", -1.0),
            ("ts.0.failures", 1.0),
        ]));
        assert!(negative.scalar_value.is_none());
        let bad_count = ThompsonSamplingOperator::new()
            .evaluate(&st(&[("ts.count", 513.0)]));
        assert!(bad_count.scalar_value.is_none());
    }

    #[test]
    fn explicit_empty_batch_is_computed_empty() {
        let out = ThompsonSamplingOperator::new().evaluate(&st(&[("ts.count", 0.0)]));
        assert!(out.scalar_value.is_none());
        assert_eq!(out.metadata.get("computed"), Some(&0.0));
    }
}
