// frontend/features/agent-insights/AgentInsightsClient.test.tsx
//
// DAPP-SURFACE-FAIL (Business truth) regression — 2026-08-31.
//
// The workbook flagged /agent-insights for serving May-era static evidence as
// current verdicts. The fix renders a dated provenance footer on every agent
// card: "ledger verified: <date|—>" + "runtime re-verify: <time|not executed>".
// SSR-branch test (R1-deterministic): effects don't run server-side, so the
// initial snapshot render is stable for renderToStaticMarkup.
import React from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it, vi } from "vitest";

// The summary strip (pre-existing) uses LocalTime — a mount-only locale view
// convenience that doesn't render under the classic-SSR test runtime. Stub it
// with a deterministic passthrough; the footer under test uses fmtDateTime.
vi.mock("@/components/LocalTime", () => ({
  LocalTime: ({ iso }: { iso: string }) => <span>{iso}</span>,
}));

import { AgentInsightsClient } from "./AgentInsightsClient";

const row = (over: Record<string, unknown>) => ({
  id: "test-agent",
  name: "Test Agent",
  category: "backend",
  verdict: "PASS",
  status: "healthy",
  evidence: ["ledger line"],
  last_run_at: null,
  verified_at: "2026-05-13",
  source: "workspace_verified",
  blocks: [],
  next_action: null,
  risk: "low",
  operator_required: false,
  ...over,
});

const snapshot = (agents: unknown[]) => ({
  generated_at: "2026-08-31T00:00:00.000Z",
  source: "mixed",
  overall_status: "blocked",
  agents,
  summary: { pass: 1, blocked: 0, partial: 0, no_go: 0, not_run: 0, unknown: 0, total: 1 },
});

describe("AgentInsightsClient — dated provenance rendering", () => {
  it("renders the ledger verification date on the card", () => {
    const html = renderToStaticMarkup(
      <AgentInsightsClient initialData={snapshot([row({})])} />,
    );
    expect(html).toContain("ledger verified: 2026-05-13");
  });

  it("renders an honest em-dash when verified_at is absent (old backend snapshot)", () => {
    const html = renderToStaticMarkup(
      <AgentInsightsClient initialData={snapshot([row({ verified_at: undefined })])} />,
    );
    expect(html).toContain("ledger verified: —");
  });

  it("states 'not executed' for static-ledger agents instead of implying a runtime run", () => {
    const html = renderToStaticMarkup(
      <AgentInsightsClient initialData={snapshot([row({ last_run_at: null })])} />,
    );
    expect(html).toContain("runtime re-verify: not executed");
  });

  it("renders the runtime re-verify timestamp when the probe actually ran", () => {
    const html = renderToStaticMarkup(
      <AgentInsightsClient
        initialData={snapshot([row({ last_run_at: "2026-08-31T12:00:00.000Z" })])}
      />,
    );
    expect(html).toContain("runtime re-verify:");
    expect(html).not.toContain("runtime re-verify: not executed");
  });
});
