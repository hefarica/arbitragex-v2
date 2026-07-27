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
import {
  KpiPayloadSchema,
  SCurvePayloadSchema,
  VariancePayloadSchema,
  ScannerHeartbeatResponseSchema,
  type KpiPayload,
  type SCurvePayload,
  type VariancePayload,
  type ScannerHeartbeatResponse,
} from "@/lib/operations-schemas";

// Server Components run inside Docker — INTERNAL_EDGE_URL reaches the edge via
// Docker DNS (http://edge:8787). The browser uses NEXT_PUBLIC_EDGE_URL which
// resolves via the operator's SSH tunnel to the edge port.
const isBrowser = typeof window !== "undefined";

export function getApiBaseUrl(): string {
  // BROWSER FIX: Use relative paths to avoid CORS when developing locally.
  // Next.js rewrites (configured in next.config.js) proxy /api/* to the edge.
  // This eliminates CORS issues when frontend (localhost:3000) calls VPS edge.
  if (isBrowser) {
    return "";
  }

  // Server-side: use INTERNAL_EDGE_URL for direct edge communication
  const envUrl = process.env.INTERNAL_EDGE_URL;
  
  const isProd = process.env.NODE_ENV === "production";

  // H1 fix: honor the same escape hatch next.config.js uses so a local
  // production build (docker compose dev / sandbox) does not 500 every SSR
  // page that fetches from a localhost edge.
  if (
    isProd &&
    process.env.ARBX_ALLOW_LOCALHOST_PROD !== "true" &&
    envUrl &&
    /localhost|127\.0\.0\.1|0\.0\.0\.0/.test(envUrl)
  ) {
    throw new Error("Production API base URL cannot point to localhost");
  }

  if (envUrl && envUrl.trim().length > 0) {
    return envUrl.replace(/\/$/, "");
  }

  return "";
}

export function getWsBaseUrl(): string {
  const envUrl = process.env.NEXT_PUBLIC_WS_URL;
  const isProd = process.env.NODE_ENV === "production";

  // H1 fix: same ARBX_ALLOW_LOCALHOST_PROD escape hatch as getApiBaseUrl().
  if (
    isProd &&
    process.env.ARBX_ALLOW_LOCALHOST_PROD !== "true" &&
    envUrl &&
    /localhost|127\.0\.0\.1|0\.0\.0\.0/.test(envUrl)
  ) {
    throw new Error("Production WS base URL cannot point to localhost");
  }

  if (envUrl && envUrl.trim().length > 0) {
    return envUrl.replace(/\/$/, "");
  }

  if (isBrowser) {
    const loc = window.location;
    const protocol = loc.protocol === "https:" ? "wss:" : "ws:";
    return `${protocol}//${loc.host}`;
  }

  return "";
}

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
  opts: { timeoutMs?: number; retries?: number; extraHeaders?: Record<string, string> } = {},
): Promise<Result<T>> {
  const timeoutMs = opts.timeoutMs ?? DEFAULT_TIMEOUT_MS;
  const retries = opts.retries ?? DEFAULT_RETRIES;
  const url = `${getApiBaseUrl()}${path}`;
  const init: RequestInit = {
    next: { revalidate: 0 },
    headers: { accept: "application/json", ...(opts.extraHeaders ?? {}) },
    // V-AT-1: browser sends the arbx_admin_session httpOnly cookie cross-port
    // so admin-gated GETs (e.g. /api/admin/chains) authenticate. Public GETs
    // ignore the cookie upstream; no harm in always including credentials.
    credentials: "include",
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
  const url = `${getApiBaseUrl()}${path}`;
  try {
    const r = await fetchWithTimeout(
      url,
      {
        method: "POST",
        headers: { "content-type": "application/json", ...extraHeaders },
        body: JSON.stringify(body),
        // V-AT-1: include credentials so browser sends the httpOnly admin cookie.
        credentials: "include",
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
  OnboardingPhase2Result,
  OnboardingPhase3Result,
  OnboardingPhase4Result,
  OnboardingPhase5Result,
  AuditLogRow,
  AuditLogsResponse,
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

export function getReadiness() {
  return getValidated("/api/readiness", S.ReadinessReportSchema);
}

// Strategy runtime status — per-strategy observability fed by Postgres
// (opportunities table) + Redis (heartbeat, pool index, reserves cache).
// Backend returns 503 if PG unavailable; FE renders SystemGuardBanner's
// "Engine status" tile as `unavailable` in that case (R8 fail-honest).
export function getRuntimeStatus(chainId = 1) {
  return getValidated(
    `/api/strategies/runtime-status?chain_id=${chainId}`,
    S.RuntimeStatusResponseSchema,
  );
}

// Readiness blockers — flat list of every concrete obstacle between paper
// mode and live execution. Env values are NEVER on the wire (presence-only).
export function getReadinessBlockers() {
  return getValidated("/api/readiness/blockers", S.ReadinessBlockersResponseSchema);
}

// Readiness decision — go/no-go verdict derived from blockers + immutable
// safety flags (live_trading=false, capital_exposure_usd=0).
export function getReadinessDecision() {
  return getValidated("/api/readiness/decision", S.ReadinessDecisionResponseSchema);
}

// Readiness steps — server-side 4-step "Live Readiness" stepper (Topology /
// Credentials / Market Topology / Resolution Engines). The N/4 count is derived
// from real backend evidence (vault providers, credential presence, registry
// counts, enabled engines) — NOT localStorage. Credentials are redacted
// (present|null), live_enabled is structurally false.
export function getReadinessSteps() {
  return getValidated("/api/readiness/steps", S.ReadinessStepsResponseSchema);
}

// Agent teams status — 17 workspace-verified agent verdicts (P2-continued).
// Backend overlays runtime context (verifyAll outcome) onto each agent so
// PASS demotes to BLOCKED when readiness fails.
export function getAgentTeamsStatus() {
  return getValidated("/api/agents/status", S.AgentsStatusResponseSchema);
}

// A.8 confidence scoring wire status. Reports honest "primitives wired vs
// pipeline integration" split. Backend returns scoring_pipeline_wired=false
// until the scanner hot-path actually invokes bayesian/kelly per candidate.
export function getScoringStatus() {
  return getValidated("/api/scoring/status", S.ScoringStatusResponseSchema);
}

// A.6 comprehensive circuit breakers. 10 breakers with honest state evaluation
// (PASS / WARN / PAUSED / KILLED / BLOCKED / NOT_AVAILABLE / UNKNOWN). Missing
// data sources (DD curve, revert window, gas burn ledger) render NOT_AVAILABLE
// — never fabricated PASS.
export function getCircuitBreakersStatus() {
  return getValidated("/api/risk/circuit-breakers/status", S.CircuitBreakersStatusResponseSchema);
}

export function getCircuitBreakerEvents() {
  return getValidated("/api/risk/circuit-breakers/events", S.CircuitBreakerEventsResponseSchema);
}

// ─────── B1 Chains Admin CRUD client (admin-token gated) ───────
//
// All mutations route through the edge with `credentials: "include"` so the
// V-AT-1 httpOnly cookie travels automatically. GETs use the same
// getValidated() shared pipeline.

export function getAdminChains(enabledFilter?: boolean) {
  const qs = enabledFilter == null ? "" : `?enabled=${enabledFilter ? "true" : "false"}`;
  return getValidated(`/api/admin/chains${qs}`, S.AdminChainsListSchema);
}

export function getAdminChain(chainId: number) {
  return getValidated(`/api/admin/chains/${chainId}`, S.AdminChainRowSchema);
}

export async function createAdminChain(
  body: S.AdminChainCreateBody,
  adminToken: string,
  actor: string,
): Promise<{ ok: true; data: S.AdminChainRow } | { ok: false; error: string }> {
  const url = `${getApiBaseUrl()}/api/admin/chains`;
  try {
    const r = await fetchWithTimeout(
      url,
      {
        method: "POST",
        headers: {
          "content-type": "application/json",
          "x-arbx-admin-token": adminToken,
          "x-arbx-actor": actor,
        },
        credentials: "include",
        body: JSON.stringify(body),
      },
      DEFAULT_TIMEOUT_MS,
    );
    const text = await r.text();
    if (!r.ok) return { ok: false, error: `HTTP ${r.status}: ${text.slice(0, MAX_ERROR_PREVIEW)}` };
    let parsed: unknown;
    try { parsed = JSON.parse(text); } catch (e) { return { ok: false, error: `invalid JSON: ${(e as Error).message}` }; }
    const result = S.AdminChainRowSchema.safeParse(parsed);
    if (!result.success) return { ok: false, error: `edge response shape invalid: ${formatSchemaIssues(result.error)}` };
    return { ok: true, data: result.data };
  } catch (e) {
    return { ok: false, error: (e as Error).message };
  }
}

export async function updateAdminChain(
  chainId: number,
  body: S.AdminChainUpdateBody,
  adminToken: string,
  actor: string,
): Promise<{ ok: true; data: S.AdminChainRow } | { ok: false; error: string }> {
  const url = `${getApiBaseUrl()}/api/admin/chains/${chainId}`;
  try {
    const r = await fetchWithTimeout(
      url,
      {
        method: "PUT",
        headers: {
          "content-type": "application/json",
          "x-arbx-admin-token": adminToken,
          "x-arbx-actor": actor,
        },
        credentials: "include",
        body: JSON.stringify(body),
      },
      DEFAULT_TIMEOUT_MS,
    );
    const text = await r.text();
    if (!r.ok) return { ok: false, error: `HTTP ${r.status}: ${text.slice(0, MAX_ERROR_PREVIEW)}` };
    let parsed: unknown;
    try { parsed = JSON.parse(text); } catch (e) { return { ok: false, error: `invalid JSON: ${(e as Error).message}` }; }
    // Response may include `hash_changed` extra field; relax schema via passthrough on parse error.
    const result = S.AdminChainRowSchema.safeParse(parsed);
    if (!result.success) return { ok: false, error: `edge response shape invalid: ${formatSchemaIssues(result.error)}` };
    return { ok: true, data: result.data };
  } catch (e) {
    return { ok: false, error: (e as Error).message };
  }
}

export async function deleteAdminChain(
  chainId: number,
  adminToken: string,
  actor: string,
): Promise<{ ok: true; data: S.AdminChainRow } | { ok: false; error: string }> {
  const url = `${getApiBaseUrl()}/api/admin/chains/${chainId}`;
  try {
    const r = await fetchWithTimeout(
      url,
      {
        method: "DELETE",
        headers: { "x-arbx-admin-token": adminToken, "x-arbx-actor": actor },
        credentials: "include",
      },
      DEFAULT_TIMEOUT_MS,
    );
    const text = await r.text();
    if (!r.ok) return { ok: false, error: `HTTP ${r.status}: ${text.slice(0, MAX_ERROR_PREVIEW)}` };
    let parsed: unknown;
    try { parsed = JSON.parse(text); } catch (e) { return { ok: false, error: `invalid JSON: ${(e as Error).message}` }; }
    const result = S.AdminChainRowSchema.safeParse(parsed);
    if (!result.success) return { ok: false, error: `edge response shape invalid: ${formatSchemaIssues(result.error)}` };
    return { ok: true, data: result.data };
  } catch (e) {
    return { ok: false, error: (e as Error).message };
  }
}

export async function probeAdminChain(
  chainId: number,
  adminToken: string,
  timeoutMs = 5000,
): Promise<{ ok: true; data: S.AdminChainProbeResult } | { ok: false; error: string }> {
  const url = `${getApiBaseUrl()}/api/admin/chains/${chainId}/probe?timeout_ms=${timeoutMs}`;
  try {
    const r = await fetchWithTimeout(
      url,
      {
        method: "POST",
        headers: { "x-arbx-admin-token": adminToken },
        credentials: "include",
      },
      Math.min(timeoutMs + 2000, 30_000),
    );
    const text = await r.text();
    if (!r.ok) return { ok: false, error: `HTTP ${r.status}: ${text.slice(0, MAX_ERROR_PREVIEW)}` };
    let parsed: unknown;
    try { parsed = JSON.parse(text); } catch (e) { return { ok: false, error: `invalid JSON: ${(e as Error).message}` }; }
    const result = S.AdminChainProbeResultSchema.safeParse(parsed);
    if (!result.success) return { ok: false, error: `edge response shape invalid: ${formatSchemaIssues(result.error)}` };
    return { ok: true, data: result.data };
  } catch (e) {
    return { ok: false, error: (e as Error).message };
  }
}

export function getTradingConfig(chainId: number) {
  return getValidated(`/api/trading-config?chain_id=${chainId}`, S.TradingConfigResponseSchema);
}

// PUT helper — mutations skip the GET retry path; mirror postValidated semantics.
export async function putTradingConfig(
  chainId: number,
  body: Omit<S.TradingConfigConfigured, "chain_id" | "configured" | "updated_at" | "updated_by">,
  adminToken: string,
  actor: string,
): Promise<{ ok: true; data: S.TradingConfigPutResult } | { ok: false; error: string }> {
  const url = `${getApiBaseUrl()}/admin/trading-config/${chainId}`;
  try {
    const r = await fetchWithTimeout(
      url,
      {
        method: "PUT",
        headers: {
          "content-type": "application/json",
          "x-arbx-admin-token": adminToken,
          "x-arbx-actor": actor,
        },
        credentials: "include",
        body: JSON.stringify(body),
      },
      DEFAULT_TIMEOUT_MS,
    );
    const text = await r.text();
    if (!r.ok) {
      return { ok: false, error: `HTTP ${r.status}: ${text.slice(0, MAX_ERROR_PREVIEW)}` };
    }
    let parsed: unknown;
    try { parsed = JSON.parse(text); }
    catch (e) { return { ok: false, error: `edge returned invalid JSON: ${(e as Error).message}` }; }
    const result = S.TradingConfigPutResultSchema.safeParse(parsed);
    if (!result.success) {
      return { ok: false, error: `edge response shape invalid: ${formatSchemaIssues(result.error)}` };
    }
    return { ok: true, data: result.data };
  } catch (e) {
    const err = e as Error;
    return { ok: false, error: err.name === "AbortError" ? `edge timeout after ${DEFAULT_TIMEOUT_MS}ms` : err.message };
  }
}

export function getAuditLogs(limit = 50, cursor?: string, action?: string, actor?: string, targetKind?: string) {
  const url = new URL(`${getApiBaseUrl()}/admin/audit`);
  url.searchParams.set("limit", String(limit));
  if (cursor) url.searchParams.set("cursor", cursor);
  if (action) url.searchParams.set("action", action);
  if (actor) url.searchParams.set("actor", actor);
  if (targetKind) url.searchParams.set("target_kind", targetKind);
  // V-AT-2: SSR server components cannot rely on the httpOnly cookie (no browser
  // session). Pass the admin token from the server-side env var so the edge
  // adminProxy authenticates via x-arbx-admin-token header.
  const adminToken = typeof window === "undefined" ? process.env.ARBX_ADMIN_TOKEN : undefined;
  const extraHeaders: Record<string, string> = adminToken ? { "x-arbx-admin-token": adminToken } : {};
  return getValidated(url.pathname + url.search, S.AuditLogsResponseSchema, { extraHeaders });
}

// ─────── DeFi data (via edge proxy → api-server defiRouter) ───────

export function getDefiChains() {
  return getValidated("/api/chains", S.DefiChainsResponseSchema);
}

export function getDefiRpcs() {
  return getValidated("/api/rpcs", S.DefiRpcsResponseSchema);
}

export function getDefiPools() {
  return getValidated("/api/pools", S.DefiPoolsResponseSchema);
}

export function getDefiMetrics() {
  return getValidated("/api/metrics/defi", S.DefiMetricsResponseSchema);
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

export function completeOnboardingPhase2(
  confirmedBy: string,
  rpcs: Array<{ chain_id: number; http: string; ws: string }>,
  slackWebhook: string | null,
  rpcProbeOk: boolean,
) {
  return postValidated(
    "/admin/onboarding/2/complete",
    { confirmed_by: confirmedBy, rpcs, slack_webhook: slackWebhook, rpc_probe_ok: rpcProbeOk },
    { "x-arbx-actor": confirmedBy },
    S.OnboardingPhase2ResultSchema,
  );
}

export function completeOnboardingPhase3(
  confirmedBy: string,
  simProvider: string,
  simEndpoint: string,
  tokenSafety: { provider: string; base_url: string; api_key_ref: string },
  scoringWeightsSignedOff: boolean,
) {
  return postValidated(
    "/admin/onboarding/3/complete",
    {
      confirmed_by: confirmedBy,
      sim_provider: simProvider,
      sim_endpoint: simEndpoint,
      token_safety: tokenSafety,
      scoring_weights_signed_off: scoringWeightsSignedOff,
    },
    { "x-arbx-actor": confirmedBy },
    S.OnboardingPhase3ResultSchema,
  );
}

export function completeOnboardingPhase4(
  confirmedBy: string,
  relayCount: number,
  signerZeroBalanceVerified: boolean,
  cfTunnelDeployed: boolean,
) {
  return postValidated(
    "/admin/onboarding/4/complete",
    {
      confirmed_by: confirmedBy,
      relay_count: relayCount,
      signer_zero_balance_verified: signerZeroBalanceVerified,
      cf_tunnel_deployed: cfTunnelDeployed,
    },
    { "x-arbx-actor": confirmedBy },
    S.OnboardingPhase4ResultSchema,
  );
}

export function completeOnboardingPhase5(
  confirmedBy: string,
  pagerdutIntegrationKeyRef: string,
  backupDestination: string,
  capitalCapUsd: number,
  incidentContactsCount: number,
  paperModeOff: boolean,
) {
  return postValidated(
    "/admin/onboarding/5/complete",
    {
      confirmed_by: confirmedBy,
      pagerduty_integration_key_ref: pagerdutIntegrationKeyRef,
      backup_destination: backupDestination,
      capital_cap_usd: capitalCapUsd,
      incident_contacts_count: incidentContactsCount,
      paper_mode_off: paperModeOff,
    },
    { "x-arbx-actor": confirmedBy },
    S.OnboardingPhase5ResultSchema,
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

// ── Operations PnL (Sprint 3) ───────────────────────────────────────────
// Public-read PMI/EVM KPIs surfaced through the edge (no admin token).

export function getOperationsKpi(chainId = 1): Promise<Result<KpiPayload>> {
  return getValidated(`/api/operations/kpi?chain_id=${chainId}`, KpiPayloadSchema);
}

export function getOperationsScurve(chainId = 1, bucketMinutes = 15): Promise<Result<SCurvePayload>> {
  return getValidated(
    `/api/operations/scurve?chain_id=${chainId}&bucket_minutes=${bucketMinutes}`,
    SCurvePayloadSchema,
  );
}

export function getOperationsVariance(chainId = 1): Promise<Result<VariancePayload>> {
  return getValidated(`/api/operations/variance?chain_id=${chainId}`, VariancePayloadSchema);
}

// Scanner pipeline heartbeat — latest funnel snapshot persisted by
// searcher-rs::workers::heartbeat_worker. 404 when key absent (searcher
// down or recently restarted) — UI treats as "loading" state.
export function getScannerHeartbeat(chainId = 1): Promise<Result<ScannerHeartbeatResponse>> {
  return getValidated(`/api/scanner/heartbeat?chain_id=${chainId}`, ScannerHeartbeatResponseSchema);
}

// ── Strategy Catalog (Sprint 2) ─────────────────────────────────────────
// Public-read universal MEV strategy library — surfaces the dropdown for
// "+ Add strategy from catalog" in the /strategies page.
export function getStrategyCatalog(): Promise<Result<S.StrategyCatalogResponse>> {
  return getValidated("/api/strategy-catalog", S.StrategyCatalogResponseSchema);
}

// ─────── Paper Mode State (Task 7 — Paper-Mode State Authority) ───────
//
// Returns the canonical paper-mode resolution for the given chain (or global).
// Fail-safe: on any error (network, timeout, parse) the Result resolves to
// { ok: false, error } — callers never get a thrown promise.
export function getPaperModeState(chainId?: number): Promise<Result<S.PaperModeState>> {
  const path = chainId != null
    ? `/api/paper-mode/state?chain_id=${chainId}`
    : `/api/paper-mode/state`;
  return getValidated(path, S.PaperModeStateSchema);
}
