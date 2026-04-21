import type { Request, Response, NextFunction, RequestHandler } from "express";
import { httpRequestsTotal, httpRequestDuration, metricsText } from "../metrics/index.js";

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

/** Enforces `X-ArbX-Edge-Token` for upstream calls from edge → api-server. */
export function requireEdgeToken(expected: string): RequestHandler {
  return (req, res, next) => {
    const got = req.header("x-arbx-edge-token");
    if (!got || got !== expected) {
      res.status(401).json({ error: "unauthorized", source: "edge_token" });
      return;
    }
    next();
  };
}

/** Enforces `X-ArbX-Admin-Token` for admin endpoints. */
export function requireAdminToken(expected: string): RequestHandler {
  return (req, res, next) => {
    const got = req.header("x-arbx-admin-token");
    if (!got || got !== expected) {
      res.status(401).json({ error: "unauthorized", source: "admin_token" });
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
