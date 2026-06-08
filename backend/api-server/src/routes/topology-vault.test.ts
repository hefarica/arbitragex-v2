import { mkdtemp, readFile, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import path from "node:path";
import express, { type RequestHandler } from "express";
import request from "supertest";
import { afterEach, describe, expect, it, vi } from "vitest";
import {
  __forTesting,
  buildTopologyVaultRouter,
  type TopologyMutationCommitted,
} from "./topology-vault.js";

const ALCHEMY_SECRET = "alchemySuperSecretKey1234abcd";
const QUICKNODE_SECRET = "quicknodeSuperSecret5678wxyz";
const HTTP_SECRET = "httpSecret9876zzzz";

function adminGate(): RequestHandler {
  return (_req, _res, next) => next();
}

async function makeApp() {
  const dir = await mkdtemp(path.join(tmpdir(), "arbx-topology-vault-"));
  const published: Array<{ channel: string; message: string }> = [];
  const audit = vi.fn(async () => undefined);
  const app = express();
  app.use(express.json());
  app.use(buildTopologyVaultRouter({
    redis: {
      publish: async (channel: string, message: string) => {
        published.push({ channel, message });
        return 2;
      },
    },
    requireAdminToken: () => adminGate(),
    adminToken: "test-admin-token-with-high-entropy-000000",
    writeAudit: audit,
    logger: {
      info: vi.fn(),
      warn: vi.fn(),
      error: vi.fn(),
    },
    vaultPath: path.join(dir, "topology-vault.json"),
    now: () => new Date("2026-05-23T20:00:00.000Z"),
  }));
  return { app, dir, published, audit };
}

afterEach(() => {
  vi.restoreAllMocks();
});

describe("Topology Vault primitives", () => {
  it("redacts provider URLs without leaking API keys", () => {
    const masked = __forTesting.maskUrl(`wss://eth-mainnet.g.alchemy.com/v2/${ALCHEMY_SECRET}`);
    expect(masked).toContain("wss://...");
    expect(masked).toContain("alchemy.com");
    expect(masked).toContain("****abcd");
    expect(masked).not.toContain(ALCHEMY_SECRET);
  });

  it("requires Alchemy as first WS provider when mempool mode is filtered", () => {
    expect(() => __forTesting.assertMutationCompatible({
      scope: "staging",
      chain_id: 1,
      mempool_mode: "filtered",
      rpc_http_1: `alchemy=https://eth-mainnet.g.alchemy.com/v2/${HTTP_SECRET}`,
      rpc_ws_1: `quicknode=wss://example.quiknode.pro/${QUICKNODE_SECRET},alchemy=wss://eth-mainnet.g.alchemy.com/v2/${ALCHEMY_SECRET}`,
    })).toThrow("filtered_requires_first_ws_alchemy");

    expect(() => __forTesting.assertMutationCompatible({
      scope: "staging",
      chain_id: 1,
      mempool_mode: "filtered",
      rpc_http_1: `alchemy=https://eth-mainnet.g.alchemy.com/v2/${HTTP_SECRET}`,
      rpc_ws_1: `alchemy=wss://eth-mainnet.g.alchemy.com/v2/${ALCHEMY_SECRET},quicknode=wss://example.quiknode.pro/${QUICKNODE_SECRET}`,
    })).not.toThrow();
  });

  it("rejects explicit proxy hosts", () => {
    expect(() => __forTesting.parseProviderList("proxy=wss://demo.ngrok-free.app/rpc", "ws")).toThrow("proxy_forbidden");
    expect(() => __forTesting.parseProviderList("edge=wss://rpc-proxy.example.com/ws", "ws")).toThrow("proxy_forbidden");
  });
});

describe("Topology Vault endpoints", () => {
  it("POST persists full server-side topology but returns only a redacted snapshot", async () => {
    const { app, dir, published, audit } = await makeApp();
    try {
      const res = await request(app)
        .post("/api/admin/topology/mutations")
        .send({
          scope: "staging",
          chain_id: 1,
          mempool_mode: "filtered",
          rpc_http_1: `alchemy=https://eth-mainnet.g.alchemy.com/v2/${HTTP_SECRET}`,
          rpc_ws_1: `alchemy=wss://eth-mainnet.g.alchemy.com/v2/${ALCHEMY_SECRET},quicknode=wss://example.quiknode.pro/${QUICKNODE_SECRET}`,
        })
        .expect(202);

      const bodyText = JSON.stringify(res.body);
      expect(bodyText).not.toContain(ALCHEMY_SECRET);
      expect(bodyText).not.toContain(QUICKNODE_SECRET);
      expect(bodyText).not.toContain(HTTP_SECRET);
      expect(res.body.topology.rpc_ws_1[0].provider_kind).toBe("alchemy");
      expect(res.body.topology.rpc_ws_1[0].url_masked).toContain("****abcd");
      expect(res.body.published_to_channel).toBe("arbx:topology:mutation");
      expect(res.body.subscribers_notified).toBe(2);

      const stored = await readFile(path.join(dir, "topology-vault.json"), "utf8");
      expect(stored).toContain(ALCHEMY_SECRET);
      expect(stored).toContain(QUICKNODE_SECRET);
      expect(published).toHaveLength(1);
      const event = JSON.parse(published[0]!.message) as TopologyMutationCommitted;
      expect(published[0]!.channel).toBe("arbx:topology:mutation");
      expect(event.event).toBe("topology.mutation.committed");
      expect(event.version_id).toBe(Math.floor(new Date("2026-05-23T20:00:00.000Z").getTime() / 1000));
      expect(event.rpc_ws_1).toContain(ALCHEMY_SECRET);
      expect(event.checksum).toMatch(/^[a-f0-9]{64}$/);
      expect(audit).toHaveBeenCalledTimes(1);
      expect(JSON.stringify(audit.mock.calls)).not.toContain(ALCHEMY_SECRET);
    } finally {
      await rm(dir, { recursive: true, force: true });
    }
  });

  it("GET snapshot returns the persisted topology without leaking credentials", async () => {
    const { app, dir } = await makeApp();
    try {
      await request(app)
        .post("/api/admin/topology/mutations")
        .send({
          mempool_mode: "filtered",
          rpc_http_1: `alchemy=https://eth-mainnet.g.alchemy.com/v2/${HTTP_SECRET}`,
          rpc_ws_1: `alchemy=wss://eth-mainnet.g.alchemy.com/v2/${ALCHEMY_SECRET}`,
        })
        .expect(202);

      const res = await request(app)
        .get("/api/admin/topology/snapshot")
        .expect(200);

      const bodyText = JSON.stringify(res.body);
      expect(res.body.ok).toBe(true);
      expect(res.body.ui.background).toBe("#FFFFFF");
      expect(res.body.ui.mode_options).toEqual(["auto", "filtered", "firehose"]);
      expect(bodyText).not.toContain(ALCHEMY_SECRET);
      expect(bodyText).not.toContain(HTTP_SECRET);
      expect(res.body.topology.rpc_ws_1[0].url_masked).toContain("****abcd");
    } finally {
      await rm(dir, { recursive: true, force: true });
    }
  });

  it("rejects filtered mode when the first WS provider is not Alchemy", async () => {
    const { app, dir, published } = await makeApp();
    try {
      const res = await request(app)
        .post("/api/admin/topology/mutations")
        .send({
          mempool_mode: "filtered",
          rpc_http_1: `alchemy=https://eth-mainnet.g.alchemy.com/v2/${HTTP_SECRET}`,
          rpc_ws_1: `publicnode=wss://ethereum-rpc.publicnode.com,alchemy=wss://eth-mainnet.g.alchemy.com/v2/${ALCHEMY_SECRET}`,
        })
        .expect(400);

      expect(res.body.error).toBe("filtered_requires_first_ws_alchemy");
      expect(JSON.stringify(res.body)).not.toContain(ALCHEMY_SECRET);
      expect(published).toHaveLength(0);
    } finally {
      await rm(dir, { recursive: true, force: true });
    }
  });

  it("rejects third-party proxy URLs and never publishes them", async () => {
    const { app, dir, published } = await makeApp();
    try {
      const res = await request(app)
        .post("/api/admin/topology/mutations")
        .send({
          mempool_mode: "firehose",
          rpc_http_1: `alchemy=https://eth-mainnet.g.alchemy.com/v2/${HTTP_SECRET}`,
          rpc_ws_1: "proxy=wss://test-proxy.ngrok-free.app/ws",
        })
        .expect(400);

      expect(res.body.error).toBe("proxy_forbidden");
      expect(JSON.stringify(res.body)).not.toContain(HTTP_SECRET);
      expect(published).toHaveLength(0);
    } finally {
      await rm(dir, { recursive: true, force: true });
    }
  });
});
