//! Op 32: NSGA-II multiobjective selection of explicitly evaluated candidates.
//!
//! `core` is the typed, dependency-free API. The existing MarketState feature
//! map also supports the following wire format, without changing its schema:
//! - `nsga2.count`: integral candidate count, 0..=512;
//! - `nsga2.population_size`: optional survivor cap, 1..=512 (default 64);
//! - `nsga2.{i}.net_profit_usd`: net expected profit after all known costs;
//! - `nsga2.{i}.risk_cvar_usd`: nonnegative CVaR loss magnitude;
//! - `nsga2.{i}.latency_ms`: nonnegative estimated/measured latency.
//!
//! Objective definitions, confidence level, horizon and snapshot must be
//! comparable across the entire batch. Missing data returns `computed=0` and
//! no scalar/vector/matrix. Ordinary market prices never become invented route
//! objectives. The scalar is the input Pareto-front size, not expected profit.
//! `vector_result` contains survivor indices into the submitted batch;
//! matrix rows are [input index, rank, finite crowding distance, boundary flag].
//! This legacy wrapper performs selection only. Typed `evolve` requires valid
//! domain-aware offspring and actual objective evaluation callbacks.

pub mod core;
pub use self::core::{
    EvaluatedCandidate, EvolutionResult, Nsga2Config, Nsga2Error, Nsga2Optimizer, Objectives,
    Selection, Variation,
};

use super::{MarketState, OperatorOutput, TopologicalOperator};
use std::collections::HashMap;

const API_MAX_CANDIDATES: usize = 512;

#[derive(Default)]
pub struct Nsga2Operator;

impl Nsga2Operator {
    pub fn new() -> Self {
        Self
    }

    /// Typed path for callers with route objects: retain the original objects
    /// and use returned indices to select them without cloning or rewriting.
    pub fn select_candidates(
        &self,
        objectives: &[Objectives],
        config: Nsga2Config,
    ) -> Result<Selection, Nsga2Error> {
        Nsga2Optimizer::new(config)?.select(objectives)
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

impl TopologicalOperator for Nsga2Operator {
    fn id(&self) -> u8 {
        32
    }

    fn name(&self) -> &'static str {
        "NSGA-II"
    }

    fn category(&self) -> &'static str {
        "multiobjective_optimization"
    }

    fn evaluate(&self, state: &MarketState) -> OperatorOutput {
        let count = match state.features.get("nsga2.count") {
            None => return self.unavailable("missing_candidate_objectives"),
            Some(&value) => match integer_feature(value, 0, API_MAX_CANDIDATES) {
                Some(count) => count,
                None => return self.unavailable("invalid_candidate_count"),
            },
        };
        let population_size = match state.features.get("nsga2.population_size") {
            None => 64,
            Some(&value) => match integer_feature(value, 1, API_MAX_CANDIDATES) {
                Some(count) => count,
                None => return self.unavailable("invalid_population_size"),
            },
        };
        let mut objectives = Vec::with_capacity(count);
        for index in 0..count {
            let profit = state.features.get(&format!("nsga2.{index}.net_profit_usd"));
            let risk = state.features.get(&format!("nsga2.{index}.risk_cvar_usd"));
            let latency = state.features.get(&format!("nsga2.{index}.latency_ms"));
            match (profit, risk, latency) {
                (Some(&expected_profit), Some(&risk_cvar), Some(&latency_ms)) => {
                    objectives.push(Objectives {
                        expected_profit,
                        risk_cvar,
                        latency_ms,
                    });
                }
                _ => return self.unavailable("incomplete_candidate_objectives"),
            }
        }
        let config = Nsga2Config {
            population_size,
            ..Nsga2Config::default()
        };
        let selection = match self.select_candidates(&objectives, config) {
            Ok(selection) => selection,
            Err(Nsga2Error::InvalidObjective { .. }) => {
                return self.unavailable("invalid_candidate_objectives")
            }
            Err(_) => return self.unavailable("invalid_selection_configuration"),
        };
        let metadata = HashMap::from([
            ("computed".to_string(), 1.0),
            ("candidate_count".to_string(), count as f64),
            (
                "selected_count".to_string(),
                selection.selected_indices.len() as f64,
            ),
            (
                "pareto_count".to_string(),
                selection.pareto_indices().len() as f64,
            ),
            ("front_count".to_string(), selection.fronts.len() as f64),
            ("generations".to_string(), 0.0),
        ]);
        OperatorOutput {
            operator_id: self.id(),
            operator_name: self.name().to_string(),
            scalar_value: Some(selection.pareto_indices().len() as f64),
            vector_result: Some(
                selection
                    .selected_indices
                    .iter()
                    .map(|&i| i as f64)
                    .collect(),
            ),
            matrix_result: Some(
                (0..count)
                    .map(|i| {
                        vec![
                            i as f64,
                            selection.ranks[i] as f64,
                            selection.crowding_distance[i],
                            if selection.boundary[i] { 1.0 } else { 0.0 },
                        ]
                    })
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
    use crate::operators::{OperatorRegistry, OPERATOR_COUNT};

    fn state() -> MarketState {
        MarketState {
            price_matrix: vec![vec![1.0, 2.0], vec![2.0, 3.0]],
            liquidity_reserves: Vec::new(),
            gas_price_gwei: 1.0,
            block_timestamp: 1,
            block_number: 1,
            features: HashMap::new(),
        }
    }

    fn add_candidate(state: &mut MarketState, index: usize, profit: f64, risk: f64, latency: f64) {
        for (suffix, value) in [
            ("net_profit_usd", profit),
            ("risk_cvar_usd", risk),
            ("latency_ms", latency),
        ] {
            state
                .features
                .insert(format!("nsga2.{index}.{suffix}"), value);
        }
    }

    #[test]
    fn registry_preserves_existing_ids_and_adds_nsga2() {
        let registry = OperatorRegistry::new();
        assert_eq!(registry.all().len(), OPERATOR_COUNT as usize);
        assert_eq!(registry.get(1).unwrap().name(), "SVD");
        assert_eq!(registry.get(32).unwrap().name(), "NSGA-II");
        assert_eq!(
            registry.all().iter().map(|op| op.id()).collect::<Vec<_>>(),
            (1..=OPERATOR_COUNT).collect::<Vec<_>>()
        );
    }

    #[test]
    fn missing_or_partial_objectives_never_use_market_prices_as_candidates() {
        let operator = Nsga2Operator::new();
        let mut state = state();
        let absent = operator.evaluate(&state);
        assert!(absent.scalar_value.is_none());
        assert_eq!(absent.metadata.get("computed"), Some(&0.0));
        state.features.insert("nsga2.count".to_string(), 1.0);
        state
            .features
            .insert("nsga2.0.net_profit_usd".to_string(), 10.0);
        let partial = operator.evaluate(&state);
        assert!(partial.vector_result.is_none());
        assert!(partial.matrix_result.is_none());
    }

    #[test]
    fn wrapper_maps_survivors_to_input_indices_and_serializes_finitely() {
        let mut state = state();
        state.features.insert("nsga2.count".to_string(), 3.0);
        state
            .features
            .insert("nsga2.population_size".to_string(), 2.0);
        add_candidate(&mut state, 0, 10.0, 1.0, 2.0);
        add_candidate(&mut state, 1, 11.0, 2.0, 1.0);
        add_candidate(&mut state, 2, 5.0, 3.0, 4.0);
        let result = OperatorRegistry::new().dispatch(32, &state).unwrap();
        assert_eq!(result.scalar_value, Some(2.0));
        assert_eq!(result.vector_result, Some(vec![0.0, 1.0]));
        assert_eq!(result.matrix_result.as_ref().unwrap()[2][1], 1.0);
        assert!(result
            .matrix_result
            .as_ref()
            .unwrap()
            .iter()
            .flatten()
            .all(|value| value.is_finite()));
        assert!(serde_json::to_string(&result).is_ok());
    }

    #[test]
    fn wrapper_rejects_invalid_counts_and_missing_risk() {
        for count in [f64::NAN, 1.5, -1.0, 513.0] {
            let mut state = state();
            state.features.insert("nsga2.count".to_string(), count);
            assert!(Nsga2Operator::new().evaluate(&state).scalar_value.is_none());
        }
        let mut state = state();
        state.features.insert("nsga2.count".to_string(), 1.0);
        add_candidate(&mut state, 0, 1.0, -1.0, 1.0);
        assert!(Nsga2Operator::new().evaluate(&state).scalar_value.is_none());
    }

    #[test]
    fn explicit_empty_batch_has_empty_results() {
        let mut state = state();
        state.features.insert("nsga2.count".to_string(), 0.0);
        let result = Nsga2Operator::new().evaluate(&state);
        assert_eq!(result.scalar_value, Some(0.0));
        assert_eq!(result.vector_result, Some(Vec::new()));
        assert_eq!(result.matrix_result, Some(Vec::new()));
    }
}
