/**
 * t4 μ-measurement harness — G-PRICE-2 SSE technique, LOCAL staging (nowire).
 *
 * Chain under measurement (the REAL modules, no doubles except the canonical
 * Redis read, which the technique does not change):
 *   writer trigger (broadcastPriceUpdate) → api-server SSE route frame
 *   → dev-local edge proxyStream → client reader receipt.
 *
 * m1 = per-update SSE delivery latency through the chain (publish→client).
 * m2 = loopback REST refresh RTT on the same api-server process (SANITY
 *      cross-check only — the admission formula uses the frozen x_ref wire
 *      baseline, NOT m2).
 *
 * Declared exclusions (fail-honest): the Redis PUBLISH→PSUBSCRIBE hop is not
 * measurable without a live Redis (ssh arbx exit=255; no local Redis per
 * RULE 01). It is an O(1) in-memory Redis op (sub-ms on the VPS docker
 * network) and is therefore EXCLUDED from m1 — declared, not estimated into
 * the number. The public-wire component `w` is handled as a declared BOUND
 * in ADM_registry.md §1, not measured here.
 *
 * Run: npx tsx docs/integration-v4/t4-evidence/measure_mu.ts   (from backend/api-server)
 */

import { createServer, type Server } from "node:http";
import express from "express";
import type { Redis } from "ioredis";
import { mountPricesStreamSse, broadcastPriceUpdate } from "../../../backend/api-server/src/routes/prices-stream-sse.js";
import { mountPricesLive } from "../../../backend/api-server/src/routes/prices-live.js";
import { proxyStream } from "../../../edge/dev-local/src/proxy-stream.js";

process.env["ARBX_PRICES_SSE_PING_MS"] = "60000"; // silence pings during measurement

const HASH = {
    WETH: "2500.5",
    USDC: "1.0001",
    WBTC: "65000.1",
    ARB: "0.85",
    LINK: "14.2",
    PEPE: "0.0000011",
    UNI: "7.77",
    AAVE: "161.2",
};
const fakeRedis = {
    hgetall: async (_key: string) => ({ ...HASH }),
    ttl: async (_key: string) => 42,
} as unknown as Redis;

function listen(server: Server): Promise<number> {
    return new Promise((resolve) => {
        server.listen(0, "127.0.0.1", () => {
            resolve((server.address() as { port: number }).port);
        });
    });
}

function stats(xs: number[]): { n: number; mean_ms: number; p50_ms: number; max_ms: number; min_ms: number } {
    const s = [...xs].sort((a, b) => a - b);
    const mean = xs.reduce((a, b) => a + b, 0) / xs.length;
    return {
        n: xs.length,
        mean_ms: Number(mean.toFixed(3)),
        p50_ms: Number(s[Math.floor(s.length / 2)].toFixed(3)),
        max_ms: Number(s[s.length - 1].toFixed(3)),
        min_ms: Number(s[0].toFixed(3)),
    };
}

async function main(): Promise<void> {
    // ── api-server app: SSE route + REST live route ─────────────────────────
    const apiApp = express();
    mountPricesStreamSse(apiApp, fakeRedis);
    mountPricesLive(apiApp, fakeRedis);
    const apiServer = createServer(apiApp);
    const apiPort = await listen(apiServer);

    // ── dev-local edge app: the real proxyStream route ─────────────────────
    const edgeApp = express();
    const streamPass = proxyStream({ apiServerUrl: `http://127.0.0.1:${apiPort}`, edgeToken: "measure" });
    edgeApp.get("/api/prices/stream", (req, res) => streamPass("/api/v1/prices/stream", req, res));
    const edgeServer = createServer(edgeApp);
    const edgePort = await listen(edgeServer);

    // ── m1: SSE per-update delivery through the chain ──────────────────────
    const ac = new AbortController();
    const res = await fetch(`http://127.0.0.1:${edgePort}/api/prices/stream?chain_id=1`, {
        signal: ac.signal,
        headers: { accept: "text/event-stream" },
    });
    if (res.status !== 200) throw new Error(`expected 200, got ${res.status}`);
    const reader = res.body!.getReader();
    const decoder = new TextDecoder();
    let buffer = "";

    /** Await the next SSE event with the given name (skipping others). */
    const nextEvent = (want: string): Promise<Record<string, unknown>> =>
        new Promise((resolve, reject) => {
            const step = async (): Promise<void> => {
                try {
                    const { done, value } = await reader.read();
                    if (done) return reject(new Error("stream ended"));
                    buffer += decoder.decode(value, { stream: true });
                    let sep: number;
                    while ((sep = buffer.indexOf("\n\n")) !== -1) {
                        const frame = buffer.slice(0, sep);
                        buffer = buffer.slice(sep + 2);
                        let event = "message";
                        const dataLines: string[] = [];
                        for (const line of frame.split("\n")) {
                            if (line.startsWith("event: ")) event = line.slice(7).trim();
                            else if (line.startsWith("data: ")) dataLines.push(line.slice(6));
                        }
                        if (event === want && dataLines.length > 0) {
                            return resolve(JSON.parse(dataLines.join("\n")) as Record<string, unknown>);
                        }
                    }
                    await step();
                } catch (e) {
                    reject(e as Error);
                }
            };
            void step();
        });

    await nextEvent("snapshot"); // consume the initial frame

    const N = 20;
    const m1: number[] = [];
    for (let i = 0; i < N; i++) {
        const t0 = performance.now();
        await broadcastPriceUpdate(fakeRedis as Redis, 1, "measure");
        await nextEvent("update");
        m1.push(performance.now() - t0);
    }
    ac.abort();

    // ── m2: loopback REST refresh RTT (sanity cross-check) ─────────────────
    const m2: number[] = [];
    for (let i = 0; i < N; i++) {
        const t0 = performance.now();
        const r = await fetch(`http://127.0.0.1:${apiPort}/api/v1/prices/live?chain_id=1`);
        await r.json();
        m2.push(performance.now() - t0);
    }

    await new Promise<void>((r) => apiServer.close(() => r()));
    await new Promise<void>((r) => edgeServer.close(() => r()));

    const out = {
        measured_at_utc: new Date().toISOString(),
        harness: "docs/integration-v4/t4-evidence/measure_mu.ts (tsx, node local staging)",
        chain: "broadcastPriceUpdate → api-server SSE route → dev-local proxyStream → client reader",
        declared_exclusion: "Redis PUBLISH→PSUBSCRIBE hop (no live Redis locally; O(1) in-memory Redis op, sub-ms on VPS docker net)",
        m1_sse_delivery: stats(m1),
        m2_rest_loopback_sanity: stats(m2),
        node: process.version,
    };
    console.log(JSON.stringify(out, null, 2));
}

void main().catch((e: Error) => {
    console.error("MEASUREMENT FAILED:", e.message);
    process.exit(1);
});
