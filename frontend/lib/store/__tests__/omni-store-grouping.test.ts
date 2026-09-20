// frontend/lib/store/__tests__/omni-store-grouping.test.ts
//
// CARDS-DEDUP-HOPS (operator order 2026-09-20): the dashboard used to render
// ONE CARD PER DETECTION — the same route re-detected every few seconds
// duplicated its card again and again. The store now merges by ROUTE GROUP
// KEY (routeGroupKeyOf — the bit-for-bit twin of the grouped LIVE_QUERY):
// the card keeps its position (stable React key → no remount/flicker), the
// LATEST detection's economics replace the trade values in place, and the
// vigency aggregates roll forward (first_seen/last_seen/confirmations).
//
// Authority rule: a row carrying SERVER aggregates (grouped snapshot) is the
// SSOT and wins verbatim; plain WS rows roll the local count forward and the
// next snapshot reconciles it. R8 throughout: nothing fabricated.
import { describe, it, expect, beforeEach, vi } from "vitest";
import { useOmniStore } from "@/lib/store/omni-store";
import type { OmniOpportunity } from "@/lib/store/types";

function makeOpp(id: string, over: Partial<OmniOpportunity> = {}): OmniOpportunity {
  return {
    id,
    chain_id: 1,
    strategy_kind: "dex_arb",
    detected_at: "2026-09-20T09:00:00Z",
    trace_id: `trace-${id}`,
    dex_a: "uniswap_v2",
    dex_b: null,
    pair_symbol: null,
    token_in: `0xroute-${id}`,
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

// One shared route identity for re-detection fixtures.
const ROUTE = {
  token_in: "0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
  token_out: "0xbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
  dex_a: "uniswap_v2",
  dex_b: "sushiswap",
} as const;

const T1 = "2026-09-20T09:00:00Z";
const T2 = "2026-09-20T11:00:00Z";
const T3 = "2026-09-20T12:00:00Z";

describe("omni-store — CARDS-DEDUP-HOPS route-group merge", () => {
  beforeEach(() => {
    useOmniStore.getState().clearOpportunities();
  });

  it("addOpportunity: a re-detection (new id, same route) merges into the EXISTING card — no duplicate, position preserved", () => {
    const { addOpportunity } = useOmniStore.getState();
    addOpportunity(makeOpp("det-1", { ...ROUTE, detected_at: T1, net_expected_profit_usd: 1 }));
    // A DIFFERENT route between the two detections of ROUTE — proves the
    // merged card stays at its position, not at the top.
    addOpportunity(makeOpp("other-route", { detected_at: T2 }));
    addOpportunity(makeOpp("det-2", { ...ROUTE, detected_at: T3, net_expected_profit_usd: 5 }));

    const list = useOmniStore.getState().opportunities;
    expect(list).toHaveLength(2);
    // Position preserved: the ROUTE card stays below the newer other-route card.
    expect(list.map((o) => o.id)).toEqual(["other-route", "det-2"]);
    // Latest detection's economics replace the trade values in place.
    const card = list[1]!;
    expect(card.net_expected_profit_usd).toBe(5);
    // Aggregates roll forward: earliest first detection, latest ratification,
    // one re-detection counted locally (snapshot reconciles the exact total).
    expect(card.first_seen_at).toBe(T1);
    expect(card.last_seen_at).toBe(T3);
    expect(card.confirmations).toBe(1);
  });

  it("addOpportunity: a same-id row UPDATE keeps the rolled aggregates (R8 — WS rows carry none)", () => {
    const { addOpportunity } = useOmniStore.getState();
    addOpportunity(makeOpp("det-1", { ...ROUTE, detected_at: T1 }));
    addOpportunity(makeOpp("det-2", { ...ROUTE, detected_at: T3 }));
    addOpportunity(
      makeOpp("det-2", { ...ROUTE, detected_at: T3, net_expected_profit_usd: 9 }),
    );

    const card = useOmniStore.getState().opportunities[0]!;
    expect(card.id).toBe("det-2");
    expect(card.net_expected_profit_usd).toBe(9);
    expect(card.first_seen_at).toBe(T1);
    expect(card.last_seen_at).toBe(T3);
    expect(card.confirmations).toBe(1);
  });

  it("setOpportunities: intra-batch re-detections collapse to ONE card; newest-first economics win", () => {
    const { setOpportunities } = useOmniStore.getState();
    setOpportunities([
      makeOpp("det-3", { ...ROUTE, detected_at: T3, net_expected_profit_usd: 3 }),
      makeOpp("det-2", { ...ROUTE, detected_at: T2, net_expected_profit_usd: 2 }),
      makeOpp("det-1", { ...ROUTE, detected_at: T1, net_expected_profit_usd: 1 }),
      makeOpp("other-route", { detected_at: T1, net_expected_profit_usd: 9 }),
    ]);

    const list = useOmniStore.getState().opportunities;
    expect(list).toHaveLength(2);
    const card = list[0]!;
    expect(card.id).toBe("det-3"); // newest row of the group wins economics
    expect(card.net_expected_profit_usd).toBe(3);
    expect(card.first_seen_at).toBe(T1);
    expect(card.last_seen_at).toBe(T3);
    expect(card.confirmations).toBe(3);
  });

  it("setOpportunities: a server snapshot row (with aggregates) is SSOT and wins verbatim over local state", () => {
    const { setOpportunities } = useOmniStore.getState();
    // Local state rolled from raw WS pushes.
    setOpportunities([
      makeOpp("det-1", { ...ROUTE, detected_at: T1 }),
      makeOpp("det-2", { ...ROUTE, detected_at: T3 }),
    ]);
    expect(useOmniStore.getState().opportunities[0]!.confirmations).toBe(2);

    // Grouped LIVE_QUERY snapshot arrives with authoritative aggregates.
    setOpportunities([
      makeOpp("latest-id", {
        ...ROUTE,
        detected_at: T3,
        net_expected_profit_usd: 7,
        first_seen_at: T1,
        last_seen_at: T3,
        confirmations: 9,
      }),
    ]);

    const card = useOmniStore.getState().opportunities[0]!;
    expect(card.id).toBe("latest-id");
    expect(card.net_expected_profit_usd).toBe(7);
    expect(card.confirmations).toBe(9); // server COUNT(*), not the local tally
  });

  it("setOpportunities: OLDEST-first batch (ws-ingest-buffer flush order) still picks the LATEST detection's economics — WARN-1, adversarial review 2026-09-20", () => {
    const { setOpportunities } = useOmniStore.getState();
    // Same group as the intra-batch test above but ARRIVAL-ordered: the flush
    // of ws-ingest-buffer is a Map in insertion order = oldest-first. Array
    // position must never decide the economics.
    setOpportunities([
      makeOpp("det-1", { ...ROUTE, detected_at: T1, net_expected_profit_usd: 1 }),
      makeOpp("det-2", { ...ROUTE, detected_at: T2, net_expected_profit_usd: 2 }),
      makeOpp("det-3", { ...ROUTE, detected_at: T3, net_expected_profit_usd: 3 }),
    ]);

    const card = useOmniStore.getState().opportunities[0]!;
    expect(card.id).toBe("det-3"); // T3 (latest detected_at) wins, not rows[0]
    expect(card.net_expected_profit_usd).toBe(3);
    expect(card.first_seen_at).toBe(T1);
    expect(card.last_seen_at).toBe(T3);
    expect(card.confirmations).toBe(3);
  });

  it("setOpportunities: a WS push after a snapshot builds on the server aggregates", () => {
    const { setOpportunities } = useOmniStore.getState();
    setOpportunities([
      makeOpp("latest-id", {
        ...ROUTE,
        detected_at: T3,
        first_seen_at: T1,
        last_seen_at: T3,
        confirmations: 9,
      }),
    ]);
    setOpportunities([makeOpp("det-10", { ...ROUTE, detected_at: T3 })]);

    const card = useOmniStore.getState().opportunities[0]!;
    expect(card.first_seen_at).toBe(T1); // preserved from the snapshot
    expect(card.confirmations).toBe(10); // 9 server + 1 local re-detection
  });

  it("setOpportunities: legacy duplicate cards already in state collapse on the next batch", () => {
    useOmniStore.setState({
      opportunities: [
        makeOpp("dup-a", { ...ROUTE, detected_at: T1 }),
        makeOpp("dup-b", { ...ROUTE, detected_at: T2 }),
        makeOpp("other-route", { detected_at: T1 }),
      ],
    });
    useOmniStore.getState().setOpportunities([]);

    const list = useOmniStore.getState().opportunities;
    expect(list).toHaveLength(2);
    expect(list.map((o) => o.token_in)).toEqual([ROUTE.token_in, "0xroute-other-route"]);
  });

  it("setOpportunities: batch NEVER drops state rows (eviction belongs to pruneStale)", () => {
    const { setOpportunities } = useOmniStore.getState();
    setOpportunities([makeOpp("a"), makeOpp("b")]);
    setOpportunities([makeOpp("c")]);
    const ids = useOmniStore.getState().opportunities.map((o) => o.id);
    expect(ids).toEqual(["c", "a", "b"]);
  });
});

describe("omni-store pruneStale — CARDS-DEDUP-HOPS vigency clock (last_seen_at ?? detected_at)", () => {
  beforeEach(() => {
    useOmniStore.getState().clearOpportunities();
  });

  it("a route first detected hours ago but ratified seconds ago is ALIVE", () => {
    const NOW = Date.parse("2026-09-20T12:00:00Z");
    const { setOpportunities } = useOmniStore.getState();
    setOpportunities([
      makeOpp("alive", {
        ...ROUTE,
        detected_at: T1,
        first_seen_at: T1,
        last_seen_at: new Date(NOW - 1_000).toISOString(),
        confirmations: 40,
      }),
    ]);
    vi.setSystemTime(NOW);
    useOmniStore.getState().pruneStale(5 * 60_000);
    expect(useOmniStore.getState().opportunities).toHaveLength(1);
    vi.useRealTimers();
  });

  it("a route whose last ratification is older than the cutoff is evicted even if first_seen is null", () => {
    const NOW = Date.parse("2026-09-20T12:00:00Z");
    const { setOpportunities } = useOmniStore.getState();
    setOpportunities([
      makeOpp("dead", {
        ...ROUTE,
        detected_at: T1,
        last_seen_at: new Date(NOW - 10 * 60_000).toISOString(),
      }),
    ]);
    vi.setSystemTime(NOW);
    useOmniStore.getState().pruneStale(5 * 60_000);
    expect(useOmniStore.getState().opportunities).toHaveLength(0);
    vi.useRealTimers();
  });
});
