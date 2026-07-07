/**
 * StatusPill tests using react-dom/server renderToStaticMarkup.
 *
 * Why not @testing-library/react:
 *   - Not installed; requires jsdom + operator approval (toolkit-alignment memory).
 *   - StatusPill is a pure component with no hooks, no events.
 *   - renderToStaticMarkup validates label rendering and title attribute.
 *
 * 10 tests total:
 *   - 9 via it.each: every OpportunityStatus value renders its label.
 *   - 1 explicit: rejection_reason surfaces in title attribute for status="rejected".
 */
import React from "react";
import { describe, it, expect } from "vitest";
import { renderToStaticMarkup } from "react-dom/server";
import { StatusPill } from "../StatusPill";
import type { OpportunityStatus } from "../StatusPill";

const ALL_STATUSES: OpportunityStatus[] = [
  "detected",
  "validated",
  "simulated",
  "scored",
  "executing",
  "executed",
  "reconciled",
  "rejected",
  "failed",
];

describe("StatusPill", () => {
  it.each(ALL_STATUSES)("renders label for status=%s", (s) => {
    const html = renderToStaticMarkup(<StatusPill status={s} />);
    // Labels are uppercase (e.g. "DETECTED"); compare case-insensitively.
    expect(html.toUpperCase()).toContain(s.toUpperCase());
  });

  it("includes rejection_reason in title attribute when status=rejected", () => {
    const html = renderToStaticMarkup(
      <StatusPill status="rejected" rejection_reason="TokenNotAllowed" />
    );
    expect(html).toMatch(/title="[^"]*TokenNotAllowed/);
  });
});
