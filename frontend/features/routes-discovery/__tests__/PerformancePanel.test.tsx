// frontend/features/routes-discovery/__tests__/PerformancePanel.test.tsx
//
// FE-MASTER · FE-0036 — Route Discovery Performance panel, SSR-branch tests.
//
// Pure-props panel over the provider-fed tick (same split as the §18 header):
// the nine 10_LATENCY stage rows with target_ms and windowed percentiles,
// rendered verbatim from the wire (µs shown as ms via the exact ÷1000), and
// the §44 PASS rule — lat_pass_p95 is the ONLY verdict authority; null is
// "SIN PASS — muestra insuficiente", never PASS and never FAIL.
import React from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";

import {
  PerformancePanel,
  STAGE_ORDER,
  formatHeadroom,
  usToMs,
} from "../PerformancePanel";
import type { LatencyStageRow, RouteDiscoveryTickSummary } from "@/lib/apex/schemas";

function stage(partial: Partial<LatencyStageRow>): LatencyStageRow {
  return {
    key: "lat.decode",
    target_ms: 5,
    p50_us: 1_500,
    p90_us: 2_000,
    p95_us: 2_500,
    p99_us: 3_000,
    headroom_p95_us: 2_500,
    ...partial,
  };
}

function tickWith(
  rows: LatencyStageRow[],
  extra: Partial<RouteDiscoveryTickSummary> = {},
): RouteDiscoveryTickSummary {
  return { lat_stages: rows, lat_pass_p95: null, lat_cycles: 3, ...extra };
}

function render(tick: RouteDiscoveryTickSummary | null) {
  return renderToStaticMarkup(React.createElement(PerformancePanel, { tick }));
}

// The nine workbook stages, wire order scrambled to prove the panel folds
// them back into the canonical 10_LATENCY order.
const SCRAMBLED = [
  stage({ key: "lat.emit" }),
  stage({ key: "lat.decode" }),
  stage({ key: "lat.total", target_ms: 30, p50_us: 9_000, headroom_p95_us: 500 }),
  stage({ key: "lat.state" }),
  stage({ key: "lat.pair" }),
  stage({ key: "lat.refine" }),
  stage({ key: "lat.expand" }),
  stage({ key: "lat.gates" }),
  stage({ key: "lat.reprice" }),
];

describe("PerformancePanel — SSR branches (FE-0036 · §43/§44)", () => {
  it("renders the nine stage rows in canonical 10_LATENCY order regardless of wire order", () => {
    const html = render(tickWith(SCRAMBLED));
    const idx = (k: string) => html.indexOf(`>${k}</td>`);
    expect(STAGE_ORDER.length).toBe(9);
    for (const k of STAGE_ORDER) expect(idx(k)).toBeGreaterThanOrEqual(0);
    // decode first, total last — the canonical envelope.
    expect(idx("lat.decode")).toBeLessThan(idx("lat.state"));
    expect(idx("lat.refine")).toBeLessThan(idx("lat.gates"));
    expect(idx("lat.total")).toBeGreaterThan(idx("lat.emit"));
  });

  it("renders wire values verbatim: target in ms, percentiles µs→ms exact", () => {
    const html = render(tickWith([stage({ target_ms: 5, p50_us: 1_500, p95_us: 2_500 })]));
    expect(html).toContain(">5</td>");
    expect(html).toContain(usToMs(1_500)); // 1.500
    expect(html).toContain(usToMs(2_500)); // 2.500
    expect(usToMs(1_500)).toBe("1.500");
    expect(usToMs(0)).toBe("0.000");
  });

  it("a percentile without samples renders the honest dash — never a zero (R8)", () => {
    const html = render(
      tickWith([stage({ p50_us: null, p90_us: null, p95_us: null, p99_us: null, headroom_p95_us: null })]),
    );
    // four percentile cells + headroom = five dashes in that row's cells
    expect((html.match(/>—<\/td>/g) ?? []).length).toBe(5);
    expect(html).not.toContain("0.000</td>");
  });

  it("headroom renders signed: +over-budget-free, -over-budget in text (color is never the only signal)", () => {
    const html = render(tickWith([stage({ headroom_p95_us: 250 }), stage({ key: "lat.gates", headroom_p95_us: -500 })]));
    expect(html).toContain("+250");
    expect(html).toContain("-500");
    expect(html).toContain("text-destructive");
    expect(formatHeadroom(250)).toBe("+250");
    expect(formatHeadroom(-500)).toBe("-500");
    expect(formatHeadroom(0)).toBe("0");
  });

  it("§44: lat_pass_p95 null renders SIN PASS — muestra insuficiente, and the cycle count", () => {
    const html = render(tickWith(SCRAMBLED, { lat_pass_p95: null, lat_cycles: 3 }));
    expect(html).toContain("SIN PASS — muestra insuficiente (§44)");
    expect(html).toContain("ciclos completados: 3");
    expect(html).not.toContain("PASS p95 vs SLA");
    expect(html).not.toContain("FAIL p95 vs SLA");
  });

  it("§44: the verdict authority is the wire boolean — true PASS / false FAIL", () => {
    expect(render(tickWith(SCRAMBLED, { lat_pass_p95: true, lat_cycles: 41 }))).toContain("PASS p95 vs SLA");
    expect(render(tickWith(SCRAMBLED, { lat_pass_p95: false, lat_cycles: 41 }))).toContain("FAIL p95 vs SLA");
  });

  it("a tick WITHOUT the latency group renders the honest absence note — no empty table", () => {
    const html = render({ routes_dispatched: 4 } as RouteDiscoveryTickSummary);
    expect(html).toContain("grupo lat.* ausente");
    expect(html).not.toContain("<th");
  });

  it("tick null (nothing accepted yet) renders the honest dash", () => {
    const html = render(null);
    expect(html).toContain("—");
    expect(html).not.toContain("<th");
  });

  it("lat.total rides as the emphasized summary row", () => {
    const html = render(tickWith(SCRAMBLED));
    expect(html).toMatch(/border-t font-semibold/);
  });

  it("the unit note discloses the µs wire and the ÷1000 display", () => {
    const html = render(tickWith(SCRAMBLED));
    expect(html).toContain("µs, mostrados en ms (÷1000 exacto)");
    expect(html).toContain("Headroom p95 (µs)");
  });
});
