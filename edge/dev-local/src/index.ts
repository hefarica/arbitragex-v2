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

const SERVICE = "edge-dev-local";
const VERSION = "0.1.0";
const cfg = loadAppConfig();
const logger = createLogger({ service: SERVICE, level: cfg.observability.log_level ?? "info" });
initMetrics(SERVICE);

const API_SERVER_URL = process.env["API_SERVER_URL"] ?? "http://api-server:8080";
const ARBX_EDGE_TOKEN = requireEnv("ARBX_EDGE_TOKEN");

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

// DEV-ONLY: permissive CORS so the browser-side frontend (localhost:5173 via
// SSH tunnel) can reach this edge shim. Production uses CF Workers CORS.
app.use((req, res, next) => {
  const origin = req.headers.origin;
  if (origin) {
    res.setHeader("access-control-allow-origin", origin);
    res.setHeader("access-control-allow-credentials", "true");
    res.setHeader("access-control-allow-headers", "content-type, x-arbx-admin-token, x-arbx-trace-id, x-arbx-actor");
    res.setHeader("access-control-allow-methods", "GET, POST, OPTIONS");
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

app.get("/health", healthHandler(SERVICE, VERSION, startedAt));
app.get("/metrics", metricsHandler);

async function proxy(path: string, req: express.Request, res: express.Response) {
  try {
    const upstream = await fetch(`${API_SERVER_URL}${path}`, {
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

app.get("/status", (req, res) => proxy("/status", req, res));
app.get("/api/opportunities/live", (req, res) => proxy("/api/v1/opportunities/live", req, res));
app.get("/api/risk/alerts", (req, res) => proxy("/api/v1/risk/alerts", req, res));
// S7: new operator-console endpoints.
app.get("/api/executions/recent", (req, res) => proxy("/api/v1/executions/recent", req, res));
app.get("/api/recon/summary", (req, res) => proxy("/api/v1/recon/summary", req, res));
app.get("/api/config/current", (req, res) => proxy("/api/v1/config/current", req, res));
// Phase 0.5: relays catalog (public list of enabled) + onboarding status.
app.get("/api/relays", (req, res) => proxy("/api/v1/relays", req, res));
app.get("/api/onboarding/status", (req, res) => proxy("/api/v1/onboarding/status", req, res));
app.get("/api/readiness", (req, res) => proxy("/api/v1/readiness", req, res));
// DeFi data routes (defiRouter is mounted at /api in api-server, no /v1/ prefix).
app.get("/api/chains",  (req, res) => proxy("/api/chains", req, res));
app.get("/api/rpcs",    (req, res) => proxy("/api/rpcs", req, res));
app.get("/api/pools",   (req, res) => proxy("/api/pools", req, res));
app.get("/api/metrics/defi", (req, res) => proxy("/api/metrics", req, res));

// S7: admin POST proxies — forward caller's x-arbx-admin-token alongside the
// edge token. Rejected by api-server if admin token is missing/wrong.
app.use(express.json({ limit: "64kb" }));

// ─── V-AT-1 hardening: httpOnly cookie session for admin token ───
// The admin token (T1 per secrets.policy.md) is stored in an httpOnly cookie
// instead of localStorage, eliminating the XSS attack vector.
const SESSION_COOKIE = "arbx_admin_session";
const SESSION_TTL_COOKIE = "arbx_admin_session_ttl";
const SESSION_TTL_S = 8 * 60 * 60; // 8 hours

function parseCookies(header: string | undefined): Record<string, string> {
  if (!header) return {};
  const out: Record<string, string> = {};
  for (const pair of header.split(";")) {
    const idx = pair.indexOf("=");
    if (idx < 0) continue;
    out[pair.slice(0, idx).trim()] = pair.slice(idx + 1).trim();
  }
  return out;
}

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
    `${SESSION_COOKIE}=${token}; HttpOnly; SameSite=Strict; Path=/; Max-Age=${SESSION_TTL_S}`,
    `${SESSION_TTL_COOKIE}=${expiresAtMs}; SameSite=Strict; Path=/; Max-Age=${SESSION_TTL_S}`,
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
    `${SESSION_COOKIE}=; HttpOnly; SameSite=Strict; Path=/; Max-Age=0`,
    `${SESSION_TTL_COOKIE}=; SameSite=Strict; Path=/; Max-Age=0`,
  ]);
  // ── Event 5: logout ──
  emitAuditEvent(API_SERVER_URL, ARBX_EDGE_TOKEN, {
    action: "auth.logout", actor, ipAddress: ip,
    userAgent: ua, traceId,
  });
  res.json({ ok: true });
});

async function adminProxy(path: string, req: express.Request, res: express.Response, method: string = "POST"): Promise<void> {
  // Accept admin token from: (1) header (CLI/programmatic), (2) httpOnly cookie (browser).
  const cookies = parseCookies(req.headers.cookie);
  const adminToken = req.header("x-arbx-admin-token") || cookies[SESSION_COOKIE];
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
app.post("/admin/config/paper-mode",          (req, res) => adminProxy("/admin/config/paper-mode", req, res, "POST"));
app.post("/admin/onboarding/1/complete",      (req, res) => adminProxy("/admin/onboarding/1/complete", req, res, "POST"));

// PR-2.b Audit Log endpoint
app.get("/admin/audit", (req, res) => {
  // forward query parameters safely
  const url = new URL(`${API_SERVER_URL}/admin/audit`);
  for (const [k, v] of Object.entries(req.query)) {
    if (typeof v === "string") url.searchParams.set(k, v);
  }
  adminProxy(url.pathname + url.search, req, res, "GET");
});

const PORT = Number(process.env["EDGE_PORT"] ?? 8787);
app.listen(PORT, () => {
  logger.info({ event: "service.boot", port: PORT, api_server: API_SERVER_URL, env: cfg.system.env }, "edge-dev-local listening");
});

