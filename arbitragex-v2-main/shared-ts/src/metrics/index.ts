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

// ─────────── Sprint 3: selector-api metrics ───────────
export const selectorDecisionsTotal = new Counter({
  name: "arbx_selector_decisions_total",
  help: "Selector decisions by accept/reject + reason + chain",
  labelNames: ["decision","reason","chain_id"] as const,
  registers: [registry],
});

export const selectorProcessingSeconds = new Histogram({
  name: "arbx_selector_consumer_processing_seconds",
  help: "End-to-end time per opportunity in selector consumer",
  buckets: [0.005,0.01,0.025,0.05,0.1,0.25,0.5,1,2.5,5],
  registers: [registry],
});

export const selectorInvalidMessagesTotal = new Counter({
  name: "arbx_selector_invalid_messages_total",
  help: "Messages that failed schema parse; dropped after XACK",
  registers: [registry],
});

export const selectorConsumerLag = new Gauge({
  name: "arbx_selector_consumer_lag",
  help: "Pending messages in XREADGROUP group",
  labelNames: ["stream","group"] as const,
  registers: [registry],
});

export const tokenSafetyCallsTotal = new Counter({
  name: "arbx_token_safety_calls_total",
  help: "Token safety lookups by provider + result",
  labelNames: ["provider","result"] as const,
  registers: [registry],
});

export const tokenSafetyCacheHitsTotal = new Counter({
  name: "arbx_token_safety_cache_hits_total",
  help: "Token safety cache hits vs misses",
  labelNames: ["hit"] as const,
  registers: [registry],
});

export const blacklistHitsTotal = new Counter({
  name: "arbx_blacklist_hits_total",
  help: "Opportunities rejected because of blacklist",
  labelNames: ["chain_id","reason"] as const,
  registers: [registry],
});

export const cbStateGauge = new Gauge({
  name: "arbx_cb_state",
  help: "Circuit breaker state (0=closed,1=half_open,2=open)",
  labelNames: ["name"] as const,
  registers: [registry],
});

export const cbTripsTotal = new Counter({
  name: "arbx_cb_trips_total",
  help: "Circuit breaker trips by name + reason",
  labelNames: ["name","reason"] as const,
  registers: [registry],
});

// ─────────── PR-2.b: Audit-Log metrics ───────────
export const auditEventsTotal = new Counter({
  name: "arbx_audit_events_total",
  help: "Audit log events written by action",
  labelNames: ["action"] as const,
  registers: [registry],
});

export const auditEmitFailedTotal = new Counter({
  name: "arbx_audit_emit_failed_total",
  help: "Audit events that failed to emit from the edge by reason",
  labelNames: ["reason"] as const,
  registers: [registry],
});

// ─────────── OMEGA-7 PR-1: runtime_ack WSS bridge metrics ───────────
// Emitted from broadcastRuntimeAck after a successful PostgreSQL INSERT in
// POST /api/system/runtime-ack. Closes the dark-channel decoherence between
// searcher-rs Arc-swap acks and the frontend useActionState 12-states
// WAITING_RUNTIME_ACK → VERIFIED transition.
export const runtimeAckBroadcastTotal = new Counter({
  name: "arbx_runtime_ack_broadcast_total",
  help: "Runtime ACK WSS broadcasts emitted post-INSERT by status",
  labelNames: ["status"] as const,
  registers: [registry],
});

export const runtimeAckBroadcastLatencyMs = new Histogram({
  name: "arbx_runtime_ack_broadcast_latency_ms",
  help: "Latency (ms) of the WSS emit step inside broadcastRuntimeAck",
  buckets: [0.1, 0.25, 0.5, 1, 2.5, 5, 10, 25, 50, 100, 250, 500],
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
