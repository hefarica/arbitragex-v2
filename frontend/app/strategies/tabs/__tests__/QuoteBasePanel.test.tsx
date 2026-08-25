// frontend/app/strategies/tabs/__tests__/QuoteBasePanel.test.tsx
//
// FE-MASTER · FE-0013/0014/0015/0016 — Quote/Base panel, SSR-branch tests.
//
// Store seam: `useOmniStore` is mocked at the module boundary (the slice's
// fetch semantics are pinned by quote-slices tests of 7b). What THESE tests
// pin is the panel's contract over whatever the slice serves:
//   - §8 anchor card renders payload values verbatim (symbol, score/100,
//     version badges, 5-axis components, runtime weight mirror toFixed(3));
//   - §9 table renders the per-token rows in PAYLOAD order (backend-fixed);
//   - the endpoint's honest 503/error renders verbatim (role=alert); idle
//     renders "—"; tokens:[] renders the honest empty note;
//   - §10 in its non-editing state carries the QB-TOPOLOGY-01 doctrine copy;
//   - §11 coherencia: SSR (no effects) renders the honest "knobs snapshot no
//     servido" note — never a fabricated coherencia.
// Pure exports (PreviewResult, quoteWeightsKey, knobsToQuoteWeights,
// QuoteWeightsCoherency) are tested directly.
import React from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { beforeEach, describe, expect, it, vi } from "vitest";

const storeState = vi.hoisted(() => ({ current: {} as Record<string, unknown> }));

vi.mock("@/lib/store/omni-store", () => ({
  useOmniStore: (selector: (s: Record<string, unknown>) => unknown) =>
    selector(storeState.current),
}));

import {
  PreviewResult,
  QuoteBasePanel,
  QuoteWeightsCoherency,
  knobsToQuoteWeights,
  quoteWeightsKey,
} from "../QuoteBasePanel";
import type {
  QuoteAnchorResponse,
  QuotePreviewResponse,
  QuoteWeights,
} from "@/lib/apex/schemas";

const WEIGHTS: QuoteWeights = { prior: 0.3, liquidity: 0.3, venues: 0.2, stability: 0.1, cross_dex: 0.1 };

const ANCHOR: QuoteAnchorResponse = {
  chain_id: 1,
  quote_symbol: "USDC",
  quote_score: 96,
  quote_version: 7,
  graph_version: 42,
  components: { prior: 80.5, liquidity: 90.25, venues: 70, stability: 60, cross_dex: 50 },
  weights: WEIGHTS,
  tokens: [
    {
      symbol: "USDC",
      address: "0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48",
      components: { prior: 80, liquidity: 90, venues: 70, stability: 60, cross_dex: 50 },
      score: 96,
    },
    {
      symbol: "WETH",
      address: "0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2",
      components: { prior: 70, liquidity: 85, venues: 65, stability: 55, cross_dex: 40 },
      score: 91,
    },
  ],
};

function seed(partial: {
  anchor?: QuoteAnchorResponse | null;
  status?: string;
  error?: string | null;
}) {
  storeState.current = {
    quoteAnchor: partial.anchor ?? null,
    quoteAnchorStatus: partial.status ?? "ready",
    quoteAnchorError: partial.error ?? null,
    quoteAnchorUpdatedAt: "2026-08-24T00:00:00.000Z",
    fetchQuoteAnchor: vi.fn(),
  };
}

function render() {
  return renderToStaticMarkup(
    React.createElement(QuoteBasePanel, { chainId: 1, adminToken: "t", actor: "a" }),
  );
}

beforeEach(() => {
  seed({ anchor: null, status: "idle" });
});

describe("QuoteBasePanel — SSR branches (FE-0013/0014 · §8/§9)", () => {
  it("§8 renders the anchor verbatim: symbol, score/100, version badges, axes, weight mirror", () => {
    seed({ anchor: ANCHOR });
    const html = render();
    expect(html).toContain("USDC");
    expect(html).toContain("score 96.0/100");
    expect(html).toContain("quote_version 7");
    expect(html).toContain("graph_version 42");
    expect(html).toContain("Componentes del score (§9)");
    expect(html).toContain("Pesos runtime (espejo de quote_w_*)");
    // Five axis labels render on both §8 blocks + the §9 table header.
    for (const label of ["Prior", "Liquidity", "Venues", "Stability", "Cross-DEX"]) {
      expect(html.match(new RegExp(`>${label}<`, "g"))?.length).toBe(3);
    }
    // Weight mirror is display-formatted 3-decimals from the payload (§9).
    expect(html).toContain("0.300");
    expect(html).toContain("0.100");
    // Component value renders fixed(1) display-only.
    expect(html).toContain("90.3");
  });

  it("§9 table renders per-token rows in PAYLOAD order with address elision", () => {
    seed({ anchor: ANCHOR });
    const html = render();
    expect(html).toContain("Score explicable por token (§9)");
    expect(html.indexOf("USDC")).toBeLessThan(html.indexOf("WETH"));
    // Address elision renders first-8…last-6 of the payload address.
    expect(html).toContain("0xC02aaA");
    expect(html).toContain("756Cc2");
    expect(html).toContain("Score");
  });

  it("error state renders the endpoint reason verbatim (role=alert)", () => {
    seed({ anchor: null, status: "error", error: "HTTP 503: quote_anchor_not_published" });
    const html = render();
    expect(html).toContain("quote_anchor_not_published");
    expect(html).toContain('role="alert"');
  });

  it("idle/null renders the honest dash — never a fabricated anchor", () => {
    const html = render();
    expect(html).toContain("—");
    expect(html).not.toContain("quote_version 7");
  });

  it("tokens: [] renders the honest empty note — the zero is served, not an error", () => {
    seed({ anchor: { ...ANCHOR, tokens: [] } });
    const html = render();
    expect(html).toContain("Sin tokens computables este tick");
    expect(html).not.toContain('role="alert"');
  });
});

describe("QuoteBasePanel — §10 idle copy + §11 SSR (FE-0015/0016)", () => {
  it("§10 non-editing state carries the QB-TOPOLOGY-01 doctrine copy", () => {
    seed({ anchor: ANCHOR });
    const html = render();
    expect(html).toContain("Re-rankeo determinístico");
    expect(html).toContain("QB-TOPOLOGY-01");
    expect(html).toContain("El apply fluye por los knobs canónicos");
  });

  it("§11 SSR (no effects ran): the honest not-served note, never a fabricated coherencia", () => {
    seed({ anchor: ANCHOR });
    const html = render();
    expect(html).toContain("knobs snapshot no servido");
    expect(html).not.toContain("knobs ↔ runtime");
  });
});

describe("PreviewResult — pure (FE-0015 · §10)", () => {
  const PREVIEW: QuotePreviewResponse = {
    impact: {
      graph_rebuild_required: false,
      quote_revaluation_required: true,
      quote_cache_invalidation_required: true,
      affected_pairs: 12,
      affected_edges: 34,
      affected_cached_routes: 5,
      current_quote_version: 7,
      proposed_quote_version: 8,
      topology_version_unchanged: true,
    },
    proposed_quote_symbol: "WETH",
    proposed_quote_score: 91,
    proposed_tokens: [
      {
        symbol: "WETH",
        address: "0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2",
        components: { prior: 75, liquidity: 88, venues: 66, stability: 58, cross_dex: 42 },
        score: 91.2,
      },
      {
        symbol: "USDC",
        address: "0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48",
        components: { prior: 80, liquidity: 90, venues: 70, stability: 60, cross_dex: 50 },
        score: 89.9,
      },
    ],
  };

  function renderPreview(preview: QuotePreviewResponse) {
    return renderToStaticMarkup(React.createElement(PreviewResult, { preview }));
  }

  it("renders the impact badges verbatim, including the doctrine literals", () => {
    const html = renderPreview(PREVIEW);
    expect(html).toContain("Cambia anchor → WETH");
    expect(html).toContain("quote_version 7 → 8");
    expect(html).toContain("pares afectados 12");
    expect(html).toContain("edges afectados 34");
    expect(html).toContain("rutas cacheadas 5");
    expect(html).toContain("graph_rebuild_required false");
    expect(html).toContain("topology_unchanged true");
  });

  it("proposed table renders payload order with ONLY the proposed-anchor row highlighted", () => {
    const html = renderPreview(PREVIEW);
    expect(html.indexOf("WETH")).toBeLessThan(html.indexOf("USDC"));
    expect(html.match(/bg-muted\/50/g)?.length).toBe(1);
    expect(html).toContain("Score propuesto");
    expect(html).toContain("91.2");
  });

  it("no-revaluation preview renders the honest no-change badge", () => {
    const html = renderPreview({
      ...PREVIEW,
      impact: { ...PREVIEW.impact, quote_revaluation_required: false, proposed_quote_version: 7 },
    });
    expect(html).toContain("Anchor sin cambio");
    expect(html).not.toContain("Cambia anchor");
  });
});

describe("§11 coherencia — pure helpers (FE-0016)", () => {
  it("quoteWeightsKey is the fixed(6) five-axis serialization", () => {
    expect(quoteWeightsKey(WEIGHTS)).toBe("0.300000/0.300000/0.200000/0.100000/0.100000");
  });

  it("knobsToQuoteWeights maps serde snake_case onto the mirror shape", () => {
    expect(
      knobsToQuoteWeights({
        quote_w_prior: 0.3,
        quote_w_liquidity: 0.3,
        quote_w_venue_coverage: 0.2,
        quote_w_stability: 0.1,
        quote_w_cross_dex: 0.1,
        beam_k: 8,
      }),
    ).toEqual(WEIGHTS);
  });

  it("knobsToQuoteWeights: a missing knob → null (NOT_EXPOSED — never zero-filled, R8)", () => {
    expect(knobsToQuoteWeights({ quote_w_prior: 0.3 })).toBe(null);
    expect(knobsToQuoteWeights({ quote_w_venue_coverage: "0.2" })).toBe(null);
  });

  function renderCoherency(snapshot: QuoteWeights | null, mirror: QuoteWeights | null) {
    return renderToStaticMarkup(
      React.createElement(QuoteWeightsCoherency, { snapshot, mirror }),
    );
  }

  it("null snapshot renders the honest not-served note", () => {
    const html = renderCoherency(null, WEIGHTS);
    expect(html).toContain("knobs snapshot no servido");
  });

  it("snapshot present, mirror absent → NOT EXPOSED (R8 dash, never zero)", () => {
    const html = renderCoherency(WEIGHTS, null);
    expect(html).toContain("NOT EXPOSED");
    expect(html).toContain("—");
  });

  it("knobs == mirror → EFFECTIVE (the runtime runs exactly what was configured)", () => {
    const html = renderCoherency(WEIGHTS, WEIGHTS);
    expect(html).toContain("EFFECTIVE");
    expect(html).toContain(quoteWeightsKey(WEIGHTS));
  });

  it("knobs ≠ mirror (e.g. env changed, searcher not restarted) → CONFIGURED", () => {
    const html = renderCoherency(WEIGHTS, { ...WEIGHTS, prior: 0.25, liquidity: 0.35 });
    expect(html).toContain("CONFIGURED");
    expect(html).toContain("0.250000/0.350000/0.200000/0.100000/0.100000");
  });
});
