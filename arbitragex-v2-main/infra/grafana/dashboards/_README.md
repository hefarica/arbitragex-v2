# Grafana Dashboards

This directory contains Grafana dashboard provisioning for the arbitragex-v2 platform.

## Existing Dashboards

Dashboards are provisioned automatically via `monitoring/grafana/provisioning/dashboards/`:

| Dashboard | Panels | Description |
|-----------|--------|-------------|
| API RED Metrics | 6 | Request rate, error rate, p50/p95/p99 latency |
| Edge RED Metrics | 6 | Same RED metrics for edge proxy |
| Database Health | 2 | PostgreSQL connections, Redis ops/sec |
| Business Metrics | 4 | Opportunities, executions, kill-switch, paper-mode |

## RED Methodology

All dashboards follow the RED method:
- **Rate**: requests per second
- **Errors**: percentage of failed requests
- **Duration**: latency distribution (p50/p95/p99)

## Adding Dashboards

1. Create dashboard JSON in `monitoring/grafana/provisioning/dashboards/`
2. Reference it in `dashboards.yml`
3. Restart Grafana container
4. Verify in UI at http://localhost:3000
