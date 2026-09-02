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
 *   (f) healthy proxy, daemon 404 → 404 container_not_found
 *   (g) SERVICE-CTRL-01: proxy itself erroring (list 502 + inspect 502, the
 *       production DOCKER_GID drift) → 502 control_plane_error, NEVER a 404
 *   (h) proxy unreachable (network throw) → 502 control_plane_error
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
    expect(res.body.compose_project).toBe("arbitragex-v2");
  });

  it("(g) proxy itself erroring (list 502 + inspect 502 = the DOCKER_GID drift) → 502 control_plane_error, NOT a false 404", async () => {
    process.env["ARBX_SERVICE_CONTROL"] = "on";
    // SERVICE-CTRL-01 production evidence (2026-09-01): socket-proxy alive but
    // its daemon call fails → {ok:false, status:502} on both resolution passes.
    vi.stubGlobal(
      "fetch",
      vi.fn().mockResolvedValue({
        ok: false,
        status: 502,
        json: async () => ({ error: "upstream_error" }),
      }),
    );
    const res = await request(buildApp())
      .post("/api/v1/admin/services/searcher-rs/start")
      .set("x-arbx-admin-token", ADMIN_TOKEN);
    expect(res.status).toBe(502);
    expect(res.body.error).toBe("control_plane_error");
    expect(res.body.detail).toContain("inspect HTTP 502");
    expect(writeAudit).not.toHaveBeenCalled();
  });

  it("(h) proxy unreachable (network throw) → 502 control_plane_error with detail", async () => {
    process.env["ARBX_SERVICE_CONTROL"] = "on";
    vi.stubGlobal(
      "fetch",
      vi.fn().mockRejectedValue(new TypeError("fetch failed: connect EACCES")),
    );
    // searcher-rs is in this harness's allowlist (beforeEach); recon is not —
    // the point here is the proxy path, not the allowlist gate.
    const res = await request(buildApp())
      .post("/api/v1/admin/services/searcher-rs/start")
      .set("x-arbx-admin-token", ADMIN_TOKEN);
    expect(res.status).toBe(502);
    expect(res.body.error).toBe("control_plane_error");
    expect(res.body.detail).toContain("socket-proxy unreachable");
    expect(writeAudit).not.toHaveBeenCalled();
  });
});

describe("service control readiness — DAPP-SVCCTRL-READINESS (read-only evidence)", () => {
  it("(r1) without admin token → 401 (auth gate first, always)", async () => {
    const res = await request(buildApp()).get("/api/v1/admin/services/readiness");
    expect(res.status).toBe(401);
  });

  it("(r2) flag OFF → still 200 with control_enabled:false — the report observes the flag, it is not gated by it", async () => {
    process.env["ARBX_SERVICE_CONTROL"] = "off";
    vi.stubGlobal("fetch", vi.fn().mockRejectedValue(new TypeError("fetch failed")));
    const res = await request(buildApp())
      .get("/api/v1/admin/services/readiness")
      .set("x-arbx-admin-token", ADMIN_TOKEN);
    expect(res.status).toBe(200);
    expect(res.body.read_only).toBe(true);
    expect(res.body.control_enabled).toBe(false);
  });

  it("(r3) healthy proxy → full evidence report: flag, proxy, allowlist, per-service state, audit, deploy sha — zero audit rows", async () => {
    process.env["ARBX_SERVICE_CONTROL"] = "on";
    // Probe (containers/json?limit=1) + one label-list per allowlisted service,
    // each answering a running container — the happy path end to end.
    vi.stubGlobal(
      "fetch",
      vi.fn().mockResolvedValue({
        ok: true,
        status: 200,
        json: async () => [{ Id: "abc123", State: "running" }],
      }),
    );
    const res = await request(buildApp())
      .get("/api/v1/admin/services/readiness")
      .set("x-arbx-admin-token", ADMIN_TOKEN);
    expect(res.status).toBe(200);
    expect(res.body.kind).toBe("service_control_readiness");
    expect(res.body.read_only).toBe(true);
    expect(res.body.control_enabled).toBe(true);
    expect(res.body.proxy).toMatchObject({ reachable: true, status: 200 });
    expect(res.body.proxy.url).toBe("http://socket-proxy:2375");
    expect(res.body.compose_project).toBe("arbitragex-v2");
    expect(res.body.allowlist).toEqual(["searcher-rs", "sim-ctl", "token-enricher"]);
    expect(res.body.services).toHaveLength(3);
    for (const svc of res.body.services) {
      expect(svc).toMatchObject({ allowed: true, resolution: "container", state: "running" });
    }
    expect(res.body.audit).toEqual({ writer: "wired" });
    expect(res.body.deploy.sha).toBe("unknown"); // R8: no ARBX_DEPLOY_SHA in this harness
    expect(typeof res.body.ts).toBe("string");
    // Read-only proof: resolving N services wrote NO audit rows.
    expect(writeAudit).not.toHaveBeenCalled();
  });

  it("(r4) proxy unreachable → 200 honest report: reachable:false + every service proxy_error (never fabricated running)", async () => {
    process.env["ARBX_SERVICE_CONTROL"] = "on";
    vi.stubGlobal("fetch", vi.fn().mockRejectedValue(new TypeError("connect EACCES")));
    const res = await request(buildApp())
      .get("/api/v1/admin/services/readiness")
      .set("x-arbx-admin-token", ADMIN_TOKEN);
    expect(res.status).toBe(200);
    expect(res.body.proxy.reachable).toBe(false);
    expect(res.body.proxy.detail).toContain("EACCES");
    expect(res.body.services).toHaveLength(3);
    for (const svc of res.body.services) {
      expect(svc.resolution).toBe("proxy_error");
      expect(svc.state).toBeUndefined();
    }
  });

  it("(r5) mixed resolution: healthy proxy, one container genuinely absent → not_found reported verbatim", async () => {
    process.env["ARBX_SERVICE_CONTROL"] = "on";
    // Probe answers; per-service label lists: empty for sim-ctl (then its
    // deterministic-name inspect 404s = genuinely absent), one running
    // container for the other two.
    vi.stubGlobal(
      "fetch",
      vi.fn((url: string) => {
        if (url.includes("/containers/") && url.endsWith("/json")) {
          return Promise.resolve({ ok: false, status: 404, json: async () => null });
        }
        const filters = decodeURIComponent(url.split("filters=")[1] ?? "");
        if (filters.includes("sim-ctl")) {
          return Promise.resolve({ ok: true, status: 200, json: async () => [] });
        }
        return Promise.resolve({ ok: true, status: 200, json: async () => [{ Id: "id1", State: "running" }] });
      }),
    );
    const res = await request(buildApp())
      .get("/api/v1/admin/services/readiness")
      .set("x-arbx-admin-token", ADMIN_TOKEN);
    expect(res.status).toBe(200);
    const byName = Object.fromEntries(res.body.services.map((s: { name: string }) => [s.name, s]));
    expect(byName["searcher-rs"].resolution).toBe("container");
    expect(byName["sim-ctl"].resolution).toBe("not_found");
    expect(byName["token-enricher"].resolution).toBe("container");
  });
});
