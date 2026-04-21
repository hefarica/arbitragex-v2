import express from "express";
import pg from "pg";
import { Redis } from "ioredis";
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
  OpportunitySchema,
  SimulationResultSchema,
  opportunitiesTotal,
  initMetrics,
  KillSwitchClient,
  CircuitBreaker,
  globalRegistry,
  selectorDecisionsTotal,
  selectorProcessingSeconds,
  selectorInvalidMessagesTotal,
  selectorConsumerLag,
  cbStateGauge,
  cbTripsTotal,
} from "@arbx/shared";
import { scoreOpportunity } from "./score.js";
import { StreamConsumer } from "./consumer.js";

const SERVICE = "selector-api";
const VERSION = "0.2.0";

const cfg = loadAppConfig();
const logger = createLogger({ service: SERVICE, level: cfg.observability.log_level ?? "info" });
initMetrics(SERVICE);

const DATABASE_URL = requireEnv("DATABASE_URL");
const REDIS_URL = requireEnv("REDIS_URL");

const pool = new pg.Pool({ connectionString: DATABASE_URL, max: 10, idleTimeoutMillis: 30_000 });
pool.query("SELECT 1").catch((e: Error) => {
  logger.error({ event: "db.check.fail", err: e.message }, "cannot reach postgres at boot");
  process.exit(1);
});

const redis = new Redis(REDIS_URL, { lazyConnect: false, maxRetriesPerRequest: 3 });
const killSwitch = new KillSwitchClient({
  redisUrl: REDIS_URL,
  defaultWhenAbsent: cfg.system.kill_switch_enabled_default,
});
killSwitch.subscribeChanges().catch(() => {});

// ─── Circuit breakers (registered centrally, metrics on events) ───
const cbByName = new Map<string, CircuitBreaker>();
for (const c of cfg.circuit_breakers) {
  const cb = new CircuitBreaker(c);
  cb.on("trip", (e: { name: string; reason: string }) => {
    cbStateGauge.labels(e.name).set(2);
    cbTripsTotal.labels(e.name, e.reason.slice(0, 60)).inc();
    logger.warn({ event: "cb.trip", ...e });
  });
  cb.on("reset", (e: { name: string }) => {
    cbStateGauge.labels(e.name).set(0);
    logger.info({ event: "cb.reset", ...e });
  });
  cb.on("half_open", (e: { name: string }) => {
    cbStateGauge.labels(e.name).set(1);
  });
  cbStateGauge.labels(c.name).set(0);
  globalRegistry.register(cb);
  cbByName.set(c.name, cb);
}
const mustCb = (name: string): CircuitBreaker => {
  const cb = cbByName.get(name);
  if (!cb) throw new Error(`circuit breaker '${name}' not configured in app.toml`);
  return cb;
};

// ─── Express app (HTTP surface) ───
const ScoreRequest = z.object({
  opportunity: OpportunitySchema,
  simulation: SimulationResultSchema.nullable().optional(),
  safety_score: z.number().int().min(0).max(100).optional(),
});

const startedAt = new Date();
const app = express();
app.disable("x-powered-by");
app.use(express.json({ limit: "1mb" }));
app.use(traceIdMiddleware());
app.use(createHttpLogger(SERVICE, cfg.observability.log_level ?? "info"));
app.use(metricsMiddleware(SERVICE));

app.get("/health", healthHandler(SERVICE, VERSION, startedAt));
app.get("/metrics", metricsHandler);

app.post("/score", (req, res) => {
  const parsed = ScoreRequest.safeParse(req.body);
  if (!parsed.success) {
    res.status(400).json({ error: "invalid_request", details: parsed.error.flatten() });
    return;
  }
  const { opportunity, simulation, safety_score } = parsed.data;
  const scored = scoreOpportunity(opportunity, simulation ?? null, safety_score ?? 50);
  opportunitiesTotal.labels(
    String(opportunity.chain_id),
    opportunity.strategy_kind,
    scored.decision === "accept" ? "scored" : "rejected",
  ).inc();
  res.status(200).json(scored);
});

app.get("/opportunities", async (req, res) => {
  const status = String(req.query["status"] ?? "detected");
  const allowed = new Set(["detected","validated","simulated","scored","executing","executed","reconciled","rejected","failed"]);
  if (!allowed.has(status)) {
    res.status(400).json({ error: "invalid_status" });
    return;
  }
  const limit = Math.min(200, Math.max(1, Number(req.query["limit"] ?? 50)));
  try {
    const result = await pool.query(
      `SELECT id, chain_id, strategy_kind, dex_a, dex_b, pair_symbol,
              token_in, token_out, amount_in_wei, expected_profit_usd,
              status, risk_score, rejection_reason, detected_at, updated_at, trace_id
         FROM opportunities
        WHERE status = $1
        ORDER BY detected_at DESC
        LIMIT $2`, [status, limit]);
    res.status(200).json({ count: result.rowCount, rows: result.rows });
  } catch (e) {
    logger.error({ err: (e as Error).message }, "db error on /opportunities");
    res.status(500).json({ error: "db_error" });
  }
});

// ─── Stream consumer (S3 core) ───
const consumer = new StreamConsumer({
  redis, pool, logger, cfg,
  killSwitch,
  tokenSafetyCb: mustCb("token_safety_api"),
  dbWritesCb: mustCb("db_writes"),
  streamConsumerCb: mustCb("stream_consumer"),
  metrics: {
    decisionsTotal: (labels) => selectorDecisionsTotal.labels(labels).inc(),
    processingSeconds: (s) => selectorProcessingSeconds.observe(s),
    invalidMessagesTotal: () => selectorInvalidMessagesTotal.inc(),
    lagGauge: (n) => selectorConsumerLag.labels("arbx:opps:detected", "selector-g0").set(n),
  },
});
consumer.start().catch((e: Error) =>
  logger.error({ event: "consumer.start_failed", err: e.message }));

// ─── HTTP server ───
const PORT = Number(process.env["SELECTOR_PORT"] ?? 3002);
const server = app.listen(PORT, () => {
  logger.info({ event: "service.boot", port: PORT, env: cfg.system.env,
    circuit_breakers: Array.from(cbByName.keys()) },
    `${SERVICE} listening (consumer + HTTP)`);
});

// ─── Shutdown ───
const shutdown = async (sig: string) => {
  logger.info({ event: "service.shutdown", signal: sig });
  await consumer.stop();
  server.close();
  await killSwitch.close().catch(() => {});
  await redis.quit().catch(() => {});
  await pool.end().catch(() => {});
  process.exit(0);
};
process.on("SIGINT", () => shutdown("SIGINT"));
process.on("SIGTERM", () => shutdown("SIGTERM"));
