// frontend/app/strategies/tabs/__tests__/PairAlphaHeatmap.test.tsx
//
// FE-MASTER · FE-0018 — directed alpha heatmap, SSR-branch tests.
//
// Pure presentational component (no store seam needed — props in, markup
// out). renderToStaticMarkup asserts the deterministic branches:
//   - cell value = (F_e−1)×10⁴ bps of the PAYLOAD number (never a
//     recomputed spot spread — §79);
//   - r15: (i,j) and (j,i) are independent cells fed by forward/reverse;
//   - null alpha renders "·" (no data), never 0;
//   - NO computed alpha at all ⇒ the honest no-data message replaces the
//     matrix (an all-gray grid would read as "all zero");
//   - dimension honesty: >MAX_DIMENSION tokens ⇒ refused with the filter
//     instruction, never a silent crop.
import React from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";

import { PairAlphaHeatmap } from "../PairAlphaHeatmap";
import type { PairView } from "@/lib/apex/schemas";

const A = "0x" + "a".repeat(40);
const B = "0x" + "b".repeat(40);
const C = "0x" + "c".repeat(40);

function pair(
  symA: string,
  symB: string,
  addrA: string,
  addrB: string,
  fwd: number | null,
  rev: number | null,
): PairView {
  return {
    chain_id: 1,
    token_a: { chain_id: 1, address: addrA, symbol: symA, decimals: 18 },
    token_b: { chain_id: 1, address: addrB, symbol: symB, decimals: 18 },
    pools: [],
    venue_count: 0,
    alpha_forward: fwd,
    alpha_reverse: rev,
    dirty: false,
    last_reserve_update: null,
  };
}

const ASYMMETRIC: PairView[] = [
  // forward +2.5bps, reverse NOT computed — the two triangles differ.
  pair("WETH", "USDC", A, B, 1.00025, null),
];

const RICH: PairView[] = [
  pair("WETH", "USDC", A, B, 1.00025, 0.9997),
  pair("WETH", "DAI", A, C, 0.99951, 1.00003),
];

function render(pairs: PairView[]) {
  return renderToStaticMarkup(React.createElement(PairAlphaHeatmap, { pairs }));
}

describe("PairAlphaHeatmap — SSR branches (FE-0018 · §14)", () => {
  it("cell value is the bps form of the PAYLOAD F_e — (F_e−1)×10⁴, no rate math", () => {
    const html = render(RICH);
    expect(html).toContain("2.5"); // (1.00025−1)×10⁴
    expect(html).toContain("-3.0"); // (0.9997−1)×10⁴ — sign preserved
    expect(html).toContain("-4.9"); // (0.99951−1)×10⁴
    expect(html).toContain("0.3"); // (1.00003−1)×10⁴
  });

  it("r15: (i,j) and (j,i) are independent cells (asymmetric alphas differ)", () => {
    const html = render(RICH);
    // WETH→USDC = +2.5 exists; USDC→WETH = -3.0 (NOT −forward, which would
    // be -2.5 — the directions are independent payload fields).
    expect(html).toContain("2.5");
    expect(html).toContain("-3.0");
    expect(html).not.toContain("-2.5"); // the collapse bug would mirror the forward
    // 3 tokens (WETH/USDC/DAI), 4 computed cells: WETH↔USDC both directions
    // + WETH↔DAI both directions; USDC↔DAI renders the honest "·".
    expect(html).toContain("3 tokens · 4 celdas computadas");
    expect(html).toContain("·");
  });

  it("null alpha renders the empty dot, never 0 (R8)", () => {
    const html = render(ASYMMETRIC);
    expect(html).toContain("·");
    expect(html).not.toContain(">0.0<"); // a fabricated zero cell
  });

  it("NO computed alpha at all ⇒ honest no-data message replaces the matrix", () => {
    const html = render([pair("WETH", "USDC", A, B, null, null)]);
    // The honest 0 summary coexists with the message; what must NOT render
    // is the matrix itself — no colored cells, no computed values.
    expect(html).toContain("0 celdas computadas");
    expect(html).toContain("Ningún par con α computado");
    expect(html).not.toContain("<table"); // the matrix is replaced, never grayed
    expect(html).not.toContain("bg-emerald"); // no fabricated cells
  });

  it("empty pair set renders the honest dash", () => {
    const html = render([]);
    expect(html).toContain("—");
  });

  it("dimension honesty: >40 tokens refuses the matrix with the filter instruction", () => {
    // Each token needs a DISTINCT address (identity is by address since the
    // R3 note — one address carries one symbol, the old fixture reused A/B).
    const addr = (n: number): string => "0x" + n.toString(16).padStart(40, "0");
    const many: PairView[] = [];
    for (let i = 0; i < 41; i++) {
      many.push(pair(`T${i}`, `U${i}`, addr(i * 2), addr(i * 2 + 1), 1.001, 0.999));
    }
    const html = render(many);
    expect(html).toContain("exceden la dimensión renderizable");
    expect(html).not.toContain(">1.0<"); // no cells rendered at all
  });

  it("identity by ADDRESS: two tokens sharing a symbol keep SEPARATE axes, never collapse (R3 7b note)", () => {
    // The scam-clone pattern: a second "USDC" at a different address.
    const D = "0x" + "d".repeat(40);
    const dup: PairView[] = [
      pair("WETH", "USDC", A, B, 1.00025, null),
      pair("WETH", "USDC", A, D, 0.99951, null),
    ];
    const html = render(dup);
    // THREE tokens (WETH + two distinct USDCs) — symbol-keying would say 2.
    expect(html).toContain("3 tokens");
    // Disambiguated labels carry the shortAddr suffix…
    expect(html.match(/USDC·/g)?.length).toBeGreaterThanOrEqual(2);
    // …and both pairs' alphas survive as INDEPENDENT cells (+2.5 and −4.9);
    // a collapsed shared cell would have last-write-wins destroyed one.
    expect(html).toContain("2.5");
    expect(html).toContain("-4.9");
  });
});
