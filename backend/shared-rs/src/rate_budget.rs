//! rate_budget — client-side per-provider token bucket (WO-13 / PERF-STACK).
//!
//! El circuit breaker 429 de `rpc_failover` (ARBX-R-0003) REACCIONA al 429;
//! este módulo lo PREVIENE: cada proveedor con presupuesto configurado en
//! `RPC_HTTP_RATE_BUDGETS` (CSV `name=calls_per_min`) consume 1 token por
//! intento de request dentro de `HttpRpcPool::with_retry`. Sin env = sin
//! presupuesto (backward compatible).
//!
//! Burst capacity = 1 minuto completo de presupuesto, refill continuo.
//! Mutex breve (nunca cruzando await): `pick()` es síncrono y la contención
//! por acquire es despreciable frente al RTT del RPC.

use std::sync::Mutex;
use std::time::Instant;

const MILLI: u64 = 1_000;

/// Pure refill math — tokens (milli) tras `elapsed_ms`, capped at capacity.
/// refill_milli/min = per_minute * 1000 → por ms transcurrido: per_minute * ms / 60.
pub(crate) fn refill_to_cap(
    tokens_milli: u64,
    elapsed_ms: u64,
    cap_milli: u64,
    per_minute: u32,
) -> u64 {
    let refill = (per_minute as u64).saturating_mul(elapsed_ms) / 60;
    tokens_milli.saturating_add(refill).min(cap_milli)
}

pub struct TokenBucket {
    cap_milli: u64,
    per_minute: u32,
    /// (last_refill_instant, tokens_milli)
    inner: Mutex<(Instant, u64)>,
}

impl TokenBucket {
    /// Burst = un minuto completo del presupuesto. `per_minute` >= 1.
    pub fn new(per_minute: u32) -> Self {
        assert!(per_minute >= 1, "rate budget per_minute must be >= 1");
        let cap = per_minute as u64 * MILLI;
        Self {
            cap_milli: cap,
            per_minute,
            inner: Mutex::new((Instant::now(), cap)),
        }
    }

    /// Consume 1 token si hay; false si el bucket está agotado (el caller
    /// debe tratar al proveedor como temporalmente no elegible — NUNCA como
    /// una falla del proveedor).
    pub fn try_acquire(&self) -> bool {
        let mut g = self.inner.lock().expect("rate_budget lock");
        let now = Instant::now();
        let elapsed_ms = now.duration_since(g.0).as_millis() as u64;
        g.1 = refill_to_cap(g.1, elapsed_ms, self.cap_milli, self.per_minute);
        if g.1 < MILLI {
            g.0 = now;
            return false;
        }
        g.1 -= MILLI;
        g.0 = now;
        true
    }

    /// Tokens enteros disponibles (refill lazy incluido). NO consume.
    pub fn tokens_remaining(&self) -> u64 {
        let mut g = self.inner.lock().expect("rate_budget lock");
        let now = Instant::now();
        let elapsed_ms = now.duration_since(g.0).as_millis() as u64;
        g.1 = refill_to_cap(g.1, elapsed_ms, self.cap_milli, self.per_minute);
        g.0 = now;
        g.1 / MILLI
    }

    /// Test-only: retrocede el reloj interno `d` para simular refill
    /// transcurrido SIN dormir (determinista en CI).
    #[cfg(test)]
    pub(crate) fn fast_forward(&self, d: std::time::Duration) {
        let mut g = self.inner.lock().expect("rate_budget lock");
        g.0 =
            g.0.checked_sub(d)
                .expect("fast_forward beyond process start");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn refill_math_linear_and_capped() {
        // 60/min → 60*ms/60 = ms milli por ms: 500 ms elapsed → +500 milli.
        assert_eq!(refill_to_cap(0, 500, 60_000, 60), 500);
        // Cap: nunca excede la capacidad.
        assert_eq!(refill_to_cap(59_999, 60_000, 60_000, 60), 60_000);
        // Un minuto completo a 60/min repone exactamente 60 tokens.
        assert_eq!(refill_to_cap(0, 60_000, 60_000, 60), 60_000);
        // 300/min → 5 milli/ms → 1000 ms repone 5000 milli (5 tokens).
        assert_eq!(refill_to_cap(0, 1_000, 300_000, 300), 5_000);
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
        let b = TokenBucket::new(60); // 60/min → 1 token/min
        for _ in 0..60 {
            assert!(b.try_acquire());
        }
        assert!(!b.try_acquire());
        b.fast_forward(Duration::from_millis(1_050));
        assert!(b.try_acquire(), "refilled token must be acquirable");
        assert_eq!(b.tokens_remaining(), 0);
    }

    #[test]
    fn burst_capacity_equals_one_minute() {
        let b = TokenBucket::new(300);
        assert_eq!(b.tokens_remaining(), 300);
    }
}
