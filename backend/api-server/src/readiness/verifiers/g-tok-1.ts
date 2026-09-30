import type pg from "pg";
import { promises as fs } from "node:fs";
import path from "node:path";
import type { ReadinessItem } from "../types.js";

const DEFAULT_REPO = "/repo";
const DEFAULT_SELECTOR = process.env["SELECTOR_API_INTERNAL_URL"] ?? "http://selector-api:3002";

/**
 * G-TOK-1 — Token safety screen (honeypot, tax, blacklist).
 *
 * Three-layer verification:
 *
 *   1. Code: selector-api/src/policy/blacklist.ts +
 *      selector-api/src/token_safety/{client,cache}.ts present.
 *
 *   2. Runtime: selector-api /health responds 2xx (the screening service
 *      must actually be running, not just present in repo).
 *
 *   3. State: at least one row in token_safety_cache OR Redis has the
 *      blacklist key set. This proves the screening pipeline has produced
 *      verdicts (or operator pre-seeded a blacklist) — not just compiled.
 */
export async function verifyGTOK1(opts?: {
  pool?: pg.Pool | null;
  repo?: string;
  selectorUrl?: string;
  timeoutMs?: number;
  now?: () => Date;
}): Promise<ReadinessItem> {
  const repo = opts?.repo ?? DEFAULT_REPO;
  const selector = opts?.selectorUrl ?? DEFAULT_SELECTOR;
  const timeout = opts?.timeoutMs ?? 3000;
  const verified_at = (opts?.now ?? (() => new Date()))().toISOString();
  const base = {
    id: "G-TOK-1",
    group: "tokens_strategies" as const,
    label: "Token safety screen (honeypot, tax, blacklist)",
    doctrine: "arbx-token-safety-screen",
    verified_at,
  };

  // Layer 1: code artifacts.
  const required = [
    "backend/selector-api/src/policy/blacklist.ts",
    "backend/selector-api/src/token_safety/client.ts",
    "backend/selector-api/src/token_safety/cache.ts",
  ];
  const missing: string[] = [];
  for (const rel of required) {
    try {
      await fs.access(path.join(repo, rel));
    } catch {
      missing.push(rel);
    }
  }
  if (missing.length > 0) {
    return {
      ...base,
      status: missing.length === required.length ? "red" : "yellow",
      reason: `code artifacts missing: ${missing.join(", ")}`,
    };
  }

  // Layer 2: runtime health.
  const ctrl = new AbortController();
  const t = setTimeout(() => ctrl.abort(), timeout);
  let alive = false;
  try {
    const r = await fetch(`${selector}/health`, { signal: ctrl.signal });
    alive = r.ok;
  } catch {
    alive = false;
  } finally {
    clearTimeout(t);
  }
  if (!alive) {
    return {
      ...base,
      status: "yellow",
      reason: `code present but selector-api ${selector}/health unreachable`,
    };
  }

  // Layer 3: state evidence (DB only today). Best-effort; skip if no pool.
  //
  // KS-TOK-01 (2026-09-29, medido): esta capa se anunciaba "DB or Redis" pero sólo
  // consultaba `token_safety_cache` y devolvía **green incondicional** — con 0
  // filas, con la tabla ausente o incluso SIN POOL. Es decir: el gate no podía
  // fallar nunca y afirmaba tener evidencia de estado que no había leído (falso
  // verde, R10). Ahora el estado se reporta por lo que se midió: verde sólo con
  // filas reales; amarillo si no hay pool, si la tabla no responde o si está vacía.
  if (!opts?.pool) {
    return {
      ...base,
      status: "yellow",
      reason: "code (3 modules) + selector-api healthy, but token-safety state NOT computed (no DB pool)",
      evidence: { kind: "endpoint", ref: `${selector}/health` },
    };
  }
  let state_rows: number | null = null;
  let state_error: string | null = null;
  try {
    const r = await opts.pool.query(`SELECT COUNT(*)::int AS n FROM token_safety_cache`);
    state_rows = r.rows[0]?.n ?? 0;
  } catch (e) {
    state_error = (e as Error).message.slice(0, 80);
  }

  if (state_error != null) {
    return {
      ...base,
      status: "yellow",
      reason: `code (3 modules) + selector-api healthy, but token_safety_cache unreadable: ${state_error}`,
      evidence: { kind: "endpoint", ref: `${selector}/health` },
    };
  }
  if ((state_rows ?? 0) === 0) {
    return {
      ...base,
      status: "yellow",
      reason: "code (3 modules) + selector-api healthy, but token_safety_cache is EMPTY (no token screened yet)",
      evidence: { kind: "endpoint", ref: `${selector}/health` },
    };
  }

  return {
    ...base,
    status: "green",
    reason: `code (3 modules) + selector-api healthy + ${state_rows} token_safety_cache rows`,
    evidence: { kind: "endpoint", ref: `${selector}/health` },
  };
}
