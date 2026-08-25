// frontend/features/routes-discovery/__tests__/pipeline.test.tsx
//
// FE-MASTER · FE-0026 — Market Event Pipeline model + header (§18).
//
// The pure model is framework-free (pipeline.ts); the header is props-in
// pure (MarketEventPipelineHeader.tsx). Both test directly under node env.
import React from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";

import {
  FILTERABLE_KPIS,
  HOP_CONTROL_RANGE,
  PIPELINE_KPIS,
  PIPELINE_KPI_IDS,
  filterRoutesByHops,
  filterRoutesByKpi,
  hopCounts,
  type PipelineKpiId,
} from "../pipeline";
import { MarketEventPipelineHeader } from "../MarketEventPipelineHeader";
import { HopControls } from "../HopControls";
import type { RouteDiscoveryTickSummary } from "@/lib/apex/schemas";

// A tick carrying every group the funnel reads (values chosen distinct so a
// mis-bound KPI fails, not silently shows a neighbour's number).
const TICK: RouteDiscoveryTickSummary = {
  drain_drained: 41,
  dirty_seeds: 17,
  fe_prefilter_evaluated: 88,
  fe_prefilter_pass: 52,
  multi_hop_hot_seed: "USDC|WETH",
  routes_found: 63,
  strategy_status_counts: { route_ready: 79, needs_route_data: 174, observe_only: 8, no_compatible_route: 3 },
};

describe("pipeline — §18 slots in doctrine order", () => {
  it("is exactly the ten §18 slots, in order", () => {
    expect(PIPELINE_KPI_IDS).toEqual([
      "dirty_pools",
      "pairs",
      "evaluated",
      "prefilter",
      "hot_seeds",
      "routes",
      "strategies",
      "sized",
      "net_positive",
      "sim_pass",
    ]);
  });

  it("binds each served slot to ITS wire field (distinct-value alarm)", () => {
    const by = (id: string) => PIPELINE_KPIS.find((k) => k.id === id)!.value(TICK);
    expect(by("dirty_pools")).toBe(41);
    expect(by("pairs")).toBe(17);
    expect(by("evaluated")).toBe(88);
    expect(by("prefilter")).toBe(52);
    expect(by("hot_seeds")).toBe("USDC|WETH"); // a NAME, not a count
    expect(by("routes")).toBe(63);
    expect(by("strategies")).toBe(79 + 174 + 8 + 3); // Σ census
  });

  it("the funnel tail is HONESTLY absent — never a zero (no wire today)", () => {
    const by = (id: string) => PIPELINE_KPIS.find((k) => k.id === id)!.value(TICK);
    expect(by("sized")).toBeNull();
    expect(by("net_positive")).toBeNull();
    expect(by("sim_pass")).toBeNull();
  });

  it("a knob-off tick (prefilter group absent) renders honest nulls for its slots", () => {
    const knobOff: RouteDiscoveryTickSummary = { drain_drained: 3 };
    const by = (id: string) => PIPELINE_KPIS.find((k) => k.id === id)!.value(knobOff);
    expect(by("evaluated")).toBeNull();
    expect(by("prefilter")).toBeNull();
    expect(by("pairs")).toBeNull(); // dirty_seeds not in this tick either
    expect(by("dirty_pools")).toBe(3); // present → served
  });

  it("null tick = every slot null (R8: absence is a state)", () => {
    for (const k of PIPELINE_KPIS) {
      if (k.id === "sized" || k.id === "net_positive" || k.id === "sim_pass") continue;
      expect(k.value(null), k.id).toBeNull();
    }
  });
});

describe("pipeline — filters (only real per-route predicates)", () => {
  const ROUTES = [
    { applicable_strategies: ["MEV-01-001"] },
    { applicable_strategies: [] },
    { applicable_strategies: ["MEV-03-004", "MEV-05-002"] },
  ];

  it("FILTERABLE_KPIS is exactly routes + strategies", () => {
    expect(FILTERABLE_KPIS).toEqual(["routes", "strategies"]);
  });

  it("null = no filter; routes = all; strategies = ≥1 applicable", () => {
    expect(filterRoutesByKpi(ROUTES, null)).toHaveLength(3);
    expect(filterRoutesByKpi(ROUTES, "routes")).toHaveLength(3);
    expect(filterRoutesByKpi(ROUTES, "strategies")).toHaveLength(2);
  });

  it("a non-filterable id is defensively a no-op (UI gates clickability)", () => {
    expect(filterRoutesByKpi(ROUTES, "dirty_pools")).toHaveLength(3);
  });
});

describe("MarketEventPipelineHeader — SSR render", () => {
  const render = (tick: RouteDiscoveryTickSummary | null, active: PipelineKpiId | null = null) =>
    renderToStaticMarkup(
      React.createElement(MarketEventPipelineHeader, {
        tick,
        active,
        onToggle: () => {},
      }),
    );

  it("renders all ten labels with their served values; absent slots dash", () => {
    const html = render(TICK);
    for (const k of PIPELINE_KPIS) {
      expect(html).toContain(k.label);
    }
    expect(html).toContain(">41<");
    expect(html).toContain(">17<");
    expect(html).toContain(">88<");
    expect(html).toContain(">52<");
    expect(html).toContain("USDC|WETH");
    expect(html).toContain(">63<");
    expect(html).toContain(String(79 + 174 + 8 + 3));
    // Honest dashes for the no-wire tail.
    expect(html.match(/>—</g)?.length).toBe(3);
  });

  it("null tick = ten honest dashes (what SSR + first client render produce)", () => {
    const html = render(null);
    expect(html.match(/>—</g)?.length).toBe(10);
  });

  it("aggregate slots are disabled (no fabricated filter); filterable are enabled", () => {
    const html = render(TICK);
    const buttons = html.match(/<button[^>]*disabled[^>]*>/g) ?? [];
    // The 8 non-filterable slots: 5 aggregates + 3 no-wire tail.
    expect(buttons.length).toBe(8);
    expect(html).toContain('aria-pressed="false"'); // filterable, inactive
  });

  it("active KPI renders pressed", () => {
    const html = render(TICK, "strategies");
    expect(html).toContain('aria-pressed="true"');
  });

  it("aggregate titles explain WHY there is no filter (§40)", () => {
    const html = render(TICK);
    expect(html).toContain("aggregate sin flag por-ruta: no se fabrica filtro");
  });

  it("two renders are byte-identical (R1)", () => {
    expect(render(TICK)).toBe(render(TICK));
  });
});

// ─── FE-0027 — hop view-controls (§19 §20 §63) ──────────────────────────────

describe("pipeline — hop model (§19)", () => {
  const ROUTES = [
    { hops: 2 },
    { hops: 2 },
    { hops: 3 },
    { hops: 5 },
    { hops: 9 }, // outside the control range: counted, not chip-filterable
  ];

  it("HOP_CONTROL_RANGE is the runtime policy 2..7", () => {
    expect(HOP_CONTROL_RANGE).toEqual([2, 3, 4, 5, 6, 7]);
  });

  it("hopCounts derives FROM the dataset (no registry pins)", () => {
    const c = hopCounts(ROUTES);
    expect(c.get(2)).toBe(2);
    expect(c.get(3)).toBe(1);
    expect(c.get(4)).toBeUndefined(); // never a fabricated 0 row
    expect(c.get(9)).toBe(1);
  });

  it("filterRoutesByHops: null/empty = all; a set matches only members", () => {
    expect(filterRoutesByHops(ROUTES, null)).toHaveLength(5);
    expect(filterRoutesByHops(ROUTES, new Set())).toHaveLength(5);
    expect(filterRoutesByHops(ROUTES, new Set([2]))).toHaveLength(2);
    expect(filterRoutesByHops(ROUTES, new Set([2, 5]))).toHaveLength(3);
    expect(filterRoutesByHops(ROUTES, new Set([7]))).toHaveLength(0);
  });
});

describe("HopControls — VIEW_FILTER vs runtime (§20 §63)", () => {
  const renderHops = (
    counts: ReadonlyMap<number, number>,
    active: ReadonlySet<number> | null,
    effectiveBounds: readonly [number, number] | null,
  ) =>
    renderToStaticMarkup(
      React.createElement(HopControls, {
        counts,
        active,
        onToggle: () => {},
        effectiveBounds,
      }),
    );

  it("renders exactly the h2..h7 chips with dataset-derived counts", () => {
    const html = renderHops(new Map([[2, 4], [5, 1]]), null, null);
    for (const h of HOP_CONTROL_RANGE) expect(html).toContain(`h${h}`);
    expect(html).toContain(">4<");
    expect(html).toContain(">1<");
  });

  it("carries the VIEW_FILTER label and the §20 mutation contract in the title", () => {
    const html = renderHops(new Map(), null, null);
    expect(html).toContain("VIEW_FILTER");
    expect(html).toContain("mutación de configuración con ACK");
  });

  it("shows the runtime EFFECTIVE bounds from the wire; null = honest dash", () => {
    expect(renderHops(new Map(), null, [2, 7])).toContain("[2,7]");
    expect(renderHops(new Map(), null, null)).toContain("runtime efectivo: —");
  });

  it("selected hops render pressed; unselected do not", () => {
    const on = renderHops(new Map(), new Set([3]), null);
    expect(on.match(/aria-pressed="true"/g)?.length).toBe(1);
  });

  it("two renders are byte-identical (R1)", () => {
    expect(renderHops(new Map([[2, 1]]), null, [2, 7])).toBe(
      renderHops(new Map([[2, 1]]), null, [2, 7]),
    );
  });
});
