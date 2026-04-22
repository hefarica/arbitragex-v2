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

// S7: admin kill-switch POST. Forwards caller's x-arbx-admin-token in addition
// to the edge token. Rejected by api-server if the admin token is missing/wrong.
app.post("/admin/killswitch", async (c) => {
  const adminToken = c.req.header("x-arbx-admin-token");
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

app.notFound((c) => c.json({ error: "not_found" }, 404));
app.onError((err, c) => {
  console.error(JSON.stringify({ event: "edge.error", err: err.message }));
  return c.json({ error: "internal_error" }, 500);
});

export default app;
