/**
 * Sprint 3 Task 3.2 — Operations PnL endpoints.
 *
 * Surface:
 *   GET  /api/v1/operations/kpi?chain_id=N        public read of live PMI KPIs
 *   GET  /api/v1/operations/scurve?chain_id=N     cumulative ops/profit by 15m bucket
 *   GET  /api/v1/operations/variance?chain_id=N   realized vs planned hourly
 *   GET  /admin/operations/{kpi,scurve,variance}  admin mirrors (admin token gate)
 *
 * Doctrine:
 *   - Source of capital + target = `trading_config` table (one row per chain).
 *     If absent → 503 trading_config_not_seeded (operator must seed via /config/trading first).
 *   - Source of realized PnL = `executions.actual_profit_usd` (NOT expected_profit_usd).
 *   - Source of timestamps  = `executions.submitted_at` (no executed_at column).
 *   - chain_id lives on `opportunities`, NOT `executions` → JOIN required.
 */

import { Router, type Request, type Response } from "express";
import type { Pool } from "pg";

import { computeKpis } from "../services/pmiCalculator.js";

interface Deps {
  pool: Pool | null;
  requireAdminToken: (token: string) => (req: Request, res: Response, next: () => void) => void;
  adminToken: string;
  logger: { warn: (obj: object, msg?: string) => void; info?: (obj: object, msg?: string) => void };
}

/** Until Sprint 2 UI seeds it, the operator's daily ops target defaults to 100. */
const DEFAULT_OPS_TARGET_PER_DAY = 100;

function utcMidnightFraction(now: Date = new Date()): number {
  const utcMidnight = Date.UTC(now.getUTCFullYear(), now.getUTCMonth(), now.getUTCDate());
  return (now.getTime() - utcMidnight) / 86_400_000;
}

function readChainId(req: Request): number | null {
  const raw = req.query["chain_id"];
  const n = Number(raw ?? 1);
  if (!Number.isInteger(n) || n < 1) return null;
  return n;
}

export function buildOperationsRouter(deps: Deps): Router {
  const r = Router();

  if (!deps.pool) {
    r.use((_req, res) => res.status(503).json({ error: "db_unavailable" }));
    return r;
  }

  const pool = deps.pool;
  const adminGate = deps.requireAdminToken(deps.adminToken);

  // ── /kpi handler ────────────────────────────────────────────────────────
  const kpiHandler = async (req: Request, res: Response): Promise<void> => {
    const chainId = readChainId(req);
    if (chainId === null) {
      res.status(400).json({ error: "invalid_chain_id" });
      return;
    }
    try {
      const cfgQ = await pool.query<{ capital_usd: string; min_profit_usd: string }>(
        `SELECT capital_usd::text, min_profit_usd::text
           FROM trading_config
          WHERE chain_id = $1 AND enabled = TRUE`,
        [chainId],
      );
      if (cfgQ.rowCount === 0) {
        res.status(503).json({
          error: "trading_config_not_seeded",
          hint: "seed via /config/trading first",
          chain_id: chainId,
        });
        return;
      }
      const capitalUsd = Number(cfgQ.rows[0]!.capital_usd);
      const minProfitUsd = Number(cfgQ.rows[0]!.min_profit_usd);

      const sumQ = await pool.query<{ profit: string; ops: string }>(
        `SELECT COALESCE(SUM(e.actual_profit_usd), 0)::text AS profit,
                COUNT(*)::text                              AS ops
           FROM executions e
           JOIN opportunities o ON o.id = e.opportunity_id
          WHERE o.chain_id = $1
            AND e.submitted_at >= date_trunc('day', NOW() AT TIME ZONE 'UTC')`,
        [chainId],
      );
      const profit = Number(sumQ.rows[0]!.profit);
      const ops = Number(sumQ.rows[0]!.ops);

      const now = new Date();
      const elapsedDayFraction = utcMidnightFraction(now);

      const k = computeKpis({
        capitalDeployedUsd: capitalUsd,
        profitRealizedUsd: profit,
        opsCompleted: ops,
        opsTargetPerDay: DEFAULT_OPS_TARGET_PER_DAY,
        minProfitUsdTarget: minProfitUsd,
        elapsedDayFraction,
      });

      res.status(200).json({
        chain_id: chainId,
        capital_deployed_usd: capitalUsd,
        profit_realized_usd: profit,
        ops_completed: ops,
        ops_target_per_day: DEFAULT_OPS_TARGET_PER_DAY,
        elapsed_day_fraction: elapsedDayFraction,
        kpis: {
          cpi: k.cpi,
          spi: k.spi,
          eac_usd: k.eacUsd,
          etc_usd: k.etcUsd,
          tcpi: k.tcpi,
          vac_usd: k.vacUsd,
          cv_usd: k.cvUsd,
        },
        computed_at: now.toISOString(),
      });
    } catch (err) {
      deps.logger.warn({ event: "operations.kpi_failed", err: String(err) });
      // Fail-honest: a query/compute failure here is transient (DB hiccup) →
      // 503 (retryable/degraded), not a silent 500. Cause is in the logger
      // above; body stays detail-free because these are public-mirror routes.
      res.status(503).json({ error: "query_failed" });
    }
  };

  // ── /scurve handler ─────────────────────────────────────────────────────
  const scurveHandler = async (req: Request, res: Response): Promise<void> => {
    const chainId = readChainId(req);
    if (chainId === null) {
      res.status(400).json({ error: "invalid_chain_id" });
      return;
    }
    const bucketMin = Number(req.query["bucket_minutes"] ?? 15);
    if (!Number.isInteger(bucketMin) || bucketMin < 1 || bucketMin > 60) {
      res.status(400).json({ error: "invalid_bucket_minutes" });
      return;
    }

    try {
      const cfgQ = await pool.query<{ capital_usd: string; min_profit_usd: string }>(
        `SELECT capital_usd::text, min_profit_usd::text FROM trading_config WHERE chain_id = $1`,
        [chainId],
      );
      const minProfitUsd = cfgQ.rowCount === 0 ? 0 : Number(cfgQ.rows[0]!.min_profit_usd);
      const targetPnl = DEFAULT_OPS_TARGET_PER_DAY * minProfitUsd;
      const targetPerBucket = targetPnl * (bucketMin / (24 * 60));

      const q = await pool.query<{ ts: Date; ops: string; profit: string }>(
        `SELECT date_trunc('hour', e.submitted_at)
                + FLOOR(EXTRACT(MINUTE FROM e.submitted_at) / $2) * ($2 || ' minutes')::interval AS ts,
                COUNT(*)::text                                AS ops,
                COALESCE(SUM(e.actual_profit_usd), 0)::text   AS profit
           FROM executions e
           JOIN opportunities o ON o.id = e.opportunity_id
          WHERE o.chain_id = $1
            AND e.submitted_at >= NOW() - INTERVAL '24 hours'
          GROUP BY 1
          ORDER BY 1`,
        [chainId, bucketMin],
      );

      let cumOps = 0;
      let cumProfit = 0;
      const buckets = q.rows.map((row, idx) => {
        cumOps += Number(row.ops);
        cumProfit += Number(row.profit);
        return {
          ts: row.ts.toISOString(),
          ops_cumulative: cumOps,
          profit_cumulative_usd: cumProfit,
          target_cumulative_usd: targetPerBucket * (idx + 1),
        };
      });

      res.status(200).json({ chain_id: chainId, bucket_minutes: bucketMin, buckets });
    } catch (err) {
      deps.logger.warn({ event: "operations.scurve_failed", err: String(err) });
      // Fail-honest: a query/compute failure here is transient (DB hiccup) →
      // 503 (retryable/degraded), not a silent 500. Cause is in the logger
      // above; body stays detail-free because these are public-mirror routes.
      res.status(503).json({ error: "query_failed" });
    }
  };

  // ── /variance handler ───────────────────────────────────────────────────
  const varianceHandler = async (req: Request, res: Response): Promise<void> => {
    const chainId = readChainId(req);
    if (chainId === null) {
      res.status(400).json({ error: "invalid_chain_id" });
      return;
    }
    try {
      const cfgQ = await pool.query<{ min_profit_usd: string }>(
        `SELECT min_profit_usd::text FROM trading_config WHERE chain_id = $1 AND enabled = TRUE`,
        [chainId],
      );
      const minProfit = cfgQ.rowCount === 0 ? 0 : Number(cfgQ.rows[0]!.min_profit_usd);
      const opsTargetPerHour = DEFAULT_OPS_TARGET_PER_DAY / 24;
      const plannedPerHour = minProfit * opsTargetPerHour;

      const q = await pool.query<{ bucket_start: Date; realized: string }>(
        `SELECT date_trunc('hour', e.submitted_at)              AS bucket_start,
                COALESCE(SUM(e.actual_profit_usd), 0)::text     AS realized
           FROM executions e
           JOIN opportunities o ON o.id = e.opportunity_id
          WHERE o.chain_id = $1
            AND e.submitted_at >= NOW() - INTERVAL '24 hours'
          GROUP BY 1
          ORDER BY 1`,
        [chainId],
      );
      const rows = q.rows.map((row) => {
        const realized = Number(row.realized);
        return {
          bucket_start: row.bucket_start.toISOString(),
          realized_pnl_usd: realized,
          planned_pnl_usd: plannedPerHour,
          cv_usd: realized - plannedPerHour,
        };
      });
      res.status(200).json({ chain_id: chainId, rows });
    } catch (err) {
      deps.logger.warn({ event: "operations.variance_failed", err: String(err) });
      // Fail-honest: a query/compute failure here is transient (DB hiccup) →
      // 503 (retryable/degraded), not a silent 500. Cause is in the logger
      // above; body stays detail-free because these are public-mirror routes.
      res.status(503).json({ error: "query_failed" });
    }
  };

  // Public-mirror routes (numbers only, no actor identity)
  r.get("/api/v1/operations/kpi", kpiHandler);
  r.get("/api/v1/operations/scurve", scurveHandler);
  r.get("/api/v1/operations/variance", varianceHandler);

  // Admin routes (gated)
  r.get("/admin/operations/kpi", adminGate, kpiHandler);
  r.get("/admin/operations/scurve", adminGate, scurveHandler);
  r.get("/admin/operations/variance", adminGate, varianceHandler);

  return r;
}
