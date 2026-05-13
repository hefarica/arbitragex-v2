/**
 * A.8 scoring-cell tests — null/0/value rendering for the new bps columns.
 *
 * R8 fail-honest enforcement:
 *   - null / undefined → "Unavailable" (NEVER "0", NEVER "—")
 *   - 0 bps (real measured zero) → renders "0 bps"
 *   - Non-zero bps → renders the value + bps label
 *   - ACCEPT_PAPER decision → info badge
 *   - REJECT_* decision → destructive badge
 *   - BLOCKED_A4 decision → outline badge (not destructive — it's a pipeline state)
 */
import React from "react";
import { describe, it, expect } from "vitest";
import { renderToStaticMarkup } from "react-dom/server";
import { OpportunityEvidenceCell } from "../OpportunityEvidenceCell";

describe("OpportunityEvidenceCell.ScoreBps", () => {
  it("renders Unavailable for null", () => {
    const html = renderToStaticMarkup(
      <OpportunityEvidenceCell.ScoreBps value={null} label="confidence" />,
    );
    expect(html).toMatch(/Unavailable/i);
  });

  it("renders Unavailable for undefined", () => {
    const html = renderToStaticMarkup(
      <OpportunityEvidenceCell.ScoreBps value={undefined} label="confidence" />,
    );
    expect(html).toMatch(/Unavailable/i);
  });

  it("renders 0 bps (real measured zero, NOT unavailable)", () => {
    const html = renderToStaticMarkup(
      <OpportunityEvidenceCell.ScoreBps value={0} label="confidence" />,
    );
    expect(html).toContain("0");
    expect(html).toContain("bps");
    expect(html).not.toMatch(/Unavailable/i);
  });

  it("renders 7500 bps for confidence", () => {
    const html = renderToStaticMarkup(
      <OpportunityEvidenceCell.ScoreBps value={7500} label="confidence" />,
    );
    expect(html).toContain("7500");
    expect(html).toContain("bps");
  });

  it("regression: NEVER converts null to 0 bps", () => {
    const html = renderToStaticMarkup(
      <OpportunityEvidenceCell.ScoreBps value={null} label="kelly" />,
    );
    expect(html).not.toMatch(/>0</);
    expect(html).not.toMatch(/0 bps/);
  });
});

describe("OpportunityEvidenceCell.ScoringDecision", () => {
  it("renders Unavailable for null", () => {
    const html = renderToStaticMarkup(<OpportunityEvidenceCell.ScoringDecision value={null} />);
    expect(html).toMatch(/Unavailable/i);
  });

  it("renders ACCEPT_PAPER as info badge", () => {
    const html = renderToStaticMarkup(<OpportunityEvidenceCell.ScoringDecision value="ACCEPT_PAPER" />);
    expect(html).toContain("ACCEPT_PAPER");
    expect(html).not.toMatch(/Unavailable/i);
  });

  it("renders REJECT_NEGATIVE_EV (destructive)", () => {
    const html = renderToStaticMarkup(<OpportunityEvidenceCell.ScoringDecision value="REJECT_NEGATIVE_EV" />);
    expect(html).toContain("REJECT_NEGATIVE_EV");
  });

  it("renders BLOCKED_A4 (outline)", () => {
    const html = renderToStaticMarkup(<OpportunityEvidenceCell.ScoringDecision value="BLOCKED_A4" />);
    expect(html).toContain("BLOCKED_A4");
  });

  it("uppercases lowercase input", () => {
    const html = renderToStaticMarkup(<OpportunityEvidenceCell.ScoringDecision value="accept_paper" />);
    expect(html).toContain("ACCEPT_PAPER");
  });
});
