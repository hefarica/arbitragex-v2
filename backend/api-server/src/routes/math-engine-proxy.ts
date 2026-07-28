/**
 * Math-engine proxy — forwards the 31-operator surface to the math-engine
 * service (axum, :3006). Read-mostly; the only mutation is the operator
 * enable/disable toggle (soft, runtime-only — no capital, no execution).
 *
 * Routes (mounted at /api/math → math-engine /api):
 *   GET  /api/math/operators              list all 31 operators + availability
 *   GET  /api/math/operators/:id          single operator metadata
 *   POST /api/math/operators/:id/toggle   enable / disable an operator
 *   GET  /api/math/matrix/projection      264×31 projection metadata
 *   GET  /api/math/matrix/operators       matrix view of operator outputs
 *
 * R8 fail-honest: math-engine unreachable / timeout → 503 { reason:
 * "math_engine_unavailable" } — never a fabricated operator list.
 */

import { Router, type Request, type Response } from "express";

interface Deps {
  logger: { warn: (obj: object, msg?: string) => void };
  requireAdminToken?: (expected: string) => import("express").RequestHandler;
  adminToken?: string;
}

const MATH_BASE =
  process.env["MATH_ENGINE_URL"] ?? "http://math-engine:3006";
const TIMEOUT_MS = 10_000;

async function forward(
  upstreamPath: string,
  method: "GET" | "POST",
  req: Request,
  res: Response,
  deps: Deps,
): Promise<void> {
  const ctrl = new AbortController();
  const timer = setTimeout(() => ctrl.abort(), TIMEOUT_MS);
  try {
    const headers: Record<string, string> = { accept: "application/json" };
    const init: RequestInit = { method, headers, signal: ctrl.signal };
    if (method === "POST") {
      headers["content-type"] = "application/json";
      init.body = JSON.stringify(req.body ?? {});
    }
    const upstream = await fetch(`${MATH_BASE}${upstreamPath}`, init);
    const text = await upstream.text();
    let parsed: unknown;
    try {
      parsed = text ? JSON.parse(text) : {};
    } catch {
      parsed = { raw: text };
    }
    res.status(upstream.status).json(parsed);
  } catch (e) {
    deps.logger.warn({ event: "math_engine.proxy_failed", err: (e as Error).message, path: upstreamPath });
    res.status(503).json({
      ok: false,
      reason: "math_engine_unavailable",
      detail: (e as Error).message,
    });
  } finally {
    clearTimeout(timer);
  }
}

export function buildMathEngineRouter(deps: Deps): Router {
  const r = Router();

  // Read-only operator surface (public observe).
  r.get("/api/math/operators", (req, res) => void forward("/api/operators", "GET", req, res, deps));
  r.get("/api/math/operators/:id", (req, res) =>
    void forward(`/api/operators/${encodeURIComponent(String(req.params["id"] ?? ""))}`, "GET", req, res, deps),
  );
  r.get("/api/math/matrix/projection", (req, res) => void forward("/api/matrix/projection", "GET", req, res, deps));
  r.get("/api/math/matrix/operators", (req, res) => void forward("/api/matrix/operators", "GET", req, res, deps));

  // Operator toggle — soft enable/disable at the math-engine runtime. Admin-
  // gated when the admin middleware is provided: an unauthenticated caller
  // must not be able to silence a mathematical operator that gates strategy
  // scoring downstream.
  const toggleHandler = (req: Request, res: Response) =>
    void forward(
      `/api/operators/${encodeURIComponent(String(req.params["id"] ?? ""))}/toggle`,
      "POST",
      req,
      res,
      deps,
    );
  if (deps.requireAdminToken && deps.adminToken) {
    r.post("/api/math/operators/:id/toggle", deps.requireAdminToken(deps.adminToken), toggleHandler);
  } else {
    r.post("/api/math/operators/:id/toggle", toggleHandler);
  }

  return r;
}
