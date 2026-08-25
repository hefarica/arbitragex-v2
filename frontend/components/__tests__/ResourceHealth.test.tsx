// frontend/components/__tests__/ResourceHealth.test.tsx
//
// FE-MASTER · FE-0006 — SSR-branch tests for the health leaf.
//
// Node env: renderToStaticMarkup over the pure component. The leaf is
// deterministic BY DESIGN (no Date.now(), no fetch) — these tests pin the
// §65 closed vocabulary, the §40 explain-yourself hints, and R8 detail
// fallback.
import React from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";

import {
  RESOURCE_HEALTH_COPY,
  ResourceHealth,
  type ResourceHealthState,
} from "../ResourceHealth";

const STATES: ResourceHealthState[] = [
  "HEALTHY",
  "DEGRADED",
  "STALE",
  "FAILED",
  "UNCONFIGURED",
];

function render(props: Parameters<typeof ResourceHealth>[0]) {
  return renderToStaticMarkup(React.createElement(ResourceHealth, props));
}

describe("ResourceHealth — §65 closed vocabulary", () => {
  it("renders every state with its label and canonical hint (§40)", () => {
    for (const state of STATES) {
      const html = render({ label: "pairs snapshot", state });
      expect(html).toContain(`>${RESOURCE_HEALTH_COPY[state].label}<`);
      expect(html).toContain(RESOURCE_HEALTH_COPY[state].hint);
    }
  });

  it("renders the resource label verbatim", () => {
    const html = render({ label: "runtime_ack room", state: "HEALTHY" });
    expect(html).toContain("runtime_ack room");
  });

  it("each state picks a DISTINCT chip class + icon pair (no two states look identical)", () => {
    const combos = new Set<string>();
    for (const state of STATES) {
      const html = render({ label: "x", state });
      const m = html.match(/rounded-full border [^"]*/);
      expect(m, `state ${state} must have a chip class`).not.toBeNull();
      const icon = html.match(/lucide-([a-z0-9-]+)/);
      expect(icon, `state ${state} must have an icon`).not.toBeNull();
      combos.add(`${m![0]!}|${icon![1]!}`);
    }
    expect(combos.size).toBe(STATES.length);
  });
});

describe("ResourceHealth — R8 detail semantics", () => {
  it("detail null = canonical copy only (no fabricated per-resource story)", () => {
    const html = render({ label: "x", state: "FAILED" });
    expect(html).toContain(RESOURCE_HEALTH_COPY.FAILED.hint);
    expect(html.match(/ — /g)?.length ?? 0).toBe(0);
  });

  it("a provided detail rides verbatim, appended to the canonical hint", () => {
    const html = render({ label: "x", state: "STALE", detail: "no payload for 104s (budget 105s)" });
    expect(html).toContain(RESOURCE_HEALTH_COPY.STALE.hint);
    expect(html).toContain("no payload for 104s (budget 105s)");
  });
});

describe("ResourceHealth — R1 determinism", () => {
  it("two renders of the same props are byte-identical (no clock, no random)", () => {
    const a = render({ label: "tick", state: "DEGRADED" });
    const b = render({ label: "tick", state: "DEGRADED" });
    expect(a).toBe(b);
  });

  it("carries role=status (screen-reader surface)", () => {
    expect(render({ label: "x", state: "HEALTHY" })).toContain('role="status"');
  });
});
