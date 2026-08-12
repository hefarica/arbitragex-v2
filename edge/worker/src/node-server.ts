/**
 * Node entry for the canonical edge/worker Hono app — POC for B-02.
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
