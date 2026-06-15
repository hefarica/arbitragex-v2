/**
 * GET /api/sim-ctl/fork-status  (+ /api/v1/sim-ctl/fork-status)
 *
 * Thin proxy to the sim-ctl service's /fork-status, consumed by
 * components/ForkValidationPanel.tsx.
 *
 * R8 fail-honest contract (encoded as the panel reads it):
 *   - sim-ctl reachable AND returns a NUMERIC block_number → 200 + mapped metrics.
 *   - sim-ctl unreachable / non-2xx / no numeric block_number → HTTP 404.
 *     The panel treats 404 as the expected "DEGRADED — paper mode" state and
 *     ignores the 404 body. A 200 with a null/absent block_number would instead
 *     trip the panel's "Invalid response shape" guard, so we NEVER do that and
 *     NEVER fabricate a block number.
 */

import type { Request, Response } from "express";

interface Deps {
  logger: { warn: (obj: object, msg?: string) => void };
}

// sim-ctl base URL — same env convention as operator-selftest / readiness verifiers.
const SIM_BASE =
  process.env["SIM_URL"] ??
  process.env["SIM_CTL_INTERNAL_URL"] ??
  "http://sim-ctl:3003";

const TIMEOUT_MS = 5_000;

function num(v: unknown, fallback: number): number {
  return typeof v === "number" && Number.isFinite(v) ? v : fallback;
}

export function mountForkStatus(app: import("express").Express, deps: Deps): void {
  const handler = async (_req: Request, res: Response): Promise<void> => {
    const generated_at = new Date().toISOString();

    const ctrl = new AbortController();
    const timer = setTimeout(() => ctrl.abort(), TIMEOUT_MS);
    let raw: unknown;
    try {
      const upstream = await fetch(`${SIM_BASE}/fork-status`, {
        headers: { accept: "application/json" },
        signal: ctrl.signal,
      });
      if (!upstream.ok) {
        res.status(404).json({
          error: "sim_ctl_unavailable",
          source: "sim-ctl",
          detail: `sim-ctl /fork-status -> HTTP ${upstream.status}`,
          generated_at,
        });
        return;
      }
      raw = await upstream.json();
    } catch (e) {
      deps.logger.warn({ event: "fork_status.upstream_unreachable", err: (e as Error).message });
      res.status(404).json({
        error: "sim_ctl_unreachable",
        source: "sim-ctl",
        detail: (e as Error).message,
        generated_at,
      });
      return;
    } finally {
      clearTimeout(timer);
    }

    // Accept either the canonical { metrics: {...} } shape or a flat body.
    const body = (raw ?? {}) as Record<string, unknown>;
    const m = (body["metrics"] ?? body) as Record<string, unknown>;
    const blockNumber = m["block_number"];
    if (typeof blockNumber !== "number" || !Number.isFinite(blockNumber)) {
      // No real fork pinned yet — honest DEGRADED via 404, never a fake block.
      res.status(404).json({
        error: "fork_not_ready",
        source: "sim-ctl",
        detail: "sim-ctl responded without a numeric block_number",
        generated_at,
      });
      return;
    }

    const status =
      m["status"] === "HEALTHY" || m["status"] === "DEGRADED" || m["status"] === "UNAVAILABLE"
        ? (m["status"] as "HEALTHY" | "DEGRADED" | "UNAVAILABLE")
        : "HEALTHY";

    res.status(200).json({
      metrics: {
        block_number: blockNumber,
        rpc_latency_ms: num(m["rpc_latency_ms"], 0),
        simulations_today: num(m["simulations_today"], 0),
        fork_age_seconds: num(m["fork_age_seconds"], 0),
        rpc_url_redacted: typeof m["rpc_url_redacted"] === "string" ? m["rpc_url_redacted"] : "sim-ctl:***",
        executor_address: typeof m["executor_address"] === "string" ? m["executor_address"] : null,
        status,
      },
      generated_at,
    });
  };

  app.get("/api/sim-ctl/fork-status", handler);
  app.get("/api/v1/sim-ctl/fork-status", handler);
}
