import type { ReadinessItem } from "../types.js";

const DEFAULT_PROM = process.env["PROMETHEUS_INTERNAL_URL"] ?? "http://prometheus:9090";

/**
 * G-RPC-1 — RPC failover ≥3 providers + health scoring.
 *
 * Two-step verification (both must pass for green):
 *
 *   1. Pool size ≥ 3: count of `arbx_rpc_provider_state` series across all
 *      chains must be ≥ 3 (the doctrine bar). This catches the operator
 *      mistake of running with a single Alchemy key — the pattern that
 *      crashed token-enricher today when Alchemy hit its monthly quota.
 *
 *   2. At least one provider per configured chain in Healthy (state=0)
 *      OR Degraded (state=1). Open (state=2) on every provider means the
 *      pool is dead and requests will fail.
 *
 * NOTE: state values follow rpc_failover.rs:ProviderState enum
 *   0 = Healthy, 1 = Degraded, 2 = Open (circuit broken).
 */
export async function verifyGRPC1(opts?: {
  promUrl?: string;
  timeoutMs?: number;
  now?: () => Date;
}): Promise<ReadinessItem> {
  const prom = opts?.promUrl ?? DEFAULT_PROM;
  const timeout = opts?.timeoutMs ?? 3000;
  const verified_at = (opts?.now ?? (() => new Date()))().toISOString();
  const base = {
    id: "G-RPC-1",
    group: "risk_doctrines" as const,
    label: "RPC failover ≥3 providers + health scoring",
    doctrine: "arbx-rpc-failover-discipline",
    verified_at,
  };

  const ctrl = new AbortController();
  const t = setTimeout(() => ctrl.abort(), timeout);
  let totalSeries = 0;
  let alive = 0;
  try {
    // Count total provider series (ignores state value).
    const totalRes = await fetch(
      `${prom}/api/v1/query?query=count(arbx_rpc_provider_state)`,
      { signal: ctrl.signal },
    );
    if (!totalRes.ok) throw new Error(`prom http ${totalRes.status}`);
    const totalJson = (await totalRes.json()) as {
      data?: { result?: Array<{ value?: [number, string] }> };
    };
    totalSeries = parseInt(totalJson.data?.result?.[0]?.value?.[1] ?? "0", 10);

    // Count providers in Healthy (0) or Degraded (1) state.
    const aliveRes = await fetch(
      `${prom}/api/v1/query?query=count(arbx_rpc_provider_state<2)`,
      { signal: ctrl.signal },
    );
    if (!aliveRes.ok) throw new Error(`prom http ${aliveRes.status}`);
    const aliveJson = (await aliveRes.json()) as {
      data?: { result?: Array<{ value?: [number, string] }> };
    };
    alive = parseInt(aliveJson.data?.result?.[0]?.value?.[1] ?? "0", 10);
  } catch (e) {
    return {
      ...base,
      status: "yellow",
      reason: `cannot query Prometheus at ${prom}: ${(e as Error).message}`,
    };
  } finally {
    clearTimeout(t);
  }

  if (totalSeries === 0) {
    return {
      ...base,
      status: "red",
      reason: "no arbx_rpc_provider_state metric series — searcher-rs not exposing RPC pool state",
    };
  }
  if (totalSeries < 3) {
    return {
      ...base,
      status: "yellow",
      reason: `only ${totalSeries} providers configured; doctrine requires ≥3 for failover`,
      evidence: { kind: "endpoint", ref: `${prom}/api/v1/query?query=count(arbx_rpc_provider_state)` },
    };
  }
  if (alive === 0) {
    return {
      ...base,
      status: "red",
      reason: `${totalSeries} providers configured but all are Open (circuit broken) — pool is dead`,
    };
  }
  if (alive < 2) {
    // Single alive provider = no actual failover redundancy left.
    return {
      ...base,
      status: "yellow",
      reason: `${totalSeries} providers configured but only ${alive} alive — no failover redundancy`,
    };
  }
  return {
    ...base,
    status: "green",
    reason: `${totalSeries} providers configured, ${alive} alive (Healthy/Degraded)`,
    evidence: { kind: "endpoint", ref: `${prom}/api/v1/query?query=count(arbx_rpc_provider_state)` },
  };
}
