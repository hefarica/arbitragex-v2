//! rate_budget — client-side per-provider token bucket (WO-13 / PERF-STACK,
//! RPC-RECOVERY-01).
//!
//! El circuit breaker 429 de `rpc_failover` (ARBX-R-0003) REACCIONA al 429;
//! este módulo lo PREVIENE: cada proveedor con presupuesto configurado en
//! `RPC_HTTP_RATE_BUDGETS` (CSV `name=calls_per_min`) consume 1 token por
//! intento de request dentro de `HttpRpcPool::with_retry`. Sin env = sin
//! presupuesto (backward compatible).
//!
//! ## RPC-RECOVERY-01 — correcciones sobre la implementación anterior
//!
//! La implementación anterior tenía DOS defectos (audit del operador,
//! reproducción aritmética en 6 pruebas de modelo):
//!
//! 1. **Ráfaga = un minuto completo de cuota** (`cap = per_minute`): un
//!    proveedor de 240/min podía disparar 240 requests instantáneos. Ahora
//!    la ráfaga (capacidad del bucket) es un parámetro INDEPENDIENTE
//!    (`with_burst` / env `RPC_HTTP_RATE_BURSTS=name=burst`). Compatibilidad:
//!    `TokenBucket::new(rpm)` conserva burst=rpm (comportamiento histórico
//!    documentado) hasta que el operador fije la ráfaga explícita.
//! 2. **Recarga truncada a milisegundos con reset del reloj en CADA lectura**
//!    (`try_acquire` y `tokens_remaining`): una consulta cada 100µs perdía
//!    TODA la fracción de recarga (el reloj se reiniciaba con elapsed=0ms).
//!    Como `pick()` consulta el saldo de todas las entradas en cada
//!    selección, esto MATABA DE HAMBRE a los proveedores presupuestados y
//!    concentraba el tráfico en los sin presupuesto. Ahora: recarga en
//!    nanosegundos con **acarreo del resto** (u128) — pérdida CERO, la
//!    frecuencia de inspección no altera el crédito acumulado (regresión
//!    trasladada del modelo Python a estos tests).
//!
//! Semántica: 1 token = 10^9 unidades internas (nano-tokens). Recarga =
//! `rpm * elapsed_ns / 60e9` nano-tokens, con resto persistido. El reloj usa
//! `Instant` (monótono); `saturating_duration_since` hace imposible el
//! movimiento hacia atrás.

use std::sync::Mutex;
use std::time::Instant;

/// 1 token en unidades internas (nano).
const NANOS_PER_TOKEN: u128 = 1_000_000_000;
/// Nanosegundos en un minuto — denominador de la recarga.
const NANOS_PER_MINUTE: u128 = 60 * NANOS_PER_TOKEN;
/// Capacidad máxima defensiva (tokens): 1M/min es más que cualquier plan real.
const MAX_BURST_TOKENS: u32 = 1_000_000;

/// Estado interno del bucket. `credit_nano` = crédito en nano-tokens (1e9 = 1
/// token); `rem` = resto de la última recarga en unidades `rpm * ns` (acarreo
/// exacto, sin pérdida); `last` = instante de la última materialización.
#[derive(Debug)]
struct BucketState {
    credit_nano: u64,
    rem: u64,
    last: Instant,
}

#[derive(Debug)]
pub struct TokenBucket {
    /// Capacidad del bucket en nano-tokens (ráfaga), NO la cuota sostenida.
    cap_nano: u64,
    per_minute: u32,
    inner: Mutex<BucketState>,
}

impl TokenBucket {
    /// Compatibilidad histórica: ráfaga = un minuto completo de cuota
    /// (documentado; usar `with_burst` o `RPC_HTTP_RATE_BURSTS` para fijar
    /// una ráfaga independiente). `per_minute >= 1`.
    pub fn new(per_minute: u32) -> Self {
        Self::with_burst(per_minute, per_minute)
    }

    /// Cuota sostenida `per_minute` (tokens/min) con ráfaga `burst` (tokens,
    /// capacidad del bucket). Ambos >= 1. La recarga NO depende de la ráfaga.
    pub fn with_burst(per_minute: u32, burst: u32) -> Self {
        assert!(per_minute >= 1, "rate budget per_minute must be >= 1");
        assert!(burst >= 1, "rate budget burst must be >= 1");
        let burst = burst.min(MAX_BURST_TOKENS);
        let cap_nano = (burst as u64).saturating_mul(NANOS_PER_TOKEN as u64);
        Self {
            cap_nano,
            per_minute,
            inner: Mutex::new(BucketState {
                // Arranque LLENO hasta la ráfaga (comportamiento histórico:
                // el bucket nuevo puede servir su ráfaga inmediatamente).
                credit_nano: cap_nano,
                rem: 0,
                last: Instant::now(),
            }),
        }
    }

    /// Materializa la recarga acumulada hasta `now` (nanosegundos + acarreo
    /// del resto). PÉRDIDA CERO: la fracción que no alcanza un nano-token
    /// completo queda en `rem` y se suma a la siguiente recarga. Consultar el
    /// saldo no reduce lo que se acumula (idempotente en la frecuencia).
    fn refill_to(&self, st: &mut BucketState, now: Instant) {
        // Instant es monótono; un `now` anterior a `last` (imposible por
        // construcción salvo mocks) no resta crédito ni entra en pánico.
        let elapsed_ns = now.saturating_duration_since(st.last).as_nanos() as u128;
        if elapsed_ns == 0 {
            return;
        }
        // prod = rpm * elapsed_ns + resto_previo (u128: máx ~1e6 * 6e13 ≈ 6e19 < u128 max).
        let prod = (self.per_minute as u128)
            .saturating_mul(elapsed_ns)
            .saturating_add(st.rem as u128);
        let add_nano = (prod / NANOS_PER_MINUTE) as u64; // nano-tokens enteros
        st.rem = (prod % NANOS_PER_MINUTE) as u64; // fracción acarreada — jamás se pierde
        st.credit_nano = st.credit_nano.saturating_add(add_nano).min(self.cap_nano);
        st.last = now;
    }

    /// Consume 1 token si hay; false si el bucket está agotado (el caller
    /// debe tratar al proveedor como temporalmente no elegible — NUNCA como
    /// una falla del proveedor).
    pub fn try_acquire(&self) -> bool {
        let mut g = self.inner.lock().expect("rate_budget lock");
        self.refill_to(&mut g, Instant::now());
        if g.credit_nano < NANOS_PER_TOKEN as u64 {
            return false;
        }
        g.credit_nano -= NANOS_PER_TOKEN as u64;
        true
    }

    /// Tokens enteros disponibles (refill lazy incluido). NO consume y NO
    /// pierde crédito: la fracción sub-token queda acarreada en `rem`.
    pub fn tokens_remaining(&self) -> u64 {
        let mut g = self.inner.lock().expect("rate_budget lock");
        self.refill_to(&mut g, Instant::now());
        g.credit_nano / NANOS_PER_TOKEN as u64
    }

    /// Ráfaga (capacidad) en tokens — para diagnóstico y métricas.
    pub fn burst_capacity(&self) -> u64 {
        self.cap_nano / NANOS_PER_TOKEN as u64
    }

    /// Test-only: retrocede el reloj interno `d` para simular refill
    /// transcurrido SIN dormir (determinista en CI).
    #[cfg(test)]
    pub(crate) fn fast_forward(&self, d: std::time::Duration) {
        let mut g = self.inner.lock().expect("rate_budget lock");
        g.last = g
            .last
            .checked_sub(d)
            .expect("fast_forward beyond process start");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    // ── RPC-RECOVERY-01: regresiones trasladadas del modelo Python (6/6) ──

    /// Regresión central (modelo: test_current_code_loses_submillisecond_refill
    /// + test_proposed_refill_is_independent_of_inspection_frequency): un
    /// bucket de 60/min agotado, inspeccionado cada 100µs durante 1s, DEBE
    /// recuperar 1 token — independiente de la frecuencia de inspección.
    /// (La implementación con truncado a ms + reset de reloj recuperaba 0.)
    #[test]
    fn refill_independent_of_inspection_frequency() {
        let b = TokenBucket::new(60);
        for _ in 0..60 {
            assert!(b.try_acquire());
        }
        assert!(!b.try_acquire(), "bucket exhausted");
        // 1s total en pasos de 100µs, consultando el saldo en cada paso.
        for _ in 0..10 {
            b.fast_forward(Duration::from_micros(100));
            let _ = b.tokens_remaining(); // la lectura NO debe comer la recarga
        }
        assert!(
            b.try_acquire(),
            "1s at 60/min must refill exactly 1 token regardless of inspection frequency"
        );
        assert_eq!(b.tokens_remaining(), 0);
    }

    /// Variante adversarial: consultas MUCHO más frecuentes (10µs) durante
    /// 1s deben acumular la fracción exacta: 1 token en total.
    #[test]
    fn sub_microsecond_queries_preserve_fraction() {
        let b = TokenBucket::new(60);
        for _ in 0..60 {
            assert!(b.try_acquire());
        }
        for _ in 0..1000 {
            b.fast_forward(Duration::from_micros(10));
            let _ = b.tokens_remaining();
        }
        assert!(b.try_acquire(), "1s of 10µs-granularity queries must still yield 1 token");
    }

    /// Modelo: test_preserves_low_rate_fraction — 1/min: tras 30s hay 0.5
    /// tokens (invisible en enteros), que NO se pierde: otros 30s → 1 token.
    #[test]
    fn preserves_low_rate_fraction() {
        let b = TokenBucket::new(1);
        assert!(b.try_acquire(), "burst inicial = 1");
        assert!(!b.try_acquire());
        b.fast_forward(Duration::from_secs(30));
        assert_eq!(b.tokens_remaining(), 0, "0.5 token no es un token entero");
        let _ = b.tokens_remaining(); // consultar no pierde la fracción
        b.fast_forward(Duration::from_secs(30));
        assert!(b.try_acquire(), "30s + 30s = 1 token exacto");
    }

    /// Modelo: test_explicit_burst_is_not_one_minute_capacity — la ráfaga es
    /// independiente de la cuota sostenida: 240/min con burst 10 admite 10
    /// inmediatos y recarga a RITMO de 240/min (4/s).
    #[test]
    fn explicit_burst_is_independent_of_sustained_quota() {
        let b = TokenBucket::with_burst(240, 10);
        assert_eq!(b.tokens_remaining(), 10, "burst capacity, not 240");
        assert_eq!(b.burst_capacity(), 10);
        for _ in 0..10 {
            assert!(b.try_acquire());
        }
        assert!(!b.try_acquire(), "burst agotado");
        b.fast_forward(Duration::from_secs(1));
        // 240/min = 4 tokens/s → tras 1s hay 4 tokens (NO cap de 10: el ritmo
        // sostenido manda en la recarga; el burst solo acota el acumulado).
        assert_eq!(b.tokens_remaining(), 4, "sustained 240/min refills 4/s");
    }

    /// Modelo: test_no_refill_without_time — lecturas sin tiempo transcurrido
    /// no generan crédito ni consumen el existente.
    #[test]
    fn no_refill_without_time() {
        let b = TokenBucket::new(60);
        assert!(b.try_acquire());
        let before = b.tokens_remaining();
        for _ in 0..1000 {
            assert_eq!(b.tokens_remaining(), before);
        }
    }

    /// Modelo: test_backward_clock_rejected — el reloj monótono no puede
    /// retroceder; la saturación garantiza que ni pánico ni crédito extra.
    #[test]
    fn backward_clock_saturates_without_panic_or_credit() {
        let b = TokenBucket::new(60);
        for _ in 0..60 {
            assert!(b.try_acquire());
        }
        b.fast_forward(Duration::from_secs(30));
        let seen = b.tokens_remaining();
        assert!(seen <= 30, "60/min en 30s = 30 tokens máx");
        // Consultas repetidas tras la saturación: sin pánico, sin crecimiento.
        assert_eq!(seen, b.tokens_remaining());
    }

    // ── Pruebas históricas (adaptadas a la nueva aritmética) ──

    #[test]
    fn refill_math_linear_and_capped() {
        // 60/min: 500ms → 0.5 token (invisible)…
        let b = TokenBucket::new(60);
        for _ in 0..60 {
            assert!(b.try_acquire());
        }
        b.fast_forward(Duration::from_millis(500));
        assert_eq!(b.tokens_remaining(), 0, "0.5 token no es entero");
        // …y otros 500ms completan el token exacto (la fracción se acarreó).
        b.fast_forward(Duration::from_millis(500));
        assert_eq!(b.tokens_remaining(), 1);
        // 300/min → 5 tokens/s: 1s → 5.
        let c = TokenBucket::new(300);
        for _ in 0..300 {
            assert!(c.try_acquire());
        }
        c.fast_forward(Duration::from_secs(1));
        assert_eq!(c.tokens_remaining(), 5);
        // Cap por ráfaga: nunca excede la capacidad del bucket.
        let d = TokenBucket::with_burst(300, 10);
        for _ in 0..10 {
            assert!(d.try_acquire());
        }
        d.fast_forward(Duration::from_secs(300));
        assert_eq!(d.tokens_remaining(), 10, "el burst acota el acumulado");
    }

    #[test]
    fn acquire_drains_capacity_then_rejects() {
        let b = TokenBucket::new(5);
        for _ in 0..5 {
            assert!(b.try_acquire());
        }
        assert!(!b.try_acquire(), "bucket exhausted must reject");
        assert_eq!(b.tokens_remaining(), 0);
    }

    #[test]
    fn budget_prevention_real_refill_restores_token() {
        // PREVENCIÓN (requisito -61): tras agotar, el refill del presupuesto
        // devuelve tokens SIN necesidad de un 429 que "despierte" nada.
        let b = TokenBucket::new(60);
        for _ in 0..60 {
            assert!(b.try_acquire());
        }
        assert!(!b.try_acquire());
        b.fast_forward(Duration::from_millis(1_050));
        assert!(b.try_acquire(), "refilled token must be acquirable");
        assert_eq!(b.tokens_remaining(), 0);
    }

    #[test]
    fn burst_capacity_defaults_to_one_minute_for_compat() {
        // Compatibilidad documentada: new(rpm) conserva ráfaga = minuto
        // completo hasta que el operador fije RPC_HTTP_RATE_BURSTS.
        let b = TokenBucket::new(300);
        assert_eq!(b.tokens_remaining(), 300);
        assert_eq!(b.burst_capacity(), 300);
    }
}
