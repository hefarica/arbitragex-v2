// frontend/components/opportunities/__tests__/WindowTotalSegment.test.tsx
//
// AUDIT-CARDS-MINOR (§2) — window_total (WO-H4 envelope) render contract:
//   - null/undefined  → NO segment (R8: absent ≠ 0, never invented)
//   - 0               → segment RENDERS (computed empty window — honest zero)
//   - a real count    → "N mostradas · W en ventana (≤5 min)"
// R1: pure prop-driven render — deterministic static markup.
import React from "react";
import { describe, it, expect } from "vitest";
import { renderToStaticMarkup } from "react-dom/server";

import { WindowTotalSegment } from "../WindowTotalSegment";

function html(props: { shown: number; windowTotal: number | null }): string {
  return renderToStaticMarkup(React.createElement(WindowTotalSegment, props));
}

describe("WindowTotalSegment — AUDIT-CARDS-MINOR (§2) fail-honest render", () => {
  it("windowTotal = null renders NOTHING (R8: absent is never invented as 0)", () => {
    expect(html({ shown: 50, windowTotal: null })).toBe("");
  });

  it("renders nothing before any snapshot carried the field (first paint / pre-WO-H4 edge)", () => {
    // the store's initial value is null — the counter line stays as-is
    expect(html({ shown: 0, windowTotal: null })).toBe("");
  });

  it("windowTotal = 0 RENDERS — computed empty window is honest exact zero", () => {
    const out = html({ shown: 0, windowTotal: 0 });
    expect(out).toContain("0 en ventana (≤5 min)");
    expect(out).toContain("0</span> mostradas");
  });

  it("a real window count pairs shown vs window", () => {
    const out = html({ shown: 50, windowTotal: 12155 });
    expect(out).toContain("50</span> mostradas");
    expect(out).toContain("12155 en ventana (≤5 min)");
  });

  it("carries the tooltip explaining shown-vs-window semantics", () => {
    const out = html({ shown: 3, windowTotal: 137 });
    expect(out).toContain("window_total");
    expect(out).toContain("COUNT over window");
  });

  it("R1: render is byte-identical across invocations (no clock/locale)", () => {
    const props = { shown: 7, windowTotal: 999 } as const;
    expect(html(props)).toBe(html(props));
  });
});
