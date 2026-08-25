/**
 * Paper Trade History API — READ-ONLY analytics over the durable
 * `paper_trade_runs` table (FASE OMEGA SHADOW drift-analysis surface).
 *
 * The PaperTradeArchiver (paper-trade-archiver.ts) WRITES the table passively
 * (and, since PAPERLEDGER-08, the relays-client Rust paper-mode terminus path
 * writes it too — same gas derivation, same route_hash fingerprint).
 * This module is the missing READ-SIDE.
 * 100% read-only / NO-ACTIVE — never touches capital, signers, or execution.
 *
 * R8 fail-honest: pool absent → 503; empty → ok:true with empty data (never fabricates).
 * Ghost Protocol: capital_exposure_usd = 0.0000000000 (never displayed here).
 *
 *   GET /api/v1/paper/history?limit=50&offset=0
 *       → { ok, source, count, data: [ ...rows ] }
 *       Rows carry `strategy` (alias of strategy_kind — the column the client
 *       type expects), `route_hash`, and `opp_*` context from a LEFT JOIN on
 *       opportunities (pair/tokens/dexes/amount). Rows whose source opportunity
 *       was purged (>30d retention) JOIN to NULL — rendered honestly, never
 *       fabricated.
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
      // NOTE: schema (mig 051) has no `sim_net_profit_usd`/`strategy` columns —
      // those references made this SELECT throw → HTTP 503. `sim_net_profit_usd`
      // is computed (expected − gas) and aliased so the response shape is
      // unchanged; `strategy_kind` is the real column.
      // PAPERLEDGER-08: alias `strategy_kind AS strategy` (the client type reads
      // `strategy`), select `route_hash`, and LEFT JOIN opportunities for route
      // context (`opp_*` columns). The FK may dangle for runs whose opportunity
      // was purged (>30d retention) — those JOIN to NULL and render honestly.
      // FE-0056 (§61) evidence layer: surface the columns the contracts DO
      // carry today — `reason` (mig 091, the failure reason; NULL = accepted),
      // `cartridge_id` (mig 102, the strategy identity), `route_metadata`
      // (mig 099, the FE derives hop count from it — single derivation, §26),
      // and the persisted `roi_pct` (net bps = display unit conversion ×100,
      // §79 — never recomputed here). route_id/detector_id/quote_version/
      // graph_version/config_version are NOT persisted anywhere (nivel-(b))
      // and are declared gaps client-side, never fabricated.
      const rows = await pool.query(
        `SELECT
           r.id,
           r.opportunity_id,
           r.sim_expected_profit_usd,
           r.sim_gas_cost_usd,
           (r.sim_expected_profit_usd - COALESCE(r.sim_gas_cost_usd, 0)) AS sim_net_profit_usd,
           r.strategy_kind AS strategy,
           r.route_hash,
           r.reason       AS failure_reason,
           r.chain_id,
           r.created_at,
           o.pair_symbol   AS opp_pair_symbol,
           o.token_in      AS opp_token_in,
           o.token_out     AS opp_token_out,
           o.dex_a         AS opp_dex_a,
           o.dex_b         AS opp_dex_b,
           o.amount_in_wei AS opp_amount_in_wei,
           o.cartridge_id  AS opp_cartridge_id,
           o.route_metadata AS opp_route_metadata,
           o.roi_pct::float AS opp_roi_pct
         FROM paper_trade_runs r
         LEFT JOIN opportunities o ON o.id = r.opportunity_id
         ORDER BY r.created_at DESC
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
  // A4 fix (R8-04): split into ACCEPTED (reason IS NULL) vs ALL rows.
  // The dashboard was showing avg profit of REJECTED opportunities as if it
  // were P&L — economically meaningless. The `accepted` block is the honest
  // P&L signal; `all` remains for volume context.
  router.get("/api/v1/paper/history/summary", async (req: Request, res: Response) => {
    if (!pool) return unavailable(res, "db_unavailable");
    const hours = clampInt(req.query.hours, 24, 1, 336);
    const since = new Date(Date.now() - hours * 3600 * 1000);
    const cleanFilter = `(reason IS NULL OR reason NOT LIKE '%unscaled_legacy%')`;
    try {
      const totals = await pool.query(
        `SELECT
           count(*)::bigint                                                              AS total,
           count(*) FILTER (WHERE (sim_expected_profit_usd - COALESCE(sim_gas_cost_usd, 0)) > 0)::bigint AS profitable,
           avg(sim_expected_profit_usd)                                                 AS avg_expected_profit_usd,
           avg(sim_expected_profit_usd - COALESCE(sim_gas_cost_usd, 0))                 AS avg_net_profit_usd,
           sum(sim_gas_cost_usd)                                                        AS total_gas_cost_usd,
           count(DISTINCT strategy_kind)::int                                           AS strategies,
           count(DISTINCT chain_id)::int                                                AS chains
         FROM paper_trade_runs
         WHERE created_at >= $1 AND ${cleanFilter}`,
        [since],
      );
      const accepted = await pool.query(
        `SELECT
           count(*)::bigint                                                              AS total,
           count(*) FILTER (WHERE (sim_expected_profit_usd - COALESCE(sim_gas_cost_usd, 0)) > 0)::bigint AS profitable,
           avg(sim_expected_profit_usd)                                                 AS avg_expected_profit_usd,
           avg(sim_expected_profit_usd - COALESCE(sim_gas_cost_usd, 0))                 AS avg_net_profit_usd,
           sum(sim_gas_cost_usd)                                                        AS total_gas_cost_usd,
           percentile_cont(0.25) WITHIN GROUP (ORDER BY (sim_expected_profit_usd - COALESCE(sim_gas_cost_usd, 0))) AS p25_net,
           percentile_cont(0.50) WITHIN GROUP (ORDER BY (sim_expected_profit_usd - COALESCE(sim_gas_cost_usd, 0))) AS median_net,
           percentile_cont(0.75) WITHIN GROUP (ORDER BY (sim_expected_profit_usd - COALESCE(sim_gas_cost_usd, 0))) AS p75_net,
           count(DISTINCT strategy_kind)::int                                           AS strategies
         FROM paper_trade_runs
         WHERE created_at >= $1 AND reason IS NULL AND ${cleanFilter}`,
        [since],
      );
      res.json({
        ok: true,
        source: "postgres",
        window_hours: hours,
        data: {
          totals: totals.rows[0] ?? {
            total: 0, profitable: 0, avg_expected_profit_usd: null,
            avg_net_profit_usd: null, total_gas_cost_usd: null,
            strategies: 0, chains: 0,
          },
          accepted: accepted.rows[0] ?? {
            total: 0, profitable: 0, avg_expected_profit_usd: null,
            avg_net_profit_usd: null, total_gas_cost_usd: null,
            p25_net: null, median_net: null, p75_net: null,
            strategies: 0,
          },
        },
      });
    } catch (e) {
      return failed(res, e);
    }
  });

  return router;
}
