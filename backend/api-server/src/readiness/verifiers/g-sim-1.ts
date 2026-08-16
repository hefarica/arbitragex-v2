import type pg from "pg";
import type { ReadinessItem } from "../types.js";
import { G_SIM_1_ITEM_KEYS, computeIsFresh } from "../../routes/readiness-evidence.js";

const DEFAULT_SIM_CTL = process.env["SIM_CTL_INTERNAL_URL"] ?? "http://sim-ctl:3003";
const DEFAULT_PROM = process.env["PROMETHEUS_INTERNAL_URL"] ?? "http://prometheus:9090";
const GATE_ID = "G-SIM-1";

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
 *   2. simulator-v2 readiness derives from the REAL topology (FASE 3,
 *      operator directive 2026-08-16: "the readiness must derive its language
 *      from the REAL topology — never again a stale string"): GET sim-ctl
 *      /capabilities (FASE 1) for the build-reality module/backend view, and
 *      read the readiness_evidence registry (FASE 2) for the closed 7-key
 *      checklist with 30-day STRICT freshness. Zero-Mocks applied to the
 *      readiness itself: every token in the reason (module count, backend,
 *      E/7, pending keys, stale flags) is derived, never hardcoded.
 *
 *        a) ARBX_SIMULATOR_V2_READY=false → red with truthful language
 *           describing what IS implemented (N módulos, backend X) and which
 *           checklist evidence is still missing. The word "stub" is ONLY
 *           permitted when /capabilities reports backend v1 or is absent
 *           (fetch failure) — prohibited otherwise. Backend v2 with an
 *           empty/non-array modules field is an INCONSISTENCY: red, no
 *           "stub", no module enumeration.
 *        b) flag=true AND 7/7 fresh evidence → proceed to layer 3.
 *        c) flag=true AND <7/7 → red "premature flag — SECURE_BOOT violated"
 *           naming the missing keys.
 *
 *      Registry unreadable (pool null / query error) → 0/7 evidenced AND the
 *      reason must SAY the registry is unreadable — never a silent 0/7 of a
 *      healthy registry, never fabricated evidence.
 *
 *   3. arbx_simulation_total has FLOW in the last 24h — proving simulations
 *      are actually running, not just that the binary boots. GREEN requires
 *      recent_count > 0 (sum(increase(arbx_simulation_total[24h])) > 0).
 *      Series presence alone NEVER yields green: arbx_simulation_total is a
 *      lazily-created counter child that persists on every scrape, so
 *      series_present >= 1 with recent_count === 0 is zero flow — the
 *      platform is idle (quiet market / fresh boot) → yellow, not green.
 */

/** GET /capabilities payload (FASE 1) — only the fields this verifier reads. */
interface SimCapabilities {
  simulator_backend?: string | null;
  modules?: unknown;
}

/**
 * Parse a Prometheus sample value into a finite number. Malformed payloads
 * ("NaN", garbage strings, undefined when the result vector is empty) must
 * fall back — a NaN would silently break every `=== 0` guard downstream and
 * could read as flow (NaN !== 0) when there is none.
 */
function toFiniteNumber(v: string | undefined, fallback: number): number {
  const n = Number(v);
  return Number.isFinite(n) ? n : fallback;
}

export async function verifyGSIM1(opts?: {
  simCtlUrl?: string;
  /** Override the capabilities path (default /capabilities) — for tests. */
  capabilitiesPath?: string;
  promUrl?: string;
  /** PG pool for the readiness_evidence registry; null = registry unreadable. */
  pool?: pg.Pool | null;
  timeoutMs?: number;
  now?: () => Date;
}): Promise<ReadinessItem> {
  const simCtl = opts?.simCtlUrl ?? DEFAULT_SIM_CTL;
  const capsPath = opts?.capabilitiesPath ?? "/capabilities";
  const prom = opts?.promUrl ?? DEFAULT_PROM;
  const timeout = opts?.timeoutMs ?? 3000;
  const now = opts?.now ?? (() => new Date());
  const at = now();
  const base = {
    id: GATE_ID,
    group: "risk_doctrines" as const,
    label: "Simulation mandatory (revm or eth_call+stateOverride)",
    doctrine: "arbx-simulation-mandatory",
    verified_at: at.toISOString(),
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

  // Layer 2 (FASE 3): derive the language from the REAL topology.
  // (1) /capabilities — build-reality view of the simulator binary.
  const ctrlC = new AbortController();
  const tC = setTimeout(() => ctrlC.abort(), timeout);
  let caps: SimCapabilities | null = null;
  try {
    const r = await fetch(`${simCtl}${capsPath}`, { signal: ctrlC.signal });
    if (r.ok) caps = (await r.json()) as SimCapabilities;
  } catch {
    caps = null; // /health is alive but /capabilities is not — noted below
  } finally {
    clearTimeout(tC);
  }

  // (2) readiness_evidence registry — latest row per checklist item (the
  // table is keyed (gate_id, item_key), so each row IS the latest).
  let registryProblem: string | null = null;
  let rows: Array<{ item_key: string; status: string; verified_at: Date | string }> = [];
  if (!opts?.pool) {
    registryProblem = "registry unavailable (pool not configured)";
  } else {
    try {
      const r = await opts.pool.query(
        `SELECT item_key, status, verified_at
           FROM readiness_evidence
          WHERE gate_id = $1`,
        [GATE_ID],
      );
      rows = r.rows;
    } catch (e) {
      const msg = (e as Error).message.split("\n")[0]?.slice(0, 120) ?? "unknown";
      registryProblem = `registry unreadable (${msg})`;
    }
  }

  // (3) Classify the closed 7-key checklist. evidenced = latest row is
  // status='evidenced' AND fresh (verified_at > now-30d, strict — same rule
  // as the FASE 2 reader). Everything else is pending; a pending key whose
  // row exists but is old is additionally flagged stale.
  const evidenced: string[] = [];
  const pending: string[] = [];
  const stale: string[] = [];
  for (const key of G_SIM_1_ITEM_KEYS) {
    const row = rows.find((x) => x.item_key === key);
    const fresh = row ? computeIsFresh(row.verified_at, at) : false;
    if (row && row.status === "evidenced" && fresh) {
      evidenced.push(key);
    } else {
      pending.push(key);
      if (row && !fresh) stale.push(key);
    }
  }
  const total = G_SIM_1_ITEM_KEYS.length;
  const checklist = `evidencia de checklist ${evidenced.length}/${total}`;
  const pendingList = pending.length > 0 ? `[${pending.join(", ")}]` : "";
  const staleNote = stale.length > 0 ? ` — stale (>30d): ${stale.join(", ")}` : "";
  const registryNote = registryProblem ? ` — readiness_evidence ${registryProblem}` : "";

  const v2_ready = process.env["ARBX_SIMULATOR_V2_READY"] === "true";
  // Only the module COUNT is ever quoted — module names are never enumerated
  // in the readiness language (and never invented when caps is unavailable).
  const modules: unknown[] = caps && Array.isArray(caps.modules) ? caps.modules : [];

  if (!v2_ready) {
    if (caps && modules.length >= 1) {
      // Truthful RED: the simulator IS implemented; what is missing is
      // checklist evidence and/or the flag itself. Zero-Mocks: N, X and E/7
      // all come from /capabilities and the registry.
      const backend = caps.simulator_backend ?? "no reportado";
      const reason =
        pending.length === 0
          ? `simulator-v2 IMPLEMENTADO (${modules.length} módulos, backend ${backend} disponible); ${checklist} completa; hard blocker de checklist resuelto — único paso restante: ARBX_SIMULATOR_V2_READY=false`
          : `simulator-v2 IMPLEMENTADO (${modules.length} módulos, backend ${backend} disponible); ${checklist}; hard blocker hasta completar: ${pendingList}${staleNote}${registryNote}`;
      return {
        ...base,
        status: "red",
        reason,
        evidence: {
          kind: "endpoint",
          ref: `${simCtl}${capsPath} + readiness_evidence ${evidenced.length}/${total}`,
        },
      };
    }
    if (!caps) {
      // /health alive but /capabilities not answerable: flag-only truthful
      // reasoning. No module enumeration, no "stub" (prohibited when the
      // topology is simply unread).
      return {
        ...base,
        status: "red",
        reason: `sim-ctl alive, ARBX_SIMULATOR_V2_READY=false; ${simCtl}${capsPath} unavailable — topología real no verificable, módulos no enumerables; ${checklist}${pendingList ? `; hard blocker hasta completar: ${pendingList}` : ""}${staleNote}${registryNote}`,
        evidence: {
          kind: "endpoint",
          ref: `${simCtl}/health (alive) + ${simCtl}${capsPath} (unavailable) + readiness_evidence ${evidenced.length}/${total}`,
        },
      };
    }
    if (caps.simulator_backend === "v1") {
      // /capabilities answers with the legacy v1 backend and 0 modules: the
      // binary genuinely does not contain simulator-v2 — one of the ONLY two
      // states where "stub" is permitted (backend v1, or caps fetch-failure).
      return {
        ...base,
        status: "red",
        reason: `sim-ctl reports backend ${caps.simulator_backend} with ${modules.length} modules — simulator-v2 still a stub in this build (audit A3); ARBX_SIMULATOR_V2_READY=false; ${checklist}${pendingList ? `; hard blocker hasta completar: ${pendingList}` : ""}${staleNote}${registryNote}`,
        evidence: {
          kind: "endpoint",
          ref: `${simCtl}${capsPath} (${modules.length} modules) + readiness_evidence ${evidenced.length}/${total}`,
        },
      };
    }
    // Backend claims v2 (or goes unreported) while enumerating 0 modules — a
    // self-inconsistent /capabilities payload. Truthful INCONSISTENCY
    // language: no "stub" (that word is prohibited here), no module
    // enumeration (a payload claiming v2 with 0 modules cannot be trusted to
    // enumerate anything).
    const backendClaim = caps.simulator_backend ?? "no reportado";
    return {
      ...base,
      status: "red",
      reason: `inconsistencia en /capabilities: backend ${backendClaim} con 0 módulos enumerados — un backend v2 debe declarar al menos 1 módulo; topología real no verificable; ARBX_SIMULATOR_V2_READY=false; ${checklist}${pendingList ? `; hard blocker hasta completar: ${pendingList}` : ""}${staleNote}${registryNote}`,
      evidence: {
        kind: "endpoint",
        ref: `${simCtl}${capsPath} (backend ${backendClaim}, 0 modules) + readiness_evidence ${evidenced.length}/${total}`,
      },
    };
  }

  if (pending.length > 0) {
    // Flag flipped before the checklist is complete — the flag is a claim of
    // readiness the registry does not support. SECURE_BOOT: evidence first,
    // flag second.
    return {
      ...base,
      status: "red",
      reason: `premature flag — SECURE_BOOT violated: ARBX_SIMULATOR_V2_READY=true with ${checklist}; missing evidence: ${pendingList}${staleNote}${registryNote}`,
      evidence: {
        kind: "endpoint",
        ref: `ARBX_SIMULATOR_V2_READY=true + readiness_evidence ${evidenced.length}/${total}`,
      },
    };
  }

  // Layer 3: flag + full fresh checklist — confirm simulations are flowing.
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
    series_present = toFiniteNumber(j1.data?.result?.[0]?.value?.[1], 0);

    // Window: last 24h. Markets can legitimately go quiet for hours when no
    // arb crosses min_profit_usd; what matters is the simulation path has
    // *some* recent activity (not idle for days). Tighter windows produce
    // false yellows during quiet markets.
    const r2 = await fetch(
      `${prom}/api/v1/query?query=sum(increase(arbx_simulation_total[24h]))`,
      { signal: ctrl2.signal },
    );
    const j2 = (await r2.json()) as { data?: { result?: Array<{ value?: [number, string] }> } };
    recent_count = Math.round(toFiniteNumber(j2.data?.result?.[0]?.value?.[1], 0));
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

  if (recent_count === 0) {
    // Zero flow in the last 24h → NEVER green. Series presence alone is not
    // flow: the counter child is lazily created and then persists on every
    // scrape, so series_present >= 1 with recent_count === 0 must still read
    // as idle. Could be a fresh boot before the first scan tick, or a quiet
    // market. Yellow rather than red — the wiring is real, just quiet.
    return {
      ...base,
      status: "yellow",
      reason:
        series_present > 0
          ? "simulator-v2 ready and sim-ctl alive, but arbx_simulation_total has no recent samples — the series is alive but quiet (idle market)"
          : "simulator-v2 ready and sim-ctl alive, but arbx_simulation_total has no recent samples — idle market / first scan tick pending",
    };
  }

  return {
    ...base,
    status: "green",
    reason: `sim-ctl alive, simulator-v2 ready, ${recent_count} simulations in last 24h`,
    evidence: { kind: "endpoint", ref: `${simCtl}/health + arbx_simulation_total` },
  };
}
