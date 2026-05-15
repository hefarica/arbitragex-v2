//! Bayesian Allocator — Dynamic Capital Assignment via Posterior Inference.
//!
//! Cierra el bucle de adaptación asintótica (S3/S6) consumiendo los signals
//! del módulo `feedback` y produciendo asignaciones de capital coherentes con
//! la convex optimization on configuration manifold descrita en C1 del
//! Super-Prompt Ω-S5++.∞.
//!
//! ## Modelo matemático
//!
//! Para cada par `(strategy_kind, chain_id)` se mantiene una distribución
//! beta posterior sobre la probabilidad de éxito `p_success` actualizada con
//! la regla bayesiana clásica:
//!
//!   α_post = α_prior + successes
//!   β_post = β_prior + failures
//!   E[p_success] = α_post / (α_post + β_post)
//!   Var[p_success] = α·β / ((α+β)²·(α+β+1))
//!
//! La asignación de capital se calcula con la formulación de **Kelly fraction
//! conservadora** (½-Kelly) atenuada por la varianza posterior:
//!
//!   f_kelly = (p·b − q) / b              con b = expected_yield_ratio
//!   f_assigned = min(0.5·f_kelly, cap_usd_ceiling) · (1 − k·σ)
//!
//! donde `k` es el factor de aversión a la decoherencia (default 2.0).
//!
//! ## Ghost Protocol
//!
//! Si el operador soberano no ha firmado escalación, `cap_usd_ceiling = 0.00`
//! y el allocator devuelve `Allocation::zero()` siempre — invariante absoluta.
//!
//! ## Lexicón Absoluto
//!
//! Cero stub/dummy/placeholder/TODO/FIXME en código productivo.

use crate::feedback::AdaptiveSignal;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use std::time::{Duration, SystemTime};

/// Factor de aversión a la decoherencia (½-Kelly atenuado por σ).
const KAPPA_VARIANCE_AVERSION: f64 = 2.0;

/// Fracción Kelly máxima absoluta (cota dura por encima del ½-Kelly).
const KELLY_FRACTION_CAP: f64 = 0.5;

/// TTL de un posterior antes de marcarse como stale.
pub const POSTERIOR_TTL: Duration = Duration::from_secs(900);

/// Distribución beta posterior sobre la probabilidad de éxito.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BetaPosterior {
    pub alpha: f64,
    pub beta: f64,
    pub last_updated: SystemTime,
}

impl BetaPosterior {
    pub fn new_prior() -> Self {
        // Prior uniforme Beta(1,1) — equivalente a desconocimiento.
        Self {
            alpha: 1.0,
            beta: 1.0,
            last_updated: SystemTime::now(),
        }
    }

    /// Esperanza de la posterior.
    pub fn mean(&self) -> f64 {
        self.alpha / (self.alpha + self.beta)
    }

    /// Varianza de la posterior.
    pub fn variance(&self) -> f64 {
        let n = self.alpha + self.beta;
        (self.alpha * self.beta) / (n * n * (n + 1.0))
    }

    /// Desviación estándar de la posterior.
    pub fn std_dev(&self) -> f64 {
        self.variance().sqrt()
    }

    /// Actualiza con observaciones reales (no fabricadas).
    pub fn update(&mut self, successes: u64, failures: u64) {
        self.alpha += successes as f64;
        self.beta += failures as f64;
        self.last_updated = SystemTime::now();
    }

    pub fn is_stale(&self) -> bool {
        SystemTime::now()
            .duration_since(self.last_updated)
            .map(|d| d >= POSTERIOR_TTL)
            .unwrap_or(true)
    }
}

/// Asignación de capital resultante para un par (strategy, chain).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Allocation {
    pub strategy_kind: String,
    pub chain_id: u64,
    pub fraction: f64,            // Fracción del cap_usd_ceiling
    pub usd_amount: f64,          // Monto absoluto en USD
    pub p_success_mean: f64,
    pub p_success_std: f64,
    pub kelly_fraction: f64,
    pub source: AllocationSource,
}

impl Allocation {
    pub fn zero(strategy_kind: String, chain_id: u64, source: AllocationSource) -> Self {
        Self {
            strategy_kind,
            chain_id,
            fraction: 0.0,
            usd_amount: 0.0,
            p_success_mean: 0.0,
            p_success_std: 0.0,
            kelly_fraction: 0.0,
            source,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AllocationSource {
    /// Posterior bayesiana fresca (TTL ok).
    Posterior,
    /// Cota dura por Ghost Protocol (cap_usd_ceiling = 0.00).
    GhostProtocol,
    /// Stale → fallback conservador a cero.
    StalePosterior,
    /// Sin observaciones suficientes → prior puro.
    Prior,
}

/// Allocator concurrent-safe.  Una sola instancia por proceso searcher-rs.
#[derive(Debug)]
pub struct BayesianAllocator {
    posteriors: Arc<RwLock<HashMap<(String, u64), BetaPosterior>>>,
}

impl Default for BayesianAllocator {
    fn default() -> Self {
        Self {
            posteriors: Arc::new(RwLock::new(HashMap::new())),
        }
    }
}

impl BayesianAllocator {
    pub fn new() -> Self {
        Self::default()
    }

    /// Incorpora un `AdaptiveSignal` proveniente del módulo `feedback`.
    ///
    /// El signal trae `n_observations` y `success_rate`; reconstruimos
    /// successes/failures y actualizamos la posterior.
    pub fn ingest_signal(&self, signal: &AdaptiveSignal) {
        if signal.n_observations == 0 {
            return;
        }
        let successes =
            (signal.success_rate.clamp(0.0, 1.0) * signal.n_observations as f64).round() as u64;
        let failures = signal.n_observations.saturating_sub(successes);

        let key = (signal.strategy_kind.clone(), signal.chain_id);
        let mut map = self.posteriors.write().unwrap();
        let post = map.entry(key).or_insert_with(BetaPosterior::new_prior);
        post.update(successes, failures);
    }

    /// Calcula la asignación bayesiana para `(strategy, chain)` dada
    /// `cap_usd_ceiling` (del operator_parametrization) y la ratio de retorno
    /// esperado `expected_yield_ratio = (gross_profit / capital_at_risk)`.
    ///
    /// Si `cap_usd_ceiling == 0.0` → siempre `Allocation::zero(GhostProtocol)`.
    pub fn assign(
        &self,
        strategy_kind: &str,
        chain_id: u64,
        cap_usd_ceiling: f64,
        expected_yield_ratio: f64,
    ) -> Allocation {
        if cap_usd_ceiling <= 0.0 {
            return Allocation::zero(
                strategy_kind.to_string(),
                chain_id,
                AllocationSource::GhostProtocol,
            );
        }

        let key = (strategy_kind.to_string(), chain_id);
        let map = self.posteriors.read().unwrap();
        let post = match map.get(&key) {
            Some(p) if !p.is_stale() => p.clone(),
            Some(_) => {
                return Allocation::zero(
                    strategy_kind.to_string(),
                    chain_id,
                    AllocationSource::StalePosterior,
                );
            }
            None => BetaPosterior::new_prior(),
        };

        let p_mean = post.mean();
        let p_std = post.std_dev();
        let b = expected_yield_ratio.max(0.0);

        // Kelly clásico: f* = (p·b - q) / b, con q = 1 - p
        let raw_kelly = if b > 0.0 {
            ((p_mean * b) - (1.0 - p_mean)) / b
        } else {
            0.0
        };
        let kelly_pos = raw_kelly.max(0.0).min(KELLY_FRACTION_CAP);

        // Atenuación por varianza: cuanto mayor σ, menor confianza, menor fracción.
        let variance_penalty = (1.0 - KAPPA_VARIANCE_AVERSION * p_std).max(0.0);
        let fraction = kelly_pos * variance_penalty;
        let usd_amount = (fraction * cap_usd_ceiling).max(0.0);

        let source = if matches!(map.get(&key), Some(p) if !p.is_stale()) {
            AllocationSource::Posterior
        } else {
            AllocationSource::Prior
        };

        Allocation {
            strategy_kind: strategy_kind.to_string(),
            chain_id,
            fraction,
            usd_amount,
            p_success_mean: p_mean,
            p_success_std: p_std,
            kelly_fraction: kelly_pos,
            source,
        }
    }

    /// Snapshot diagnóstico para `/api/admin/allocator/snapshot`.
    pub fn snapshot(&self) -> Vec<((String, u64), BetaPosterior)> {
        let map = self.posteriors.read().unwrap();
        map.iter().map(|(k, v)| (k.clone(), v.clone())).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ghost_protocol_always_zero() {
        let a = BayesianAllocator::new();
        let alloc = a.assign("hf", 1, 0.0, 0.05);
        assert_eq!(alloc.usd_amount, 0.0);
        assert_eq!(alloc.source, AllocationSource::GhostProtocol);
    }

    #[test]
    fn prior_uniform_returns_zero_fraction_when_yield_low() {
        let a = BayesianAllocator::new();
        // Prior Beta(1,1) → p_mean = 0.5; yield 0.01 → kelly negativo → 0.
        let alloc = a.assign("hf", 1, 1000.0, 0.01);
        assert_eq!(alloc.fraction, 0.0);
        assert_eq!(alloc.source, AllocationSource::Prior);
    }

    #[test]
    fn posterior_after_successes_increases_allocation() {
        let a = BayesianAllocator::new();
        let signal = AdaptiveSignal {
            strategy_kind: "hf".to_string(),
            chain_id: 1,
            success_rate: 0.95,
            n_observations: 200,
            published_at: SystemTime::now(),
        };
        a.ingest_signal(&signal);
        let alloc = a.assign("hf", 1, 1000.0, 0.05);
        assert!(alloc.fraction > 0.0);
        assert_eq!(alloc.source, AllocationSource::Posterior);
        assert!(alloc.p_success_mean > 0.9);
    }

    #[test]
    fn variance_penalty_attenuates_with_few_observations() {
        let a = BayesianAllocator::new();
        // 4 obs apenas → varianza alta → fracción muy reducida
        let signal_few = AdaptiveSignal {
            strategy_kind: "hf".to_string(),
            chain_id: 137,
            success_rate: 0.75,
            n_observations: 4,
            published_at: SystemTime::now(),
        };
        a.ingest_signal(&signal_few);
        let alloc_few = a.assign("hf", 1000.0, 1000.0, 0.05);
        // 1000 obs estables → varianza pequeña → fracción mayor
        let signal_many = AdaptiveSignal {
            strategy_kind: "hf2".to_string(),
            chain_id: 137,
            success_rate: 0.75,
            n_observations: 1000,
            published_at: SystemTime::now(),
        };
        a.ingest_signal(&signal_many);
        let alloc_many = a.assign("hf2", 137, 1000.0, 0.05);
        assert!(alloc_many.fraction > alloc_few.fraction);
    }

    // ──────────────────────────────────────────────────────────────────────
    // OMEGA-8/M4 Fase 5: property tests for BayesianAllocator
    // ──────────────────────────────────────────────────────────────────────
    //
    // Invariants we want to hold under any (cap, yield, success_rate, n_obs):
    //   (i)   assign(cap=0, ...) → usd_amount == 0 (Ghost Protocol).
    //   (ii)  fraction ∈ [0, KELLY_FRACTION_CAP].
    //   (iii) usd_amount ≤ cap_usd_ceiling.
    //   (iv)  ROI ≤ 0 with prior Beta(1,1) → kelly_pos == 0.
    //   (v)   determinism: same input → same output.
    //   (vi)  monotonicity-ish: holding (n_obs, yield, cap) fixed and ramping
    //         success_rate from low to high never decreases fraction.

    use proptest::prelude::*;

    proptest! {
        /// OMEGA-8/M4 Fase 5: Ghost Protocol invariant.
        /// cap_usd_ceiling ≤ 0 ⇒ Allocation::zero(GhostProtocol).
        #[test]
        fn ghost_protocol_dominates_any_yield(yield_ratio in -10.0f64..10.0f64) {
            let a = BayesianAllocator::new();
            let alloc = a.assign("hf", 1, 0.0, yield_ratio);
            prop_assert_eq!(alloc.usd_amount, 0.0);
            prop_assert_eq!(alloc.fraction, 0.0);
            prop_assert_eq!(alloc.source, AllocationSource::GhostProtocol);
        }

        /// OMEGA-8/M4 Fase 5: fraction is bounded ∈ [0, KELLY_FRACTION_CAP]
        /// and usd_amount never exceeds the ceiling. The bayesian allocator
        /// must never recommend a position larger than the cap (matching the
        /// Risk Engine institutional discipline).
        #[test]
        fn fraction_is_bounded_and_usd_within_cap(
            success_rate in 0.0f64..1.0f64,
            n_obs in 0u64..10_000u64,
            cap in 0.01f64..1_000_000.0f64,
            yield_ratio in 0.0f64..2.0f64,
        ) {
            let a = BayesianAllocator::new();
            if n_obs > 0 {
                a.ingest_signal(&AdaptiveSignal {
                    strategy_kind: "k".to_string(),
                    chain_id: 1,
                    success_rate,
                    n_observations: n_obs,
                    published_at: SystemTime::now(),
                });
            }
            let alloc = a.assign("k", 1, cap, yield_ratio);
            prop_assert!(alloc.fraction >= 0.0, "fraction must be >= 0");
            prop_assert!(alloc.fraction <= KELLY_FRACTION_CAP + 1e-9,
                         "fraction {} exceeded KELLY cap {}",
                         alloc.fraction, KELLY_FRACTION_CAP);
            prop_assert!(alloc.usd_amount >= 0.0, "usd_amount must be >= 0");
            prop_assert!(alloc.usd_amount <= cap + 1e-6,
                         "usd_amount {} exceeded cap {}",
                         alloc.usd_amount, cap);
        }

        /// OMEGA-8/M4 Fase 5: determinism — same (cap, yield) on a fresh
        /// allocator with no signal must produce identical Allocation
        /// fields (prior path). The ranking must be stable per turn.
        #[test]
        fn determinism_on_prior(
            cap in 0.01f64..1_000.0f64,
            yield_ratio in 0.0f64..1.0f64,
        ) {
            let a = BayesianAllocator::new();
            let alloc1 = a.assign("k", 1, cap, yield_ratio);
            let alloc2 = a.assign("k", 1, cap, yield_ratio);
            prop_assert_eq!(alloc1.fraction, alloc2.fraction);
            prop_assert_eq!(alloc1.usd_amount, alloc2.usd_amount);
            prop_assert_eq!(alloc1.p_success_mean, alloc2.p_success_mean);
            prop_assert_eq!(alloc1.kelly_fraction, alloc2.kelly_fraction);
        }

        /// OMEGA-8/M4 Fase 5: monotonicity in success_rate.
        /// Holding (cap, yield, n_obs) fixed, increasing the observed success
        /// rate from low to high must NOT decrease the recommended fraction.
        /// Equivalently: a strategy that wins more often gets at least as
        /// much capital, never less.
        #[test]
        fn success_rate_monotonicity(
            cap in 100.0f64..10_000.0f64,
            yield_ratio in 0.05f64..0.5f64,
            n_obs in 50u64..500u64,
            low_rate in 0.1f64..0.4f64,
        ) {
            // Two independent allocators so the signals don't contaminate.
            let a_low = BayesianAllocator::new();
            a_low.ingest_signal(&AdaptiveSignal {
                strategy_kind: "k".to_string(),
                chain_id: 1,
                success_rate: low_rate,
                n_observations: n_obs,
                published_at: SystemTime::now(),
            });
            let alloc_low = a_low.assign("k", 1, cap, yield_ratio);

            let high_rate = (low_rate + 0.5).min(0.99);
            let a_hi = BayesianAllocator::new();
            a_hi.ingest_signal(&AdaptiveSignal {
                strategy_kind: "k".to_string(),
                chain_id: 1,
                success_rate: high_rate,
                n_observations: n_obs,
                published_at: SystemTime::now(),
            });
            let alloc_hi = a_hi.assign("k", 1, cap, yield_ratio);

            prop_assert!(alloc_hi.fraction >= alloc_low.fraction - 1e-9,
                         "higher success_rate {} produced LOWER fraction {} vs low {}",
                         high_rate, alloc_hi.fraction, alloc_low.fraction);
        }
    }

    /// OMEGA-8/M4 Fase 5: BetaPosterior invariants — mean ∈ [0,1] and
    /// variance ≥ 0 for any (alpha, beta) > 0.
    proptest! {
        #[test]
        fn beta_posterior_mean_in_unit_interval(
            successes in 0u64..100_000u64,
            failures in 0u64..100_000u64,
        ) {
            let mut p = BetaPosterior::new_prior();
            p.update(successes, failures);
            let m = p.mean();
            prop_assert!(m >= 0.0 && m <= 1.0, "mean {} out of [0,1]", m);
            prop_assert!(p.variance() >= 0.0, "variance must be non-negative");
            prop_assert!(p.std_dev() >= 0.0, "std_dev must be non-negative");
        }
    }
}
