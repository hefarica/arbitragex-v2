/**
 * Thin client for the edge. Only uses endpoints the edge actually exposes.
 *
 * Honesty contract: every helper returns a discriminated union
 *   { ok: true, data } | { ok: false, error }
 * Pages consume `error` verbatim; we NEVER fabricate data on failure.
 *
 * Defensive posture:
 *  - 5s timeout via AbortController (configurable per-call).
 *  - GET retries with exponential backoff on network errors / 5xx (never on 4xx,
 *    never on POST — mutations must not be replayed).
 *  - Every response is parsed through a Zod schema; shape drift fails loudly
 *    with a truncated diagnostic instead of rendering broken UI.
 */

import type { z } from "zod";
import * as S from "@/lib/schemas";

const EDGE_URL = process.env.NEXT_PUBLIC_EDGE_URL || "http://localhost:8787";
const DEFAULT_TIMEOUT_MS = 5000;
const DEFAULT_RETRIES = 2;
const MAX_ERROR_PREVIEW = 200;
const MAX_SCHEMA_ISSUES = 3;

type Result<T> = { ok: true; data: T } | { ok: false; error: string };

async function fetchWithTimeout(
  url: string,
  init: RequestInit,
  timeoutMs: number,
): Promise<Response> {
  const ctrl = new AbortController();
  const timer = setTimeout(() => ctrl.abort(), timeoutMs);
  try {
    return await fetch(url, { ...init, signal: ctrl.signal });
  } finally {
    clearTimeout(timer);
  }
}

function backoff(attempt: number): Promise<void> {
  const delay = 200 * 2 ** attempt; // 200, 400, 800 ms
  return new Promise((r) => setTimeout(r, delay));
}

function formatSchemaIssues(err: z.ZodError): string {
  return err.issues
    .slice(0, MAX_SCHEMA_ISSUES)
    .map((i) => `${i.path.join(".") || "<root>"}: ${i.message}`)
    .join("; ");
}

async function getValidated<T>(
  path: string,
  schema: z.ZodType<T>,
  opts: { timeoutMs?: number; retries?: number } = {},
): Promise<Result<T>> {
  const timeoutMs = opts.timeoutMs ?? DEFAULT_TIMEOUT_MS;
  const retries = opts.retries ?? DEFAULT_RETRIES;
  const url = `${EDGE_URL}${path}`;
  const init: RequestInit = {
    next: { revalidate: 0 },
    headers: { accept: "application/json" },
  } as RequestInit;

  let lastError = "unknown error";
  for (let attempt = 0; attempt <= retries; attempt++) {
    try {
      const r = await fetchWithTimeout(url, init, timeoutMs);
      const text = await r.text();

      if (!r.ok) {
        const errMsg = `edge HTTP ${r.status}${text ? `: ${text.slice(0, MAX_ERROR_PREVIEW)}` : ""}`;
        if (r.status >= 500 && attempt < retries) {
          lastError = errMsg;
          await backoff(attempt);
          continue;
        }
        return { ok: false, error: errMsg };
      }

      let parsed: unknown;
      try {
        parsed = JSON.parse(text);
      } catch (e) {
        return { ok: false, error: `edge returned invalid JSON: ${(e as Error).message}` };
      }

      const result = schema.safeParse(parsed);
      if (!result.success) {
        return { ok: false, error: `edge response shape invalid: ${formatSchemaIssues(result.error)}` };
      }
      return { ok: true, data: result.data };
    } catch (e) {
      const err = e as Error;
      lastError = err.name === "AbortError" ? `edge timeout after ${timeoutMs}ms` : err.message;
      if (attempt < retries) {
        await backoff(attempt);
        continue;
      }
    }
  }
  return { ok: false, error: lastError };
}

async function postValidated<T>(
  path: string,
  body: unknown,
  extraHeaders: Record<string, string>,
  schema: z.ZodType<T>,
  timeoutMs = DEFAULT_TIMEOUT_MS,
): Promise<Result<T>> {
  const url = `${EDGE_URL.replace(/\/$/, "")}${path}`;
  try {
    const r = await fetchWithTimeout(
      url,
      {
        method: "POST",
        headers: { "content-type": "application/json", ...extraHeaders },
        body: JSON.stringify(body),
      },
      timeoutMs,
    );
    const text = await r.text();
    if (!r.ok) {
      return { ok: false, error: `HTTP ${r.status}: ${text.slice(0, MAX_ERROR_PREVIEW)}` };
    }
    let parsed: unknown;
    try {
      parsed = JSON.parse(text);
    } catch (e) {
      return { ok: false, error: `edge returned invalid JSON: ${(e as Error).message}` };
    }
    const result = schema.safeParse(parsed);
    if (!result.success) {
      return { ok: false, error: `edge response shape invalid: ${formatSchemaIssues(result.error)}` };
    }
    return { ok: true, data: result.data };
  } catch (e) {
    const err = e as Error;
    return {
      ok: false,
      error: err.name === "AbortError" ? `edge timeout after ${timeoutMs}ms` : err.message,
    };
  }
}

// ─────── Types (re-exported from schemas for backward compat) ───────

export type {
  KillSwitchState,
  StatusResponse,
  OpportunityRow,
  OpportunitiesLive,
  RiskAlertRow,
  RiskAlertsResponse,
  ExecutionRow,
  ExecutionsRecent,
  ReconSummary,
  ReconTimeseriesPoint,
  ReconTimeseriesResponse,
  AppConfigView,
  RelayRow,
  RelaysResponse,
  OnboardingStatus,
} from "@/lib/schemas";

// ─────── GET endpoints ───────

export function getStatus() {
  return getValidated("/status", S.StatusResponseSchema);
}

export function getOpportunitiesLive(limit = 50) {
  return getValidated(`/api/opportunities/live?limit=${limit}`, S.OpportunitiesLiveSchema);
}

export function getRiskAlerts(hours = 24) {
  return getValidated(`/api/risk/alerts?hours=${hours}`, S.RiskAlertsResponseSchema);
}

export function getExecutionsRecent(limit = 50) {
  return getValidated(`/api/executions/recent?limit=${limit}`, S.ExecutionsRecentSchema);
}

export function getReconSummary(hours = 1) {
  return getValidated(`/api/recon/summary?hours=${hours}`, S.ReconSummarySchema);
}

export function getReconTimeseries(hours = 24, bucketMinutes = 60) {
  return getValidated(
    `/api/recon/timeseries?hours=${hours}&bucket_minutes=${bucketMinutes}`,
    S.ReconTimeseriesResponseSchema,
  );
}

export function getConfigCurrent() {
  return getValidated("/api/config/current", S.AppConfigViewSchema);
}

export function getRelays() {
  return getValidated("/api/relays", S.RelaysResponseSchema);
}

export function getOnboardingStatus() {
  return getValidated("/api/onboarding/status", S.OnboardingStatusSchema);
}

// ─────── POST endpoints (no retry — mutations must not be replayed) ───────

export function completeOnboardingPhase1(
  confirmedBy: string,
  vaultSealedHealthy: boolean,
  notes: string,
  adminToken: string,
) {
  return postValidated(
    "/admin/onboarding/1/complete",
    { confirmed_by: confirmedBy, vault_sealed_healthy: vaultSealedHealthy, notes },
    { "x-arbx-admin-token": adminToken, "x-arbx-actor": confirmedBy },
    S.OnboardingPhase1ResultSchema,
  );
}

// Admin: killswitch toggle. The edge (CF Worker in prod, dev-local Express in dev)
// proxies POST /admin/killswitch and forwards the operator's x-arbx-admin-token to
// api-server. The worker rejects unauthenticated callers itself; api-server enforces
// the actual token check.
export function toggleKillswitch(enabled: boolean, reason: string, adminToken: string) {
  return postValidated(
    "/admin/killswitch",
    { enabled, reason },
    { "x-arbx-admin-token": adminToken },
    S.KillSwitchStateSchema,
  );
}
