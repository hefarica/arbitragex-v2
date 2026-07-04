/**
 * POST /api/v1/opportunities/:id/simulate
 *
 * On-demand simulation bridge: proxies to the sim-ctl /simulate endpoint.
 * Shadows the A8 stub in stubs.ts — this real handler is mounted earlier in
 * index.ts, so Express dispatches it instead of the 501 stub. Public (no admin
 * gate), matching the stub: simulation is read-only and never touches capital.
 *
 * G-SIM-1 PR-B2b: supports `route_source` selector (pg_metadata | searcher_api |
 * simctl_lookup). When `searcher_api` is selected, enriches the request with
 * route metadata from searcher-rs before forwarding to sim-ctl.
 *
 * R8 fail-honest:
 *   - sim-ctl unreachable / timeout → 503 { error: "sim_unavailable" }.
 *   - searcher-rs unreachable when route_source=searcher_api → 503 with clear
 *     error (caller can retry with a different route_source).
 */

import type { Request, Response } from "express";
import { fetchRouteFromSearcher } from "./searcher-route-client.js";

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

const VALID_ROUTE_SOURCES = new Set(["pg_metadata", "searcher_api", "simctl_lookup"]);

export function mountOpportunitySimulate(app: import("express").Express, deps: Deps): void {
  app.post(
    "/api/v1/opportunities/:id/simulate",
    async (req: Request, res: Response): Promise<void> => {
      const id = String(req.params["id"] ?? "");
      if (!ID_RE.test(id)) {
        res.status(400).json({ error: "invalid_opportunity_id" });
        return;
      }

      const reqBody = (req.body ?? {}) as Record<string, unknown>;
      const routeSource = typeof reqBody["route_source"] === "string"
        ? reqBody["route_source"]
        : "simctl_lookup"; // default: let sim-ctl do its own lookup (A3)

      if (!VALID_ROUTE_SOURCES.has(routeSource)) {
        res.status(400).json({
          error: "invalid_route_source",
          detail: `route_source must be one of: ${[...VALID_ROUTE_SOURCES].join(", ")}`,
        });
        return;
      }

      // A2 enrichment: fetch route metadata from searcher-rs.
      // Typed as Record<string, unknown> so we can attach route_metadata (A2)
      // without TS narrowing the inferred type to just { opportunity_id }.
      let enrichedBody: Record<string, unknown> = { ...reqBody, opportunity_id: id };
      if (routeSource === "searcher_api") {
        const routeResp = await fetchRouteFromSearcher(id);
        if (routeResp === null) {
          res.status(503).json({
            error: "searcher_route_unavailable",
            opportunity_id: id,
            detail: "searcher-rs /route endpoint unreachable or returned 404; try route_source=pg_metadata or simctl_lookup",
          });
          return;
        }
        if (!routeResp.populated) {
          res.status(422).json({
            error: "route_metadata_empty",
            opportunity_id: id,
            detail: "searcher-rs found the opportunity but route_metadata is unpopulated",
          });
          return;
        }
        // Attach the route metadata so sim-ctl can build the OpportunityCandidate.
        enrichedBody = { ...enrichedBody, route_metadata: routeResp.route_metadata };
      }

      const ctrl = new AbortController();
      const timer = setTimeout(() => ctrl.abort(), TIMEOUT_MS);
      try {
        const upstream = await fetch(`${SIM_BASE}/simulate`, {
          method: "POST",
          headers: { "content-type": "application/json", accept: "application/json" },
          body: JSON.stringify(enrichedBody),
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
          route_source: routeSource,
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
