// frontend/components/opportunities/__tests__/OpportunitySummaryGrid.test.tsx
//
// FE-0033 (§36) — the canonical summary grid: every §36 field renders from
// its wire source or states its honest absence. Fixtures go through the real
// mapper (hop_count/semantic fields computed, never hand-set). §79: the Sim
// cell is a VALUE, never a PASS verdict — the wire persists none.
import React from "react";
import { describe, it, expect } from "vitest";
import { renderToStaticMarkup } from "react-dom/server";

import {
  OpportunitySummaryGrid,
  NOT_EMITTED,
} from "../OpportunitySummaryGrid";
import { mapToOmniOpportunity } from "@/lib/store/types";

const wire = (over: Record<string, unknown>) => ({
  id: "id",
  chain_id: 1,
  strategy_kind: "dex_arb",
  detected_at: "2026-08-11T00:00:00Z",
  status: "detected",
  trace_id: "t",
  dex_a: "uniswap-v2",
  dex_b: "sushiswap",
  token_in: "0xa",
  token_out: "0xb",
  block_number: 123,
  ...over,
});

const rm2hop = {
  dex_adapters: ["uniswap-v2", "sushiswap"],
  token_addresses: ["0xa", "0xb", "0xa"],
  pool_addresses: ["0xpool1", "0xpool2"],
};

describe("OpportunitySummaryGrid (§36)", () => {
  it("renders every §36 field wired to its real source", () => {
    const opp = mapToOmniOpportunity(
      wire({
        route_metadata: rm2hop,
        expected_profit_usd: 10.5,
        net_expected_profit_usd: 8.25,
        roi_pct: 0.12,
        risk_score: 0.34,
        amount_in_wei: "1500000000000000000",
        simulated_amount_in_usd: 4500,
        simulated_net_profit_usd: 7.9,
      }),
    );
    const html = renderToStaticMarkup(
      React.createElement(OpportunitySummaryGrid, { opp }),
    );
    for (const label of [
      "ruta",
      "strategy",
      "detector",
      "hops",
      "in",
      "Gross",
      "Net",
      "bps",
      "Risk",
      "Sim",
      "latencia",
    ]) {
      expect(html).toContain(label);
    }
    expect(html).toContain("uniswap-v2 → sushiswap");
    expect(html).toContain("dex_arb");
    expect(html).toContain("2"); // hops from route_metadata (FE-0028)
    expect(html).toContain("$10.50"); // Gross
    expect(html).toContain("$8.25"); // Net
    expect(html).toContain("12"); // bps = 0.12% × 100
    expect(html).toContain("0.34"); // Risk
    expect(html).toContain("$4500.00"); // in (simulated USD)
    expect(html).toContain("~$7.90"); // Sim VALUE
  });

  it("honest nulls: uncomputed economics render the dash, never a zero", () => {
    const opp = mapToOmniOpportunity(wire({ route_metadata: rm2hop }));
    const html = renderToStaticMarkup(
      React.createElement(OpportunitySummaryGrid, { opp }),
    );
    for (const absent of ["Gross", "Net", "bps", "Risk", "Sim", "in"]) {
      expect(html).toContain(absent);
    }
    // the dash cells carry the R8 titles
    expect(html).toContain("roi_pct no computado (R8)");
    expect(html).not.toContain("$0.00");
  });

  it("hops stays null without route_metadata — never the synthetic §29 count", () => {
    const opp = mapToOmniOpportunity(wire({ route_metadata: null }));
    const html = renderToStaticMarkup(
      React.createElement(OpportunitySummaryGrid, { opp }),
    );
    expect(html).toContain("jamás el conteo sintético §29");
  });

  it("gaps declared: detector and latencia are 'no emitido' (nivel-(b))", () => {
    const opp = mapToOmniOpportunity(wire({ route_metadata: rm2hop }));
    const html = renderToStaticMarkup(
      React.createElement(OpportunitySummaryGrid, { opp }),
    );
    expect(html).toContain(NOT_EMITTED);
    expect(html).toContain("ARBX-FE-EMIT-09"); // latencia gap points at its emission task
  });

  it("§79: the Sim cell carries a VALUE, never a PASS/FAIL verdict", () => {
    const opp = mapToOmniOpportunity(
      wire({ route_metadata: rm2hop, simulated_net_profit_usd: 3.2 }),
    );
    const html = renderToStaticMarkup(
      React.createElement(OpportunitySummaryGrid, { opp }),
    );
    expect(html).toContain("~$3.20");
    expect(html).toContain("el wire no persiste veredicto PASS/FAIL");
    // no verdict words in the Sim value cell
    expect(html).not.toContain("PASS</div>");
  });

  it("negative net keeps its sign and the $ formatting", () => {
    const opp = mapToOmniOpportunity(
      wire({ route_metadata: rm2hop, net_expected_profit_usd: -1.5 }),
    );
    const html = renderToStaticMarkup(
      React.createElement(OpportunitySummaryGrid, { opp }),
    );
    expect(html).toContain("-$1.50");
  });

  it("R1: pure render is byte-identical across invocations", () => {
    const opp = mapToOmniOpportunity(
      wire({ route_metadata: rm2hop, net_expected_profit_usd: 1 }),
    );
    const a = renderToStaticMarkup(
      React.createElement(OpportunitySummaryGrid, { opp }),
    );
    const b = renderToStaticMarkup(
      React.createElement(OpportunitySummaryGrid, { opp }),
    );
    expect(a).toBe(b);
  });
});
