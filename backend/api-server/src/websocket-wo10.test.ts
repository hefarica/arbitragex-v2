/**
 * WO-10 (2026-09-06) — detection→broadcast latency observability tests.
 *
 * RULE 00: nothing here fabricates latency data — the tests exercise the PURE
 * percentile helper and the broadcastOpportunity contract (payload passthrough
 * unchanged; unparseable `detected_at` skips observation instead of throwing
 * or inventing a number). The Rust-side spans are verified by unit tests in
 * backend/searcher-rs/src/publisher.rs (registry + R8 guards).
 */

import { describe, expect, it, vi } from "vitest";
import type { Server as IoServer } from "socket.io";
import { broadcastOpportunity, wo10PercentileStats } from "./websocket.js";

describe("WO-10 wo10PercentileStats (pure)", () => {
    it("returns zeros for an empty window (fail-honest: no samples ≠ 0ms)", () => {
        const s = wo10PercentileStats([]);
        expect(s).toEqual({ count: 0, p50: 0, p95: 0, p99: 0, max: 0 });
    });

    it("computes monotone percentiles over a known distribution", () => {
        // 1..100 → p50 between 50 and 51, p95 ≥ 95, p99 ≥ 99, max 100.
        const samples = Array.from({ length: 100 }, (_, i) => i + 1);
        const s = wo10PercentileStats(samples);
        expect(s.count).toBe(100);
        expect(s.max).toBe(100);
        expect(s.p50).toBeGreaterThanOrEqual(50);
        expect(s.p50).toBeLessThanOrEqual(51);
        expect(s.p95).toBeGreaterThanOrEqual(95);
        expect(s.p99).toBeGreaterThanOrEqual(99);
        // Monotonicity: p50 ≤ p95 ≤ p99 ≤ max.
        expect(s.p50).toBeLessThanOrEqual(s.p95);
        expect(s.p95).toBeLessThanOrEqual(s.p99);
        expect(s.p99).toBeLessThanOrEqual(s.max);
    });

    it("accepts a Float64Array window (the ring's native type)", () => {
        const s = wo10PercentileStats(Float64Array.from([10, 20, 30]));
        expect(s.count).toBe(3);
        expect(s.max).toBe(30);
        expect(s.p50).toBe(20);
    });
});

describe("WO-10 broadcastOpportunity E2E observation", () => {
    function fakeIo() {
        const emit = vi.fn();
        const io = { to: (_room: string) => ({ emit }) } as unknown as IoServer;
        return { io, emit };
    }

    it("keeps the wire contract byte-identical: event + payload passthrough", () => {
        const { io, emit } = fakeIo();
        const opp = { id: "abc", detected_at: new Date().toISOString() };
        broadcastOpportunity(io, opp);
        expect(emit).toHaveBeenCalledTimes(1);
        expect(emit).toHaveBeenCalledWith("new_opportunity", opp);
    });

    it("does not throw and still broadcasts when detected_at is absent (R8 skip, not fabricate)", () => {
        const { io, emit } = fakeIo();
        expect(() => broadcastOpportunity(io, { id: "abc" })).not.toThrow();
        expect(emit).toHaveBeenCalledTimes(1);
        expect(emit).toHaveBeenCalledWith("new_opportunity", { id: "abc" });
    });

    it("does not throw on unparseable detected_at", () => {
        const { io } = fakeIo();
        expect(() =>
            broadcastOpportunity(io, { id: "abc", detected_at: "not-a-timestamp" }),
        ).not.toThrow();
    });

    it("does not throw on a null payload object", () => {
        const { io, emit } = fakeIo();
        expect(() => broadcastOpportunity(io, null)).not.toThrow();
        expect(emit).toHaveBeenCalledTimes(1);
        expect(emit).toHaveBeenCalledWith("new_opportunity", null);
    });
});
