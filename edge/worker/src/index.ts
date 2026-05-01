/*
 * ArbitrageX v2 — edge Worker (Cloudflare).
 *
 * Responsibilities:
 *   - Public entry point. ONLY this component is exposed to the Internet.
 *   - Propagates `x-arbx-trace-id`.
 *   - Injects `x-arbx-edge-token` when calling api-server (internal auth).
 *   - Read-only proxy to api-server; never calls hot-path services directly.
 *   - KV-backed cache for `/status` and `/api/opportunities/live` (S1: 2 s TTL).
 *   - Naive in-isolate rate limit (S1 temporary; S7 replaces with KV-backed sliding window).
 *   - Never exposes internal service URLs.
 *
 * Honesty rule: if an upstream 501s, we surface it verbatim. Never synthesize data.
 */

import { Hono } from "hono";
import { deleteCookie, getCookie, setCookie } from "hono/cookie";

type Env = {
  ARBX_ENV: string;
  API_SERVER_URL: string;
  ALLOWED_ORIGINS: string;
  ARBX_EDGE_TOKEN: string;
  JWT_SECRET: string;
  ARBX_CACHE: KVNamespace;
  ARBX_TELEMETRY?: D1Database;
};

const app = new Hono<{ Bindings: Env }>();

// Per-isolate rate limit state (resets on new isolate). Documented temporary.
const RL_WINDOW_MS = 60_000;
const RL_MAX = 120;
const rlState = new Map<string, { count: number; windowStart: number }>();
function rateLimit(key: string, now: number): { ok: boolean; remaining: number } {
  const cur = rlState.get(key);
  if (!cur || now - cur.windowStart > RL_WINDOW_MS) {
    rlState.set(key, { count: 1, windowStart: now });
    return { ok: true, remaining: RL_MAX - 1 };
  }
  cur.count++;
  if (cur.count > RL_MAX) return { ok: false, remaining: 0 };
  return { ok: true, remaining: RL_MAX - cur.count };
}

// Stricter per-path limit for /admin/session (anti brute-force).
// 5 attempts / 60s / IP. In-memory per-isolate (S7 will move to KV).
const ADMIN_SESSION_WINDOW_MS = 60_000;
const ADMIN_SESSION_MAX = 5;
const adminRlState = new Map<string, { count: number; windowStart: number }>();
function rateLimitAdminSession(key: string, now: number): { ok: boolean; remaining: number } {
  const cur = adminRlState.get(key);
  if (!cur || now - cur.windowStart > ADMIN_SESSION_WINDOW_MS) {
    adminRlState.set(key, { count: 1, windowStart: now });
    return { ok: true, remaining: ADMIN_SESSION_MAX - 1 };
  }
  cur.count++;
  if (cur.count > ADMIN_SESSION_MAX) return { ok: false, remaining: 0 };
  return { ok: true, remaining: ADMIN_SESSION_MAX - cur.count };
}

// 401 lockout: 10 consecutive auth failures from same IP → block 15 min.
const LOCKOUT_THRESHOLD = 10;
const LOCKOUT_MS = 15 * 60_000;
const lockoutState = new Map<string, { fails: number; blockedUntil: number }>();
function isLockedOut(key: string, now: number): boolean {
  const s = lockoutState.get(key);
  return !!s && s.blockedUntil > now;
}
function recordAuthFailure(key: string, now: number): void {
  const s = lockoutState.get(key);
  if (!s) {
    lockoutState.set(key, { fails: 1, blockedUntil: 0 });
    return;
  }
  // If a previous lockout has now elapsed, start fresh.
  // (blockedUntil === 0 means "never locked yet" — keep counting.)
  if (s.blockedUntil > 0 && s.blockedUntil < now) {
    lockoutState.set(key, { fails: 1, blockedUntil: 0 });
    return;
  }
  s.fails++;
  if (s.fails >= LOCKOUT_THRESHOLD) s.blockedUntil = now + LOCKOUT_MS;
}
function recordAuthSuccess(key: string): void {
  lockoutState.delete(key);
}

const SESSION_COOKIE = "arbx_admin_session";
const SESSION_TTL_COOKIE = "arbx_admin_session_ttl";
const SESSION_TTL_S = 8 * 60 * 60; // 8 hours

app.use("*", async (c, next) => {
  const origin = c.req.header("origin") ?? "";
  const allowed = c.env.ALLOWED_ORIGINS === "*" ? "*" :
    c.env.ALLOWED_ORIGINS.split(",").map(s => s.trim()).includes(origin) ? origin : "";
  c.header("access-control-allow-origin", allowed);
  c.header("access-control-allow-headers", "content-type,authorization,x-arbx-trace-id");
  c.header("vary", "origin");
  if (c.req.method === "OPTIONS") return c.body(null, 204);

  const traceId = c.req.header("x-arbx-trace-id") ?? crypto.randomUUID();
  c.header("x-arbx-trace-id", traceId);
  (c as unknown as { traceId: string }).traceId = traceId;

  const key = c.req.header("cf-connecting-ip") ?? "anon";
  const now = Date.now();
  const rl = rateLimit(key, now);
  c.header("x-ratelimit-remaining", String(rl.remaining));
  if (!rl.ok) return c.json({ error: "rate_limited" }, 429);

  await next();
});

app.get("/health", (c) => c.json({ ok: true, service: "edge-worker", env: c.env.ARBX_ENV }));

async function proxy(c: import("hono").Context<{ Bindings: Env }>, path: string, cacheKey?: string, ttl = 2) {
  // Forward incoming query string so `?limit=`, `?hours=`, etc. reach api-server.
  // Cache key is query-scoped to prevent cross-variant collisions.
  const incomingQs = new URL(c.req.url).search;
  const upstreamPath = incomingQs ? `${path}${incomingQs}` : path;
  const fullCacheKey = cacheKey ? `${cacheKey}${incomingQs}` : undefined;
  if (fullCacheKey) {
    const cached = await c.env.ARBX_CACHE.get(fullCacheKey);
    if (cached) {
      c.header("x-arbx-cache", "HIT");
      c.header("content-type", "application/json");
      return c.body(cached);
    }
  }
  const upstream = await fetch(`${c.env.API_SERVER_URL}${upstreamPath}`, {
    headers: {
      "x-arbx-edge-token": c.env.ARBX_EDGE_TOKEN,
      "x-arbx-trace-id": (c as unknown as { traceId: string }).traceId,
      "accept": "application/json",
    },
    cf: { cacheTtl: 0, cacheEverything: false },
  });
  const body = await upstream.text();
  if (fullCacheKey && upstream.ok) {
    await c.env.ARBX_CACHE.put(fullCacheKey, body, { expirationTtl: ttl });
  }
  c.header("x-arbx-cache", "MISS");
  c.header("content-type", upstream.headers.get("content-type") ?? "application/json");
  return c.body(body, upstream.status as 200 | 501 | 502);
}

app.get("/status", (c) => proxy(c, "/status", "arbx:cache:status", 2));
app.get("/api/opportunities/live", (c) => proxy(c, "/api/v1/opportunities/live", "arbx:cache:opps", 2));
// Risk alerts view (read-only). No cache in S1; S3 adds.
app.get("/api/risk/alerts", (c) => proxy(c, "/api/v1/risk/alerts"));
// S7: executions feed, recon summary, config view.
app.get("/api/executions/recent", (c) => proxy(c, "/api/v1/executions/recent", "arbx:cache:execs", 5));
app.get("/api/recon/summary",    (c) => proxy(c, "/api/v1/recon/summary",    "arbx:cache:recon", 10));
app.get("/api/recon/timeseries", (c) => proxy(c, "/api/v1/recon/timeseries", "arbx:cache:recon-ts", 15));
app.get("/api/config/current",   (c) => proxy(c, "/api/v1/config/current",   "arbx:cache:config", 30));
app.get("/api/readiness",        (c) => proxy(c, "/api/v1/readiness",        "arbx:cache:readiness", 15));

// V-AT-1 hardening: httpOnly cookie session for the admin token.
// POST /admin/session — validate token, set httpOnly cookie. Rate-limited (5/min/IP)
// and protected by 401 lockout (10 consecutive failures → 15 min block).
app.post("/admin/session", async (c) => {
  const ip = c.req.header("cf-connecting-ip") ?? "anon";
  const now = Date.now();

  if (isLockedOut(ip, now)) {
    return c.json({ error: "locked_out", retry_after_s: LOCKOUT_MS / 1000 }, 429);
  }
  const rate = rateLimitAdminSession(ip, now);
  c.header("x-ratelimit-admin-session-remaining", String(rate.remaining));
  if (!rate.ok) return c.json({ error: "rate_limited" }, 429);

  const body = await c.req
    .json<{ token?: string }>()
    .catch((): { token?: string } => ({}));
  const token = body.token;
  if (!token) return c.json({ error: "token_required" }, 400);

  // Validate by probing api-server with a sentinel killswitch body.
  const probe = await fetch(`${c.env.API_SERVER_URL}/admin/killswitch`, {
    method: "POST",
    headers: {
      "content-type": "application/json",
      "x-arbx-edge-token": c.env.ARBX_EDGE_TOKEN,
      "x-arbx-admin-token": token,
    },
    body: JSON.stringify({ enabled: null, reason: "__session_probe__" }),
  });
  if (probe.status === 401 || probe.status === 403) {
    recordAuthFailure(ip, now);
    return c.json({ error: "invalid_admin_token" }, 401);
  }

  recordAuthSuccess(ip);
  const expiresAtMs = now + SESSION_TTL_S * 1000;
  setCookie(c, SESSION_COOKIE, token, {
    httpOnly: true,
    secure: true,
    sameSite: "Strict",
    path: "/",
    maxAge: SESSION_TTL_S,
  });
  setCookie(c, SESSION_TTL_COOKIE, String(expiresAtMs), {
    secure: true,
    sameSite: "Strict",
    path: "/",
    maxAge: SESSION_TTL_S,
  });
  return c.json({ ok: true, expires_at: expiresAtMs });
});

// POST /admin/session/logout — clear httpOnly cookie.
app.post("/admin/session/logout", (c) => {
  deleteCookie(c, SESSION_COOKIE, { path: "/", secure: true, sameSite: "Strict" });
  deleteCookie(c, SESSION_TTL_COOKIE, { path: "/", secure: true, sameSite: "Strict" });
  return c.json({ ok: true });
});

// S7: admin kill-switch POST. Forwards caller's x-arbx-admin-token in addition
// to the edge token. Rejected by api-server if the admin token is missing/wrong.
// Accepts admin token from header (CLI) or httpOnly cookie (browser, V-AT-1).
app.post("/admin/killswitch", async (c) => {
  const adminToken = c.req.header("x-arbx-admin-token") ?? getCookie(c, SESSION_COOKIE);
  if (!adminToken) return c.json({ error: "missing_admin_token" }, 401);
  const body = await c.req.text();
  const upstream = await fetch(`${c.env.API_SERVER_URL}/admin/killswitch`, {
    method: "POST",
    headers: {
      "content-type": "application/json",
      "x-arbx-edge-token": c.env.ARBX_EDGE_TOKEN,
      "x-arbx-admin-token": adminToken,
      "x-arbx-trace-id": (c as unknown as { traceId: string }).traceId,
    },
    body,
  });
  const text = await upstream.text();
  c.header("content-type", upstream.headers.get("content-type") ?? "application/json");
  return c.body(text, upstream.status as 200 | 400 | 401 | 403 | 500 | 502);
});

// PR-2.b Audit Log proxy
app.get("/admin/audit", async (c) => {
  const adminToken = c.req.header("x-arbx-admin-token") ?? getCookie(c, SESSION_COOKIE);
  if (!adminToken) return c.json({ error: "missing_admin_token" }, 401);
  
  const incomingQs = new URL(c.req.url).search;
  const upstreamPath = incomingQs ? `/admin/audit${incomingQs}` : `/admin/audit`;
  
  const upstream = await fetch(`${c.env.API_SERVER_URL}${upstreamPath}`, {
    method: "GET",
    headers: {
      "accept": "application/json",
      "x-arbx-edge-token": c.env.ARBX_EDGE_TOKEN,
      "x-arbx-admin-token": adminToken,
      "x-arbx-trace-id": (c as unknown as { traceId: string }).traceId,
    },
  });
  const text = await upstream.text();
  c.header("content-type", upstream.headers.get("content-type") ?? "application/json");
  return c.body(text, upstream.status as any);
});

app.notFound((c) => c.json({ error: "not_found" }, 404));
app.onError((err, c) => {
  console.error(JSON.stringify({ event: "edge.error", err: err.message }));
  return c.json({ error: "internal_error" }, 500);
});

export default app;
