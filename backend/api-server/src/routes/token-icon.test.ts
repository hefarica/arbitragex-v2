// Tests for the token-icon route — cascade Redis → registry → DexScreener →
// jazzicon, fail-honest (never 500), with a fake Redis and a mocked fetch.

import express, { type Express } from "express";
import request from "supertest";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { mountTokenIconRoutes, tokenIconKey } from "./token-icon.js";

// Canonical mainnet addresses (real — present in the committed registry).
const WETH = "0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2";
// A random, registry-absent address (forces the DexScreener / jazzicon tiers).
const UNKNOWN = "0x1111111111111111111111111111111111111111";

interface FakeRedis {
  get: (k: string) => Promise<string | null>;
  setex: (k: string, ttl: number, v: string) => Promise<"OK">;
  _store: Map<string, string>;
}

function makeFakeRedis(seed: Record<string, string> = {}): FakeRedis {
  const store = new Map<string, string>(Object.entries(seed));
  return {
    get: async (k: string) => store.get(k) ?? null,
    setex: async (k: string, _ttl: number, v: string) => {
      store.set(k, v);
      return "OK";
    },
    _store: store,
  };
}

// Minimal fake pg pool: `query` returns the configured rows (default: none).
interface FakePool {
  query: (text: string, params: unknown[]) => Promise<{ rows: { logo_url?: string }[] }>;
}
function makeFakePool(rows: { logo_url?: string }[] = []): FakePool {
  return { query: async () => ({ rows }) };
}

function appWith(redis: FakeRedis, pool?: FakePool): Express {
  const app = express();
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  mountTokenIconRoutes(app, { redis: redis as any, pool: (pool ?? null) as any });
  return app;
}

const origFetch = globalThis.fetch;
afterEach(() => {
  globalThis.fetch = origFetch;
  vi.restoreAllMocks();
});

describe("GET /api/v1/token-icon/:chainId/:address", () => {
  it("returns 400 for an invalid address (no DexScreener call)", async () => {
    const fetchSpy = vi.fn();
    globalThis.fetch = fetchSpy as unknown as typeof fetch;
    const res = await request(appWith(makeFakeRedis())).get(
      "/api/v1/token-icon/1/0xNOTANADDRESS",
    );
    expect(res.status).toBe(400);
    expect(res.body).toMatchObject({ ok: false, error: "invalid_address" });
    expect(fetchSpy).not.toHaveBeenCalled();
  });

  it("returns 400 for an invalid chainId", async () => {
    const res = await request(appWith(makeFakeRedis())).get(
      `/api/v1/token-icon/abc/${WETH}`,
    );
    expect(res.status).toBe(400);
    expect(res.body).toMatchObject({ ok: false, error: "invalid_chain_id" });
  });

  it("serves a Redis hit (source=redis) when the producer wrote the key", async () => {
    const url = "https://cdn.example/icon/weth.png";
    const redis = makeFakeRedis({ [tokenIconKey(1, WETH)]: url });
    const res = await request(appWith(redis)).get(`/api/v1/token-icon/1/${WETH}`);
    expect(res.status).toBe(200);
    expect(res.body).toMatchObject({
      ok: true,
      chainId: "1",
      address: WETH,
      iconUrl: url,
      source: "redis",
      cached: true,
    });
  });

  it("falls back to the curated registry (source=known) and caches it", async () => {
    const redis = makeFakeRedis(); // empty → Redis miss
    const res = await request(appWith(redis)).get(`/api/v1/token-icon/1/${WETH}`);
    expect(res.status).toBe(200);
    expect(res.body.source).toBe("known");
    expect(typeof res.body.iconUrl).toBe("string");
    expect(res.body.iconUrl).toMatch(/^https?:\/\//);
    // best-effort cache population for next time
    expect(redis._store.get(tokenIconKey(1, WETH))).toBe(res.body.iconUrl);
  });

  it("falls back to the durable PG tokens.logo_url tier (source=db) before DexScreener", async () => {
    // PG is tier 3 — it must resolve a registry-absent token WITHOUT hitting the
    // rate-limited DexScreener tier. This is the hardening that stops logos from
    // disappearing when Redis is empty and external APIs are rate-limited.
    const fetchSpy = vi.fn();
    globalThis.fetch = fetchSpy as unknown as typeof fetch;
    const dbUrl =
      "https://raw.githubusercontent.com/trustwallet/assets/master/blockchain/ethereum/assets/0xABC/logo.png";
    const redis = makeFakeRedis();
    const res = await request(appWith(redis, makeFakePool([{ logo_url: dbUrl }]))).get(
      `/api/v1/token-icon/1/${UNKNOWN}`,
    );
    expect(res.status).toBe(200);
    expect(res.body).toMatchObject({ source: "db", iconUrl: dbUrl });
    // cached into Redis for the long TTL so the next read is instant
    expect(redis._store.get(tokenIconKey(1, UNKNOWN))).toBe(dbUrl);
    // DexScreener must NOT have been hit — PG resolved first
    expect(fetchSpy).not.toHaveBeenCalled();
  });

  it("never 500s when the PG tier throws (degrades past it)", async () => {
    globalThis.fetch = vi
      .fn()
      .mockRejectedValue(new Error("dex down")) as unknown as typeof fetch;
    const throwingPool: FakePool = {
      query: async () => {
        throw new Error("pg down");
      },
    };
    const res = await request(appWith(makeFakeRedis(), throwingPool)).get(
      `/api/v1/token-icon/1/${UNKNOWN}`,
    );
    expect(res.status).toBe(200);
    expect(res.body.ok).toBe(true);
    // PG threw + DexScreener threw → jazzicon, never a 500
    expect(res.body.source).toBe("jazzicon");
  });

  it("falls back to DexScreener (source=dexscreener) for a registry-absent token", async () => {
    const img = "https://dd.dexscreener.com/ds-data/tokens/ethereum/x.png";
    globalThis.fetch = vi.fn().mockResolvedValue({
      ok: true,
      json: async () => ({
        pairs: [
          {
            chainId: "ethereum",
            liquidity: { usd: 1_000_000 },
            baseToken: { address: UNKNOWN },
            info: { imageUrl: img },
          },
        ],
      }),
    }) as unknown as typeof fetch;

    const redis = makeFakeRedis();
    const res = await request(appWith(redis)).get(
      `/api/v1/token-icon/1/${UNKNOWN}`,
    );
    expect(res.status).toBe(200);
    expect(res.body).toMatchObject({
      source: "dexscreener",
      iconUrl: img,
    });
    expect(redis._store.get(tokenIconKey(1, UNKNOWN))).toBe(img);
  });

  it("degrades to jazzicon (iconUrl=null) when DexScreener has no image — never 500", async () => {
    globalThis.fetch = vi.fn().mockResolvedValue({
      ok: true,
      json: async () => ({ pairs: [] }),
    }) as unknown as typeof fetch;

    const res = await request(appWith(makeFakeRedis())).get(
      `/api/v1/token-icon/1/${UNKNOWN}`,
    );
    expect(res.status).toBe(200);
    expect(res.body).toMatchObject({ ok: true, iconUrl: null, source: "jazzicon" });
  });

  it("never 500s even when Redis throws and DexScreener errors", async () => {
    globalThis.fetch = vi
      .fn()
      .mockRejectedValue(new Error("network down")) as unknown as typeof fetch;
    const throwingRedis: FakeRedis = {
      get: async () => {
        throw new Error("redis down");
      },
      setex: async () => {
        throw new Error("redis down");
      },
      _store: new Map(),
    };
    const res = await request(appWith(throwingRedis)).get(
      `/api/v1/token-icon/1/${UNKNOWN}`,
    );
    expect(res.status).toBe(200);
    expect(res.body.ok).toBe(true);
    expect(res.body.source).toBe("jazzicon");
  });
});
