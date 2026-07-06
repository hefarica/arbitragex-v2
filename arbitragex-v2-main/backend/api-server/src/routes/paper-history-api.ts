/**
 * Paper Trade History API — READ-ONLY analytics over the durable
 * `paper_trade_runs` table (FASE OMEGA SHADOW drift-analysis surface).
 *
 * The PaperTradeArchiver (paper-trade-archiver.ts) WRITES the table passively.
 * This module is the missing READ-SIDE: nothing read it until now.
 * 100% read-only / NO-ACTIVE — never touches capital, signers, or execution.
 *
 * R8 fail-honest: pool absent → 503; empty → ok:true with empty data (never fabricates).
 * Ghost Protocol: capital_exposure_usd = 0.0000000000 (never displayed here).
 *
 *   GET /api/v1/paper/history?limit=50&offset=0
 *       → { ok, source, count, data: [ ...rows ] }
 *
 *   GET /api/v1/paper/history/summary?hours=24
 *       → { ok, source, window_hours, data: { totals, avg_sim_profit_usd } }
 */
import { Router, type Request, type Response } from "express";
import pg from "pg";

export function buildPaperHistoryRouter(pool: pg.Pool | null): Router {
  const router = Router();

  const unavailable = (res: Response, reason: string) =>
    res.status(503).json({ ok: false, reason, source: "postgres", data: null });

  const failed = (res: Response, e: unknown) =>
    res.status(503).json({
      ok: false,
      reason: "query_failed",
      source: "postgres",
      error: (e as Error).message,
      data: null,
    });

  const clampInt = (v: unknown, def: number, lo: number, hi: number) => {
    const n = parseInt(String(v ?? def), 10);
    return Number.isFinite(n) ? Math.min(Math.max(n, lo), hi) : def;
  };

  // ── GET /api/v1/paper/history — paginated list of paper_trade_runs rows ──────
  router.get("/api/v1/paper/history", async (req: Request, res: Response) => {
    if (!pool) return unavailable(res, "db_unavailable");
    const limit  = clampInt(req.query.limit,  50, 1, 200);
    const offset = clampInt(req.query.offset,  0, 0, 1_000_000);
    try {
      const rows = await pool.query(
        `SELECT
           id,
           opportunity_id,
           route_hash,
           sim_expected_profit_usd,
           sim_gas_cost_usd,
           sim_net_profit_usd,
           strategy,
           chain_id,
           created_at
         FROM paper_trade_runs
         ORDER BY created_at DESC
         LIMIT $1 OFFSET $2`,
        [limit, offset],
      );
      const countRes = await pool.query(
        `SELECT count(*)::bigint AS total FROM paper_trade_runs`,
      );
      res.json({
        ok: true,
        source: "postgres",
        count: Number(countRes.rows[0]?.total ?? 0),
        limit,
        offset,
        data: rows.rows,
      });
    } catch (e) {
      return failed(res, e);
    }
  });

  // ── GET /api/v1/paper/history/summary — aggregate stats over a time window ──
  router.get("/api/v1/paper/history/summary", async (req: Request, res: Response) => {
    if (!pool) return unavailable(res, "db_unavailable");
    const hours = clampInt(req.query.hours, 24, 1, 336);
    const since = new Date(Date.now() - hours * 3600 * 1000);
    try {
      const totals = await pool.query(
        `SELECT
           count(*)::bigint                                          AS total,
           count(*) FILTER (WHERE sim_net_profit_usd > 0)::bigint   AS profitable,
           avg(sim_expected_profit_usd)                             AS avg_expected_profit_usd,
           avg(sim_net_profit_usd)                                  AS avg_net_profit_usd,
           sum(sim_gas_cost_usd)                                    AS total_gas_cost_usd,
           count(DISTINCT strategy)::int                            AS strategies,
           count(DISTINCT chain_id)::int                            AS chains
         FROM paper_trade_runs
         WHERE created_at >= $1`,
        [since],
      );
      res.json({
        ok: true,
        source: "postgres",
        window_hours: hours,
        data: {
          totals: totals.rows[0] ?? {
            total: 0,
            profitable: 0,
            avg_expected_profit_usd: null,
            avg_net_profit_usd: null,
            total_gas_cost_usd: null,
            strategies: 0,
            chains: 0,
          },
        },
      });
    } catch (e) {
      return failed(res, e);
    }
  });

  return router;
}
