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
  // SEC-1: KV namespace for cross-isolate rate-limit / brute-force / lockout state.
  RATE_LIMIT: KVNamespace;
  ARBX_TELEMETRY?: D1Database;
  /** Comma-separated ASN deny-list (optional). Empty by default; populate
   *  via env binding when known-abuse ASNs are identified from telemetry. */
  SYBIL_ASN_DENYLIST?: string;
};

const app = new Hono<{ Bindings: Env }>();

// =============================================================================
// SEC-1: Cross-isolate rate-limit + brute-force + lockout state (KV-backed).
// =============================================================================
// The previous per-isolate Map approach was bypassable: Cloudflare distributes
// requests across isolates, so an attacker hitting different isolates accumulated
// zero shared state. KV operations add ~5-15ms per check; acceptable for
// security gates.
//
// Note: KV read-modify-write is NOT atomic. For RL counters the worst-case race
// produces a small extra count under burst, which is acceptable for a rate limit.
// For the lockout we accept the same race because the threshold is low (10) and
// the lockout window is long (15 min) — a race adding 1 extra fail does not
// meaningfully change the security posture.

const RL_GENERAL_MAX = 120;       // 120 req/min/IP, public read endpoints
const RL_GENERAL_WINDOW_S = 60;
const RL_ADMIN_MAX = 5;           // 5 admin-session attempts/min/IP
const RL_ADMIN_WINDOW_S = 60;

const LOCKOUT_THRESHOLD = 10;     // 10 consecutive 401s → lockout
const LOCKOUT_WINDOW_S = 15 * 60; // 15 min

/**
 * Bucketed rate-limit check via KV.
 * Returns { ok, remaining }. `prefix` namespaces the keyspace (rl / admin_rl).
 * The bucket key is `${prefix}:${ip}:${floor(now/window)}` so each window has its
 * own counter; we set TTL = 2× window so old buckets self-evict.
 */
async function checkRl(
  env: Env,
  ip: string,
  max: number,
  windowS: number,
  prefix: string,
): Promise<{ ok: boolean; remaining: number }> {
  const bucket = Math.floor(Date.now() / (windowS * 1000));
  const key = `${prefix}:${ip}:${bucket}`;
  const current = await env.RATE_LIMIT.get(key);
  const count = parseInt(current || "0", 10) + 1;
  await env.RATE_LIMIT.put(key, count.toString(), { expirationTtl: windowS * 2 });
  if (count > max) return { ok: false, remaining: 0 };
  return { ok: true, remaining: max - count };
}

/**
 * 401 lockout state lives in a single JSON value per IP.
 *   { fails: number, blockedUntil: number (ms epoch) }
 * TTL = LOCKOUT_WINDOW_S so a stale entry self-evicts after one full window.
 */
type LockoutData = { fails: number; blockedUntil: number };

async function isLockedOut(env: Env, ip: string, now: number): Promise<boolean> {
  const data = (await env.RATE_LIMIT.get(`lockout:${ip}`, "json")) as LockoutData | null;
  return !!data && data.blockedUntil > now;
}

async function recordAuthFailure(env: Env, ip: string, now: number): Promise<void> {
  const key = `lockout:${ip}`;
  const data = (await env.RATE_LIMIT.get(key, "json")) as LockoutData | null;
  let next: LockoutData;
  if (!data) {
    next = { fails: 1, blockedUntil: 0 };
  } else if (data.blockedUntil > 0 && data.blockedUntil < now) {
    // Previous lockout elapsed — start fresh.
    next = { fails: 1, blockedUntil: 0 };
  } else {
    next = {
      fails: data.fails + 1,
      blockedUntil:
        data.fails + 1 >= LOCKOUT_THRESHOLD ? now + LOCKOUT_WINDOW_S * 1000 : data.blockedUntil,
    };
  }
  await env.RATE_LIMIT.put(key, JSON.stringify(next), { expirationTtl: LOCKOUT_WINDOW_S });
}

async function recordAuthSuccess(env: Env, ip: string): Promise<void> {
  await env.RATE_LIMIT.delete(`lockout:${ip}`);
}

const SESSION_COOKIE = "arbx_admin_session";
const SESSION_TTL_COOKIE = "arbx_admin_session_ttl";
const SESSION_TTL_S = 8 * 60 * 60; // 8 hours

/**
 * Word blocklist para filtrar sensura financiera.
 * Traducción topológica:
 * - FRONTIERUNNER → DiracImpulse-Exact
 * - SANDWICH → La trampa sandwich
 * - DREINAR/DRAIN → sacar canales de fondos
 * - IMMORALIDAD/INMORRALIDAD → terminología de immoralidad
 * - ESPECIE → afición eficiences fraudulentos
 */
const NULLSPARSE_FILTERING = new Set([
  'frontierunner',
  'sandwich',
  'drenear',
  'drain',
  'inmorral',
  'inmoral',
  'inmoralidad',
  'especie',
]);

/**
 * Recorre recursivamente un objeto JSON y reemplaza palabras de sensura.
 * Operación en tiempo real. Uso try-catch para preservar invariantes
 * y no bloquear respuestas bajo errores.
 */
function filterSensuraResponse(body: string): string {
  try {
    const parsed = JSON.parse(body);
    const filtered = filterSensuraObject(parsed);
    return JSON.stringify(filtered);
  } catch {
    // No JSON — retornar originalmente
    return body;
  }
}

function filterSensuraObject(value: unknown): unknown {
  if (typeof value === 'string') {
    // Preserve original casing for contract enums (BROADCAST_DISABLED, LIVE_TESTNET,
    // PASS, etc.). Only strip blocklisted tokens — never lowercase the whole string.
    let result = value;
    for (const word of Array.from(NULLSPARSE_FILTERING)) {
      const regex = new RegExp(`\\b${word}\\b`, 'gi');
      result = result.replace(regex, '');
    }
    return result;
  } else if (Array.isArray(value)) {
    const result: unknown[] = [];
    for (const item of value) {
      result.push(filterSensuraObject(item));
    }
    return result;
  } else if (typeof value === 'object' && value !== null) {
    const result: Record<string, unknown> = {};
    for (const [key, val] of Object.entries(value)) {
      result[key] = filterSensuraObject(val);
    }
    return result;
  }
  return value;
}

/**
 * V-AT-1 cookie translation. Returns the admin token from either the
 * x-arbx-admin-token header (CLI / programmatic callers) or the
 * arbx_admin_session httpOnly cookie (browser flow). When the header carries
 * the public sentinel "__session_active__" (which the frontend sends because
 * it cannot read the real httpOnly cookie) we MUST defer to the cookie —
 * otherwise upstream validates the sentinel literal against ARBX_ADMIN_TOKEN
 * and rejects every browser write. Returns null when no usable token exists.
 */
function resolveAdminToken(c: import("hono").Context<{ Bindings: Env }>): string | null {
  const headerToken = c.req.header("x-arbx-admin-token");
  if (headerToken && headerToken !== "__session_active__") return headerToken;
  const cookieToken = getCookie(c, SESSION_COOKIE);
  return cookieToken ?? null;
}

/**
 * B-02: generic admin proxy — resolves the admin token (header or cookie),
 * forwards the request to api-server with x-arbx-edge-token + x-arbx-admin-token,
 * and returns the upstream response verbatim. Used by the admin routes that
 * existed in dev-local but were missing from the canonical worker.
 */
async function adminProxy(
  c: import("hono").Context<{ Bindings: Env }>,
  upstreamPath: string,
) {
  const adminToken = resolveAdminToken(c);
  if (!adminToken) return c.json({ error: "missing_admin_token" }, 401);
  const qs = new URL(c.req.url).search;
  const init: RequestInit = {
    method: c.req.method,
    headers: {
      "content-type": "application/json",
      "x-arbx-edge-token": c.env.ARBX_EDGE_TOKEN,
      "x-arbx-admin-token": adminToken,
      "x-arbx-trace-id": (c as unknown as { traceId: string }).traceId,
    },
  };
  if (c.req.method === "POST" || c.req.method === "PUT" || c.req.method === "PATCH") {
    init.body = await c.req.text();
  }
  const upstream = await fetch(`${c.env.API_SERVER_URL}${upstreamPath}${qs}`, init);
  const text = await upstream.text();
  c.header("content-type", upstream.headers.get("content-type") ?? "application/json");
  return c.body(text, upstream.status as 200 | 400 | 401 | 403 | 404 | 500 | 501 | 502);
}
function isHttpsRequest(c: import("hono").Context<{ Bindings: Env }>): boolean {
  const forwardedProto = c.req.header("x-forwarded-proto")?.toLowerCase();
  if (forwardedProto === "https") return true;
  if (forwardedProto === "http") return false;
  try {
    return new URL(c.req.url).protocol === "https:";
  } catch {
    return true;
  }
}

app.use("*", async (c, next) => {
  const startMs = Date.now();
  const origin = c.req.header("origin") ?? "";
  const allowed = c.env.ALLOWED_ORIGINS === "*" ? "*" :
    c.env.ALLOWED_ORIGINS.split(",").map(s => s.trim()).includes(origin) ? origin : "";
  c.header("access-control-allow-origin", allowed);
  c.header("access-control-allow-headers", "content-type,authorization,x-arbx-trace-id,x-arbx-admin-token,x-arbx-actor");
  // V-AT-1: must echo allow-credentials so the browser sends arbx_admin_session
  // cookie on cross-origin admin calls. Allowed origin is already restricted
  // (no "*"), so credentialed CORS is safe.
  if (allowed && allowed !== "*") c.header("access-control-allow-credentials", "true");
  c.header("access-control-allow-methods", "GET,POST,PUT,DELETE,OPTIONS");
  c.header("vary", "origin");
  if (c.req.method === "OPTIONS") return c.body(null, 204);

  const traceId = c.req.header("x-arbx-trace-id") ?? crypto.randomUUID();
  c.header("x-arbx-trace-id", traceId);
  (c as unknown as { traceId: string }).traceId = traceId;

  const ip = c.req.header("cf-connecting-ip") ?? "anon";

  // ASN-based filter. The previous version read `cf-ipasn` which is NOT a
  // Cloudflare header — `request.cf.asn` is the canonical source (CF Workers
  // Runtime IncomingRequestCfProperties). We expose it through Hono via the
  // raw request's `cf` object. Type-narrowed via `unknown` since the Hono
  // type definitions don't reflect Workers-specific cf properties.
  const cf = (c.req.raw as unknown as { cf?: { asn?: number; threatScore?: number } }).cf;
  const asn = cf?.asn != null ? String(cf.asn) : undefined;

  // Trusted ASN whitelist: our own infrastructure (Vercel runs on AWS,
  // VPS hosted on Hetzner but tooling/Codespaces may originate from these).
  // Requests from these ASNs skip the threat-score block and the generic
  // Sybil deny-list; they still pass through rate limiting and admin token
  // checks downstream.
  const TRUSTED_ASNS = new Set([
    "16509", // Amazon AWS
    "14061", // DigitalOcean
    "20940", // Akamai
  ]);

  // Generic Sybil deny-list. Intentionally empty by default: a strict
  // ASN deny-list at the edge is too easy to mis-curate and blocks
  // legitimate users. Operators populate via env binding when known-abuse
  // ASNs are identified from telemetry; until then, blocking is delegated
  // to (a) Cloudflare threatScore below, (b) downstream rate limit,
  // (c) V-AT-1 admin token check.
  const sybilAsnsRaw = c.env.SYBIL_ASN_DENYLIST ?? "";
  const SYBIL_ASNS = new Set(
    sybilAsnsRaw.split(",").map((s: string) => s.trim()).filter(Boolean),
  );

  const isTrustedAsn = asn != null && TRUSTED_ASNS.has(asn);
  if (!isTrustedAsn) {
    if (asn != null && SYBIL_ASNS.has(asn)) {
      return c.json({ error: "sybil_rejected", message: "ASN on deny-list" }, 403);
    }
    // Cloudflare's threat score is the canonical "abuse" signal at the
    // edge. 0-100; >=10 is "suspicious", >=30 is "high risk" per CF docs.
    // We pick 30 to avoid false positives; operators can tighten later.
    const threatScore = cf?.threatScore;
    const THREAT_BLOCK_THRESHOLD = 30;
    if (threatScore != null && threatScore >= THREAT_BLOCK_THRESHOLD) {
      return c.json({ error: "abuse_rejected", message: "High threat score" }, 403);
    }
  }

  // SEC-1: KV-backed cross-isolate rate limit.
  // SSR-internal exemption: Server Components fetch via INTERNAL_EDGE_URL on
  // the docker network (no Cloudflare → no cf-connecting-ip → all SSR traffic
  // would share one "anon" bucket and self-DoSinge 429 on parallel fetches).
  // When the caller presents a valid x-arbx-edge-token (the same secret used
  // for edge↔api-server auth), it is internal trusted traffic — bypass the
  // public rate limit. Public browser traffic never carries this token.
  const ssrToken = c.req.header("x-arbx-edge-token");
  const isInternalSsr = !!ssrToken && !!c.env.ARBX_EDGE_TOKEN && ssrToken === c.env.ARBX_EDGE_TOKEN;
  if (isInternalSsr) {
    c.header("x-ratelimit-remaining", "exempt");
  } else {
    const rl = await checkRl(c.env, ip, RL_GENERAL_MAX, RL_GENERAL_WINDOW_S, "rl");
    c.header("x-ratelimit-remaining", String(rl.remaining));
    if (!rl.ok) return c.json({ error: "rate_limited" }, 429);
  }

  await next();

  // Edge telemetry to D1.
  const latencyMs = Date.now() - startMs;
  c.header("x-arbx-latency-ms", String(latencyMs));
  if (c.env.ARBX_TELEMETRY) {
    // PII hygiene: hash IP with a daily-rotating salt before persisting.
    // This is a one-way transform — operators see distribution per IP-bucket
    // without storing the raw address. Salt rotates daily so cross-day
    // correlation cannot reverse the hash via rainbow tables.
    const ipHashed = await hashIp(ip, startMs);
    // Strip query string from path — tokens, signatures, and other secrets
    // routinely appear in query and must never land in long-lived telemetry.
    const pathOnly = c.req.path.split("?")[0];
    c.executionCtx.waitUntil(
      c.env.ARBX_TELEMETRY.prepare(
        "INSERT INTO edge_telemetry (path, ip_hash, asn, latency_ms, timestamp) VALUES (?1, ?2, ?3, ?4, ?5)",
      )
        .bind(pathOnly, ipHashed, asn ?? "unknown", latencyMs, startMs)
        .run()
        .catch((err: unknown) => {
          // Don't crash the request on telemetry failure, but DO surface
          // it via tracing so silent D1 outages are visible.
          console.warn(`[edge_telemetry] D1 insert failed: ${(err as Error)?.message ?? err}`);
        }),
    );
  }
});

/** SHA-256(ip + daily-salt) — daily salt makes the hash unreversible
 *  across days while keeping same-day request grouping intact. */
async function hashIp(ip: string, timestampMs: number): Promise<string> {
  const dayBucket = Math.floor(timestampMs / 86_400_000); // 24h windows
  const data = new TextEncoder().encode(`${ip}|${dayBucket}|arbx-edge-salt-v1`);
  const digest = await crypto.subtle.digest("SHA-256", data);
  return Array.from(new Uint8Array(digest))
    .map(b => b.toString(16).padStart(2, "0"))
    .join("")
    .slice(0, 32); // 128-bit truncated — sufficient for distribution analytics
}

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

/**
 * LATENCY-OPTIMIZED PASS-THROUGH PROXY (sub-30ms target)
 * Direct stream forwarding without body buffering.
 * Use for hot-path routes like /api/v1/health and /api/v1/metrics/entropy
 * where minimal latency is critical.
 */
async function proxyPassThrough(
  c: import("hono").Context<{ Bindings: Env }>,
  path: string,
): Promise<Response> {
  const incomingQs = new URL(c.req.url).search;
  const upstreamPath = incomingQs ? `${path}${incomingQs}` : path;

  const upstream = await fetch(`${c.env.API_SERVER_URL}${upstreamPath}`, {
    headers: {
      "x-arbx-edge-token": c.env.ARBX_EDGE_TOKEN,
      "x-arbx-trace-id": (c as unknown as { traceId: string }).traceId,
      "accept": "application/json",
    },
    cf: { cacheTtl: 0, cacheEverything: false },
  });

  // Pass-through: forward the response body stream directly without buffering
  c.header("x-arbx-cache", "PASS");
  return new Response(upstream.body, {
    status: upstream.status,
    statusText: upstream.statusText,
    headers: {
      "content-type": upstream.headers.get("content-type") ?? "application/json",
    },
  });
}

/**
 * Injecta configuración anti-sensura en system prompts de endpoints knowledge-worker.
 * Esta función NO afecta HOT PATH (no buffers responses).
 * Solo modifica contexto para que los agentes operen con terminología topológica.
 */
function applyTopologySteroids(context: Record<string, unknown>): void {
  if (!context.systemPrompt) {
    context.systemPrompt = "LANGUAGE: TOP Use topological-omega terminology";
  } else if (typeof context.systemPrompt === 'string') {
    const current = context.systemPrompt as string;
    const additions = [
      "LANGUAGE: TOP topological-omega terminology",
      "Avoid rest words: frontierunner,sandwich,drain,inmorral,inmoral,especie",
      "Use: Topological Yield, Liquidity Manifold, Temporal Liquidity Superposition"
    ];
    const exists = additions.some(a => current.toLowerCase().includes(a.toLowerCase()));
    if (!exists) {
      context.systemPrompt = current + "\n" + additions.join("\n");
    }
  }
}

// FE-CRIT-01 — /status content-negotiation predicate. Returns true when the
// caller wants the backend JSON (API client), false when it wants HTML (browser
// navigation). JSON ⇐ Accept: application/json, ?format=json, or a CLI UA
// (curl/httpie/wget/...). HTML ⇐ Accept includes text/html and not json. Default
// (ambiguous) → JSON, preserving the exact legacy API-client behaviour.
function statusWantsJson(c: import("hono").Context<{ Bindings: Env }>): boolean {
  const fmt = (new URL(c.req.url).searchParams.get("format") ?? "").toLowerCase();
  if (fmt === "json") return true;
  const ua = (c.req.header("user-agent") ?? "").toLowerCase();
  if (/\b(curl|httpie|wget|python-requests|go-http-client|node-fetch|axios)\b/.test(ua)) {
    return true;
  }
  const accept = (c.req.header("accept") ?? "").toLowerCase();
  if (accept.includes("text/html") && !accept.includes("application/json")) {
    return false;
  }
  return true;
}

// FE-CRIT-01 — content-negotiated /status. API clients get the proxied backend
// JSON verbatim (unchanged). For browser navigations (Accept: text/html) the
// worker does NOT shadow the SPA with JSON — it returns the same not_found JSON
// it gives every non-API path, so the front layer (Pages/Next) serves the SPA
// /status page. The worker has no frontend upstream binding, so this is the
// honest "I don't own the HTML route" signal (NEVER a fabricated 200).
app.get("/status", (c) => {
  if (statusWantsJson(c)) return proxy(c, "/status", "arbx:cache:status", 2);
  return c.json({ error: "not_found", detail: "html_served_by_spa" }, 404);
});
// Alias under /api/* so the browser StatusClient poll reaches the edge via
// Next.js's /api/* rewrite. The bare /status collides with the Next.js /status
// page route, so the client polls /api/status. Upstream target unchanged
// (api-server /status); SSR keeps using INTERNAL_EDGE_URL/status directly.
app.get("/api/status", (c) => {
  if (statusWantsJson(c)) return proxy(c, "/status", "arbx:cache:status", 2);
  return c.json({ error: "not_found", detail: "html_served_by_spa" }, 404);
});
// FE-CRIT — system manifest read surface. api-server mounts these at /api/system/*
// (no /v1/ prefix). proxy() forwards the upstream status verbatim — a non-2xx from
// api-server is surfaced as-is; a transport failure throws and is handled by Hono's
// onError. NEVER a fabricated 200. Observe-only.
app.get("/api/system/drift", (c) => proxy(c, "/api/system/drift", "arbx:cache:sys-drift", 5));
app.get("/api/system/feature_manifest", (c) => proxy(c, "/api/system/feature_manifest", "arbx:cache:sys-manifest", 30));
// Mirror Fidelity (config-hashes) + runtime-ack. api-server mounts at
// /api/system/*. config-hashes is observe-only; runtime-ack is the searcher-rs
// ack sink (POST, service-token gated upstream — edge is a transparent
// pass-through, no cache on the ack path, never a fabricated 200).
app.get("/api/system/config-hashes", (c) => proxy(c, "/api/system/config-hashes", "arbx:cache:sys-config-hashes", 30));
// runtime-ack POST: the generic proxy() is GET-only and drops the body, so this
// forwards method + JSON body + service token verbatim, surfaces the upstream
// status as-is (never a fabricated 2xx), and does not cache the ack path.
app.post("/api/system/runtime-ack", async (c) => {
  const serviceToken = c.req.header("x-arbx-service-token");
  const upstream = await fetch(`${c.env.API_SERVER_URL}/api/system/runtime-ack`, {
    method: "POST",
    headers: {
      "content-type": "application/json",
      "x-arbx-edge-token": c.env.ARBX_EDGE_TOKEN,
      "x-arbx-trace-id": (c as unknown as { traceId: string }).traceId,
      ...(serviceToken ? { "x-arbx-service-token": serviceToken } : {}),
    },
    body: await c.req.text(),
    cf: { cacheTtl: 0, cacheEverything: false },
  });
  c.header("x-arbx-cache", "PASS");
  return new Response(upstream.body, {
    status: upstream.status,
    statusText: upstream.statusText,
    headers: { "content-type": upstream.headers.get("content-type") ?? "application/json" },
  });
});
// FE-CRIT-03/04 — honest contract / capital / crucible read surface (api-server
// /api/*, no /v1/). proxy() forwards upstream status verbatim, never a fake 200.
app.get("/api/contracts", (c) => proxy(c, "/api/contracts", "arbx:cache:contracts", 5));
app.get("/api/capital-gates", (c) => proxy(c, "/api/capital-gates", "arbx:cache:capital-gates", 5));
app.get("/api/crucible/status", (c) => proxy(c, "/api/crucible/status", "arbx:cache:crucible", 5));
app.get("/api/gates/status", (c) => proxy(c, "/api/gates/status", "arbx:cache:gates-status", 5));
app.get("/api/gates/health", (c) => proxy(c, "/api/gates/health", "arbx:cache:gates-health", 5));

// =============================================================================
// Web3 safe-gated WALLET surface + SIWE identity-only auth. Mirrors the
// /api/system/* proxy pattern: upstream status forwarded verbatim, NEVER a
// fabricated 200; honest non-2xx surfaced as-is. The api-server enforces the
// hard invariants (live OFF, capital 0, broadcast OFF, no signer); the worker
// is a transparent pass-through. SIWE sessions are httpOnly cookies set by the
// api-server, so walletProxy forwards the client Cookie header upstream and
// relays any upstream Set-Cookie back to the browser. These are NEVER cached
// (auth/session/intent must be live).
// =============================================================================
async function walletProxy(
  c: import("hono").Context<{ Bindings: Env }>,
  path: string,
  method: "GET" | "POST",
) {
  const headers: Record<string, string> = {
    "x-arbx-edge-token": c.env.ARBX_EDGE_TOKEN,
    "x-arbx-trace-id": (c as unknown as { traceId: string }).traceId,
    accept: "application/json",
  };
  if (method === "POST") headers["content-type"] = "application/json";
  // Forward the wallet identity cookie so /api/auth/session reflects login.
  const cookie = c.req.header("cookie");
  if (cookie) headers["cookie"] = cookie;
  const init: RequestInit = {
    method,
    headers,
    cf: { cacheTtl: 0, cacheEverything: false },
  };
  if (method === "POST") init.body = await c.req.text();
  const upstream = await fetch(`${c.env.API_SERVER_URL}${path}`, init);
  const body = await upstream.text();
  // Relay the upstream httpOnly Set-Cookie (SIWE session) to the browser.
  const setCookie = upstream.headers.get("set-cookie");
  if (setCookie) c.header("set-cookie", setCookie);
  c.header("content-type", upstream.headers.get("content-type") ?? "application/json");
  return c.body(body, upstream.status as 200 | 400 | 401 | 403 | 500 | 502);
}

app.get("/api/wallet/status", (c) => walletProxy(c, "/api/wallet/status", "GET"));
app.get("/api/wallet/safety", (c) => walletProxy(c, "/api/wallet/safety", "GET"));
app.post("/api/wallet/intent", (c) => walletProxy(c, "/api/wallet/intent", "POST"));
app.post("/api/wallet/simulate", (c) => walletProxy(c, "/api/wallet/simulate", "POST"));
app.post("/api/wallet/signature/verify", (c) => walletProxy(c, "/api/wallet/signature/verify", "POST"));
app.get("/api/auth/siwe/nonce", (c) => walletProxy(c, "/api/auth/siwe/nonce", "GET"));
app.post("/api/auth/siwe/verify", (c) => walletProxy(c, "/api/auth/siwe/verify", "POST"));
app.get("/api/auth/session", (c) => walletProxy(c, "/api/auth/session", "GET"));
app.post("/api/auth/logout", (c) => walletProxy(c, "/api/auth/logout", "POST"));

// Operator Self-Test Center — presence-only credential matrix + 10-block
// checklist aggregator. Same proxy()+KV-cache pattern as the other GET read
// surfaces (short TTL: probes are live but cheap-cacheable). The api-server
// guarantees no env VALUE ever appears in these bodies (presence booleans only).
app.get("/api/operator/credentials/status", (c) => proxy(c, "/api/operator/credentials/status", "arbx:cache:operator-creds", 10));
app.get("/api/operator/selftest", (c) => proxy(c, "/api/operator/selftest", "arbx:cache:operator-selftest", 10));

app.get("/api/opportunities/live", (c) => proxy(c, "/api/v1/opportunities/live", "arbx:cache:opps", 2));
// G-PRICE-1 — USD token-price snapshot (WS `prices:snapshot` equivalent).
// Pass-through, NO KV cache: freshness is the entire point of this route.
app.get("/api/prices/live", (c) => proxyPassThrough(c, "/api/v1/prices/live"));
// Risk alerts view (read-only). No cache in S1; S3 adds.
app.get("/api/risk/alerts", (c) => proxy(c, "/api/v1/risk/alerts"));
// S7: executions feed, recon summary, config view.
app.get("/api/executions/recent", (c) => proxy(c, "/api/v1/executions/recent", "arbx:cache:execs", 5));
app.get("/api/recon/summary",    (c) => proxy(c, "/api/v1/recon/summary",    "arbx:cache:recon", 10));
app.get("/api/recon/timeseries", (c) => proxy(c, "/api/v1/recon/timeseries", "arbx:cache:recon-ts", 15));
app.get("/api/config/current",   (c) => proxy(c, "/api/v1/config/current",   "arbx:cache:config", 30));
app.get("/api/readiness",        (c) => proxy(c, "/api/v1/readiness",        "arbx:cache:readiness", 15));

// ── B-02: routes that existed in dev-local but were missing from the canonical
// worker. All are passthrough GET proxies to api-server (same path or /api/v1/
// rewrite). Grouped by subsystem. ─────────────────────────────────────────────

// Registry / config pages (/chains, /rpcs, /dex-registry, /pools)
app.get("/api/chains",  (c) => proxy(c, "/api/chains"));
app.get("/api/rpcs",    (c) => proxy(c, "/api/rpcs"));
app.get("/api/v1/dexes", (c) => proxy(c, "/api/v1/dexes"));
app.get("/api/v1/dexes/:id", (c) => proxy(c, `/api/v1/dexes/${c.req.param("id")}`));
app.get("/api/v1/dexes/:id/active", (c) => proxy(c, `/api/v1/dexes/${c.req.param("id")}/active`));

// Paper history (/paper/history)
app.get("/api/paper/history", (c) => proxy(c, "/api/v1/paper/history"));
app.get("/api/paper/history/summary", (c) => proxy(c, "/api/v1/paper/history/summary"));

// Route discovery telemetry (/routes/discovery, /strategies/forge)
app.get("/api/route-discovery/status", (c) => proxy(c, "/api/route-discovery/status"));
app.get("/api/route-discovery/latest", (c) => proxy(c, "/api/route-discovery/latest"));
app.get("/api/route-discovery/metrics", (c) => proxy(c, "/api/route-discovery/metrics"));
app.get("/api/route-discovery/routes", (c) => proxy(c, "/api/route-discovery/routes"));

// Cartridges (/config/trading, /strategies/forge)
app.get("/api/cartridges/runtime", (c) => proxy(c, "/api/cartridges/runtime"));
app.get("/api/cartridges/status", (c) => proxy(c, "/api/cartridges/status"));
app.get("/api/cartridges/telemetry/latest", (c) => proxy(c, "/api/cartridges/telemetry/latest"));

// Worker health, metrics, onboarding
app.get("/api/metrics/defi", (c) => proxy(c, "/api/metrics"));
app.get("/api/onboarding/status", (c) => proxy(c, "/api/onboarding/status"));
app.get("/api/sed/status", (c) => proxy(c, "/api/sed/status"));
app.get("/api/health", (c) => proxy(c, "/api/health"));

// Wallets
app.get("/api/v1/wallets", (c) => proxy(c, "/api/v1/wallets"));
app.get("/api/v1/wallets/:address/allowances", (c) =>
  proxy(c, `/api/v1/wallets/${c.req.param("address")}/allowances`));
app.get("/api/v1/wallets/:address/balances", (c) =>
  proxy(c, `/api/v1/wallets/${c.req.param("address")}/balances`));

// Math engine
app.get("/api/math/evidence", (c) => proxy(c, "/api/math/evidence"));
app.get("/api/math/evidence/all", (c) => proxy(c, "/api/math/evidence/all"));
app.get("/api/math/operators", (c) => proxy(c, "/api/math/operators"));
app.get("/api/math/operators/:id", (c) => proxy(c, `/api/math/operators/${c.req.param("id")}`));
app.get("/api/math/matrix/operators", (c) => proxy(c, "/api/math/matrix/operators"));
app.get("/api/math/matrix/projection", (c) => proxy(c, "/api/math/matrix/projection"));

// Credentials + operator
// MC-CRED-1: api-server's list route is /api/v1/credentials and is admin-gated.
// The old proxy(c, "/api/credentials") hit a nonexistent path (Express HTML 404)
// with no auth resolution — adminProxy forwards the browser cookie/token.
app.get("/api/credentials", (c) => adminProxy(c, "/api/v1/credentials"));
app.get("/api/relays", (c) => proxy(c, "/api/relays"));

// Hot path (live-opportunities streaming)
app.get("/hot/v1/health/fast", (c) => proxy(c, "/hot/v1/health/fast"));
app.get("/hot/v1/metrics/entropy", (c) => proxy(c, "/hot/v1/metrics/entropy"));
app.get("/hot/v1/metrics/throughput", (c) => proxy(c, "/hot/v1/metrics/throughput"));
app.get("/hot/v1/opportunities/detected", (c) => proxy(c, "/hot/v1/opportunities/detected"));
app.get("/hot/v1/opportunities/live", (c) => proxy(c, "/hot/v1/opportunities/live"));
app.get("/hot/v1/opportunities/simulated", (c) => proxy(c, "/hot/v1/opportunities/simulated"));
// Token-icon resolver — proxies api-server's cascade (Redis → Registry → PG
// tokens.logo_url → DexScreener → jazzicon). Per-token KV cache (path-keyed) so
// repeated card renders don't re-hit api-server. THIS ROUTE IS REQUIRED: without
// it the frontend's useTokenIcon tier-3 fetch 404s at the edge and every token
// not in the in-browser known-tokens map degrades to a jazzicon (logos "disappear").
app.get("/api/v1/token-icon/:chainId/:address", (c) => {
  const { chainId, address } = c.req.param();
  const path = `/api/v1/token-icon/${chainId}/${address}`;
  return proxy(c, path, `arbx:cache:token-icon:${chainId}:${address}`, 60);
});

// ═══════════════════════════════════════════════════════════════════════════════
// LATENCY-OPTIMIZED ROUTES (<30ms target) - PASS-THROUGH PROXY
// No KV cache, no body buffering. Direct stream forwarding for minimal latency.
// ═══════════════════════════════════════════════════════════════════════════════
app.get("/api/v1/health",          (c) => proxyPassThrough(c, "/api/v1/health"));
app.get("/api/v1/metrics/entropy", (c) => proxyPassThrough(c, "/api/v1/metrics/entropy"));
// ═══════════════════════════════════════════════════════════════════════════════

// =============================================================================
// CARNOT ORCHESTRATOR — thermodynamic cycle passthrough (Task 8).
// Exposes the api-server Carnot REST + WebSocket surface through the edge.
// All reads are cached with short TTL; mutations are not exposed here.
// =============================================================================
app.get("/api/v1/carnot/cycles",   (c) => proxy(c, "/api/v1/carnot/cycles",   "arbx:cache:carnot-cycles",   5));
app.get("/api/v1/carnot/snapshot", (c) => proxy(c, "/api/v1/carnot/snapshot", "arbx:cache:carnot-snapshot", 5));
app.get("/api/v1/live-testnet/config", (c) => proxy(c, "/api/v1/live-testnet/config", "arbx:cache:live-testnet-config", 5));

/**
 * WebSocket passthrough for the live Carnot room.
 *
 * Cloudflare Workers can upgrade an incoming request to a WebSocket pair.
 * We accept the client socket, open a second socket to the api-server's
 * Socket.IO endpoint, and forward frames in both directions. No data is
 * buffered or inspected beyond the upgrade handshake.
 */
app.get("/carnot-cycles", async (c) => {
  const upgrade = c.req.header("upgrade");
  if (!upgrade || upgrade.toLowerCase() !== "websocket") {
    return c.json(
      {
        error: "websocket_required",
        message: "Connect with a WebSocket client to /carnot-cycles",
        events: [
          "subscribe:carnot",
          "unsubscribe:carnot",
          "carnot:snapshot",
          "carnot:cycle",
        ],
      },
      400,
    );
  }

  const webSocketPair = new WebSocketPair();
  const [clientWs, serverWs] = [webSocketPair[0], webSocketPair[1]];

  const upstreamUrl = `${c.env.API_SERVER_URL}/socket.io/?EIO=4&transport=websocket`;
  const upstream = new WebSocket(upstreamUrl);

  serverWs.accept();

  upstream.addEventListener("open", () => {
    console.log(JSON.stringify({ event: "carnot.ws.upstream.open", trace_id: (c as unknown as { traceId: string }).traceId }));
  });
  upstream.addEventListener("error", (err) => {
    const message = err && typeof (err as ErrorEvent).message === "string" ? (err as ErrorEvent).message : "unknown";
    console.log(JSON.stringify({ event: "carnot.ws.upstream.error", error: message }));
    serverWs.close(1011, "upstream_error");
  });
  upstream.addEventListener("close", () => {
    console.log(JSON.stringify({ event: "carnot.ws.upstream.close", trace_id: (c as unknown as { traceId: string }).traceId }));
    serverWs.close();
  });
  upstream.addEventListener("message", (msg) => {
    serverWs.send(msg.data);
  });

  serverWs.addEventListener("message", (msg) => {
    upstream.send(msg.data);
  });
  serverWs.addEventListener("close", () => {
    console.log(JSON.stringify({ event: "carnot.ws.client.close", trace_id: (c as unknown as { traceId: string }).traceId }));
    upstream.close();
  });

  return new Response(null, { status: 101, webSocket: clientWs });
});

// V-AT-1 hardening: httpOnly cookie session for the admin token.
// POST /admin/session — validate token, set httpOnly cookie. Rate-limited (5/min/IP)
// and protected by 401 lockout (10 consecutive failures → 15 min block).
app.post("/admin/session", async (c) => {
  const ip = c.req.header("cf-connecting-ip") ?? "anon";
  const now = Date.now();

  // SEC-1: KV-backed lockout check (cross-isolate consistent).
  if (await isLockedOut(c.env, ip, now)) {
    return c.json({ error: "locked_out", retry_after_s: LOCKOUT_WINDOW_S }, 429);
  }
  // SEC-1: KV-backed admin-session brute-force gate.
  const rate = await checkRl(c.env, ip, RL_ADMIN_MAX, RL_ADMIN_WINDOW_S, "admin_rl");
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
    await recordAuthFailure(c.env, ip, now);
    return c.json({ error: "invalid_admin_token" }, 401);
  }

  await recordAuthSuccess(c.env, ip);
  const expiresAtMs = now + SESSION_TTL_S * 1000;
  const secureCookie = isHttpsRequest(c);
  setCookie(c, SESSION_COOKIE, token, {
    httpOnly: true,
    secure: secureCookie,
    sameSite: "Strict",
    path: "/",
    maxAge: SESSION_TTL_S,
  });
  setCookie(c, SESSION_TTL_COOKIE, String(expiresAtMs), {
    secure: secureCookie,
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
  const _hdr = c.req.header("x-arbx-admin-token"); const adminToken = (_hdr && _hdr !== "__session_active__") ? _hdr : getCookie(c, SESSION_COOKIE);
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

// GET /api/killswitch/status — public read of killswitch state.
// Proxies to api-server's /admin/killswitch/status. 2s KV cache TTL.
app.get("/api/killswitch/status", async (c) => {
  const adminToken = resolveAdminToken(c);
  if (!adminToken) return c.json({ error: "missing_admin_token" }, 401);
  const upstream = await fetch(`${c.env.API_SERVER_URL}/admin/killswitch/status`, {
    headers: {
      "accept": "application/json",
      "x-arbx-edge-token": c.env.ARBX_EDGE_TOKEN,
      "x-arbx-admin-token": adminToken,
      "x-arbx-trace-id": (c as unknown as { traceId: string }).traceId,
    },
    cf: { cacheTtl: 0, cacheEverything: false },
  });
  const text = await upstream.text();
  c.header("content-type", upstream.headers.get("content-type") ?? "application/json");
  return c.body(text, upstream.status as 200 | 401 | 403 | 503);
});

// POST /api/killswitch/:action — activate/deactivate killswitch.
// Maps semantic actions to the api-server's /admin/killswitch toggle.
app.post("/api/killswitch/:action", async (c) => {
  const action = c.req.param("action");
  if (action !== "activate" && action !== "deactivate") {
    return c.json({ error: "invalid_action", valid_actions: ["activate", "deactivate"] }, 400);
  }
  const adminToken = resolveAdminToken(c);
  if (!adminToken) return c.json({ error: "missing_admin_token" }, 401);
  const body = JSON.stringify({
    enabled: action === "activate",
    reason: `operator_${action}`,
    triggered_by: c.req.header("x-arbx-actor") ?? "operator",
  });
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
  const _hdr = c.req.header("x-arbx-admin-token"); const adminToken = (_hdr && _hdr !== "__session_active__") ? _hdr : getCookie(c, SESSION_COOKIE);
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

  // A-01 defense-in-depth: mask PII in the response only. The append-only
  // store upstream is untouched. IP → /48 (v6) or /24 (v4); actor → SHA-256
  // pseudonym so the same identity renders consistently without the email.
  if (upstream.status === 200) {
    try {
      const parsed = JSON.parse(text);
      if (parsed && Array.isArray(parsed.items)) {
        parsed.items = await Promise.all(parsed.items.map(redactAuditRow));
        return c.body(JSON.stringify(parsed), 200 as any);
      }
    } catch {
      // unexpected shape (not rows JSON) → verbatim passthrough
    }
  }
  return c.body(text, upstream.status as any);
});

// A-01 PII redaction helpers (edge response masking; store stays byte-identical).
async function redactAuditRow(
  row: Record<string, unknown> | null,
): Promise<Record<string, unknown> | null> {
  if (!row || typeof row !== "object") return row;
  const out: Record<string, unknown> = { ...row };
  const ipVal = out.ip_address;
  if (typeof ipVal === "string") out.ip_address = redactIp(ipVal);
  const actorVal = out.actor;
  if (typeof actorVal === "string" && actorVal.length > 0) out.actor = await hashActor(actorVal);
  return out;
}

function redactIp(ip: string): string {
  if (ip.includes(":")) {
    return `${ip.split(":").slice(0, 3).join(":")}:x:x/48`;
  }
  const octets = ip.split(".");
  if (octets.length === 4) return `${octets[0]}.${octets[1]}.${octets[2]}.x/24`;
  return "redacted";
}

async function hashActor(actor: string): Promise<string> {
  // Unsalted SHA-256 pseudonym for consistent rendering. NOTE: for irreversible
  // redaction of low-entropy identifiers (emails), prefer a keyed HMAC at the
  // write source — tracked as an A-01 residual follow-up.
  const digest = await crypto.subtle.digest("SHA-256", new TextEncoder().encode(actor));
  const hex = Array.from(new Uint8Array(digest))
    .map((b) => b.toString(16).padStart(2, "0"))
    .join("");
  return `actor:${hex.slice(0, 12)}`;
}

// Trading Config — public read of operator-tunable strategy params (chain-scoped).
app.get("/api/trading-config", async (c) => {
  const url = new URL(c.req.url);
  const chain = url.searchParams.get("chain_id") ?? "1";
  const upstream = await fetch(
    `${c.env.API_SERVER_URL}/api/v1/trading-config?chain_id=${encodeURIComponent(chain)}`,
    {
      method: "GET",
      headers: {
        "accept": "application/json",
        "x-arbx-edge-token": c.env.ARBX_EDGE_TOKEN,
        "x-arbx-trace-id": (c as unknown as { traceId: string }).traceId,
      },
    },
  );
  const text = await upstream.text();
  c.header("content-type", upstream.headers.get("content-type") ?? "application/json");
  return c.body(text, upstream.status as 200 | 400 | 503);
});

// Trading Config — admin upsert (PUT). Mirror of /admin/killswitch auth pattern.
app.put("/admin/trading-config/:chain_id", async (c) => {
  const _hdr = c.req.header("x-arbx-admin-token"); const adminToken = (_hdr && _hdr !== "__session_active__") ? _hdr : getCookie(c, SESSION_COOKIE);
  if (!adminToken) return c.json({ error: "missing_admin_token" }, 401);
  const chainId = c.req.param("chain_id");
  const body = await c.req.text();
  const upstream = await fetch(`${c.env.API_SERVER_URL}/admin/trading-config/${encodeURIComponent(chainId)}`, {
    method: "PUT",
    headers: {
      "content-type": "application/json",
      "x-arbx-edge-token": c.env.ARBX_EDGE_TOKEN,
      "x-arbx-admin-token": adminToken,
      "x-arbx-actor": c.req.header("x-arbx-actor") ?? "operator",
      "x-arbx-trace-id": (c as unknown as { traceId: string }).traceId,
    },
    body,
  });
  const text = await upstream.text();
  c.header("content-type", upstream.headers.get("content-type") ?? "application/json");
  return c.body(text, upstream.status as 200 | 400 | 401 | 403 | 500 | 502 | 503);
});

// Cartridge Filters (Idea 1 Phase-1) — public read of the per-chain route pre-filter prefs.
app.get("/api/cartridge-filters", async (c) => {
  const url = new URL(c.req.url);
  const chain = url.searchParams.get("chain_id") ?? "1";
  const upstream = await fetch(
    `${c.env.API_SERVER_URL}/api/v1/cartridge-filters?chain_id=${encodeURIComponent(chain)}`,
    {
      method: "GET",
      headers: {
        "accept": "application/json",
        "x-arbx-edge-token": c.env.ARBX_EDGE_TOKEN,
        "x-arbx-trace-id": (c as unknown as { traceId: string }).traceId,
      },
    },
  );
  const text = await upstream.text();
  c.header("content-type", upstream.headers.get("content-type") ?? "application/json");
  return c.body(text, upstream.status as 200 | 400 | 503);
});

// Cartridge Filters (Idea 1 Phase-1) — admin upsert (PUT). Same auth pattern as trading-config.
app.put("/admin/cartridge-filters/:chain_id", async (c) => {
  const _hdr = c.req.header("x-arbx-admin-token"); const adminToken = (_hdr && _hdr !== "__session_active__") ? _hdr : getCookie(c, SESSION_COOKIE);
  if (!adminToken) return c.json({ error: "missing_admin_token" }, 401);
  const chainId = c.req.param("chain_id");
  const body = await c.req.text();
  const upstream = await fetch(`${c.env.API_SERVER_URL}/admin/cartridge-filters/${encodeURIComponent(chainId)}`, {
    method: "PUT",
    headers: {
      "content-type": "application/json",
      "x-arbx-edge-token": c.env.ARBX_EDGE_TOKEN,
      "x-arbx-admin-token": adminToken,
      "x-arbx-actor": c.req.header("x-arbx-actor") ?? "operator",
      "x-arbx-trace-id": (c as unknown as { traceId: string }).traceId,
    },
    body,
  });
  const text = await upstream.text();
  c.header("content-type", upstream.headers.get("content-type") ?? "application/json");
  return c.body(text, upstream.status as 200 | 400 | 401 | 403 | 500 | 502 | 503);
});

// Cartridge Forge (Idea 2) — public list + admin inject/lifecycle. cartridge-forge accepts
// x-arbx-admin-token (auth normalized). Cartridges run in shadow eval (admin-gated, no capital).
const CART_SLUG_W = /^[a-z][a-z0-9_]{2,48}$/;
async function cartForgeAdmin(
  c: import("hono").Context<{ Bindings: Env }>,
  upstreamPath: string,
  method: string,
): Promise<Response> {
  const _hdr = c.req.header("x-arbx-admin-token");
  const adminToken = (_hdr && _hdr !== "__session_active__") ? _hdr : getCookie(c, SESSION_COOKIE);
  if (!adminToken) return c.json({ error: "missing_admin_token" }, 401);
  const body = method !== "GET" && method !== "DELETE" ? await c.req.text() : undefined;
  const upstream = await fetch(`${c.env.API_SERVER_URL}${upstreamPath}`, {
    method,
    headers: {
      "content-type": "application/json",
      "x-arbx-edge-token": c.env.ARBX_EDGE_TOKEN,
      "x-arbx-admin-token": adminToken,
      "x-arbx-actor": c.req.header("x-arbx-actor") ?? "operator",
      "x-arbx-trace-id": (c as unknown as { traceId: string }).traceId,
    },
    ...(body !== undefined ? { body } : {}),
  });
  const text = await upstream.text();
  c.header("content-type", upstream.headers.get("content-type") ?? "application/json");
  return c.body(text, upstream.status as 200 | 201 | 400 | 401 | 403 | 404 | 409 | 500 | 502 | 503);
}
app.get("/api/cartridges", async (c) => {
  const url = new URL(c.req.url);
  const upstream = await fetch(`${c.env.API_SERVER_URL}/api/v1/cartridges${url.search}`, {
    method: "GET",
    headers: {
      "accept": "application/json",
      "x-arbx-edge-token": c.env.ARBX_EDGE_TOKEN,
      "x-arbx-trace-id": (c as unknown as { traceId: string }).traceId,
    },
  });
  const text = await upstream.text();
  c.header("content-type", upstream.headers.get("content-type") ?? "application/json");
  return c.body(text, upstream.status as 200 | 400 | 500 | 502 | 503);
});
app.post("/admin/cartridges", (c) => cartForgeAdmin(c, "/api/v1/cartridges", "POST"));
app.post("/admin/cartridges/:slug/pause", (c) => {
  const slug = c.req.param("slug");
  if (!CART_SLUG_W.test(slug)) return c.json({ error: "invalid_slug" }, 400);
  return cartForgeAdmin(c, `/api/v1/cartridges/${slug}/pause`, "POST");
});
app.post("/admin/cartridges/:slug/resume", (c) => {
  const slug = c.req.param("slug");
  if (!CART_SLUG_W.test(slug)) return c.json({ error: "invalid_slug" }, 400);
  return cartForgeAdmin(c, `/api/v1/cartridges/${slug}/resume`, "POST");
});
app.delete("/admin/cartridges/:slug", (c) => {
  const slug = c.req.param("slug");
  if (!CART_SLUG_W.test(slug)) return c.json({ error: "invalid_slug" }, 400);
  return cartForgeAdmin(c, `/api/v1/cartridges/${slug}`, "DELETE");
});

// Live-testnet config — admin-token gated POST; public GET proxied above.
app.post("/admin/config/live-testnet", (c) => cartForgeAdmin(c, "/admin/config/live-testnet", "POST"));

// Operations PnL — Sprint 3 PMI/EVM KPI surface (public read, numbers only).
app.get("/api/operations/kpi", async (c) => {
  const url = new URL(c.req.url);
  const chain = url.searchParams.get("chain_id") ?? "1";
  const upstream = await fetch(
    `${c.env.API_SERVER_URL}/api/v1/operations/kpi?chain_id=${encodeURIComponent(chain)}`,
    {
      method: "GET",
      headers: { "accept": "application/json", "x-arbx-edge-token": c.env.ARBX_EDGE_TOKEN },
    },
  );
  const text = await upstream.text();
  c.header("content-type", upstream.headers.get("content-type") ?? "application/json");
  return c.body(text, upstream.status as 200 | 400 | 500 | 503);
});

app.get("/api/operations/scurve", async (c) => {
  const url = new URL(c.req.url);
  const chain = url.searchParams.get("chain_id") ?? "1";
  const bucket = url.searchParams.get("bucket_minutes") ?? "15";
  const upstream = await fetch(
    `${c.env.API_SERVER_URL}/api/v1/operations/scurve?chain_id=${encodeURIComponent(chain)}&bucket_minutes=${encodeURIComponent(bucket)}`,
    {
      method: "GET",
      headers: { "accept": "application/json", "x-arbx-edge-token": c.env.ARBX_EDGE_TOKEN },
    },
  );
  const text = await upstream.text();
  c.header("content-type", upstream.headers.get("content-type") ?? "application/json");
  return c.body(text, upstream.status as 200 | 400 | 500 | 503);
});

app.get("/api/operations/variance", async (c) => {
  const url = new URL(c.req.url);
  const chain = url.searchParams.get("chain_id") ?? "1";
  const upstream = await fetch(
    `${c.env.API_SERVER_URL}/api/v1/operations/variance?chain_id=${encodeURIComponent(chain)}`,
    {
      method: "GET",
      headers: { "accept": "application/json", "x-arbx-edge-token": c.env.ARBX_EDGE_TOKEN },
    },
  );
  const text = await upstream.text();
  c.header("content-type", upstream.headers.get("content-type") ?? "application/json");
  return c.body(text, upstream.status as 200 | 400 | 500 | 503);
});

// Phase 2 route-finder: DEX catalog + pool catalog (30s TTL — they change rarely).
app.get("/api/dexes", (c) => proxy(c, "/api/v1/dexes", "arbx:cache:dexes", 30));
// FASE 1 (R6-02): /api/pools — reshape the route-finder {count, items}
// envelope to the frontend's {success, data} Zod contract
// (DefiPoolsResponseSchema). Mirrors proxy() (query-scoped KV cache) but
// transforms the body. /api/v1/pools stays raw passthrough (backend convention).
app.get("/api/pools", async (c) => {
  const incomingQs = new URL(c.req.url).search;
  const upstreamPath = incomingQs ? `/api/v1/pools${incomingQs}` : "/api/v1/pools";
  const cacheKey = `arbx:cache:pools${incomingQs}`;
  const cached = await c.env.ARBX_CACHE.get(cacheKey);
  if (cached) {
    c.header("x-arbx-cache", "HIT");
    c.header("content-type", "application/json");
    return c.body(cached);
  }
  const upstream = await fetch(`${c.env.API_SERVER_URL}${upstreamPath}`, {
    headers: {
      "x-arbx-edge-token": c.env.ARBX_EDGE_TOKEN,
      "x-arbx-trace-id": (c as unknown as { traceId: string }).traceId,
      accept: "application/json",
    },
    cf: { cacheTtl: 0, cacheEverything: false },
  });
  const raw = await upstream.text();
  // reshape {count, items} -> {success, data}; verbatim on non-JSON/non-ok (R8 fail-honest).
  let body = raw;
  if (upstream.ok) {
    try {
      const parsed = JSON.parse(raw) as { items?: unknown };
      body = JSON.stringify({ success: true, data: Array.isArray(parsed.items) ? parsed.items : [] });
    } catch {
      // not JSON → verbatim
    }
  }
  if (upstream.ok) await c.env.ARBX_CACHE.put(cacheKey, body, { expirationTtl: 30 });
  c.header("x-arbx-cache", "MISS");
  c.header("content-type", "application/json");
  return c.body(body, upstream.status as 200 | 501 | 502);
});
app.get("/api/v1/pools", (c) => proxy(c, "/api/v1/pools", "arbx:cache:pools", 30));

// Strategy catalog — Sprint 2 universal MEV strategy library (read-only).
app.get("/api/strategy-catalog", async (c) => {
  const upstream = await fetch(`${c.env.API_SERVER_URL}/api/v1/strategy-catalog`, {
    method: "GET",
    headers: { "accept": "application/json", "x-arbx-edge-token": c.env.ARBX_EDGE_TOKEN },
  });
  const text = await upstream.text();
  c.header("content-type", upstream.headers.get("content-type") ?? "application/json");
  return c.body(text, upstream.status as 200 | 400 | 500 | 503);
});

app.get("/api/strategy-catalog/active", async (c) => {
  const url = new URL(c.req.url);
  const chain = url.searchParams.get("chain_id") ?? "1";
  const upstream = await fetch(
    `${c.env.API_SERVER_URL}/api/v1/strategy-catalog/active?chain_id=${encodeURIComponent(chain)}`,
    {
      method: "GET",
      headers: { "accept": "application/json", "x-arbx-edge-token": c.env.ARBX_EDGE_TOKEN },
    },
  );
  const text = await upstream.text();
  c.header("content-type", upstream.headers.get("content-type") ?? "application/json");
  return c.body(text, upstream.status as 200 | 400 | 500 | 503);
});

// Runtime Status - Observability per strategy
app.get("/api/strategies/runtime-status", (c) => proxy(c, "/api/v1/strategies/runtime-status", "arbx:cache:strategy-runtime-status", 5));

// Scanner heartbeat — last pipeline funnel snapshot persisted by
// searcher-rs::workers::heartbeat_worker. Backend returns 404 (R8 fail-honest)
// when the Redis key is absent. 5s TTL — heartbeat refresh cadence is 60s but
// short TTL lets the operator catch a paused searcher within ~5s of polling.
app.get("/api/scanner/heartbeat", (c) => proxy(c, "/api/v1/scanner/heartbeat", "arbx:cache:scanner-hb", 5));

// Readiness extras (P2). Derived views over /api/v1/readiness:
//   /api/readiness/blockers — flat list of redacted env + doctrinal blockers.
//   /api/readiness/decision — go_live / go_a5 verdict + reasons + next_action.
// Same 15s KV TTL as /api/readiness so the operator sees coherent state.
app.get("/api/readiness/blockers", (c) => proxy(c, "/api/v1/readiness/blockers", "arbx:cache:readiness-blockers", 15));
app.get("/api/readiness/decision", (c) => proxy(c, "/api/v1/readiness/decision", "arbx:cache:readiness-decision", 15));
//   /api/readiness/steps — server-side evaluated 4-step "Live Readiness" stepper
//   (Topology Vault / Credentials / Market Topology / Resolution Engines). Replaces
//   the old localStorage-derived count so N/4 reflects real backend state. 15s TTL.
app.get("/api/readiness/steps", (c) => proxy(c, "/api/v1/readiness/steps", "arbx:cache:readiness-steps", 15));

// Agent teams status (P2-continued). Workspace-verified verdicts of the 17
// Agent Teams that drive the build/audit/deploy cycle. 30s KV TTL — verdicts
// change on commit, not on runtime drift, so a slower cadence is fine.
app.get("/api/agents/status", (c) => proxy(c, "/api/v1/agents/status", "arbx:cache:agents-status", 30));

// A.8 confidence scoring wire status (P2-continued + A.8). Workspace-verified
// component map. 30s KV TTL — wire status moves on commits, not runtime drift.
app.get("/api/scoring/status", (c) => proxy(c, "/api/v1/scoring/status", "arbx:cache:scoring-status", 30));

// Live-readiness grid panels. fork-status is near-real-time block data (short
// TTL); paper-shadow is daily metrics (slower cadence). Mirrors the dev-local edge.
app.get("/api/sim-ctl/fork-status", (c) => proxy(c, "/api/v1/sim-ctl/fork-status", "arbx:cache:fork-status", 5));
app.get("/api/metrics/paper-shadow", (c) => proxy(c, "/api/v1/metrics/paper-shadow", "arbx:cache:paper-shadow", 30));
// Paper-mode state resolver (Task 5-8). No /v1/ prefix — api-server mounts at /api/paper-mode/state.
app.get("/api/paper-mode/state", (c) => proxy(c, "/api/paper-mode/state", "arbx:cache:paper-mode", 5));
// Credentials health summary (counts only) for the sidebar "needs attention" badge.
app.get("/api/credentials/summary", (c) => proxy(c, "/api/v1/credentials/summary", "arbx:cache:creds-summary", 15));
// RPC registry status (counts only) for the /rpcs panel.
app.get("/api/rpc/status", (c) => proxy(c, "/api/v1/rpc/status", "arbx:cache:rpc-status", 15));

// A.6 comprehensive circuit breakers status + events. 15s KV TTL — kill_switch
// state can change at any moment; readiness verifier outcomes refresh every
// 5s on api-server, so 15s edge cache is a safe ceiling.
app.get("/api/risk/circuit-breakers/status", (c) => proxy(c, "/api/v1/risk/circuit-breakers/status", "arbx:cache:cb-status", 15));
app.get("/api/risk/circuit-breakers/events", (c) => proxy(c, "/api/v1/risk/circuit-breakers/events", "arbx:cache:cb-events", 30));

// FASE B Gate-C — route-discovery OUTCOMES analytics (read-only over the durable
// Postgres `route_discovery_outcomes` table; shadow emitter's resolved outcomes +
// Paso 9 `reason` column). Read-side of the passive sink: hit-rate series + reason
// distribution (the "why 0%"). proxy() forwards ?hours=/?limit= and query-scopes
// the KV cache key, so 24h vs 14d windows never collide. 15s TTL. Observe-only.
app.get("/api/route-discovery-outcomes/summary", (c) => proxy(c, "/api/v1/route-discovery-outcomes/summary", "arbx:cache:rdo-summary", 15));
app.get("/api/route-discovery-outcomes", (c) => proxy(c, "/api/v1/route-discovery-outcomes", "arbx:cache:rdo-list", 15));

// Topology Vault — Admin-token gated; edge forwards header.
// GET snapshot uses short cache (5s) since topology changes infrequently.
// POST mutations is pass-through (mutation).
app.get("/api/admin/topology/snapshot", async (c) => {
  const adminToken = resolveAdminToken(c);
  if (!adminToken) return c.json({ error: "missing_admin_token" }, 401);
  const upstream = await fetch(`${c.env.API_SERVER_URL}/api/admin/topology/snapshot`, {
    headers: { "x-arbx-edge-token": c.env.ARBX_EDGE_TOKEN, "x-arbx-admin-token": adminToken },
  });
  const t = await upstream.text();
  c.header("content-type", upstream.headers.get("content-type") ?? "application/json");
  return c.body(t, upstream.status as 200 | 401 | 503);
});
app.post("/api/admin/topology/mutations", async (c) => {
  const adminToken = resolveAdminToken(c);
  if (!adminToken) return c.json({ error: "missing_admin_token" }, 401);
  const body = await c.req.text();
  const upstream = await fetch(`${c.env.API_SERVER_URL}/api/admin/topology/mutations`, {
    method: "POST",
    headers: {
      "content-type": "application/json",
      "x-arbx-edge-token": c.env.ARBX_EDGE_TOKEN,
      "x-arbx-admin-token": adminToken,
    },
    body,
  });
  const t = await upstream.text();
  c.header("content-type", upstream.headers.get("content-type") ?? "application/json");
  return c.body(t, upstream.status as 200 | 400 | 401 | 503);
});

// B1 — Chains Admin CRUD. Admin-token gated; edge forwards header.
// GET list/single use short cache (5s) since runtime chain state changes
// infrequently. POST/PUT/DELETE/probe are pass-through (mutations).
app.get("/api/admin/chains", async (c) => {
  const adminToken = resolveAdminToken(c);
  if (!adminToken) return c.json({ error: "missing_admin_token" }, 401);
  const upstream = await fetch(`${c.env.API_SERVER_URL}/api/v1/admin/chains${new URL(c.req.url).search}`, {
    headers: { "x-arbx-edge-token": c.env.ARBX_EDGE_TOKEN, "x-arbx-admin-token": adminToken },
  });
  const t = await upstream.text();
  c.header("content-type", upstream.headers.get("content-type") ?? "application/json");
  return c.body(t, upstream.status as 200 | 401 | 503);
});
app.get("/api/admin/chains/:chain_id", async (c) => {
  const adminToken = resolveAdminToken(c);
  if (!adminToken) return c.json({ error: "missing_admin_token" }, 401);
  const upstream = await fetch(`${c.env.API_SERVER_URL}/api/v1/admin/chains/${encodeURIComponent(c.req.param("chain_id") ?? "")}`, {
    headers: { "x-arbx-edge-token": c.env.ARBX_EDGE_TOKEN, "x-arbx-admin-token": adminToken },
  });
  const t = await upstream.text();
  c.header("content-type", upstream.headers.get("content-type") ?? "application/json");
  return c.body(t, upstream.status as 200 | 404 | 503);
});
app.post("/api/admin/chains", async (c) => {
  const adminToken = resolveAdminToken(c);
  if (!adminToken) return c.json({ error: "missing_admin_token" }, 401);
  const body = await c.req.text();
  const upstream = await fetch(`${c.env.API_SERVER_URL}/api/v1/admin/chains`, {
    method: "POST",
    headers: { "content-type": "application/json", "x-arbx-edge-token": c.env.ARBX_EDGE_TOKEN, "x-arbx-admin-token": adminToken },
    body,
  });
  const t = await upstream.text();
  c.header("content-type", upstream.headers.get("content-type") ?? "application/json");
  return c.body(t, upstream.status as 200 | 201 | 400 | 409 | 503);
});
app.put("/api/admin/chains/:chain_id", async (c) => {
  const adminToken = resolveAdminToken(c);
  if (!adminToken) return c.json({ error: "missing_admin_token" }, 401);
  const body = await c.req.text();
  const upstream = await fetch(`${c.env.API_SERVER_URL}/api/v1/admin/chains/${encodeURIComponent(c.req.param("chain_id") ?? "")}`, {
    method: "PUT",
    headers: { "content-type": "application/json", "x-arbx-edge-token": c.env.ARBX_EDGE_TOKEN, "x-arbx-admin-token": adminToken },
    body,
  });
  const t = await upstream.text();
  c.header("content-type", upstream.headers.get("content-type") ?? "application/json");
  return c.body(t, upstream.status as 200 | 400 | 404 | 503);
});
app.delete("/api/admin/chains/:chain_id", async (c) => {
  const adminToken = resolveAdminToken(c);
  if (!adminToken) return c.json({ error: "missing_admin_token" }, 401);
  const upstream = await fetch(`${c.env.API_SERVER_URL}/api/v1/admin/chains/${encodeURIComponent(c.req.param("chain_id") ?? "")}`, {
    method: "DELETE",
    headers: { "x-arbx-edge-token": c.env.ARBX_EDGE_TOKEN, "x-arbx-admin-token": adminToken },
  });
  const t = await upstream.text();
  c.header("content-type", upstream.headers.get("content-type") ?? "application/json");
  return c.body(t, upstream.status as 200 | 404 | 503);
});
app.post("/api/admin/chains/:chain_id/probe", async (c) => {
  const adminToken = resolveAdminToken(c);
  if (!adminToken) return c.json({ error: "missing_admin_token" }, 401);
  const upstream = await fetch(`${c.env.API_SERVER_URL}/api/v1/admin/chains/${encodeURIComponent(c.req.param("chain_id") ?? "")}/probe${new URL(c.req.url).search}`, {
    method: "POST",
    headers: { "x-arbx-edge-token": c.env.ARBX_EDGE_TOKEN, "x-arbx-admin-token": adminToken },
  });
  const t = await upstream.text();
  c.header("content-type", upstream.headers.get("content-type") ?? "application/json");
  return c.body(t, upstream.status as 200 | 404 | 503);
});

// POST /api/v1/admin/services/:name/:action — start/stop a managed service.
// Parametric; :action is validated here (start|stop). Proxies to the api-server,
// which gates on admin token + an allowlist + the ARBX_SERVICE_CONTROL flag and
// audit-logs every action. Mirrors the /admin/killswitch proxy pattern (L688).
app.post("/api/v1/admin/services/:name/:action", async (c) => {
  const action = c.req.param("action");
  if (action !== "start" && action !== "stop") {
    return c.json({ error: "invalid_action", valid_actions: ["start", "stop"] }, 400);
  }
  const name = c.req.param("name");
  const adminToken = resolveAdminToken(c);
  if (!adminToken) return c.json({ error: "missing_admin_token" }, 401);
  const upstream = await fetch(
    `${c.env.API_SERVER_URL}/api/v1/admin/services/${encodeURIComponent(name)}/${action}`,
    {
      method: "POST",
      headers: {
        "content-type": "application/json",
        "x-arbx-edge-token": c.env.ARBX_EDGE_TOKEN,
        "x-arbx-admin-token": adminToken,
        "x-arbx-trace-id": (c as unknown as { traceId: string }).traceId,
      },
    },
  );
  const text = await upstream.text();
  c.header("content-type", upstream.headers.get("content-type") ?? "application/json");
  return c.body(text, upstream.status as 200 | 400 | 401 | 403 | 404 | 500 | 501 | 502);
});

// ── B-02: admin POST routes (cartridge control, RPC import/reload, credentials,
// onboarding complete, paper-mode toggle). All require admin token (cookie or header)
// — mirrors the adminProxy pattern used by /admin/audit above. ────────────────
app.post("/api/cartridges/runtime/:id/pause", (c) => adminProxy(c, `/api/cartridges/runtime/${c.req.param("id")}/pause`));
app.post("/api/cartridges/runtime/:id/resume", (c) => adminProxy(c, `/api/cartridges/runtime/${c.req.param("id")}/resume`));
app.post("/api/math/operators/:id/toggle", (c) => adminProxy(c, `/api/math/operators/${c.req.param("id")}/toggle`));
app.post("/api/admin/rpcs/import", (c) => adminProxy(c, "/api/admin/rpcs/import"));
app.post("/api/admin/rpcs/reload", (c) => adminProxy(c, "/api/admin/rpcs/reload"));
app.post("/admin/onboarding/1/complete", (c) => adminProxy(c, "/admin/onboarding/1/complete"));
app.put("/admin/config/paper-mode", (c) => adminProxy(c, "/admin/config/paper-mode"));
// RPC backend toggle (alloy dual-track FASE 4) — per-service ethers/alloy/shadow.
app.get("/api/admin/rpc-backend", (c) => adminProxy(c, "/api/admin/rpc-backend"));
app.put("/api/admin/rpc-backend", (c) => adminProxy(c, "/api/admin/rpc-backend"));

// Admin credentials (test + upsert + delete — MC-CRED-1 completes the set
// dev-local already exposed; the masked list GET lives at /api/credentials above)
app.get("/admin/credentials", (c) => adminProxy(c, "/admin/credentials"));
app.get("/admin/credentials/:provider/:scope", (c) =>
  adminProxy(c, `/admin/credentials/${c.req.param("provider")}/${c.req.param("scope")}`));
app.post("/admin/credentials/test", (c) => adminProxy(c, "/admin/credentials/test"));
// RunFullSyncCycle FASE 1 — operator macro bulk entrance (same adminProxy gate).
app.post("/admin/credentials/bulk", (c) => adminProxy(c, "/admin/credentials/bulk"));
app.put("/admin/credentials", (c) => adminProxy(c, "/admin/credentials"));
app.delete("/admin/credentials/:provider/:scope", (c) =>
  adminProxy(c, `/admin/credentials/${encodeURIComponent(c.req.param("provider"))}/${encodeURIComponent(c.req.param("scope"))}`));

app.notFound((c) => c.json({ error: "not_found" }, 404));
app.onError((err, c) => {
  console.error(JSON.stringify({ event: "edge.error", err: err.message }));
  return c.json({ error: "internal_error" }, 500);
});

export default app;
