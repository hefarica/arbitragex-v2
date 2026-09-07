/**
 * Node entry for the canonical edge/worker Hono app.
 *
 * WO-13 (2026-09-06): this is the PRODUCTION edge entrypoint, not a POC.
 * B-02 closed with PR #321 (commit 60f3f702, 2026-08-11): compose.prod.yml
 * `edge` builds edge/worker/Dockerfile.node whose CMD runs this file's build
 * (dist/node-server.js). Verified live on the VPS 2026-09-06T23:45Z: container
 * CMD == ["node","/app/edge/worker/dist/node-server.js"] + boot log
 * `edge-worker.node.listen` (emitted only here). The Express dev-local variant
 * (edge/dev-local/, used by compose.dev.yml) is DEV-ONLY and separate.
 *
 * The worker source (./index.ts) is byte-identical to the Cloudflare deploy;
 * this file only adapts the runtime: it builds the worker `Env` from
 * process.env + two RedisKV shim instances and serves via @hono/node-server.
 *
 * ARBX_TELEMETRY (D1) is intentionally left undefined — the worker guards it
 * with `if (c.env.ARBX_TELEMETRY)`, so the one executionCtx.waitUntil
 * reference is unreachable and the missing CF executionCtx cannot crash.
 */
import { serve } from "@hono/node-server";
import { Redis } from "ioredis";
import app from "./index.js";
import { RedisKV } from "./kv-redis.js";

const REDIS_URL = process.env["REDIS_URL"] ?? "redis://localhost:6379";
const PORT = Number(process.env["EDGE_PORT"] ?? process.env["PORT"] ?? 8788);

const redis = new Redis(REDIS_URL, {
  retryStrategy: (times: number) => Math.min(times * 50, 2000),
  maxRetriesPerRequest: 3,
});

const env = {
  ARBX_ENV: process.env["ARBX_ENV"] ?? "production",
  API_SERVER_URL: process.env["API_SERVER_URL"] ?? "http://127.0.0.1:8080",
  ALLOWED_ORIGINS: process.env["ALLOWED_ORIGINS"] ?? "",
  ARBX_EDGE_TOKEN: process.env["ARBX_EDGE_TOKEN"] ?? "",
  JWT_SECRET: process.env["JWT_SECRET"] ?? "",
  SYBIL_ASN_DENYLIST: process.env["SYBIL_ASN_DENYLIST"],
  ARBX_CACHE: new RedisKV(redis, "edge:cache:"),
  RATE_LIMIT: new RedisKV(redis, "edge:rl:"),
};

serve(
  { fetch: (req) => app.fetch(req, env), port: PORT },
  (info) => {
    console.log(JSON.stringify({
      event: "edge-worker.node.listen",
      port: info.port,
      api_server_url: env.API_SERVER_URL,
    }));
  },
);
