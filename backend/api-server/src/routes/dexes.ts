/**
 * GET /api/v1/dexes
 *
 * Dual-shape route, branch on `chain_id` query parameter:
 *
 *   1. `?chain_id=N` PRESENT  → per-chain shape `{ count, chain_id, items: [...] }`
 *      Consumed by `/strategies` DexesTab — each item is a chain-specific
 *      view with `factory_count` / `pool_count` / `enabled`.
 *
 *   2. `chain_id` ABSENT       → registry shape `{ dexes: [...] }`
 *      Consumed by `/dex-registry` page — each item is a chain-AGNOSTIC view
 *      that AGGREGATES factories by `chain_ids: number[]` and uses
 *      `dexes.tvl_usd` / `dexes.volume_24h_usd` (the global aggregates from
 *      migration 045 / DeFiLlama enricher).
 *
 * enabled logic (per-chain shape only):
 *   - trading_config.enabled_dex_ids IS NULL  → all DEXes are enabled (legacy compat)
 *   - trading_config.enabled_dex_ids IS NOT NULL → only listed UUIDs are enabled
 *   - no trading_config row for this chain → all DEXes are enabled (no config = no restriction)
 *
 * R8 fail-honest: never synthesises rows. Empty arrays = no DEXes seeded.
 */

import type { Request, Response } from "express";
import type { Pool } from "pg";

interface Deps {
  pool: Pool | null;
  logger: { warn: (obj: object, msg?: string) => void };
}

export function mountDexes(app: import("express").Express, deps: Deps): void {
  // ── PUT /api/v1/dexes/:id/active ────────────────────────────────────────
  // Operator toggle from the /dex-registry page. Admin token required.
  // R8: mounting deps.requireAdminToken would be cleaner, but `mountDexes`
  // does not receive admin deps; we inline-validate via env header check
  // mirroring the convention in trading-config route's other admin paths.
  app.put("/api/v1/dexes/:id/active", async (req: Request, res: Response) => {
    const expected = process.env["ARBX_ADMIN_TOKEN"] ?? "";
    const got = String(req.header("x-arbx-admin-token") ?? "");
    if (!expected || got !== expected) {
      res.status(401).json({ error: "unauthorized" });
      return;
    }
    if (!deps.pool) {
      res.status(503).json({ error: "db_unavailable" });
      return;
    }
    const id = String(req.params["id"] ?? "");
    if (!/^[0-9a-fA-F-]{36}$/.test(id)) {
      res.status(400).json({ error: "invalid_uuid" });
      return;
    }
    const body = req.body as { is_active?: unknown };
    if (typeof body.is_active !== "boolean") {
      res.status(400).json({ error: "is_active boolean required" });
      return;
    }
    try {
      const q = await deps.pool.query<{ id: string; is_active: boolean; updated_at: Date }>(
        `UPDATE dexes
            SET is_active = $2,
                updated_at = NOW()
          WHERE id = $1
        RETURNING id, is_active, updated_at`,
        [id, body.is_active],
      );
      if (q.rowCount === 0) {
        res.status(404).json({ error: "dex_not_found" });
        return;
      }
      const row = q.rows[0]!;
      res.status(200).json({
        id: row.id,
        is_active: row.is_active,
        updated_at: row.updated_at.toISOString(),
      });
    } catch (e) {
      deps.logger.warn({ event: "dexes.toggle_failed", err: (e as Error).message });
      res.status(503).json({ error: "query_failed", detail: (e as Error).message });
    }
  });

  app.get("/api/v1/dexes", async (req: Request, res: Response) => {
    if (!deps.pool) {
      res.status(503).json({ error: "db_unavailable", detail: "DATABASE_URL not configured" });
      return;
    }

    // Registry shape branch — when no chain_id is supplied, return the
    // cross-chain aggregate consumed by /dex-registry.
    if (req.query["chain_id"] === undefined) {
      try {
        const q = await deps.pool.query(
          `SELECT d.id,
                  d.name,
                  d.protocol_type,
                  d.is_active,
                  d.tvl_usd::float        AS tvl_usd,
                  d.volume_24h_usd::float AS volume_24h_usd,
                  d.created_at,
                  COALESCE(
                    (SELECT array_agg(DISTINCT f.chain_id ORDER BY f.chain_id)
                       FROM factories f
                      WHERE f.dex_id = d.id),
                    ARRAY[]::int[]
                  ) AS chain_ids,
                  (SELECT f.address
                     FROM factories f
                    WHERE f.dex_id = d.id
                    ORDER BY f.chain_id ASC
                    LIMIT 1) AS factory_address
             FROM dexes d
            ORDER BY d.tvl_usd DESC NULLS LAST, d.name ASC`,
        );
        res.status(200).json({
          dexes: q.rows.map((r: Record<string, unknown>) => ({
            id:               r["id"]               as string,
            name:             r["name"]             as string,
            protocol_type:    r["protocol_type"]    as string,
            chain_ids:        (r["chain_ids"]      ?? []) as number[],
            volume_24h_usd:   (r["volume_24h_usd"]   as number | null) ?? null,
            tvl_usd:          (r["tvl_usd"]          as number | null) ?? null,
            is_active:        Boolean(r["is_active"]),
            // R8: router/fee not yet a column on `dexes` — surface null
            // instead of inventing values. Future migration adds them.
            router_address:   null,
            factory_address:  (r["factory_address"] as string | null) ?? null,
            fee_bps:          null,
            created_at:       r["created_at"]
                                ? new Date(r["created_at"] as string | Date).toISOString()
                                : null,
          })),
        });
      } catch (e) {
        deps.logger.warn({ event: "dexes.registry_query_failed", err: (e as Error).message });
        res.status(503).json({ error: "query_failed", detail: (e as Error).message });
      }
      return;
    }

    const chainId = Number(req.query["chain_id"] ?? 1);
    if (!Number.isFinite(chainId) || chainId < 1) {
      res.status(400).json({ error: "invalid_chain_id" });
      return;
    }

    try {
      // Per-chain query: COALESCE on dex_chain_metrics (migration 045) so each
      // chain reports its own TVL / volume; falls back to global aggregates on
      // dexes.* if the per-chain row is missing. R8 fail-honest: NULL flows
      // through if BOTH per-chain and global are missing — never substitute 0.
      const q = await deps.pool.query(
        `WITH dex_chain AS (
           SELECT DISTINCT
                  d.id,
                  d.name,
                  d.protocol_type,
                  d.is_active,
                  COALESCE(m.volume_24h_usd, d.volume_24h_usd)::float AS volume_24h_usd,
                  COALESCE(m.tvl_usd,        d.tvl_usd)::float        AS tvl_usd,
                  m.source              AS metrics_source,
                  m.last_updated_at     AS metrics_updated_at,
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
             LEFT JOIN dex_chain_metrics m
                    ON m.dex_id = d.id
                   AND m.chain_id = f.chain_id
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
          id:                 r.id,
          name:               r.name,
          protocol_type:      r.protocol_type,
          is_active:          r.is_active,
          chain_id:           r.chain_id,
          factory_count:      r.factory_count,
          pool_count:         r.pool_count,
          volume_24h_usd:     r.volume_24h_usd,
          tvl_usd:            r.tvl_usd,
          metrics_source:     r.metrics_source ?? null,
          metrics_updated_at: r.metrics_updated_at
            ? new Date(r.metrics_updated_at).toISOString()
            : null,
          enabled:            r.enabled,
        })),
      });
    } catch (e) {
      deps.logger.warn({ event: "dexes.query_failed", err: (e as Error).message });
      res.status(503).json({ error: "query_failed", detail: (e as Error).message });
    }
  });
}
