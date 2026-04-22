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

// S7: admin POST proxies — forward caller's x-arbx-admin-token alongside the
// edge token. Rejected by api-server if admin token is missing/wrong.
app.use(express.json({ limit: "64kb" }));

async function adminPost(path: string, req: express.Request, res: express.Response): Promise<void> {
  const adminToken = req.header("x-arbx-admin-token");
  if (!adminToken) { res.status(401).json({ error: "missing_admin_token" }); return; }
  try {
    const upstream = await fetch(`${API_SERVER_URL}${path}`, {
      method: "POST",
      headers: {
        "content-type": "application/json",
        "x-arbx-edge-token": ARBX_EDGE_TOKEN,
        "x-arbx-admin-token": adminToken,
        "x-arbx-trace-id": (req as express.Request & { traceId?: string }).traceId ?? "",
        "x-arbx-actor": req.header("x-arbx-actor") ?? "",
      },
      body: JSON.stringify(req.body ?? {}),
    });
    const text = await upstream.text();
    res.status(upstream.status)
      .setHeader("content-type", upstream.headers.get("content-type") ?? "application/json")
      .send(text);
  } catch (e) {
    res.status(502).json({ error: "upstream_unreachable", detail: (e as Error).message });
  }
}

app.post("/admin/killswitch",                 (req, res) => adminPost("/admin/killswitch", req, res));
app.post("/admin/onboarding/1/complete",      (req, res) => adminPost("/admin/onboarding/1/complete", req, res));

const PORT = Number(process.env["EDGE_PORT"] ?? 8787);
app.listen(PORT, () => {
  logger.info({ event: "service.boot", port: PORT, api_server: API_SERVER_URL, env: cfg.system.env }, "edge-dev-local listening");
});
