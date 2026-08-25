// frontend/app/strategies/tabs/__tests__/StrategyHopMatrix.test.tsx
//
// FE-MASTER · FE-0023 — Strategy×Hop matrix, SSR-branch tests.
//
// Pure presentational component (props in, markup out — no store seam).
// renderToStaticMarkup asserts the deterministic branches:
//   - cell = MEMBERSHIP of the payload's already-expanded allowed_hops
//     (never a mask decode, never a computed matrix — §23/§79);
//   - columns are DERIVED from the payload's distinct hops (never a pinned
//     [2..7] — a re-ingested canon could change the envelope);
//   - allowed cells carry the status tint + full title; absent hops render
//     the dashed hole (the absence IS the data, R8);
//   - empty row set renders the honest message, not an error.
import React from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it, vi } from "vitest";

import { StrategyHopMatrix } from "../StrategyHopMatrix";
import type { StrategyCatalogRow } from "@/lib/apex/schemas";

function row(
  mev_id: string,
  allowed_hops: number[],
  status: StrategyCatalogRow["status"],
): StrategyCatalogRow {
  return {
    mev_id,
    group: 1,
    name: `Strategy ${mev_id}`,
    family: "FAM",
    surface: "DEX_AMM",
    backend_module: "route_graph_engine",
    detector_id: "R_CLOSED_CYCLE",
    min_legs: 2,
    max_legs: 4,
    allowed_hops,
    graph_model: "TOKEN_MULTIGRAPH",
    quotebase_role: "PRIMARY_PAIR+NUMERAIRE",
    search_policy: "enumerate closed cycles",
    execution_class: "DETERMINISTIC_EXECUTABLE",
    primary_ops: ["op_27 Path Ordering"],
    discovery_equation: "x_{t+1} = f(x_t)",
    gate_live: "net yield > gas + slippage",
    status,
  };
}

function render(rows: StrategyCatalogRow[]) {
  return renderToStaticMarkup(
    React.createElement(StrategyHopMatrix, {
      rows,
      onRowSelect: vi.fn(),
      selectedId: null,
    }),
  );
}

describe("StrategyHopMatrix — SSR branches (FE-0023 · §23)", () => {
  it("columns are the DISTINCT hops the payload carries, sorted — never a pinned envelope", () => {
    const html = render([
      row("MEV-01-001", [2, 3], "ROUTE_READY"),
      row("MEV-01-002", [5], "OBSERVE_ONLY"),
    ]);
    // Derived column set {2,3,5}: every one is a header, and NO other hop is.
    expect(html).toContain(">h2<");
    expect(html).toContain(">h3<");
    expect(html).toContain(">h5<");
    expect(html).not.toContain(">h4<");
    expect(html).not.toContain(">h6<");
    expect(html).not.toContain(">h7<");
  });

  it("cell = membership of allowed_hops: allowed cells carry the status tint + full title", () => {
    const html = render([row("MEV-01-001", [2, 3], "ROUTE_READY")]);
    // The allowed cells are tinted by dispatch status and titled with hop+status.
    expect(html.match(/bg-success\/25/g)?.length).toBe(2);
    expect(html).toContain("MEV-01-001 · hop 2 permitido · ROUTE_READY");
    expect(html).toContain("MEV-01-001 · hop 3 permitido · ROUTE_READY");
  });

  it("absent hops render the dashed hole — the absence IS the data (R8), never a gray zero", () => {
    // Column h5 must EXIST for the hole to render: a second row carries hop 5
    // so the derived column set is {2,3,5}; row 001 does not authorize 5.
    const html = render([
      row("MEV-01-001", [2, 3], "ROUTE_READY"),
      row("MEV-01-002", [2, 5], "OBSERVE_ONLY"),
    ]);
    expect(html).toContain("MEV-01-001 · hop 5 NO permitido por el canon");
    expect(html).toContain("border-dashed");
    // and it is NOT tinted as any status.
    expect(html.match(/aria-label="MEV-01-001 hop 5 permitido/g)).toBe(null);
  });

  it("every row renders its MEV_ID and clickable name (§24 drawer entry)", () => {
    const html = render([row("MEV-01-001", [2], "ROUTE_READY")]);
    expect(html).toContain("MEV-01-001");
    expect(html).toContain("Strategy MEV-01-001");
    expect(html).toContain("title=\"Detalle MEV-01-001 (§24)\"");
  });

  it("empty row set renders the honest message, not an error", () => {
    const html = render([]);
    expect(html).toContain("Sin filas que mostrar");
    expect(html).not.toContain("<table");
    expect(html).not.toContain("role=\"alert\"");
  });

  it("caption pins the contract: expanded hops, FE never decodes bits or computes the matrix", () => {
    const html = render([row("MEV-01-001", [2], "ROUTE_READY")]);
    expect(html).toContain("nunca decodifica bits ni calcula la matriz");
    expect(html).toContain("§23/§79");
  });
});
