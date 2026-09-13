//! Deterministic, bounded NSGA-II for externally evaluated candidates.
//!
//! The three objectives are net expected profit (maximise), CVaR loss and
//! latency (minimise). The caller supplies values in consistent units and on
//! one comparable state snapshot. No price, risk, latency or profit is inferred.
//! Selection is O(n²) in time and memory; evolution repeats that work on at
//! most `max_candidates` individuals per generation. It belongs after bounded
//! candidate generation, not inside an unbounded route enumeration loop.
//!
//! This module depends only on std and can be tested with `rustc --test`.

use std::cmp::Ordering;
use std::fmt;

pub const HARD_MAX_CANDIDATES: usize = 1024;
pub const HARD_MAX_GENERATIONS: usize = 64;

/// All three values must be finite; CVaR is a nonnegative loss magnitude and
/// latency is a nonnegative duration. Negative expected profit is admissible:
/// execution viability gates remain the caller's responsibility.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Objectives {
    pub expected_profit: f64,
    pub risk_cvar: f64,
    pub latency_ms: f64,
}

impl Objectives {
    fn validate(self, index: usize) -> Result<(), Nsga2Error> {
        for (name, value) in [
            ("expected_profit", self.expected_profit),
            ("risk_cvar", self.risk_cvar),
            ("latency_ms", self.latency_ms),
        ] {
            if !value.is_finite() || (name != "expected_profit" && value < 0.0) {
                return Err(Nsga2Error::InvalidObjective { index, name });
            }
        }
        Ok(())
    }

    fn minimize(self) -> [f64; 3] {
        [-self.expected_profit, self.risk_cvar, self.latency_ms]
    }

    fn dominates(self, other: Self) -> bool {
        self.expected_profit >= other.expected_profit
            && self.risk_cvar <= other.risk_cvar
            && self.latency_ms <= other.latency_ms
            && (self.expected_profit > other.expected_profit
                || self.risk_cvar < other.risk_cvar
                || self.latency_ms < other.latency_ms)
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Nsga2Config {
    /// Survivor cap; an undersized initial population keeps its initial size.
    pub population_size: usize,
    /// Includes BOTH parents and children during environmental selection.
    pub max_candidates: usize,
    /// Zero means selection only. Bounded evolution is opt-in.
    pub generations: usize,
    pub crossover_probability: f64,
    pub mutation_probability: f64,
    /// Reproducibility seed, not a cryptographic random source.
    pub seed: u64,
}

impl Default for Nsga2Config {
    fn default() -> Self {
        Self {
            population_size: 64,
            max_candidates: 512,
            generations: 0,
            crossover_probability: 0.9,
            mutation_probability: 0.1,
            seed: 0,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Nsga2Error {
    InvalidConfig(&'static str),
    TooManyCandidates {
        count: usize,
        limit: usize,
    },
    InvalidObjective {
        index: usize,
        name: &'static str,
    },
    InfeasibleCandidate {
        generation: usize,
        index: usize,
    },
    CallbackFailed {
        stage: &'static str,
        generation: usize,
        index: usize,
        message: String,
    },
}

impl fmt::Display for Nsga2Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidConfig(reason) => write!(f, "invalid NSGA-II configuration: {reason}"),
            Self::TooManyCandidates { count, limit } => {
                write!(f, "NSGA-II candidate count {count} exceeds limit {limit}")
            }
            Self::InvalidObjective { index, name } => {
                write!(f, "candidate {index} has invalid {name}")
            }
            Self::InfeasibleCandidate { generation, index } => {
                write!(
                    f,
                    "candidate {index} in generation {generation} failed validation"
                )
            }
            Self::CallbackFailed {
                stage,
                generation,
                index,
                message,
            } => {
                write!(
                    f,
                    "{stage} failed for candidate {index} in generation {generation}: {message}"
                )
            }
        }
    }
}

impl std::error::Error for Nsga2Error {}

/// Indices always address the exact slice passed to `select`.
#[derive(Debug, Clone, PartialEq)]
pub struct Selection {
    pub selected_indices: Vec<usize>,
    /// All nondominated fronts, including candidates outside the survivor cap.
    pub fronts: Vec<Vec<usize>>,
    pub ranks: Vec<usize>,
    /// Finite accumulated normalized distance; inspect `boundary` separately.
    pub crowding_distance: Vec<f64>,
    /// Explicit boundary marker avoids exporting infinity to JSON consumers.
    pub boundary: Vec<bool>,
}

impl Selection {
    pub fn pareto_indices(&self) -> &[usize] {
        self.fronts.first().map(Vec::as_slice).unwrap_or(&[])
    }

    fn compare(&self, a: usize, b: usize) -> Ordering {
        self.ranks[a]
            .cmp(&self.ranks[b])
            .then_with(|| self.boundary[b].cmp(&self.boundary[a]))
            .then_with(|| self.crowding_distance[b].total_cmp(&self.crowding_distance[a]))
            .then_with(|| a.cmp(&b))
    }
}

#[derive(Debug, Clone)]
pub struct EvaluatedCandidate<T> {
    pub candidate: T,
    pub objectives: Objectives,
}

/// Route/domain-specific recombination belongs to the caller. These flags
/// request crossover and mutation; `seed` supports deterministic variation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Variation {
    pub generation: usize,
    pub offspring_index: usize,
    pub crossover: bool,
    pub mutate: bool,
    pub seed: u64,
}

#[derive(Debug, Clone)]
pub struct EvolutionResult<T> {
    pub population: Vec<EvaluatedCandidate<T>>,
    /// Indices into the returned population, not the initial input.
    pub pareto_indices: Vec<usize>,
    pub generations_completed: usize,
    pub evaluations: usize,
}

#[derive(Debug, Clone, Copy)]
pub struct Nsga2Optimizer {
    config: Nsga2Config,
}

impl Nsga2Optimizer {
    pub fn new(config: Nsga2Config) -> Result<Self, Nsga2Error> {
        if config.population_size == 0 || config.population_size > config.max_candidates {
            return Err(Nsga2Error::InvalidConfig(
                "population_size must be in 1..=max_candidates",
            ));
        }
        if config.max_candidates > HARD_MAX_CANDIDATES {
            return Err(Nsga2Error::InvalidConfig(
                "max_candidates exceeds hard limit",
            ));
        }
        if config.generations > HARD_MAX_GENERATIONS {
            return Err(Nsga2Error::InvalidConfig("generations exceeds hard limit"));
        }
        if config.generations > 0 && config.population_size > config.max_candidates / 2 {
            return Err(Nsga2Error::InvalidConfig(
                "evolution needs space for parents and offspring",
            ));
        }
        for probability in [config.crossover_probability, config.mutation_probability] {
            if !probability.is_finite() || !(0.0..=1.0).contains(&probability) {
                return Err(Nsga2Error::InvalidConfig(
                    "probabilities must be finite and in [0, 1]",
                ));
            }
        }
        Ok(Self { config })
    }

    pub fn config(&self) -> Nsga2Config {
        self.config
    }

    /// Fast nondominated sorting, crowding distance and elitist survivor
    /// selection. Every input is validated before ranking; none is silently
    /// dropped. Ties resolve by original input index.
    pub fn select(&self, objectives: &[Objectives]) -> Result<Selection, Nsga2Error> {
        let n = objectives.len();
        self.check_count(n)?;
        for (index, objective) in objectives.iter().enumerate() {
            objective.validate(index)?;
        }
        let mut dominated_by_count = vec![0_usize; n];
        let mut dominates = vec![Vec::new(); n];
        for a in 0..n {
            for b in (a + 1)..n {
                if objectives[a].dominates(objectives[b]) {
                    dominates[a].push(b);
                    dominated_by_count[b] += 1;
                } else if objectives[b].dominates(objectives[a]) {
                    dominates[b].push(a);
                    dominated_by_count[a] += 1;
                }
            }
        }
        let mut selection = Selection {
            selected_indices: Vec::with_capacity(n.min(self.config.population_size)),
            fronts: Vec::new(),
            ranks: vec![0; n],
            crowding_distance: vec![0.0; n],
            boundary: vec![false; n],
        };
        let mut current: Vec<usize> = (0..n).filter(|&i| dominated_by_count[i] == 0).collect();
        while !current.is_empty() {
            let rank = selection.fronts.len();
            let mut next = Vec::new();
            for &a in &current {
                selection.ranks[a] = rank;
                for &b in &dominates[a] {
                    dominated_by_count[b] -= 1;
                    if dominated_by_count[b] == 0 {
                        next.push(b);
                    }
                }
            }
            Self::crowding(&current, objectives, &mut selection);
            selection.fronts.push(current);
            next.sort_unstable();
            current = next;
        }
        for front in &selection.fronts {
            let remaining = self.config.population_size - selection.selected_indices.len();
            if remaining == 0 {
                break;
            }
            let mut ordered = front.clone();
            ordered.sort_unstable_by(|&a, &b| selection.compare(a, b));
            selection
                .selected_indices
                .extend(ordered.into_iter().take(remaining));
        }
        Ok(selection)
    }

    /// Complete elitist generational loop with binary tournaments. The caller
    /// supplies representation-aware crossover/mutation, a feasibility check
    /// (e.g. chain, allowed edges, hops and inventory) and objective evaluation.
    /// Every child is validated AND evaluated before it enters the population.
    /// Parents retain objectives for the same immutable market snapshot; to
    /// change snapshots start a new run. Callback failures abort explicitly.
    ///
    /// This API does not fabricate routes or claim that crossover is valid for
    /// arbitrary graphs. `reproduce` must implement its domain's variation.
    pub fn evolve<T>(
        &self,
        initial: Vec<T>,
        mut validate: impl FnMut(&T) -> bool,
        mut evaluate: impl FnMut(&T) -> Result<Objectives, String>,
        mut reproduce: impl FnMut(&T, &T, Variation) -> Result<T, String>,
    ) -> Result<EvolutionResult<T>, Nsga2Error> {
        self.check_count(initial.len())?;
        let mut evaluations = 0;
        let mut population = Vec::with_capacity(initial.len());
        for (index, candidate) in initial.into_iter().enumerate() {
            let evaluated = evaluate_candidate(candidate, 0, index, &mut validate, &mut evaluate)?;
            evaluations += 1;
            population.push(evaluated);
        }
        population = self.survivors(population)?;
        let mut rng = DeterministicRng(self.config.seed);
        let mut generations_completed = 0;
        if !population.is_empty() {
            for generation in 1..=self.config.generations {
                let objectives: Vec<_> = population.iter().map(|p| p.objectives).collect();
                let selection = self.select(&objectives)?;
                let mut children = Vec::with_capacity(population.len());
                for index in 0..population.len() {
                    let a = tournament(&selection, &mut rng);
                    let b = tournament(&selection, &mut rng);
                    let variation = Variation {
                        generation,
                        offspring_index: index,
                        crossover: rng.probability(self.config.crossover_probability),
                        mutate: rng.probability(self.config.mutation_probability),
                        seed: rng.next(),
                    };
                    let candidate = reproduce(
                        &population[a].candidate,
                        &population[b].candidate,
                        variation,
                    )
                    .map_err(|message| Nsga2Error::CallbackFailed {
                        stage: "reproduction",
                        generation,
                        index,
                        message,
                    })?;
                    children.push(evaluate_candidate(
                        candidate,
                        generation,
                        index,
                        &mut validate,
                        &mut evaluate,
                    )?);
                    evaluations += 1;
                }
                // Parents precede offspring so a complete tie preserves the
                // existing candidate, rather than churning equivalent routes.
                population.extend(children);
                population = self.survivors(population)?;
                generations_completed += 1;
            }
        }
        let objectives: Vec<_> = population.iter().map(|p| p.objectives).collect();
        let pareto_indices = self.select(&objectives)?.pareto_indices().to_vec();
        Ok(EvolutionResult {
            population,
            pareto_indices,
            generations_completed,
            evaluations,
        })
    }

    fn survivors<T>(
        &self,
        candidates: Vec<EvaluatedCandidate<T>>,
    ) -> Result<Vec<EvaluatedCandidate<T>>, Nsga2Error> {
        let objectives: Vec<_> = candidates.iter().map(|p| p.objectives).collect();
        let selection = self.select(&objectives)?;
        let mut slots: Vec<_> = candidates.into_iter().map(Some).collect();
        Ok(selection
            .selected_indices
            .into_iter()
            .filter_map(|i| slots[i].take())
            .collect())
    }

    fn check_count(&self, count: usize) -> Result<(), Nsga2Error> {
        if count > self.config.max_candidates {
            Err(Nsga2Error::TooManyCandidates {
                count,
                limit: self.config.max_candidates,
            })
        } else {
            Ok(())
        }
    }

    fn crowding(front: &[usize], objectives: &[Objectives], selection: &mut Selection) {
        if front.len() <= 2 {
            for &i in front {
                selection.boundary[i] = true;
            }
            return;
        }
        for dimension in 0..3 {
            let mut ordered = front.to_vec();
            let value = |i: usize| objectives[i].minimize()[dimension];
            ordered.sort_unstable_by(|&a, &b| {
                let (va, vb) = (value(a), value(b));
                (if va == vb {
                    Ordering::Equal
                } else {
                    va.total_cmp(&vb)
                })
                .then_with(|| a.cmp(&b))
            });
            let min = value(ordered[0]);
            let max = value(ordered[ordered.len() - 1]);
            if min == max {
                // A constant objective conveys no diversity information.
                continue;
            }
            selection.boundary[ordered[0]] = true;
            selection.boundary[ordered[ordered.len() - 1]] = true;
            for window in ordered.windows(3) {
                let (prev, next) = (value(window[0]), value(window[2]));
                let range = max - min;
                let distance = if range.is_finite() {
                    (next - prev) / range
                } else {
                    // Finite inputs can span [-f64::MAX, f64::MAX]. Scaling
                    // first prevents inf/inf and preserves bounded distance.
                    let scale = min.abs().max(max.abs());
                    (next / scale - prev / scale) / (max / scale - min / scale)
                };
                selection.crowding_distance[window[1]] += distance;
            }
        }
    }
}

fn evaluate_candidate<T>(
    candidate: T,
    generation: usize,
    index: usize,
    validate: &mut impl FnMut(&T) -> bool,
    evaluate: &mut impl FnMut(&T) -> Result<Objectives, String>,
) -> Result<EvaluatedCandidate<T>, Nsga2Error> {
    if !validate(&candidate) {
        return Err(Nsga2Error::InfeasibleCandidate { generation, index });
    }
    let objectives = evaluate(&candidate).map_err(|message| Nsga2Error::CallbackFailed {
        stage: "evaluation",
        generation,
        index,
        message,
    })?;
    objectives.validate(index)?;
    Ok(EvaluatedCandidate {
        candidate,
        objectives,
    })
}

fn tournament(selection: &Selection, rng: &mut DeterministicRng) -> usize {
    let n = selection.ranks.len();
    let a = (rng.next() % n as u64) as usize;
    let b = (rng.next() % n as u64) as usize;
    if selection.compare(a, b) == Ordering::Greater {
        b
    } else {
        a
    }
}

/// SplitMix64 is local, reproducible and does not add a runtime dependency.
struct DeterministicRng(u64);

impl DeterministicRng {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9e3779b97f4a7c15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d049bb133111eb);
        z ^ (z >> 31)
    }

    fn probability(&mut self, probability: f64) -> bool {
        let unit = (self.next() >> 11) as f64 / 9_007_199_254_740_992.0;
        unit < probability
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn objective(profit: f64, risk: f64, latency: f64) -> Objectives {
        Objectives {
            expected_profit: profit,
            risk_cvar: risk,
            latency_ms: latency,
        }
    }

    fn optimizer(population_size: usize) -> Nsga2Optimizer {
        Nsga2Optimizer::new(Nsga2Config {
            population_size,
            ..Nsga2Config::default()
        })
        .unwrap()
    }

    #[test]
    fn front_ranking_respects_all_three_objectives_and_duplicates() {
        let values = [
            objective(10.0, 1.0, 5.0),
            objective(12.0, 2.0, 4.0),
            objective(8.0, 2.0, 6.0),
            objective(7.0, 3.0, 7.0),
            objective(10.0, 1.0, 5.0),
            objective(9.0, 0.5, 6.0),
        ];
        let ranked = optimizer(6).select(&values).unwrap();
        assert_eq!(ranked.fronts, vec![vec![0, 1, 4, 5], vec![2], vec![3]]);
        assert_eq!(ranked.ranks, vec![0, 0, 1, 2, 0, 0]);
    }

    #[test]
    fn environmental_selection_preserves_extremes_and_crowding_diversity() {
        let values: Vec<_> = [0.0, 1.0, 2.0, 9.0, 10.0]
            .iter()
            .map(|&x| objective(x, x, 1.0))
            .collect();
        let selection = optimizer(3).select(&values).unwrap();
        assert_eq!(selection.fronts.len(), 1);
        assert_eq!(selection.selected_indices, vec![0, 4, 2]);
        assert!(selection.crowding_distance[2] > selection.crowding_distance[1]);
        assert!((selection.crowding_distance[2] - 1.6).abs() < 1e-12);
    }

    #[test]
    fn identical_objectives_and_signed_zero_have_stable_ties() {
        let mut values = vec![objective(1.0, 0.0, 0.0); 4];
        values[1].risk_cvar = -0.0;
        let selection = optimizer(2).select(&values).unwrap();
        assert_eq!(selection.selected_indices, vec![0, 1]);
        assert_eq!(selection.crowding_distance, vec![0.0; 4]);
        assert_eq!(selection.boundary, vec![false; 4]);
    }

    #[test]
    fn crowding_remains_finite_for_extreme_finite_input() {
        let values = [
            objective(-f64::MAX, 0.0, 1.0),
            objective(0.0, 1.0, 1.0),
            objective(f64::MAX, 2.0, 1.0),
        ];
        let selection = optimizer(3).select(&values).unwrap();
        assert_eq!(selection.fronts.len(), 1);
        assert_eq!(selection.crowding_distance[1], 2.0);
        assert!(selection
            .crowding_distance
            .iter()
            .all(|value| value.is_finite()));
    }

    #[test]
    fn empty_population_has_no_fabricated_front() {
        let selection = optimizer(2).select(&[]).unwrap();
        assert!(selection.fronts.is_empty());
        assert!(selection.selected_indices.is_empty());
        assert!(selection.pareto_indices().is_empty());
    }

    #[test]
    fn malformed_objectives_are_rejected_even_if_they_would_not_survive() {
        for value in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            for index in 0..3 {
                let mut invalid = objective(1.0, 1.0, 1.0);
                match index {
                    0 => invalid.expected_profit = value,
                    1 => invalid.risk_cvar = value,
                    _ => invalid.latency_ms = value,
                }
                assert!(matches!(
                    optimizer(1).select(&[objective(100.0, 0.0, 0.0), invalid]),
                    Err(Nsga2Error::InvalidObjective { index: 1, .. })
                ));
            }
        }
        assert!(optimizer(1).select(&[objective(1.0, -1.0, 1.0)]).is_err());
        assert!(optimizer(1).select(&[objective(1.0, 1.0, -1.0)]).is_err());
        assert!(optimizer(1).select(&[objective(-1.0, 1.0, 1.0)]).is_ok());
    }

    #[test]
    fn configuration_and_workload_limits_are_enforced() {
        for config in [
            Nsga2Config {
                population_size: 0,
                ..Nsga2Config::default()
            },
            Nsga2Config {
                max_candidates: HARD_MAX_CANDIDATES + 1,
                ..Nsga2Config::default()
            },
            Nsga2Config {
                generations: HARD_MAX_GENERATIONS + 1,
                ..Nsga2Config::default()
            },
            Nsga2Config {
                mutation_probability: f64::NAN,
                ..Nsga2Config::default()
            },
            Nsga2Config {
                crossover_probability: 1.1,
                ..Nsga2Config::default()
            },
            Nsga2Config {
                population_size: 300,
                generations: 1,
                ..Nsga2Config::default()
            },
        ] {
            assert!(Nsga2Optimizer::new(config).is_err());
        }
        let configured = Nsga2Optimizer::new(Nsga2Config {
            population_size: 1,
            max_candidates: 2,
            ..Nsga2Config::default()
        })
        .unwrap();
        assert_eq!(
            configured.select(&[objective(1.0, 1.0, 1.0); 3]),
            Err(Nsga2Error::TooManyCandidates { count: 3, limit: 2 })
        );
    }

    #[test]
    fn full_grid_matches_independent_front_peeling_oracle() {
        let mut values = Vec::new();
        for p in 0..4 {
            for r in 0..4 {
                for l in 0..4 {
                    values.push(objective(p as f64, r as f64, l as f64));
                }
            }
        }
        let actual = optimizer(64).select(&values).unwrap();
        let mut remaining: Vec<_> = (0..values.len()).collect();
        let mut expected = Vec::new();
        while !remaining.is_empty() {
            let front: Vec<_> = remaining
                .iter()
                .copied()
                .filter(|&a| {
                    !remaining.iter().copied().any(|b| {
                        let x = values[a];
                        let y = values[b];
                        y.expected_profit >= x.expected_profit
                            && y.risk_cvar <= x.risk_cvar
                            && y.latency_ms <= x.latency_ms
                            && y != x
                    })
                })
                .collect();
            remaining.retain(|i| !front.contains(i));
            expected.push(front);
        }
        assert_eq!(actual.fronts, expected);
    }

    fn evolutionary_config() -> Nsga2Config {
        Nsga2Config {
            population_size: 2,
            max_candidates: 8,
            generations: 3,
            crossover_probability: 1.0,
            mutation_probability: 1.0,
            seed: 12,
        }
    }

    #[test]
    fn evolution_uses_real_callbacks_for_every_child_and_is_reproducible() {
        let optimizer = Nsga2Optimizer::new(evolutionary_config()).unwrap();
        let run = || {
            let mut calls = 0;
            let mut mutations = 0;
            let result = optimizer
                .evolve(
                    vec![1_u32, 2],
                    |&x| x <= 100,
                    |&x| {
                        calls += 1;
                        Ok(objective(x as f64, (100 - x) as f64, 1.0))
                    },
                    |&a, &b, flags| {
                        assert!(flags.mutate && flags.crossover);
                        mutations += 1;
                        Ok(a.max(b) + 1)
                    },
                )
                .unwrap();
            assert_eq!(calls, 8);
            assert_eq!(result.evaluations, calls);
            assert_eq!(mutations, 6);
            assert_eq!(result.generations_completed, 3);
            assert!(result.population[0].candidate >= 4);
            result
                .population
                .into_iter()
                .map(|p| p.candidate)
                .collect::<Vec<_>>()
        };
        assert_eq!(run(), run());
    }

    #[test]
    fn elitism_keeps_parents_when_all_offspring_are_worse() {
        let optimizer = Nsga2Optimizer::new(evolutionary_config()).unwrap();
        let result = optimizer
            .evolve(
                vec![100_u32, 80],
                |_| true,
                |&x| Ok(objective(x as f64, 0.0, 1.0)),
                |_, _, _| Ok(0),
            )
            .unwrap();
        assert_eq!(
            result
                .population
                .iter()
                .map(|p| p.candidate)
                .collect::<Vec<_>>(),
            vec![100, 80]
        );
        assert_eq!(result.pareto_indices, vec![0]);
    }

    #[test]
    fn invalid_children_and_evaluation_errors_abort_before_selection() {
        let optimizer = Nsga2Optimizer::new(evolutionary_config()).unwrap();
        let result = optimizer.evolve(
            vec![1_i32],
            |&x| x > 0,
            |&x| Ok(objective(x as f64, 0.0, 1.0)),
            |_, _, _| Ok(-1),
        );
        assert!(matches!(
            result,
            Err(Nsga2Error::InfeasibleCandidate {
                generation: 1,
                index: 0
            })
        ));
        let result = optimizer.evolve(
            vec![1_i32],
            |_| true,
            |_| Err("state unavailable".to_string()),
            |_, _, _| Ok(1),
        );
        assert!(matches!(
            result,
            Err(Nsga2Error::CallbackFailed {
                stage: "evaluation",
                generation: 0,
                ..
            })
        ));
        let result = optimizer.evolve(
            vec![1_i32],
            |_| true,
            |_| Ok(objective(f64::NAN, 0.0, 1.0)),
            |_, _, _| Ok(1),
        );
        assert!(matches!(result, Err(Nsga2Error::InvalidObjective { .. })));
    }

    #[test]
    fn probability_extremes_have_exact_semantics() {
        let mut rng = DeterministicRng(0);
        for _ in 0..100 {
            assert!(!rng.probability(0.0));
            assert!(rng.probability(1.0));
        }
    }
}
