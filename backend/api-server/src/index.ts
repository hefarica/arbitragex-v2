import express from "express";
import pg from "pg";
import { Redis } from "ioredis";
import { PaperTradeArchiver } from "./routes/paper-trade-archiver.js";
import { PaperExecutor } from "./paper/executor.js";
import { ScoredOpportunitiesArchiver } from "./routes/scored-opportunities-archiver.js";
import { RouteDiscoveryOutcomeSink, outcomeSinkEnabled } from "./routes/route-discovery-outcome-sink.js";
import { OpportunitiesBridgeArchiver, opportunitiesBridgeEnabled } from "./routes/opportunities-bridge-archiver.js";
import { buildRouteDiscoveryOutcomesRouter } from "./routes/route-discovery-outcomes-api.js";
import { buildViableKpisRouter } from "./routes/viable-kpis.js";
import { buildPaperHistoryRouter } from "./routes/paper-history-api.js";
import { buildOperatorRouter } from "./routes/operator.js";
import { buildCartridgeForgeRouter } from "./routes/cartridge-forge.js";
import { z } from "zod";
import { clampBucketMinutes, clampHours, rowToPoint } from "./recon-timeseries.js";
import { redactAuditRow } from "./lib/audit-redact.js";
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
  securityHeadersMiddleware,
  requireEnv,
  requireAdminToken,
  requireEdgeToken,
  requireServiceToken,
  assertSecureBootTokens,
  KillSwitchClient,
  initMetrics,
} from "@arbx/shared";

const SERVICE = "api-server";
const VERSION = "0.1.0";

const cfg = loadAppConfig();
const logger = createLogger({ service: SERVICE, level: cfg.observability.log_level ?? "info" });
initMetrics(SERVICE);

const REDIS_URL = requireEnv("REDIS_URL");

// SECURE_BOOT (audit C1, 2026-05-10): refuse to start if admin/edge tokens
// are empty, known placeholders, or under 32 bytes of entropy.
assertSecureBootTokens(process.env);

const ARBX_ADMIN_TOKEN = requireEnv("ARBX_ADMIN_TOKEN");
const ARBX_EDGE_TOKEN = requireEnv("ARBX_EDGE_TOKEN");
// OMEGA-8/M3 P0-1: dedicated service-token for inter-service POSTs
// (searcher-rs, recon, relays-client → api-server). NEVER reuse admin token.
const ARBX_SERVICE_TOKEN = requireEnv("ARBX_SERVICE_TOKEN");

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
  "math-engine":  process.env["MATH_ENGINE_URL"] ?? "http://math-engine:3006",
  "token-enricher": process.env["ENRICHER_URL"] ?? "http://token-enricher:9004",
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
import { buildTradingConfigRouter, rehydrateTradingConfigMirror, tokenUniverseVersionKey } from "./routes/trading-config.js";
import { buildCartridgeFiltersRouter } from "./routes/cartridge-filters.js";
import { mountTokenIconRoutes } from "./routes/token-icon.js";
import { buildOperationsRouter } from "./routes/operations.js";
import { buildStrategyCatalogRouter } from "./routes/strategy-catalog.js";
import quotebaseCatalogRouter from "./routes/quotebase-catalog.js";
import { mountQuoteAnchor } from "./routes/quote-anchor.js";
import { buildCredentialsRouter } from "./routes/credentials.js";
import { rehydrateSvcCredMirror } from "./credentials/projection.js";
import { mountOpportunitiesLive } from "./routes/opportunities-live.js";
import { mountPricesLive } from "./routes/prices-live.js";
import { attachPriceRooms, subscribeToPriceUpdates } from "./prices-stream.js";
import { mountDexes } from "./routes/dexes.js";
import { mountPools } from "./routes/pools.js";
import { mountPairs } from "./routes/pairs.js";
import { mountStubs } from "./routes/stubs.js";
import { buildServiceControlRouter } from "./routes/service-control.js";
import { buildArchiveControlRouter } from "./routes/archive-control.js";
import { mountWallets } from "./routes/wallets.js";
import { CarnotStore } from "./services/carnotStore.js";
import { mountCarnotCycles } from "./routes/carnot-cycles.js";
import { mountStrategyRuntimeStatus } from "./routes/strategy-runtime-status.js";
import { mountReadinessExtras } from "./routes/readiness-extras.js";
import { mountReadinessSteps } from "./routes/readiness-steps.js";
import { mountReadinessEvidence } from "./routes/readiness-evidence.js";
import { mountAgentsStatus } from "./routes/agents-status.js";
import { mountScoringStatus } from "./routes/scoring-status.js";
// ARBX-RDY-06 (A.9): formal GO/NO-GO ledger machinery.
import { mountGoNoGo, buildDefaultLedgerFacts } from "./routes/go-no-go.js";
import { mountPaperShadowMetrics } from "./routes/paper-shadow-metrics.js";
import { mountPaperShadowAudit } from "./routes/paper-shadow-audit.js";
import { mountForkStatus } from "./routes/fork-status.js";
import { mountRpcRegistry } from "./routes/rpc-registry.js";
import { mountRpcBackend } from "./routes/rpc-backend.js";
import { mountOpportunitySimulate } from "./routes/opportunity-simulate.js";
import { buildMathEngineRouter } from "./routes/math-engine-proxy.js";
import { buildMathEvidenceRouter } from "./routes/math-evidence.js";
import { mountAlertmanagerWebhook } from "./routes/alertmanager-webhook.js";
import { mountRiskCircuitBreakers } from "./routes/risk-circuit-breakers.js";
import { mountAdminChains } from "./routes/admin-chains.js";
import { mountSedStatus } from "./routes/sed-status.js";
import { mountCanonicalKnobs } from "./routes/canonical-knobs.js";
import { mountSimPipeline } from "./routes/sim-pipeline.js";
import { mountSystemManifest } from "./routes/system-manifest.js";
import { mountLiveTestnet } from "./routes/live-testnet.js";
import { mountWalletRoutes } from "./routes/wallet.js";
import { mountPaperModeState } from "./routes/paper-mode-state.js";
import { mountPaperModeReconcile } from "./routes/paper-mode-reconcile.js";
import { createForkSimulator } from "./routes/wallet-sim-runtime.js";
import { mountAuthSiwe } from "./routes/auth-siwe.js";
import { mountOperatorSelfTest } from "./routes/operator-selftest.js";
import { buildTopologyVaultRouter } from "./routes/topology-vault.js";
import { mountHealthRouter, setupMetricsWebSocket } from "./routes/health.js";
import { registerGatesStatusRoutes } from "./routes/gates-status.js";
import {
  setupWebSocketGateway,
  broadcastOpportunity,
  subscribeToConvergenceSignals,
  subscribeToCartridgeTelemetry,
  subscribeToRouteDiscoveryTelemetry,
  OpportunityHotStreamer,
} from "./websocket.js";
import {
  TelemetryCache,
  buildRouteDiscoveryRouter,
  buildCartridgesRouter,
} from "./routes/route-discovery.js";
import { createServer } from "http";
import rateLimit from "express-rate-limit";

// defi routes mounted later (after `pool` and `logger` are constructed). See mountDefi() below.

app.disable("x-powered-by");
// OMEGA-8/M4 Fase 7: institutional HTTP security headers. nosniff, frameguard,
// no-referrer, CSP compatible with Socket.IO ws:/wss: upgrades. HSTS is gated
// by ARBX_ENABLE_HSTS=true because the api-server runs behind plain HTTP
// inside the VPS network — only the edge worker is TLS-terminated.
app.use(securityHeadersMiddleware());
app.use(express.json({ limit: "256kb" }));
app.use(traceIdMiddleware());
app.use(createHttpLogger(SERVICE, cfg.observability.log_level ?? "info"));
app.use(metricsMiddleware(SERVICE));

// API-3: defense-in-depth rate limit on /admin/*. The edge worker rate-limits at the
// network boundary, but if someone reaches the api-server directly (intra-VPS, debug
// tunnels, future multi-instance), admin endpoints would be unprotected. 30 req/min/IP
// is generous for one-operator dashboard use; tune via ADMIN_RATE_LIMIT_PER_MIN.
const ADMIN_RATE_LIMIT_PER_MIN = Math.max(
  1,
  parseInt(process.env["ADMIN_RATE_LIMIT_PER_MIN"] ?? "30", 10),
);
const adminLimiter = rateLimit({
  windowMs: 60_000,
  max: ADMIN_RATE_LIMIT_PER_MIN,
  standardHeaders: true,
  legacyHeaders: false,
  message: { error: "rate_limited", retry_after_seconds: 60 },
});
app.use("/admin/", adminLimiter);

// SECURITY (CodeQL js/missing-rate-limiting): defense-in-depth GLOBAL IP rate-limit
// covering EVERY route below. The edge worker already throttles at the network
// boundary; this protects the api-server if reached directly. Ceiling is very generous
// (6000/min/IP) so legit dashboard polling is never affected — only genuine abuse/DoS
// trips it. The stricter /admin/ + runtime-ack limiters remain in force. Tune via
// API_RATE_LIMIT_PER_MIN. /health + /metrics are registered before this on purpose.
const GLOBAL_RATE_LIMIT_PER_MIN = Math.max(
  60,
  parseInt(process.env["API_RATE_LIMIT_PER_MIN"] ?? "6000", 10),
);
const globalLimiter = rateLimit({
  windowMs: 60_000,
  max: GLOBAL_RATE_LIMIT_PER_MIN,
  standardHeaders: true,
  legacyHeaders: false,
  message: { error: "rate_limited", retry_after_seconds: 60 },
});

app.get("/health", healthHandler(SERVICE, VERSION, startedAt));
// REST convention alias — load balancers / external monitors expect /api/health.
// Same handler, no behaviour drift.
app.get("/api/health", healthHandler(SERVICE, VERSION, startedAt));
app.get("/metrics", metricsHandler);

// Apply the global rate-limit AFTER health/metrics (never throttle the compose
// healthcheck or Prometheus scrape) and BEFORE every data/admin route below.
app.use(globalLimiter);

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
    // AUDIT-2026-08-29 P0-1 (deployment coherence): the exact commit the
    // deploy workflow anchored + verified on the VPS (G4 deploy-veraz) before
    // building this stack. All services deploy from the same anchored
    // checkout. R8 fail-honest: 'unknown' = the container was created without
    // the workflow exports (manual `up`) — reported verbatim, never fabricated.
    deploy: {
      sha: process.env.ARBX_DEPLOY_SHA ?? "unknown",
      id: process.env.ARBX_DEPLOY_ID ?? "unknown",
      at: process.env.ARBX_DEPLOYED_AT ?? "unknown",
    },
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
    ip, traceId, reqUA(req),
  );
  logger.warn({ event: "admin.killswitch", actor: actorHeader, state: out }, "kill-switch toggled");
  res.status(200).json(out);
});

/** Admin: read killswitch state. */
app.get("/admin/killswitch/status", requireAdminToken(ARBX_ADMIN_TOKEN), async (_req, res) => {
  const state = await killSwitch.state().catch(() => null);
  if (!state) return res.status(503).json({ error: "killswitch_unavailable" });
  res.status(200).json(state);
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
    // R3: user_agent comes from the edge handshake body (AuditAuthBody).
    // Falls back to the request's own UA header when the body omits it.
    parsed.data.user_agent ?? reqUA(req),
  );
  res.status(204).end();
});

// ─────── Sprint 3: admin endpoints for blacklist + circuit breakers ───────

const DATABASE_URL = process.env["DATABASE_URL"] ?? "";
// API-1: pool size now env-configurable (default 20). max=3 was a bottleneck under
// concurrent admin + readiness + recon-timeseries load. Connection timeout added so
// stuck pool acquisition fails fast instead of hanging the request.
const PG_POOL_MAX = Math.max(1, parseInt(process.env["PG_POOL_MAX"] ?? "20", 10));
const pool = DATABASE_URL
  ? new pg.Pool({
      connectionString: DATABASE_URL,
      max: PG_POOL_MAX,
      idleTimeoutMillis: 30_000,
      connectionTimeoutMillis: 5_000,
    })
  : null;
const redis = new Redis(REDIS_URL, { lazyConnect: false, maxRetriesPerRequest: 3 });

function normAddr(a: string): string {
  const s = a.trim().toLowerCase();
  if (!/^0x[0-9a-f]{40}$/.test(s)) throw new Error("invalid_address");
  return s;
}

async function writeAudit(
  action: string,
  actor: string,
  targetKind: string | null,
  targetId: string | null,
  before: unknown,
  after: unknown,
  ip: string | null,
  traceId: string | null,
  userAgent: string | null = null,
): Promise<void> {
  if (!pool) return;
  try {
    // R3 (audit re-run #2, 2026-05-10): user_agent now plumbed end-to-end.
    // Both PII columns wrap with the helpers from migration 053 so raw values
    // never reach the row:
    //   - arbx_anonymize_ip($7)::cidr   → /24 IPv4 or /48 IPv6 network only.
    //   - arbx_hash_user_agent($9)      → 'sha256:<hex>' fingerprint, never raw UA.
    // userAgent defaults to null so non-HTTP callers (boot scripts, tests)
    // don't have to pass it; HTTP callsites pass req.header("user-agent") || null.
    await pool.query(
      `INSERT INTO audit_log (actor, action, target_kind, target_id, before_state, after_state, ip_address, trace_id, user_agent)
       VALUES ($1,$2,$3,$4,$5::jsonb,$6::jsonb,arbx_anonymize_ip($7)::cidr,$8,arbx_hash_user_agent($9))`,
      [actor, action, targetKind, targetId, JSON.stringify(before), JSON.stringify(after), ip, traceId, userAgent],
    );
    // auditEventsTotal.labels({ action }).inc();
  } catch (e) {
    logger.warn({ event: "audit.write_failed", err: (e as Error).message });
  }
}

// R3: shorthand to extract UA in HTTP handlers. Returns null when header
// is absent (boot scripts, tests, edge proxy with stripped headers).
function reqUA(req: express.Request): string | null {
  const ua = req.header("user-agent");
  return ua && ua.length > 0 ? ua : null;
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
  await writeAudit("blacklist.add", actor, "token", `${parsed.data.chain_id}:${addr}`, null, parsed.data, ip, traceId, reqUA(req));
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
  await writeAudit("blacklist.remove", actor, "token", `${chain}:${addr}`, { chain_id: chain, token_address: addr }, { removed }, req.ip ?? null, (req as any).traceId ?? null, reqUA(req));
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
  await writeAudit("cb.trip", req.header("x-arbx-actor") ?? "admin", "circuit_breaker", req.params.name ?? "", null, parsed.data, req.ip ?? null, (req as any).traceId ?? null, reqUA(req));
  res.status(202).json({ ok: true, published_to_channel: CB_CHANNEL, subscribers_notified: delivered });
});

app.post("/admin/circuit_breakers/:name/reset", requireAdminToken(ARBX_ADMIN_TOKEN), async (req, res) => {
  const msg = JSON.stringify({ action: "reset", name: req.params.name });
  const delivered = await redis.publish(CB_CHANNEL, msg);
  await writeAudit("cb.reset", req.header("x-arbx-actor") ?? "admin", "circuit_breaker", req.params.name ?? "", null, null, req.ip ?? null, (req as any).traceId ?? null, reqUA(req));
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
      // A-01: defense-in-depth redaction at the read boundary — raw/legacy IPs
      // collapse to /48-/24 and the operator email in `actor` is hashed (also
      // catches raw IPs in `target_id`). The edge worker masks again on egress;
      // this protects every consumer at the data origin. Append-only store untouched.
      items: q.rows.map(redactAuditRow),
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

mountOpportunitiesLive(app, pool, redis, logger);
// G-PRICE-1 — REST price snapshot (SSR initial state, WS-degraded polling, L4 curl target).
mountPricesLive(app, redis, logger);
const carnotStore = new CarnotStore();
mountCarnotCycles(app, { store: carnotStore, logger });
mountDexes(app, { pool, logger });
mountPools(app, { pool, logger });
// EMIT-06 (FE-MASTER P5 §13): effective pair universe — PG registry (the
// same table the Rust side loads) + live reserves + undrained dirty set.
mountPairs(app, { pool, redis, logger });
mountWallets(app, { pool, logger });
mountDefi(app, { pool, logger });
mountStrategyRuntimeStatus(app, { pool, redis, logger });
mountReadinessExtras(app, { pool, redis, logger });
mountReadinessSteps(app, { pool, redis, logger });
// G-SIM-1 FASE 2: append-only readiness evidence registry. Admin-gated
// (requireAdminToken) inside the module; sits under the /admin/ rate limiter.
mountReadinessEvidence(app, { pool, requireAdminToken, adminToken: ARBX_ADMIN_TOKEN, logger });
registerGatesStatusRoutes(app, { pool, redis, logger });
mountAgentsStatus(app, { pool, logger });
mountScoringStatus(app, { pool, logger });
// Web3 safe-gated wallet surface (read-only / paper) + SIWE identity-only auth.
// HARD INVARIANTS: live OFF, capital_exposed 0, broadcast OFF, no signer/keys
// server-side; every intent terminates at BROADCAST_DISABLED. Public, behind
// the existing globalLimiter (applied above, before all data routes).
mountWalletRoutes(app, {
  logger,
  // Read-only fork-sim adapter (fail-closed until simulator-v2 is ready + reachable) + REAL posture
  // gates. None of these can flip live_gate_open/broadcast/allow — the endpoint forces those false.
  forkSimulator: createForkSimulator({ logger }),
  readiness: async () => ({ green: !(await verifyAll({ pool })).flip_blocked }),
  killSwitch: async () => ({ off: !(await killSwitch.state()).enabled }),
});
mountAuthSiwe(app, { logger });
// Operator Self-Test Center (PR-1+PR-2) — presence-only credential matrix +
// 10-block checklist aggregator. READ-ONLY, public, behind the existing
// globalLimiter. HARD INVARIANTS: no env VALUE ever leaves the server (presence
// booleans only); live OFF / capital 0 / broadcast OFF (structural, reused from
// the wallet SAFE_POSTURE). Block 10 reuses the existing 17-item verifyAll().
mountOperatorSelfTest(app, {
  pool,
  redis,
  logger,
  readiness: () => verifyAll({ pool }),
});
mountRiskCircuitBreakers(app, { pool, killSwitch, logger });
mountAdminChains(app, {
  pool,
  redis,
  requireAdminToken,
  adminToken: ARBX_ADMIN_TOKEN,
  writeAudit,
  logger,
});

// ── Code-brechas (paper-shadow) — real handlers that SHADOW the A8 stubs and add
// the two missing live-readiness panel endpoints. Mounted here (well before
// mountStubs at the bottom of this file) so Express dispatches these real handlers
// instead of the 501 stubs. All fail-honest; paper-safe; zero capital.
// See docs/superpowers/specs/2026-06-14-arbx-code-brechas-design.md
mountPaperShadowMetrics(app, { pool, logger });
mountPaperShadowAudit(app, { pool, logger });
mountForkStatus(app, { logger });
mountOpportunitySimulate(app, { logger });

// Math-engine proxy — the 31 topological operators (list/toggle/compute/matrix
// projection) served by the math-engine service. Mounted at /api/math/*.
app.use(buildMathEngineRouter({ logger, requireAdminToken, adminToken: ARBX_ADMIN_TOKEN }));

// Service control plane (start/stop managed containers). Replaces the A8 501
// stubs (stubs.ts) — mounted here, before mountStubs, so these real handlers
// take precedence. Admin-gated + allowlisted + audit-logged (writeAudit); OFF
// by default (ARBX_SERVICE_CONTROL). Talks to the least-privilege socket-proxy
// sidecar — the api-server never mounts the raw docker socket.
app.use(
  buildServiceControlRouter({
    requireAdminToken,
    adminToken: ARBX_ADMIN_TOKEN,
    writeAudit,
    reqUA,
    logger,
  }),
);

// Archive control plane (DAPP-ARCHIVE-UI-01): cold-tier export surface for
// ARBX-RETENTION-01 — capacity (statfs), manual export (keyset → CSV → gzip
// into the ../archives bind mount), and the DB-backed auto-archive toggle
// the nightly cron reads. Admin-gated + audit-logged; fail-closed disk floor.
app.use(
  buildArchiveControlRouter({
    pool,
    requireAdminToken,
    adminToken: ARBX_ADMIN_TOKEN,
    writeAudit,
    logger,
  }),
);

// Math evidence — LIVE regime + per-operator values persisted by the searcher
// (Fix B) at arbx:math_evidence:<chain>:<strategy>. Read-only.
app.use(buildMathEvidenceRouter({ redis, logger }));
mountLiveTestnet(app, {
  logger,
  requireAdminToken,
  adminToken: ARBX_ADMIN_TOKEN,
  readiness: async () => verifyAll({ pool }),
});

// Paper-Mode State Authority — Tasks 5-6
// GET /api/paper-mode/state  (public, no-cache)
// POST /admin/paper-mode/reconcile (admin, Lua-atomic, gated by ARBX_PAPER_AUTO_RECONCILE)
const enabledChainIds = (cfg.chains ?? [])
  .filter((c: { enabled?: boolean }) => c.enabled !== false)
  .map((c: { chain_id: number }) => c.chain_id);
mountPaperModeState(app, { redis, env: process.env, enabledChainIds, logger });
mountPaperModeReconcile(app, {
  redis,
  env: process.env,
  requireAdminToken,
  adminToken: ARBX_ADMIN_TOKEN,
  logger,
});

// ARBX-RDY-06 (A.9) — formal GO/NO-GO ledger: canonical facts document +
// sha256 ledger_hash (persisted per generation to audit_log) + two-operator
// sign-off registry (migration 110). Pure record-keeping: NEVER flips
// anything live (§34.3 default-deny unchanged); go_live_eligible is a read
// of recorded state only.
mountGoNoGo(app, {
  pool,
  logger,
  requireAdminToken,
  adminToken: ARBX_ADMIN_TOKEN,
  buildLedgerFacts: () =>
    buildDefaultLedgerFacts({ pool, redis, enabledChainIds, logger }),
});

// RPC registry sync (Excel catalog → rpc_endpoints): public status + admin import/reload.
// status is counts-only (ungated); import/reload are requireAdminToken-gated.
mountRpcRegistry(app, { pool, redis, requireAdminToken, adminToken: ARBX_ADMIN_TOKEN, logger });
// RPC backend toggle (alloy dual-track FASE 4) — Redis arbx:rpc_backend:<service>.
// Admin-gated GET/PUT; every effective change audited (before/after). Mode-invariant
// (§34.1): selects the RPC implementation track only, never trading mode/capital.
mountRpcBackend(app, { redis, requireAdminToken, adminToken: ARBX_ADMIN_TOKEN, writeAudit, reqUA, logger });
mountAlertmanagerWebhook(app, {
  requireAdminToken,
  adminToken: ARBX_ADMIN_TOKEN,
  writeAudit,
  logger,
});

app.use(buildTopologyVaultRouter({
  redis,
  requireAdminToken,
  adminToken: ARBX_ADMIN_TOKEN,
  writeAudit,
  logger,
}));

// QUANTUM FULLSTACK SYMMETRY — read-only telemetry snapshot caches + REST
// routers for the OMEGA Route Discovery radar and cartridge telemetry. The
// caches are populated at runtime by the WS Redis bridges (instantiated near the
// bottom of this file); these routers serve the last known state without
// fabricating data (R8 fail-honest: ok=false when empty). RULE 00 / NO-ACTIVE:
// strictly observational — never touch arbx:opps:detected, never execute.
const routeDiscoveryCache = new TelemetryCache(200);
const cartridgeTelemetryCache = new TelemetryCache(200);
app.use(buildRouteDiscoveryRouter(routeDiscoveryCache));
app.use(buildCartridgesRouter(cartridgeTelemetryCache, { redis, requireAdminToken, adminToken: ARBX_ADMIN_TOKEN }));

// FASE B Gate-C — read-only analytics over the durable route_discovery_outcomes
// table (the shadow outcomes the sink persists, incl. the Paso 9 `reason`). This is
// the missing READ side for that passive sink. NO-ACTIVE: pure SELECT, never writes.
app.use(buildRouteDiscoveryOutcomesRouter(pool));
// XLS-DASH-01 — workbook 29_SUPER_DASHBOARD KPI set (viable by_hops / by_kind +
// viability %) over REAL opportunities rows. Read-only SELECT, observe-only.
app.use(buildViableKpisRouter(pool));
// FASE OMEGA SHADOW — paper_trade_runs read-side (drift-analysis surface).
// 100% read-only / NO-ACTIVE: pure SELECT, never touches capital or execution.
app.use(buildPaperHistoryRouter(pool));

// Enterprise-audit follow-up: mount control-plane routers that were built but never
// mounted, gating auth INTERNALLY. operator (requireOperatorRole per route; relative
// paths -> base /api/operator) + cartridge-forge (admin-token validator passed here;
// shadow strategy injection). admin-registries and admin-promote-mainnet are
// INTENTIONALLY NOT mounted:
//   - admin-registries: VERIFIED 2026-06-04 to carry NO authentication — its router
//     (lib/registry-engine.ts buildRegistryRouter) exposes raw POST/PATCH/DELETE and
//     actorOf() reads only an `x-omega-actor` header / req.ip (no token check). It is a
//     full CRUD mutation surface over live-adjacent entities (risk_gate, capital_gate,
//     contract_registry, relay_endpoints, rpc) that publishes hot-reload events to the
//     searcher-rs Arc-swap. Mounting it = unauthenticated runtime mutation of risk/capital
//     gates + executor contract targets = FASE D territory (blocked: no KMS, no human
//     authorization-of-record, no audit sign-off). To enable LATER it must be wrapped in
//     requireAdminToken (V-AT-1) and pass FASE D authorization; read-only (GET-only)
//     mounting could come first if observability is the only need.
//   - admin-promote-mainnet: live-adjacent (mainnet promotion) — same FASE D gate.
if (pool) {
  app.use("/api/operator", buildOperatorRouter(pool));
  app.use(
    buildCartridgeForgeRouter({
      db: pool,
      redis,
      adminTokenValidator: (t: string) => !!ARBX_ADMIN_TOKEN && t === ARBX_ADMIN_TOKEN,
    }),
  );
}

mountSedStatus(app, { pool, logger });

// A.8 surface: Gate-C confidence-scoring pipeline state, per STRATEGY
// (STRAT-IDENT-01). Read-only aggregates from scored_opportunities.
mountSimPipeline(app, { pool, logger });

// XLS-CANON-01: the 42-knob canonical configuration surface (workbook
// 01_CONFIG) — searcher-rs boot snapshot from Redis, served verbatim.
mountCanonicalKnobs(app, { redis, logger });

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

// EMIT-05 (FE-MASTER §18/§43-44): latest route-discovery tick summary — the
// durable snapshot searcher-rs writes each tick alongside its pub/sub publish
// (`arbx:route_discovery:tick:<chain_id>`, SET .. EX 60). Served FLAT: the
// frontier Zod mirror (frontend/lib/apex/schemas/telemetry.ts) validates the
// worker's tick_summary verbatim, so no wrapper object may be added here.
// 404 when the key is absent → discovery loop down or recently restarted
// (R8 fail-honest: the client renders a loading/error state, never a zeroed
// funnel).
app.get("/api/route-discovery/tick", async (req, res) => {
  const chainId = Number(req.query["chain_id"] ?? 1);
  if (!Number.isFinite(chainId) || chainId < 1) {
    res.status(400).json({ error: "invalid_chain_id" });
    return;
  }
  const key = `arbx:route_discovery:tick:${chainId}`;
  try {
    const raw = await redis.get(key);
    if (raw == null) {
      res.status(404).json({
        error: "tick_not_available",
        detail: `no snapshot at ${key} — route discovery may be down or recently restarted`,
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
    res.status(200).json(parsed);
  } catch (e) {
    logger.warn({ event: "route_discovery.tick.read_failed", err: (e as Error).message });
    res.status(503).json({ error: "redis_read_failed", detail: (e as Error).message });
  }
});

// EMIT-01 (FE-MASTER §4-§6): symbol → TokenKey resolution over the
// pre-indexed universe snapshot the searcher publishes on each identity
// rebuild (`arbx:token_universe:<chain_id>`, SET .. EX 35). The snapshot
// carries the TW-002-normalized symbol → addresses index built by the REAL
// Rust normalizer — this handler only does EXACT lookups, it never
// re-implements matching. One row per REQUESTED symbol (echoed
// `input_symbol`): address-form entries are their own identity (RESOLVED,
// TW-002), symbol entries are NOT_FOUND (0 matches), RESOLVED (exactly 1)
// or AMBIGUOUS (>1 — the UI blocks the save, §5). UNSUPPORTED is never
// emitted in v1 (no cross-chain existence source yet — honest absence).
// `decimals` / `pool_count` / `venue_count` / `liquidity_usd` are Layer-2
// (scan discards meta; venue-per-pool needs PG by epoch): null, never
// fabricated. `active_pools` rides the EMIT-05 tick snapshot when present.
app.post("/api/admin/tokens/resolve", requireAdminToken(ARBX_ADMIN_TOKEN), async (req, res) => {
  const body = (req.body ?? {}) as { chain_id?: unknown; symbols?: unknown };
  const chainId = Number(body.chain_id);
  if (!Number.isInteger(chainId) || chainId < 1) {
    res.status(400).json({ error: "invalid_chain_id" });
    return;
  }
  const symbols = body.symbols;
  if (
    !Array.isArray(symbols) ||
    symbols.length < 1 ||
    symbols.length > 200 ||
    !symbols.every((s) => typeof s === "string" && s.trim().length >= 1)
  ) {
    res.status(400).json({ error: "invalid_symbols", min: 1, max: 200 });
    return;
  }
  const key = `arbx:token_universe:${chainId}`;
  try {
    const raw = await redis.get(key);
    if (raw == null) {
      res.status(404).json({
        error: "universe_not_available",
        detail: `no snapshot at ${key} — searcher may be down or recently restarted`,
        chain_id: chainId,
      });
      return;
    }
    let parsed: {
      symbols?: Record<string, string[]>;
      tokens?: Record<string, { symbol?: unknown; decimals?: unknown }>;
      kpis?: { allowed_tokens?: number; possible_pairs?: number; directed_token_pairs?: number };
    };
    try {
      parsed = JSON.parse(raw);
    } catch (e) {
      res.status(503).json({ error: "snapshot_parse_failed", detail: (e as Error).message });
      return;
    }
    const symbolsIdx = parsed.symbols ?? {};
    // EMIT-02 Layer-2: decimals ride the snapshot's display map since the
    // scan keeps the TokenMeta field. Null = absent from the snapshot —
    // honest, never a guessed 18.
    const tokDecimals = (addr: string): number | null => {
      const e = parsed.tokens?.[addr];
      if (e && typeof e === "object" && typeof e.decimals === "number") {
        return e.decimals;
      }
      return null;
    };
    // Address-form mirror of shared-rs `is_address_form` (0x + exactly 40
    // ASCII hex, either prefix case; identity compare is lowercase).
    const ADDR_RE = /^(0x|0X)[0-9a-fA-F]{40}$/;
    const results = symbols.map((inputSymbol: string) => {
      const t = inputSymbol.trim();
      if (ADDR_RE.test(t)) {
        // TW-002: an address entry IS its own TokenKey — no universe lookup.
        const addrLower = t.toLowerCase();
        return {
          input_symbol: inputSymbol,
          chain_id: chainId,
          address: addrLower,
          decimals: tokDecimals(addrLower),
          pool_count: null,
          venue_count: null,
          liquidity_usd: null,
          resolution_status: "RESOLVED" as const,
        };
      }
      const matches = symbolsIdx[t.toUpperCase()] ?? [];
      const status =
        matches.length === 0 ? "NOT_FOUND" : matches.length === 1 ? "RESOLVED" : "AMBIGUOUS";
      const resolvedAddr = status === "RESOLVED" ? matches[0] ?? null : null;
      return {
        input_symbol: inputSymbol,
        chain_id: chainId,
        address: resolvedAddr,
        decimals: resolvedAddr !== null ? tokDecimals(resolvedAddr) : null,
        pool_count: null,
        venue_count: null,
        liquidity_usd: null,
        resolution_status: status,
      };
    });

    // §6 KPIs: token/pair counts from the snapshot; active_pools from the
    // EMIT-05 tick when one exists. venues/graph/universe versions are
    // EMIT-04 / Layer-2 — null until their source exists (R8).
    let activePools: number | null = null;
    try {
      const tickRaw = await redis.get(`arbx:route_discovery:tick:${chainId}`);
      if (tickRaw != null) {
        const tick = JSON.parse(tickRaw) as { pools_total?: unknown };
        if (Number.isFinite(Number(tick.pools_total))) {
          activePools = Number(tick.pools_total);
        }
      }
    } catch {
      // absent/unparsable tick stays null — honest absence, not an error
    }
    const k = parsed.kpis ?? {};
    // EMIT-04: universe_version — same Redis counter the trading-config PUT
    // bumps (tokenUniverseVersionKey). Absent = never changed = honest null.
    // graph_version stays null until the graph builder actually versions its
    // rebuilds (documented gap — never fabricated here).
    let universeVersion: number | null = null;
    try {
      const v = await redis.get(tokenUniverseVersionKey(chainId));
      const n = Number(v);
      if (v !== null && Number.isSafeInteger(n) && n >= 0) universeVersion = n;
    } catch {
      // unreadable counter stays null — honest absence
    }
    res.status(200).json({
      results,
      universe: {
        allowed_tokens: k.allowed_tokens ?? null,
        possible_pairs: k.possible_pairs ?? null,
        directed_token_pairs: k.directed_token_pairs ?? null,
        active_pools: activePools,
        active_venues: null,
        graph_version: null,
        universe_version: universeVersion,
      },
    });
  } catch (e) {
    logger.warn({ event: "tokens.resolve.read_failed", err: (e as Error).message });
    res.status(503).json({ error: "redis_read_failed", detail: (e as Error).message });
  }
});

// EMIT-02 Layer-2 + EMIT-03 (FE-MASTER §10, corrected by operator ruling):
// GET /api/quote/anchor (flattened 8-key view over the searcher-published
// snapshot) + POST /api/admin/quote/preview (deterministic re-ranking of the
// SAME live rows, never a mutation). Routes + fail-honest contract live in
// routes/quote-anchor.ts (pattern canonical-knobs).
mountQuoteAnchor(app, { redis, logger, adminToken: ARBX_ADMIN_TOKEN });

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
                COALESCE(SUM(CASE WHEN status='included' THEN 1 ELSE 0 END), 0)::int AS included,
                COALESCE(SUM(CASE WHEN status='reverted' THEN 1 ELSE 0 END), 0)::int AS reverted,
                COALESCE(SUM(CASE WHEN status='dropped'  THEN 1 ELSE 0 END), 0)::int AS dropped,
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
                     req.ip ?? null, (req as any).traceId ?? null, reqUA(req));
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
                   existing, parsed.data, req.ip ?? null, (req as any).traceId ?? null, reqUA(req));
  // Respond with the updated row. Without this the handler completed the UPDATE +
  // audit but never sent a response, so the request hung until the proxy/client
  // timed out (the relay-catalog admin flow was silently dead).
  res.status(200).json(q.rows[0]);
});

// Paper mode admin endpoint.
//
// B0.2 (2026-05-13) — Per-chain isolation:
//   - Body accepts optional `chain_id` (integer). When present, writes to
//     `arbx:papermode:<chain_id>` and publishes `arbx:papermode:<chain_id>:changes`.
//   - When `chain_id` is OMITTED, the call is REJECTED with 400. Operator
//     must explicitly target a chain — no more global flip footgun.
//   - The legacy global `arbx:papermode` key is now READ-ONLY (fallback for
//     30 days from 2026-05-13). All NEW writes are per-chain.
app.post("/admin/config/paper-mode", requireAdminToken(ARBX_ADMIN_TOKEN), async (req, res) => {
  const actor = req.header("x-arbx-actor") ?? "admin";
  const { enabled, updated_by, chain_id } = req.body;
  if (typeof enabled !== "boolean") {
    res.status(400).json({ error: "invalid_body", detail: "enabled must be boolean" });
    return;
  }
  // B0.2 enforcement: operator MUST specify chain_id. No more global flips.
  if (typeof chain_id !== "number" || !Number.isInteger(chain_id) || chain_id < 1) {
    res.status(400).json({
      error: "chain_id_required",
      detail: "B0.2 isolation: chain_id (positive integer) is required to avoid global papermode flips. Use {enabled, chain_id: 1} per-chain.",
    });
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
      chain_id,
    };
    const json = JSON.stringify(state);
    const perChainKey = `arbx:papermode:${chain_id}`;
    const perChainChannel = `arbx:papermode:${chain_id}:changes`;
    await rc.set(perChainKey, json);
    await rc.publish(perChainChannel, json);
    // Compat: also publish on legacy channel for subscribers not yet migrated.
    // We do NOT write to legacy KEY (that would defeat the isolation).
    await rc.publish("arbx:papermode:changes", json);

    await writeAudit("config.papermode.update", actor, "config", `papermode:${chain_id}`,
                     null, state, req.ip ?? null, (req as any).traceId ?? null, reqUA(req));
    res.status(200).json({ ...state, source: "per_chain", key: perChainKey });
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
                   before.rows[0], null, req.ip ?? null, (req as any).traceId ?? null, reqUA(req));
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

// ─────── FE-CRIT-03/04/01 — honest contract/capital/crucible read surface ───────
//
// Three public read endpoints the frontend's system-guard / live-readiness layer
// consumes. Doctrine (RULE 00 + R8 fail-honest):
//   • SAFETY fields (live_enabled / capital_exposed / broadcast / submit_enabled /
//     private_relay_enabled) are HARDCODED to the safe posture (false / 0). They are
//     NOT derived from request input or mutable runtime state — this is a security
//     invariant of the shadow/paper deployment, not a reflection of data.
//   • DATA fields are sourced READ-ONLY from real tables when present; on an empty
//     table OR a missing-table/query error the response is an HONEST-EMPTY payload
//     with an explicit `reason` — NEVER a fabricated 200 and NEVER a 500.
//   • crucible `ready` carries a FALSE-GREEN GUARD: it is `false` unless count>0 AND
//     every row clears the qualification bar.

/** Postgres "undefined_table" (42P01) — crucible_runs has no migration yet, so its
 *  absence is an expected honest-empty condition, not an error to surface as 500. */
function isUndefinedTable(e: unknown): boolean {
  return !!e && typeof e === "object" && (e as { code?: string }).code === "42P01";
}

// GET /api/contracts — read-only view of the contract_registry (migration 066).
// Honest-empty when the registry has no rows. Safety fields are always false/0.
app.get("/api/contracts", async (_req, res) => {
  const p = requireDbPool();
  const base = {
    status: "ok" as const,
    mode: "shadow" as const,
    source: "api-server" as const,
    live_enabled: false,
    capital_exposed: 0,
    broadcast: false,
  };
  if (!p) {
    res.status(200).json({ ...base, contracts: [], count: 0, reason: "db_unavailable" });
    return;
  }
  try {
    const q = await p.query(
      `SELECT chain_id, label, address, contract_kind, proxy_kind, verified,
              enabled, status, updated_at
         FROM contract_registry
        WHERE status <> 'deprecated'
        ORDER BY chain_id, label
        LIMIT 500`,
    );
    res.status(200).json({
      ...base,
      contracts: q.rows,
      count: q.rows.length,
      ...(q.rows.length === 0 ? { reason: "empty_contract_registry" } : {}),
    });
  } catch (e) {
    if (isUndefinedTable(e)) {
      res.status(200).json({ ...base, contracts: [], count: 0, reason: "contract_registry_absent" });
      return;
    }
    logger.warn({ event: "contracts.query_failed", err: (e as Error).message });
    res.status(200).json({ ...base, contracts: [], count: 0, reason: "query_failed" });
  }
});

// GET /api/capital-gates — safety posture + (read-only) configured capital gates.
// The top-level safety flags are HARDCODED-safe (shadow invariant); the `gates`
// list reflects real capital_gates rows when present, else a single honest
// capital_exposure gate proving exposure is zero.
app.get("/api/capital-gates", async (_req, res) => {
  const p = requireDbPool();
  const safety = {
    status: "ok" as const,
    live_enabled: false,
    capital_exposed: 0,
    broadcast: false,
    submit_enabled: false,
    private_relay_enabled: false,
  };
  const exposureGate = { name: "capital_exposure", status: "PASS" as const, value: 0 };
  if (!p) {
    res.status(200).json({ ...safety, gates: [exposureGate], reason: "db_unavailable" });
    return;
  }
  try {
    const q = await p.query(
      `SELECT scope, scope_ref, name,
              capital_cap_usd::float   AS capital_cap_usd,
              max_gas_burn_usd::float  AS max_gas_burn_usd,
              max_drawdown_pct::float  AS max_drawdown_pct,
              enabled, status
         FROM capital_gates
        WHERE enabled = TRUE AND status = 'active'
        ORDER BY scope, name
        LIMIT 500`,
    );
    // The exposure-proof gate is ALWAYS first (it proves capital_exposed=0 in the
    // shadow deployment); configured gates follow as informational rows.
    const configured = q.rows.map((g) => ({
      name: g.name,
      status: g.status === "active" ? ("PASS" as const) : ("HOLD" as const),
      value: Number(g.capital_cap_usd ?? 0),
      scope: g.scope,
      scope_ref: g.scope_ref,
    }));
    res.status(200).json({ ...safety, gates: [exposureGate, ...configured] });
  } catch (e) {
    if (isUndefinedTable(e)) {
      res.status(200).json({ ...safety, gates: [exposureGate], reason: "capital_gates_absent" });
      return;
    }
    logger.warn({ event: "capital_gates.query_failed", err: (e as Error).message });
    res.status(200).json({ ...safety, gates: [exposureGate], reason: "query_failed" });
  }
});

// GET /api/crucible/status — read-only crucible_runs qualification snapshot.
// FALSE-GREEN GUARD: `ready` is `false` on empty AND on any per-row failure to
// clear the bar (≥72h uptime, ≥95% success, 0 non-doctrinal reverts). The
// crucible_runs table has no migration yet → a missing table is honest-empty,
// not a 500.
const CRUCIBLE_BAR = { uptime_hours: 72, success_rate: 0.95, non_doctrinal_reverts: 0 };
app.get("/api/crucible/status", async (req, res) => {
  const p = requireDbPool();
  if (!p) {
    res.status(200).json({
      status: "ok", ready: false, rows: [], count: 0,
      reason: "db_unavailable", false_green_guard: true,
    });
    return;
  }
  const chainFilter = Number(req.query["chain_id"]);
  const hasChain = Number.isFinite(chainFilter) && chainFilter >= 1;
  try {
    const q = await p.query(
      `SELECT chain_id,
              EXTRACT(EPOCH FROM (NOW() - MIN(started_at))) / 3600.0 AS uptime_hours,
              COALESCE(SUM(CASE WHEN status='success' THEN 1 ELSE 0 END)::float
                       / NULLIF(COUNT(*), 0), 0)                     AS success_rate,
              COALESCE(SUM(CASE WHEN status='revert' AND revert_kind <> 'doctrinal'
                                THEN 1 ELSE 0 END), 0)               AS non_doctrinal_reverts,
              COUNT(*)::int                                          AS runs
         FROM crucible_runs
        WHERE started_at > NOW() - INTERVAL '7 days'
          AND ($1::bigint IS NULL OR chain_id = $1)
        GROUP BY chain_id
        ORDER BY chain_id`,
      [hasChain ? chainFilter : null],
    );
    const rows = q.rows.map((r) => {
      const uptime = Number(r.uptime_hours ?? 0);
      const success = Number(r.success_rate ?? 0);
      const nonDoc = Number(r.non_doctrinal_reverts ?? 0);
      return {
        chain_id: Number(r.chain_id),
        uptime_hours: uptime,
        success_rate: success,
        non_doctrinal_reverts: nonDoc,
        runs: Number(r.runs ?? 0),
        qualified:
          uptime >= CRUCIBLE_BAR.uptime_hours &&
          success >= CRUCIBLE_BAR.success_rate &&
          nonDoc === CRUCIBLE_BAR.non_doctrinal_reverts,
      };
    });
    // FALSE-GREEN GUARD: ready ONLY when there ARE rows and EVERY row qualifies.
    const ready = rows.length > 0 && rows.every((r) => r.qualified);
    res.status(200).json({
      status: "ok",
      ready,
      rows,
      count: rows.length,
      required: CRUCIBLE_BAR,
      false_green_guard: true,
      ...(rows.length === 0 ? { reason: "no_crucible_rows_available" } : {}),
    });
  } catch (e) {
    if (isUndefinedTable(e)) {
      res.status(200).json({
        status: "ok", ready: false, rows: [], count: 0,
        reason: "no_crucible_rows_available", false_green_guard: true,
      });
      return;
    }
    logger.warn({ event: "crucible.status.query_failed", err: (e as Error).message });
    res.status(200).json({
      status: "ok", ready: false, rows: [], count: 0,
      reason: "query_failed", false_green_guard: true,
    });
  }
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
                   null, parsed.data, req.ip ?? null, (req as any).traceId ?? null, reqUA(req));
  res.status(200).json(q.rows[0]);
});

// ── Trading Config (per-chain operator-tunable strategy parameters) ─────
// Mounted AFTER `io` exists (below) so the EMIT-04 token_universe runtime_ack
// broadcast has the WSS gateway. Still mounted late enough to inherit
// express.json + admin token middleware semantics.

// Boot re-hydration: PG is the source of truth for trading_config; the Redis
// mirror (arbx:trading_config:<chain>) is written ONLY by the admin PUT, so a
// Redis restart/eviction silently drops it → searcher-rs sees has_config=false
// → 0 opportunities. Re-mirror enabled rows from PG at boot so the config (and
// thus the paper feed) survives Redis restarts. Fire-and-forget; never blocks
// boot and never throws (handles its own errors). Replays existing PG values only.
void rehydrateTradingConfigMirror({ pool, redis, logger });

// RunFullSyncCycle FASE 3: same boot-rehydration contract for the service
// credential projection (arbx:svc_cred:<provider>:<scope>) — replays existing
// PG rows only, fire-and-forget, never throws. Runtime consumers read this
// projection with .env as fallback (precedence documented in projection.ts).
void rehydrateSvcCredMirror({ pool, redis, logger });

// ── Cartridge Filters (Idea 1 Phase-1 foundation — operator route pre-filter prefs) ─
// Redis-only stored preference (arbx:cartridge:filters:<chain>). NOT yet consumed by
// searcher-rs; the route pre-filter consumer + PG durability are a Phase 2 follow-up.
app.use(buildCartridgeFiltersRouter({
  redis,
  requireAdminToken,
  adminToken: ARBX_ADMIN_TOKEN,
  writeAudit,
  logger,
}));

// ── Token Icons (dashboard visual layer) ───────────────────────────────────
// GET /api/v1/token-icon/:chainId/:address — Redis(producer cache) → curated
// tokenRegistry → DexScreener(env-gated) → jazzicon fallback. Read-only +
// best-effort cache population; never 500s for a missing icon (R8).
mountTokenIconRoutes(app, { pool, redis, logger });

// ── Operations PnL (Sprint 3 — PMI/EVM KPI surface) ────────────────────
app.use(buildOperationsRouter({
  pool,
  requireAdminToken,
  adminToken: ARBX_ADMIN_TOKEN,
  logger,
}));

// ── Strategy catalog (Sprint 2 — read-only universal MEV strategy library) ─
app.use(buildStrategyCatalogRouter({ pool, logger }));

// ── QuoteBase workbook catalogs (EMIT-07/08 — static-per-canon, generated) ─
app.use("/api", quotebaseCatalogRouter);

// ── Operator credentials (migration 057 — RPC, CEX, Flashbots, etc.) ────
// Each credential has its own live validator; status only flips to "valid"
// after the api-server actually executes the provider-specific test.
app.use(buildCredentialsRouter({
  pool,
  redis,
  requireAdminToken,
  adminToken: ARBX_ADMIN_TOKEN,
  logger,
}));

app.get("/api/v1/config/current", async (_req, res) => {
  // Merge dynamic paper_mode from Redis (B0.2: per-chain key with legacy fallback).
  //
  // Reads per-chain keys for every chain in cfg.chains. Reports the per-chain
  // map under `paper_mode_per_chain` so the operator can see each chain
  // independently. The legacy `execution.paper_mode` boolean is kept for
  // retro-compat and is set to TRUE if ANY chain is in paper mode (safe-side
  // aggregation — never reports live unless ALL chains agree).
  const perChainPaperMode: Record<number, { enabled: boolean; source: string }> = {};
  let anyEnabled = false;
  let allEnabled = true;
  try {
    const rc = redis;
    if (rc) {
      for (const c of cfg.chains ?? []) {
        if (!c.enabled) continue;
        const perChainKey = `arbx:papermode:${c.chain_id}`;
        let raw = await rc.get(perChainKey);
        let source = "per_chain";
        if (!raw) {
          // Legacy fallback (read-only; 30 days from 2026-05-13).
          raw = await rc.get("arbx:papermode");
          source = raw ? "legacy_fallback" : "default";
        }
        let enabled = cfg.execution.paper_mode;
        if (raw) {
          try {
            enabled = JSON.parse(raw).enabled === true;
          } catch { /* keep config default */ }
        }
        perChainPaperMode[c.chain_id] = { enabled, source };
        anyEnabled = anyEnabled || enabled;
        allEnabled = allEnabled && enabled;
      }
    }
  } catch (e) {
    logger.warn({ err: (e as Error).message }, "failed to read per-chain papermode from redis");
  }

  // Aggregate flag: TRUE if any chain still in paper-mode (safe-side default).
  const dynamicPaperMode = Object.keys(perChainPaperMode).length === 0
    ? cfg.execution.paper_mode
    : anyEnabled;

  res.status(200).json({
    system: cfg.system,
    risk: cfg.risk,
    execution: {
      ...cfg.execution,
      paper_mode: dynamicPaperMode,
      paper_mode_per_chain: perChainPaperMode,
      paper_mode_all_chains_in_paper: allEnabled,
    },
    observability: cfg.observability,
    chains: cfg.chains,
    relays: cfg.relays,
    scoring: cfg.scoring,
    token_safety: cfg.token_safety,
    circuit_breakers: cfg.circuit_breakers,
  });
});

// ── A8 stubs — endpoints scaffolded for the frontend that aren't wired yet ─
// MUST be mounted LAST, after all real routes. Express dispatches the first
// matching route, so promoting any of these stubs to a real implementation
// is done by registering the canonical handler earlier (e.g. in a routes/
// module) — Express will pick the real handler and the stub becomes inert.
mountStubs(app, { requireAdminToken, adminToken: ARBX_ADMIN_TOKEN });

// Live readiness checklist — 17 items verified dynamically (option C from spec).
// API-4: TTL reduced from 30s to 5s default (env-configurable). 30s could mask
// degradations for half a minute. ?force=true bypasses the cache entirely for
// operator-driven re-verification immediately after a deploy or fix.
const READINESS_CACHE_MS = Math.max(
  0,
  parseInt(process.env["READINESS_CACHE_TTL_MS"] ?? "5000", 10),
);
let readinessCache: { report: ReadinessReport; expires: number } | null = null;
app.get("/api/v1/readiness", async (req, res) => {
  const now = Date.now();
  const force = req.query["force"] === "true" || req.query["force"] === "1";
  if (!force && readinessCache && now < readinessCache.expires) {
    res.setHeader("x-arbx-cache", "HIT");
    res.status(200).json(readinessCache.report);
    return;
  }
  try {
    const report = await verifyAll({ pool });
    readinessCache = { report, expires: now + READINESS_CACHE_MS };
    res.setHeader("x-arbx-cache", force ? "BYPASS" : "MISS");
    res.status(200).json(report);
  } catch (e) {
    logger.error({ err: (e as Error).message }, "readiness verifyAll failed");
    res.status(500).json({ error: "verifier_error", detail: (e as Error).message });
  }
});

// OMEGA-8/M4 Fase 8: align Dockerfile `EXPOSE 8080` with the runtime default.
// docker/compose.{dev,prod}.yml already set `API_PORT: "8080"` explicitly,
// so the fallback only fires for `npm run dev` / bare `node dist/index.js`
// invocations. Standardising on 8080 eliminates the previous mismatch where
// the container's exposed port could differ from the listener port if the
// env var was absent.
const PORT = Number(process.env["API_PORT"] ?? 8080);
const httpServer = createServer(app);
const io = setupWebSocketGateway(httpServer, carnotStore);

// Trading Config router — EMIT-04: mounted here (after `io`) because the
// token_universe runtime_ack POST-INSERT broadcast needs the WSS gateway.
app.use(buildTradingConfigRouter({
  pool,
  redis,
  requireAdminToken,
  adminToken: ARBX_ADMIN_TOKEN,
  writeAudit,
  logger,
  io,
}));

// G-PRICE-1 — snapshot+push price rooms (`subscribe:prices` → `prices:snapshot`
// + `prices:update`). Additive `io.on('connection')` listener; the gateway's
// own handlers are untouched. Requires the shared command `redis` (NOT the
// subscriber instances below).
attachPriceRooms(io, redis);

// OMEGA Health & Telemetry endpoints — léxico físico-matemático
// Montar DESPUÉS de que pool/redis/io estén inicializados
app.use("/api/v1/health", mountHealthRouter({ pool, redis, io }));
app.use("/api/v1/metrics", mountHealthRouter({ pool, redis, io }));

// OMEGA-7 PR-1: system manifest + runtime_ack handler. Must be mounted AFTER
// `io` exists because POST /api/system/runtime-ack emits a WSS broadcast into
// the `runtime_ack` room after a successful PostgreSQL INSERT (invariant
// I-2). Requires `pool` to be non-null; if the DB is missing this is a
// fail-fast condition because runtime_ack persistence is non-negotiable.
if (pool) {
  // OMEGA-8/M3 P0-1: per-IP rate-limits on runtime_ack endpoints. Defaults are
  // conservative but operator-overridable via env. POST is higher (legitimate
  // searcher-rs reloads burst up to ~60/min in M5 reload tests); GET is lower
  // (frontend fallback poll, bounded by useRuntimeAckSocket retry budget).
  const postWindowMs = Number(process.env["RUNTIME_ACK_POST_RATE_WINDOW_MS"] ?? 60_000);
  const postMax = Number(process.env["RUNTIME_ACK_POST_RATE_MAX"] ?? 120);
  const getWindowMs = Number(process.env["RUNTIME_ACK_GET_RATE_WINDOW_MS"] ?? 60_000);
  const getMax = Number(process.env["RUNTIME_ACK_GET_RATE_MAX"] ?? 30);
  const runtimeAckPostLimiter = rateLimit({
    windowMs: postWindowMs,
    max: postMax,
    standardHeaders: true,
    legacyHeaders: false,
    message: { error: "rate_limited", source: "runtime_ack_post" },
  });
  const runtimeAckGetLimiter = rateLimit({
    windowMs: getWindowMs,
    max: getMax,
    standardHeaders: true,
    legacyHeaders: false,
    message: { error: "rate_limited", source: "runtime_ack_get" },
  });
  app.use(
    "/api/system",
    mountSystemManifest(pool, redis, io, {
      requireServiceToken: requireServiceToken(ARBX_SERVICE_TOKEN),
      requireAdminToken: requireAdminToken(ARBX_ADMIN_TOKEN),
      runtimeAckPostLimiter,
      runtimeAckGetLimiter,
    }),
  );
} else {
  logger.error(
    { event: "system_manifest.disabled" },
    "system-manifest routes NOT mounted — DATABASE_URL missing",
  );
}

// Arteria WSS — OMEGA-v2: puente Redis Pub/Sub → WebSocket para señales de
// convergencia del motor SED (Rust).  Instancia dedicada (subscriber) porque
// ioredis no permite mezclar comandos regulares con modo SUBSCRIBE.
// Fail-honest: si Redis falla, las oportunidades por PostgreSQL NOTIFY
// siguen funcionando; la conexión se auto-reconecta.
const convergenceSubscriber = subscribeToConvergenceSignals(io, REDIS_URL);

// G-PRICE-1 hotfix — the pub/sub→room bridge was imported in #418 but never
// STARTED, so `prices:update` never fired (snapshots worked; pushes didn't —
// L4 caught it). Dedicated subscriber connection + its own command client,
// exactly like the convergence/cartridge bridges above.
const priceUpdatesSubscriber = subscribeToPriceUpdates(io, REDIS_URL);

// FASE OMEGA — puente Redis Pub/Sub → WebSocket para la telemetría de cartuchos
// (`log_quantum` del motor Rhai en Rust). Misma postura fail-honest que convergencia.
const cartridgeTelemetrySubscriber = subscribeToCartridgeTelemetry(
  io,
  REDIS_URL,
  (m) => cartridgeTelemetryCache.record(m),
);

// QUANTUM FULLSTACK SYMMETRY — puente Redis Pub/Sub → WebSocket para la
// telemetría del radar Route Discovery (`arbx:route_discovery:telemetry`).
// Espejo 1:1 del puente de cartuchos; alimenta el cache REST de solo-lectura.
// Fail-honest; NUNCA escribe arbx:opps:detected (observe-only).
const routeDiscoveryTelemetrySubscriber = subscribeToRouteDiscoveryTelemetry(
  io,
  REDIS_URL,
  (m) => routeDiscoveryCache.record(m),
);

// OMEGA Health & Telemetry — WebSocket namespace para streaming de métricas
// en tiempo real. Emite entropía, convergencia y métricas topológicas cada 5s.
setupMetricsWebSocket(io, redis, pool);

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

// FASE OMEGA SHADOW — paper_trade_runs archiver (passive telemetry sink).
// Reads arbx:opps:detected and persists each detected opportunity's sim
// prediction into paper_trade_runs for drift analysis. 100% passive: reads
// Redis + writes its own table only; never computes profit, never fabricates
// rows. Dormant by default (project doctrine: land off, enable with evidence) —
// set ARBX_PAPER_ARCHIVER_MODE=on to activate. Requires DATABASE_URL (pool).
let paperArchiver: PaperTradeArchiver | null = null;
if (pool && (process.env["ARBX_PAPER_ARCHIVER_MODE"] ?? "off").toLowerCase() === "on") {
  paperArchiver = new PaperTradeArchiver({ redisUrl: REDIS_URL, pool, logger });
  // A-02: seed the outlier-guard capital from trading_config (best-effort;
  // falls back to the $1000 floor if the read fails or capital is 0).
  void (async () => {
    try {
      const r = await pool.query(
        `SELECT capital_usd::float AS capital FROM trading_config WHERE enabled = true ORDER BY chain_id LIMIT 1`,
      );
      const cap = Number(r.rows[0]?.capital ?? 0);
      paperArchiver?.setCapitalUsd(cap);
      logger.info({ event: "paper_archiver.capital_seed", capital_usd: cap }, "A-02 outlier guard capital seeded");
    } catch (e) {
      logger.warn({ event: "paper_archiver.capital_seed_err", err: (e as Error).message }, "A-02 capital seed failed — using floor");
    }
  })();
  paperArchiver.start().catch((e) =>
    logger.error({ event: "paper_archiver.start_err", err: (e as Error).message },
      "paper_trade_runs archiver failed to start"),
  );
} else {
  logger.info(
    { event: "paper_archiver.dormant", reason: pool ? "mode_off" : "no_database_url" },
    "paper_trade_runs archiver dormant (set ARBX_PAPER_ARCHIVER_MODE=on to enable)",
  );
}

// OMEGA Pipeline Task 6 — Paper Trade Executor (Redis Streams consumer).
// Consumes from arbx:hot:simulated, calculates net topological yield,
// persists to paper_trade_runs, and emits to arbx:hot:paper_executed.
// 100% passive: shadow/paper mode only, never touches real capital.
// Dormant by default — set ARBX_PAPER_EXECUTOR_MODE=on to activate.
let paperExecutor: PaperExecutor | null = null;
if (pool && (process.env["ARBX_PAPER_EXECUTOR_MODE"] ?? "off").toLowerCase() === "on") {
  paperExecutor = new PaperExecutor({ redisUrl: REDIS_URL, pool, logger });
  paperExecutor.start().catch((e) =>
    logger.error({ event: "paper_executor.start_err", err: (e as Error).message },
      "paper executor failed to start"),
  );
} else {
  logger.info(
    { event: "paper_executor.dormant", reason: pool ? "mode_off" : "no_database_url" },
    "paper executor dormant (set ARBX_PAPER_EXECUTOR_MODE=on to enable)",
  );
}


// FASE B Paso 2 — route_discovery outcome sink (passive durable sink).
// Reads arbx:route_discovery:outcomes (the Rust shadow emitter, Fase B Paso 1) and
// persists each resolved outcome to route_discovery_outcomes — preserving the
// >=2-week hit-rate series the capped Redis stream would trim. 100% passive: reads
// the stream + writes its own table only; never opps:detected, never capital.
// Dormant by default — set ARBX_ROUTE_DISCOVERY_OUTCOMES_SINK=shadow. Requires DATABASE_URL.
let rdOutcomeSink: RouteDiscoveryOutcomeSink | null = null;
if (pool && outcomeSinkEnabled()) {
  rdOutcomeSink = new RouteDiscoveryOutcomeSink({ redisUrl: REDIS_URL, pool, logger });
  rdOutcomeSink.start().catch((e) =>
    logger.error({ event: "rd_outcome_sink.start_err", err: (e as Error).message },
      "route-discovery outcome sink failed to start"),
  );
} else {
  logger.info(
    { event: "rd_outcome_sink.dormant", reason: pool ? "gate_off" : "no_database_url" },
    "route-discovery outcome sink dormant (set ARBX_ROUTE_DISCOVERY_OUTCOMES_SINK=shadow to enable)",
  );
}

// OMEGA Pipeline Task 5 — Hot Opportunity Streamer (Redis Streams → WebSocket)
// Emite oportunidades detectadas y simuladas a clientes en tiempo real.
const hotOpportunityStreamer = new OpportunityHotStreamer({
  io,
  redisUrl: REDIS_URL,
  // H4 fix: the streamer's logger type is now pino-shaped (.info/.warn/.error).
  logger: logger as unknown as { info: (...args: unknown[]) => void; error: (...args: unknown[]) => void; warn: (...args: unknown[]) => void },
});
hotOpportunityStreamer.start().catch((e) =>
  logger.error({ event: "hot_streamer.start_err", err: (e as Error).message },
    "hot opportunity streamer failed to start"),
);

// Gate C — scored_opportunities archiver (passive scoring telemetry sink).
// Reads arbx:scoring:scored (the Rust OpportunityEmitter's ConfidenceScore XADD)
// and persists each scored opportunity into scored_opportunities. 100% passive:
// reads the stream + writes its own table only; never opps:detected, never
// capital. recommended_usd is a HYPOTHETICAL paper figure. Dormant by default —
// set ARBX_SCORING_ARCHIVER_MODE=on to activate. Requires DATABASE_URL (pool).
let scoredArchiver: ScoredOpportunitiesArchiver | null = null;
if (pool && (process.env["ARBX_SCORING_ARCHIVER_MODE"] ?? "off").toLowerCase() === "on") {
  scoredArchiver = new ScoredOpportunitiesArchiver({ redisUrl: REDIS_URL, pool, logger });
  scoredArchiver.start().catch((e) =>
    logger.error({ event: "scoring_archiver.start_err", err: (e as Error).message },
      "scored_opportunities archiver failed to start"),
  );
} else {
  logger.info(
    { event: "scoring_archiver.dormant", reason: pool ? "mode_off" : "no_database_url" },
    "scored_opportunities archiver dormant (set ARBX_SCORING_ARCHIVER_MODE=on to enable)",
  );
}

// Opportunities bridge (paper): reads arbx:route_discovery:outcomes and writes the
// opportunities PG table (status detected|rejected, paper-marked, amount_in_wei=0,
// dex_a='route_discovery') so /opportunities surfaces the real shadow routes with
// their values. The searcher is UNTOUCHED — only the api-server writes here.
// Dormant by default — set ARBX_OPPS_BRIDGE_MODE=on to activate. Requires DATABASE_URL.
let oppsBridge: OpportunitiesBridgeArchiver | null = null;
if (pool && opportunitiesBridgeEnabled()) {
  oppsBridge = new OpportunitiesBridgeArchiver({ redisUrl: REDIS_URL, pool, logger });
  oppsBridge.start().catch((e) =>
    logger.error({ event: "opps_bridge.start_err", err: (e as Error).message },
      "opportunities bridge failed to start"),
  );
} else {
  logger.info(
    { event: "opps_bridge.dormant", reason: pool ? "mode_off" : "no_database_url" },
    "opportunities bridge dormant (set ARBX_OPPS_BRIDGE_MODE=on to enable)",
  );
}

httpServer.listen(PORT, () => {
  logger.info({ event: "service.boot", port: PORT, env: cfg.system.env,
    upstreams: Object.keys(UPSTREAMS) }, `${SERVICE} listening`);
});

// H6 hardening (security-auditor FAIL-2): guard against concurrent shutdown().
// A second SIGINT/SIGTERM during the drain window would re-enter shutdown() and
// race redis.quit()/pool.end(). Idempotence guard makes re-entry a no-op.
let shutdownStarted = false;
const shutdown = async (sig: string) => {
  if (shutdownStarted) return;
  shutdownStarted = true;
  logger.info({ event: "service.shutdown", signal: sig }, "shutting down");
  await killSwitch.close().catch(() => {});
  // Stop the passive paper-trade archiver (closes its dedicated Redis conn).
  await paperArchiver?.stop().catch(() => {});
  // Stop the paper executor (closes its dedicated Redis conn).
  await paperExecutor?.stop().catch(() => {});
  await rdOutcomeSink?.stop().catch(() => {});
  await scoredArchiver?.stop().catch(() => {});
  await oppsBridge?.stop().catch(() => {});
  await hotOpportunityStreamer.stop().catch(() => {});
  // Arteria WSS: cerrar el subscriber de convergencia antes que el redis
  // principal para evitar errores de "Connection is closed" en handlers.
  await convergenceSubscriber.quit().catch(() => {});
  await cartridgeTelemetrySubscriber.quit().catch(() => {});
  await routeDiscoveryTelemetrySubscriber.quit().catch(() => {});
  await redis.quit().catch(() => {});
  // H6 fix: stop accepting new connections and drain in-flight requests BEFORE
  // ending the PG pool. Previously pool.end() ran while httpServer was still
  // serving, so any in-flight query threw `Cannot use a pool after calling
  // end` as an uncaught exception and the process died with an ugly stack.
  await new Promise<void>((resolve) => {
    const drainTimeout = setTimeout(() => resolve(), 5_000);
    // Stop accepting new connections; callback fires once existing ones close.
    httpServer.close(() => {
      clearTimeout(drainTimeout);
      resolve();
    });
    // If there are no active connections close() may never call back on some
    // Node versions — the timeout above guarantees we still proceed.
  });
  if (pool) await pool.end().catch(() => {});
  process.exit(0);
};
process.on("SIGINT", () => shutdown("SIGINT"));
process.on("SIGTERM", () => shutdown("SIGTERM"));
// Cache buster: 1781425985
