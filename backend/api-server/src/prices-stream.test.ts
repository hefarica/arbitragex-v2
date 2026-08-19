/**
 * G-PRICE-1 — prices room regression (snapshot on subscribe).
 *
 * attachPriceRooms mounts the `subscribe:prices` handler: valid payload →
 * room join + immediate `prices:snapshot` built from the canonical Redis hash
 * (validated: finite/positive only, uppercased symbols); invalid payload →
 * `prices:error` and NO room join.
 *
 * Uses a minimal in-memory Redis double (hgetall/ttl) — the same
 * test-double discipline as the Rust StubOracle / wiremock suites.
 */

import { describe, it, expect, beforeAll, afterAll } from "vitest";
import { createServer, type Server as HttpServer } from "node:http";
import { Server as IoServer } from "socket.io";
import { io as ioClient } from "socket.io-client";
import type { Redis } from "ioredis";
import { attachPriceRooms } from "./prices-stream.js";

function fakeRedisFor(hash: Record<string, string>, ttl: number): Redis {
    return {
        hgetall: async (_key: string) => ({ ...hash }),
        ttl: async (_key: string) => ttl,
    } as unknown as Redis;
}

const GOOD_HASH = {
    WETH: "2500.5",
    usdc: "1.0001",
    BAD_NAN: "NaN",
    BAD_NEG: "-1",
    BAD_ZERO: "0",
    BAD_INF: "Infinity",
};

describe("G-PRICE-1 — prices room (snapshot on subscribe)", () => {
    let httpServer: HttpServer;
    let io: IoServer;
    let port: number;

    beforeAll(async () => {
        httpServer = createServer();
        io = new IoServer(httpServer);
        attachPriceRooms(io, fakeRedisFor(GOOD_HASH, 42));
        await new Promise<void>((resolve) => {
            httpServer.listen(0, () => {
                const addr = httpServer.address();
                port = typeof addr === "object" && addr ? addr.port : 0;
                resolve();
            });
        });
    });
    afterAll(async () => {
        await new Promise<void>((resolve) => io.close(() => resolve()));
        await new Promise<void>((resolve) => httpServer.close(() => resolve()));
    });

    function connect(): Promise<ReturnType<typeof ioClient>> {
        return new Promise((resolve, reject) => {
            const sock = ioClient(`http://127.0.0.1:${port}`, { transports: ["websocket"] });
            sock.on("connect", () => resolve(sock));
            sock.on("connect_error", (err: Error) => reject(err));
        });
    }

    it("emits a validated, uppercased snapshot on valid subscribe", async () => {
        const sock = await connect();
        const snap = await new Promise<Record<string, unknown>>((resolve, reject) => {
            sock.on("prices:snapshot", (evt: Record<string, unknown>) => resolve(evt));
            sock.on("prices:error", (e: Record<string, unknown>) => reject(new Error(JSON.stringify(e))));
            sock.emit("subscribe:prices", { chain_id: 1 });
        });
        expect(snap["chain_id"]).toBe(1);
        expect(snap["ttl_secs"]).toBe(42);
        expect(snap["seq"]).toBeGreaterThan(0);
        expect(typeof snap["ts"]).toBe("string");
        const prices = snap["prices"] as Record<string, number>;
        // R8: garbage entries (NaN/negative/zero/Infinity) dropped, not coerced.
        expect(Object.keys(prices).sort()).toEqual(["USDC", "WETH"]);
        expect(prices["WETH"]).toBe(2500.5);
        expect(prices["USDC"]).toBe(1.0001);
        sock.close();
    });

    it("rejects a subscribe without chain_id (prices:error, no snapshot)", async () => {
        const sock = await connect();
        const err = await new Promise<Record<string, unknown>>((resolve, reject) => {
            sock.on("prices:error", (e: Record<string, unknown>) => resolve(e));
            sock.on("prices:snapshot", (e: Record<string, unknown>) =>
                reject(new Error(`unexpected snapshot: ${JSON.stringify(e)}`)),
            );
            sock.emit("subscribe:prices", {});
        });
        expect(err["code"]).toBe("invalid_chain_id");
        sock.close();
    });

    it("rejects a non-integer chain_id", async () => {
        const sock = await connect();
        const err = await new Promise<Record<string, unknown>>((resolve) => {
            sock.on("prices:error", (e: Record<string, unknown>) => resolve(e));
            sock.emit("subscribe:prices", { chain_id: 1.5 });
        });
        expect(err["code"]).toBe("invalid_chain_id");
        sock.close();
    });

    it("emits an honest EMPTY snapshot when the hash is absent (R8)", async () => {
        // Second server with an empty hash — missing key, TTL -2.
        const hs = createServer();
        const io2 = new IoServer(hs);
        attachPriceRooms(io2, fakeRedisFor({}, -2));
        const p2 = await new Promise<number>((resolve) => {
            hs.listen(0, () => {
                const addr = hs.address();
                resolve(typeof addr === "object" && addr ? addr.port : 0);
            });
        });
        const sock = ioClient(`http://127.0.0.1:${p2}`, { transports: ["websocket"] });
        await new Promise<void>((r) => sock.on("connect", () => r()));
        const snap = await new Promise<Record<string, unknown>>((resolve) => {
            sock.on("prices:snapshot", (e: Record<string, unknown>) => resolve(e));
            sock.emit("subscribe:prices", { chain_id: 7 });
        });
        expect(snap["prices"]).toEqual({});
        expect(snap["count"]).toBe(0);
        expect(snap["ttl_secs"]).toBeNull();
        sock.close();
        await new Promise<void>((r) => io2.close(() => r()));
        await new Promise<void>((r) => hs.close(() => r()));
    });
});
