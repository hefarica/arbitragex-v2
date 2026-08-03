/**
 * A8 stubs — integration tests (audit 2026-05-10).
 *
 * Confirms each stub:
 *   - returns HTTP 501
 *   - returns Content-Type application/json
 *   - returns structured body { error, message, feature, roadmap }
 *   - admin-gated stubs reject missing/wrong tokens BEFORE returning 501
 */

import { describe, it, expect, beforeAll } from "vitest";
import express, { type Express, type RequestHandler } from "express";
import request from "supertest";
import { mountStubs } from "./stubs.js";

const ADMIN_TOKEN = "test-admin-token-32-bytes-of-entropy-aaaa";

/** Test-only requireAdminToken — mirrors the shared middleware contract. */
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

function buildApp(): Express {
  const app = express();
  app.use(express.json());
  mountStubs(app, { requireAdminToken, adminToken: ADMIN_TOKEN });
  return app;
}

interface StubBody {
  error: string;
  message: string;
  feature: string;
  roadmap: string;
}

function expectStubBody(body: unknown): asserts body is StubBody {
  expect(body).toMatchObject({
    error: "not_implemented",
    message: expect.any(String),
    feature: expect.any(String),
    roadmap: expect.any(String),
  });
}

describe("A8 stubs (audit 2026-05-10)", () => {
  let app: Express;

  beforeAll(() => {
    app = buildApp();
  });

  describe("public stubs (no auth)", () => {
    it("GET /api/v1/wallets → 501 with structured body", async () => {
      const res = await request(app).get("/api/v1/wallets");
      expect(res.status).toBe(501);
      expect(res.headers["content-type"]).toMatch(/application\/json/);
      expectStubBody(res.body);
      expect(res.body.feature).toContain("wallets");
    });

    it("GET /api/v1/wallets/:addr/balances → 501", async () => {
      const res = await request(app).get(
        "/api/v1/wallets/0xdeadbeef00000000000000000000000000000000/balances",
      );
      expect(res.status).toBe(501);
      expectStubBody(res.body);
      expect(res.body.feature).toContain("balances");
    });

    it("GET /api/v1/wallets/:addr/allowances → 501", async () => {
      const res = await request(app).get(
        "/api/v1/wallets/0xdeadbeef00000000000000000000000000000000/allowances",
      );
      expect(res.status).toBe(501);
      expectStubBody(res.body);
      expect(res.body.feature).toContain("allowances");
    });

    it("POST /api/v1/opportunities/:id/simulate → 501", async () => {
      const res = await request(app)
        .post("/api/v1/opportunities/abc-123/simulate")
        .send({});
      expect(res.status).toBe(501);
      expectStubBody(res.body);
      expect(res.body.feature).toContain("simulate");
    });
  });

  describe("admin-gated stubs", () => {
    // NOTE: /api/v1/admin/services/:name/{start,stop} were stubs tested here —
    // they are now a REAL handler (routes/service-control.ts) with their own
    // tests in service-control.test.ts. Admin-gating (401) is covered there too.

    it("POST /admin/alertmanager/webhook → 401 without token", async () => {
      const res = await request(app).post("/admin/alertmanager/webhook").send({});
      expect(res.status).toBe(401);
    });

    it("POST /admin/alertmanager/webhook → 501 with valid token", async () => {
      const res = await request(app)
        .post("/admin/alertmanager/webhook")
        .set("x-arbx-admin-token", ADMIN_TOKEN)
        .send({ alerts: [] });
      expect(res.status).toBe(501);
      expectStubBody(res.body);
    });

    it("PUT /api/v1/dexes/:id/active → 401 without token", async () => {
      const res = await request(app)
        .put("/api/v1/dexes/00000000-0000-0000-0000-000000000000/active")
        .send({ active: true });
      expect(res.status).toBe(401);
    });

    it("PUT /api/v1/dexes/:id/active → 501 with valid token + pointer to canonical writer", async () => {
      const res = await request(app)
        .put("/api/v1/dexes/00000000-0000-0000-0000-000000000000/active")
        .set("x-arbx-admin-token", ADMIN_TOKEN)
        .send({ active: true });
      expect(res.status).toBe(501);
      expectStubBody(res.body);
      expect(res.body.message).toContain("trading-config");
    });
  });
});
