// frontend/lib/store/__tests__/omni-store-prune-stale.test.ts
//
// MEM-RENDER-01 regression tests: the live grid must keep ONLY active/vigent
// cards. Before the fix, in LIVE mode nothing ever removed a dead card — the
// rolling 200-item window retained hours of stale routes (renderer memory grew
// to multi-GB). pruneStale is the vigency eviction; setOpportunities keeps the
// batched merge semantics the WS flush relies on.
//
// R8 fail-honest: an opportunity whose detected_at cannot be parsed is KEPT —
// we never silently drop data we cannot date.
import { describe, it, expect, beforeEach, vi } from "vitest";
import { useOmniStore } from "@/lib/store/omni-store";
import type { OmniOpportunity } from "@/lib/store/types";

function makeOpp(id: string, over: Partial<OmniOpportunity> = {}): OmniOpportunity {
  return {
    id,
    chain_id: 1,
    strategy_kind: "dex_arb",
    detected_at: "2026-08-18T00:00:00Z",
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

const NOW = Date.parse("2026-08-22T12:00:00Z");

describe("omni-store pruneStale — MEM-RENDER-01 vigency eviction", () => {
  beforeEach(() => {
    useOmniStore.getState().clearOpportunities();
  });

  it("drops cards detected before the cutoff and keeps vigent ones", () => {
    const iso = (msAgo: number) => new Date(NOW - msAgo).toISOString();
    const { setOpportunities } = useOmniStore.getState();
    setOpportunities([
      makeOpp("fresh-1s", { detected_at: iso(1_000) }),
      makeOpp("edge-4m59s", { detected_at: iso(4 * 60_000 + 59_000) }),
      makeOpp("stale-5m01s", { detected_at: iso(5 * 60_000 + 1_000) }),
      makeOpp("stale-1h", { detected_at: iso(60 * 60_000) }),
    ]);
    vi.setSystemTime(NOW);
    useOmniStore.getState().pruneStale(5 * 60_000);
    const ids = useOmniStore.getState().opportunities.map((o) => o.id);
    expect(ids).toEqual(["fresh-1s", "edge-4m59s"]);
    vi.useRealTimers();
  });

  it("KEEPS an opportunity whose detected_at is unparseable (R8 fail-honest)", () => {
    const { setOpportunities } = useOmniStore.getState();
    setOpportunities([
      makeOpp("undated", { detected_at: "not-a-date" }),
      makeOpp("fresh", { detected_at: new Date(NOW).toISOString() }),
    ]);
    vi.setSystemTime(NOW);
    useOmniStore.getState().pruneStale(5 * 60_000);
    const ids = useOmniStore.getState().opportunities.map((o) => o.id);
    expect(ids).toContain("undated");
    vi.useRealTimers();
  });

  it("is a no-op (same state reference) when nothing is stale — no churn on quiet feeds", () => {
    const { setOpportunities } = useOmniStore.getState();
    setOpportunities([makeOpp("fresh", { detected_at: new Date(NOW).toISOString() })]);
    const before = useOmniStore.getState();
    vi.setSystemTime(NOW);
    useOmniStore.getState().pruneStale(5 * 60_000);
    expect(useOmniStore.getState()).toBe(before);
    vi.useRealTimers();
  });
});

describe("omni-store setOpportunities — MEM-RENDER-01 batched WS flush semantics", () => {
  beforeEach(() => {
    useOmniStore.getState().clearOpportunities();
  });

  it("one batch call merges new ids on top and preserves existing order", () => {
    const { setOpportunities } = useOmniStore.getState();
    // Simulate a 1 Hz flush containing a burst of buffered events.
    setOpportunities([makeOpp("a"), makeOpp("b")]);
    setOpportunities([makeOpp("c"), makeOpp("a", { status: "scored" })]);
    const list = useOmniStore.getState().opportunities;
    // New ids prepended; 'a' updated in place below them; order stable.
    expect(list.map((o) => o.id)).toEqual(["c", "a", "b"]);
    expect(list.find((o) => o.id === "a")!.status).toBe("scored");
  });

  it("caps the window at 200 entries (batch cannot grow the heap beyond the cap)", () => {
    const { setOpportunities } = useOmniStore.getState();
    const burst = Array.from({ length: 260 }, (_, i) => makeOpp(`opp-${i}`));
    setOpportunities(burst);
    expect(useOmniStore.getState().opportunities.length).toBeLessThanOrEqual(200);
  });
});
