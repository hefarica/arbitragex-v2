// frontend/app/operations/components/__tests__/LatencyBudgetCard.test.tsx
//
// ARBX-QB-07-008 (REQ-QB-015, workbook 10_LATENCY) — pure presentational
// card, SSR-branch tests (repo pattern: node env, renderToStaticMarkup).
//
// Pins the panel's contract:
//   - one row per lat.* stage in WIRE order (backend-fixed) with the
//     workbook columns Target/p50/p95/Headroom (µs→ms fixed-2; p90/p99 ride
//     the wire but are NOT 10_LATENCY columns — they must NOT appear);
//   - R8: null percentile → "—", never 0;
//   - headroom sign ALWAYS visible; over-budget renders destructive;
//   - lat.total emphasized (PASS_p95 is decided on it);
//   - PASS_p95 chip truth table (true/false/null = no completed cycles);
//   - honest absence note when no rows; error verbatim (role=alert) with no
//     fabricated table.
import React from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";

import type { LatencyStageRow } from "@/lib/apex/schemas";
import { LatencyBudgetCard } from "../LatencyBudgetCard";
import { headroomMsText, usToMsText } from "../latency-budget";

const row = (over: Partial<LatencyStageRow>): LatencyStageRow => ({
  key: "lat.decode",
  target_ms: 2,
  p50_us: 12340,
  p90_us: 15000,
  p95_us: 18000,
  p99_us: 20000,
  headroom_p95_us: 200,
  ...over,
});

const STAGES: LatencyStageRow[] = [
  row({ key: "lat.decode", p95_us: 18000, headroom_p95_us: 200 }),
  row({ key: "lat.state", target_ms: 4, p50_us: 2000, p95_us: 35000, headroom_p95_us: -5000 }),
  // lat.refine today: amount-aware pass not wired in the worker → nulls.
  row({
    key: "lat.refine",
    target_ms: 8,
    p50_us: null,
    p90_us: null,
    p95_us: null,
    p99_us: null,
    headroom_p95_us: null,
  }),
  row({ key: "lat.total", target_ms: 30, p50_us: 22000, p95_us: 28000, headroom_p95_us: 2000 }),
];

describe("LatencyBudgetCard — rows (10_LATENCY columns)", () => {
  it("renders one row per stage in wire order with µs→ms fixed-2 values", () => {
    const html = renderToStaticMarkup(
      <LatencyBudgetCard stages={STAGES} passP95={true} cycles={42} error={null} />,
    );
    // Cell-anchored: the caption mentions lat.total, so anchor on the
    // table-cell pattern (>key<), never the bare key.
    const order = ["lat.decode", "lat.state", "lat.refine", "lat.total"].map(
      (k) => html.indexOf(`>${k}<`),
    );
    expect(order.every((i) => i >= 0)).toBe(true);
    expect(order).toEqual([...order].sort((a, b) => a - b));
    expect(html).toContain("12.34"); // 12340 µs → p50 of lat.decode
    expect(html).toContain("35.00"); // 35000 µs → p95 of lat.state
    expect(html).toContain(">30<"); // target_ms of lat.total
  });

  it("keeps p90/p99 OFF the display — they are not 10_LATENCY columns", () => {
    const html = renderToStaticMarkup(
      <LatencyBudgetCard stages={STAGES} passP95={true} cycles={1} error={null} />,
    );
    expect(html).not.toContain(">p90");
    expect(html).not.toContain(">p99");
  });

  it("null percentile renders the honest dash, never a zero (R8)", () => {
    const html = renderToStaticMarkup(
      <LatencyBudgetCard stages={STAGES} passP95={null} cycles={0} error={null} />,
    );
    // lat.refine row: p50/p95/headroom all null → three dashes in its row.
    const refineRow = html.slice(
      html.indexOf(">lat.refine<"),
      html.indexOf(">lat.total<"),
    );
    expect(refineRow.match(/—/g)).toHaveLength(3);
    expect(refineRow).not.toContain("0.00");
    // No fabricated raw-µs title on a null cell.
    expect(refineRow).not.toContain("µs raw");
  });

  it("headroom sign is always visible; over-budget renders destructive", () => {
    const html = renderToStaticMarkup(
      <LatencyBudgetCard stages={STAGES} passP95={false} cycles={7} error={null} />,
    );
    expect(html).toContain("+0.20"); // lat.decode headroom 200 µs
    expect(html).toContain("-5.00"); // lat.state headroom -5000 µs
    const stateIdx = html.indexOf("lat.state");
    const stateRow = html.slice(stateIdx, html.indexOf("lat.refine"));
    expect(stateRow).toContain("text-destructive");
    const decodeRow = html.slice(html.indexOf("lat.decode"), stateIdx);
    expect(decodeRow).not.toContain("text-destructive");
  });

  it("emphasizes the lat.total row (the PASS_p95 row)", () => {
    const html = renderToStaticMarkup(
      <LatencyBudgetCard stages={STAGES} passP95={true} cycles={9} error={null} />,
    );
    expect(html).toContain('font-semibold">lat.total<');
  });

  it("carries the raw µs in the cell title for precision", () => {
    const html = renderToStaticMarkup(
      <LatencyBudgetCard stages={STAGES} passP95={true} cycles={1} error={null} />,
    );
    expect(html).toContain('title="12340 µs raw"');
    expect(html).toContain('title="18000 µs raw"');
  });
});

describe("LatencyBudgetCard — PASS_p95 chip truth table", () => {
  it("null → no completed cycles yet (honest, not FAIL)", () => {
    const html = renderToStaticMarkup(
      <LatencyBudgetCard stages={STAGES} passP95={null} cycles={0} error={null} />,
    );
    expect(html).toContain("PASS_p95: no completed cycles yet");
    expect(html).not.toContain("FAIL_p95");
  });

  it("true → PASS_p95; false → FAIL_p95 with the destructive tone", () => {
    const pass = renderToStaticMarkup(
      <LatencyBudgetCard stages={STAGES} passP95={true} cycles={5} error={null} />,
    );
    expect(pass).toContain("PASS_p95");
    const fail = renderToStaticMarkup(
      <LatencyBudgetCard stages={STAGES} passP95={false} cycles={5} error={null} />,
    );
    expect(fail).toContain("FAIL_p95 — lat.total over SLA");
    expect(fail.match(/FAIL_p95[^<]*/)?.[0]).toBeDefined();
  });

  it("renders the completed-cycle count the window aggregates", () => {
    const html = renderToStaticMarkup(
      <LatencyBudgetCard stages={STAGES} passP95={true} cycles={4242} error={null} />,
    );
    expect(html).toContain("4242");
  });
});

describe("LatencyBudgetCard — honest absence (R8)", () => {
  it("null stages → the absence note, no fabricated table", () => {
    const html = renderToStaticMarkup(
      <LatencyBudgetCard stages={null} passP95={null} cycles={0} error={null} />,
    );
    expect(html).toContain("lat.* not served in this snapshot");
    expect(html).not.toContain("<table");
  });

  it("empty stages array → the same absence note (absence ≠ zero rows)", () => {
    const html = renderToStaticMarkup(
      <LatencyBudgetCard stages={[]} passP95={null} cycles={0} error={null} />,
    );
    expect(html).toContain("lat.* not served in this snapshot");
  });

  it("error renders verbatim (role=alert) with no table", () => {
    const html = renderToStaticMarkup(
      <LatencyBudgetCard stages={null} passP95={null} cycles={0} error="HTTP 503: tick not published yet" />,
    );
    expect(html).toContain('role="alert"');
    expect(html).toContain("HTTP 503: tick not published yet");
    expect(html).not.toContain("<table");
  });
});

describe("latency-budget pure helpers", () => {
  it("usToMsText: null/undefined pass through as the dash; 0 is a real zero", () => {
    expect(usToMsText(null)).toBe("—");
    expect(usToMsText(undefined)).toBe("—");
    expect(usToMsText(0)).toBe("0.00");
    expect(usToMsText(12340)).toBe("12.34");
  });

  it("headroomMsText: sign always visible on both sides of zero", () => {
    expect(headroomMsText(null)).toBe("—");
    expect(headroomMsText(200)).toBe("+0.20");
    expect(headroomMsText(-5000)).toBe("-5.00");
    expect(headroomMsText(0)).toBe("+0.00");
  });
});
