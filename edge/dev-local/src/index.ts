/*
 * DEV-ONLY local edge shim. Mirrors the Cloudflare Worker's public interface
 * so developers without a CF account can run the full stack locally.
 *
 * DO NOT deploy this to production. It lacks CF's protections (WAF, DDoS, rate-limit).
 */

import express from "express";
import {
  createHttpLogger,
  createLogger,
  healthHandler,
  metricsHandler,
  metricsMiddleware,
  traceIdMiddleware,
  loadAppConfig,
  requireEnv,
  initMetrics,
  safeTokenEqual,
} from "@arbx/shared";
// Stricter per-path rate-limit + 401 lockout for /admin/session POST live in
// admin-session-limits.ts (pure module so unit tests don't trigger config load).
import {
  LOCKOUT_MS,
  LOCKOUT_THRESHOLD,
  hitAdminSession,
  isLockedOut,
  recordAuthFailure,
  recordAuthSuccess,
} from "./admin-session-limits.js";
import { emitAuditEvent, tokenFingerprint } from "./audit-emit.js";
import { parseCookies as parseCookiesShared, resolveAdminToken } from "./admin-token-resolver.js";
import { createProxyMiddleware } from "http-proxy-middleware";

const SERVICE = "edge-dev-local";
const VERSION = "0.1.0";
const cfg = loadAppConfig();
const logger = createLogger({ service: SERVICE, level: cfg.observability.log_level ?? "info" });
initMetrics(SERVICE);

const API_SERVER_URL = process.env["API_SERVER_URL"] ?? "http://api-server:8080";
const ARBX_EDGE_TOKEN = requireEnv("ARBX_EDGE_TOKEN");
// QUANTUM FULLSTACK SYMMETRY — frontend (Next.js) upstream for the SPA-fallback
// catch-all. Default resolves via Docker DNS on arbx-net. PUBLIC_EDGE_HOST is the
// externally-visible https host, forwarded so Next builds https-correct absolute
// URLs (no Mixed Content).
const FRONTEND_URL = process.env["FRONTEND_URL"] ?? "http://frontend:5173";
const PUBLIC_EDGE_HOST = process.env["PUBLIC_EDGE_HOST"] ?? "<VPS_HOST>";

// Very naive in-memory rate-limit (per-IP, 60s window, 120 req).
const WINDOW_MS = 60_000;
const MAX_HITS = 120;
const rl = new Map<string, { count: number; windowStart: number }>();
function hit(ip: string): { ok: boolean; remaining: number } {
  const now = Date.now();
  const cur = rl.get(ip);
  if (!cur || now - cur.windowStart > WINDOW_MS) {
    rl.set(ip, { count: 1, windowStart: now });
    return { ok: true, remaining: MAX_HITS - 1 };
  }
  cur.count++;
  if (cur.count > MAX_HITS) return { ok: false, remaining: 0 };
  return { ok: true, remaining: MAX_HITS - cur.count };
}

const startedAt = new Date();
const app = express();
app.disable("x-powered-by");

// DEV-LOCAL CORS: allowlist localhost / 127.0.0.1 / *.ape-tv.net via regex, PLUS any
// EXACT origin in the operator-configured ALLOWED_ORIGINS env (comma-separated). This
// covers the public VPS frontend served from a raw-IP origin (e.g.
// http://<VPS_IP>:5173) that the regex deliberately does not match. The IP is
// NEVER hardcoded in code — the operator supplies origins via env (RULE 00 / no-hardcode);
// the env is already set on the edge container. Production uses CF Workers CORS.
const CORS_ENV_ORIGINS = (process.env["ALLOWED_ORIGINS"] ?? "")
  .split(",")
  .map((s) => s.trim())
  .filter(Boolean);
app.use((req, res, next) => {
  const origin = req.headers.origin;
  // CodeQL js/cors-misconfiguration-for-credentials: with credentials=true we reflect
  // ONLY allowlisted origins, never "*", AND we gate the reflected value behind a
  // membership check the query recognizes as a sanitizer (Set.has). A bare
  // RegExp.test() guard is NOT modeled by CodeQL, which is why the prior regex-OR form
  // stayed flagged. Same allow-rules (env-exact OR localhost/loopback OR *.ape-tv.net);
  // we just funnel the final reflected origin through an explicit allowlist Set.
  const CORS_ALLOWED = /^https?:\/\/(localhost|127\.0\.0\.1)(:\d+)?$|^https:\/\/[a-z0-9-]+\.ape-tv\.net$/i;
  if (origin) {
    const allowedOrigins = new Set<string>(CORS_ENV_ORIGINS);
    if (CORS_ALLOWED.test(origin)) allowedOrigins.add(origin);
    if (allowedOrigins.has(origin)) {
      res.setHeader("access-control-allow-origin", origin);
      res.setHeader("access-control-allow-credentials", "true");
      res.setHeader("access-control-allow-headers", "content-type, x-arbx-admin-token, x-arbx-trace-id, x-arbx-actor");
      res.setHeader("access-control-allow-methods", "GET, POST, PUT, DELETE, OPTIONS");
    }
  }
  if (req.method === "OPTIONS") { res.status(204).end(); return; }
  next();
});

app.use(traceIdMiddleware());
app.use(createHttpLogger(SERVICE));
app.use(metricsMiddleware(SERVICE));
app.use((req, res, next) => {
  const ip = (req.headers["x-forwarded-for"] as string | undefined)?.split(",")[0]?.trim() ?? req.socket.remoteAddress ?? "unknown";
  const { ok, remaining } = hit(ip);
  res.setHeader("x-ratelimit-remaining", String(remaining));
  if (!ok) {
    res.status(429).json({ error: "rate_limited" });
    return;
  }
  next();
});

// JSON body parser — registered early so all POST/PUT routes (including
// /api/admin/chains and /admin/session, which arrive before the original
// later-mounted express.json) receive a parsed req.body.
app.use(express.json({ limit: "64kb" }));

app.get("/health", healthHandler(SERVICE, VERSION, startedAt));
app.get("/metrics", metricsHandler);

async function proxy(path: string, req: express.Request, res: express.Response) {
  try {
    // 2026-05-10 fix (revised): propagate query string from the client request
    // to the upstream api-server, but ONLY when the caller-supplied `path` does
    // NOT already contain a query. The earlier revision appended the request
    // query to paths that pre-built their own (e.g. /api/v1/trading-config
    // ?chain_id=1), producing duplicate keys (?chain_id=1&chain_id=1) which
    // Express parses as an array → Number(arr) → NaN → 400 invalid_chain_id.
    //
    // Two-mode contract:
    //   1. path has NO query     → forward the client's query verbatim.
    //   2. path ALREADY has query → caller took responsibility; do not append.
    //
    // Callers that need both behaviours should pre-build the full upstream
    // path (mode 2) — they already have to extract+encode what they want.
    let upstreamPath = path;
    if (!path.includes("?")) {
      const reqQuery = req.url.includes("?") ? req.url.slice(req.url.indexOf("?") + 1) : "";
      if (reqQuery) {
        upstreamPath += "?" + reqQuery;
      }
    }
    const upstream = await fetch(`${API_SERVER_URL}${upstreamPath}`, {
      headers: {
        "x-arbx-edge-token": ARBX_EDGE_TOKEN,
        "x-arbx-trace-id": (req as express.Request & { traceId?: string }).traceId ?? "",
        "accept": "application/json",
      },
    });
    const body = await upstream.text();
    res.status(upstream.status);
    res.setHeader("content-type", upstream.headers.get("content-type") ?? "application/json");
    res.send(body);
  } catch (e) {
    logger.error({ err: (e as Error).message, path }, "proxy error");
    res.status(502).json({ error: "upstream_unreachable" });
  }
}

// FE-CRIT-01 — /status content-negotiation. The edge historically proxied the
// api-server's JSON /status UNCONDITIONALLY, which SHADOWED the Next.js SPA
// /status page (a browser navigating to /status got raw JSON instead of the
// dashboard). This predicate decides whether a request wants the backend JSON
// (API clients) or the SPA HTML (browser navigation):
//   • JSON  ⇐ Accept: application/json, OR ?format=json, OR a CLI User-Agent
//            (curl / httpie / wget / python-requests).
//   • HTML  ⇐ Accept includes text/html (browser navigation) and not the above.
// When neither is decisive we DEFAULT TO JSON to preserve the legacy API-client
// behaviour exactly (no contract drift for existing programmatic callers).
function statusWantsJson(req: express.Request): boolean {
  const fmt = typeof req.query["format"] === "string" ? req.query["format"].toLowerCase() : "";
  if (fmt === "json") return true;
  const ua = (req.header("user-agent") ?? "").toLowerCase();
  if (/\b(curl|httpie|wget|python-requests|go-http-client|node-fetch|axios)\b/.test(ua)) {
    return true;
  }
  const accept = (req.header("accept") ?? "").toLowerCase();
  // Browser navigations send `text/html` first. Only treat as HTML when the
  // client explicitly asks for text/html AND did not ask for application/json.
  if (accept.includes("text/html") && !accept.includes("application/json")) {
    return false;
  }
  // Default (no Accept, */*, application/json, etc.) → JSON, preserving the
  // exact behaviour API clients relied on before this change.
  return true;
}

// N4 fix (2026-06-13): http-proxy-middleware v3 strips the mount prefix before
// forwarding to upstream. When mounted at '/socket.io', the proxy sends '/?EIO=4'
// instead of '/socket.io/?EIO=4', causing the api-server to return 404 for
// Socket.IO polling requests. pathRewrite restores the stripped prefix so the
// api-server receives the correct path for both HTTP polling and WS upgrade.
const wsProxy = createProxyMiddleware({
  target: API_SERVER_URL, 
  ws: true, 
  changeOrigin: true,
  pathRewrite: (path: string) => `/socket.io${path}`,
});
app.use('/socket.io', wsProxy);

// FE-CRIT-01 — /status is content-negotiated. The actual handler is registered
// LATER (just before the SPA catch-all) so its HTML branch can delegate to
// `frontendProxy` (a const declared further down). API clients (Accept: json,
// ?format=json, curl/wget/httpie UA) still get the backend JSON verbatim.
// Health probe alias — REST convention for load balancers / external monitors.
// Proxied to api-server's /api/health which returns service/version/uptime JSON.
app.get("/api/health", (req, res) => proxy("/api/health", req, res));

// ═══════════════════════════════════════════════════════════════════════════════
// LATENCY-OPTIMIZED PASS-THROUGH PROXY (<30ms target)
// Routes: /api/v1/health, /api/v1/metrics/entropy
// Strategy: Direct passthrough using existing proxy helper (no body buffering)
// This eliminates JSON serialization overhead and reduces memory pressure.
// ═══════════════════════════════════════════════════════════════════════════════
app.get('/api/v1/health', (req, res) => {
  // Forward with edge token for auth
  req.headers['x-arbx-edge-token'] = ARBX_EDGE_TOKEN;
  proxy('/api/v1/health', req, res);
});
app.get('/api/v1/metrics/entropy', (req, res) => {
  req.headers['x-arbx-edge-token'] = ARBX_EDGE_TOKEN;
  proxy('/api/v1/metrics/entropy', req, res);
});
// ═══════════════════════════════════════════════════════════════════════════════

app.get("/api/opportunities/live", (req, res) => proxy("/api/v1/opportunities/live", req, res));
app.get("/api/scanner/heartbeat", (req, res) => {
  const chain = String(req.query["chain_id"] ?? 1);
  proxy(`/api/v1/scanner/heartbeat?chain_id=${encodeURIComponent(chain)}`, req, res);
});
app.get("/api/strategies/runtime-status", (req, res) => {
  const chain = String(req.query["chain_id"] ?? 1);
  proxy(`/api/v1/strategies/runtime-status?chain_id=${encodeURIComponent(chain)}`, req, res);
});
app.get("/api/risk/alerts", (req, res) => proxy("/api/v1/risk/alerts", req, res));
// S7: new operator-console endpoints.
app.get("/api/executions/recent", (req, res) => proxy("/api/v1/executions/recent", req, res));
app.get("/api/recon/summary", (req, res) => proxy("/api/v1/recon/summary", req, res));
app.get("/api/config/current", (req, res) => proxy("/api/v1/config/current", req, res));
// Credentials health summary (counts only) for the sidebar "needs attention" badge.
app.get("/api/credentials/summary", (req, res) => proxy("/api/v1/credentials/summary", req, res));
// RPC registry status (counts only) for the /rpcs panel.
app.get("/api/rpc/status", (req, res) => proxy("/api/v1/rpc/status", req, res));
// Phase 0.5: relays catalog (public list of enabled) + onboarding status.
app.get("/api/relays", (req, res) => proxy("/api/v1/relays", req, res));
app.get("/api/onboarding/status", (req, res) => proxy("/api/v1/onboarding/status", req, res));
app.get("/api/readiness", (req, res) => proxy("/api/v1/readiness", req, res));
// P2 readiness extras: derived blockers list and go/no-go decision.
app.get("/api/readiness/blockers", (req, res) => proxy("/api/v1/readiness/blockers", req, res));
app.get("/api/readiness/decision", (req, res) => proxy("/api/v1/readiness/decision", req, res));
// Server-side evaluated 4-step "Live Readiness" stepper (replaces localStorage count).
app.get("/api/readiness/steps", (req, res) => proxy("/api/v1/readiness/steps", req, res));
// P2-continued: agent teams status.
app.get("/api/agents/status", (req, res) => proxy("/api/v1/agents/status", req, res));
// A.8 confidence scoring wire status.
app.get("/api/scoring/status", (req, res) => proxy("/api/v1/scoring/status", req, res));
// Live-readiness grid panels: sim/fork status + paper-shadow metrics. Without
// these the ForkValidationPanel + PaperShadowPanel 404 at the edge and render
// DEGRADED/INACTIVE even though the api-server serves them (audit gap, 2026-06-22).
app.get("/api/sim-ctl/fork-status", (req, res) => proxy("/api/v1/sim-ctl/fork-status", req, res));
app.get("/api/metrics/paper-shadow", (req, res) => proxy("/api/v1/metrics/paper-shadow", req, res));
// A.6 comprehensive circuit breakers.
app.get("/api/risk/circuit-breakers/status", (req, res) => proxy("/api/v1/risk/circuit-breakers/status", req, res));
app.get("/api/risk/circuit-breakers/events", (req, res) => proxy("/api/v1/risk/circuit-breakers/events", req, res));

// QUANTUM FULLSTACK SYMMETRY — OMEGA Route Discovery radar + cartridge telemetry
// read-only snapshots. api-server mounts these at the SAME paths (no /v1/ prefix),
// fed by its Redis pub/sub cache. Observe-only; never touches opportunities.
app.get("/api/route-discovery/status", (req, res) => proxy("/api/route-discovery/status", req, res));
app.get("/api/route-discovery/latest", (req, res) => proxy("/api/route-discovery/latest", req, res));
app.get("/api/route-discovery/metrics", (req, res) => proxy("/api/route-discovery/metrics", req, res));
app.get("/api/route-discovery/routes", (req, res) => proxy("/api/route-discovery/routes", req, res));
app.get("/api/cartridges/status", (req, res) => proxy("/api/cartridges/status", req, res));
app.get("/api/cartridges/telemetry/latest", (req, res) => proxy("/api/cartridges/telemetry/latest", req, res));

// FE-CRIT — system manifest read surface. api-server mounts these at
// /api/system/* (system-manifest.ts, no /v1/ prefix). proxy() forwards the
// upstream status verbatim — if api-server returns non-2xx, the real status +
// body are surfaced; on transport failure proxy() returns an honest 502
// {error:"upstream_unreachable"}. NEVER a fabricated 200. Observe-only.
app.get("/api/system/drift", (req, res) => proxy("/api/system/drift", req, res));
app.get("/api/system/feature_manifest", (req, res) => proxy("/api/system/feature_manifest", req, res));
// SED Convergence Status — backend mounts at /api/v1/sed/status (sed-status.ts).
// Observe-only; never writes capital or triggers execution. Query string
// (chain_id, window_minutes) forwarded verbatim by proxy() mode-1.
app.get("/api/sed/status", (req, res) => proxy("/api/v1/sed/status", req, res));
// Paper Trade History — backend mounts at /api/v1/paper/history (paper-history-api.ts).
// Read-only drift-analysis surface over paper_trade_runs. Never touches capital.
app.get("/api/paper/history", (req, res) => proxy("/api/v1/paper/history", req, res));
app.get("/api/paper/history/summary", (req, res) => proxy("/api/v1/paper/history/summary", req, res));

// FE-CRIT-03/04 — honest contract / capital / crucible read surface. api-server
// mounts these at /api/* (no /v1/ prefix). proxy() forwards upstream status
// verbatim (honest 502 on transport failure, never a fabricated 200). The
// frontend reads them via the edge (getApiBaseUrl()), so the edge MUST proxy
// them — otherwise the edge 404s and the 6 omega-s5 pages stay data-dead.
app.get("/api/contracts", (req, res) => proxy("/api/contracts", req, res));
app.get("/api/capital-gates", (req, res) => proxy("/api/capital-gates", req, res));
app.get("/api/crucible/status", (req, res) => proxy("/api/crucible/status", req, res));

// =============================================================================
// Web3 safe-gated WALLET surface + SIWE identity-only auth. Mirrors the
// /api/system/* proxy pattern: upstream status forwarded verbatim, honest 502
// on transport failure, NEVER a fabricated 200. The api-server enforces the
// hard invariants (live OFF, capital 0, broadcast OFF, no signer); the edge is
// a transparent pass-through. SIWE sessions are httpOnly cookies set by the
// api-server, so this public proxy forwards the client Cookie header upstream
// and relays any upstream Set-Cookie back to the browser.
// =============================================================================

// Public POST proxy that forwards the JSON body + the client's Cookie header to
// the api-server, and relays upstream Set-Cookie back to the browser. Used by
// the wallet/auth POST endpoints (intent, simulate, signature/verify, siwe
// verify, logout). NEVER a fabricated 200.
async function walletProxy(path: string, req: express.Request, res: express.Response, method: string): Promise<void> {
  try {
    const upstream = await fetch(`${API_SERVER_URL}${path}`, {
      method,
      headers: {
        "content-type": "application/json",
        "x-arbx-edge-token": ARBX_EDGE_TOKEN,
        "x-arbx-trace-id": (req as express.Request & { traceId?: string }).traceId ?? "",
        // Forward the wallet identity cookie so /api/auth/session reflects login.
        ...(req.headers.cookie ? { cookie: req.headers.cookie } : {}),
        accept: "application/json",
      },
      body: method !== "GET" && method !== "HEAD" ? JSON.stringify(req.body ?? {}) : null,
    });
    const text = await upstream.text();
    // Relay the upstream httpOnly Set-Cookie (SIWE session) to the browser.
    const setCookie = upstream.headers.get("set-cookie");
    if (setCookie) res.setHeader("set-cookie", setCookie);
    res.status(upstream.status);
    res.setHeader("content-type", upstream.headers.get("content-type") ?? "application/json");
    res.send(text);
  } catch (e) {
    logger.error({ err: (e as Error).message, path }, "wallet proxy error");
    res.status(502).json({ error: "upstream_unreachable" });
  }
}

// GET /api/wallet/* — public read surface (status + safety). proxy() already
// forwards cookies? No — the read endpoints don't need the cookie. Plain proxy.
app.get("/api/wallet/status", (req, res) => proxy("/api/wallet/status", req, res));
app.get("/api/wallet/safety", (req, res) => proxy("/api/wallet/safety", req, res));
// POST /api/wallet/* — intent + simulate + signature verify (never broadcast).
app.post("/api/wallet/intent", (req, res) => walletProxy("/api/wallet/intent", req, res, "POST"));
app.post("/api/wallet/simulate", (req, res) => walletProxy("/api/wallet/simulate", req, res, "POST"));
app.post("/api/wallet/signature/verify", (req, res) => walletProxy("/api/wallet/signature/verify", req, res, "POST"));
// SIWE identity-only auth. nonce (GET) needs no cookie; verify/session/logout do.
app.get("/api/auth/siwe/nonce", (req, res) => walletProxy("/api/auth/siwe/nonce", req, res, "GET"));
app.post("/api/auth/siwe/verify", (req, res) => walletProxy("/api/auth/siwe/verify", req, res, "POST"));
app.get("/api/auth/session", (req, res) => walletProxy("/api/auth/session", req, res, "GET"));
app.post("/api/auth/logout", (req, res) => walletProxy("/api/auth/logout", req, res, "POST"));

// Operator Self-Test Center — presence-only credential matrix + 10-block
// checklist aggregator. Mirrors the /api/wallet/* GET pattern: plain proxy,
// upstream status forwarded verbatim, honest 502 on transport failure, NEVER a
// fabricated 200. The api-server guarantees no env VALUE ever appears in these
// bodies (presence booleans only).
app.get("/api/operator/credentials/status", (req, res) => proxy("/api/operator/credentials/status", req, res));
app.get("/api/operator/selftest", (req, res) => proxy("/api/operator/selftest", req, res));

// FASE B Gate-C — route-discovery OUTCOMES analytics (read-only over the durable
// Postgres `route_discovery_outcomes` table; the shadow emitter's resolved
// outcomes + Paso 9 `reason` column). This is the read-side of the passive sink:
// it surfaces the hit-rate series and the reason distribution (the "why 0%").
// Edge path mirrors api-server /api/v1/route-discovery-outcomes*. Query string
// (?hours= / ?limit=) is forwarded verbatim by proxy() (mode-1). Observe-only;
// never touches arbx:opps:detected, capital, or execution.
app.get("/api/route-discovery-outcomes/summary", (req, res) => proxy("/api/v1/route-discovery-outcomes/summary", req, res));
app.get("/api/route-discovery-outcomes", (req, res) => proxy("/api/v1/route-discovery-outcomes", req, res));

// Chains Admin registry — admin-token gated. Routed through adminProxy so the
// V-AT-1 httpOnly cookie (arbx_admin_session) is translated to the upstream
// x-arbx-admin-token header. adminProxy is defined later in the file; function
// hoisting via `async function` keeps the reference valid here.
app.get("/api/admin/chains", (req, res) => {
  const search = new URL(req.url, "http://x").search || "";
  adminProxy(`/api/v1/admin/chains${search}`, req, res, "GET");
});
app.get("/api/admin/chains/:chain_id", (req, res) => {
  adminProxy(`/api/v1/admin/chains/${encodeURIComponent(req.params["chain_id"] ?? "")}`, req, res, "GET");
});
app.post("/api/admin/chains", (req, res) => {
  adminProxy("/api/v1/admin/chains", req, res, "POST");
});
app.put("/api/admin/chains/:chain_id", (req, res) => {
  adminProxy(`/api/v1/admin/chains/${encodeURIComponent(req.params["chain_id"] ?? "")}`, req, res, "PUT");
});
app.delete("/api/admin/chains/:chain_id", (req, res) => {
  adminProxy(`/api/v1/admin/chains/${encodeURIComponent(req.params["chain_id"] ?? "")}`, req, res, "DELETE");
});
app.post("/api/admin/chains/:chain_id/probe", (req, res) => {
  const search = new URL(req.url, "http://x").search || "";
  adminProxy(`/api/v1/admin/chains/${encodeURIComponent(req.params["chain_id"] ?? "")}/probe${search}`, req, res, "POST");
});
// RPC registry sync — admin import (Excel catalog → rpc_endpoints) + bare reload.
app.post("/api/admin/rpcs/import", (req, res) => {
  adminProxy("/api/v1/admin/rpcs/import", req, res, "POST");
});
app.post("/api/admin/rpcs/reload", (req, res) => {
  adminProxy("/api/v1/admin/rpcs/reload", req, res, "POST");
});
// Topology Vault — admin-token gated RPC/WSS hot-swap control plane.
// Uses the same V-AT-1 httpOnly cookie translation as Chains Admin; the
// upstream API Server stores full URLs in Vault/Redis and returns only masked
// provider snapshots to the browser.
app.get("/api/admin/topology/snapshot", (req, res) => {
  adminProxy("/api/admin/topology/snapshot", req, res, "GET");
});
app.post("/api/admin/topology/mutations", (req, res) => {
  adminProxy("/api/admin/topology/mutations", req, res, "POST");
});
// Trading Config — operator-tunable strategy parameters per chain.
app.get("/api/trading-config", (req, res) => {
  const chain = typeof req.query["chain_id"] === "string" ? req.query["chain_id"] : "1";
  proxy(`/api/v1/trading-config?chain_id=${encodeURIComponent(chain)}`, req, res);
});
// Cartridge Filters (Idea 1 Phase-1) — public read of the per-chain route pre-filter prefs.
app.get("/api/cartridge-filters", (req, res) => {
  const chain = typeof req.query["chain_id"] === "string" ? req.query["chain_id"] : "1";
  proxy(`/api/v1/cartridge-filters?chain_id=${encodeURIComponent(chain)}`, req, res);
});
// Cartridge Forge (Idea 2) — public list of injected cartridges (registry + counters).
app.get("/api/cartridges", (req, res) => {
  const qs = new URL(req.url, "http://x").search || "";
  proxy(`/api/v1/cartridges${qs}`, req, res);
});
// Operations PnL — Sprint 3 PMI/EVM KPI surface.
app.get("/api/operations/kpi", (req, res) => {
  const chain = typeof req.query["chain_id"] === "string" ? req.query["chain_id"] : "1";
  proxy(`/api/v1/operations/kpi?chain_id=${encodeURIComponent(chain)}`, req, res);
});
app.get("/api/operations/scurve", (req, res) => {
  const chain = typeof req.query["chain_id"] === "string" ? req.query["chain_id"] : "1";
  const bucket = typeof req.query["bucket_minutes"] === "string" ? req.query["bucket_minutes"] : "15";
  proxy(`/api/v1/operations/scurve?chain_id=${encodeURIComponent(chain)}&bucket_minutes=${encodeURIComponent(bucket)}`, req, res);
});
app.get("/api/operations/variance", (req, res) => {
  const chain = typeof req.query["chain_id"] === "string" ? req.query["chain_id"] : "1";
  proxy(`/api/v1/operations/variance?chain_id=${encodeURIComponent(chain)}`, req, res);
});
// Strategy catalog (Sprint 2 — universal MEV strategy library, read-only).
app.get("/api/strategy-catalog", (req, res) => proxy("/api/v1/strategy-catalog", req, res));
app.get("/api/strategy-catalog/active", (req, res) => {
  const chain = typeof req.query["chain_id"] === "string" ? req.query["chain_id"] : "1";
  proxy(`/api/v1/strategy-catalog/active?chain_id=${encodeURIComponent(chain)}`, req, res);
});
// DeFi data routes (defiRouter is mounted at /api in api-server, no /v1/ prefix).
app.get("/api/chains",  (req, res) => proxy("/api/chains", req, res));
app.get("/api/rpcs",    (req, res) => proxy("/api/rpcs", req, res));
// Phase 2 maps /api/pools to the richer route-finder /api/v1/pools, which
// returns the {count, items} envelope. The frontend's DefiPoolsResponseSchema
// (like /api/chains + /api/rpcs) expects {success, data}, so reshape the
// envelope here — `items` become `data` — keeping all three defi endpoints on
// one contract. RULE 00: never fabricate — an upstream error is forwarded
// verbatim; on a transport failure we return an honest {success:false} so the
// dashboard shows its degraded state rather than a fake-empty list.
app.get("/api/pools", async (req, res) => {
  const qs = new URLSearchParams();
  for (const [k, v] of Object.entries(req.query)) {
    if (typeof v === "string") qs.set(k, v);
  }
  const qStr = qs.toString();
  try {
    const upstream = await fetch(`${API_SERVER_URL}/api/v1/pools${qStr ? "?" + qStr : ""}`, {
      headers: {
        "x-arbx-edge-token": ARBX_EDGE_TOKEN,
        "x-arbx-trace-id": (req as express.Request & { traceId?: string }).traceId ?? "",
        accept: "application/json",
      },
    });
    const text = await upstream.text();
    if (!upstream.ok) {
      res.status(upstream.status).setHeader("content-type", "application/json");
      res.send(text);
      return;
    }
    let parsed: unknown = null;
    try {
      parsed = JSON.parse(text);
    } catch {
      parsed = null;
    }
    const items =
      parsed && typeof parsed === "object" && Array.isArray((parsed as { items?: unknown }).items)
        ? (parsed as { items: unknown[] }).items
        : [];
    res.json({ success: true, data: items });
  } catch (e) {
    logger.error({ err: (e as Error).message }, "/api/pools reshape error");
    res.status(502).json({ success: false, error: "upstream_unreachable", data: [] });
  }
});
app.get("/api/metrics/defi", (req, res) => proxy("/api/metrics", req, res));
// Phase 2 route-finder: DEX catalog + pool catalog.
app.get("/api/dexes", (req, res) => {
  const chain = typeof req.query["chain_id"] === "string" ? req.query["chain_id"] : "1";
  proxy(`/api/v1/dexes?chain_id=${encodeURIComponent(chain)}`, req, res);
});
app.get("/api/v1/pools", (req, res) => {
  // Forward all query parameters verbatim (chain_id, dex_id, protocol_type, limit).
  const qs = new URLSearchParams();
  for (const [k, v] of Object.entries(req.query)) {
    if (typeof v === "string") qs.set(k, v);
  }
  const qStr = qs.toString();
  proxy(`/api/v1/pools${qStr ? "?" + qStr : ""}`, req, res);
});

// 2026-05-10 audit follow-up: DEX Registry page calls /api/v1/dexes WITHOUT
// chain_id to get the cross-chain aggregate. Forward verbatim (any query) so
// the backend's dual-shape route can branch on chain_id presence.
app.get("/api/v1/dexes", (req, res) => {
  const qs = new URLSearchParams();
  for (const [k, v] of Object.entries(req.query)) {
    if (typeof v === "string") qs.set(k, v);
  }
  const qStr = qs.toString();
  proxy(`/api/v1/dexes${qStr ? "?" + qStr : ""}`, req, res);
});

// PUT /api/v1/dexes/:id/active — admin toggle from /dex-registry.
// Uses adminProxy convention so the operator's httpOnly session cookie is
// translated to the real x-arbx-admin-token header on the way upstream.
// Declared AFTER adminProxy is defined; see below near other admin PUTs.

// Operational Wallets endpoints — list + balances + allowances.
app.get("/api/v1/wallets", (req, res) => {
  proxy("/api/v1/wallets", req, res);
});
app.get("/api/v1/wallets/:address/balances", (req, res) => {
  const addr = String(req.params.address ?? "");
  proxy(`/api/v1/wallets/${encodeURIComponent(addr)}/balances`, req, res);
});
app.get("/api/v1/wallets/:address/allowances", (req, res) => {
  const addr = String(req.params.address ?? "");
  proxy(`/api/v1/wallets/${encodeURIComponent(addr)}/allowances`, req, res);
});

// S7: admin POST proxies — forward caller's x-arbx-admin-token alongside the
// edge token. Rejected by api-server if admin token is missing/wrong.
// express.json body parser is already registered above (moved earlier so
// admin chains POST/PUT/probe also receive a parsed body).

// ─── V-AT-1 hardening: httpOnly cookie session for admin token ───
// The admin token (T1 per secrets.policy.md) is stored in an httpOnly cookie
// instead of localStorage, eliminating the XSS attack vector.
const SESSION_COOKIE = "arbx_admin_session";
const SESSION_TTL_COOKIE = "arbx_admin_session_ttl";
const SESSION_TTL_S = 8 * 60 * 60; // 8 hours

// Cookie parser re-exported from the V-AT-1 token resolver module so both
// runtime and unit tests share the same implementation.
const parseCookies = parseCookiesShared;

// POST /admin/session — validate token, set httpOnly cookie.
// Hardened: per-path rate-limit (5/min/IP) + 401 lockout (10 fails → 15min block).
app.post("/admin/session", async (req, res) => {
  const ip = (req.headers["x-forwarded-for"] as string | undefined)?.split(",")[0]?.trim()
    ?? req.socket.remoteAddress ?? "unknown";
  const ua = req.header("user-agent");
  const traceId = (req as express.Request & { traceId?: string }).traceId;

  // ── Event 8: locked_attempt ──
  if (isLockedOut(ip)) {
    emitAuditEvent(API_SERVER_URL, ARBX_EDGE_TOKEN, {
      action: "auth.locked_attempt", actor: "anonymous", ipAddress: ip,
      userAgent: ua, traceId, afterState: { locked: true },
    });
    res.status(429).json({ error: "locked_out", retry_after_s: LOCKOUT_MS / 1000 });
    return;
  }
  const rate = hitAdminSession(ip);
  res.setHeader("x-ratelimit-admin-session-remaining", String(rate.remaining));
  // ── Event 7: rate_limited ──
  if (!rate.ok) {
    emitAuditEvent(API_SERVER_URL, ARBX_EDGE_TOKEN, {
      action: "auth.rate_limited", actor: "anonymous", ipAddress: ip,
      userAgent: ua, traceId, afterState: { remaining: 0 },
    });
    res.status(429).json({ error: "rate_limited" });
    return;
  }

  const { token } = req.body as { token?: string };
  if (!token) { res.status(400).json({ error: "token_required" }); return; }
  try {
    // Validate the token by probing api-server's admin health endpoint.
    const probe = await fetch(`${API_SERVER_URL}/admin/killswitch`, {
      method: "POST",
      headers: {
        "content-type": "application/json",
        "x-arbx-edge-token": ARBX_EDGE_TOKEN,
        "x-arbx-admin-token": token,
      },
      // Dry probe: send a no-op body that api-server rejects gracefully.
      body: JSON.stringify({ enabled: null, reason: "__session_probe__" }),
    });
    // api-server returns 401 if admin token is wrong.
    if (probe.status === 401 || probe.status === 403) {
      recordAuthFailure(ip);
      // ── Event 4: login_fail ──
      emitAuditEvent(API_SERVER_URL, ARBX_EDGE_TOKEN, {
        action: "auth.login_fail", actor: "anonymous", ipAddress: ip,
        userAgent: ua, traceId,
      });
      // ── Event 6: lockout_triggered (if threshold just crossed) ──
      if (isLockedOut(ip)) {
        emitAuditEvent(API_SERVER_URL, ARBX_EDGE_TOKEN, {
          action: "auth.lockout_triggered", actor: "anonymous", ipAddress: ip,
          userAgent: ua, traceId,
          afterState: { blocked_until_ms: Date.now() + LOCKOUT_MS, total_fails: LOCKOUT_THRESHOLD },
        });
      }
      res.status(401).json({ error: "invalid_admin_token" });
      return;
    }
  } catch (e) {
    res.status(502).json({ error: "upstream_unreachable", detail: (e as Error).message });
    return;
  }
  recordAuthSuccess(ip);
  const expiresAtMs = Date.now() + SESSION_TTL_S * 1000;
  // Set the actual token in an httpOnly cookie (JS cannot read it).
  res.setHeader("set-cookie", [
    `${SESSION_COOKIE}=${token}; HttpOnly; SameSite=Lax; Path=/; Max-Age=${SESSION_TTL_S}`,
    `${SESSION_TTL_COOKIE}=${expiresAtMs}; SameSite=Lax; Path=/; Max-Age=${SESSION_TTL_S}`,
  ]);
  // ── Event 3: login_ok ──
  const fp = tokenFingerprint(token);
  emitAuditEvent(API_SERVER_URL, ARBX_EDGE_TOKEN, {
    action: "auth.login_ok", actor: fp, ipAddress: ip,
    userAgent: ua, traceId, afterState: { remaining_rate: rate.remaining },
  });
  res.json({ ok: true, expires_at: expiresAtMs });
});

// POST /admin/session/logout — clear httpOnly cookie.
app.post("/admin/session/logout", (req, res) => {
  const ip = (req.headers["x-forwarded-for"] as string | undefined)?.split(",")[0]?.trim()
    ?? req.socket.remoteAddress ?? "unknown";
  const ua = req.header("user-agent");
  const traceId = (req as express.Request & { traceId?: string }).traceId;
  const cookies = parseCookies(req.headers.cookie);
  const sessionToken = cookies[SESSION_COOKIE];
  const actor = sessionToken ? tokenFingerprint(sessionToken) : "anonymous";
  res.setHeader("set-cookie", [
    `${SESSION_COOKIE}=; HttpOnly; SameSite=Lax; Path=/; Max-Age=0`,
    `${SESSION_TTL_COOKIE}=; SameSite=Lax; Path=/; Max-Age=0`,
  ]);
  // ── Event 5: logout ──
  emitAuditEvent(API_SERVER_URL, ARBX_EDGE_TOKEN, {
    action: "auth.logout", actor, ipAddress: ip,
    userAgent: ua, traceId,
  });
  res.json({ ok: true });
});

async function adminProxy(path: string, req: express.Request, res: express.Response, method: string = "POST"): Promise<void> {
  // V-AT-1 token translation: header for CLI/programmatic callers, cookie for
  // the browser flow (where the real token is httpOnly and the header carries
  // only the public sentinel). resolveAdminToken centralises this contract.
  const adminToken = resolveAdminToken({
    headerToken: req.header("x-arbx-admin-token"),
    cookieHeader: req.headers.cookie,
  });
  if (!adminToken) { res.status(401).json({ error: "missing_admin_token" }); return; }
  try {
    const upstream = await fetch(`${API_SERVER_URL}${path}`, {
      method,
      headers: {
        "content-type": "application/json",
        "x-arbx-edge-token": ARBX_EDGE_TOKEN,
        "x-arbx-admin-token": adminToken,
        "x-arbx-trace-id": (req as express.Request & { traceId?: string }).traceId ?? "",
        "x-arbx-actor": req.header("x-arbx-actor") ?? "",
      },
      body: method !== "GET" && method !== "HEAD" ? JSON.stringify(req.body ?? {}) : null,
    });
    const text = await upstream.text();
    res.status(upstream.status)
      .setHeader("content-type", upstream.headers.get("content-type") ?? "application/json")
      .send(text);
  } catch (e) {
    res.status(502).json({ error: "upstream_unreachable", detail: (e as Error).message });
  }
}

app.post("/admin/killswitch",                 (req, res) => adminProxy("/admin/killswitch", req, res, "POST"));

// GET /api/killswitch/status — public read of killswitch state (admin-token gated).
app.get("/api/killswitch/status", (req, res) => {
  adminProxy("/admin/killswitch/status", req, res, "GET");
});

// POST /api/killswitch/:action — activate/deactivate killswitch.
// Maps semantic actions to the api-server's /admin/killswitch toggle.
app.post("/api/killswitch/:action", async (req, res) => {
  const action = req.params["action"];
  if (action !== "activate" && action !== "deactivate") {
    res.status(400).json({ error: "invalid_action", valid_actions: ["activate", "deactivate"] });
    return;
  }
  const body = {
    enabled: action === "activate",
    reason: `operator_${action}`,
    triggered_by: req.header("x-arbx-actor") ?? "operator",
  };
  // Override req.body so adminProxy serialises the mapped payload.
  (req as express.Request & { body?: unknown }).body = body;
  adminProxy("/admin/killswitch", req, res, "POST");
});

app.post("/admin/config/paper-mode",          (req, res) => adminProxy("/admin/config/paper-mode", req, res, "POST"));
app.post("/admin/onboarding/1/complete",      (req, res) => adminProxy("/admin/onboarding/1/complete", req, res, "POST"));
// 2026-05-10 audit follow-up: DEX active toggle from /dex-registry. Mounted
// alongside the admin PUTs so the httpOnly cookie session is honoured.
app.put("/api/v1/dexes/:id/active", (req, res) => {
  const id = String(req.params.id ?? "");
  adminProxy(`/api/v1/dexes/${encodeURIComponent(id)}/active`, req, res, "PUT");
});

// DEX registry CRUD — Add new DEX (with factories per chain) + hard delete.
// Both behind adminProxy so httpOnly session translates to upstream token.
app.post("/api/v1/dexes", (req, res) => {
  adminProxy("/api/v1/dexes", req, res, "POST");
});
app.delete("/api/v1/dexes/:id", (req, res) => {
  const id = String(req.params.id ?? "");
  adminProxy(`/api/v1/dexes/${encodeURIComponent(id)}`, req, res, "DELETE");
});
app.put("/admin/trading-config/:chain_id",    (req, res) => {
  const cid = req.params["chain_id"];
  if (!cid || !/^[0-9]+$/.test(cid)) { res.status(400).json({ error: "invalid_chain_id" }); return; }
  adminProxy(`/admin/trading-config/${cid}`, req, res, "PUT");
});
// Cartridge Filters (Idea 1 Phase-1) — admin upsert via adminProxy (httpOnly session → upstream token).
app.put("/admin/cartridge-filters/:chain_id", (req, res) => {
  const cid = req.params["chain_id"];
  if (!cid || !/^[0-9]+$/.test(cid)) { res.status(400).json({ error: "invalid_chain_id" }); return; }
  adminProxy(`/admin/cartridge-filters/${cid}`, req, res, "PUT");
});
// Cartridge Forge (Idea 2) — admin inject + lifecycle via adminProxy (cookie → upstream token).
// Upstream cartridge-forge accepts x-arbx-admin-token (auth normalized). Cartridges run in
// shadow eval (admin-gated, no capital). Slug format mirrors the api-server validation.
const CART_SLUG = /^[a-z][a-z0-9_]{2,48}$/;
app.post("/admin/cartridges", (req, res) => adminProxy("/api/v1/cartridges", req, res, "POST"));
app.post("/admin/cartridges/:slug/pause", (req, res) => {
  const slug = req.params["slug"];
  if (!slug || !CART_SLUG.test(slug)) { res.status(400).json({ error: "invalid_slug" }); return; }
  adminProxy(`/api/v1/cartridges/${slug}/pause`, req, res, "POST");
});
app.post("/admin/cartridges/:slug/resume", (req, res) => {
  const slug = req.params["slug"];
  if (!slug || !CART_SLUG.test(slug)) { res.status(400).json({ error: "invalid_slug" }); return; }
  adminProxy(`/api/v1/cartridges/${slug}/resume`, req, res, "POST");
});
app.delete("/admin/cartridges/:slug", (req, res) => {
  const slug = req.params["slug"];
  if (!slug || !CART_SLUG.test(slug)) { res.status(400).json({ error: "invalid_slug" }); return; }
  adminProxy(`/api/v1/cartridges/${slug}`, req, res, "DELETE");
});

// Operator credentials — migration 057. List behind adminProxy so the
// httpOnly session reaches the upstream admin gate. Mutations (PUT/POST/DELETE)
// all go through adminProxy too.
app.get("/api/credentials", (req, res) => adminProxy("/api/v1/credentials", req, res, "GET"));
app.post("/admin/credentials/test", (req, res) => adminProxy("/admin/credentials/test", req, res, "POST"));
app.put("/admin/credentials", (req, res) => adminProxy("/admin/credentials", req, res, "PUT"));
app.delete("/admin/credentials/:provider/:scope", (req, res) => {
  const provider = String(req.params.provider ?? "");
  const scope = String(req.params.scope ?? "");
  adminProxy(
    `/admin/credentials/${encodeURIComponent(provider)}/${encodeURIComponent(scope)}`,
    req,
    res,
    "DELETE",
  );
});

// PR-2.b Audit Log endpoint
app.get("/admin/audit", (req, res) => {
  // forward query parameters safely
  const url = new URL(`${API_SERVER_URL}/admin/audit`);
  for (const [k, v] of Object.entries(req.query)) {
    if (typeof v === "string") url.searchParams.set(k, v);
  }
  adminProxy(url.pathname + url.search, req, res, "GET");
});

// ─── QUANTUM FULLSTACK SYMMETRY — SPA fallback to the Next.js frontend ────────
// Everything NOT matched by an explicit /api, /admin, /status, /health, /metrics
// route above and NOT /socket.io is a frontend route or static asset (/,
// /config, /strategies/forge, /_next/static/*, /favicon.ico, fonts, …). Stream
// it to the frontend with http-proxy-middleware: it PIPES the upstream response
// (no full-body buffering, no http->https text rewrite), so JS/CSS/font chunks
// are delivered byte-exact — avoiding the corruption + Mixed-Content + memory
// hazards of a manual fetch()+replace(). Next serves https-correct relative asset
// URLs; x-forwarded-proto/host tell it the public origin for any absolute URL.
//
// Registered LAST so it only catches requests no explicit route handled.
const frontendProxy = createProxyMiddleware({
  target: FRONTEND_URL,
  changeOrigin: true,
  ws: false,
  headers: {
    "x-forwarded-proto": "https",
    "x-forwarded-host": PUBLIC_EDGE_HOST,
    // CRITICAL: force the edge->frontend hop to identity so the frontend never
    // compresses. Otherwise a Content-Encoding: gzip header can end up on an
    // uncompressed body through the proxy path -> browser ERR_CONTENT_DECODING_
    // FAILED on /_next/static chunks (the webpack runtime), which kills the JS
    // bootstrap on every page. The edge then serves uncompressed; Cloudflare
    // re-compresses for the public client. The edge->frontend hop is on the
    // local Docker network, so the uncompressed bytes cost nothing public-facing.
    "accept-encoding": "identity",
  },
  // AUDIT-P0: emit an honest 502 (frontend_unreachable) instead of letting
  // http-proxy-middleware v3 surface an opaque 500. This makes the root cause
  // immediately visible in logs and browser DevTools. The Docker network alias
  // bug was hidden precisely because the 500 carried no diagnostic detail.
  // In v3, error handling uses the plugins API — RequestHandler does not expose
  // .on() and onError was removed from Options. The plugin receives the internal
  // http-proxy server instance and registers the error handler there.
  plugins: [
    (proxyServer) => {
      proxyServer.on("error", (err, req, res) => {
        logger.warn(
          { event: "frontend_proxy_error", path: (req as express.Request).path, err: (err as Error).message },
          "frontend unreachable — returning 502"
        );
        const expressRes = res as unknown as express.Response;
        if (!expressRes.headersSent) {
          expressRes.status(502).json({
            error: "frontend_unreachable",
            detail: (err as Error).message,
          });
        }
      });
    },
  ],
});

// FE-CRIT-01 — content-negotiated /status (registered here so the HTML branch
// can delegate to `frontendProxy`, declared just above). API clients get the
// backend JSON /status verbatim (preserving the exact legacy behaviour); browser
// navigations (Accept: text/html) get the Next.js SPA /status page instead of
// being shadowed by raw JSON.
app.get("/status", (req, res, next) => {
  if (statusWantsJson(req)) {
    void proxy("/status", req, res);
    return;
  }
  return frontendProxy(req, res, next);
});

app.use((req, res, next) => {
  // /api and /socket.io are owned by the explicit routes above (or 404 if an
  // unknown /api path) — never fall through to the frontend. /_next/* MUST fall
  // through to frontendProxy: Next.js static assets (CSS/JS/font chunks) live on
  // the frontend origin, so excluding it here 404s every asset and breaks the
  // styling + JS hydration of the whole dapp.
  if (req.path.startsWith("/api/") || req.path.startsWith("/socket.io")) {
    return next();
  }
  return frontendProxy(req, res, next);
});

const PORT = Number(process.env["EDGE_PORT"] ?? 8787);
const server = app.listen(PORT, "0.0.0.0", () => {
  logger.info({ event: "service.boot", port: PORT, api_server: API_SERVER_URL, frontend: FRONTEND_URL, env: cfg.system.env }, "edge-dev-local listening");
});

// IMPORTANT: Bind the upgrade event to the proxy so WebSockets correctly upgrade.
//
// N3 fix (audit re-run 2026-05-10): admin-token gate on the upgrade handshake.
// Defense-in-depth — api-server's io.use() (audit A1) is the authoritative gate,
// but the dev-local edge now ALSO filters at first hop so the proxy is never
// trust-on-first-hop fragile if this process is ever exposed beyond loopback.
//
// Token sources, in priority order (mirrors backend/api-server/src/websocket.ts):
//   1. X-ArbX-Admin-Token header  (tooling / curl)
//   2. sec-websocket-protocol     (browser fallback — io.connect can't set custom headers)
//   3. ?token= query param        (last-resort browser fallback)
// Constant-time compare via safeTokenEqual.
const ARBX_ADMIN_TOKEN_FOR_WS = process.env["ARBX_ADMIN_TOKEN"] ?? "";

function extractUpgradeToken(req: import("http").IncomingMessage): string {
  // 1. Header
  const headerToken = req.headers["x-arbx-admin-token"];
  if (typeof headerToken === "string" && headerToken.length > 0) return headerToken;
  if (Array.isArray(headerToken) && headerToken.length > 0 && typeof headerToken[0] === "string") {
    return headerToken[0];
  }
  // 2. sec-websocket-protocol — browsers can ONLY pass auth via this subprotocol header.
  const proto = req.headers["sec-websocket-protocol"];
  if (typeof proto === "string" && proto.length > 0) {
    // Browsers send a comma-separated list; first token is conventionally the bearer.
    return proto.split(",")[0]?.trim() ?? "";
  }
  // 3. Query param fallback.
  try {
    const url = new URL(req.url ?? "/", "http://localhost");
    const q = url.searchParams.get("token");
    if (q && q.length > 0) return q;
  } catch {
    /* ignore malformed URL */
  }
  return "";
}

server.on("upgrade", (req, socket, head) => {
  if (ARBX_ADMIN_TOKEN_FOR_WS) {
    const got = extractUpgradeToken(req);
    if (!got || !safeTokenEqual(got, ARBX_ADMIN_TOKEN_FOR_WS)) {
      logger.warn(
        { event: "ws.upgrade.unauthorized", path: req.url, ip: req.socket.remoteAddress },
        "WS upgrade rejected — invalid or missing admin token",
      );
      socket.write("HTTP/1.1 401 Unauthorized\r\n\r\n");
      socket.destroy();
      return;
    }
  } else {
    // Pure dev mode (no ARBX_ADMIN_TOKEN set): explicit acknowledgment that we
    // trust loopback. Log loudly so this is never silently accepted in prod.
    logger.warn(
      { event: "ws.upgrade.unauthenticated", path: req.url },
      "[edge-dev-local] ARBX_ADMIN_TOKEN not set — WS proxy unauthenticated (dev/loopback only)",
    );
  }
  // Cast: server.on('upgrade') hands us a `Duplex`, but http-proxy-middleware
  // declares `Socket`. At runtime the upgrade socket IS a net.Socket — the
  // looser typing on the event signature is the only mismatch.
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  wsProxy.upgrade!(req, socket as any, head);
});

// Cache buster: 1781425985
