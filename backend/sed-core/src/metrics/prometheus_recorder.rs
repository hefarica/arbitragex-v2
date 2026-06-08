//! `prometheus_recorder` — Production Prometheus implementation of `SedMetricsRecorder`.
//!
//! Feature-gated under `prometheus-metrics`. Registers all SED pipeline metrics
//! in the provided `prometheus::Registry` and exposes them for scraping.
//!
//! ## ZERO-MOCKS COMPLIANCE
//!
//! Every metric value recorded here comes from real SED pipeline computation.
//! If no data flows, counters stay at 0 — never fabricated.
//!
//! ## CAPITAL EXPOSED: STRICTLY 0
//!
//! This module only observes. No signing, no submission, no RPC calls.

use super::sed_metrics::SedMetricsRecorder;
use prometheus::{
    exponential_buckets, histogram_opts, opts, CounterVec, GaugeVec, HistogramVec, Registry,
};

/// Production Prometheus-backed metrics recorder for the SED pipeline.
///
/// Thread-safe via prometheus's internal `Arc<Mutex<...>>` on each metric.
#[derive(Clone)]
pub struct PrometheusMetricsRecorder {
    // Phase 2 — Filtration
    cdc_value: GaugeVec,
    filtration_duration_ms: HistogramVec,

    // Phase 3 — Eigenstate
    eigenstate_energy: GaugeVec,

    // Phase 5 — Allocator
    dirac_amplitude: GaugeVec,

    // Phase 6 — Hedger
    hedge_covariance: GaugeVec,
    holonomy_value: prometheus::Gauge,
    holonomic_yield: GaugeVec,

    // Pipeline control
    convergence_rate: prometheus::Gauge,
    kill_switch_checks: CounterVec,
    infra_501_responses: CounterVec,
    invariant_violations: CounterVec,
    gate_verdicts: CounterVec,
    variance_headroom: prometheus::Gauge,
}

impl PrometheusMetricsRecorder {
    /// Create and register all SED metrics in the given registry.
    ///
    /// # Errors
    /// Returns `Err` if any metric fails to register (duplicate name, etc.).
    pub fn new(registry: &Registry) -> Result<Self, prometheus::Error> {
        let cdc_value = GaugeVec::new(
            opts!(
                "sed_cdc_value",
                "State Divergence Coefficient (CDC) — latest value per market"
            ),
            &["market"],
        )?;

        let filtration_duration_ms = HistogramVec::new(
            histogram_opts!(
                "sed_filtration_duration_ms",
                "Filtration phase execution time in milliseconds",
                exponential_buckets(0.1, 2.0, 15)?
            ),
            &[],
        )?;

        let eigenstate_energy = GaugeVec::new(
            opts!("sed_eigenstate_energy", "Eigenstate energy level by index"),
            &["index"],
        )?;

        let dirac_amplitude = GaugeVec::new(
            opts!("sed_dirac_amplitude", "Dirac impulse amplitude per market"),
            &["market"],
        )?;

        let hedge_covariance = GaugeVec::new(
            opts!(
                "sed_hedge_covariance",
                "Hedge covariance off-diagonal between market pairs"
            ),
            &["market_a", "market_b"],
        )?;

        let holonomy_value = prometheus::Gauge::with_opts(opts!(
            "sed_holonomy_value",
            "Contour integral holonomy value (latest)"
        ))?;

        let holonomic_yield = GaugeVec::new(
            opts!(
                "sed_holonomic_yield",
                "Net topological yield after friction + vacuum cost"
            ),
            &["manifolds"],
        )?;

        let convergence_rate = prometheus::Gauge::with_opts(opts!(
            "sed_convergence_rate",
            "Current convergence rate of the SED pipeline"
        ))?;

        let kill_switch_checks = CounterVec::new(
            opts!("sed_kill_switch_checks_total", "Kill-switch state checks"),
            &["state"],
        )?;

        let infra_501_responses = CounterVec::new(
            opts!(
                "sed_501_responses_total",
                "Infrastructure 501 Not Implemented responses"
            ),
            &["module", "sprint"],
        )?;

        let invariant_violations = CounterVec::new(
            opts!(
                "sed_holonomic_invariant_violation_total",
                "Holonomic invariant violations by type"
            ),
            &["type"],
        )?;

        let gate_verdicts = CounterVec::new(
            opts!("sed_gate_verdict_total", "GateManager dispatch decisions"),
            &["dispatched", "barrier"],
        )?;

        let variance_headroom = prometheus::Gauge::with_opts(opts!(
            "sed_variance_headroom",
            "Remaining variance headroom in GateManager"
        ))?;

        // Register all metrics
        registry.register(Box::new(cdc_value.clone()))?;
        registry.register(Box::new(filtration_duration_ms.clone()))?;
        registry.register(Box::new(eigenstate_energy.clone()))?;
        registry.register(Box::new(dirac_amplitude.clone()))?;
        registry.register(Box::new(hedge_covariance.clone()))?;
        registry.register(Box::new(holonomy_value.clone()))?;
        registry.register(Box::new(holonomic_yield.clone()))?;
        registry.register(Box::new(convergence_rate.clone()))?;
        registry.register(Box::new(kill_switch_checks.clone()))?;
        registry.register(Box::new(infra_501_responses.clone()))?;
        registry.register(Box::new(invariant_violations.clone()))?;
        registry.register(Box::new(gate_verdicts.clone()))?;
        registry.register(Box::new(variance_headroom.clone()))?;

        Ok(Self {
            cdc_value,
            filtration_duration_ms,
            eigenstate_energy,
            dirac_amplitude,
            hedge_covariance,
            holonomy_value,
            holonomic_yield,
            convergence_rate,
            kill_switch_checks,
            infra_501_responses,
            invariant_violations,
            gate_verdicts,
            variance_headroom,
        })
    }
}

impl SedMetricsRecorder for PrometheusMetricsRecorder {
    fn record_cdc(&self, market: &str, value: f64) {
        self.cdc_value.with_label_values(&[market]).set(value);
    }

    fn record_filtration_duration_ms(&self, duration_ms: u64) {
        self.filtration_duration_ms
            .with_label_values(&[])
            .observe(duration_ms as f64);
    }

    fn record_eigenstate_energy(&self, index: u32, energy: f64) {
        self.eigenstate_energy
            .with_label_values(&[&index.to_string()])
            .set(energy);
    }

    fn record_dirac_amplitude(&self, market: &str, amplitude: f64) {
        self.dirac_amplitude
            .with_label_values(&[market])
            .set(amplitude);
    }

    fn record_hedge_covariance(&self, market_a: &str, market_b: &str, covariance: f64) {
        self.hedge_covariance
            .with_label_values(&[market_a, market_b])
            .set(covariance);
    }

    fn record_holonomy_value(&self, holonomy: f64) {
        self.holonomy_value.set(holonomy);
    }

    fn record_holonomic_yield(&self, net_yield: f64, manifold_count: usize) {
        self.holonomic_yield
            .with_label_values(&[&manifold_count.to_string()])
            .set(net_yield);
    }

    fn record_convergence_rate(&self, rate: f64) {
        self.convergence_rate.set(rate);
    }

    fn record_kill_switch_check(&self, state: &str) {
        self.kill_switch_checks.with_label_values(&[state]).inc();
    }

    fn record_501_response(&self, module: &str, sprint: &str) {
        self.infra_501_responses
            .with_label_values(&[module, sprint])
            .inc();
    }

    fn record_invariant_violation(&self, violation_type: &str) {
        self.invariant_violations
            .with_label_values(&[violation_type])
            .inc();
    }

    fn record_gate_verdict(&self, dispatched: bool, barrier: Option<u8>) {
        let barrier_str = barrier
            .map(|b| b.to_string())
            .unwrap_or_else(|| "none".to_string());
        self.gate_verdicts
            .with_label_values(&[&dispatched.to_string(), &barrier_str])
            .inc();
    }

    fn record_variance_headroom(&self, headroom: f64) {
        self.variance_headroom.set(headroom);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registers_without_panic() {
        let registry = Registry::new();
        let recorder = PrometheusMetricsRecorder::new(&registry);
        assert!(recorder.is_ok());
    }

    #[test]
    fn double_registration_fails() {
        let registry = Registry::new();
        let _r1 = PrometheusMetricsRecorder::new(&registry).unwrap();
        let r2 = PrometheusMetricsRecorder::new(&registry);
        assert!(r2.is_err(), "Duplicate registration should fail");
    }

    #[test]
    fn cdc_gauge_updates() {
        let registry = Registry::new();
        let recorder = PrometheusMetricsRecorder::new(&registry).unwrap();
        recorder.record_cdc("uniswap-v2", 1.42);
        let val = recorder.cdc_value.with_label_values(&["uniswap-v2"]).get();
        assert!((val - 1.42).abs() < 1e-10);
    }

    #[test]
    fn gate_verdict_counter_increments() {
        let registry = Registry::new();
        let recorder = PrometheusMetricsRecorder::new(&registry).unwrap();
        recorder.record_gate_verdict(true, None);
        recorder.record_gate_verdict(true, Some(2));
        recorder.record_gate_verdict(false, Some(3));
        let dispatched = recorder
            .gate_verdicts
            .with_label_values(&["true", "none"])
            .get();
        assert!((dispatched - 1.0).abs() < 1e-10);
    }
}
