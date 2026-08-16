/**
 * readiness-evidence registry — unit tests (G-SIM-1 FASE 2, gate G1).
 *
 * Mocks the pg pool (client-level recording of SQL/params); never touches a
 * real Postgres. Mirrors the service-control.test.ts harness (vitest +
 * express + supertest + a test-only requireAdminToken mirroring the shared
 * middleware contract).
 *
 * Cases:
 *   (a)  POST without admin token → 401 (before any DB access)
 *   (b)  POST valid → 201 + BOTH history insert and ON CONFLICT upsert issued
 *        in order inside BEGIN/COMMIT, parameterized only
 *   (c)  POST bad status enum → 400
 *   (d)  POST empty required fields → 400
 *   (e)  POST unknown item_key for G-SIM-1 → 400; arbitrary key OK for other gates
 *   (f)  POST mid-transaction failure → ROLLBACK + 500 (nothing half-written)
 *   (f2) POST pool.connect() throws (DB unreachable) → 503 db_unavailable
 *        with a reason — NOT an unhandled rejection
 *   (g)  pool null → 503 db_unavailable (POST + GET)
 *   (h)  GET without token → 401; missing gate_id → 400
 *   (i)  GET returns latest-per-item with is_fresh true/false around the
 *        30-day boundary (strict >; clock injected + mocked)
 */
import { describe, it, expect, vi } from "vitest";
import express, { type Express, type RequestHandler } from "express";
import request from "supertest";
import type pg from "pg";
import { mountReadinessEvidence, computeIsFresh, FRESHNESS_DAYS } from "./readiness-evidence.js";

const ADMIN_TOKEN = "test-admin-token-32-bytes-of-entropy-aaaa";

/** Test-only requireAdminToken — mirrors @arbx/shared contract. */
function requireAdminToken(expected: string): RequestHandler {
  return (req, res, next) => {
    const got = req.header("x-arbx-admin-token");
    if (!got || got !== expected) {
      res.status(401).json({ error: "unauthorized", source: "admin_token" });
      return;
    }
    next();
  };
}

const logger = { warn: vi.fn() };

// Deterministic clock for is_fresh assertions.
const NOW = new Date("2026-08-16T12:00:00.000Z");
const DAY_MS = 24 * 60 * 60 * 1000;

interface RecordedQuery {
  sql: string;
  params: unknown[];
}

/**
 * Mock pg pool. Client-level queries (POST transaction) and pool-level queries
 * (GET) are all recorded in order. The upsert INSERT returns the canned
 * verified_at so the 201 body is deterministic.
 */
function makeMockPool(opts?: {
  clientFailOn?: RegExp;
  getRows?: Record<string, unknown>[];
  connectFail?: boolean;
}): { pool: pg.Pool; queries: RecordedQuery[] } {
  const queries: RecordedQuery[] = [];
  const client = {
    query: async (sql: string, params?: unknown[]): Promise<{ rows: Record<string, unknown>[] }> => {
      // Record BEFORE a forced failure so the test sees the attempted SQL too.
      queries.push({ sql, params: params ?? [] });
      if (opts?.clientFailOn && opts.clientFailOn.test(sql)) throw new Error("boom");
      if (sql.startsWith("INSERT INTO readiness_evidence\n") || sql.includes("ON CONFLICT (gate_id, item_key)")) {
        return { rows: [{ verified_at: NOW }] };
      }
      return { rows: [] };
    },
    release: vi.fn(),
  };
  const pool = {
    connect: async () => {
      if (opts?.connectFail) throw new Error("connect refused");
      return client;
    },
    query: async (sql: string, params?: unknown[]): Promise<{ rows: Record<string, unknown>[] }> => {
      queries.push({ sql, params: params ?? [] });
      return { rows: opts?.getRows ?? [] };
    },
  };
  return { pool: pool as unknown as pg.Pool, queries };
}

function buildApp(pool: pg.Pool | null): Express {
  const app = express();
  app.use(express.json());
  mountReadinessEvidence(app, {
    pool,
    requireAdminToken,
    adminToken: ADMIN_TOKEN,
    logger,
    now: () => NOW,
  });
  return app;
}

const VALID_BODY = {
  gate_id: "G-SIM-1",
  item_key: "unit_tests",
  status: "evidenced",
  evidence_ref: "https://github.com/hefarica/arbitragex-v2/actions/runs/123456",
  detail: { suite: "cargo test --lib", passed: 412 },
  verified_by: "ci:rust.yml",
};

describe("readiness evidence registry — POST /admin/readiness-evidence", () => {
  it("(a) without admin token → 401, no DB access", async () => {
    const { pool, queries } = makeMockPool();
    const res = await request(buildApp(pool)).post("/admin/readiness-evidence").send(VALID_BODY);
    expect(res.status).toBe(401);
    expect(queries).toHaveLength(0);
  });

  it("(b) valid write → 201; history insert THEN upsert inside BEGIN/COMMIT, parameterized", async () => {
    const { pool, queries } = makeMockPool();
    const res = await request(buildApp(pool))
      .post("/admin/readiness-evidence")
      .set("x-arbx-admin-token", ADMIN_TOKEN)
      .send(VALID_BODY);

    expect(res.status).toBe(201);
    expect(res.body).toEqual({
      ok: true,
      gate_id: "G-SIM-1",
      item_key: "unit_tests",
      verified_at: NOW.toISOString(),
    });

    // Exact transaction shape: BEGIN → history INSERT → upsert → COMMIT.
    expect(queries).toHaveLength(4);
    expect(queries[0]!.sql).toBe("BEGIN");
    // History BEFORE upsert (append-only trail never misses an accepted write).
    expect(queries[1]!.sql).toContain("INSERT INTO readiness_evidence_history");
    expect(queries[2]!.sql).toContain("ON CONFLICT (gate_id, item_key) DO UPDATE");
    expect(queries[3]!.sql).toBe("COMMIT");
    const history = queries[1]!;
    const upsert = queries[2]!;
    // Parameterized only — no value interpolated into the SQL text.
    for (const q of [history, upsert]) {
      expect(q.sql).not.toContain("G-SIM-1");
      expect(q.sql).not.toContain("ci:rust.yml");
      expect(q.sql.match(/\$\d/g)).toHaveLength(7);
    }
    expect(history.params).toEqual([
      "G-SIM-1",
      "unit_tests",
      "evidenced",
      "https://github.com/hefarica/arbitragex-v2/actions/runs/123456",
      JSON.stringify(VALID_BODY.detail),
      NOW,
      "ci:rust.yml",
    ]);
    // Same timestamp object flows to BOTH rows (history PK == latest row).
    expect(upsert.params).toEqual(history.params);
    // detail optional → null when absent.
  });

  it("(b2) detail omitted → null param, still 201", async () => {
    const { pool, queries } = makeMockPool();
    const noDetail: Omit<typeof VALID_BODY, "detail"> = {
      gate_id: VALID_BODY.gate_id,
      item_key: VALID_BODY.item_key,
      status: VALID_BODY.status,
      evidence_ref: VALID_BODY.evidence_ref,
      verified_by: VALID_BODY.verified_by,
    };
    const res = await request(buildApp(pool))
      .post("/admin/readiness-evidence")
      .set("x-arbx-admin-token", ADMIN_TOKEN)
      .send(noDetail);
    expect(res.status).toBe(201);
    expect(queries[1]!.params[4]).toBeNull();
  });

  it("(c) bad status enum → 400, no DB access", async () => {
    const { pool, queries } = makeMockPool();
    const res = await request(buildApp(pool))
      .post("/admin/readiness-evidence")
      .set("x-arbx-admin-token", ADMIN_TOKEN)
      .send({ ...VALID_BODY, status: "maybe" });
    expect(res.status).toBe(400);
    expect(res.body.error).toBe("invalid_request");
    expect(queries).toHaveLength(0);
  });

  it("(d) empty required fields → 400 each", async () => {
    const { pool } = makeMockPool();
    for (const field of ["gate_id", "item_key", "evidence_ref", "verified_by"] as const) {
      const res = await request(buildApp(pool))
        .post("/admin/readiness-evidence")
        .set("x-arbx-admin-token", ADMIN_TOKEN)
        .send({ ...VALID_BODY, [field]: "" });
      expect(res.status).toBe(400);
      expect(res.body.error).toBe("invalid_request");
    }
  });

  it("(e) unknown item_key for G-SIM-1 → 400 with allowlist", async () => {
    const { pool, queries } = makeMockPool();
    const res = await request(buildApp(pool))
      .post("/admin/readiness-evidence")
      .set("x-arbx-admin-token", ADMIN_TOKEN)
      .send({ ...VALID_BODY, item_key: "vibes_only" });
    expect(res.status).toBe(400);
    expect(res.body.error).toBe("invalid_item_key");
    expect(res.body.allowed).toContain("second_signoff");
    expect(queries).toHaveLength(0);
  });

  it("(e2) arbitrary item_key accepted for a non-G-SIM-1 gate → 201", async () => {
    const { pool } = makeMockPool();
    const res = await request(buildApp(pool))
      .post("/admin/readiness-evidence")
      .set("x-arbx-admin-token", ADMIN_TOKEN)
      .send({ ...VALID_BODY, gate_id: "G-DEPLOY-1", item_key: "custom_check" });
    expect(res.status).toBe(201);
    expect(res.body.gate_id).toBe("G-DEPLOY-1");
  });

  it("(f) mid-transaction failure → ROLLBACK + 500", async () => {
    const { pool, queries } = makeMockPool({ clientFailOn: /readiness_evidence_history/ });
    const res = await request(buildApp(pool))
      .post("/admin/readiness-evidence")
      .set("x-arbx-admin-token", ADMIN_TOKEN)
      .send(VALID_BODY);
    expect(res.status).toBe(500);
    expect(res.body.error).toBe("readiness_evidence_write_failed");
    // BEGIN → history INSERT (throws) → ROLLBACK. Upsert never ran, no COMMIT.
    expect(queries).toHaveLength(3);
    expect(queries[0]!.sql).toBe("BEGIN");
    expect(queries[1]!.sql).toContain("INSERT INTO readiness_evidence_history");
    expect(queries[2]!.sql).toBe("ROLLBACK");
  });

  it("(f2) pool.connect() throws (DB unreachable) → 503 db_unavailable with reason, zero queries", async () => {
    const { pool, queries } = makeMockPool({ connectFail: true });
    const res = await request(buildApp(pool))
      .post("/admin/readiness-evidence")
      .set("x-arbx-admin-token", ADMIN_TOKEN)
      .send(VALID_BODY);
    expect(res.status).toBe(503);
    expect(res.body.error).toBe("db_unavailable");
    expect(res.body.detail).toBe("connect refused");
    // No transaction ever started — nothing attempted against the DB.
    expect(queries).toHaveLength(0);
  });

  it("(g) pool null → 503 db_unavailable", async () => {
    const res = await request(buildApp(null))
      .post("/admin/readiness-evidence")
      .set("x-arbx-admin-token", ADMIN_TOKEN)
      .send(VALID_BODY);
    expect(res.status).toBe(503);
    expect(res.body.error).toBe("db_unavailable");
  });
});

describe("readiness evidence registry — GET /admin/readiness-evidence", () => {
  it("(h) without admin token → 401; missing gate_id → 400", async () => {
    const { pool } = makeMockPool();
    const app = buildApp(pool);
    const unauth = await request(app).get("/admin/readiness-evidence?gate_id=G-SIM-1");
    expect(unauth.status).toBe(401);
    const noGate = await request(app)
      .get("/admin/readiness-evidence")
      .set("x-arbx-admin-token", ADMIN_TOKEN);
    expect(noGate.status).toBe(400);
    expect(noGate.body.error).toBe("missing_gate_id");
  });

  it("(i) latest-per-item + is_fresh around the 30-day boundary (mocked now)", async () => {
    const rows = [
      { gate_id: "G-SIM-1", item_key: "unit_tests", status: "evidenced", evidence_ref: "u1", detail: null,
        verified_at: new Date(NOW.getTime() - 1 * DAY_MS), verified_by: "ci:rust.yml" },        // fresh
      { gate_id: "G-SIM-1", item_key: "fork_suite", status: "evidenced", evidence_ref: "u2", detail: null,
        verified_at: new Date(NOW.getTime() - 29 * DAY_MS), verified_by: "ci:a4.yml" },          // fresh (edge)
      { gate_id: "G-SIM-1", item_key: "dep_tree", status: "failed", evidence_ref: "u3", detail: null,
        verified_at: new Date(NOW.getTime() - 31 * DAY_MS), verified_by: "operator:hefarica" },  // stale
      { gate_id: "G-SIM-1", item_key: "variance_benchmark", status: "evidenced", evidence_ref: "u4", detail: null,
        verified_at: new Date(NOW.getTime() - FRESHNESS_DAYS * DAY_MS), verified_by: "ci:bench.yml" }, // exactly 30d → stale (strict >)
    ];
    const { pool, queries } = makeMockPool({ getRows: rows });
    const res = await request(buildApp(pool))
      .get("/admin/readiness-evidence?gate_id=G-SIM-1")
      .set("x-arbx-admin-token", ADMIN_TOKEN);

    expect(res.status).toBe(200);
    expect(res.body.gate_id).toBe("G-SIM-1");
    expect(res.body.generated_at).toBe(NOW.toISOString());
    const byKey: Record<string, { is_fresh: boolean; status: string }> = Object.fromEntries(
      res.body.items.map((i: { item_key: string; is_fresh: boolean; status: string }) => [i.item_key, i]),
    );
    expect(byKey["unit_tests"]!.is_fresh).toBe(true);
    expect(byKey["fork_suite"]!.is_fresh).toBe(true);
    expect(byKey["dep_tree"]!.is_fresh).toBe(false);
    expect(byKey["variance_benchmark"]!.is_fresh).toBe(false);
    // Read-only: single parameterized SELECT, nothing else.
    expect(queries).toHaveLength(1);
    expect(queries[0]!.sql).toContain("DISTINCT ON (item_key)");
    expect(queries[0]!.sql).not.toContain("G-SIM-1");
    expect(queries[0]!.params).toEqual(["G-SIM-1"]);
  });

  it("(i2) computeIsFresh pure boundary checks", () => {
    expect(computeIsFresh(NOW.toISOString(), NOW)).toBe(true);
    expect(computeIsFresh(new Date(NOW.getTime() - FRESHNESS_DAYS * DAY_MS + 1).toISOString(), NOW)).toBe(true);
    expect(computeIsFresh(new Date(NOW.getTime() - FRESHNESS_DAYS * DAY_MS).toISOString(), NOW)).toBe(false);
    expect(computeIsFresh("not-a-date", NOW)).toBe(false);
  });

  it("(g2) pool null → 503 db_unavailable", async () => {
    const res = await request(buildApp(null))
      .get("/admin/readiness-evidence?gate_id=G-SIM-1")
      .set("x-arbx-admin-token", ADMIN_TOKEN);
    expect(res.status).toBe(503);
  });
});
