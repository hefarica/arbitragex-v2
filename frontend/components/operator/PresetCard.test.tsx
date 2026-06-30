// SSR-branch tests for PresetCard. The frontend test env is `node` (no jsdom),
// so we render to static HTML via react-dom/server and assert on deterministic
// branches — exactly like TokenIcon.test.tsx. Interactive selection + the
// localStorage persistence live in the hook/page and are covered by the
// Playwright E2E (e2e/operator-presets.spec.ts).

import React from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { describe, it, expect } from "vitest";

import { PresetCard } from "./PresetCard";
import { RISK_POSTURES, resolvePosture } from "@/lib/presets/riskPresets";

const SEVEN_TESTIDS = (id: string) => [
  `dim-${id}-deployed`,
  `dim-${id}-daily-loss`,
  `dim-${id}-per-token`,
  `dim-${id}-per-pool`,
  `dim-${id}-per-dex`,
  `dim-${id}-per-chain`,
  `dim-${id}-per-strategy`,
  `dim-${id}-consecutive`,
];

describe("PresetCard (SSR)", () => {
  it("renders the label, description and all 7 dimension rows", () => {
    const html = renderToStaticMarkup(
      <PresetCard
        posture={RISK_POSTURES.balanced}
        resolved={resolvePosture("balanced", { capitalUsd: 1000 })}
        selected={false}
        onSelect={() => {}}
      />,
    );
    expect(html).toContain("Balanced");
    for (const t of SEVEN_TESTIDS("balanced")) {
      expect(html, `has ${t}`).toContain(t);
    }
  });

  it("only renders non-predatory strategy kinds (ethics gate at the UI layer)", () => {
    const html = renderToStaticMarkup(
      <PresetCard
        posture={RISK_POSTURES.aggressive}
        resolved={resolvePosture("aggressive", { capitalUsd: 1000 })}
        selected={false}
        onSelect={() => {}}
      />,
    );
    expect(html).toContain("cross_pool_arb");
    for (const bad of ["sandwich", "frontrun", "oracle", "jit", "victim"]) {
      expect(html.toLowerCase(), `must not render ${bad}`).not.toContain(bad);
    }
  });

  it("shows the Selected marker only when selected", () => {
    const resolved = resolvePosture("conservative", { capitalUsd: 1000 });
    const sel = renderToStaticMarkup(
      <PresetCard posture={RISK_POSTURES.conservative} resolved={resolved} selected onSelect={() => {}} />,
    );
    const unsel = renderToStaticMarkup(
      <PresetCard posture={RISK_POSTURES.conservative} resolved={resolved} selected={false} onSelect={() => {}} />,
    );
    expect(sel).toContain("preset-selected-conservative");
    expect(sel).toContain('aria-pressed="true"');
    expect(unsel).not.toContain("preset-selected-conservative");
  });

  it("fail-honest: capital 0 renders $0.00 caps + a warning, no invented numbers", () => {
    const html = renderToStaticMarkup(
      <PresetCard
        posture={RISK_POSTURES.aggressive}
        resolved={resolvePosture("aggressive", { capitalUsd: 0 })}
        selected={false}
        onSelect={() => {}}
      />,
    );
    expect(html).toContain("preset-warnings-aggressive");
    expect(html).toContain("$0.00");
  });
});
