//! `sed_metrics` — Metric recording interface for the SED pipeline.
//!
//! Trait-based metrics so sed-core doesn't depend on a specific metrics
//! runtime. The api-server layer provides `PrometheusMetricsRecorder`.
//! Tests use `InMemoryMetricsRecorder`.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

/// Trait for recording SED pipeline metrics.
///
/// Implementors connect to Prometheus, StatsD, or in-memory sinks.
pub trait SedMetricsRecorder: Send + Sync {
    /// Record a CDC value observation.
    fn record_cdc(&self, market: &str, value: f64);

    /// Record filtration duration.
    fn record_filtration_duration_ms(&self, duration_ms: u64);

    /// Record eigenstate energy for a given index.
    fn record_eigenstate_energy(&self, index: u32, energy: f64);

    /// Record Dirac impulse amplitude.
    fn record_dirac_amplitude(&self, market: &str, amplitude: f64);

    /// Record hedge covariance between two markets.
    fn record_hedge_covariance(&self, market_a: &str, market_b: &str, covariance: f64);

    /// Record holonomy value from contour integral.
    fn record_holonomy_value(&self, holonomy: f64);

    /// Record topological yield (net, after friction + vacuum cost).
    fn record_holonomic_yield(&self, net_yield: f64, manifold_count: usize);

    /// Record convergence rate.
    fn record_convergence_rate(&self, rate: f64);

    /// Record kill-switch check event.
    fn record_kill_switch_check(&self, state: &str);

    /// Record 501 infrastructure response.
    fn record_501_response(&self, module: &str, sprint: &str);

    /// Record holonomic invariant violation.
    fn record_invariant_violation(&self, violation_type: &str);

    /// Record GateManager dispatch decision.
    fn record_gate_verdict(&self, dispatched: bool, barrier: Option<u8>);

    /// Record variance headroom snapshot.
    fn record_variance_headroom(&self, headroom: f64);
}

/// A single metric observation.
#[derive(Debug, Clone)]
pub struct MetricObservation {
    pub name: String,
    pub labels: HashMap<String, String>,
    pub value: f64,
}

/// In-memory metrics recorder for testing.
///
/// Thread-safe: wraps observations in `Arc<Mutex<Vec>>`.
#[derive(Debug, Clone)]
pub struct InMemoryMetricsRecorder {
    observations: Arc<Mutex<Vec<MetricObservation>>>,
}

impl InMemoryMetricsRecorder {
    /// Create a new empty recorder.
    pub fn new() -> Self {
        Self {
            observations: Arc::new(Mutex::new(Vec::new())),
        }
    }

    /// Get all recorded observations.
    pub fn observations(&self) -> Vec<MetricObservation> {
        if let Ok(guard) = self.observations.lock() {
            guard.clone()
        } else {
            Vec::new()
        }
    }

    /// Count observations matching a metric name.
    pub fn count(&self, name: &str) -> usize {
        self.observations()
            .iter()
            .filter(|o| o.name == name)
            .count()
    }

    /// Get the latest value for a metric name.
    pub fn latest(&self, name: &str) -> Option<f64> {
        self.observations()
            .iter()
            .filter(|o| o.name == name)
            .last()
            .map(|o| o.value)
    }

    fn record(&self, name: &str, labels: HashMap<String, String>, value: f64) {
        if let Ok(mut guard) = self.observations.lock() {
            guard.push(MetricObservation {
                name: name.to_string(),
                labels,
                value,
            });
        }
    }
}

impl Default for InMemoryMetricsRecorder {
    fn default() -> Self {
        Self::new()
    }
}

impl SedMetricsRecorder for InMemoryMetricsRecorder {
    fn record_cdc(&self, market: &str, value: f64) {
        let mut labels = HashMap::new();
        labels.insert("market".into(), market.into());
        self.record("sed_cdc_value", labels, value);
    }

    fn record_filtration_duration_ms(&self, duration_ms: u64) {
        self.record(
            "sed_filtration_duration_ms",
            HashMap::new(),
            duration_ms as f64,
        );
    }

    fn record_eigenstate_energy(&self, index: u32, energy: f64) {
        let mut labels = HashMap::new();
        labels.insert("index".into(), index.to_string());
        self.record("sed_eigenstate_energy", labels, energy);
    }

    fn record_dirac_amplitude(&self, market: &str, amplitude: f64) {
        let mut labels = HashMap::new();
        labels.insert("market".into(), market.into());
        self.record("sed_dirac_amplitude", labels, amplitude);
    }

    fn record_hedge_covariance(&self, market_a: &str, market_b: &str, covariance: f64) {
        let mut labels = HashMap::new();
        labels.insert("market_a".into(), market_a.into());
        labels.insert("market_b".into(), market_b.into());
        self.record("sed_hedge_covariance", labels, covariance);
    }

    fn record_holonomy_value(&self, holonomy: f64) {
        self.record("sed_holonomy_value", HashMap::new(), holonomy);
    }

    fn record_holonomic_yield(&self, net_yield: f64, manifold_count: usize) {
        let mut labels = HashMap::new();
        labels.insert("manifolds".into(), manifold_count.to_string());
        self.record("sed_holonomic_yield", labels, net_yield);
    }

    fn record_convergence_rate(&self, rate: f64) {
        self.record("sed_convergence_rate", HashMap::new(), rate);
    }

    fn record_kill_switch_check(&self, state: &str) {
        let mut labels = HashMap::new();
        labels.insert("state".into(), state.into());
        self.record("sed_kill_switch_checks_total", labels, 1.0);
    }

    fn record_501_response(&self, module: &str, sprint: &str) {
        let mut labels = HashMap::new();
        labels.insert("module".into(), module.into());
        labels.insert("sprint".into(), sprint.into());
        self.record("sed_501_responses_total", labels, 1.0);
    }

    fn record_invariant_violation(&self, violation_type: &str) {
        let mut labels = HashMap::new();
        labels.insert("type".into(), violation_type.into());
        self.record("sed_holonomic_invariant_violation_total", labels, 1.0);
    }

    fn record_gate_verdict(&self, dispatched: bool, barrier: Option<u8>) {
        let mut labels = HashMap::new();
        labels.insert("dispatched".into(), dispatched.to_string());
        if let Some(b) = barrier {
            labels.insert("barrier".into(), b.to_string());
        }
        self.record("sed_gate_verdict", labels, 1.0);
    }

    fn record_variance_headroom(&self, headroom: f64) {
        self.record("sed_variance_headroom", HashMap::new(), headroom);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn in_memory_recorder_captures_observations() {
        let rec = InMemoryMetricsRecorder::new();
        rec.record_cdc("uniswap-v2", 1.5);
        rec.record_cdc("uniswap-v2", 2.8);
        assert_eq!(rec.count("sed_cdc_value"), 2);
    }

    #[test]
    fn latest_returns_most_recent() {
        let rec = InMemoryMetricsRecorder::new();
        rec.record_eigenstate_energy(0, 1.0);
        rec.record_eigenstate_energy(0, 3.0);
        assert!((rec.latest("sed_eigenstate_energy").unwrap() - 3.0).abs() < 1e-10);
    }

    #[test]
    fn gate_verdict_records_barrier() {
        let rec = InMemoryMetricsRecorder::new();
        rec.record_gate_verdict(false, Some(3));
        let obs = rec.observations();
        assert_eq!(obs.len(), 1);
        assert_eq!(obs[0].labels.get("barrier").unwrap(), "3");
        assert_eq!(obs[0].labels.get("dispatched").unwrap(), "false");
    }

    #[test]
    fn invariant_violation_records_type() {
        let rec = InMemoryMetricsRecorder::new();
        rec.record_invariant_violation("TrivialHolonomy");
        assert_eq!(rec.count("sed_holonomic_invariant_violation_total"), 1);
    }

    #[test]
    fn empty_recorder_returns_none_for_latest() {
        let rec = InMemoryMetricsRecorder::new();
        assert!(rec.latest("nonexistent_metric").is_none());
    }
}
