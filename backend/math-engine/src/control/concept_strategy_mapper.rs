//! Maps mathematical concepts to actionable strategy profiles.
//!
//! A `ConceptStrategyMapper` answers the question: "Given a set of activated
//! math-physics concepts, what strategy should the system adopt?" The mapping
//! is declarative and read-only; execution is delegated to external consumers
//! that apply their own safety gates.

use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

/// Identifier for a mathematical concept in the engine.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConceptId {
    /// Singular value decomposition.
    Svd,
    /// Principal component analysis.
    Pca,
    /// Eigen-decomposition / spectral analysis.
    Eigen,
    /// Von Neumann entropy.
    VonNeumann,
    /// Piecewise-deterministic Markov process.
    Pdmp,
    /// Discrete Markov chains.
    MarkovChain,
    /// Hidden Markov model.
    Hmm,
    /// Kalman filter.
    Kalman,
    /// Stable Lévy processes.
    Levy,
    /// Welford online variance.
    Welford,
    /// Beta-Binomial Bayesian update.
    Bayes,
    /// Maximum likelihood estimation.
    Mle,
    /// Linear and logistic regression.
    Regression,
    /// Kullback-Leibler divergence.
    KlDivergence,
    /// Golden-section line search.
    GoldenSection,
    /// Kelly criterion sizing.
    Kelly,
    /// Pontryagin maximum principle.
    Pontryagin,
    /// Lagrangian constraint enforcement.
    Lagrangian,
    /// Linear programming via simplex.
    Simplex,
    /// Gradient-based optimization (Adam).
    GradientDescent,
    /// Newton-Raphson root finding.
    Newton,
    /// Monte Carlo simulation.
    MonteCarlo,
    /// Queueing theory models.
    Queueing,
    /// Game-theoretic equilibrium.
    GameTheory,
}

impl ConceptId {
    /// All concept identifiers, in canonical order.
    pub fn all() -> &'static [ConceptId] {
        &[
            ConceptId::Svd,
            ConceptId::Pca,
            ConceptId::Eigen,
            ConceptId::VonNeumann,
            ConceptId::Pdmp,
            ConceptId::MarkovChain,
            ConceptId::Hmm,
            ConceptId::Kalman,
            ConceptId::Levy,
            ConceptId::Welford,
            ConceptId::Bayes,
            ConceptId::Mle,
            ConceptId::Regression,
            ConceptId::KlDivergence,
            ConceptId::GoldenSection,
            ConceptId::Kelly,
            ConceptId::Pontryagin,
            ConceptId::Lagrangian,
            ConceptId::Simplex,
            ConceptId::GradientDescent,
            ConceptId::Newton,
            ConceptId::MonteCarlo,
            ConceptId::Queueing,
            ConceptId::GameTheory,
        ]
    }

    /// Category label for grouping in UIs and reports.
    pub fn category(&self) -> &'static str {
        match self {
            ConceptId::Svd | ConceptId::Pca | ConceptId::Eigen | ConceptId::VonNeumann => {
                "spectral"
            }
            ConceptId::Pdmp
            | ConceptId::MarkovChain
            | ConceptId::Hmm
            | ConceptId::Kalman
            | ConceptId::Levy
            | ConceptId::Welford => "stochastic",
            ConceptId::Bayes | ConceptId::Mle | ConceptId::Regression | ConceptId::KlDivergence => {
                "inference"
            }
            ConceptId::GoldenSection
            | ConceptId::Kelly
            | ConceptId::Pontryagin
            | ConceptId::Lagrangian
            | ConceptId::Simplex
            | ConceptId::GradientDescent
            | ConceptId::Newton => "optimization",
            ConceptId::MonteCarlo => "monte_carlo",
            ConceptId::Queueing => "queueing",
            ConceptId::GameTheory => "game_theory",
        }
    }
}

/// A strategy recommendation produced from the active concept set.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct StrategyProfile {
    /// Human-readable strategy name.
    pub name: String,
    /// Ordered list of concept identifiers that justify this profile.
    pub supporting_concepts: Vec<ConceptId>,
    /// Conservative sizing recommendation in basis points.
    pub sizing_bps: u32,
    /// Whether the profile suggests multi-hop route exploration.
    pub explore_multi_hop: bool,
    /// Whether the profile suggests adversarial modeling of other searchers.
    pub model_adversaries: bool,
}

impl StrategyProfile {
    /// Baseline strategy with no math-physics concepts enabled.
    pub fn none() -> Self {
        Self {
            name: "none".to_string(),
            supporting_concepts: vec![],
            sizing_bps: 0,
            explore_multi_hop: false,
            model_adversaries: false,
        }
    }
}

impl Default for StrategyProfile {
    fn default() -> Self {
        Self::none()
    }
}

/// Maps a set of active concepts to a strategy profile.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ConceptStrategyMapper {
    /// Fixed rules: concept subset -> strategy profile.
    rules: Vec<(HashSet<ConceptId>, StrategyProfile)>,
    /// Fallback profile when no rule matches.
    fallback: StrategyProfile,
}

impl ConceptStrategyMapper {
    /// Build the canonical mapper used by the engine.
    pub fn canonical() -> Self {
        let mut rules: Vec<(HashSet<ConceptId>, StrategyProfile)> = vec![];

        // Rule: spectral analysis alone => cautious regime detection.
        rules.push((
            [ConceptId::Eigen, ConceptId::VonNeumann]
                .iter()
                .copied()
                .collect(),
            StrategyProfile {
                name: "spectral_regime_watch".to_string(),
                supporting_concepts: vec![ConceptId::Eigen, ConceptId::VonNeumann],
                sizing_bps: 25,
                explore_multi_hop: false,
                model_adversaries: false,
            },
        ));

        // Rule: stochastic + inference => adaptive opportunity filter.
        let mut stochastic_inference = HashSet::new();
        stochastic_inference.insert(ConceptId::Hmm);
        stochastic_inference.insert(ConceptId::Kalman);
        stochastic_inference.insert(ConceptId::Bayes);
        rules.push((
            stochastic_inference,
            StrategyProfile {
                name: "adaptive_filter".to_string(),
                supporting_concepts: vec![ConceptId::Hmm, ConceptId::Kalman, ConceptId::Bayes],
                sizing_bps: 75,
                explore_multi_hop: true,
                model_adversaries: false,
            },
        ));

        // Rule: full optimization + game theory => adversarial sizing.
        let mut adversarial = HashSet::new();
        adversarial.insert(ConceptId::GameTheory);
        adversarial.insert(ConceptId::Kelly);
        adversarial.insert(ConceptId::Simplex);
        rules.push((
            adversarial,
            StrategyProfile {
                name: "adversarial_sizer".to_string(),
                supporting_concepts: vec![
                    ConceptId::GameTheory,
                    ConceptId::Kelly,
                    ConceptId::Simplex,
                ],
                sizing_bps: 150,
                explore_multi_hop: true,
                model_adversaries: true,
            },
        ));

        Self {
            rules,
            fallback: StrategyProfile {
                name: "partial_default".to_string(),
                supporting_concepts: vec![],
                sizing_bps: 50,
                explore_multi_hop: false,
                model_adversaries: false,
            },
        }
    }

    /// Select the best matching strategy profile for the active concepts.
    ///
    /// Rules are evaluated in order; the first rule whose required concepts are
    /// all active wins. If no rule matches, the fallback is returned.
    pub fn map(&self, active: &HashSet<ConceptId>) -> StrategyProfile {
        for (required, profile) in &self.rules {
            if required.is_subset(active) {
                return profile.clone();
            }
        }
        self.fallback.clone()
    }

    /// Return a diagnostic showing which rules matched and why.
    pub fn diagnose(&self, active: &HashSet<ConceptId>) -> HashMap<String, bool> {
        self.rules
            .iter()
            .map(|(required, profile)| (profile.name.clone(), required.is_subset(active)))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn none_returns_fallback() {
        let mapper = ConceptStrategyMapper::canonical();
        let profile = mapper.map(&HashSet::new());
        assert_eq!(profile.name, "partial_default");
        assert_eq!(profile.sizing_bps, 50);
    }

    #[test]
    fn spectral_rule_matches() {
        let mapper = ConceptStrategyMapper::canonical();
        let active = [ConceptId::Eigen, ConceptId::VonNeumann]
            .iter()
            .copied()
            .collect();
        let profile = mapper.map(&active);
        assert_eq!(profile.name, "spectral_regime_watch");
        assert_eq!(profile.sizing_bps, 25);
    }

    #[test]
    fn adversarial_rule_matches() {
        let mapper = ConceptStrategyMapper::canonical();
        let active = [
            ConceptId::GameTheory,
            ConceptId::Kelly,
            ConceptId::Simplex,
            ConceptId::MarkovChain,
        ]
        .iter()
        .copied()
        .collect();
        let profile = mapper.map(&active);
        assert_eq!(profile.name, "adversarial_sizer");
        assert!(profile.model_adversaries);
    }
}
