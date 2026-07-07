import type { ReadinessItem } from "../types.js";

const DEFAULT_PROM = process.env["PROMETHEUS_INTERNAL_URL"] ?? "http://prometheus:9090";
const DEFAULT_GRAFANA = process.env["GRAFANA_INTERNAL_URL"] ?? "http://grafana:3000";

async function probe(url: string, timeoutMs: number): Promise<boolean> {
  const ctrl = new AbortController();
  const t = setTimeout(() => ctrl.abort(), timeoutMs);
  try {
    const res = await fetch(url, { signal: ctrl.signal });
    return res.ok;
  } catch {
    return false;
  } finally {
    clearTimeout(t);
  }
}

/**
 * Monitoring + alerts — Prometheus + Grafana healthchecks.
 *
 * - Both healthy → green.
 * - One healthy → yellow ("partial monitoring").
 * - Both unreachable → red.
 *
 * NOTE: this verifier does NOT check that alert rules are configured —
 * "alerts_configured" remains a separate pending item until a real alert
 * fires through to PagerDuty/Telegram.
 */
export async function verifyMonitoring(opts?: {
  promUrl?: string;
  grafanaUrl?: string;
  timeoutMs?: number;
  now?: () => Date;
}): Promise<ReadinessItem> {
  const prom = opts?.promUrl ?? DEFAULT_PROM;
  const grafana = opts?.grafanaUrl ?? DEFAULT_GRAFANA;
  const timeout = opts?.timeoutMs ?? 3000;
  const verified_at = (opts?.now ?? (() => new Date()))().toISOString();
  const base = {
    id: "MONITORING",
    group: "operations" as const,
    label: "Prometheus + Grafana healthcheck reachable",
    verified_at,
  };

  const [promOk, grafOk] = await Promise.all([
    probe(`${prom}/-/healthy`, timeout),
    probe(`${grafana}/api/health`, timeout),
  ]);

  if (promOk && grafOk) {
    return {
      ...base,
      status: "green",
      reason: "Prometheus /-/healthy + Grafana /api/health both 2xx",
      evidence: { kind: "endpoint", ref: `${prom}/-/healthy + ${grafana}/api/health` },
    };
  }
  if (promOk || grafOk) {
    const which = promOk ? "Prometheus" : "Grafana";
    const fail = promOk ? grafana : prom;
    return {
      ...base,
      status: "yellow",
      reason: `only ${which} responding; ${fail} unhealthy`,
    };
  }
  return {
    ...base,
    status: "red",
    reason: `both Prometheus (${prom}) and Grafana (${grafana}) unreachable`,
  };
}
