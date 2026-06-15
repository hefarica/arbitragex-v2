/**
 * POST /api/v1/opportunities/:id/simulate
 *
 * On-demand simulation bridge: proxies to the sim-ctl /simulate endpoint.
 * Shadows the A8 stub in stubs.ts — this real handler is mounted earlier in
 * index.ts, so Express dispatches it instead of the 501 stub. Public (no admin
 * gate), matching the stub: simulation is read-only and never touches capital.
 *
 * R8 fail-honest:
 *   - sim-ctl unreachable / timeout → 503 { error: "sim_unavailable" }.
 *   - If simulation.provider is still "not_implemented", sim-ctl returns its own
 *     honest non-2xx; we surface that status + body verbatim. A refusal here is
 *     CORRECT behavior (the provider isn't wired), not a bug to paper over.
 */

import type { Request, Response } from "express";

interface Deps {
  logger: { warn: (obj: object, msg?: string) => void };
}

const SIM_BASE =
  process.env["SIM_URL"] ??
  process.env["SIM_CTL_INTERNAL_URL"] ??
  "http://sim-ctl:3003";

const TIMEOUT_MS = 15_000;
// Opportunity ids are uuids or stream ids (e.g. "169...-0"); keep it permissive but bounded.
const ID_RE = /^[\w:.-]{1,128}$/;

export function mountOpportunitySimulate(app: import("express").Express, deps: Deps): void {
  app.post(
    "/api/v1/opportunities/:id/simulate",
    async (req: Request, res: Response): Promise<void> => {
      const id = String(req.params["id"] ?? "");
      if (!ID_RE.test(id)) {
        res.status(400).json({ error: "invalid_opportunity_id" });
        return;
      }

      const ctrl = new AbortController();
      const timer = setTimeout(() => ctrl.abort(), TIMEOUT_MS);
      try {
        const reqBody = (req.body ?? {}) as Record<string, unknown>;
        const upstream = await fetch(`${SIM_BASE}/simulate`, {
          method: "POST",
          headers: { "content-type": "application/json", accept: "application/json" },
          body: JSON.stringify({ opportunity_id: id, ...reqBody }),
          signal: ctrl.signal,
        });

        const text = await upstream.text();
        let parsed: unknown;
        try {
          parsed = text ? JSON.parse(text) : {};
        } catch {
          parsed = { raw: text };
        }

        // Honest passthrough of sim-ctl's status + body.
        res.status(upstream.status).json({
          source: "sim-ctl",
          opportunity_id: id,
          result: parsed,
        });
      } catch (e) {
        deps.logger.warn({ event: "opportunity_simulate.upstream_failed", err: (e as Error).message });
        res.status(503).json({ error: "sim_unavailable", source: "sim-ctl", detail: (e as Error).message });
      } finally {
        clearTimeout(timer);
      }
    },
  );
}
