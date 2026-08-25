// frontend/app/strategies/tabs/__tests__/PairDetailDrawer.test.tsx
//
// FE-MASTER · FE-0019 — pair detail drawer body, SSR-branch tests.
//
// `PairDetailBody` is the pure presentational core (extracted exactly so it
// is testable without the Radix portal); renderToStaticMarkup asserts the
// deterministic branches:
//   - r15 front and center: forward and reverse render as INDEPENDENT
//     cards — both when asymmetric (one Some, one None) and when null;
//   - §62: reserves ride as decimal strings, verbatim;
//   - the canonical PairIndex key + quote context render from props
//     (null quote = honest "—", never a guess);
//   - hot seed renders the honest "no publicado (FE-0020)".
import React from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";

import { PairDetailBody, type QuoteContext } from "../PairDetailDrawer";
import type { PairView } from "@/lib/apex/schemas";

const A = "0x" + "a".repeat(40);
const B = "0x" + "b".repeat(40);

const QUOTE: QuoteContext = {
  quote_symbol: "USDC",
  quote_version: 7,
  graph_version: 21_000_456,
};

const PAIR: PairView = {
  chain_id: 1,
  token_a: { chain_id: 1, address: A, symbol: "WETH", decimals: 18 },
  token_b: { chain_id: 1, address: B, symbol: "USDC", decimals: 6 },
  pools: [
    {
      pool_address: "0x" + "1".repeat(40),
      venue: "uniswap_v2",
      fee_bps: 30,
      reserves_a: "123456789012345678901", // §62 u256-width decimal string
      reserves_b: "987654321",
    },
    {
      pool_address: "0x" + "2".repeat(40),
      venue: "uniswap_v3",
      fee_bps: 100,
      reserves_a: "42",
      reserves_b: "84000",
    },
  ],
  venue_count: 2,
  // r15 asymmetry: forward computed, reverse NOT — the cards must render
  // independently (a collapsed view would show −forward or mirror 0).
  alpha_forward: 1.00042,
  alpha_reverse: null,
  dirty: true,
  last_reserve_update: 1_724_000_000_000,
};

function render(pair: PairView, quote: QuoteContext | null) {
  return renderToStaticMarkup(
    React.createElement(PairDetailBody, { pair, quote }),
  );
}

describe("PairDetailBody — SSR branches (FE-0019 · §15/§16)", () => {
  it("renders the canonical PairIndex key (a|b, address-asc) verbatim", () => {
    const html = render(PAIR, QUOTE);
    expect(html).toContain(`${A}|${B}`);
  });

  it("renders quote context badges from props; null quote renders the honest dash", () => {
    const withQuote = render(PAIR, QUOTE);
    expect(withQuote).toContain("quote USDC");
    expect(withQuote).toContain("quote_version 7");
    expect(withQuote).toContain("graph_version 21000456");

    const noQuote = render(PAIR, null);
    expect(noQuote).toContain("sin snapshot servido");
    expect(noQuote).not.toContain("quote_version");
  });

  it("r15: forward and reverse render as INDEPENDENT values — asymmetry preserved", () => {
    const html = render(PAIR, QUOTE);
    // forward computed: F_e to 6dp + its bps display form
    expect(html).toContain("1.000420");
    expect(html).toContain("4.2 bps");
    // reverse null: em-dash in BOTH its value and bps slots — never −forward,
    // never a mirrored 0.
    expect(html).toContain("reverse");
    expect(html).not.toContain("-1.000420"); // −forward would be the collapse bug
    expect(html).toContain("reverse NUNCA es −forward"); // the invariant caption
  });

  it("§62: pool reserves ride as verbatim decimal strings (u256 width)", () => {
    const html = render(PAIR, QUOTE);
    expect(html).toContain("123456789012345678901");
    expect(html).toContain("987654321");
    expect(html).toContain("42");
    expect(html).toContain("84000");
  });

  it("lists every parallel pool (no dedupe, both venues, fees verbatim)", () => {
    const html = render(PAIR, QUOTE);
    expect(html).toContain("uniswap_v2");
    expect(html).toContain("uniswap_v3");
    expect(html).toContain("Pools paralelos (2 · 2 venues)");
  });

  it("dirty renders DIRTY; hot seed renders the honest FE-0020 gap", () => {
    const html = render(PAIR, QUOTE);
    expect(html).toContain("DIRTY");
    expect(html).toContain("no publicado (FE-0020)");
  });

  it("null last_reserve_update renders the honest dash", () => {
    const pair: PairView = { ...PAIR, last_reserve_update: null };
    const html = render(pair, QUOTE);
    expect(html).toContain("—");
  });
});
