// frontend/lib/store/__tests__/ws-ingest-buffer.test.ts
//
// FE-0047 — the MEM-RENDER-01 WS ingest buffer as a pure seam (§33 realtime
// semantics). Extracted verbatim from useOmniOpportunities so dedup /
// out-of-order / flush cadence are testable without renderHook (node env).
//
// Pins the streaming contract the store's in-place upsert depends on:
//   - DUPLICATE: re-broadcasts of the same id collapse to ONE row per flush
//     window (Map key semantics).
//   - OUT-OF-ORDER: last ARRIVAL wins the CONTENT while the row keeps its
//     FIRST-arrival position (JS Map.set does not move an existing key) —
//     the same doctrine the store applies to card positions.
//   - FLUSH: one batch per cadence, then empty; empty window → [] (never
//     null — R8: absence is an honest empty array).
import { describe, it, expect } from "vitest";
import { createWsIngestBuffer } from "@/lib/store/ws-ingest-buffer";
import type { OmniOpportunity } from "@/lib/store/types";

// Same factory pattern as omni-store-upsert.test.ts (cast — the buffer only
// keys on `.id`; full row shape is display data for these tests).
function makeOpp(id: string, over: Partial<OmniOpportunity> = {}): OmniOpportunity {
  return {
    id,
    chain_id: 1,
    strategy_kind: "dex_arb",
    detected_at: "2026-08-24T00:00:00Z",
    trace_id: `trace-${id}`,
    dex_a: "uniswap_v2",
    dex_b: null,
    pair_symbol: null,
    token_in: "0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
    token_out: "0xbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
    amount_in_wei: "0",
    token_in_info: null,
    token_out_info: null,
    chain_base_token_symbol: null,
    expected_profit_usd: null,
    net_expected_profit_usd: null,
    roi_pct: null,
    risk_score: null,
    status: "detected",
    rejection_reason: null,
    paper_status: null,
    block_number: null,
    chain_id_out: null,
    bridge: null,
    bridge_fee_usd: null,
    chains_used: [],
    dexes_used: [],
    route_metadata: null,
    ...over,
  } as OmniOpportunity;
}

describe("ws-ingest-buffer — duplicate collapse (one row per id per window)", () => {
  it("re-broadcasts of the same id collapse to one row per flush", () => {
    const buffer = createWsIngestBuffer();
    buffer.upsert(makeOpp("opp-1", { block_number: 100 }));
    buffer.upsert(makeOpp("opp-2"));
    buffer.upsert(makeOpp("opp-1", { block_number: 101 })); // re-broadcast
    buffer.upsert(makeOpp("opp-1", { block_number: 102 })); // re-broadcast
    const batch = buffer.flush();
    expect(batch).toHaveLength(2);
    expect(batch.map((o) => o.id).sort()).toEqual(["opp-1", "opp-2"]);
  });
});

describe("ws-ingest-buffer — out-of-order arrivals (last content, first position)", () => {
  it("last arrival wins the CONTENT, first arrival keeps the POSITION", () => {
    const buffer = createWsIngestBuffer();
    buffer.upsert(makeOpp("opp-1", { status: "detected", expected_profit_usd: null }));
    buffer.upsert(makeOpp("opp-2"));
    // Late update for opp-1 arrives while opp-3 is already buffered.
    buffer.upsert(makeOpp("opp-3"));
    buffer.upsert(makeOpp("opp-1", { status: "scored", expected_profit_usd: 12.5 }));

    const batch = buffer.flush();
    // First-arrival order: opp-1 stays FIRST (Map.set does not move the key)…
    expect(batch.map((o) => o.id)).toEqual(["opp-1", "opp-2", "opp-3"]);
    // …but its content is the LAST arrival (execution-time values prevail).
    const updated = batch.find((o) => o.id === "opp-1")!;
    expect(updated.status).toBe("scored");
    expect(updated.expected_profit_usd).toBe(12.5);
  });
});

describe("ws-ingest-buffer — flush cadence", () => {
  it("flush returns the batch AND clears (next window starts empty)", () => {
    const buffer = createWsIngestBuffer();
    buffer.upsert(makeOpp("opp-1"));
    buffer.upsert(makeOpp("opp-2"));
    const first = buffer.flush();
    expect(first.map((o) => o.id)).toEqual(["opp-1", "opp-2"]);
    const second = buffer.flush();
    expect(second).toEqual([]);
  });

  it("empty window returns an honest [] (never null, R8)", () => {
    const buffer = createWsIngestBuffer();
    expect(buffer.flush()).toEqual([]);
  });
});

describe("ws-ingest-buffer — clear (unmount / dispose path)", () => {
  it("clear drops everything without emitting", () => {
    const buffer = createWsIngestBuffer();
    buffer.upsert(makeOpp("opp-1"));
    buffer.upsert(makeOpp("opp-2"));
    buffer.clear();
    expect(buffer.flush()).toEqual([]);
  });
});
