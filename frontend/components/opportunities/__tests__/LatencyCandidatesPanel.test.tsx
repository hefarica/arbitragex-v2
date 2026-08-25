// frontend/components/opportunities/__tests__/LatencyCandidatesPanel.test.tsx
//
// FE-0037 (§45) — the per-candidate latency waterfall over the ARBX-FE-EMIT-09
// wire (lat_candidates + lat_candidates_meta). The three honest states agreed
// with d9 (cont.72/73) are each pinned: group-absent (pre-EMIT-09 backend),
// honest-empty-tick (sampled 0), and per-row reprice absence (R8: the key is
// ABSENT, never 0). Pure component → renderToStaticMarkup, no store seam.
import React from "react";
import { describe, it, expect } from "vitest";
import { renderToStaticMarkup } from "react-dom/server";

import {
  LatencyCandidatesPanel,
  fmtMs,
} from "../LatencyCandidatesPanel";
import type {
  LatCandidateRow,
  LatCandidatesTelemetry,
} from "@/lib/apex/schemas";

// ─── Fixtures (mirror shapes; totals coherent with Σ stages) ────────────────

const META_TRUNCATED = {
  attribution: { gates: "measured", reprice: "measured-upper-bound" },
  cap: 10,
  sampled: 12,
  truncated: true,
  dropped: 2,
} as const;

const META_EMPTY_TICK = {
  attribution: { gates: "measured", reprice: "measured-upper-bound" },
  cap: 10,
  sampled: 0,
  truncated: false,
  dropped: 0,
} as const;

const ROW_TRIANGULAR: LatCandidateRow = {
  route_hash: "a".repeat(64),
  route_kind: "triangular",
  hops: 3,
  stages: { gates_us: 1200, reprice_us: 350 },
  total_us: 1550,
};

const ROW_V2V2_NO_REPRICE: LatCandidateRow = {
  route_hash: "b".repeat(64),
  route_kind: "v2v2",
  hops: 2,
  stages: { gates_us: 800 }, // reprice ABSENT — never traversed the adapter
  total_us: 800,
};

function renderPanel(props: {
  rows?: LatCandidateRow[];
  meta?: LatCandidatesTelemetry["lat_candidates_meta"];
}): string {
  return renderToStaticMarkup(React.createElement(LatencyCandidatesPanel, props));
}

// ─── fmtMs ───────────────────────────────────────────────────────────────────

describe("fmtMs", () => {
  it("1200 µs → 1.20 ms", () => {
    expect(fmtMs(1200)).toBe("1.20 ms");
  });
  it("350 µs → 0.35 ms", () => {
    expect(fmtMs(350)).toBe("0.35 ms");
  });
});

// ─── State 1 — group absent (pre-EMIT-09 backend) ──────────────────────────

describe("LatencyCandidatesPanel — group absent", () => {
  it("renders the group gap: 'no emitido' + the ARBX-FE-EMIT-09 pointer", () => {
    const html = renderPanel({});
    expect(html).toContain("no emitido");
    expect(html).toContain("ARBX-FE-EMIT-09");
    // NOT an empty tick — the states are distinct.
    expect(html).not.toContain("0 candidatos en este tick");
  });
});

// ─── State 2 — honest empty tick ────────────────────────────────────────────

describe("LatencyCandidatesPanel — honest empty tick", () => {
  it("rows=[] renders '0 candidatos' as a REAL state, not an error", () => {
    const html = renderPanel({ rows: [], meta: META_EMPTY_TICK });
    expect(html).toContain("0 candidatos en este tick");
    expect(html).toContain("no un error");
    expect(html).not.toContain("no emitido");
  });

  it("counters still render from meta (sampled 0, no truncation badge)", () => {
    const html = renderPanel({ rows: [], meta: META_EMPTY_TICK });
    expect(html).toContain("sampled 0");
    expect(html).toContain("dropped 0");
    expect(html).not.toContain("top-K truncado");
  });
});

// ─── Full render — waterfall ────────────────────────────────────────────────

describe("LatencyCandidatesPanel — full waterfall", () => {
  const html = renderPanel({
    rows: [ROW_TRIANGULAR, ROW_V2V2_NO_REPRICE],
    meta: META_TRUNCATED,
  });

  it("frame honesty: declares NO per-opportunity join (route_hash gap)", () => {
    expect(html).toContain("NO hay join per-oportunidad");
    expect(html).toContain("nivel-(b)");
  });

  it("attribution literals render VERBATIM (producer vocabulary)", () => {
    // The value is span-wrapped (highlighted); assert the literal content
    // nodes so a producer-side vocabulary drift fails here.
    expect(html).toContain("attribution: gates=");
    expect(html).toContain("reprice=");
    expect(html).toContain(">measured<");
    expect(html).toContain(">measured-upper-bound<");
  });

  it("cut counters + truncation badge make the top-K recorte visible", () => {
    expect(html).toContain("sampled 12");
    expect(html).toContain("kept 10");
    expect(html).toContain("dropped 2");
    expect(html).toContain("cap 10");
    expect(html).toContain("top-K truncado");
  });

  it("row 0 (triangular): hash elided with title, kind chip, stage values ms", () => {
    // 8 + … + 6 elision over a 64-char hash of 'a's; full hash in the title.
    expect(html).toContain("aaaaaaaa…aaaaaa");
    expect(html).toContain(`title="${"a".repeat(64)}"`);
    expect(html).toContain("triangular · 3 hops");
    expect(html).toContain("1.20 ms"); // gates
    expect(html).toContain("0.35 ms"); // reprice
    expect(html).toContain("1.55 ms"); // total (Σ)
  });

  it("row 0: proportional bar widths derived from the row's total_us", () => {
    // gates 1200/1550 = 77%; reprice 350/1550 = 23%
    expect(html).toContain("width:77%");
    expect(html).toContain("width:23%");
  });

  it("row 1 (v2v2): reprice ABSENT renders '—' + the absence note, never 0", () => {
    expect(html).toContain("v2v2 · 2 hops");
    expect(html).toContain("no atravesó el adapter este tick");
    // total 800 µs = gates only
    expect(html).toContain("0.80 ms");
  });

  it("total carries the Σ-stages disclaimer (not the tick wall-clock)", () => {
    expect(html).toContain("total (Σ stages)");
    expect(html).toContain("NO wall-clock del tick");
  });

  it("R1: pure render is byte-identical across invocations", () => {
    const again = renderPanel({
      rows: [ROW_TRIANGULAR, ROW_V2V2_NO_REPRICE],
      meta: META_TRUNCATED,
    });
    expect(html).toBe(again);
  });
});

// ─── Defensive — meta absent while rows present (schema-partial window) ─────

describe("LatencyCandidatesPanel — meta absent, rows present", () => {
  it("renders the rows without fabricating counters", () => {
    const html = renderPanel({ rows: [ROW_V2V2_NO_REPRICE] });
    expect(html).toContain("v2v2 · 2 hops");
    expect(html).not.toContain("sampled");
    expect(html).not.toContain("dropped");
  });
});
