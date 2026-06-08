# Metrics Reference

ArbitrageX v2 exposes a comprehensive metrics surface through Prometheus. This reference documents all metric families, their labels, and example PromQL queries for operational dashboards and alerting.

---

## RED Metrics

The platform follows the [RED method](https://grafana.com/files/grafanacon_eu_2018/Tom_Wilkie_GrafanaCon_EU_2018.pdf): **Rate**, **Errors**, **Duration** for request-focused monitoring.

### Rate Metrics

| Metric Name | Type | Labels | Description |
|-------------|------|--------|-------------|
| `ax_http_requests_total` | Counter | `method`, `path`, `status` | Total HTTP requests |
| `ax_grpc_requests_total` | Counter | `service`, `method`, `status` | Total gRPC requests |
| `ax_ws_connections_total` | Counter | `channel` | WebSocket connection attempts |
| `ax_ws_messages_sent_total` | Counter | `channel`, `event_type` | Messages sent over WebSocket |
| `ax_opportunities_detected_total` | Counter | `strategy`, `chain` | Opportunities detected |
| `ax_trades_executed_total` | Counter | `mode`, `status`, `strategy` | Trades executed |

### Error Metrics

| Metric Name | Type | Labels | Description |
|-------------|------|--------|-------------|
| `ax_http_errors_total` | Counter | `method`, `path`, `error_code` | HTTP error responses |
| `ax_rpc_errors_total` | Counter | `endpoint`, `error_type` | RPC call failures |
| `ax_trade_reverts_total` | Counter | `strategy`, `revert_reason` | Trade reverts |
| `ax_simulation_errors_total` | Counter | `engine`, `error_type` | Simulation failures |

### Duration Metrics

| Metric Name | Type | Labels | Description |
|-------------|------|--------|-------------|
| `ax_http_request_duration_ms` | Histogram | `method`, `path` | HTTP request latency |
| `ax_grpc_request_duration_ms` | Histogram | `service`, `method` | gRPC request latency |
| `ax_strategy_eval_duration_ms` | Histogram | `strategy` | Strategy evaluation time |
| `ax_simulation_duration_ms` | Histogram | `engine` | REVM simulation time |
| `ax_trade_execution_duration_ms` | Histogram | `mode`, `strategy` | End-to-end trade latency |
| `ax_rpc_request_duration_ms` | Histogram | `endpoint` | RPC request latency |

---

## Business Metrics

### Profit & Loss

| Metric Name | Type | Labels | Description |
|-------------|------|--------|-------------|
| `ax_profit_gross_usd` | Gauge | `strategy`, `chain` | Gross profit before costs |
| `ax_profit_net_usd` | Gauge | `strategy`, `chain` | Net profit after gas |
| `ax_gas_cost_usd` | Gauge | `strategy` | Gas cost per trade |
| `ax_paper_balance_usd` | Gauge | — | Current paper balance |
| `ax_daily_cap_used_pct` | Gauge | — | Daily capital limit consumed |

### Operational Counters

| Metric Name | Type | Labels | Description |
|-------------|------|--------|-------------|
| `ax_pools_monitored_total` | Gauge | `chain`, `protocol` | Pools under surveillance |
| `ax_mempool_tx_seen_total` | Counter | `chain` | Transactions observed in mempool |
| `ax_blocks_processed_total` | Counter | `chain` | Blocks processed by watcher |
| `ax_container_restarts_total` | Counter | `container` | Container restart events |

---

## PromQL Query Examples

### Request Rate

```promql
# HTTP request rate per minute
rate(ax_http_requests_total[1m])

# Request rate by status code
sum by (status) (rate(ax_http_requests_total[5m]))

# gRPC request rate per service
sum by (service) (rate(ax_grpc_requests_total[5m]))
```

### Error Rate

```promql
# Overall HTTP error rate
sum(rate(ax_http_errors_total[5m])) / sum(rate(ax_http_requests_total[5m]))

# RPC error rate by endpoint
sum by (endpoint) (rate(ax_rpc_errors_total[5m]))

# Trade revert rate by strategy
sum by (strategy) (rate(ax_trade_reverts_total[5m])) /
sum by (strategy) (rate(ax_trades_executed_total[5m]))
```

### Latency Percentiles

```promql
# HTTP request latency P50
histogram_quantile(0.5, sum by (le, path) (
  rate(ax_http_request_duration_ms_bucket[5m])
))

# HTTP request latency P99
histogram_quantile(0.99, sum by (le, path) (
  rate(ax_http_request_duration_ms_bucket[5m])
))

# Strategy evaluation time P95
histogram_quantile(0.95, sum by (le, strategy) (
  rate(ax_strategy_eval_duration_ms_bucket[5m])
))

# Simulation time P99
histogram_quantile(0.99, sum by (le, engine) (
  rate(ax_simulation_duration_ms_bucket[5m])
))

# End-to-end trade latency P95
histogram_quantile(0.95, sum by (le, strategy) (
  rate(ax_trade_execution_duration_ms_bucket[5m])
))
```

### Profit Tracking

```promql
# Net profit per strategy over 24h
sum by (strategy) (
  increase(ax_profit_net_usd[24h])
)

# Running paper balance
ax_paper_balance_usd

# Gas cost trend
avg_over_time(ax_gas_cost_usd[1h])
```

### System Health

```promql
# Container restart rate
sum by (container) (rate(ax_container_restarts_total[1h]))

# RPC endpoint latency comparison
avg by (endpoint) (ax_rpc_request_duration_ms)

# Opportunity detection rate
sum by (strategy) (rate(ax_opportunities_detected_total[5m]))

# WebSocket connection count
ax_ws_connections_total - ax_ws_disconnects_total
```

---

## Alerting Rules

Example Prometheus alerting rules for production monitoring:

```yaml
groups:
  - name: arbitragex-critical
    rules:
      - alert: HighErrorRate
        expr: |
          sum(rate(ax_http_errors_total[5m]))
          /
          sum(rate(ax_http_requests_total[5m])) > 0.05
        for: 2m
        labels:
          severity: critical
        annotations:
          summary: "High error rate detected"

      - alert: RPCHighLatency
        expr: |
          histogram_quantile(0.99,
            sum(rate(ax_rpc_request_duration_ms_bucket[5m])) by (le, endpoint)
          ) > 500
        for: 5m
        labels:
          severity: warning
        annotations:
          summary: "RPC endpoint {{ $labels.endpoint }} latency > 500ms"

      - alert: ContainerUnhealthy
        expr: |
          count by (container) (
            ax_container_restarts_total[1h] > 3
          )
        for: 1m
        labels:
          severity: critical
        annotations:
          summary: "Container {{ $labels.container }} restarting frequently"

      - alert: LowOpportunityRate
        expr: |
          sum(rate(ax_opportunities_detected_total[30m])) < 0.1
        for: 10m
        labels:
          severity: warning
        annotations:
          summary: "Opportunity detection rate is abnormally low"

      - alert: HighRevertRate
        expr: |
          sum(rate(ax_trade_reverts_total[10m]))
          /
          sum(rate(ax_trades_executed_total[10m])) > 0.15
        for: 5m
        labels:
          severity: warning
        annotations:
          summary: "Trade revert rate exceeds 15%"

      - alert: SimulationEngineDown
        expr: |
          sum(rate(ax_simulation_duration_ms_count[5m])) == 0
        for: 2m
        labels:
          severity: critical
        annotations:
          summary: "Simulation engine has stopped processing"
```

---

## Grafana Dashboard Variables

| Variable | Query | Purpose |
|----------|-------|---------|
| `strategy` | `label_values(ax_opportunities_detected_total, strategy)` | Filter by strategy |
| `chain` | `label_values(ax_trades_executed_total, chain)` | Filter by blockchain |
| `endpoint` | `label_values(ax_rpc_request_duration_ms, endpoint)` | Filter by RPC endpoint |
| `container` | `label_values(ax_container_restarts_total, container)` | Filter by container |
