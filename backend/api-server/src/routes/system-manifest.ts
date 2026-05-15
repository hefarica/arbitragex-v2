/**
 * =============================================================================
 * OMEGA system-manifest — Mirror Fidelity contract + drift + runtime_ack
 * =============================================================================
 *
 * Endpoints:
 *   GET  /api/system/feature_manifest   — authoritative feature surface
 *   GET  /api/system/config-hashes      — current per-resource hashes
 *   GET  /api/system/drift              — unresolved drift observations
 *   POST /api/system/runtime-ack        — receives ack from searcher-rs
 *
 * @module routes/system-manifest
 */

import { Router, type Request, type Response } from "express";
import type { Pool } from "pg";
import type { Redis } from "ioredis";
import type { Server as IoServer } from "socket.io";
import { z } from "zod";
import { broadcastRuntimeAck } from "../websocket.js";

const RuntimeAckSchema = z.object({
  event_id: z.string().uuid(),
  resource: z.string().min(1).max(64),
  chain_id: z.number().int().positive().nullable(),
  idempotency_key: z.string().min(1).max(256),
  config_hash_before: z.string().regex(/^[0-9a-f]{64}$/).nullable(),
  config_hash_after: z.string().regex(/^[0-9a-f]{64}$/),
  worker_id: z.string().min(1).max(128),
  layer: z.enum([
    "api","persistence","redis_pubsub","searcher_rs","arc_swap",
    "frontend_refresh","readiness","audit",
  ]),
  status: z.enum(["pending","received","applied","rejected","timeout","failed"]),
  latency_ms: z.number().int().nonnegative().nullable().optional(),
  error: z.string().max(2000).nullable().optional(),
});

export function mountSystemManifest(db: Pool, _redis: Redis, io: IoServer): Router {
  const r = Router();

  r.get("/feature_manifest", async (_req: Request, res: Response) => {
    const q = await db.query(
      `SELECT feature_key, description, layer, state_hash, panel_path, required, updated_at
         FROM feature_manifest
        ORDER BY layer, feature_key`,
    );
    res.json({ features: q.rows, generated_at: new Date().toISOString() });
  });

  r.get("/config-hashes", async (req: Request, res: Response) => {
    const chainId = req.query.chain_id ? Number(req.query.chain_id) : null;
    const q = await db.query(
      `SELECT DISTINCT ON (resource, chain_id)
              resource, chain_id, hash_value, row_count, computed_at, snapshot_id
         FROM config_hash_registry
        WHERE ($1::bigint IS NULL OR chain_id = $1 OR chain_id IS NULL)
        ORDER BY resource, chain_id, computed_at DESC`,
      [chainId],
    );
    res.json({ hashes: q.rows });
  });

  r.get("/drift", async (_req: Request, res: Response) => {
    const q = await db.query(
      `SELECT *
         FROM drift_observations
        WHERE resolved_at IS NULL
        ORDER BY severity DESC, observed_at DESC
        LIMIT 200`,
    );
    res.json({ drift: q.rows, count: q.rowCount });
  });

  r.post("/runtime-ack", async (req: Request, res: Response) => {
    const parse = RuntimeAckSchema.safeParse(req.body);
    if (!parse.success) {
      res.status(400).json({ error: "schema_violation", details: parse.error.format() });
      return;
    }
    const p = parse.data;
    await db.query(
      `INSERT INTO runtime_ack (
         event_id, resource, chain_id, idempotency_key,
         config_hash_before, config_hash_after,
         worker_id, layer, status, latency_ms, error
       ) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11)`,
      [
        p.event_id,
        p.resource,
        p.chain_id,
        p.idempotency_key,
        p.config_hash_before,
        p.config_hash_after,
        p.worker_id,
        p.layer,
        p.status,
        p.latency_ms ?? null,
        p.error ?? null,
      ],
    );
    // OMEGA-7 PR-1: emit the ack into the runtime_ack WSS room AFTER the
    // INSERT has succeeded (invariant I-2 idempotencia POST-INSERT). The
    // broadcast is best-effort — if it throws for any reason we log and
    // continue, because the ack is already durably persisted in PostgreSQL
    // and the client can recover via the (future) GET /:event_id fallback.
    try {
      broadcastRuntimeAck(io, p);
    } catch (err) {
      console.warn(
        '[system-manifest] broadcastRuntimeAck failed post-INSERT (ack persisted; emit skipped):',
        (err as Error).message,
      );
    }
    res.status(202).json({ accepted: true });
  });

  return r;
}
