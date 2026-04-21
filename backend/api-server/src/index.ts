import express from "express";
import pg from "pg";
import Redis from "ioredis";
import { z } from "zod";
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
app.disable("x-powered-by");
app.use(express.json({ limit: "256kb" }));
app.use(traceIdMiddleware());
app.use(createHttpLogger(SERVICE, cfg.observability.log_level ?? "info"));
app.use(metricsMiddleware(SERVICE));

app.get("/health", healthHandler(SERVICE, VERSION, startedAt));
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
  const out = await killSwitch.set({
    enabled: parsed.data.enabled,
    reason: parsed.data.reason ?? null,
    triggered_by: parsed.data.triggered_by ?? actorHeader,
  });
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

const PORT = Number(process.env["API_PORT"] ?? 8080);
app.listen(PORT, () => {
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
