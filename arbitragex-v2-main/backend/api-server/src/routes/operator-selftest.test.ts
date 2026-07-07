/**
 * operator-selftest tests — SECURITY-CRITICAL.
 *
 * Pins the hard invariants of the Operator Self-Test Center:
 *   (a) NO env VALUE ever appears in either response (sentinel leak scan),
 *   (b) absent WalletConnect projectId → BLOCKED_REQUIRES_OPERATOR_SECRET,
 *   (c) live-future secrets present → DORMANT_SEALED, never the value,
 *   (d) pg/redis failure → FAILING blocks, endpoint still 200 (never crash),
 *   (e) safe posture fields are always false / 0 / false.
 */

import express, { type Express } from "express";
import request from "supertest";
import { describe, expect, it, vi } from "vitest";
import type pg from "pg";

import {
  mountOperatorSelfTest,
  type OperatorSelfTestDeps,
  type RedisLike,
  SELFTEST_BLOCK_SLUGS,
  SELFTEST_STATUSES,
  __forTesting,
} from "./operator-selftest.js";

// ---------------------------------------------------------------------------
// Fakes — mirror the structural deps (pool.query / redis.ping+get / fetch).
// ---------------------------------------------------------------------------

// Sentinel SECRET VALUES — if any of these strings ever appears in a response
// body, the surface is leaking. Deliberately unique and grep-able.
const SENTINEL = {
  DATABASE_URL: "postgres://arbx:SENTINEL_PG_PASSWORD_a1b2c3@db.internal:5432/arbx",
  REDIS_URL: "redis://:SENTINEL_REDIS_PASSWORD_d4e5f6@redis.internal:6379/0",
  JWT_SECRET: "SENTINEL_JWT_SECRET_0123456789abcdef0123456789abcdef",
  ARBX_ADMIN_TOKEN: "SENTINEL_ADMIN_TOKEN_fedcba9876543210fedcba9876543210",
  ARBX_SERVICE_TOKEN: "SENTINEL_SERVICE_TOKEN_00112233445566778899aabbccddeeff",
  FLASHBOTS_SIGNER_KEY: "0xSENTINELaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa1",
  BLOXROUTE_AUTH: "SENTINEL_BLOXROUTE_AUTH_ZmFrZS1hdXRoLWhlYWRlcg==",
  EDEN_AUTH: "SENTINEL_EDEN_AUTH_xyz123",
} as const;

function envWith(extra: Record<string, string> = {}): NodeJS.ProcessEnv {
  // A CLEAN env (never process.env) so the test is deterministic and the
  // sentinel scan can prove non-leakage.
  return { ...extra };
}

function fakePoolOk(): pg.Pool {
  return {
    query: vi.fn(async (text: string) => {
      if (text.includes("to_regclass")) return { rows: [{ ok: true }] };
      if (text.includes("SELECT 1")) return { rows: [{ "?column?": 1 }] };
      if (text.includes("FROM trading_config")) return { rows: [{ n: 2 }] };
      if (text.includes("FROM opportunities") && text.includes("MIN")) {
        return { rows: [{ first: new Date(Date.now() - 10 * 86_400_000).toISOString() }] };
      }
      if (text.includes("FROM opportunities")) return { rows: [{ total: 379, recent: 12 }] };
      if (text.includes("FROM cartridge_registry")) {
        return { rows: [{ state: "active", n: 3 }, { state: "draft", n: 1 }] };
      }
      if (text.includes("FROM route_discovery_outcomes")) {
        return { rows: [{ total: 100, last_ts: String(Date.now() - 60_000) }] };
      }
      return { rows: [] };
    }),
  } as unknown as pg.Pool;
}

function fakePoolDown(): pg.Pool {
  return {
    query: vi.fn(async () => {
      throw new Error(`connection refused to ${SENTINEL.DATABASE_URL}`);
    }),
  } as unknown as pg.Pool;
}

function fakeRedisOk(store: Record<string, string> = {}): RedisLike {
  return {
    ping: async () => "PONG",
    get: async (k: string) => store[k] ?? null,
  };
}

function fakeRedisDown(): RedisLike {
  return {
    ping: async () => {
      throw new Error(`ECONNREFUSED ${SENTINEL.REDIS_URL}`);
    },
    get: async () => {
      throw new Error("ECONNREFUSED");
    },
  };
}

const fetchOk = vi.fn(async () => ({ ok: true, status: 200 })) as unknown as typeof fetch;
const fetchDown = vi.fn(async () => {
  throw new Error("fetch failed: ECONNREFUSED sim-ctl");
}) as unknown as typeof fetch;

const readinessOk = async () => ({
  summary: { green: 9, yellow: 5, red: 3, pending: 0, total: 17 },
  flip_blocked: true,
});

const noopLogger = { warn: () => {} };

function appWith(deps: Partial<OperatorSelfTestDeps>): Express {
  const app = express();
  mountOperatorSelfTest(app, {
    pool: null,
    redis: null,
    logger: noopLogger,
    env: envWith(),
    fetchImpl: fetchDown,
    ...deps,
  });
  return app;
}

// ---------------------------------------------------------------------------
// (a) NO env VALUE leaks — sentinel scan over BOTH serialized responses.
// ---------------------------------------------------------------------------

describe("secret non-leakage (sentinel scan)", () => {
  it("no sentinel env VALUE appears anywhere in /api/operator/credentials/status", async () => {
    const app = appWith({ env: envWith({ ...SENTINEL }) });
    const res = await request(app).get("/api/operator/credentials/status");
    expect(res.status).toBe(200);
    const body = JSON.stringify(res.body);
    for (const [name, value] of Object.entries(SENTINEL)) {
      expect(body.includes(value), `${name} VALUE leaked into the response`).toBe(false);
      // Even fragments of the secret must not appear (no fingerprints/masks).
      expect(body.includes(value.slice(-12)), `${name} value FRAGMENT leaked`).toBe(false);
    }
    // The NAMES are expected (it's a presence matrix) — presence is boolean only.
    for (const entry of res.body.secrets) {
      expect(typeof entry.present).toBe("boolean");
      expect(Object.keys(entry).sort()).not.toContain("value");
    }
  });

  it("no sentinel env VALUE appears anywhere in /api/operator/selftest (even via probe errors)", async () => {
    // Failing pool/redis whose error messages EMBED the sentinel URLs — the
    // sanitizer must keep them out of evidence strings.
    const app = appWith({
      env: envWith({ ...SENTINEL }),
      pool: fakePoolDown(),
      redis: fakeRedisDown(),
      fetchImpl: fetchDown,
      readiness: readinessOk,
    });
    const res = await request(app).get("/api/operator/selftest");
    expect(res.status).toBe(200);
    const body = JSON.stringify(res.body);
    for (const [name, value] of Object.entries(SENTINEL)) {
      expect(body.includes(value), `${name} VALUE leaked into the selftest response`).toBe(false);
    }
    // URL-with-password shape must never appear.
    expect(/:\/\/[^/\s"']+:[^/\s"'@]+@/.test(body), "credentialed URL leaked").toBe(false);
  });
});

// ---------------------------------------------------------------------------
// (b) WalletConnect projectId absent → BLOCKED_REQUIRES_OPERATOR_SECRET.
// ---------------------------------------------------------------------------

describe("credential matrix rules", () => {
  it("WC projectId absent → BLOCKED_REQUIRES_OPERATOR_SECRET (public_build_env, exposed)", async () => {
    const app = appWith({ env: envWith() });
    const res = await request(app).get("/api/operator/credentials/status");
    const wc = res.body.secrets.find(
      (s: { name: string }) => s.name === "NEXT_PUBLIC_WALLETCONNECT_PROJECT_ID",
    );
    expect(wc).toBeTruthy();
    expect(wc.present).toBe(false);
    expect(wc.status).toBe("BLOCKED_REQUIRES_OPERATOR_SECRET");
    expect(wc.scope).toBe("public_build_env");
    expect(wc.exposed_to_frontend).toBe(true);
  });

  it("WC projectId present → VERIFIED (and still no value)", async () => {
    const app = appWith({
      env: envWith({ NEXT_PUBLIC_WALLETCONNECT_PROJECT_ID: "SENTINEL_WC_PROJECT_ID_123456" }),
    });
    const res = await request(app).get("/api/operator/credentials/status");
    const wc = res.body.secrets.find(
      (s: { name: string }) => s.name === "NEXT_PUBLIC_WALLETCONNECT_PROJECT_ID",
    );
    expect(wc.present).toBe(true);
    expect(wc.status).toBe("VERIFIED");
    expect(JSON.stringify(res.body).includes("SENTINEL_WC_PROJECT_ID_123456")).toBe(false);
  });

  // (c) live-future secrets present → DORMANT_SEALED, never the value.
  it("live-future secrets present → DORMANT_SEALED; absent → ABSENT_BY_POLICY", async () => {
    const app = appWith({
      env: envWith({
        FLASHBOTS_SIGNER_KEY: SENTINEL.FLASHBOTS_SIGNER_KEY,
        BLOXROUTE_AUTH: SENTINEL.BLOXROUTE_AUTH,
      }),
    });
    const res = await request(app).get("/api/operator/credentials/status");
    const find = (n: string) => res.body.secrets.find((s: { name: string }) => s.name === n);
    expect(find("FLASHBOTS_SIGNER_KEY").status).toBe("DORMANT_SEALED");
    expect(find("BLOXROUTE_AUTH").status).toBe("DORMANT_SEALED");
    expect(find("EDEN_AUTH").status).toBe("ABSENT_BY_POLICY");
    expect(find("KMS_KEY_ID").status).toBe("ABSENT_BY_POLICY");
    expect(find("SAFE_MULTISIG_ADDRESS").status).toBe("ABSENT_BY_POLICY");
    expect(find("OPERATOR_APPROVAL_TOKEN").status).toBe("ABSENT_BY_POLICY");
    const body = JSON.stringify(res.body);
    expect(body.includes(SENTINEL.FLASHBOTS_SIGNER_KEY)).toBe(false);
    expect(body.includes(SENTINEL.BLOXROUTE_AUTH)).toBe(false);
  });

  it("required server vars: present → VERIFIED, absent → MISSING", async () => {
    const app = appWith({
      env: envWith({ DATABASE_URL: SENTINEL.DATABASE_URL, REDIS_URL: SENTINEL.REDIS_URL }),
    });
    const res = await request(app).get("/api/operator/credentials/status");
    const find = (n: string) => res.body.secrets.find((s: { name: string }) => s.name === n);
    expect(find("DATABASE_URL").status).toBe("VERIFIED");
    expect(find("REDIS_URL").status).toBe("VERIFIED");
    expect(find("JWT_SECRET").status).toBe("MISSING");
    expect(find("ARBX_ADMIN_TOKEN").status).toBe("MISSING");
  });

  it("RPC vars absent → PARTIAL; optional data providers absent → HONEST_EMPTY", async () => {
    const res = await request(appWith({})).get("/api/operator/credentials/status");
    const find = (n: string) => res.body.secrets.find((s: { name: string }) => s.name === n);
    expect(find("RPC_HTTP_1").status).toBe("PARTIAL");
    expect(find("RPC_WS_42161").status).toBe("PARTIAL");
    expect(find("TENDERLY_API_KEY").status).toBe("HONEST_EMPTY");
    expect(find("GOPLUS_API_KEY").status).toBe("HONEST_EMPTY");
  });

  it("SIWE domain: either var satisfies the group; neither → BLOCKED_REQUIRES_OPERATOR_CONFIG", async () => {
    const withHost = await request(
      appWith({ env: envWith({ PUBLIC_EDGE_HOST: "edge.example.net" }) }),
    ).get("/api/operator/credentials/status");
    const find1 = (n: string) => withHost.body.secrets.find((s: { name: string }) => s.name === n);
    expect(find1("PUBLIC_EDGE_HOST").status).toBe("VERIFIED");
    expect(find1("ARBX_WALLET_SIWE_DOMAIN").status).toBe("VERIFIED"); // group satisfied

    const without = await request(appWith({})).get("/api/operator/credentials/status");
    const find2 = (n: string) => without.body.secrets.find((s: { name: string }) => s.name === n);
    expect(find2("PUBLIC_EDGE_HOST").status).toBe("BLOCKED_REQUIRES_OPERATOR_CONFIG");
    expect(find2("ARBX_WALLET_SIWE_DOMAIN").status).toBe("BLOCKED_REQUIRES_OPERATOR_CONFIG");
  });
});

// ---------------------------------------------------------------------------
// (d) pg/redis failure → FAILING blocks, endpoint never crashes.
// ---------------------------------------------------------------------------

describe("selftest aggregator — fail-honest, never crash", () => {
  it("pg + redis down → connections FAILING, db blocks FAILING, HTTP still 200 with 10 blocks", async () => {
    const app = appWith({
      pool: fakePoolDown(),
      redis: fakeRedisDown(),
      fetchImpl: fetchDown,
      readiness: readinessOk,
    });
    const res = await request(app).get("/api/operator/selftest");
    expect(res.status).toBe(200);
    expect(res.body.blocks).toHaveLength(10);
    const byBlock = (n: string) => res.body.blocks.find((b: { block: string }) => b.block === n);
    expect(byBlock("connections").status).toBe("FAILING");
    expect(byBlock("settings").status).toBe("FAILING");
    expect(byBlock("data_feeds").status).toBe("FAILING");
    expect(byBlock("simulation").status).toBe("FAILING");
    // wallet posture is structural — VERIFIED even with everything down.
    expect(byBlock("wallet").status).toBe("VERIFIED");
    // every status is from the allowed taxonomy.
    for (const b of res.body.blocks) {
      expect(SELFTEST_STATUSES).toContain(b.status);
      expect(typeof b.evidence).toBe("string");
      expect(b.evidence.length).toBeGreaterThan(0);
      expect(typeof b.last_checked_at).toBe("string");
      expect(typeof b.link).toBe("string");
    }
  });

  it("null pool + null redis (no DB configured at all) → still 200, honest FAILING evidence", async () => {
    const res = await request(appWith({ readiness: readinessOk })).get("/api/operator/selftest");
    expect(res.status).toBe(200);
    const conn = res.body.blocks.find((b: { block: string }) => b.block === "connections");
    expect(conn.status).toBe("FAILING");
    expect(conn.evidence).toMatch(/not configured/);
  });

  it("healthy deps → connections VERIFIED with latency, settings/data_feeds/strategies/signals VERIFIED, paper VERIFIED (10d-old data)", async () => {
    const app = appWith({
      pool: fakePoolOk(),
      redis: fakeRedisOk(),
      fetchImpl: fetchOk,
      readiness: readinessOk,
    });
    const res = await request(app).get("/api/operator/selftest");
    expect(res.status).toBe(200);
    const byBlock = (n: string) => res.body.blocks.find((b: { block: string }) => b.block === n);
    expect(byBlock("connections").status).toBe("VERIFIED");
    expect(typeof byBlock("connections").latency_ms).toBe("number");
    expect(byBlock("settings").status).toBe("VERIFIED");
    expect(byBlock("data_feeds").status).toBe("VERIFIED");
    expect(byBlock("strategies").status).toBe("VERIFIED");
    expect(byBlock("simulation").status).toBe("VERIFIED");
    expect(byBlock("signals").status).toBe("VERIFIED");
    expect(byBlock("paper").status).toBe("VERIFIED");
    // live_readiness 9/17 green → PARTIAL, honest evidence.
    expect(byBlock("live_readiness").status).toBe("PARTIAL");
    expect(byBlock("live_readiness").evidence).toContain("9/17");
    // credentials block with an empty env → MISSING (worst-of required).
    expect(byBlock("credentials").status).toBe("MISSING");
  });

  it("paper window <7d → BLOCKED_REQUIRES_TIME_WINDOW with days_remaining", async () => {
    const pool = {
      query: vi.fn(async (text: string) => {
        if (text.includes("to_regclass")) return { rows: [{ ok: true }] };
        if (text.includes("MIN")) {
          return { rows: [{ first: new Date(Date.now() - 2 * 86_400_000).toISOString() }] };
        }
        return { rows: [{ total: 1, recent: 1, n: 1, last_ts: String(Date.now()) }] };
      }),
    } as unknown as pg.Pool;
    const app = appWith({ pool, redis: fakeRedisOk(), fetchImpl: fetchOk, readiness: readinessOk });
    const res = await request(app).get("/api/operator/selftest");
    const paper = res.body.blocks.find((b: { block: string }) => b.block === "paper");
    expect(paper.status).toBe("BLOCKED_REQUIRES_TIME_WINDOW");
    expect(paper.days_remaining).toBe(5);
  });

  it("paper feed off (redis arbx:papermode enabled:false) → BLOCKED_REQUIRES_OPERATOR_CONFIG", async () => {
    const app = appWith({
      pool: fakePoolOk(),
      redis: fakeRedisOk({ "arbx:papermode": JSON.stringify({ enabled: false }) }),
      fetchImpl: fetchOk,
      readiness: readinessOk,
    });
    const res = await request(app).get("/api/operator/selftest");
    const paper = res.body.blocks.find((b: { block: string }) => b.block === "paper");
    expect(paper.status).toBe("BLOCKED_REQUIRES_OPERATOR_CONFIG");
    expect(typeof paper.operator_action).toBe("string");
  });

  it("strategies table absent (42P01) → HONEST_EMPTY, not a crash", async () => {
    const pool = {
      query: vi.fn(async (text: string) => {
        if (text.includes("FROM cartridge_registry")) {
          const err = new Error('relation "cartridge_registry" does not exist') as Error & { code: string };
          err.code = "42P01";
          throw err;
        }
        if (text.includes("to_regclass")) return { rows: [{ ok: true }] };
        return { rows: [{ total: 0, recent: 0, n: 0, first: null, last_ts: null }] };
      }),
    } as unknown as pg.Pool;
    const app = appWith({ pool, redis: fakeRedisOk(), fetchImpl: fetchOk, readiness: readinessOk });
    const res = await request(app).get("/api/operator/selftest");
    const strategies = res.body.blocks.find((b: { block: string }) => b.block === "strategies");
    expect(strategies.status).toBe("HONEST_EMPTY");
    expect(strategies.evidence).toContain("42P01");
  });
});

// ---------------------------------------------------------------------------
// (e) safe posture fields — always false / 0 / false in BOTH responses.
// ---------------------------------------------------------------------------

describe("safe posture invariants", () => {
  it("/api/operator/credentials/status posture is false/0/false + shadow_paper", async () => {
    const res = await request(appWith({})).get("/api/operator/credentials/status");
    expect(res.body.mode).toBe("shadow_paper");
    expect(res.body.live_enabled).toBe(false);
    expect(res.body.capital_exposed).toBe(0);
    expect(res.body.broadcast).toBe(false);
  });

  it("/api/operator/selftest posture is false/0/false + shadow_paper, regardless of probe health", async () => {
    for (const deps of [
      {},
      { pool: fakePoolOk(), redis: fakeRedisOk(), fetchImpl: fetchOk, readiness: readinessOk },
      { pool: fakePoolDown(), redis: fakeRedisDown() },
    ] as Array<Partial<OperatorSelfTestDeps>>) {
      const res = await request(appWith(deps)).get("/api/operator/selftest");
      expect(res.body.mode).toBe("shadow_paper");
      expect(res.body.live_enabled).toBe(false);
      expect(res.body.capital_exposed).toBe(0);
      expect(res.body.broadcast).toBe(false);
    }
  });

  it("the 10 block slugs are exactly the documented checklist", () => {
    expect([...SELFTEST_BLOCK_SLUGS]).toEqual([
      "credentials",
      "connections",
      "wallet",
      "settings",
      "data_feeds",
      "strategies",
      "simulation",
      "signals",
      "paper",
      "live_readiness",
    ]);
  });
});

// ---------------------------------------------------------------------------
// sanitizer — defense-in-depth for probe error strings.
// ---------------------------------------------------------------------------

describe("sanitizeDetail", () => {
  it("redacts credentialed URLs, long hex, and JWT-shaped tokens", () => {
    const dirty = `failed: ${SENTINEL.DATABASE_URL} key=0x${"ab".repeat(32)} jwt=eyJhbGciOiJIUzI1NiJ9.payload`;
    const clean = __forTesting.sanitizeDetail(dirty);
    expect(clean).not.toContain("SENTINEL_PG_PASSWORD");
    expect(clean).not.toContain("ab".repeat(32));
    expect(clean).not.toContain("eyJhbGciOiJIUzI1NiJ9");
    expect(clean).toContain("[redacted-url]");
  });
});
