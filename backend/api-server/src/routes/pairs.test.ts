/**
 * pairs tests — EMIT-06 surface (FE-MASTER P5 §13).
 *
 * Pins the hard invariants of GET /api/pairs:
 *   (a) envelope is EXACTLY `{ entries }` — no extra keys (PairsResponseSchema
 *       is .strict()),
 *   (b) canonical leg order is backend-fixed (address ascending) and reserves
 *       are ORIENTED onto it by the pool's own token0,
 *   (c) §62: reserves ride as decimal strings, untouched,
 *   (d) R8: pools without live reserves are excluded (and pairs left with no
 *       live pool drop out); registry-incomplete pools are skipped;
 *       Redis ERRORS are 503 — absence is not an outage,
 *   (e) `dirty` reflects the undrained SET membership; `alpha_*` ride the
 *       EMIT-06b hash verbatim — absent/poisoned ⇒ null, never fabricated,
 *   (f) fee_bps falls back to the 30 class constant only for NULL fee_tier,
 *   (g) last_reserve_update = max pool ts (epoch s → ms), null when absent.
 */
import express, { type Express } from "express";
import request from "supertest";
import { describe, expect, it, vi } from "vitest";
import type { Pool as PgPool } from "pg";
import type { Redis } from "ioredis";

import { mountPairs } from "./pairs.js";

const logger = { warn: vi.fn() };

function buildApp(pool: unknown, redis: unknown): Express {
  const app = express();
  mountPairs(app, { pool: pool as PgPool | null, redis: redis as Redis | null, logger });
  return app;
}

// Universe: 3 pools → 2 canonical pairs.
//   PAIR-AB: pools P1 (DexOne, V2 tier null) + P2 (DexTwo, tier 100) — both live.
//   PAIR-AC: pool P3 (DexOne) — NO reserves entry → excluded → pair drops.
const A = `0x${"a".repeat(40)}`; // canonical leg A (lowest)
const B = `0x${"b".repeat(40)}`;
const C = `0x${"c".repeat(40)}`;
const P1 = `0x${"1".repeat(40)}`;
const P2 = `0x${"2".repeat(40)}`;
const P3 = `0x${"3".repeat(40)}`;

const POOL_ROWS = [
  {
    address: P1,
    dex_name: "DexOne",
    fee_tier: null,
    token0_symbol: "AAA",
    token0_address: A,
    token0_decimals: 18,
    token1_symbol: "BBB",
    token1_address: B,
    token1_decimals: 6,
  },
  {
    address: P2,
    // token0/token1 SWAPPED vs P1 — exercises reserve re-orientation.
    dex_name: "DexTwo",
    fee_tier: 100,
    token0_symbol: "BBB",
    token0_address: B,
    token0_decimals: 6,
    token1_symbol: "AAA",
    token1_address: A,
    token1_decimals: 18,
  },
  {
    address: P3,
    dex_name: "DexOne",
    fee_tier: 500,
    token0_symbol: "AAA",
    token0_address: A,
    token0_decimals: 18,
    token1_symbol: "CCC",
    token1_address: C,
    token1_decimals: 18,
  },
];

const RESERVES: Record<string, string> = {
  // P1: token0 = A ⇒ reserves_a = r0 = "1000".
  [P1]: JSON.stringify({ r0: "1000", r1: "2000", token0_addr: A, blk: 10, ts: 1_700_000_000 }),
  // P2: token0 = B ⇒ reserves_a = r1 = "77".
  [P2]: JSON.stringify({ r0: "88", r1: "77", token0_addr: B, blk: 12, ts: 1_700_000_050 }),
};

function mockPool(rows: unknown[]) {
  return { query: vi.fn(async () => ({ rows })) };
}
function mockRedis(
  reserves: Record<string, string>,
  dirty: string[] = [],
  alpha: Record<string, string> = {},
): {
  redis: object;
  mget: ReturnType<typeof vi.fn>;
  smembers: ReturnType<typeof vi.fn>;
  hgetall: ReturnType<typeof vi.fn>;
} {
  const mget = vi.fn(async (...keys: string[]) =>
    keys.map((k) => {
      const addr = k.split(":").pop() ?? "";
      return reserves[addr] ?? null;
    }),
  );
  const smembers = vi.fn(async () => dirty);
  const hgetall = vi.fn(async () => alpha);
  return { redis: { mget, smembers, hgetall }, mget, smembers, hgetall };
}

describe("GET /api/pairs (EMIT-06)", () => {
  it("returns 503 when PG is null (fail-honest)", async () => {
    const res = await request(buildApp(null, mockRedis(RESERVES).redis)).get("/api/pairs");
    expect(res.status).toBe(503);
    expect(res.body.error).toBe("db_unavailable");
  });

  it("returns 503 when Redis is null (fail-honest)", async () => {
    const res = await request(buildApp(mockPool(POOL_ROWS), null)).get("/api/pairs");
    expect(res.status).toBe(503);
    expect(res.body.error).toBe("redis_unavailable");
  });

  it("400 invalid_chain_id on garbage", async () => {
    const res = await request(buildApp(mockPool([]), mockRedis({}).redis)).get("/api/pairs?chain_id=x");
    expect(res.status).toBe(400);
    expect(res.body.error).toBe("invalid_chain_id");
  });

  it("serves { entries } EXACTLY with canonical order, oriented reserves, venues, dirty", async () => {
    const { redis } = mockRedis(RESERVES, [P2]);
    const res = await request(buildApp(mockPool(POOL_ROWS), redis)).get("/api/pairs?chain_id=1");
    expect(res.status).toBe(200);
    expect(Object.keys(res.body)).toEqual(["entries"]);
    expect(res.body.entries).toHaveLength(1); // PAIR-AC dropped (no live pool)

    const pair = res.body.entries[0];
    // Canonical legs: address ascending — A then B.
    expect(pair.token_a.address).toBe(A);
    expect(pair.token_a).toEqual({ chain_id: 1, address: A, symbol: "AAA", decimals: 18 });
    expect(pair.token_b).toEqual({ chain_id: 1, address: B, symbol: "BBB", decimals: 6 });
    expect(pair.chain_id).toBe(1);
    // Both live pools, oriented: P1 (token0=A) ⇒ a=r0; P2 (token0=B) ⇒ a=r1.
    expect(pair.pools).toHaveLength(2);
    const p1 = pair.pools.find((x: { pool_address: string }) => x.pool_address === P1);
    const p2 = pair.pools.find((x: { pool_address: string }) => x.pool_address === P2);
    expect(p1.reserves_a).toBe("1000");
    expect(p1.reserves_b).toBe("2000");
    expect(p1.fee_bps).toBe(30); // NULL tier → 30 class constant
    expect(p1.venue).toBe("DexOne");
    expect(p2.reserves_a).toBe("77");
    expect(p2.reserves_b).toBe("88");
    expect(p2.fee_bps).toBe(100); // on-chain tier verbatim
    expect(p2.venue).toBe("DexTwo");
    // venue_count = distinct venues; dirty via SET membership of P2.
    expect(pair.venue_count).toBe(2);
    expect(pair.dirty).toBe(true);
    // last_reserve_update = max ts in ms.
    expect(pair.last_reserve_update).toBe(1_700_000_050 * 1000);
    // alpha r15 not published — honest nulls (EMIT-06b).
    expect(pair.alpha_forward).toBeNull();
    expect(pair.alpha_reverse).toBeNull();
  });

  it("a clean pair (no member in the dirty SET) reports dirty=false", async () => {
    const { redis } = mockRedis(RESERVES, []);
    const res = await request(buildApp(mockPool(POOL_ROWS), redis)).get("/api/pairs");
    expect(res.status).toBe(200);
    expect(res.body.entries[0].dirty).toBe(false);
  });

  it("registry-incomplete pools are skipped, never fabricated", async () => {
    const rows = [
      {
        address: P1,
        dex_name: "DexOne",
        fee_tier: null,
        token0_symbol: "AAA",
        token0_address: A,
        token0_decimals: 18,
        token1_symbol: null, // incomplete identity
        token1_address: null,
        token1_decimals: null,
      },
    ];
    const res = await request(buildApp(mockPool(rows), mockRedis(RESERVES).redis)).get("/api/pairs");
    expect(res.status).toBe(200);
    expect(res.body.entries).toEqual([]);
  });

  it("unparseable reserve entries degrade to pool-excluded, no crash", async () => {
    const mget = vi.fn(async () => ["not-json{{"]);
    const redis = { mget, smembers: vi.fn(async () => []), hgetall: vi.fn(async () => ({})) };
    const res = await request(buildApp(mockPool(POOL_ROWS), redis)).get("/api/pairs");
    expect(res.status).toBe(200);
    expect(res.body.entries).toEqual([]);
  });

  it("a transient MGET error is 503 redis_unavailable, never empty-as-clean", async () => {
    const redis = { mget: vi.fn(async () => { throw new Error("ECONNRESET"); }), smembers: vi.fn(async () => []), hgetall: vi.fn(async () => ({})) };
    const res = await request(buildApp(mockPool(POOL_ROWS), redis)).get("/api/pairs");
    expect(res.status).toBe(503);
    expect(res.body.error).toBe("redis_unavailable");
  });

  it("a transient SMEMBERS error is 503 redis_unavailable (dirty unknowable)", async () => {
    const redis = { mget: vi.fn(async () => []), smembers: vi.fn(async () => { throw new Error("ECONNRESET"); }), hgetall: vi.fn(async () => ({})) };
    const res = await request(buildApp(mockPool(POOL_ROWS), redis)).get("/api/pairs");
    expect(res.status).toBe(503);
    expect(res.body.error).toBe("redis_unavailable");
  });

  it("a PG query failure is 503 query_failed", async () => {
    const pool = { query: vi.fn(async () => { throw new Error("relation missing"); }) };
    const res = await request(buildApp(pool, mockRedis(RESERVES).redis)).get("/api/pairs");
    expect(res.status).toBe(503);
    expect(res.body.error).toBe("query_failed");
    expect(logger.warn).toHaveBeenCalled();
  });

  it("empty registry serves an honest empty array", async () => {
    const res = await request(buildApp(mockPool([]), mockRedis({}).redis)).get("/api/pairs");
    expect(res.status).toBe(200);
    expect(res.body).toEqual({ entries: [] });
  });

  // ── EMIT-06b: alpha join ────────────────────────────────────────────────
  it("serves alpha_forward/alpha_reverse from the EMIT-06b hash, joined by canonical key", async () => {
    const alpha = {
      [`${A}|${B}`]: JSON.stringify({ forward: 1.00042, reverse: 0.99961 }),
    };
    const { redis } = mockRedis(RESERVES, [], alpha);
    const res = await request(buildApp(mockPool(POOL_ROWS), redis)).get("/api/pairs?chain_id=1");
    expect(res.status).toBe(200);
    expect(res.body.entries[0].alpha_forward).toBe(1.00042);
    expect(res.body.entries[0].alpha_reverse).toBe(0.99961); // NEVER −forward (r15)
  });

  it("absent alpha field ⇒ honest nulls (knob OFF / pair not in this tick's graph)", async () => {
    const { redis } = mockRedis(RESERVES, [], {}); // empty hash
    const res = await request(buildApp(mockPool(POOL_ROWS), redis)).get("/api/pairs");
    expect(res.status).toBe(200);
    expect(res.body.entries[0].alpha_forward).toBeNull();
    expect(res.body.entries[0].alpha_reverse).toBeNull();
  });

  it("poisoned alpha row drops BOTH directions, non-finite numbers are rejected", async () => {
    const alpha = {
      [`${A}|${B}`]: "not-json{{",
      // PAIR-AC is dropped from entries (no live pool), so its field is a
      // no-op — but it also exercises the non-finite rejection path.
      [`${A}|${C}`]: JSON.stringify({ forward: Number.NaN, reverse: 1.5 }),
    };
    const { redis } = mockRedis(RESERVES, [], alpha);
    const res = await request(buildApp(mockPool(POOL_ROWS), redis)).get("/api/pairs");
    expect(res.status).toBe(200);
    expect(res.body.entries[0].alpha_forward).toBeNull();
    expect(res.body.entries[0].alpha_reverse).toBeNull();
  });

  it("alpha rides only via the canonical (a<b) field — a reversed field never matches", async () => {
    const alpha = {
      [`${B}|${A}`]: JSON.stringify({ forward: 9.9, reverse: 9.9 }), // wrong order
    };
    const { redis } = mockRedis(RESERVES, [], alpha);
    const res = await request(buildApp(mockPool(POOL_ROWS), redis)).get("/api/pairs");
    expect(res.status).toBe(200);
    expect(res.body.entries[0].alpha_forward).toBeNull(); // no join on a mis-keyed field
  });

  it("a transient HGETALL error is 503 redis_unavailable (alpha unknowable, never guessed)", async () => {
    const redis = { mget: vi.fn(async () => []), smembers: vi.fn(async () => []), hgetall: vi.fn(async () => { throw new Error("ECONNRESET"); }) };
    const res = await request(buildApp(mockPool(POOL_ROWS), redis)).get("/api/pairs");
    expect(res.status).toBe(503);
    expect(res.body.error).toBe("redis_unavailable");
  });
});
