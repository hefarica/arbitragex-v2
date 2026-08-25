// frontend/components/__tests__/ProvenanceBadge.test.tsx
//
// FE-MASTER · FE-0007 — SSR-branch tests for the provenance leaf.
// Same determinism contract as ResourceHealth: pure, no clock, closed §66
// vocabulary with §40 hints, R8 detail fallback.
import React from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";

import { PROVENANCE_COPY, ProvenanceBadge, type Provenance } from "../ProvenanceBadge";

const TOKENS: Provenance[] = [
  "REALTIME",
  "SIMULATION",
  "PAPER",
  "HISTORICAL",
  "CONFIG",
  "ESTIMATE",
];

function render(props: Parameters<typeof ProvenanceBadge>[0]) {
  return renderToStaticMarkup(React.createElement(ProvenanceBadge, props));
}

describe("ProvenanceBadge — §66 closed vocabulary", () => {
  it("renders all six tokens with their canonical hints (§40)", () => {
    for (const p of TOKENS) {
      const html = render({ provenance: p });
      expect(html).toContain(`>${PROVENANCE_COPY[p].label}<`);
      expect(html).toContain(PROVENANCE_COPY[p].hint);
    }
  });

  it("the vocabulary is EXACTLY six tokens (drift alarm)", () => {
    expect(Object.keys(PROVENANCE_COPY).sort()).toEqual(
      [...TOKENS].sort(),
    );
  });

  it("each token picks a DISTINCT chip class + icon pair", () => {
    const combos = new Set<string>();
    for (const p of TOKENS) {
      const html = render({ provenance: p });
      const m = html.match(/rounded-full border [^"]*/);
      expect(m, `token ${p} must have a chip class`).not.toBeNull();
      const icon = html.match(/lucide-([a-z0-9-]+)/);
      expect(icon, `token ${p} must have an icon`).not.toBeNull();
      combos.add(`${m![0]!}|${icon![1]!}`);
    }
    expect(combos.size).toBe(TOKENS.length);
  });
});

describe("ProvenanceBadge — R8 detail + R1 determinism", () => {
  it("detail null = canonical copy only", () => {
    const html = render({ provenance: "ESTIMATE" });
    expect(html).toContain(PROVENANCE_COPY.ESTIMATE.hint);
    expect(html.match(/ — /g)?.length ?? 0).toBe(0);
  });

  it("a provided detail rides verbatim", () => {
    const html = render({ provenance: "REALTIME", detail: "route_discovery_telemetry push" });
    expect(html).toContain("route_discovery_telemetry push");
  });

  it("two renders of the same props are byte-identical", () => {
    expect(render({ provenance: "PAPER" })).toBe(render({ provenance: "PAPER" }));
  });
});
