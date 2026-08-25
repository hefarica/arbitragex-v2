// frontend/app/strategies/tabs/__tests__/TokenAllowlistTab.resolve.test.tsx
//
// FE-MASTER · FE-0010/0011/0012 — P3 token universe, SSR-branch tests.
//
// The tab's interactive shell (chips, save) is client-event driven and stays
// uncovered here by design (repo node env); the three CONTRACT-BEARING
// surfaces are pure and render/assert directly:
//   - TokenResolvePreviewTable — §5: one row per REQUESTED symbol, honest
//     statuses, §62 liquidity decimal-string verbatim, null = "—";
//   - UniverseKpiCards — §6: backend-derived KPIs verbatim, null = "—"
//     (never 0), §79 caption;
//   - unresolvedSymbols — the §5/FE-0012 save gate as a pure function.
import React from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";

import {
  TokenResolvePreviewTable,
  UniverseKpiCards,
  unresolvedSymbols,
} from "../TokenAllowlistTab";
import type { TokenResolvePreviewRow, TokenUniverseKpi } from "@/lib/apex/schemas";

const A = "0x" + "a".repeat(40);

// One row per REQUESTED symbol across all four honest states.
const ROWS: TokenResolvePreviewRow[] = [
  {
    input_symbol: "WETH",
    chain_id: 1,
    address: A,
    decimals: 18,
    pool_count: 3,
    venue_count: 2,
    liquidity_usd: "123456.78",
    resolution_status: "RESOLVED",
  },
  {
    input_symbol: "USDC",
    chain_id: 1,
    address: null,
    decimals: null,
    pool_count: null,
    venue_count: null,
    liquidity_usd: null,
    resolution_status: "AMBIGUOUS",
  },
  {
    input_symbol: "FOO",
    chain_id: 1,
    address: null,
    decimals: null,
    pool_count: null,
    venue_count: null,
    liquidity_usd: null,
    resolution_status: "NOT_FOUND",
  },
  {
    input_symbol: "MGMT",
    chain_id: 1,
    address: null,
    decimals: null,
    pool_count: null,
    venue_count: null,
    liquidity_usd: null,
    resolution_status: "UNSUPPORTED",
  },
];

const UNIVERSE: TokenUniverseKpi = {
  allowed_tokens: 24,
  possible_pairs: 276,
  directed_token_pairs: 552,
  active_pools: 310,
  active_venues: 6,
  graph_version: 21_000_456,
  universe_version: 7,
};

describe("TokenResolvePreviewTable — SSR branches (FE-0010 · §5)", () => {
  it("renders one row per requested symbol with ALL four honest statuses", () => {
    const html = renderToStaticMarkup(
      React.createElement(TokenResolvePreviewTable, { rows: ROWS, chainId: 1 }),
    );
    expect(html).toContain("WETH");
    expect(html).toContain("USDC");
    expect(html).toContain("FOO");
    expect(html).toContain("MGMT");
    expect(html).toContain("4 símbolos pedidos");
    expect(html).toContain("RESOLVED");
    expect(html).toContain("AMBIGUOUS");
    expect(html).toContain("NOT_FOUND");
    expect(html).toContain("UNSUPPORTED");
  });

  it("RESOLVED row renders its metadata; unresolved rows render honest dashes (R8)", () => {
    const html = renderToStaticMarkup(
      React.createElement(TokenResolvePreviewTable, { rows: ROWS, chainId: 1 }),
    );
    // §62: liquidity is a decimal string, verbatim — never reformatted.
    expect(html).toContain("123456.78");
    expect(html).toContain("18");
    expect(html).toContain("3");
    // The unresolved rows show the honest dash, never a fabricated 0/guess.
    expect(html).toContain("—");
  });
});

describe("UniverseKpiCards — SSR branches (FE-0011 · §6)", () => {
  it("renders backend-derived KPIs verbatim — combinatorics NEVER computed in React", () => {
    const html = renderToStaticMarkup(
      React.createElement(UniverseKpiCards, { universe: UNIVERSE }),
    );
    expect(html).toContain("Tokens N");
    expect(html).toContain("24");
    expect(html).toContain("Pares C(N,2)");
    expect(html).toContain("276"); // Σ C(24,2) — served, not derived
    expect(html).toContain("Dirigidos N(N−1)");
    expect(html).toContain("552");
    expect(html).toContain("310");
    expect(html).toContain("6");
    expect(html).toContain("21000456");
    expect(html).toContain("7");
    expect(html).toContain("§79");
  });

  it("null universe renders all dashes + the honest absence note (never zeros)", () => {
    const html = renderToStaticMarkup(
      React.createElement(UniverseKpiCards, { universe: null }),
    );
    expect(html).toContain("Sin KPIs servidos aún");
    expect(html).not.toContain(">0<"); // a zeroed KPI would be fabricated
  });
});

describe("unresolvedSymbols — the §5 save gate (FE-0012)", () => {
  it("null preview blocks EVERYTHING — nothing may save unvalidated", () => {
    expect(unresolvedSymbols(["WETH", "USDC"], null)).toEqual(["WETH", "USDC"]);
  });

  it("blocks AMBIGUOUS/NOT_FOUND/UNSUPPORTED; passes RESOLVED", () => {
    expect(unresolvedSymbols(["WETH", "USDC", "FOO", "MGMT"], ROWS)).toEqual([
      "USDC",
      "FOO",
      "MGMT",
    ]);
    expect(unresolvedSymbols(["WETH"], ROWS)).toEqual([]);
  });

  it("symbols added AFTER the preview are uncovered ⇒ blocked (re-resolve)", () => {
    expect(unresolvedSymbols(["WETH", "NEW"], ROWS)).toEqual(["NEW"]);
  });

  it("matching is case-insensitive on the preview rows (input is upper-cased)", () => {
    const lower = ROWS.map((r) => ({ ...r, input_symbol: r.input_symbol.toLowerCase() }));
    expect(unresolvedSymbols(["WETH"], lower)).toEqual([]);
    expect(unresolvedSymbols(["FOO"], lower)).toEqual(["FOO"]);
  });
});
