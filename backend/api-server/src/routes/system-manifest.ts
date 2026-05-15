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

import { Router, type Request, type RequestHandler, type Response } from "express";
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

/**
 * Optional auth/rate-limit middlewares injected by the host application.
 * The host (index.ts) wires:
 *   - `requireServiceToken` for POST /runtime-ack (P0-1, OMEGA-8/M3)
 *   - `requireAdminToken`   for GET  /runtime-ack/:event_id (P1-4)
 *   - `rateLimit`           per-IP for both runtime_ack handlers
 *
 * Why injected: the router is `pool`-scoped and reusable in tests; the
 * middlewares depend on env-derived tokens loaded once at boot.
 */
export interface SystemManifestDeps {
  requireServiceToken: RequestHandler;
  requireAdminToken: RequestHandler;
  runtimeAckPostLimiter: RequestHandler;
  runtimeAckGetLimiter: RequestHandler;
}

export function mountSystemManifest(
  db: Pool,
  _redis: Redis,
  io: IoServer,
  deps: SystemManifestDeps,
): Router {
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

  r.post(
    "/runtime-ack",
    deps.runtimeAckPostLimiter,
    deps.requireServiceToken,
    async (req: Request, res: Response) => {
    const parse = RuntimeAckSchema.safeParse(req.body);
    if (!parse.success) {
      res.status(400).json({ error: "schema_violation", details: parse.error.format() });
      return;
    }
    const p = parse.data;
    // OMEGA-8/M3 P1-1 + P1-5: advisory lock keyed by (resource, chain_id) so
    // concurrent inserts from horizontally-scaled producers serialize through
    // PostgreSQL rather than racing on the UNIQUE(event_id, layer) index. The
    // lock is held only for the duration of this UPSERT — pg_advisory_xact_lock
    // releases automatically on COMMIT/ROLLBACK. We compute a stable 32-bit key
    // by hashing "resource:chain_id" so that different resources/chains do not
    // contend, and use a single-int variant to avoid catalog probes.
    // The UPSERT uses ON CONFLICT (event_id, layer) — when migration 069 is
    // applied this short-circuits duplicates; until then the conflict target
    // matches the non-unique index and the INSERT proceeds, preserving
    // backward-compatibility during the migration window.
    const lockKey = `runtime_ack:${p.resource}:${p.chain_id ?? "null"}`;
    let inserted = false;
    let deduped = false;
    await db.query("BEGIN");
    try {
      await db.query("SELECT pg_advisory_xact_lock(hashtext($1)::int)", [lockKey]);
      const result = await db.query(
        `INSERT INTO runtime_ack (
           event_id, resource, chain_id, idempotency_key,
           config_hash_before, config_hash_after,
           worker_id, layer, status, latency_ms, error
         ) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11)
         ON CONFLICT (event_id, layer) DO NOTHING
         RETURNING id`,
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
      inserted = (result.rowCount ?? 0) > 0;
      deduped = !inserted;
      await db.query("COMMIT");
    } catch (err) {
      await db.query("ROLLBACK").catch(() => {});
      throw err;
    }
    // OMEGA-7 PR-1: emit the ack into the runtime_ack WSS room AFTER the
    // INSERT has succeeded (invariant I-2 idempotencia POST-INSERT).
    //
    // OMEGA-8/M3 P1-1: do NOT re-broadcast on dedup — original INSERT already
    // emitted, and a duplicate emit would double-count the Prometheus
    // counters and trigger the frontend state machine twice. If we inserted
    // the row for the first time we emit; otherwise we skip silently and
    // return `deduped: true`.
    if (inserted) {
      try {
        broadcastRuntimeAck(io, p);
      } catch (err) {
        console.warn(
          '[system-manifest] broadcastRuntimeAck failed post-INSERT (ack persisted; emit skipped):',
          (err as Error).message,
        );
      }
    }
    res.status(202).json({ accepted: true, inserted, deduped });
  });

  // OMEGA-8/M3 P1-4: GET fallback for runtime_ack. The frontend hook
  // (useRuntimeAckSocket) escucha WSS; cuando un timeout vence sin recibir
  // ack — porque el WSS subscriber estaba reconectando, o porque el broadcast
  // de POST se silenció defensivamente — el hook consulta este endpoint para
  // recuperar el row persistido. Auth: admin token (operadores y herramientas
  // CLI tienen acceso; usuarios anonymous NO). Validamos UUID antes de tocar
  // la DB para no quemar pool capacity en payloads abusivos.
  r.get(
    "/runtime-ack/:event_id",
    deps.runtimeAckGetLimiter,
    deps.requireAdminToken,
    async (req: Request, res: Response) => {
      const eventId = req.params.event_id ?? "";
      const uuidRe = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i;
      if (!uuidRe.test(eventId)) {
        res.status(400).json({ error: "invalid_event_id" });
        return;
      }
      const q = await db.query(
        `SELECT event_id, resource, chain_id,
                config_hash_before, config_hash_after,
                worker_id, layer, status, latency_ms, error,
                received_at
           FROM runtime_ack
          WHERE event_id = $1
          ORDER BY received_at DESC`,
        [eventId],
      );
      if (q.rowCount === 0) {
        res.status(404).json({ error: "not_found", event_id: eventId });
        return;
      }
      // Note: idempotency_key intentionally omitted to avoid filtering
      // operational secrets to operators with read-only role.
      res.status(200).json({ event_id: eventId, acks: q.rows });
    },
  );

  return r;
}
