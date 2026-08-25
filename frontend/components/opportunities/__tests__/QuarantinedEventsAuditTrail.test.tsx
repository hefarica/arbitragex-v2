// frontend/components/opportunities/__tests__/QuarantinedEventsAuditTrail.test.tsx
//
// FE-0032 (§31) — the audit-trail subsection contract: the quarantined rows
// of the CURRENT snapshot in the §31 columns. Fixtures go through the real
// mapper so `semantic_violations` is COMPUTED by validateOpportunitySemantics
// (integration, never hand-set). Gaps (candidate_id / payload version) are
// declared columns that stay "no emitido" — they exist for the day the wire
// carries them, never as fabricated values.
import React from "react";
import { describe, it, expect } from "vitest";
import { renderToStaticMarkup } from "react-dom/server";

import {
  QuarantinedEventsAuditTrail,
  AUDIT_TRAIL_LIMIT,
  NOT_EMITTED,
} from "../QuarantinedEventsAuditTrail";
import { mapToOmniOpportunity } from "@/lib/store/types";

const wire = (over: Record<string, unknown>) => ({
  id: "id",
  chain_id: 1,
  strategy_kind: "dex_arb",
  detected_at: "2026-08-11T00:00:00Z",
  status: "detected",
  trace_id: "trace-abcdef123456",
  dex_a: "uniswap-v2",
  dex_b: "sushiswap",
  token_in: "0xa",
  token_out: "0xb",
  block_number: 123,
  rejection_reason: "cartridge_gate:addr",
  ...over,
});

const rm2hop = {
  dex_adapters: ["uniswap-v2", "sushiswap"],
  token_addresses: ["0xa", "0xb", "0xa"],
  pool_addresses: ["0xpool1", "0xpool2"],
};

// A row the validator QUARANTINES: no block anchor + non-numeric profit.
const quarantined = (id: string) =>
  mapToOmniOpportunity(
    wire({ id, block_number: undefined, net_expected_profit_usd: "oops" }),
  );
// A clean wire-grade row: zero violations.
const clean = (id: string) =>
  mapToOmniOpportunity(wire({ id, route_metadata: rm2hop }));

describe("QuarantinedEventsAuditTrail (§31)", () => {
  it("empty snapshot renders the COMPUTED zero — not a fabricated all-clear", () => {
    const html = renderToStaticMarkup(
      React.createElement(QuarantinedEventsAuditTrail, { opportunities: [] }),
    );
    expect(html).toContain("0 eventos en cuarentena en este snapshot");
    expect(html).toContain("recuento computado");
  });

  it("a clean snapshot (no violations) renders the same computed zero", () => {
    const html = renderToStaticMarkup(
      React.createElement(QuarantinedEventsAuditTrail, {
        opportunities: [clean("1"), clean("2")],
      }),
    );
    expect(html).toContain("0 eventos en cuarentena en este snapshot");
    expect(html).not.toContain("<th");
  });

  it("lists ONLY quarantined rows with the §31 columns wired to real fields", () => {
    const html = renderToStaticMarkup(
      React.createElement(QuarantinedEventsAuditTrail, {
        opportunities: [clean("1"), quarantined("2")],
      }),
    );
    // the §31 column headers exist
    for (const h of [
      "Timestamp",
      "candidate_id",
      "Reason",
      "Source",
      "Payload ver.",
      "Strategy",
      "Route",
      "Block",
      "Errores (§30)",
    ]) {
      expect(html).toContain(h);
    }
    // the quarantined row's real fields surface verbatim
    expect(html).toContain("2026-08-11T00:00:00Z"); // timestamp
    expect(html).toContain("cartridge_gate:addr"); // reason
    expect(html).toContain("trace-abcd…"); // source (short form, 10 chars)
    expect(html).toContain("dex_arb"); // strategy
    expect(html).toContain("uniswap-v2 → sushiswap"); // route
    // block + profit violations as §30 codes
    expect(html).toContain("missing_block · profit_not_numeric");
    // the clean row's id is not listed
    expect(html).not.toContain('key="1"');
  });

  it("gap columns stay 'no emitido' — candidate_id / payload version", () => {
    const html = renderToStaticMarkup(
      React.createElement(QuarantinedEventsAuditTrail, {
        opportunities: [quarantined("2")],
      }),
    );
    expect(html).toContain(NOT_EMITTED);
    // disclosed once in the caption too
    expect(html).toContain("nivel-(b)");
  });

  it("hops come from route_metadata only — null stays a dash, never synthetic 2", () => {
    // quarantined row WITHOUT route_metadata: hop_count is null (FE-0028) and
    // the §29 synthetic legs must NOT feed the audit route cell.
    const html = renderToStaticMarkup(
      React.createElement(QuarantinedEventsAuditTrail, {
        opportunities: [quarantined("2")],
      }),
    );
    expect(html).toContain("(hops —)");
    expect(html).not.toContain("(2 hops)");
  });

  it("cap: 25 rows shown, +N disclosed — no silent truncation", () => {
    const many = Array.from({ length: AUDIT_TRAIL_LIMIT + 7 }, (_, i) =>
      quarantined(String(i)),
    );
    const html = renderToStaticMarkup(
      React.createElement(QuarantinedEventsAuditTrail, { opportunities: many }),
    );
    expect(html).toContain(`+7 evento(s)`);
    expect(html).toContain(`cap ${AUDIT_TRAIL_LIMIT}`);
  });

  it("R1: pure render is byte-identical across two invocations", () => {
    const opps = [clean("1"), quarantined("2")];
    const a = renderToStaticMarkup(
      React.createElement(QuarantinedEventsAuditTrail, { opportunities: opps }),
    );
    const b = renderToStaticMarkup(
      React.createElement(QuarantinedEventsAuditTrail, { opportunities: opps }),
    );
    expect(a).toBe(b);
  });
});
