/**
 * RPC registry — focused admin surface for the Excel-catalog sync feature.
 *
 *   GET  /api/v1/rpc/status            (public, counts only) — powers the /rpcs panel
 *   POST /api/v1/admin/rpcs/import     (admin) — idempotent bulk upsert + hot-reload
 *   POST /api/v1/admin/rpcs/reload     (admin) — publish a bare reload signal
 *
 * Intentionally does NOT mount the generic create/patch/delete CRUD for
 * rpc_endpoints (that stays unmounted per the 2026-06-04 security finding). This
 * is a narrow, purpose-built surface that reuses the registry-engine primitives
 * (RpcDescriptor schema/hashColumns, computeConfigHash, publishReload).
 *
 * Source of truth caveat: searcher-rs reads RPC from the `.env` RPC_HTTP_<id>
 * vars (shared-rs/rpc_failover.rs); writing rpc_endpoints here hot-reloads the
 * registry/UI layer via arbx:config:rpcs (Arc-swap, no restart), but the
 * searcher's live RPC still updates on the next .env deploy/recreate.
 *
 * Follow-up (not in this PR): full audit_event + config_hash_registry 7-layer
 * integration (needs post-mig DB functions). For now the table's updated_by/
 * updated_at + structured logs are the trail; admin gating is the control.
 *
 * @module routes/rpc-registry
 */
import type { Express, Request, Response } from "express";
import type { Pool } from "pg";
import type { Redis } from "ioredis";
import { randomUUID } from "node:crypto";
import { z } from "zod";
import {
  RpcDescriptor,
  computeConfigHash,
  publishReload,
  type ReloadEvent,
} from "../lib/registry-engine.js";

interface Deps {
  pool: Pool | null;
  redis: Redis;
  requireAdminToken: (token: string) => (req: Request, res: Response, next: () => void) => void;
  adminToken: string;
  logger: { warn: (o: object, m?: string) => void; info: (o: object, m?: string) => void };
}

// Reuse the canonical rpc schema (validates chain_id/url/transport/tier/... with
// defaults). Bulk body: { rows: [...] } with a sane cap.
const ImportBody = z.object({
  rows: z.array(RpcDescriptor.schema).min(1).max(2000),
});

function actorOf(req: Request): string {
  return String(req.header("x-omega-actor") ?? req.ip ?? "admin").slice(0, 64);
}

export function mountRpcRegistry(app: Express, deps: Deps): void {
  // ── GET status (public, counts only — no URLs/secrets) ───────────────────
  const statusHandler = async (_req: Request, res: Response): Promise<void> => {
    if (!deps.pool) {
      res.status(503).json({ error: "db_unavailable" });
      return;
    }
    try {
      const r = await deps.pool.query(
        `SELECT chain_id,
                COUNT(*) FILTER (WHERE transport IN ('http','https')) AS http,
                COUNT(*) FILTER (WHERE transport IN ('ws','wss'))     AS ws,
                COUNT(*) FILTER (WHERE enabled)                        AS enabled,
                COUNT(*)                                               AS total,
                MAX(updated_at)                                        AS last_updated
           FROM rpc_endpoints
          GROUP BY chain_id
          ORDER BY chain_id`,
      );
      const chains = r.rows.map((row) => ({
        chain_id: Number(row.chain_id),
        http: Number(row.http),
        ws: Number(row.ws),
        enabled: Number(row.enabled),
        total: Number(row.total),
        last_updated: row.last_updated ? new Date(row.last_updated as string).toISOString() : null,
      }));
      res.status(200).json({
        chains,
        chain_count: chains.length,
        total: chains.reduce((a, c) => a + c.total, 0),
        generated_at: new Date().toISOString(),
      });
    } catch (e) {
      deps.logger.warn({ event: "rpc.status_failed", err: (e as Error).message });
      res.status(500).json({ error: "db_error", detail: (e as Error).message });
    }
  };
  app.get("/api/v1/rpc/status", statusHandler);
  app.get("/api/rpc/status", statusHandler); // non-/v1 alias for the edge convention

  // ── POST import (admin) — idempotent bulk upsert + per-chain hot-reload ───
  app.post("/api/v1/admin/rpcs/import", deps.requireAdminToken(deps.adminToken), async (req, res) => {
    if (!deps.pool) {
      res.status(503).json({ error: "db_unavailable" });
      return;
    }
    const parsed = ImportBody.safeParse(req.body);
    if (!parsed.success) {
      res.status(400).json({ error: "schema_violation", details: parsed.error.format() });
      return;
    }
    const actor = actorOf(req);
    const rows = parsed.data.rows;

    let inserted = 0;
    let updated = 0;
    let unchanged = 0;
    const changedChains = new Set<number>();

    const client = await deps.pool.connect();
    try {
      await client.query("BEGIN");
      for (const row of rows as Array<Record<string, unknown>>) {
        const hash = computeConfigHash(row, RpcDescriptor.hashColumns);
        const r = await client.query(
          `INSERT INTO rpc_endpoints
             (chain_id, url, transport, tier, auth_kind, auth_credential_id, weight,
              rate_limit_rps, max_concurrency, enabled, status, notes,
              config_hash, created_by, updated_by)
           VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14,$14)
           ON CONFLICT (chain_id, url) DO UPDATE SET
             transport = EXCLUDED.transport, tier = EXCLUDED.tier,
             auth_kind = EXCLUDED.auth_kind, auth_credential_id = EXCLUDED.auth_credential_id,
             weight = EXCLUDED.weight, rate_limit_rps = EXCLUDED.rate_limit_rps,
             max_concurrency = EXCLUDED.max_concurrency, enabled = EXCLUDED.enabled,
             status = EXCLUDED.status, notes = EXCLUDED.notes,
             config_hash = EXCLUDED.config_hash, updated_by = EXCLUDED.updated_by,
             updated_at = now()
           WHERE rpc_endpoints.config_hash IS DISTINCT FROM EXCLUDED.config_hash
           RETURNING (xmax = 0) AS was_insert`,
          [
            row.chain_id, row.url, row.transport, row.tier, row.auth_kind,
            row.auth_credential_id ?? null, row.weight, row.rate_limit_rps ?? null,
            row.max_concurrency, row.enabled, row.status, row.notes ?? null,
            hash, actor,
          ],
        );
        if ((r.rowCount ?? 0) === 0) {
          unchanged += 1; // ON CONFLICT WHERE false → identical row, no write
          continue;
        }
        if (r.rows[0].was_insert === true) inserted += 1;
        else updated += 1;
        changedChains.add(Number(row.chain_id));
      }
      await client.query("COMMIT");
    } catch (e) {
      await client.query("ROLLBACK");
      deps.logger.warn({ event: "rpc.import_failed", err: (e as Error).message });
      res.status(500).json({ error: "import_failed", detail: (e as Error).message });
      client.release();
      return;
    }
    client.release();

    // Hot-reload AFTER commit — one event per affected chain (Arc-swap receiver).
    for (const chainId of changedChains) {
      const evt: ReloadEvent = {
        event_id: randomUUID(),
        idempotency_key: randomUUID(),
        resource: "rpc",
        chain_id: chainId,
        action: "update",
        entity_id: `chain:${chainId}`,
        config_hash_before: null,
        config_hash_after: "bulk-import",
        actor,
        timestamp: new Date().toISOString(),
      };
      await publishReload(deps.redis, RpcDescriptor, evt);
    }

    deps.logger.info({ event: "rpc.import", inserted, updated, unchanged, reloaded_chains: changedChains.size, actor });
    res.status(200).json({
      inserted,
      updated,
      unchanged,
      total: rows.length,
      reloaded_chains: [...changedChains].sort((a, b) => a - b),
      generated_at: new Date().toISOString(),
    });
  });

  // ── POST reload (admin) — bare global reload signal (the "Recargar" button) ─
  app.post("/api/v1/admin/rpcs/reload", deps.requireAdminToken(deps.adminToken), async (req, res) => {
    const actor = actorOf(req);
    try {
      const evt: ReloadEvent = {
        event_id: randomUUID(),
        idempotency_key: randomUUID(),
        resource: "rpc",
        chain_id: null,
        action: "reload",
        entity_id: "all",
        config_hash_before: null,
        config_hash_after: "reload",
        actor,
        timestamp: new Date().toISOString(),
      };
      await publishReload(deps.redis, RpcDescriptor, evt);
      res.status(200).json({ reloaded: true, event_id: evt.event_id, generated_at: new Date().toISOString() });
    } catch (e) {
      deps.logger.warn({ event: "rpc.reload_failed", err: (e as Error).message });
      res.status(500).json({ error: "reload_failed", detail: (e as Error).message });
    }
  });
}
