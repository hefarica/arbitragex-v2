// frontend/app/operations/__tests__/OperationsClient.test.tsx
//
// ARBX-0011 R3 regression — both-ways isolation of the mode strip.
//
// The R3 defect (peer probe, 2026-08-24): the KPI early-returns
// (`if (error && !kpi)` Alert, `if (!kpi)` Loading) fired BEFORE the
// ByModeKpiStrip render, so an outage of KPI data hid the terminus scope —
// exactly the moment it matters most. The strip is doctrine labels + the
// boot knobs snapshot, never KPI data, so it must render in every branch.
//
// Repo pattern (node env, no jsdom): renderToStaticMarkup of the client
// component with the `initial*` props the Server Component passes; the
// useEffect poll never runs server-side, so no fetch wiring is needed.
import React from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";

import { OperationsClient } from "../OperationsClient";

const MODE_VIEW = {
  execution_mode: "PAPER_SHADOW" as const,
  selected_execution_mode: "PAPER_SHADOW" as const,
  coherent: true,
};

const NO_DATA_PROPS = {
  initialKpi: null,
  initialScurve: null,
  initialHeartbeat: null,
  initialHeartbeatError: null,
  initialError: null,
  initialModeView: MODE_VIEW,
  initialModeError: null,
  // ARBX-QB-07-008: boot lat.* snapshot (2 stages incl. the total row).
  initialLatStages: [
    {
      key: "lat.decode" as const,
      target_ms: 2,
      p50_us: 12340,
      p90_us: 15000,
      p95_us: 18000,
      p99_us: 20000,
      headroom_p95_us: 200,
    },
    {
      key: "lat.total" as const,
      target_ms: 30,
      p50_us: 22000,
      p90_us: 25000,
      p95_us: 28000,
      p99_us: 29000,
      headroom_p95_us: 2000,
    },
  ],
  initialLatPass: true,
  initialLatCycles: 4242,
  initialLatError: null,
};

describe("OperationsClient — mode strip survives KPI absence (ARBX-0011 R3)", () => {
  it("renders the strip in the loading branch (no KPIs yet)", () => {
    const html = renderToStaticMarkup(<OperationsClient {...NO_DATA_PROPS} />);
    expect(html).toContain("Loading convergence metrics…");
    expect(html).toContain("Execution modes · KPI scope");
    expect(html).toContain("PAPER_SHADOW");
    expect(html.match(/aria-current="true"/g)).toHaveLength(1);
  });

  it("renders the strip in the error branch (convergence endpoint down)", () => {
    const html = renderToStaticMarkup(
      <OperationsClient {...NO_DATA_PROPS} initialError="HTTP 503: upstream" />,
    );
    expect(html).toContain("convergence endpoint error");
    expect(html).toContain("HTTP 503: upstream");
    // The strip stands above the Alert — terminus scope visible during outage.
    expect(html).toContain("Execution modes · KPI scope");
    expect(html).toContain("active · KPIs below reflect this terminus (paper ledger)");
    expect(html.indexOf("Execution modes · KPI scope")).toBeLessThan(
      html.indexOf("convergence endpoint error"),
    );
  });

  it("renders the honest strip absence (R8) instead of fabricating modes", () => {
    const html = renderToStaticMarkup(
      <OperationsClient
        {...NO_DATA_PROPS}
        initialModeView={null}
        initialModeError="HTTP 503: knobs_not_published"
      />,
    );
    expect(html).toContain("Loading convergence metrics…");
    expect(html).toContain("HTTP 503: knobs_not_published");
    expect(html).not.toContain("aria-current");
  });

  // ARBX-QB-07-008: the lat.* panel gets the SAME both-ways isolation —
  // it renders with its boot snapshot in the outage branches, and its own
  // fetch failure renders the honest error without touching the page.
  it("renders the lat.* panel with its boot snapshot in the loading branch (QB-07-008)", () => {
    const html = renderToStaticMarkup(<OperationsClient {...NO_DATA_PROPS} />);
    expect(html).toContain("Loading convergence metrics…");
    expect(html).toContain("Discovery latency budget · lat.* stages (10_LATENCY)");
    expect(html).toContain("lat.decode");
    expect(html).toContain("lat.total");
    expect(html).toContain("12.34");
    expect(html).toContain("PASS_p95");
    expect(html).toContain("4242");
  });

  it("renders the lat.* panel in the error branch, above the Alert", () => {
    const html = renderToStaticMarkup(
      <OperationsClient {...NO_DATA_PROPS} initialError="HTTP 503: upstream" />,
    );
    expect(html).toContain("convergence endpoint error");
    expect(html).toContain("Discovery latency budget");
    expect(html.indexOf("Discovery latency budget")).toBeLessThan(
      html.indexOf("convergence endpoint error"),
    );
  });

  it("renders the lat.* honest error (R8) instead of a fabricated table", () => {
    const html = renderToStaticMarkup(
      <OperationsClient
        {...NO_DATA_PROPS}
        initialLatStages={null}
        initialLatPass={null}
        initialLatCycles={0}
        initialLatError="HTTP 404: tick not published yet"
      />,
    );
    expect(html).toContain("Loading convergence metrics…");
    expect(html).toContain('role="alert"');
    expect(html).toContain("HTTP 404: tick not published yet");
    expect(html).not.toContain("lat.decode");
  });
});
