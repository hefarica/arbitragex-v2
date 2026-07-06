/**
 * ConfidenceScoringPanel SSR-only static markup tests.
 *
 * Asserts the structural invariants:
 *   - "Confidence scoring (A.8)" title present
 *   - "paper-only · live OFF" badge present (immutable)
 *   - "Loading scoring wire status…" copy at SSR
 *   - NO "all wired" or live-enable button anywhere
 */
import React from "react";
import { describe, it, expect } from "vitest";
import { renderToStaticMarkup } from "react-dom/server";
import { ConfidenceScoringPanel } from "../ConfidenceScoringPanel";

const html = renderToStaticMarkup(<ConfidenceScoringPanel />);

describe("ConfidenceScoringPanel (SSR initial state)", () => {
  it("renders the A.8 title", () => {
    expect(html).toMatch(/Confidence scoring \(A\.8\)/);
  });

  it("renders paper-only · live OFF badge", () => {
    expect(html).toMatch(/paper-only/);
    expect(html).toMatch(/live OFF/i);
  });

  it("renders loading state initially", () => {
    expect(html).toMatch(/Loading scoring wire status/i);
  });

  it("regression: never renders 'all wired' or live-enable controls", () => {
    expect(html).not.toMatch(/all (components )?wired/i);
    expect(html).not.toMatch(/>Enable scoring</i);
    expect(html).not.toMatch(/>Activate live</i);
    expect(html).not.toMatch(/>Submit transaction</i);
    expect(html).not.toMatch(/Live trading: ON/);
  });
});
