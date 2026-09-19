/**
 * StatusPill tests using react-dom/server renderToStaticMarkup.
 *
 * Why not @testing-library/react:
 *   - Not installed; requires jsdom + operator approval (toolkit-alignment memory).
 *   - StatusPill is a pure component with no hooks, no events.
 *   - renderToStaticMarkup validates label rendering and title attribute.
 *
 * 13 tests total:
 *   - 9 via it.each: every OpportunityStatus value renders its label.
 *   - 1 explicit: rejection_reason surfaces in title attribute for status="rejected".
 *   - 3 GAP1-REJ-REASON-FEED (2026-09-17): reason as visible text; null → label
 *     only (fail-honest); non-rejected statuses render no reason text.
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

  // GAP1-REJ-REASON-FEED (2026-09-17): the reason must be VISIBLE text, not
  // only a hover title — document.body.innerText must carry it so the operator
  // sees WHY a detection was REJECTED without leaving /opportunities.
  const stripTags = (html: string) => html.replace(/<[^>]*>/g, "");

  it("renders rejection_reason as visible text when status=rejected", () => {
    const html = renderToStaticMarkup(
      <StatusPill status="rejected" rejection_reason="v3_quote_unavailable" />
    );
    expect(stripTags(html)).toContain("v3_quote_unavailable");
  });

  it("renders no reason text when rejection_reason is null (fail-honest)", () => {
    const html = renderToStaticMarkup(
      <StatusPill status="rejected" rejection_reason={null} />
    );
    expect(stripTags(html)).not.toContain("·");
    expect(stripTags(html).trim()).toBe("REJECTED");
  });

  it("does not render rejection_reason text for non-rejected statuses", () => {
    const html = renderToStaticMarkup(
      <StatusPill status="detected" rejection_reason="v3_quote_unavailable" />
    );
    expect(stripTags(html)).not.toContain("v3_quote_unavailable");
  });
});
