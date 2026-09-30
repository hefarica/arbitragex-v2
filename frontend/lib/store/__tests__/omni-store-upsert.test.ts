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
    // CARDS-DEDUP-HOPS: the store merges by route group key — every fixture
    // id gets its OWN route so these upsert tests stay about ids, not groups
    // (grouping semantics live in omni-store-grouping.test.ts).
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

// ── ECON-DECLARE-01 (2026-09-27) ─────────────────────────────────────────────
// The notional-basis declaration rides `route_metadata` (its own JSONB column,
// REST-only: `publisher::publish` serializes the `Opportunity` row and never the
// topology). A raw WS row therefore maps `route_metadata` to null, and the merge
// used to spread that null over the snapshot — wiping the topology, `hop_count`,
// the per-hop ledger AND the declaration. A card would show a declared notional
// and then lose it mid-session, which is worse than never showing it.
describe("ECON-DECLARE-01 — the declaration survives a WS redetection", () => {
  const DECLARED_RM = {
    token_addresses: ["0xroute-a", "0xbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb", "0xroute-a"],
    pool_addresses: ["0xpool1", "0xpool2"],
    dex_adapters: ["uniswap_v2", "sushiswap"],
    economics_amount_in_wei: "1000000000000000000",
    economics_basis: { gross: "probe", net: "kernel", amount: "intent" },
  };

  it("a snapshot row's route_metadata + declaration are NOT wiped by a raw WS row", () => {
    const { addOpportunity } = useOmniStore.getState();
    // 1) the REST snapshot row: full topology + the producer's declaration.
    addOpportunity(
      makeOpp("opp-1", {
        token_in: "0xroute-a",
        dex_a: "uniswap_v2",
        dex_b: "sushiswap",
        route_metadata: DECLARED_RM as never,
        expected_profit_usd: 12.5,
        first_seen_at: "2026-08-18T00:00:00Z",
        confirmations: 1,
      } as Partial<OmniOpportunity>),
    );
    // 2) a raw WS redetection of the SAME route group — no route_metadata, the
    //    way the wire actually delivers it.
    addOpportunity(
      makeOpp("opp-1", {
        token_in: "0xroute-a",
        dex_a: "uniswap_v2",
        dex_b: "sushiswap",
        route_metadata: null,
        expected_profit_usd: 13.9,
      } as Partial<OmniOpportunity>),
    );

    const row = useOmniStore.getState().opportunities.find((o) => o.id === "opp-1")!;
    // the live-updated figure wins…
    expect(row.expected_profit_usd).toBe(13.9);
    // …and the declaration is still there (this is the regression).
    expect(row.route_metadata).not.toBeNull();
    expect(row.route_metadata?.economics_amount_in_wei).toBe("1000000000000000000");
    expect(row.route_metadata?.economics_basis?.net).toBe("kernel");
    // the topology fields ride along, so hop_count and the ledger stay intact.
    expect(row.route_metadata?.dex_adapters).toEqual(["uniswap_v2", "sushiswap"]);
  });

  it("a snapshot WITH route_metadata still wins over a row that carries none", () => {
    const { addOpportunity } = useOmniStore.getState();
    addOpportunity(
      makeOpp("opp-9", { token_in: "0xroute-z", route_metadata: null } as Partial<OmniOpportunity>),
    );
    addOpportunity(
      makeOpp("opp-9", {
        token_in: "0xroute-z",
        route_metadata: DECLARED_RM as never,
        confirmations: 2,
      } as Partial<OmniOpportunity>),
    );
    const row = useOmniStore.getState().opportunities.find((o) => o.id === "opp-9")!;
    expect(row.route_metadata?.economics_amount_in_wei).toBe("1000000000000000000");
  });
});
