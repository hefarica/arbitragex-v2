// frontend/app/strategies/tabs/__tests__/StrategyDetailDrawer.test.tsx
//
// FE-MASTER · FE-0024 — strategy detail drawer, SSR-branch tests.
//
// The pure core (StrategyDetailBody) renders the FULL canon record of one
// strategy verbatim; the honest-gap block lists what the static catalog wire
// does NOT carry (financing modes, runtime KPIs/p95, per-chain active flag)
// so nothing is ever fabricated there (RULE 00 / §28 / R8). The Sheet wrapper
// owns open/close and stays untested here by design (Radix portal, repo node
// env — same split as PairDetailDrawer).
import React from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";

import { StrategyDetailBody } from "../StrategyDetailDrawer";
import type { StrategyCatalogRow } from "@/lib/apex/schemas";

const ROW: StrategyCatalogRow = {
  mev_id: "MEV-02-014",
  group: 7,
  name: "DEX–DEX arbitrage",
  family: "CROSS_VENUE",
  surface: "DEX_AMM",
  backend_module: "route_graph_engine",
  detector_id: "R_CLOSED_CYCLE",
  min_legs: 2,
  max_legs: 16,
  allowed_hops: [2, 3, 4, 5, 6, 7],
  graph_model: "TOKEN_MULTIGRAPH",
  quotebase_role: "PRIMARY_PAIR+NUMERAIRE",
  search_policy: "enumerate closed cycles over the effective universe",
  execution_class: "DETERMINISTIC_EXECUTABLE",
  primary_ops: ["op_27 Path Ordering", "op_16 Kelly Criterion"],
  discovery_equation: "x_{t+1} = f(x_t) - \\lambda_{friction}",
  gate_live: "net yield > gas + slippage",
  status: "ROUTE_READY",
};

function render(row: StrategyCatalogRow = ROW) {
  return renderToStaticMarkup(React.createElement(StrategyDetailBody, { row }));
}

describe("StrategyDetailBody — SSR branches (FE-0024 · §24)", () => {
  it("renders the canon record verbatim: identity, metadata, legs envelope", () => {
    const html = render();
    expect(html).toContain("MEV-02-014");
    expect(html).toContain("DEX–DEX arbitrage");
    expect(html).toContain("CROSS_VENUE");
    expect(html).toContain("route_graph_engine");
    expect(html).toContain("R_CLOSED_CYCLE");
    expect(html).toContain("TOKEN_MULTIGRAPH");
    expect(html).toContain("PRIMARY_PAIR+NUMERAIRE");
    expect(html).toContain("2–16");
    expect(html).toContain("2 · 3 · 4 · 5 · 6 · 7");
  });

  it("renders the workbook sentences verbatim: equation, search policy, gate", () => {
    const html = render();
    expect(html).toContain("x_{t+1} = f(x_t) - \\lambda_{friction}");
    expect(html).toContain("enumerate closed cycles over the effective universe");
    expect(html).toContain("net yield &gt; gas + slippage");
  });

  it("renders primary ops as chips and the dispatch status badge", () => {
    const html = render();
    expect(html).toContain("op_27 Path Ordering");
    expect(html).toContain("op_16 Kelly Criterion");
    expect(html).toContain("ROUTE_READY");
    expect(html).toContain("DETERMINISTIC_EXECUTABLE");
  });

  it("notes the runtime-7 cap is POLICY, not catalog metadata (legs canon up to max)", () => {
    const html = render();
    expect(html).toContain("cap runtime de 7 hops es política");
    expect(html).toContain("hasta 16");
  });

  it("honest gaps: financing / runtime KPIs (p95) / active flag are listed as NOT emitted — never fabricated", () => {
    const html = render();
    expect(html).toContain("No emitido en el catálogo");
    expect(html).toContain("Financing modes");
    expect(html).toContain("p95");
    expect(html).toContain("trading_config.enabled_strategies");
    // Nothing masquerading as a runtime value (a fabricated p95 ms, a fake
    // active toggle) — the gap block is prose, not data.
    expect(html).not.toMatch(/p95[^<]*\d+\s*ms/);
  });

  it("a non-READY status renders its own badge variant (NO_COMPATIBLE_ROUTE = destructive)", () => {
    const html = render({ ...ROW, mev_id: "MEV-03-001", status: "NO_COMPATIBLE_ROUTE" });
    expect(html).toContain("NO_COMPATIBLE_ROUTE");
    expect(html).toContain("destructive");
  });
});
