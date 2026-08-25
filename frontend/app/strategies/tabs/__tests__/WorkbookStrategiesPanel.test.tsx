// frontend/app/strategies/tabs/__tests__/WorkbookStrategiesPanel.test.tsx
//
// FE-MASTER · FE-0021/FE-0022 — workbook strategies panel, SSR-branch tests.
//
// Store seam: `useOmniStore` is mocked at the module boundary (the slice's
// fetch-once semantics are pinned by catalog-slices tests of 7b). What THESE
// tests pin is the panel's contract over whatever the slice serves:
//   - rows render payload values verbatim with the §22 status badges;
//   - status chips carry counts DERIVED from the payload (never the pinned
//     workbook 79/174/8/3 — a re-ingestion may change them);
//   - error renders verbatim (role=alert); null catalog renders the dash;
//   - empty entries render the honest empty note, not a fake error;
//   - filterStrategies (pure) — text + status subsetting, empty set = all.
import React from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { beforeEach, describe, expect, it, vi } from "vitest";

const storeState = vi.hoisted(() => ({ current: {} as Record<string, unknown> }));

vi.mock("@/lib/store/omni-store", () => ({
  useOmniStore: (selector: (s: Record<string, unknown>) => unknown) =>
    selector(storeState.current),
}));

import { WorkbookStrategiesPanel, filterStrategies } from "../WorkbookStrategiesPanel";
import type { StrategyCatalogRow } from "@/lib/apex/schemas";

function row(partial: Partial<StrategyCatalogRow>): StrategyCatalogRow {
  return {
    mev_id: "MEV-01-001",
    group: 1,
    name: "DEX–DEX arbitrage",
    family: "CROSS_VENUE",
    surface: "DEX_AMM",
    backend_module: "route_graph_engine",
    detector_id: "R_CLOSED_CYCLE",
    min_legs: 2,
    max_legs: 4,
    allowed_hops: [2, 3],
    graph_model: "TOKEN_MULTIGRAPH",
    quotebase_role: "PRIMARY_PAIR+NUMERAIRE",
    search_policy: "enumerate closed cycles",
    execution_class: "DETERMINISTIC_EXECUTABLE",
    primary_ops: ["op_27 Path Ordering"],
    discovery_equation: "x_{t+1} = f(x_t)",
    gate_live: "net yield > gas + slippage",
    status: "ROUTE_READY",
    ...partial,
  };
}

const ROWS: StrategyCatalogRow[] = [
  row({}),
  row({
    mev_id: "MEV-02-002",
    name: "CEX–DEX dispersion",
    detector_id: "R_FLOW_DIVERGENCE",
    status: "NEEDS_ROUTE_DATA",
  }),
  row({
    mev_id: "MEV-03-003",
    name: "Oracle-latency observation",
    detector_id: "R_ORACLE_DRIFT",
    status: "OBSERVE_ONLY",
    execution_class: "OBSERVE_ONLY",
  }),
];

function seed(partial: {
  rows?: StrategyCatalogRow[] | null;
  status?: string;
  error?: string | null;
}) {
  storeState.current = {
    strategyCatalog: partial.rows ?? null,
    strategyCatalogStatus: partial.status ?? "ready",
    strategyCatalogError: partial.error ?? null,
    fetchStrategyCatalog: vi.fn(),
  };
}

function render() {
  return renderToStaticMarkup(React.createElement(WorkbookStrategiesPanel));
}

beforeEach(() => {
  seed({ rows: null, status: "idle" });
});

describe("WorkbookStrategiesPanel — SSR branches (FE-0021/0022 · §21/§22)", () => {
  it("renders served rows verbatim: MEV_ID, name, detector, legs envelope, hops join", () => {
    seed({ rows: ROWS });
    const html = render();
    expect(html).toContain("MEV-01-001");
    expect(html).toContain("DEX–DEX arbitrage");
    expect(html).toContain("CEX–DEX dispersion");
    expect(html).toContain("R_FLOW_DIVERGENCE");
    expect(html).toContain("2–4");
    expect(html).toContain("2·3");
    expect(html).toContain("3 de 3 filas servidas");
  });

  it("§22: all four dispatch states render as badges; chips show PAYLOAD-derived counts", () => {
    seed({ rows: ROWS });
    const html = render();
    expect(html).toContain("ROUTE_READY");
    expect(html).toContain("NEEDS_ROUTE_DATA");
    expect(html).toContain("OBSERVE_ONLY");
    // Counts derived from the served rows (1/1/1 — NOT the workbook 79/174/8/3):
    expect(html).toContain("ROUTE_READY 1");
    expect(html).toContain("NEEDS_ROUTE_DATA 1");
    expect(html).toContain("OBSERVE_ONLY 1");
    expect(html).toContain("NO_COMPATIBLE_ROUTE 0");
  });

  it("error state renders the endpoint reason verbatim", () => {
    seed({ rows: null, status: "error", error: "HTTP 503: catalog unavailable" });
    const html = render();
    expect(html).toContain("HTTP 503: catalog unavailable");
    expect(html).toContain("role=\"alert\"");
  });

  it("idle/null renders the honest dash (catalog never fetched)", () => {
    seed({ rows: null, status: "idle" });
    const html = render();
    expect(html).toContain("—");
    expect(html).not.toContain("MEV-01-001");
  });

  it("entries: [] renders the honest empty note — the zero is served, not an error", () => {
    seed({ rows: [] });
    const html = render();
    expect(html).toContain("Catálogo servido vacío");
    expect(html).toContain("entries: []");
    expect(html).not.toContain("role=\"alert\"");
  });

  it("default view is the LIST (no matrix table cell tints render)", () => {
    seed({ rows: ROWS });
    const html = render();
    expect(html).toContain("Lista");
    expect(html).toContain("Matriz ×hop");
    // The matrix view is NOT active by default — no status-tinted matrix cells.
    expect(html).not.toContain("bg-success/25");
  });
});

describe("filterStrategies — pure presentation filter", () => {
  it("empty query + empty status set = all rows pass (payload order untouched)", () => {
    expect(filterStrategies(ROWS, "", new Set())).toEqual(ROWS);
  });

  it("text matches mev_id / name / detector (case-insensitive)", () => {
    expect(filterStrategies(ROWS, "mev-02", new Set())).toEqual([ROWS[1]]);
    expect(filterStrategies(ROWS, "dispersion", new Set())).toEqual([ROWS[1]]);
    expect(filterStrategies(ROWS, "R_ORACLE_DRIFT", new Set())).toEqual([ROWS[2]]);
    expect(filterStrategies(ROWS, "nomatch", new Set())).toEqual([]);
  });

  it("status set filters by dispatch status; empty set means NO status filter", () => {
    const only = new Set(["ROUTE_READY"] as const);
    expect(filterStrategies(ROWS, "", only)).toEqual([ROWS[0]]);
    const two = new Set(["NEEDS_ROUTE_DATA", "OBSERVE_ONLY"] as const);
    expect(filterStrategies(ROWS, "", two)).toEqual([ROWS[1], ROWS[2]]);
  });

  it("text AND status compose (both must hold)", () => {
    const only = new Set(["NEEDS_ROUTE_DATA", "OBSERVE_ONLY"] as const);
    expect(filterStrategies(ROWS, "oracle", only)).toEqual([ROWS[2]]);
    expect(filterStrategies(ROWS, "dispersion", only)).toEqual([ROWS[1]]);
  });
});
