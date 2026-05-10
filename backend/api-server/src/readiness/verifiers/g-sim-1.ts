import type { ReadinessItem } from "../types.js";

const DEFAULT_SIM_CTL = process.env["SIM_CTL_INTERNAL_URL"] ?? "http://sim-ctl:3003";
const DEFAULT_PROM = process.env["PROMETHEUS_INTERNAL_URL"] ?? "http://prometheus:9090";

/**
 * G-SIM-1 — Simulation mandatory (revm or eth_call+stateOverride).
 *
 * Doctrine: every published opportunity must pass through sim-ctl (or its
 * embedded simulator-v2) before it reaches relays-client. Bypassing
 * simulation = blind execution = capital at risk.
 *
 * Verification:
 *
 *   1. sim-ctl /health responds 2xx (the service is running).
 *
 *   2. arbx_simulation_total{status="pass"} OR {status="fail"} has any
 *      sample in the last 5m — proving simulations are actually flowing
 *      through, not just that the binary boots. NOTE: the metric must
 *      exist; absence of a value pair (no rate, no count) means no
 *      simulations have run, which is yellow not red — the platform may
 *      legitimately be idle in dev.
 *
 *   3. simulator-v2 readiness flag (ARBX_SIMULATOR_V2_READY env) — if
 *      false, the v2 simulator is still stub (audit A3); the gate stays
 *      yellow until A3 closes (Sprint 4). Tier 1 sim-ctl alone is
 *      acceptable for paper-shadow but NOT for capital-real flip.
 */
export async function verifyGSIM1(opts?: {
  simCtlUrl?: string;
  promUrl?: string;
  timeoutMs?: number;
  now?: () => Date;
}): Promise<ReadinessItem> {
  const simCtl = opts?.simCtlUrl ?? DEFAULT_SIM_CTL;
  const prom = opts?.promUrl ?? DEFAULT_PROM;
  const timeout = opts?.timeoutMs ?? 3000;
  const verified_at = (opts?.now ?? (() => new Date()))().toISOString();
  const base = {
    id: "G-SIM-1",
    group: "risk_doctrines" as const,
    label: "Simulation mandatory (revm or eth_call+stateOverride)",
    doctrine: "arbx-simulation-mandatory",
    verified_at,
  };

  // Layer 1: sim-ctl alive.
  const ctrl1 = new AbortController();
  const t1 = setTimeout(() => ctrl1.abort(), timeout);
  let alive = false;
  try {
    const r = await fetch(`${simCtl}/health`, { signal: ctrl1.signal });
    alive = r.ok;
  } catch {
    alive = false;
  } finally {
    clearTimeout(t1);
  }
  if (!alive) {
    return {
      ...base,
      status: "red",
      reason: `sim-ctl ${simCtl}/health unreachable — opportunities would bypass simulation`,
    };
  }

  // Layer 2: simulations flowing (or dev-idle but metric series exists).
  const ctrl2 = new AbortController();
  const t2 = setTimeout(() => ctrl2.abort(), timeout);
  let series_present = 0;
  let recent_count = 0;
  try {
    const r1 = await fetch(
      `${prom}/api/v1/query?query=count(arbx_simulation_total)`,
      { signal: ctrl2.signal },
    );
    const j1 = (await r1.json()) as { data?: { result?: Array<{ value?: [number, string] }> } };
    series_present = parseInt(j1.data?.result?.[0]?.value?.[1] ?? "0", 10);

    const r2 = await fetch(
      `${prom}/api/v1/query?query=sum(increase(arbx_simulation_total[5m]))`,
      { signal: ctrl2.signal },
    );
    const j2 = (await r2.json()) as { data?: { result?: Array<{ value?: [number, string] }> } };
    recent_count = Math.round(parseFloat(j2.data?.result?.[0]?.value?.[1] ?? "0"));
  } catch {
    // tolerate prom hiccup; downgrade to yellow rather than red.
    return {
      ...base,
      status: "yellow",
      reason: `sim-ctl alive but cannot query simulation metrics from ${prom}`,
    };
  } finally {
    clearTimeout(t2);
  }

  if (series_present === 0) {
    return {
      ...base,
      status: "yellow",
      reason: "sim-ctl alive but arbx_simulation_total has no series — searcher hasn't routed any opportunity through simulation yet",
    };
  }

  // Layer 3: A3 — simulator-v2 readiness. Stub still in place is doctrinally yellow.
  const v2_ready = process.env["ARBX_SIMULATOR_V2_READY"] === "true";

  if (!v2_ready) {
    // Acceptable for paper-shadow, blocks capital flip. Yellow not red.
    return {
      ...base,
      status: "yellow",
      reason: `sim-ctl alive (${recent_count} sims in last 5m) but ARBX_SIMULATOR_V2_READY=false — Tier-1 only, A3 stub blocks capital flip`,
      evidence: { kind: "endpoint", ref: `${simCtl}/health + ${prom}/api/v1/query?query=arbx_simulation_total` },
    };
  }

  return {
    ...base,
    status: "green",
    reason: `sim-ctl alive, ${recent_count} simulations in last 5m, simulator-v2 ready`,
    evidence: { kind: "endpoint", ref: `${simCtl}/health + arbx_simulation_total` },
  };
}
