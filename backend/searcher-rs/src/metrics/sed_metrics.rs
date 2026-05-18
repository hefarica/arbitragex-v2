use prometheus::{Counter, Gauge, Histogram, Registry, opts, histogram_opts, exponential_buckets};
use lazy_static::lazy_static;

lazy_static! {
    pub static ref SED_METRICS: SedMetrics = SedMetrics::new();
}

pub struct SedMetrics {
    pub registry: Registry,

    // Mempool
    pub mempool_connected: Gauge,
    pub mempool_tx_received: Counter,
    pub mempool_tx_missing: Counter,
    pub mempool_rpc_errors: Counter,
    pub mempool_normalization_latency_us: Histogram,

    // Reserves
    pub reserve_fetch_success: Counter,
    pub reserve_fetch_latency_ms: Histogram,

    // SED Pipeline
    pub sed_opportunities_detected: Counter,
    pub sed_opportunities_rejected: Counter,
    pub sed_simulations_run: Counter,
    pub sed_simulations_success: Counter,
    pub sed_pipeline_latency_ms: Histogram,

    // Kill-switch
    pub killswitch_triggered: Counter,
    pub killswitch_blocks_prevented: Counter,
}

impl SedMetrics {
    fn new() -> Self {
        let registry = Registry::new();

        let mempool_connected = Gauge::with_opts(opts!(
            "sed_mempool_connected",
            "Mempool WebSocket connection status (1=connected, 0=disconnected)"
        )).unwrap();

        let mempool_tx_received = Counter::with_opts(opts!(
            "sed_mempool_tx_received_total",
            "Total transactions received from mempool"
        )).unwrap();

        let mempool_normalization_latency_us = Histogram::with_opts(histogram_opts!(
            "sed_mempool_normalization_latency_microseconds",
            "Latency of U256→f64 normalization per transaction",
            exponential_buckets(1.0, 2.0, 15).unwrap()
        )).unwrap();

        let reserve_fetch_latency_ms = Histogram::with_opts(histogram_opts!(
            "sed_reserve_fetch_latency_milliseconds",
            "Latency of reserve fetch from RPC",
            exponential_buckets(5.0, 2.0, 12).unwrap()
        )).unwrap();

        let sed_pipeline_latency_ms = Histogram::with_opts(histogram_opts!(
            "sed_pipeline_latency_milliseconds",
            "End-to-end latency of SED pipeline (detect to select)",
            exponential_buckets(10.0, 2.0, 12).unwrap()
        )).unwrap();

        registry.register(Box::new(mempool_connected.clone())).unwrap();
        registry.register(Box::new(mempool_tx_received.clone())).unwrap();
        registry.register(Box::new(mempool_normalization_latency_us.clone())).unwrap();
        registry.register(Box::new(reserve_fetch_latency_ms.clone())).unwrap();
        registry.register(Box::new(sed_pipeline_latency_ms.clone())).unwrap();

        Self {
            registry,
            mempool_connected,
            mempool_tx_received,
            mempool_tx_missing: Counter::with_opts(opts!("sed_mempool_tx_missing_total", "TX hashes without full data")).unwrap(),
            mempool_rpc_errors: Counter::with_opts(opts!("sed_mempool_rpc_errors_total", "RPC errors in mempool")).unwrap(),
            mempool_normalization_latency_us,
            reserve_fetch_success: Counter::with_opts(opts!("sed_reserve_fetch_success_total", "Successful reserve fetches")).unwrap(),
            reserve_fetch_latency_ms,
            sed_opportunities_detected: Counter::with_opts(opts!("sed_opportunities_detected_total", "Opportunities detected")).unwrap(),
            sed_opportunities_rejected: Counter::with_opts(opts!("sed_opportunities_rejected_total", "Opportunities rejected")).unwrap(),
            sed_simulations_run: Counter::with_opts(opts!("sed_simulations_run_total", "Simulations executed")).unwrap(),
            sed_simulations_success: Counter::with_opts(opts!("sed_simulations_success_total", "Simulations successful")).unwrap(),
            sed_pipeline_latency_ms,
            killswitch_triggered: Counter::with_opts(opts!("sed_killswitch_triggered_total", "Kill-switch activations")).unwrap(),
            killswitch_blocks_prevented: Counter::with_opts(opts!("sed_killswitch_blocks_prevented_total", "Executions blocked by kill-switch")).unwrap(),
        }
    }

    pub fn gather(&self) -> Vec<prometheus::proto::MetricFamily> {
        self.registry.gather()
    }
}
