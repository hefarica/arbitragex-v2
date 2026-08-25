// frontend/features/routes-discovery/__tests__/RoutesDiscoveryClient.tabs.test.tsx
//
// FE-MASTER · FE-0036 — Route Discovery view tabs, SSR-branch affordance test.
//
// renderToStaticMarkup renders the MOUNT state only (effects never run), so
// this pins the affordance contract of the tab switch: both tabs present,
// radar is the default selected view (grid + §18/§19 radar sections), and
// the Performance panel content is NOT mounted by default. The Performance
// panel's own branches are pinned in PerformancePanel.test.tsx; the radar
// sections' behavior is FE-0026/27 territory (untouched by FE-0036).
import React from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { beforeEach, describe, expect, it, vi } from "vitest";

const storeState = vi.hoisted(() => ({ current: {} as Record<string, unknown> }));

vi.mock("@/lib/store/omni-store", () => ({
  useOmniStore: (selector: (s: Record<string, unknown>) => unknown) =>
    selector(storeState.current),
  // The client imports this named hook, not the raw store.
  useRouteTick: () => storeState.current.tick,
}));

import { RoutesDiscoveryClient } from "../RoutesDiscoveryClient";

function render() {
  return renderToStaticMarkup(
    React.createElement(RoutesDiscoveryClient, {
      initialData: { routes: null, status: null },
    }),
  );
}

beforeEach(() => {
  storeState.current = { tick: null };
});

describe("RoutesDiscoveryClient — FE-0036 view tabs (§43/§44)", () => {
  it("renders the tablist with radar selected by default", () => {
    const html = render();
    expect(html).toContain('data-testid="routes-view-tabs"');
    expect(html).toContain('role="tablist"');
    expect(html).toMatch(/role="tab"[^>]*aria-selected="true"[^>]*>radar</);
    expect(html).toMatch(/role="tab"[^>]*aria-selected="false"[^>]*>performance</);
  });

  it("default (radar) view keeps the radar surfaces and does NOT mount the performance table", () => {
    const html = render();
    expect(html).toContain('data-testid="market-event-pipeline"');
    expect(html).toContain('data-testid="routes-empty"');
    expect(html).not.toContain('data-testid="performance-panel"');
  });
});
