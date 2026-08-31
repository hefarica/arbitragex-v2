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

import { summarizeTickerError } from "../OpportunityTicker";

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
