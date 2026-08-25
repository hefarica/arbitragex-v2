// frontend/app/operations/components/__tests__/route-funnel.test.ts
//
// FE-MASTER · FE-0038 §46 — the route-discovery funnel stage model.
// Pure, framework-free: every stage binds to the wire field that carries it,
// null propagates honestly, and windows are tagged (tick counters and 24h
// totals never share a scale — the model DISCLOSES, the view never blends).
import { describe, expect, it } from "vitest";

import {
  buildRouteFunnelStages,
  FUNNEL_WINDOW_NOTE,
} from "../route-funnel";
import type { RouteDiscoveryTickSummary } from "@/lib/apex/schemas";
import type { OutcomeTotals } from "@/lib/hooks/useRouteDiscoveryOutcomes";

const TICK = {
  drain_drained: 183,
  dirty_seeds: 42,
  fe_prefilter_evaluated: 120,
  fe_prefilter_pass: 37,
  routes_found: 9,
  routes_dispatched: 4,
} as RouteDiscoveryTickSummary;

const OUTCOMES: OutcomeTotals = {
  total: 1200,
  opportunities: 3,
  with_reserves: null,
  profit_gt0: null,
  chains: null,
  cartridges: null,
};

describe("buildRouteFunnelStages — §46 stage model", () => {
  it("chains the nine stages in §46 order with the Reconciled terminus last", () => {
    const stages = buildRouteFunnelStages(TICK, OUTCOMES, { included: 12 });
    expect(stages.map((s) => s.id)).toEqual([
      "dirty_pools",
      "pairs",
      "evaluated",
      "prefilter",
      "routes",
      "dispatched",
      "outcomes",
      "opportunities",
      "reconciled",
    ]);
    expect(stages[8]!.label).toBe("Reconciled");
    expect(stages[8]!.source).toBe("recon.summary.totals.included");
  });

  it("carries each wire value verbatim — no derivation between stages", () => {
    const stages = buildRouteFunnelStages(TICK, OUTCOMES, { included: 12 });
    const by = (id: string) => stages.find((s) => s.id === id)!.value;
    expect(by("dirty_pools")).toBe(183);
    expect(by("pairs")).toBe(42);
    expect(by("evaluated")).toBe(120);
    expect(by("prefilter")).toBe(37);
    expect(by("routes")).toBe(9);
    expect(by("dispatched")).toBe(4);
    expect(by("outcomes")).toBe(1200);
    expect(by("opportunities")).toBe(3);
    expect(by("reconciled")).toBe(12);
  });

  it("tags the windows: six tick stages, three 24h stages", () => {
    const stages = buildRouteFunnelStages(TICK, OUTCOMES, { included: 12 });
    expect(stages.filter((s) => s.window === "tick")).toHaveLength(6);
    expect(stages.filter((s) => s.window === "24h")).toHaveLength(3);
    expect(FUNNEL_WINDOW_NOTE).toContain("NO comparten escala");
    expect(FUNNEL_WINDOW_NOTE).toContain("jamás un cero");
  });

  it("absent tick fields (knob OFF) and unavailable downstream wires stay null — never zero", () => {
    // Tick WITHOUT the prefilter group (knob OFF) and everything downstream absent.
    const stages = buildRouteFunnelStages({ drain_drained: 5 } as RouteDiscoveryTickSummary, null, null);
    const by = (id: string) => stages.find((s) => s.id === id)!.value;
    expect(by("dirty_pools")).toBe(5);
    expect(by("evaluated")).toBeNull();
    expect(by("prefilter")).toBeNull();
    expect(by("outcomes")).toBeNull();
    expect(by("reconciled")).toBeNull();
  });

  it("tick null (provider never accepted a payload) leaves the whole upstream half null", () => {
    const stages = buildRouteFunnelStages(null, OUTCOMES, { included: 1 });
    for (const s of stages.slice(0, 6)) expect(s.value).toBeNull();
    expect(stages[6]!.value).toBe(1200);
  });
});
