/**
 * prices-dispatch.test.ts — α11 dispatcher seam (t5, 2026-10-13).
 *
 * Exercises the REAL production wiring: setupWebSocketGateway() (v4 global
 * bus arm, websocket.ts) + attachPriceRooms() (legacy per-chain arm,
 * prices-stream.ts) mounted on the SAME io — exactly as index.ts:1793/:1815.
 *
 * Dispatcher contract (ratified by captain α11 + backend-dev intel):
 *   - `subscribe:prices` with NO payload (undefined) → v4 arm ONLY:
 *     prices:ready + prices:snapshot {ticks} — and ZERO prices:error from the
 *     legacy arm (prices-stream.ts:104 null-yield guard).
 *   - `subscribe:prices` with {chain_id:N} → legacy arm ONLY: per-chain
 *     snapshot, ZERO prices:ready/{ticks} contamination from the v4 arm.
 *   - `subscribe:prices` with {} → legacy prices:error invalid_chain_id
 *     (prices-stream.test.ts:85-96 contract, byte-identical).
 *   - Fan-out: a bus tick reaches the global room, never the legacy socket.
 */
import { describe, it, expect, beforeAll, afterAll } from "vitest";
import { createServer, type Server as HttpServer } from "node:http";
import { io as ioClient, type Socket as ClientSocket } from "socket.io-client";

import { setupWebSocketGateway } from "../websocket.js";
import { attachPriceRooms } from "../prices-stream.js";
import { priceBus } from "./priceBus.js";

// Minimal fake of the shared command Redis: one hash for chain 1 + TTL 42.
const fakeRedis = {
    hgetall: async (_key: string): Promise<Record<string, string>> => ({ WETH: "3000.5", USDC: "1.0001" }),
    ttl: async (_key: string): Promise<number> => 42,
} as unknown as Parameters<typeof attachPriceRooms>[1];

function withTimeout<T>(p: Promise<T>, ms: number, label: string): Promise<T> {
    return new Promise<T>((resolve, reject) => {
        const t = setTimeout(() => reject(new Error(`timeout waiting ${label} (${ms}ms)`)), ms);
        p.then((v) => { clearTimeout(t); resolve(v); }, (e) => { clearTimeout(t); reject(e); });
    });
}

let httpServer: HttpServer;
let port: number;
let io: ReturnType<typeof setupWebSocketGateway>;

function client(): Promise<ClientSocket> {
    return new Promise((resolve) => {
        const sock = ioClient(`http://127.0.0.1:${port}`, { transports: ["websocket"] });
        sock.on("connect", () => resolve(sock));
    });
}

beforeAll(async () => {
    httpServer = createServer();
    // PRODUCTION ORDER (index.ts:1793 then :1815): gateway first, price rooms second.
    io = setupWebSocketGateway(httpServer);
    attachPriceRooms(io, fakeRedis);
    port = await new Promise<number>((resolve) => {
        httpServer.listen(0, () => {
            const addr = httpServer.address();
            resolve(typeof addr === "object" && addr ? addr.port : 0);
        });
    });
});

afterAll(async () => {
    await new Promise<void>((r) => io.close(() => r()));
    await new Promise<void>((r) => httpServer.close(() => r()));
});

describe("α11 dispatcher — payload-less subscribe → v4 arm ONLY", () => {
    it("gets prices:ready + prices:snapshot {ticks} and NO prices:error", async () => {
        const sock = await client();
        const ready = withTimeout(new Promise<Record<string, unknown>>((res) =>
            sock.on("prices:ready", (e: Record<string, unknown>) => res(e))), 3000, "prices:ready");
        const snap = withTimeout(new Promise<Record<string, unknown>>((res) =>
            sock.on("prices:snapshot", (e: Record<string, unknown>) => res(e))), 3000, "prices:snapshot");
        let strayError: Record<string, unknown> | null = null;
        sock.on("prices:error", (e: Record<string, unknown>) => { strayError = e; });

        sock.emit("subscribe:prices"); // payload-less — the v4 shape

        const s = await snap;
        await ready;
        // Empty bus ⇒ honest empty snapshot (R8/RG-01: no wire producer yet).
        expect(Array.isArray((s as { ticks?: unknown[] }).ticks)).toBe(true);
        expect((s as { ticks: unknown[] }).ticks).toEqual([]);
        // The guard must have silenced the legacy arm for this shape.
        await new Promise((r) => setTimeout(r, 150));
        expect(strayError).toBeNull();
        sock.close();
    }, 8000);
});

describe("α11 dispatcher — {chain_id} subscribe → legacy arm ONLY", () => {
    it("gets the legacy per-chain snapshot and NO v4 prices:ready/{ticks}", async () => {
        const sock = await client();
        const snap = withTimeout(new Promise<Record<string, unknown>>((res) =>
            sock.on("prices:snapshot", (e: Record<string, unknown>) => res(e))), 3000, "prices:snapshot");
        let strayReady = false;
        sock.on("prices:ready", () => { strayReady = true; });

        sock.emit("subscribe:prices", { chain_id: 1 });

        const s = await snap;
        expect(s["chain_id"]).toBe(1);
        expect(s["prices"]).toEqual({ WETH: 3000.5, USDC: 1.0001 });
        expect(s["count"]).toBe(2);
        expect(s["ttl_secs"]).toBe(42);
        expect(typeof s["seq"]).toBe("number");
        await new Promise((r) => setTimeout(r, 150));
        expect(strayReady).toBe(false);
        sock.close();
    }, 8000);
});

describe("α11 dispatcher — {} subscribe → legacy invalid_chain_id intact", () => {
    it("still emits prices:error invalid_chain_id (prices-stream.test.ts:85-96 contract)", async () => {
        const sock = await client();
        const err = withTimeout(new Promise<Record<string, unknown>>((res) =>
            sock.on("prices:error", (e: Record<string, unknown>) => res(e))), 3000, "prices:error");

        sock.emit("subscribe:prices", {});

        const e = await err;
        expect(e["code"]).toBe("invalid_chain_id");
        sock.close();
    }, 8000);
});

describe("α11 dispatcher — bus fan-out reaches the global room only", () => {
    it("publish() → prices:tick to the v4 subscriber, NOT to the legacy subscriber", async () => {
        const v4sock = await client();
        const legacySock = await client();

        const tickP = withTimeout(new Promise<Record<string, unknown>>((res) =>
            v4sock.on("prices:tick", (e: Record<string, unknown>) => res(e))), 3000, "prices:tick");
        let legacyGotTick = false;
        legacySock.on("prices:tick", () => { legacyGotTick = true; });

        v4sock.emit("subscribe:prices"); // join global room
        await new Promise((r) => setTimeout(r, 100));
        legacySock.emit("subscribe:prices", { chain_id: 1 }); // join per-chain room
        await new Promise((r) => setTimeout(r, 100));

        priceBus.publish({ pair: "WETH/USDC", mid: 3000.5, ts: Date.now(), src: "dispatch-test" });

        const t = await tickP;
        expect(t["pair"]).toBe("WETH/USDC");
        expect(typeof t["seq"]).toBe("number");
        await new Promise((r) => setTimeout(r, 150));
        expect(legacyGotTick).toBe(false);
        v4sock.close();
        legacySock.close();
    }, 8000);
});
