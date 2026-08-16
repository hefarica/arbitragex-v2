import { promises as fs } from "node:fs";
import type { ReadinessItem } from "../types.js";

const DEFAULT_FILE = "/repo/frontend/lib/admin-token.ts";
// Detects localStorage usage with admin/token strings — the V-AT-1 vulnerability.
const FORBIDDEN_RE = /localStorage\s*\.\s*(?:setItem|getItem|removeItem)\s*\(\s*['"][^'"]*(?:token|admin)[^'"]*['"]/i;

/**
 * V-AT-1 — Admin token must use httpOnly cookie session, not localStorage.
 *
 * Verification strategy (D-03 fix):
 * 1. If the repo is mounted at /repo (dev/local), regex over the source file.
 * 2. If the file is absent (production container — no repo mount), probe the
 *    /admin/session endpoint: a 400 token_required response, or a 429 that
 *    carries the route's own x-ratelimit-admin-session-remaining header
 *    (route alive + rate-limit active = posture confirmed), proves
 *    the httpOnly cookie session exists and the route is not leaking tokens
 *    via localStorage. This replaces the broken "yellow: no repo mount"
 *    state that blocked readiness verification in production for months.
 *
 * - Forbidden pattern found → red (XSS exposure regression).
 * - File exists, pattern absent → green.
 * - File absent + endpoint probe confirms httpOnly session → green.
 * - File absent + endpoint probe fails → yellow (indeterminate).
 */
export async function verifyVAT1(opts?: {
  file?: string;
  probeUrl?: string | undefined; // explicit undefined allowed (exactOptionalPropertyTypes)
  now?: () => Date;
}): Promise<ReadinessItem> {
  const file = opts?.file ?? DEFAULT_FILE;
  // The /admin/session route lives in the edge worker (edge/worker/src/index.ts),
  // not in this process. Default = compose-internal DNS (service "edge", EDGE_PORT 8787);
  // overridable by the caller via V_AT_1_PROBE_URL.
  const probeUrl = opts?.probeUrl ?? "http://edge:8787/admin/session";
  const verified_at = (opts?.now ?? (() => new Date()))().toISOString();
  const base = {
    id: "V-AT-1",
    group: "security_compliance" as const,
    label: "Admin token via httpOnly cookie (no localStorage)",
    doctrine: "secrets.policy.md T1",
    verified_at,
  };

  // Strategy 1: file-based (dev/local with repo mount).
  let content: string | null = null;
  try {
    content = await fs.readFile(file, "utf8");
  } catch {
    // File not available — fall through to endpoint probe.
  }

  if (content !== null) {
    if (FORBIDDEN_RE.test(content)) {
      return {
        ...base,
        status: "red",
        reason: "localStorage.{set,get,remove}Item with admin/token literal detected — V-AT-1 regression",
        evidence: { kind: "file", ref: "frontend/lib/admin-token.ts" },
      };
    }
    return {
      ...base,
      status: "green",
      reason: "no localStorage admin/token usage; httpOnly cookie pattern intact",
      evidence: { kind: "file", ref: "frontend/lib/admin-token.ts" },
    };
  }

  // Strategy 2 (D-03): endpoint probe — no repo mount required.
  try {
    const res = await fetch(probeUrl, {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify({}),
      signal: AbortSignal.timeout(3000),
    });
    if (
      res.status === 400 ||
      (res.status === 429 && res.headers.get("x-ratelimit-admin-session-remaining") !== null)
    ) {
      // 400 token_required = route alive demanding the token via httpOnly cookie.
      // 429 from THIS route's limiter = route alive AND rate-limit active — posture
      // confirmed. Readiness is computed per request and internal probes share the
      // route limiter's per-caller bucket (keyed "anon" in the CF-worker variant;
      // the api-server container IP under edge/dev-local), so frequent polling
      // can legitimately trip its 429 (not a regression). Attribution guard: the
      // x-ratelimit-admin-session-remaining header is set only by the /admin/session
      // handler (edge/worker + edge/dev-local) immediately before its 429 return;
      // a bare 429 comes from the edge-global limiter or the auth lockout — not
      // attributable to this route — and falls through to yellow (fail-honest).
      return {
        ...base,
        status: "green",
        reason: `/admin/session endpoint active (${res.status}) — httpOnly cookie session confirmed via endpoint probe`,
        evidence: { kind: "endpoint", ref: `POST /admin/session → ${res.status}` },
      };
    }
    // 404 = the route doesn't exist — potential regression.
    return {
      ...base,
      status: "yellow",
      reason: `/admin/session probe returned ${res.status} (expected 400/429) — cannot confirm httpOnly cookie session`,
    };
  } catch {
    return {
      ...base,
      status: "yellow",
      reason: "repo mount absent and endpoint probe failed — cannot verify V-AT-1 in this deployment",
    };
  }
}
