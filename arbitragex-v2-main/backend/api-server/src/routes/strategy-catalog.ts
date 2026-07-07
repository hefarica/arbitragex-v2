/**
 * Sprint 2 Task 2.2 — Strategy catalog read endpoints.
 *
 *   GET /api/v1/strategy-catalog            list every strategy entry (read-only)
 *   GET /api/v1/strategy-catalog/active     active strategies for chain_id (default 1)
 *
 * Catalog is metadata-only: which strategies the platform CAN run. The
 * activation per-chain lives in `trading_config.enabled_strategies` and
 * is mutated through the existing /admin/trading-config/:chain_id PUT
 * endpoint — no new mutation surface here.
 */

import { Router, type Request, type Response } from "express";
import type { Pool } from "pg";

interface Deps {
  pool: Pool | null;
  logger: { warn: (obj: object, msg?: string) => void };
}

interface CatalogRow {
  kind: string;
  display_name: string;
  description: string;
  category: string;
  is_implemented: boolean;
  is_default: boolean;
  // Migration 035 — replaces the binary is_implemented as the operative
  // signal. UI uses this 5-state field exclusively for status badges + to
  // disable toggles for non-LIVE strategies (RULE 00 honesty).
  lifecycle_status: "live" | "designed" | "scaffold" | "not_started" | "defensive_only" | null;
  risk_level: string;
  capital_required: string | null;
  expected_complexity: string | null;
  competitive_advantage: string | null;
  ethical_constraint: string | null;
  skill_refs: string[];
  requires_flashloan: boolean;
  chains_supported: string[]; // pg returns BIGINT[] as string[] by default
}

export function buildStrategyCatalogRouter(deps: Deps): Router {
  const r = Router();

  if (!deps.pool) {
    r.use((_req, res) => res.status(503).json({ error: "db_unavailable" }));
    return r;
  }
  const pool = deps.pool;

  r.get("/api/v1/strategy-catalog", async (_req: Request, res: Response): Promise<void> => {
    try {
      const q = await pool.query<CatalogRow>(
        `SELECT kind, display_name, description, category, is_implemented, is_default,
                lifecycle_status,
                risk_level, capital_required, expected_complexity, competitive_advantage,
                ethical_constraint, skill_refs, requires_flashloan,
                chains_supported::text[] AS chains_supported
           FROM strategy_catalog
          ORDER BY is_default DESC, display_name`,
      );
      const entries = q.rows.map((row) => ({
        ...row,
        chains_supported: row.chains_supported.map((s) => Number(s)),
      }));
      res.status(200).json({ entries });
    } catch (e) {
      deps.logger.warn({ event: "strategy_catalog.read_failed", err: (e as Error).message });
      res.status(503).json({ error: "query_failed", detail: (e as Error).message });
    }
  });

  r.get("/api/v1/strategy-catalog/active", async (req: Request, res: Response): Promise<void> => {
    const chainId = Number(req.query["chain_id"] ?? 1);
    if (!Number.isInteger(chainId) || chainId < 1) {
      res.status(400).json({ error: "invalid_chain_id" });
      return;
    }
    try {
      const q = await pool.query<{ enabled_strategies: string[] }>(
        `SELECT enabled_strategies FROM trading_config WHERE chain_id = $1`,
        [chainId],
      );
      const active = q.rowCount === 0 ? [] : q.rows[0]!.enabled_strategies;
      res.status(200).json({ chain_id: chainId, active });
    } catch (e) {
      deps.logger.warn({ event: "strategy_catalog.active_failed", err: (e as Error).message });
      res.status(503).json({ error: "query_failed", detail: (e as Error).message });
    }
  });

  return r;
}
