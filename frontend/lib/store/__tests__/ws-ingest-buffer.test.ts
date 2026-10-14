// frontend/lib/store/__tests__/ws-ingest-buffer.test.ts
//
// FE-0047 — the MEM-RENDER-01 WS ingest buffer as a pure seam (§33 realtime
// semantics). Extracted verbatim from useOmniOpportunities so dedup /
// out-of-order / flush cadence are testable without renderHook (node env).
//
// Pins the streaming contract the store's in-place upsert depends on:
//   - DUPLICATE: re-broadcasts of the same id collapse to ONE row per flush
//     window (Map key semantics).
//   - OUT-OF-ORDER: a NEWER computation on the SAME detection replaces the
//     content while the row keeps its FIRST-arrival position (JS Map.set does
//     not move an existing key) — the same doctrine the store applies to card
//     positions. An OLDER observation is REJECTED (STREAM-SEQ-01, §11.4) — see
//     the dedicated block at the bottom, which corrects the previous
//     "last ARRIVAL wins the CONTENT" rule.
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

// ─── STREAM-SEQ-01 (§11.4) — an OLD event never overwrites a NEW one ─────────
// This is the correction of the rule the previous header documented ("last
// ARRIVAL wins the CONTENT"). Arrival order is not authority on a push channel:
// a NOTIFY replay, a reconnected socket and an overlapping REST snapshot all
// deliver indistinguishable messages.
describe("ws-ingest-buffer — §11.4 an older observation is REJECTED, not applied", () => {
  const at = (iso: string, over: Partial<OmniOpportunity> = {}) =>
    makeOpp("opp-1", { detected_at: iso, ...over });

  it("same id, OLDER clock: the newer content survives and the rejection is counted", () => {
    const buffer = createWsIngestBuffer();
    buffer.upsert(at("2026-08-24T00:05:00Z", { status: "scored", expected_profit_usd: 12.5 }));
    const outcome = buffer.upsert(
      at("2026-08-24T00:00:00Z", { status: "detected", expected_profit_usd: null }),
    );
    expect(outcome).toBe("stale_rejected");
    // Counters are per-WINDOW: read them before the flush resets the window.
    expect(buffer.counters().stale_rejected).toBe(1);
    const [row] = buffer.flush();
    expect(row!.status).toBe("scored");
    expect(row!.expected_profit_usd).toBe(12.5);
  });

  it("NEW id, same plan, older clock: a delayed re-detection is rejected too", () => {
    const buffer = createWsIngestBuffer();
    const route = {
      token_in: "0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
      token_out: "0xbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
      dex_a: "uniswap_v2",
      dex_b: "sushiswap",
    };
    buffer.upsert(makeOpp("det-2", { ...route, detected_at: "2026-08-24T00:05:00Z" }));
    const outcome = buffer.upsert(
      makeOpp("det-1", { ...route, detected_at: "2026-08-24T00:00:00Z" }),
    );
    expect(outcome).toBe("stale_rejected");
    expect(buffer.counters().stale_rejected).toBe(1);
    expect(buffer.flush().map((o) => o.id)).toEqual(["det-2"]);
  });

  it("a NEWER computation on the SAME detection is still an accepted UPDATE (migration-107)", () => {
    const buffer = createWsIngestBuffer();
    buffer.upsert(at("2026-08-24T00:00:00Z"));
    expect(buffer.upsert(at("2026-08-24T00:00:00Z", { status: "validated" }))).toBe(
      "accepted_update",
    );
    expect(buffer.counters().accepted).toBe(2);
  });

  it("a row we cannot date is ACCEPTED and counted as incomparable (R8: never dropped)", () => {
    const buffer = createWsIngestBuffer();
    buffer.upsert(at("2026-08-24T00:05:00Z"));
    expect(buffer.upsert(at(null as unknown as string))).toBe("accepted_incomparable");
    expect(buffer.counters().incomparable).toBe(1);
  });

  it("an identical replay is a NO-OP (duplicate), so reconnect replays cannot churn the grid", () => {
    const buffer = createWsIngestBuffer();
    buffer.upsert(at("2026-08-24T00:00:00Z", { expected_profit_usd: 3 }));
    expect(buffer.upsert(at("2026-08-24T00:00:00Z", { expected_profit_usd: 3 }))).toBe("duplicate");
    expect(buffer.counters().duplicates).toBe(1);
    expect(buffer.flush()).toHaveLength(1);
  });

  it("flush resets the window counters (each window reports its own truth)", () => {
    const buffer = createWsIngestBuffer();
    buffer.upsert(at("2026-08-24T00:00:00Z"));
    buffer.flush();
    expect(buffer.counters()).toEqual({
      accepted: 0,
      duplicates: 0,
      stale_rejected: 0,
      incomparable: 0,
    });
  });
});
