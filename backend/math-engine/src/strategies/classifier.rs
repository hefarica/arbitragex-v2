//! Motor de Clasificacion Determinista MEV → Matematica Pura
//! Mapea 264 estrategias a 31 conceptos matematicos de forma determinista

use std::collections::HashMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MathDomain {
    GraphTheory, NumericalAnalysis, BayesianInference, LinearAlgebra,
    QueueingTheory, Topology, OptimalControl, Optimization,
    GameTheory, StatisticalInference, ProbabilityTheory,
}

impl MathDomain {
    pub fn primary_concepts(&self) -> Vec<u8> {
        match self {
            Self::GraphTheory => vec![1, 15, 16],
            Self::NumericalAnalysis => vec![3, 4, 21],
            Self::BayesianInference => vec![5, 7, 8],
            Self::LinearAlgebra => vec![1, 2, 4],
            Self::QueueingTheory => vec![10, 14, 23],
            Self::Topology => vec![1, 2, 9],
            Self::OptimalControl => vec![17, 22, 8],
            Self::Optimization => vec![19, 11, 12],
            Self::GameTheory => vec![24, 11, 29],
            Self::StatisticalInference => vec![12, 13, 22],
            Self::ProbabilityTheory => vec![22, 11, 13],
        }
    }
    pub fn secondary_concepts(&self) -> Vec<u8> {
        match self {
            Self::GraphTheory => vec![2, 3, 19, 22, 23],
            Self::NumericalAnalysis => vec![1, 2, 19, 22],
            Self::BayesianInference => vec![6, 10, 11, 14],
            Self::LinearAlgebra => vec![3, 13, 19],
            Self::QueueingTheory => vec![8, 12, 22],
            Self::Topology => vec![5, 8, 14],
            Self::OptimalControl => vec![3, 12, 13],
            Self::Optimization => vec![13, 16, 20],
            Self::GameTheory => vec![16, 19, 22],
            Self::StatisticalInference => vec![1, 2, 11],
            Self::ProbabilityTheory => vec![12, 14, 20],
        }
    }
    pub fn module_name(&self) -> &'static str {
        match self {
            Self::GraphTheory => "route_graph_engine",
            Self::NumericalAnalysis => "amm_curve_engine",
            Self::BayesianInference => "state_event_engine",
            Self::LinearAlgebra => "parity_redemption_engine",
            Self::QueueingTheory => "cex_external_engine",
            Self::Topology => "cross_domain_engine",
            Self::OptimalControl => "derivatives_engine",
            Self::Optimization => "credit_liquidation_engine",
            Self::GameTheory => "intents_solver_engine",
            Self::StatisticalInference => "nft_engine",
            Self::ProbabilityTheory => "prediction_engine",
        }
    }
}

#[derive(Debug, Clone)]
pub struct StrategyClassification {
    pub mev_id: String, pub group: u8, pub name: String, pub family: String,
    pub domain: MathDomain, pub atomic_possible: bool, pub nonatomic_possible: bool,
    pub min_legs: u32, pub max_legs: u32, pub concept_weights: HashMap<u8, f64>,
}

pub fn classify(mev_id: &str) -> Option<StrategyClassification> {
    let p: Vec<&str> = mev_id.split('-').collect();
    if p.len() != 3 || p[0] != "MEV" { return None; }
    let g: u8 = p[1].parse().ok()?;
    let i: u32 = p[2].parse().ok()?;
    let domain = match g {
        1 => MathDomain::GraphTheory, 2 => MathDomain::NumericalAnalysis,
        3 => MathDomain::BayesianInference, 4 => MathDomain::LinearAlgebra,
        5 => MathDomain::QueueingTheory, 6 => MathDomain::Topology,
        7 => MathDomain::OptimalControl, 8 => MathDomain::Optimization,
        9 => MathDomain::GameTheory, 10 => MathDomain::StatisticalInference,
        11 => MathDomain::ProbabilityTheory, _ => return None,
    };
    let pri = domain.primary_concepts();
    let sec = domain.secondary_concepts();
    let mut w = HashMap::new();
    for c in 1..=31 { w.insert(c, if pri.contains(&c) { 1.0 } else if sec.contains(&c) { 0.7 } else { 0.3 }); }
    let fam = ["","Spot DEX","AMM","State","Paridad","CEX-DEX","Cross-chain","Derivados","Credito","Intents","NFT","Prediction"];
    Some(StrategyClassification {
        mev_id: mev_id.to_string(), group: g,
        name: format!("MEV-{:02}-{:03}", g, i), family: fam[g as usize].to_string(),
        domain, atomic_possible: g != 5 && g != 6 && g != 10, nonatomic_possible: true,
        min_legs: if g == 8 { 1 } else { 2 },
        max_legs: match g { 5|6 => 16, 9 => if i==8 {16} else {12}, 11 => if i==5 {16} else {12}, _ => 8 },
        concept_weights: w,
    })
}

pub fn classify_all() -> Vec<StrategyClassification> {
    let c = [0,36,17,31,31,14,30,30,25,20,18,12];
    let mut r = Vec::with_capacity(264);
    for g in 1..=11 { for i in 1..=c[g] { if let Some(x) = classify(&format!("MEV-{:02}-{:03}",g,i)) { r.push(x); } } }
    r
}
