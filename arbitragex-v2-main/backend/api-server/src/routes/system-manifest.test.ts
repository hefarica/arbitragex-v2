/**
 * OMEGA-8 / M3 — Capa 2 hardening tests.
 *
 * Phases covered:
 *   - FASE 1 (P0-1): auth + rate-limit on POST /api/system/runtime-ack
 *   - FASE 3 (P1-1): UNIQUE (event_id, layer) + ON CONFLICT DO NOTHING dedup
 *   - FASE 4 (P1-4): GET /api/system/runtime-ack/:event_id fallback
 *   - FASE 7 (P1-5): advisory lock keyed by (resource, chain_id)
 *
 * Strategy: mock the `pg.Pool` interface to assert (a) auth gates fire before
 * any DB call, (b) ON CONFLICT result-row count distinguishes inserted vs
 * deduped, (c) GET fallback validates UUID and respects auth.
 */

import { describe, it, expect, beforeAll, vi } from "vitest";
import express, { type Express, type RequestHandler } from "express";
import request from "supertest";
import type { Pool } from "pg";
import type { Server as IoServer } from "socket.io";
import type { Redis } from "ioredis";
import { mountSystemManifest } from "./system-manifest.js";

const SERVICE_TOKEN = "test-service-token-32-bytes-of-entropy-aaaa";
const ADMIN_TOKEN = "test-admin-token-32-bytes-of-entropy-aaaaaa";

const VALID_PAYLOAD = {
  event_id: "11111111-1111-1111-1111-111111111111",
  resource: "trading_config",
  chain_id: 1,
  idempotency_key: "idem-001",
  config_hash_before: "a".repeat(64),
  config_hash_after: "b".repeat(64),
  worker_id: "searcher-rs-1",
  layer: "searcher_rs",
  status: "applied",
  latency_ms: 42,
  error: null,
} as const;

interface MockState {
  insertedOnConflict: boolean;
  getRows: Array<Record<string, unknown>>;
  queries: Array<{ sql: string; params: unknown[] | undefined }>;
}

function buildMockPool(state: MockState): Pool {
  return {
    query: vi.fn(async (sql: string, params?: unknown[]) => {
      state.queries.push({ sql, params });
      if (sql === "BEGIN" || sql === "COMMIT" || sql === "ROLLBACK") {
        return { rows: [], rowCount: 0 };
      }
      if (sql.includes("pg_advisory_xact_lock")) {
        return { rows: [], rowCount: 0 };
      }
      if (sql.includes("INSERT INTO runtime_ack")) {
        // ON CONFLICT DO NOTHING returns rowCount=1 on insert, 0 on dedup
        if (state.insertedOnConflict) {
          return { rows: [{ id: "row-id" }], rowCount: 1 };
        }
        return { rows: [], rowCount: 0 };
      }
      if (sql.includes("FROM runtime_ack")) {
        return { rows: state.getRows, rowCount: state.getRows.length };
      }
      return { rows: [], rowCount: 0 };
    }),
  } as unknown as Pool;
}

function mockIo(): IoServer {
  return {
    to: vi.fn().mockReturnThis(),
    emit: vi.fn(),
  } as unknown as IoServer;
}

function requireServiceTokenMw(expected: string): RequestHandler {
  return (req, res, next) => {
    const got = req.header("x-arbx-service-token");
    if (!got || got !== expected) {
      res.status(401).json({ error: "unauthorized", source: "service_token" });
      return;
    }
    next();
  };
}

function requireAdminTokenMw(expected: string): RequestHandler {
  return (req, res, next) => {
    const got = req.header("x-arbx-admin-token");
    if (!got || got !== expected) {
      res.status(401).json({ error: "unauthorized", source: "admin_token" });
      return;
    }
    next();
  };
}

const noopLimiter: RequestHandler = (_req, _res, next) => next();

function buildApp(state: MockState): { app: Express; pool: Pool; io: IoServer } {
  const app = express();
  app.use(express.json());
  const pool = buildMockPool(state);
  const io = mockIo();
  const redis = {} as unknown as Redis;
  app.use(
    "/api/system",
    mountSystemManifest(pool, redis, io, {
      requireServiceToken: requireServiceTokenMw(SERVICE_TOKEN),
      requireAdminToken: requireAdminTokenMw(ADMIN_TOKEN),
      runtimeAckPostLimiter: noopLimiter,
      runtimeAckGetLimiter: noopLimiter,
    }),
  );
  return { app, pool, io };
}

describe("OMEGA-8/M3 P0-1: POST /api/system/runtime-ack auth", () => {
  it("rejects POST without X-ArbX-Service-Token (401)", async () => {
    const state: MockState = { insertedOnConflict: true, getRows: [], queries: [] };
    const { app } = buildApp(state);
    const res = await request(app).post("/api/system/runtime-ack").send(VALID_PAYLOAD);
    expect(res.status).toBe(401);
    expect(res.body).toEqual({ error: "unauthorized", source: "service_token" });
    // Auth fired BEFORE the DB — no DB queries executed.
    expect(state.queries.length).toBe(0);
  });

  it("rejects POST with wrong service token (401)", async () => {
    const state: MockState = { insertedOnConflict: true, getRows: [], queries: [] };
    const { app } = buildApp(state);
    const res = await request(app)
      .post("/api/system/runtime-ack")
      .set("x-arbx-service-token", "WRONG_TOKEN")
      .send(VALID_PAYLOAD);
    expect(res.status).toBe(401);
    expect(state.queries.length).toBe(0);
  });

  it("accepts POST with valid service token + valid payload (202)", async () => {
    const state: MockState = { insertedOnConflict: true, getRows: [], queries: [] };
    const { app, io } = buildApp(state);
    const res = await request(app)
      .post("/api/system/runtime-ack")
      .set("x-arbx-service-token", SERVICE_TOKEN)
      .send(VALID_PAYLOAD);
    expect(res.status).toBe(202);
    expect(res.body).toMatchObject({ accepted: true, inserted: true, deduped: false });
    // Broadcast emitted on first insert.
    expect((io.to as ReturnType<typeof vi.fn>)).toHaveBeenCalledWith("runtime_ack");
  });

  it("returns 400 on schema_violation with valid token but bad payload", async () => {
    const state: MockState = { insertedOnConflict: true, getRows: [], queries: [] };
    const { app } = buildApp(state);
    const res = await request(app)
      .post("/api/system/runtime-ack")
      .set("x-arbx-service-token", SERVICE_TOKEN)
      .send({ ...VALID_PAYLOAD, layer: "invalid_layer" });
    expect(res.status).toBe(400);
    expect(res.body.error).toBe("schema_violation");
  });
});

describe("OMEGA-8/M3 P1-1: ON CONFLICT (event_id, layer) DO NOTHING dedup", () => {
  it("second identical POST is deduped — no second broadcast", async () => {
    const state: MockState = { insertedOnConflict: false, getRows: [], queries: [] };
    const { app, io } = buildApp(state);
    const res = await request(app)
      .post("/api/system/runtime-ack")
      .set("x-arbx-service-token", SERVICE_TOKEN)
      .send(VALID_PAYLOAD);
    expect(res.status).toBe(202);
    expect(res.body).toMatchObject({ accepted: true, inserted: false, deduped: true });
    // No broadcast on dedup.
    expect((io.to as ReturnType<typeof vi.fn>)).not.toHaveBeenCalled();
  });

  it("UPSERT uses ON CONFLICT (event_id, layer) DO NOTHING and advisory lock", async () => {
    const state: MockState = { insertedOnConflict: true, getRows: [], queries: [] };
    const { app } = buildApp(state);
    await request(app)
      .post("/api/system/runtime-ack")
      .set("x-arbx-service-token", SERVICE_TOKEN)
      .send(VALID_PAYLOAD);
    const insertQ = state.queries.find((q) => q.sql.includes("INSERT INTO runtime_ack"));
    expect(insertQ).toBeDefined();
    expect(insertQ!.sql).toContain("ON CONFLICT (event_id, layer) DO NOTHING");
    expect(insertQ!.sql).toContain("RETURNING id");
    // Advisory lock keyed by hashtext(resource:chain_id) ran inside a TX.
    const lockQ = state.queries.find((q) => q.sql.includes("pg_advisory_xact_lock"));
    expect(lockQ).toBeDefined();
    expect(lockQ!.params).toEqual(["runtime_ack:trading_config:1"]);
    // Wrapped in BEGIN/COMMIT.
    expect(state.queries[0].sql).toBe("BEGIN");
    expect(state.queries[state.queries.length - 1].sql).toBe("COMMIT");
  });
});

describe("OMEGA-8/M3 P1-4: GET /api/system/runtime-ack/:event_id fallback", () => {
  const validUuid = "22222222-2222-2222-2222-222222222222";

  it("rejects GET without admin token (401)", async () => {
    const state: MockState = { insertedOnConflict: true, getRows: [], queries: [] };
    const { app } = buildApp(state);
    const res = await request(app).get(`/api/system/runtime-ack/${validUuid}`);
    expect(res.status).toBe(401);
    expect(state.queries.length).toBe(0);
  });

  it("rejects GET with invalid UUID (400)", async () => {
    const state: MockState = { insertedOnConflict: true, getRows: [], queries: [] };
    const { app } = buildApp(state);
    const res = await request(app)
      .get("/api/system/runtime-ack/not-a-uuid")
      .set("x-arbx-admin-token", ADMIN_TOKEN);
    expect(res.status).toBe(400);
    expect(res.body).toEqual({ error: "invalid_event_id" });
    // No DB query for invalid UUID.
    expect(state.queries.length).toBe(0);
  });

  it("returns 404 when event_id not found", async () => {
    const state: MockState = { insertedOnConflict: true, getRows: [], queries: [] };
    const { app } = buildApp(state);
    const res = await request(app)
      .get(`/api/system/runtime-ack/${validUuid}`)
      .set("x-arbx-admin-token", ADMIN_TOKEN);
    expect(res.status).toBe(404);
    expect(res.body).toMatchObject({ error: "not_found", event_id: validUuid });
  });

  it("returns 200 + acks array when row exists, excludes idempotency_key", async () => {
    const state: MockState = {
      insertedOnConflict: true,
      getRows: [
        {
          event_id: validUuid,
          resource: "trading_config",
          chain_id: 1,
          config_hash_before: "a".repeat(64),
          config_hash_after: "b".repeat(64),
          worker_id: "searcher-rs-1",
          layer: "searcher_rs",
          status: "applied",
          latency_ms: 42,
          error: null,
          received_at: "2026-05-15T12:00:00Z",
        },
      ],
      queries: [],
    };
    const { app } = buildApp(state);
    const res = await request(app)
      .get(`/api/system/runtime-ack/${validUuid}`)
      .set("x-arbx-admin-token", ADMIN_TOKEN);
    expect(res.status).toBe(200);
    expect(res.body.event_id).toBe(validUuid);
    expect(res.body.acks).toHaveLength(1);
    expect(res.body.acks[0]).not.toHaveProperty("idempotency_key");
    expect(res.body.acks[0]).toMatchObject({
      resource: "trading_config",
      layer: "searcher_rs",
      status: "applied",
    });
  });
});
