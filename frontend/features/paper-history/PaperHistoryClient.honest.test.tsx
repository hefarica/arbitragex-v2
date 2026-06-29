import React from "react";
import { describe, it, expect } from "vitest";
import { renderToStaticMarkup } from "react-dom/server";

import { PaperHistoryClient } from "./PaperHistoryClient";

/**
 * These lock in the fail-honest distinction the FASE-3 increment-2 fix
 * introduces: a failed/degraded fetch must NEVER render as the healthy
 * "No paper trade runs yet" empty — it must surface the verbatim reason.
 */
describe("PaperHistoryClient — honest degraded vs empty", () => {
  it("SSR snapshot failure → DegradedBanner with verbatim reason, NOT the healthy empty", () => {
    const html = renderToStaticMarkup(
      <PaperHistoryClient initialData={{ history: null, summary: null, initialError: "history HTTP 503" }} />,
    );
    expect(html).toContain('data-testid="degraded-banner"');
    expect(html).toContain("Paper history unavailable");
    expect(html).toContain("history HTTP 503"); // verbatim upstream reason
    expect(html).not.toContain("No paper trade runs yet"); // must not fake a healthy empty
    expect(html).not.toContain('data-testid="paper-history-empty"');
  });

  it("degraded WITH stale data → 'showing last known' banner + the preserved table, not 'unavailable'", () => {
    const html = renderToStaticMarkup(
      <PaperHistoryClient
        initialData={{
          history: {
            ok: true,
            source: "postgres",
            count: 1,
            limit: 50,
            offset: 0,
            data: [
              {
                id: "r1",
                opportunity_id: null,
                route_hash: null,
                sim_expected_profit_usd: null,
                sim_gas_cost_usd: null,
                sim_net_profit_usd: null,
                strategy: "backrun",
                chain_id: 1,
                created_at: "2026-06-28T00:00:00Z",
              },
            ],
          },
          summary: null,
          initialError: "history HTTP 503",
        }}
      />,
    );
    expect(html).toContain('data-testid="degraded-banner"');
    expect(html).toContain("Degraded — showing last known paper history");
    expect(html).toContain("history HTTP 503");
    expect(html).toContain('data-testid="paper-history-table"'); // last-good preserved
    expect(html).not.toContain("Paper history unavailable");
    expect(html).not.toContain("No paper trade runs yet");
  });
});
