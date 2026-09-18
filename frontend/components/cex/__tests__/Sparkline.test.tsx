// frontend/components/cex/__tests__/Sparkline.test.tsx
//
// (c) WO-PRICE-EXCHANGE-V1 — the sparkline renders a polyline with exactly N
// points, pure inline SVG (no chart library), and stays deterministic so SSR
// markup is byte-stable.
import React from "react";
import { describe, expect, it } from "vitest";
import { renderToStaticMarkup } from "react-dom/server";

import { Sparkline } from "../Sparkline";

function polylinePoints(html: string): string[] {
  const m = html.match(/<polyline[^>]*points="([^"]*)"/);
  expect(m, "sparkline markup contains a <polyline> with points").toBeTruthy();
  return m![1]!.trim().split(/\s+/);
}

describe("Sparkline — inline SVG mini-trend", () => {
  it("renders a polyline with exactly N coordinate pairs for N points", () => {
    const html = renderToStaticMarkup(
      React.createElement(Sparkline, { points: [1, 2, 3, 4, 5] }),
    );
    const pts = polylinePoints(html);
    expect(pts).toHaveLength(5);
    // each pair is a deterministic x,y coordinate
    for (const p of pts) expect(p).toMatch(/^\d+(\.\d+)?,\d+(\.\d+)?$/);
    expect(html).toContain("data-points=\"5\"");
  });

  it("also fills a soft polygon under the line (premium fill, no library)", () => {
    const html = renderToStaticMarkup(
      React.createElement(Sparkline, { points: [3, 1, 4] }),
    );
    expect(html).toMatch(/<polygon[^>]*fill="currentColor"/);
  });

  it("rising series is success-toned, falling series destructive-toned", () => {
    const up = renderToStaticMarkup(React.createElement(Sparkline, { points: [1, 2, 3] }));
    const down = renderToStaticMarkup(React.createElement(Sparkline, { points: [3, 2, 1] }));
    expect(up).toContain("text-success");
    expect(down).toContain("text-destructive");
  });

  it("flat series renders a midline without dividing by zero", () => {
    const html = renderToStaticMarkup(
      React.createElement(Sparkline, { points: [7, 7, 7, 7] }),
    );
    const pts = polylinePoints(html);
    expect(pts).toHaveLength(4);
    // all y identical (the midline)
    expect(new Set(pts.map((p) => p.split(",")[1])).size).toBe(1);
  });

  it("fewer than 2 finite points → honest placeholder, no fabricated line", () => {
    expect(
      renderToStaticMarkup(React.createElement(Sparkline, { points: [1] })),
    ).toContain("sparkline-empty");
    expect(
      renderToStaticMarkup(React.createElement(Sparkline, { points: undefined })),
    ).toContain("sparkline-empty");
    expect(
      renderToStaticMarkup(React.createElement(Sparkline, { points: [1, NaN, Infinity] })),
    ).toContain("sparkline-empty");
  });

  it("non-finite values are dropped, not plotted", () => {
    const html = renderToStaticMarkup(
      React.createElement(Sparkline, { points: [1, NaN, 2, Infinity, 3] }),
    );
    expect(polylinePoints(html)).toHaveLength(3);
  });

  it("R1: deterministic — identical props render byte-identical markup", () => {
    const pts = [4, 2, 6, 3, 5];
    expect(
      renderToStaticMarkup(React.createElement(Sparkline, { points: pts })),
    ).toBe(renderToStaticMarkup(React.createElement(Sparkline, { points: pts.slice() })));
  });
});
