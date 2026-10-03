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
        // STREAM-SEQ-01 (§11.4): the raw leg is asserted BY NAME, not by the
        // total call count. `broadcastOpportunity` now also publishes the SAME
        // row on the ordered `opportunity_stream_event` leg, so "called once"
        // stopped being a way to say "the raw contract is untouched" — and a
        // count-based assertion would have to be deleted, not corrected, the
        // next time the room gains a leg. The contract being pinned is the
        // payload passthrough of `new_opportunity`, which is asserted exactly.
        expect(emit).toHaveBeenCalledWith("new_opportunity", opp);
        const rawCalls = emit.mock.calls.filter(([event]) => event === "new_opportunity");
        expect(rawCalls).toHaveLength(1);
    });

    it("§11.4: also publishes the SAME row on the ordered envelope leg", () => {
        const { io, emit } = fakeIo();
        const opp = { id: "abc", detected_at: new Date().toISOString() };
        broadcastOpportunity(io, opp);
        const streamCalls = emit.mock.calls.filter(
            ([event]) => event === "opportunity_stream_event",
        );
        expect(streamCalls).toHaveLength(1);
        const envelope = streamCalls[0]![1] as {
            schema_version: number;
            seq: number;
            payload: unknown;
        };
        expect(envelope.schema_version).toBe(2);
        expect(envelope.seq).toBeGreaterThan(0);
        // The envelope stamps identity+order; it never substitutes the row.
        expect(envelope.payload).toBe(opp);
    });

    it("does not throw and still broadcasts when detected_at is absent (R8 skip, not fabricate)", () => {
        const { io, emit } = fakeIo();
        expect(() => broadcastOpportunity(io, { id: "abc" })).not.toThrow();
        expect(emit).toHaveBeenCalledWith("new_opportunity", { id: "abc" });
        expect(
            emit.mock.calls.filter(([event]) => event === "new_opportunity"),
        ).toHaveLength(1);
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
        expect(emit).toHaveBeenCalledWith("new_opportunity", null);
        // A null row has NO identity to stamp: the ordered leg is not emitted at
        // all rather than carrying a fabricated key (R8).
        expect(
            emit.mock.calls.filter(([event]) => event === "opportunity_stream_event"),
        ).toHaveLength(0);
    });
});
