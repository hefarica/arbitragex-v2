// frontend/app/opportunities/__tests__/FirstPaint.test.tsx
//
// SSR-FIRSTPAINT-01 — the /opportunities grid must render the SERVER snapshot on
// the first paint. Production evidence that it did not (2026-09-26):
//   · 3 consecutive `GET http://<frontend>:5173/opportunities` → HTML with
//     `data-opp-id` × 0 and the "0 matching" counter;
//   · `Cache-Control: private, no-cache, no-store` on those responses (a live
//     dynamic render, not a stale prerender);
//   · the api-server log shows the page's OWN snapshot query answering 200
//     (`GET /api/v1/opportunities/live?order=profit_usd`) at the same instants;
//   · the edge URL returns 38 items from inside the frontend container.
// Cause: `useOmniOpportunities` seeds the store from `initialOpportunities`
// inside a useEffect (effects never run during SSR), and the grid read the
// store directly — so the fetched snapshot, with every computed figure on it,
// was thrown away for the first paint.
//
// R1: this is a pure static render (no effects, no clock, no RNG) — the server
// and the browser's first render must produce the same markup.
import React from "react";
import { describe, it, expect, vi } from "vitest";
import { renderToStaticMarkup } from "react-dom/server";

vi.mock("@/components/ui/skeleton", () => ({
  Skeleton: (props: React.ComponentProps<"div">) =>
    React.createElement("div", { "data-slot": "skeleton", ...props }),
}));
// The WS/socket bridge is a client-only EFFECT seam (socket.io, timers). The
// first paint must not depend on it — that is the whole point of the gate.
vi.mock("@/lib/store/useOmniOpportunities", () => ({
  useOmniOpportunities: () => undefined,
}));
// Live price ribbon: client-only fetch loop, not part of the card grid.
vi.mock("@/components/opportunities/exchange/PriceTicker", () => ({
  PriceTicker: () => null,
}));
// The detail Sheet is a separate surface (its own classic-JSX SSR hazard, cf.
// the ui/skeleton passthrough above) and is not part of the grid first paint.
vi.mock("@/components/OpportunityDetailDialog", () => ({
  OpportunityDetailDialog: () => null,
}));
// Audit-trail section: not the card grid; keeps the gate focused on the grid.
vi.mock("@/components/opportunities/QuarantinedEventsAuditTrail", () => ({
  QuarantinedEventsAuditTrail: () => null,
}));

import OpportunitiesClient from "../OpportunitiesClient";
import { mapToOmniOpportunity } from "@/lib/store/types";

const WETH = "0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2";
const USDT = "0xdac17f958d2ee523a2206206994597c13d831ec7";

// The real live row's shape (same fixture family as CardsLayout.test.tsx).
const liveRow = mapToOmniOpportunity({
  id: "opp-firstpaint-1",
  chain_id: 1,
  chain_base_token_symbol: "WETH",
  strategy_kind: "dex_arb",
  detected_at: "2026-09-26T18:15:12.905Z",
  status: "rejected",
  rejection_reason: "non_positive_profit",
  trace_id: "trace-fp-1",
  dex_a: "UniswapV2",
  dex_b: "SushiSwap",
  token_in: WETH,
  token_out: USDT,
  token_in_info: { symbol: "WETH", decimals: 18, logo_url: null, resolved_via: "onchain_full" },
  token_out_info: { symbol: "USDT", decimals: 6, logo_url: null, resolved_via: "onchain_full" },
  pipeline_latency_ms: 24,
  expected_profit_usd: 23.04257007,
  net_expected_profit_usd: -0.00001,
  // REAL-LIVE-CARDS-SSOT-01: una fila es CARD sólo con `economics.computed` y
  // aritmética cerrada (net = gross − cost, roi = net/amount_in×100). Este
  // payload es el que la convierte en card real; sin él es diagnóstico.
  economics: {
    computation_status: "computed",
    error_reason: null,
    amount_in_wei: "1000000000000000000",
    amount_out_wei: "1008314000000000000",
    amount_in_usd: 2687.079437602,
    amount_out_usd: 2710.121007672,
    gross_profit_usd: 23.04257007,
    gas_usd: 0.18,
    dex_fees_usd: null,
    flash_fee_usd: 22.85258007,
    bribe_usd: 0,
    slippage_usd: null,
    other_costs_usd: 0.01,
    total_cost_usd: 23.04258007,
    net_profit_usd: -0.00001,
    roi_pct: -3.7216e-7,
    target_net_usd: 50,
    target_delta_usd: -50.00001,
    meets_target: false,
    quote_block: 26068721,
    simulation_block: null,
    legs: [],
    not_computed_reasons: {},
  },
  simulated_net_profit_usd: -15.304361738499997,
  simulated_amount_in_usd: 2687.2794,
  simulated_target: {
    target_net_usd: 50,
    target_roi_pct: 0.3,
    target_source: "simulation_tab",
    binding_floor: "net-per-usd-nonpositive",
    required_amount_in_usd: null,
    cap_amount_in_usd: 1000,
    suggested_amount_in_usd: 0,
    suggested_net_usd: -15.304361738499997,
    suggested_roi_pct: 0,
    meets_target_at_cap: false,
    estimation_basis: "observed-gross",
    notes: ["net-per-usd-nonpositive"],
  },
  token_prices_usd: { WETH: 2687.079437602, USDT: 0.99979512 },
  route_metadata: {
    decimals: { map: { [WETH]: 18, [USDT]: 6 } },
    dex_adapters: ["UniswapV2", "SushiSwap"],
    pool_addresses: ["0xpool1", "0xpool2"],
    token_addresses: [WETH, USDT, WETH],
  },
});

const render = (opportunities: ReturnType<typeof mapToOmniOpportunity>[]) =>
  renderToStaticMarkup(
    React.createElement(OpportunitiesClient, {
      initialSnapshot: {
        opportunities,
        serverTime: null,
        source: "server-snapshot",
      },
    }),
  );

describe("SSR-FIRSTPAINT-01 — the server snapshot reaches the first paint", () => {
  it("renders the snapshot's cards (and their computed figures) with an empty store", () => {
    const html = render([liveRow]);
    // the card is IN the server markup — not deferred to a mount effect
    expect(html).toContain('data-opp-id="opp-firstpaint-1"');
    expect(html).toContain("Capital path (USD)");
    expect(html).toContain("Applied strategy config");
    // …carrying the computed values, not placeholders
    expect(html).toContain("$23.04"); // gross out
    expect(html).toContain("$50.00"); // min net USD
    expect(html).toContain("24ms"); // pipeline latency
    // one row per hop, on the first paint
    expect((html.match(/Hop 1\/2/g) ?? []).length).toBe(1);
    expect((html.match(/Hop 2\/2/g) ?? []).length).toBe(1);
  });

  it("an empty snapshot renders the honest empty grid (never a fabricated card)", () => {
    const html = render([]);
    expect(html).not.toContain("data-opp-id");
    // the counter renders "<span …>0</span> matching" (count in its own span)
    expect(html).toContain(">0</span> matching");
  });

  it("R1: the pure render is byte-identical across invocations", () => {
    expect(render([liveRow])).toBe(render([liveRow]));
  });
});

// SHOW-REJECTED-01 — operator order 2026-09-27 (verbatim): "agregar un toggle
// 'Mostrar rechazadas'". The control must be ON the page, OFF by default, and
// the default paint must stay exactly the real/live card set (the diagnostics
// are counted — never silently dropped, never painted with a substituted
// figure until the operator asks for them).
describe("SHOW-REJECTED-01 — toggle 'Mostrar rechazadas'", () => {
  const errorRow = mapToOmniOpportunity({
    id: "opp-diagnostic-1",
    chain_id: 1,
    chain_base_token_symbol: "WETH",
    strategy_kind: "dex_arb",
    detected_at: "2026-09-26T18:16:00.000Z",
    status: "rejected",
    rejection_reason: "v3_quote_unavailable",
    economics: { computation_status: "error", error_reason: "v3_quote_unavailable" },
  });

  it("renders the filter-bar control and the banner action, OFF by default", () => {
    const html = render([liveRow, errorRow]);
    expect(html).toContain('data-testid="toggle-show-rejected"');
    expect(html).toContain("Mostrar rechazadas");
    expect(html).toContain('data-testid="banner-toggle-show-rejected"');
    expect(html).toMatch(
      /data-testid="toggle-show-rejected"[^>]{0,240}aria-pressed="false"/,
    );
  });

  it("default OFF: paints the real/live card and COUNTS the diagnostic (no silent drop)", () => {
    const html = render([liveRow, errorRow]);
    expect(html).toContain('data-opp-id="opp-firstpaint-1"');
    expect(html).not.toContain('data-opp-id="opp-diagnostic-1"');
    expect(html).toContain("1 detecciones no se presentan como cards económicas");
    expect(html).toContain("no se rellenan con supuestos");
  });
});
