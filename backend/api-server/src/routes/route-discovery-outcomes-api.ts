/**
 * Route-Discovery Outcomes API — READ-ONLY analytics over the durable
 * `route_discovery_outcomes` table (FASE B Gate-C hit-rate series; the shadow
 * emitter's resolved outcomes, with the Paso 9 `reason` column).
 *
 * This is the missing read-side for the passive sink (route-discovery-outcome-sink.ts):
 * the sink WRITES the table; nothing READ it until now. 100% read-only / NO-ACTIVE —
 * never touches arbx:opps:detected, capital, signers, or execution. R8 fail-honest:
 * pool absent -> 503; empty -> ok:true with empty data (never fabricates).
 *
 *   GET /api/v1/route-discovery-outcomes/summary?hours=24
 *       -> { ok, source, window_hours, data: { totals, by_reason, by_chain,
 *            by_cartridge, by_pair } }   // FE-0038 §47 groupings
 *   GET /api/v1/route-discovery-outcomes?limit=100
 *       -> { ok, source, count, data: [ ...rows ] }
 */

import { Router, type Request, type Response } from "express";
import pg from "pg";

export function buildRouteDiscoveryOutcomesRouter(pool: pg.Pool | null): Router {
  const router = Router();

  const unavailable = (res: Response, reason: string) =>
    res.status(503).json({ ok: false, reason, source: "postgres", data: null });
  const failed = (res: Response, e: unknown) =>
    res.status(503).json({ ok: false, reason: "query_failed", source: "postgres", error: (e as Error).message, data: null });

  const clampInt = (v: unknown, def: number, lo: number, hi: number) => {
    const n = parseInt(String(v ?? def), 10);
    return Number.isFinite(n) ? Math.min(Math.max(n, lo), hi) : def;
  };

  // RDO-SUMMARY-HANG (2026-08-31): each aggregation runs inside its own
  // short-lived transaction with a 15s statement timeout, and the five
  // groupings execute in PARALLEL. At diagnosis the table held 26M rows / 12 GB
  // growing ~1.36M rows/h with NO leading ts_ms index: every 8s FE poll fired
  // five sequential full seq-scans, stacked up to 18 concurrent aggregations,
  // hung the edge proxy past nginx's 60s read timeout, and held AccessShare
  // locks that aborted the deploy migration gate (run 435, 092 lock-timeout).
  // With this bound a heavy window either completes within one query budget or
  // fails honest (503 query_failed via `failed()`) — never stacks, never
  // starves the pool, never blocks ALTER TABLE.
  const RDO_STATEMENT_TIMEOUT_MS = 15_000;

  const timedQuery = (sql: string, params: unknown[]) => {
    const run = async (): Promise<pg.QueryResult> => {
      // pool is non-null — the route guards `if (!pool)` before reaching here.
      const client = await (pool as pg.Pool).connect();
      try {
        await client.query("BEGIN");
        await client.query(`SET LOCAL statement_timeout = ${RDO_STATEMENT_TIMEOUT_MS}`);
        const r = await client.query(sql, params);
        await client.query("COMMIT");
        return r;
      } catch (e) {
        await client.query("ROLLBACK").catch(() => {});
        throw e;
      } finally {
        client.release();
      }
    };
    return run();
  };

  // Aggregate hit-rate / reason / coverage over a time window (default 24h, max 14d).
  router.get("/api/v1/route-discovery-outcomes/summary", async (req: Request, res: Response) => {
    if (!pool) return unavailable(res, "db_unavailable");
    const hours = clampInt(req.query.hours, 24, 1, 336);
    const since = Date.now() - hours * 3600 * 1000;
    try {
      const [totals, byReason, byChain, byCartridge, byPair] = await Promise.all([
        timedQuery(
          `SELECT count(*)::bigint AS total,
                  count(*) FILTER (WHERE is_opportunity)::bigint AS opportunities,
                  count(*) FILTER (WHERE had_reserves)::bigint AS with_reserves,
                  count(*) FILTER (WHERE estimated_profit > 0)::bigint AS profit_gt0,
                  count(DISTINCT chain_id)::int AS chains,
                  count(DISTINCT cartridge_id)::int AS cartridges
           FROM route_discovery_outcomes WHERE ts_ms >= $1`,
          [since],
        ),
        timedQuery(
          `SELECT COALESCE(NULLIF(reason, ''), '(null)') AS reason, count(*)::bigint AS n
           FROM route_discovery_outcomes WHERE ts_ms >= $1 GROUP BY 1 ORDER BY 2 DESC LIMIT 25`,
          [since],
        ),
        timedQuery(
          `SELECT chain_id, count(*)::bigint AS n,
                  count(*) FILTER (WHERE is_opportunity)::bigint AS opportunities
           FROM route_discovery_outcomes WHERE ts_ms >= $1 GROUP BY 1 ORDER BY 2 DESC LIMIT 25`,
          [since],
        ),
        // FE-0038 (§47): by-strategy (cartridge_id) and by-pair groupings over the
        // SAME window — the dimensions the sink already persists. hop / detector /
        // DEX are NOT columns of this table; the FE renders them as honest gaps
        // (nivel-(b)) rather than this route inventing joins.
        timedQuery(
          `SELECT COALESCE(NULLIF(cartridge_id, ''), '(null)') AS cartridge_id, count(*)::bigint AS n,
                  count(*) FILTER (WHERE is_opportunity)::bigint AS opportunities
           FROM route_discovery_outcomes WHERE ts_ms >= $1 GROUP BY 1 ORDER BY 2 DESC LIMIT 25`,
          [since],
        ),
        timedQuery(
          `SELECT token_in, token_out, count(*)::bigint AS n,
                  count(*) FILTER (WHERE is_opportunity)::bigint AS opportunities
           FROM route_discovery_outcomes WHERE ts_ms >= $1 GROUP BY 1, 2 ORDER BY 3 DESC LIMIT 25`,
          [since],
        ),
      ]);
      res.json({
        ok: true,
        source: "postgres",
        window_hours: hours,
        data: {
          totals: totals.rows[0],
          by_reason: byReason.rows,
          by_chain: byChain.rows,
          by_cartridge: byCartridge.rows,
          by_pair: byPair.rows,
        },
      });
    } catch (e) {
      failed(res, e);
    }
  });

  // Recent raw outcome rows (newest first).
  router.get("/api/v1/route-discovery-outcomes", async (req: Request, res: Response) => {
    if (!pool) return unavailable(res, "db_unavailable");
    const limit = clampInt(req.query.limit, 100, 1, 500);
    try {
      const r = await pool.query(
        `SELECT stream_id, ts_ms, schema_ver, chain_id, cartridge_id, tx_hash,
                source_event, pool_hint, token_in, token_out, is_opportunity,
                estimated_profit, confidence, urgency, had_reserves, mode, reason, inserted_at
         FROM route_discovery_outcomes ORDER BY id DESC LIMIT $1`,
        [limit],
      );
      res.json({ ok: true, source: "postgres", count: r.rowCount, data: r.rows });
    } catch (e) {
      failed(res, e);
    }
  });

  return router;
}
