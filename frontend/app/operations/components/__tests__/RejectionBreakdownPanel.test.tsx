// frontend/app/operations/components/__tests__/RejectionBreakdownPanel.test.tsx
//
// REJECT-BREAKDOWN-EXPORT-01 — view + CSV builder tests.
//
// Repo pattern (ByModeKpiStrip.test.tsx): the frontend test env is `node`
// (no jsdom) — the pure presentational view renders to static HTML via
// react-dom/server and the deterministic branches assert. buildCsv is a pure
// function and is unit-tested directly.
//
//   - ready view: families with counts/share/avg, flood with resolved symbol
//   - honest nulls (R8): avg_net null → "—", unknown symbol → "—" with the
//     raw address preserved in the title attribute
//   - fetch error renders verbatim; no data → "Cargando…" + disabled CSV
//   - the active window button carries aria-pressed="true"
//   - buildCsv: both sections, nulls as EMPTY fields (never 0), header meta
import React from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";

import { buildCsv, RejectionBreakdownView } from "../RejectionBreakdownPanel";
import type { RejectionBreakdown } from "@/lib/api-client";

const BREAKDOWN: RejectionBreakdown = {
  ok: true,
  kind: "rejection_breakdown",
  window_hours: 24,
  chain_id: null,
  generated_at: "2026-09-06T02:30:00.000Z",
  total_rows: 48397,
  rejected_rows: 42260,
  raw_groups_truncated: false,
  families: [
    {
      family: "token_not_allowed",
      count: 37856,
      share_pct_of_rejected: 89.6,
      avg_gross_usd: 10.5,
      avg_net_usd: null,
      top_raw: [{ reason: "TokenNotAllowed:0x3235", count: 20888 }],
    },
    {
      family: "gas_floor_breach",
      count: 1904,
      share_pct_of_rejected: 4.5,
      avg_gross_usd: 3,
      avg_net_usd: 0.5,
      top_raw: [{ reason: "gas_floor_breach", count: 1904 }],
    },
  ],
  token_flood: [
    { address: "0x32353a6c91143bfd6c7d363b546e62a9a2489a20", symbol: "AGLD", count: 20888 },
    { address: "0x06450dee7fd2fb8e39061434babcfc05599a6fb8", symbol: null, count: 16968 },
  ],
};

const noop = () => {};

function renderView(p: Partial<Parameters<typeof RejectionBreakdownView>[0]>) {
  return renderToStaticMarkup(
    <RejectionBreakdownView
      data={null}
      error={null}
      hours={24}
      onHoursChange={noop}
      onDownloadCsv={noop}
      {...p}
    />,
  );
}

describe("RejectionBreakdownView", () => {
  it("renders families with count, share and averages, and the flood symbols", () => {
    const html = renderView({ data: BREAKDOWN });
    expect(html).toContain("token_not_allowed");
    expect(html).toContain("37,856");
    expect(html).toContain("89.6%");
    expect(html).toContain("$10.50");
    expect(html).toContain("gas_floor_breach");
    expect(html).toContain("AGLD");
    expect(html).toContain("42,260 rechazadas de 48,397 oportunidades");
  });

  it("honest nulls — avg_net renders —, unknown symbol renders — with raw address in title", () => {
    const html = renderView({ data: BREAKDOWN });
    expect(html).toContain("—"); // R8: null is a dash, never 0
    // unknown token keeps its address reachable via the title attribute
    expect(html).toContain('title="0x06450dee7fd2fb8e39061434babcfc05599a6fb8"');
  });

  it("renders the fetch error verbatim", () => {
    const html = renderView({ error: "query_failed" });
    expect(html).toContain("query_failed");
    expect(html).toContain('role="alert"');
  });

  it("no data → loading line + disabled CSV button", () => {
    const html = renderView({});
    expect(html).toContain("Cargando…");
    // disabled button in static markup carries the disabled attribute
    expect(html).toMatch(/Descargar CSV[\s\S]*?disabled|disabled[\s\S]*?Descargar CSV/);
  });

  it("marks exactly one window button active (aria-pressed, count form)", () => {
    const html = renderView({ data: BREAKDOWN, hours: 168 });
    // repo pattern (ByModeKpiStrip aria-current): count, never adjacency.
    expect(html.match(/aria-pressed="true"/g)).toHaveLength(1);
    expect(html.match(/aria-pressed="false"/g)).toHaveLength(2);
    expect(html).toContain("24 h");
    expect(html).toContain("7 d");
    expect(html).toContain("30 d");
  });

  it("surfaces the raw_groups_truncated honesty warning (R8)", () => {
    const html = renderView({ data: { ...BREAKDOWN, raw_groups_truncated: true } });
    expect(html).toContain("Más de 500 razones crudas");
  });
});

describe("buildCsv", () => {
  it("emits both sections with header meta and EMPTY fields for nulls (never 0)", () => {
    const csv = buildCsv(BREAKDOWN);
    const lines = csv.split("\r\n");
    expect(lines[0]).toContain("window_hours=24");
    expect(lines[0]).toContain("generated_at=2026-09-06T02:30:00.000Z");
    expect(lines[2]).toBe("section,family_or_token,count,share_pct_of_rejected,avg_gross_usd,avg_net_usd");
    // family row: avg_net null → empty field, not 0
    const floodRow = lines.find((l) => l.startsWith("family,token_not_allowed"));
    expect(floodRow).toBe("family,token_not_allowed,37856,89.6,10.5,");
    // token flood rows carry symbol (or raw address when unknown)
    expect(csv).toContain("token_flood,AGLD,20888,,,");
    expect(csv).toContain("token_flood,0x06450dee7fd2fb8e39061434babcfc05599a6fb8,16968,,,");
  });

  it("starts with a UTF-8 BOM so Excel decodes non-ASCII symbols", () => {
    const csv = buildCsv(BREAKDOWN);
    expect(csv.charCodeAt(0)).toBe(0xfeff);
    expect(csv[1]).toBe("#");
  });

  it("neutralizes CSV formula injection (CWE-1236) from on-chain symbols", () => {
    const malicious: RejectionBreakdown = {
      ...BREAKDOWN,
      token_flood: [
        { address: "0x" + "c".repeat(40), symbol: "=HYPERLINK(\"http://evil\",\"click\")", count: 1 },
        { address: "0x" + "d".repeat(40), symbol: "+SUM(1,2)", count: 2 },
        { address: "0x" + "e".repeat(40), symbol: "-2+1|cmd", count: 3 },
        { address: "0x" + "f".repeat(40), symbol: "@x", count: 4 },
        { address: "0x" + "1".repeat(40), symbol: "AGLD", count: 5 },
      ],
    };
    const csv = buildCsv(malicious);
    // every dangerous cell gains the leading apostrophe; benign ones untouched
    expect(csv).toContain("'=HYPERLINK");
    expect(csv).toContain("'+SUM(1,2)");
    expect(csv).toContain("'-2+1|cmd");
    expect(csv).toContain("'@x");
    expect(csv).toContain("token_flood,AGLD,5,,,");
    // and no UNQUOTED dangerous cell survives
    expect(csv).not.toMatch(/(^|[\r\n,])=(HYPERLINK|SUM)/);
  });

  it("carries the truncation honesty comment when raw_groups_truncated", () => {
    const csv = buildCsv({ ...BREAKDOWN, raw_groups_truncated: true });
    expect(csv).toContain("# raw_groups_truncated=true");
  });
});
