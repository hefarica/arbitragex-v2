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
 * This item lives in the LIVE-FLIP readiness panel (the gate between
 * paper_mode=true and any future capital flip), so its severity is judged by
 * the go-live lens, NOT the paper-shadow lens.
 *
 * Verification (evaluated in this order — strongest blocker first):
 *
 *   1. sim-ctl /health responds 2xx (the service is running). Unreachable =
 *      red: opportunities would bypass simulation entirely.
 *
 *   2. simulator-v2 readiness flag (ARBX_SIMULATOR_V2_READY env). If false,
 *      the v2 simulator is still a stub (audit A3) — it cannot faithfully
 *      validate the path that actually broadcasts, so for the capital-flip
 *      gate this is a HARD blocker (red), regardless of how much Tier-1
 *      sim-ctl traffic flows. (Corrected 2026-06-29: was yellow. A stub
 *      emitting metrics is still a stub; yellow understated a structural
 *      gap. Tier-1 sim-ctl alone is acceptable for paper-shadow operation
 *      but NOT for the live flip this panel certifies.)
 *
 *   3. arbx_simulation_total has a recent sample (last 24h) — proving
 *      simulations are actually flowing, not just that the binary boots.
 *      Only meaningful once the real simulator (step 2) is in place. With a
 *      real simulator but no recent samples the platform is simply idle
 *      (quiet market / fresh boot) → yellow, not red.
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

  // Layer 2: the real simulator must exist. A stub simulator-v2 cannot
  // faithfully validate the path that actually broadcasts, so for the live
  // flip this gate certifies, a stub is a HARD blocker (red) — evaluated
  // BEFORE the metric check so it is red even when the platform is idle and
  // no simulations have flowed. This is the go-live lens, not a paper-shadow
  // health light.
  const v2_ready = process.env["ARBX_SIMULATOR_V2_READY"] === "true";
  if (!v2_ready) {
    return {
      ...base,
      status: "red",
      reason:
        "sim-ctl alive but ARBX_SIMULATOR_V2_READY=false — simulator-v2 is still a stub (audit A3); the mandatory-simulation gate is NOT satisfied for a capital flip. Hard blocker until the real fork-backed simulator validates the actual broadcast path.",
      evidence: { kind: "endpoint", ref: `${simCtl}/health (alive) + ARBX_SIMULATOR_V2_READY=false` },
    };
  }

  // Layer 3: with a real simulator in place, confirm simulations are flowing.
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

    // Window: last 24h. Markets can legitimately go quiet for hours when no
    // arb crosses min_profit_usd; what matters is the simulation path has
    // *some* recent activity (not idle for days). Tighter windows produce
    // false yellows during quiet markets.
    const r2 = await fetch(
      `${prom}/api/v1/query?query=sum(increase(arbx_simulation_total[24h]))`,
      { signal: ctrl2.signal },
    );
    const j2 = (await r2.json()) as { data?: { result?: Array<{ value?: [number, string] }> } };
    recent_count = Math.round(parseFloat(j2.data?.result?.[0]?.value?.[1] ?? "0"));
  } catch {
    // tolerate prom hiccup; downgrade to yellow rather than red.
    return {
      ...base,
      status: "yellow",
      reason: `sim-ctl alive and simulator-v2 ready but cannot query simulation metrics from ${prom}`,
    };
  } finally {
    clearTimeout(t2);
  }

  if (series_present === 0 && recent_count === 0) {
    // Real simulator is ready, but no metric has been written recently. Could
    // be a fresh boot before the first scan tick, or a quiet market. Yellow
    // rather than red — the wiring is real, just hasn't seen traffic yet.
    return {
      ...base,
      status: "yellow",
      reason:
        "simulator-v2 ready and sim-ctl alive, but arbx_simulation_total has no recent samples — idle market / first scan tick pending",
    };
  }

  return {
    ...base,
    status: "green",
    reason: `sim-ctl alive, simulator-v2 ready, ${recent_count} simulations in last 24h`,
    evidence: { kind: "endpoint", ref: `${simCtl}/health + arbx_simulation_total` },
  };
}
