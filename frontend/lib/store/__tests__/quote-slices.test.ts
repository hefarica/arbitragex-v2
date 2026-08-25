// frontend/lib/store/__tests__/quote-slices.test.ts
//
// FE-MASTER · FE-0013 tramo 2 — quote anchor slice (P4).
//
// Locks the store semantics, NOT the wire (wire locks live in the schema
// tests): live-snapshot fetch, loading guard, honest error/absence (R8 —
// null stays null, never a fabricated anchor), direct setter for the
// realtime push path. Mirrors the catalog-slices test shape. api-client is
// mocked at the module boundary because the unit under test is the slice
// state machine, not the transport.
import { beforeEach, describe, expect, it, vi } from "vitest";
import { create } from "zustand";

const getQuoteAnchor = vi.fn();

vi.mock("@/lib/api-client", () => ({
  getQuoteAnchor: (...a: unknown[]) => getQuoteAnchor(...a),
}));

import { createQuoteAnchorSlice, type QuoteAnchorSlice } from "../quote-slices";
import type { QuoteAnchorResponse } from "@/lib/apex/schemas";

const COMPONENTS = { prior: 92, liquidity: 88, venues: 70, stability: 95, cross_dex: 60 };
const WEIGHTS = { prior: 0.2, liquidity: 0.3, venues: 0.15, stability: 0.25, cross_dex: 0.1 };

const ANCHOR: QuoteAnchorResponse = {
  chain_id: 1,
  quote_symbol: "USDC",
  quote_score: 87.5,
  quote_version: 3,
  graph_version: 21_000_123,
  components: COMPONENTS,
  weights: WEIGHTS,
  tokens: [
    {
      symbol: "USDC",
      address: "0x" + "a".repeat(40),
      components: COMPONENTS,
      score: 87.5,
    },
    {
      symbol: "WETH",
      address: "0x" + "b".repeat(40),
      components: { prior: 80, liquidity: 90, venues: 65, stability: 50, cross_dex: 75 },
      score: 74.2,
    },
  ],
};

function makeStore() {
  // Compose through zustand so the slice's (set, get) wiring is exercised.
  return create<QuoteAnchorSlice>()((set, get) => createQuoteAnchorSlice(set, get));
}

beforeEach(() => {
  vi.clearAllMocks();
});

describe("QuoteAnchorSlice — live surface (P4 · EMIT-02)", () => {
  it("idle → fetch → ready: anchor lands verbatim, updatedAt stamped", async () => {
    getQuoteAnchor.mockResolvedValue({ ok: true, data: ANCHOR });
    const store = makeStore();
    expect(store.getState().quoteAnchor).toBeNull(); // R8: null = never served
    await store.getState().fetchQuoteAnchor(1);
    const s = store.getState();
    expect(s.quoteAnchorStatus).toBe("ready");
    expect(s.quoteAnchor).toBe(ANCHOR); // payload verbatim — §79: never recomputed
    expect(s.quoteAnchorError).toBeNull();
    expect(s.quoteAnchorUpdatedAt).not.toBeNull();
    expect(getQuoteAnchor).toHaveBeenCalledWith(1);
  });

  it("default chainId=1 when the consumer omits it (strategies tab is chain 1)", async () => {
    getQuoteAnchor.mockResolvedValue({ ok: true, data: ANCHOR });
    const store = makeStore();
    await store.getState().fetchQuoteAnchor();
    expect(getQuoteAnchor).toHaveBeenCalledWith(1);
  });

  it("loading guard: a poll while in-flight is a no-op (30s cadence overlap)", async () => {
    let release: (() => void) | null = null;
    getQuoteAnchor.mockImplementation(
      () => new Promise((resolve) => { release = () => resolve({ ok: true, data: ANCHOR }); }),
    );
    const store = makeStore();
    const first = store.getState().fetchQuoteAnchor(1);
    const second = store.getState().fetchQuoteAnchor(1); // in-flight
    release!();
    await Promise.all([first, second]);
    expect(getQuoteAnchor).toHaveBeenCalledTimes(1);
  });

  it("honest error: the endpoint's 503 reason lands as the error string, anchor stays null (RULE 00)", async () => {
    getQuoteAnchor.mockResolvedValue({ ok: false, error: "HTTP 503: quote_anchor_not_published" });
    const store = makeStore();
    await store.getState().fetchQuoteAnchor(1);
    const s = store.getState();
    expect(s.quoteAnchorStatus).toBe("error");
    expect(s.quoteAnchorError).toBe("HTTP 503: quote_anchor_not_published");
    expect(s.quoteAnchor).toBeNull();
  });

  it("recovery: a failed poll then a successful one flips back to ready", async () => {
    getQuoteAnchor
      .mockResolvedValueOnce({ ok: false, error: "HTTP 503: redis_unavailable" })
      .mockResolvedValueOnce({ ok: true, data: ANCHOR });
    const store = makeStore();
    await store.getState().fetchQuoteAnchor(1);
    await store.getState().fetchQuoteAnchor(1);
    const s = store.getState();
    expect(s.quoteAnchorStatus).toBe("ready");
    expect(s.quoteAnchor).toBe(ANCHOR);
    expect(s.quoteAnchorError).toBeNull(); // stale error cleared on the loading transition
  });

  it("setQuoteAnchor: direct setter stamps ready for the realtime push path", () => {
    const store = makeStore();
    store.getState().setQuoteAnchor(ANCHOR);
    const s = store.getState();
    expect(s.quoteAnchor).toBe(ANCHOR);
    expect(s.quoteAnchorStatus).toBe("ready");
    expect(s.quoteAnchorUpdatedAt).not.toBeNull();
  });
});
