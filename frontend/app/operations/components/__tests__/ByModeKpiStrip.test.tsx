// frontend/app/operations/components/__tests__/ByModeKpiStrip.test.tsx
//
// ARBX-0011 (REQ-DASH-BY-MODE) — by-mode KPI scope strip, SSR-branch tests.
//
// Repo pattern (PairIntelligencePanel.test.tsx): the frontend test env is
// `node` (no jsdom), so the pure presentational strip renders to a static
// HTML string via react-dom/server and the deterministic branches assert:
//   - ready view: three canonical chips, the ACTIVE terminus marked
//     (aria-current) with its KPI-scope line, the other two with their
//     doctrine terminus states — never a fabricated per-mode number;
//   - boot-vs-selected mismatch is surfaced verbatim, not reconciled;
//   - null view renders the honest absence (R8) — the fetch error verbatim,
//     or the absence-of-fields default when error is null.
import React from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";

import { ByModeKpiStrip } from "../ByModeKpiStrip";

const PAPER_VIEW = {
  execution_mode: "PAPER_SHADOW" as const,
  selected_execution_mode: "PAPER_SHADOW" as const,
  coherent: true,
};

describe("ByModeKpiStrip", () => {
  it("renders the three canonical modes with the active one marked", () => {
    const html = renderToStaticMarkup(<ByModeKpiStrip view={PAPER_VIEW} error={null} />);
    expect(html).toContain("LIVE_MAINNET");
    expect(html).toContain("TESTNET");
    expect(html).toContain("PAPER_SHADOW");
    // Exactly one active chip.
    expect(html.match(/aria-current="true"/g)).toHaveLength(1);
    expect(html).toContain("active · KPIs below reflect this terminus (paper ledger)");
  });

  it("states the doctrine terminus states, not fabricated per-mode numbers", () => {
    const html = renderToStaticMarkup(<ByModeKpiStrip view={PAPER_VIEW} error={null} />);
    expect(html).toContain("default-deny: MainnetRefused (§34.3)");
    expect(html).toContain("broadcast gated: testnet allowlist (ARBX_LIVE_EXEC_CHAINS)");
    expect(html).toContain("mode authority: relays-client live_exec_policy");
    expect(html).toContain("one math pipeline (§34.1)");
  });

  it("surfaces a boot-vs-selected mismatch verbatim", () => {
    const html = renderToStaticMarkup(
      <ByModeKpiStrip
        view={{
          execution_mode: "PAPER_SHADOW",
          selected_execution_mode: "TESTNET",
          coherent: false,
        }}
        error={null}
      />,
    );
    expect(html).toContain("MISMATCH with boot mode (PAPER_SHADOW)");
    expect(html).toContain('selected_execution_mode <span class="font-mono">TESTNET</span>');
  });

  it("renders the honest absence on a null view (R8)", () => {
    const html = renderToStaticMarkup(
      <ByModeKpiStrip view={null} error="HTTP 503: knobs_not_published" />,
    );
    expect(html).toContain("HTTP 503: knobs_not_published");
    // No mode chips are fabricated around the absence.
    expect(html).not.toContain("aria-current");
  });

  it("defaults the absence reason when error is null", () => {
    const html = renderToStaticMarkup(<ByModeKpiStrip view={null} error={null} />);
    expect(html).toContain("canonical mode fields absent from knobs snapshot");
  });
});
