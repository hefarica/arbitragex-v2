/**
 * FE-0042 — HomeStoreAggregation (§58 §59)
 *
 * The home aggregation must derive EVERY axis from the Omni-Store via
 * selectors — zero fetching, zero second business logic. Repo pattern (same
 * as RegistryCoherenceStrip/FE-0040): the VIEW is pure and real-typed props,
 * so these tests render it with store-shaped fixtures — no mocks, no seams.
 * The container (the only selector-reading site) is pinned by its production
 * SSR truth: zustand v5's server snapshot reads getInitialState(), so the
 * server HTML is the honest blank skeleton and the client fills it after
 * hydration (R1).
 *
 * Coverage doctrine:
 *   full-data props  — every axis tallies the store's own vocabulary
 *   blank props      — honest empties (§59): "sin canales" / "sin snapshot" /
 *                      "feed vacío", NEVER a zero or invented value
 *   knob-conditional — absent tick keys render "—", never 0 (R8)
 *   gaps             — EV and risk axes render "no computado" + nivel-(b) note
 *   R1               — same props → byte-identical SSR output
 */

import { describe, it, expect } from "vitest";
import * as React from "react";
import { renderToStaticMarkup } from "react-dom/server";

import type {
  PairView,
  RouteDiscoveryTickSummary,
} from "@/lib/apex/schemas";
import type { OmniOpportunity } from "@/lib/store/types";
import type { RealtimeChannelState } from "@/lib/store/realtime-slices";
import {
  HomeStoreAggregation,
  HomeStoreAggregationContainer,
  tallyStatuses,
  usToMs,
  totalP95Ms,
  type HomeAggregationProps,
} from "../HomeStoreAggregation";

// ─── Fixtures (real store shapes — the same ones the slices hold) ───────────

function blankChannel(over: Partial<RealtimeChannelState> = {}): RealtimeChannelState {
  return { transport: "rest", status: "connecting", lastMessageAt: null, lastError: null, ...over };
}

function pair(over: Partial<PairView> = {}): PairView {
  return {
    chain_id: 1,
    token_a: { chain_id: 1, address: "0x" + "a".repeat(40), symbol: "WETH", decimals: 18 },
    token_b: { chain_id: 1, address: "0x" + "b".repeat(40), symbol: "USDC", decimals: 6 },
    pools: [],
    venue_count: 0,
    alpha_forward: null,
    alpha_reverse: null,
    dirty: false,
    last_reserve_update: null,
    ...over,
  };
}

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
    token_in: "0x" + "a".repeat(40),
    token_out: "0x" + "b".repeat(40),
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

const FULL_TICK: RouteDiscoveryTickSummary = {
  drain_seeded: 5,
  fe_prefilter_evaluated: 10,
  fe_prefilter_pass: 3,
  routes_dispatched: 2,
  strategy_status_counts: { active: 25, shadow: 3, paused: 1 },
  lat_stages: [
    {
      key: "lat.total",
      target_ms: 250,
      p50_us: 9000,
      p90_us: 12000,
      p95_us: 15870,
      p99_us: 20000,
      headroom_p95_us: 9130,
    },
  ],
  lat_pass_p95: true,
};

/** The store's honest blank state (what the slices initialize to). */
function blankProps(): HomeAggregationProps {
  return {
    channels: {
      routes: blankChannel(),
      runtime_ack: blankChannel(),
      pairs: blankChannel(),
      quote_anchor: blankChannel(),
    },
    tick: null,
    pairs: null,
    opps: [],
  };
}

function fullProps(): HomeAggregationProps {
  return {
    channels: {
      routes: blankChannel({ transport: "ws", status: "live" }),
      runtime_ack: blankChannel({ transport: "ws", status: "live" }),
      pairs: blankChannel({ status: "polling" }),
      quote_anchor: blankChannel({ status: "connecting" }),
    },
    tick: FULL_TICK,
    pairs: [pair({ dirty: true }), pair({ dirty: true }), pair()],
    opps: [
      makeOpp("1"),
      makeOpp("2"),
      makeOpp("3", { status: "validated" }),
      makeOpp("4", { status: null }),
    ],
  };
}

function renderAgg(props: HomeAggregationProps): string {
  return renderToStaticMarkup(React.createElement(HomeStoreAggregation, props));
}

// ─── Pure derivations ────────────────────────────────────────────────────────

describe("tallyStatuses — counting only (§58)", () => {
  it("counts and sorts by count desc, then alphabetical", () => {
    expect(tallyStatuses(["live", "polling", "live", "live", "polling"])).toEqual([
      ["live", 3],
      ["polling", 2],
    ]);
  });

  it("ties break alphabetically (deterministic render, R1)", () => {
    expect(tallyStatuses(["b", "a", "b", "a"])).toEqual([
      ["a", 2],
      ["b", 2],
    ]);
  });

  it("empty input → empty tally (renders 'sin canales')", () => {
    expect(tallyStatuses([])).toEqual([]);
  });
});

describe("usToMs / totalP95Ms — null stays honest (R8, §44)", () => {
  it("null → '—', never 0.00 ms", () => {
    expect(usToMs(null)).toBe("—");
    expect(usToMs(undefined)).toBe("—");
  });

  it("15870 µs → 15.87 ms", () => {
    expect(usToMs(15870)).toBe("15.87 ms");
  });

  it("totalP95Ms: null tick → '—'", () => {
    expect(totalP95Ms(null)).toBe("—");
  });

  it("totalP95Ms: row present but p95 not computed → '—'", () => {
    const tick = {
      lat_stages: [
        { key: "lat.total" as const, target_ms: 250, p50_us: null, p90_us: null, p95_us: null, p99_us: null, headroom_p95_us: null },
      ],
    };
    expect(totalP95Ms(tick as RouteDiscoveryTickSummary)).toBe("—");
  });

  it("totalP95Ms: no lat.total row (knob-conditional) → '—'", () => {
    expect(totalP95Ms({ lat_stages: [] } as RouteDiscoveryTickSummary)).toBe("—");
  });
});

// ─── Pure view — full-data props ────────────────────────────────────────────

describe("HomeStoreAggregation — full-data props", () => {
  it("posture: tallies the store's channel-status vocabulary verbatim", () => {
    const html = renderAgg(fullProps());
    expect(html).toContain("live ×2");
    expect(html).toContain("polling ×1");
    expect(html).toContain("connecting ×1");
  });

  it("funnel: renders every stage from the tick keys", () => {
    const html = renderAgg(fullProps());
    expect(html).toContain("seeds 5 → eval 10 → pass 3 → dispatch 2");
  });

  it("hot pairs: dirty tally over the pair snapshot", () => {
    const html = renderAgg(fullProps());
    expect(html).toContain("2 / 3");
    expect(html).toContain("dirty");
  });

  it("strategies: census slugs verbatim, sorted by count desc (§21)", () => {
    const html = renderAgg(fullProps());
    expect(html).toContain("active 25");
    expect(html).toContain("shadow 3");
    expect(html).toContain("paused 1");
    expect(html.indexOf("active 25")).toBeLessThan(html.indexOf("paused 1"));
  });

  it("p95: µs→ms from the lat.total row + PASS note", () => {
    const html = renderAgg(fullProps());
    expect(html).toContain("15.87 ms");
    expect(html).toContain("PASS p95 vs SLA");
  });

  it("ejecuciones: status tally incl. honest 'unknown' bucket for null (§28)", () => {
    const html = renderAgg(fullProps());
    expect(html).toContain("detected 2");
    expect(html).toContain("validated 1");
    expect(html).toContain("unknown 1");
  });

  it("gaps: exactly two 'no computado' axes (EV, risk) with nivel-(b) notes", () => {
    const html = renderAgg(fullProps());
    expect(html.split("no computado").length - 1).toBe(2);
    expect(html).toContain("sin slice de EV en el store (nivel-(b))");
    expect(html).toContain("sin slice de risk en el store (nivel-(b))");
  });

  it("R1: same props → byte-identical SSR output", () => {
    expect(renderAgg(fullProps())).toBe(renderAgg(fullProps()));
  });
});

// ─── Pure view — honest empties (§59) ────────────────────────────────────────

describe("HomeStoreAggregation — blank props", () => {
  it("renders an honest empty for every unfed slice, never a zero", () => {
    const html = renderAgg(blankProps());
    // All 4 channels exist in the store's blank state — each honestly
    // "connecting" (nothing accepted yet, realtime-slices R8 note). This is
    // the true blank posture, not "sin canales".
    expect(html).toContain("connecting ×4");
    expect(html).toContain("seeds — → eval — → pass — → dispatch —");
    expect(html).toContain("sin snapshot (R8)");
    expect(html).toContain("sin censo en tick");
    expect(html).toContain("sin ciclos completos (§44)");
    expect(html).toContain("feed vacío");
  });

  it("still declares the EV/risk gaps (structure is mode-invariant)", () => {
    const html = renderAgg(blankProps());
    expect(html).toContain("sin slice de EV en el store (nivel-(b))");
    expect(html).toContain("sin slice de risk en el store (nivel-(b))");
  });
});

// ─── Pure view — knob-conditional tick keys (R8) ────────────────────────────

describe("HomeStoreAggregation — absent tick keys render '—'", () => {
  it("absent keys render '—', the present key renders its value", () => {
    // Tick present but only one funnel key emitted — the rest are genuinely
    // absent on the wire (knob OFF emits NOTHING, telemetry.ts doc).
    const html = renderAgg({ ...blankProps(), tick: { drain_seeded: 7 } as RouteDiscoveryTickSummary });
    expect(html).toContain("seeds 7 → eval — → pass — → dispatch —");
  });

  it("lat_pass_p95=false renders 'over budget' (computed, not absent)", () => {
    const html = renderAgg({ ...blankProps(), tick: { ...FULL_TICK, lat_pass_p95: false } });
    expect(html).toContain("over budget");
    expect(html).not.toContain("sin ciclos completos");
  });

  it("lat_pass_p95 absent renders the §44 note (no fabricated PASS)", () => {
    const html = renderAgg({ ...blankProps(), tick: { drain_seeded: 7 } as RouteDiscoveryTickSummary });
    expect(html).toContain("sin ciclos completos (§44)");
  });
});

// ─── Container — production SSR truth ────────────────────────────────────────

describe("HomeStoreAggregationContainer — production SSR", () => {
  it("renders the honest blank skeleton on the server (zustand getInitialState snapshot, R1)", () => {
    // zustand v5's getServerSnapshot reads getInitialState() — the pristine
    // creation state. The real server HTML for this island is therefore the
    // blank skeleton; the client fills it after hydration. This pins that
    // production behavior (deterministic server output, no Date.now, no
    // fabricated "live" states server-side).
    const html = renderToStaticMarkup(
      React.createElement(HomeStoreAggregationContainer),
    );
    expect(html).toContain("connecting ×4");
    expect(html).toContain("seeds — → eval — → pass — → dispatch —");
    expect(html).toContain("sin snapshot (R8)");
    expect(html).toContain("feed vacío");
    expect(html).not.toContain("live ×");
  });
});
