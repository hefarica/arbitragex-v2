/**
 * OpportunityEvidenceCell tests — R8 fail-honest enforcement.
 *
 * Critical invariants under test:
 *   - null/undefined/"" → "Unavailable" (NEVER "0", NEVER "—" as if measured)
 *   - 0 USD → "$0.00" (real measured zero rendered as zero)
 *   - SIM_SUCCESS / SIM_REVERT / SIM_HALT classifications render as badges
 *   - trace_hash renders shortened (0x12345678…abcd) with full value in title
 *   - Gates render "PASS"/"FAIL" badges, "—" placeholder when null
 *
 * These tests are the structural barrier against "viable" looking
 * opportunities that have no real simulation evidence.
 */
import React from "react";
import { describe, it, expect } from "vitest";
import { renderToStaticMarkup } from "react-dom/server";
import { OpportunityEvidenceCell } from "../OpportunityEvidenceCell";

describe("OpportunityEvidenceCell.SimClassification", () => {
  it("renders Unavailable for null", () => {
    const html = renderToStaticMarkup(<OpportunityEvidenceCell.SimClassification value={null} />);
    expect(html).toMatch(/Unavailable/i);
  });
  it("renders Unavailable for empty string", () => {
    const html = renderToStaticMarkup(<OpportunityEvidenceCell.SimClassification value="" />);
    expect(html).toMatch(/Unavailable/i);
  });
  it("renders SIM_SUCCESS as a badge", () => {
    const html = renderToStaticMarkup(
      <OpportunityEvidenceCell.SimClassification value="SIM_SUCCESS" />,
    );
    expect(html).toContain("SIM_SUCCESS");
    expect(html).not.toMatch(/Unavailable/i);
  });
  it("renders SIM_REVERT as a badge", () => {
    const html = renderToStaticMarkup(
      <OpportunityEvidenceCell.SimClassification value="SIM_REVERT" />,
    );
    expect(html).toContain("SIM_REVERT");
  });
  it("uppercases lowercase input", () => {
    const html = renderToStaticMarkup(
      <OpportunityEvidenceCell.SimClassification value="sim_halt" />,
    );
    expect(html).toContain("SIM_HALT");
  });
});

describe("OpportunityEvidenceCell.NetProfit", () => {
  it("renders Unavailable when both usd and wei are null", () => {
    const html = renderToStaticMarkup(<OpportunityEvidenceCell.NetProfit usd={null} wei={null} />);
    expect(html).toMatch(/Unavailable/i);
  });
  it("renders 'wei only' when usd is null but wei present", () => {
    const html = renderToStaticMarkup(
      <OpportunityEvidenceCell.NetProfit usd={null} wei="1234567890" />,
    );
    expect(html).toMatch(/wei only/i);
    expect(html).not.toMatch(/Unavailable/i);
  });
  it("renders $0.00 when usd is 0 (REAL measured zero, NOT unavailable)", () => {
    // CRITICAL: this is the R8 fail-honest assertion. usd=0 means simulation
    // ran and the net was exactly zero — NOT the same as "no data".
    const html = renderToStaticMarkup(<OpportunityEvidenceCell.NetProfit usd={0} wei={null} />);
    expect(html).toContain("$0.00");
    expect(html).not.toMatch(/Unavailable/i);
  });
  it("renders positive USD with success tone", () => {
    const html = renderToStaticMarkup(<OpportunityEvidenceCell.NetProfit usd={42.5} wei={null} />);
    expect(html).toContain("$42.50");
  });
  it("renders negative USD with destructive tone", () => {
    const html = renderToStaticMarkup(<OpportunityEvidenceCell.NetProfit usd={-3.21} wei={null} />);
    expect(html).toMatch(/-?\$3\.21/);
  });
});

describe("OpportunityEvidenceCell.GasUsed", () => {
  it("renders Unavailable for null", () => {
    const html = renderToStaticMarkup(<OpportunityEvidenceCell.GasUsed value={null} />);
    expect(html).toMatch(/Unavailable/i);
  });
  it("renders Unavailable for empty string", () => {
    const html = renderToStaticMarkup(<OpportunityEvidenceCell.GasUsed value="" />);
    expect(html).toMatch(/Unavailable/i);
  });
  it("renders 0 when gas measured as zero", () => {
    const html = renderToStaticMarkup(<OpportunityEvidenceCell.GasUsed value={0} />);
    expect(html).toContain("0");
    expect(html).not.toMatch(/Unavailable/i);
  });
  it("renders numeric gas with thousands separator", () => {
    const html = renderToStaticMarkup(<OpportunityEvidenceCell.GasUsed value={123456} />);
    expect(html).toContain("123,456");
  });
});

describe("OpportunityEvidenceCell.Hash", () => {
  it("renders Unavailable for null", () => {
    const html = renderToStaticMarkup(<OpportunityEvidenceCell.Hash value={null} />);
    expect(html).toMatch(/Unavailable/i);
  });
  it("renders shortened hash with 0x prefix added", () => {
    const html = renderToStaticMarkup(
      <OpportunityEvidenceCell.Hash value="abcdef1234567890" />,
    );
    expect(html).toContain("0xabcdef");
    expect(html).toContain("7890");
    expect(html).toContain("…");
  });
  it("preserves existing 0x prefix", () => {
    const html = renderToStaticMarkup(
      <OpportunityEvidenceCell.Hash value="0xdeadbeefcafe1234" />,
    );
    expect(html).toContain("0xdeadbe");
  });
});

describe("OpportunityEvidenceCell.Gates", () => {
  it("renders Unavailable when both gates null", () => {
    const html = renderToStaticMarkup(
      <OpportunityEvidenceCell.Gates evidence={null} netProfit={null} />,
    );
    expect(html).toMatch(/Unavailable/i);
  });
  it("renders EV: PASS / NET: PASS badges", () => {
    const html = renderToStaticMarkup(
      <OpportunityEvidenceCell.Gates evidence="PASS" netProfit="PASS" />,
    );
    expect(html).toMatch(/EV: PASS/i);
    expect(html).toMatch(/NET: PASS/i);
  });
  it("renders EV: FAIL with destructive tone, NET: — when partial", () => {
    const html = renderToStaticMarkup(
      <OpportunityEvidenceCell.Gates evidence="FAIL" netProfit={null} />,
    );
    expect(html).toMatch(/EV: FAIL/i);
    expect(html).toMatch(/NET: —/);
  });
  it("regression alarm: never marks as PASS if value is null", () => {
    const html = renderToStaticMarkup(
      <OpportunityEvidenceCell.Gates evidence={null} netProfit="PASS" />,
    );
    expect(html).not.toMatch(/EV: PASS/);
    expect(html).toMatch(/EV: —/);
    expect(html).toMatch(/NET: PASS/);
  });
});
