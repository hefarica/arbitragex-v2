/**
 * RiskCircuitPanel SSR-only tests.
 *
 * Asserts structural invariants:
 *   - Title "Risk circuit breakers (A.6)" present
 *   - "live OFF" badge present
 *   - "Loading circuit breaker status…" at SSR
 *   - NO "all green" claim
 *   - NO live-enable controls
 */
import React from "react";
import { describe, it, expect } from "vitest";
import { renderToStaticMarkup } from "react-dom/server";
import { RiskCircuitPanel } from "../RiskCircuitPanel";

const html = renderToStaticMarkup(<RiskCircuitPanel />);

describe("RiskCircuitPanel (SSR initial state)", () => {
  it("renders the A.6 title", () => {
    expect(html).toMatch(/Risk circuit breakers \(A\.6\)/);
  });

  it("renders live OFF badge", () => {
    expect(html).toMatch(/live OFF/i);
  });

  it("renders loading state initially", () => {
    expect(html).toMatch(/Loading circuit breaker status/i);
  });

  it("renders R8 disclaimer about NOT_AVAILABLE", () => {
    expect(html).toMatch(/NOT_AVAILABLE/);
    expect(html).toMatch(/never fabricated PASS/i);
  });

  it("regression: never renders 'all green' or live-enable controls", () => {
    expect(html).not.toMatch(/all (breakers )?green/i);
    expect(html).not.toMatch(/>Enable live</i);
    expect(html).not.toMatch(/>Submit transaction</i);
    expect(html).not.toMatch(/Live trading: ON/);
  });
});
