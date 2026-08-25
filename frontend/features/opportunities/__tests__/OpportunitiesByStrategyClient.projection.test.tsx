// frontend/features/opportunities/__tests__/OpportunitiesByStrategyClient.projection.test.tsx
//
// FE-MASTER · FE-0039 §48/§49 — By Strategy client, SSR mount-state test.
// renderToStaticMarkup renders the MOUNT state (effects never run), so the
// store seam mock IS the registry state. Pins: the §49 provenance strip,
// the null-kind row landing in the honest unknown group (UNKNOWN badge, no
// dex_arb), registry chips verbatim on a matched group, and the join
// coverage in the summary. Fixtures via mapToOmniOpportunity.
import React from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { beforeEach, describe, expect, it, vi } from "vitest";

const storeState = vi.hoisted(() => ({ current: {} as Record<string, unknown> }));

vi.mock("@/lib/store/omni-store", () => ({
  useOmniStore: (selector: (s: Record<string, unknown>) => unknown) =>
    selector(storeState.current),
}));

import { OpportunitiesByStrategyClient } from "../OpportunitiesByStrategyClient";
import { mapToOmniOpportunity, type OmniOpportunity } from "@/lib/store/types";
import type { StrategyCatalogRow } from "@/lib/apex/schemas";

function opp(raw: Record<string, unknown>): OmniOpportunity {
  return mapToOmniOpportunity({
    id: "1",
    token_in: "0xAAA",
    token_out: "0xBBB",
    strategy_kind: "triangular",
    ...raw,
  });
}

function catalogRow(mev_id: string): StrategyCatalogRow {
  return {
    mev_id,
    group: 1,
    name: "Closed Cycle Exhaustive",
    family: "TOPOLOGICAL_TRIPLE",
    surface: "dex-amm pool graph",
    backend_module: "mod",
    detector_id: "R_CLOSED_CYCLE",
    min_legs: 2,
    max_legs: 3,
    allowed_hops: [2, 3],
    graph_model: "EXHAUSTIVE_2",
    quotebase_role: "core",
    search_policy: "policy",
    execution_class: "DETERMINISTIC_EXECUTABLE",
    primary_ops: ["op_01"],
    discovery_equation: "eq",
    gate_live: "gate",
  } as StrategyCatalogRow;
}

function render(opps: OmniOpportunity[]) {
  return renderToStaticMarkup(
    React.createElement(OpportunitiesByStrategyClient, { initialOpportunities: opps }),
  );
}

beforeEach(() => {
  storeState.current = {
    strategyByMevId: new Map([["MEV-01-015", catalogRow("MEV-01-015")]]),
    fetchStrategyCatalog: vi.fn(),
  };
});

describe("OpportunitiesByStrategyClient — §48/§49 mount state", () => {
  it("renders the §49 provenance strip (projection of the Exchange Feed)", () => {
    const html = render([]);
    expect(html).toContain('data-testid="by-strategy-provenance"');
    expect(html).toContain("Proyección del Exchange Feed");
    expect(html).toContain("sin universo propio de estrategias");
  });

  it("a matched cartridge group renders the registry chips verbatim + the join coverage", () => {
    const html = render([opp({ id: "a", cartridge_id: "MEV-01-015", strategy_kind: null })]);
    expect(html).toContain("MEV-01-015");
    expect(html).toContain("Closed Cycle Exhaustive");
    expect(html).toContain("R_CLOSED_CYCLE");
    expect(html).toContain("DETERMINISTIC_EXECUTABLE");
    expect(html).toContain("Registry join:");
    expect(html).toContain("matched");
  });

  it("a null-identity row renders the UNKNOWN group — never dex_arb", () => {
    const html = render([opp({ id: "a", strategy_kind: null, cartridge_id: null })]);
    expect(html).toContain("unknown — payload sin strategy_id ni cartridge_id");
    expect(html).toContain("UNKNOWN");
    expect(html).not.toContain("DEX Arbitrage");
  });

  it("an unmatched cartridge renders the honest drift line, not borrowed metadata", () => {
    const html = render([opp({ id: "a", cartridge_id: "MEV-99-999", strategy_kind: null })]);
    expect(html).toContain("cartridge sin match en el registry");
    expect(html).not.toContain("Closed Cycle Exhaustive");
  });
});
