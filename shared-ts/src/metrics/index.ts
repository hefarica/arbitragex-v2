import { collectDefaultMetrics, Counter, Gauge, Histogram, Registry } from "prom-client";

export const registry = new Registry();

collectDefaultMetrics({ register: registry, prefix: "arbx_nodejs_" });

export const httpRequestsTotal = new Counter({
  name: "arbx_http_requests_total",
  help: "Total HTTP requests by service/method/path/status",
  labelNames: ["service", "method", "path", "status"] as const,
  registers: [registry],
});

export const httpRequestDuration = new Histogram({
  name: "arbx_http_request_duration_seconds",
  help: "HTTP request duration in seconds",
  labelNames: ["service", "method", "path"] as const,
  buckets: [0.001, 0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1, 2.5, 5, 10],
  registers: [registry],
});

export const opportunitiesTotal = new Counter({
  name: "arbx_opportunity_total",
  help: "Opportunities by chain/strategy/status",
  labelNames: ["chain_id", "strategy_kind", "status"] as const,
  registers: [registry],
});

export const simulationsTotal = new Counter({
  name: "arbx_simulation_total",
  help: "Simulations by simulator and pass/fail",
  labelNames: ["simulator", "passed"] as const,
  registers: [registry],
});

export const executionsTotal = new Counter({
  name: "arbx_execution_total",
  help: "Executions by relay/status/chain",
  labelNames: ["relay", "status", "chain_id"] as const,
  registers: [registry],
});

export const killswitchEnabled = new Gauge({
  name: "arbx_killswitch_enabled",
  help: "1 when kill switch is ON",
  registers: [registry],
});

export const serviceUp = new Gauge({
  name: "arbx_service_up",
  help: "Service up gauge",
  labelNames: ["service"] as const,
  registers: [registry],
});

export function initMetrics(serviceName: string) {
  serviceUp.labels(serviceName).set(1);
}

export async function metricsText(): Promise<{ contentType: string; body: string }> {
  return {
    contentType: registry.contentType,
    body: await registry.metrics(),
  };
}
