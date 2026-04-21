import express from "express";
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
  });
});

const PORT = Number(process.env["API_PORT"] ?? 8080);
app.listen(PORT, () => {
  logger.info({ event: "service.boot", port: PORT, env: cfg.system.env,
    upstreams: Object.keys(UPSTREAMS) }, `${SERVICE} listening`);
});

const shutdown = async (sig: string) => {
  logger.info({ event: "service.shutdown", signal: sig }, "shutting down");
  await killSwitch.close().catch(() => {});
  process.exit(0);
};
process.on("SIGINT", () => shutdown("SIGINT"));
process.on("SIGTERM", () => shutdown("SIGTERM"));
