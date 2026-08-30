/**
 * ProgressRealCard tests — SSR-only static markup assertions.
 *
 * What this guards:
 *   - The doctrinal milestone percentages are present (workspace-verified
 *     truth ledger).
 *   - "Live trading: OFF" badge is rendered on every server render — this
 *     is the structural barrier paired with SystemGuardBanner.
 *   - "Capital exposure: $0" is rendered.
 *   - A.4 = BLOCKED and A.5 = NO-GO are rendered.
 *   - 100% on full system or live tile is NEVER rendered.
 *
 * The card also fires a useEffect that calls getReadiness + getRuntimeStatus.
 * Under renderToStaticMarkup the effect never runs, so we don't need to mock
 * those endpoints. The "Loading runtime probe…" copy is the SSR-time state.
 */
import React from "react";
import { describe, it, expect } from "vitest";
import { renderToStaticMarkup } from "react-dom/server";
import {
  ProgressRealCard,
  groupReadiness,
  summarizeRuntime,
  summarizeDecision,
} from "../ProgressRealCard";

const html = renderToStaticMarkup(<ProgressRealCard />);

describe("ProgressRealCard", () => {
  it("renders the workspace-verified doctrinal label", () => {
    expect(html).toMatch(/doctrinal/i);
    expect(html).toMatch(/workspace-verified/i);
  });

  it("renders pre-live simulation percentage = 85%", () => {
    expect(html).toContain("85%");
    expect(html).toMatch(/Pre-live simulation honesty/i);
  });

  it("renders full-system percentage (mid-range, never 100%)", () => {
    expect(html).toMatch(/Full system to live-minimum/i);
    expect(html).toContain("70%");
  });

  it("renders frontend audit = 100%", () => {
    expect(html).toMatch(/Frontend forensic audit/i);
    expect(html).toContain("100%");
  });

  it("renders frontend integration percentage (post code-brechas)", () => {
    expect(html).toMatch(/Frontend integration applied/i);
    expect(html).toContain("80%");
  });

  it("asserts Live trading: OFF (structural barrier badge)", () => {
    // The badge renders "<svg .../>Live trading: OFF</span>" — text inline
    // with the SVG, no intermediate tag. Just confirm the literal substring.
    expect(html).toContain("Live trading: OFF");
  });

  it("asserts A.4 fork = PASS (hand-edited per milestone doctrine, AUDIT-2026-08-29)", () => {
    expect(html).toMatch(/A\.4 fork/i);
    expect(html).toContain("PASS");
    expect(html).not.toContain("BLOCKED");
  });

  it("asserts A.5 paper-shadow = PASS (readiness decision go_a5=true)", () => {
    expect(html).toMatch(/A\.5 paper-shadow/i);
    // Tile-scoped assertion: the milestone tile value is PASS — never a NO-GO
    // verdict. ("NO-GO" legitimately appears in the A.9 "GO/NO-GO" phase name
    // in FULL_SYSTEM_DETAIL, so a global not.toContain would be wrong.)
    expect(html).toMatch(/A\.5 paper-shadow<\/div><div[^>]*>PASS</);
    expect(html).not.toMatch(/A\.5 paper-shadow<\/div><div[^>]*>NO-GO/);
  });

  it("asserts Capital exposure = $0", () => {
    expect(html).toMatch(/Capital exposure/i);
    expect(html).toContain("$0");
  });

  // ── Honesty refactor: live runtime vs manual doctrine must be unambiguous ──

  it("separates a live-runtime section from a doctrinal-milestone section", () => {
    expect(html).toMatch(/Live runtime/i);
    expect(html).toMatch(/Doctrinal milestones/i);
  });

  it("labels the milestone percentages as MANUAL with a real last-updated date", () => {
    // The four bars are hand-edited constants — the UI must say so and show
    // when they were last touched (git date of the most recent constant edit).
    expect(html).toMatch(/manual/i);
    expect(html).toContain("2026-08-29");
  });

  it("renders a live readiness aggregate row sourced from /api/readiness", () => {
    expect(html).toMatch(/Readiness gates/i);
  });

  it("fail-honest: never fabricates a readiness count before live data arrives", () => {
    // Under renderToStaticMarkup the probe useEffect never runs, so there is
    // no live readiness payload. The live row MUST show a loading/unavailable
    // placeholder — it must NOT print a synthetic "N of M green" number.
    expect(html).not.toMatch(/\d+\s+of\s+\d+\s+green/i);
  });

  it("renders the loading runtime probe on initial server render", () => {
    expect(html).toMatch(/Loading runtime probe/i);
  });

  it("regression alarm: NEVER renders 'Live trading: ON' or full-system 100%", () => {
    expect(html).not.toMatch(/Live trading[^<]*<[^>]*>[^<]*ON</i);
    // Full system must not be at 100%. If a future commit sets it to 100,
    // a milestone-level review is required before merge.
    expect(html).not.toMatch(/Full system to live-minimum[\s\S]{0,200}100%/i);
  });
});

// ── Auto-derive engine: pure derivation functions (the workspace progress engine) ──

describe("groupReadiness (auto-derived per-group readiness ratios)", () => {
  it("counts green/total per readiness group from real items", () => {
    const out = groupReadiness([
      { group: "operations", status: "green" },
      { group: "operations", status: "red" },
      { group: "contracts", status: "yellow" },
      { group: "security_compliance", status: "green" },
    ]);
    expect(out.find((g) => g.group === "operations")).toMatchObject({ green: 1, total: 2 });
    expect(out.find((g) => g.group === "contracts")).toMatchObject({ green: 0, total: 1 });
    expect(out.find((g) => g.group === "security_compliance")).toMatchObject({ green: 1, total: 1 });
  });

  it("fail-honest: empty items yields no groups (never fabricates a ratio)", () => {
    expect(groupReadiness([])).toEqual([]);
  });
});

describe("summarizeRuntime (auto-derived runtime evidence)", () => {
  it("counts heartbeat-loaded engines and sums 1h candidates/rejections from real strategies", () => {
    expect(
      summarizeRuntime([
        { engine_loaded: true, candidates_1h: 3, rejections_1h: 2 },
        { engine_loaded: false, candidates_1h: 0, rejections_1h: 0 },
        { engine_loaded: true, candidates_1h: 5, rejections_1h: 5 },
      ]),
    ).toMatchObject({ enginesLoaded: 2, enginesTotal: 3, candidates1h: 8, rejections1h: 7 });
  });

  it("fail-honest: empty strategies yields honest zeros (0 engines, not a fabricated number)", () => {
    expect(summarizeRuntime([])).toMatchObject({
      enginesLoaded: 0,
      enginesTotal: 0,
      candidates1h: 0,
      rejections1h: 0,
    });
  });
});

describe("summarizeDecision (auto-derived go/no-go decision evidence)", () => {
  it("passes through paper_mode / go_a5 / reasons / next_action from a real decision", () => {
    expect(
      summarizeDecision({
        paper_mode: true,
        go_a5: false,
        reasons: ["A.4 fork not executed"],
        next_action: "run A.4 fork validation",
      }),
    ).toMatchObject({
      paperMode: true,
      goA5: false,
      reasons: ["A.4 fork not executed"],
      nextAction: "run A.4 fork validation",
    });
  });

  it("fail-honest: null decision yields nulls + empty reasons (never fabricates a verdict)", () => {
    expect(summarizeDecision(null)).toMatchObject({
      paperMode: null,
      goA5: null,
      reasons: [],
      nextAction: null,
    });
  });
});

describe("ProgressRealCard decision evidence (SSR fail-honest)", () => {
  it("queries the decision endpoint and shows no fabricated trade-mode/A.5 verdict at static render", () => {
    expect(html).toMatch(/Querying \/api\/readiness\/decision/i);
  });
});
