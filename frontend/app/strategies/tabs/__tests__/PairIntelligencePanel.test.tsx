// frontend/app/strategies/tabs/__tests__\PairIntelligencePanel.test.tsx
//
// FE-MASTER · FE-0017 — Pair Intelligence panel, SSR-branch tests.
//
// The repo's frontend test env is `node` (no jsdom, no @testing-library/react),
// so — exactly like TokenIcon.test.tsx — we render to a static HTML string via
// react-dom/server and assert the deterministic branches:
//   - ready rows render payload values verbatim (RULE 00/§79);
//   - alpha null renders the honest "—" (R8) — NEVER a recomputed spread;
//   - dirty renders the DIRTY badge (FE-0020 upgrades the binary later);
//   - empty universe renders the honest empty message, not a fake error;
//   - error renders verbatim.
//
// Store seam: `useOmniStore` is mocked at the module boundary. The component
// is a pure renderer over the slice state (its own slice semantics are pinned
// by catalog-slices.test.ts); mocking the hook also sidesteps zustand v5's
// SSR path — useSyncExternalStore's getServerSnapshot reads the INITIAL
// state, so a setState before renderToStaticMarkup would be invisible.
//
// The runtime branches (30s poll, hashchange deep-link scroll) are effect
// driven — they never run under renderToStaticMarkup and stay uncovered here
// by design.
import React from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { beforeEach, describe, expect, it, vi } from "vitest";

const storeState = vi.hoisted(() => ({ current: {} as Record<string, unknown> }));

vi.mock("@/lib/store/omni-store", () => ({
  useOmniStore: (selector: (s: Record<string, unknown>) => unknown) =>
    selector(storeState.current),
}));

// TokenPairIcon is decoration for this panel test. Mocking it at the boundary
// also avoids its unknown-address shimmer path (Skeleton), whose JSX lacks
// value-scope React under the node transformer — the icon branches themselves
// are covered by components/ui/TokenIcon.test.tsx.
vi.mock("@/components/ui/TokenIcon", () => ({
  TokenPairIcon: () => React.createElement("span", { "data-testid": "pair-icon" }),
}));

import { PairIntelligencePanel } from "../PairIntelligencePanel";
import type { PairView } from "@/lib/apex/schemas";

const A = "0x" + "a".repeat(40);
const B = "0x" + "b".repeat(40);
const C = "0x" + "c".repeat(40);

function pool(poolAddress: string, venue: string, feeBps: number, reservesA: string, reservesB: string) {
  return {
    pool_address: poolAddress,
    venue,
    fee_bps: feeBps,
    reserves_a: reservesA,
    reserves_b: reservesB,
  };
}

const PAIR_WETH_USDC: PairView = {
  chain_id: 1,
  token_a: { chain_id: 1, address: A, symbol: "WETH", decimals: 18 },
  token_b: { chain_id: 1, address: B, symbol: "USDC", decimals: 6 },
  pools: [
    pool("0x" + "1".repeat(40), "uniswap_v2", 30, "1000", "2000000"),
    pool("0x" + "2".repeat(40), "sushiswap_v2", 30, "500", "1100000"),
    pool("0x" + "3".repeat(40), "uniswap_v3", 100, "700", "1500000"),
  ],
  venue_count: 3,
  alpha_forward: null,
  alpha_reverse: null,
  dirty: true,
  last_reserve_update: 1_724_000_000_000,
};

const PAIR_WETH_DAI: PairView = {
  chain_id: 1,
  token_a: { chain_id: 1, address: A, symbol: "WETH", decimals: 18 },
  token_b: { chain_id: 1, address: C, symbol: "DAI", decimals: 18 },
  pools: [pool("0x" + "4".repeat(40), "uniswap_v2", 30, "42", "84000")],
  venue_count: 1,
  alpha_forward: null,
  alpha_reverse: null,
  dirty: false,
  last_reserve_update: null, // never synced — honest null
};

function seed(partial: { pairs?: PairView[] | null; status?: string; error?: string | null }) {
  storeState.current = {
    pairs: partial.pairs ?? null,
    pairsStatus: partial.status ?? "ready",
    pairsError: partial.error ?? null,
    pairsUpdatedAt: partial.pairs ? "2026-08-24T00:00:00.000Z" : null,
    fetchPairs: vi.fn(),
    // FE-0020: the panel also drives the telemetry slice (tick snapshot for
    // the §17 state flow) — null fetch = never served, the honest branch.
    tick: null,
    tickError: null,
    fetchTick: vi.fn(),
  };
}

function render() {
  return renderToStaticMarkup(React.createElement(PairIntelligencePanel, { chainId: 1 }));
}

beforeEach(() => {
  seed({ pairs: null, status: "idle" });
});

describe("PairIntelligencePanel — SSR branches (FE-0017 · §13/§14)", () => {
  it("renders payload values verbatim: symbols, venue/pool counts, fee join, canonical a/b", () => {
    seed({ pairs: [PAIR_WETH_USDC, PAIR_WETH_DAI] });
    const html = render();
    expect(html).toContain("WETH/USDC");
    expect(html).toContain("WETH/DAI");
    // venue_count comes from the payload (no recomputation)
    expect(html).toMatch(/>3</);
    // distinct fee_bps joined ascending — payload values, no math
    expect(html).toContain("30 · 100");
    // count summary: 2 pares · 1 dirty
    expect(html).toContain("2 pares");
    expect(html).toContain("1 dirty");
  });

  it("alpha null renders the honest em-dash, never a recomputed spread (R8/§79)", () => {
    seed({ pairs: [PAIR_WETH_USDC] });
    const html = render();
    expect(html).toContain("—");
    expect(html).not.toContain("0.0000"); // a fabricated/recomputed alpha would look like this
  });

  it("dirty renders DIRTY badge, clean renders CLEAN (binary payload; FE-0020 upgrades)", () => {
    seed({ pairs: [PAIR_WETH_USDC, PAIR_WETH_DAI] });
    const html = render();
    expect(html).toContain("DIRTY");
    expect(html).toContain("CLEAN");
  });

  it("empty universe = honest empty message, not an error (entries: [])", () => {
    seed({ pairs: [] });
    const html = render();
    expect(html).toContain("universo efectivo vacío");
    expect(html).not.toContain("role=\"alert\"");
  });

  it("error state renders the endpoint reason verbatim", () => {
    seed({ pairs: null, status: "error", error: "HTTP 503: redis_unavailable" });
    const html = render();
    expect(html).toContain("HTTP 503: redis_unavailable");
    expect(html).toContain("role=\"alert\"");
  });

  it("idle/null renders the honest dash (never served)", () => {
    seed({ pairs: null, status: "idle" });
    const html = render();
    expect(html).toContain("—");
    expect(html).not.toContain("WETH/");
  });
});
