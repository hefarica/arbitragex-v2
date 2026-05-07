/**
 * GET /api/v1/dexes?chain_id=N
 *
 * Returns all DEXes that have at least one factory on the requested chain,
 * augmented with per-chain pool counts and the operator's enabled flag.
 *
 * enabled logic:
 *   - trading_config.enabled_dex_ids IS NULL  → all DEXes are enabled (legacy compat)
 *   - trading_config.enabled_dex_ids IS NOT NULL → only listed UUIDs are enabled
 *   - no trading_config row for this chain → all DEXes are enabled (no config = no restriction)
 *
 * R8 fail-honest: never synthesizes rows. Array empty = no DEXes on this chain.
 */

import { Router, type Request, type Response } from "express";
import type { Pool } from "pg";

interface Deps {
  pool: Pool | null;
  logger: { warn: (obj: object, msg?: string) => void };
}

export function mountDexes(app: import("express").Express, deps: Deps): void {
  app.get("/api/v1/dexes", async (req: Request, res: Response) => {
    if (!deps.pool) {
      res.status(503).json({ error: "db_unavailable", detail: "DATABASE_URL not configured" });
      return;
    }

    const chainId = Number(req.query["chain_id"] ?? 1);
    if (!Number.isFinite(chainId) || chainId < 1) {
      res.status(400).json({ error: "invalid_chain_id" });
      return;
    }

    try {
      const q = await deps.pool.query(
        `WITH dex_chain AS (
           SELECT DISTINCT
                  d.id,
                  d.name,
                  d.protocol_type,
                  d.is_active,
                  d.volume_24h_usd::float          AS volume_24h_usd,
                  d.tvl_usd::float                 AS tvl_usd,
                  f.chain_id,
                  (SELECT COUNT(*)::int
                     FROM factories f2
                    WHERE f2.dex_id = d.id
                      AND f2.chain_id = f.chain_id) AS factory_count,
                  (SELECT COUNT(*)::int
                     FROM pools p
                    WHERE p.factory_id IN (
                            SELECT id FROM factories
                             WHERE dex_id = d.id
                               AND chain_id = f.chain_id
                          )
                      AND p.is_active = TRUE)        AS pool_count
             FROM dexes d
             JOIN factories f ON f.dex_id = d.id
            WHERE f.chain_id = $1
         ),
         cfg AS (
           SELECT enabled_dex_ids
             FROM trading_config
            WHERE chain_id = $1
         )
         SELECT dc.*,
                -- enabled = true when:
                --   (a) no trading_config row  OR
                --   (b) enabled_dex_ids IS NULL (all enabled)  OR
                --   (c) enabled_dex_ids @> ARRAY[dc.id]
                COALESCE(
                  (SELECT cfg.enabled_dex_ids IS NULL
                          OR cfg.enabled_dex_ids @> ARRAY[dc.id]
                     FROM cfg),
                  TRUE   -- no trading_config row → all enabled
                ) AS enabled
           FROM dex_chain dc
          ORDER BY dc.tvl_usd DESC NULLS LAST, dc.name ASC`,
        [chainId],
      );

      res.status(200).json({
        count: q.rows.length,
        chain_id: chainId,
        items: q.rows.map((r) => ({
          id:             r.id,
          name:           r.name,
          protocol_type:  r.protocol_type,
          is_active:      r.is_active,
          chain_id:       r.chain_id,
          factory_count:  r.factory_count,
          pool_count:     r.pool_count,
          volume_24h_usd: r.volume_24h_usd,
          tvl_usd:        r.tvl_usd,
          enabled:        r.enabled,
        })),
      });
    } catch (e) {
      deps.logger.warn({ event: "dexes.query_failed", err: (e as Error).message });
      res.status(503).json({ error: "query_failed", detail: (e as Error).message });
    }
  });
}
