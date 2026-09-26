// frontend/lib/store/__tests__/omni-store-enrich-preserve.test.ts
//
// ENRICH-PRESERVE-01 regression (operator report 2026-09-26): raw WS
// redetections are plain PG rows and must NOT wipe the REST-snapshot
// enrichment when merged into the same route group. Operator symptoms on the
// live dashboard: token symbol letters vanished after the first refresh
// (icon-only chips) and the "Applied Strategy Config" block went blank — both
// were prev's enriched values being overwritten by the raw push.
//
// R1-safe: the store is a client module; tests exercise actions directly.
import { describe, it, expect, beforeEach } from "vitest";
import { useOmniStore } from "@/lib/store/omni-store";
import type { OmniOpportunity } from "@/lib/store/types";

const ROUTE_IN = "0xroute-enrich-preserve";
const ROUTE_OUT = "0xbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";

function makeOpp(id: string, over: Partial<OmniOpportunity> = {}): OmniOpportunity {
  return {
    id,
    chain_id: 1,
    strategy_kind: "dex_arb",
    detected_at: "2026-09-26T00:00:00Z",
    trace_id: `trace-${id}`,
    dex_a: "uniswap_v3",
    dex_b: null,
    pair_symbol: "WETH/USDT",
    // Same route (token_in/out) for both fixtures → same route group key, so
    // the second push takes the merge path under test.
    token_in: ROUTE_IN,
    token_out: ROUTE_OUT,
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

/** REST snapshot row: server aggregates + full enrichment (SSOT). */
function enrichedSnapshot(): OmniOpportunity {
  return makeOpp("det-snapshot", {
    first_seen_at: "2026-09-26T00:00:00Z",
    confirmations: 3,
    detected_at: "2026-09-26T00:00:00Z",
    token_in_info: {
      symbol: "WETH",
      decimals: 18,
      logo_url: "https://logo/weth.png",
      resolved_via: "onchain_full",
    },
    token_out_info: {
      symbol: "USDT",
      decimals: 6,
      logo_url: null,
      resolved_via: "onchain_full",
    },
    chain_base_token_symbol: "WETH",
    leg_symbols: { [ROUTE_OUT.toLowerCase()]: "USDT" },
    token_prices_usd: { WETH: 2685.78, USDT: 0.9963 },
    // CARDS-PRECEDENCE-01: the three fields the FIRST fix missed. All three are
    // REST-only derivations (`backend/api-server/src/websocket.ts:506-514` names
    // them as absent from the WS wire) and were proven to be wiped on live data.
    paper_status: "paper_rejected",
    chains_used: [1],
    dexes_used: ["sushiswap", "uniswapv2"],
    raw_simulated_net_profit_usd: "-0.5",
    simulated_net_profit_usd: -0.5,
    simulated_amount_in_usd: 1000,
    simulated_roi_pct: -0.0005,
    simulated_at: "2026-09-26T00:00:05Z",
    simulated_notes: ["net-per-usd-nonpositive"],
  } as Partial<OmniOpportunity>);
}

/** Raw WS redetection: newer detection of the SAME route, zero enrichment. */
function rawWsRedetection(): OmniOpportunity {
  return makeOpp("det-ws", {
    detected_at: "2026-09-26T00:02:00Z",
    first_seen_at: null,
    confirmations: null,
    token_in_info: null,
    token_out_info: null,
    chain_base_token_symbol: null,
    leg_symbols: null,
    token_prices_usd: null,
    // The raw transport has no column for these — the mapper's defaults are
    // `null`, `[]`, `[]`. `[]` is a mapper artifact, never a computed value.
    paper_status: null,
    chains_used: [],
    dexes_used: [],
    raw_simulated_net_profit_usd: null,
    simulated_net_profit_usd: null,
    simulated_amount_in_usd: null,
    simulated_roi_pct: null,
    simulated_at: null,
    simulated_notes: null,
  });
}

describe("ENRICH-PRESERVE-01 — raw WS rows never wipe snapshot enrichment", () => {
  beforeEach(() => {
    useOmniStore.getState().clearOpportunities();
  });

  it("preserves token metadata, prices, SIM ladder and base token across a raw push", () => {
    useOmniStore.getState().setOpportunities([enrichedSnapshot()]);
    useOmniStore.getState().setOpportunities([rawWsRedetection()]);

    const opps = useOmniStore.getState().opportunities;
    expect(opps).toHaveLength(1);
    const row = opps[0]!;

    // The raw push still wins the row identity/timestamp (live update flows)…
    expect(row.id).toBe("det-ws");
    expect(row.detected_at).toBe("2026-09-26T00:02:00Z");

    // …while every REST-only enrichment field survives.
    expect(row.token_in_info?.symbol).toBe("WETH");
    expect(row.token_in_info?.logo_url).toBe("https://logo/weth.png");
    expect(row.token_out_info?.symbol).toBe("USDT");
    expect(row.chain_base_token_symbol).toBe("WETH");
    expect(row.token_prices_usd).toEqual({ WETH: 2685.78, USDT: 0.9963 });
    expect(row.raw_simulated_net_profit_usd).toBe("-0.5");
    expect(row.simulated_net_profit_usd).toBe(-0.5);
    expect(row.simulated_amount_in_usd).toBe(1000);
    expect(row.simulated_roi_pct).toBe(-0.0005);
    expect(row.simulated_at).toBe("2026-09-26T00:00:05Z");
  });

  it("a fresh snapshot row still overrides everything (server SSOT wins)", () => {
    useOmniStore.getState().setOpportunities([enrichedSnapshot()]);
    useOmniStore.getState().setOpportunities([rawWsRedetection()]);
    // New snapshot with the enrichment explicitly cleared → must clear.
    const cleared = { ...enrichedSnapshot(), token_prices_usd: null, simulated_net_profit_usd: null };
    useOmniStore.getState().setOpportunities([cleared as OmniOpportunity]);

    const row = useOmniStore.getState().opportunities[0]!;
    expect(row.token_prices_usd).toBeNull();
    expect(row.simulated_net_profit_usd).toBeNull();
    // Symbol metadata still present — the snapshot carried it.
    expect(row.token_in_info?.symbol).toBe("WETH");
  });

  // CARDS-PRECEDENCE-01 (2026-09-26). Reproduces the EXACT live-data diff
  // observed on route group `1||dex_arb|0xc02aaa…|0xdac17f…|PancakeSwap V3|
  // PancakeSwap V3` between the enriched snapshot row and the raw PG row:
  //   paper_status "paper_rejected" → null
  //   chains_used  [1]              → []
  //   dexes_used   ["pancakeswap v3"] → []
  // All three are documented as WS-absent by the producer itself
  // (`backend/api-server/src/websocket.ts:506-514`) and were the gap left by
  // the first, hardcoded preserve list.
  it("CARDS-PRECEDENCE-01: paper_status / chains_used / dexes_used survive a raw push", () => {
    useOmniStore.getState().setOpportunities([enrichedSnapshot()]);
    useOmniStore.getState().setOpportunities([rawWsRedetection()]);

    const row = useOmniStore.getState().opportunities[0]!;
    expect(row.paper_status).toBe("paper_rejected");
    expect(row.chains_used).toEqual([1]);
    expect(row.dexes_used).toEqual(["sushiswap", "uniswapv2"]);
    // The raw push still wins what it actually carries (rule 2 of the contract).
    expect(row.id).toBe("det-ws");
    expect(row.detected_at).toBe("2026-09-26T00:02:00Z");
  });

  it("CARDS-PRECEDENCE-01: rule 3 (None ≠ Some(0)) — a REAL empty dexes_used is not overwritten by prev either", () => {
    // Guard against over-preserving: when the SNAPSHOT itself carries `[]`
    // (a genuinely computed "no dexes recorded"), a later raw push must not
    // resurrect a stale non-empty list out of nowhere. Prev is `[]` here, so
    // the merge keeps `[]` — never a fabrication in either direction.
    useOmniStore
      .getState()
      .setOpportunities([{ ...enrichedSnapshot(), chains_used: [], dexes_used: [] } as OmniOpportunity]);
    useOmniStore.getState().setOpportunities([rawWsRedetection()]);

    const row = useOmniStore.getState().opportunities[0]!;
    expect(row.chains_used).toEqual([]);
    expect(row.dexes_used).toEqual([]);
  });

  it("CARDS-PRECEDENCE-01: simulated_notes emptiness is preserved as-is (computed 'nothing to report')", () => {
    // `simulated_notes: []` is a COMPUTED empty (OpportunityDetailTabs.tsx
    // comments it as "nothing to report"), so it must NOT be treated as absent.
    useOmniStore
      .getState()
      .setOpportunities([{ ...enrichedSnapshot(), simulated_notes: [] } as OmniOpportunity]);
    useOmniStore.getState().setOpportunities([rawWsRedetection()]);

    const row = useOmniStore.getState().opportunities[0]!;
    expect(row.simulated_notes).toEqual([]);
  });
});
