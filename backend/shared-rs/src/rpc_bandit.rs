//! WO-14 (PERF-STACK-2026-09-20): Thompson Sampling + hazard EWMA para
//! selección de proveedor RPC.
//!
//! Veredicto de diseño (Hermes run_273901de4b384d7a94ca0326e31cbe67, revisado
//! por -61 con port-with-validation):
//! - Thompson Sampling Beta(α,β): ADOPTADO. O(1) por request, exploración
//!   óptima asintótica, sin deps nuevas (rand_distr ya en workspace).
//! - LSTM/tch: RECHAZADO — dep C++ ~GB + inferencia por request en hot-path.
//!   Sustituto: hazard EWMA por proveedor (misma señal, costo O(1)).
//! - El "MPC" propuesto ES un producto de scores normalizado, no un QP: se
//!   implementa en `rpc_allocation` (Task 3) con nombre honesto.
//! - Correcciones al draft Hermes: el olvido exponencial DECRECE
//!   (exp(-γ·Δ), NO exp(+γ·Δ)); el hazard es POR PROVEEDOR, no global; sin
//!   perturbación de Gumbel (Thompson ya explora); sin dep statrs.

use rand::Rng;
use rand_distr::{Beta, Distribution};

/// Suficiencia conjugada por brazo (proveedor). La actualización fraccional
/// mantiene E[X] = α/(α+β) = reward medio observado (éxito rápido empuja α
/// más que éxito lento; el fallo empuja β completo).
pub struct ConjugateArm {
    alpha: f64,
    beta: f64,
    tau_sum_latency_ms: f64,
    n: u64,
}

const MEMORY_DECAY: f64 = 0.99;
const MEMORY_DECAY_AFTER: u64 = 1000;

impl ConjugateArm {
    fn new() -> Self {
        // Prior Beta(1,1) uniforme: sin datos, todos los proveedores igual
        // de creíbles (fail-honest, R8).
        Self { alpha: 1.0, beta: 1.0, tau_sum_latency_ms: 0.0, n: 0 }
    }

    fn update(&mut self, success: bool, latency_ms: f64) {
        let reward = (1.0 + latency_ms / 100.0).recip().clamp(0.0, 1.0);
        if success {
            self.alpha += reward;
            self.beta += 1.0 - reward;
        } else {
            self.beta += 1.0;
        }
        self.tau_sum_latency_ms += latency_ms;
        self.n += 1;

        // Decaimiento de memoria: regime no-estacionario (free-tier congesta
        // en horas pico); sin esto la posterior se congela en historia vieja.
        if self.n > MEMORY_DECAY_AFTER {
            self.alpha *= MEMORY_DECAY;
            self.beta *= MEMORY_DECAY;
            self.n = (self.n as f64 * MEMORY_DECAY) as u64;
        }
    }

    fn posterior_mean(&self) -> f64 {
        self.alpha / (self.alpha + self.beta)
    }

    fn sample<R: Rng>(&self, rng: &mut R) -> f64 {
        let d = Beta::new(self.alpha, self.beta).unwrap_or_else(|_| {
            // α,β > 0 siempre por construcción; unreachable salvo NaN externo.
            Beta::new(1.0, 1.0).expect("valid uniform beta")
        });
        d.sample(rng)
    }
}

/// Observación de un intento contra un proveedor.
pub struct ArmObservation {
    pub success: bool,
    pub latency_ms: f64,
}

/// Bandit Thompson sobre el pool de proveedores. `select` samplea
/// θᵢ ~ Beta(αᵢ,βᵢ) y devuelve argmax: exploración automática al inicio,
/// explotación cuando las posteriors se separan.
pub struct ThompsonSamplingSelector {
    arms: Vec<ConjugateArm>,
}

impl ThompsonSamplingSelector {
    pub fn new(provider_count: usize) -> Self {
        Self { arms: (0..provider_count).map(|_| ConjugateArm::new()).collect() }
    }

    pub fn select<R: Rng>(&mut self, rng: &mut R) -> usize {
        let mut best = 0usize;
        let mut best_theta = f64::NEG_INFINITY;
        for (i, arm) in self.arms.iter().enumerate() {
            let theta = arm.sample(rng);
            if theta > best_theta {
                best_theta = theta;
                best = i;
            }
        }
        best
    }

    pub fn update(&mut self, idx: usize, obs: ArmObservation) {
        if let Some(arm) = self.arms.get_mut(idx) {
            arm.update(obs.success, obs.latency_ms);
        }
    }

    pub fn posterior_mean(&self, idx: usize) -> Option<f64> {
        self.arms.get(idx).map(|a| a.posterior_mean())
    }

    pub fn len(&self) -> usize {
        self.arms.len()
    }

    pub fn is_empty(&self) -> bool {
        self.arms.is_empty()
    }
}

/// Hazard EWMA de 429 POR PROVEEDOR (sustituto O(1) del LSTM propuesto).
/// λ ← γ·1{429} + (1-γ)·λ en fallo por rate-limit; λ ← (1-γ)·λ en éxito;
/// decaimiento adicional por ventana de tiempo inactiva. La predicción es
/// suavizado exponencial del indicador, no una secuencia: misma señal a
/// costo O(1) y actualización online.
pub struct HazardEwma429 {
    lambda: f64,
    gamma: f64,
    window_ms: u64,
    last_update_ms: u64,
}

impl HazardEwma429 {
    pub fn new(gamma: f64, window_ms: u64) -> Self {
        Self {
            lambda: 0.0,
            gamma: gamma.clamp(f64::MIN_POSITIVE, 1.0),
            window_ms: window_ms.max(1),
            // 0 = sin marca previa: el primer record() no decae (R8).
            last_update_ms: 0,
        }
    }

    pub fn record(&mut self, hit_limit: bool, now_ms: u64) {
        if self.last_update_ms > 0 && now_ms > self.last_update_ms {
            let elapsed = now_ms - self.last_update_ms;
            // Olvido exponencial DECRECIENTE por inactividad: cada ventana
            // transcurrida multiplica λ por (1-γ). (Signo corregido vs
            // draft Hermes, que hacía crecer el peso con el tiempo.)
            let windows = (elapsed / self.window_ms) as u32;
            self.lambda *= (1.0 - self.gamma).powi(windows as i32);
        }
        self.lambda = if hit_limit {
            self.gamma + (1.0 - self.gamma) * self.lambda
        } else {
            (1.0 - self.gamma) * self.lambda
        };
        self.last_update_ms = now_ms;
    }

    /// Riesgo instantáneo estimado ∈ [0,1].
    pub fn hazard(&self) -> f64 {
        self.lambda.clamp(0.0, 1.0)
    }

    /// Pre-throttle: true cuando el riesgo estimado supera el umbral.
    pub fn should_pre_throttle(&self, threshold: f64) -> bool {
        self.hazard() >= threshold
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::rngs::StdRng;
    use rand::SeedableRng;

    #[test]
    fn select_returns_valid_index_on_cold_start() {
        let mut s = ThompsonSamplingSelector::new(3);
        let mut rng = StdRng::seed_from_u64(42);
        let i = s.select(&mut rng);
        assert!(i < 3);
    }

    #[test]
    fn empty_pool_is_honest() {
        let mut s = ThompsonSamplingSelector::new(0);
        let mut rng = StdRng::seed_from_u64(1);
        // 0 proveedores: índice 0 con len 0 = contrato del iterador vacío;
        // el caller (rpc_failover) nunca construye un pool vacío (fail-fast
        // en config), este test documenta el comportamiento del borde.
        assert_eq!(s.len(), 0);
        assert!(s.posterior_mean(0).is_none());
        let _ = s.select(&mut rng);
    }

    /// Vectores INDEPENDIENTES (no recomputan la fórmula): tras 2000 tiradas
    /// con un proveedor siempre exitoso/rápido y otro siempre 429, el bandit
    /// debe concentrar >80% de sus picks en el sano.
    #[test]
    fn thompson_converges_to_healthy_provider() {
        let mut s = ThompsonSamplingSelector::new(2);
        let mut rng = StdRng::seed_from_u64(7);
        let mut healthy_picks = 0usize;
        for _ in 0..2000 {
            let i = s.select(&mut rng);
            if i == 0 {
                healthy_picks += 1;
            }
            s.update(
                i,
                if i == 0 {
                    ArmObservation { success: true, latency_ms: 80.0 }
                } else {
                    ArmObservation { success: false, latency_ms: 900.0 }
                },
            );
        }
        assert!(healthy_picks > 1600, "healthy_picks={healthy_picks}");
    }

    /// Latency-sensitivity: dos proveedores igual de exitosos, uno 20× más
    /// rápido → su media posterior debe superar la del lento.
    #[test]
    fn reward_is_latency_sensitive() {
        let mut s = ThompsonSamplingSelector::new(2);
        for _ in 0..500 {
            s.update(0, ArmObservation { success: true, latency_ms: 40.0 });
            s.update(1, ArmObservation { success: true, latency_ms: 800.0 });
        }
        let fast = s.posterior_mean(0).expect("arm 0");
        let slow = s.posterior_mean(1).expect("arm 1");
        assert!(fast > slow, "fast={fast} slow={slow}");
    }

    /// El fallo (429) castiga más que el éxito lento: media posterior del
    /// que falla < media del que solo tiene éxitos lentos.
    #[test]
    fn failure_penalizes_more_than_slow_success() {
        let mut s = ThompsonSamplingSelector::new(2);
        for _ in 0..500 {
            s.update(0, ArmObservation { success: false, latency_ms: 0.0 });
            s.update(1, ArmObservation { success: true, latency_ms: 2000.0 });
        }
        let failing = s.posterior_mean(0).expect("arm 0");
        let slow_ok = s.posterior_mean(1).expect("arm 1");
        assert!(failing < slow_ok, "failing={failing} slow_ok={slow_ok}");
    }

    #[test]
    fn hazard_rises_with_429_streak_and_decays_on_success() {
        let mut h = HazardEwma429::new(0.3, 60_000);
        for t in 0..5u64 {
            h.record(true, t * 1_000);
        }
        let after_streak = h.hazard();
        assert!(after_streak > 0.5, "after_streak={after_streak}");
        assert!(h.should_pre_throttle(0.5));

        for t in 0..50u64 {
            h.record(false, 5_000 + t * 1_000);
        }
        assert!(h.hazard() < 0.05, "hazard={}", h.hazard());
        assert!(!h.should_pre_throttle(0.5));
    }

    #[test]
    fn hazard_decays_across_inactive_windows() {
        let mut h = HazardEwma429::new(0.5, 60_000);
        h.record(true, 0);
        let hot = h.hazard();
        // 10 ventanas de inactividad (10 min) sin eventos: λ debe caer.
        h.record(false, 600_000);
        assert!(h.hazard() < hot, "hot={hot} cold={}", h.hazard());
    }

    #[test]
    fn hazard_starts_at_zero_and_never_exceeds_one() {
        let h = HazardEwma429::new(0.3, 60_000);
        assert_eq!(h.hazard(), 0.0);
        let mut h2 = HazardEwma429::new(1.0, 1);
        for t in 0..100u64 {
            h2.record(true, t);
        }
        assert_eq!(h2.hazard(), 1.0);
    }
}
