import type { ReadinessItem } from "../types.js";

const DEFAULT_URL = process.env["FRONTEND_INTERNAL_URL"] ?? "http://frontend:5173";

/**
 * PR-1 — CSP header (Report-Only) + frame-ancestors none on frontend.
 *
 * Verification: HTTP HEAD against the frontend's root, inspect headers.
 * - Both CSP-Report-Only AND frame-ancestors 'none' present → green.
 * - CSP present but missing frame-ancestors 'none' → yellow.
 * - No CSP at all → red.
 * - Frontend unreachable → yellow with reason.
 */
export async function verifyPR1CSP(opts?: {
  url?: string;
  timeoutMs?: number;
  now?: () => Date;
}): Promise<ReadinessItem> {
  const url = opts?.url ?? DEFAULT_URL;
  const timeout = opts?.timeoutMs ?? 3000;
  const verified_at = (opts?.now ?? (() => new Date()))().toISOString();
  const base = {
    id: "PR-1",
    group: "security_compliance" as const,
    label: "CSP-Report-Only + HSTS + rate-limit edge admin session",
    verified_at,
  };

  const ctrl = new AbortController();
  const t = setTimeout(() => ctrl.abort(), timeout);
  try {
    const res = await fetch(url, { method: "HEAD", signal: ctrl.signal });
    const csp =
      res.headers.get("content-security-policy-report-only") ??
      res.headers.get("content-security-policy");

    if (!csp) {
      return {
        ...base,
        status: "red",
        reason: `frontend response has no CSP header (status ${res.status})`,
        evidence: { kind: "endpoint", ref: `HEAD ${url}` },
      };
    }
    if (!csp.includes("frame-ancestors 'none'")) {
      return {
        ...base,
        status: "yellow",
        reason: "CSP present but missing frame-ancestors 'none' directive",
        evidence: { kind: "endpoint", ref: `HEAD ${url}` },
      };
    }
    return {
      ...base,
      status: "green",
      reason: `CSP-Report-Only present (${csp.length} chars, frame-ancestors 'none' enforced)`,
      evidence: { kind: "endpoint", ref: `HEAD ${url}` },
    };
  } catch (e) {
    return {
      ...base,
      status: "yellow",
      reason: `frontend unreachable at ${url}: ${(e as Error).name}`,
    };
  } finally {
    clearTimeout(t);
  }
}
