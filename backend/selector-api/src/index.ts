import express from "express";
import pg from "pg";
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
} from "@arbx/shared";
import { scoreOpportunity } from "./score.js";

const SERVICE = "selector-api";
const VERSION = "0.1.0";

const cfg = loadAppConfig();
const logger = createLogger({ service: SERVICE, level: cfg.observability.log_level ?? "info" });
initMetrics(SERVICE);

const DATABASE_URL = requireEnv("DATABASE_URL");
const pool = new pg.Pool({ connectionString: DATABASE_URL, max: 10, idleTimeoutMillis: 30_000 });

// Verify DB on boot — fail fast if unreachable.
pool.query("SELECT 1").catch((e: Error) => {
  logger.error({ event: "db.check.fail", err: e.message }, "cannot reach postgres at boot");
  process.exit(1);
});

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
              status, detected_at, trace_id
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

const PORT = Number(process.env["SELECTOR_PORT"] ?? 3002);
app.listen(PORT, () => {
  logger.info({ event: "service.boot", port: PORT, env: cfg.system.env }, `${SERVICE} listening`);
});

// Graceful shutdown
const shutdown = async (sig: string) => {
  logger.info({ event: "service.shutdown", signal: sig }, "shutting down");
  await pool.end().catch(() => {});
  process.exit(0);
};
process.on("SIGINT", () => shutdown("SIGINT"));
process.on("SIGTERM", () => shutdown("SIGTERM"));
