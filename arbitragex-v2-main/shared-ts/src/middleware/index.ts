import type { Request, Response, NextFunction, RequestHandler } from "express";
import { timingSafeEqual } from "node:crypto";
import { httpRequestsTotal, httpRequestDuration, metricsText } from "../metrics/index.js";

/**
 * Constant-time string comparison (resists timing side-channels).
 * Returns false fast if lengths differ to avoid leaking length itself.
 *
 * Exported (audit A1, 2026-05-10) so the WebSocket handshake middleware in
 * `backend/api-server/src/websocket.ts` can reuse it for admin-token gating
 * instead of duplicating the constant-time compare.
 */
export function safeTokenEqual(got: string, expected: string): boolean {
  if (got.length !== expected.length) return false;
  const a = Buffer.from(got, "utf8");
  const b = Buffer.from(expected, "utf8");
  if (a.length !== b.length) return false;
  return timingSafeEqual(a, b);
}

/**
 * Sentinel placeholders that MUST NOT be accepted as real tokens.
 * If a deployment boots with these as ARBX_ADMIN_TOKEN / ARBX_EDGE_TOKEN,
 * the boot validator below rejects them.
 */
const FORBIDDEN_TOKEN_VALUES = new Set([
  "",
  "REPLACE_ME_256_BITS_RANDOM",
  "REPLACE_ME",
  "<RANDOM_BASE64_64_BYTES>",
  "hfrc2026", // legacy backdoor — hard-blocked at boot
  "changeme",
  "default",
  "test",
]);

/**
 * Boot-time validator. Call once at process startup BEFORE binding routes.
 * Throws if any required token is empty, a known placeholder, or under 32 bytes
 * of entropy. Failing fast at boot is intentional — better to crash than to
 * accept a weak credential silently (audit C1, 2026-05-10).
 */
export function assertSecureBootTokens(env: NodeJS.ProcessEnv = process.env): void {
  const required: Array<[string, number]> = [
    ["ARBX_ADMIN_TOKEN", 32],
    ["ARBX_EDGE_TOKEN", 32],
    // OMEGA-8/M3 P0-1: service-token for inter-service POSTs. Same 32-byte
    // floor as admin/edge tokens; rejected if absent, placeholder, or short.
    ["ARBX_SERVICE_TOKEN", 32],
  ];
  for (const [name, minLen] of required) {
    const v = env[name] ?? "";
    if (!v) {
      throw new Error(`SECURE_BOOT: ${name} is empty — refuse to start`);
    }
    if (FORBIDDEN_TOKEN_VALUES.has(v)) {
      throw new Error(`SECURE_BOOT: ${name} is a known placeholder/backdoor value — refuse to start`);
    }
    if (v.length < minLen) {
      throw new Error(`SECURE_BOOT: ${name} is shorter than ${minLen} bytes — refuse to start`);
    }
  }
}

/** Canonical `/health` handler factory. */
export function healthHandler(service: string, version: string, startedAt: Date): RequestHandler {
  return (_req, res) => {
    res.status(200).json({
      ok: true,
      service,
      version,
      uptime_s: Math.floor((Date.now() - startedAt.getTime()) / 1000),
    });
  };
}

/** Canonical `/metrics` handler. */
export const metricsHandler: RequestHandler = async (_req, res) => {
  const { contentType, body } = await metricsText();
  res.setHeader("content-type", contentType);
  res.status(200).send(body);
};

/** Middleware that increments counters + histograms per-request. */
export function metricsMiddleware(service: string): RequestHandler {
  return (req: Request, res: Response, next: NextFunction) => {
    const start = process.hrtime.bigint();
    const path = (req.route?.path ?? req.path) || "unknown";
    res.on("finish", () => {
      const durNs = Number(process.hrtime.bigint() - start);
      const durS = durNs / 1e9;
      httpRequestsTotal.labels(service, req.method, path, String(res.statusCode)).inc();
      httpRequestDuration.labels(service, req.method, path).observe(durS);
    });
    next();
  };
}

/**
 * Enforces `X-ArbX-Edge-Token` for upstream calls from edge → api-server.
 * Uses constant-time comparison (audit C2, 2026-05-10).
 */
export function requireEdgeToken(expected: string): RequestHandler {
  return (req, res, next) => {
    const got = req.header("x-arbx-edge-token");
    if (!got || !safeTokenEqual(got, expected)) {
      res.status(401).json({ error: "unauthorized", source: "edge_token" });
      return;
    }
    next();
  };
}

/**
 * Enforces `X-ArbX-Admin-Token` for admin endpoints.
 * Uses constant-time comparison (audit C2, 2026-05-10).
 *
 * SECURITY (audit C1, 2026-05-10): backdoor tokens `hfrc2026` and
 * `REPLACE_ME_256_BITS_RANDOM` previously accepted here have been REMOVED.
 * Boot validator `assertSecureBootTokens()` rejects deployments that ship
 * those placeholders as `ARBX_ADMIN_TOKEN` so they cannot be re-introduced
 * via .env.
 */
export function requireAdminToken(expected: string): RequestHandler {
  return (req, res, next) => {
    const got = req.header("x-arbx-admin-token");
    if (!got || !safeTokenEqual(got, expected)) {
      res.status(401).json({ error: "unauthorized", source: "admin_token" });
      return;
    }
    next();
  };
}

/**
 * Enforces `X-ArbX-Service-Token` for inter-service POSTs (searcher-rs,
 * recon, relays-client → api-server). Used by POST /api/system/runtime-ack
 * to reject anonymous writes that would otherwise poison the runtime_ack
 * state machine.
 *
 * Distinct from `ARBX_ADMIN_TOKEN` (operator-curated, scope: admin endpoints)
 * and from `ARBX_EDGE_TOKEN` (edge worker → api-server). Service-tokens are
 * scoped to backend daemons; rotating them does not impact operators or the
 * edge worker.
 *
 * Failure mode: 401 with a generic `{ error: "unauthorized", source:
 * "service_token" }` body — NEVER echo the received token, NEVER log its
 * value (caller may add structured log with `redacted: true`).
 *
 * @since OMEGA-8 / M3 — P0-1 close.
 */
export function requireServiceToken(expected: string): RequestHandler {
  return (req, res, next) => {
    const got = req.header("x-arbx-service-token");
    if (!got || !safeTokenEqual(got, expected)) {
      res.status(401).json({ error: "unauthorized", source: "service_token" });
      return;
    }
    next();
  };
}

/** Ensures `x-arbx-trace-id` header exists, generating one if missing. Propagates to response. */
export function traceIdMiddleware(): RequestHandler {
  return (req, res, next) => {
    const existing = req.header("x-arbx-trace-id");
    const id = existing && existing.length > 0 ? existing : crypto.randomUUID();
    (req as Request & { traceId?: string }).traceId = id;
    res.setHeader("x-arbx-trace-id", id);
    next();
  };
}

/**
 * OMEGA-8/M4 Fase 7 — security headers middleware.
 *
 * Hand-rolled subset of helmet's recommended defaults, chosen so the
 * backend api-server and selector-api expose institutional-grade HTTP
 * hardening without pulling helmet's transitive deps. Matches the spec's
 * required header set:
 *
 *   - `X-Content-Type-Options: nosniff`
 *   - `X-Frame-Options: DENY` (frameguard)
 *   - `Referrer-Policy: no-referrer`
 *   - `Cross-Origin-Resource-Policy: same-site`
 *   - `Cross-Origin-Opener-Policy: same-origin`
 *   - `Content-Security-Policy` — restrictive but compatible with Socket.IO
 *     handshake (default-src 'self'; connect-src 'self' ws: wss:).
 *   - `Strict-Transport-Security: max-age=63072000` — opt-in via
 *     `ARBX_ENABLE_HSTS=true`. NEVER set unconditionally because the
 *     api-server runs behind plain HTTP inside the VPS network; only the
 *     edge worker is TLS-terminated. Setting HSTS on plain HTTP would
 *     break browser access.
 *
 * `app.disable("x-powered-by")` must still be called explicitly at the app
 * level — it is an Express setting, not a header.
 */
export function securityHeadersMiddleware(opts?: {
  enableHsts?: boolean;
  cspExtras?: string;
}): RequestHandler {
  const enableHsts =
    opts?.enableHsts ??
    process.env["ARBX_ENABLE_HSTS"] === "true";
  // Default CSP: deny everything except same-origin for fetch + Socket.IO
  // websocket upgrades. `cspExtras` (callers can pass) appends additional
  // directives without re-implementing the policy in each service.
  const cspBase =
    "default-src 'self'; " +
    "script-src 'self'; " +
    "style-src 'self'; " +
    "img-src 'self' data:; " +
    "connect-src 'self' ws: wss:; " +
    "frame-ancestors 'none'; " +
    "base-uri 'self'; " +
    "object-src 'none';";
  const csp = opts?.cspExtras ? `${cspBase} ${opts.cspExtras}` : cspBase;

  return (_req, res, next) => {
    res.setHeader("X-Content-Type-Options", "nosniff");
    res.setHeader("X-Frame-Options", "DENY");
    res.setHeader("Referrer-Policy", "no-referrer");
    res.setHeader("Cross-Origin-Resource-Policy", "same-site");
    res.setHeader("Cross-Origin-Opener-Policy", "same-origin");
    res.setHeader("Content-Security-Policy", csp);
    if (enableHsts) {
      // 2 years, includeSubDomains. Only emit when the caller knows it's
      // behind real TLS.
      res.setHeader(
        "Strict-Transport-Security",
        "max-age=63072000; includeSubDomains",
      );
    }
    next();
  };
}
