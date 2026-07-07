/**
 * DeFi registry endpoints (legacy mount under /api/{chains,rpcs,pools,metrics}).
 *
 * SEC-01 fix: removed hardcoded postgres password fallback. The shared app pool
 * (created in index.ts from process.env.DATABASE_URL) is injected via deps.
 * If DATABASE_URL is unset the server starts with pool=null and these routes
 * return 503 — fail-fast per CLAUDE.md RULE 02.
 *
 * API-01 fix: corrected table references that pointed at non-existent tables.
 *   defi_chains → chains   (migration 021)
 *   defi_rpcs   → rpcs     (migration 021)
 *   defi_pools  → pools    (migration 022)
 * Also: pools.active → pools.is_active (real column name per migration 022).
 *
 * R8 fail-honest: never fabricates rows. Empty array = no data.
 */

import type { Request, Response } from "express";
import type { Pool } from "pg";

interface Deps {
  pool: Pool | null;
  logger: { warn: (obj: object, msg?: string) => void };
}

export function mountDefi(app: import("express").Express, deps: Deps): void {
  // ==========================================
  // REGISTRIES: CHAINS, RPCS
  // ==========================================

  app.get("/api/chains", async (_req: Request, res: Response) => {
    if (!deps.pool) {
      res.status(503).json({ success: false, error: "db_unavailable", detail: "DATABASE_URL not configured" });
      return;
    }
    try {
      const result = await deps.pool.query(
        "SELECT * FROM chains WHERE is_active = TRUE ORDER BY chain_id ASC",
      );
      res.json({ success: true, data: result.rows });
    } catch (e) {
      const msg = (e as Error).message;
      deps.logger.warn({ event: "defi.chains.query_failed", err: msg });
      res.status(500).json({ success: false, error: "Database error", message: msg });
    }
  });

  app.get("/api/rpcs", async (_req: Request, res: Response) => {
    if (!deps.pool) {
      res.status(503).json({ success: false, error: "db_unavailable", detail: "DATABASE_URL not configured" });
      return;
    }
    try {
      const result = await deps.pool.query(
        "SELECT * FROM rpcs ORDER BY latency_ms ASC NULLS LAST",
      );
      res.json({ success: true, data: result.rows });
    } catch (e) {
      const msg = (e as Error).message;
      deps.logger.warn({ event: "defi.rpcs.query_failed", err: msg });
      res.status(500).json({ success: false, error: "Database error", message: msg });
    }
  });

  // ==========================================
  // POOLS
  // ==========================================

  app.get("/api/pools", async (_req: Request, res: Response) => {
    if (!deps.pool) {
      res.status(503).json({ success: false, error: "db_unavailable", detail: "DATABASE_URL not configured" });
      return;
    }
    try {
      const result = await deps.pool.query(
        "SELECT * FROM pools WHERE is_active = TRUE ORDER BY created_at DESC LIMIT 100",
      );
      res.json({ success: true, data: result.rows });
    } catch (e) {
      const msg = (e as Error).message;
      deps.logger.warn({ event: "defi.pools.query_failed", err: msg });
      res.status(500).json({ success: false, error: "Database error", message: msg });
    }
  });

  // ==========================================
  // METRICS (process-level worker health)
  // ==========================================

  app.get("/api/metrics", async (_req: Request, res: Response) => {
    res.json({
      success: true,
      data: {
        active_workers: 0,
        cpu_usage_pct: 0,
        memory_usage_mb: process.memoryUsage().rss / 1024 / 1024,
        uptime_seconds: process.uptime(),
        kernel_bypass_active: false,
      },
    });
  });
}
