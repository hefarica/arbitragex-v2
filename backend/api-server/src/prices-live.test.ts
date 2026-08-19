/**
 * G-PRICE-1 — integration tests for GET /api/v1/prices/live.
 *
 * Express + supertest with a minimal in-memory Redis double (hgetall/ttl) —
 * no testcontainers needed (route touches Redis only). Mirrors the
 * opportunities-live.test.ts harness shape without its PG container.
 *
 * Tests:
 *   1. 200 happy path: validated/uppercased prices, count, ttl_secs, ts
 *   2. R8: NaN/negative/zero entries dropped, never coerced
 *   3. 400 when chain_id missing or non-integer
 *   4. 503 when Redis is null (redis_unavailable)
 *   5. 200 with empty prices when the hash is absent (honest empty)
 */

import { describe, it, expect } from "vitest";
import express from "express";
import request from "supertest";
import type { Redis } from "ioredis";
import { mountPricesLive } from "./routes/prices-live.js";

function fakeRedis(hash: Record<string, string>, ttl: number): Redis {
    return {
        hgetall: async (_key: string) => ({ ...hash }),
        ttl: async (_key: string) => ttl,
    } as unknown as Redis;
}

function appWith(redis: Redis | null) {
    const app = express();
    mountPricesLive(app, redis);
    return app;
}

describe("GET /api/v1/prices/live (G-PRICE-1)", () => {
    it("returns a validated, uppercased snapshot with ttl and ts", async () => {
        const app = appWith(
            fakeRedis({ WETH: "2500.5", usdc: "1.0001", BAD: "NaN", NEG: "-1", ZERO: "0" }, 53),
        );
        const res = await request(app).get("/api/v1/prices/live?chain_id=1");
        expect(res.status).toBe(200);
        expect(res.body.chain_id).toBe(1);
        expect(res.body.ttl_secs).toBe(53);
        expect(typeof res.body.ts).toBe("string");
        expect(Object.keys(res.body.prices).sort()).toEqual(["USDC", "WETH"]);
        expect(res.body.count).toBe(2);
    });

    it("drops NaN/negative/zero entries (R8 — never coerced)", async () => {
        const app = appWith(fakeRedis({ A: "NaN", B: "-1", C: "0", D: "Infinity", E: "3.5" }, 10));
        const res = await request(app).get("/api/v1/prices/live?chain_id=137");
        expect(res.status).toBe(200);
        expect(res.body.prices).toEqual({ E: 3.5 });
        expect(res.body.count).toBe(1);
    });

    it("400 when chain_id is missing", async () => {
        const res = await request(appWith(fakeRedis({}, -2))).get("/api/v1/prices/live");
        expect(res.status).toBe(400);
        expect(res.body.error).toBe("invalid_chain_id");
    });

    it("400 when chain_id is not a positive integer", async () => {
        const res = await request(appWith(fakeRedis({}, -2))).get("/api/v1/prices/live?chain_id=abc");
        expect(res.status).toBe(400);
        expect(res.body.error).toBe("invalid_chain_id");
    });

    it("503 when Redis is not configured", async () => {
        const res = await request(appWith(null)).get("/api/v1/prices/live?chain_id=1");
        expect(res.status).toBe(503);
        expect(res.body.error).toBe("redis_unavailable");
    });

    it("returns an honest EMPTY snapshot when the hash is absent", async () => {
        const res = await request(appWith(fakeRedis({}, -2))).get("/api/v1/prices/live?chain_id=8453");
        expect(res.status).toBe(200);
        expect(res.body.prices).toEqual({});
        expect(res.body.count).toBe(0);
        expect(res.body.ttl_secs).toBeNull();
    });
});
