import type { ReadinessItem } from "../types.js";

// Target order: explicit CSP_PROBE_URL → FRONTEND_INTERNAL_URL → compose-internal
// default. The edge worker does NOT serve the frontend (no upstream binding),
// so the probe must reach the frontend itself — or the public URL via
// CSP_PROBE_URL, which is equally valid evidence: it verifies exactly the
// headers users receive.
// READY-CIRC-01: the probe target MUST be a static asset (/favicon.ico), never
// `/`. `/` is force-dynamic SSR that fetches /api/readiness/decision →
// verifyAll() → this verifier → self-deadlock (HEAD / waits for itself, abort
// 8s, every ~39s TTL beat). /favicon.ico is served by Next static middleware
// with the SAME CSP-Report-Only + frame-ancestors + HSTS headers (verified
// live: 416-char CSP identical, 5-60ms vs 6174ms cold SSR).
export const DEFAULT_URL =
  process.env["CSP_PROBE_URL"] ??
  process.env["FRONTEND_INTERNAL_URL"] ??
  "http://frontend:5173/favicon.ico";

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
  // Evidence-based (ONDA 1 FASE 2 / H4, 2026-08-17): 10 cold HEADs against
  // http://frontend:5173 measured p50=23ms warm but 2611ms cold-SSR (full GET
  // 3095ms) right after a frontend redeploy — a 3000ms default raced that cold
  // first hit and produced AbortError yellows while the site served fine.
  // p95 + margin => 8000ms: covers cold SSR comfortably, still fails fast on
  // genuine unavailability.
  const timeout = opts?.timeoutMs ?? 8000;
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
    // The item label also promises HSTS — verify it, never assume it.
    // The frontend emits HSTS only when built with ARBX_TLS_ENABLED=true
    // (frontend/next.config.js SEC-3). Yellow — never green — while missing.
    const hsts = res.headers.get("strict-transport-security");
    if (!hsts) {
      return {
        ...base,
        status: "yellow",
        reason:
          "CSP + frame-ancestors 'none' OK, but no strict-transport-security header — set ARBX_TLS_ENABLED=true on the frontend (TLS is on)",
        evidence: { kind: "endpoint", ref: `HEAD ${url}` },
      };
    }
    return {
      ...base,
      status: "green",
      reason: `CSP-Report-Only present (${csp.length} chars, frame-ancestors 'none' enforced) + HSTS (${hsts})`,
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
