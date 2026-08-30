/**
 * DeploymentCard tests — AUDIT-2026-08-29 P0-1 (deployment coherence).
 *
 * SSR-only static markup assertions (same toolkit alignment as
 * SystemGuardBanner.test.tsx: react-dom/server, no jsdom, no network).
 *
 * What this guards (R8 fail-honest, RULE 00 zero mocks in reverse —
 * the card may ONLY render what the /status payload actually carries):
 *   - present deploy block → full sha + short badge + run link + timestamp
 *   - deploy absent (api-server older than the field) → "not reported",
 *     never a guessed SHA
 *   - deploy.sha === "unknown" (manual compose up) → explained verbatim,
 *     never rendered as if it were a real SHA
 */
import React from "react";
import { describe, it, expect } from "vitest";
import { renderToStaticMarkup } from "react-dom/server";
import { DeploymentCard } from "../DeploymentCard";
import type { StatusResponse } from "@/lib/api-client";

const base = {
  ok: true,
  services: {},
  killswitch: null,
  env: "production",
  version: "0.1.0",
  ts: "2026-08-29T00:00:00.000Z",
} as unknown as StatusResponse;

describe("DeploymentCard", () => {
  it("renders the full sha + short badge + run link + timestamp when deploy is present", () => {
    const html = renderToStaticMarkup(
      <DeploymentCard
        status={{
          ...base,
          deploy: {
            sha: "0622c9e06d85e5df861874f3964e53779d35b906",
            id: "1234567890",
            at: "2026-08-29T18:30:00Z",
          },
        } as StatusResponse}
      />,
    );
    expect(html).toContain("0622c9e06d85e5df861874f3964e53779d35b906");
    expect(html).toContain("0622c9e");
    expect(html).toContain("1234567890");
    expect(html).toContain("/actions/runs/1234567890");
    expect(html).toContain("2026-08-29T18:30:00Z");
  });

  it("fail-honest: deploy absent (skew window) → 'not reported', never a guessed SHA", () => {
    const html = renderToStaticMarkup(<DeploymentCard status={base} />);
    expect(html).toMatch(/not reported/i);
    // No 40-hex SHA can appear — nothing was reported to render.
    expect(html).not.toMatch(/\b[0-9a-f]{40}\b/);
  });

  it("fail-honest: sha 'unknown' (manual up) → explained verbatim, not styled as a SHA", () => {
    const html = renderToStaticMarkup(
      <DeploymentCard
        status={{
          ...base,
          deploy: { sha: "unknown", id: "unknown", at: "unknown" },
        } as StatusResponse}
      />,
    );
    expect(html).toMatch(/unknown/i);
    expect(html).toMatch(/manual/i);
    // The short-badge carve must not run on the sentinel.
    expect(html).not.toContain(">unknow</span>");
  });
});
