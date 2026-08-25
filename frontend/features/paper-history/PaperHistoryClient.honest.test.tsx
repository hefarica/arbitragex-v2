import React from "react";
import { describe, it, expect } from "vitest";
import { renderToStaticMarkup } from "react-dom/server";

import { PaperHistoryClient, type PaperTradeRow } from "./PaperHistoryClient";

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

/**
 * FE-0056 (§61) — Paper History as an evidence layer. The wire now carries the
 * identity/economics columns the contracts DO persist (reason mig 091;
 * cartridge_id mig 102; route_metadata mig 099; roi_pct). These pin:
 *  - present evidence renders verbatim;
 *  - absent evidence renders honest "—" (never 0, never fabricated — §28/R8);
 *  - net bps is a pure display unit conversion of roi_pct (×100, §79 — no recompute);
 *  - hop is derived from route_metadata.dex_adapters length (§26 single derivation);
 *  - the nivel-(b) gaps are DECLARED in a footnote, not silently omitted.
 */
describe("PaperHistoryClient — FE-0056 evidence layer (§61)", () => {
  const makeRow = (overrides: Partial<PaperTradeRow> = {}): PaperTradeRow => ({
    id: "r1",
    opportunity_id: null,
    route_hash: null,
    sim_expected_profit_usd: "1.23",
    sim_gas_cost_usd: "0.10",
    sim_net_profit_usd: "1.13",
    strategy: "flashloan_arb",
    chain_id: 1,
    created_at: "2026-08-23T00:00:00Z",
    ...overrides,
  });

  const renderRows = (rows: PaperTradeRow[]) =>
    renderToStaticMarkup(
      <PaperHistoryClient
        initialData={{
          history: { ok: true, source: "postgres", count: rows.length, limit: 50, offset: 0, data: rows },
          summary: null,
        }}
      />,
    );

  it("full-evidence row renders cartridge, hop (from route_metadata), net bps (roi_pct×100) and verbatim reason", () => {
    const html = renderRows([
      makeRow({
        id: "r-ev",
        opp_cartridge_id: "MEV-01-015",
        opp_route_metadata: {
          token_addresses: ["0xA", "0xB", "0xC", "0xD"],
          pool_addresses: ["p1", "p2", "p3"],
          dex_adapters: ["uniswap-v2", "sushi-v2", "uniswap-v3"],
        },
        opp_roi_pct: 0.0521,
        failure_reason: "gas_floor_breach",
      }),
    ]);
    expect(html).toContain("MEV-01-015");
    // hop = len(dex_adapters) = 3 — derived from the persisted topology, §26
    expect(html).toMatch(/<td[^>]*title="hop = len\(dex_adapters\)[^>]*>3</);
    // net bps = 0.0521 × 100 = 5.21 → "5.2" (display unit conversion, §79)
    expect(html).toContain("5.2");
    expect(html).toContain("net bps = roi_pct persistido ×100");
    expect(html).toContain("gas_floor_breach");
  });

  it("evidence-absent row (purged opportunity / pre-column row) renders honest '—', never 0", () => {
    const html = renderRows([makeRow({ id: "r-null" })]);
    // four evidence cells + their honest-null titles
    expect(html).toContain("cartridge_id NULL — oportunidad purgada o fila pre-mig-102 (R8)");
    expect(html).toContain("reason NULL ⇒ run aceptada (no rechazada) — mig 091");
    // net bps absent → "—" in its own cell, and no fabricated "0.0"
    expect(html).toMatch(/title="net bps = roi_pct[^"]*">\s*—</);
    expect(html).not.toContain(">0.0<");
  });

  it("route_metadata with EMPTY dex_adapters → hop '—' (absence is a state, not zero — R8)", () => {
    const html = renderRows([
      makeRow({
        id: "r-empty-adapters",
        opp_route_metadata: { token_addresses: [], pool_addresses: [], dex_adapters: [] },
      }),
    ]);
    expect(html).toMatch(/title="hop = len\(dex_adapters\)[^>]*>—</);
  });

  it("new evidence columns are declared in the header row", () => {
    const html = renderRows([makeRow()]);
    for (const h of ["Cartridge", "Hop", "Net bps", "Reason"]) {
      expect(html).toContain(`>${h}</th>`);
    }
  });

  it("nivel-(b) gaps are DECLARED in the footnote — not rendered as columns, not fabricated", () => {
    const html = renderRows([makeRow()]);
    // footnote present with every unpersisted field named + the doctrine refs
    expect(html).toContain('data-testid="paper-history-evidence-gaps"');
    for (const field of ["route_id", "detector_id", "quote_version", "graph_version", "config_version"]) {
      expect(html).toContain(field);
    }
    expect(html).toContain("no están persistidos");
    expect(html).toContain("nivel-(b)");
    expect(html).toContain("§79");
    expect(html).toContain("§26");
    // and none of them becomes a table column
    expect(html).not.toContain(">Detector</th>");
    expect(html).not.toContain(">Quote Version</th>");
  });

  it("R1: SSR render is deterministic (two renders byte-identical)", () => {
    const rows = [
      makeRow({ id: "r-a", opp_cartridge_id: "MEV-02-001", opp_roi_pct: 0.1, failure_reason: "impact_zero" }),
      makeRow({ id: "r-b" }),
    ];
    expect(renderRows(rows)).toBe(renderRows(rows));
  });
});
