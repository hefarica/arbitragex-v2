/**
 * Viable-KPIs API — workbook 29_SUPER_DASHBOARD KPI set over REAL opportunities
 * rows (XLS-DASH-01).
 *
 *   GET /api/v1/analytics/viable-kpis?hours=24
 *       -> { ok, source, window_hours, data: { totals, by_hops, by_kind } }
 *
 * Serves the workbook's route-funnel KPIs from the `opportunities` table —
 * the same rows the /opportunities cards render:
 *   - by_hops  : viable opportunities grouped by route hop count. Hops come
 *                from `jsonb_array_length(route_metadata->'pool_addresses')`
 *                (the multi-hop topology the searcher persists per row,
 *                migration 099 / persistence.rs build_route_metadata_from_plan).
 *   - by_kind  : viable opportunities grouped by `strategy_kind` (canonical
 *                cartridge stems, migration 103).
 *   - totals   : viable / total in window and the viability % (null when
 *                total = 0 — R8: "not computed", never a fabricated 0%).
 *
 * VIABLE_STATUSES is imported from opportunities-live.ts (the single TS mirror
 * of the searcher's persistence.rs array — no re-derivation drift).
 *
 * Read-only / observe-only (RULE 00): no fabrication — pool absent → 503;
 * a quiet window returns ok:true with empty groupings.
 */

import { Router, type Request, type Response } from "express";
import pg from "pg";

import { VIABLE_STATUSES } from "./opportunities-live.js";

export function buildViableKpisRouter(pool: pg.Pool | null): Router {
  const router = Router();

  const clampInt = (v: unknown, def: number, lo: number, hi: number) => {
    const n = parseInt(String(v ?? def), 10);
    return Number.isFinite(n) ? Math.min(Math.max(n, lo), hi) : def;
  };

  router.get("/api/v1/analytics/viable-kpis", async (req: Request, res: Response) => {
    if (!pool) {
      res.status(503).json({ ok: false, reason: "db_unavailable", source: "postgres", data: null });
      return;
    }
    const hours = clampInt(req.query.hours, 24, 1, 336);
    try {
      // Totals over the window. `viable` mirrors the cards-visible statuses;
      // `routed` counts viable rows that carry a persisted multi-hop topology
      // (the subset by_hops groups over — surfaced separately so an empty
      // by_hops on a non-empty window reads as "no route_metadata yet",
      // not as a broken query).
      const totals = await pool.query(
        `SELECT count(*) FILTER (WHERE status = ANY($1))::int AS viable,
                count(*) FILTER (WHERE status = ANY($1)
                  AND jsonb_typeof(route_metadata->'pool_addresses') = 'array'
                  AND jsonb_array_length(route_metadata->'pool_addresses') > 0)::int AS routed,
                count(*)::int AS total
         FROM opportunities
         WHERE detected_at >= now() - make_interval(hours => $2)`,
        [[...VIABLE_STATUSES], hours],
      );
      const byHops = await pool.query(
        `SELECT jsonb_array_length(route_metadata->'pool_addresses') AS hops,
                count(*)::int AS n
         FROM opportunities
         WHERE detected_at >= now() - make_interval(hours => $2)
           AND status = ANY($1)
           AND jsonb_typeof(route_metadata->'pool_addresses') = 'array'
           AND jsonb_array_length(route_metadata->'pool_addresses') > 0
         GROUP BY 1 ORDER BY 1`,
        [[...VIABLE_STATUSES], hours],
      );
      const byKind = await pool.query(
        `SELECT COALESCE(NULLIF(strategy_kind, ''), '(unclassified)') AS strategy_kind,
                count(*)::int AS n
         FROM opportunities
         WHERE detected_at >= now() - make_interval(hours => $2)
           AND status = ANY($1)
         GROUP BY 1 ORDER BY 2 DESC LIMIT 12`,
        [[...VIABLE_STATUSES], hours],
      );
      const t = totals.rows[0] ?? { viable: 0, routed: 0, total: 0 };
      // R8: viability % is null when nothing was observed — never 0%.
      const viabilityPct =
        t.total > 0 ? Math.round((t.viable / t.total) * 1000) / 10 : null;
      res.json({
        ok: true,
        source: "postgres",
        window_hours: hours,
        data: {
          totals: {
            viable: t.viable,
            routed: t.routed,
            total: t.total,
            viability_pct: viabilityPct,
          },
          by_hops: byHops.rows,
          by_kind: byKind.rows,
        },
      });
    } catch (e) {
      res.status(503).json({
        ok: false,
        reason: "query_failed",
        source: "postgres",
        error: (e as Error).message,
        data: null,
      });
    }
  });

  return router;
}
