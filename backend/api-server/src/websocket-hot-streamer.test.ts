/**
 * WO-15 (2026-09-06) — consumer-group hygiene invariants for
 * OpportunityHotStreamer (D-9 systemic leak: every boot registered
 * `ws-emitter-<pid>-<ts>` and nothing ever ran XGROUP DELCONSUMER).
 *
 * Verified with an injected Redis command spy (test double of the CLIENT, not
 * of data — RULE 00 applies to served data, this exercises the real streamer):
 *
 *   1. stop() deregisters the boot's consumer from BOTH hot streams
 *      (XGROUP DELCONSUMER) BEFORE closing the connection — and reports the
 *      number of pending entries it discarded (must stay 0, zero-loss).
 *   2. purgeIdleConsumers removes only idle orphans, never the live consumer
 *      nor a recent predecessor, and XAUTOCLAIMs pending entries BEFORE the
 *      DELCONSUMER of the orphan that owned them (zero-loss ordering).
 *   3. pollLoop acknowledges every entry it broadcast (XACK) — the invariant
 *      that keeps our own PEL empty so DELCONSUMER can never drop entries.
 */

import { describe, it, expect, vi } from "vitest";
import type { Server as IoServer } from "socket.io";
import type { Redis } from "ioredis";
import { OpportunityHotStreamer } from "./websocket.js";

type Call = { cmd: string; args: unknown[] };

interface FakeRedisOptions {
    /** Lazy so tests can include the streamer's own (unknown-ahead) consumerName. */
    consumers?: () => Array<{ name: string; pending: number; idle: number }>;
    autoclaimEntries?: [string, string[]][];
    xreadgroupResults?: Array<[string, [string, string[]][]] | null>;
}

/** Records every command so tests assert exact ordering (XAUTOCLAIM < DELCONSUMER). */
function makeFakeRedis(opts: FakeRedisOptions) {
    const calls: Call[] = [];
    let readCalls = 0;
    const fake = {
        calls,
        xgroup: async (sub: string, ...args: unknown[]) => {
            calls.push({ cmd: `xgroup:${sub}`, args });
            return sub === "DELCONSUMER" ? 0 : "OK";
        },
        xinfo: async (...args: unknown[]) => {
            calls.push({ cmd: "xinfo", args });
            return (opts.consumers?.() ?? []).map((c) => ({ ...c }));
        },
        xautoclaim: async (...args: unknown[]) => {
            calls.push({ cmd: "xautoclaim", args });
            return ["0-0", opts.autoclaimEntries ?? []];
        },
        xack: async (...args: unknown[]) => {
            calls.push({ cmd: "xack", args });
            return 1;
        },
        xreadgroup: async (...args: unknown[]) => {
            calls.push({ cmd: "xreadgroup", args });
            const r = opts.xreadgroupResults ?? [];
            const res = r[Math.min(readCalls, r.length - 1)] ?? null;
            readCalls++;
            if (!res) await new Promise((resolve) => setTimeout(resolve, 10));
            return res;
        },
        quit: async () => {
            calls.push({ cmd: "quit", args: [] });
            return "OK";
        },
    };
    return fake;
}

function makeStreamer(fake: ReturnType<typeof makeFakeRedis>) {
    const emit = vi.fn();
    const io = { to: () => ({ emit }) } as unknown as IoServer;
    const logger = { info: vi.fn(), warn: vi.fn(), error: vi.fn() };
    const streamer = new OpportunityHotStreamer({
        io,
        redisUrl: "redis://unused-in-tests",
        logger,
        redisClient: fake as unknown as Redis,
    });
    return { streamer, emit, logger, consumerName: (streamer as unknown as { consumerName: string }).consumerName };
}

const HOT_STREAMS = ["arbx:hot:detected", "arbx:hot:simulated"] as const;
const HOUR_MS = 3_600_000;

describe("WO-15: OpportunityHotStreamer consumer-group hygiene", () => {
    it("stop() XGROUP DELCONSUMERs this boot's consumer from BOTH streams before quit", async () => {
        const fake = makeFakeRedis({});
        const { streamer, consumerName } = makeStreamer(fake);

        await streamer.stop();

        const dels = fake.calls.filter((c) => c.cmd === "xgroup:DELCONSUMER");
        expect(dels).toHaveLength(2);
        for (const stream of HOT_STREAMS) {
            const del = dels.find((c) => c.args[0] === stream);
            expect(del, `DELCONSUMER missing for ${stream}`).toBeDefined();
            // args: [stream, group, consumer]
            expect(del?.args[1]).toBe("ws-emitter-g0");
            expect(del?.args[2]).toBe(consumerName);
        }
        // Ordering: every DELCONSUMER precedes quit() (connection teardown).
        const firstDel = fake.calls.findIndex((c) => c.cmd === "xgroup:DELCONSUMER");
        const quitAt = fake.calls.findIndex((c) => c.cmd === "quit");
        expect(quitAt).toBeGreaterThan(firstDel);
        expect(firstDel).toBeGreaterThanOrEqual(0);
    });

    it("purge removes only idle orphans and XAUTOCLAIMs pending entries BEFORE their DELCONSUMER", async () => {
        const orphanPending = { name: "ws-emitter-111-1", pending: 2, idle: 200 * HOUR_MS };
        let liveName = "";
        const fake = makeFakeRedis({
            consumers: () => [
                // (idle in ms; a live peer resets its idle every XREADGROUP and
                // a recent predecessor is under the threshold — both are kept)
                { name: "ws-emitter-222-2", pending: 0, idle: 1000 },
                { name: "ws-emitter-333-3", pending: 5, idle: 60_000 },
                { name: "ws-emitter-444-4", pending: 0, idle: 200 * HOUR_MS },
                orphanPending,
                { name: liveName, pending: 0, idle: 1 },
            ],
            autoclaimEntries: [["1788000000000-0", ["json", '{"id":"claimed"}']]],
        });
        const { streamer, emit, consumerName } = makeStreamer(fake);
        liveName = consumerName;

        await (streamer as unknown as { purgeIdleConsumers: () => Promise<void> }).purgeIdleConsumers();

        const delNames = fake.calls
            .filter((c) => c.cmd === "xgroup:DELCONSUMER")
            .map((c) => c.args[2]);
        // Only the two idle orphans are removed — never self, never recent.
        expect(delNames).toContain("ws-emitter-444-4");
        expect(delNames).toContain(orphanPending.name);
        expect(delNames).not.toContain("ws-emitter-222-2");
        expect(delNames).not.toContain("ws-emitter-333-3");
        expect(delNames).not.toContain(consumerName);

        // Zero-loss ordering: XAUTOCLAIM (this stream) happens BEFORE the
        // DELCONSUMER of the orphan that owned pending entries.
        const claimAt = fake.calls.findIndex(
            (c) => c.cmd === "xautoclaim" && c.args[0] === "arbx:hot:detected",
        );
        const delOrphanPendingAt = fake.calls.findIndex(
            (c) => c.cmd === "xgroup:DELCONSUMER" && c.args[2] === orphanPending.name,
        );
        expect(claimAt).toBeGreaterThanOrEqual(0);
        expect(delOrphanPendingAt).toBeGreaterThan(claimAt);

        // Claimed entry is re-broadcast (at-least-once) and acknowledged.
        expect(emit).toHaveBeenCalledWith(
            "opportunity:detected",
            expect.objectContaining({ _stream_id: "1788000000000-0", json: '{"id":"claimed"}' }),
        );
        expect(fake.calls.some((c) => c.cmd === "xack" && c.args[2] === "1788000000000-0")).toBe(true);
    });

    it("pollLoop XACKs every entry it broadcast (PEL stays empty — DELCONSUMER never drops entries)", async () => {
        const fake = makeFakeRedis({
            xreadgroupResults: [
                [["arbx:hot:detected", [["1788000000001-0", ["json", '{"id":"a"}']]]]],
                null,
            ],
        });
        const { streamer, emit } = makeStreamer(fake);

        await streamer.start();
        try {
            await vi.waitFor(
                () => {
                    expect(
                        fake.calls.some(
                            (c) => c.cmd === "xack" && (c.args as string[]).includes("1788000000001-0"),
                        ),
                    ).toBe(true);
                },
                { timeout: 2_000, interval: 25 },
            );
        } finally {
            await streamer.stop();
        }

        expect(emit).toHaveBeenCalledWith(
            "opportunity:detected",
            expect.objectContaining({ _stream_id: "1788000000001-0" }),
        );
    });
});
