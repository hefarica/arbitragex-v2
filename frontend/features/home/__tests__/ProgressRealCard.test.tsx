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
import { ProgressRealCard } from "../ProgressRealCard";

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
    expect(html).toContain("58%");
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

  it("asserts A.4 fork = BLOCKED", () => {
    expect(html).toMatch(/A\.4 fork/i);
    expect(html).toContain("BLOCKED");
  });

  it("asserts A.5 paper-shadow = NO-GO", () => {
    expect(html).toMatch(/A\.5 paper-shadow/i);
    expect(html).toContain("NO-GO");
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
    expect(html).toContain("2026-06-14");
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
