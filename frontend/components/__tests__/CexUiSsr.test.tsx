// frontend/components/__tests__/CexUiSsr.test.tsx
//
// (d) WO-PRICE-EXCHANGE-V1 — SSR regression: the CEX-premium treatment adds
// ZERO non-determinism to the server snapshot. With isMounted=false (the R1
// Mounted Snapshot gate) the card's server markup must contain:
//   - no flash classes (first observation never animates),
//   - no sparkline <polyline> (client-only, fed from effects),
//   - the freshness badge in its honest pre-mount state (muted dot, "--"),
// and must be byte-identical across repeated renders. The whole suite runs
// in vitest's node environment — `window` is absent by construction here.
import React from "react";
import { describe, expect, it, vi } from "vitest";
import { renderToStaticMarkup } from "react-dom/server";

// Same passthrough as OpportunityTradeCard.test.tsx (ui/skeleton type-only
// React import breaks the classic-JSX SSR path).
vi.mock("@/components/ui/skeleton", () => ({
  Skeleton: (props: React.ComponentProps<"div">) =>
    React.createElement("div", { "data-slot": "skeleton", ...props }),
}));

import { OpportunityTradeCard } from "../OpportunityTradeCard";
import { mapToOmniOpportunity } from "@/lib/store/types";

const wire = (over: Record<string, unknown>) => ({
  id: "opp-cex-1",
  chain_id: 1,
  strategy_kind: "triangular_atomic",
  detected_at: "2026-09-18T00:00:00Z",
  status: "detected",
  trace_id: "trace-cex",
  dex_a: "uniswap-v2",
  dex_b: "sushiswap",
  token_in: "0x" + "a".repeat(40),
  token_out: "0x" + "b".repeat(40),
  token_in_info: { symbol: "WETH", decimals: 18, logo_url: null, resolved_via: "onchain_full" },
  expected_profit_usd: 12.5,
  net_expected_profit_usd: 9.1,
  pair_symbol: "WETH/USDC",
  block_number: 123,
  ...over,
});

function ssrCard(over: Record<string, unknown> = {}): string {
  return renderToStaticMarkup(
    React.createElement(OpportunityTradeCard, {
      opp: mapToOmniOpportunity(wire(over)),
      now: 0,
      isMounted: false, // R1: the SSR path
      simLoading: false,
      onExecute: () => {},
      onInspect: () => {},
    }),
  );
}

describe("CEX treatment — SSR regression (R1/R5, no window)", () => {
  it("node test env really has no window (the regression premise)", () => {
    expect(typeof window).toBe("undefined");
  });

  it("server markup carries NO flash class — first observation never animates", () => {
    expect(ssrCard()).not.toContain("arbx-flash-up");
    expect(ssrCard()).not.toContain("arbx-flash-down");
  });

  it("server markup carries NO sparkline polyline — client-only via effects", () => {
    expect(ssrCard()).not.toContain("<polyline");
    expect(ssrCard()).not.toContain("sparkline-empty");
    expect(ssrCard()).not.toContain("Route trend");
  });

  it("freshness badge renders its honest pre-mount state: symbol, muted dot, '--'", () => {
    const html = ssrCard();
    expect(html).toContain("freshness-badge");
    expect(html).toContain('data-level="unknown"');
    expect(html).toContain("bg-muted-foreground/50");
    expect(html).toContain("WETH");
    expect(html).toContain("--");
    // never a green/amber/red claim before the client clock exists
    expect(html).not.toContain('data-level="fresh"');
  });

  it("missing token_in_info → badge shows the shortAddr fallback symbol", () => {
    const html = ssrCard({ token_in_info: null });
    expect(html).toContain("freshness-badge");
    expect(html).toContain("0xaaaa…aaaa");
  });

  it("R1: repeated SSR renders are byte-identical (ref-backed flash adds no drift)", () => {
    expect(ssrCard()).toBe(ssrCard());
    expect(ssrCard({ expected_profit_usd: 20 })).toBe(ssrCard({ expected_profit_usd: 20 }));
  });
});
