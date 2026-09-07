//! Real publisher: XADD to Redis Stream `arbx:opps:detected` with MAXLEN trim.
//!
//! WO-10 (2026-09-06) also lands here the detection→publish latency family
//! (informe §6.11 / MN-006): this module is the publish terminus shared by the
//! scanner legacy path, the `OpportunityEmitter` and the legacy workers, so
//! the stage histogram lives next to the XADD it names.
//!
//! ## Channel
//!
//! The metrics register into `shared_rs::metrics::REGISTRY` — the SAME
//! registry the searcher's live `/metrics` HTTP exporter serves on
//! `SEARCHER_HEALTH_PORT` (main.rs). No new channel is created: percentiles
//! p50/p95/p99 are Prometheus `histogram_quantile()` over that exporter.
//!
//! ## Hot-path discipline
//!
//! Observation is zero-alloc: `Instant::elapsed().as_secs_f64()` +
//! `Histogram::observe(f64)` on pre-resolved `Lazy<Histogram>` children (no
//! String, no `format!`, no label resolution in the loop). Nothing is logged
//! per-item (R9) — the aggregate surface IS Prometheus.
//!
//! ## R8
//!
//! Registration is lazy: each stage series appears on `/metrics` only after
//! its first real observation (absent series = "no computado", never zero-
//! fabricated). Wall-clock spans (`construction_to_publish`) carry an explicit
//! invalid-guard (`arbx_pipeline_latency_invalid_total`) for clock steps —
//! invalid observations are counted, never injected into the histogram.

// M11 allow: Lazy<MetricVec> initializers use .expect() on infallible paths
// (constant metric names, no duplicate registration possible across one
// binary) — same pattern as crate::metrics and shared_rs::metrics.
#![allow(clippy::expect_used)]

use anyhow::Context;
use once_cell::sync::Lazy;
use prometheus::{Histogram, HistogramVec};
use shared_rs::contracts::Opportunity;
use shared_rs::metrics::REGISTRY;

pub const STREAM_KEY: &str = "arbx:opps:detected";
pub const STREAM_MAXLEN: usize = 10_000;

// ---------------------------------------------------------------------------
// WO-10 (2026-09-06) — detection→publish stage latency family
// ---------------------------------------------------------------------------

/// Stage-latency histogram (seconds). `stage` values:
///
/// - `decode_route`             — V2 `route_decoder::decode_to_route_intents`
///   (scanner.rs; production decode path).
/// - `decode_calldata`          — legacy `calldata::decode` (scanner.rs).
/// - `construction_to_publish`  — wall-clock `Opportunity.detected_at`
///   (stamped at construction, e.g. cartridge_boot.rs) → publish XADD
///   complete. Covers gates + SizeOptimizer + Gate-C scoring + PG insert
///   + XADD.
/// - `emit_boundary`            — `OpportunityEmitter::emit_*` real-I/O entry
///   (dedup) → publish complete (monotonic).
/// - `publish_xadd`             — the XADD round-trip inside `publish()`
///   (this file; covers EVERY publish call-site).
/// - `decode_to_publish_legacy` — scanner legacy path: tx-on-hand → XADD
///   complete (monotonic; V2 early-returns before the legacy sites, so
///   this series is empty while `OrchestratorMode::V2` is active).
///
/// Span decomposition of the informe §6.11 latency:
/// `decode_route` + `construction_to_publish` + (api-server PG-NOTIFY→WS leg,
/// observed in `backend/api-server/src/websocket.ts`) ≈ detección→broadcast.
pub static PIPELINE_LATENCY: Lazy<HistogramVec> = Lazy::new(|| {
    let h = HistogramVec::new(
        prometheus::HistogramOpts::new(
            "arbx_pipeline_latency_seconds",
            "WO-10: detection-to-publish stage latencies in seconds (p50/p95/p99 via histogram_quantile)",
        )
        .buckets(vec![
            0.0005, 0.001, 0.0025, 0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0, 10.0,
        ]),
        &["stage"],
    )
    .expect("metric arbx_pipeline_latency_seconds");
    REGISTRY
        .register(Box::new(h.clone()))
        .expect("register arbx_pipeline_latency_seconds");
    h
});

// Pre-resolved children — observe() on these is allocation-free after the
// first touch (the prometheus client caches the child behind the label key).
pub static STAGE_DECODE_ROUTE: Lazy<Histogram> =
    Lazy::new(|| PIPELINE_LATENCY.with_label_values(&["decode_route"]));
pub static STAGE_DECODE_CALLDATA: Lazy<Histogram> =
    Lazy::new(|| PIPELINE_LATENCY.with_label_values(&["decode_calldata"]));
pub static STAGE_CONSTRUCTION_TO_PUBLISH: Lazy<Histogram> =
    Lazy::new(|| PIPELINE_LATENCY.with_label_values(&["construction_to_publish"]));
pub static STAGE_EMIT_BOUNDARY: Lazy<Histogram> =
    Lazy::new(|| PIPELINE_LATENCY.with_label_values(&["emit_boundary"]));
pub static STAGE_PUBLISH_XADD: Lazy<Histogram> =
    Lazy::new(|| PIPELINE_LATENCY.with_label_values(&["publish_xadd"]));
pub static STAGE_DECODE_TO_PUBLISH_LEGACY: Lazy<Histogram> =
    Lazy::new(|| PIPELINE_LATENCY.with_label_values(&["decode_to_publish_legacy"]));

/// Guard-counter for wall-clock observations rejected as invalid (R8): a
/// wall-clock step (NTP) can yield non-positive elapsed, and a replayed/stale
/// row can exceed the plausibility cap. Counted here, never injected into
/// `PIPELINE_LATENCY`.
pub static PIPELINE_LATENCY_INVALID: Lazy<prometheus::IntCounterVec> = Lazy::new(|| {
    let c = prometheus::IntCounterVec::new(
        prometheus::opts!(
            "arbx_pipeline_latency_invalid_total",
            "WO-10: latency observations skipped as invalid (clock step / stale stamp), by stage and reason"
        ),
        &["stage", "reason"],
    )
    .expect("metric arbx_pipeline_latency_invalid_total");
    REGISTRY
        .register(Box::new(c.clone()))
        .expect("register arbx_pipeline_latency_invalid_total");
    c
});

/// Wall-clock plausibility cap for `construction_to_publish`: observations
/// beyond 5 minutes are stale/replayed stamps, not pipeline latency.
const CONSTRUCTION_SPAN_STALE_CAP_NANOS: i64 = 300 * 1_000_000_000;

/// WO-10 (2026-09-06): observe the `construction_to_publish` stage from the
/// wall-clock `detected_at` stamp. `now` is passed in (pure, testable).
///
/// R8 guard: elapsed must be `0 < ns <= 300s`; otherwise the observation is
/// counted in `arbx_pipeline_latency_invalid_total` (`non_positive_elapsed`
/// / `stale_detected_at` / `out_of_range`) and the histogram is NOT touched.
pub fn observe_construction_to_publish(
    detected_at: chrono::DateTime<chrono::Utc>,
    now: chrono::DateTime<chrono::Utc>,
) {
    let stage = "construction_to_publish";
    match (now - detected_at).num_nanoseconds() {
        Some(ns) if ns > 0 && ns <= CONSTRUCTION_SPAN_STALE_CAP_NANOS => {
            STAGE_CONSTRUCTION_TO_PUBLISH.observe(ns as f64 / 1e9);
        }
        Some(ns) if ns <= 0 => PIPELINE_LATENCY_INVALID
            .with_label_values(&[stage, "non_positive_elapsed"])
            .inc(),
        Some(_) => PIPELINE_LATENCY_INVALID
            .with_label_values(&[stage, "stale_detected_at"])
            .inc(),
        None => PIPELINE_LATENCY_INVALID
            .with_label_values(&[stage, "out_of_range"])
            .inc(),
    }
}

pub async fn publish(
    redis: &mut redis::aio::ConnectionManager,
    opp: &Opportunity,
) -> anyhow::Result<()> {
    let json = serde_json::to_string(opp).context("serialize opportunity")?;
    // XADD arbx:opps:detected MAXLEN ~ 10000 * json <payload>
    // Using a raw command because redis-rs high-level doesn't expose MAXLEN on xadd yet.
    // WO-10 (2026-09-06): XADD round-trip span — observed only on success (`?`
    // propagates the error), zero-alloc, R9-silent.
    let xadd_start = std::time::Instant::now();
    let _: String = redis::cmd("XADD")
        .arg(STREAM_KEY)
        .arg("MAXLEN")
        .arg("~")
        .arg(STREAM_MAXLEN)
        .arg("*")
        .arg("json")
        .arg(&json)
        .query_async(redis)
        .await
        .context("XADD opps.detected")?;
    STAGE_PUBLISH_XADD.observe(xadd_start.elapsed().as_secs_f64());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── WO-10 (2026-09-06): registry + guard semantics ──────────────────────

    #[test]
    fn pipeline_latency_family_is_registered_in_live_exporter() {
        // Force the Lazy registration, then verify the family is served by the
        // SAME registry the searcher's /metrics endpoint exports — the
        // "reuse the live exporter, no new channel" contract.
        let _ = &*STAGE_PUBLISH_XADD;
        let families = shared_rs::metrics::REGISTRY.gather();
        assert!(
            families
                .iter()
                .any(|f| f.get_name() == "arbx_pipeline_latency_seconds"),
            "arbx_pipeline_latency_seconds must be registered in shared_rs::metrics::REGISTRY"
        );
    }

    #[test]
    fn construction_to_publish_observes_valid_span() {
        let now = chrono::Utc::now();
        let before = STAGE_CONSTRUCTION_TO_PUBLISH.get_sample_count();
        // 250ms wall-clock span → observed in the histogram. (Only the
        // histogram count is asserted here: the invalid counters are global
        // and the parallel guard test below mutates them, so cross-test
        // assertions on them would race.)
        observe_construction_to_publish(now - chrono::Duration::milliseconds(250), now);
        assert_eq!(
            STAGE_CONSTRUCTION_TO_PUBLISH.get_sample_count(),
            before + 1,
            "valid span must land in the histogram"
        );
    }

    #[test]
    fn construction_to_publish_rejects_clock_step_and_stale() {
        let now = chrono::Utc::now();
        // Negative elapsed (detected_at in the future — NTP step): counted,
        // never observed.
        let neg = PIPELINE_LATENCY_INVALID
            .with_label_values(&["construction_to_publish", "non_positive_elapsed"])
            .get();
        observe_construction_to_publish(now + chrono::Duration::seconds(5), now);
        assert_eq!(
            PIPELINE_LATENCY_INVALID
                .with_label_values(&["construction_to_publish", "non_positive_elapsed"])
                .get(),
            neg + 1,
            "non-positive elapsed must be counted as invalid"
        );
        // Stale stamp (10 min old): counted under stale_detected_at.
        let stale = PIPELINE_LATENCY_INVALID
            .with_label_values(&["construction_to_publish", "stale_detected_at"])
            .get();
        observe_construction_to_publish(now - chrono::Duration::minutes(10), now);
        assert_eq!(
            PIPELINE_LATENCY_INVALID
                .with_label_values(&["construction_to_publish", "stale_detected_at"])
                .get(),
            stale + 1,
            "stale stamp must be counted as invalid"
        );
    }
}
