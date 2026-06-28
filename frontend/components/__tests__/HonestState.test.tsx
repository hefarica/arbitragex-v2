import React from "react";
import { describe, it, expect } from "vitest";
import { renderToStaticMarkup } from "react-dom/server";

import { DegradedBanner } from "@/components/DegradedBanner";
import { SourceMeta } from "@/components/SourceMeta";

describe("DegradedBanner", () => {
  it("renders the default title + the VERBATIM upstream reason (never paraphrased)", () => {
    const html = renderToStaticMarkup(
      <DegradedBanner reason="503 upstream_unavailable" endpoint="GET /api/x" lastOk="13:12:48" />,
    );
    expect(html).toContain("Degraded — showing last known data");
    expect(html).toContain("503 upstream_unavailable"); // verbatim, not paraphrased
    expect(html).toContain("GET /api/x");
    expect(html).toContain("last good · 13:12:48");
    expect(html).toContain('data-testid="degraded-banner"');
    expect(html).toContain('role="status"');
  });

  it("omits the upstream line entirely when no reason is given", () => {
    const html = renderToStaticMarkup(<DegradedBanner />);
    expect(html).not.toContain("upstream:");
  });
});

describe("SourceMeta", () => {
  it("renders the real source label", () => {
    const html = renderToStaticMarkup(<SourceMeta source="edge" />);
    expect(html).toContain("source: edge");
    expect(html).toContain('data-testid="source-meta"');
  });

  it("hides freshness until a fetch timestamp exists (no fabricated 'updated 0s ago')", () => {
    const html = renderToStaticMarkup(<SourceMeta source="api-server" pollMs={5000} at={null} />);
    expect(html).toContain("source: api-server");
    // LastUpdated renders nothing while at == null → no freshness claim is invented.
    expect(html).not.toContain("ago");
  });
});
