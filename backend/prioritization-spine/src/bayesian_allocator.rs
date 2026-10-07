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
    pub fraction: f64,   // Fracción del cap_usd_ceiling
    pub usd_amount: f64, // Monto absoluto en USD
    pub p_success_mean: f64,
    pub p_success_std: f64,
    pub kelly_fraction: f64,
    pub source: AllocationSource,
    /// KELLY-GUARD-01: decisión de sizing (ver `SizingDecision`). Separada de
    /// `source` para no confundir procedencia con razón.
    pub sizing: SizingDecision,
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
            sizing: SizingDecision::NotAttempted,
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

/// KELLY-GUARD-01: POR QUÉ el allocator dimensionó (o no). Campo SEPARADO de
/// `AllocationSource` a propósito: `source` describe la PROCEDENCIA de la
/// posterior (fresca vs prior), esto describe la DECISIÓN de sizing.
/// Mezclarlos era el defecto: un `fraction == 0` con `source = Prior` no decía
/// si fue "no hay apuesta" o "la fórmula no aplica".
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SizingDecision {
    /// La fórmula aplicó dentro de dominio y produjo un tamaño positivo.
    Sized,
    /// Ni se intentó por política previa (Ghost Protocol / posterior stale).
    NotAttempted,
    /// `expected_yield_ratio <= 0` (o NaN): la fórmula de Kelly **NO APLICA** —
    /// exige odds `b > 0`. Con `b < 0` devolvería un `f*` POSITIVO plausible
    /// (medido en t48: `b = -0.005437521`, `p = 0.95` ⇒ `f* = 10.14`), o sea
    /// "apostá 10.14×" en una ruta que pierde en todo tamaño positivo.
    NonPositiveYieldRatio,
    /// Dentro del dominio (`b > 0`) pero `f* <= 0`: **NO HAY APUESTA**. Es un
    /// hecho distinto del rechazo de dominio y por eso se nombra aparte.
    KellyNoEdge,
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
    /// El signal trae `sample_count` (nº de ejecuciones en la ventana de
    /// agregación) y `revert_rate` (fracción que revirtió ≈ p_fail).
    /// Reconstruimos successes/failures con `success_rate = 1 − revert_rate`
    /// y actualizamos la posterior beta:
    ///   successes = round(sample_count · (1 − revert_rate))
    ///   failures  = sample_count − successes
    pub fn ingest_signal(&self, signal: &AdaptiveSignal) {
        if signal.sample_count <= 0 {
            return;
        }
        let n = signal.sample_count as u64;
        let success_rate = (1.0 - signal.revert_rate).clamp(0.0, 1.0);
        let successes = (success_rate * n as f64).round() as u64;
        let failures = n.saturating_sub(successes);

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

        // Procedencia de la posterior — NO es la razón del sizing (ver
        // `SizingDecision`). Se calcula ANTES de las guardas para no perderla.
        let source = if matches!(map.get(&key), Some(p) if !p.is_stale()) {
            AllocationSource::Posterior
        } else {
            AllocationSource::Prior
        };

        // ── KELLY-GUARD-01: guarda de DOMINIO, antes de aplicar la fórmula ──
        // Kelly exige odds `b > 0`. El camino anterior hacía
        // `expected_yield_ratio.max(0.0)` — un clamp SILENCIOSO de `b` que
        // convertía un edge negativo en "no hay apuesta" y escondía que la
        // fórmula NO APLICA. Con `b < 0` la fórmula devuelve un número positivo
        // plausible: medido (t48) `b = -0.005437521`, `p = 0.95` →
        // `f* = (b·p − q)/b = 10.14`, o sea "apostá 10.14×" en una ruta que
        // pierde en TODO tamaño positivo (retorno marginal 0.994562479 < 1).
        // Forma NO negada (KELLY-GUARD-CLIPPY-01): `x <= 0.0 || !x.is_finite()`
        // es equivalente a `!(x > 0.0)` para todo valor finito y para NaN, y
        // ESTRICTAMENTE MAS ESTRICTA en `+Inf` — que el original dejaba pasar a
        // la fórmula. Además evita `clippy::neg_cmp_op_on_partial_ord`, que bajo
        // `-D warnings` rechaza la comparación negada sobre un tipo parcialmente
        // ordenado. NO se usa `#[allow]`: el lint se RESUELVE, no se silencia.
        if expected_yield_ratio <= 0.0 || !expected_yield_ratio.is_finite() {
            let mut alloc = Allocation::zero(strategy_kind.to_string(), chain_id, source);
            alloc.p_success_mean = p_mean;
            alloc.p_success_std = p_std;
            alloc.sizing = SizingDecision::NonPositiveYieldRatio;
            return alloc;
        }
        let b = expected_yield_ratio;

        // Kelly clásico: f* = (p·b - q) / b, con q = 1 - p
        let raw_kelly = ((p_mean * b) - (1.0 - p_mean)) / b;

        // Dentro del dominio, `f* <= 0` significa "no hay apuesta" (edge no
        // positivo). Se NOMBRA en vez de clamparse: el consumidor tiene que
        // poder distinguirlo del rechazo de dominio de arriba.
        // Misma forma no negada que la guarda precedente (KELLY-GUARD-CLIPPY-01):
        // equivalente para finito/NaN y MAS ESTRICTA en `+Inf` — que sin esto
        // llegaba a `clamp(0.0, KELLY_FRACTION_CAP)` como CAP y producía tamaño
        // positivo.
        if raw_kelly <= 0.0 || !raw_kelly.is_finite() {
            let mut alloc = Allocation::zero(strategy_kind.to_string(), chain_id, source);
            alloc.p_success_mean = p_mean;
            alloc.p_success_std = p_std;
            alloc.sizing = SizingDecision::KellyNoEdge;
            return alloc;
        }
        let kelly_pos = raw_kelly.clamp(0.0, KELLY_FRACTION_CAP);

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
            sizing: SizingDecision::Sized,
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
            revert_rate: 0.05, // success_rate 0.95
            sample_count: 200,
            received_at: SystemTime::now(),
        };
        a.ingest_signal(&signal);
        // yield 0.2: at a 5% edge Kelly needs p > 95.2%; the 0.95 posterior sits just
        // below that, so use a feasible edge to exercise the positive-allocation path.
        let alloc = a.assign("hf", 1, 1000.0, 0.2);
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
            revert_rate: 0.25, // success_rate 0.75
            sample_count: 4,
            received_at: SystemTime::now(),
        };
        a.ingest_signal(&signal_few);
        // chain_id 137 matches signal_few (the original literal 1000.0 was an f64 in a
        // u64 slot — a latent bug in this never-compiled test). yield 1.0 keeps BOTH
        // allocations above the Kelly threshold so the comparison isolates the
        // observation-count effect (few obs → wider posterior + lower mean → lower
        // fraction) instead of degenerating to 0 == 0 at a sub-threshold edge.
        let alloc_few = a.assign("hf", 137, 1000.0, 1.0);
        // 1000 obs estables → varianza pequeña → fracción mayor
        let signal_many = AdaptiveSignal {
            strategy_kind: "hf2".to_string(),
            chain_id: 137,
            revert_rate: 0.25, // success_rate 0.75
            sample_count: 1000,
            received_at: SystemTime::now(),
        };
        a.ingest_signal(&signal_many);
        let alloc_many = a.assign("hf2", 137, 1000.0, 1.0);
        assert!(alloc_many.fraction > alloc_few.fraction);
    }

    // ──────────────────────────────────────────────────────────────────────
    // OMEGA-8/M4 Fase 5: table-driven invariant tests for BayesianAllocator
    // ──────────────────────────────────────────────────────────────────────
    //
    // We use table-driven `#[test]` blocks (no proptest dep) to exercise the
    // six invariants over a representative sample of inputs. Proptest was
    // dropped because regenerating Cargo.lock under `--locked` CI requires
    // a separate sprint; the invariants below cover the same surface with
    // hand-picked corner cases plus a few interior points.
    //
    // Invariants:
    //   (i)   assign(cap=0, ...) → usd_amount == 0 (Ghost Protocol).
    //   (ii)  fraction ∈ [0, KELLY_FRACTION_CAP].
    //   (iii) usd_amount ≤ cap_usd_ceiling.
    //   (iv)  ROI ≤ 0 with prior Beta(1,1) → kelly_pos == 0.
    //   (v)   determinism: same input → same output.
    //   (vi)  monotonicity in success_rate.

    /// OMEGA-8/M4 Fase 5 (i): Ghost Protocol invariant — cap_usd_ceiling ≤ 0
    /// ⇒ Allocation::zero(GhostProtocol) regardless of yield_ratio sign.
    #[test]
    fn ghost_protocol_dominates_any_yield_table() {
        let a = BayesianAllocator::new();
        for &y in &[-10.0_f64, -1.0, -0.001, 0.0, 0.001, 1.0, 10.0] {
            let alloc = a.assign("hf", 1, 0.0, y);
            assert_eq!(alloc.usd_amount, 0.0, "yield={y} broke ghost cap=0");
            assert_eq!(alloc.fraction, 0.0);
            assert_eq!(alloc.source, AllocationSource::GhostProtocol);
        }
    }

    /// OMEGA-8/M4 Fase 5 (ii, iii): bounded fraction + usd ≤ cap.
    /// Sweep a representative table; never recommend more than the cap.
    #[test]
    fn fraction_is_bounded_and_usd_within_cap_table() {
        let cases: &[(f64, u64, f64, f64)] = &[
            // (success_rate, n_obs, cap, yield)
            (0.0, 0, 1_000.0, 0.05),
            (1.0, 1, 1_000.0, 0.05),
            (0.5, 100, 1_000_000.0, 0.5),
            (0.99, 1000, 0.01, 0.001),
            (0.75, 50, 500.0, 2.0),
            (0.25, 10_000, 100.0, 0.0),
        ];
        for &(sr, n, cap, y) in cases {
            let a = BayesianAllocator::new();
            if n > 0 {
                a.ingest_signal(&AdaptiveSignal {
                    strategy_kind: "k".to_string(),
                    chain_id: 1,
                    revert_rate: 1.0 - sr,
                    sample_count: n as i64,
                    received_at: SystemTime::now(),
                });
            }
            let alloc = a.assign("k", 1, cap, y);
            assert!(
                alloc.fraction >= 0.0,
                "fraction < 0 for case {:?}",
                (sr, n, cap, y)
            );
            assert!(
                alloc.fraction <= KELLY_FRACTION_CAP + 1e-9,
                "fraction {} > KELLY cap {} for case {:?}",
                alloc.fraction,
                KELLY_FRACTION_CAP,
                (sr, n, cap, y)
            );
            assert!(alloc.usd_amount >= 0.0);
            assert!(
                alloc.usd_amount <= cap + 1e-6,
                "usd_amount {} > cap {} for case {:?}",
                alloc.usd_amount,
                cap,
                (sr, n, cap, y)
            );
        }
    }

    /// OMEGA-8/M4 Fase 5 (v): determinism — same (cap, yield) on a fresh
    /// allocator with no signal must produce identical Allocation fields.
    /// The ranking must be stable per turn.
    #[test]
    fn determinism_on_prior_table() {
        for &(cap, y) in &[(1.0_f64, 0.0_f64), (100.0, 0.1), (999.99, 0.5)] {
            let a = BayesianAllocator::new();
            let alloc1 = a.assign("k", 1, cap, y);
            let alloc2 = a.assign("k", 1, cap, y);
            assert_eq!(alloc1.fraction, alloc2.fraction);
            assert_eq!(alloc1.usd_amount, alloc2.usd_amount);
            assert_eq!(alloc1.p_success_mean, alloc2.p_success_mean);
            assert_eq!(alloc1.kelly_fraction, alloc2.kelly_fraction);
        }
    }

    /// OMEGA-8/M4 Fase 5 (vi): monotonicity in success_rate.
    /// Holding (cap, yield, n_obs) fixed, increasing the observed success
    /// rate from low to high must NOT decrease the recommended fraction.
    #[test]
    fn success_rate_monotonicity_table() {
        let cases: &[(f64, f64, u64, f64)] = &[
            // (cap, yield, n_obs, low_rate); high_rate = low_rate + 0.5 capped at 0.99
            (1_000.0, 0.1, 100, 0.2),
            (10_000.0, 0.3, 200, 0.3),
            (500.0, 0.05, 50, 0.1),
        ];
        for &(cap, y, n, low) in cases {
            let a_low = BayesianAllocator::new();
            a_low.ingest_signal(&AdaptiveSignal {
                strategy_kind: "k".to_string(),
                chain_id: 1,
                revert_rate: 1.0 - low,
                sample_count: n as i64,
                received_at: SystemTime::now(),
            });
            let alloc_low = a_low.assign("k", 1, cap, y);

            let high = (low + 0.5).min(0.99);
            let a_hi = BayesianAllocator::new();
            a_hi.ingest_signal(&AdaptiveSignal {
                strategy_kind: "k".to_string(),
                chain_id: 1,
                revert_rate: 1.0 - high,
                sample_count: n as i64,
                received_at: SystemTime::now(),
            });
            let alloc_hi = a_hi.assign("k", 1, cap, y);

            assert!(
                alloc_hi.fraction >= alloc_low.fraction - 1e-9,
                "higher rate {} produced LOWER fraction {} vs low rate {} fraction {} (case: cap={cap}, y={y}, n={n})",
                high, alloc_hi.fraction, low, alloc_low.fraction
            );
        }
    }

    /// OMEGA-8/M4 Fase 5: BetaPosterior invariants — mean ∈ [0,1] and
    /// variance ≥ 0 for any (alpha, beta) > 0.
    #[test]
    fn beta_posterior_mean_in_unit_interval_table() {
        let cases: &[(u64, u64)] = &[
            (0, 0),
            (1, 0),
            (0, 1),
            (100, 100),
            (1_000, 1),
            (1, 1_000),
            (100_000, 100_000),
        ];
        for &(s, f) in cases {
            let mut p = BetaPosterior::new_prior();
            p.update(s, f);
            let m = p.mean();
            assert!(
                (0.0..=1.0).contains(&m),
                "mean {} out of [0,1] for ({s},{f})",
                m
            );
            assert!(p.variance() >= 0.0);
            assert!(p.std_dev() >= 0.0);
        }
    }

    // ── KELLY-GUARD-01: dominio de Kelly (b > 0) ─────────────────────────────

    /// El PELIGRO medido, fijado como número ejecutable: con `b < 0` la fórmula
    /// clásica `f* = (b·p − q)/b` devuelve un POSITIVO plausible.
    /// `b = -0.005437521`, `p = 0.95` ⇒ `f* ≈ 10.145` (t48 lo midió como 10.14).
    /// No es un caso borde: es la fórmula FUERA DE DOMINIO, y por eso la guarda
    /// va antes de aplicarla.
    #[test]
    fn kelly_out_of_domain_formula_would_return_10x_measured_hazard() {
        let b: f64 = -0.005437521;
        let p: f64 = 0.95;
        let q: f64 = 1.0 - p;
        let f_star = ((b * p) - q) / b;
        assert!(
            (f_star - 10.1453).abs() < 0.01,
            "f* = {f_star}, esperado ≈ 10.145 (el 10.14 que midió t48)"
        );
        assert!(
            f_star > 0.0,
            "el peligro es que sea POSITIVO, no que sea raro"
        );
    }

    /// NEGATIVO: un edge negativo (`b <= 0`, incluido NaN) se RECHAZA NOMBRADO y
    /// con tamaño 0 — nunca un `f*` positivo, nunca un clamp silencioso.
    #[test]
    fn kelly_non_positive_yield_is_named_rejection_table() {
        let cap = 1_000.0_f64;
        for &y in &[
            -0.005437521_f64, // el b medido por t48
            -1.0,
            -10.0,
            0.0,               // frontera: odds nulas ⇒ no es una apuesta
            f64::NAN,          // fail-closed: no es finito
            f64::INFINITY,     // KELLY-GUARD-CLIPPY-01: +Inf PASA a rechazarse (endurece)
            f64::NEG_INFINITY, // -Inf: rechazada por `<= 0.0`
        ] {
            let a = BayesianAllocator::new();
            let alloc = a.assign("k", 1, cap, y);
            assert_eq!(alloc.fraction, 0.0, "yield={y} debió dar fracción 0");
            assert_eq!(alloc.usd_amount, 0.0, "yield={y} debió dar usd 0");
            assert_eq!(
                alloc.sizing,
                SizingDecision::NonPositiveYieldRatio,
                "yield={y} debió RECHAZARSE por dominio, nombrado"
            );
            assert_ne!(
                alloc.sizing,
                SizingDecision::KellyNoEdge,
                "yield={y}: 'la fórmula no aplica' NO es 'no hay apuesta'"
            );
        }
    }

    /// POSITIVO: `b > 0` con edge real ⇒ la fórmula aplica y el tamaño es normal
    /// (> 0), con la cota dura respetada. Sin este lado el negativo no prueba
    /// nada.
    #[test]
    fn kelly_positive_domain_still_sizes() {
        let a = BayesianAllocator::new();
        a.ingest_signal(&AdaptiveSignal {
            strategy_kind: "k".to_string(),
            chain_id: 1,
            revert_rate: 0.05,
            sample_count: 100,
            received_at: SystemTime::now(),
        });
        let cap = 1_000.0;
        // b = 0.2 (net odds 20%): f* = (0.9412*0.2 - 0.0588)/0.2 ≈ 0.647 → cap 0.5
        let alloc = a.assign("k", 1, cap, 0.2);
        assert_eq!(alloc.source, AllocationSource::Posterior);
        assert_eq!(alloc.sizing, SizingDecision::Sized);
        assert!(
            alloc.kelly_fraction > 0.0,
            "un edge positivo debe producir fracción Kelly > 0 (fue {})",
            alloc.kelly_fraction
        );
        assert!(
            (alloc.kelly_fraction - KELLY_FRACTION_CAP).abs() < 1e-9,
            "kelly_pos = {} debería tocar la cota dura {KELLY_FRACTION_CAP}",
            alloc.kelly_fraction
        );
        assert!(
            alloc.fraction > 0.0,
            "fraction debe ser > 0 con edge positivo"
        );
        assert!(alloc.usd_amount > 0.0);
        assert!(alloc.fraction <= KELLY_FRACTION_CAP + 1e-9);
        assert!(alloc.usd_amount <= cap + 1e-6);
    }

    /// Distinción obligatoria: `b > 0` SIN edge ⇒ `KellyNoEdge` (no hay
    /// apuesta), que NO es lo mismo que `NonPositiveYieldRatio` (la fórmula no
    /// aplica). Los dos dan tamaño 0 por razones DISTINTAS y nombradas.
    #[test]
    fn kelly_no_edge_is_named_and_distinct_from_domain_rejection() {
        let a = BayesianAllocator::new(); // prior Beta(1,1) ⇒ p = 0.5
                                          // f* = (0.5*0.1 - 0.5)/0.1 = -4.5 ⇒ no hay apuesta DENTRO del dominio
        let no_edge = a.assign("k", 1, 1_000.0, 0.1);
        assert_eq!(no_edge.sizing, SizingDecision::KellyNoEdge);
        assert_eq!(no_edge.fraction, 0.0);
        assert_eq!(no_edge.usd_amount, 0.0);
        let domain = a.assign("k", 1, 1_000.0, -0.1);
        assert_eq!(domain.sizing, SizingDecision::NonPositiveYieldRatio);
        assert_ne!(
            no_edge.sizing, domain.sizing,
            "los dos ceros deben ser distinguibles"
        );
    }
}
