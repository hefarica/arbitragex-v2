import express from "express";
import pg from "pg";
import { Redis } from "ioredis";
import { z } from "zod";
import { clampBucketMinutes, clampHours, rowToPoint } from "./recon-timeseries.js";
import { verifyAll } from "./readiness/verifiers/index.js";
import type { ReadinessReport } from "./readiness/types.js";
import {
  loadAppConfig,
  createHttpLogger,
  createLogger,
  healthHandler,
  metricsHandler,
  metricsMiddleware,
  traceIdMiddleware,
  requireEnv,
  requireAdminToken,
  requireEdgeToken,
  KillSwitchClient,
  initMetrics,
} from "@arbx/shared";

const SERVICE = "api-server";
const VERSION = "0.1.0";

const cfg = loadAppConfig();
const logger = createLogger({ service: SERVICE, level: cfg.observability.log_level ?? "info" });
initMetrics(SERVICE);

const REDIS_URL = requireEnv("REDIS_URL");
const ARBX_ADMIN_TOKEN = requireEnv("ARBX_ADMIN_TOKEN");
const ARBX_EDGE_TOKEN = requireEnv("ARBX_EDGE_TOKEN");

const killSwitch = new KillSwitchClient({
  redisUrl: REDIS_URL,
  defaultWhenAbsent: cfg.system.kill_switch_enabled_default,
});
killSwitch.subscribeChanges().catch((e: Error) => {
  logger.warn({ err: e.message }, "killswitch pub/sub subscribe failed — will fall back to TTL poll");
});

const UPSTREAMS = {
  "selector-api": process.env["SELECTOR_URL"] ?? "http://selector-api:3002",
  "sim-ctl":      process.env["SIM_URL"]       ?? "http://sim-ctl:3003",
  "recon":        process.env["RECON_URL"]     ?? "http://recon:3004",
  "relays-client":process.env["RELAYS_URL"]    ?? "http://relays-client:3005",
  "searcher-rs":  process.env["SEARCHER_URL"]  ?? "http://searcher-rs:9001",
} as const;

async function pingUpstream(name: string, url: string): Promise<{ ok: boolean; status?: number; detail?: string }> {
  try {
    const controller = new AbortController();
    const to = setTimeout(() => controller.abort(), 1500);
    const r = await fetch(`${url}/health`, { signal: controller.signal });
    clearTimeout(to);
    return { ok: r.ok, status: r.status };
  } catch (e) {
    return { ok: false, detail: (e as Error).message };
  }
}

const startedAt = new Date();
const app = express();

// ==========================================
// IMPORT DEFI ROUTER & WEBSOCKET
// ==========================================
import { mountDefi } from "./routes/defi.js";
import { buildTradingConfigRouter } from "./routes/trading-config.js";
import { buildOperationsRouter } from "./routes/operations.js";
import { buildStrategyCatalogRouter } from "./routes/strategy-catalog.js";
import { mountOpportunitiesLive } from "./routes/opportunities-live.js";
import { mountDexes } from "./routes/dexes.js";
import { mountPools } from "./routes/pools.js";
import { setupWebSocketGateway, broadcastOpportunity } from "./websocket.js";
import { createServer } from "http";

// defi routes mounted later (after `pool` and `logger` are constructed). See mountDefi() below.

app.disable("x-powered-by");
app.use(express.json({ limit: "256kb" }));
app.use(traceIdMiddleware());
app.use(createHttpLogger(SERVICE, cfg.observability.log_level ?? "info"));
app.use(metricsMiddleware(SERVICE));

app.get("/health", healthHandler(SERVICE, VERSION, startedAt));
// REST convention alias — load balancers / external monitors expect /api/health.
// Same handler, no behaviour drift.
app.get("/api/health", healthHandler(SERVICE, VERSION, startedAt));
app.get("/metrics", metricsHandler);

/** Public read-only snapshot of system health + kill-switch state. */
app.get("/status", async (_req, res) => {
  const probes = await Promise.all(
    Object.entries(UPSTREAMS).map(async ([name, url]) => [name, await pingUpstream(name, url)] as const)
  );
  const ks = await killSwitch.state().catch(() => null);
  res.status(200).json({
    ok: probes.every(([, r]) => r.ok),
    services: Object.fromEntries(probes),
    killswitch: ks,
    env: cfg.system.env,
    version: VERSION,
    ts: new Date().toISOString(),
  });
});

/** Admin: toggle kill-switch. Requires admin token. */
const KillSwitchReq = z.object({
  enabled: z.boolean(),
  reason: z.string().max(500).optional(),
  triggered_by: z.string().max(200).optional(),
});
app.post("/admin/killswitch", requireAdminToken(ARBX_ADMIN_TOKEN), async (req, res) => {
  const parsed = KillSwitchReq.safeParse(req.body);
  if (!parsed.success) {
    res.status(400).json({ error: "invalid_request", details: parsed.error.flatten() });
    return;
  }
  const actorHeader = req.header("x-arbx-actor") ?? "admin";
  // Capture before-state for audit trail.
  const beforeState = await killSwitch.state().catch(() => null);
  const out = await killSwitch.set({
    enabled: parsed.data.enabled,
    reason: parsed.data.reason ?? null,
    triggered_by: parsed.data.triggered_by ?? actorHeader,
  });
  const action = parsed.data.enabled ? "killswitch.armed" : "killswitch.disabled";
  const ip = req.ip ?? req.socket.remoteAddress ?? null;
  const traceId = (req as express.Request & { traceId?: string }).traceId ?? null;
  await writeAudit(
    action, actorHeader, "killswitch", "global",
    beforeState, { enabled: parsed.data.enabled, reason: parsed.data.reason ?? null },
    ip, traceId,
  );
  logger.warn({ event: "admin.killswitch", actor: actorHeader, state: out }, "kill-switch toggled");
  res.status(200).json(out);
});

/** Admin: read current effective config (secrets never included). */
app.get("/admin/config", requireAdminToken(ARBX_ADMIN_TOKEN), (_req, res) => {
  res.status(200).json({
    system: cfg.system,
    risk: cfg.risk,
    execution: cfg.execution,
    observability: cfg.observability,
    chains: cfg.chains,
    relays: cfg.relays,
    scoring: cfg.scoring,
    token_safety: cfg.token_safety,
    circuit_breakers: cfg.circuit_breakers,
  });
});

// ─── PR-2: Internal audit endpoint for edge auth events ───
const AuditAuthBody = z.object({
  action: z.enum([
    "auth.login_ok", "auth.login_fail", "auth.logout",
    "auth.lockout_triggered", "auth.rate_limited", "auth.locked_attempt",
  ]),
  actor: z.string().min(1).max(64),
  target_kind: z.literal("ip"),
  target_id: z.string().min(1).max(64),
  ip_address: z.string().min(1).max(64),
  user_agent: z.string().max(512).optional(),
  trace_id: z.string().uuid().optional(),
  after_state: z.record(z.unknown()).optional(),
});

app.post("/internal/audit/auth", requireEdgeToken(ARBX_EDGE_TOKEN), async (req, res) => {
  const parsed = AuditAuthBody.safeParse(req.body);
  if (!parsed.success) {
    res.status(400).json({ error: "invalid_request", details: parsed.error.flatten() });
    return;
  }
  await writeAudit(
    parsed.data.action,
    parsed.data.actor,
    parsed.data.target_kind,
    parsed.data.target_id,
    null,
    parsed.data.after_state ?? null,
    parsed.data.ip_address,
    parsed.data.trace_id ?? null,
  );
  res.status(204).end();
});

// ─────── Sprint 3: admin endpoints for blacklist + circuit breakers ───────

const DATABASE_URL = process.env["DATABASE_URL"] ?? "";
const pool = DATABASE_URL
  ? new pg.Pool({ connectionString: DATABASE_URL, max: 3, idleTimeoutMillis: 30_000 })
  : null;
const redis = new Redis(REDIS_URL, { lazyConnect: false, maxRetriesPerRequest: 3 });

function normAddr(a: string): string {
  const s = a.trim().toLowerCase();
  if (!/^0x[0-9a-f]{40}$/.test(s)) throw new Error("invalid_address");
  return s;
}

async function writeAudit(action: string, actor: string, targetKind: string | null, targetId: string | null, before: unknown, after: unknown, ip: string | null, traceId: string | null): Promise<void> {
  if (!pool) return;
  try {
    await pool.query(
      `INSERT INTO audit_log (actor, action, target_kind, target_id, before_state, after_state, ip_address, trace_id)
       VALUES ($1,$2,$3,$4,$5::jsonb,$6::jsonb,$7::inet,$8)`,
      [actor, action, targetKind, targetId, JSON.stringify(before), JSON.stringify(after), ip, traceId],
    );
    // auditEventsTotal.labels({ action }).inc();
  } catch (e) {
    logger.warn({ event: "audit.write_failed", err: (e as Error).message });
  }
}

const BlacklistAdd = z.object({
  chain_id: z.number().int().positive(),
  token_address: z.string().min(42).max(42),
  reason: z.string().max(500).optional(),
});
app.post("/admin/blacklist/tokens", requireAdminToken(ARBX_ADMIN_TOKEN), async (req, res) => {
  const parsed = BlacklistAdd.safeParse(req.body);
  if (!parsed.success) { res.status(400).json({ error: "invalid_request", details: parsed.error.flatten() }); return; }
  let addr: string;
  try { addr = normAddr(parsed.data.token_address); }
  catch { res.status(400).json({ error: "invalid_address" }); return; }
  const key = `arbx:blacklist:tokens:${parsed.data.chain_id}`;
  await redis.sadd(key, addr);
  const actor = req.header("x-arbx-actor") ?? "admin";
  const ip = req.ip ?? null;
  const traceId = (req as express.Request & { traceId?: string }).traceId ?? null;
  await writeAudit("blacklist.add", actor, "token", `${parsed.data.chain_id}:${addr}`, null, parsed.data, ip, traceId);
  logger.warn({ event: "admin.blacklist.add", addr, chain_id: parsed.data.chain_id, actor });
  res.status(201).json({ ok: true, chain_id: parsed.data.chain_id, token_address: addr });
});

app.delete("/admin/blacklist/tokens/:chain/:addr", requireAdminToken(ARBX_ADMIN_TOKEN), async (req, res) => {
  const chain = Number(req.params.chain);
  let addr: string;
  try { addr = normAddr(req.params.addr!); } catch { res.status(400).json({ error: "invalid_address" }); return; }
  if (!Number.isFinite(chain) || chain < 1) { res.status(400).json({ error: "invalid_chain" }); return; }
  const removed = await redis.srem(`arbx:blacklist:tokens:${chain}`, addr);
  const actor = req.header("x-arbx-actor") ?? "admin";
  await writeAudit("blacklist.remove", actor, "token", `${chain}:${addr}`, { chain_id: chain, token_address: addr }, { removed }, req.ip ?? null, (req as any).traceId ?? null);
  res.status(200).json({ ok: true, removed });
});

app.get("/admin/blacklist/tokens", requireAdminToken(ARBX_ADMIN_TOKEN), async (req, res) => {
  const chain = Number(req.query["chain_id"] ?? 1);
  if (!Number.isFinite(chain) || chain < 1) { res.status(400).json({ error: "invalid_chain" }); return; }
  const members = await redis.smembers(`arbx:blacklist:tokens:${chain}`);
  res.status(200).json({ chain_id: chain, count: members.length, addresses: members });
});

/** Circuit breaker admin proxied via Redis pub/sub (any service subscribed reacts). */
const CB_CHANNEL = "arbx:cb:admin";
app.get("/admin/circuit_breakers", requireAdminToken(ARBX_ADMIN_TOKEN), async (_req, res) => {
  // We don't have direct access to the running CB instances; state is reported
  // by selector-api's /metrics. Return the configured CB list + a hint.
  res.status(200).json({
    configured: cfg.circuit_breakers,
    note: "Live state available at selector-api:/metrics (arbx_cb_state{name=...}). Use /admin/circuit_breakers/:name/{reset,trip} to command.",
    admin_channel: CB_CHANNEL,
  });
});

const CbTrip = z.object({ reason: z.string().max(200) });
app.post("/admin/circuit_breakers/:name/trip", requireAdminToken(ARBX_ADMIN_TOKEN), async (req, res) => {
  const parsed = CbTrip.safeParse(req.body ?? {});
  if (!parsed.success) { res.status(400).json({ error: "invalid_request" }); return; }
  const msg = JSON.stringify({ action: "trip", name: req.params.name, reason: parsed.data.reason });
  const delivered = await redis.publish(CB_CHANNEL, msg);
  await writeAudit("cb.trip", req.header("x-arbx-actor") ?? "admin", "circuit_breaker", req.params.name ?? "", null, parsed.data, req.ip ?? null, (req as any).traceId ?? null);
  res.status(202).json({ ok: true, published_to_channel: CB_CHANNEL, subscribers_notified: delivered });
});

app.post("/admin/circuit_breakers/:name/reset", requireAdminToken(ARBX_ADMIN_TOKEN), async (req, res) => {
  const msg = JSON.stringify({ action: "reset", name: req.params.name });
  const delivered = await redis.publish(CB_CHANNEL, msg);
  await writeAudit("cb.reset", req.header("x-arbx-actor") ?? "admin", "circuit_breaker", req.params.name ?? "", null, null, req.ip ?? null, (req as any).traceId ?? null);
  res.status(202).json({ ok: true, published_to_channel: CB_CHANNEL, subscribers_notified: delivered });
});

app.get("/admin/scoring/weights", requireAdminToken(ARBX_ADMIN_TOKEN), (_req, res) => {
  res.status(200).json(cfg.scoring);
});

const AuditLogFilterSchema = z.object({
  action: z.string().optional(),
  actor: z.string().optional(),
  target_kind: z.string().optional(),
  limit: z.coerce.number().int().min(1).max(200).default(50),
  cursor: z.string().optional(), // Expected to be an ISO timestamp for pagination
});

app.get("/admin/audit", requireAdminToken(ARBX_ADMIN_TOKEN), async (req, res) => {
  const p = requireDbPool();
  if (!p) { res.status(503).json({ error: "db_unavailable" }); return; }
  
  const parsed = AuditLogFilterSchema.safeParse(req.query);
  if (!parsed.success) {
    res.status(400).json({ error: "invalid_request", details: parsed.error.flatten() });
    return;
  }
  
  const { action, actor, target_kind, limit, cursor } = parsed.data;
  
  const filters: string[] = [];
  const params: any[] = [];
  let paramIdx = 1;
  
  if (action) { filters.push(`action = $${paramIdx++}`); params.push(action); }
  if (actor) { filters.push(`actor = $${paramIdx++}`); params.push(actor); }
  if (target_kind) { filters.push(`target_kind = $${paramIdx++}`); params.push(target_kind); }
  if (cursor) {
    filters.push(`created_at < $${paramIdx++}`);
    params.push(cursor);
  }
  
  const whereClause = filters.length > 0 ? `WHERE ${filters.join(" AND ")}` : "";
  
  // Also get total count (approx or exact if fast)
  // For simplicity we just return the paginated rows
  const query = `
    SELECT id, actor, action, target_kind, target_id, before_state, after_state,
           ip_address, user_agent, trace_id, created_at
      FROM audit_log
     ${whereClause}
     ORDER BY created_at DESC
     LIMIT $${paramIdx}
  `;
  params.push(limit);
  
  try {
    const q = await p.query(query, params);
    const nextCursor = q.rows.length === limit ? q.rows[q.rows.length - 1].created_at.toISOString() : null;
    
    res.status(200).json({
      items: q.rows,
      next_cursor: nextCursor,
      ts: new Date().toISOString()
    });
  } catch (e) {
    logger.warn({ event: "audit.read_failed", err: (e as Error).message });
    res.status(500).json({ error: "query_failed", detail: (e as Error).message });
  }
});

// ─────── Sprint 7: public v1 read endpoints consumed by frontend + edge ───────
//
// Contract: every endpoint below must return HTTP 503 with { error: "db_unavailable" }
// when the pool is null or a query fails. NEVER synthesize data.

function requireDbPool(): pg.Pool | null {
  return pool;
}

mountOpportunitiesLive(app, pool, logger);
mountDexes(app, { pool, logger });
mountPools(app, { pool, logger });
mountDefi(app, { pool, logger });

// Scanner heartbeat snapshot — read latest pipeline counters from Redis.
// Persisted by searcher-rs::workers::heartbeat_worker every period (default
// 60s) with TTL = 3× period. 404 when key absent → searcher down OR very
// recent restart (R8 fail-honest: surface the gap, don't fabricate zeros).
app.get("/api/v1/scanner/heartbeat", async (req, res) => {
  const chainId = Number(req.query["chain_id"] ?? 1);
  if (!Number.isFinite(chainId) || chainId < 1) {
    res.status(400).json({ error: "invalid_chain_id" });
    return;
  }
  const key = `arbx:heartbeat:scanner:${chainId}:latest`;
  try {
    const raw = await redis.get(key);
    if (raw == null) {
      res.status(404).json({
        error: "heartbeat_not_available",
        detail: `no snapshot at ${key} — searcher may be down or recently restarted`,
        chain_id: chainId,
      });
      return;
    }
    let parsed: unknown;
    try {
      parsed = JSON.parse(raw);
    } catch (e) {
      res.status(503).json({ error: "snapshot_parse_failed", detail: (e as Error).message });
      return;
    }
    res.status(200).json({ chain_id: chainId, snapshot: parsed, fetched_at: new Date().toISOString() });
  } catch (e) {
    logger.warn({ event: "scanner.heartbeat.read_failed", err: (e as Error).message });
    res.status(503).json({ error: "redis_read_failed", detail: (e as Error).message });
  }
});

app.get("/api/v1/risk/alerts", async (req, res) => {
  const p = requireDbPool();
  if (!p) { res.status(503).json({ error: "db_unavailable" }); return; }
  const hours = Math.max(1, Math.min(168, Number(req.query["hours"] ?? 24)));
  try {
    const q = await p.query(
      `SELECT id, event_type, severity, source_service, payload,
              trace_id, opportunity_id, created_at
         FROM risk_events
        WHERE severity IN ('warning','critical')
          AND created_at >= NOW() - ($1::text || ' hours')::interval
        ORDER BY created_at DESC
        LIMIT 500`, [String(hours)],
    );
    const ks = await killSwitch.state().catch(() => null);
    res.status(200).json({
      window_hours: hours,
      killswitch: ks,
      alerts: q.rows,
      ts: new Date().toISOString(),
    });
  } catch (e) {
    logger.warn({ event: "risk.alerts.query_failed", err: (e as Error).message });
    res.status(503).json({ error: "query_failed", detail: (e as Error).message });
  }
});

app.get("/api/v1/executions/recent", async (req, res) => {
  const p = requireDbPool();
  if (!p) { res.status(503).json({ error: "db_unavailable" }); return; }
  const limit = Math.max(1, Math.min(500, Number(req.query["limit"] ?? 50)));
  try {
    const q = await p.query(
      `SELECT e.id, e.tx_hash, e.bundle_hash, e.relay_name, e.status,
              e.block_included,
              e.gas_used_wei::text         AS gas_used_wei,
              e.gas_price_effective_wei::text AS gas_price_effective_wei,
              e.expected_profit_usd::float AS expected_profit_usd,
              e.actual_profit_usd::float   AS actual_profit_usd,
              e.error_message, e.trace_id, e.submitted_at, e.confirmed_at,
              o.chain_id, o.strategy_kind, o.pair_symbol
         FROM executions e
         JOIN opportunities o ON o.id = e.opportunity_id
        ORDER BY e.submitted_at DESC
        LIMIT $1`, [limit],
    );
    res.status(200).json({
      count: q.rows.length,
      items: q.rows,
      ts: new Date().toISOString(),
    });
  } catch (e) {
    logger.warn({ event: "executions.recent.query_failed", err: (e as Error).message });
    res.status(503).json({ error: "query_failed", detail: (e as Error).message });
  }
});

app.get("/api/v1/recon/summary", async (req, res) => {
  const p = requireDbPool();
  if (!p) { res.status(503).json({ error: "db_unavailable" }); return; }
  const hours = Math.max(1, Math.min(168, Number(req.query["hours"] ?? 1)));
  try {
    const [agg, top, anomalies] = await Promise.all([
      p.query(
        `SELECT COUNT(*)::int AS total,
                SUM(CASE WHEN status='included' THEN 1 ELSE 0 END)::int AS included,
                SUM(CASE WHEN status='reverted' THEN 1 ELSE 0 END)::int AS reverted,
                SUM(CASE WHEN status='dropped'  THEN 1 ELSE 0 END)::int AS dropped,
                AVG(CASE WHEN status='included' AND actual_profit_usd IS NOT NULL
                         THEN actual_profit_usd END)::float           AS avg_pnl_included_usd,
                AVG(CASE WHEN confirmed_at IS NOT NULL
                         THEN EXTRACT(EPOCH FROM (confirmed_at - submitted_at)) * 1000.0 END)::float
                                                                       AS avg_confirm_latency_ms
           FROM executions
          WHERE submitted_at >= NOW() - ($1::text || ' hours')::interval`, [String(hours)],
      ),
      p.query(
        `SELECT strategy_kind, chain_id, sample_count,
                success_rate::float AS success_rate,
                revert_rate::float  AS revert_rate,
                avg_profit_usd::float AS avg_profit_usd,
                score::float AS score,
                window_end
           FROM strategy_scores
          WHERE window_end >= NOW() - ($1::text || ' hours')::interval
          ORDER BY score DESC NULLS LAST, window_end DESC
          LIMIT 10`, [String(Math.max(24, hours))],
      ),
      p.query(
        `SELECT event_type, severity, source_service, payload, created_at
           FROM risk_events
          WHERE severity = 'critical'
            AND created_at >= NOW() - INTERVAL '24 hours'
          ORDER BY created_at DESC
          LIMIT 20`,
      ),
    ]);
    const row = agg.rows[0] ?? { total: 0, included: 0, reverted: 0, dropped: 0,
                                  avg_pnl_included_usd: null, avg_confirm_latency_ms: null };
    const revertRate = row.total > 0 ? (row.reverted / row.total) : null;
    res.status(200).json({
      window_hours: hours,
      totals: row,
      revert_rate: revertRate,
      top_strategies: top.rows,
      critical_anomalies_24h: anomalies.rows,
      ts: new Date().toISOString(),
    });
  } catch (e) {
    logger.warn({ event: "recon.summary.query_failed", err: (e as Error).message });
    res.status(503).json({ error: "query_failed", detail: (e as Error).message });
  }
});

// Timeseries of realised PnL / attempts / revert-rate bucketed over time.
// Source: executions table (same as /recon/summary). Backfills empty buckets
// with zeros so the chart never shows a hole — that matches the "honesty
// contract": if there were no attempts in a bucket, we say zero explicitly.
app.get("/api/v1/recon/timeseries", async (req, res) => {
  const p = requireDbPool();
  if (!p) { res.status(503).json({ error: "db_unavailable" }); return; }
  const hours = clampHours(req.query["hours"]);
  const bucketMinutes = clampBucketMinutes(req.query["bucket_minutes"]);
  try {
    const q = await p.query(
      `WITH buckets AS (
         SELECT generate_series(
                  date_bin(($1::text || ' minutes')::interval,
                           NOW() - ($2::text || ' hours')::interval,
                           TIMESTAMP 'epoch'),
                  date_bin(($1::text || ' minutes')::interval, NOW(), TIMESTAMP 'epoch'),
                  ($1::text || ' minutes')::interval
                ) AS bucket_start
       ),
       agg AS (
         SELECT date_bin(($1::text || ' minutes')::interval, submitted_at, TIMESTAMP 'epoch') AS bucket_start,
                COUNT(*)::int AS attempts,
                SUM(CASE WHEN status='included' THEN 1 ELSE 0 END)::int AS included,
                SUM(CASE WHEN status='reverted' THEN 1 ELSE 0 END)::int AS reverted,
                AVG(CASE WHEN status='included' AND actual_profit_usd IS NOT NULL
                         THEN actual_profit_usd END)::float AS avg_pnl_included_usd
           FROM executions
          WHERE submitted_at >= NOW() - ($2::text || ' hours')::interval
          GROUP BY 1
       )
       SELECT b.bucket_start,
              COALESCE(a.attempts, 0)::int AS attempts,
              COALESCE(a.included, 0)::int AS included,
              COALESCE(a.reverted, 0)::int AS reverted,
              a.avg_pnl_included_usd
         FROM buckets b
         LEFT JOIN agg a USING (bucket_start)
         ORDER BY b.bucket_start ASC`,
      [String(bucketMinutes), String(hours)],
    );
    const points = q.rows.map(rowToPoint);
    res.status(200).json({
      window_hours: hours,
      bucket_minutes: bucketMinutes,
      points,
      ts: new Date().toISOString(),
    });
  } catch (e) {
    logger.warn({ event: "recon.timeseries.query_failed", err: (e as Error).message });
    res.status(503).json({ error: "query_failed", detail: (e as Error).message });
  }
});

// ─────── Sprint 7 / Phase 0.5 — relay catalog CRUD (admin) ───────
//
// Relays live in the DB (migration 013). Hot-path services will migrate to
// query this table directly in a follow-up PR; for now api-server is the
// single writer.

const RelayCreate = z.object({
  name: z.string().min(1).max(64),
  chain_id: z.number().int().positive(),
  endpoint: z.string().min(8).optional(),
  auth_scheme: z.enum(["none","x-flashbots-signature","bearer","header-auth","custom"]).default("none"),
  auth_secret_ref: z.string().max(512).optional(),
  enabled: z.boolean().default(false),
  priority: z.number().int().min(0).max(1000).default(100),
  notes: z.string().max(500).optional(),
});
const RelayUpdate = RelayCreate.partial();

app.get("/admin/relays", requireAdminToken(ARBX_ADMIN_TOKEN), async (_req, res) => {
  const p = requireDbPool();
  if (!p) { res.status(503).json({ error: "db_unavailable" }); return; }
  const q = await p.query(
    `SELECT id, name, chain_id, endpoint, auth_scheme, auth_secret_ref,
            enabled, priority, notes, created_by, created_at, updated_at
       FROM relays ORDER BY chain_id, priority, name`,
  );
  res.status(200).json({ count: q.rows.length, items: q.rows });
});

app.post("/admin/relays", requireAdminToken(ARBX_ADMIN_TOKEN), async (req, res) => {
  const p = requireDbPool();
  if (!p) { res.status(503).json({ error: "db_unavailable" }); return; }
  const parsed = RelayCreate.safeParse(req.body);
  if (!parsed.success) { res.status(400).json({ error: "invalid_request", details: parsed.error.flatten() }); return; }
  const b = parsed.data;
  if (b.enabled && (!b.endpoint || b.endpoint.length < 8)) {
    res.status(400).json({ error: "enabled_requires_endpoint" });
    return;
  }
  const actor = req.header("x-arbx-actor") ?? "admin";
  try {
    const q = await p.query(
      `INSERT INTO relays (name, chain_id, endpoint, auth_scheme, auth_secret_ref, enabled, priority, notes, created_by)
       VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9)
       RETURNING id, name, chain_id, endpoint, auth_scheme, enabled, priority, notes, created_at`,
      [b.name, b.chain_id, b.endpoint ?? null, b.auth_scheme, b.auth_secret_ref ?? null,
       b.enabled, b.priority, b.notes ?? null, actor],
    );
    await writeAudit("relay.create", actor, "relay", q.rows[0].id, null, b,
                     req.ip ?? null, (req as any).traceId ?? null);
    res.status(201).json(q.rows[0]);
  } catch (e) {
    const msg = (e as Error).message;
    if (msg.includes("relays_uq_name_chain")) { res.status(409).json({ error: "duplicate_relay" }); return; }
    logger.warn({ event: "relay.create.failed", err: msg });
    res.status(500).json({ error: "db_error", detail: msg });
  }
});

app.put("/admin/relays/:id", requireAdminToken(ARBX_ADMIN_TOKEN), async (req, res) => {
  const p = requireDbPool();
  if (!p) { res.status(503).json({ error: "db_unavailable" }); return; }
  const parsed = RelayUpdate.safeParse(req.body);
  if (!parsed.success) { res.status(400).json({ error: "invalid_request", details: parsed.error.flatten() }); return; }
  const before = await p.query(`SELECT * FROM relays WHERE id = $1`, [req.params.id]);
  if (before.rowCount === 0) { res.status(404).json({ error: "not_found" }); return; }
  const existing = before.rows[0];
  const merged = { ...existing, ...parsed.data };
  if (merged.enabled && (!merged.endpoint || merged.endpoint.length < 8)) {
    res.status(400).json({ error: "enabled_requires_endpoint" });
    return;
  }
  const actor = req.header("x-arbx-actor") ?? "admin";
  const q = await p.query(
    `UPDATE relays
        SET endpoint        = COALESCE($2, endpoint),
            auth_scheme     = COALESCE($3, auth_scheme),
            auth_secret_ref = COALESCE($4, auth_secret_ref),
            enabled         = COALESCE($5, enabled),
            priority        = COALESCE($6, priority),
            notes           = COALESCE($7, notes)
      WHERE id = $1
  RETURNING id, name, chain_id, endpoint, auth_scheme, enabled, priority, notes, updated_at`,
    [req.params.id,
     parsed.data.endpoint ?? null,
     parsed.data.auth_scheme ?? null,
     parsed.data.auth_secret_ref ?? null,
     parsed.data.enabled ?? null,
     parsed.data.priority ?? null,
     parsed.data.notes ?? null],
  );
  await writeAudit("relay.update", actor, "relay", req.params.id ?? "",
                   existing, parsed.data, req.ip ?? null, (req as any).traceId ?? null);
});

app.post("/admin/config/paper-mode", requireAdminToken(ARBX_ADMIN_TOKEN), async (req, res) => {
  const actor = req.header("x-arbx-actor") ?? "admin";
  const { enabled, updated_by } = req.body;
  if (typeof enabled !== "boolean") {
    res.status(400).json({ error: "invalid_body", detail: "enabled must be boolean" });
    return;
  }
  try {
    const rc = redis;
    if (!rc) {
      res.status(503).json({ error: "redis_unavailable" });
      return;
    }
    const state = {
      enabled,
      updated_at: new Date().toISOString(),
      updated_by: updated_by ?? actor,
    };
    const json = JSON.stringify(state);
    await rc.set("arbx:papermode", json);
    await rc.publish("arbx:papermode:changes", json);
    
    await writeAudit("config.papermode.update", actor, "config", "papermode",
                     null, state, req.ip ?? null, (req as any).traceId ?? null);
    res.status(200).json(state);
  } catch (e) {
    logger.error({ err: (e as Error).message }, "papermode update failed");
    res.status(500).json({ error: "redis_error" });
  }
});

app.delete("/admin/relays/:id", requireAdminToken(ARBX_ADMIN_TOKEN), async (req, res) => {
  const p = requireDbPool();
  if (!p) { res.status(503).json({ error: "db_unavailable" }); return; }
  const before = await p.query(`SELECT * FROM relays WHERE id = $1`, [req.params.id]);
  if (before.rowCount === 0) { res.status(404).json({ error: "not_found" }); return; }
  await p.query(`DELETE FROM relays WHERE id = $1`, [req.params.id]);
  const actor = req.header("x-arbx-actor") ?? "admin";
  await writeAudit("relay.delete", actor, "relay", req.params.id ?? "",
                   before.rows[0], null, req.ip ?? null, (req as any).traceId ?? null);
  res.status(204).end();
});

// Public read — used by /config page + relays-client Rust loader (future PR).
app.get("/api/v1/relays", async (_req, res) => {
  const p = requireDbPool();
  if (!p) { res.status(503).json({ error: "db_unavailable" }); return; }
  const q = await p.query(
    `SELECT name, chain_id, endpoint, auth_scheme, enabled, priority
       FROM relays WHERE enabled = TRUE
      ORDER BY chain_id, priority, name`,
  );
  res.status(200).json({ count: q.rows.length, items: q.rows, ts: new Date().toISOString() });
});

// ─────── Sprint 7 / Phase 0.5 R9 — onboarding ───────

app.get("/api/v1/onboarding/status", async (_req, res) => {
  const p = requireDbPool();
  if (!p) { res.status(503).json({ error: "db_unavailable" }); return; }
  const q = await p.query(
    `SELECT org_id,
            phase_1_completed_at, phase_1_completed_by, phase_1_vault_sealed_healthy,
            phase_2_completed_at, phase_2_completed_by, phase_2_rpc_probe_ok,
            phase_3_completed_at, phase_3_completed_by,
            phase_4_completed_at, phase_4_completed_by, phase_4_signer_zero_balance_verified,
            phase_5_completed_at, phase_5_completed_by, phase_5_paper_mode_off_at,
            created_at, updated_at
       FROM onboarding_progress WHERE org_id = 'default'`,
  );
  if (q.rowCount === 0) { res.status(503).json({ error: "onboarding_row_missing" }); return; }
  res.status(200).json(q.rows[0]);
});

const OnboardingPhase1 = z.object({
  confirmed_by: z.string().min(1).max(200),
  vault_sealed_healthy: z.boolean(),
  notes: z.string().max(1000).optional(),
});
app.post("/admin/onboarding/1/complete", requireAdminToken(ARBX_ADMIN_TOKEN), async (req, res) => {
  const p = requireDbPool();
  if (!p) { res.status(503).json({ error: "db_unavailable" }); return; }
  const parsed = OnboardingPhase1.safeParse(req.body);
  if (!parsed.success) { res.status(400).json({ error: "invalid_request", details: parsed.error.flatten() }); return; }
  if (!parsed.data.vault_sealed_healthy) {
    res.status(412).json({
      error: "precondition_failed",
      detail: "phase 1 requires Vault to be unsealed and reachable — see docs/governance/DATA-MATRIX.md M8",
    });
    return;
  }
  const q = await p.query(
    `UPDATE onboarding_progress
        SET phase_1_completed_at = NOW(),
            phase_1_completed_by = $1,
            phase_1_vault_sealed_healthy = TRUE
      WHERE org_id = 'default'
  RETURNING phase_1_completed_at, phase_1_completed_by, phase_1_vault_sealed_healthy`,
    [parsed.data.confirmed_by],
  );
  const actor = req.header("x-arbx-actor") ?? parsed.data.confirmed_by;
  await writeAudit("onboarding.phase1.complete", actor, "onboarding", "default",
                   null, parsed.data, req.ip ?? null, (req as any).traceId ?? null);
  res.status(200).json(q.rows[0]);
});

// ── Trading Config (per-chain operator-tunable strategy parameters) ─────
// Mounted late so it inherits express.json + admin token middleware semantics.
app.use(buildTradingConfigRouter({
  pool,
  redis,
  requireAdminToken,
  adminToken: ARBX_ADMIN_TOKEN,
  writeAudit,
  logger,
}));

// ── Operations PnL (Sprint 3 — PMI/EVM KPI surface) ────────────────────
app.use(buildOperationsRouter({
  pool,
  requireAdminToken,
  adminToken: ARBX_ADMIN_TOKEN,
  logger,
}));

// ── Strategy catalog (Sprint 2 — read-only universal MEV strategy library) ─
app.use(buildStrategyCatalogRouter({ pool, logger }));

app.get("/api/v1/config/current", async (_req, res) => {
  // Merge dynamic paper_mode from Redis if available
  let dynamicPaperMode = cfg.execution.paper_mode;
  try {
    const rc = redis;
    if (rc) {
      const pmStr = await rc.get("arbx:papermode");
      if (pmStr) {
        const pm = JSON.parse(pmStr);
        dynamicPaperMode = pm.enabled;
      }
    }
  } catch (e) {
    logger.warn({ err: (e as Error).message }, "failed to read dynamic papermode from redis");
  }

  res.status(200).json({
    system: cfg.system,
    risk: cfg.risk,
    execution: { ...cfg.execution, paper_mode: dynamicPaperMode },
    observability: cfg.observability,
    chains: cfg.chains,
    relays: cfg.relays,
    scoring: cfg.scoring,
    token_safety: cfg.token_safety,
    circuit_breakers: cfg.circuit_breakers,
  });
});

// Live readiness checklist — 17 items verified dynamically (option C from spec).
// 30s in-memory cache to avoid hammering verifiers on UI poll.
const READINESS_CACHE_MS = 30_000;
let readinessCache: { report: ReadinessReport; expires: number } | null = null;
app.get("/api/v1/readiness", async (_req, res) => {
  const now = Date.now();
  if (readinessCache && now < readinessCache.expires) {
    res.setHeader("x-arbx-cache", "HIT");
    res.status(200).json(readinessCache.report);
    return;
  }
  try {
    const report = await verifyAll({ pool });
    readinessCache = { report, expires: now + READINESS_CACHE_MS };
    res.setHeader("x-arbx-cache", "MISS");
    res.status(200).json(report);
  } catch (e) {
    logger.error({ err: (e as Error).message }, "readiness verifyAll failed");
    res.status(500).json({ error: "verifier_error", detail: (e as Error).message });
  }
});

const PORT = Number(process.env["API_PORT"] ?? 3000); // 3000 to match frontend fetch
const httpServer = createServer(app);
const io = setupWebSocketGateway(httpServer);

if (pool) {
  pool.connect().then(client => {
    client.query('LISTEN opportunities_channel');
    client.on('notification', (msg) => {
      if (msg.channel === 'opportunities_channel' && msg.payload) {
        try {
          const opp = JSON.parse(msg.payload);
          broadcastOpportunity(io, opp);
        } catch (e) {
          logger.warn({ event: "websocket.parse_error" }, "failed to parse notification");
        }
      }
    });
    logger.info({ event: "websocket.listen" }, "Listening to PostgreSQL opportunities_channel for WebSockets");
  }).catch(e => {
    logger.error({ err: (e as Error).message }, "failed to connect pg client for LISTEN");
  });
}

httpServer.listen(PORT, () => {
  logger.info({ event: "service.boot", port: PORT, env: cfg.system.env,
    upstreams: Object.keys(UPSTREAMS) }, `${SERVICE} listening`);
});

const shutdown = async (sig: string) => {
  logger.info({ event: "service.shutdown", signal: sig }, "shutting down");
  await killSwitch.close().catch(() => {});
  await redis.quit().catch(() => {});
  if (pool) await pool.end().catch(() => {});
  process.exit(0);
};
process.on("SIGINT", () => shutdown("SIGINT"));
process.on("SIGTERM", () => shutdown("SIGTERM"));
