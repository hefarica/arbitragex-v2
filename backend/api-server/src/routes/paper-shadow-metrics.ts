/**
 * GET /api/metrics/paper-shadow  (+ /api/v1/metrics/paper-shadow)
 *
 * Reads `paper_trade_runs` (migration 051 base + 091 archiver cols) and reports
 * the A.5 paper-shadow accumulation progress consumed by
 * components/PaperShadowPanel.tsx.
 *
 * R8 fail-honest:
 *   - pool null / query error → 503 (the panel renders the verbatim error).
 *   - 0 rows → 200 with status "INACTIVE" and zeros (never fabricates trades).
 *
 * IMPORTANT: there is NO `pnl_usd` column on paper_trade_runs (the audit SQL was
 * wrong). Realized paper PnL per row is:
 *     COALESCE(actual_profit_usd, sim_expected_profit_usd, 0)
 * `actual_profit_usd` is populated by the drift tracker once a run is reconciled;
 * `sim_expected_profit_usd` is the at-detection estimate used until then.
 *
 * Response numbers are JSON numbers (not strings) — the panel calls .toFixed(2).
 */

import type { Request, Response } from "express";
import type { Pool } from "pg";

interface Deps {
  pool: Pool | null;
  logger: { warn: (obj: object, msg?: string) => void };
}

// Frontend paper-shadow surface is chain 1 (ethereum) only for now.
const CHAIN_ID = 1;
const PNL_EXPR = "COALESCE(actual_profit_usd, sim_expected_profit_usd, 0)";

export function mountPaperShadowMetrics(app: import("express").Express, deps: Deps): void {
  const handler = async (_req: Request, res: Response): Promise<void> => {
    if (!deps.pool) {
      res.status(503).json({ error: "db_unavailable", detail: "DATABASE_URL not configured" });
      return;
    }

    // target_days: A.5 minimum paper-shadow streak (operator-tunable via env).
    // Default 7 to match scoring-status.ts + .env.example + activate_paper_shadow.sh
    // (the span threshold the rest of the system uses). Floor at 1 whole day.
    const targetRaw = Number(process.env["ARBX_PAPER_SHADOW_MIN_DAYS"] ?? 7);
    const targetDays =
      Number.isFinite(targetRaw) && Math.floor(targetRaw) >= 1 ? Math.floor(targetRaw) : 7;

    try {
      const totals = await deps.pool.query<{
        total_trades: string;
        green_trades: string;
        red_trades: string;
        pnl_accumulated_usd: string;
        pnl_today_usd: string;
        started_at: Date | null;
        last_trade_at: Date | null;
      }>(
        `SELECT
            COUNT(*)::text                                                   AS total_trades,
            COALESCE(SUM(CASE WHEN ${PNL_EXPR} > 0  THEN 1 ELSE 0 END), 0)::text AS green_trades,
            COALESCE(SUM(CASE WHEN ${PNL_EXPR} <= 0 THEN 1 ELSE 0 END), 0)::text AS red_trades,
            COALESCE(SUM(${PNL_EXPR}), 0)::text                              AS pnl_accumulated_usd,
            COALESCE(SUM(CASE WHEN created_at > NOW() - INTERVAL '1 day'
                              THEN ${PNL_EXPR} ELSE 0 END), 0)::text         AS pnl_today_usd,
            MIN(created_at)                                                  AS started_at,
            MAX(created_at)                                                  AS last_trade_at
           FROM paper_trade_runs
          WHERE chain_id = $1`,
        [CHAIN_ID],
      );

      // Per-day PnL (most recent first) for the consecutive-green-day streak.
      const days = await deps.pool.query<{ day_pnl: string }>(
        `SELECT COALESCE(SUM(${PNL_EXPR}), 0)::text AS day_pnl
           FROM paper_trade_runs
          WHERE chain_id = $1
          GROUP BY date_trunc('day', created_at)
          ORDER BY date_trunc('day', created_at) DESC`,
        [CHAIN_ID],
      );

      let consecutiveGreenDays = 0;
      for (const row of days.rows) {
        if (Number(row.day_pnl) > 0) consecutiveGreenDays += 1;
        else break;
      }

      const t = totals.rows[0]!;
      const totalTrades = Number(t.total_trades);
      const status: "ACTIVE" | "INACTIVE" | "COMPLETED" =
        totalTrades === 0
          ? "INACTIVE"
          : consecutiveGreenDays >= targetDays
            ? "COMPLETED"
            : "ACTIVE";

      res.status(200).json({
        metrics: {
          consecutive_green_days: consecutiveGreenDays,
          target_days: targetDays,
          pnl_today_usd: Number(t.pnl_today_usd),
          pnl_accumulated_usd: Number(t.pnl_accumulated_usd),
          status,
          started_at: t.started_at ? t.started_at.toISOString() : null,
          last_trade_at: t.last_trade_at ? t.last_trade_at.toISOString() : null,
          total_trades: totalTrades,
          green_trades: Number(t.green_trades),
          red_trades: Number(t.red_trades),
        },
        generated_at: new Date().toISOString(),
      });
    } catch (e) {
      deps.logger.warn({ event: "paper_shadow_metrics.query_failed", err: (e as Error).message });
      res.status(503).json({ error: "query_failed", detail: (e as Error).message });
    }
  };

  // Dual-path: the FE fetches /api/...; the edge maps /api/v1/... Both resolve here.
  app.get("/api/metrics/paper-shadow", handler);
  app.get("/api/v1/metrics/paper-shadow", handler);
}
