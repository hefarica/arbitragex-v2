// frontend/components/__tests__/OpportunityTradeCard.test.tsx
//
// HOPS-CARD-03 — the step-ladder renders the REAL N-leg topology from
// deriveLegs (route_metadata when present, §29-marked synthetic fallback
// otherwise), with per-leg symbols (pair-info → leg_symbols → shortAddr).
// The old hardcoded "Buy A / Buy B" 2-row ladder hid every N-leg route.
// Fixtures go through the real mapper. R1: static render only — the card's
// time-dependent cells are gated by isMounted and stay deterministic here.
import React from "react";
import { describe, it, expect, vi } from "vitest";
import { renderToStaticMarkup } from "react-dom/server";

// ui/skeleton's type-only React import breaks vitest's classic-JSX SSR path
// ("React is not defined" — cf. ui/tabs.tsx which carries the value-import
// convention). Passthrough mock: the ladder assertions never depend on the
// skeleton visual. (Latent issue in ui/skeleton.tsx, noted in the PR.)
vi.mock("@/components/ui/skeleton", () => ({
  Skeleton: (props: React.ComponentProps<"div">) =>
    React.createElement("div", { "data-slot": "skeleton", ...props }),
}));

import { OpportunityTradeCard } from "../OpportunityTradeCard";
import { mapToOmniOpportunity } from "@/lib/store/types";

const wire = (over: Record<string, unknown>) => ({
  id: "opp-1",
  chain_id: 1,
  strategy_kind: "triangular_atomic",
  detected_at: "2026-08-11T00:00:00Z",
  status: "detected",
  trace_id: "trace-1",
  dex_a: "uniswap-v2",
  dex_b: "sushiswap",
  token_in: "0xa",
  token_out: "0xb",
  block_number: 123,
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

const A = "0x" + "a".repeat(40);
const B = "0x" + "b".repeat(40);
const C = "0x" + "c".repeat(40);

describe("OpportunityTradeCard — HOPS-CARD-03 step ladder", () => {
  it("renders every leg of a 3-hop route with symbols — not the hardcoded 2-row ladder", () => {
    const info = { symbol: "WETH", decimals: 18, logo_url: null, resolved_via: "onchain_full" };
    // leg_symbols is injected at the VIEWMODEL level (post-mapper): the mapper
    // passthrough lands in PR #535 (HOPS-SYM-02) — this PR stays
    // merge-order-independent. The card resolves whatever the store holds.
    const opp = {
      ...mapToOmniOpportunity(
        wire({
          token_in: A,
          token_out: A,
          token_in_info: info,
          token_out_info: info,
          route_metadata: {
            dex_adapters: ["uniswap_v2_router", "sushiswap", "uniswap_v2_router"],
            token_addresses: [A, B, C, A],
            pool_addresses: ["0xpool1", "0xpool2", "0xpool3"],
          },
        }),
      ),
      leg_symbols: { [B.toLowerCase()]: "USDC", [C.toLowerCase()]: "PEPE" },
    };
    const html = card(opp);
    // all three hops present, in order, with the hop counter
    expect(html).toContain("Hop 1/3");
    expect(html).toContain("Hop 2/3");
    expect(html).toContain("Hop 3/3");
    // symbol resolution: pair-info endpoints + leg_symbols intermediates
    expect(html).toContain("WETH→USDC");
    expect(html).toContain("USDC→PEPE");
    expect(html).toContain("PEPE→WETH");
    // dex per leg in the hint surface
    expect(html).toContain("uniswap_v2_router");
    expect(html).toContain("sushiswap");
    // the hardcoded ladder is gone
    expect(html).not.toContain("Buy A");
    expect(html).not.toContain("Buy B");
    // real topology ⇒ NO §29 marker
    expect(html).not.toContain("SYNTHETIC LEGACY VIEW");
  });

  it("2-hop dex route renders its 2 real legs (dex_a/dex_b no longer drive the ladder)", () => {
    const opp = mapToOmniOpportunity(
      wire({
        route_metadata: {
          dex_adapters: ["uniswap-v2", "sushiswap"],
          token_addresses: ["0xa", "0xb", "0xa"],
          pool_addresses: ["0xpool1", "0xpool2"],
        },
      }),
    );
    const html = card(opp);
    expect(html).toContain("Hop 1/2");
    expect(html).toContain("Hop 2/2");
    // shortAddr fallback for short fixture addresses (R8 — no fabrication)
    expect(html).toContain("0xa→0xb");
    expect(html).not.toContain("SYNTHETIC LEGACY VIEW");
  });

  it("no route_metadata ⇒ §29 synthetic fallback ladder, MARKED — never ROUTE VERIFIED", () => {
    const opp = mapToOmniOpportunity(wire({ route_metadata: null }));
    const html = card(opp);
    // deriveLegs synthetic 2-leg cycle from dex_a/dex_b
    expect(html).toContain("Hop 1/2");
    expect(html).toContain("Hop 2/2");
    expect(html).toContain("SYNTHETIC LEGACY VIEW");
    expect(html).toContain("no ROUTE VERIFIED");
    expect(html).toContain("syn"); // per-leg marker in the hint
  });

  it("no topology at all (no route_metadata, no dex pair) ⇒ honest gap row, no fabricated hops", () => {
    const opp = mapToOmniOpportunity(
      wire({ route_metadata: null, dex_a: "", dex_b: null }),
    );
    const html = card(opp);
    expect(html).toContain("sin topología persistida (§38)");
    expect(html).not.toContain("Hop 1/");
    expect(html).not.toContain("SYNTHETIC LEGACY VIEW");
  });

  it("per-leg amounts stay honest nulls — the ladder shows topology only until the wire carries amounts", () => {
    // HOPS-LEDGER-04 (wire amounts) is a sibling PR; until it lands every
    // hop row renders the dash for its value, never a fabricated running total.
    const opp = mapToOmniOpportunity(
      wire({
        route_metadata: {
          dex_adapters: ["uniswap-v2", "sushiswap"],
          token_addresses: ["0xa", "0xb", "0xa"],
          pool_addresses: ["0xpool1", "0xpool2"],
        },
      }),
    );
    const html = card(opp);
    expect(html).toContain("Capital path (USD)");
    // the hop rows exist and the ledger block still renders its honest ends
    expect(html).toContain("Gross out (AMM spread)");
    expect(html).toContain("Net yield");
  });

  it("R1: pure render is byte-identical across invocations", () => {
    const opp = mapToOmniOpportunity(
      wire({
        route_metadata: {
          dex_adapters: ["uniswap-v2", "sushiswap"],
          token_addresses: ["0xa", "0xb", "0xa"],
          pool_addresses: ["0xpool1", "0xpool2"],
        },
      }),
    );
    expect(card(opp)).toBe(card(opp));
  });
});
