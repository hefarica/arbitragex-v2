// frontend/features/route-discovery/__tests__/RouteOutcomesAnalyticsPanel.test.tsx
//
// FE-MASTER · FE-0038 §47 — outcomes analytics body, SSR-branch tests.
// The pure core renders the §47 group-bys the sink actually persists
// (by-strategy / by-pair) with the honest-gap block for the dimensions it
// does NOT (hop / detector / DEX — nivel-(b), never an invented join).
import React from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";

import { OutcomesAnalyticsBody, shortAddr } from "../RouteOutcomesAnalyticsPanel";
import type { OutcomeCartridgeRow, OutcomePairRow } from "@/lib/hooks/useRouteDiscoveryOutcomes";

const CARTRIDGES: OutcomeCartridgeRow[] = [
  { cartridge_id: "MEV-01-015", n: 400, opportunities: 1 },
  { cartridge_id: "(null)", n: 12, opportunities: 0 },
];

const PAIRS: OutcomePairRow[] = [
  {
    token_in: "0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2",
    token_out: "0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48",
    n: 250,
    opportunities: 1,
  },
];

function render(props: Partial<Parameters<typeof OutcomesAnalyticsBody>[0]> = {}) {
  return renderToStaticMarkup(
    React.createElement(OutcomesAnalyticsBody, {
      byCartridge: CARTRIDGES,
      byPair: PAIRS,
      groupingsServed: true,
      windowHours: 24,
      ...props,
    }),
  );
}

describe("shortAddr — display-only folding", () => {
  it("folds long addresses to 8+…+6 and leaves short ones untouched", () => {
    expect(shortAddr("0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2")).toBe("0xC02aaA…756Cc2");
    expect(shortAddr("0xAbCd")).toBe("0xAbCd");
    expect(shortAddr("—")).toBe("—");
  });
});

describe("OutcomesAnalyticsBody — §47 SSR branches", () => {
  it("renders by-strategy rows verbatim (cartridge_id IS the strategy key)", () => {
    const html = render();
    expect(html).toContain("MEV-01-015");
    expect(html).toContain(">400</td>");
    expect(html).toContain("cartridge_id — la llave de estrategia");
    // The SQL-side '(null)' fold passes through untouched.
    expect(html).toContain("(null)");
  });

  it("renders by-pair with short addresses AND the full address in the title", () => {
    const html = render();
    expect(html).toContain("0xC02aaA…756Cc2");
    expect(html).toContain("→");
    expect(html).toContain('title="0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2 → 0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48"');
    expect(html).toContain(">250</td>");
  });

  it("carries the honest-gap block: hop / detector / DEX are NOT in the sink", () => {
    const html = render();
    expect(html).toContain("No emitido en el sink");
    expect(html).toContain("Por hop");
    expect(html).toContain("Por detector");
    expect(html).toContain("Por DEX");
    expect(html).toContain("jamás un JOIN inventado");
    // By chain is a POINTER to the Gate-C panel, not a duplicated table.
    expect(html).toContain("By chain vive en el panel Gate-C");
    expect(html).not.toContain("By chain</h4>");
  });

  it("served-but-empty groupings render the honest zero-row message — not the not-served note", () => {
    const html = render({ byCartridge: [], byPair: [] });
    expect(html).toContain("Sin filas en la ventana 24h");
    expect(html).not.toContain("no servidas por esta api-server");
  });

  it("groupings NOT carried by the response render the pre-deploy absence note", () => {
    const html = render({ groupingsServed: false });
    expect(html).toContain("Agrupaciones §47 no servidas por esta api-server");
    expect(html).not.toContain("By strategy");
  });

  it("the window is disclosed next to the zero-row message", () => {
    const html = render({ byPair: [], windowHours: null });
    expect(html).toContain("Sin filas en la ventana —h");
  });
});
