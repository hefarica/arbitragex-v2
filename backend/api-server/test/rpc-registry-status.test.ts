/**
 * EDGE-HARD-2 — RPC registry status smoke (structural invariants).
 *
 * 2026-08-20 incident (D-02 / "RPC Registry: SYNCING"): the health layer was
 * fully alive (20 prometheus provider series, 6/6 healthy) while the
 * `rpc_endpoints` table the UI reads was EMPTY — the registry surface the
 * dashboard consumes never reflected reality. This test pins the CONTRACT of
 * that surface so a regression fails CI:
 *
 *   1. GET /api/v1/rpc/status → 200 with the documented shape
 *      { chains: [{chain_id, http, ws, enabled, total, last_updated}], ... }
 *   2. Counts reflect the persisted rows (structure + arithmetic — NO magic
 *      numbers: the VPS has 11 rows today, CI's stack has its own; the
 *      portable invariant is "status mirrors the table").
 *   3. Honest empty: zero rows → {chains: [], total: 0}, never an error and
 *      never fabricated entries (R8).
 *   4. Privacy: counts only — the response must NOT contain any endpoint URL.
 *
 * Harness: same testcontainers pattern as opportunities-live.test.ts (the
 * migration comes from 066_omni_entity_registries.sql).
 */

import { describe, it, expect, beforeAll, afterAll } from "vitest";
import { GenericContainer, type StartedTestContainer, Wait } from "testcontainers";
import { Pool } from "pg";
import express from "express";
import request from "supertest";
import type { Redis } from "ioredis";
import { mountRpcRegistry } from "../src/routes/rpc-registry.js";

let container: StartedTestContainer;
let pool: Pool;

// The router requires a redis client for hot-reload publishing on the IMPORT
// route; the status route never touches it. A minimal double keeps the test
// focused on the status contract (import coverage belongs to its own suite).
const redisDouble = { publish: async () => 1 } as unknown as Redis;

beforeAll(async () => {
  container = await new GenericContainer("postgres:15")
    .withEnvironment({ POSTGRES_PASSWORD: "test", POSTGRES_DB: "arbitragex" })
    .withExposedPorts(5432)
    .withWaitStrategy(
      Wait.forLogMessage(/database system is ready to accept connections/, 2),
    )
    .start();
  const port = container.getMappedPort(5432);
  pool = new Pool({
    connectionString: `postgres://postgres:test@127.0.0.1:${port}/arbitragex`,
  });
  // Test-local DDL mirroring the 066 migration's rpc_endpoints shape. Loading
  // the real migration drags its FK dependency chain ("chains" et al.); the
  // status contract only needs the columns the handler reads.
  await pool.query(`
    CREATE TABLE rpc_endpoints (
      id BIGSERIAL PRIMARY KEY,
      chain_id INTEGER NOT NULL,
      url TEXT NOT NULL,
      transport TEXT NOT NULL,
      tier TEXT NOT NULL DEFAULT 'primary',
      auth_kind TEXT NOT NULL DEFAULT 'none',
      auth_credential_id UUID,
      weight INTEGER NOT NULL DEFAULT 100,
      rate_limit_rps INTEGER,
      max_concurrency INTEGER NOT NULL DEFAULT 16,
      enabled BOOLEAN NOT NULL DEFAULT TRUE,
      status TEXT NOT NULL DEFAULT 'pending_validation',
      notes TEXT,
      config_hash TEXT,
      created_by TEXT,
      updated_by TEXT,
      created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
      updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
      UNIQUE (chain_id, url)
    )`);
});

afterAll(async () => {
  await pool.end();
  await container.stop();
});

function appWith() {
  const app = express();
  mountRpcRegistry(app, {
    pool,
    redis: redisDouble,
    adminToken: "test-admin-token-32-bytes-of-entropy-ok",
    requireAdminToken:
      (token: string) =>
      (_req, _res, next) => {
        void token;
        next();
      },
    logger: {
      info: () => {},
      warn: () => {},
    },
  });
  return app;
}

describe("GET /api/v1/rpc/status (EDGE-HARD-2 structural smoke)", () => {
  it("returns 200 with an HONEST empty registry (R8 — no fabrication)", async () => {
    const res = await request(appWith()).get("/api/v1/rpc/status");
    expect(res.status).toBe(200);
    expect(res.body).toEqual({ chains: [], chain_count: 0, total: 0 });
    expect(typeof res.body.generated_at).toBe("string");
  });

  it("mirrors persisted rows in per-chain counts (http/ws/enabled/total)", async () => {
    // Real-shape rows (fields per 066 migration + RpcDescriptor schema).
    await pool.query(
      `INSERT INTO rpc_endpoints
         (chain_id, url, transport, tier, auth_kind, weight, max_concurrency,
          enabled, status, config_hash, created_by, updated_by)
       VALUES
         (1, 'https://alpha.example.io/v1/x', 'https', 'primary',   'none', 100, 16, true, 'active', 'h1', 'test', 'test'),
         (1, 'https://beta.example.io/v1/y',  'https', 'fallback',  'none',  50, 16, true, 'active', 'h2', 'test', 'test'),
         (1, 'wss://gamma.example.io/ws',     'wss',   'fallback',  'none',  40,  8, false,'paused', 'h3', 'test', 'test')`,
    );
    const res = await request(appWith()).get("/api/v1/rpc/status");
    expect(res.status).toBe(200);
    const chain = res.body.chains?.[0];
    expect(chain).toMatchObject({ chain_id: 1, http: 2, ws: 1, enabled: 2, total: 3 });
    expect(res.body.total).toBe(3);
    expect(chain.last_updated).toBeTruthy();
  });

  it("never leaks endpoint URLs (counts-only privacy contract)", async () => {
    const res = await request(appWith()).get("/api/v1/rpc/status");
    const raw = JSON.stringify(res.body);
    expect(raw).not.toContain("example.io");
    expect(raw).not.toContain("https://");
    expect(raw).not.toContain("wss://");
  });
});
