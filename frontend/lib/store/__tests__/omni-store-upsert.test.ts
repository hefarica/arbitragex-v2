// frontend/lib/store/__tests__/omni-store-upsert.test.ts
//
// Regression tests for the Binance-style streaming upsert (operator directive
// 2026-08-18). The old addOpportunity guard silently DISCARDED pushes for ids
// already in the list — emitted cards stayed frozen (no live profit/status
// updates) until the next full poll. Migration 107 + this upsert make a row
// UPDATE flow through the WS push path and replace the card IN PLACE.
//
// R1-safe: the store is a client module; tests exercise actions directly.
import { describe, it, expect, beforeEach } from "vitest";
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

describe("omni-store addOpportunity — streaming upsert", () => {
  beforeEach(() => {
    useOmniStore.getState().clearOpportunities();
  });

  it("NEW id prepends (card enters at the top)", () => {
    const { addOpportunity } = useOmniStore.getState();
    addOpportunity(makeOpp("opp-1"));
    addOpportunity(makeOpp("opp-2"));
    const list = useOmniStore.getState().opportunities;
    expect(list.map((o) => o.id)).toEqual(["opp-2", "opp-1"]);
  });

  it("EXISTING id replaces IN PLACE (position preserved) — the frozen-card regression", () => {
    const { addOpportunity } = useOmniStore.getState();
    addOpportunity(makeOpp("opp-1"));
    addOpportunity(makeOpp("opp-2"));
    // Row update push: economics computed after acceptance.
    addOpportunity(
      makeOpp("opp-1", { expected_profit_usd: 12.5, net_expected_profit_usd: 4.2, status: "scored" }),
    );
    const list = useOmniStore.getState().opportunities;
    // Position unchanged (Binance-style row update, no jump to top)…
    expect(list.map((o) => o.id)).toEqual(["opp-2", "opp-1"]);
    // …but the values are the fresh row.
    const updated = list.find((o) => o.id === "opp-1")!;
    expect(updated.expected_profit_usd).toBe(12.5);
    expect(updated.net_expected_profit_usd).toBe(4.2);
    expect(updated.status).toBe("scored");
  });

  it("execution-time values PREVAIL over earlier ones (last write wins on push)", () => {
    const { addOpportunity } = useOmniStore.getState();
    addOpportunity(makeOpp("opp-1", { net_expected_profit_usd: 4.2 }));
    addOpportunity(makeOpp("opp-1", { net_expected_profit_usd: 9.9, paper_status: "paper_viable" }));
    const updated = useOmniStore.getState().opportunities.find((o) => o.id === "opp-1")!;
    expect(updated.net_expected_profit_usd).toBe(9.9);
    expect(updated.paper_status).toBe("paper_viable");
  });

  it("identical object reference is a no-op (no store churn on replay)", () => {
    const { addOpportunity } = useOmniStore.getState();
    const opp = makeOpp("opp-1");
    addOpportunity(opp);
    const before = useOmniStore.getState();
    addOpportunity(opp); // same ref — reconnect replay
    const after = useOmniStore.getState();
    expect(after.opportunities).toBe(before.opportunities);
  });
});
