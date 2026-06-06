//! Per-source circuit breaker + timeout for the pool-enumeration sources.
//!
//! Each external data source (The Graph / DeFiLlama / GeckoTerminal) gets its
//! own `SourceCircuit`. After `fail_threshold` consecutive failures the source
//! is QUARANTINED for `cooldown_ms`; during quarantine the worker skips it and
//! keeps using the healthy sources, so one flapping API never stalls discovery.
//!
//! Read-only / shadow: this only governs whether an HTTP read is attempted. No
//! signer, capital, or broadcast is involved.

use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use tracing::{info, warn};

/// A consecutive-failure circuit breaker for one data source.
pub struct SourceCircuit {
    pub name: &'static str,
    consecutive_fails: AtomicU32,
    quarantine_until_ms: AtomicU64,
    in_quarantine: AtomicBool,
    fail_threshold: u32,
    cooldown_ms: u64,
}

impl SourceCircuit {
    pub fn new(name: &'static str, fail_threshold: u32, cooldown_ms: u64) -> Arc<Self> {
        Arc::new(Self {
            name,
            consecutive_fails: AtomicU32::new(0),
            quarantine_until_ms: AtomicU64::new(0),
            in_quarantine: AtomicBool::new(false),
            // A 0 threshold would open the circuit instantly; floor at 1.
            fail_threshold: fail_threshold.max(1),
            cooldown_ms,
        })
    }

    /// True while the source is quarantined. Auto-closes once the cooldown ends.
    pub fn is_quarantined(&self) -> bool {
        if !self.in_quarantine.load(Ordering::Relaxed) {
            return false;
        }
        if epoch_ms() >= self.quarantine_until_ms.load(Ordering::Relaxed) {
            self.in_quarantine.store(false, Ordering::Relaxed);
            self.consecutive_fails.store(0, Ordering::Relaxed);
            info!(event = "poolenum.source.circuit_closed", source = self.name);
            return false;
        }
        true
    }

    pub fn record_success(&self) {
        self.consecutive_fails.store(0, Ordering::Relaxed);
        self.in_quarantine.store(false, Ordering::Relaxed);
    }

    pub fn record_failure(&self) {
        let n = self.consecutive_fails.fetch_add(1, Ordering::Relaxed) + 1;
        if n >= self.fail_threshold && !self.in_quarantine.load(Ordering::Relaxed) {
            self.quarantine_until_ms.store(
                epoch_ms().saturating_add(self.cooldown_ms),
                Ordering::Relaxed,
            );
            self.in_quarantine.store(true, Ordering::Relaxed);
            warn!(
                event = "poolenum.source.circuit_open",
                source = self.name,
                failures = n,
                cooldown_sec = self.cooldown_ms / 1000,
            );
        }
    }
}

/// Run `op` under the circuit + a hard timeout. Returns `None` (and records a
/// failure) on quarantine, timeout, or error — fail-honest, never panics.
pub async fn guarded_fetch<F, Fut, T>(
    circuit: &Arc<SourceCircuit>,
    timeout_ms: u64,
    op: F,
) -> Option<T>
where
    F: FnOnce() -> Fut,
    Fut: std::future::Future<Output = anyhow::Result<T>>,
{
    if circuit.is_quarantined() {
        warn!(
            event = "poolenum.source.skipped_quarantined",
            source = circuit.name
        );
        return None;
    }
    match tokio::time::timeout(Duration::from_millis(timeout_ms), op()).await {
        Ok(Ok(r)) => {
            circuit.record_success();
            Some(r)
        }
        Ok(Err(e)) => {
            circuit.record_failure();
            warn!(event = "poolenum.source.fetch_err", source = circuit.name, error = %e);
            None
        }
        Err(_) => {
            circuit.record_failure();
            warn!(
                event = "poolenum.source.timeout",
                source = circuit.name,
                timeout_ms
            );
            None
        }
    }
}

fn epoch_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn opens_after_threshold_then_blocks() {
        let c = SourceCircuit::new("t", 3, 60_000);
        assert!(!c.is_quarantined());
        c.record_failure();
        c.record_failure();
        assert!(!c.is_quarantined()); // 2 < 3
        c.record_failure();
        assert!(c.is_quarantined()); // 3rd opens it
    }

    #[test]
    fn success_resets() {
        let c = SourceCircuit::new("t", 2, 60_000);
        c.record_failure();
        c.record_success();
        c.record_failure();
        assert!(!c.is_quarantined()); // counter reset → 1 < 2
    }

    #[test]
    fn zero_threshold_floored_to_one() {
        let c = SourceCircuit::new("t", 0, 60_000);
        c.record_failure();
        assert!(c.is_quarantined()); // floored to 1 → opens on first failure
    }
}
