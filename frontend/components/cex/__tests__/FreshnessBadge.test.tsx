// frontend/components/cex/__tests__/FreshnessBadge.test.tsx
//
// (b) WO-PRICE-EXCHANGE-V1 — badge freshness colors by age (dates mocked via
// vi.setSystemTime for formatAgo), StatusPill-style dot classes, R1 purity
// (no Date/window inside the component itself).
import React from "react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { renderToStaticMarkup } from "react-dom/server";

import {
  FRESH_MAX_SECS,
  FreshnessBadge,
  freshnessLevel,
  WARM_MAX_SECS,
} from "../FreshnessBadge";
import { formatAgo } from "@/components/OpportunityTicker";

afterEach(() => {
  vi.useRealTimers();
});

describe("freshnessLevel — age thresholds (green <30s, amber <2m, red ≥2m)", () => {
  it("fresh under 30s (boundaries: 29s fresh, 30s warm)", () => {
    expect(freshnessLevel(0)).toBe("fresh");
    expect(freshnessLevel(29)).toBe("fresh");
    expect(freshnessLevel(FRESH_MAX_SECS)).toBe("warm");
  });

  it("warm from 30s to under 2m (boundaries: 119s warm, 120s stale)", () => {
    expect(freshnessLevel(45)).toBe("warm");
    expect(freshnessLevel(119)).toBe("warm");
    expect(freshnessLevel(WARM_MAX_SECS)).toBe("stale");
  });

  it("stale at and past 2m", () => {
    expect(freshnessLevel(150)).toBe("stale");
    expect(freshnessLevel(3600)).toBe("stale");
  });

  it("no age → unknown (never fabricates a freshness claim)", () => {
    expect(freshnessLevel(null)).toBe("unknown");
    expect(freshnessLevel(NaN)).toBe("unknown");
  });
});

describe("FreshnessBadge — dot classes by level", () => {
  it("fresh → success dot, warm → warning dot, stale → destructive dot", () => {
    const mk = (level: "fresh" | "warm" | "stale") =>
      renderToStaticMarkup(
        React.createElement(FreshnessBadge, { symbol: "WETH", agoText: "10s", level }),
      );
    expect(mk("fresh")).toContain("bg-success");
    expect(mk("warm")).toContain("bg-warning");
    expect(mk("stale")).toContain("bg-destructive");
  });

  it("unknown (no age yet / pre-mount) → muted dot, honest '--' age", () => {
    const html = renderToStaticMarkup(
      React.createElement(FreshnessBadge, { symbol: "WETH", agoText: null, level: "unknown" }),
    );
    expect(html).toContain("bg-muted-foreground/50");
    expect(html).toContain("--");
    expect(html).toContain('data-level="unknown"');
  });

  it("null symbol renders an honest dash, never a fabricated symbol", () => {
    const html = renderToStaticMarkup(
      React.createElement(FreshnessBadge, { symbol: null, agoText: "5s", level: "fresh" }),
    );
    expect(html).toContain("—");
  });

  it("R1: pure component — no Date/window usage, deterministic output", () => {
    const props = { symbol: "USDC", agoText: "45s", level: "warm" as const };
    const a = renderToStaticMarkup(React.createElement(FreshnessBadge, props));
    const b = renderToStaticMarkup(React.createElement(FreshnessBadge, props));
    expect(a).toBe(b);
  });
});

describe("formatAgo — age text from detected_at (mocked clock)", () => {
  it("renders seconds/minutes/hours from a fixed system time", () => {
    vi.useFakeTimers();
    vi.setSystemTime(new Date("2026-09-18T12:00:00Z"));
    expect(formatAgo("2026-09-18T11:59:50Z")).toBe("10s");
    expect(formatAgo("2026-09-18T11:58:20Z")).toBe("1m");
    expect(formatAgo("2026-09-18T09:30:00Z")).toBe("2h");
  });

  it("badge dot + text agree end-to-end at the thresholds (mocked dates)", () => {
    vi.useFakeTimers();
    vi.setSystemTime(new Date("2026-09-18T12:00:00Z"));
    // 10s old → fresh / green; 45s old → warm / amber; 150s old → stale / red
    const cases: Array<[string, string, string]> = [
      ["2026-09-18T11:59:50Z", "fresh", "bg-success"],
      ["2026-09-18T11:59:15Z", "warm", "bg-warning"],
      ["2026-09-18T11:57:30Z", "stale", "bg-destructive"],
    ];
    for (const [detectedAt, level, dot] of cases) {
      const ago = formatAgo(detectedAt);
      const html = renderToStaticMarkup(
        React.createElement(FreshnessBadge, { symbol: "WETH", agoText: ago, level: level as never }),
      );
      expect(html).toContain(dot);
      expect(html).toContain(`data-level="${level}"`);
    }
  });
});
