/**
 * admin-chains — Operator Admin Console: Chains CRUD (B1).
 *
 * Endpoints (all gated by V-AT-1 admin token):
 *
 *   GET    /api/v1/admin/chains
 *     List runtime chains (PG-backed). Optional `?enabled=true|false` filter.
 *
 *   GET    /api/v1/admin/chains/:chain_id
 *     Get single chain (404 if not in PG, even if exists in TOML — TOML is
 *     bootstrap-only after B1; PG is source of truth for runtime).
 *
 *   POST   /api/v1/admin/chains
 *     Create new chain (validates chain_id not duplicate). Computes
 *     config_hash and publishes `arbx:config:chains:reload` on success.
 *
 *   PUT    /api/v1/admin/chains/:chain_id
 *     Edit existing chain. Recomputes config_hash if any field changed; only
 *     publishes reload event when hash differs (deduplication).
 *
 *   DELETE /api/v1/admin/chains/:chain_id
 *     Soft-disable (enabled=false). NEVER physical DELETE — preserves history.
 *
 *   POST   /api/v1/admin/chains/:chain_id/probe
 *     Live RPC health probe: calls `eth_chainId` on the registered rpc_http_url,
 *     verifies returned chain id matches, measures latency. R8 fail-honest:
 *     returns full diagnostic even on failure; never silently passes.
 *
 * Precedence rule (B1 doctrine):
 *   - PG chains_runtime gains over TOML configs/app.toml [[chains]] seed.
 *   - TOML is read-only after B1 (bootstrap shim).
 *   - Conflict detection: api-server emits `config_drift` warning + Prometheus
 *     counter when TOML enabled flag differs from PG enabled flag.
 *
 * Doctrinal invariants preserved:
 *   - Live trading OFF — these endpoints DO NOT enable any submission path.
 *   - Capital $0 — no contract calls, no signer interaction.
 *   - Per-chain isolation — enabling/disabling chain X does NOT affect chain Y.
 *   - No mocks — if PG/Redis unavailable, returns 503 with diagnostic.
 *
 * Hot-reload (B1.b follow-up, not in this file):
 *   - searcher-rs subscribes to `arbx:config:chains:reload` channel.
 *   - On message, reads chains_runtime, rebuilds HashMap<u64, Arc<HttpRpcPool>>.
 *   - Uses tokio_util::sync::CancellationToken for graceful shutdown (30s timeout).
 *   - Idempotent via config_hash: same hash = no-op.
 */

import type { Application, Request, Response } from "express";
import type pg from "pg";
import type { Redis } from "ioredis";
import { createHash } from "node:crypto";
import { z } from "zod";

// ---------------------------------------------------------------------------
// Wire schemas (Zod) — explicit validation, no defaults inside Zod (each
// field declares its own permissive vs required semantics).
// ---------------------------------------------------------------------------

const ChainIdSchema = z.number().int().positive();
const RpcUrlHttpSchema = z.string().url().refine(
  (u) => u.startsWith("http://") || u.startsWith("https://"),
  { message: "rpc_http_url must start with http:// or https://" },
);
const RpcUrlWsSchema = z
  .string()
  .url()
  .refine((u) => u.startsWith("ws://") || u.startsWith("wss://"), {
    message: "rpc_ws_url must start with ws:// or wss://",
  });

const CreateChainSchema = z.object({
  chain_id: ChainIdSchema,
  name: z.string().min(1).max(64),
  rpc_http_url: RpcUrlHttpSchema,
  rpc_ws_url: RpcUrlWsSchema.optional(),
  native_currency: z.string().min(1).max(16).optional(),
  block_time_ms: z.number().int().positive().max(600_000).optional(), // ≤10 min
  enabled: z.boolean().optional(),
  notes: z.string().max(1024).optional(),
});

const UpdateChainSchema = z.object({
  name: z.string().min(1).max(64).optional(),
  rpc_http_url: RpcUrlHttpSchema.optional(),
  rpc_ws_url: RpcUrlWsSchema.optional(),
  native_currency: z.string().min(1).max(16).optional(),
  block_time_ms: z.number().int().positive().max(600_000).optional(),
  enabled: z.boolean().optional(),
  notes: z.string().max(1024).optional(),
});

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

interface ChainRow {
  id: number;
  chain_id: number;
  name: string;
  rpc_http_url: string;
  rpc_ws_url: string | null;
  native_currency: string;
  block_time_ms: number;
  enabled: boolean;
  config_hash: string | null;
  notes: string | null;
  created_at: string;
  updated_at: string;
  created_by: string | null;
  updated_by: string | null;
}

/**
 * Compute a deterministic SHA-256 of the canonical chain config payload.
 * Used by searcher-rs hot-reload subscriber to dedup no-op events.
 * Excludes id, created_at, updated_at, *_by fields (those don't affect runtime).
 */
function computeConfigHash(row: Omit<ChainRow, "id" | "config_hash" | "created_at" | "updated_at" | "created_by" | "updated_by">): string {
  const canonical = JSON.stringify({
    chain_id: row.chain_id,
    name: row.name,
    rpc_http_url: row.rpc_http_url,
    rpc_ws_url: row.rpc_ws_url,
    native_currency: row.native_currency,
    block_time_ms: row.block_time_ms,
    enabled: row.enabled,
  });
  return createHash("sha256").update(canonical).digest("hex");
}

function publishReload(
  redis: Redis,
  chainId: number,
  configHash: string,
  action: "created" | "updated" | "disabled",
): Promise<number> {
  // B1: pub/sub channel. Per-chain channel + global channel. searcher-rs
  // subscribes to the global; can ignore on config_hash match for dedup.
  const payload = JSON.stringify({
    action,
    chain_id: chainId,
    config_hash: configHash,
    timestamp: new Date().toISOString(),
  });
  return Promise.all([
    redis.publish("arbx:config:chains:reload", payload),
    redis.publish(`arbx:config:chains:${chainId}:reload`, payload),
  ]).then(([a, b]) => a + b);
}

// ---------------------------------------------------------------------------
// RPC probe — eth_chainId roundtrip with latency measurement
// ---------------------------------------------------------------------------

interface ProbeResult {
  chain_id: number;
  probed_rpc_url_redacted: string; // domain only, no path
  reachable: boolean;
  matches: boolean | null;
  observed_chain_id: number | null;
  latency_ms: number | null;
  block_number: number | null;
  error: string | null;
  probed_at: string;
}

async function probeRpc(rpcUrl: string, expectedChainId: number, timeoutMs = 5000): Promise<ProbeResult> {
  const now = new Date().toISOString();
  // CodeQL js/resource-exhaustion: clamp the caller-supplied timeout to a sane bound
  // so a large ?timeout_ms cannot hold an abort timer / connection open indefinitely.
  timeoutMs = Math.min(Math.max(timeoutMs, 500), 15_000);
  const u = new URL(rpcUrl);
  const redacted = `${u.protocol}//${u.hostname}/<path-redacted>`;

  const ctrl = new AbortController();
  const timer = setTimeout(() => ctrl.abort(), timeoutMs);
  const start = Date.now();

  try {
    // Batch request: eth_chainId + eth_blockNumber in one round-trip.
    const r = await fetch(rpcUrl, {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify([
        { jsonrpc: "2.0", id: 1, method: "eth_chainId", params: [] },
        { jsonrpc: "2.0", id: 2, method: "eth_blockNumber", params: [] },
      ]),
      signal: ctrl.signal,
    });
    const latency = Date.now() - start;
    if (!r.ok) {
      return {
        chain_id: expectedChainId,
        probed_rpc_url_redacted: redacted,
        reachable: false,
        matches: null,
        observed_chain_id: null,
        latency_ms: latency,
        block_number: null,
        error: `HTTP ${r.status}`,
        probed_at: now,
      };
    }
    const body = (await r.json()) as Array<{ id: number; result?: string; error?: { message: string } }>;
    const chainIdResp = body.find((x) => x.id === 1);
    const blockResp = body.find((x) => x.id === 2);
    const observedChainId =
      chainIdResp?.result ? parseInt(chainIdResp.result, 16) : null;
    const blockNumber = blockResp?.result ? parseInt(blockResp.result, 16) : null;
    if (chainIdResp?.error) {
      return {
        chain_id: expectedChainId,
        probed_rpc_url_redacted: redacted,
        reachable: true,
        matches: null,
        observed_chain_id: null,
        latency_ms: latency,
        block_number: blockNumber,
        error: `eth_chainId error: ${chainIdResp.error.message}`,
        probed_at: now,
      };
    }
    return {
      chain_id: expectedChainId,
      probed_rpc_url_redacted: redacted,
      reachable: true,
      matches: observedChainId === expectedChainId,
      observed_chain_id: observedChainId,
      latency_ms: latency,
      block_number: blockNumber,
      error: null,
      probed_at: now,
    };
  } catch (e) {
    const latency = Date.now() - start;
    const err = e as Error;
    return {
      chain_id: expectedChainId,
      probed_rpc_url_redacted: redacted,
      reachable: false,
      matches: null,
      observed_chain_id: null,
      latency_ms: latency,
      block_number: null,
      error: err.name === "AbortError" ? `timeout after ${timeoutMs}ms` : err.message,
      probed_at: now,
    };
  } finally {
    clearTimeout(timer);
  }
}

// ---------------------------------------------------------------------------
// Test-only exports
// ---------------------------------------------------------------------------

export const __forTesting = {
  computeConfigHash,
  CreateChainSchema,
  UpdateChainSchema,
};

// ---------------------------------------------------------------------------
// Route mounting
// ---------------------------------------------------------------------------

export function mountAdminChains(
  app: Application,
  deps: {
    pool: pg.Pool | null;
    redis: Redis | null;
    requireAdminToken: (token: string) => (req: Request, res: Response, next: () => void) => void;
    adminToken: string;
    writeAudit: (
      action: string,
      actor: string,
      kind: string,
      target: string,
      before: unknown,
      after: unknown,
      ip: string | null,
      traceId: string | null,
      ua: string | null,
    ) => Promise<void>;
    logger: { warn: (obj: object, msg?: string) => void; info: (obj: object, msg?: string) => void };
  },
): void {
  const { pool, redis, requireAdminToken, adminToken, writeAudit, logger } = deps;

  // GET list
  app.get("/api/v1/admin/chains", requireAdminToken(adminToken), async (req: Request, res: Response) => {
    if (!pool) { res.status(503).json({ error: "db_unavailable" }); return; }
    const enabledFilter = req.query["enabled"];
    let sql = `SELECT id, chain_id, name, rpc_http_url, rpc_ws_url, native_currency,
                      block_time_ms, enabled, config_hash, notes,
                      created_at, updated_at, created_by, updated_by
                 FROM chains_runtime`;
    const params: unknown[] = [];
    if (enabledFilter === "true" || enabledFilter === "false") {
      sql += ` WHERE enabled = $1`;
      params.push(enabledFilter === "true");
    }
    sql += ` ORDER BY chain_id ASC`;
    try {
      const q = await pool.query(sql, params);
      res.status(200).json({ count: q.rowCount, items: q.rows, ts: new Date().toISOString() });
    } catch (e) {
      logger.warn({ event: "admin_chains.list_failed", err: (e as Error).message });
      res.status(503).json({ error: "db_query_failed", detail: (e as Error).message });
    }
  });

  // GET single
  app.get("/api/v1/admin/chains/:chain_id", requireAdminToken(adminToken), async (req: Request, res: Response) => {
    if (!pool) { res.status(503).json({ error: "db_unavailable" }); return; }
    const cid = Number(req.params["chain_id"]);
    if (!Number.isInteger(cid) || cid < 1) { res.status(400).json({ error: "invalid_chain_id" }); return; }
    try {
      const q = await pool.query(`SELECT * FROM chains_runtime WHERE chain_id = $1`, [cid]);
      if (q.rowCount === 0) { res.status(404).json({ error: "chain_not_found", detail: `chain_id ${cid} not in chains_runtime; check TOML bootstrap if needed` }); return; }
      res.status(200).json(q.rows[0]);
    } catch (e) {
      res.status(503).json({ error: "db_query_failed", detail: (e as Error).message });
    }
  });

  // POST create
  app.post("/api/v1/admin/chains", requireAdminToken(adminToken), async (req: Request, res: Response) => {
    if (!pool) { res.status(503).json({ error: "db_unavailable" }); return; }
    if (!redis) { res.status(503).json({ error: "redis_unavailable", detail: "B1 requires Redis for hot-reload pub/sub" }); return; }
    const parsed = CreateChainSchema.safeParse(req.body);
    if (!parsed.success) { res.status(400).json({ error: "invalid_body", issues: parsed.error.issues }); return; }
    const data = parsed.data;
    const actor = req.header("x-arbx-actor") ?? "admin";
    const row: Omit<ChainRow, "id" | "config_hash" | "created_at" | "updated_at" | "created_by" | "updated_by"> = {
      chain_id: data.chain_id,
      name: data.name,
      rpc_http_url: data.rpc_http_url,
      rpc_ws_url: data.rpc_ws_url ?? null,
      native_currency: data.native_currency ?? "ETH",
      block_time_ms: data.block_time_ms ?? 12000,
      enabled: data.enabled ?? true,
      notes: data.notes ?? null,
    };
    const hash = computeConfigHash(row);
    try {
      const q = await pool.query(
        `INSERT INTO chains_runtime
           (chain_id, name, rpc_http_url, rpc_ws_url, native_currency,
            block_time_ms, enabled, config_hash, notes, created_by, updated_by)
         VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$10)
         RETURNING *`,
        [row.chain_id, row.name, row.rpc_http_url, row.rpc_ws_url, row.native_currency,
         row.block_time_ms, row.enabled, hash, row.notes, actor],
      );
      const created = q.rows[0];
      await publishReload(redis, row.chain_id, hash, "created");
      await writeAudit("admin.chain.create", actor, "chain", String(row.chain_id),
                       null, created, req.ip ?? null, (req as any).traceId ?? null,
                       req.header("user-agent") ?? null);
      logger.info({ event: "admin_chains.created", chain_id: row.chain_id, config_hash: hash });
      res.status(201).json(created);
    } catch (e) {
      const err = e as Error & { code?: string };
      if (err.code === "23505") {
        res.status(409).json({ error: "chain_id_already_exists", chain_id: data.chain_id });
        return;
      }
      logger.warn({ event: "admin_chains.create_failed", err: err.message });
      res.status(503).json({ error: "db_write_failed", detail: err.message });
    }
  });

  // PUT update
  app.put("/api/v1/admin/chains/:chain_id", requireAdminToken(adminToken), async (req: Request, res: Response) => {
    if (!pool) { res.status(503).json({ error: "db_unavailable" }); return; }
    if (!redis) { res.status(503).json({ error: "redis_unavailable" }); return; }
    const cid = Number(req.params["chain_id"]);
    if (!Number.isInteger(cid) || cid < 1) { res.status(400).json({ error: "invalid_chain_id" }); return; }
    const parsed = UpdateChainSchema.safeParse(req.body);
    if (!parsed.success) { res.status(400).json({ error: "invalid_body", issues: parsed.error.issues }); return; }
    const data = parsed.data;
    const actor = req.header("x-arbx-actor") ?? "admin";
    try {
      const before = await pool.query(`SELECT * FROM chains_runtime WHERE chain_id = $1`, [cid]);
      if (before.rowCount === 0) { res.status(404).json({ error: "chain_not_found" }); return; }
      const existing = before.rows[0];
      const merged = {
        chain_id: existing.chain_id,
        name: data.name ?? existing.name,
        rpc_http_url: data.rpc_http_url ?? existing.rpc_http_url,
        rpc_ws_url: data.rpc_ws_url ?? existing.rpc_ws_url,
        native_currency: data.native_currency ?? existing.native_currency,
        block_time_ms: data.block_time_ms ?? existing.block_time_ms,
        enabled: data.enabled ?? existing.enabled,
        notes: data.notes ?? existing.notes,
      };
      const newHash = computeConfigHash(merged);
      const hashChanged = newHash !== existing.config_hash;
      const q = await pool.query(
        `UPDATE chains_runtime
            SET name = $2, rpc_http_url = $3, rpc_ws_url = $4,
                native_currency = $5, block_time_ms = $6, enabled = $7,
                config_hash = $8, notes = $9, updated_at = NOW(), updated_by = $10
          WHERE chain_id = $1
          RETURNING *`,
        [cid, merged.name, merged.rpc_http_url, merged.rpc_ws_url, merged.native_currency,
         merged.block_time_ms, merged.enabled, newHash, merged.notes, actor],
      );
      const updated = q.rows[0];
      if (hashChanged) {
        await publishReload(redis, cid, newHash, "updated");
        logger.info({ event: "admin_chains.updated_hash_changed", chain_id: cid, old_hash: existing.config_hash, new_hash: newHash });
      } else {
        logger.info({ event: "admin_chains.updated_no_op", chain_id: cid, reason: "config_hash_unchanged" });
      }
      await writeAudit("admin.chain.update", actor, "chain", String(cid), existing, updated,
                       req.ip ?? null, (req as any).traceId ?? null, req.header("user-agent") ?? null);
      res.status(200).json({ ...updated, hash_changed: hashChanged });
    } catch (e) {
      logger.warn({ event: "admin_chains.update_failed", err: (e as Error).message });
      res.status(503).json({ error: "db_write_failed", detail: (e as Error).message });
    }
  });

  // DELETE soft-disable
  app.delete("/api/v1/admin/chains/:chain_id", requireAdminToken(adminToken), async (req: Request, res: Response) => {
    if (!pool) { res.status(503).json({ error: "db_unavailable" }); return; }
    if (!redis) { res.status(503).json({ error: "redis_unavailable" }); return; }
    const cid = Number(req.params["chain_id"]);
    if (!Number.isInteger(cid) || cid < 1) { res.status(400).json({ error: "invalid_chain_id" }); return; }
    const actor = req.header("x-arbx-actor") ?? "admin";
    try {
      const before = await pool.query(`SELECT * FROM chains_runtime WHERE chain_id = $1`, [cid]);
      if (before.rowCount === 0) { res.status(404).json({ error: "chain_not_found" }); return; }
      const existing = before.rows[0];
      if (!existing.enabled) {
        res.status(200).json({ ...existing, already_disabled: true });
        return;
      }
      const merged = {
        chain_id: existing.chain_id,
        name: existing.name,
        rpc_http_url: existing.rpc_http_url,
        rpc_ws_url: existing.rpc_ws_url,
        native_currency: existing.native_currency,
        block_time_ms: existing.block_time_ms,
        enabled: false,
        notes: existing.notes,
      };
      const newHash = computeConfigHash(merged);
      const q = await pool.query(
        `UPDATE chains_runtime SET enabled = FALSE, config_hash = $2, updated_at = NOW(), updated_by = $3
          WHERE chain_id = $1 RETURNING *`,
        [cid, newHash, actor],
      );
      await publishReload(redis, cid, newHash, "disabled");
      await writeAudit("admin.chain.disable", actor, "chain", String(cid), existing, q.rows[0],
                       req.ip ?? null, (req as any).traceId ?? null, req.header("user-agent") ?? null);
      logger.info({ event: "admin_chains.disabled", chain_id: cid });
      res.status(200).json(q.rows[0]);
    } catch (e) {
      res.status(503).json({ error: "db_write_failed", detail: (e as Error).message });
    }
  });

  // POST probe — live RPC eth_chainId roundtrip
  app.post("/api/v1/admin/chains/:chain_id/probe", requireAdminToken(adminToken), async (req: Request, res: Response) => {
    if (!pool) { res.status(503).json({ error: "db_unavailable" }); return; }
    const cid = Number(req.params["chain_id"]);
    if (!Number.isInteger(cid) || cid < 1) { res.status(400).json({ error: "invalid_chain_id" }); return; }
    // Reject an out-of-range caller-supplied timeout instead of silently clamping it.
    // CodeQL js/resource-exhaustion models a *rejecting guard* (an early-return on a
    // range check) as a sanitizer barrier; a Math.min/Math.max clamp is NOT recognized,
    // which is why the prior clamp left the setTimeout sink inside probeRpc still flagged.
    const rawTimeoutMs = Number(req.query["timeout_ms"] ?? 5000);
    if (!Number.isInteger(rawTimeoutMs) || rawTimeoutMs < 1000 || rawTimeoutMs > 30000) {
      res.status(400).json({ error: "invalid_timeout_ms", allowed: "integer 1000..30000" }); return;
    }
    const timeoutMs = rawTimeoutMs;
    try {
      const q = await pool.query(`SELECT rpc_http_url FROM chains_runtime WHERE chain_id = $1`, [cid]);
      if (q.rowCount === 0) { res.status(404).json({ error: "chain_not_found" }); return; }
      const result = await probeRpc(q.rows[0].rpc_http_url, cid, timeoutMs);
      res.status(200).json(result);
    } catch (e) {
      res.status(503).json({ error: "probe_failed", detail: (e as Error).message });
    }
  });
}
