// frontend/components/__tests__/CardsLayout.test.tsx
//
// CARDS-LAYOUT-01 / CARDS-LAYOUT-02 / CARDS-PRECEDENCE-02 — SSR gates for the
// operator's 2026-09-26 card report: "something is still overlapping in the
// cards or somewhere; the pipeline breaks and the calculations do not arrive
// … everything that is computed must take PRECEDENCE to be displayed."
//
// The fixtures mirror the REAL production row fetched while diagnosing
// (GET /api/opportunities/live, api-server/edge on the VPS):
//   WETH→USDT UniswapV2 / USDT→WETH SushiSwap · status rejected ·
//   rejection_reason non_positive_profit · gross 23.04257007 ·
//   net_expected_profit_usd -0.00001 · simulated_net_profit_usd -15.3043617 ·
//   simulated_amount_in_usd 2687.2794 · pipeline_latency_ms 24 ·
//   simulated_target {target_net_usd 50, target_roi_pct 0.3,
//   binding_floor "net-per-usd-nonpositive", suggested_amount_in_usd 0}.
//
// What is pinned here (each gate fails on the pre-fix markup):
//   1. ANTI-DOUBLING — N hops render exactly N rows and the same `Hop i/N`
//      label never appears twice in the card markup.
//   2. NO OVERLAP — the ledger row is a two-track grid whose label and hint
//      both truncate, and a rich per-hop value cell is its OWN full-width line
//      (never squeezed onto the header line where it collided with the label).
//   3. ONE CLEAN PAIR PER LINE — the "Applied strategy config" block is not the
//      2-column grid whose `minmax(0,1fr)` tracks were narrower than their
//      content (the measured production overlap: `net-per-usd-nonpositive`
//      wrapped over three lines into the neighbouring column).
//   4. PRECEDENCE — a computed value wins its cell over the quiet placeholder,
//      and the simulated source is marked `~` exactly like the card headline.
//   5. QUIET EMPTINESS — no `no computado` / `no emitido` occupies a value slot
//      in the operator-facing card; the R8 reason stays reachable in `title`.
//   6. R1 — the pure render is byte-identical (no clock, no RNG in render).
import React from "react";
import { describe, it, expect, vi } from "vitest";
import { renderToStaticMarkup } from "react-dom/server";

// Same classic-JSX SSR passthrough the card's other gates use (ui/skeleton's
// type-only React import breaks vitest's classic-JSX path).
vi.mock("@/components/ui/skeleton", () => ({
  Skeleton: (props: React.ComponentProps<"div">) =>
    React.createElement("div", { "data-slot": "skeleton", ...props }),
}));

import { OpportunityTradeCard } from "../OpportunityTradeCard";
import { mapToOmniOpportunity } from "@/lib/store/types";

const WETH = "0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2";
const USDT = "0xdac17f958d2ee523a2206206994597c13d831ec7";
const wethInfo = {
  symbol: "WETH",
  decimals: 18,
  logo_url: null,
  resolved_via: "onchain_full",
};
const usdtInfo = {
  symbol: "USDT",
  decimals: 6,
  logo_url: null,
  resolved_via: "onchain_full",
};

/** The live row's `simulated_target` (inverse-sizing result) verbatim. */
const SIM_TARGET = {
  target_net_usd: 50,
  target_roi_pct: 0.3,
  target_source: "simulation_tab",
  binding_floor: "net-per-usd-nonpositive",
  required_amount_in_usd: null, // Infinity on the wire → null (CARDS-CRASH-01)
  cap_amount_in_usd: 1000,
  suggested_amount_in_usd: 0,
  suggested_net_usd: -15.304361738499997,
  suggested_roi_pct: 0,
  meets_target_at_cap: false,
  estimation_basis: "observed-gross",
  notes: ["linear-extrap", "net-per-usd-nonpositive"],
};

const ROUTE_2 = {
  decimals: { map: { [WETH]: 18, [USDT]: 6 } },
  dex_adapters: ["UniswapV2", "SushiSwap"],
  pool_addresses: [
    "0x0d4a11d5eeaac28ec3f61d100daf4d40471f1852",
    "0x06da0fd433c1a5d7a4faa01111c044910a184553",
  ],
  token_addresses: [WETH, USDT, WETH],
};

const wire = (over: Record<string, unknown>) => ({
  id: "opp-live-1",
  chain_id: 1,
  chain_base_token_symbol: "WETH",
  strategy_kind: "dex_arb",
  detected_at: "2026-09-26T18:15:12.905Z",
  status: "rejected",
  rejection_reason: "non_positive_profit",
  trace_id: "trace-live-1",
  dex_a: "UniswapV2",
  dex_b: "SushiSwap",
  pair_symbol: "WETH/USDT",
  token_in: WETH,
  token_out: USDT,
  token_in_info: wethInfo,
  token_out_info: usdtInfo,
  block_number: 26063352,
  pipeline_latency_ms: 24,
  expected_profit_usd: 23.04257007,
  net_expected_profit_usd: -0.00001,
  simulated_net_profit_usd: -15.304361738499997,
  simulated_amount_in_usd: 2687.2794,
  simulated_cost_breakdown: {
    gas_usd: 0.67181985,
    lp_fees_usd: 8.0618382,
    slippage_usd: 13.436397,
    failure_buffer_usd: 1.07491176,
    copied_buffer_usd: 11.521285035,
    capital_cost_usd: 0,
    ops_overhead_usd: 0.01,
    flashloan_fee_usd: 2.4185514599999998,
    relay_fee_usd: 1.1521285035,
  },
  simulated_target: SIM_TARGET,
  token_prices_usd: { WETH: 2687.079437602, USDT: 0.99979512 },
  route_metadata: ROUTE_2,
  ...over,
});

function card(opp: ReturnType<typeof mapToOmniOpportunity>): string {
  return renderToStaticMarkup(
    React.createElement(OpportunityTradeCard, {
      opp,
      now: 0,
      isMounted: false, // R1: SSR path — no time-dependent text
      simLoading: false,
      onExecute: () => {},
      onInspect: () => {},
    }),
  );
}

const count = (html: string, needle: string): number =>
  (html.match(new RegExp(needle.replace(/[[\]()/\\^$*+?.|{}-]/g, "\\$&"), "g")) ?? [])
    .length;

/** The rendered "Applied strategy config" block, sliced out of the markup. */
function configBlock(html: string): string {
  const start = html.indexOf("Applied strategy config");
  // the next block boundary in the card: the shadow-execute button label
  const end = html.indexOf("Execute (shadow");
  expect(start).toBeGreaterThan(-1);
  expect(end).toBeGreaterThan(start);
  return html.slice(start, end);
}

/** The summary grid's `Net` cell only (between the Net and bps labels). */
function netCell(html: string): string {
  const start = html.indexOf(">Net<");
  const end = html.indexOf(">bps<");
  expect(start).toBeGreaterThan(-1);
  expect(end).toBeGreaterThan(start);
  return html.slice(start, end);
}

/**
 * ECON-DECLARE-01: the VALUE a summary cell actually renders.
 *
 * The cell now carries its notional basis beside the value
 * (`<span data-testid="opp-cell-bps">12</span><span data-basis=…>@undeclared</span>`),
 * so a raw `>12</div>` substring no longer identifies the value. Reading the
 * testid span is equivalent and narrower — it asserts the value AND leaves the
 * basis markup free to evolve.
 */
function cellValue(html: string, label: string): string {
  const m = new RegExp(`data-testid="opp-cell-${label}">([^<]*)<`).exec(html);
  expect(m, `cell ${label} must be rendered`).not.toBeNull();
  return m![1]!;
}

describe("CARDS-LAYOUT-01 — every hop renders exactly once, never doubled", () => {
  it("a 2-hop live route renders exactly one row per hop (the operator's doubling)", () => {
    const html = card(mapToOmniOpportunity(wire({})));
    // the label appears ONCE per hop — a second occurrence is the reported bug
    expect(count(html, "Hop 1/2")).toBe(1);
    expect(count(html, "Hop 2/2")).toBe(1);
    // …and so does each hop's row node
    expect(count(html, 'data-testid="ledger-hop-1"')).toBe(1);
    expect(count(html, 'data-testid="ledger-hop-2"')).toBe(1);
    expect(count(html, 'data-testid="ledger-hop-3"')).toBe(0);
  });

  it("a 7-hop route (operator max) renders 7 hop labels, each exactly once", () => {
    const tokens = [1, 2, 3, 4, 5, 6, 7].map((i) => `0x${String(i).repeat(40)}`);
    const html = card(
      mapToOmniOpportunity(
        wire({
          token_in: tokens[0],
          token_out: tokens[6],
          route_metadata: {
            dex_adapters: Array.from({ length: 7 }, () => "UniswapV2"),
            token_addresses: [...tokens, tokens[0]],
            pool_addresses: Array.from({ length: 7 }, (_, i) => `0xpool${i}`),
          },
        }),
      ),
    );
    for (let i = 1; i <= 7; i++) {
      expect(count(html, `Hop ${i}/7`)).toBe(1);
      expect(count(html, `data-testid="ledger-hop-${i}"`)).toBe(1);
    }
  });

  it("every ledger row's header is the two-track grid with truncating label + capped hint", () => {
    const html = card(mapToOmniOpportunity(wire({})));
    // the header track is `minmax(0,1fr) auto`: content can never overflow it
    expect(count(html, "grid-cols-[minmax(0,1fr)_auto]")).toBeGreaterThanOrEqual(15);
    // label truncates (min-w-0 + flex-1 + truncate) and the `(dex)` hint is capped
    expect(html).toContain("min-w-0 flex-1 truncate");
    expect(html).toContain("max-w-[45%] shrink-0 truncate");
  });

  it("a sized hop puts its exact wei/Δ cell on its OWN line, not on the header line", () => {
    const sized = mapToOmniOpportunity(
      wire({
        route_metadata: {
          ...ROUTE_2,
          leg_amounts_in: ["1000000000000000000", "2700000000"],
          leg_amounts_out: ["2700000000000", "1002000000000000000"],
          leg_zero_for_one: [true, false],
        },
      }),
    );
    const html = card(sized);
    // REAL-LIVE-CARDS-SSOT-01: cada hop con ledger propio pinta su bloque de
    // importes, y LP fees / slippage declaran su procedencia real ('incl. en
    // quote' / 'incl. en curva') en vez de un cero falso ⇒ 2 hops + 2 avisos.
    expect(count(html, 'data-testid="ledger-row-amounts"')).toBe(4);
    // …carrying the exact ledger figures, still one row per hop
    expect(html).toContain("1 WETH");
    expect(html).toContain("2,700 USDT");
    expect(count(html, "Hop 1/2")).toBe(1);
    expect(count(html, "Hop 2/2")).toBe(1);
  });

  it("sin ledger por-hop, la fila sigue muda salvo la procedencia declarada", () => {
    const html = card(mapToOmniOpportunity(wire({})));
    // Los unicos bloques de importes son los avisos de procedencia del fixset
    // (LP fees 'incl. en quote', slippage 'incl. en curva'): nunca una cifra
    // inventada ni un cero por ausencia.
    expect(count(html, 'data-testid="ledger-row-amounts"')).toBe(2);
    expect(html).toContain('incl. en quote');
    expect(html).toContain('incl. en curva');
    // the hop row still renders (topology) with its muted value cell
    expect(count(html, "Hop 1/2")).toBe(1);
  });
});

describe("CARDS-LAYOUT-02 — Applied strategy config is one clean pair per line", () => {
  const html = card(mapToOmniOpportunity(wire({})));
  const block = configBlock(html);

  it("renders the 4 pairs of the live target, each on its own row", () => {
    expect(count(block, 'data-testid="config-pair"')).toBe(4);
    for (const label of ["min net USD", "min ROI %", "binding floor", "suggested borrow"]) {
      expect(block).toContain(label);
    }
    // the values are the computed ones, not placeholders
    expect(block).toContain("$50.00");
    expect(block).toContain("0.30%");
    expect(block).toContain("$0.0000");
  });

  it("is NOT the overlapping 2-column grid, and its value cells cannot wrap mid-token", () => {
    expect(block).toContain("grid-cols-1");
    expect(block).not.toContain("grid-cols-2");
    // `net-per-usd-nonpositive` renders whole (never split across 3 lines into
    // the neighbouring column, which is what the operator saw)
    expect(block).toContain("net-per-usd-nonpositive");
    expect(count(block, "whitespace-nowrap tabular-nums")).toBe(4);
    // the raw wire code stays discoverable in the tooltip
    expect(block).toContain(
      "binding_floor del wire (inverse-sizing kernel): net-per-usd-nonpositive",
    );
  });
});

describe("CARDS-PRECEDENCE-02 — a computed value always wins its cell", () => {
  it("Net cell: canonical spine net wins when present", () => {
    const html = card(mapToOmniOpportunity(wire({ net_expected_profit_usd: 8.25 })));
    expect(netCell(html)).toContain("$8.25");
    expect(netCell(html)).not.toContain("~");
  });

  it("Net cell: falls back to the SIM net (marked ~) when the canonical net is absent", () => {
    const html = card(
      mapToOmniOpportunity(
        wire({ net_expected_profit_usd: null, simulated_net_profit_usd: -15.304361738499997 }),
      ),
    );
    // the computed simulated net is displayed instead of an empty cell
    expect(netCell(html)).toContain("~-$15.30");
  });

  it("bps cell: sale del ratio canónico; el forward-sim NO se mezcla en otra base", () => {
    const canonical = card(
      mapToOmniOpportunity(wire({ roi_pct: 0.12, simulated_roi_pct: 0.38 })),
    );
    // ECON-DECLARE-01: the canonical ratio wins its cell — asserted on the cell's
    // own value span, which is what the operator reads.
    expect(cellValue(canonical, "bps")).toBe("12");
    expect(cellValue(canonical, "bps")).not.toContain("~38");

    // REAL-LIVE-CARDS-SSOT-01: el ratio del forward-sim no se pinta en un slot
    // canónico — sin ratio canónico la celda es honesta (—), nunca mezcla bases.
    const simulated = card(
      mapToOmniOpportunity(wire({ roi_pct: null, simulated_roi_pct: 0.38 })),
    );
    const sStart = simulated.indexOf(">bps<");
    expect(simulated.slice(sStart, sStart + 400)).toContain(">—</div>");
    expect(simulated.slice(sStart, sStart + 400)).not.toContain("~38");
  });

  it("the hop rows' numbers come from the kernel ledger only — never a fabricated USD total", () => {
    // no per-hop ledger ⇒ no per-hop figure anywhere on the card
    const html = card(mapToOmniOpportunity(wire({})));
    expect(html).not.toContain("·wei");
    expect(html).not.toContain("ciclo ");
    expect(html).not.toContain("Δ $");
  });
});

describe("CARDS-QUIET-01 (card surface) — no placeholder wall in a value slot", () => {
  it("a rejected row with null economics keeps the values quiet and the reason in title", () => {
    const html = card(
      mapToOmniOpportunity(
        wire({
          expected_profit_usd: null,
          net_expected_profit_usd: null,
          simulated_net_profit_usd: null,
          simulated_amount_in_usd: null,
          roi_pct: null,
          simulated_roi_pct: null,
        }),
      ),
    );
    expect(html).not.toContain(">no computado<");
    expect(html).not.toContain(">no emitido<");
    // nothing is hidden: the machine reason still travels in the cell titles
    expect(html).toContain("no computado: non_positive_profit (R8)");
    // the QUIET empty state is the dash, and no figure is invented for the
    // missing values (the cost rows and the ledger ends stay dashes).
    expect(html).toContain("Capital path (USD)");
    expect(html).toContain("Net yield");
    expect(html).not.toContain("ciclo ");
    expect(html).not.toContain("·wei");
  });
});

describe("R1 — SSR determinism of the fixed card", () => {
  it("pure render is byte-identical across invocations", () => {
    const opp = mapToOmniOpportunity(wire({}));
    expect(card(opp)).toBe(card(opp));
  });
});
