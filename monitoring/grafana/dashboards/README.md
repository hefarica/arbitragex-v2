# Grafana dashboards

Empty in Sprint 1 by design. Dashboards are populated in Sprint 8 after the observability baseline has real data to visualize.

Contract: dashboards MUST only display metrics that actually exist in `arbx_*`
counters/histograms declared in `shared-rs/src/metrics.rs` and
`shared-ts/src/metrics/index.ts`. No synthesized panels.
