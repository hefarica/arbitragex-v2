/**
 * Service control plane — unit tests.
 *
 * Mocks the socket-proxy `fetch` and the `writeAudit` dep; never touches a real
 * docker socket. Mirrors the stubs.test.ts harness (vitest + express + supertest
 * + a test-only requireAdminToken mirroring the shared middleware contract).
 *
 * Cases:
 *   (a) flag off  → 501
 *   (b) non-allowlisted name → 400
 *   (c) invalid name (regex fail) → 400
 *   (d) flag on + proxy ok → 200 + audit row
 *   (e) proxy error → 502, no audit row
 */
import { describe, it, expect, beforeEach, afterEach, vi } from "vitest";
import express, { type Express, type RequestHandler } from "express";
import request from "supertest";
import { buildServiceControlRouter } from "./service-control.js";

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

const writeAudit = vi.fn().mockResolvedValue(undefined);

function buildApp(): Express {
  const app = express();
  app.use(express.json());
  app.use(
    buildServiceControlRouter({
      requireAdminToken,
      adminToken: ADMIN_TOKEN,
      writeAudit,
      reqUA: () => "ua-hash",
      logger: { warn: vi.fn(), error: vi.fn() },
    }),
  );
  return app;
}

beforeEach(() => {
  vi.restoreAllMocks();
  writeAudit.mockClear();
  writeAudit.mockResolvedValue(undefined);
  process.env["ARBX_SERVICE_CONTROL_ALLOWLIST"] = "searcher-rs,sim-ctl,token-enricher";
  process.env["DOCKER_PROXY_URL"] = "http://socket-proxy:2375";
  delete process.env["ARBX_SERVICE_CONTROL"];
});

afterEach(() => {
  vi.unstubAllGlobals();
});

describe("service control plane", () => {
  it("(a) flag off → 501 even with a valid admin token", async () => {
    process.env["ARBX_SERVICE_CONTROL"] = "off";
    const res = await request(buildApp())
      .post("/api/v1/admin/services/searcher-rs/start")
      .set("x-arbx-admin-token", ADMIN_TOKEN);
    expect(res.status).toBe(501);
    expect(res.body.error).toBe("not_implemented");
    expect(writeAudit).not.toHaveBeenCalled();
  });

  it("(a2) without admin token → 401 (gate runs before the flag check)", async () => {
    process.env["ARBX_SERVICE_CONTROL"] = "on";
    const res = await request(buildApp()).post("/api/v1/admin/services/searcher-rs/start");
    expect(res.status).toBe(401);
  });

  it("(b) non-allowlisted service → 400", async () => {
    process.env["ARBX_SERVICE_CONTROL"] = "on";
    const res = await request(buildApp())
      .post("/api/v1/admin/services/postgres/stop")
      .set("x-arbx-admin-token", ADMIN_TOKEN);
    expect(res.status).toBe(400);
    expect(res.body.error).toBe("service_not_controllable");
    expect(writeAudit).not.toHaveBeenCalled();
  });

  it("(c) invalid name (fails ^[a-z0-9-]+$) → 400", async () => {
    process.env["ARBX_SERVICE_CONTROL"] = "on";
    const res = await request(buildApp())
      .post("/api/v1/admin/services/foo.bar/start")
      .set("x-arbx-admin-token", ADMIN_TOKEN);
    expect(res.status).toBe(400);
    expect(res.body.error).toBe("service_not_controllable");
  });

  it("(d) flag on + proxy 204 → 200 + audit row written", async () => {
    process.env["ARBX_SERVICE_CONTROL"] = "on";
    vi.stubGlobal(
      "fetch",
      vi
        .fn()
        .mockResolvedValueOnce({
          ok: true,
          json: async () => [{ Id: "abc123", State: "exited" }],
        })
        .mockResolvedValueOnce({ status: 204 }),
    );

    const res = await request(buildApp())
      .post("/api/v1/admin/services/searcher-rs/start")
      .set("x-arbx-admin-token", ADMIN_TOKEN);

    expect(res.status).toBe(200);
    expect(res.body).toMatchObject({
      service: "searcher-rs",
      action: "start",
      status: "running",
    });
    expect(writeAudit).toHaveBeenCalledTimes(1);
    expect(writeAudit).toHaveBeenCalledWith(
      "service.start",
      "admin",
      "service",
      "searcher-rs",
      "exited",
      "running",
      expect.any(String),
      null,
      "ua-hash",
    );
  });

  it("(d2) proxy 304 (already in state) → 200 (idempotent)", async () => {
    process.env["ARBX_SERVICE_CONTROL"] = "on";
    vi.stubGlobal(
      "fetch",
      vi
        .fn()
        .mockResolvedValueOnce({
          ok: true,
          json: async () => [{ Id: "abc123", State: "running" }],
        })
        .mockResolvedValueOnce({ status: 304 }),
    );
    const res = await request(buildApp())
      .post("/api/v1/admin/services/searcher-rs/start")
      .set("x-arbx-admin-token", ADMIN_TOKEN);
    expect(res.status).toBe(200);
  });

  it("(e) proxy 500 → 502, no audit row", async () => {
    process.env["ARBX_SERVICE_CONTROL"] = "on";
    vi.stubGlobal(
      "fetch",
      vi
        .fn()
        .mockResolvedValueOnce({
          ok: true,
          json: async () => [{ Id: "abc123", State: "running" }],
        })
        .mockResolvedValueOnce({ status: 500 }),
    );
    const res = await request(buildApp())
      .post("/api/v1/admin/services/searcher-rs/stop")
      .set("x-arbx-admin-token", ADMIN_TOKEN);
    expect(res.status).toBe(502);
    expect(res.body.error).toBe("control_plane_error");
    expect(writeAudit).not.toHaveBeenCalled();
  });

  it("(f) container not found → 404", async () => {
    process.env["ARBX_SERVICE_CONTROL"] = "on";
    // list empty + fallback inspect 404
    vi.stubGlobal(
      "fetch",
      vi.fn().mockResolvedValue({ ok: false, status: 404, json: async () => null }),
    );
    const res = await request(buildApp())
      .post("/api/v1/admin/services/token-enricher/start")
      .set("x-arbx-admin-token", ADMIN_TOKEN);
    expect(res.status).toBe(404);
    expect(res.body.error).toBe("container_not_found");
  });
});
