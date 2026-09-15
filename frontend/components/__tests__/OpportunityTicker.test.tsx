// frontend/components/__tests__/OpportunityTicker.test.tsx
//
// DAPP-SURFACE-FAIL (a11y) regression — 2026-08-31.
//
// The ticker lives in the root layout and its error state used to dump the
// raw upstream error — including full Cloudflare 502 JSON bodies — into an
// aria-labeled region, unbounded. summarizeTickerError bounds it: HTTP status
// extraction, first-line collapse, 120-char cap. These tests pin that the
// worst observed payload (Cloudflare 502 JSON) renders as a bounded string.
import { describe, expect, it } from "vitest";

import { summarizeTickerError, opportunityToTickerItem, formatTickerYield } from "../OpportunityTicker";

const CF_502 =
  'edge HTTP 502: {"error":"HTTP 502","message":"error code: 502", ' +
  '"cloudflare_ray":"8f2a1b9c2d3e4f5a-ABC","trace":"worker fetch failed while awaiting ' +
  'upstream connection pool drain retry attempt 3/3 origin api-server:8080 reset"}';

describe("summarizeTickerError — DAPP-SURFACE-FAIL bound", () => {
  it("collapses a Cloudflare 502 JSON payload to the HTTP status", () => {
    expect(summarizeTickerError(CF_502)).toBe("edge HTTP 502");
  });

  it("caps non-HTTP errors at 120 chars", () => {
    const long = "x".repeat(500);
    const out = summarizeTickerError(long);
    expect(out.length).toBeLessThanOrEqual(120);
    expect(out.endsWith("…")).toBe(true);
  });

  it("passes a short plain error through unchanged", () => {
    expect(summarizeTickerError("fetch failed")).toBe("fetch failed");
  });

  it("never leaks JSON braces or ray ids into the ticker line", () => {
    const out = summarizeTickerError(CF_502);
    expect(out).not.toContain("{");
    expect(out).not.toContain("cloudflare_ray");
  });
});


describe("HOPS-PROVENANCE — ticker reports measured ROI and rejection state", () => {
  const record = (roi: number | null) => ({
    id: "fixture", chain_id: 1, strategy_kind: "dex_arb", dex_a: "unknown", dex_b: null,
    token_in: "0xaaaa", token_out: "0xbbbb", amount_in_wei: "42", pair_symbol: "A/B",
    expected_profit_usd: 7.6, net_expected_profit_usd: 7, roi_pct: roi,
    status: "rejected", rejection_reason: "TokenNotAllowed:fixture", trace_id: "fixture",
    detected_at: "2026-09-14T00:00:00Z", risk_score: null, block_number: null,
  }) as Parameters<typeof opportunityToTickerItem>[0];

  it("USD 7 without a denominator never becomes fabricated ROI 0.7%", () => {
    const item = opportunityToTickerItem(record(null));
    expect(item).not.toBeNull();
    expect(item!.yield).toBeNull();
    expect(formatTickerYield(item!.yield)).toBe("ROI —");
    expect(item!.status).toBe("rejected");
    expect(item!.rejectionReason).toBe("TokenNotAllowed:fixture");
  });
  it.each([0, -0.5, 2.5])("preserves an actual ROI of %s", (roi) => {
    expect(opportunityToTickerItem(record(roi))!.yield).toBe(roi);
    expect(formatTickerYield(roi)).toContain(`${roi >= 0 ? "+" : ""}${roi.toFixed(2)}%`);
  });
  it.each([NaN, Infinity, -Infinity])("non-finite ROI stays unavailable (%s)", (roi) => {
    expect(opportunityToTickerItem(record(roi))!.yield).toBeNull();
    expect(formatTickerYield(roi)).toBe("ROI —");
  });
  it("a recorded rejection overrides a stale detected status in the marquee", () => {
    expect(opportunityToTickerItem({ ...record(null), status: "detected" })!.status).toBe("rejected");
  });
});


describe("ticker lifecycle precedence matches the card",()=>{
  it.each([null,"build_error","timeout"])("failed is never rewritten as rejected (reason=%s)",(reason)=>{
    const opp={token_in:"A",token_out:"B",dex_a:"v2",dex_b:"v2",pair_symbol:"A/B",
      net_expected_profit_usd:0,expected_profit_usd:0,roi_pct:null,status:"failed",
      rejection_reason:reason,detected_at:"2026-09-14T00:00:00Z"} as Parameters<typeof opportunityToTickerItem>[0];
    const item=opportunityToTickerItem(opp);
    expect(item?.status).toBe("failed");
    expect(item?.rejectionReason).toBe(reason);
    expect(item?.yield).toBeNull();
  });
});

// PR569: a terminal failure before economics is evidence, not an empty feed.
describe("terminal ticker rows without calculated economics", () => {
  it.each(["rejected", "failed"])("retains %s with honest null profit and ROI", (status) => {
    const opp = { token_in: "A", token_out: "B", status,
      expected_profit_usd: null, net_expected_profit_usd: null, roi_pct: null,
      detected_at: "2026-09-14T00:00:00Z", rejection_reason: "missing_quote" } as Parameters<typeof opportunityToTickerItem>[0];
    const result = opportunityToTickerItem(opp);
    expect(result?.status).toBe(status);
    expect(result?.yield).toBeNull();
    expect(result?.rejectionReason).toBe("missing_quote");
    expect(formatTickerYield(result?.yield ?? null)).toBe("ROI —");
  });
  it("retains an explicit rejection even with stale detected status and no economics", () => {
    const opp = { token_in: "A", token_out: "B", status: "detected",
      expected_profit_usd: null, net_expected_profit_usd: null, roi_pct: null,
      detected_at: "2026-09-14T00:00:00Z", rejection_reason: "TokenNotAllowed" } as Parameters<typeof opportunityToTickerItem>[0];
    expect(opportunityToTickerItem(opp)?.status).toBe("rejected");
  });
});
