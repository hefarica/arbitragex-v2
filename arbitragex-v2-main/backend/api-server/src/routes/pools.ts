/**
 * GET /api/v1/pools?chain_id=N[&dex_id=UUID][&protocol_type=STR][&limit=N]
 *
 * Returns active pools for the requested chain.
 * Optional filters:
 *   dex_id        – restrict to pools belonging to this DEX
 *   protocol_type – restrict to pools of this protocol (UNISWAP_V2, UNISWAP_V3, etc.)
 *   limit         – max rows returned; clamped to [1, 500]; default 200
 *
 * R8 fail-honest: never synthesizes rows. Array empty = no matching pools.
 */

import { Router, type Request, type Response } from "express";
import type { Pool } from "pg";

interface Deps {
  pool: Pool | null;
  logger: { warn: (obj: object, msg?: string) => void };
}

// UUID v4 pattern — permissive (accepts upper/lower case and braces-free form).
const UUID_RE = /^[0-9a-fA-F]{8}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{12}$/;

export function mountPools(app: import("express").Express, deps: Deps): void {
  app.get("/api/v1/pools", async (req: Request, res: Response) => {
    if (!deps.pool) {
      res.status(503).json({ error: "db_unavailable", detail: "DATABASE_URL not configured" });
      return;
    }

    const chainId = Number(req.query["chain_id"] ?? 1);
    if (!Number.isFinite(chainId) || chainId < 1) {
      res.status(400).json({ error: "invalid_chain_id" });
      return;
    }

    const dexIdRaw = req.query["dex_id"];
    const dexId = typeof dexIdRaw === "string" ? dexIdRaw : null;
    if (dexId !== null && !UUID_RE.test(dexId)) {
      res.status(400).json({ error: "invalid_dex_id" });
      return;
    }

    const protocolType =
      typeof req.query["protocol_type"] === "string" ? req.query["protocol_type"] : null;

    const limitRaw = Number(req.query["limit"] ?? 200);
    const limit = Number.isFinite(limitRaw) ? Math.max(1, Math.min(500, limitRaw)) : 200;

    try {
      const q = await deps.pool.query(
        `SELECT
             p.id,
             p.chain_id,
             p.address,
             p.fee_tier,
             p.is_active,
             d.id          AS dex_id,
             d.name        AS dex_name,
             d.protocol_type,
             t0.symbol     AS token0_symbol,
             t0.address    AS token0_address,
             t1.symbol     AS token1_symbol,
             t1.address    AS token1_address
           FROM pools p
           JOIN factories f  ON f.id  = p.factory_id
           JOIN dexes    d   ON d.id  = f.dex_id
           LEFT JOIN tokens  t0 ON t0.id = p.token0_id
           LEFT JOIN tokens  t1 ON t1.id = p.token1_id
          WHERE p.chain_id    = $1
            AND p.is_active   = TRUE
            AND ($2::uuid IS NULL OR d.id = $2::uuid)
            AND ($3::text IS NULL OR d.protocol_type = $3)
          ORDER BY d.name ASC, p.address ASC
          LIMIT $4`,
        [chainId, dexId, protocolType, limit],
      );

      res.status(200).json({
        count: q.rows.length,
        chain_id: chainId,
        filters: { dex_id: dexId, protocol_type: protocolType },
        items: q.rows.map((r) => ({
          id:              r.id,
          chain_id:        r.chain_id,
          address:         r.address,
          dex_id:          r.dex_id,
          dex_name:        r.dex_name,
          protocol_type:   r.protocol_type,
          fee_tier:        r.fee_tier,
          token0_symbol:   r.token0_symbol,
          token0_address:  r.token0_address,
          token1_symbol:   r.token1_symbol,
          token1_address:  r.token1_address,
          is_active:       r.is_active,
        })),
      });
    } catch (e) {
      deps.logger.warn({ event: "pools.query_failed", err: (e as Error).message });
      res.status(503).json({ error: "query_failed", detail: (e as Error).message });
    }
  });
}
